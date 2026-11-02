//! Operations on the cuts: add, trim, move, split, crossfade, framing
//! settings, delete (`OPS-API.md` step 3). What the player read from its
//! clip library (a clip's length, its colour mode) comes in the op.

use super::{End, OpError, Outcome, said};
use crate::model::edit::MIN_CUT;
use crate::project::{Cut, Id, Project};
use crate::stab::Mode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where a new cut goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CutPlace")]
pub enum Place {
    /// After the last cut.
    End,
    /// At edit time `t`: the cut playing there is split and the new one
    /// goes between the halves (within `MIN_CUT` of a join, at the join).
    At(f64),
    /// At the join of two cuts nearest edit time `t`.
    Join(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum CutOp {
    /// A cut of `input` from `start` to `end` (seconds in the clip), with
    /// its stabilisation and LUT (a library name or a `.cube` file).
    Add {
        input: PathBuf,
        start: f64,
        end: f64,
        place: Place,
        #[serde(default)]
        stabilize: Option<Mode>,
        #[serde(default)]
        lut: Option<String>,
    },
    /// Its in or out point to `t` (clip time); `most` is the clip's length.
    Trim { cut: Id, end: End, t: f64, most: f64 },
    /// Moved to play just before cut `before`, or last if none.
    Move { cut: Id, before: Option<Id> },
    /// Split at `t` (clip time); the second half is a new cut.
    Split { cut: Id, t: f64 },
    /// How far it overlaps the cut before, seconds; kept within both.
    Crossfade { cut: Id, seconds: f64 },
    Stabilize { cut: Id, mode: Option<Mode> },
    Lut { cut: Id, lut: Option<String> },
    Delete { cut: Id },
}

pub(super) fn apply(p: &mut Project, op: &CutOp) -> Result<Outcome, OpError> {
    match op {
        CutOp::Add { input, start, end, place, stabilize, lut } => {
            if !(start.is_finite() && end - start >= MIN_CUT) {
                return Err(OpError::Invalid(format!("a cut from {start} to {end} s is shorter than {MIN_CUT} s")));
            }
            let id = p.next_id();
            let cut = Cut { id, stabilize: *stabilize, lut: lut.clone(), ..Cut::new(input.clone(), *start, *end) };
            let at = match *place {
                Place::End => p.insert_cut(usize::MAX, cut),
                Place::At(t) => p.insert_at(t, cut),
                Place::Join(t) => {
                    let join = p.nearest_join(t);
                    p.insert_cut(join, cut)
                }
            };
            // Put in among the cuts, it may have split one: its second half
            // gets an id of its own.
            p.settle_ids();
            Ok(Outcome { summary: format!("cut #{id} added, {}", position(at, p.cuts.len())), made: Some(id) })
        }
        &CutOp::Trim { cut, end, t, most } => {
            let i = find(p, cut)?;
            p.trim(i, end == End::In, t, most);
            let c = &p.cuts[i];
            said(format!("cut #{cut}: {:.2}-{:.2} s", c.start, c.end))
        }
        &CutOp::Move { cut, before } => {
            let from = find(p, cut)?;
            let to = match before {
                Some(b) => find(p, b)?,
                None => p.cuts.len(),
            };
            p.move_cut(from, to);
            said(format!("cut #{cut} moved, {}", position(find(p, cut)?, p.cuts.len())))
        }
        &CutOp::Split { cut, t } => {
            let i = find(p, cut)?;
            if !p.split(i, t) {
                let c = &p.cuts[i];
                return Err(OpError::OutOfRange { what: "split time", value: t, least: c.start + MIN_CUT, most: c.end - MIN_CUT });
            }
            let id = p.next_id();
            p.cuts[i + 1].id = id;
            Ok(Outcome { summary: format!("cut #{cut} split at {t:.2} s; the second half is cut #{id}"), made: Some(id) })
        }
        &CutOp::Crossfade { cut, seconds } => {
            let i = find(p, cut)?;
            p.set_crossfade(i, seconds);
            said(format!("cut #{cut}: crossfade {:.2} s", p.cuts[i].crossfade))
        }
        &CutOp::Stabilize { cut, mode } => {
            let i = find(p, cut)?;
            p.cuts[i].stabilize = mode;
            said(format!("cut #{cut}: stabilisation {}", mode.map_or("as the export's".to_owned(), |m| format!("{m:?}").to_lowercase())))
        }
        CutOp::Lut { cut, lut } => {
            let i = find(p, *cut)?;
            p.cuts[i].lut = lut.clone();
            said(format!("cut #{cut}: LUT {}", lut.as_deref().unwrap_or("none")))
        }
        &CutOp::Delete { cut } => {
            let i = find(p, cut)?;
            p.remove(i);
            said(format!("cut #{cut} removed"))
        }
    }
}

/// Where cut `id` is.
fn find(p: &Project, id: Id) -> Result<usize, OpError> {
    p.cuts.iter().position(|c| c.id == id).ok_or_else(|| OpError::NotFound(format!("cut #{id}")))
}

fn position(i: usize, of: usize) -> String {
    format!("{} of {of}", i + 1)
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;

    fn add(input: &str, len: f64, place: Place) -> Op {
        Op::Cut(CutOp::Add { input: input.into(), start: 0.0, end: len, place, stabilize: None, lut: None })
    }

    fn names(e: &Edit) -> Vec<String> {
        e.cuts.iter().map(|c| format!("{}#{}", c.input.display(), c.id)).collect()
    }

    #[test]
    fn cuts_are_added_split_moved_and_deleted_by_id() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let b = e.apply(&add("b", 3.0, Place::End)).unwrap().made.unwrap();
        // At 2 s: a is split, c goes between its halves; a's second half
        // gets its own id.
        let c = e.apply(&add("c", 1.0, Place::At(2.0))).unwrap().made.unwrap();
        assert_eq!(names(&e), ["a#1", "c#3", "a#4", "b#2"]);
        check(&e).unwrap();
        // Moved before a, by id, whatever the indices are now.
        e.apply(&Op::Cut(CutOp::Move { cut: b, before: Some(a) })).unwrap();
        e.apply(&Op::Cut(CutOp::Move { cut: c, before: None })).unwrap();
        assert_eq!(names(&e), ["b#2", "a#1", "a#4", "c#3"]);
        let half = e.apply(&Op::Cut(CutOp::Split { cut: b, t: 1.0 })).unwrap().made.unwrap();
        assert_eq!(names(&e), ["b#2", format!("b#{half}").as_str(), "a#1", "a#4", "c#3"]);
        assert!(matches!(e.apply(&Op::Cut(CutOp::Split { cut: b, t: 0.1 })), Err(OpError::OutOfRange { .. })));
        e.apply(&Op::Cut(CutOp::Delete { cut: a })).unwrap();
        assert!(matches!(e.apply(&Op::Cut(CutOp::Delete { cut: a })), Err(OpError::NotFound(_))));
        check(&e).unwrap();
        // Every op was its own step.
        for _ in 0..7 {
            assert!(e.undo());
            check(&e).unwrap();
        }
        assert!(e.cuts.is_empty() && !e.undo());
    }

    #[test]
    fn trims_and_crossfades_keep_within_the_clip() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let b = e.apply(&add("b", 3.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::Out, t: 9.0, most: 6.0 })).unwrap();
        assert_eq!(e.cuts[0].end, 6.0, "not past the clip's length");
        e.apply(&Op::Cut(CutOp::Crossfade { cut: b, seconds: 10.0 })).unwrap();
        assert!(e.cuts[1].crossfade < 3.0 && e.cuts[1].crossfade > 0.0, "within the shorter cut: {}", e.cuts[1].crossfade);
        e.apply(&Op::Cut(CutOp::Stabilize { cut: a, mode: Some(Mode::Level) })).unwrap();
        assert_eq!(e.cuts[0].stabilize, Some(Mode::Level));
        check(&e).unwrap();
    }
}
