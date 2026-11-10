//! Several clips selected, and groups of them (gap 8): Shift+click adds a
//! clip to the selection (Ctrl+click puts a volume point on a sound), a
//! click on a grouped clip selects its group (Alt+click the clip alone),
//! Ctrl+G groups what's selected, Ctrl+Shift+G undoes the group.

use super::{App, Selection};
use crate::model::ops::{Op, project::ProjectOp};
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
