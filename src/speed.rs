//! How fast a cut reads its clip (gap 31): its rate over its clip time,
//! stored as a curve from the start so the speed window (31c) changes no
//! format. A key sets the rate from its clip time on, held to the next or
//! eased there (a ramp, 31c: in log scale, its edit length the integral of
//! 1 / rate); before the first, the first's rate. No keys is normal speed,
//! one key a constant. Rates from 0.25× to 4×.
//!
//! The time seam (`docs/omnix/TIME-SEAM.md`) turns clip time into edit
//! time and back through here; the player's clock, the decoders' frame
//! picks and the sound's tempo (`ffmpeg::tempo`) read it too.

use crate::view::Ease;
use serde::{Deserialize, Serialize};

/// The slowest and the fastest rate.
pub const RANGE: (f64, f64) = (0.25, 4.0);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Speed {
    /// In clip-time order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<SpeedKey>,
    /// Its sound plays like tape, slower is lower (gap 31b); else it keeps
    /// its pitch, stretched in time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tape: bool,
}

/// Normal speed, for what is always played as it is (a title, a picture).
pub static NORMAL: Speed = Speed { keys: Vec::new(), tape: false };

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpeedKey {
    /// Clip time it starts at.
    pub t: f64,
    /// Clip seconds read per edit second: 2 is twice as fast.
    pub rate: f64,
    /// How it goes on to the next key's rate (31c): held, then a step
    /// (as speeds were stored before ramps; absent in the file), or eased
    /// there, smoothly or evenly, in log scale (½× to 2× passes 1× half
    /// way).
    #[serde(default = "held", skip_serializing_if = "is_held")]
    pub ease: Ease,
}

fn held() -> Ease {
    Ease::Hold
}

fn is_held(e: &Ease) -> bool {
    *e == Ease::Hold
}

impl SpeedKey {
    /// A key at clip time `t` holding `rate` to the next.
    pub fn held(t: f64, rate: f64) -> SpeedKey {
        SpeedKey { t, rate, ease: Ease::Hold }
    }
}

/// A ramp is read in even steps (Simpson's rule) for its edit length: at
/// least this many, and one per `STEP` clip seconds, far finer than a
/// frame, so a stretch and its pieces agree to well under a microsecond.
const STEPS: usize = 32;
const STEP: f64 = 0.005;

/// A ramp's sound plays in constant-tempo pieces about this long, clip
/// seconds (at most `MAX_PIECES` a ramp).
const PIECE: f64 = 0.25;
const MAX_PIECES: usize = 64;

impl Speed {
    /// One rate throughout.
    pub fn constant(rate: f64) -> Speed {
        Speed { keys: vec![SpeedKey::held(0.0, rate)], tape: false }
    }

    /// What a cut's label says of it: «2×», «0.5×», «speed curve»; none at
    /// normal speed.
    pub fn short(&self) -> Option<String> {
        match &self.keys[..] {
            _ if self.is_normal() => None,
            keys if keys.iter().all(|k| k.rate == keys[0].rate) => Some(format!("{}×", (keys[0].rate * 100.0).round() / 100.0)),
            _ => Some("speed curve".to_owned()),
        }
    }

    /// What it is, in words: «normal speed», «2× speed», «a speed curve
    /// of 3 keys», with «like tape» where it's so.
    pub fn describe(&self) -> String {
        let rate = |r: f64| format!("{}×", (r * 100.0).round() / 100.0);
        let what = match &self.keys[..] {
            _ if self.is_normal() => "normal speed".to_owned(),
            [k] => format!("{} speed", rate(k.rate)),
            keys if keys.iter().all(|k| k.rate == keys[0].rate) => format!("{} speed", rate(keys[0].rate)),
            keys => format!("a speed curve of {} keys, {}", keys.len(), keys.iter().map(|k| format!("{} from {:.2} s", rate(k.rate), k.t)).collect::<Vec<_>>().join(", ")),
        };
        if self.tape { format!("{what}, its sound like tape") } else { what }
    }

    /// Whether there's nothing to save: normal speed, the pitch kept.
    pub fn is_default(&self) -> bool {
        self.is_normal() && !self.tape
    }

    /// The stretches of constant rate reading the clip from `a` to `b`:
    /// each one's clip span and rate. A ramp is cut into pieces of about
    /// `PIECE`, each at the rate that gives it its edit length exactly, so
    /// the sound's pieces add up to the picture's.
    pub fn pieces(&self, a: f64, b: f64) -> Vec<(f64, f64, f64)> {
        let mut out = Vec::new();
        let mut t = a;
        for c in self.changes().filter(|&c| c > a && c < b).chain(std::iter::once(b)) {
            if self.ramp_at(t).is_some() && c.is_finite() {
                let n = (((c - t) / PIECE).ceil() as usize).clamp(1, MAX_PIECES);
                for k in 0..n {
                    let (x, y) = (t + (c - t) * k as f64 / n as f64, t + (c - t) * (k + 1) as f64 / n as f64);
                    out.push((x, y, (y - x) / self.segment_secs(x, y)));
                }
            } else {
                out.push((t, c, self.after(t)));
            }
            t = c;
        }
        out
    }

    /// Whether it reads its clip as recorded: one clip second an edit
    /// second, exactly.
    pub fn is_normal(&self) -> bool {
        self.keys.iter().all(|k| k.rate == 1.0)
    }

    /// Whether its keys are in order and their rates in `RANGE`.
    pub fn check(&self) -> Result<(), String> {
        let (lo, hi) = RANGE;
        if let Some(k) = self.keys.iter().find(|k| !(lo..=hi).contains(&k.rate)) {
            return Err(format!("the speed {}× is outside {lo}..{hi}", k.rate));
        }
        if self.keys.iter().any(|k| !k.t.is_finite()) {
            return Err("a speed key's time isn't a number".into());
        }
        if self.keys.windows(2).any(|w| w[1].t <= w[0].t) {
            return Err("the speed's keys are out of order".into());
        }
        Ok(())
    }

    /// The rate at clip time `t`, a ramp's eased one inside it.
    pub fn rate_at(&self, t: f64) -> f64 {
        match self.ramp_at(t) {
            Some((a, b)) => {
                let x = ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
                let e = a.ease.shape(x).unwrap_or(0.0);
                a.rate * (b.rate / a.rate).powf(e)
            }
            None => self.after(t),
        }
    }

    /// The ramp clip time `t` is in, if it's in one: its key and the next,
    /// the first not held.
    fn ramp_at(&self, t: f64) -> Option<(SpeedKey, SpeedKey)> {
        self.keys.windows(2).find(|w| w[0].t <= t && t < w[1].t).filter(|w| w[0].ease != Ease::Hold).map(|w| (w[0], w[1]))
    }

    /// The rate just after clip time `t`, held keys only (a ramp's is
    /// `rate_at`).
    fn after(&self, t: f64) -> f64 {
        self.keys.iter().rev().find(|k| k.t <= t).or(self.keys.first()).map_or(1.0, |k| k.rate)
    }

    /// The rate just before clip time `t`, held keys only.
    fn before(&self, t: f64) -> f64 {
        self.keys.iter().rev().find(|k| k.t < t).or(self.keys.first()).map_or(1.0, |k| k.rate)
    }

    /// Where the rate's form changes: every key's time but the first's,
    /// and the first's too where a ramp starts there.
    fn changes(&self) -> impl DoubleEndedIterator<Item = f64> + '_ {
        let first = self.keys.first().filter(|k| k.ease != Ease::Hold).map(|k| k.t);
        first.into_iter().chain(self.keys.iter().skip(1).map(|k| k.t))
    }

    /// The edit seconds from clip time `a` to `b` within one stretch of
    /// one form (`changes`): held, by its rate; in a ramp, the integral
    /// of 1 / rate.
    fn segment_secs(&self, a: f64, b: f64) -> f64 {
        if self.ramp_at((a + b) / 2.0).is_none() {
            return (b - a) / self.after(a);
        }
        let n = (((b - a) / STEP).ceil() as usize).max(STEPS).div_ceil(2) * 2;
        let h = (b - a) / n as f64;
        let f = |k: usize| 1.0 / self.rate_at(a + h * k as f64);
        let inner: f64 = (1..n).map(|k| f(k) * if k % 2 == 1 { 4.0 } else { 2.0 }).sum();
        h / 3.0 * (f(0) + inner + f(n))
    }

    /// The clip time `x` in `[a, b]` (one stretch, as `segment_secs`) where
    /// `secs(x)` reaches `left`: by its rate where held, else by halving.
    fn solve(&self, (a, b): (f64, f64), left: f64, secs: impl Fn(f64) -> f64, held: f64) -> f64 {
        if self.ramp_at((a + b) / 2.0).is_none() {
            return held;
        }
        let (mut lo, mut hi) = (a, b);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if secs(mid) < left { lo = mid } else { hi = mid }
        }
        (lo + hi) / 2.0
    }

    /// The edit seconds reading the clip from `a` to `b` takes; less than
    /// none when `b` is before `a`.
    pub fn edit_secs(&self, a: f64, b: f64) -> f64 {
        if self.is_normal() {
            return b - a;
        }
        if b < a {
            return -self.edit_secs(b, a);
        }
        let mut t = a;
        let mut secs = 0.0;
        for c in self.changes().filter(|&c| c > a && c < b) {
            secs += self.segment_secs(t, c);
            t = c;
        }
        secs + self.segment_secs(t, b)
    }

    /// The clip time reached from clip time `a` after `dt` edit seconds
    /// (back, `dt` less than none): `edit_secs` undone.
    pub fn clip_after(&self, a: f64, dt: f64) -> f64 {
        if self.is_normal() {
            return a + dt;
        }
        let (mut t, mut left) = (a, dt.abs());
        if dt >= 0.0 {
            for c in self.changes().filter(|&c| c > a) {
                let piece = self.segment_secs(t, c);
                if piece >= left {
                    return self.solve((t, c), left, |x| self.segment_secs(t, x), t + left * self.after(t));
                }
                (left, t) = (left - piece, c);
            }
            t + left * self.after(t)
        } else {
            for c in self.changes().rev().filter(|&c| c < a) {
                let piece = self.segment_secs(c, t);
                if piece >= left {
                    // Walking back: the further from `t`, the more seconds.
                    let to = t;
                    return self.solve((c, t), -left, |x| -self.segment_secs(x, to), t - left * self.before(t));
                }
                (left, t) = (left - piece, c);
            }
            t - left * self.before(t)
        }
    }

    /// The clip frame, counted from clip time `start` at `fps`, that edit
    /// frame `k` shows: at 0.5× each clip frame twice, at 2× every other
    /// one. At 1× it is `k`, without a sum.
    pub fn frame_for(&self, start: f64, k: usize, fps: f64) -> usize {
        if self.is_normal() {
            return k;
        }
        ((self.clip_after(start, k as f64 / fps) - start) * fps).round().max(0.0) as usize
    }
}

/// The speed window's ready-made curves (31c), over a cut's clip span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Down to half speed through its middle and back up.
    SlowMiddle,
    /// Gathering speed into the cut's end, up to 3×.
    RampToCut,
}

/// Two keys closer than this (clip seconds) would be one.
const KEY_GAP: f64 = 0.01;

impl Speed {
    /// A curve to edit over a cut whose clip span starts at `start`: its
    /// keys, or one at `start` holding 1× when it has none.
    pub fn for_editing(&self, start: f64) -> Speed {
        let keys = if self.keys.is_empty() { vec![SpeedKey::held(start, 1.0)] } else { self.keys.clone() };
        Speed { keys, tape: self.tape }
    }

    /// A key at clip time `t`, `rate` kept in `RANGE`, eased as `ease` to
    /// the next; one already there (within `KEY_GAP`) takes its rate.
    /// Where it is now.
    pub fn put_key(&mut self, t: f64, rate: f64, ease: Ease) -> usize {
        let rate = rate.clamp(RANGE.0, RANGE.1);
        if let Some(i) = self.keys.iter().position(|k| (k.t - t).abs() < KEY_GAP) {
            self.keys[i].rate = rate;
            return i;
        }
        let i = self.keys.iter().position(|k| k.t > t).unwrap_or(self.keys.len());
        self.keys.insert(i, SpeedKey { t, rate, ease });
        i
    }

    /// Key `i` to clip time `t` (kept between its neighbours) and `rate`
    /// (kept in `RANGE`).
    pub fn move_key(&mut self, i: usize, t: f64, rate: f64) {
        let lo = if i > 0 { self.keys[i - 1].t + KEY_GAP } else { f64::NEG_INFINITY };
        let hi = self.keys.get(i + 1).map_or(f64::INFINITY, |k| k.t - KEY_GAP);
        if let Some(k) = self.keys.get_mut(i) {
            (k.t, k.rate) = (t.clamp(lo, hi.max(lo)), rate.clamp(RANGE.0, RANGE.1));
        }
    }

    /// Key `i` taken away; the last one stays (a curve needs one).
    pub fn remove_key(&mut self, i: usize) {
        if self.keys.len() > 1 && i < self.keys.len() {
            self.keys.remove(i);
        }
    }

    /// `preset` over the clip span `start..end`, its pitch kept or not as
    /// `tape`.
    pub fn preset(preset: Preset, (start, end): (f64, f64), tape: bool) -> Speed {
        let at = |share: f64| start + (end - start) * share;
        let key = |t: f64, rate: f64, ease: Ease| SpeedKey { t, rate, ease };
        let keys = match preset {
            Preset::SlowMiddle => vec![key(start, 1.0, Ease::Smooth), key(at(0.35), 0.5, Ease::Hold), key(at(0.65), 0.5, Ease::Smooth), key(at(0.9), 1.0, Ease::Hold)],
            Preset::RampToCut => vec![key(start, 1.0, Ease::Hold), key(at(0.5), 1.0, Ease::Smooth), key(end, 3.0, Ease::Hold)],
        };
        Speed { keys, tape }
    }
}

/// What the export reads of a clip for a cut: `total` edit frames (or to
/// the clip's end) from clip time `start`, at `speed`. The CPU and the
/// GPU pick their frames through it alike.
pub struct Reading<'a> {
    pub start: f64,
    pub total: Option<usize>,
    pub speed: &'a Speed,
}

impl Reading<'_> {
    /// The clip frame edit frame `k` shows (`Speed::frame_for`).
    pub fn frame(&self, k: usize, fps: f64) -> usize {
        self.speed.frame_for(self.start, k, fps)
    }

    /// Edit frame `k`'s clip time: where its view and its looks are read.
    pub fn clip_time(&self, k: usize, fps: f64) -> f64 {
        self.speed.clip_after(self.start, k as f64 / fps)
    }

    /// Clip frame `m`'s own time: its gyro and its rolling shutter.
    pub fn frame_time(&self, m: usize, fps: f64) -> f64 {
        self.start + m as f64 / fps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1× until clip second 2, 2× to 4, 0.5× after.
    fn curve() -> Speed {
        Speed { keys: vec![SpeedKey::held(0.0, 1.0), SpeedKey::held(2.0, 2.0), SpeedKey::held(4.0, 0.5)], tape: false }
    }

    #[test]
    fn normal_speed_is_the_clip_as_it_is_exactly() {
        for s in [Speed::default(), Speed::constant(1.0)] {
            assert!(s.is_normal());
            // Not near: equal, so the seam changes no number at 1×.
            assert_eq!((s.edit_secs(10.3, 14.7), s.clip_after(10.3, 4.4), s.clip_after(14.7, -4.4)), (14.7 - 10.3, 10.3 + 4.4, 14.7 - 4.4));
        }
        // Several keys all at 1×: as exact, not a sum of pieces.
        let ones = Speed { keys: vec![SpeedKey::held(0.0, 1.0), SpeedKey::held(11.1, 1.0), SpeedKey::held(12.9, 1.0)], tape: false };
        assert_eq!((ones.edit_secs(10.3, 14.7), ones.clip_after(10.3, 4.4)), (14.7 - 10.3, 10.3 + 4.4));
        assert!(!Speed::constant(2.0).is_normal());
    }

    #[test]
    fn a_faster_clip_is_shorter_on_the_edit() {
        let (twice, half) = (Speed::constant(2.0), Speed::constant(0.5));
        assert_eq!((twice.edit_secs(10.0, 14.0), half.edit_secs(10.0, 14.0)), (2.0, 8.0));
        assert_eq!((twice.clip_after(10.0, 1.5), twice.clip_after(14.0, -1.5)), (13.0, 11.0));
        assert_eq!(twice.edit_secs(14.0, 10.0), -2.0, "backwards");
    }

    #[test]
    fn a_curve_adds_its_pieces_and_comes_back() {
        let s = curve();
        // 0-2 at 1× (2 s), 2-4 at 2× (1 s), 4-5 at 0.5× (2 s).
        assert_eq!(s.edit_secs(0.0, 5.0), 5.0);
        assert_eq!((s.edit_secs(1.0, 3.0), s.edit_secs(3.0, 4.5)), (1.5, 1.5));
        assert_eq!(s.pieces(1.0, 4.5), vec![(1.0, 2.0, 1.0), (2.0, 4.0, 2.0), (4.0, 4.5, 0.5)]);
        assert_eq!((s.short(), Speed::constant(0.5).short(), Speed::default().short()), (Some("speed curve".into()), Some("0.5×".into()), None));
        assert_eq!(Speed::default().pieces(1.0, 3.0), vec![(1.0, 3.0, 1.0)]);
        assert_eq!((s.clip_after(0.0, 2.5), s.clip_after(0.0, 4.0), s.clip_after(5.0, -2.5)), (3.0, 4.5, 3.0));
        // Round trips anywhere, either way.
        for a in [0.0, 1.7, 2.0, 3.3, 4.0, 6.1] {
            for b in [0.4, 2.0, 2.9, 4.0, 5.5] {
                let back = s.clip_after(a, s.edit_secs(a, b));
                assert!((back - b).abs() < 1e-12, "{a} to {b}: {back}");
            }
        }
    }

    #[test]
    fn a_speed_out_of_range_or_order_is_refused() {
        assert!(Speed::constant(5.0).check().is_err() && Speed::constant(0.2).check().is_err());
        assert!(Speed { keys: vec![SpeedKey::held(2.0, 1.0), SpeedKey::held(1.0, 2.0)], tape: false }.check().is_err());
        assert!(Speed { keys: vec![SpeedKey::held(f64::NAN, 1.0)], tape: false }.check().is_err());
        assert!(curve().check().is_ok() && Speed::default().check().is_ok());
    }

    #[test]
    fn each_edit_frame_shows_the_clip_frame_its_time_reads() {
        let picks = |s: &Speed| (0..8).map(|k| s.frame_for(10.0, k, 30.0)).collect::<Vec<_>>();
        assert_eq!(picks(&Speed::default()), (0..8).collect::<Vec<_>>());
        assert_eq!(picks(&Speed::constant(2.0)), vec![0, 2, 4, 6, 8, 10, 12, 14]);
        // Half speed: each frame twice (k/2 rounds half away from none).
        assert_eq!(picks(&Speed::constant(0.5)), vec![0, 1, 1, 2, 2, 3, 3, 4]);
        // Never back, through a curve's changes.
        let c = curve();
        let along: Vec<usize> = (0..200).map(|k| c.frame_for(0.0, k, 30.0)).collect();
        assert!(along.windows(2).all(|w| w[1] >= w[0]));
        let r = Reading { start: 10.0, total: Some(8), speed: &Speed::constant(2.0) };
        assert_eq!((r.frame(3, 30.0), r.clip_time(3, 30.0), r.frame_time(6, 30.0)), (6, 10.2, 10.2));
    }

    /// 31c: a ramp from 1× at clip 0 s to 2× at 2 s, even in log scale:
    /// its edit length is the integral of 1 / rate (1 / ln 2), half way it
    /// runs at √2×, every time comes back, and its sound's pieces add up
    /// to its length. A held key is written as before, without an ease.
    #[test]
    fn a_ramp_eases_its_rate_and_its_times_add_up() {
        let ramp = |ease| Speed { keys: vec![SpeedKey { t: 0.0, rate: 1.0, ease }, SpeedKey::held(2.0, 2.0)], tape: false };
        let s = ramp(Ease::Linear);
        assert!((s.edit_secs(0.0, 2.0) - 1.0 / 2f64.ln()).abs() < 1e-8, "{}", s.edit_secs(0.0, 2.0));
        assert!((s.edit_secs(0.0, 4.0) - (1.0 / 2f64.ln() + 1.0)).abs() < 1e-8);
        assert!((s.rate_at(1.0) - 2f64.sqrt()).abs() < 1e-12 && (ramp(Ease::Smooth).rate_at(1.0) - 2f64.sqrt()).abs() < 1e-12);
        assert_eq!((s.rate_at(-1.0), s.rate_at(3.0)), (1.0, 2.0));
        for sp in [ramp(Ease::Linear), ramp(Ease::Smooth)] {
            for a in [-0.5, 0.0, 0.7, 1.9, 2.5] {
                for b in [-0.2, 0.3, 1.0, 2.0, 3.1] {
                    let back = sp.clip_after(a, sp.edit_secs(a, b));
                    assert!((back - b).abs() < 1e-9, "{a} to {b}: {back}");
                }
            }
            let pieces = sp.pieces(0.0, 3.0);
            assert_eq!(pieces.len(), 8 + 1, "the ramp in quarter seconds, then the held 2×");
            let secs: f64 = pieces.iter().map(|&(x, y, r)| (y - x) / r).sum();
            assert!((secs - sp.edit_secs(0.0, 3.0)).abs() < 1e-8 && pieces.windows(2).all(|w| w[0].1 == w[1].0));
        }
        let json = serde_json::to_string(&Speed { keys: vec![SpeedKey::held(0.0, 2.0), SpeedKey { t: 1.0, rate: 1.0, ease: Ease::Linear }], tape: false }).unwrap();
        assert_eq!(json, r#"{"keys":[{"t":0.0,"rate":2.0},{"t":1.0,"rate":1.0,"ease":"linear"}]}"#);
    }

    /// 31c's editing rules: a point put in order (one already there takes
    /// the new rate), moved only between its neighbours and within the
    /// range, the last one kept; the presets are valid curves that slow
    /// the middle and speed up the end.
    #[test]
    fn the_speed_window_edits_a_curve_within_its_rules() {
        let mut s = Speed::default().for_editing(10.0);
        assert_eq!(s.keys, [SpeedKey::held(10.0, 1.0)]);
        assert_eq!(s.put_key(12.0, 9.0, Ease::Smooth), 1);
        assert_eq!(s.keys[1].rate, RANGE.1, "kept in range");
        assert_eq!(s.put_key(11.0, 0.5, Ease::Hold), 1, "in order");
        assert_eq!((s.put_key(11.005, 2.0, Ease::Hold), s.keys.len(), s.keys[1].rate), (1, 3, 2.0), "the one there");
        s.move_key(1, 20.0, 0.1);
        assert!((s.keys[1].t - (12.0 - KEY_GAP)).abs() < 1e-12 && s.keys[1].rate == RANGE.0, "between its neighbours");
        assert!(s.check().is_ok());
        for _ in 0..5 {
            s.remove_key(0);
        }
        assert_eq!(s.keys.len(), 1, "one stays");
        let span = (10.0, 20.0);
        let slow = Speed::preset(Preset::SlowMiddle, span, false);
        let ramp = Speed::preset(Preset::RampToCut, span, true);
        assert!(slow.check().is_ok() && ramp.check().is_ok() && ramp.tape);
        assert!(slow.edit_secs(10.0, 20.0) > 10.0 && slow.rate_at(15.0) == 0.5, "the middle slowed");
        assert!(ramp.edit_secs(10.0, 20.0) < 10.0 && ramp.rate_at(19.9) > 2.5, "faster into the cut");
    }
}
