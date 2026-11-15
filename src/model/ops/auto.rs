//! Automation clips' operations (gap 39): a clip on a value's lane of a
//! cut or an item, moved, trimmed, taken away, and the points of its
//! curve. A clip is found by its value's name and a time inside it, as
//! keys are by their time.

use super::keys::Graded;
use super::{End, OpError, Outcome, in_range, said};
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
    /// The clip of `param` at `at` gone, its points with it.
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
    /// `clip` put on `on` as it is — its stretch, points and eases — over
    /// none of its value's others (the audit, 2026-11-12: Add's flat ends
    /// made motion a copied lane never had).
    Put { on: Graded, clip: AutoClip },
    /// An ordinary cut's frame keyed at clip time `t` (gap 107, step e):
    /// a point on each of its four Frame lanes, a lane made where none
    /// runs at `t` (over the stretch free of others, its one point holding
    /// before and after), as Kdenlive keys a whole frame at once.
    FrameKey { cut: Id, t: f64, frame: crate::frame::Frame, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// The frame key at `from` moved to `t`, on all four lanes together.
    FrameKeyMove { cut: Id, from: f64, t: f64, #[serde(default = "half_a_frame")] tolerance: f64 },
    /// The frame key at `t` taken away from all four lanes; a lane left
    /// without a point goes.
    FrameKeyDelete { cut: Id, t: f64, #[serde(default = "half_a_frame")] tolerance: f64 },
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
            let (effects, frame, range) = (c.effects.clone(), c.frame.unwrap_or_default(), (c.start, c.end));
            // A Frame lane's value where no clip is: the cut's frame (gap 107).
            let base = move |param: &str, t: f64| match crate::frame::PARAMS.iter().position(|n| *n == param) {
                Some(k) => frame.values()[k],
                None => grade_value(&effects, param, t),
            };
            Ok(Owner { clips: &mut c.automation, range, base: Box::new(base), item: false })
        }
        Graded::Item(id) => {
            let it = p.item_mut(id).ok_or_else(|| OpError::NotFound(format!("item #{id}")))?;
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
    let place = o.item && crate::overlay::Place::NAMES.contains(&param) || !o.item && crate::frame::PARAMS.contains(&param);
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

/// `Put`: `clip` on `on` as it is, within its owner, a point at least,
/// its points in order inside it and their values in range.
fn put_clip(p: &mut Project, on: Graded, clip: &AutoClip) -> Result<Outcome, OpError> {
    let o = owner(p, on)?;
    check_param(&o, &clip.param)?;
    let ordered = clip.points.windows(2).all(|w| w[0].t <= w[1].t);
    let inside = clip.points.iter().all(|q| q.t >= clip.from - 1e-9 && q.t <= clip.to + 1e-9);
    if clip.points.is_empty() || !ordered || !inside || clip.to - clip.from < SHORTEST || clip.from < o.range.0 - 1e-9 || clip.to > o.range.1 + 1e-9 {
        return Err(OpError::Invalid(format!("a {} clip needs a point, its points in order inside it, {SHORTEST} s at least within its own", clip.param)));
    }
    for q in &clip.points {
        in_range(&clip.param, q.v)?;
    }
    clear(o.clips, &clip.param, clip.from, clip.to, None)?;
    let i = o.clips.partition_point(|c| c.from <= clip.from);
    o.clips.insert(i, clip.clone());
    said(format!("{}: {} clip put {:.2}-{:.2} s", name(on), clip.param, clip.from, clip.to))
}

/// A cut's automation clips, by its id.
fn cut_clips(p: &mut Project, cut: Id) -> Result<(&mut Vec<AutoClip>, (f64, f64)), OpError> {
    let i = p.cut_index(cut).ok_or_else(|| OpError::NotFound(format!("cut #{cut}")))?;
    let c = &mut p.cuts[i];
    Ok((&mut c.automation, (c.start, c.end)))
}

/// `FrameKey`: a point at `t` on each Frame lane, a lane made where none runs.
fn frame_key(p: &mut Project, cut: Id, t: f64, frame: crate::frame::Frame, tolerance: f64) -> Result<Outcome, OpError> {
    frame.check().map_err(OpError::Invalid)?;
    let (clips, (start, end)) = cut_clips(p, cut)?;
    if !(start..=end).contains(&t) {
        return Err(OpError::OutOfRange { what: "frame key time", value: t, least: start, most: end });
    }
    for (param, v) in crate::frame::PARAMS.iter().zip(frame.values()) {
        let point = AutoPoint { t, v, ease: Ease::Smooth };
        match clips.iter_mut().find(|c| c.param == *param && c.covers(t)) {
            Some(c) => match c.points.iter_mut().find(|q| (q.t - t).abs() <= tolerance) {
                Some(q) => q.v = v,
                None => {
                    let at = c.points.partition_point(|q| q.t < t);
                    c.points.insert(at, point);
                }
            },
            None => {
                // The stretch around `t` that no other clip of it holds.
                let lo = clips.iter().filter(|c| c.param == *param && c.to <= t).map(|c| c.to).fold(start, f64::max);
                let hi = clips.iter().filter(|c| c.param == *param && c.from >= t).map(|c| c.from).fold(end, f64::min);
                if hi - lo < SHORTEST {
                    return Err(OpError::Invalid(format!("no room for a {param} lane at {t:.2} s")));
                }
                let at = clips.partition_point(|c| c.from <= lo);
                clips.insert(at, AutoClip { param: (*param).to_owned(), from: lo, to: hi, points: vec![point], follows: None, animation: None });
            }
        }
    }
    said(format!("cut #{cut}: frame key at {t:.2} s, {}", frame.said()))
}

/// `FrameKeyMove`: each Frame lane's point at `from` to `t`, within its lane.
fn frame_key_move(p: &mut Project, cut: Id, from: f64, t: f64, tolerance: f64) -> Result<Outcome, OpError> {
    let (clips, _) = cut_clips(p, cut)?;
    let holding = |c: &AutoClip| crate::frame::PARAMS.contains(&c.param.as_str()) && c.points.iter().any(|q| (q.t - from).abs() <= tolerance);
    // One time for all its lanes: within every lane that holds it (the
    // audit, 2026-11-12: lanes of other lengths split it).
    let (lo, hi) = clips.iter().filter(|c| holding(c)).fold((f64::NEG_INFINITY, f64::INFINITY), |(lo, hi), c| (lo.max(c.from), hi.min(c.to)));
    if lo > hi {
        return Err(OpError::NotFound(format!("a frame key at {from:.2} s")));
    }
    let t = t.clamp(lo, hi);
    // Onto another key: refused, not silently merged.
    if (t - from).abs() > tolerance && clips.iter().filter(|c| holding(c)).any(|c| c.points.iter().any(|q| (q.t - t).abs() <= tolerance)) {
        return Err(OpError::Invalid(format!("another frame key is at {t:.2} s")));
    }
    for c in clips.iter_mut().filter(|c| holding(c)) {
        let i = c.points.iter().position(|q| (q.t - from).abs() <= tolerance).expect("holding");
        let mut q = c.points.remove(i);
        q.t = t;
        let at = c.points.partition_point(|o| o.t < q.t);
        c.points.insert(at, q);
    }
    said(format!("cut #{cut}: frame key {from:.2} → {t:.2} s"))
}

/// `FrameKeyDelete`: each Frame lane's point at `t` gone, an emptied lane with it.
fn frame_key_delete(p: &mut Project, cut: Id, t: f64, tolerance: f64) -> Result<Outcome, OpError> {
    let (clips, _) = cut_clips(p, cut)?;
    let before = clips.iter().map(|c| c.points.len()).sum::<usize>();
    for c in clips.iter_mut().filter(|c| crate::frame::PARAMS.contains(&c.param.as_str())) {
        c.points.retain(|q| (q.t - t).abs() > tolerance);
    }
    clips.retain(|c| !c.points.is_empty());
    if clips.iter().map(|c| c.points.len()).sum::<usize>() == before {
        return Err(OpError::NotFound(format!("a frame key at {t:.2} s")));
    }
    said(format!("cut #{cut}: frame key at {t:.2} s gone"))
}

/// `Add`: a clip of `param` over `from`..`to`, flat at the value there.
fn add_clip(p: &mut Project, on: Graded, param: &str, from: f64, to: f64) -> Result<Outcome, OpError> {
    let o = owner(p, on)?;
    check_param(&o, param)?;
    let (from, to) = (from.min(to).max(o.range.0), from.max(to).min(o.range.1));
    if (to - from).partial_cmp(&SHORTEST) == Some(std::cmp::Ordering::Less) || (to - from).is_nan() {
        return Err(OpError::Invalid(format!("an automation clip runs {SHORTEST} s at least, within its own")));
    }
    clear(o.clips, param, from, to, None)?;
    let points = [from, to].map(|t| AutoPoint { t, v: (o.base)(param, t), ease: Ease::Smooth }).to_vec();
    let i = o.clips.partition_point(|c| c.from <= from);
    o.clips.insert(i, AutoClip { param: param.to_owned(), from, to, points, follows: None, animation: None });
    said(format!("{}: {param} automated {from:.2}-{to:.2} s", name(on)))
}

/// `Move`: the clip of `param` at `at` to start at `from`, within its owner.
fn move_clip(p: &mut Project, on: Graded, param: &str, at: f64, from: f64) -> Result<Outcome, OpError> {
    let o = owner(p, on)?;
    let i = clip_at(o.clips, param, at)?;
    let len = o.clips[i].to - o.clips[i].from;
    let from = from.clamp(o.range.0, (o.range.1 - len).max(o.range.0));
    clear(o.clips, param, from, from + len, Some(i))?;
    let c = &mut o.clips[i];
    let by = from - c.from;
    (c.from, c.to) = (from, from + len);
    c.points.iter_mut().for_each(|q| q.t += by);
    o.clips.sort_by(|a, b| a.from.total_cmp(&b.from));
    said(format!("{}: {param} clip to {from:.2} s", name(on)))
}

pub(super) fn apply(p: &mut Project, op: &AutoOp) -> Result<Outcome, OpError> {
    match op {
        AutoOp::Add { on, param, from, to } => add_clip(p, *on, param, *from, *to),
        AutoOp::Move { on, param, at, from } => move_clip(p, *on, param, *at, *from),
        AutoOp::Trim { on, param, at, end, t } => {
            let o = owner(p, *on)?;
            let i = clip_at(o.clips, param, *at)?;
            let (from, to) = (o.clips[i].from, o.clips[i].to);
            let start = *end == End::In;
            let Some(t) = crate::automation::trim_to(start, (from, to), o.range, SHORTEST, *t) else {
                return Err(OpError::Invalid(format!("{param}'s clip has no room to trim there: it runs past its owner")));
            };
            let (from, to) = if start { (t, to) } else { (from, t) };
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
            in_range(param, *v)?;
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
        AutoOp::Put { on, clip } => put_clip(p, *on, clip),
        &AutoOp::FrameKey { cut, t, frame, tolerance } => frame_key(p, cut, t, frame, tolerance),
        &AutoOp::FrameKeyMove { cut, from, t, tolerance } => frame_key_move(p, cut, from, t, tolerance),
        &AutoOp::FrameKeyDelete { cut, t, tolerance } => frame_key_delete(p, cut, t, tolerance),
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
    let samples: Vec<(f64, crate::automation::TrackSample)> = tr.samples.iter().filter(|s| s.t >= c.start && s.t <= c.end).map(|s| (at + c.edit_at(s.t), *s)).collect();
    let it = p.item_mut(item).ok_or_else(|| OpError::NotFound(format!("item #{item}")))?;
    let within: Vec<(f64, crate::automation::TrackSample)> = samples.into_iter().map(|(e, s)| (it.local(e), s)).filter(|(t, _)| *t >= it.start && *t <= it.end).collect();
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
    let it = p.item_mut(item).ok_or_else(|| OpError::NotFound(format!("item #{item}")))?;
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
        // An end point a hair inside its edge still goes with it, shrunk or grown.
        e.apply(&op(AutoOp::Point { on, param: "exposure".into(), t: 8.98, v: 0.5, from: Some(9.0), tolerance: 0.01 })).unwrap();
        for t in [8.0, 9.0] {
            e.apply(&op(AutoOp::Trim { on, param: "exposure".into(), at: 7.6, end: End::Out, t })).unwrap();
            let c = e.cuts[0].automation.iter().find(|c| c.param == "exposure").unwrap();
            assert_eq!((c.points.len(), c.points[1].t, c.points[1].v), (2, t, 0.5), "kept on the edge at {t}");
        }
        assert!(e.apply(&op(AutoOp::Add { on, param: "sparkle".into(), from: 1.0, to: 2.0 })).is_err());
        // A clip hanging past its owner's edge (the cut trimmed under it):
        // no room to trim is an error, not a panic (audit A1).
        e.cuts[0].start = 9.0 - SHORTEST / 2.0;
        assert!(e.apply(&op(AutoOp::Trim { on, param: "exposure".into(), at: 8.0, end: End::In, t: 7.0 })).is_err());
        e.cuts[0].start = 1.0;
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
        let item = e.apply(&Op::Item(crate::model::ops::items::ItemOp::AddText { text: Default::default(), at: 1.0, len: 2.0, lane: 0 })).unwrap().made.unwrap();
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
        let item = e.apply(&Op::Item(crate::model::ops::items::ItemOp::AddText { text: Default::default(), at: 0.0, len: 6.0, lane: 0 })).unwrap().made.unwrap();
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


    #[test]
    fn a_cuts_frame_lanes_zoom_it_from_its_base() {
        use crate::frame::Frame;
        let mut e = Edit::default();
        let cut = e.apply(&Op::Cut(crate::model::ops::cuts::CutOp::Add { input: "a.mp4".into(), start: 0.0, end: 4.0, place: crate::model::ops::cuts::Place::End, stabilize: None, lut: None })).unwrap().made.unwrap();
        e.apply(&Op::Cut(crate::model::ops::cuts::CutOp::Frame { cut, frame: Some(Frame { x: 10.0, ..Frame::default() }) })).unwrap();
        let on = Graded::Cut(cut);
        e.apply(&Op::Auto(AutoOp::Add { on, param: "frame.zoom".into(), from: 0.0, to: 4.0 })).unwrap();
        for (t, v) in [(0.0, 100.0), (4.0, 200.0)] {
            e.apply(&Op::Auto(AutoOp::Point { on, param: "frame.zoom".into(), t, v, from: None, tolerance: 0.0 })).unwrap();
        }
        let at = |t| e.cuts[0].frame_at(t).unwrap();
        assert_eq!((at(0.0).zoom, at(4.0).zoom, at(2.0).x), (100.0, 200.0, 10.0), "the zoom moves, the base's x stays");
        assert!(at(2.0).zoom > 100.0 && at(2.0).zoom < 200.0);
        assert!(e.apply(&Op::Auto(AutoOp::Add { on, param: "x".into(), from: 0.0, to: 1.0 })).is_err(), "an overlay's place isn't a cut's");
        check(&e).unwrap();
    }

    #[test]
    fn frame_keys_at_5_12_and_15_s_move_the_frame_between_them_and_hold_outside() {
        use crate::frame::Frame;
        let mut e = Edit::default();
        let cut = e.apply(&Op::Cut(crate::model::ops::cuts::CutOp::Add { input: "a.mp4".into(), start: 0.0, end: 20.0, place: crate::model::ops::cuts::Place::End, stabilize: None, lut: None })).unwrap().made.unwrap();
        let key = |t, x, zoom| Op::Auto(AutoOp::FrameKey { cut, t, frame: Frame { x, y: 0.0, zoom, rotation: 0.0 }, tolerance: 0.01 });
        // The scooter at 5 s, the two of us at 12 s, all of it at 15 s.
        for (t, x, z) in [(5.0, 30.0, 180.0), (12.0, -20.0, 150.0), (15.0, 0.0, 100.0)] {
            e.apply(&key(t, x, z)).unwrap();
        }
        let f = |e: &Edit, t| e.cuts[0].frame_at(t).unwrap();
        assert_eq!((f(&e, 0.0).zoom, f(&e, 5.0).x, f(&e, 12.0).zoom, f(&e, 20.0).zoom), (180.0, 30.0, 150.0, 100.0), "held before the first, after the last");
        assert!(f(&e, 8.0).x < 30.0 && f(&e, 8.0).x > -20.0, "on its way from the scooter to us");
        assert_eq!(e.cuts[0].automation.len(), 4, "a lane each");
        e.apply(&Op::Auto(AutoOp::FrameKeyMove { cut, from: 12.0, t: 10.0, tolerance: 0.01 })).unwrap();
        assert_eq!(f(&e, 10.0).x, -20.0, "moved on all four");
        e.apply(&Op::Auto(AutoOp::FrameKeyDelete { cut, t: 10.0, tolerance: 0.01 })).unwrap();
        assert!(f(&e, 10.0).x > -20.0 && e.cuts[0].automation.iter().all(|c| c.points.len() == 2));
        check(&e).unwrap();
    }

    #[test]
    fn a_frame_key_moves_as_one_never_onto_another_and_holds_past_its_lanes() {
        use crate::frame::Frame;
        let mut e = Edit::default();
        let cut = e.apply(&Op::Cut(crate::model::ops::cuts::CutOp::Add { input: "a.mp4".into(), start: 0.0, end: 20.0, place: crate::model::ops::cuts::Place::End, stabilize: None, lut: None })).unwrap().made.unwrap();
        let on = Graded::Cut(cut);
        // A zoom lane by hand over 4-20 s, then keys at 5 and 10 s.
        e.apply(&Op::Auto(AutoOp::Add { on, param: "frame.zoom".into(), from: 4.0, to: 20.0 })).unwrap();
        for (t, x) in [(5.0, 30.0), (10.0, -20.0)] {
            e.apply(&Op::Auto(AutoOp::FrameKey { cut, t, frame: Frame { x, zoom: 150.0, ..Frame::default() }, tolerance: 0.01 })).unwrap();
        }
        assert!(e.apply(&Op::Auto(AutoOp::FrameKeyMove { cut, from: 10.0, t: 5.0, tolerance: 0.01 })).is_err(), "not onto the other");
        // The lane's own edge point at 4 s (from Add) out of the way.
        e.apply(&Op::Auto(AutoOp::PointDelete { on, param: "frame.zoom".into(), t: 4.0, tolerance: 0.01 })).unwrap();
        let said = e.apply(&Op::Auto(AutoOp::FrameKeyMove { cut, from: 10.0, t: 2.0, tolerance: 0.01 })).unwrap().summary;
        assert!(said.contains("4.00"), "within every lane holding it: {said}");
        assert!(e.cuts[0].automation.iter().all(|c| c.points.iter().any(|q| (q.t - 4.0).abs() < 1e-9) || c.points.iter().all(|q| q.t != 10.0)));
        // Past the lanes (the cut grown): the nearest edge held.
        let mut cuts = e.cuts.clone();
        cuts[0].end = 30.0;
        assert_eq!(cuts[0].frame_at(25.0).unwrap().zoom, cuts[0].frame_at(20.0).unwrap().zoom);
        // Put: a clip exactly as it is.
        let clip = e.cuts[0].automation[0].clone();
        e.apply(&Op::Auto(AutoOp::Clear { on, params: vec![clip.param.clone()] })).unwrap();
        e.apply(&Op::Auto(AutoOp::Put { on, clip: clip.clone() })).unwrap();
        assert_eq!(e.cuts[0].automation.iter().find(|c| c.param == clip.param), Some(&clip));
        check(&e).unwrap();
    }
}
