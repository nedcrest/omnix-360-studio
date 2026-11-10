//! The Transitions panel (user, 2026-11-08: a side panel, the picture left
//! free to watch): docked at the right like the Effects panel. It works on
//! one join, the one before a cut: its look from the library's groups
//! (`transition::Look`), its length in 0.25 s steps, the same on every
//! join, or none; and the join played: from a few frames before it to a
//! few after, then back to where it started, paused (user, 2026-11-08).
//! A click on a crossfade's X selects that join and opens it here. Its
//! Edges tab puts looks on the two cuts' ends instead (`transition_edges`).

use super::{App, Message, Side, tip};
use crate::model::ops::{End, Op, cuts::CutOp};
use crate::project::Id;
use crate::transition::{self, Group, Look};
use iced::widget::{button, column, container, row, rule, scrollable, slider, text, tooltip};
use iced::{Alignment, Element, Length, Task};

const WIDTH: f32 = 300.0;
/// The settings panel beside it.
const KNOBS_WIDTH: f32 = 220.0;
/// Playing a join: this many frames before its transition, this many
/// after (user, 2026-11-08: 10 to 20 before).
const BEFORE: f64 = 15.0;
const AFTER: f64 = 10.0;
/// The kinds, as tabs atop the panel (user, 2026-11-08; `TRANSITIONS.md`
/// «Two kinds» and stingers); one not built yet says so on hover.
const KINDS: [(&str, &str, Option<Kind>); 3] = [
    ("Overlap", "Both cuts seen at once through the join", Some(Kind::Overlap)),
    ("Edges", "A look on the end of one cut and the start of the next, no overlap", Some(Kind::Edges)),
    ("Stingers", "Soon: a short clip with its own sound over the join", None),
];

/// The panel's open tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Kind {
    #[default]
    Overlap,
    Edges,
}

/// Whose settings the side panel shows and sets: the join's look, or an
/// edge's (`App::edge_side` on the Edges tab).
#[derive(Clone, Copy)]
enum Knobs {
    Join(Id),
    Edge(Id, End),
}

impl Knobs {
    fn op(self, key: String, value: Option<f32>) -> Op {
        match self {
            Knobs::Join(cut) => Op::Cut(CutOp::TransitionKnob { cut, key, value }),
            Knobs::Edge(cut, end) => Op::Cut(CutOp::EdgeKnob { cut, end, key, value }),
        }
    }
}

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
                self.last_overlap = look.unwrap_or(self.last_overlap);
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
            Message::TransitionAll if self.transitions_tab == Kind::Edges => self.edges_on_every_join(),
            // Every join as this one, its settings too: one undo step.
            Message::TransitionAll => {
                let Some(c) = self.join_cut() else { return Task::none() };
                let (look, seconds, params) = (c.transition, c.crossfade, c.transition_params.clone());
                let cuts: Vec<Id> = self.edit.cuts.iter().skip(1).map(|c| c.id).collect();
                self.edit.begin();
                for cut in cuts {
                    self.op(Op::Cut(CutOp::Transition { cut, look }));
                    self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
                    for key in look.iter().flat_map(|l| l.knobs()).map(|k| k.key) {
                        self.op(Op::Cut(CutOp::TransitionKnob { cut, key: key.to_owned(), value: params.get(key).copied() }));
                    }
                }
                self.edit.commit();
            }
            // The join nearest the playhead, played from a few frames
            // before its transition to a few after, then paused back at the
            // start (`follow_stretch`).
            Message::TransitionPreview => {
                self.join = self.nearest_join_cut();
                return self.play_stretch(self.join_window());
            }
            Message::TransitionsTab(kind) => self.transitions_tab = kind,
            _ => {}
        }
        Task::none()
    }

    /// The settings panel's messages, for the join's look or an edge's
    /// (`Knobs`): a slider's drag one undo step (`TransitionKnobDone`), a
    /// choice done at once.
    pub(super) fn on_knobs(&mut self, message: Message) {
        let Some((of, look, set, _)) = self.knobs() else { return };
        match message {
            Message::TransitionKnob(key, value) => {
                let choice = look.knob(key).is_some_and(|k| !k.choices.is_empty());
                self.edit.begin();
                self.op(of.op(key.to_owned(), Some(value)));
                if choice {
                    self.edit.commit();
                }
            }
            Message::TransitionKnobDone => self.edit.commit(),
            Message::TransitionKnobsReset => {
                let keys: Vec<String> = set.keys().cloned().collect();
                self.edit.begin();
                for key in keys {
                    self.op(of.op(key, None));
                }
                self.edit.commit();
            }
            _ => {}
        }
    }

    /// The look the settings panel is for, its moved settings and values.
    fn knobs(&self) -> Option<(Knobs, Look, &std::collections::BTreeMap<String, f32>, transition::Spec)> {
        match self.transitions_tab {
            Kind::Overlap => {
                let c = self.join_cut()?;
                Some((Knobs::Join(c.id), c.transition?, &c.transition_params, c.transition_spec()))
            }
            Kind::Edges => {
                let (c, e) = self.edge(self.edge_side)?;
                Some((Knobs::Edge(c.id, self.edge_side), e.look, &e.params, e.spec()))
            }
        }
    }

    /// From a few frames before the join's transition and the out edge to
    /// a few after it and the in edge, edit time.
    fn join_window(&self) -> Option<(f64, f64)> {
        let i = self.join_index()?;
        let (before, c) = (&self.edit.cuts[i - 1], &self.edit.cuts[i]);
        let at = crate::project::offsets(&self.edit.cuts)[i];
        let out_end = at + c.crossfade;
        let from = (out_end - before.edge_out.as_ref().map_or(0.0, |e| e.seconds)).min(at);
        let to = (at + c.edge_in.as_ref().map_or(0.0, |e| e.seconds)).max(out_end);
        let frame = 1.0 / self.fps();
        Some(((from - BEFORE * frame).max(0.0), to + AFTER * frame))
    }

    /// The playhead moved (a click, a drag, a key, playing): the panel
    /// takes the join nearest it (user, 2026-11-08: not only by Previous
    /// and Next). Moved means in the clip: a longer transition shifts the
    /// edit's time under a playhead that stays put. Not while a stretch
    /// plays (a join's or a marked range).
    pub(super) fn follow_playhead_join(&mut self, before: Option<(usize, f64)>) {
        if self.side == Some(Side::Transitions) && self.stretch_play.is_none() && self.clip_place() != before {
            self.join = self.nearest_join_cut();
        }
    }

    /// Where the playhead is: the cut and the time in its clip.
    pub(super) fn clip_place(&self) -> Option<(usize, f64)> {
        self.clip_now().map(|(_, t)| (self.current, t))
    }

    /// The cut after the join nearest the playhead (the first has none).
    fn nearest_join_cut(&self) -> Option<Id> {
        let n = self.edit.cuts.len();
        (n > 1).then(|| self.edit.nearest_join(self.position()).clamp(1, n - 1)).and_then(|i| self.edit.cuts.get(i)).map(|c| c.id)
    }

    pub(super) fn join_index(&self) -> Option<usize> {
        self.join.and_then(|id| self.edit.cut_index(id)).filter(|&i| i > 0)
    }

    fn join_cut(&self) -> Option<&crate::project::Cut> {
        self.join_index().and_then(|i| self.edit.cuts.get(i))
    }

    pub(super) fn transitions_panel(&self) -> Element<'_, Message> {
        let mut body = column![text("Transitions").size(18), self.kind_tabs()].spacing(12);
        match self.join_index() {
            None => body = body.push(note("No join: an edit of two cuts or more has one between each.")),
            Some(i) => {
                body = body.push(self.join_header(i));
                body = body.push(match self.transitions_tab {
                    Kind::Overlap => self.overlap_body(i),
                    Kind::Edges => self.edges_body(i),
                });
                body = body.push(rule::horizontal(1)).push(self.templates_section()).push(rule::horizontal(1)).push(self.join_footer());
            }
        }
        // The film's own ends: from and to black (gap 94: they were in the
        // Effects panel, which is a clip's).
        body = body.push(rule::horizontal(1)).push(text("The film's start and end").size(15)).push(self.whole_edit());
        container(scrollable(body.padding(12))).width(WIDTH).height(Length::Fill).into()
    }

    fn kind_tabs(&self) -> Element<'_, Message> {
        let tabs = KINDS.map(|(name, about, kind)| {
            let open = kind == Some(self.transitions_tab);
            let label = text(name).size(13).center().style(if kind.is_some() { text::default } else { text::secondary });
            let tab = button(label).width(Length::Fill).padding([4, 6]).style(if open { button::primary } else { button::text });
            tip(tab.on_press_maybe(kind.filter(|_| !open).map(Message::TransitionsTab)), text(about), tooltip::Position::Bottom)
        });
        iced::widget::row(tabs).spacing(4).into()
    }

    /// Which join, and the way to the one before or after.
    fn join_header(&self, i: usize) -> Element<'_, Message> {
        let at = crate::project::offsets(&self.edit.cuts)[i];
        let n = self.edit.cuts.len();
        let step = |label: &'static str, by: i64, on: bool| button(text(label).size(13)).padding([3, 10]).style(button::secondary).on_press_maybe(on.then_some(Message::TransitionJoin(by)));
        column![
            text(format!("Join: cut {} → cut {}  ({})", i, i + 1, dailix_sheet::clock(at, 1))).size(14),
            row![step("◀ Previous", -1, i > 1), step("Next ▶", 1, i + 1 < n)].spacing(8),
        ]
        .spacing(6)
        .into()
    }

    /// The overlap's length and look.
    fn overlap_body(&self, i: usize) -> Element<'_, Message> {
        let c = &self.edit.cuts[i];
        let length = if c.crossfade > 0.0 { format!("{:.2} s", c.crossfade) } else { "a cut, no overlap".to_owned() };
        let mut body = column![length_row(length, Message::TransitionLength), rule::horizontal(1)].spacing(12);
        for g in Group::OVERLAP {
            let looks = Look::OVERLAP.iter().filter(|l| l.group() == g).map(|&l| (l, Message::TransitionLook(Some(l))));
            body = body.push(column![text(g.name()).size(14), tiles(looks, c.transition)].spacing(6));
        }
        body.push(note("Those marked «soon» play as a dissolve until they're built; the choice is kept with the edit.")).into()
    }

    fn join_footer(&self) -> Element<'_, Message> {
        let every = match self.transitions_tab {
            Kind::Overlap => "This look, its settings and length on every join of the edit (one undo)",
            Kind::Edges => "This out and in, their settings and lengths, on every join of the edit (one undo)",
        };
        let mut buttons = row![tip(button(text("On every join").size(13)).style(button::secondary).on_press(Message::TransitionAll), text(every), tooltip::Position::Top)].spacing(8);
        if self.transitions_tab == Kind::Overlap {
            buttons = buttons.push(button(text("Remove").size(13)).style(button::secondary).on_press(Message::TransitionRemove));
        }
        column![button(text("▶ Play the join")).width(Length::Fill).on_press(Message::TransitionPreview), buttons].spacing(8).into()
    }

    /// Beside the panel while the look in hand has settings of its own
    /// (user, 2026-11-08): a slider each, or a choice's buttons; «Reset»
    /// puts them all back.
    pub(super) fn knobs_panel(&self) -> Element<'_, Message> {
        let Some((_, look, set, spec)) = self.knobs().filter(|(_, l, ..)| !l.knobs().is_empty()) else {
            return iced::widget::space().into();
        };
        let mut body = column![text(look.name()).size(16)].spacing(14);
        for (k, &v) in look.knobs().iter().zip(&spec.values) {
            body = body.push(knob(k, v, Message::TransitionKnob, Message::TransitionKnobDone));
        }
        let reset = button(text("Reset").size(13)).style(button::secondary).on_press_maybe((!set.is_empty()).then_some(Message::TransitionKnobsReset));
        body = body.push(tip(reset, text("Every setting back to the look's own"), tooltip::Position::Top));
        container(scrollable(body.padding(12))).width(KNOBS_WIDTH).height(Length::Fill).into()
    }
}

fn note(s: &str) -> iced::widget::Text<'_> {
    text(s).size(13).style(text::secondary)
}

/// «Length − 0.50 s +», by the transitions' steps.
pub(super) fn length_row<'a>(length: String, by: impl Fn(i64) -> Message) -> Element<'a, Message> {
    let step = |label: &'static str, steps: i64| button(text(label).size(14)).padding([2, 10]).style(button::secondary).on_press(by(steps));
    row![text("Length").size(14).width(Length::Fill), step("−", -1), text(length).size(14), step("+", 1)].spacing(8).align_y(Alignment::Center).into()
}

/// A look's tiles, the `chosen` one lit; a look not built yet says so.
pub(super) fn tiles<'a>(looks: impl Iterator<Item = (Look, Message)>, chosen: Option<Look>) -> Element<'a, Message> {
    let tiles = looks.map(|(l, pressed)| {
        let label = if Look::code(Some(l)) == 0 { format!("{} · soon", l.name()) } else { l.name().to_owned() };
        let tile = button(text(label).size(13)).padding([6, 10]).style(if chosen == Some(l) { button::primary } else { button::secondary }).on_press(pressed);
        tip(tile, text(l.about()), tooltip::Position::Left)
    });
    tiles_of(tiles)
}

/// Tiles in rows, wrapping.
pub(super) fn tiles_of<'a>(tiles: impl Iterator<Item = Element<'a, Message>>) -> Element<'a, Message> {
    iced::widget::row(tiles).spacing(6).wrap().vertical_spacing(6).into()
}

/// One setting: its choices as buttons, or a slider with its value.
/// `set` the message a move sends, `done` the one a slider's release does.
pub(super) fn knob(k: &'static transition::Knob, v: f32, set: fn(&'static str, f32) -> Message, done: Message) -> Element<'static, Message> {
    let name = text(k.name).size(13);
    if !k.choices.is_empty() {
        let options = k.choices.iter().enumerate().map(|(i, &label)| {
            let on = v.round() as usize == i;
            button(text(label).size(12)).padding([3, 8]).style(if on { button::primary } else { button::secondary }).on_press(set(k.key, i as f32)).into()
        });
        return column![name, iced::widget::row(options).spacing(4).wrap().vertical_spacing(4)].spacing(6).into();
    }
    let shown = if k.step >= 1.0 { format!("{v:.0}") } else { format!("{v:.2}") };
    column![row![name.width(Length::Fill), text(shown).size(13)], slider(k.least..=k.most, v, move |v| set(k.key, v)).step(k.step).on_release(done)].spacing(4).into()
}
