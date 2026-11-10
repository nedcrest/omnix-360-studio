//! Several clips selected, and groups of them (gap 8): Shift+click adds a
//! clip to the selection (Ctrl+click puts a volume point on a sound), a
//! click on a grouped clip selects its group (Alt+click the clip alone),
//! Ctrl+G groups what's selected, Ctrl+Shift+G undoes the group.

use super::{App, Selection};
use crate::model::ops::{Op, clips::ClipOp, cuts::CutOp, items::ItemOp, project::ProjectOp};
use crate::project::Id;

impl App {
    /// A clip clicked, `s` its selection: with Shift, added to (or taken
    /// from) the clips selected beside the first; else selected, and its
    /// group with it unless Alt is held.
    pub(super) fn pick(&mut self, s: Selection) {
        let id = s.clip_id();
        if self.modifiers.shift()
            && let (Some(id), Some(first)) = (id, self.selection.clip_id())
        {
            if id != first && !self.also.contains(&id) {
                self.also.push(id);
            } else {
                self.also.retain(|&a| a != id);
            }
            self.picked_with = self.selection;
            return;
        }
        self.selection = s;
        let group = id.and_then(|i| self.edit.group_of(i)).filter(|_| !self.modifiers.alt());
        self.also = group.map_or_else(Vec::new, |g| self.edit.members(g).into_iter().filter(|&m| Some(m) != id).collect());
        self.picked_with = self.selection;
    }

    /// The clips selected: the first, then those beside it.
    pub(super) fn picked_ids(&self) -> Vec<Id> {
        self.selection.clip_id().into_iter().chain(self.also.iter().copied()).collect()
    }

    /// The selection changed some other way (a key, Esc, a delete): the
    /// clips beside it go.
    pub(super) fn settle_picked(&mut self) {
        if self.selection != self.picked_with {
            self.also.clear();
            self.picked_with = self.selection;
        }
        let there = |id: &Id| self.edit.cut_index(*id).is_some() || self.edit.clip_index(*id).is_some() || self.edit.item_index(*id).is_some();
        let kept: Vec<Id> = self.also.iter().copied().filter(there).collect();
        self.also = kept;
    }

    /// The clip that was dragged `by` seconds took the sound clips, text
    /// and pictures selected beside it along, by as much (a group moves
    /// together; its cuts stay where the magnetic edit puts them).
    pub(super) fn move_also(&mut self, by: f64) {
        if by == 0.0 {
            return;
        }
        for id in self.also.clone() {
            let op = if let Some((w, i)) = self.edit.clip_index(id) {
                Op::Clip(ClipOp::Move { clip: id, at: self.edit.track(w).clips[i].at + by, overlap: false })
            } else if let Some(at) = self.edit.item_by_id(id).map(|it| it.at) {
                Op::Item(ItemOp::Move { item: id, at: at + by, lane: None })
            } else {
                continue;
            };
            self.op(op);
        }
    }

    /// Del with clips selected beside the first: all of them gone, one undo
    /// step. Whether there were any.
    pub(super) fn delete_picked(&mut self) -> bool {
        if self.also.is_empty() {
            return false;
        }
        let ops: Vec<Op> = self
            .picked_ids()
            .into_iter()
            .filter_map(|id| {
                if self.edit.cut_index(id).is_some() {
                    Some(Op::Cut(CutOp::Delete { cut: id }))
                } else if self.edit.clip_index(id).is_some() {
                    Some(Op::Clip(ClipOp::Delete { clip: id }))
                } else {
                    self.edit.item_by_id(id).map(|_| Op::Item(ItemOp::Delete { item: id }))
                }
            })
            .collect();
        match self.edit.batch(&ops) {
            Ok(_) => {
                self.selection = Selection::None;
                self.also.clear();
                self.save_status = Some(format!("{} clips deleted (Ctrl+Z brings them back)", ops.len()));
            }
            Err((_, e)) => self.error = Some(e.to_string()),
        }
        true
    }

    /// Ctrl+G: what's selected made a group; Ctrl+Shift+G: the selected
    /// clip's group undone.
    pub(super) fn group_selected(&mut self, undo: bool) {
        if undo {
            if let Some(group) = self.selection.clip_id().and_then(|id| self.edit.group_of(id)) {
                self.op(Op::Project(ProjectOp::Ungroup { group }));
                self.also.clear();
            }
            return;
        }
        let members = self.picked_ids();
        if members.len() >= 2 {
            self.op(Op::Project(ProjectOp::Group { members, name: None, colour: None }));
        } else {
            self.save_status = Some("Shift+click a second clip to group them".to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Message, tests::app};
    use super::*;

    #[test]
    fn the_timeline_gets_the_groups_colours_and_the_clips_beside_the_first() {
        let mut app = app();
        let (a, b) = (app.edit.cuts[0].id, app.edit.cuts[1].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![a, b], name: None, colour: Some(crate::text::Colour([10, 20, 30])) }));
        let _ = app.update(Message::Select(0));
        let t = app.timeline_data();
        assert_eq!((t.groups.len(), t.groups[0].1, t.also.as_slice()), (1, iced::Color::from_rgb8(10, 20, 30), &[b][..]));
        assert_eq!(t.blocks[1].cut.group, app.edit.group_of(b));
    }

    #[test]
    fn a_group_moves_together_and_is_deleted_together() {
        use crate::project::SoundTrack;
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 1.0, end: 9.0, ..Default::default() });
        let item = crate::overlay::Item { id: 0, input: "".into(), text: Some(Default::default()), at: 2.0, start: 0.0, end: 1.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None, group: None };
        app.edit.text.push(crate::overlay::Lane { items: vec![item], ..Default::default() });
        app.edit.settle_ids();
        let (cut, song, words) = (app.edit.cuts[0].id, app.edit.music.clips[0].id, app.edit.text[0].items[0].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![cut, song, words], name: None, colour: None }));
        // The song clicked (its group with it) and dragged 0.5 s on.
        let _ = app.update(Message::SelectMusic(SoundTrack::Music, 0));
        let _ = app.update(Message::MusicMove(SoundTrack::Music, 0, 1.5, false));
        let _ = app.update(Message::ScrubDone);
        assert_eq!((app.edit.music.clips[0].at, app.edit.text[0].items[0].at, app.edit.offset(0)), (1.5, 2.5, 0.0), "the text along, the cut where it is");
        // Del: all three, one undo step back.
        let _ = app.update(Message::DeleteCut);
        assert!(app.edit.music.clips.is_empty() && app.edit.text[0].items.is_empty() && app.edit.cut_index(cut).is_none());
        assert!(app.edit.undo() && app.edit.cut_index(cut).is_some() && app.edit.music.clips.len() == 1 && app.edit.text[0].items.len() == 1);
    }

    #[test]
    fn shift_adds_ctrl_g_groups_and_a_click_then_selects_the_group() {
        let mut app = app();
        let (a, b) = (app.edit.cuts[0].id, app.edit.cuts[1].id);
        let _ = app.update(Message::Select(0));
        app.modifiers = iced::keyboard::Modifiers::SHIFT;
        let _ = app.update(Message::Select(1));
        assert_eq!(app.picked_ids(), [a, b]);
        app.modifiers = iced::keyboard::Modifiers::empty();
        app.group_selected(false);
        let g = app.edit.group_of(a).unwrap();
        assert_eq!(app.edit.members(g), [a, b]);
        // Another click: the group comes with it; Alt, the clip alone.
        let _ = app.update(Message::Select(1));
        assert_eq!(app.picked_ids(), [b, a]);
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(1));
        assert_eq!(app.picked_ids(), [b]);
        // Esc-like: the selection let go some other way, the rest go too.
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::Select(0));
        app.selection = Selection::None;
        let _ = app.update(Message::ScrubDone);
        assert!(app.picked_ids().is_empty());
        app.selection = Selection::Cut(a);
        app.group_selected(true);
        assert!(app.edit.groups.is_empty());
    }
}
