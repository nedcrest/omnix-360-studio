//! Operations: every change to a project as data (`OPS-API.md`), applied
//! the same way by the player, `omnix apply`, an AI agent and the
//! tests. One enum per area, each in its own file with its own `apply`;
//! a new area is a new file and a line in `Op`.
//!
//! An op names what it changes by id, never by index, and carries every
//! value it needs: no playhead, selection or setting is read here. Values
//! are absolute, and ops derived from the sound start from the original,
//! so applying one twice is applying it once.

pub mod auto;
pub mod clips;
pub mod cuts;
pub mod items;
pub mod keys;
pub mod project;
pub mod sound;
#[cfg(test)]
mod props;

use super::edit::Edit;
use crate::project::{Id, Own, Project, SoundTrack};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "area", rename_all = "snake_case")]
pub enum Op {
    /// The edit's cuts: added, trimmed, moved, split, their joins, edges,
    /// styles, markers and settings.
    Cut(cuts::CutOp),
    /// Sound clips on the music, SFX and voice tracks.
    Clip(clips::ClipOp),
    /// Pictures, text and bars over the picture, and their lanes.
    Item(items::ItemOp),
    /// Keys: a cut's view, a cut's or item's effects, an item's place.
    Key(keys::KeyOp),
    /// The sound of a cut or a sound clip, the tracks' levels, the edit's
    /// fades, the music ducked under speech.
    Sound(sound::SoundOp),
    /// Automation clips (gap 39).
    Auto(auto::AutoOp),
    /// The edit as a whole: its aspect.
    Project(project::ProjectOp),
}

/// A sound that has its own level, line and fades: a cut's or a sound
/// clip's, by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoundRef {
    Cut(Id),
    Clip(Id),
}

impl SoundRef {
    /// Where it is in `p` now.
    pub fn own(self, p: &Project) -> Option<Own> {
        match self {
            SoundRef::Cut(id) => p.cut_index(id).map(Own::Cut),
            SoundRef::Clip(id) => p.clip_index(id).map(|(w, i)| Own::Clip(w, i)),
        }
    }

    /// The ref for `own` in `p`.
    pub fn of(own: Own, p: &Project) -> Option<SoundRef> {
        match own {
            Own::Cut(i) => p.cuts.get(i).map(|c| SoundRef::Cut(c.id)),
            Own::Clip(w, i) => p.track(w).clips.get(i).map(|c| SoundRef::Clip(c.id)),
        }
    }
}

impl fmt::Display for SoundRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundRef::Cut(id) => write!(f, "cut #{id}"),
            SoundRef::Clip(id) => write!(f, "sound clip #{id}"),
        }
    }
}

/// What an op did, in words for a person or an agent («sound clip #5:
/// level −5.0 → −8.0 dB»), and the id of what it made, if it made one (an
/// added cut, a split's second half).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub summary: String,
    pub made: Option<Id>,
}

/// An outcome that made nothing.
fn said(summary: String) -> Result<Outcome, OpError> {
    Ok(Outcome { summary, made: None })
}

/// Which end of a cut, a clip or a fade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum End {
    In,
    Out,
}

/// Why an op was refused. The project is as it was.
#[derive(Clone, Debug, PartialEq)]
pub enum OpError {
    NotFound(String),
    /// It can't be done here, and why.
    Invalid(String),
    OutOfRange { what: &'static str, value: f64, least: f64, most: f64 },
}

/// A range's end as said: «∞» for none.
fn bound(v: f64) -> String {
    match v {
        f64::MAX => "∞".to_owned(),
        f64::MIN => "−∞".to_owned(),
        _ => v.to_string(),
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::NotFound(what) => write!(f, "no {what}"),
            OpError::Invalid(why) => write!(f, "{why}"),
            OpError::OutOfRange { what, value, least, most } => write!(f, "{what} {value} is outside {}..{}", bound(*least), bound(*most)),
        }
    }
}

impl std::error::Error for OpError {}

/// Applies `op` to `p`, or leaves it as it was and says why.
fn apply(p: &mut Project, op: &Op) -> Result<Outcome, OpError> {
    match op {
        // A cut made shorter or longer moves the sync-locked tracks after
        // it (gap 99).
        Op::Cut(op) => {
            let before = p.cuts.clone();
            let outcome = cuts::apply(p, op)?;
            if let Some(ripple) = crate::project::ripple_of(&before, &p.cuts) {
                p.ripple_locked(ripple);
            }
            Ok(outcome)
        }
        Op::Clip(op) => clips::apply(p, op),
        Op::Item(op) => items::apply(p, op),
        Op::Key(op) => keys::apply(p, op),
        Op::Sound(op) => sound::apply(p, op),
        Op::Auto(op) => auto::apply(p, op),
        Op::Project(op) => project::apply(p, op),
    }
}

impl Edit {
    /// Applies `op` as one undo step (none if it changed nothing), or
    /// within the change under way (`begin`, a drag). Refused, the edit is
    /// as it was.
    pub fn apply(&mut self, op: &Op) -> Result<Outcome, OpError> {
        let mut after = Project::clone(self);
        let outcome = apply(&mut after, op)?;
        after.settle_ids();
        if after != **self {
            if self.changing() {
                self.put(after);
            } else {
                self.attempt(|e| {
                    e.put(after);
                    true
                });
            }
        }
        Ok(outcome)
    }

    /// Applies `ops` in order as one undo step, all or none: the first
    /// refused leaves the edit as it was.
    pub fn batch(&mut self, ops: &[Op]) -> Result<Vec<Outcome>, (usize, OpError)> {
        let mut after = Project::clone(self);
        let mut outcomes = Vec::new();
        for (n, op) in ops.iter().enumerate() {
            outcomes.push(apply(&mut after, op).map_err(|e| (n, e))?);
            after.settle_ids();
        }
        if after != **self {
            self.attempt(|e| {
                e.put(after);
                true
            });
        }
        Ok(outcomes)
    }
}

/// What must hold in any project an op leaves: ids given and unique,
/// stretches the right way round, volume points in time order and
/// numbers. The tests run it after every op, undo and redo.
/// Automation clips that have a length and a point, points in time order,
/// and no two of one value overlapping.
/// Value `v` of `param`, an effect (`grade::EFFECTS`) or a place value
/// (`Place::RANGES`), within its range (audit C4: exposure 1000 was taken).
pub(super) fn in_range(param: &str, v: f64) -> Result<(), OpError> {
    use crate::overlay::Place;
    let range = crate::grade::NAMES.iter().position(|n| *n == param).map(|k| crate::grade::RANGES[k]).or_else(|| Place::NAMES.iter().position(|n| *n == param).map(|k| Place::RANGES[k]));
    match range {
        Some((lo, hi)) if (lo..=hi).contains(&v) => Ok(()),
        Some((lo, hi)) => Err(OpError::Invalid(format!("{param} {v} is outside {lo}..{hi}"))),
        None => Err(OpError::NotFound(format!("value {param:?}"))),
    }
}

/// Every value of `grade` within its range.
pub(super) fn grade_in_range(grade: &crate::grade::Grade) -> Result<(), OpError> {
    crate::grade::NAMES.iter().zip(grade.values()).try_for_each(|(n, v)| in_range(n, v))
}

/// Every value of `place` within its range.
pub(super) fn place_in_range(place: &crate::overlay::Place) -> Result<(), OpError> {
    crate::overlay::Place::NAMES.iter().zip(place.values()).try_for_each(|(n, v)| in_range(n, v))
}

/// Time `t` within `span` (clip or own time), give or take `tolerance`:
/// a new key past its cut or item would never show. (One there already
/// may be changed: a split leaves each half the other's keys.)
pub(super) fn in_span(what: &str, t: f64, (from, to): (f64, f64), tolerance: f64) -> Result<(), OpError> {
    if t >= from - tolerance && t <= to + tolerance {
        Ok(())
    } else {
        Err(OpError::Invalid(format!("{what} at {t:.2} s is outside {from:.2}-{to:.2} s")))
    }
}

fn automation(what: &str, clips: &[crate::automation::AutoClip]) -> Result<(), String> {
    for (i, c) in clips.iter().enumerate() {
        if c.to.partial_cmp(&c.from) != Some(std::cmp::Ordering::Greater) || c.points.is_empty() || c.points.windows(2).any(|w| w[0].t > w[1].t) || c.points.iter().any(|q| !q.v.is_finite()) {
            return Err(format!("{what}'s {} clip {:.2}-{:.2} s is empty or out of order", c.param, c.from, c.to));
        }
        if clips[..i].iter().any(|o| o.param == c.param && c.from < o.to && c.to > o.from) {
            return Err(format!("{what}'s {} clips overlap", c.param));
        }
    }
    Ok(())
}

pub fn check(p: &Project) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut id = |what: &str, id: Id| if id == 0 || !ids.insert(id) { Err(format!("{what} has id {id}, not given or taken")) } else { Ok(()) };
    let line = |what: String, keys: &[crate::project::VolumeKey]| {
        if keys.iter().any(|k| !k.t.is_finite() || !k.volume.is_finite()) || keys.windows(2).any(|w| w[0].t > w[1].t) { Err(format!("{what}'s volume points aren't numbers in time order")) } else { Ok(()) }
    };
    for c in &p.cuts {
        id("a cut", c.id)?;
        if c.end <= c.start {
            return Err(format!("cut #{} ends before it starts", c.id));
        }
        line(format!("cut #{}", c.id), &c.volume)?;
        let (from, to) = c.sound_span();
        if !(from >= -1e-9 && to - from >= crate::model::edit::MIN_CUT - 1e-9) {
            return Err(format!("cut #{}'s sound runs {from:.3}-{to:.3} s: before its clip or too short", c.id));
        }
        if c.marks.iter().any(|m| !m.t.is_finite()) || c.marks.windows(2).any(|w| w[0].t > w[1].t) {
            return Err(format!("cut #{}'s markers aren't numbers in time order", c.id));
        }
        automation(&format!("cut #{}", c.id), &c.automation)?;
        let numbers = c.keyframes.iter().all(|k| [k.t, k.view.yaw, k.view.pitch, k.view.roll, k.view.fov, k.view.correction].iter().all(|v| v.is_finite()));
        if !numbers || c.effects.iter().any(|k| !k.t.is_finite() || k.grade.values().iter().any(|v| !v.is_finite())) {
            return Err(format!("cut #{}'s view or effect keys aren't numbers", c.id));
        }
    }
    for w in SoundTrack::ALL {
        for c in &p.track(w).clips {
            id("a sound clip", c.id)?;
            if c.end <= c.start || c.at < 0.0 {
                return Err(format!("sound clip #{} is empty or starts before the edit", c.id));
            }
            line(format!("sound clip #{}", c.id), &c.volume)?;
        }
    }
    for i in p.overlays.iter().chain(&p.text).flat_map(|l| &l.items) {
        id("an overlay", i.id)?;
        automation(&format!("item #{}", i.id), &i.automation)?;
        if i.motion.iter().any(|k| !k.t.is_finite() || k.place.values().iter().any(|v| !v.is_finite())) || i.effects.iter().any(|k| !k.t.is_finite() || k.grade.values().iter().any(|v| !v.is_finite())) {
            return Err(format!("item #{}'s keys aren't numbers", i.id));
        }
        if i.end <= i.start {
            return Err(format!("overlay #{} ends before it starts", i.id));
        }
    }
    Ok(())
}
