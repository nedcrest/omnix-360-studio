//! Auto-duck: the music lowered under speech (gap 3). The user's design
//! (2026-10-23): one action and one «how much»; where speech is comes from
//! the cleaned voice; the result is ordinary volume points on the Music
//! clips' own lines, to be edited by hand like any (the track's own line
//! went, gap 33i).
//!
//! Speech is found in the waveforms of what certainly is speech: cuts with a
//! cleaned voice (`Cut::audio`, footage-agent's denoised talk) and the
//! takes on the Voice lane. A cut's own sound without one is wind, traffic
//! and music as often as talk, so it doesn't count. Plain model maths: the
//! waveforms are read by the caller.

use crate::project::{Project, VolumeKey, volume_at};
use std::path::PathBuf;

/// The music starts going down this long before speech, so the first
/// word is heard clear.
pub const ATTACK: f64 = 0.3;
/// It comes back up over this long after speech, so a pause between
/// sentences doesn't pump.
pub const RELEASE: f64 = 0.6;
/// Pauses shorter than this stay ducked: two sentences are one stretch.
pub const HOLD: f64 = 0.8;
/// A peak this far under the loudest of a voice's stretch is a pause.
const BELOW_LOUDEST_DB: f64 = 24.0;
/// …but never quieter than this, dBFS: a denoised pause sits far lower.
const FLOOR_DB: f64 = -50.0;
/// Shorter blips (a click, a breath) aren't speech.
const SHORTEST: f64 = 0.25;

/// What may hold speech: a file, its stretch in its own time, where that
/// stretch starts on the edit, and how fast it plays (a cut's speed; a
/// take's is normal).
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub file: PathBuf,
    pub start: f64,
    pub end: f64,
    pub at: f64,
    pub speed: crate::speed::Speed,
}

/// The project's sources of speech: cuts with a cleaned voice and the
/// Voice lane's takes, muted ones left out.
pub fn sources(project: &Project) -> Vec<Source> {
    let offsets = crate::project::offsets(&project.cuts);
    // A cleaned voice, or its own sound when it's marked as speech (gap 103).
    let cuts = project.cuts.iter().zip(offsets).filter(|(c, _)| !c.mute).filter_map(|(c, at)| Some(Source { file: c.audio.clone().or_else(|| c.speech.then(|| c.input.clone()))?, start: c.start, end: c.end, at, speed: c.speed.clone() }));
    let takes = project.voice.clips.iter().filter(|c| !c.mute).map(|c| Source { file: c.input.clone(), start: c.start, end: c.end, at: c.at, speed: Default::default() });
    cuts.chain(takes).collect()
}

/// Where a waveform holds speech within `start..end` (its own time), as
/// stretches of edit time from `at`: peaks (one per `step` seconds, 0..1)
/// above the stretch's loudest less `BELOW_LOUDEST_DB` (and above
/// `FLOOR_DB`), pauses under `HOLD` bridged, blips under `SHORTEST` dropped.
pub fn speech(peaks: &[f32], step: f64, source: &Source) -> Vec<(f64, f64)> {
    let first = (source.start / step).floor().max(0.0) as usize;
    let last = ((source.end / step).ceil() as usize).min(peaks.len());
    if first >= last {
        return Vec::new();
    }
    let db = |p: f32| 20.0 * (p.max(1e-6) as f64).log10();
    let loudest = peaks[first..last].iter().fold(0.0f32, |m, &p| m.max(p));
    let threshold = (db(loudest) - BELOW_LOUDEST_DB).max(FLOOR_DB);
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for (i, &peak) in peaks.iter().enumerate().take(last).skip(first) {
        if db(peak) < threshold {
            continue;
        }
        let (a, b) = ((i as f64 * step).max(source.start), ((i + 1) as f64 * step).min(source.end));
        match spans.last_mut() {
            Some(last) if a - last.1 < HOLD => last.1 = b,
            _ => spans.push((a, b)),
        }
    }
    spans.retain(|(a, b)| b - a >= SHORTEST);
    // Own time to the edit's, through its speed.
    let edit = |t: f64| if source.speed.is_normal() { source.at + t - source.start } else { source.at + source.speed.edit_secs(source.start, t) };
    spans.iter().map(|&(a, b)| (edit(a), edit(b))).collect()
}

/// Where `project` holds speech, edit time: each source's waveform read
/// (from the cache, or decoded once, `waveform`). Slow on a cold cache:
/// off the UI thread.
pub fn find_speech(project: &Project) -> anyhow::Result<Vec<(f64, f64)>> {
    use crate::waveform::{WAVE_STEP, Waveform};
    let mut spans = Vec::new();
    for s in sources(project) {
        let wave = Waveform::read(&s.file, false)?;
        spans.extend(speech(&wave.peaks, WAVE_STEP, &s));
    }
    Ok(merge(spans))
}

/// `omnix duck`: the music lowered `depth` dB under the project's
/// speech, as the Audio panel's «Lower under speech» does; prints where,
/// writes the project to `output` if given.
pub fn run(path: &std::path::Path, depth: f64, ops: bool, output: Option<&std::path::Path>) -> anyhow::Result<()> {
    let project = Project::load(path)?;
    let spans = find_speech(&project)?;
    for (a, b) in &spans {
        crate::apply::say(ops, &format!("speech {a:7.2} – {b:7.2} s"));
    }
    if spans.is_empty() {
        crate::apply::say(ops, "no speech found: it's looked for in cleaned voices and on the Voice lane");
    }
    // Through the ops, as the Audio panel's button: checked, the same change.
    let duck = crate::model::ops::Op::Sound(crate::model::ops::sound::SoundOp::Duck { depth, speech: spans });
    crate::apply::finish(project, &[duck], ops, output)
}

/// The music lowered under `speech` (edit time): each clip's own line, in
/// its file's time, ducked where speech falls on it or within a ramp of it.
/// A duck from before is taken out first, so pressing it again (another
/// depth, a new take) replaces it rather than lowering twice; points moved
/// by hand since keep their change.
pub fn duck_music(music: &mut crate::project::Music, speech: &[(f64, f64)], depth: f64) {
    for c in &mut music.clips {
        let to_file = |t: f64| c.local(t);
        let reach = ATTACK + RELEASE;
        let on: Vec<(f64, f64)> = speech.iter().filter(|&&(a, b)| b > c.at - reach && a < c.ends() + reach).map(|&(a, b)| (to_file(a), to_file(b))).collect();
        let base = match c.ducked.take() {
            Some(before) => unduck(&c.volume, &before),
            None => c.volume.clone(),
        };
        c.volume = if on.is_empty() { base } else { duck(&base, &on, depth) };
        c.ducked = (!on.is_empty()).then_some(crate::project::Ducked { depth, speech: on });
    }
}

/// Stretches from several sources as one sorted list, overlapping or near
/// ones (closer than `HOLD`) joined.
pub fn merge(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in spans {
        match out.last_mut() {
            Some(last) if a - last.1 < HOLD => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// A volume line with the music `depth` dB lower over each speech
/// stretch, ramped down `ATTACK` before it and up over `RELEASE` after:
/// the duck added to what the line already says, so a hand-drawn line
/// keeps its shape. Ramps that would meet between two stretches keep it
/// down.
pub fn duck(line: &[VolumeKey], speech: &[(f64, f64)], depth: f64) -> Vec<VolumeKey> {
    let bends = bends(speech, depth);
    // Every time either line bends, with both added there: straight lines
    // between them in dB, as `volume_at` reads them.
    let mut times: Vec<f64> = line.iter().map(|k| k.t).chain(bends.iter().map(|b| b.0)).collect();
    times.sort_by(f64::total_cmp);
    // Points closer than a hundredth of a second are one (as `volume_key_at`
    // has it): a bend on an existing point doesn't make a second.
    times.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    times.into_iter().map(|t| VolumeKey { t, volume: volume_at(line, t) + at(&bends, t) }).collect()
}

/// The line `duck` made from `before`'s, with that duck taken out: each
/// point back up by the duck there, and the points only the duck put in
/// (on the line without them) dropped. Both lines are straight between
/// the same points, so a line left as `duck` made it comes back exactly.
pub fn unduck(line: &[VolumeKey], before: &crate::project::Ducked) -> Vec<VolumeKey> {
    let bends = bends(&before.speech, before.depth);
    let mut keys: Vec<VolumeKey> = line.iter().map(|k| VolumeKey { t: k.t, volume: k.volume - at(&bends, k.t) }).collect();
    let mut i = 0;
    while i < keys.len() {
        let bend = bends.iter().any(|b| (b.0 - keys[i].t).abs() < 0.01);
        let mut without = keys.clone();
        let k = without.remove(i);
        if bend && (volume_at(&without, k.t) - k.volume).abs() < 0.01 {
            keys = without;
        } else {
            i += 1;
        }
    }
    keys
}

/// Where the duck itself bends: 0 dB, down, down, 0 dB per stretch,
/// stretches whose ramps would cross joined first.
fn bends(speech: &[(f64, f64)], depth: f64) -> Vec<(f64, f64)> {
    let mut spans: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in speech {
        match spans.last_mut() {
            Some(last) if a - ATTACK <= last.1 + RELEASE => last.1 = last.1.max(b),
            _ => spans.push((a, b)),
        }
    }
    spans.into_iter().flat_map(|(a, b)| [((a - ATTACK).max(0.0), 0.0), (a, -depth.abs()), (b, -depth.abs()), (b + RELEASE, 0.0)]).collect()
}

/// The duck's own level at `t`, dB: 0 away from speech.
fn at(bends: &[(f64, f64)], t: f64) -> f64 {
    let i = bends.partition_point(|k| k.0 <= t);
    match (i.checked_sub(1).map(|j| bends[j]), bends.get(i)) {
        (Some(a), Some(b)) => a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0).max(1e-9),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The speech's sources on the edit, for the time seam's film (guard 1).
    #[test]
    fn the_speech_comes_from_where_it_did() {
        use crate::time_snapshot::{film, golden, n};
        let mut p = film();
        p.cuts[0].speech = true;
        let lines: String = sources(&p).iter().map(|s| format!("{} {}..{} at {}\n", s.file.display(), n(s.start), n(s.end), n(s.at))).collect();
        golden("time-speech", &lines);
    }

    fn source(at: f64) -> Source {
        Source { file: "voice.flac".into(), start: 1.0, end: 9.0, at, speed: Default::default() }
    }

    /// Peaks every 0.1 s: quiet, then speech from 2 s to 4 s with a 0.5 s
    /// pause at 3 s, a click at 6 s, speech 7..8.
    fn peaks() -> Vec<f32> {
        (0..100)
            .map(|i| {
                let t = i as f64 * 0.1;
                let talking = (2.0..3.0).contains(&t) || (3.5..4.0).contains(&t) || (7.0..8.0).contains(&t);
                if talking { 0.5 } else if (6.0..6.1).contains(&t) { 0.4 } else { 0.001 }
            })
            .collect()
    }

    #[test]
    fn speech_is_found_on_the_edit_with_short_pauses_bridged() {
        // The source's own 1 s is the edit's 10 s.
        let got = speech(&peaks(), 0.1, &source(10.0));
        let near = |x: f64, y: f64| (x - y).abs() < 1e-6;
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(near(got[0].0, 11.0) && near(got[0].1, 13.0), "the pause at 3 s bridged: {got:?}");
        assert!(near(got[1].0, 16.0) && near(got[1].1, 17.0), "the click at 6 s dropped: {got:?}");
        // A cut at twice the speed: its speech half as far in, half as long.
        let fast = speech(&peaks(), 0.1, &Source { speed: crate::speed::Speed::constant(2.0), ..source(10.0) });
        assert!(near(fast[0].0, 10.5) && near(fast[0].1, 11.5) && near(fast[1].0, 13.0), "{fast:?}");
        // Outside its stretch nothing counts.
        let short = Source { start: 4.5, end: 6.5, ..source(0.0) };
        assert!(speech(&peaks(), 0.1, &short).is_empty());
    }

    #[test]
    fn stretches_from_several_voices_join() {
        assert_eq!(merge(vec![(5.0, 6.0), (1.0, 2.0), (2.5, 3.0), (5.5, 7.0)]), vec![(1.0, 3.0), (5.0, 7.0)]);
    }

    #[test]
    fn the_music_goes_down_under_speech_on_top_of_its_own_line() {
        // The music's own line falls from 0 to −6 dB over 0..20 s.
        let line = [VolumeKey { t: 0.0, volume: 0.0 }, VolumeKey { t: 20.0, volume: -6.0 }];
        let ducked = duck(&line, &[(5.0, 8.0)], 12.0);
        let at = |t: f64| volume_at(&ducked, t);
        assert!((at(0.0) - 0.0).abs() < 1e-9 && (at(20.0) + 6.0).abs() < 1e-9, "its own line kept");
        assert!((at(4.0) - volume_at(&line, 4.0)).abs() < 1e-9, "untouched before the attack");
        assert!((at(6.5) - (volume_at(&line, 6.5) - 12.0)).abs() < 1e-9, "12 dB down under speech");
        assert!((at(5.0 - ATTACK / 2.0) - (volume_at(&line, 5.0 - ATTACK / 2.0) - 6.0)).abs() < 1e-9, "half way down");
        assert!((at(9.0) - volume_at(&line, 9.0)).abs() < 1e-9, "back up after the release");
        // Two sentences closer than the ramps: down all through.
        let close = duck(&[], &[(5.0, 6.0), (6.5, 8.0)], 10.0);
        assert!((volume_at(&close, 6.2) + 10.0).abs() < 1e-9);
        // No speech, no change.
        assert_eq!(duck(&line, &[], 12.0), line.to_vec());
    }

    #[test]
    fn each_song_is_lowered_on_its_own_line_where_speech_falls_on_it() {
        let mut music = crate::project::Music::default();
        let song = |at: f64, start: f64| crate::project::MusicClip { input: "s.wav".into(), at, start, end: start + 10.0, ..Default::default() };
        // Song A plays its 20..30 s at edit 0..10; song B its 0..10 at 10..20.
        music.clips = vec![song(0.0, 20.0), song(10.0, 0.0), song(40.0, 0.0)];
        duck_music(&mut music, &[(4.0, 6.0), (12.0, 13.0)], 10.0);
        let at = |i: usize, file_t: f64| volume_at(&music.clips[i].volume, file_t);
        // Edit 5 s is A's 25 s; edit 12.5 s is B's 2.5 s.
        assert!((at(0, 25.0) + 10.0).abs() < 1e-9 && (at(0, 21.0)).abs() < 1e-9);
        assert!((at(1, 2.5) + 10.0).abs() < 1e-9 && (at(1, 6.0)).abs() < 1e-9);
        // The song no speech falls on is left alone.
        assert!(music.clips[2].volume.is_empty());
    }

    #[test]
    fn pressed_again_it_replaces_the_duck_before() {
        let song = crate::project::MusicClip { input: "s.wav".into(), at: 0.0, start: 0.0, end: 20.0, volume: vec![VolumeKey { t: 2.0, volume: -3.0 }, VolumeKey { t: 15.0, volume: 0.0 }], ..Default::default() };
        let speech = [(5.0, 8.0)];
        let once = |depth: f64| {
            let mut m = crate::project::Music::default();
            m.clips.push(song.clone());
            duck_music(&mut m, &speech, depth);
            m
        };
        // 20, then 18: as 18 alone, not 38 down (user, 2026-11-02).
        let mut twice = once(20.0);
        duck_music(&mut twice, &speech, 18.0);
        assert_eq!(twice, once(18.0));
        // Taken out, the hand-drawn line comes back as it was.
        let ducked = &twice.clips[0];
        assert_eq!(unduck(&ducked.volume, ducked.ducked.as_ref().unwrap()), song.volume);
        // A point moved by hand after the duck keeps its change through the next.
        let mut edited = once(18.0);
        let k = edited.clips[0].volume.iter().position(|k| k.t == 15.0).unwrap();
        edited.clips[0].volume[k].volume = -4.0;
        duck_music(&mut edited, &speech, 12.0);
        assert!((volume_at(&edited.clips[0].volume, 15.0) + 4.0).abs() < 1e-9);
        assert!((volume_at(&edited.clips[0].volume, 6.0) - (volume_at(&song.volume, 6.0) - 12.0)).abs() < 1e-9);
    }

    #[test]
    fn a_line_deleted_after_a_duck_is_not_raised_by_the_next() {
        let mut p = Project::default();
        p.music.clips.push(crate::project::MusicClip { input: "s.wav".into(), at: 0.0, start: 0.0, end: 20.0, ..Default::default() });
        duck_music(&mut p.music, &[(5.0, 8.0)], 18.0);
        let own = crate::project::Own::Clip(crate::project::SoundTrack::Music, 0);
        p.own_sound(own).unwrap().volume.clear();
        p.forget_duck(own);
        duck_music(&mut p.music, &[(5.0, 8.0)], 18.0);
        assert!(p.music.clips[0].volume.iter().all(|k| k.volume <= 1e-9), "{:?}", p.music.clips[0].volume);
    }

    #[test]
    fn sources_are_cleaned_voices_and_voice_takes_not_muted() {
        let mut p = Project::default();
        let mut talk = crate::project::Cut::new("a.mp4".into(), 2.0, 6.0);
        talk.audio = Some("voice/a.flac".into());
        let wind = crate::project::Cut::new("b.mp4".into(), 0.0, 3.0);
        let mut muted = talk.clone();
        muted.mute = true;
        p.cuts = vec![wind, talk, muted];
        p.voice.clips.push(crate::project::MusicClip { input: "take.wav".into(), at: 1.0, start: 0.5, end: 2.0, ..Default::default() });
        let got = sources(&p);
        assert_eq!(got, vec![Source { file: "voice/a.flac".into(), start: 2.0, end: 6.0, at: 3.0, speed: Default::default() }, Source { file: "take.wav".into(), start: 0.5, end: 2.0, at: 1.0, speed: Default::default() }]);
    }
}
