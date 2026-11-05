//! A marker's note (gap 5): a double-click or a right-click on its flag on
//! the ruler opens a small window to name it or delete it. Enter saves,
//! Esc closes.

use super::{App, Message};
use crate::model::ops::Op;
use crate::model::ops::cuts::CutOp;
use crate::project::Id;
use iced::widget::{button, column, container, row, rule, space, text, text_input};
use iced::{Element, Length, Task};

/// The note field, focused when the window opens.
fn field() -> iced::widget::Id {
    iced::widget::Id::new("marker-note")
}

/// The marker being named: its cut, its clip time, the note as typed.
pub struct Editing {
    cut: Id,
    t: f64,
    note: String,
}

impl App {
    pub(super) fn on_marks(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MarkEdit(i, t) => {
                let Some(c) = self.edit.cuts.get(i) else { return Task::none() };
                let note = c.marks.iter().find(|m| (m.t - t).abs() < crate::model::ops::cuts::SAME_MARK).map_or_else(String::new, |m| m.note.clone());
                self.mark_edit = Some(Editing { cut: c.id, t, note });
                return iced::widget::operation::focus(field());
            }
            Message::MarkNote(note) => {
                if let Some(e) = &mut self.mark_edit {
                    e.note = note;
                }
            }
            Message::MarkSave => {
                if let Some(Editing { cut, t, note }) = self.mark_edit.take() {
                    self.op(Op::Cut(CutOp::Mark { cut, t, note: note.trim().to_owned() }));
                }
            }
            Message::MarkDelete => {
                if let Some(Editing { cut, t, .. }) = self.mark_edit.take() {
                    self.op(Op::Cut(CutOp::Unmark { cut, t }));
                }
            }
            Message::MarkClose => self.mark_edit = None,
            _ => {}
        }
        Task::none()
    }

    /// The marker's window over `below`, while one is being named.
    pub(super) fn mark_dialog<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some(e) = &self.mark_edit else { return below };
        let card = container(
            column![
                text("Marker").size(18),
                rule::horizontal(1),
                text_input("What's here", &e.note).id(field()).on_input(Message::MarkNote).on_submit(Message::MarkSave),
                row![
                    button(text("Delete")).style(button::danger).on_press(Message::MarkDelete),
                    space().width(Length::Fill),
                    button(text("Cancel")).style(button::secondary).on_press(Message::MarkClose),
                    button(text("Save")).style(button::primary).on_press(Message::MarkSave),
                ]
                .spacing(10),
            ]
            .spacing(12)
            .width(360),
        )
        .padding(18)
        .style(container::rounded_box);
        iced::widget::stack![below, super::ask::modal(card, None)].into()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::app;
    use super::*;

    /// A marker named in its window, then deleted, each one undo step.
    #[test]
    fn a_marker_is_named_and_deleted_in_its_window() {
        let mut app = app();
        let cut = app.edit.cuts[0].id;
        app.edit.apply(&crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Mark { cut, t: 1.0, note: String::new() })).unwrap();
        let _ = app.update(Message::MarkEdit(0, 1.0));
        let _ = app.update(Message::MarkNote("  the castle ".into()));
        let _ = app.update(Message::MarkSave);
        assert!(app.mark_edit.is_none());
        assert_eq!(app.edit.cuts[0].marks[0].note, "the castle");
        let _ = app.update(Message::MarkEdit(0, 1.0));
        let _ = app.update(Message::MarkDelete);
        assert!(app.edit.cuts[0].marks.is_empty());
        assert!(app.edit.undo() && app.edit.cuts[0].marks[0].note == "the castle");
    }

    /// A flag clicked selects its marker: Del then takes the marker, not
    /// the cut under the playhead (the user, 2026-11-03).
    #[test]
    fn del_after_a_flag_s_click_takes_the_marker() {
        let mut app = app();
        let cut = app.edit.cuts[0].id;
        app.edit.apply(&crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Mark { cut, t: 1.0, note: String::new() })).unwrap();
        let _ = app.update(Message::MarkSelect(0, 1.0));
        let _ = app.update(Message::DeleteCut);
        assert_eq!((app.edit.cuts.len(), app.edit.cuts[0].marks.len()), (2, 0));
        assert_eq!(app.toast.map(|t| t.0), Some("Marker deleted"));
    }
}
