//! Automation clips' operations (gap 39): a clip on a value's lane of a
//! cut or an item, moved, trimmed, taken away, and the points of its
//! curve. A clip is found by its value's name and a time inside it, as
//! keys are by their time.

use super::keys::Graded;
use super::{End, OpError, Outcome, said};
use crate::automation::{AutoClip, AutoPoint};
use crate::project::{Id, Project};
use crate::view::Ease;
use serde::{Deserialize, Serialize};

/// The shortest clip, seconds.
pub const SHORTEST: f64 = 0.1;

fn half_a_frame() -> f64 {
    1.0 / 60.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum AutoOp {
    /// A clip moving `param` (an effect, `grade::NAMES`; on an item also
    /// its place, `Place::NAMES`) over `from`..`to` (clip or own time,
    /// kept within the cut or item), flat at the value it has there; it
    /// mustn't overlap another of that value.
    Add { on: Graded, param: String, from: f64, to: f64 },
    /// The clip of `param` at `at` to start at `from`, its points with it.
    Move { on: Graded, param: String, at: f64, from: f64 },
    /// Its start (`In`) or end (`Out`) to `t`; its points stay where they
    /// are in time.
    Trim { on: Graded, param: String, at: f64, end: End, t: f64 },
    Delete { on: Graded, param: String, at: f64 },
    /// A point of the clip of `param` covering `t`: the one at `from` (or,
    /// without it, at `t`) to `t` and value `v`, or a new one there.
    Point { on: Graded, param: String, t: f64, v: f64, #[serde(default)] from: Option<f64>, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// The point at `t` taken away (a clip keeps one at least).
    PointDelete { on: Graded, param: String, t: f64, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// How the point at `t` moves on to the next.
    PointEase { on: Graded, param: String, t: f64, ease: Ease, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// Item `item`'s place values `params` ("x", "y", "size", "rotation")
    /// made to follow cut `cut`'s track `track` where the two run at once,
    /// `offsets` added (one each, 0 if left out): a clip each, its points
    /// the track's samples, marked as following it; clips of those values
    /// there before are replaced (gap 37's contract).
    Follow { item: Id, cut: Id, track: String, params: Vec<String>, #[serde(default)] offsets: Vec<f64> },
    /// Item `item`'s in (`In`) or out (`Out`): `anim` (`animate::Anim`)
    /// `seconds` long, made into clips on that edge (those of the one
    /// before, and hand clips of the same values there, replaced), or none.
    Animate { item: Id, edge: End, anim: Option<crate::animate::Anim>, seconds: f64 },
    /// Every clip moving `params` taken away (a reset: ins and outs among
    /// them).
    Clear { on: Graded, params: Vec<String> },
}

/// What an operation works on: the clips, the stretch they live in, the
/// values they can move, the value each has where no clip is.
struct Owner<'a> {
    clips: &'a mut Vec<AutoClip>,
    range: (f64, f64),
    base: Base<'a>,
    item: bool,
}

/// A value's value where no clip is, by its name and a time.
type Base<'a> = Box<dyn Fn(&str, f64) -> f64 + 'a>;

fn owner(p: &mut Project, on: Graded) -> Result<Owner<'_>, OpError> {
    match on {
        Graded::Cut(id) => {
            let i = p.cut_index(id).ok_or_else(|| OpError::NotFound(format!("cut #{id}")))?;
            let c = &mut p.cuts[i];
            let (effects, range) = (c.effects.clone(), (c.start, c.end));
            Ok(Owner { clips: &mut c.automation, range, base: Box::new(move |param, t| grade_value(&effects, param, t)), item: false })
        }
        Graded::Item(id) => {
            let (track, l, i) = p.item_index(id).ok_or_else(|| OpError::NotFound(format!("item #{id}")))?;
            let lanes = match track {
                crate::overlay::Track::Overlay => &mut p.overlays,
                crate::overlay::Track::Text => &mut p.text,
            };
            let it = &mut lanes[l].items[i];
            let (effects, motion, range) = (it.effects.clone(), it.motion.clone(), (it.start, it.end));
            let base = move |param: &str, t: f64| match crate::overlay::Place::NAMES.iter().position(|n| *n == param) {
                Some(k) => crate::overlay::place_at(&motion, t).values()[k],
                None => grade_value(&effects, param, t),
            };
            Ok(Owner { clips: &mut it.automation, range, base: Box::new(base), item: true })
        }
    }
}

fn grade_value(effects: &[crate::grade::EffectKey], param: &str, t: f64) -> f64 {
    let k = crate::grade::NAMES.iter().position(|n| *n == param).unwrap_or(0);
    crate::grade::grade_at(effects, t).values()[k]
}

/// `param` names something `o` can move.
fn check_param(o: &Owner, param: &str) -> Result<(), OpError> {
    let place = o.item && crate::overlay::Place::NAMES.contains(&param);
    let effect = crate::grade::NAMES.iter().position(|n| *n == param).is_some_and(|k| o.item || !crate::grade::ITEM_ONLY.contains(&k));
    if effect || place {
        Ok(())
    } else {
        Err(OpError::NotFound(format!("value {param:?} to automate")))
    }
}

/// The clip of `param` covering `at`.
fn clip_at(clips: &[AutoClip], param: &str, at: f64) -> Result<usize, OpError> {
    clips.iter().position(|c| c.param == param && c.covers(at)).ok_or_else(|| OpError::NotFound(format!("{param} clip at {at:.2} s")))
}

/// `from`..`to` clear of `param`'s other clips (all but `but`).
fn clear(clips: &[AutoClip], param: &str, from: f64, to: f64, but: Option<usize>) -> Result<(), OpError> {
    let hit = clips.iter().enumerate().any(|(i, c)| Some(i) != but && c.param == param && from < c.to && to > c.from);
    if hit { Err(OpError::Invalid(format!("another {param} clip is there"))) } else { Ok(()) }
}

pub(super) fn apply(p: &mut Project, op: &AutoOp) -> Result<Outcome, OpError> {
    match op {
        AutoOp::Add { on, param, from, to } => {
            let o = owner(p, *on)?;
            check_param(&o, param)?;
            let (from, to) = (from.min(*to).max(o.range.0), from.max(*to).min(o.range.1));
            if (to - from).partial_cmp(&SHORTEST) == Some(std::cmp::Ordering::Less) || (to - from).is_nan() {
                return Err(OpError::Invalid(format!("an automation clip runs {SHORTEST} s at least, within its own")));
            }
            clear(o.clips, param, from, to, None)?;
            let points = [from, to].map(|t| AutoPoint { t, v: (o.base)(param, t), ease: Ease::Smooth }).to_vec();
            let i = o.clips.partition_point(|c| c.from <= from);
            o.clips.insert(i, AutoClip { param: param.clone(), from, to, points, follows: None, animation: None });
            said(format!("{}: {param} automated {from:.2}-{to:.2} s", name(*on)))
        }
        AutoOp::Move { on, param, at, from } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *at)?;
            let len = o.clips[i].to - o.clips[i].from;
            let from = from.clamp(o.range.0, (o.range.1 - len).max(o.range.0));
            clear(o.clips, param, from, from + len, Some(i))?;
            let c = &mut o.clips[i];
            let by = from - c.from;
            (c.from, c.to) = (from, from + len);
            c.points.iter_mut().for_each(|q| q.t += by);
            o.clips.sort_by(|a, b| a.from.total_cmp(&b.from));
            said(format!("{}: {param} clip to {from:.2} s", name(*on)))
        }
        AutoOp::Trim { on, param, at, end, t } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *at)?;
            let (from, to) = (o.clips[i].from, o.clips[i].to);
            let (from, to) = if *end == End::In { (t.clamp(o.range.0, to - SHORTEST), to) } else { (from, t.clamp(from + SHORTEST, o.range.1)) };
            clear(o.clips, param, from, to, Some(i))?;
            o.clips[i].trim(from, to);
            said(format!("{}: {param} clip {from:.2}-{to:.2} s", name(*on)))
        }
        AutoOp::Delete { on, param, at } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *at)?;
            o.clips.remove(i);
            said(format!("{}: {param} clip at {at:.2} s gone", name(*on)))
        }
        AutoOp::Point { on, param, t, v, from, tolerance } => {
            if !v.is_finite() {
                return Err(OpError::Invalid(format!("{param} {v} isn't a number")));
            }
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, from.unwrap_or(*t))?;
            let c = &mut o.clips[i];
            let t = t.clamp(c.from, c.to);
            let near = |at: f64| c.points.iter().position(|q| (q.t - at).abs() <= *tolerance);
            match near(from.unwrap_or(t)) {
                Some(k) => (c.points[k].t, c.points[k].v) = (t, *v),
                None => c.points.push(AutoPoint { t, v: *v, ease: Ease::Smooth }),
            }
            c.points.sort_by(|a, b| a.t.total_cmp(&b.t));
            said(format!("{}: {param} {v} at {t:.2} s", name(*on)))
        }
        AutoOp::PointDelete { on, param, t, tolerance } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *t)?;
            let c = &mut o.clips[i];
            let k = c.points.iter().position(|q| (q.t - t).abs() <= *tolerance).ok_or_else(|| OpError::NotFound(format!("{param} point at {t:.2} s")))?;
            if c.points.len() == 1 {
                return Err(OpError::Invalid("a clip keeps one point at least: delete the clip instead".into()));
            }
            c.points.remove(k);
            said(format!("{}: {param} point at {t:.2} s gone", name(*on)))
        }
        AutoOp::Follow { item, cut, track, params, offsets } => follow(p, *item, *cut, track, params, offsets),
        &AutoOp::Animate { item, edge, anim, seconds } => animate(p, item, edge, anim, seconds),
        AutoOp::Clear { on, params } => {
            let o = owner(p, *on)?;
            o.clips.retain(|c| !params.contains(&c.param));
            said(format!("{}: {} no longer automated", name(*on), params.join(", ")))
        }
        AutoOp::PointEase { on, param, t, ease, tolerance } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *t)?;
            let c = &mut o.clips[i];
            let q = c.points.iter_mut().find(|q| (q.t - t).abs() <= *tolerance).ok_or_else(|| OpError::NotFound(format!("{param} point at {t:.2} s")))?;
            q.ease = *ease;
            said(format!("{}: {param} point at {t:.2} s eases {ease}", name(*on)))
        }
    }
}

/// `AutoOp::Follow`: the track's samples where both run, in the item's
/// own time, made into its place's clips.
fn follow(p: &mut Project, item: Id, cut: Id, track: &str, params: &[String], offsets: &[f64]) -> Result<Outcome, OpError> {
    let ci = p.cut_index(cut).ok_or_else(|| OpError::NotFound(format!("cut #{cut}")))?;
    let at = crate::project::offsets(&p.cuts)[ci];
    let c = &p.cuts[ci];
    let tr = c.tracks.iter().find(|t| t.name == track).ok_or_else(|| OpError::NotFound(format!("track «{track}» on cut #{cut}")))?;
    let samples: Vec<(f64, crate::automation::TrackSample)> = tr.samples.iter().filter(|s| s.t >= c.start && s.t <= c.end).map(|s| (at + s.t - c.start, *s)).collect();
    let (track_at, l, i) = p.item_index(item).ok_or_else(|| OpError::NotFound(format!("item #{item}")))?;
    let lanes = match track_at {
        crate::overlay::Track::Overlay => &mut p.overlays,
        crate::overlay::Track::Text => &mut p.text,
    };
    let it = &mut lanes[l].items[i];
    let own = |edit: f64| it.start + edit - it.at;
    let within: Vec<(f64, crate::automation::TrackSample)> = samples.into_iter().map(|(e, s)| (own(e), s)).filter(|(t, _)| *t >= it.start && *t <= it.end).collect();
    for (k, param) in params.iter().enumerate() {
        if !crate::overlay::Place::NAMES.contains(&param.as_str()) {
            return Err(OpError::NotFound(format!("place value {param:?} to follow with")));
        }
        let offset = offsets.get(k).copied().unwrap_or(0.0);
        let points: Vec<AutoPoint> = within.iter().filter_map(|(t, s)| Some(AutoPoint { t: *t, v: s.value(param)? + offset, ease: Ease::Linear })).collect();
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return Err(OpError::Invalid(format!("track «{track}» has no {param} while item #{item} runs")));
        };
        let (from, to) = (first.t, last.t.max(first.t + SHORTEST));
        it.automation.retain(|a| !(a.param == *param && a.from < to && a.to > from));
        let follows = Some(crate::automation::Follows { cut, track: track.to_owned(), offset });
        it.automation.push(AutoClip { param: param.clone(), from, to, points, follows, animation: None });
    }
    it.automation.sort_by(|a, b| a.from.total_cmp(&b.from));
    said(format!("item #{item} follows «{track}» on cut #{cut}: {}", params.join(", ")))
}

/// `AutoOp::Animate`: item `item`'s `edge` animated by `anim`, from its
/// values where it holds still.
fn animate(p: &mut Project, item: Id, edge: End, anim: Option<crate::animate::Anim>, seconds: f64) -> Result<Outcome, OpError> {
    let (track, l, i) = p.item_index(item).ok_or_else(|| OpError::NotFound(format!("item #{item}")))?;
    let lanes = match track {
        crate::overlay::Track::Overlay => &mut p.overlays,
        crate::overlay::Track::Text => &mut p.text,
    };
    let it = &mut lanes[l].items[i];
    it.automation.retain(|a| a.animation.is_none_or(|x| x.edge != edge));
    let Some(anim) = anim else { return said(format!("item #{item}: no {}", if edge == End::In { "in" } else { "out" })) };
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err(OpError::Invalid(format!("an animation runs some time, not {seconds} s")));
    }
    let at = if edge == End::In { it.start } else { it.end };
    let (grade, place) = (crate::grade::grade_at(&it.effects, at), crate::overlay::place_at(&it.motion, at));
    let made = crate::animate::clips(crate::animate::Animation { anim, edge, seconds }, (it.start, it.end), &grade, place);
    for c in &made {
        it.automation.retain(|a| !(a.param == c.param && a.from < c.to && a.to > c.from));
    }
    it.automation.extend(made);
    it.automation.sort_by(|a, b| a.from.total_cmp(&b.from));
    said(format!("item #{item}: {} {} {seconds:.2} s", if edge == End::In { "in" } else { "out" }, anim.name()))
}

fn name(on: Graded) -> String {
    match on {
        Graded::Cut(id) => format!("cut #{id}"),
        Graded::Item(id) => format!("item #{id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;
    use crate::project::Cut;

    #[test]
    fn a_clip_is_added_flat_shaped_by_points_moved_trimmed_and_kept_apart() {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 1.0, 9.0)], ..Default::default() };
        p.settle_ids();
        let mut e = Edit::default();
        e.replace(p);
        let on = Graded::Cut(e.cuts[0].id);
        let op = |op| Op::Auto(op);
        e.apply(&op(AutoOp::Add { on, param: "exposure".into(), from: 2.0, to: 4.0 })).unwrap();
        assert_eq!(e.cuts[0].automation[0].points.iter().map(|q| (q.t, q.v)).collect::<Vec<_>>(), [(2.0, 0.0), (4.0, 0.0)], "flat at the value there");
        e.apply(&op(AutoOp::Point { on, param: "exposure".into(), t: 3.0, v: 1.0, from: None, tolerance: 0.01 })).unwrap();
        assert!((e.cuts[0].grade_at(3.0).exposure - 1.0).abs() < 1e-9 && e.cuts[0].grade_at(5.0).exposure == 0.0);
        assert!(e.apply(&op(AutoOp::Add { on, param: "exposure".into(), from: 3.5, to: 6.0 })).is_err(), "no overlap with its own");
        e.apply(&op(AutoOp::Add { on, param: "vignette".into(), from: 3.5, to: 6.0 })).unwrap();
        e.apply(&op(AutoOp::Move { on, param: "exposure".into(), at: 3.0, from: 6.0 })).unwrap();
        let c = e.cuts[0].automation.iter().find(|c| c.param == "exposure").unwrap();
        assert_eq!((c.from, c.to, c.points[1].t), (6.0, 8.0, 7.0), "its points with it");
        e.apply(&op(AutoOp::Trim { on, param: "exposure".into(), at: 7.0, end: End::Out, t: 20.0 })).unwrap();
        let c = e.cuts[0].automation.iter().find(|c| c.param == "exposure").unwrap();
        assert_eq!((c.to, c.points.last().unwrap().t), (9.0, 9.0), "within the cut, its end point with its edge");
        e.apply(&op(AutoOp::Trim { on, param: "exposure".into(), at: 7.0, end: End::In, t: 7.5 })).unwrap();
        let c = e.cuts[0].automation.iter().find(|c| c.param == "exposure").unwrap();
        assert_eq!(c.points.iter().map(|q| q.t).collect::<Vec<_>>(), [7.5, 9.0], "its start point too; the one left outside gone");
        assert!(e.apply(&op(AutoOp::Add { on, param: "sparkle".into(), from: 1.0, to: 2.0 })).is_err());
        assert!(e.apply(&op(AutoOp::Add { on, param: "x".into(), from: 1.0, to: 2.0 })).is_err(), "a cut has no place to move");
        assert!(e.apply(&op(AutoOp::Add { on, param: "reveal".into(), from: 1.0, to: 2.0 })).is_err(), "nor a reveal");
        e.apply(&op(AutoOp::Delete { on, param: "vignette".into(), at: 4.0 })).unwrap();
        assert_eq!(e.cuts[0].automation.len(), 1);
        check(&e).unwrap();
    }

    #[test]
    fn a_reset_takes_its_values_clips_away_and_clear_takes_them_all() {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 1.0, 9.0)], ..Default::default() };
        p.settle_ids();
        let mut e = Edit::default();
        e.replace(p);
        let on = Graded::Cut(e.cuts[0].id);
        for param in ["exposure", "vignette"] {
            e.apply(&Op::Auto(AutoOp::Add { on, param: param.into(), from: 2.0, to: 4.0 })).unwrap();
        }
        e.apply(&Op::Auto(AutoOp::Clear { on, params: vec!["exposure".into()] })).unwrap();
        assert_eq!(e.cuts[0].automation.iter().map(|c| c.param.as_str()).collect::<Vec<_>>(), ["vignette"]);
        e.apply(&Op::Key(crate::model::ops::keys::KeyOp::Clear { on })).unwrap();
        assert!(e.cuts[0].automation.is_empty(), "the whole reset clears automation too");
    }

    #[test]
    fn an_item_follows_a_tracked_object_where_both_run() {
        use crate::automation::{ObjectTrack, TrackSample};
        // Cut 0..4 s of its clip 10..14; the item at edit 1..3 s.
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 10.0, 14.0)], ..Default::default() };
        p.settle_ids();
        let mut e = Edit::default();
        e.replace(p);
        let cut = e.cuts[0].id;
        let item = e.apply(&Op::Item(crate::model::ops::items::ItemOp::AddText { text: Default::default(), at: 1.0, len: 2.0 })).unwrap().made.unwrap();
        let samples = (0..=8).map(|k| TrackSample { t: 10.0 + k as f64 * 0.5, x: 0.1 * k as f64, y: 0.5, size: None, rotation: None, confidence: 1.0 }).collect();
        e.apply(&Op::Cut(crate::model::ops::cuts::CutOp::Track { cut, track: ObjectTrack { name: "ball".into(), samples } })).unwrap();
        e.apply(&Op::Auto(AutoOp::Follow { item, cut, track: "ball".into(), params: vec!["x".into(), "y".into()], offsets: vec![0.0, -0.1] })).unwrap();
        let it = &e.text[0].items[0];
        let x = it.automation.iter().find(|a| a.param == "x").unwrap();
        // Edit 1..3 s is the clip's 11..13 s: samples 2..6, x 0.2..0.6.
        assert_eq!((x.points.len(), x.from, x.to), (5, 0.0, 2.0));
        assert!((x.value_at(1.0) - 0.4).abs() < 1e-9 && x.follows.as_ref().is_some_and(|f| f.track == "ball"));
        assert!((it.automation.iter().find(|a| a.param == "y").unwrap().value_at(1.0) - 0.4).abs() < 1e-9, "the offset added");
        assert!(e.apply(&Op::Auto(AutoOp::Follow { item, cut, track: "ball".into(), params: vec!["size".into()], offsets: vec![] })).is_err(), "no size tracked");
        check(&e).unwrap();
    }


    #[test]
    fn an_item_animates_in_and_out_and_keeps_them_on_its_edges() {
        use crate::animate::Anim;
        let mut e = Edit::default();
        let item = e.apply(&Op::Item(crate::model::ops::items::ItemOp::AddText { text: Default::default(), at: 0.0, len: 6.0 })).unwrap().made.unwrap();
        let animate = |edge, anim| Op::Auto(AutoOp::Animate { item, edge, anim, seconds: 1.0 });
        e.apply(&animate(End::In, Some(Anim::Fade))).unwrap();
        e.apply(&animate(End::Out, Some(Anim::Zoom))).unwrap();
        e.apply(&animate(End::In, Some(Anim::SlideUp))).unwrap();
        let params = |e: &Edit| e.text[0].items[0].automation.iter().map(|c| (c.param.clone(), c.animation.unwrap().edge)).collect::<Vec<_>>();
        assert_eq!(params(&e), [("y".into(), End::In), ("size".into(), End::Out), ("opacity".into(), End::Out)], "the in replaced, the out's two values");
        e.apply(&Op::Item(crate::model::ops::items::ItemOp::Trim { item, end: End::Out, t: 4.0, most: 3600.0 })).unwrap();
        assert!(e.text[0].items[0].automation.iter().filter(|c| c.animation.unwrap().edge == End::Out).all(|c| c.to == 4.0), "kept on the out");
        e.apply(&animate(End::Out, None)).unwrap();
        assert_eq!(params(&e).len(), 1);
        check(&e).unwrap();
    }

}
