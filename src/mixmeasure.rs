//! Measuring a project's sound for the mix check (gap 45, step 2): each
//! lane rendered alone through the export's own mix graph (`ffmpeg::
//! sound_stream`), so a clip is measured as the export will make it (its
//! level, line, fades, filters, the lane's level), and the whole film
//! once more as exported. Read as it streams: a film's worth would be
//! hundreds of MB. `omnix mixcheck` prints it with the findings.

use crate::ffmpeg::{self, Mix, Sound};
use crate::lufs::Loudness;
use crate::mixcheck::{Item, Lane, Measured, Speech};
use crate::project::{Own, Project, SoundTrack};
use anyhow::{Context, Result};
use std::io::Read;

const RATE: f64 = 48000.0;

/// The cuts' sound as the export joins it (`export.rs`, by seconds rather
/// than its frames: near enough to measure).
fn cut_sounds(project: &Project) -> Vec<Sound<'_>> {
    project
        .cuts
        .iter()
        .enumerate()
        .map(|(n, c)| Sound::of_cut(c, c.audio.as_deref().filter(|a| a.exists()).unwrap_or(&c.input), c.start, Some(c.len()), if n > 0 { c.crossfade } else { 0.0 }))
        .collect()
}

/// A stretch of the edit to measure on one rendered lane.
struct Window {
    from: f64,
    to: f64,
    loudness: Loudness,
}

/// Renders a lane (or the whole film) and feeds every window its frames;
/// returns its loudest sample, dBFS.
fn run(audio: &[Sound<'_>], mix: &Mix<'_>, windows: &mut [&mut Window]) -> Result<f64> {
    let mut child = ffmpeg::sound_stream(audio, mix)?;
    let mut out = child.stdout.take().context("ffmpeg's sound")?;
    let mut buf = vec![0u8; 8 * 4800];
    let (mut frame, mut peak) = (0u64, 0f32);
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let n = out.read(&mut buf)?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&buf[..n]);
        let whole = carry.len() / 8 * 8;
        let frames: Vec<[f32; 2]> = carry[..whole].chunks_exact(8).map(|b| [f32::from_le_bytes([b[0], b[1], b[2], b[3]]), f32::from_le_bytes([b[4], b[5], b[6], b[7]])]).collect();
        carry.drain(..whole);
        peak = frames.iter().fold(peak, |m, f| m.max(f[0].abs()).max(f[1].abs()));
        let (t0, t1) = (frame as f64 / RATE, (frame + frames.len() as u64) as f64 / RATE);
        for w in windows.iter_mut().filter(|w| w.to > t0 && w.from < t1) {
            let a = (((w.from - t0) * RATE).round().max(0.0) as usize).min(frames.len());
            let b = (((w.to - t0) * RATE).round().max(0.0) as usize).min(frames.len());
            w.loudness.feed(RATE, &frames[a..b]);
        }
        frame += frames.len() as u64;
    }
    child.wait()?;
    Ok(if peak > 0.0 { 20.0 * (peak as f64).log10() } else { -120.0 })
}

/// Measures `project` for the mix check. `progress` hears how many of
/// the renders are done, of how many.
pub fn measure(project: &Project, progress: &mut dyn FnMut(usize, usize)) -> Result<Measured> {
    let video = crate::project::edit_length(&project.cuts);
    let length = if project.cuts.is_empty() { project.sound_clips().map(|c| c.ends()).fold(0.0, f64::max) } else { video };
    // A voice it can't read is said, not taken for silence: the rule on
    // music under speech would pass without looking.
    let speech = crate::duck::find_speech(project).context("finding the speech")?;
    let cuts = cut_sounds(project);
    let tracks: Vec<SoundTrack> = SoundTrack::ALL.into_iter().filter(|&w| !project.track(w).clips.is_empty()).collect();
    let renders = usize::from(!cuts.is_empty()) + tracks.len() + 1;
    let mut done = 0;
    let window = |from: f64, to: f64| Window { from, to, loudness: Loudness::default() };
    let mut items = Vec::new();
    // Each stretch of speech, measured on every lane: the voice is the
    // louder of the Voice lane and the clips' sound there; the music's
    // clip under it is the one covering most of it.
    let mut spans: Vec<Vec<(Lane, Window)>> = speech.iter().map(|_| Vec::new()).collect();
    // The clips' own sound.
    if !cuts.is_empty() {
        let offsets = crate::project::offsets(&project.cuts);
        let mut ws: Vec<Window> = project.cuts.iter().zip(&offsets).map(|(c, &at)| window(at, at + c.len())).collect();
        let mut sp: Vec<Window> = speech.iter().map(|&(a, b)| window(a, b)).collect();
        // With their sound past the pictures (J- and L-cuts).
        let beyond = project.sound_beyond();
        let mix = Mix { sound_db: project.sound.volume, tracks: beyond.iter().collect(), length, fades: project.fades };
        run(&cuts, &mix, &mut ws.iter_mut().chain(sp.iter_mut()).collect::<Vec<_>>())?;
        for ((n, c), w) in project.cuts.iter().enumerate().zip(ws) {
            items.push(Item { lane: Lane::ClipSound, own: Own::Cut(n), at: w.from, len: c.len(), lufs: w.loudness.integrated(), loudest: w.loudness.loudest(), speech: c.audio.is_some() && !c.mute });
        }
        for (k, w) in sp.into_iter().enumerate() {
            spans[k].push((Lane::ClipSound, w));
        }
        done += 1;
        progress(done, renders);
    }
    // Each sound track alone.
    for which in tracks {
        let track = project.track(which);
        let mut ws: Vec<Window> = track.clips.iter().map(|c| window(c.at, c.ends())).collect();
        let mut sp: Vec<Window> = speech.iter().map(|&(a, b)| window(a, b)).collect();
        let mix = Mix { sound_db: 0.0, tracks: vec![track], length, fades: project.fades };
        run(&[], &mix, &mut ws.iter_mut().chain(sp.iter_mut()).collect::<Vec<_>>())?;
        for ((i, c), w) in track.clips.iter().enumerate().zip(ws) {
            items.push(Item { lane: Lane::Track(which), own: Own::Clip(which, i), at: c.at, len: c.ends() - c.at, lufs: w.loudness.integrated(), loudest: w.loudness.loudest(), speech: which == SoundTrack::Voice && !c.mute });
        }
        for (k, w) in sp.into_iter().enumerate() {
            spans[k].push((Lane::Track(which), w));
        }
        done += 1;
        progress(done, renders);
    }
    // The whole film, as the export makes it.
    let mut whole = window(0.0, length);
    let beyond = project.sound_beyond();
    let mix = Mix::whole(project, &beyond, length);
    let peak = run(&cuts, &mix, &mut [&mut whole])?;
    progress(renders, renders);
    let speech = speech
        .iter()
        .zip(spans)
        .filter_map(|(&(from, to), lanes)| {
            let level = |lane: Lane| lanes.iter().find(|(l, _)| *l == lane).and_then(|(_, w)| w.loudness.integrated());
            let voice = [level(Lane::Track(SoundTrack::Voice)), level(Lane::ClipSound)].into_iter().flatten().reduce(f64::max)?;
            let music = &project.music.clips;
            let song = (0..music.len()).max_by(|&a, &b| overlap(music[a].at, music[a].ends(), from, to).total_cmp(&overlap(music[b].at, music[b].ends(), from, to))).filter(|&i| overlap(music[i].at, music[i].ends(), from, to) > 0.0);
            let under = song.and_then(|i| Some((Own::Clip(SoundTrack::Music, i), level(Lane::Track(SoundTrack::Music))?))).into_iter().collect();
            Some(Speech { from, to, voice, under })
        })
        .collect();
    Ok(Measured { items, speech, mix: whole.loudness.integrated(), peak })
}

fn overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// `omnix mixcheck`: measures the project, prints each clip's loudness
/// and the findings against `target` LUFS.
pub fn run_cli(path: &std::path::Path, target: f64) -> Result<()> {
    let project = Project::load(path)?;
    let m = measure(&project, &mut |done, of| eprintln!("measured {done} of {of}"))?;
    let lufs = |v: Option<f64>| v.map_or("silent".to_owned(), |v| format!("{v:6.1}"));
    for i in &m.items {
        let lane = match i.lane {
            Lane::ClipSound => "Clip sound".to_owned(),
            Lane::Track(w) => w.name().to_owned(),
        };
        println!("{:7.2} s  {lane:<10}  {} LUFS, loudest {}{}", i.at, lufs(i.lufs), lufs(i.loudest), if i.speech { "  speech" } else { "" });
    }
    for s in &m.speech {
        let under: Vec<String> = s.under.iter().map(|&(_, l)| format!("music {l:.1} LUFS, {:.1} dB under", s.voice - l)).collect();
        println!("{:7.2} s  speech to {:.2} s  voice {:.1} LUFS{}", s.from, s.to, s.voice, under.iter().map(|u| format!(", {u}")).collect::<String>());
    }
    println!("film: {} LUFS, peak {:.1} dBFS", lufs(m.mix), m.peak);
    for f in crate::mixcheck::check(&m, &crate::mixcheck::Rules::at(target)) {
        println!("{:7.2} s  {}  {}", f.at, if f.severity == crate::mixcheck::Severity::Fix { "fix " } else { "hint" }, f.text);
    }
    Ok(())
}
