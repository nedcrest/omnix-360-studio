//! The Transitions panel's Edges tab (gap 25b, user, 2026-11-08): a look on
//! the end of the cut before the join (Out) and on the start of the cut
//! after it (In), no overlap; each half belongs to its own cut
//! (`Cut::edge_out`, `edge_in`) and goes where the cut goes.

use super::transitions_panel::{Kind, length_row, tiles};
use super::{App, Message};
use crate::model::ops::{End, Op, cuts::CutOp};
use crate::project::{Cut, Edge};
use crate::transition::{self, Look};
use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Length};

impl App {
    /// The tab's messages: a look on an edge (none: off), its length by
    /// steps, which edge the settings panel shows.
    pub(super) fn on_edges(&mut self, message: Message) {
        match message {
            Message::EdgeLook(end, look) => {
                self.last_edge = look.unwrap_or(self.last_edge);
                let Some(cut) = self.edge_cut(end) else { return };
                let seconds = edge_of(cut, end).map_or(transition::DEFAULT, |e| e.seconds);
                let cut = cut.id;
                self.edge_side = end;
                self.op(Op::Cut(CutOp::Edge { cut, end, look, seconds }));
            }
            Message::EdgeLength(end, steps) => {
                let Some((cut, edge)) = self.edge(end) else { return };
                let (cut, look, seconds) = (cut.id, Some(edge.look), edge.seconds + steps as f64 * transition::STEP);
                self.edge_side = end;
                self.op(Op::Cut(CutOp::Edge { cut, end, look, seconds }));
            }
            Message::EdgeSide(end) => self.edge_side = end,
            Message::TransitionZone(join, side) => self.open_zone(join, side),
            Message::TransitionDraw(a, b) => self.draw_zone(a, b),
            // Its inner edge dragged: one undo step with the drag
            // (`ScrubDone` closes it).
            Message::EdgeSeconds(i, end, seconds) => {
                let Some(c) = self.edit.cuts.get(i) else { return };
                let Some(look) = edge_of(c, end).map(|e| e.look) else { return };
                let cut = c.id;
                self.edit.begin();
                self.op(Op::Cut(CutOp::Edge { cut, end, look: Some(look), seconds }));
            }
            _ => {}
        }
    }

    /// The panel on the join before cut `join`, at its overlap (`side`
    /// none; the join picked) or at that edge.
    fn open_zone(&mut self, join: usize, side: Option<End>) {
        self.join = self.edit.cuts.get(join).map(|c| c.id);
        self.side = Some(super::Side::Transitions);
        match side {
            None => {
                self.transitions_tab = Kind::Overlap;
                self.selection = crate::model::selection::Selection::join_at(&self.edit, join);
            }
            Some(end) => (self.transitions_tab, self.edge_side) = (Kind::Edges, end),
        }
    }

    /// A rectangle drawn on the lane from edit time `a` to `b`: the zone
    /// it makes (`Project::drawn_zone`) with the look chosen last, or its
    /// own look kept if it had one, then opened in the panel.
    fn draw_zone(&mut self, a: f64, b: f64) {
        use crate::model::edit::Zone;
        let Some(zone) = self.edit.drawn_zone(a, b) else { return };
        self.edit.begin();
        match zone {
            Zone::Overlap { join, seconds } => {
                let c = &self.edit.cuts[join];
                let (cut, look) = (c.id, c.transition.unwrap_or(self.last_overlap));
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
                self.op(Op::Cut(CutOp::Transition { cut, look: Some(look) }));
                self.edit.commit();
                self.open_zone(join, None);
            }
            Zone::Edge { join, end, seconds } => {
                let c = &self.edit.cuts[if end == End::Out { join - 1 } else { join }];
                let (cut, look) = (c.id, edge_of(c, end).map_or(self.last_edge, |e| e.look));
                self.op(Op::Cut(CutOp::Edge { cut, end, look: Some(look), seconds }));
                self.edit.commit();
                self.open_zone(join, Some(end));
            }
        }
    }

    /// The cut whose `end` meets the panel's join: the one before it for
    /// the out, the one after for the in.
    fn edge_cut(&self, end: End) -> Option<&Cut> {
        let i = self.join_index()?;
        self.edit.cuts.get(if end == End::Out { i - 1 } else { i })
    }

    /// That cut and its look on that end, if it has one.
    pub(super) fn edge(&self, end: End) -> Option<(&Cut, &Edge)> {
        let c = self.edge_cut(end)?;
        Some((c, edge_of(c, end)?))
    }

    /// This join's out and in on every join: one undo step.
    pub(super) fn edges_on_every_join(&mut self) {
        let sides = [End::Out, End::In].map(|end| (end, self.edge(end).map(|(_, e)| e.clone())));
        let n = self.edit.cuts.len();
        self.edit.begin();
        for i in 1..n {
            for (end, edge) in &sides {
                let cut = self.edit.cuts[if *end == End::Out { i - 1 } else { i }].id;
                let (look, seconds) = edge.as_ref().map_or((None, 0.0), |e| (Some(e.look), e.seconds));
                self.op(Op::Cut(CutOp::Edge { cut, end: *end, look, seconds }));
                for key in look.iter().flat_map(|l| l.knobs()).map(|k| k.key) {
                    let value = edge.as_ref().and_then(|e| e.params.get(key).copied());
                    self.op(Op::Cut(CutOp::EdgeKnob { cut, end: *end, key: key.to_owned(), value }));
                }
            }
        }
        self.edit.commit();
    }

    /// The tab: the out on the cut before, the in on the cut after.
    pub(super) fn edges_body(&self, i: usize) -> Element<'_, Message> {
        let side = |end: End, title: String| {
            let edge = self.edge(end).map(|(_, e)| e);
            // The side the settings panel shows is lit.
            let lit = edge.is_some() && self.edge_side == end;
            let name = button(text(title).size(14)).padding([2, 6]).style(if lit { button::primary } else { button::text }).on_press(Message::EdgeSide(end));
            let off = button(text("✕").size(12)).padding([1, 6]).style(button::text).on_press_maybe(edge.map(|_| Message::EdgeLook(end, None)));
            let mut block = column![row![name, iced::widget::space().width(Length::Fill), off].align_y(Alignment::Center)].spacing(8);
            if let Some(e) = edge {
                block = block.push(length_row(format!("{:.2} s", e.seconds), move |steps| Message::EdgeLength(end, steps)));
            }
            block.push(tiles(Look::EDGES.into_iter().map(|l| (l, Message::EdgeLook(end, Some(l)))), edge.map(|e| e.look)))
        };
        column![
            side(End::Out, format!("Out: the end of cut {i}")),
            side(End::In, format!("In: the start of cut {}", i + 1)),
        ]
        .spacing(16)
        .into()
    }
}

/// A cut's look on `end`.
fn edge_of(c: &Cut, end: End) -> Option<&Edge> {
    if end == End::In { c.edge_in.as_ref() } else { c.edge_out.as_ref() }
}
