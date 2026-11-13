//! The marking keys of a clip's player (I, O, M, N, Page Up / Down,
//! Delete) as a pure function: what's around the playhead and a key in,
//! what to do out. The window carries it out; the rules live here, tested,
//! because the user found them wrong more than once (a segment grown back
//! to an I set earlier in the clip).

use crate::{Mark, MarkKind, MarkOp, Op};

/// A marking key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// I: inside the picked segment, its start here; else an in point.
    In,
    /// O: inside the picked segment, its end here; after an I, a segment
    /// from there; on a flag picked, the flag made a segment to here.
    Out,
    /// M: a flag here, a note to self.
    Flag,
    /// N: a moment, a second either side, taking a flag's role.
    Moment,
    /// Page Up / Down: the mark before or after, picked.
    Prev,
    Next,
    /// Delete / Backspace: the picked mark gone.
    Delete,
}

/// What's around the playhead.
#[derive(Debug, Clone, Copy)]
pub struct At<'a> {
    /// The clip's sheet key.
    pub clip: &'a str,
    pub marks: &'a [Mark],
    /// The picked mark's id.
    pub picked: Option<&'a str>,
    /// An in point set with I, waiting for its O.
    pub mark_in: Option<f64>,
    /// The playhead, and the clip's length.
    pub now: f64,
    pub length: f64,
}

/// What a key does.
#[derive(Debug, Clone, PartialEq)]
pub enum Does {
    /// The sheet changed by `ops`, then `made` says what follows.
    Change { ops: Vec<Op>, made: Made },
    /// An in point at this time; nothing picked.
    InPoint(f64),
    /// This mark picked and the playhead taken to `t`; no in point.
    Go { mark: String, t: f64 },
    /// Refused, and why.
    No(&'static str),
    Nothing,
}

/// After a change: what becomes of the mark made, the pick and the in
/// point.
#[derive(Debug, Clone, PartialEq)]
pub enum Made {
    /// As they were (an edge moved).
    Keep,
    /// The mark made asks for its role (`offer` first, if any); the in
    /// point is used up.
    Ask { offer: Option<String> },
    /// The mark made is picked.
    Pick,
    /// Nothing picked (the picked one went).
    Unpick,
}

/// A flag this near the playhead counts as at it (a few frames).
pub const NEAR_FLAG: f64 = 0.15;

/// What `key` does at `at`.
pub fn key(at: &At<'_>, key: Key) -> Does {
    let picked = at.picked.and_then(|id| at.marks.iter().find(|m| m.id.as_deref() == Some(id)));
    let id = |m: &Mark| m.id.clone().unwrap_or_default();
    let now = at.now;
    let change = |ops: Vec<Op>, made: Made| Does::Change { ops, made };
    let add = |t: f64, end: Option<f64>, flag: bool, role: Option<String>, note: String| Op::Mark(MarkOp::Add { clip: at.clip.to_owned(), t, end, flag, role, note });
    // Inside the picked segment: I from its start on, O up to its end.
    let segment = |inside: &dyn Fn(f64, f64) -> bool| picked.filter(|m| m.kind == MarkKind::Segment && m.end.is_some_and(|e| inside(m.t, e)));
    match key {
        Key::In => match segment(&|s, e| now >= s && now < e) {
            Some(m) => change(vec![Op::Mark(MarkOp::Move { mark: id(m), t: now, end: m.end })], Made::Keep),
            None => Does::InPoint(now),
        },
        Key::Out => match (segment(&|s, e| now > s && now <= e), at.mark_in, picked) {
            (Some(m), ..) => change(vec![Op::Mark(MarkOp::Move { mark: id(m), t: m.t, end: Some(now) })], Made::Keep),
            (_, Some(start), _) => change(vec![add(start.min(now), Some(start.max(now)), false, None, String::new())], Made::Ask { offer: None }),
            (_, None, Some(f)) if f.kind == MarkKind::Flag && now > f.t => change(vec![Op::Mark(MarkOp::Remove { mark: id(f) }), add(f.t, Some(now), false, f.role.clone(), f.note.clone())], Made::Pick),
            _ => Does::No("No start for a segment: set one with I first"),
        },
        Key::Flag => change(vec![add(now, None, true, None, String::new())], Made::Ask { offer: None }),
        Key::Moment => {
            let flag = picked.filter(|m| m.kind == MarkKind::Flag).or_else(|| at.marks.iter().find(|m| m.kind == MarkKind::Flag && (m.t - now).abs() <= NEAR_FLAG));
            let role = flag.and_then(|f| f.role.clone());
            change(vec![add((now - 1.0).max(0.0), Some((now + 1.0).min(at.length)), false, role.clone(), String::new())], Made::Ask { offer: role })
        }
        Key::Prev | Key::Next => {
            let mut marks: Vec<&Mark> = at.marks.iter().collect();
            marks.sort_by(|a, b| a.t.total_cmp(&b.t));
            let from = picked.map_or(now, |m| m.t);
            let next = if key == Key::Next { marks.into_iter().find(|m| m.t > from + 1e-3) } else { marks.into_iter().rev().find(|m| m.t < from - 1e-3) };
            next.map_or(Does::Nothing, |m| Does::Go { mark: id(m), t: m.t })
        }
        Key::Delete => match at.picked {
            Some(mark) => change(vec![Op::Mark(MarkOp::Remove { mark: mark.to_owned() })], Made::Unpick),
            None => Does::Nothing,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(id: &str, t: f64, end: Option<f64>, kind: MarkKind, role: Option<&str>) -> Mark {
        Mark { id: Some(id.into()), t, end, kind, role: role.map(Into::into), note: format!("{id}'s note") }
    }

    fn at<'a>(marks: &'a [Mark], picked: Option<&'a str>, mark_in: Option<f64>, now: f64) -> At<'a> {
        At { clip: "2030-05-01/A.MP4", marks, picked, mark_in, now, length: 30.0 }
    }

    #[test]
    fn i_and_o_move_only_the_picked_segment_the_playhead_is_in() {
        let marks = [mark("m1", 10.0, Some(20.0), MarkKind::Segment, None)];
        let moved = |t: f64, end: Option<f64>| Does::Change { ops: vec![Op::Mark(MarkOp::Move { mark: "m1".into(), t, end })], made: Made::Keep };
        assert_eq!(key(&at(&marks, Some("m1"), None, 12.0), Key::In), moved(12.0, Some(20.0)));
        assert_eq!(key(&at(&marks, Some("m1"), None, 15.0), Key::Out), moved(10.0, Some(15.0)));
        // Outside it, I sets an in point and never stretches it.
        assert_eq!(key(&at(&marks, Some("m1"), None, 25.0), Key::In), Does::InPoint(25.0));
        // O after an I: a segment either way round, asking its role.
        let Does::Change { ops, made } = key(&at(&marks, None, Some(25.0), 22.0), Key::Out) else { panic!() };
        assert_eq!((ops, made), (vec![Op::Mark(MarkOp::Add { clip: "2030-05-01/A.MP4".into(), t: 22.0, end: Some(25.0), flag: false, role: None, note: String::new() })], Made::Ask { offer: None }));
        assert!(matches!(key(&at(&marks, None, None, 22.0), Key::Out), Does::No(_)));
    }

    #[test]
    fn a_flag_becomes_a_segment_or_lends_its_role_to_a_moment() {
        let marks = [mark("m2", 5.0, None, MarkKind::Flag, Some("cut"))];
        let Does::Change { ops, made } = key(&at(&marks, Some("m2"), None, 9.0), Key::Out) else { panic!() };
        assert_eq!(made, Made::Pick);
        assert_eq!(ops[0], Op::Mark(MarkOp::Remove { mark: "m2".into() }));
        assert!(matches!(&ops[1], Op::Mark(MarkOp::Add { t: 5.0, end: Some(9.0), role: Some(r), note, .. }) if r == "cut" && note == "m2's note"));
        // N within a few frames of the flag, nothing picked: its role
        // offered; at the clip's start, clamped.
        let Does::Change { ops, made } = key(&at(&marks, None, None, 5.1), Key::Moment) else { panic!() };
        assert_eq!(made, Made::Ask { offer: Some("cut".into()) });
        assert!(matches!(&ops[0], Op::Mark(MarkOp::Add { role: Some(r), .. }) if r == "cut"));
        let Does::Change { ops, .. } = key(&at(&[], None, None, 0.4), Key::Moment) else { panic!() };
        assert!(matches!(ops[0], Op::Mark(MarkOp::Add { t: 0.0, end: Some(e), .. }) if (e - 1.4).abs() < 1e-9));
    }

    #[test]
    fn page_keys_go_mark_to_mark_and_delete_takes_the_picked() {
        let marks = [mark("m1", 10.0, Some(20.0), MarkKind::Segment, None), mark("m2", 5.0, None, MarkKind::Flag, None)];
        assert_eq!(key(&at(&marks, None, None, 0.0), Key::Next), Does::Go { mark: "m2".into(), t: 5.0 });
        assert_eq!(key(&at(&marks, Some("m2"), None, 0.0), Key::Next), Does::Go { mark: "m1".into(), t: 10.0 });
        assert_eq!(key(&at(&marks, Some("m2"), None, 0.0), Key::Prev), Does::Nothing);
        assert_eq!(key(&at(&marks, Some("m1"), None, 0.0), Key::Delete), Does::Change { ops: vec![Op::Mark(MarkOp::Remove { mark: "m1".into() })], made: Made::Unpick });
        assert_eq!(key(&at(&marks, None, None, 0.0), Key::Delete), Does::Nothing);
    }
}
