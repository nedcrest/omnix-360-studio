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
    /// The whole film's sound brought to this loudness, LUFS integrated
    /// (`loudness::bring_to`); a file per cut is left as it is.
    pub loudness: Option<f64>,
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

/// Per cut: its clip's pipeline, its start (on a frame, `handles` early)
/// and how many frames.
fn frame_plan(project: &project::Project, pipelines: &[Pipeline], of_cut: &[usize], handles: f64) -> Result<Vec<(usize, f64, usize)>> {
    project
        .cuts
        .iter()
        .zip(of_cut)
        .map(|(cut, &i)| {
            let p = &pipelines[i];
            let from = (cut.start - handles).max(0.0);
            let end = (cut.end + handles).min(ffmpeg::duration(&cut.input)?);
            if end <= from {
                bail!("the cut from {} s starts after {} ends", cut.start, cut.input.display());
            }
            // Count frames over the span asked for, not from the frame the
            // start snaps to: snapping 14 s forward to 14.014 left `-d 5` at
            // 149 frames.
            Ok((i, p.snap(from), ((end - from) * p.fps).round().max(1.0) as usize))
        })
        .collect()
}

/// Each file the export writes and the cuts it holds: one for the whole
/// edit, or one per cut in a folder made for them.
fn files(target: &Target, format: ffmpeg::Format, cuts: usize) -> Result<Vec<(PathBuf, std::ops::Range<usize>)>> {
    Ok(match target {
        Target::One(path) => vec![(path.clone(), 0..cuts)],
        Target::Clips { dir, name, .. } => {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            (0..cuts).map(|k| (dir.join(format!("{name} - {:03}.{}", k + 1, format.extension())), k..k + 1)).collect()
        }
    })
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
    let plan = frame_plan(project, &pipelines, &of_cut, handles)?;
    let fps = pipelines[0].fps;
    if pipelines.iter().any(|p| (p.fps - fps).abs() > 0.01) {
        eprintln!("the clips' frame rates differ; exporting at {fps:.3} fps");
    }
    // Frames each cut shares with the one before: its crossfade, in the
    // whole edit only (a file per cut is for another editor's own), and
    // never more than either cut has to give.
    let frames: Vec<usize> = plan.iter().map(|p| p.2).collect();
    let overlap = if matches!(out.target, Target::One(_)) { overlaps(&frames, &project.cuts.iter().map(|c| c.crossfade).collect::<Vec<_>>(), fps) } else { vec![0; plan.len()] };
    let files = files(&out.target, out.format, plan.len())?;
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
                ffmpeg::Sound::of_cut(&project.cuts[n], input, start, Some(frames as f64 / fps), crossfade)
            })
            .collect();
        // The whole edit gets its mix; a file per cut is for another editor,
        // so its sound is left as recorded and the music goes beside it.
        let length = edit_seconds(&frames, &overlap, cuts.clone(), fps);
        // The cuts' sound past their pictures (J- and L-cuts) mixed as sound
        // clips at the clips' level, as the player plays it.
        let beyond = project.sound_beyond();
        let mix = ffmpeg::Mix::whole(project, &beyond, length);
        let mix = matches!(out.target, Target::One(_)).then_some(&mix);
        let mut encoder = ffmpeg::Encoder::new(&path, out.size, fps, &sound, mix, out.format, out.format == ffmpeg::Format::Resolve && gpu.is_some())?;
        // Overlays over the whole edit only; a file per cut has none.
        let lanes = if matches!(out.target, Target::One(_)) { project.overlays.iter().chain(&project.text).collect() } else { Vec::new() };
        let mut burn = overlay::Burn::new(&mut encoder, lanes, out.size, fps, project.fades, project.duration())?;
        let mut sink = dissolve::Dissolve::new(&mut burn);
        for (n, &(i, start, frames)) in cuts.clone().zip(&plan[cuts.clone()]) {
            let tail = if n + 1 < cuts.end { overlap[n + 1] } else { 0 };
            let head = if n > cuts.start { overlap[n] } else { 0 };
            // Edge looks in the whole edit, as its fades (a file per cut is
            // for another editor).
            let edges = if matches!(out.target, Target::One(_)) { (0..frames).map(|k| project.cuts[n].edge_at(start + k as f64 / fps)).collect() } else { Vec::new() };
            sink.cut(frames, head, tail, project.cuts[n].transition_spec(), edges)?;
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
            // On an error the encoder, dropped unfinished, removes the file.
            match &mut gpu {
                Some(gpu) => player::render::encode(gpu, &pipelines[i], &clips[i], start, frames, &mut sink, camera, size, &mut tick)?,
                None => encode(&pipelines[i], start, Some(frames), &mut sink, |t| Shot::View { camera: Box::new(camera(t)), size }, &mut tick)?,
            }
            done += frames;
        }
        drop(sink);
        drop(burn);
        encoder.finish()?;
        if let (Some(target), Target::One(_)) = (out.loudness, &out.target)
            && let Some((before, gain)) = crate::loudness::bring_to(&path, target, out.format)?
        {
            eprintln!("loudness {before:.1} → {target:.1} LUFS ({gain:+.1} dB)");
        }
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

/// The frames each cut shares with the one before (its crossfade, at
/// `fps`), never more than either has to give, keeping a frame of each its
/// own: `frames` per cut, `crossfades` in seconds.
fn overlaps(frames: &[usize], crossfades: &[f64], fps: f64) -> Vec<usize> {
    let mut overlap = vec![0usize; frames.len()];
    for n in 1..frames.len() {
        let room = (frames[n - 1] - overlap[n - 1]).min(frames[n]).saturating_sub(1);
        overlap[n] = ((crossfades[n] * fps).round() as usize).min(room);
    }
    overlap
}

/// How long cuts `cuts` last joined, seconds: their frames less what each
/// shares with the one before it within the range. The mix is cut to it,
/// so picture and sound end together.
fn edit_seconds(frames: &[usize], overlap: &[usize], cuts: std::ops::Range<usize>, fps: f64) -> f64 {
    let first = cuts.start;
    cuts.map(|n| (frames[n] - if n > first { overlap[n] } else { 0 }) as f64 / fps).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfades_share_frames_within_what_each_cut_has() {
        // 2 s, 2 s, 1 s at 30 fps; crossfades of 0.5 s and of 9 s.
        let frames = [60, 60, 30];
        let overlap = overlaps(&frames, &[0.0, 0.5, 9.0], 30.0);
        // 15 frames; then all but one of what both have (60 − 15 vs 30).
        assert_eq!(overlap, vec![0, 15, 29]);
        assert!((edit_seconds(&frames, &overlap, 0..3, 30.0) - (150 - 15 - 29) as f64 / 30.0).abs() < 1e-12);
        // A file per cut stands alone: what it shares with the one before
        // isn't taken off.
        assert_eq!(edit_seconds(&frames, &overlap, 1..2, 30.0), 2.0);
        // As long as the project says, to the frame.
        let mut cuts = vec![crate::project::Cut::new("a".into(), 0.0, 2.0), crate::project::Cut::new("b".into(), 0.0, 2.0)];
        cuts[1].crossfade = 0.5;
        let overlap = overlaps(&[60, 60], &[0.0, 0.5], 30.0);
        assert!((edit_seconds(&[60, 60], &overlap, 0..2, 30.0) - crate::project::edit_length(&cuts)).abs() < 1.0 / 30.0);
    }
}
