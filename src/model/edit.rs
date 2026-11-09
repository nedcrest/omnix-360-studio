//! The timeline's edit: a `Project` (cuts played one after another, the
//! sound tracks under them, overlays, the mix) with undo. It derefs to the
//! project, so `edit.cuts` is the project's; an undo step is a copy of the
//! project. Plain data, so the edits can be checked without a window.
//!
//! Edit time runs along the timeline from 0 to the edit's length; each cut
//! covers its stretch of it, and clip time is where in its own clip that is.
//! A cut with a crossfade starts that much before the one before it ends;
//! through the overlap the earlier cut is the one located, the later one
//! dissolving in over it.

use crate::overlay::{Item, ItemRef, Lane, Track};
use crate::model::ops::End;
use crate::project::{Cut, Project};

/// A transition drawn on the lane (`Project::drawn_zone`): the join it
/// belongs to (the cut after it), and an overlap or an edge look on the
/// cut before (`Out`) or after (`In`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    Overlap { join: usize, seconds: f64 },
    Edge { join: usize, end: End, seconds: f64 },
}

/// The shortest a split or trim may leave a cut, in seconds.
pub const MIN_CUT: f64 = 0.2;

/// How many steps undo goes back.
pub const UNDO_STEPS: usize = 300;

#[derive(Default)]
pub struct Edit {
    /// What's being edited. Its `aspect` isn't used: the app keeps the
    /// shape and writes it on save (`App::to_project`). The player shows and
    /// edits the first overlay lane and the first text lane.
    project: Project,
    /// Copies of the project to go back and forth to.
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// Counts changes, so the app can tell whether the edit was saved.
    pub version: u64,
    /// A change made of many small ones under way (a drag, a run of
    /// typing): the edit before it, and the version then.
    open: Option<(Project, u64)>,
}

impl std::ops::Deref for Edit {
    type Target = Project;

    fn deref(&self) -> &Project {
        &self.project
    }
}

impl std::ops::DerefMut for Edit {
    fn deref_mut(&mut self) -> &mut Project {
        &mut self.project
    }
}

/// Reading and changing the edit's cuts and lanes: plain project rules,
/// so operations (`model::ops`) apply them to a `Project` and the player
/// through `Edit`'s deref alike.
impl Project {
    /// The timeline's length: the video's, or the music's (or sound
    /// effects') while it runs on beyond it (the frame still to fill). The
    /// export ends with the video.
    pub fn frame(&self) -> f64 {
        let overlays = self.lanes().flat_map(|l| &l.items).map(Item::ends).fold(0.0, f64::max);
        let sounds = crate::project::SoundTrack::ALL.iter().map(|&t| self.track(t).end()).fold(0.0, f64::max);
        self.duration().max(sounds).max(overlays)
    }

    /// Where cut `i` starts on the timeline (the edit's end for `i` past
    /// the last).
    pub fn offset(&self, i: usize) -> f64 {
        match self.cuts.get(i) {
            Some(_) => crate::project::offsets(&self.cuts[..=i])[i],
            None => self.duration(),
        }
    }

    /// The cut at edit time `t` and the clip time there. The end of the edit
    /// is the last cut's out point.
    pub fn locate(&self, t: f64) -> Option<(usize, f64)> {
        let offsets = crate::project::offsets(&self.cuts);
        for (i, (c, from)) in self.cuts.iter().zip(offsets).enumerate() {
            if t < from + c.len() || i == self.cuts.len() - 1 {
                return Some((i, (c.start + (t - from).max(0.0)).min(c.end)));
            }
        }
        None
    }

    /// Where the crossfade into the cut after cut `i` begins, clip time in
    /// cut `i`, if it has one.
    pub fn crossfade_from(&self, i: usize) -> Option<f64> {
        let next = self.cuts.get(i + 1).filter(|n| n.crossfade > 0.0)?;
        Some(self.cuts[i].end - next.crossfade)
    }

    pub fn edit_time(&self, i: usize, clip_t: f64) -> f64 {
        self.offset(i) + (clip_t - self.cuts[i].start)
    }

}

impl Edit {
    /// Keeps `before` as the latest undo step, forgetting the oldest past
    /// `UNDO_STEPS`: each is a whole copy of the project, and a long
    /// session's would otherwise grow without end.
    fn remember(&mut self, before: Project) {
        if self.undo.len() >= UNDO_STEPS {
            self.undo.remove(0);
        }
        self.undo.push(before);
    }

    fn snapshot(&self) -> Project {
        self.project.clone()
    }

    fn restore(&mut self, then: Project) -> Project {
        std::mem::replace(&mut self.project, then)
    }

    /// Starts a change made of many small ones (a drag, a run of typing),
    /// to be one undo step at `commit`; while one is under way, this does
    /// nothing. It counts as unsaved from the start, as the edit shows it.
    pub fn begin(&mut self) {
        if self.open.is_none() {
            self.open = Some((self.snapshot(), self.version));
            self.version += 1;
        }
    }

    /// Whether a change begun is still under way.
    pub fn changing(&self) -> bool {
        self.open.is_some()
    }

    /// Ends the change begun: one undo step, or none if it changed nothing
    /// (a press without a move used to leave an empty one).
    pub fn commit(&mut self) {
        let Some((before, version)) = self.open.take() else { return };
        if before == self.project {
            self.version = version;
        } else {
            self.remember(before);
            self.redo.clear();
        }
    }


    /// Drops the change under way (Esc during a drag): the edit as it was
    /// before it began, saved or not as it was then.
    pub fn cancel(&mut self) {
        if let Some((before, version)) = self.open.take() {
            self.project = before;
            self.version = version;
        }
    }

    /// Makes `change` one undo step if it says it changed anything; if it
    /// says not, the edit is as it was, with redo and the saved state
    /// untouched. (Saving the edit first and undoing on a failed change
    /// wiped redo and marked the edit unsaved.)
    pub fn attempt(&mut self, change: impl FnOnce(&mut Edit) -> bool) -> bool {
        self.commit();
        let before = self.snapshot();
        if change(self) {
            self.remember(before);
            self.redo.clear();
            self.version += 1;
            true
        } else {
            self.restore(before);
            false
        }
    }

    pub fn undo(&mut self) -> bool {
        self.commit();
        let Some(then) = self.undo.pop() else { return false };
        let now = self.restore(then);
        self.redo.push(now);
        self.version += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        self.commit();
        let Some(then) = self.redo.pop() else { return false };
        let now = self.restore(then);
        self.remember(now);
        self.version += 1;
        true
    }

    /// Starts over with `project` (an opened or new one), forgetting undo.
    pub fn replace(&mut self, project: Project) {
        self.project = project;
        self.open = None;
        self.undo.clear();
        self.redo.clear();
        self.version += 1;
    }

}

impl Project {
    /// Every overlay lane, lowest first: the overlays', then the text's.
    pub fn lanes(&self) -> impl Iterator<Item = &Lane> {
        self.overlays.iter().chain(&self.text)
    }

    /// `track`'s lanes (gap 70), lowest first: a lane is drawn over the
    /// ones before it, and shown above them on the timeline.
    pub fn track_lanes(&self, track: Track) -> &[Lane] {
        match track {
            Track::Overlay => &self.overlays,
            Track::Text => &self.text,
        }
    }

    /// Lane `l` of `track`, if there is one.
    pub fn lane(&self, track: Track, l: usize) -> Option<&Lane> {
        self.track_lanes(track).get(l)
    }

    /// Lane `l` of `track`, made (with any below it) if it isn't there yet.
    pub fn lane_mut(&mut self, track: Track, l: usize) -> &mut Lane {
        let lanes = match track {
            Track::Overlay => &mut self.overlays,
            Track::Text => &mut self.text,
        };
        while lanes.len() <= l {
            lanes.push(Lane::default());
        }
        &mut lanes[l]
    }

    pub fn item(&self, (track, l, i): ItemRef) -> Option<&Item> {
        self.lane(track, l)?.items.get(i)
    }

    /// Splits cut `i` at clip time `t`. Both halves keep all its keyframes,
    /// so the view runs on across the split unchanged, and each half can be
    /// reframed on its own from then on.
    pub fn split(&mut self, i: usize, t: f64) -> bool {
        let c = &self.cuts[i];
        if t < c.start + MIN_CUT || t > c.end - MIN_CUT {
            return false;
        }
        // Each half keeps its sound fade, its sound past the picture and
        // its edge look at its own outer end only; the join before is the
        // first half's.
        let mut second = c.clone();
        second.start = t;
        second.crossfade = 0.0;
        second.transition = None;
        second.transition_params.clear();
        second.sound_fade_in = 0.0;
        second.sound_before = 0.0;
        second.edge_in = None;
        self.cuts[i].end = t;
        self.cuts[i].sound_fade_out = 0.0;
        self.cuts[i].sound_after = 0.0;
        self.cuts[i].edge_out = None;
        // A sound stopping short of the picture may leave a half too little.
        self.cuts[i].settle_sound(f64::INFINITY);
        second.settle_sound(f64::INFINITY);
        self.cuts.insert(i + 1, second);
        crate::project::settle_crossfades(&mut self.cuts);
        true
    }

    /// Moves cut `i`'s in point (`start`) or out point to clip time `t`,
    /// kept within the clip (`0..clip_len`) and at least `MIN_CUT` long.
    pub fn trim(&mut self, i: usize, start: bool, t: f64, clip_len: f64) {
        let c = &mut self.cuts[i];
        // Bounds kept the right way round: a cut ending near its clip's
        // end (or a clip shorter than told) would make `clamp` panic.
        if start {
            c.start = t.clamp(0.0, (c.end - MIN_CUT).max(0.0));
        } else {
            c.end = t.clamp(c.start + MIN_CUT, clip_len.max(c.start + MIN_CUT));
        }
        // The sound past the picture moves with its edge, within the clip.
        c.settle_sound(clip_len);
        crate::project::settle_crossfades(&mut self.cuts);
    }

    /// Sets the crossfade into cut `i`, within what the cuts either side
    /// allow.
    pub fn set_crossfade(&mut self, i: usize, seconds: f64) {
        let max = crate::project::max_crossfade(&self.cuts, i).max(0.0);
        if let Some(c) = self.cuts.get_mut(i) {
            c.crossfade = seconds.clamp(0.0, max);
        }
    }

    pub fn remove(&mut self, i: usize) {
        self.cuts.remove(i);
        crate::project::settle_crossfades(&mut self.cuts);
    }

    /// What a rectangle drawn on the transitions lane from edit time `a` to
    /// `b` makes (gap 25f): across a join's middle, its overlap as long as
    /// the rectangle; on one side, an edge look on the end of that cut
    /// nearer the rectangle, from the cut's edge to the rectangle's far
    /// side. Lengths on the transitions' steps. None on an end with no join
    /// (the edit's own fades are there) or off the cuts.
    pub fn drawn_zone(&self, a: f64, b: f64) -> Option<Zone> {
        let (a, b) = (a.min(b), a.max(b));
        let offsets = crate::project::offsets(&self.cuts);
        let mid = |i: usize| offsets[i] + self.cuts[i].crossfade / 2.0;
        let centre = (a + b) / 2.0;
        let across = (1..self.cuts.len()).filter(|&i| a < mid(i) && mid(i) < b).min_by(|&i, &j| (mid(i) - centre).abs().total_cmp(&(mid(j) - centre).abs()));
        if let Some(i) = across {
            return Some(Zone::Overlap { join: i, seconds: crate::transition::stepped(b - a) });
        }
        let k = (0..self.cuts.len()).find(|&k| centre >= offsets[k] && centre < offsets[k] + self.cuts[k].len())?;
        let (from, to) = (offsets[k], offsets[k] + self.cuts[k].len());
        let (end, seconds, join) = if centre - from < to - centre { (End::In, b - from, k) } else { (End::Out, to - a, k + 1) };
        let seconds = crate::transition::stepped(seconds).min(self.cuts[k].len());
        (join >= 1 && join < self.cuts.len()).then_some(Zone::Edge { join, end, seconds })
    }

    /// The join of two cuts nearest edit time `t`, as the position a cut
    /// put there gets: 0 before the first cut, `cuts.len()` after the last.
    pub fn nearest_join(&self, t: f64) -> usize {
        let offsets = crate::project::offsets(&self.cuts);
        let mut joins: Vec<f64> = offsets.clone();
        joins.push(self.duration());
        (0..joins.len()).min_by(|&a, &b| (joins[a] - t).abs().total_cmp(&(joins[b] - t).abs())).unwrap_or(0)
    }

    /// Puts `cut` at position `at`; returns where it went.
    pub fn insert_cut(&mut self, at: usize, cut: Cut) -> usize {
        let at = at.min(self.cuts.len());
        self.cuts.insert(at, cut);
        crate::project::settle_crossfades(&mut self.cuts);
        at
    }

    /// Puts `cut` in at edit time `t`: the cut playing there is split and
    /// `cut` goes between the halves; within `MIN_CUT` of a join, it goes at
    /// that join. Returns where it went.
    pub fn insert_at(&mut self, t: f64, cut: Cut) -> usize {
        if let Some((i, clip_t)) = self.locate(t)
            && self.split(i, clip_t)
        {
            return self.insert_cut(i + 1, cut);
        }
        let at = self.nearest_join(t);
        self.insert_cut(at, cut)
    }

    /// Where the cut at index `i` is after cut `from` moved to position `to`
    /// (`move_cut`'s numbers): for keeping the playing and the selected cut
    /// the same cuts.
    pub fn moved_index(from: usize, to: usize, i: usize) -> usize {
        let landed = if to > from { to - 1 } else { to };
        if i == from {
            return landed;
        }
        let j = if i > from { i - 1 } else { i };
        if j >= landed { j + 1 } else { j }
    }

    /// Moves cut `from` so it plays at position `to` (counted before the move).
    pub fn move_cut(&mut self, from: usize, to: usize) {
        let cut = self.cuts.remove(from);
        let to = if to > from { to - 1 } else { to };
        let to = to.min(self.cuts.len());
        self.cuts.insert(to, cut);
        crate::project::settle_crossfades(&mut self.cuts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cut(name: &str, start: f64, end: f64) -> Cut {
        Cut::new(PathBuf::from(name), start, end)
    }

    fn edit() -> Edit {
        Edit { project: Project { cuts: vec![cut("a", 10.0, 14.0), cut("b", 2.0, 5.0)], ..Default::default() }, ..Default::default() }
    }

    #[test]
    fn a_drag_is_one_undo_step_and_a_still_press_none() {
        let mut e = edit();
        let v = e.version;
        // Pressed and let go without a move: no step, saved as it was.
        e.begin();
        assert!(e.changing() && e.version != v, "unsaved while under way");
        e.commit();
        assert!(!e.changing() && e.version == v && !e.undo());
        // Many moves, one step back to before them.
        e.begin();
        for t in [11.0, 12.0, 13.0] {
            e.trim(0, true, t, 20.0);
            e.begin();
        }
        e.commit();
        assert_eq!(e.cuts[0].start, 13.0);
        assert!(e.undo());
        assert_eq!(e.cuts[0].start, 10.0);
        assert!(!e.undo(), "one step only");
        // A change of another kind ends the one under way first.
        e.begin();
        e.trim(0, true, 11.0, 20.0);
        e.attempt(|e| {
            e.cuts[1].crossfade = 0.5;
            true
        });
        assert!(e.undo() && e.cuts[0].start == 11.0 && e.undo() && e.cuts[0].start == 10.0);
        // Cancelled, a drag leaves nothing: no change, no step, as saved.
        let v = e.version;
        e.begin();
        e.trim(0, true, 12.0, 20.0);
        e.cancel();
        assert!(!e.changing() && e.cuts[0].start == 10.0 && e.version == v && !e.undo());
    }

    #[test]
    fn undo_keeps_the_latest_steps_only() {
        let mut e = edit();
        for n in 0..UNDO_STEPS + 5 {
            e.attempt(|e| {
                e.cuts[0].gain = n as f64;
                true
            });
        }
        let mut back = 0;
        while e.undo() {
            back += 1;
        }
        assert_eq!(back, UNDO_STEPS);
        assert_eq!(e.cuts[0].gain, 4.0, "the oldest five forgotten");
    }

    #[test]
    fn inserts_at_the_playhead_splitting_the_cut_there() {
        let mut e = edit();
        // a: 0..4 on the edit (clip 10..14), b: 4..7 (clip 2..5)
        assert_eq!(e.insert_at(1.5, cut("x", 0.0, 2.0)), 1);
        let got: Vec<_> = e.cuts.iter().map(|c| (c.input.to_string_lossy().into_owned(), c.start, c.end)).collect();
        assert_eq!(got, [("a".into(), 10.0, 11.5), ("x".into(), 0.0, 2.0), ("a".into(), 11.5, 14.0), ("b".into(), 2.0, 5.0)]);
    }

    #[test]
    fn inserts_at_a_join_when_the_playhead_is_on_one() {
        let mut e = edit();
        assert_eq!(e.insert_at(4.0, cut("x", 0.0, 2.0)), 1);
        assert_eq!(e.cuts.len(), 3);
        assert_eq!(e.cuts[2].input, PathBuf::from("b"));
    }

    #[test]
    fn nearest_join_counts_both_ends() {
        let e = edit();
        assert_eq!(e.nearest_join(1.0), 0);
        assert_eq!(e.nearest_join(3.0), 1);
        assert_eq!(e.nearest_join(6.9), 2);
    }

    #[test]
    fn maps_edit_time_to_cuts() {
        let e = edit();
        assert_eq!(e.duration(), 7.0);
        assert_eq!(e.locate(1.0), Some((0, 11.0)));
        assert_eq!(e.locate(4.5), Some((1, 2.5)));
        assert_eq!(e.locate(9.0), Some((1, 5.0)));
        assert_eq!(e.edit_time(1, 3.0), 5.0);
    }

    #[test]
    fn crossfades_overlap_cuts_and_stay_within_them() {
        let mut e = edit();
        e.set_crossfade(1, 1.0);
        // b starts a second before a ends, and the edit is a second shorter;
        // through the overlap a is the cut located.
        assert_eq!((e.offset(1), e.duration()), (3.0, 6.0));
        assert_eq!(e.locate(3.5), Some((0, 13.5)));
        assert_eq!(e.locate(4.5), Some((1, 3.5)));
        assert_eq!(e.crossfade_from(0), Some(13.0));
        assert_eq!(e.edit_time(1, 2.0), 3.0);
        // Not longer than the cuts allow, none into the first, and a trim
        // that leaves too little shortens it.
        e.set_crossfade(1, 9.0);
        assert!((e.cuts[1].crossfade - 2.9).abs() < 1e-9);
        e.set_crossfade(0, 1.0);
        assert_eq!(e.cuts[0].crossfade, 0.0);
        e.trim(1, false, 3.0, 9.0);
        assert!((e.cuts[1].crossfade - 0.9).abs() < 1e-9);
        // Moved first, it has none.
        e.move_cut(1, 0);
        assert_eq!((e.cuts[0].crossfade, e.duration()), (0.0, 5.0));
    }

    #[test]
    fn splits_trims_moves_and_undoes() {
        let mut e = edit();
        e.begin();
        assert!(e.split(0, 12.0));
        assert!(!e.split(0, 11.9));
        assert_eq!((e.cuts[0].end, e.cuts[1].start, e.cuts.len()), (12.0, 12.0, 3));
        e.trim(2, false, 99.0, 6.0);
        e.trim(1, true, 13.95, 20.0);
        assert_eq!((e.cuts[2].end, e.cuts[1].start), (6.0, 13.8));
        e.move_cut(2, 0);
        assert_eq!(e.cuts[0].input, PathBuf::from("b"));
        e.move_cut(0, 3);
        assert_eq!(e.cuts[2].input, PathBuf::from("b"));
        e.commit();
        assert!(e.undo());
        assert_eq!(e.cuts.len(), 2);
        assert!(e.redo());
        assert_eq!(e.cuts.len(), 3);
    }

    #[test]
    fn crossfades_settle_after_removing_and_inserting() {
        let mut e = Edit { project: Project { cuts: vec![cut("a", 0.0, 4.0), cut("b", 0.0, 4.0), cut("c", 0.0, 4.0)], ..Default::default() }, ..Default::default() };
        e.cuts[1].crossfade = 1.0;
        e.cuts[2].crossfade = 1.0;
        // The first cut has nothing before it: its crossfade goes.
        e.remove(0);
        assert_eq!((e.cuts[0].crossfade, e.cuts[1].crossfade), (0.0, 1.0));
        // A short cut put before c: c's crossfade can't be longer than it.
        e.insert_cut(1, cut("x", 0.0, 0.5));
        assert!(e.cuts[2].crossfade <= 0.5 && e.cuts[2].crossfade >= 0.0, "{}", e.cuts[2].crossfade);
        // Too short for any crossfade at all: none, never negative.
        e.insert_cut(2, cut("y", 0.0, 0.05));
        assert!(e.cuts.iter().all(|c| c.crossfade >= 0.0));
        assert_eq!(e.cuts[3].crossfade, 0.0);
    }

    #[test]
    fn edit_time_and_clip_time_go_back_and_forth() {
        let mut e = Edit { project: Project { cuts: vec![cut("a", 10.0, 14.0), cut("b", 2.0, 5.0), cut("c", 0.0, 6.0)], ..Default::default() }, ..Default::default() };
        e.cuts[1].crossfade = 0.5;
        e.cuts[2].crossfade = 1.0;
        let duration = e.duration();
        assert!((duration - (4.0 + 3.0 + 6.0 - 1.5)).abs() < 1e-9);
        let mut t = 0.0;
        while t < duration {
            let (i, clip_t) = e.locate(t).unwrap();
            // Within an overlap locate gives the earlier cut; either way the
            // time comes back.
            assert!((e.edit_time(i, clip_t) - t).abs() < 1e-9, "t {t}: cut {i} at {clip_t}");
            assert!(clip_t >= e.cuts[i].start && clip_t <= e.cuts[i].end);
            t += 0.05;
        }
        // Past the end: the last cut's out point.
        assert_eq!(e.locate(duration + 5.0), Some((2, 6.0)));
        assert_eq!(Edit::default().locate(1.0), None);
    }

    #[test]
    fn moved_index_follows_every_move() {
        let names = ["a", "b", "c", "d", "e"];
        for from in 0..5 {
            for to in 0..=5 {
                let mut e = Edit { project: Project { cuts: names.iter().map(|n| cut(n, 0.0, 1.0)).collect(), ..Default::default() }, ..Default::default() };
                e.move_cut(from, to);
                let mut seen = [false; 5];
                for (i, name) in names.iter().enumerate() {
                    let j = Project::moved_index(from, to, i);
                    assert_eq!(e.cuts[j].input, PathBuf::from(name), "from {from} to {to}: {name}");
                    seen[j] = true;
                }
                assert!(seen.iter().all(|&s| s), "a permutation, from {from} to {to}");
            }
        }
    }

    #[test]
    fn a_rectangle_on_the_lane_makes_an_overlap_across_a_join_and_an_edge_beside_it() {
        // a 0..4, b 4..7, c 7..13: joins at 4 and 7.
        let e = Edit { project: Project { cuts: vec![cut("a", 0.0, 4.0), cut("b", 0.0, 3.0), cut("c", 0.0, 6.0)], ..Default::default() }, ..Default::default() };
        assert_eq!(e.drawn_zone(3.6, 4.4), Some(Zone::Overlap { join: 1, seconds: 0.75 }));
        assert_eq!(e.drawn_zone(7.2, 6.6), Some(Zone::Overlap { join: 2, seconds: 0.5 }), "either way round");
        assert_eq!(e.drawn_zone(2.5, 3.2), Some(Zone::Edge { join: 1, end: End::Out, seconds: 1.5 }), "a's end, from the rectangle's start");
        assert_eq!(e.drawn_zone(4.3, 4.9), Some(Zone::Edge { join: 1, end: End::In, seconds: 1.0 }), "b's start, to the rectangle's end");
        assert_eq!(e.drawn_zone(8.0, 8.4), Some(Zone::Edge { join: 2, end: End::In, seconds: 1.5 }));
        assert_eq!((e.drawn_zone(0.5, 1.0), e.drawn_zone(12.0, 12.5)), (None, None), "the edit's own ends have no join");
    }

    #[test]
    fn a_split_keeps_each_sound_fade_and_edge_at_its_outer_end() {
        use crate::transition::Look;
        let mut e = edit();
        e.cuts[0].sound_fade_in = 0.5;
        e.cuts[0].sound_fade_out = 1.0;
        let edge = |look| Some(crate::project::Edge { look, seconds: 0.5, params: Default::default() });
        (e.cuts[0].edge_in, e.cuts[0].edge_out, e.cuts[0].transition) = (edge(Look::EdgeBlur), edge(Look::EdgeFade), Some(Look::Flash));
        assert!(e.split(0, 12.0));
        assert_eq!((e.cuts[0].sound_fade_in, e.cuts[0].sound_fade_out), (0.5, 0.0));
        assert_eq!((e.cuts[1].sound_fade_in, e.cuts[1].sound_fade_out), (0.0, 1.0));
        assert_eq!((e.cuts[0].edge_in.is_some(), e.cuts[0].edge_out.is_some(), e.cuts[1].edge_in.is_some(), e.cuts[1].edge_out.is_some()), (true, false, false, true));
        assert_eq!((e.cuts[0].transition, e.cuts[1].transition), (Some(Look::Flash), None), "the join before is the first half's");
    }

    #[test]
    fn indices_follow_a_moved_cut() {
        // Five cuts a b c d e; b (1) moved to before e (4): a c d b e.
        let mut e = Edit { project: Project { cuts: ["a", "b", "c", "d", "e"].iter().map(|n| cut(n, 0.0, 1.0)).collect(), ..Default::default() }, ..Default::default() };
        e.move_cut(1, 4);
        let names: Vec<_> = e.cuts.iter().map(|c| c.input.to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a", "c", "d", "b", "e"]);
        // Each old index lands where its cut now is.
        assert_eq!((0..5).map(|i| Project::moved_index(1, 4, i)).collect::<Vec<_>>(), [0, 3, 1, 2, 4]);
        // And moving one earlier.
        assert_eq!((0..5).map(|i| Project::moved_index(3, 0, i)).collect::<Vec<_>>(), [1, 2, 3, 0, 4]);
    }

    #[test]
    fn a_change_that_fails_leaves_undo_redo_and_the_saved_state() {
        let mut e = Edit { project: Project { cuts: vec![cut("a.mp4", 0.0, 10.0)], ..Default::default() }, ..Default::default() };
        e.attempt(|e| {
            e.cuts[0].end = 8.0;
            true
        });
        e.undo();
        let version = e.version;
        // Too near the in point to split: nothing changes, redo survives.
        assert!(!e.attempt(|e| e.split(0, 0.05)));
        assert_eq!((e.version, e.cuts.len(), e.cuts[0].end), (version, 1, 10.0));
        assert!(e.redo(), "redo kept");
        // A change that works is one undo step.
        assert!(e.attempt(|e| e.split(0, 4.0)));
        assert_eq!(e.cuts.len(), 2);
        assert!(e.undo());
        assert_eq!(e.cuts.len(), 1);
    }
}
