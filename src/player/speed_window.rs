//! The speed window (gap 31c): one cut's speed as a curve, over the
//! timeline's part of the window while the picture above shows the frame
//! at the point picked. Across is the cut's time on the edit, as it will
//! be; up, the rate on a log scale (½× and 2× as far from 1×). Above the
//! curve a ruler in the footage's own seconds, its ticks spreading where
//! the footage is slowed; under it the edit's seconds. The rules for a
//! curve are `speed::Speed`'s (`put_key`, `move_key`, presets); OK applies
//! it as one undo step, Cancel leaves the cut as it was.

use super::{App, Message, tip};
use crate::model::ops::Op;
use crate::model::ops::cuts::CutOp;
use crate::project::Id;
use crate::speed::{Preset, RANGE, Speed};
use crate::view::Ease;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, canvas as canvas_widget, checkbox, column, container, pick_list, row, space, text, tooltip};
use iced::{Alignment, Element, Length, Point, Rectangle, Renderer, Size, Theme};

/// The rulers' height, and the curve's.
const RULER: f32 = 22.0;
const CURVE: f32 = 170.0;
/// A point's half side, and how near a press picks it.
const DOT: f32 = 4.5;
const PICK: f32 = 8.0;
/// Room at either side, so a point at the cut's ends is whole.
const PAD: f32 = 12.0;

/// The window while it's open: the cut, its curve as edited so far, the
/// key picked.
pub struct SpeedEdit {
    pub cut: Id,
    pub draft: Speed,
    pub picked: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SpeedMsg {
    Open(Id),
    /// Closed: applied (true) or let go.
    Close(bool),
    /// A key put at clip time `.0` with rate `.1`, and picked.
    Put(f64, f64),
    Pick(usize),
    /// Key `.0` dragged to clip time `.1`, rate `.2`.
    Move(usize, f64, f64),
    /// The key picked taken away; or key `.0` (a right-click on it).
    Remove,
    RemoveAt(usize),
    Ease(EaseChoice),
    Preset(Preset),
    Reset,
    Pitch(bool),
}

/// An ease as the window names it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EaseChoice(pub Ease);

impl EaseChoice {
    const ALL: [EaseChoice; 3] = [EaseChoice(Ease::Hold), EaseChoice(Ease::Linear), EaseChoice(Ease::Smooth)];
}

impl std::fmt::Display for EaseChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            Ease::Hold => "Step to the next",
            Ease::Linear => "Even ramp",
            Ease::Smooth => "Smooth ramp",
        })
    }
}

/// Where the curve maps: the cut's clip span, read at `speed` (the draft,
/// or the one a drag froze), across `width` pixels from `PAD`.
struct Map<'a> {
    speed: &'a Speed,
    span: (f64, f64),
    width: f32,
}

impl Map<'_> {
    fn length(&self) -> f64 {
        self.speed.edit_secs(self.span.0, self.span.1).max(1e-6)
    }

    /// The x of clip time `t`.
    fn x(&self, t: f64) -> f32 {
        PAD + (self.speed.edit_secs(self.span.0, t) / self.length()) as f32 * self.width
    }

    /// The x of edit second `e` (from the cut's start).
    fn x_edit(&self, e: f64) -> f32 {
        PAD + (e / self.length()) as f32 * self.width
    }

    /// The clip time at `x`, kept in the span.
    fn t(&self, x: f32) -> f64 {
        let e = ((x - PAD) / self.width).clamp(0.0, 1.0) as f64 * self.length();
        self.speed.clip_after(self.span.0, e).clamp(self.span.0, self.span.1)
    }
}

/// The curve's height of `rate`, and back.
fn y(rate: f64) -> f32 {
    let (lo, hi) = (RANGE.0.log2(), RANGE.1.log2());
    RULER + CURVE * (1.0 - ((rate.log2() - lo) / (hi - lo)) as f32)
}

fn rate(y: f32) -> f64 {
    let (lo, hi) = (RANGE.0.log2(), RANGE.1.log2());
    2f64.powf(lo + (1.0 - ((y - RULER) / CURVE).clamp(0.0, 1.0)) as f64 * (hi - lo))
}

/// The curve, its rulers and its points.
struct Curve<'a> {
    draft: &'a Speed,
    span: (f64, f64),
    picked: Option<usize>,
}

/// A drag under way: the key and the curve as it was when pressed, which
/// the mapping keeps till let go (else the key ran from the pointer as the
/// edit's length changed under it).
#[derive(Default)]
struct Drag {
    key: Option<usize>,
    frozen: Option<Speed>,
}

impl Curve<'_> {
    fn map<'b>(&'b self, drag: &'b Drag, width: f32) -> Map<'b> {
        Map { speed: drag.frozen.as_ref().unwrap_or(self.draft), span: self.span, width: (width - 2.0 * PAD).max(1.0) }
    }

    /// The key under `p`, if any.
    fn key_at(&self, map: &Map, p: Point) -> Option<usize> {
        self.draft.keys.iter().position(|k| k.t >= self.span.0 - 1e-9 && k.t <= self.span.1 + 1e-9 && (map.x(k.t) - p.x).abs() <= PICK && (y(k.rate) - p.y).abs() <= PICK)
    }
}

impl canvas::Program<Message> for Curve<'_> {
    type State = Drag;

    fn update(&self, drag: &mut Drag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        let send = |m: SpeedMsg| Some(canvas::Action::publish(Message::Speed(m)).and_capture());
        match event {
            Mouse(mouse::Event::ButtonPressed(button)) => {
                let p = cursor.position_in(bounds).filter(|p| p.y >= RULER && p.y <= RULER + CURVE)?;
                let map = self.map(drag, bounds.width);
                let key = self.key_at(&map, p);
                match (button, key) {
                    (mouse::Button::Right, Some(k)) => {
                        *drag = Drag::default();
                        send(SpeedMsg::RemoveAt(k))
                    }
                    (mouse::Button::Left, Some(k)) => {
                        *drag = Drag { key: Some(k), frozen: Some(self.draft.clone()) };
                        send(SpeedMsg::Pick(k))
                    }
                    (mouse::Button::Left, None) => send(SpeedMsg::Put(map.t(p.x), rate(p.y))),
                    _ => None,
                }
            }
            Mouse(mouse::Event::CursorMoved { .. }) => {
                let k = drag.key?;
                let p = cursor.position_in(bounds).or_else(|| cursor.position().map(|q| Point::new(q.x - bounds.x, q.y - bounds.y)))?;
                let map = self.map(drag, bounds.width);
                send(SpeedMsg::Move(k, map.t(p.x), rate(p.y)))
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if drag.key.is_some() => {
                *drag = Drag::default();
                Some(canvas::Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(&self, drag: &Drag, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let ink = palette.background.base.text;
        let map = self.map(drag, bounds.width);
        self.draw_grid(&mut frame, ink, bounds.width);
        self.draw_rulers(&mut frame, &map, ink);
        // The curve: the rate at each 2 px of the edit.
        let line = Path::new(|p| {
            let mut x = PAD;
            while x <= PAD + map.width {
                let at = Point::new(x, y(map.speed.rate_at(map.t(x))));
                if x == PAD { p.move_to(at) } else { p.line_to(at) }
                x += 2.0;
            }
        });
        frame.stroke(&line, Stroke::default().with_width(2.0).with_color(super::timeline::EFFECT_MARK));
        for (i, k) in self.draft.keys.iter().enumerate().filter(|(_, k)| k.t >= self.span.0 - 1e-9 && k.t <= self.span.1 + 1e-9) {
            let (x, yy) = (map.x(k.t), y(k.rate));
            let colour = if Some(i) == self.picked { iced::Color::WHITE } else { super::timeline::EFFECT_MARK };
            frame.fill(&Path::rectangle(Point::new(x - DOT, yy - DOT), Size::new(2.0 * DOT, 2.0 * DOT)), colour);
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, drag: &Drag, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let over_key = cursor.position_in(bounds).and_then(|p| self.key_at(&self.map(drag, bounds.width), p)).is_some();
        if drag.key.is_some() {
            mouse::Interaction::Grabbing
        } else if over_key {
            mouse::Interaction::Grab
        } else if cursor.position_in(bounds).is_some_and(|p| p.y >= RULER && p.y <= RULER + CURVE) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl Curve<'_> {
    /// The rates' lines, ¼× to 4×, 1× the strongest, named at the right.
    fn draw_grid(&self, frame: &mut Frame, ink: iced::Color, width: f32) {
        for (r, name) in [(0.25, "¼×"), (0.5, "½×"), (1.0, "1×"), (2.0, "2×"), (4.0, "4×")] {
            let yy = y(r);
            frame.fill(&Path::rectangle(Point::new(0.0, yy), Size::new(width, 1.0)), ink.scale_alpha(if r == 1.0 { 0.35 } else { 0.12 }));
            frame.fill_text(Text { content: name.to_owned(), position: Point::new(width - 4.0, yy - 12.0), color: ink.scale_alpha(0.6), size: 10.0.into(), align_x: iced::advanced::text::Alignment::Right, ..Default::default() });
        }
    }

    /// The footage's seconds above (spread where it's slowed) and the
    /// edit's below, a tick each second, named where there's room.
    fn draw_rulers(&self, frame: &mut Frame, map: &Map, ink: iced::Color) {
        let ticks = |frame: &mut Frame, xs: Vec<(f32, f64)>, top: f32| {
            let mut named = f32::NEG_INFINITY;
            for (x, s) in xs {
                frame.fill(&Path::rectangle(Point::new(x, top + RULER - 7.0), Size::new(1.0, 6.0)), ink.scale_alpha(0.6));
                if x - named > 36.0 {
                    frame.fill_text(Text { content: format!("{s:.0}s"), position: Point::new(x + 2.0, top + 2.0), color: ink.scale_alpha(0.7), size: 10.0.into(), ..Default::default() });
                    named = x;
                }
            }
        };
        let (start, end) = self.span;
        let footage: Vec<(f32, f64)> = (0..).map(|k| start.ceil() + k as f64).take_while(|&t| t <= end).map(|t| (map.x(t), t - start)).collect();
        ticks(frame, footage, 0.0);
        let length = map.length();
        let edit: Vec<(f32, f64)> = (0..).map(|k| k as f64).take_while(|&e| e <= length).map(|e| (map.x_edit(e), e)).collect();
        ticks(frame, edit, RULER + CURVE);
    }
}

impl App {
    /// The window in the timeline's place, while it's open.
    pub(super) fn speed_window(&self) -> Option<Element<'_, Message>> {
        let edit = self.speed_edit.as_ref()?;
        let cut = self.edit.cuts.iter().find(|c| c.id == edit.cut)?;
        let span = (cut.start, cut.end);
        let curve = canvas_widget(Curve { draft: &edit.draft, span, picked: edit.picked }).width(Length::Fill).height(2.0 * RULER + CURVE);
        let len = |s: &Speed| format!("{:.2} s", s.edit_secs(span.0, span.1));
        let m = |s: SpeedMsg| Message::Speed(s);
        let side = column![
            text(format!("Clip  {:.2} s", span.1 - span.0)).size(13),
            text(format!("Edit  {}", len(&edit.draft))).size(13),
            text(format!("(was  {})", len(&cut.speed))).size(12).style(text::secondary),
            space().height(8),
            button(text("Slow in the middle").size(13)).style(button::secondary).width(Length::Fill).on_press(m(SpeedMsg::Preset(Preset::SlowMiddle))),
            button(text("Ramp to the cut").size(13)).style(button::secondary).width(Length::Fill).on_press(m(SpeedMsg::Preset(Preset::RampToCut))),
        ]
        .spacing(4)
        .width(170);
        let head = row![text(format!("Speed · {}", super::clip_name(&cut.input))).size(14), space().width(Length::Fill), button(text("✕").size(14)).style(button::text).on_press(m(SpeedMsg::Close(false)))].align_y(Alignment::Center);
        let body = row![curve, side].spacing(12);
        Some(container(column![head, body, self.speed_bar(edit)].spacing(6)).padding(10).style(container::rounded_box).into())
    }

    /// The window's bottom row: the key picked (its rate, its time, how it
    /// goes on), the pitch, and Reset, Cancel and OK.
    fn speed_bar(&self, edit: &SpeedEdit) -> Element<'_, Message> {
        let m = |s: SpeedMsg| Message::Speed(s);
        let key = edit.picked.and_then(|i| edit.draft.keys.get(i));
        let picked: Element<'_, Message> = match key {
            Some(k) => row![
                text(format!("Point  {:.2}×  at {:.2} s", k.rate, k.t)).size(13),
                pick_list(&EaseChoice::ALL[..], Some(EaseChoice(k.ease)), move |e| m(SpeedMsg::Ease(e))).text_size(13),
                tip(button(text("Delete").size(13)).style(button::secondary).on_press(m(SpeedMsg::Remove)), text("Or right-click the point"), tooltip::Position::Top),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            None => text("Click the curve to put a point; drag it; right-click takes it away").size(12).style(text::secondary).into(),
        };
        row![
            picked,
            space().width(Length::Fill),
            checkbox(!edit.draft.tape).label("Keep the sound's pitch").text_size(13).on_toggle(move |keep| m(SpeedMsg::Pitch(!keep))),
            button(text("Reset to 1×").size(13)).style(button::secondary).on_press(m(SpeedMsg::Reset)),
            button(text("Cancel").size(13)).style(button::secondary).on_press(m(SpeedMsg::Close(false))),
            button(text("OK").size(13)).style(button::primary).on_press(m(SpeedMsg::Close(true))),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    /// The window's messages: the curve changed by its rules, the picture
    /// shown at the key picked; OK applies curve and pitch as one step.
    pub(super) fn on_speed(&mut self, message: SpeedMsg) -> iced::Task<Message> {
        self.speed_message(message);
        iced::Task::none()
    }

    fn speed_message(&mut self, message: SpeedMsg) {
        if let SpeedMsg::Open(id) = message {
            self.sound_menu = None;
            if let Some(c) = self.edit.cuts.iter().find(|c| c.id == id) {
                self.speed_edit = Some(SpeedEdit { cut: id, draft: c.speed.for_editing(c.start), picked: None });
            }
            return;
        }
        let Some(edit) = self.speed_edit.as_mut() else { return };
        let Some(i) = self.edit.cuts.iter().position(|c| c.id == edit.cut) else {
            self.speed_edit = None;
            return;
        };
        let span = (self.edit.cuts[i].start, self.edit.cuts[i].end);
        let mut look = None;
        match message {
            SpeedMsg::Open(_) => {}
            SpeedMsg::Close(apply) => return self.close_speed(apply),
            SpeedMsg::Put(t, r) => {
                let ease = edit.picked.and_then(|k| edit.draft.keys.get(k)).map_or(Ease::Smooth, |k| k.ease);
                edit.picked = Some(edit.draft.put_key(t, r, ease));
                look = Some(t);
            }
            SpeedMsg::Pick(k) => {
                edit.picked = Some(k);
                look = edit.draft.keys.get(k).map(|k| k.t);
            }
            SpeedMsg::Move(k, t, r) => {
                edit.draft.move_key(k, t, r);
                look = edit.draft.keys.get(k).map(|k| k.t);
            }
            SpeedMsg::Remove => {
                if let Some(k) = edit.picked.take() {
                    edit.draft.remove_key(k);
                }
            }
            SpeedMsg::RemoveAt(k) => {
                edit.draft.remove_key(k);
                edit.picked = None;
            }
            SpeedMsg::Ease(e) => {
                if let Some(k) = edit.picked.and_then(|k| edit.draft.keys.get_mut(k)) {
                    k.ease = e.0;
                }
            }
            SpeedMsg::Preset(p) => (edit.draft, edit.picked) = (Speed::preset(p, span, edit.draft.tape), None),
            SpeedMsg::Reset => (edit.draft, edit.picked) = (Speed { tape: edit.draft.tape, ..Speed::default() }.for_editing(span.0), None),
            SpeedMsg::Pitch(tape) => edit.draft.tape = tape,
        }
        // The picture above: the cut's frame at the key's footage time.
        if let Some(t) = look {
            self.goto(i, t.clamp(span.0, span.1), false);
        }
    }

    /// Closes the window, the curve and the pitch applied as one undo step
    /// (refused: said, and the window stays) or let go.
    fn close_speed(&mut self, apply: bool) {
        let Some(edit) = self.speed_edit.take() else { return };
        if !apply {
            return;
        }
        let keys = if edit.draft.is_normal() { Vec::new() } else { edit.draft.keys.clone() };
        self.edit.begin();
        let done = self.op(Op::Cut(CutOp::SpeedCurve { cut: edit.cut, keys })).is_some() && self.op(Op::Cut(CutOp::TapePitch { cut: edit.cut, on: edit.draft.tape })).is_some();
        if done {
            self.edit.commit();
            self.reload_cut_sound();
        } else {
            self.edit.cancel();
            self.speed_edit = Some(edit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::app;

    /// The rate's height and back, ¼× at the bottom, 4× at the top, 1×
    /// half way; a curve's key found where it's drawn.
    #[test]
    fn the_curve_maps_rates_and_times_both_ways() {
        assert!((y(1.0) - (RULER + CURVE / 2.0)).abs() < 1e-4 && (y(RANGE.1) - RULER).abs() < 1e-4);
        for r in [0.25, 0.5, 1.0, 1.7, 4.0] {
            assert!((rate(y(r)) - r).abs() < 1e-4, "{r}");
        }
        let slow = Speed::constant(0.5);
        let map = Map { speed: &slow, span: (10.0, 14.0), width: 800.0 };
        assert!((map.x(12.0) - (PAD + 400.0)).abs() < 1e-3 && (map.t(PAD + 400.0) - 12.0).abs() < 1e-9);
        assert!((map.x_edit(4.0) - (PAD + 400.0)).abs() < 1e-3, "8 edit seconds across");
    }

    /// Opened on a cut, a point put and dragged, a preset, Cancel leaves
    /// the cut; OK applies curve and pitch as one undo step.
    #[test]
    fn the_window_applies_its_curve_as_one_step() {
        let mut app = app();
        let id = app.edit.cuts[0].id;
        let speed = |m| Message::Speed(m);
        let _ = app.update(speed(SpeedMsg::Open(id)));
        let _ = app.update(speed(SpeedMsg::Put(1.0, 2.0)));
        assert_eq!(app.speed_edit.as_ref().unwrap().picked, Some(1));
        let _ = app.update(speed(SpeedMsg::Close(false)));
        assert!(app.speed_edit.is_none() && app.edit.cuts[0].speed.is_default(), "Cancel leaves it");
        let _ = app.update(speed(SpeedMsg::Open(id)));
        let _ = app.update(speed(SpeedMsg::Preset(Preset::SlowMiddle)));
        let _ = app.update(speed(SpeedMsg::Pitch(true)));
        let _ = app.update(speed(SpeedMsg::Close(true)));
        let c = &app.edit.cuts[0];
        assert!(app.speed_edit.is_none() && c.speed.keys.len() == 4 && c.speed.tape && c.edit_len() > 2.0);
        assert!(app.edit.undo() && app.edit.cuts[0].speed.is_default(), "one step");
    }
}
