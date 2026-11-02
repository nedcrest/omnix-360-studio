//! What's selected on the timeline: one thing at a time (`CORE-API.md` step
//! 5). It was five loose `Option`s, each «select this» clearing the others
//! by hand, and some forgot one: a picked overlay left the cut it was over
//! selected too.

use super::edit::Edit;
use super::ops::SoundRef;
use crate::overlay::{ItemRef, Track};
use crate::project::{Id, Own, Project, SoundTrack};

/// Named by id (a point by its sound and time), as the ops name things: a
/// move, a delete before it or an undo leaves it on the same thing, where
/// an index had to be remapped by hand after each (core review, item 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Selection {
    #[default]
    None,
    Cut(Id),
    /// A clip on a sound track.
    Clip(Id),
    /// The point at time `.1` (its file's) on a sound's own volume line.
    Point(SoundRef, f64),
    /// An overlay or text item, in the lane of its track the player shows.
    Item(Id),
}

impl Selection {
    /// Cut `i` of `p`.
    pub fn cut_at(p: &Project, i: usize) -> Self {
        p.cuts.get(i).map_or(Selection::None, |c| Selection::Cut(c.id))
    }

    /// Clip `i` on track `w` of `p`.
    pub fn clip_at(p: &Project, w: SoundTrack, i: usize) -> Self {
        p.track(w).clips.get(i).map_or(Selection::None, |c| Selection::Clip(c.id))
    }

    /// Point `k` on `own`'s line in `p`.
    pub fn point_at(p: &Project, own: Own, k: usize) -> Self {
        match (SoundRef::of(own, p), p.own_line(own).and_then(|(keys, _)| keys.get(k))) {
            (Some(sound), Some(key)) => Selection::Point(sound, key.t),
            _ => Selection::None,
        }
    }

    /// Item `r` (in the lane the player shows) of `p`.
    pub fn item_at(p: &Project, r: ItemRef) -> Self {
        p.item(r).map_or(Selection::None, |it| Selection::Item(it.id))
    }

    /// Where the selected cut is in `p`.
    pub fn cut(self, p: &Project) -> Option<usize> {
        let Selection::Cut(id) = self else { return None };
        p.cuts.iter().position(|c| c.id == id)
    }

    /// The selected sound clip's track and place in `p`.
    pub fn clip(self, p: &Project) -> Option<(SoundTrack, usize)> {
        let Selection::Clip(id) = self else { return None };
        match SoundRef::Clip(id).own(p)? {
            Own::Clip(w, i) => Some((w, i)),
            Own::Cut(_) => None,
        }
    }

    /// The selected point's line and place on it in `p`.
    pub fn point(self, p: &Project) -> Option<(Own, usize)> {
        let Selection::Point(sound, t) = self else { return None };
        let own = sound.own(p)?;
        let k = p.own_line(own)?.0.iter().position(|k| (k.t - t).abs() < crate::project::SAME_POINT)?;
        Some((own, k))
    }

    /// The selected item's track and place, in the lanes the player shows.
    pub fn item(self, p: &Project) -> Option<ItemRef> {
        let Selection::Item(id) = self else { return None };
        [Track::Overlay, Track::Text].into_iter().find_map(|t| p.lane(t)?.items.iter().position(|it| it.id == id).map(|i| (t, i)))
    }

    /// The sound a volume point would go on: the selected point's line, or
    /// the selected clip's own.
    pub fn sound(self, p: &Project) -> Option<Own> {
        match self {
            Selection::Point(..) => self.point(p).map(|(own, _)| own),
            Selection::Clip(_) => self.clip(p).map(|(w, i)| Own::Clip(w, i)),
            _ => None,
        }
    }

    /// The selection after the project changed under it (an undo, a
    /// delete): kept while what it names is there, else nothing.
    pub fn still_valid(self, p: &Project) -> Self {
        let there = match self {
            Selection::None => false,
            Selection::Cut(_) => self.cut(p).is_some(),
            Selection::Clip(_) => self.clip(p).is_some(),
            Selection::Point(..) => self.point(p).is_some(),
            Selection::Item(_) => self.item(p).is_some(),
        };
        if there { self } else { Selection::None }
    }
}

impl Edit {
    /// Deletes what `s` names, as one undo step, through its op; false if
    /// it isn't there.
    pub fn delete(&mut self, s: Selection) -> bool {
        use super::ops::{Op, clips::ClipOp, cuts::CutOp, items::ItemOp, sound::SoundOp};
        if s.still_valid(self) == Selection::None {
            return false;
        }
        let op = match s {
            Selection::None => return false,
            Selection::Cut(cut) => Op::Cut(CutOp::Delete { cut }),
            Selection::Clip(clip) => Op::Clip(ClipOp::Delete { clip }),
            Selection::Point(sound, t) => Op::Sound(SoundOp::DeletePoint { sound, t }),
            Selection::Item(item) => Op::Item(ItemOp::Delete { item }),
        };
        self.apply(&op).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Cut, VolumeKey};
    use std::path::PathBuf;

    fn edit() -> Edit {
        let mut e = Edit::default();
        e.cuts = vec![Cut::new(PathBuf::from("a"), 0.0, 4.0), Cut::new(PathBuf::from("b"), 0.0, 3.0)];
        e.cuts[0].volume = vec![VolumeKey { t: 1.0, volume: -6.0 }];
        e.track_mut(SoundTrack::Music).append_part(PathBuf::from("song"), 0.0, 0.0, 5.0);
        e.lane_mut(Track::Text).items.push(crate::overlay::Item { id: 0, input: PathBuf::new(), text: Some(Default::default()), at: 0.0, start: 0.0, end: 5.0, motion: Vec::new(), effects: Vec::new() });
        e.settle_ids();
        e
    }

    fn every(e: &Edit) -> [Selection; 4] {
        [Selection::cut_at(e, 1), Selection::clip_at(e, SoundTrack::Music, 0), Selection::point_at(e, Own::Cut(0), 0), Selection::item_at(e, (Track::Text, 0))]
    }

    #[test]
    fn each_kind_deletes_as_one_undo_step() {
        for k in 0..4 {
            let mut e = edit();
            let s = every(&e)[k];
            assert!(e.delete(s), "{s:?}");
            assert_eq!(s.still_valid(&e), Selection::None, "{s:?} gone");
            assert!(e.undo() && s.still_valid(&e) == s, "{s:?} back after one undo");
        }
    }

    #[test]
    fn what_is_not_there_is_left_alone() {
        let mut e = edit();
        for s in [Selection::None, Selection::Cut(99), Selection::Clip(99), Selection::Point(SoundRef::Cut(2), 0.5), Selection::Item(99)] {
            assert!(!e.delete(s), "{s:?}");
        }
        assert!(!e.undo(), "no undo step taken");
    }

    #[test]
    fn a_selection_stays_on_its_thing_through_edits() {
        let mut e = edit();
        let (second, point) = (Selection::cut_at(&e, 1), Selection::point_at(&e, Own::Cut(0), 0));
        // The cut before deleted: the selected one is now first, still it,
        // and the point (on the cut deleted) is gone.
        e.delete(Selection::cut_at(&e, 0));
        assert_eq!((second.cut(&e), point.still_valid(&e)), (Some(0), Selection::None));
        // Moved, still it.
        e.undo();
        e.move_cut(1, 0);
        assert_eq!(second.cut(&e), Some(0));
        assert_eq!(point.point(&e), Some((Own::Cut(1), 0)), "the point follows its cut");
    }
}
