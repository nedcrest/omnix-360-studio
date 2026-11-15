//! How fast a cut reads its clip (gap 31): its rate over its clip time,
//! stored as a curve from the start so the speed window (31c) changes no
//! format. A key sets the rate from its clip time on, held to the next;
//! before the first, the first's rate. No keys is normal speed, one key a
//! constant. Rates from 0.25× to 4×.
//!
//! The time seam (`docs/omnix/TIME-SEAM.md`) turns clip time into edit
//! time and back through here; the player's clock, the decoders' frame
//! picks and the sound's tempo (`ffmpeg::tempo`) read it too.

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
}

impl Speed {
    /// One rate throughout.
    pub fn constant(rate: f64) -> Speed {
        Speed { keys: vec![SpeedKey { t: 0.0, rate }], tape: false }
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
    /// each one's clip span and rate.
    pub fn pieces(&self, a: f64, b: f64) -> Vec<(f64, f64, f64)> {
        let mut out = Vec::new();
        let mut t = a;
        for c in self.changes().filter(|&c| c > a && c < b) {
            out.push((t, c, self.after(t)));
            t = c;
        }
        out.push((t, b, self.after(t)));
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
        if self.keys.windows(2).any(|w| w[1].t <= w[0].t) {
            return Err("the speed's keys are out of order".into());
        }
        Ok(())
    }

    /// The rate just after clip time `t`.
    fn after(&self, t: f64) -> f64 {
        self.keys.iter().rev().find(|k| k.t <= t).or(self.keys.first()).map_or(1.0, |k| k.rate)
    }

    /// The rate just before clip time `t`.
    fn before(&self, t: f64) -> f64 {
        self.keys.iter().rev().find(|k| k.t < t).or(self.keys.first()).map_or(1.0, |k| k.rate)
    }

    /// Where the rate changes: every key's time but the first's.
    fn changes(&self) -> impl DoubleEndedIterator<Item = f64> + '_ {
        self.keys.iter().skip(1).map(|k| k.t)
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
            secs += (c - t) / self.after(t);
            t = c;
        }
        secs + (b - t) / self.after(t)
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
                let piece = (c - t) / self.after(t);
                if piece >= left {
                    break;
                }
                (left, t) = (left - piece, c);
            }
            t + left * self.after(t)
        } else {
            for c in self.changes().rev().filter(|&c| c < a) {
                let piece = (t - c) / self.before(t);
                if piece >= left {
                    break;
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
        Speed { keys: vec![SpeedKey { t: 0.0, rate: 1.0 }, SpeedKey { t: 2.0, rate: 2.0 }, SpeedKey { t: 4.0, rate: 0.5 }], tape: false }
    }

    #[test]
    fn normal_speed_is_the_clip_as_it_is_exactly() {
        for s in [Speed::default(), Speed::constant(1.0)] {
            assert!(s.is_normal());
            // Not near: equal, so the seam changes no number at 1×.
            assert_eq!((s.edit_secs(10.3, 14.7), s.clip_after(10.3, 4.4), s.clip_after(14.7, -4.4)), (14.7 - 10.3, 10.3 + 4.4, 14.7 - 4.4));
        }
        // Several keys all at 1×: as exact, not a sum of pieces.
        let ones = Speed { keys: vec![SpeedKey { t: 0.0, rate: 1.0 }, SpeedKey { t: 11.1, rate: 1.0 }, SpeedKey { t: 12.9, rate: 1.0 }], tape: false };
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
        assert!(Speed { keys: vec![SpeedKey { t: 2.0, rate: 1.0 }, SpeedKey { t: 1.0, rate: 2.0 }], tape: false }.check().is_err());
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
}
