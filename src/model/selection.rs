//! What's selected on the timeline: one thing at a time (`CORE-API.md` step
//! 5). It was five loose `Option`s, each «select this» clearing the others
//! by hand, and some forgot one: a picked overlay left the cut it was over
//! selected too.

use super::edit::Edit;
use crate::overlay::ItemRef;
use crate::project::{Own, SoundTrack};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Selection {
    #[default]
    None,
    Cut(usize),
    /// A clip on a sound track.
    Clip(SoundTrack, usize),
    /// Point `.1` on a cut's or a sound clip's own volume line.
    Point(Own, usize),
    /// An overlay or text item, in the lane of its track the player shows.
    Item(ItemRef),
}

impl Selection {
    pub fn cut(self) -> Option<usize> {
        if let Selection::Cut(i) = self { Some(i) } else { None }
    }

    pub fn clip(self) -> Option<(SoundTrack, usize)> {
        if let Selection::Clip(w, i) = self { Some((w, i)) } else { None }
    }

    pub fn point(self) -> Option<(Own, usize)> {
        if let Selection::Point(o, i) = self { Some((o, i)) } else { None }
    }

    pub fn item(self) -> Option<ItemRef> {
        if let Selection::Item(r) = self { Some(r) } else { None }
    }

    /// The sound a volume point would go on: the selected point's line, or
    /// the selected clip's own.
    pub fn sound(self) -> Option<Own> {
        match self {
            Selection::Point(o, _) => Some(o),
            Selection::Clip(w, i) => Some(Own::Clip(w, i)),
            _ => None,
        }
    }

    /// Whether what it names is in `edit`.
    fn there(self, edit: &Edit) -> bool {
        match self {
            Selection::None => false,
            Selection::Cut(i) => i < edit.cuts.len(),
            Selection::Clip(w, i) => i < edit.track(w).clips.len(),
            Selection::Point(own, i) => edit.own_line(own).is_some_and(|(keys, _)| i < keys.len()),
            Selection::Item(r) => edit.item(r).is_some(),
        }
    }

    /// The selection after `edit` changed under it (an undo, a delete): kept
    /// if what it names is still there, else nothing. A point on a cut's line
    /// goes in any case: the cuts may have moved, and its index would name
    /// another cut's.
    pub fn still_valid(self, edit: &Edit) -> Self {
        if matches!(self, Selection::Point(Own::Cut(_), _)) || !self.there(edit) { Selection::None } else { self }
    }
}

impl Edit {
    /// Deletes what `s` names, as one undo step; false if it isn't there.
    pub fn delete(&mut self, s: Selection) -> bool {
        if !s.there(self) {
            return false;
        }
        self.checkpoint();
        match s {
            Selection::None => {}
            Selection::Cut(i) => self.remove(i),
            Selection::Clip(w, i) => {
                self.track_mut(w).clips.remove(i);
            }
            Selection::Point(own, i) => {
                if let Some((keys, _)) = self.own_line_mut(own) {
                    keys.remove(i);
                }
            }
            Selection::Item((track, i)) => {
                self.lane_mut(track).items.remove(i);
            }
        }
        true
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
        e.track_mut(SoundTrack::Music).append(PathBuf::from("song"), 0.0, 5.0);
        e.lane_mut(crate::overlay::Track::Text).items.push(crate::overlay::Item { id: 0, input: PathBuf::new(), text: Some(Default::default()), at: 0.0, start: 0.0, end: 5.0, motion: Vec::new(), effects: Vec::new() });
        e
    }

    #[test]
    fn each_kind_deletes_as_one_undo_step() {
        for s in [Selection::Cut(1), Selection::Clip(SoundTrack::Music, 0), Selection::Point(Own::Cut(0), 0), Selection::Item((crate::overlay::Track::Text, 0))] {
            let mut e = edit();
            assert!(e.delete(s), "{s:?}");
            assert_eq!(s.still_valid(&e), Selection::None, "{s:?} gone");
            assert!(e.undo() && s.there(&e), "{s:?} back after one undo");
        }
    }

    #[test]
    fn what_is_not_there_is_left_alone() {
        let mut e = edit();
        for s in [Selection::None, Selection::Cut(2), Selection::Clip(SoundTrack::Voice, 0), Selection::Point(Own::Cut(1), 0), Selection::Item((crate::overlay::Track::Overlay, 0))] {
            assert!(!e.delete(s), "{s:?}");
        }
        assert!(!e.undo(), "no undo step taken");
    }

    #[test]
    fn a_selection_outlives_an_edit_only_if_still_there() {
        let mut e = edit();
        assert_eq!(Selection::Cut(1).still_valid(&e), Selection::Cut(1));
        assert_eq!(Selection::Point(Own::Cut(0), 0).still_valid(&e), Selection::None, "a cut's point may now name another cut's");
        e.delete(Selection::Cut(0));
        assert_eq!(Selection::Cut(1).still_valid(&e), Selection::None);
        assert_eq!(Selection::Clip(SoundTrack::Music, 0).still_valid(&e), Selection::Clip(SoundTrack::Music, 0));
    }
}
