//! A cut's sound by hand: the «Normalize» dialog and the menu a right-click
//! on the Clip sound lane opens.
//!
//! Normalizing measures what each cut plays (`loudness::measure_cut`) and
//! gives it the level that brings it to −16 LUFS, optionally evening it out
//! first (`Cut::even`). It runs in a dialog over the window: what and how,
//! Start, then a progress bar with Cancel, then what changed. One cut is
//! measured at a time (the footage often sits on a spinning drive); a cut
//! takes about 0.2 s. The whole run is one undo step.
//!
//! The menu acts on the same per-cut level: lower it for a voice-over,
//! mute it, even it out, back to normal, or normalize just this cut.
//!
//! The user's design (2026-10-24/28): "so I don't have to fiddle with
//! levels myself"; a modal with Start and a progress bar.

use super::{App, Message, blocking};
use iced::widget::{button, checkbox, column, container, mouse_area, opaque, pin, progress_bar, radio, row, rule, space, stack, text};
use iced::{Element, Length, Point, Task};

/// How far «lower for voice-over» takes a cut down: the usual depth of a
/// dip under speech.
pub const VOICE_OVER_DIP: f64 = -12.0;

/// Which cuts the dialog works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    Cut(usize),
}

/// The dialog: before it starts, while it runs, when it's done.
#[derive(Debug)]
pub struct Dialog {
    scope: Scope,
    even: bool,
    state: State,
}

#[derive(Debug)]
enum State {
    Ready,
    Running { todo: Vec<usize>, total: usize, done: usize, gains: Vec<f64>, silent: usize, failed: Vec<String>, cancelled: bool },
    Done(String),
}

#[derive(Clone, Debug)]
pub enum SoundMsg {
    /// Open the dialog for these cuts.
    Open(Scope),
    Scope(Scope),
    Even(bool),
    Start,
    /// Cut `i` measured: its loudness and gain, `None` if silent.
    Measured(usize, Result<Option<(f64, f64)>, String>),
    Cancel,
    Close,
    /// The right-click menu for cut `i`, where the pointer is (window
    /// coordinates), or closed.
    Menu(Option<(usize, Point)>),
    /// Cut `i`'s level down for a voice-over; back to its normal level.
    Lower(usize),
    Restore(usize),
    Mute(usize, bool),
    /// Cut `i`'s volume line shown: points at its in and out at 0 dB, so
    /// nothing sounds different until one is moved.
    ShowLine(usize),
    Even1(usize, bool),
}

impl App {
    pub(super) fn sound_ui(&mut self, msg: SoundMsg) -> Task<Message> {
        match msg {
            SoundMsg::Open(scope) => {
                self.sound_menu = None;
                // Evened out by default: it lifts quiet speech without
                // pushing the loud parts into the limiter (gap 11's
                // measurements).
                self.normalize = Some(Dialog { scope, even: true, state: State::Ready });
            }
            SoundMsg::Scope(scope) => {
                if let Some(d) = self.normalize.as_mut().filter(|d| matches!(d.state, State::Ready)) {
                    d.scope = scope;
                }
            }
            SoundMsg::Even(on) => {
                if let Some(d) = self.normalize.as_mut().filter(|d| matches!(d.state, State::Ready)) {
                    d.even = on;
                }
            }
            SoundMsg::Start => {
                let Some(d) = self.normalize.as_mut() else { return Task::none() };
                let todo: Vec<usize> = match d.scope {
                    Scope::All => (0..self.edit.cuts.len()).collect(),
                    Scope::Cut(i) if i < self.edit.cuts.len() => vec![i],
                    Scope::Cut(_) => Vec::new(),
                };
                // One undo step for the whole run, before the first change.
                // Each cut is evened out (or not) as it's measured, so a
                // stopped run leaves the rest as they were.
                self.edit.checkpoint();
                let total = todo.len();
                d.state = State::Running { todo: todo.into_iter().rev().collect(), total, done: 0, gains: Vec::new(), silent: 0, failed: Vec::new(), cancelled: false };
                return self.measure_next();
            }
            SoundMsg::Measured(i, result) => {
                let even = self.normalize.as_ref().is_some_and(|d| d.even);
                if let Some(cut) = self.edit.cuts.get_mut(i) {
                    if result.is_ok() {
                        cut.even = even;
                    }
                    match &result {
                        Ok(Some((lufs, gain))) => {
                            cut.gain = *gain;
                            cut.loudness = Some(*lufs);
                        }
                        Ok(None) | Err(_) => {}
                    }
                }
                let name = self.edit.cuts.get(i).map(|c| super::clip_name(&c.input)).unwrap_or_default();
                if let Some(Dialog { state: State::Running { done, gains, silent, failed, .. }, .. }) = self.normalize.as_mut() {
                    *done += 1;
                    match result {
                        Ok(Some((_, gain))) => gains.push(gain),
                        Ok(None) => *silent += 1,
                        Err(e) => failed.push(format!("{name}: {e}")),
                    }
                }
                self.reload_cut_sound();
                return self.measure_next();
            }
            SoundMsg::Cancel => match self.normalize.as_mut() {
                Some(Dialog { state: State::Running { cancelled, .. }, .. }) => *cancelled = true,
                _ => self.normalize = None,
            },
            SoundMsg::Close => self.normalize = None,
            SoundMsg::Menu(at) => self.sound_menu = at,
            // Twice at most: further down it would be as good as muted,
            // without saying so.
            SoundMsg::Lower(i) => self.set_cut_sound(i, |c| c.gain = (c.gain + VOICE_OVER_DIP).max(normal_gain(c) + 2.0 * VOICE_OVER_DIP)),
            SoundMsg::Restore(i) => self.set_cut_sound(i, |c| c.gain = normal_gain(c)),
            SoundMsg::Mute(i, on) => self.set_cut_sound(i, |c| c.mute = on),
            SoundMsg::ShowLine(i) => self.set_cut_sound(i, |c| {
                for t in [c.start, c.end] {
                    crate::project::add_volume_key(&mut c.volume, t, 0.0);
                }
            }),
            SoundMsg::Even1(i, on) => {
                // Evening out changes how loud it is: measured again, as
                // normalizing would.
                if on || self.edit.cuts.get(i).is_some_and(|c| c.loudness.is_some()) {
                    self.sound_menu = None;
                    self.normalize = Some(Dialog { scope: Scope::Cut(i), even: on, state: State::Ready });
                    return self.sound_ui(SoundMsg::Start);
                }
                self.set_cut_sound(i, |c| c.even = on);
            }
        }
        Task::none()
    }

    /// Changes cut `i`'s sound as one undo step, closing the menu.
    fn set_cut_sound(&mut self, i: usize, change: impl FnOnce(&mut crate::project::Cut)) {
        self.sound_menu = None;
        if i < self.edit.cuts.len() {
            self.edit.checkpoint();
            change(&mut self.edit.cuts[i]);
            self.reload_cut_sound();
        }
    }

    /// Starts measuring the next cut, or ends the run with what changed.
    fn measure_next(&mut self) -> Task<Message> {
        let Some(Dialog { state, even, .. }) = self.normalize.as_mut() else { return Task::none() };
        let even = *even;
        let State::Running { todo, total, gains, silent, failed, cancelled, done } = state else { return Task::none() };
        let next = if *cancelled { None } else { todo.pop() };
        let Some(i) = next else {
            *state = State::Done(summary(*done, *total, gains, *silent, failed, *cancelled));
            return Task::none();
        };
        let Some(mut cut) = self.edit.cuts.get(i).cloned() else { return Task::none() };
        // Measured as it will sound.
        cut.even = even;
        Task::perform(blocking(move || crate::loudness::measure_cut(&cut).map_err(|e| format!("{e:#}"))), move |r| Message::Sound(SoundMsg::Measured(i, r)))
    }

    /// The dialog over the window, if open.
    pub(super) fn normalize_dialog<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some(d) = &self.normalize else { return below };
        let selected = self.selected.filter(|&i| i < self.edit.cuts.len());
        let body: Element<'_, Message> = match &d.state {
            State::Ready => {
                let mut choices = column![radio(format!("All cuts ({})", self.edit.cuts.len()), Scope::All, Some(d.scope), |s| Message::Sound(SoundMsg::Scope(s)))].spacing(8);
                if let Some(i) = selected.or(match d.scope {
                    Scope::Cut(i) => Some(i),
                    Scope::All => None,
                }) {
                    choices = choices.push(radio(format!("Cut {}: {}", i + 1, super::clip_name(&self.edit.cuts[i].input)), Scope::Cut(i), Some(d.scope), |s| Message::Sound(SoundMsg::Scope(s))));
                }
                column![
                    choices,
                    checkbox(d.even).label("Even out quiet and loud stretches").on_toggle(|on| Message::Sound(SoundMsg::Even(on))),
                    text(format!("Speech to {:.0} LUFS; peaks held under −1 dB.", crate::loudness::TARGET_LUFS)).size(13).style(text::secondary),
                    row![
                        space().width(Length::Fill),
                        button(text("Cancel")).style(button::secondary).on_press(Message::Sound(SoundMsg::Close)),
                        button(text("Start")).style(button::primary).on_press(Message::Sound(SoundMsg::Start)),
                    ]
                    .spacing(10),
                ]
                .spacing(14)
                .into()
            }
            State::Running { total, done, cancelled, .. } => column![
                text(if *cancelled { "Stopping…".to_owned() } else { format!("Cut {} of {total}", (*done + 1).min(*total)) }),
                progress_bar(0.0..=(*total).max(1) as f32, *done as f32).girth(10),
                row![space().width(Length::Fill), button(text("Cancel")).style(button::secondary).on_press_maybe((!*cancelled).then_some(Message::Sound(SoundMsg::Cancel)))],
            ]
            .spacing(14)
            .into(),
            State::Done(summary) => column![
                text(summary.clone()),
                row![space().width(Length::Fill), button(text("Close")).style(button::primary).on_press(Message::Sound(SoundMsg::Close))],
            ]
            .spacing(14)
            .into(),
        };
        let card = container(column![text("Normalize sound").size(18), rule::horizontal(1), body].spacing(12).width(380)).padding(18).style(container::rounded_box);
        // Dimmed behind, and nothing below takes a click while it's open.
        let shade = container(iced::widget::center(opaque(card))).width(Length::Fill).height(Length::Fill).style(|_| container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55)));
        stack![below, opaque(shade)].into()
    }

    /// The menu for a cut's sound at the pointer, if open: a click outside
    /// or Esc closes it.
    pub(super) fn sound_menu_layer<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some((i, at)) = self.sound_menu else { return below };
        let Some(cut) = self.edit.cuts.get(i) else { return below };
        let item = |label: String, msg: SoundMsg| button(text(label).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(Message::Sound(msg));
        let mut items = column![item(format!("Lower for voice-over ({VOICE_OVER_DIP:+.0} dB)"), SoundMsg::Lower(i))];
        if (cut.gain - normal_gain(cut)).abs() > 0.05 {
            items = items.push(item(format!("Back to normal level ({:+.1} dB)", normal_gain(cut)), SoundMsg::Restore(i)));
        }
        // The line shows once the cut has points; then a click on it adds one.
        if cut.volume.is_empty() {
            items = items.push(item("Show volume line".into(), SoundMsg::ShowLine(i)));
        }
        items = items
            .push(item(if cut.mute { "Unmute".into() } else { "Mute".into() }, SoundMsg::Mute(i, !cut.mute)))
            .push(item(if cut.even { "Stop evening out".into() } else { "Even out quiet and loud".into() }, SoundMsg::Even1(i, !cut.even)))
            .push(item("Normalize…".into(), SoundMsg::Open(Scope::Cut(i))))
            .push(rule::horizontal(1))
            .push(button(text("Info").size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(Message::Info(super::info::Of::Cut(i))));
        let menu = container(items.width(240)).padding(4).style(container::rounded_box);
        // Above the pointer when it's low in the window: the timeline is.
        let top = (at.y - 200.0).max(0.0);
        let close = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::Sound(SoundMsg::Menu(None))).on_right_press(Message::Sound(SoundMsg::Menu(None)));
        stack![below, close, pin(opaque(menu)).position(Point::new(at.x, top))].into()
    }
}

/// The level a cut has when nothing has lowered it: what normalizing gave
/// it, or as recorded.
fn normal_gain(cut: &crate::project::Cut) -> f64 {
    cut.loudness.map_or(0.0, crate::loudness::normalizing_gain)
}

/// What a run changed, in a sentence or two.
fn summary(done: usize, total: usize, gains: &[f64], silent: usize, failed: &[String], cancelled: bool) -> String {
    let cuts = |n: usize| if n == 1 { "cut" } else { "cuts" };
    let mut s = if cancelled { format!("Stopped after {done} of {total} {}.", cuts(total)) } else { format!("{done} {} done.", cuts(done)) };
    if let (Some(lo), Some(hi)) = (gains.iter().copied().reduce(f64::min), gains.iter().copied().reduce(f64::max)) {
        s += &if (hi - lo).abs() < 0.05 { format!(" Level {lo:+.1} dB.") } else { format!(" Levels from {lo:+.1} to {hi:+.1} dB.") };
    }
    if silent > 0 {
        s += &format!(" {silent} without sound, left as they were.");
    }
    if !failed.is_empty() {
        s += &format!(" Couldn't read: {}.", failed.join("; "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_changed() {
        assert_eq!(summary(3, 3, &[2.6, 7.7, 0.7], 0, &[], false), "3 cuts done. Levels from +0.7 to +7.7 dB.");
        assert_eq!(summary(1, 1, &[16.1], 0, &[], false), "1 cut done. Level +16.1 dB.");
        assert_eq!(summary(2, 39, &[1.0, 2.0], 1, &[], true), "Stopped after 2 of 39 cuts. Levels from +1.0 to +2.0 dB. 1 without sound, left as they were.");
    }
}
