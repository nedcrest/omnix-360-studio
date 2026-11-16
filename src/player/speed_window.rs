//! The speed window (gap 31c): the speed of a cut, or of several cuts
//! selected side by side, as one curve, over the timeline's part of the
//! window while the picture above shows the frame at the point picked.
//! Across is the footage's own time, the cuts one after another (a point
//! stays where it's put: across in edit time, the curve moved under the
//! pointer as it shortened the edit; the user, 2026-11-16); up, the rate
//! on a log scale (½× and 2× as far from 1×). Above, each cut's footage
//! seconds; below, the edit's, their ticks spreading where it's slowed.
//! The rules for a curve are `speed::Speed`'s (`put_key`, `move_key`,
//! presets); «Done» applies it to every cut as one undo step, Cancel
//! leaves them as they were.

use super::{App, Message, tip};
use crate::model::ops::Op;
use crate::model::ops::cuts::CutOp;
use crate::project::{Cut, Id};
use crate::speed::{Preset, RANGE, Speed, SpeedKey};
use crate::view::Ease;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Text};
use iced::widget::{button, canvas as canvas_widget, checkbox, column, container, pick_list, row, space, text, tooltip};
use iced::{Alignment, Element, Length, Point, Rectangle, Renderer, Size, Theme};

/// The rulers' height, the curve's, and the playhead's strip under them.
const RULER: f32 = 22.0;
const CURVE: f32 = 170.0;
const STRIP: f32 = 26.0;
/// Where the strip starts, and the whole height.
const STRIP_TOP: f32 = 2.0 * RULER + CURVE + 6.0;
const HEIGHT: f32 = STRIP_TOP + STRIP;
/// A point's half side, and how near a press picks it.
const DOT: f32 = 4.5;
const PICK: f32 = 8.0;
/// Room at either side, so a point at the ends is whole.
const PAD: f32 = 12.0;

/// The window while it's open: the cuts (side by side), their clip spans
/// when it opened, their curve as one over their footage laid end to end
/// (`joined`), the key picked.
pub struct SpeedEdit {
    pub cuts: Vec<Id>,
    pub spans: Vec<(f64, f64)>,
    pub draft: Speed,
    pub picked: Option<usize>,
    /// The window's own playhead, joined footage time: the picture above
    /// shows its frame (the user, 2026-11-16).
    pub head: f64,
}

#[derive(Debug, Clone)]
pub enum SpeedMsg {
    /// Opened from cut `.0`'s menu: with the cuts selected beside it.
    Open(Id),
    /// Closed: applied (true) or let go.
    Close(bool),
    /// A key put at footage time `.0` with rate `.1`, and picked.
    Put(f64, f64),
    Pick(usize),
    /// Key `.0` dragged to footage time `.1`, rate `.2`.
    Move(usize, f64, f64),
    /// The window's playhead to footage time `.0` (its strip or a ruler
    /// pressed or dragged, Home, End), or on by `.0` edit seconds (← →).
    Head(f64),
    Step(f64),
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

/// The cuts one curve edits, opened from cut `clicked`: the run of cuts
/// selected side by side it's in, else itself (indices in `cuts`).
pub(super) fn cuts_for_curve(cuts: &[Cut], picked: &[Id], clicked: Id) -> Vec<usize> {
    let Some(at) = cuts.iter().position(|c| c.id == clicked) else { return Vec::new() };
    let mut run: Vec<usize> = cuts.iter().enumerate().filter(|(_, c)| picked.contains(&c.id)).map(|(i, _)| i).collect();
    if !run.contains(&at) || run.windows(2).any(|w| w[1] != w[0] + 1) {
        run = vec![at];
    }
    run
}

/// Where each cut's footage starts in the joined time, and the end.
fn offsets(spans: &[(f64, f64)]) -> Vec<f64> {
    let mut at = vec![0.0];
    for (a, b) in spans {
        at.push(at.last().copied().unwrap_or(0.0) + (b - a));
    }
    at
}

/// The cuts' curves as one over their footage laid end to end: each
/// one's keys in its span; where it starts, the key that carries the
/// curve into it when the cut before has it too (`split` gives both cuts
/// a ramp over their join), else a key there at the rate it starts at; the
/// first and the last keep a ramp running in or out past them. `split`
/// undoes it exactly, so Done with nothing changed changes nothing.
pub(super) fn joined(speeds: &[&Speed], spans: &[(f64, f64)]) -> Speed {
    let at = offsets(spans);
    let last = spans.len().saturating_sub(1);
    let same = |a: &SpeedKey, b: &SpeedKey| (a.t - b.t).abs() < 1e-9 && a.rate == b.rate && a.ease == b.ease;
    let mut keys: Vec<SpeedKey> = Vec::new();
    for (k, (s, &(start, end))) in speeds.iter().zip(spans).enumerate() {
        let o = at[k];
        let v = |x: &SpeedKey| SpeedKey { t: o + x.t - start, ..*x };
        let inside: Vec<SpeedKey> = s.keys.iter().filter(|x| x.t >= start && (x.t < end || (k == last && x.t <= end))).map(v).collect();
        let entering = s.keys.iter().rposition(|x| x.t < start).map(|i| v(&s.keys[i]));
        let at_start = inside.first().is_some_and(|x| (x.t - o).abs() < 1e-9);
        match entering {
            Some(e) if keys.iter().any(|x| same(x, &e)) => {}
            Some(e) if k == 0 && e.ease != Ease::Hold => keys.push(e),
            Some(e) if !at_start => keys.push(SpeedKey { t: o, rate: s.rate_at(start), ease: e.ease }),
            None if !at_start => keys.push(SpeedKey::held(o, s.rate_at(start))),
            _ => {}
        }
        keys.extend(inside);
        // The last cut's ramp running on past its end keeps its far key.
        if k == last && let Some(i) = s.keys.iter().position(|x| x.t > end) && i > 0 && s.keys[i - 1].ease != Ease::Hold {
            keys.push(v(&s.keys[i]));
        }
    }
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    keys.dedup_by(|a, b| (a.t - b.t).abs() < 1e-9);
    Speed { keys, tape: speeds.iter().any(|s| s.tape) }
}

/// The joined curve `draft` back into each cut's (`joined` undone): the
/// keys from the last at or before where it starts to the first at or
/// after where it ends, in its clip time (a key may lie outside its span),
/// so its stretch of the curve is the same curve, a ramp over a join
/// included (cut in two ramps, their shapes changed and the edit came out
/// 0.16 s longer than the window said). Normal where its stretch is all
/// 1×.
pub(super) fn split(draft: &Speed, spans: &[(f64, f64)]) -> Vec<Speed> {
    let at = offsets(spans);
    spans
        .iter()
        .zip(at.windows(2))
        .map(|(&(start, _), w)| {
            let (o, end) = (w[0], w[1]);
            let first = draft.keys.iter().rposition(|k| k.t <= o).unwrap_or(0);
            let last = draft.keys.iter().position(|k| k.t >= end).unwrap_or(draft.keys.len().saturating_sub(1));
            let keys: Vec<SpeedKey> = draft.keys.get(first..=last).unwrap_or(&[]).iter().map(|k| SpeedKey { t: start + k.t - o, ..*k }).collect();
            let s = Speed { keys, tape: draft.tape };
            let all_one = (s.edit_secs(start, start + end - o) - (end - o)).abs() < 1e-12 && draft.rate_at(o) == 1.0 && draft.keys.iter().filter(|k| k.t >= o && k.t <= end).all(|k| k.rate == 1.0);
            if all_one { Speed { tape: draft.tape, ..Speed::default() } } else { s }
        })
        .collect()
}

/// Where the curve maps: the joined footage `0..length` across `width`
/// pixels from `PAD`.
struct Map {
    length: f64,
    width: f32,
}

impl Map {
    fn x(&self, t: f64) -> f32 {
        PAD + (t / self.length.max(1e-6)) as f32 * self.width
    }

    /// The footage time at `x`, kept in it.
    fn t(&self, x: f32) -> f64 {
        ((x - PAD) / self.width).clamp(0.0, 1.0) as f64 * self.length
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

/// The curve, its rulers, the joins, the points and the playhead.
struct Curve<'a> {
    draft: &'a Speed,
    spans: &'a [(f64, f64)],
    picked: Option<usize>,
    head: f64,
}

/// What a press holds: a key, or the playhead (a ruler pressed).
#[derive(Default, Clone, Copy, PartialEq)]
enum Hold {
    #[default]
    Nothing,
    Key(usize),
    Head,
}

impl Curve<'_> {
    fn map(&self, width: f32) -> Map {
        Map { length: offsets(self.spans).last().copied().unwrap_or(0.0), width: (width - 2.0 * PAD).max(1.0) }
    }

    /// The key under `p`, if any.
    fn key_at(&self, map: &Map, p: Point) -> Option<usize> {
        self.draft.keys.iter().position(|k| (map.x(k.t) - p.x).abs() <= PICK && (y(k.rate) - p.y).abs() <= PICK)
    }
}

impl canvas::Program<Message> for Curve<'_> {
    type State = Hold;

    fn update(&self, hold: &mut Hold, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        let send = |m: SpeedMsg| Some(canvas::Action::publish(Message::Speed(m)).and_capture());
        let map = self.map(bounds.width);
        match event {
            Mouse(mouse::Event::ButtonPressed(button)) => {
                let p = cursor.position_in(bounds)?;
                // On a ruler or the playhead's strip: the playhead, dragged
                // from there.
                if !(RULER..=RULER + CURVE).contains(&p.y) {
                    (*button == mouse::Button::Left).then_some(())?;
                    *hold = Hold::Head;
                    return send(SpeedMsg::Head(map.t(p.x)));
                }
                match (button, self.key_at(&map, p)) {
                    (mouse::Button::Right, Some(k)) => send(SpeedMsg::RemoveAt(k)),
                    (mouse::Button::Left, Some(k)) => {
                        *hold = Hold::Key(k);
                        send(SpeedMsg::Pick(k))
                    }
                    // A new point, held by the pointer till let go.
                    (mouse::Button::Left, None) => {
                        let t = map.t(p.x);
                        *hold = Hold::Key(self.draft.keys.iter().position(|k| k.t > t).unwrap_or(self.draft.keys.len()));
                        send(SpeedMsg::Put(t, rate(p.y)))
                    }
                    _ => None,
                }
            }
            Mouse(mouse::Event::CursorMoved { position }) => {
                let (x, y) = (position.x - bounds.x, position.y - bounds.y);
                match *hold {
                    Hold::Key(k) => send(SpeedMsg::Move(k, map.t(x), rate(y))),
                    Hold::Head => send(SpeedMsg::Head(map.t(x))),
                    Hold::Nothing => None,
                }
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if *hold != Hold::Nothing => {
                *hold = Hold::Nothing;
                Some(canvas::Action::request_redraw().and_capture())
            }
            _ => None,
        }
    }

    fn draw(&self, _hold: &Hold, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let ink = theme.extended_palette().background.base.text;
        let map = self.map(bounds.width);
        self.draw_grid(&mut frame, ink, bounds.width);
        self.draw_rulers(&mut frame, &map, ink);
        let mark = super::timeline::EFFECT_MARK;
        let line = Path::new(|p| {
            let mut x = PAD;
            while x <= PAD + map.width {
                let at = Point::new(x, y(self.draft.rate_at(map.t(x))));
                if x == PAD { p.move_to(at) } else { p.line_to(at) }
                x += 2.0;
            }
        });
        frame.stroke(&line, canvas::Stroke::default().with_width(2.0).with_color(mark));
        for (i, k) in self.draft.keys.iter().enumerate() {
            let colour = if Some(i) == self.picked { iced::Color::WHITE } else { mark };
            frame.fill(&Path::rectangle(Point::new(map.x(k.t) - DOT, y(k.rate) - DOT), Size::new(2.0 * DOT, 2.0 * DOT)), colour);
        }
        // The window's playhead: a line through it all, a handle on each ruler.
        let (x, accent) = (map.x(self.head), theme.extended_palette().primary.strong.color);
        self.draw_strip(&mut frame, &map, ink, accent);
        frame.fill(&Path::rectangle(Point::new(x - 1.0, 0.0), Size::new(2.0, HEIGHT)), accent);
        for top in [0.0, RULER + CURVE + RULER - 8.0] {
            frame.fill(&Path::rounded_rectangle(Point::new(x - 6.0, top), Size::new(12.0, 8.0), 2.0.into()), accent);
        }
        frame.fill(&Path::rounded_rectangle(Point::new(x - 7.0, STRIP_TOP + 2.0), Size::new(14.0, STRIP - 4.0), 3.0.into()), accent);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, hold: &Hold, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let p = cursor.position_in(bounds);
        if *hold != Hold::Nothing {
            mouse::Interaction::Grabbing
        } else if p.is_some_and(|p| !(RULER..=RULER + CURVE).contains(&p.y)) {
            mouse::Interaction::ResizingHorizontally
        } else if p.is_some_and(|p| self.key_at(&self.map(bounds.width), p).is_some()) {
            mouse::Interaction::Grab
        } else if p.is_some_and(|p| p.y >= RULER && p.y <= RULER + CURVE) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl Curve<'_> {
    /// The playhead's strip (the user, 2026-11-16: the rulers were too
    /// thin to take hold of): each cut a shade, what's before the playhead
    /// filled, its edit time written at its right.
    fn draw_strip(&self, frame: &mut Frame, map: &Map, ink: iced::Color, accent: iced::Color) {
        let at = offsets(self.spans);
        for (k, w) in at.windows(2).enumerate() {
            let (l, r) = (map.x(w[0]), map.x(w[1]));
            frame.fill(&Path::rounded_rectangle(Point::new(l, STRIP_TOP), Size::new((r - l - 2.0).max(1.0), STRIP), 4.0.into()), ink.scale_alpha(if k % 2 == 0 { 0.12 } else { 0.2 }));
        }
        let x = map.x(self.head);
        frame.fill(&Path::rounded_rectangle(Point::new(PAD, STRIP_TOP + STRIP / 2.0 - 2.0), Size::new((x - PAD).max(0.0), 4.0), 2.0.into()), accent.scale_alpha(0.6));
        let label = format!("{:.2} s", self.draft.edit_secs(0.0, self.head));
        let right = x + 12.0 + 60.0 < map.x(map.length);
        let (lx, align) = if right { (x + 12.0, iced::advanced::text::Alignment::Left) } else { (x - 12.0, iced::advanced::text::Alignment::Right) };
        frame.fill_text(Text { content: label, position: Point::new(lx, STRIP_TOP + STRIP / 2.0), color: ink, size: 11.0.into(), align_x: align, align_y: iced::alignment::Vertical::Center, ..Default::default() });
    }

    /// The rates' lines, ¼× to 4×, 1× the strongest, named at the right.
    fn draw_grid(&self, frame: &mut Frame, ink: iced::Color, width: f32) {
        for (r, name) in [(0.25, "¼×"), (0.5, "½×"), (1.0, "1×"), (2.0, "2×"), (4.0, "4×")] {
            let yy = y(r);
            frame.fill(&Path::rectangle(Point::new(0.0, yy), Size::new(width, 1.0)), ink.scale_alpha(if r == 1.0 { 0.35 } else { 0.12 }));
            frame.fill_text(Text { content: name.to_owned(), position: Point::new(width - 4.0, yy - 12.0), color: ink.scale_alpha(0.6), size: 10.0.into(), align_x: iced::advanced::text::Alignment::Right, ..Default::default() });
        }
    }

    /// Each cut's footage seconds above, a line at each join; the edit's
    /// seconds below, their ticks spreading where it's slowed. A tick a
    /// second, named where there's room.
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
        let at = offsets(self.spans);
        let mut footage = Vec::new();
        for (&(start, end), &o) in self.spans.iter().zip(&at) {
            footage.extend((0..).map(|k| start.ceil() + k as f64).take_while(|&t| t <= end).map(|t| (map.x(o + t - start), t)));
            if o > 0.0 {
                frame.fill(&Path::rectangle(Point::new(map.x(o), 0.0), Size::new(1.5, 2.0 * RULER + CURVE)), ink.scale_alpha(0.45));
            }
        }
        ticks(frame, footage, 0.0);
        let length = self.draft.edit_secs(0.0, map.length);
        let edit: Vec<(f32, f64)> = (0..).map(|k| k as f64).take_while(|&e| e <= length).map(|e| (map.x(self.draft.clip_after(0.0, e)), e)).collect();
        ticks(frame, edit, RULER + CURVE);
    }
}

impl App {
    /// The window in the timeline's place, while it's open.
    pub(super) fn speed_window(&self) -> Option<Element<'_, Message>> {
        let edit = self.speed_edit.as_ref()?;
        let cuts: Vec<&Cut> = edit.cuts.iter().filter_map(|id| self.edit.cuts.iter().find(|c| c.id == *id)).collect();
        let length = offsets(&edit.spans).last().copied().unwrap_or(0.0);
        let curve = canvas_widget(Curve { draft: &edit.draft, spans: &edit.spans, picked: edit.picked, head: edit.head }).width(Length::Fill).height(HEIGHT);
        let was: f64 = cuts.iter().map(|c| c.edit_len()).sum();
        let m = |s: SpeedMsg| Message::Speed(s);
        let side = column![
            text(format!("Footage  {length:.2} s")).size(13),
            text(format!("Edit  {:.2} s", edit.draft.edit_secs(0.0, length))).size(13),
            text(format!("(was  {was:.2} s)")).size(12).style(text::secondary),
            tip(text(format!("Playhead  {:.2} s · {:.2} s", edit.head, edit.draft.edit_secs(0.0, edit.head))).size(12), text("The window's playhead: footage seconds · edit seconds. Drag it on a ruler"), tooltip::Position::Left),
            space().height(8),
            button(text("Slow in the middle").size(13)).style(button::secondary).width(Length::Fill).on_press(m(SpeedMsg::Preset(Preset::SlowMiddle))),
            button(text("Ramp to the cut").size(13)).style(button::secondary).width(Length::Fill).on_press(m(SpeedMsg::Preset(Preset::RampToCut))),
        ]
        .spacing(4)
        .width(170);
        let names: Vec<String> = cuts.iter().map(|c| super::clip_name(&c.input)).collect();
        let what = if names.len() == 1 { format!("Speed · {}", names[0]) } else { format!("Speed · {} cuts: {}", names.len(), names.join(", ")) };
        // Out of the window, plainly: the user, 2026-11-16.
        let done = tip(
            button(text("✓  Done: back to the timeline").size(15)).padding([8, 16]).style(button::success).on_press(m(SpeedMsg::Close(true))),
            text("Applies the curve (Ctrl+Z takes it back); Cancel or Esc leaves the cuts as they were"),
            tooltip::Position::Left,
        );
        let head = row![text(what).size(15), space().width(Length::Fill), done].align_y(Alignment::Center);
        let body = row![curve, side].spacing(12);
        Some(container(column![head, body, self.speed_bar(edit)].spacing(6)).padding(10).style(container::rounded_box).into())
    }

    /// The window's bottom row: the key picked (its rate, its time, how it
    /// goes on), the pitch, Reset and Cancel.
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
            None => text("Click the curve to put a point, and drag it; right-click takes it away").size(12).style(text::secondary).into(),
        };
        row![
            picked,
            space().width(Length::Fill),
            checkbox(!edit.draft.tape).label("Keep the sound's pitch").text_size(13).on_toggle(move |keep| m(SpeedMsg::Pitch(!keep))),
            button(text("Reset to 1×").size(13)).style(button::secondary).on_press(m(SpeedMsg::Reset)),
            button(text("Cancel").size(13)).style(button::secondary).on_press(m(SpeedMsg::Close(false))),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    pub(super) fn on_speed(&mut self, message: SpeedMsg) -> iced::Task<Message> {
        self.speed_message(message);
        iced::Task::none()
    }

    /// The window's messages: the curve changed by its rules, the picture
    /// shown at the key picked; Done applies the curve to the cuts.
    fn speed_message(&mut self, message: SpeedMsg) {
        if let SpeedMsg::Open(id) = message {
            return self.open_speed(id);
        }
        let Some(edit) = self.speed_edit.as_mut() else { return };
        let length = offsets(&edit.spans).last().copied().unwrap_or(0.0);
        let mut look = None;
        match message {
            SpeedMsg::Open(_) => {}
            SpeedMsg::Close(apply) => return self.close_speed(apply),
            SpeedMsg::Head(t) => look = Some(t.clamp(0.0, length)),
            // On by edit seconds: a frame of the edit, at its speed there.
            SpeedMsg::Step(secs) => look = Some(edit.draft.clip_after(edit.head, secs).clamp(0.0, length)),
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
                edit.draft.move_key(k, t.clamp(0.0, length), r);
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
            SpeedMsg::Preset(p) => (edit.draft, edit.picked) = (Speed::preset(p, (0.0, length), edit.draft.tape), None),
            SpeedMsg::Reset => (edit.draft, edit.picked) = (Speed { tape: edit.draft.tape, ..Speed::default() }.for_editing(0.0), None),
            SpeedMsg::Pitch(tape) => edit.draft.tape = tape,
        }
        if let Some(t) = look {
            edit.head = t;
            self.show_footage(t);
        }
    }

    /// Opens the window on cut `id` and the cuts selected side by side
    /// with it (`cuts_for_curve`).
    fn open_speed(&mut self, id: Id) {
        self.sound_menu = None;
        let run = cuts_for_curve(&self.edit.cuts, &self.picked_ids(), id);
        let cuts: Vec<&Cut> = run.iter().map(|&i| &self.edit.cuts[i]).collect();
        if cuts.is_empty() {
            return;
        }
        let spans: Vec<(f64, f64)> = cuts.iter().map(|c| (c.start, c.end)).collect();
        let draft = joined(&cuts.iter().map(|c| &c.speed).collect::<Vec<_>>(), &spans);
        self.speed_edit = Some(SpeedEdit { cuts: cuts.iter().map(|c| c.id).collect(), spans, draft, picked: None, head: 0.0 });
        // The picture on the cuts edited, from their start (it stayed where
        // the playhead was, on another cut; the user, 2026-11-16).
        self.show_footage(0.0);
    }

    /// The picture above at joined footage time `t`: its cut, its clip time.
    fn show_footage(&mut self, t: f64) {
        let Some(edit) = &self.speed_edit else { return };
        let at = offsets(&edit.spans);
        let k = at.windows(2).position(|w| t < w[1]).unwrap_or(edit.spans.len().saturating_sub(1));
        let (Some(&id), Some(&(start, end))) = (edit.cuts.get(k), edit.spans.get(k)) else { return };
        if let Some(i) = self.edit.cuts.iter().position(|c| c.id == id) {
            self.goto(i, (start + t - at[k]).clamp(start, end), false);
        }
    }

    /// Closes the window, each cut's share of the curve and the pitch
    /// applied as one undo step (refused: said, and the window stays), or
    /// let go.
    fn close_speed(&mut self, apply: bool) {
        let Some(edit) = self.speed_edit.take() else { return };
        if !apply {
            return;
        }
        self.edit.begin();
        let mut done = true;
        for (&cut, speed) in edit.cuts.iter().zip(split(&edit.draft, &edit.spans)) {
            let keys = if speed.is_normal() { Vec::new() } else { speed.keys };
            done = done && self.op(Op::Cut(CutOp::SpeedCurve { cut, keys })).is_some() && self.op(Op::Cut(CutOp::TapePitch { cut, on: edit.draft.tape })).is_some();
        }
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
    /// half way; footage time across, both ways.
    #[test]
    fn the_curve_maps_rates_and_times_both_ways() {
        assert!((y(1.0) - (RULER + CURVE / 2.0)).abs() < 1e-4 && (y(RANGE.1) - RULER).abs() < 1e-4);
        for r in [0.25, 0.5, 1.0, 1.7, 4.0] {
            assert!((rate(y(r)) - r).abs() < 1e-4, "{r}");
        }
        let map = Map { length: 8.0, width: 800.0 };
        assert!((map.x(4.0) - (PAD + 400.0)).abs() < 1e-3 && (map.t(PAD + 400.0) - 4.0).abs() < 1e-9);
    }

    /// The cuts one curve takes: a run selected side by side holding the
    /// one clicked, else that one alone.
    #[test]
    fn a_curve_takes_the_cuts_selected_side_by_side() {
        let cuts: Vec<Cut> = (1..=4).map(|id| Cut { id, ..Cut::new("a.mp4".into(), 0.0, 2.0) }).collect();
        assert_eq!(cuts_for_curve(&cuts, &[2, 3], 3), [1, 2]);
        assert_eq!(cuts_for_curve(&cuts, &[1, 3], 3), [2], "not side by side");
        assert_eq!(cuts_for_curve(&cuts, &[2, 3], 4), [3], "clicked outside the selection");
        assert_eq!(cuts_for_curve(&cuts, &[], 1), [0]);
    }

    /// Two cuts' curves joined and split back: held keys exactly; a ramp
    /// over the join comes back as each cut's share, the rates where it
    /// crosses kept.
    #[test]
    fn cuts_curves_join_and_split_back() {
        let spans = [(10.0, 14.0), (2.0, 4.0)];
        let (a, b) = (Speed::constant(2.0), Speed { keys: vec![SpeedKey::held(2.0, 1.0), SpeedKey::held(3.0, 0.5)], tape: false });
        let one = joined(&[&a, &b], &spans);
        assert_eq!(one.keys.iter().map(|k| (k.t, k.rate)).collect::<Vec<_>>(), [(0.0, 2.0), (4.0, 1.0), (5.0, 0.5)]);
        let back = split(&one, &spans);
        assert_eq!((back[0].rate_at(12.0), back[1].rate_at(2.5), back[1].rate_at(3.5)), (2.0, 1.0, 0.5));
        let len = |s: &[Speed]| s[0].edit_secs(10.0, 14.0) + s[1].edit_secs(2.0, 4.0);
        assert!((len(&back) - one.edit_secs(0.0, 6.0)).abs() < 1e-9, "as long apart as joined");
        // A ramp from 1× at 2 s to 3× at 5 s, over the join at 4 s.
        let mut ramp = Speed::default().for_editing(0.0);
        ramp.put_key(2.0, 1.0, Ease::Linear);
        ramp.put_key(5.0, 3.0, Ease::Hold);
        for ease in [Ease::Linear, Ease::Smooth] {
            ramp.keys[1].ease = ease;
            let parts = split(&ramp, &spans);
            assert!((len(&parts) - ramp.edit_secs(0.0, 6.0)).abs() < 1e-9, "{ease:?}: as long as the window says");
            assert!((parts[0].rate_at(13.5) - ramp.rate_at(3.5)).abs() < 1e-12 && (parts[1].rate_at(2.5) - ramp.rate_at(4.5)).abs() < 1e-12);
            assert!(parts.iter().all(|p| p.check().is_ok()));
        }
        // Split and joined again: the same curve, as long.
        let parts = split(&ramp, &spans);
        let again = joined(&parts.iter().collect::<Vec<_>>(), &spans);
        assert!((again.edit_secs(0.0, 6.0) - ramp.edit_secs(0.0, 6.0)).abs() < 1e-12, "{:?}", again.keys);
        assert!(split(&again, &spans).iter().zip(&parts).all(|(a, b)| a == b), "Done with nothing changed changes nothing");
        // A cut all at 1× before the ramp starts stays normal.
        let mut later = Speed::default().for_editing(0.0);
        later.put_key(4.5, 1.0, Ease::Smooth);
        later.put_key(6.0, 3.0, Ease::Hold);
        assert!(split(&later, &spans)[0].is_default());
    }

    /// Opened on a cut, a point put, Cancel leaves the cut; a preset and
    /// the pitch, Done applies them as one undo step.
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
        assert!(app.speed_edit.is_none() && c.speed.keys.len() >= 4 && c.speed.tape && c.edit_len() > 2.0);
        assert!(app.edit.undo() && app.edit.cuts[0].speed.is_default(), "one step");
    }

    /// The window's playhead: set from a ruler, kept in the footage, and
    /// on a point when one is picked or put.
    #[test]
    fn the_window_has_a_playhead_of_its_own() {
        let mut app = app();
        let id = app.edit.cuts[0].id;
        let speed = |m| Message::Speed(m);
        let _ = app.update(speed(SpeedMsg::Open(id)));
        let head = |app: &super::super::App| app.speed_edit.as_ref().unwrap().head;
        assert_eq!(head(&app), 0.0);
        let _ = app.update(speed(SpeedMsg::Head(1.25)));
        assert_eq!(head(&app), 1.25);
        let _ = app.update(speed(SpeedMsg::Head(9.0)));
        assert_eq!(head(&app), 2.0, "kept in the footage");
        let _ = app.update(speed(SpeedMsg::Put(0.5, 2.0)));
        assert_eq!(head(&app), 0.5, "on the point put");
        let _ = app.update(speed(SpeedMsg::Head(f64::INFINITY)));
        assert_eq!(head(&app), 2.0, "End");
        // A step is edit seconds: at 2× (from the point at 0.5 s on) a
        // tenth of a second reads two tenths of footage.
        let _ = app.update(speed(SpeedMsg::Head(1.0)));
        let _ = app.update(speed(SpeedMsg::Step(0.1)));
        assert!((head(&app) - 1.2).abs() < 1e-9, "{}", head(&app));
    }

    /// Two cuts selected side by side take one curve; Done gives each its
    /// share, one undo step for both.
    #[test]
    fn two_cuts_selected_share_one_curve() {
        let mut app = app();
        let (a, b) = (app.edit.cuts[0].id, app.edit.cuts[1].id);
        (app.selection, app.also) = (crate::model::selection::Selection::Cut(a), vec![b]);
        let _ = app.update(Message::Speed(SpeedMsg::Open(b)));
        assert_eq!(app.speed_edit.as_ref().unwrap().cuts, [a, b]);

        let _ = app.update(Message::Speed(SpeedMsg::Preset(Preset::RampToCut)));
        let _ = app.update(Message::Speed(SpeedMsg::Close(true)));
        let (ca, cb) = (&app.edit.cuts[0], &app.edit.cuts[1]);
        assert!(ca.speed.is_default() && cb.edit_len() < 2.0, "the ramp into the end falls in the second");
        assert!(app.edit.undo() && app.edit.cuts[1].speed.is_default());
    }
}
