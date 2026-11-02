//! The mix check in the viewer's place (gap 45, step 3): the sound lanes
//! as a schematic in their colours, each clip a block as tall as it is
//! loud, what breaks a rule outlined, the findings in words beside it. A
//! click on a block or a finding selects that clip on the timeline and
//! takes the playhead there, to fix it by hand. Measured off the UI thread
//! (`mixmeasure`), again on asking: the edit changes under it.

use super::{App, Message, blocking};
use crate::mixcheck::{Finding, Lane, Measured, Rules, Severity, check};
use crate::project::{Own, SoundTrack};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, canvas as canvas_widget, column, container, row, scrollable, space, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Task, Theme, mouse};

/// The time ruler's height over the lanes, px, and the lanes' names' room.
const RULER: f32 = 22.0;
const LEFT: f32 = 90.0;

/// The loudness range a block's height spans, LUFS.
const QUIET: f64 = -50.0;
const LOUD: f64 = -6.0;

#[derive(Default)]
pub struct Check {
    pub on: bool,
    running: bool,
    measured: Option<Measured>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum CheckMsg {
    Toggle(bool),
    Again,
    Measured(Result<Measured, String>),
    /// Select this clip and take the playhead to it.
    Go(Own, f64),
}

impl App {
    /// The rules at the export's loudness target, else YouTube's.
    fn check_rules(&self) -> Rules {
        Rules::at(self.export_loudness.unwrap_or(-14.0))
    }

    pub(super) fn mix_check(&mut self, msg: CheckMsg) -> Task<Message> {
        match msg {
            CheckMsg::Toggle(on) => {
                self.check.on = on;
                if on && self.check.measured.is_none() {
                    return self.mix_check(CheckMsg::Again);
                }
            }
            CheckMsg::Again => {
                if self.check.running {
                    return Task::none();
                }
                self.check.running = true;
                self.check.error = None;
                let project = (*self.edit).clone();
                return Task::perform(blocking(move || crate::mixmeasure::measure(&project, &mut |_, _| {}).map_err(|e| format!("{e:#}"))), |r| Message::Check(CheckMsg::Measured(r)));
            }
            CheckMsg::Measured(r) => {
                self.check.running = false;
                match r {
                    Ok(m) => self.check.measured = Some(m),
                    Err(e) => self.check.error = Some(e),
                }
            }
            CheckMsg::Go(own, at) => {
                let select = match own {
                    Own::Cut(i) => Message::Select(i),
                    Own::Clip(which, i) => Message::SelectMusic(which, i),
                };
                let tasks = [self.update(select), self.update(Message::Scrub(at)), self.update(Message::ScrubDone)];
                self.reveal_playhead();
                return Task::batch(tasks);
            }
        }
        Task::none()
    }

    /// Where the findings are on the edit, for the timeline's seek bar:
    /// each one's stretch and whether it's one to fix; the whole film's
    /// aren't anywhere in particular.
    pub(super) fn check_marks(&self) -> Vec<(f64, f64, bool)> {
        let Some(m) = &self.check.measured else { return Vec::new() };
        check(m, &self.check_rules()).into_iter().filter(|f| f.own.is_some()).map(|f| (f.at, f.until, f.severity == Severity::Fix)).collect()
    }

    /// What stands in the viewer's place while the check is on.
    pub(super) fn mix_check_view(&self) -> Element<'_, Message> {
        let rules = self.check_rules();
        let Some(m) = &self.check.measured else {
            let said = self.check.error.clone().map_or_else(|| "Measuring the sound…".to_owned(), |e| format!("Couldn't measure: {e}"));
            return container(text(said)).center(Length::Fill).into();
        };
        let found = check(m, &rules);
        let head = row![
            text(format!("Against {:.0} LUFS", rules.target)).size(15),
            text(format!("Film {} · peak {:.1} dBFS", m.mix.map_or("silent".to_owned(), |v| format!("{v:.1} LUFS")), m.peak)).size(13),
            space().width(Length::Fill),
            button(text(if self.check.running { "Measuring…" } else { "Measure again" }).size(13)).on_press_maybe((!self.check.running).then_some(Message::Check(CheckMsg::Again))),
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        let list = found.iter().fold(column![].spacing(2), |col, f| {
            let mark = if f.severity == Severity::Fix { "●" } else { "○" };
            let line = text(format!("{mark} {:>6.1} s  {}", f.at, f.text)).size(13);
            col.push(match f.own {
                Some(own) => button(line).style(button::text).padding([2, 4]).on_press(Message::Check(CheckMsg::Go(own, f.at))).into(),
                None => Element::from(container(line).padding([2, 4])),
            })
        });
        let list: Element<'_, Message> = if found.is_empty() { text("Nothing to fix by these rules.").size(13).into() } else { scrollable(list).height(Length::Fill).into() };
        let colours = SoundTrack::ALL.map(|w| super::timeline::lane_colour(Lane::Track(w), &self.theme));
        let schematic = Schematic { measured: m.clone(), found: found.clone(), duration: self.edit.frame().max(0.1), position: self.position(), colours, clip_sound: super::timeline::lane_colour(Lane::ClipSound, &self.theme) };
        let body = column![head, canvas_widget(schematic).width(Length::Fill).height(Length::FillPortion(3)), container(list).height(Length::FillPortion(2))].spacing(10).padding(12);
        container(body).width(Length::Fill).height(Length::Fill).into()
    }
}

/// The schematic as drawn: what was measured, what breaks the rules.
struct Schematic {
    measured: Measured,
    found: Vec<Finding>,
    duration: f64,
    position: f64,
    colours: [Color; SoundTrack::ALL.len()],
    clip_sound: Color,
}

impl Schematic {
    /// Edit time to x, in a canvas `width` wide.
    fn x(&self, t: f64, width: f32) -> f32 {
        LEFT + (t / self.duration) as f32 * (width - LEFT - 6.0)
    }

    /// The finding whose mark on the ruler is under `x`.
    fn mark_at(&self, x: f32, width: f32) -> Option<&Finding> {
        self.found.iter().filter(|f| f.own.is_some()).find(|f| x >= self.x(f.at, width) - 4.0 && x <= self.x(f.until, width).max(self.x(f.at, width) + 4.0))
    }

    /// The lanes shown, top to bottom: the clips' sound, then each track
    /// with something on it.
    fn lanes(&self) -> Vec<Lane> {
        std::iter::once(Lane::ClipSound).chain(SoundTrack::ALL.map(Lane::Track)).filter(|l| self.measured.items.iter().any(|i| i.lane == *l)).collect()
    }

    /// Each item's block: its rectangle and the item.
    fn blocks(&self, b: Size) -> Vec<(Rectangle, &crate::mixcheck::Item)> {
        let lanes = self.lanes();
        let h = (b.height - RULER) / lanes.len().max(1) as f32;
        let x = |t: f64| self.x(t, b.width);
        self.measured
            .items
            .iter()
            .filter_map(|i| {
                let row = lanes.iter().position(|l| *l == i.lane)? as f32;
                let share = i.lufs.map_or(0.04, |v| ((v - QUIET) / (LOUD - QUIET)).clamp(0.04, 1.0)) as f32;
                let (l, r) = (x(i.at), x(i.at + i.len));
                let tall = share * (h - 10.0);
                Some((Rectangle::new(Point::new(l + 1.0, RULER + row * h + h - 4.0 - tall), Size::new((r - l - 2.0).max(2.0), tall)), i))
            })
            .collect()
    }
}

impl canvas::Program<Message> for Schematic {
    type State = ();

    fn draw(&self, _state: &(), renderer: &Renderer, theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let lanes = self.lanes();
        let h = (bounds.height - RULER) / lanes.len().max(1) as f32;
        let x = |t: f64| self.x(t, bounds.width);
        // The ruler: seconds, a tick every so many as fit 70 px apart.
        let per_px = self.duration / (bounds.width - LEFT - 6.0) as f64;
        let step = [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0].into_iter().find(|&s| s / per_px >= 70.0).unwrap_or(1200.0);
        let mut t = 0.0;
        while t <= self.duration {
            frame.fill_rectangle(Point::new(x(t), RULER - 6.0), Size::new(1.0, 6.0), palette.background.base.text.scale_alpha(0.6));
            frame.fill_text(Text { content: format!("{}:{:02}", (t / 60.0) as u32, (t % 60.0) as u32), position: Point::new(x(t) + 3.0, 2.0), color: palette.background.base.text.scale_alpha(0.8), size: 10.0.into(), ..Default::default() });
            t += step;
        }
        // The findings on it, where each is: red to fix, yellow a hint.
        for f in self.found.iter().filter(|f| f.own.is_some()) {
            let ink = if f.severity == Severity::Fix { Color::from_rgb(1.0, 0.3, 0.3) } else { Color::from_rgb(0.95, 0.79, 0.3) };
            let (a, b) = (x(f.at), x(f.until).max(x(f.at) + 3.0));
            frame.fill_rectangle(Point::new(a, RULER - 5.0), Size::new(b - a, 4.0), ink);
            let tip = Path::new(|p| {
                p.move_to(Point::new(a - 4.0, RULER - 13.0));
                p.line_to(Point::new(a + 4.0, RULER - 13.0));
                p.line_to(Point::new(a, RULER - 6.0));
                p.close();
            });
            frame.fill(&tip, ink);
        }
        // Speech, faint across every lane: what the music sits under.
        for s in &self.measured.speech {
            frame.fill_rectangle(Point::new(x(s.from), RULER), Size::new(x(s.to) - x(s.from), bounds.height - RULER), Color { a: 0.07, ..palette.background.base.text });
        }
        for (row, lane) in lanes.iter().enumerate() {
            let top = RULER + row as f32 * h;
            frame.fill_rectangle(Point::new(0.0, top + 2.0), Size::new(bounds.width, h - 4.0), palette.background.weak.color.scale_alpha(0.4));
            let name = match lane {
                Lane::ClipSound => "Clip sound",
                Lane::Track(w) => w.name(),
            };
            frame.fill_text(Text { content: name.into(), position: Point::new(8.0, top + h / 2.0), color: palette.background.base.text, size: 12.0.into(), align_y: iced::alignment::Vertical::Center, ..Default::default() });
        }
        for (r, item) in self.blocks(bounds.size()) {
            let colour = match item.lane {
                Lane::ClipSound => self.clip_sound,
                Lane::Track(w) => self.colours[SoundTrack::ALL.iter().position(|&t| t == w).unwrap_or(0)],
            };
            frame.fill_rectangle(r.position(), r.size(), colour.scale_alpha(0.75));
            let worst = self.found.iter().filter(|f| f.own == Some(item.own)).map(|f| f.severity).max();
            if let Some(sev) = worst {
                let ink = if sev == Severity::Fix { Color::from_rgb(1.0, 0.3, 0.3) } else { Color::from_rgb(0.95, 0.79, 0.3) };
                frame.stroke(&Path::rectangle(r.position(), r.size()), Stroke::default().with_width(2.0).with_color(ink));
            }
            if let Some(v) = item.lufs.filter(|_| r.width > 40.0) {
                frame.fill_text(Text { content: format!("{v:.0}"), position: Point::new(r.x + 3.0, r.y - 7.0), color: palette.background.base.text, size: 10.0.into(), ..Default::default() });
            }
        }
        let at = x(self.position);
        frame.fill_rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, bounds.height), palette.primary.strong.color);
        vec![frame.into_geometry()]
    }

    fn update(&self, _state: &mut (), event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event else { return None };
        let p = cursor.position_in(bounds)?;
        // A finding's mark on the ruler: to its clip, at where it is.
        if p.y < RULER {
            let f = self.mark_at(p.x, bounds.width)?;
            return Some(canvas::Action::publish(Message::Check(CheckMsg::Go(f.own?, f.at))).and_capture());
        }
        // A block, or anywhere in its stretch of its lane.
        let lanes = self.lanes();
        let h = (bounds.height - RULER) / lanes.len().max(1) as f32;
        let row = |y: f32| ((y - RULER) / h).floor();
        let (_, item) = self.blocks(bounds.size()).into_iter().find(|(r, _)| p.x >= r.x && p.x <= r.x + r.width && row(p.y) == row(r.y))?;
        Some(canvas::Action::publish(Message::Check(CheckMsg::Go(item.own, item.at))).and_capture())
    }

    fn mouse_interaction(&self, _state: &(), bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let over = cursor.position_in(bounds).is_some_and(|p| if p.y < RULER { self.mark_at(p.x, bounds.width).is_some() } else { self.blocks(bounds.size()).iter().any(|(r, _)| p.x >= r.x && p.x <= r.x + r.width) });
        if over { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixcheck::Item;

    #[test]
    fn blocks_stand_in_their_lanes_as_tall_as_they_are_loud() {
        let item = |lane: Lane, own: Own, at: f64, lufs: f64| Item { lane, own, at, len: 2.0, lufs: Some(lufs), loudest: Some(lufs + 3.0), speech: false };
        let measured = Measured {
            items: vec![item(Lane::ClipSound, Own::Cut(0), 0.0, -40.0), item(Lane::Track(SoundTrack::Music), Own::Clip(SoundTrack::Music, 0), 4.0, -10.0)],
            ..Default::default()
        };
        let s = Schematic { measured, found: vec![], duration: 10.0, position: 0.0, colours: [Color::WHITE; SoundTrack::ALL.len()], clip_sound: Color::WHITE };
        assert_eq!(s.lanes(), vec![Lane::ClipSound, Lane::Track(SoundTrack::Music)], "lanes with nothing aren't shown");
        let blocks = s.blocks(Size::new(1000.0, 200.0));
        let (quiet, loud) = (blocks[0].0, blocks[1].0);
        assert!(quiet.y >= RULER && quiet.y + quiet.height <= RULER + 89.0 && loud.y >= RULER + 89.0, "each in its lane, under the ruler");
        assert!(loud.height > 3.0 * quiet.height, "louder is taller");
        assert!(loud.x > quiet.x + quiet.width, "later is further right");
    }
}
