//! Operations: every change to a project as data (`OPS-API.md`), applied
//! the same way by the player, `rosmo360 apply`, an AI agent and the
//! tests. One enum per area, each in its own file with its own `apply`;
//! a new area is a new file and a line in `Op`.
//!
//! An op names what it changes by id, never by index, and carries every
//! value it needs: no playhead, selection or setting is read here. Values
//! are absolute, and ops derived from the sound start from the original,
//! so applying one twice is applying it once.

pub mod clips;
pub mod cuts;
pub mod items;
pub mod keys;
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
    Cut(cuts::CutOp),
    Clip(clips::ClipOp),
    Item(items::ItemOp),
    Key(keys::KeyOp),
    Sound(sound::SoundOp),
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
            SoundRef::Cut(id) => p.cuts.iter().position(|c| c.id == id).map(Own::Cut),
            SoundRef::Clip(id) => SoundTrack::ALL.into_iter().find_map(|w| p.track(w).clips.iter().position(|c| c.id == id).map(|i| Own::Clip(w, i))),
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

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::NotFound(what) => write!(f, "no {what}"),
            OpError::Invalid(why) => write!(f, "{why}"),
            OpError::OutOfRange { what, value, least, most } => write!(f, "{what} {value} is outside {least}..{most}"),
        }
    }
}

impl std::error::Error for OpError {}

/// Applies `op` to `p`, or leaves it as it was and says why.
fn apply(p: &mut Project, op: &Op) -> Result<Outcome, OpError> {
    match op {
        Op::Cut(op) => cuts::apply(p, op),
        Op::Clip(op) => clips::apply(p, op),
        Op::Item(op) => items::apply(p, op),
        Op::Key(op) => keys::apply(p, op),
        Op::Sound(op) => sound::apply(p, op),
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
                **self = after;
            } else {
                self.attempt(|e| {
                    **e = after;
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
                **e = after;
                true
            });
        }
        Ok(outcomes)
    }
}

/// What must hold in any project an op leaves: ids given and unique,
/// stretches the right way round, volume points in time order and
/// numbers. The tests run it after every op, undo and redo.
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
        if c.marks.iter().any(|m| !m.t.is_finite()) || c.marks.windows(2).any(|w| w[0].t > w[1].t) {
            return Err(format!("cut #{}'s markers aren't numbers in time order", c.id));
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
        if i.end <= i.start {
            return Err(format!("overlay #{} ends before it starts", i.id));
        }
    }
    Ok(())
}
