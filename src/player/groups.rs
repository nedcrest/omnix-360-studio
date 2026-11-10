//! Several clips selected, and groups of them (gap 8): Shift+click adds a
//! clip to the selection (Ctrl+click puts a volume point on a sound), a
//! click selects the clip alone and Alt+click its whole group (the user,
//! 2026-11-11), Ctrl+G groups what's selected, Ctrl+Shift+G undoes it.

use super::{App, Message, Selection, tip};
use crate::model::ops::{Op, clips::ClipOp, cuts::CutOp, items::ItemOp, project::ProjectOp};
use crate::project::{GROUP_COLOURS, Group, Id};
use crate::text::Colour;
use iced::widget::{container, mouse_area, row, text, tooltip};
use iced::{Element, Task};

impl App {
    /// A clip clicked, `s` its selection: with Shift, added to (or taken
    /// from) the clips selected beside the first; else selected, and its
    /// group with it if Alt is held.
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
        let group = id.and_then(|i| self.edit.group_of(i)).filter(|_| self.modifiers.alt());
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

    /// The groups' tabs under the timeline (the user, 2026-11-11: as a
    /// spreadsheet's sheets): each its colour and name; none without
    /// groups.
    pub(super) fn group_tabs(&self) -> Option<Element<'_, Message>> {
        if self.edit.groups.is_empty() {
            return None;
        }
        let tab = |g: &Group| -> Element<'_, Message> {
            let colour = iced::Color::from_rgb8(g.colour.0[0], g.colour.0[1], g.colour.0[2]);
            let label = container(text(group_name(g)).size(12).color(iced::Color::BLACK)).padding([2, 10]).style(move |_| container::background(colour).border(iced::Border::default().rounded(3)));
            let area = mouse_area(label).on_press(Message::GroupPick(g.id)).on_double_click(Message::GroupEdit(g.id)).on_right_press(Message::GroupEdit(g.id)).interaction(iced::mouse::Interaction::Pointer);
            tip(area, text("Click: select the group and show it · double- or right-click: its name, colour, ungroup"), tooltip::Position::Top)
        };
        Some(row(self.edit.groups.iter().map(tab)).spacing(4).padding([3, 8]).into())
    }

    /// A tab's message.
    pub(super) fn on_groups(&mut self, message: Message) -> Task<Message> {
        let find = |app: &App, id: Id| app.edit.groups.iter().find(|g| g.id == id).cloned();
        match message {
            Message::GroupPick(id) => self.pick_group(id),
            Message::GroupEdit(id) => {
                let Some(g) = find(self, id) else { return Task::none() };
                return self.ask(super::ask::Ask {
                    title: group_name(&g),
                    text: "Its name, another colour, or the group undone (its clips stay).".to_owned(),
                    answers: vec![("Rename".to_owned(), Message::GroupName(id, String::new())), ("Next colour".to_owned(), Message::GroupColourNext(id)), ("Ungroup".to_owned(), Message::GroupUndo(id))],
                    cancel: Message::AskClosed,
                    field: Some(g.name.clone()),
                });
            }
            Message::GroupName(id, name) => {
                if let Some(g) = find(self, id) {
                    self.op(Op::Project(ProjectOp::GroupLook { group: id, colour: g.colour, name: Some(name) }));
                }
            }
            Message::GroupColourNext(id) => {
                if let Some(g) = find(self, id) {
                    self.op(Op::Project(ProjectOp::GroupLook { group: id, colour: next_colour(g.colour), name: None }));
                }
            }
            Message::GroupUndo(group) => {
                self.op(Op::Project(ProjectOp::Ungroup { group }));
                self.also.clear();
            }
            _ => {}
        }
        Task::none()
    }

    /// Group `id` selected, its first clip first, and the timeline taken to
    /// where it starts if that's out of sight.
    fn pick_group(&mut self, id: Id) {
        let members = self.edit.members(id);
        let Some(&first) = members.first() else { return };
        self.selection = if self.edit.cut_index(first).is_some() {
            Selection::Cut(first)
        } else if self.edit.clip_index(first).is_some() {
            Selection::Clip(first)
        } else {
            Selection::Item(first)
        };
        self.also = members[1..].to_vec();
        self.picked_with = self.selection;
        let starts = members.iter().filter_map(|&m| {
            self.edit.cut_index(m).map(|i| self.edit.offset(i)).or_else(|| self.edit.clip_index(m).map(|(w, i)| self.edit.track(w).clips[i].at)).or_else(|| self.edit.item_by_id(m).map(|it| it.at))
        });
        let start = starts.fold(f64::INFINITY, f64::min);
        let (from, span) = self.timeline_window();
        if start.is_finite() && !(from..from + span).contains(&start) {
            self.timeline_start = (start - span * 0.1).max(0.0);
        }
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

/// A group's name, «Group N» until it's given one.
fn group_name(g: &Group) -> String {
    if g.name.is_empty() { format!("Group {}", g.id) } else { g.name.clone() }
}

/// The colour after `c` in `GROUP_COLOURS` (the first after one not in it).
fn next_colour(c: Colour) -> Colour {
    let at = GROUP_COLOURS.iter().position(|&k| k == c.0).map_or(0, |i| i + 1);
    Colour(GROUP_COLOURS[at % GROUP_COLOURS.len()])
}

#[cfg(test)]
mod tests {
    use super::super::{Message, tests::app};
    use super::*;

    #[test]
    fn a_groups_tab_selects_it_and_its_dialog_renames_recolours_and_undoes_it() {
        let mut app = app();
        let (a, b) = (app.edit.cuts[0].id, app.edit.cuts[1].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![a, b], name: None, colour: None }));
        let g = app.edit.groups[0].id;
        assert_eq!(group_name(&app.edit.groups[0]), format!("Group {g}"));
        assert!(app.group_tabs().is_some());
        let _ = app.update(Message::GroupPick(g));
        assert_eq!(app.picked_ids(), [a, b]);
        // Double-click: the dialog, the name typed, «Rename».
        let _ = app.update(Message::GroupEdit(g));
        let _ = app.update(Message::AskTyped("Opening".into()));
        let _ = app.update(Message::Answer(Box::new(Message::GroupName(g, String::new()))));
        assert_eq!(app.edit.groups[0].name, "Opening");
        let was = app.edit.groups[0].colour;
        let _ = app.update(Message::GroupColourNext(g));
        assert_eq!(app.edit.groups[0].colour, next_colour(was));
        assert_ne!(app.edit.groups[0].colour, was);
        let _ = app.update(Message::GroupUndo(g));
        assert!(app.edit.groups.is_empty() && app.group_tabs().is_none());
    }

    #[test]
    fn alt_frees_a_drag_from_snapping_and_shift_no_longer_does() {
        let mut app = app();
        app.modifiers = iced::keyboard::Modifiers::SHIFT;
        assert!(app.timeline_data().snapping);
        app.modifiers = iced::keyboard::Modifiers::ALT;
        assert!(!app.timeline_data().snapping);
    }

    #[test]
    fn the_timeline_gets_the_groups_colours_and_the_clips_beside_the_first() {
        let mut app = app();
        let (a, b) = (app.edit.cuts[0].id, app.edit.cuts[1].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![a, b], name: None, colour: Some(crate::text::Colour([10, 20, 30])) }));
        app.modifiers = iced::keyboard::Modifiers::ALT;
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
        // The song Alt+clicked (its group with it) and dragged 0.5 s on.
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::SelectMusic(SoundTrack::Music, 0));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::MusicMove(SoundTrack::Music, 0, 1.5, false));
        let _ = app.update(Message::ScrubDone);
        assert_eq!((app.edit.music.clips[0].at, app.edit.text[0].items[0].at, app.edit.offset(0)), (1.5, 2.5, 0.0), "the text along, the cut where it is");
        // Del: all three, one undo step back.
        let _ = app.update(Message::DeleteCut);
        assert!(app.edit.music.clips.is_empty() && app.edit.text[0].items.is_empty() && app.edit.cut_index(cut).is_none());
        assert!(app.edit.undo() && app.edit.cut_index(cut).is_some() && app.edit.music.clips.len() == 1 && app.edit.text[0].items.len() == 1);
    }

    #[test]
    fn shift_adds_ctrl_g_groups_and_alt_click_selects_the_group() {
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
        // A click: the clip alone; Alt+click, its group with it.
        let _ = app.update(Message::Select(1));
        assert_eq!(app.picked_ids(), [b]);
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(1));
        assert_eq!(app.picked_ids(), [b, a]);
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
