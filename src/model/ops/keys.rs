//! Operations on effect and motion keyframes (`OPS-API.md` step 5): set
//! the look or an overlay's place at a time, its ease, retime, delete,
//! clear, the look to every cut. Values are whole and absolute, so a slider
//! re-sent or an agent's retry changes nothing more. A keyframe is named by
//! what it's on and its time (its own time: a cut's clip time, an item's).

use super::{OpError, Outcome, said};
use crate::grade::{EffectKey, Grade, grade_at};
use crate::model::curve::{self, Key};
use crate::overlay::{Item, Place};
use crate::project::{Id, Project};
use crate::view::{Ease, Follow, View};
use serde::{Deserialize, Serialize};

/// What has effects: a cut or an overlay/text item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Graded {
    Cut(Id),
    Item(Id),
}

/// A row of keyframes: a cut's view, something's effects, or an item's
/// motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Line {
    View(Id),
    Effects(Graded),
    Motion(Id),
}

/// Where each thing a view key can follow points at the key's time, as
/// the player measured it (the camera's attitude, a colour track): `None`
/// where it can't be told. A following key keeps its view as offsets from
/// this.
pub type Aims = Vec<(Follow, Option<(f64, f64)>)>;

/// The tolerance that names a key by the very time it has (a mark
/// dragged, the key under the playhead found first): no other near it.
pub const EXACT: f64 = 1e-6;

/// Two keys this close (seconds) are one, unless an op says otherwise.
fn half_a_frame() -> f64 {
    1.0 / 60.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum KeyOp {
    /// The view shown at `t` in cut `cut`: the key within `tolerance` set
    /// to it, or a new one easing and following like the one before.
    View { cut: Id, t: f64, view: View, #[serde(default = "half_a_frame")] tolerance: f64, #[serde(default)] aims: Aims },
    /// The view key at `t` made to follow `follow` (or stop), still showing
    /// what it shows now.
    Follow { cut: Id, t: f64, follow: Option<Follow>, #[serde(default = "half_a_frame")] tolerance: f64, #[serde(default)] aims: Aims },
    /// The look at `t`: the key within `tolerance` set to `grade`, or a new
    /// one there.
    Effects { on: Graded, t: f64, grade: Grade, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// One effect (`grade::NAMES`: "exposure", "vignette"…) at `value` for
    /// the whole cut or item: in every key it has, or a key of its own at
    /// its start if it has none (the user, 2026-11-09: an edit in Effects
    /// sets the value, it doesn't key it; automation goes on lanes, 39).
    EffectValue { on: Graded, param: String, value: f64 },
    /// One of an item's place values (`Place::NAMES`: "x", "y", "size",
    /// "rotation") for the whole item, as `EffectValue` (the user,
    /// 2026-11-09: moving an overlay doesn't key it either).
    PlaceValue { item: Id, param: String, value: f64 },
    /// An item's place at `t`, as `Effects`.
    Motion { item: Id, t: f64, place: Place, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// How the key at `t` moves on to the next.
    Ease { line: Line, t: f64, ease: Ease, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// The key at `from` to `t`, kept `gap` seconds clear of its neighbours
    /// (order kept).
    Retime { line: Line, from: f64, t: f64, gap: f64, #[serde(default = "half_a_frame")] tolerance: f64 },
    Delete { line: Line, t: f64, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// Every key gone: a cut's or item's effects, an item's motion, and
    /// their automation (its ins and outs too).
    Clear { on: Graded },
    /// The look cut `cut` has at `t`, held by every other cut from its
    /// start.
    LookToAll { cut: Id, t: f64 },
}

pub(super) fn apply(p: &mut Project, op: &KeyOp) -> Result<Outcome, OpError> {
    match op {
        KeyOp::View { cut, t, view, tolerance, aims } => {
            let keys = &mut find_cut(p, *cut)?.keyframes;
            keys.set(*t, *view, *tolerance, &|f, _| aim(aims, f));
            said(format!("cut #{cut}: view keyed at {t:.2} s"))
        }
        KeyOp::EffectValue { on, param, value } => {
            let k = crate::grade::NAMES.iter().position(|n| n == param).ok_or_else(|| OpError::NotFound(format!("effect {param:?}")))?;
            if !value.is_finite() {
                return Err(OpError::Invalid(format!("{param} {value} isn't a number")));
            }
            if matches!(on, Graded::Cut(_)) && crate::grade::ITEM_ONLY.contains(&k) {
                return Err(OpError::Invalid(format!("{param} is for text and pictures, not a cut")));
            }
            let start = match *on {
                Graded::Cut(id) => find_cut(p, id)?.start,
                Graded::Item(id) => find_item(p, id)?.start,
            };
            let keys = effects(p, *on)?;
            if keys.is_empty() {
                keys.push(EffectKey { t: start, grade: Grade::default(), ease: Default::default() });
            }
            for key in keys.iter_mut() {
                let mut values = key.grade.values();
                values[k] = *value;
                key.grade = Grade::from_values(values);
            }
            said(format!("{}: {param} {value}", name(*on)))
        }
        KeyOp::PlaceValue { item, param, value } => {
            let k = Place::NAMES.iter().position(|n| n == param).ok_or_else(|| OpError::NotFound(format!("place value {param:?}")))?;
            if !value.is_finite() {
                return Err(OpError::Invalid(format!("{param} {value} isn't a number")));
            }
            let it = find_item(p, *item)?;
            if it.motion.is_empty() {
                it.motion.push(crate::overlay::MotionKey { t: it.start, place: Place::default(), ease: Default::default() });
            }
            for key in it.motion.iter_mut() {
                let mut values = key.place.values();
                values[k] = *value;
                key.place = Place::from_values(values);
            }
            said(format!("item #{item}: {param} {value}"))
        }
        KeyOp::Follow { cut, t, follow, tolerance, aims } => {
            let keys = &mut find_cut(p, *cut)?.keyframes;
            let i = keys.near(*t, *tolerance).ok_or_else(|| OpError::NotFound(format!("view key at {t:.2} s on cut #{cut}")))?;
            keys.set_follow(i, follow.clone(), &|f, _| aim(aims, f));
            said(format!("cut #{cut}: view key at {t:.2} s {}", if follow.is_some() { "follows" } else { "fixed" }))
        }
        &KeyOp::Effects { on, t, grade, tolerance } => {
            set(effects(p, on)?, t, grade, tolerance);
            said(format!("{}: look set at {t:.2} s", name(on)))
        }
        &KeyOp::Motion { item, t, place, tolerance } => {
            set(&mut find_item(p, item)?.motion, t, place, tolerance);
            said(format!("item #{item}: place set at {t:.2} s"))
        }
        &KeyOp::Ease { line, t, ease, tolerance } => {
            let i = index(p, line, t, tolerance)?;
            match line {
                Line::View(cut) => find_cut(p, cut)?.keyframes.set_ease(i, ease),
                Line::Effects(on) => effects(p, on)?[i].ease = ease,
                Line::Motion(item) => find_item(p, item)?.motion[i].ease = ease,
            }
            said(format!("{}: key at {t:.2} s eases {ease:?}", line_name(line)))
        }
        &KeyOp::Retime { line, from, t, gap, tolerance } => {
            let i = index(p, line, from, tolerance)?;
            match line {
                Line::View(cut) => find_cut(p, cut)?.keyframes.retime(i, t, gap),
                Line::Effects(on) => crate::view::retime(effects(p, on)?, i, t, gap),
                Line::Motion(item) => crate::view::retime(&mut find_item(p, item)?.motion, i, t, gap),
            }
            said(format!("{}: key at {from:.2} s moved to {t:.2} s", line_name(line)))
        }
        &KeyOp::Delete { line, t, tolerance } => {
            let i = index(p, line, t, tolerance)?;
            match line {
                Line::View(cut) => find_cut(p, cut)?.keyframes.remove(i),
                Line::Effects(on) => {
                    effects(p, on)?.remove(i);
                }
                Line::Motion(item) => {
                    find_item(p, item)?.motion.remove(i);
                }
            }
            said(format!("{}: key at {t:.2} s removed", line_name(line)))
        }
        &KeyOp::Clear { on } => {
            effects(p, on)?.clear();
            match on {
                Graded::Item(item) => {
                    let it = find_item(p, item)?;
                    it.motion.clear();
                    it.automation.clear();
                }
                Graded::Cut(cut) => find_cut(p, cut)?.automation.clear(),
            }
            said(format!("{}: keys and automation cleared", name(on)))
        }
        &KeyOp::LookToAll { cut, t } => {
            let grade = grade_at(effects(p, Graded::Cut(cut))?, t);
            for c in p.cuts.iter_mut().filter(|c| c.id != cut) {
                c.effects = vec![EffectKey::make(c.start, grade)];
            }
            said(format!("cut #{cut}'s look at {t:.2} s on every other cut"))
        }
    }
}

/// The key within `tolerance` of `t` (or a new one) set to `v`.
fn set<K: Key>(keys: &mut Vec<K>, t: f64, v: K::V, tolerance: f64) {
    let i = curve::key_at(keys, t, tolerance);
    *keys[i].value_mut() = v;
}

fn find_cut(p: &mut Project, id: Id) -> Result<&mut crate::project::Cut, OpError> {
    let i = p.cut_index(id).ok_or_else(|| OpError::NotFound(format!("cut #{id}")))?;
    Ok(&mut p.cuts[i])
}

/// Where `f` points, from the player's `aims`.
fn aim(aims: &Aims, f: &Follow) -> Option<(f64, f64)> {
    aims.iter().find(|a| &a.0 == f).and_then(|a| a.1)
}

fn effects(p: &mut Project, on: Graded) -> Result<&mut Vec<EffectKey>, OpError> {
    match on {
        Graded::Cut(id) => Ok(&mut find_cut(p, id)?.effects),
        Graded::Item(id) => Ok(&mut find_item(p, id)?.effects),
    }
}

fn find_item(p: &mut Project, id: Id) -> Result<&mut Item, OpError> {
    let (track, l, i) = p.item_index(id).ok_or_else(|| OpError::NotFound(format!("item #{id}")))?;
    let lanes = match track {
        crate::overlay::Track::Overlay => &mut p.overlays,
        crate::overlay::Track::Text => &mut p.text,
    };
    Ok(&mut lanes[l].items[i])
}

/// Where the key at `t` on `line` is.
fn index(p: &mut Project, line: Line, t: f64, tolerance: f64) -> Result<usize, OpError> {
    let found = match line {
        Line::View(cut) => find_cut(p, cut)?.keyframes.near(t, tolerance),
        Line::Effects(on) => curve::find(effects(p, on)?, t, tolerance),
        Line::Motion(item) => curve::find(&find_item(p, item)?.motion, t, tolerance),
    };
    found.ok_or_else(|| OpError::NotFound(format!("key at {t:.2} s on {}", line_name(line))))
}

fn name(on: Graded) -> String {
    match on {
        Graded::Cut(id) => format!("cut #{id}"),
        Graded::Item(id) => format!("item #{id}"),
    }
}

fn line_name(line: Line) -> String {
    match line {
        Line::View(id) => format!("cut #{id}'s view"),
        Line::Effects(on) => format!("{}'s effects", name(on)),
        Line::Motion(id) => format!("item #{id}'s motion"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;
    use crate::project::Cut;

    fn edit() -> Edit {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 1.0, 5.0), Cut::new("b.mp4".into(), 0.0, 3.0)], ..Default::default() };
        p.settle_ids();
        let mut e = Edit::default();
        e.replace(p);
        e
    }

    fn key(o: KeyOp) -> Op {
        Op::Key(o)
    }

    #[test]
    fn a_look_set_twice_is_one_key_and_one_step() {
        let mut e = edit();
        let warm = Grade { temperature: 0.4, ..Grade::default() };
        let set = key(KeyOp::Effects { on: Graded::Cut(1), t: 2.0, grade: warm, tolerance: 0.01 });
        e.apply(&set).unwrap();
        e.apply(&set).unwrap();
        assert_eq!(e.cuts[0].effects.len(), 1);
        assert!(e.undo() && e.cuts[0].effects.is_empty() && !e.undo());
    }

    #[test]
    fn keys_ease_move_and_go_by_time() {
        let mut e = edit();
        let line = Line::Effects(Graded::Cut(1));
        for t in [2.0, 3.0] {
            e.apply(&key(KeyOp::Effects { on: Graded::Cut(1), t, grade: Grade::default(), tolerance: 0.01 })).unwrap();
        }
        e.apply(&key(KeyOp::Ease { line, t: 2.0, ease: Ease::Hold, tolerance: 0.01 })).unwrap();
        assert_eq!(e.cuts[0].effects[0].ease, Ease::Hold);
        // Dragged past its neighbour, it stops a gap short.
        e.apply(&key(KeyOp::Retime { line, from: 2.0, t: 4.0, gap: 0.1, tolerance: 0.01 })).unwrap();
        assert!((e.cuts[0].effects[0].t - 2.9).abs() < 1e-9);
        assert!(matches!(e.apply(&key(KeyOp::Delete { line, t: 2.0, tolerance: 0.01 })), Err(OpError::NotFound(_))));
        e.apply(&key(KeyOp::Delete { line, t: 2.9, tolerance: 0.01 })).unwrap();
        assert_eq!(e.cuts[0].effects.len(), 1);
        check(&e).unwrap();
    }

    #[test]
    fn a_view_key_follows_the_stick_as_offsets_from_where_it_points() {
        let mut e = edit();
        let view = View { yaw: 30.0, pitch: -10.0, ..View::home(crate::view::Projection::Perspective) };
        e.apply(&key(KeyOp::View { cut: 1, t: 2.0, view, tolerance: 0.01, aims: Vec::new() })).unwrap();
        // The stick points at yaw 20 there: following it, the key keeps 10 off.
        let aims = vec![(Follow::Stick, Some((20.0, 0.0)))];
        e.apply(&key(KeyOp::Follow { cut: 1, t: 2.0, follow: Some(Follow::Stick), tolerance: 0.01, aims })).unwrap();
        let k = &e.cuts[0].keyframes[0];
        assert_eq!((k.follow.clone(), k.view.yaw, k.view.pitch), (Some(Follow::Stick), 10.0, -10.0));
        e.apply(&key(KeyOp::Ease { line: Line::View(1), t: 2.0, ease: Ease::Linear, tolerance: 0.01 })).unwrap();
        e.apply(&key(KeyOp::Delete { line: Line::View(1), t: 2.0, tolerance: 0.01 })).unwrap();
        assert!(e.cuts[0].keyframes.iter().next().is_none());
        assert!(matches!(e.apply(&key(KeyOp::Follow { cut: 1, t: 2.0, follow: None, tolerance: 0.01, aims: Vec::new() })), Err(OpError::NotFound(_))));
    }

    #[test]
    fn a_look_goes_to_every_other_cut_from_its_start() {
        let mut e = edit();
        let warm = Grade { temperature: 0.4, ..Grade::default() };
        e.apply(&key(KeyOp::Effects { on: Graded::Cut(1), t: 2.0, grade: warm, tolerance: 0.01 })).unwrap();
        e.apply(&key(KeyOp::LookToAll { cut: 1, t: 2.0 })).unwrap();
        assert_eq!(e.cuts[1].effects, vec![EffectKey::make(0.0, warm)]);
        e.apply(&key(KeyOp::Clear { on: Graded::Cut(2) })).unwrap();
        assert!(e.cuts[1].effects.is_empty());
    }

    #[test]
    fn an_effect_value_holds_for_the_whole_cut_and_makes_no_key() {
        let mut e = edit();
        let on = Graded::Cut(e.cuts[0].id);
        let value = |param: &str, value| Op::Key(KeyOp::EffectValue { on, param: param.into(), value });
        e.apply(&value("exposure", 0.5)).unwrap();
        e.apply(&value("vignette", 0.3)).unwrap();
        assert_eq!(e.cuts[0].effects.len(), 1, "one key, at its start, for the whole cut");
        assert_eq!((e.cuts[0].effects[0].t, e.cuts[0].effects[0].grade.exposure, e.cuts[0].effects[0].grade.vignette), (1.0, 0.5, 0.3));
        // Keys from before: the value in each, the rest of them kept.
        e.cuts[0].effects.push(EffectKey { t: 4.0, grade: Grade { exposure: -1.0, contrast: 0.4, ..Default::default() }, ease: Default::default() });
        e.apply(&value("exposure", 0.2)).unwrap();
        assert!(e.cuts[0].effects.iter().all(|k| k.grade.exposure == 0.2) && e.cuts[0].effects[1].grade.contrast == 0.4);
        assert!(e.apply(&value("sparkle", 1.0)).is_err() && e.apply(&value("exposure", f64::NAN)).is_err());
        check(&e).unwrap();
    }


    #[test]
    fn a_place_value_holds_for_the_whole_item_and_makes_no_key() {
        let mut e = edit();
        let item = e.apply(&Op::Item(crate::model::ops::items::ItemOp::AddText { text: Default::default(), at: 0.0, len: 3.0 })).unwrap().made.unwrap();
        let place = |param: &str, value| Op::Key(KeyOp::PlaceValue { item, param: param.into(), value });
        e.apply(&place("x", 0.2)).unwrap();
        e.apply(&place("size", 0.5)).unwrap();
        let motion = &e.text[0].items[0].motion;
        assert_eq!((motion.len(), motion[0].place.x, motion[0].place.size, motion[0].place.y), (1, 0.2, 0.5, 0.5), "one key, the rest as they were");
        assert!(e.apply(&place("depth", 1.0)).is_err());
        check(&e).unwrap();
    }

}
