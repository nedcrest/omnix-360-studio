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
        // A press on a clip of the selected ones keeps them all: it's
        // where a drag of them starts (the user, 2026-11-11: an
        // Alt+clicked group couldn't be dragged). Let go without a drag,
        // that clip alone stays (`pick_one`).
        // Alt too (it frees a drag from snapping): a press on a picked
        // clip never narrows the set (the audit, 2026-11-11).
        if !self.also.is_empty() && id.is_some_and(|i| self.picked_ids().contains(&i)) {
            return;
        }
        self.selection = s;
        let group = id.and_then(|i| self.edit.group_of(i)).filter(|_| self.modifiers.alt());
        self.also = group.map_or_else(Vec::new, |g| self.edit.members(g).into_iter().filter(|&m| Some(m) != id).collect());
        self.picked_with = self.selection;
    }

    /// A click on clip `id` (a cut, a sound clip, text or a picture) let go
    /// without a drag: of the clips selected, it alone stays (Alt or Shift
    /// held: the selection as it is).
    pub(super) fn pick_one(&mut self, id: Id) {
        if self.modifiers.alt() || self.modifiers.shift() || self.also.is_empty() || !self.picked_ids().contains(&id) {
            return;
        }
        self.also.clear();
        self.selection = if self.edit.cut_index(id).is_some() {
            Selection::Cut(id)
        } else if self.edit.clip_index(id).is_some() {
            Selection::Clip(id)
        } else {
            Selection::Item(id)
        };
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

    /// The sound clips, text and pictures a drag of clip `id` moves: all
    /// those selected, when `id` is one of several (the audit, 2026-11-11:
    /// they moved one at a time, the dragged one twice); none when it's
    /// alone or not selected.
    pub(super) fn moving_with(&self, id: Id) -> Option<Vec<Id>> {
        let picked = self.picked_ids();
        (picked.len() > 1 && picked.contains(&id)).then(|| picked.into_iter().filter(|&p| self.edit.cut_index(p).is_none()).collect())
    }

    /// The selected sound clips, text and pictures moved `by` together, as
    /// far as all of them fit (`ProjectOp::Shift`).
    pub(super) fn shift_picked(&mut self, by: f64) {
        let things: Vec<Id> = self.picked_ids().into_iter().filter(|&p| self.edit.cut_index(p).is_none()).collect();
        if !things.is_empty() && by != 0.0 {
            self.op(Op::Project(ProjectOp::Shift { things, by }));
        }
    }

    /// Cut `from` dragged to place `to` with other clips selected beside
    /// it (a group Alt+clicked, or Shift+clicked ones): its cuts moved
    /// there together, in their order (the user, 2026-11-11), its sound
    /// clips, text and pictures by as much as the first cut moved. Whether
    /// it was so.
    pub(super) fn move_together(&mut self, from: usize, to: usize) -> bool {
        let picked = self.picked_ids();
        let cuts: Vec<Id> = self.edit.cuts.iter().map(|c| c.id).filter(|id| picked.contains(id)).collect();
        let Some(dragged) = self.edit.cuts.get(from).map(|c| c.id) else { return false };
        if picked.len() < 2 || !cuts.contains(&dragged) {
            return false;
        }
        // Before the first cut at or after `to` that isn't moving.
        let before = self.edit.cuts[to.min(self.edit.cuts.len())..].iter().map(|c| c.id).find(|id| !cuts.contains(id));
        let playing = self.edit.cuts.get(self.current).map(|c| c.id);
        let first = cuts[0];
        let at = |e: &crate::model::edit::Edit| e.cut_index(first).map_or(0.0, |i| crate::project::offsets(&e.cuts)[i]);
        let was = at(&self.edit);
        self.edit.begin();
        if self.op(Op::Cut(CutOp::MoveTogether { cuts, before })).is_some() {
            self.current = playing.and_then(|id| self.edit.cut_index(id)).unwrap_or(0);
            // The group's sound clips, text and pictures go with its cuts.
            self.shift_picked(at(&self.edit) - was);
        }
        self.edit.commit();
        true
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
        let at = self.position();
        match self.edit.batch(&ops) {
            Ok(_) => {
                self.selection = Selection::None;
                self.also.clear();
                self.save_status = Some(format!("{} clips deleted (Ctrl+Z brings them back)", ops.len()));
                // The playing cut found again, as a single delete does (the audit, 2026-11-11).
                self.after_edit(at);
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
    fn a_groups_cuts_dragged_go_together() {
        let mut app = app();
        app.edit.cuts.push(crate::project::Cut::new("nowhere/c.mp4".into(), 0.0, 2.0));
        app.edit.settle_ids();
        let ids: Vec<Id> = app.edit.cuts.iter().map(|c| c.id).collect();
        app.op(Op::Project(ProjectOp::Group { members: vec![ids[0], ids[2]], name: None, colour: None }));
        // A plain click and a drag: the cut alone, as ever.
        let _ = app.update(Message::Select(0));
        let _ = app.update(Message::MoveCut(0, 2));
        assert_eq!(app.edit.cuts.iter().map(|c| c.id).collect::<Vec<_>>(), [ids[1], ids[0], ids[2]]);
        // Alt+click and a drag to the start: the group's two together.
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(2));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::MoveCut(2, 0));
        assert_eq!(app.edit.cuts.iter().map(|c| c.id).collect::<Vec<_>>(), [ids[0], ids[2], ids[1]]);
    }

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
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 1.0, end: 2.0, ..Default::default() });
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
        // A click (pressed and let go): the clip alone; Alt+click, its group with it.
        let _ = app.update(Message::Select(1));
        let _ = app.update(Message::SelectOne(b));
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

    #[test]
    fn a_group_dragged_past_the_films_end_stops_whole() {
        use crate::project::SoundTrack;
        let mut app = app();
        let end = app.edit.duration();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 0.0, end: 1.0, ..Default::default() });
        let item = crate::overlay::Item { id: 0, input: "".into(), text: Some(Default::default()), at: 0.5, start: 0.0, end: 1.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None, group: None };
        app.edit.text.push(crate::overlay::Lane { items: vec![item], ..Default::default() });
        app.edit.settle_ids();
        let (song, words) = (app.edit.music.clips[0].id, app.edit.text[0].items[0].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![song, words], name: None, colour: None }));
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::SelectMusic(SoundTrack::Music, 0));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::MusicMove(SoundTrack::Music, 0, end + 5.0, false));
        let (s, t) = (app.edit.music.clips[0].at, app.edit.text[0].items[0].at);
        assert!((t + 1.0 - end).abs() < 1e-9 && (t - s - 0.5).abs() < 1e-9, "the text at the end, the song still half a second before it: {s} {t} of {end}");
    }

    #[test]
    fn an_alt_clicked_group_is_pressed_again_and_dragged_whole_its_song_along() {
        use crate::project::SoundTrack;
        let mut app = app();
        app.edit.cuts.push(crate::project::Cut::new("nowhere/c.mp4".into(), 0.0, 2.0));
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 2.0, end: 4.0, ..Default::default() });
        app.edit.settle_ids();
        let ids: Vec<Id> = app.edit.cuts.iter().map(|c| c.id).collect();
        let song = app.edit.music.clips[0].id;
        app.op(Op::Project(ProjectOp::Group { members: vec![ids[1], ids[2], song], name: None, colour: None }));
        // Alt+click the group; then, as a drag starts, a plain press on one of it.
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(2));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::Select(2));
        assert_eq!(app.picked_ids().len(), 3, "the press keeps the group");
        let _ = app.update(Message::MoveCut(2, 0));
        assert_eq!(app.edit.cuts.iter().map(|c| c.id).collect::<Vec<_>>(), [ids[1], ids[2], ids[0]]);
        assert_eq!(app.edit.track(SoundTrack::Music).clips[0].at, 0.0, "the song went 2 s earlier with its cuts");
        assert!(app.edit.undo() && app.edit.cuts[0].id == ids[0] && app.edit.music.clips[0].at == 2.0, "one undo step");
        // A click let go without a drag: that cut alone.
        let _ = app.update(Message::Select(1));
        let _ = app.update(Message::SelectOne(ids[1]));
        assert_eq!(app.picked_ids(), [ids[1]]);
    }

    #[test]
    fn a_selection_dragged_by_any_of_it_moves_once_and_whole() {
        use crate::project::SoundTrack;
        let mut app = app();
        // Two songs edge to edge and a text, grouped with the first cut.
        for at in [0.0, 1.0] {
            app.edit.music.clips.push(crate::project::MusicClip { input: "s.wav".into(), at, end: 1.0, ..Default::default() });
        }
        let item = crate::overlay::Item { id: 0, input: "".into(), text: Some(Default::default()), at: 0.5, start: 0.0, end: 1.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None, group: None };
        app.edit.text.push(crate::overlay::Lane { items: vec![item], ..Default::default() });
        app.edit.settle_ids();
        let (a, b, words, cut) = (app.edit.music.clips[0].id, app.edit.music.clips[1].id, app.edit.text[0].items[0].id, app.edit.cuts[0].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![cut, a, b, words], name: None, colour: None }));
        // Alt+click song A; a plain press on song B (as a drag starts) and a drag of it 0.5 s on.
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::SelectMusic(SoundTrack::Music, 0));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::SelectMusic(SoundTrack::Music, 1));
        for want in [1.2, 1.4, 1.5] {
            let _ = app.update(Message::MusicMove(SoundTrack::Music, 1, want, false));
        }
        let _ = app.update(Message::ScrubDone);
        let at = |e: &crate::model::edit::Edit, id| e.clip_index(id).map(|(w, i)| e.track(w).clips[i].at).unwrap();
        assert_eq!((at(&app.edit, a), at(&app.edit, b), app.edit.text[0].items[0].at), (0.5, 1.5, 1.0), "each once, the first too, still edge to edge");
        assert!(app.edit.undo() && at(&app.edit, a) == 0.0 && at(&app.edit, b) == 1.0, "one undo step");
        // A click on the text alone, let go: it alone stays selected.
        let _ = app.update(Message::SelectOne(words));
        assert_eq!(app.picked_ids(), [words]);
    }

    #[test]
    fn a_beat_pulled_moves_only_its_song_unless_the_song_is_selected() {
        use crate::beats::Beat;
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 0.1, end: 2.0, beats: Some(crate::project::Beats { threshold: 0.0, snap: true }), ..Default::default() });
        let item = crate::overlay::Item { id: 0, input: "".into(), text: Some(Default::default()), at: 2.0, start: 0.0, end: 1.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None, group: None };
        app.edit.text.push(crate::overlay::Lane { items: vec![item], ..Default::default() });
        app.edit.settle_ids();
        app.hits.insert("song.wav".into(), Some(std::sync::Arc::new(vec![Beat { t: 0.013, strength: 1.0 }])));
        // A cut and the text selected; the song isn't.
        let _ = app.update(Message::Select(0));
        app.also = vec![app.edit.text[0].items[0].id];
        app.picked_with = app.selection;
        // The playhead at 0: the beat (0.113 s on the edit) pulled onto it.
        let _ = app.update(Message::PullBeat(0));
        assert!(app.edit.music.clips[0].at.abs() < 1e-9 && app.edit.text[0].items[0].at == 2.0, "the song alone");
        assert!(app.edit.undo() && (app.edit.music.clips[0].at - 0.1).abs() < 1e-9, "one step");
    }

    #[test]
    fn a_groups_one_cut_dragged_takes_its_song_along() {
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "s.wav".into(), at: 0.0, end: 1.0, ..Default::default() });
        app.edit.settle_ids();
        let (a, song) = (app.edit.cuts[0].id, app.edit.music.clips[0].id);
        app.op(Op::Project(ProjectOp::Group { members: vec![a, song], name: None, colour: None }));
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(0));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::MoveCut(0, 2));
        assert_eq!((app.edit.cuts[1].id, app.edit.music.clips[0].at), (a, 2.0), "the cut last, its song 2 s on with it");
    }

    #[test]
    fn deleting_a_selection_finds_the_playing_cut_again() {
        let mut app = app();
        app.edit.cuts.push(crate::project::Cut::new("nowhere/c.mp4".into(), 0.0, 2.0));
        app.edit.settle_ids();
        app.current = 2;
        let _ = app.update(Message::Select(0));
        app.modifiers = iced::keyboard::Modifiers::SHIFT;
        let _ = app.update(Message::Select(1));
        app.modifiers = iced::keyboard::Modifiers::empty();
        let _ = app.update(Message::DeleteCut);
        assert_eq!(app.edit.cuts.len(), 1);
        assert!(app.current < app.edit.cuts.len(), "not an index past the edit: {}", app.current);
    }

    #[test]
    fn alt_arrows_step_a_group_of_cuts_both_ways() {
        let mut app = app();
        for name in ["c", "d"] {
            app.edit.cuts.push(crate::project::Cut::new(format!("nowhere/{name}.mp4").into(), 0.0, 2.0));
        }
        app.edit.settle_ids();
        let ids: Vec<Id> = app.edit.cuts.iter().map(|c| c.id).collect();
        app.op(Op::Project(ProjectOp::Group { members: vec![ids[1], ids[2]], name: None, colour: None }));
        app.modifiers = iced::keyboard::Modifiers::ALT;
        let _ = app.update(Message::Select(1));
        let order = |app: &super::super::App| app.edit.cuts.iter().map(|c| c.id).collect::<Vec<_>>();
        let _ = app.key(iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight), iced::keyboard::Modifiers::ALT);
        assert_eq!(order(&app), [ids[0], ids[3], ids[1], ids[2]], "right, past the cut after the run");
        let _ = app.key(iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft), iced::keyboard::Modifiers::ALT);
        assert_eq!(order(&app), [ids[0], ids[1], ids[2], ids[3]], "and back");
    }
}
