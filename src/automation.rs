//! Automation clips (gap 39, the user, 2026-11-09): a value moving over
//! time is a clip on that value's lane, as in FL Studio — a stretch of a
//! cut (or an overlay item) holding a curve of points. Outside its clips a
//! value holds what the Effects panel set for the whole cut
//! (`KeyOp::EffectValue`). A clip is the cut's, in its clip time: it moves
//! and trims with it. Text and overlays have their own, over their effects
//! and their place.
//!
//! The one answer to «what does it look like at t»: `graded` and `placed`
//! put a cut's or an item's clips over its value (`Cut::grade_at`,
//! `overlay::look`), read by the player, the export and the overlays alike.

use crate::grade::{EffectKey, Grade};
use crate::overlay::{MotionKey, Place};
use crate::view::Ease;
use serde::{Deserialize, Serialize};

/// A value moved over a stretch: the value's name, where the clip runs
/// (clip time, `from` before `to`) and its curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoClip {
    /// What it moves: an effect (`grade::NAMES`: "exposure", "vignette"…)
    /// or, on an overlay item, a place value (`Place::NAMES`: "x", "y",
    /// "size", "rotation").
    pub param: String,
    pub from: f64,
    pub to: f64,
    /// Its curve, in time order: held before the first point and after the
    /// last, eased from each to the next.
    pub points: Vec<AutoPoint>,
    /// The tracked object it was made from (`AutoOp::Follow`), so it can be
    /// made again when its cut moves; none: drawn by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<Follows>,
    /// The in or out it was made by (`animate`), kept on its edge as the
    /// item is trimmed; none: drawn by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<crate::animate::Animation>,
}

/// Where a clip that follows an object came from: cut `cut`'s track
/// `track`, and what was added to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Follows {
    pub cut: crate::project::Id,
    pub track: String,
    #[serde(default)]
    pub offset: f64,
}

/// An object followed through a cut's footage (the tracking contract, gap
/// 37): its name and where it is over time. A tracker writes it (or an
/// agent, `CutOp::Track`); an overlay or text is tied to it by
/// `AutoOp::Follow`, which makes its place's automation clips from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectTrack {
    /// Unique within its cut («ball», «face 1»).
    pub name: String,
    /// In clip time order.
    pub samples: Vec<TrackSample>,
}

/// Where a tracked object is at clip time `t`: its centre across and down
/// the cut's frame as it's shown (0..1, as an overlay's place), its size
/// (the share of the frame's height) and turn (degrees) when the tracker
/// knows them, and how sure it is (0..1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrackSample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default = "sure", skip_serializing_if = "is_sure")]
    pub confidence: f64,
}

fn sure() -> f64 {
    1.0
}

fn is_sure(v: &f64) -> bool {
    *v == 1.0
}

impl TrackSample {
    /// Its value for an overlay's place value `name` (`Place::NAMES`), if
    /// the tracker gave one.
    pub fn value(&self, name: &str) -> Option<f64> {
        match name {
            "x" => Some(self.x),
            "y" => Some(self.y),
            "size" => self.size,
            "rotation" => self.rotation,
            _ => None,
        }
    }
}

/// A point of a clip's curve: clip time, the value, how it moves on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoPoint {
    pub t: f64,
    pub v: f64,
    #[serde(default, skip_serializing_if = "is_smooth")]
    pub ease: Ease,
}

fn is_smooth(e: &Ease) -> bool {
    *e == Ease::Smooth
}

impl AutoClip {
    pub fn covers(&self, t: f64) -> bool {
        t >= self.from && t <= self.to
    }

    /// Trimmed to `from`..`to`: the outermost point on a side that moves
    /// goes with its edge, wherever it sat (a point dragged near the edge
    /// sits a hair inside it); inner points left outside are dropped (the
    /// user's finds, 2026-11-10: edge points stayed behind, or were
    /// swallowed).
    pub fn trim(&mut self, from: f64, to: f64) {
        let last = self.points.len().saturating_sub(1);
        if let Some(q) = self.points.first_mut().filter(|_| from != self.from) {
            q.t = from;
        }
        if let Some(q) = self.points.get_mut(last).filter(|_| to != self.to && last > 0) {
            q.t = to;
        }
        self.points.retain(|q| q.t >= from - 1e-9 && q.t <= to + 1e-9);
        (self.from, self.to) = (from, to);
    }

    /// Its curve's value at `t`.
    pub fn value_at(&self, t: f64) -> f64 {
        let p = &self.points;
        let i = p.partition_point(|q| q.t <= t);
        if i == 0 || i == p.len() {
            return p.get(i.saturating_sub(1)).map_or(0.0, |q| q.v);
        }
        let (a, b) = (p[i - 1], p[i]);
        match a.ease.shape(((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0)) {
            Some(u) => a.v + (b.v - a.v) * u,
            None => a.v,
        }
    }
}

/// Where clip `from..to`'s start (`start`) or end may go for `t`, within
/// its owner's `range` and keeping `shortest` of it; none when there's no
/// room (a clip hanging past an edge its owner was trimmed to: a clamp
/// with its ends crossed panicked, audit A1).
pub fn trim_to(start: bool, (from, to): (f64, f64), range: (f64, f64), shortest: f64, t: f64) -> Option<f64> {
    let (lo, hi) = if start { (range.0, to - shortest) } else { (from + shortest, range.1) };
    (lo <= hi).then(|| t.clamp(lo, hi))
}

/// What `clips` give `param` at `t`, if one of them covers it.
pub fn value(clips: &[AutoClip], param: &str, t: f64) -> Option<f64> {
    clips.iter().find(|c| c.param == param && c.covers(t)).map(|c| c.value_at(t))
}

/// `grade` with what `clips` move at `t`.
pub fn graded(grade: Grade, clips: &[AutoClip], t: f64) -> Grade {
    if clips.is_empty() {
        return grade;
    }
    let mut values = grade.values();
    for (k, name) in crate::grade::NAMES.iter().enumerate() {
        if let Some(v) = value(clips, name, t) {
            values[k] = v;
        }
    }
    Grade { black: grade.black, ..Grade::from_values(values) }
}

/// `frame` with what `clips` move at `t` (gap 107: the Frame lanes).
pub fn framed(frame: crate::frame::Frame, clips: &[AutoClip], t: f64) -> crate::frame::Frame {
    let mut values = frame.values();
    for (k, name) in crate::frame::PARAMS.iter().enumerate() {
        if let Some(v) = value(clips, name, t) {
            values[k] = v;
        }
    }
    crate::frame::Frame::from_values(values)
}

/// `place` with what `clips` move at `t`.
pub fn placed(place: Place, clips: &[AutoClip], t: f64) -> Place {
    let mut values = place.values();
    for (k, name) in Place::NAMES.iter().enumerate() {
        if let Some(v) = value(clips, name, t) {
            values[k] = v;
        }
    }
    Place::from_values(values)
}

/// Keys from before automation clips (effect keys, motion keys) made into
/// clips: each value that changes from key to key gets a clip over the
/// whole of its cut or item (`from`..`to`, so it holds the first key's
/// value before it and the last's after, as the keys did) with a point at
/// each key; the keys keep one, the first, for the values that don't move.
/// One key or none: as it is.
pub fn from_keys<K: Keyed>(keys: &mut Vec<K>, from: f64, to: f64) -> Vec<AutoClip> {
    if keys.len() < 2 {
        return Vec::new();
    }
    let (from, to) = (from.min(keys[0].time()), to.max(keys[keys.len() - 1].time()));
    let mut clips = Vec::new();
    for (k, &name) in K::NAMES.iter().enumerate() {
        let first = keys[0].values()[k];
        if keys.iter().all(|key| key.values()[k] == first) {
            continue;
        }
        let points = keys.iter().map(|key| AutoPoint { t: key.time(), v: key.values()[k], ease: key.ease() }).collect();
        clips.push(AutoClip { param: name.to_owned(), from, to, points, follows: None, animation: None });
    }
    keys.truncate(1);
    clips
}

/// `clips` from keys added to `into`, but for a value it automates already.
pub fn adopt(into: &mut Vec<AutoClip>, clips: Vec<AutoClip>) {
    for c in clips {
        if !into.iter().any(|have| have.param == c.param) {
            into.push(c);
        }
    }
}

/// A key `from_keys` reads: its time, its values by name, its ease.
pub trait Keyed {
    const NAMES: &'static [&'static str];
    fn time(&self) -> f64;
    fn values(&self) -> Vec<f64>;
    fn ease(&self) -> Ease;
}

impl Keyed for EffectKey {
    const NAMES: &'static [&'static str] = &crate::grade::NAMES;
    fn time(&self) -> f64 {
        self.t
    }
    fn values(&self) -> Vec<f64> {
        self.grade.values().to_vec()
    }
    fn ease(&self) -> Ease {
        self.ease
    }
}

impl Keyed for MotionKey {
    const NAMES: &'static [&'static str] = &Place::NAMES;
    fn time(&self) -> f64 {
        self.t
    }
    fn values(&self) -> Vec<f64> {
        self.place.values().to_vec()
    }
    fn ease(&self) -> Ease {
        self.ease
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(param: &str, from: f64, to: f64, points: &[(f64, f64)]) -> AutoClip {
        AutoClip { param: param.into(), from, to, points: points.iter().map(|&(t, v)| AutoPoint { t, v, ease: Ease::Linear }).collect(), follows: None, animation: None }
    }

    #[test]
    fn a_clip_moves_its_value_over_its_stretch_and_the_value_holds_elsewhere() {
        let clips = [clip("exposure", 2.0, 4.0, &[(2.0, 0.0), (4.0, 1.0)]), clip("vignette", 1.0, 5.0, &[(3.0, 0.6)])];
        let base = Grade { exposure: -0.5, contrast: 0.2, ..Default::default() };
        assert_eq!(graded(base, &clips, 3.0).exposure, 0.5, "half way, linear");
        assert_eq!(graded(base, &clips, 1.5).exposure, -0.5, "before its clip: the value set");
        assert_eq!((graded(base, &clips, 1.5).vignette, graded(base, &clips, 4.5).contrast), (0.6, 0.2), "held from one point; the rest as set");
        assert_eq!(placed(Place::default(), &[clip("x", 0.0, 1.0, &[(0.0, 0.1)])], 0.5).x, 0.1);
    }

    #[test]
    fn keys_from_before_become_a_clip_for_each_value_that_moves() {
        let key = |t, exposure, ease| EffectKey { t, grade: Grade { exposure, contrast: 0.3, ..Default::default() }, ease };
        let mut keys = vec![key(1.0, 0.0, Ease::Linear), key(3.0, 1.0, Ease::Hold), key(5.0, 0.5, Ease::Smooth)];
        let clips = from_keys(&mut keys, 0.0, 10.0);
        assert_eq!(clips.len(), 1, "contrast never moves: {clips:?}");
        assert_eq!((clips[0].param.as_str(), clips[0].from, clips[0].to, clips[0].points.len(), clips[0].points[1].ease), ("exposure", 0.0, 10.0, 3, Ease::Hold));
        assert_eq!((keys.len(), keys[0].grade.contrast), (1, 0.3), "the first key holds elsewhere");
        // It looks as it did.
        for t in [0.0, 1.0, 2.0, 3.5, 4.2, 6.0] {
            let before = crate::grade::grade_at(&[key(1.0, 0.0, Ease::Linear), key(3.0, 1.0, Ease::Hold), key(5.0, 0.5, Ease::Smooth)], t).exposure;
            assert!((graded(keys[0].grade, &clips, t).exposure - before).abs() < 1e-12, "at {t}");
        }
        assert!(from_keys(&mut vec![key(1.0, 0.0, Ease::Linear)], 0.0, 10.0).is_empty());
    }
}
