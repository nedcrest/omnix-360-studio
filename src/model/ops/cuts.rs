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
    /// How its crossfade looks (`transition::Look`); none: a dissolve.
    /// The first cut has no join before it.
    Transition { cut: Id, look: Option<crate::transition::Look> },
    /// One of its look's own settings (`Look::knobs`, by key) to `value`;
    /// none: back to the default.
    TransitionKnob { cut: Id, key: String, value: Option<f32> },
    Stabilize { cut: Id, mode: Option<Mode> },
    Lut { cut: Id, lut: Option<String> },
    Delete { cut: Id },
    /// A marker at `t` (clip time) with `note`: the one there (within
    /// `SAME_MARK`) gets the note, else a new one.
    Mark { cut: Id, t: f64, #[serde(default)] note: String },
    /// The marker at `from` to `t`, within the cut.
    MoveMark { cut: Id, from: f64, t: f64 },
    Unmark { cut: Id, t: f64 },
    /// Its sound's edge (`in`: where it starts, `out`: where it ends) to
    /// clip time `t`, the picture where it is: past the picture's edge the
    /// sound plays under the cut beside (a J- or L-cut), inside it the
    /// picture goes on in silence. Kept within the clip (`most` is its
    /// length) and at least `MIN_CUT` long.
    SoundEdge { cut: Id, end: End, t: f64, most: f64 },
    /// Its sound back to its picture's edges.
    SoundToPicture { cut: Id },
}

/// Two markers closer than this (seconds) are one.
pub const SAME_MARK: f64 = 1.0 / 60.0;

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
        &CutOp::Transition { cut, look } => {
            let i = find(p, cut)?;
            if i == 0 {
                return Err(OpError::Invalid(format!("cut #{cut} is the first: no join before it")));
            }
            // The settings were the old look's.
            if p.cuts[i].transition != look {
                p.cuts[i].transition_params.clear();
            }
            p.cuts[i].transition = look;
            said(format!("cut #{cut}: transition {}", look.map_or("dissolve", |l| l.name())))
        }
        CutOp::TransitionKnob { cut, key, value } => {
            let i = find(p, *cut)?;
            let look = p.cuts[i].transition.ok_or_else(|| OpError::Invalid(format!("cut #{cut}'s join has no look to set")))?;
            let knob = look.knob(key).ok_or_else(|| OpError::NotFound(format!("setting {key:?} of {}", look.name())))?;
            let params = &mut p.cuts[i].transition_params;
            match value {
                Some(v) if !(knob.least..=knob.most).contains(v) => {
                    return Err(OpError::OutOfRange { what: knob.name, value: *v as f64, least: knob.least as f64, most: knob.most as f64 });
                }
                // Only what differs from the default is kept.
                Some(v) if knob.settle(*v) != knob.default => {
                    params.insert(knob.key.to_owned(), knob.settle(*v));
                }
                _ => {
                    params.remove(knob.key);
                }
            }
            said(format!("cut #{cut}: {} {}", knob.name, params.get(knob.key).copied().unwrap_or(knob.default)))
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
        CutOp::Mark { cut, t, note } => {
            let i = find(p, *cut)?;
            let c = &mut p.cuts[i];
            if !t.is_finite() {
                return Err(OpError::Invalid(format!("a marker at {t} s")));
            }
            match mark_at(&c.marks, *t) {
                Some(k) => c.marks[k].note = note.clone(),
                None => {
                    let k = c.marks.partition_point(|m| m.t <= *t);
                    c.marks.insert(k, crate::project::Mark { t: *t, note: note.clone() });
                }
            }
            said(format!("cut #{cut}: marker at {t:.2} s{}", if note.is_empty() { String::new() } else { format!(" «{note}»") }))
        }
        &CutOp::MoveMark { cut, from, t } => {
            let i = find(p, cut)?;
            let c = &mut p.cuts[i];
            let k = mark_at(&c.marks, from).ok_or_else(|| OpError::NotFound(format!("marker at {from:.2} s on cut #{cut}")))?;
            let mut m = c.marks.remove(k);
            m.t = t.clamp(c.start, c.end);
            let to = c.marks.partition_point(|n| n.t <= m.t);
            let at = m.t;
            c.marks.insert(to, m);
            said(format!("cut #{cut}: marker {from:.2} s → {at:.2} s"))
        }
        &CutOp::SoundEdge { cut, end, t, most } => {
            let i = find(p, cut)?;
            if !t.is_finite() {
                return Err(OpError::Invalid(format!("a sound edge at {t} s")));
            }
            let c = &mut p.cuts[i];
            let (from, to) = c.sound_span();
            match end {
                End::In => c.sound_before = c.start - t.clamp(0.0, (to - MIN_CUT).max(0.0)),
                End::Out => c.sound_after = t.clamp(from + MIN_CUT, most.max(from + MIN_CUT)) - c.end,
            }
            let (from, to) = c.sound_span();
            said(format!("cut #{cut}: sound {from:.2}-{to:.2} s (picture {:.2}-{:.2})", c.start, c.end))
        }
        &CutOp::SoundToPicture { cut } => {
            let i = find(p, cut)?;
            let c = &mut p.cuts[i];
            (c.sound_before, c.sound_after) = (0.0, 0.0);
            said(format!("cut #{cut}: sound back to its picture"))
        }
        &CutOp::Unmark { cut, t } => {
            let i = find(p, cut)?;
            let c = &mut p.cuts[i];
            let k = mark_at(&c.marks, t).ok_or_else(|| OpError::NotFound(format!("marker at {t:.2} s on cut #{cut}")))?;
            c.marks.remove(k);
            said(format!("cut #{cut}: marker at {t:.2} s removed"))
        }
    }
}

/// The marker within `SAME_MARK` of `t`.
fn mark_at(marks: &[crate::project::Mark], t: f64) -> Option<usize> {
    marks.iter().position(|m| (m.t - t).abs() < SAME_MARK)
}

/// Where cut `id` is.
fn find(p: &Project, id: Id) -> Result<usize, OpError> {
    p.cut_index(id).ok_or_else(|| OpError::NotFound(format!("cut #{id}")))
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
    fn a_cuts_sound_runs_past_its_picture_or_stops_short_within_its_clip() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::In, t: 1.0, most: 4.0 })).unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::Out, t: 3.0, most: 4.0 })).unwrap();
        let edge = |end, t| Op::Cut(CutOp::SoundEdge { cut: a, end, t, most: 4.0 });
        // An L-cut a second past the picture, and a J-cut further back than
        // the clip has footage: stopped at its start.
        e.apply(&edge(End::Out, 4.0)).unwrap();
        e.apply(&edge(End::In, -2.0)).unwrap();
        assert_eq!(e.cuts[0].sound_span(), (0.0, 4.0));
        assert_eq!((e.cuts[0].sound_before, e.cuts[0].sound_after), (1.0, 1.0));
        // The sound past the picture: before it at edit time -1 (the cut is
        // first), after it from 2 s.
        let beyond = e.cuts[0].sound_beyond(0.0);
        assert_eq!(beyond.iter().map(|c| (c.at, c.start, c.end)).collect::<Vec<_>>(), [(-1.0, 0.0, 1.0), (2.0, 3.0, 4.0)]);
        // Inside the picture: silence after 2.5 s, the picture going on.
        e.apply(&edge(End::Out, 2.5)).unwrap();
        assert_eq!(e.cuts[0].sound_fade(2.4), 1.0);
        assert_eq!(e.cuts[0].sound_fade(2.6), 0.0);
        // Never shorter than a cut can be, and back to the picture.
        e.apply(&edge(End::In, 2.45)).unwrap();
        let (from, to) = e.cuts[0].sound_span();
        assert!((to - from - MIN_CUT).abs() < 1e-9, "{from}-{to}");
        e.apply(&Op::Cut(CutOp::SoundToPicture { cut: a })).unwrap();
        assert_eq!(e.cuts[0].sound_span(), (1.0, 3.0));
        super::super::check(&e).unwrap();
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
    fn markers_are_set_noted_moved_and_removed_by_time() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let mark = |t: f64, note: &str| Op::Cut(CutOp::Mark { cut: a, t, note: note.into() });
        e.apply(&mark(3.0, "")).unwrap();
        e.apply(&mark(1.0, "")).unwrap();
        e.apply(&mark(1.005, "the castle")).unwrap();
        assert_eq!(e.cuts[0].marks.iter().map(|m| (m.t, m.note.as_str())).collect::<Vec<_>>(), [(1.0, "the castle"), (3.0, "")], "one at 1 s, noted; in order");
        e.apply(&Op::Cut(CutOp::MoveMark { cut: a, from: 1.0, t: 9.0 })).unwrap();
        assert_eq!(e.cuts[0].marks.last().map(|m| m.t), Some(4.0), "kept within the cut, and in order");
        e.apply(&Op::Cut(CutOp::Unmark { cut: a, t: 3.0 })).unwrap();
        assert!(matches!(e.apply(&Op::Cut(CutOp::Unmark { cut: a, t: 3.0 })), Err(OpError::NotFound(_))));
        // A split's halves both carry them; each shows its own stretch's.
        e.apply(&Op::Cut(CutOp::Split { cut: a, t: 2.0 })).unwrap();
        assert_eq!((e.cuts[0].marks.len(), e.cuts[1].marks.len()), (1, 1));
        check(&e).unwrap();
    }

    #[test]
    fn trims_and_crossfades_keep_within_the_clip() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let b = e.apply(&add("b", 3.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::Out, t: 9.0, most: 6.0 })).unwrap();
        assert_eq!(e.cuts[0].end, 6.0, "not past the clip's length");
        e.apply(&Op::Cut(CutOp::Transition { cut: b, look: Some(crate::transition::Look::WhipPan) })).unwrap();
        assert_eq!(e.cuts[1].transition, Some(crate::transition::Look::WhipPan));
        assert!(e.apply(&Op::Cut(CutOp::Transition { cut: a, look: Some(crate::transition::Look::Flash) })).is_err(), "the first cut has no join before it");
        let knob = |key: &str, value| Op::Cut(CutOp::TransitionKnob { cut: b, key: key.to_owned(), value });
        e.apply(&knob("streak", Some(1.5))).unwrap();
        e.apply(&knob("direction", Some(1.0))).unwrap();
        assert!(e.apply(&knob("streak", Some(5.0))).is_err() && e.apply(&knob("turns", Some(1.0))).is_err(), "out of range; not the whip's");
        e.apply(&knob("direction", Some(0.0))).unwrap();
        assert_eq!(e.cuts[1].transition_params.len(), 1, "the default isn't kept");
        assert_eq!(e.cuts[1].transition_spec().values[..2], [0.0, 1.5]);
        e.apply(&Op::Cut(CutOp::Transition { cut: b, look: Some(crate::transition::Look::Flash) })).unwrap();
        assert!(e.cuts[1].transition_params.is_empty(), "another look starts at its defaults");
        e.apply(&Op::Cut(CutOp::Crossfade { cut: b, seconds: 10.0 })).unwrap();
        assert!(e.cuts[1].crossfade < 3.0 && e.cuts[1].crossfade > 0.0, "within the shorter cut: {}", e.cuts[1].crossfade);
        e.apply(&Op::Cut(CutOp::Stabilize { cut: a, mode: Some(Mode::Level) })).unwrap();
        assert_eq!(e.cuts[0].stabilize, Some(Mode::Level));
        check(&e).unwrap();
    }
}
