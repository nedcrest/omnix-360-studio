//! A song's beats (gap 13): where its hits fall, each with how strong it
//! is, so the music lane can show them and cuts can snap to them. Found by
//! how fast the sound's energy rises — in the lows, where a kick drum is,
//! and the highs, where a snare and hi-hats are — rather than by a tempo
//! model: a travel film's music is often free, and the user wants the hits
//! he hears (user, 2026-11-10: a threshold of his own, low for every beat,
//! high for the strong ones alone).
//!
//! ffmpeg has no beat tracker, so this is plain counting on mono samples;
//! the rise curve is kept in the clip cache, the peaks picked from it each
//! time (cheap) as the threshold changes. A hit's time is then found to
//! the sample within its step (`onsets`): a step's start put the line up
//! to 10 ms early, a pixel or two at a close zoom (the user, 2026-11-11).

use anyhow::Result;
use std::path::Path;

/// The rate the song is read at: plenty for where energy rises.
const RATE: u32 = 11025;
/// One step of the rise curve, seconds: a beat lands within it.
pub const STEP: f64 = 0.01;
/// Where the lows end and the highs start, Hz: a kick's thump below, a
/// snare's crack and hi-hats above.
const LOWS: f64 = 150.0;
const HIGHS: f64 = 2000.0;
/// A hit stands this far above the curve's level around it, which is
/// taken over this many seconds either way: a loud stretch doesn't make
/// every step a beat.
const AROUND: f64 = 0.25;
const ABOVE: f32 = 1.3;
/// Two hits closer than this are one (a flam, an echo).
const APART: f64 = 0.07;

/// A hit: when, seconds in the song's file, and how strong, 0..1 of the
/// song's strongest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub t: f64,
    pub strength: f32,
}

/// Where in each `STEP` of `samples` (mono, `rate` a second) a hit would
/// start, seconds in the song: the first sample past half the way from
/// the level before (the step two back) to the loudest of this step and
/// the one before; a hit picked at a step takes its time from here.
pub fn onsets(samples: &[f32], rate: f64) -> Vec<f32> {
    let per = ((STEP * rate).round() as usize).max(1);
    let loudest = |from: usize, to: usize| samples[from.min(samples.len())..to.min(samples.len())].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    (0..samples.len().div_ceil(per))
        .map(|i| {
            let (from, to) = (i.saturating_sub(1) * per, (i + 1) * per);
            let before = loudest(i.saturating_sub(2) * per, from);
            let half = before + 0.5 * (loudest(from, to) - before);
            let at = (from..to.min(samples.len())).find(|&n| samples[n].abs() > half).unwrap_or(i * per);
            (at as f64 / rate) as f32
        })
        .collect()
}

/// How fast the energy rises at each `STEP` of `samples` (mono, `rate`
/// a second), the lows' and highs' rises added: the onset strength.
pub fn rise(samples: &[f32], rate: f64) -> Vec<f32> {
    let per = ((STEP * rate).round() as usize).max(1);
    // One-pole filters: enough to part a kick from a hi-hat.
    let pole = |hz: f64| (-2.0 * std::f64::consts::PI * hz / rate).exp() as f32;
    let (low_k, high_k) = (pole(LOWS), pole(HIGHS));
    let (mut low, mut smooth) = (0.0f32, 0.0f32);
    let mut energy = Vec::with_capacity(samples.len() / per + 1);
    for chunk in samples.chunks(per) {
        let (mut e_low, mut e_high) = (0.0f32, 0.0f32);
        for &x in chunk {
            low = low_k * low + (1.0 - low_k) * x;
            smooth = high_k * smooth + (1.0 - high_k) * x;
            let high = x - smooth;
            e_low += low * low;
            e_high += high * high;
        }
        energy.push([e_low, e_high].map(|e| (1.0 + 1000.0 * e / chunk.len() as f32).ln()));
    }
    let mut rises = vec![0.0f32; energy.len()];
    for i in 1..energy.len() {
        rises[i] = (0..2).map(|b| (energy[i][b] - energy[i - 1][b]).max(0.0)).sum();
    }
    rises
}

/// The hits in a rise curve of `STEP` steps: each step that is the
/// highest within `APART` and stands `ABOVE` its surroundings' mean; its
/// strength against the strongest hit's; its time from `onsets` (the
/// step's start without them).
pub fn hits(rises: &[f32], onsets: Option<&[f32]>) -> Vec<Beat> {
    let around = (AROUND / STEP).round() as usize;
    let apart = (APART / STEP).round() as usize;
    let mut found: Vec<(usize, f32)> = Vec::new();
    for i in 0..rises.len() {
        let near = |r: usize| rises[i.saturating_sub(r)..(i + r + 1).min(rises.len())].iter();
        let level = near(around).sum::<f32>() / near(around).len() as f32;
        let tallest = near(apart).fold(0.0f32, |m, &v| m.max(v));
        if rises[i] > 0.0 && rises[i] >= tallest && rises[i] > ABOVE * level && found.last().is_none_or(|&(j, _)| i - j > apart) {
            found.push((i, rises[i] - level));
        }
    }
    let strongest = found.iter().fold(0.0f32, |m, &(_, s)| m.max(s)).max(f32::MIN_POSITIVE);
    let at = |i: usize| onsets.and_then(|o| o.get(i)).map_or(i as f64 * STEP, |&t| t as f64);
    found.into_iter().map(|(i, s)| Beat { t: at(i), strength: (s / strongest).clamp(0.0, 1.0) }).collect()
}

/// Song `path`'s hits: its rise curve from the clip cache, else read
/// (a few-minute song in about a second).
pub fn read(path: &Path) -> Result<Vec<Beat>> {
    const RISE: &str = "rise";
    const ONSET: &str = "onset";
    if let (Some((rises, _)), Some((onsets, _))) = (crate::cache::waveform(path, RISE, STEP), crate::cache::waveform(path, ONSET, STEP)) {
        return Ok(hits(&rises, Some(&onsets)));
    }
    let samples = crate::ffmpeg::mono(path, RATE, None)?;
    if samples.is_empty() {
        anyhow::bail!("{} has no sound", path.display());
    }
    let (rises, at) = (rise(&samples, RATE as f64), onsets(&samples, RATE as f64));
    let length = samples.len() as f64 / RATE as f64;
    crate::cache::store_waveform(path, RISE, STEP, &rises, length);
    crate::cache::store_waveform(path, ONSET, STEP, &at, length);
    Ok(hits(&rises, Some(&at)))
}

/// The hits at `threshold` (0..1) or stronger.
pub fn at_least(beats: &[Beat], threshold: f64) -> impl Iterator<Item = &Beat> {
    beats.iter().filter(move |b| b.strength as f64 >= threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `seconds` of soft noise with a kick (a low thump) every half second
    /// from `offset` and a softer click a quarter second after each.
    fn drums(seconds: f64, offset: f64) -> Vec<f32> {
        let rate = RATE as f64;
        let mut seed = 1u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32 - 0.5
        };
        (0..(seconds * rate) as usize)
            .map(|n| {
                let t = n as f64 / rate;
                let since = |every: f64, at: f64| (t - offset - at).rem_euclid(every);
                let kick = if t < offset { 0.0 } else { (-since(0.5, 0.0) * 30.0).exp() * (2.0 * std::f64::consts::PI * 60.0 * since(0.5, 0.0)).sin() };
                let click = if t < offset + 0.25 { 0.0 } else { (-since(0.5, 0.25) * 200.0).exp() * noise() as f64 * 0.8 };
                (0.6 * kick + click + 0.01 * noise() as f64) as f32
            })
            .collect()
    }

    #[test]
    fn kicks_and_clicks_are_found_where_they_fall_the_kicks_strongest() {
        let samples = drums(4.0, 0.0);
        let beats = hits(&rise(&samples, RATE as f64), Some(&onsets(&samples, RATE as f64)));
        let near = |t: f64| beats.iter().find(|b| (b.t - t).abs() <= 0.03);
        for k in 1..8 {
            let kick = near(k as f64 * 0.5).unwrap_or_else(|| panic!("a kick at {} s in {beats:?}", k as f64 * 0.5));
            let click = near(k as f64 * 0.5 + 0.25).unwrap_or_else(|| panic!("a click at {} s in {beats:?}", k as f64 * 0.5 + 0.25));
            assert!(kick.strength > click.strength, "{kick:?} against {click:?}");
        }
        assert!(beats.len() <= 17, "nothing between them: {beats:?}");
        // High, the kicks alone.
        let strong: Vec<f64> = at_least(&beats, 0.8).map(|b| b.t).collect();
        assert!(strong.iter().all(|t| (t / 0.5 - (t / 0.5).round()).abs() * 0.5 <= 0.03), "{strong:?}");
    }

    #[test]
    fn a_hit_is_placed_to_the_sample_not_the_step() {
        // Kicks and clicks off the steps' grid: 3.7 ms past it.
        let offset = 0.0037;
        let samples = drums(4.0, offset);
        let beats = hits(&rise(&samples, RATE as f64), Some(&onsets(&samples, RATE as f64)));
        let without = hits(&rise(&samples, RATE as f64), None);
        for k in 1..8 {
            let want = offset + k as f64 * 0.5;
            let off = |bs: &[Beat]| bs.iter().map(|b| (b.t - want).abs()).fold(f64::INFINITY, f64::min);
            assert!(off(&beats) < 0.002, "a kick at {want}: {beats:?}");
            assert!(off(&without) > off(&beats), "closer than the step's start");
        }
    }

    #[test]
    fn silence_has_no_beats() {
        assert!(hits(&rise(&vec![0.0; RATE as usize * 2], RATE as f64), None).is_empty());
        assert!(hits(&[], None).is_empty());
    }
}
