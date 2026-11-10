//! Auto colour (gap 89): a cut's picture measured over a dozen frames
//! across it, and the look that puts it right: levels to its darkest and
//! brightest, its middle brought to a mid-grey by exposure, white balance
//! from what's nearly grey in it. Careful by design: each value held
//! within reach, nothing moved for a small difference; set as the cut's
//! own values, one undo step, the user's to change after.

use crate::grade::{BLACK_IN, EXPOSURE, TEMPERATURE, TINT, WHITE_IN, luma};
use anyhow::{Context, Result, bail};

/// Frames measured across a cut.
pub const FRAMES: usize = 12;
/// Their width, pixels: enough for levels, cheap to decode.
const WIDTH: u32 = 320;
/// The share of pixels at either end that isn't the darkest or brightest
/// (a speck, a dead pixel).
const SPECK: f64 = 0.001;
/// Where the middle of the picture may lie, display values: outside, it's
/// brought to the nearer end; inside, exposure stays.
const MID: (f64, f64) = (0.33, 0.58);
/// The most exposure moves, stops; under `SMALL_STOPS` it stays.
const MOST_STOPS: f64 = 1.0;
const SMALL_STOPS: f64 = 0.15;
/// A pixel counts as nearly grey with its channels this close for its
/// brightness (spread over luma: a cast keeps it under), between these
/// lumas; with fewer than `GREY_SHARE` of them the balance stays.
const GREY_SPREAD: f64 = 0.3;
const GREY_LUMA: (f64, f64) = (0.15, 0.85);
const GREY_SHARE: f64 = 0.01;

/// What a picture holds: its darkest, brightest and middle luma (0..1,
/// specks aside), and the mean colour of its nearly grey pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub black: f64,
    pub white: f64,
    pub mid: f64,
    pub grey: Option<[f64; 3]>,
}

/// Measures `pixels` (display values, 0..1).
pub fn measure(pixels: impl Iterator<Item = [f64; 3]>) -> Measure {
    const BINS: usize = 1024;
    let mut hist = vec![0u64; BINS];
    let (mut grey, mut greys, mut n) = ([0.0; 3], 0u64, 0u64);
    for c in pixels {
        let l = luma(c).clamp(0.0, 1.0);
        hist[((l * (BINS - 1) as f64).round()) as usize] += 1;
        n += 1;
        let spread = c.iter().fold(f64::MIN, |a, &v| a.max(v)) - c.iter().fold(f64::MAX, |a, &v| a.min(v));
        if spread < GREY_SPREAD * l && (GREY_LUMA.0..GREY_LUMA.1).contains(&l) {
            (0..3).for_each(|k| grey[k] += c[k]);
            greys += 1;
        }
    }
    let at = |share: f64| {
        let want = ((n as f64 * share).ceil() as u64).max(1);
        let mut seen = 0;
        hist.iter().position(|&h| {
            seen += h;
            seen >= want
        })
        .unwrap_or(0) as f64
            / (BINS - 1) as f64
    };
    let enough = n > 0 && greys as f64 >= n as f64 * GREY_SHARE;
    Measure { black: at(SPECK), white: at(1.0 - SPECK), mid: at(0.5), grey: enough.then(|| grey.map(|v| v / greys as f64)) }
}

/// The look that puts a measured picture right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correction {
    pub black_in: f64,
    pub white_in: f64,
    pub exposure: f64,
    pub temperature: f64,
    pub tint: f64,
}

impl Correction {
    /// Each as an effect: its place among `grade::NAMES`, and its value.
    pub fn values(&self) -> [(usize, f64); 5] {
        [(BLACK_IN, self.black_in), (WHITE_IN, self.white_in), (EXPOSURE, self.exposure), (TEMPERATURE, self.temperature), (TINT, self.tint)]
    }

    /// The levels alone (Levels' «Auto»).
    pub fn levels(&self) -> [(usize, f64); 2] {
        [(BLACK_IN, self.black_in), (WHITE_IN, self.white_in)]
    }
}

/// What puts `m` right: its darkest to black and brightest to white (a
/// picture already reaching them left so), its middle into `MID`, its
/// nearly grey to grey.
pub fn correction(m: &Measure) -> Correction {
    let black_in = if m.black > 0.01 { m.black.min(0.3) } else { 0.0 };
    let white_in = if m.white < 0.98 { m.white.max(0.7) } else { 1.0 };
    let span = (white_in - black_in).max(1e-3);
    let level = |v: f64| ((v - black_in) / span).clamp(0.0, 1.0);
    let mid = level(m.mid).max(0.02);
    let stops = (2.2 * (mid.clamp(MID.0, MID.1) / mid).log2()).clamp(-MOST_STOPS, MOST_STOPS);
    let exposure = if stops.abs() < SMALL_STOPS { 0.0 } else { (stops * 100.0).round() / 100.0 };
    let (temperature, tint) = m.grey.map_or((0.0, 0.0), |g| crate::grade::white_balance(g.map(level)));
    let small = |v: f64| if v.abs() < 0.05 { 0.0 } else { (v * 100.0).round() / 100.0 };
    Correction { black_in, white_in, exposure, temperature: small(temperature), tint: small(tint) }
}

/// Cut `cut` measured over `FRAMES` frames across it, through its LUT as
/// it's shown. An ordinary video only: a 360 clip would need stitching
/// first. Blocking: ffmpeg decodes each frame.
pub fn measure_cut(cut: &crate::project::Cut) -> Result<Measure> {
    let flat = crate::ffmpeg::flat(&cut.input)?.context("a 360 clip: auto colour reads ordinary video only for now")?;
    let (sw, sh) = flat.size();
    let (w, h) = (WIDTH as usize, ((WIDTH as f64 * sh as f64 / sw.max(1) as f64).round() as usize).max(1) & !1);
    let lut = cut.lut.as_deref().and_then(crate::lut::find).map(|p| crate::lut::Lut::load(&p)).transpose()?;
    // The GPU's decoder where there is one: a dozen seeks into 4K HEVC take
    // most of a minute on the CPU.
    let hw = crate::ffmpeg::Hwaccel::pick(&cut.input, flat.track).unwrap_or(crate::ffmpeg::Hwaccel::Software);
    let mut pixels = Vec::new();
    for k in 0..FRAMES {
        let t = cut.start + (cut.end - cut.start) * (k as f64 + 0.5) / FRAMES as f64;
        let frame = match crate::ffmpeg::still_out(&cut.input, flat.track, t, hw, (w, h), flat.out(Some((w as u32, h as u32)), None)) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let strength = crate::grade::grade_at(&cut.effects, t).lut.clamp(0.0, 1.0);
        pixels.extend(frame.rgb.as_chunks::<3>().0.iter().map(|p| {
            let c = [p[0], p[1], p[2]].map(|v| v as f64 / 255.0);
            lut.as_ref().map_or(c, |l| l.blend(c, strength))
        }));
    }
    if pixels.is_empty() {
        bail!("no frame of {} could be read", cut.input.display());
    }
    Ok(measure(pixels.into_iter()))
}

/// Cut `cut`'s correction as ops: its effect values set for the whole of
/// it (`KeyOp::EffectValue`), the levels alone if `levels`.
pub fn ops(cut: crate::project::Id, c: &Correction, levels: bool) -> Vec<crate::model::ops::Op> {
    use crate::model::ops::{Op, keys::{Graded, KeyOp}};
    let values: Vec<(usize, f64)> = if levels { c.levels().to_vec() } else { c.values().to_vec() };
    values.into_iter().map(|(k, value)| Op::Key(KeyOp::EffectValue { on: Graded::Cut(cut), param: crate::grade::NAMES[k].to_owned(), value })).collect()
}

/// `omnix autocolour`: each cut (all, or those with ids `only`) measured
/// and corrected, a line each; one that can't be read is left
/// as it was. Writes the project to `output` if given.
pub fn run(project: &std::path::Path, only: &[crate::project::Id], levels: bool, print_ops: bool, output: Option<&std::path::Path>) -> Result<()> {
    let p = crate::project::Project::load(project)?;
    let mut all = Vec::new();
    for cut in p.cuts_by_id(only)?.into_iter().map(|i| &p.cuts[i]) {
        let name = cut.input.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        match measure_cut(cut) {
            Ok(m) => {
                let c = correction(&m);
                crate::apply::say(print_ops, &format!(
                    "cut #{:<3} {name}  black {:.0} white {:.0} middle {:.0}{}  →  black_in {:.3} white_in {:.3} exposure {:+.2} temperature {:+.2} tint {:+.2}",
                    cut.id,
                    m.black * 255.0,
                    m.white * 255.0,
                    m.mid * 255.0,
                    if m.grey.is_some() { "" } else { " (nothing grey)" },
                    c.black_in,
                    c.white_in,
                    c.exposure,
                    c.temperature,
                    c.tint
                ));
                all.extend(ops(cut.id, &c, levels));
            }
            Err(e) => crate::apply::say(print_ops, &format!("cut #{:<3} {name}  left as it is: {e:#}", cut.id)),
        }
    }
    crate::apply::finish(p, &all, print_ops, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_picture_gets_its_black_and_white() {
        // From 14 to 230 of 255, its middle low: levels and some exposure.
        let pixels = (14..=230).map(|v| [v as f64 / 255.0; 3]);
        let m = measure(pixels);
        assert!((m.black - 14.0 / 255.0).abs() < 0.01 && (m.white - 230.0 / 255.0).abs() < 0.01, "{m:?}");
        let c = correction(&m);
        assert!((c.black_in - m.black).abs() < 1e-9 && (c.white_in - m.white).abs() < 1e-9);
        assert_eq!((c.temperature, c.tint), (0.0, 0.0), "grey is grey already");
        // A picture already full and middling is left alone.
        let full = measure((0..=255).map(|v| [v as f64 / 255.0; 3]));
        assert_eq!(correction(&full), Correction { black_in: 0.0, white_in: 1.0, exposure: 0.0, temperature: 0.0, tint: 0.0 });
    }

    #[test]
    fn a_blue_cast_on_grey_is_warmed_and_a_dark_picture_lifted_within_reach() {
        let cast = (0..200).map(|v| 0.2 + v as f64 * 0.003).map(|b| [b * 0.93, b, b * 1.15]);
        let c = correction(&measure(cast));
        assert!(c.temperature > 0.1, "warmer: {c:?}");
        let dark = correction(&measure((0..200).map(|v| [0.02 + v as f64 * 0.001; 3])));
        assert_eq!(dark.exposure, MOST_STOPS, "held within a stop");
    }
}
