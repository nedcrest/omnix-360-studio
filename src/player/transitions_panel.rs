//! The Transitions panel (user, 2026-11-08: a side panel, the picture left
//! free to watch): docked at the right like the Effects panel. It works on
//! one join, the one before a cut: its look from the library's groups
//! (`transition::Look`), its length in 0.25 s steps, the same on every
//! join, or none; and the join played: from a few frames before it to a
//! few after, then back to where it started, paused (user, 2026-11-08).
//! A click on a crossfade's X selects that join and opens it here.

use super::{App, Message, Side, tip};
use crate::model::ops::{Op, cuts::CutOp};
use crate::project::Id;
use crate::transition::{self, Group, Look};
use iced::widget::{button, column, container, row, rule, scrollable, text, tooltip};
use iced::{Alignment, Element, Length, Task};

const WIDTH: f32 = 300.0;
/// Playing a join: this many frames before its transition, this many
/// after (user, 2026-11-08: 10 to 20 before).
const BEFORE: f64 = 15.0;
const AFTER: f64 = 10.0;

impl App {
    /// The panel's messages.
    pub(super) fn on_transitions(&mut self, message: Message) -> Task<Message> {
        match message {
            // Opens on the join nearest the playhead (T, the button);
            // closes if open.
            Message::TransitionsPanel => {
                if self.side == Some(Side::Transitions) {
                    self.side = None;
                } else {
                    self.join = self.nearest_join_cut();
                    self.side = Some(Side::Transitions);
                }
            }
            // A click on a crossfade's X: that join, here.
            Message::SelectJoin(i) => {
                self.selection = crate::model::selection::Selection::join_at(&self.edit, i);
                self.join = self.edit.cuts.get(i).map(|c| c.id);
                self.side = Some(Side::Transitions);
            }
            // Another join, the playhead taken to it: the panel and the
            // playhead speak of one join.
            Message::TransitionJoin(step) => {
                let n = self.edit.cuts.len();
                if let Some(i) = self.join_index() {
                    let to = (i as i64 + step).clamp(1, n as i64 - 1) as usize;
                    self.join = self.edit.cuts.get(to).map(|c| c.id);
                    let at = crate::project::offsets(&self.edit.cuts)[to];
                    return Task::batch([self.update(Message::Scrub(at)), self.update(Message::ScrubDone)]);
                }
            }
            Message::TransitionLook(look) => {
                let Some(cut) = self.join else { return Task::none() };
                self.edit.begin();
                self.op(Op::Cut(CutOp::Transition { cut, look }));
                // A look on a join with no overlap gets one to show in.
                if look.is_some() && self.join_cut().is_some_and(|c| c.crossfade <= 0.0) {
                    self.op(Op::Cut(CutOp::Crossfade { cut, seconds: transition::DEFAULT }));
                }
                self.edit.commit();
            }
            Message::TransitionLength(steps) => {
                let Some(c) = self.join_cut() else { return Task::none() };
                let (cut, seconds) = (c.id, transition::stepped(c.crossfade + steps as f64 * transition::STEP));
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
            }
            Message::TransitionRemove => {
                let Some(cut) = self.join else { return Task::none() };
                self.edit.begin();
                self.op(Op::Cut(CutOp::Transition { cut, look: None }));
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds: 0.0 }));
                self.edit.commit();
            }
            // Every join as this one: one undo step.
            Message::TransitionAll => {
                let Some(c) = self.join_cut() else { return Task::none() };
                let (look, seconds) = (c.transition, c.crossfade);
                let cuts: Vec<Id> = self.edit.cuts.iter().skip(1).map(|c| c.id).collect();
                self.edit.begin();
                for cut in cuts {
                    self.op(Op::Cut(CutOp::Transition { cut, look }));
                    self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
                }
                self.edit.commit();
            }
            // The join nearest the playhead, played from a few frames
            // before its transition to a few after, then paused back at the
            // start (`follow_join_play`).
            Message::TransitionPreview => {
                self.join = self.nearest_join_cut();
                let Some(c) = self.join_cut() else { return Task::none() };
                let i = self.join_index().unwrap_or(1);
                let at = crate::project::offsets(&self.edit.cuts)[i];
                let frame = 1.0 / self.fps();
                let (start, stop) = ((at - BEFORE * frame).max(0.0), at + c.crossfade + AFTER * frame);
                // One jump that plays: a seek and then Play started from the
                // frame still on screen, before the seek's had decoded.
                let Some((i, t)) = self.edit.locate(start) else { return Task::none() };
                self.pause_source();
                self.follow = true;
                self.goto(i, t, true);
                self.join_play = Some((stop, start));
            }
            _ => {}
        }
        Task::none()
    }

    /// While a join plays (`TransitionPreview`): past its end, paused and
    /// back at its start; stopped another way (paused, a seek), let go.
    pub(super) fn follow_join_play(&mut self) -> Task<Message> {
        let Some((stop, start)) = self.join_play else { return Task::none() };
        if !self.playing() {
            self.join_play = None;
            return Task::none();
        }
        if self.position() < stop {
            return Task::none();
        }
        self.join_play = None;
        Task::batch([self.update(Message::Toggle), self.update(Message::Scrub(start)), self.update(Message::ScrubDone)])
    }

    /// The cut after the join nearest the playhead (the first has none).
    fn nearest_join_cut(&self) -> Option<Id> {
        let n = self.edit.cuts.len();
        (n > 1).then(|| self.edit.nearest_join(self.position()).clamp(1, n - 1)).and_then(|i| self.edit.cuts.get(i)).map(|c| c.id)
    }

    fn join_index(&self) -> Option<usize> {
        self.join.and_then(|id| self.edit.cut_index(id)).filter(|&i| i > 0)
    }

    fn join_cut(&self) -> Option<&crate::project::Cut> {
        self.join_index().and_then(|i| self.edit.cuts.get(i))
    }

    pub(super) fn transitions_panel(&self) -> Element<'_, Message> {
        let note = |s: String| text(s).size(13).style(text::secondary);
        let mut body = column![text("Transitions").size(18)].spacing(12);
        let (Some(i), Some(c)) = (self.join_index(), self.join_cut()) else {
            body = body.push(note("No join: an edit of two cuts or more has one between each.".into()));
            return container(scrollable(body.padding(12))).width(WIDTH).height(Length::Fill).into();
        };
        let at = crate::project::offsets(&self.edit.cuts)[i];
        let n = self.edit.cuts.len();
        let step = |label: &'static str, by: i64, on: bool| button(text(label).size(13)).padding([3, 10]).style(button::secondary).on_press_maybe(on.then_some(Message::TransitionJoin(by)));
        body = body.push(column![
            text(format!("Join: cut {} → cut {}  ({})", i, i + 1, dailix_sheet::clock(at, 1))).size(14),
            row![step("◀ Previous", -1, i > 1), step("Next ▶", 1, i + 1 < n)].spacing(8),
        ]
        .spacing(6));
        let length = if c.crossfade > 0.0 { format!("{:.2} s", c.crossfade) } else { "a cut, no overlap".to_owned() };
        let by = |label: &'static str, steps: i64| button(text(label).size(14)).padding([2, 10]).style(button::secondary).on_press(Message::TransitionLength(steps));
        body = body.push(row![text("Length").size(14).width(Length::Fill), by("−", -1), text(length).size(14), by("+", 1)].spacing(8).align_y(Alignment::Center));
        let chosen = c.transition;
        body = body.push(rule::horizontal(1));
        for g in Group::ALL {
            let tiles = Look::ALL.iter().filter(|l| l.group() == g).map(|&l| {
                // Not built yet: shown as a dissolve, and said on its tile.
                let label = if Look::code(Some(l)) == 0 { format!("{} · soon", l.name()) } else { l.name().to_owned() };
                let tile = button(text(label).size(13)).padding([6, 10]).style(if chosen == Some(l) { button::primary } else { button::secondary }).on_press(Message::TransitionLook(Some(l)));
                tip(tile, text(l.about()), tooltip::Position::Left)
            });
            body = body.push(column![text(g.name()).size(14), iced::widget::row(tiles).spacing(6).wrap().vertical_spacing(6)].spacing(6));
        }
        body = body.push(note("Those marked «soon» play as a dissolve until they're built; the choice is kept with the edit.".into()));
        body = body.push(rule::horizontal(1));
        body = body.push(column![
            button(text("▶ Play the join")).width(Length::Fill).on_press(Message::TransitionPreview),
            row![
                tip(button(text("On every join").size(13)).style(button::secondary).on_press(Message::TransitionAll), text("This look and length on every join of the edit (one undo)"), tooltip::Position::Top),
                button(text("Remove").size(13)).style(button::secondary).on_press(Message::TransitionRemove),
            ]
            .spacing(8),
        ]
        .spacing(8));
        container(scrollable(body.padding(12))).width(WIDTH).height(Length::Fill).into()
    }
}
