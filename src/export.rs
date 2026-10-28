//! The export: an edit's cuts rendered with their sound into one video, or a file per cut
//! for another editor (`Target`), at a `Resolution`.

use crate::{dissolve, ffmpeg, overlay, player, project, stab};
use crate::pipeline::{Shot, encode, Pipeline, StitchOpts};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
pub(crate) enum Resolution {
    #[value(name = "720p")]
    #[serde(rename = "720p")]
    P720,
    #[value(name = "1080p")]
    #[serde(rename = "1080p")]
    P1080,
    #[value(name = "1440p")]
    #[serde(rename = "1440p")]
    P1440,
    #[value(name = "2160p", alias = "4k")]
    #[serde(rename = "2160p")]
    P2160,
}

impl Resolution {
    pub(crate) const ALL: [Resolution; 4] = [Resolution::P720, Resolution::P1080, Resolution::P1440, Resolution::P2160];

    /// Pixels on the picture's short side.
    pub(crate) fn short_side(self) -> u32 {
        match self {
            Resolution::P720 => 720,
            Resolution::P1080 => 1080,
            Resolution::P1440 => 1440,
            Resolution::P2160 => 2160,
        }
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}p", self.short_side())
    }
}

/// Renders every cut of `project` back to back into one MP4 at `size`,
/// with their sound. Each clip is set up once however many cuts use it.
/// `progress(done, total)` hears of every frame; an error from it (a
/// cancel) stops the export and removes the unfinished file.
/// Sets up each cut's clip once per clip and stabilisation (the cut's own
/// wins over the command's): the pipelines, and per cut the index of its own.
pub(crate) fn cut_pipelines(project: &mut project::Project, opts: &StitchOpts, proxy: bool) -> Result<(Vec<Pipeline>, Vec<usize>)> {
    let mut pipelines: Vec<Pipeline> = Vec::new();
    let mut of_cut = Vec::with_capacity(project.cuts.len());
    for cut in &mut project.cuts {
        let mode = cut.stabilize.or(opts.stabilize).unwrap_or(stab::Mode::Horizon);
        let i = match pipelines.iter().position(|p| p.input == cut.input && p.stab.mode() == mode) {
            Some(i) => i,
            None => {
                let mut p = Pipeline::new(&cut.input, &StitchOpts { stabilize: Some(mode), ..opts.clone() }, mode)?;
                if proxy {
                    p.use_proxy();
                }
                pipelines.push(p);
                pipelines.len() - 1
            }
        };
        cut.flat = pipelines[i].flat.is_some();
        if let Err(e) = cut.resolve_lut() {
            eprintln!("warning: {e:#}; {} is left as filmed", cut.input.display());
        }
        of_cut.push(i);
    }
    // The edit runs at its first 360 clip's rate (its first video's if it
    // has none, else 30 fps); clips at another rate, Osmo clips too (the
    // 100 fps mode beside 30 fps ones), are decoded at it, frames repeated
    // or dropped, so they keep their speed and stay in step, and pictures
    // are repeated at it. A 100 fps cut in a 30 fps edit came out 200
    // frames for 2 s, played 3.3 times slow.
    let fps = pipelines.iter().find(|p| p.flat.is_none()).or(pipelines.iter().find(|p| p.flat.is_some_and(|f| !f.picture))).map_or(30.0, |p| p.fps);
    for p in pipelines.iter_mut().filter(|p| p.flat.is_some_and(|f| f.picture) || (p.fps - fps).abs() > 0.01) {
        p.rate = Some(fps);
        p.fps = fps;
    }
    Ok((pipelines, of_cut))
}

/// How and where `export` writes.
pub(crate) struct ExportOpts {
    pub size: (usize, usize),
    /// Stitch from the proxies: quick and soft.
    pub draft: bool,
    /// Stitch on the CPU, not the GPU.
    pub cpu: bool,
    pub format: ffmpeg::Format,
    pub target: Target,
}

pub(crate) enum Target {
    /// The whole edit in one file.
    One(PathBuf),
    /// A file per cut in `dir`, named `<name> - 001.<ext>` and on, for an
    /// external editor, and a WAV per music clip. Each runs `handles` seconds
    /// longer either side where its clip has the footage, for that editor's
    /// own transitions.
    Clips { dir: PathBuf, name: String, handles: f64, graded: bool },
}

/// Renders `project` as `out` says, with sound, and returns the files
/// written. Each clip is set up once however many cuts use it.
/// `progress(done, total)` hears of every frame; an error from it (a
/// cancel) stops the export and removes the unfinished file.
pub(crate) fn export(project: &project::Project, opts: &StitchOpts, out: &ExportOpts, progress: &mut dyn FnMut(usize, usize) -> Result<()>) -> Result<Vec<PathBuf>> {
    if project.cuts.is_empty() {
        bail!("nothing to export yet: the project has music but no video cuts");
    }
    let mut project = project.clone();
    let (pipelines, of_cut) = cut_pipelines(&mut project, opts, out.draft)?;
    let project = &project;
    // The shader always draws both lenses, so `--only` needs the CPU.
    let mut gpu = if out.cpu || opts.only.is_some() {
        None
    } else {
        // The Resolve format is 10-bit, so 16-bit frames from the GPU.
        match player::render::Gpu::new(out.format == ffmpeg::Format::Resolve) {
            Ok(gpu) => {
                eprintln!("stitching on {}", gpu.name);
                Some(gpu)
            }
            Err(e) => {
                eprintln!("stitching on the CPU: {e:#}");
                None
            }
        }
    };
    let clips: Vec<_> = if gpu.is_some() { pipelines.iter().map(player::render::Clip::new).collect::<Result<_>>()? } else { Vec::new() };
    let handles = match out.target {
        Target::One(_) => 0.0,
        Target::Clips { handles, .. } => handles.max(0.0),
    };
    // Per cut: its clip's pipeline, start (on a frame) and frame count.
    let mut plan = Vec::with_capacity(project.cuts.len());
    for (cut, &i) in project.cuts.iter().zip(&of_cut) {
        let p = &pipelines[i];
        let from = (cut.start - handles).max(0.0);
        let end = (cut.end + handles).min(ffmpeg::duration(&cut.input)?);
        if end <= from {
            bail!("the cut from {} s starts after {} ends", cut.start, cut.input.display());
        }
        // Count frames over the span asked for, not from the frame the start
        // snaps to: snapping 14 s forward to 14.014 left `-d 5` at 149 frames.
        plan.push((i, p.snap(from), ((end - from) * p.fps).round().max(1.0) as usize));
    }
    let fps = pipelines[0].fps;
    if pipelines.iter().any(|p| (p.fps - fps).abs() > 0.01) {
        eprintln!("the clips' frame rates differ; exporting at {fps:.3} fps");
    }
    // Frames each cut shares with the one before: its crossfade, in the
    // whole edit only (a file per cut is for another editor's own), and
    // never more than either cut has to give.
    let mut overlap = vec![0usize; plan.len()];
    if matches!(out.target, Target::One(_)) {
        for n in 1..plan.len() {
            let room = (plan[n - 1].2 - overlap[n - 1]).min(plan[n].2).saturating_sub(1);
            overlap[n] = ((project.cuts[n].crossfade * fps).round() as usize).min(room);
        }
    }
    // Each file and the cuts it holds.
    let files: Vec<(PathBuf, std::ops::Range<usize>)> = match &out.target {
        Target::One(path) => vec![(path.clone(), 0..plan.len())],
        Target::Clips { dir, name, .. } => {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            (0..plan.len()).map(|k| (dir.join(format!("{name} - {:03}.{}", k + 1, out.format.extension())), k..k + 1)).collect()
        }
    };
    let total = plan.iter().map(|p| p.2).sum();
    let mut done = 0;
    let mut written = Vec::with_capacity(files.len());
    // A cut's own audio file plays in place of its clip's sound (in a file
    // per cut too: it's the sound chosen for it); one that's gone leaves the
    // clip's, as the player does.
    let heard: Vec<Option<&Path>> = project.cuts.iter().map(|c| c.audio.as_deref().filter(|a| a.exists())).collect();
    for (c, h) in project.cuts.iter().zip(&heard) {
        if let (Some(audio), None) = (&c.audio, h) {
            eprintln!("{} is missing: its cut gets its clip's own sound", audio.display());
        }
    }
    for (path, cuts) in files {
        let sound: Vec<_> = cuts
            .clone()
            .zip(&plan[cuts.clone()])
            .map(|(n, &(i, start, frames))| {
                let crossfade = if n > cuts.start { overlap[n] as f64 / fps } else { 0.0 };
                let input = heard[n].unwrap_or(&pipelines[i].input);
                ffmpeg::Sound { input, start, duration: Some(frames as f64 / fps), volume: &project.cuts[n].volume, gain: project.cuts[n].sound_db(), even: project.cuts[n].even, crossfade }
            })
            .collect();
        // The whole edit gets its mix; a file per cut is for another editor,
        // so its sound is left as recorded and the music goes beside it.
        let length: f64 = cuts.clone().map(|n| (plan[n].2 - if n > cuts.start { overlap[n] } else { 0 }) as f64 / fps).sum();
        let mix = ffmpeg::Mix { sound_db: project.sound.volume, tracks: project::SoundTrack::ALL.map(|t| project.track(t)).to_vec(), length, fades: project.fades };
        let mix = matches!(out.target, Target::One(_)).then_some(&mix);
        let mut encoder = ffmpeg::Encoder::new(&path, out.size, fps, &sound, mix, out.format, out.format == ffmpeg::Format::Resolve && gpu.is_some())?;
        // Overlays over the whole edit only; a file per cut has none.
        let lanes = if matches!(out.target, Target::One(_)) { project.overlays.iter().chain(&project.text).collect() } else { Vec::new() };
        let mut burn = overlay::Burn::new(&mut encoder, lanes, out.size, fps, project.fades, project.duration())?;
        let mut sink = dissolve::Dissolve::new(&mut burn);
        for (n, &(i, start, frames)) in cuts.clone().zip(&plan[cuts.clone()]) {
            let tail = if n + 1 < cuts.end { overlap[n + 1] } else { 0 };
            let head = if n > cuts.start { overlap[n] } else { 0 };
            sink.cut(frames, head, tail)?;
            let size = out.size;
            let mut tick = |n: usize| progress(done + n, total);
            // Framed by its keyframes and graded by its effects (in the whole
            // edit, faded to black at its ends; clips graded only if asked).
            let (whole, graded) = match out.target {
                Target::One(_) => (true, true),
                Target::Clips { graded, .. } => (false, graded),
            };
            let camera = |t: f64| {
                let mut c = project::camera_at(&project.cuts, n, t, &|f, t| pipelines[i].aim(f, t));
                if !graded {
                    // Ungraded clips are for grading elsewhere: as filmed,
                    // so D-Log M stays log for the editor's own LUT.
                    c.grade = Default::default();
                    c.lut = None;
                } else if whole {
                    c.grade.black = project.black_at(n, t);
                }
                c
            };
            let result = match &mut gpu {
                Some(gpu) => player::render::encode(gpu, &pipelines[i], &clips[i], start, frames, &mut sink, camera, size, &mut tick),
                None => encode(&pipelines[i], start, Some(frames), &mut sink, |t| Shot::View { camera: Box::new(camera(t)), size }, &mut tick),
            };
            if let Err(e) = result {
                drop(sink);
                drop(burn);
                encoder.abort();
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
            done += frames;
        }
        drop(sink);
        drop(burn);
        encoder.finish()?;
        written.push(path);
    }
    // Beside the clips, each music and sound-effect clip as it is in its
    // file, for the other editor to level and fade: with handles, but none
    // of the mix.
    if let Target::Clips { dir, name, .. } = &out.target {
        for which in project::SoundTrack::ALL {
            let mut clips: Vec<_> = project.track(which).clips.iter().collect();
            clips.sort_by(|a, b| a.at.total_cmp(&b.at));
            for (k, clip) in clips.into_iter().enumerate() {
                let path = dir.join(format!("{name} - {} {:03}.wav", which.file_word(), k + 1));
                let from = (clip.start - handles).max(0.0);
                let end = (clip.end + handles).min(ffmpeg::duration(&clip.input)?);
                ffmpeg::wav(&clip.input, from, end - from, &path)?;
                written.push(path);
            }
        }
    }
    Ok(written)
}
