//! Sound by hand: the «Normalize» dialog, and the menu a right-click opens
//! on a cut's sound (the Clip sound lane), on a sound clip, or on a sound
//! lane's empty stretch (the track's own menu). A cut's sound and a sound
//! clip's are changed alike, through `project::Own` (gap 33f).
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

use super::{App, Message, Selection, blocking};
use crate::model::ops::sound::{SoundOp, VOICE_OVER_DIP, normal_gain};
use crate::model::ops::{Op, SoundRef};
use crate::project::{Own, SoundTrack};
use iced::widget::{button, checkbox, column, container, mouse_area, opaque, pin, progress_bar, radio, row, rule, space, stack, text};
use iced::{Element, Length, Point, Task};

/// What the dialog works on: every cut, one cut's sound or sound clip, or
/// every clip on a sound track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    Own(Own),
    Track(SoundTrack),
}

/// What a right-click menu is for: a cut's sound or a sound clip, a sound
/// track as a whole, or a volume line's point or the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Own(Own),
    Track(SoundTrack),
    Point(Own, usize),
    Line(Own),
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
    Running { todo: Vec<Own>, total: usize, done: usize, gains: Vec<f64>, silent: usize, failed: Vec<String>, cancelled: bool },
    Done(String),
}

#[derive(Clone, Debug)]
pub enum SoundMsg {
    /// Open the dialog for these cuts.
    Open(Scope),
    Scope(Scope),
    Even(bool),
    Start,
    /// One measured: its loudness and gain, `None` if silent.
    Measured(Own, Result<Option<(f64, f64)>, String>),
    Cancel,
    Close,
    /// A right-click menu, where the pointer is (window coordinates), or
    /// closed.
    Menu(Option<(Menu, Point)>),
    /// Its level down for a voice-over; back to its normal level.
    Lower(Own),
    Restore(Own),
    Mute(Own, bool),
    /// Its volume line shown: points at its in and out at 0 dB, so nothing
    /// sounds different until one is moved.
    ShowLine(Own),
    Even1(Own, bool),
    /// Its line started where what plays before it ends
    /// (`Project::continue_level`).
    Continue(Own),
    /// Back to the file as it is (`OwnSound::original`).
    Original(Own),
    /// Its low rumble cut, on or off (`loudness::LOW_CUT`); its steady
    /// noise cleaned (`loudness::CLEAN`).
    LowCut(Own, bool),
    Clean(Own, bool),
    /// A sound effect moved to the other SFX lane.
    OtherSfx(SoundTrack, usize),
    /// A volume line's point `i` removed (Ctrl+click on it, or its menu).
    DeletePoint(Own, usize),
    /// A volume line removed, all its points.
    DeleteLine(Own),
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
                let todo: Vec<Own> = match d.scope {
                    Scope::All => (0..self.edit.cuts.len()).map(Own::Cut).collect(),
                    Scope::Own(own) if self.edit.own_line(own).is_some() => vec![own],
                    Scope::Own(_) => Vec::new(),
                    Scope::Track(which) => (0..self.edit.track(which).clips.len()).map(|i| Own::Clip(which, i)).collect(),
                };
                // One undo step for the whole run, before the first change.
                // Each cut is evened out (or not) as it's measured, so a
                // stopped run leaves the rest as they were.
                self.edit.checkpoint();
                let total = todo.len();
                d.state = State::Running { todo: todo.into_iter().rev().collect(), total, done: 0, gains: Vec::new(), silent: 0, failed: Vec::new(), cancelled: false };
                return self.measure_next();
            }
            SoundMsg::Measured(own, result) => {
                let even = self.normalize.as_ref().is_some_and(|d| d.even);
                if let Some(s) = self.edit.own_sound(own) {
                    if result.is_ok() {
                        *s.even = even;
                    }
                    if let Ok(Some((lufs, gain))) = &result {
                        *s.gain = *gain;
                        *s.loudness = Some(*lufs);
                    }
                }
                let name = self.edit.own_file(own, false).map(|(f, ..)| super::clip_name(&f)).unwrap_or_default();
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
            // From its normal level, not from where it is: pressed again it
            // stays put (user, 2026-11-02: operations start from the
            // original; further down is the level knob's job).
            SoundMsg::Lower(own) => self.sound_op(own, |sound| SoundOp::LowerForVoice { sound }),
            SoundMsg::Restore(own) => self.sound_op(own, |sound| SoundOp::Restore { sound }),
            SoundMsg::Mute(own, on) => self.sound_op(own, |sound| SoundOp::Mute { sound, on }),
            SoundMsg::ShowLine(own) => self.sound_op(own, |sound| SoundOp::ShowLine { sound }),
            SoundMsg::Original(own) => self.sound_op(own, |sound| SoundOp::Original { sound }),
            SoundMsg::LowCut(own, on) => self.sound_op(own, |sound| SoundOp::LowCut { sound, on }),
            SoundMsg::Clean(own, on) => self.sound_op(own, |sound| SoundOp::Clean { sound, on }),
            SoundMsg::OtherSfx(from, i) => {
                self.sound_menu = None;
                let to = if from == SoundTrack::Sfx { SoundTrack::Sfx2 } else { SoundTrack::Sfx };
                self.edit.checkpoint();
                if let Some(j) = self.edit.move_clip(from, i, to) {
                    self.selection = Selection::Clip(to, j);
                }
            }
            SoundMsg::Continue(own) => {
                self.sound_menu = None;
                self.edit.attempt(|e| e.continue_level(own));
            }
            SoundMsg::DeletePoint(own, i) => {
                self.sound_menu = None;
                self.edit.delete(Selection::Point(own, i));
                if self.selection.point().is_some() {
                    self.selection = Selection::None;
                }
            }
            SoundMsg::DeleteLine(own) => {
                self.sound_menu = None;
                if self.selection.point().is_some() {
                    self.selection = Selection::None;
                }
                self.sound_op(own, |sound| SoundOp::DeleteLine { sound });
            }
            SoundMsg::Even1(own, on) => {
                // Evening out changes how loud it is: measured again, as
                // normalizing would.
                if on || self.edit.own_sound(own).is_some_and(|s| s.loudness.is_some()) {
                    self.sound_menu = None;
                    self.normalize = Some(Dialog { scope: Scope::Own(own), even: on, state: State::Ready });
                    return self.sound_ui(SoundMsg::Start);
                }
                self.set_sound(own, |s| *s.even = on);
            }
        }
        Task::none()
    }

    /// Applies the sound op `make` builds for `own` (`OPS-API.md`), closing
    /// the menu; a refusal is shown.
    pub(super) fn sound_op(&mut self, own: Own, make: impl FnOnce(SoundRef) -> SoundOp) {
        self.sound_menu = None;
        let Some(sound) = SoundRef::of(own, &self.edit) else { return };
        match self.edit.apply(&Op::Sound(make(sound))) {
            Ok(_) => self.reload_cut_sound(),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Changes a cut's sound or a sound clip's as one undo step, closing
    /// the menu.
    fn set_sound(&mut self, own: Own, change: impl FnOnce(crate::project::OwnSound<'_>)) {
        self.sound_menu = None;
        if self.edit.own_line(own).is_some() {
            self.edit.checkpoint();
            if let Some(s) = self.edit.own_sound(own) {
                change(s);
            }
            self.reload_cut_sound();
        }
    }

    /// Starts measuring the next cut, or ends the run with what changed.
    fn measure_next(&mut self) -> Task<Message> {
        let Some(Dialog { state, even, .. }) = self.normalize.as_mut() else { return Task::none() };
        let even = *even;
        let State::Running { todo, total, gains, silent, failed, cancelled, done } = state else { return Task::none() };
        let next = if *cancelled { None } else { todo.pop() };
        let Some(own) = next else {
            *state = State::Done(summary(*done, *total, gains, *silent, failed, *cancelled));
            return Task::none();
        };
        // Measured as it will sound: evened out or not as asked.
        let Some((file, range, filter)) = self.edit.own_file(own, even) else { return Task::none() };
        Task::perform(blocking(move || crate::loudness::measure_stretch(&file, range, filter.as_deref()).map_err(|e| format!("{e:#}"))), move |r| Message::Sound(SoundMsg::Measured(own, r)))
    }

    /// The dialog over the window, if open.
    pub(super) fn normalize_dialog<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some(d) = &self.normalize else { return below };
        let pick = |label: String, scope: Scope| radio(label, scope, Some(d.scope), |s| Message::Sound(SoundMsg::Scope(s)));
        let body: Element<'_, Message> = match &d.state {
            State::Ready => {
                // Every cut always; then the one asked about (a cut or a
                // sound clip), or the selected cut, and a track's clips.
                let mut choices = column![pick(format!("All cuts ({})", self.edit.cuts.len()), Scope::All)].spacing(8);
                let one = match d.scope {
                    Scope::Own(own) => Some(own),
                    _ => self.selection.cut().filter(|&i| i < self.edit.cuts.len()).map(Own::Cut),
                };
                if let Some(own) = one.filter(|&o| self.edit.own_line(o).is_some()) {
                    choices = choices.push(pick(self.own_name(own), Scope::Own(own)));
                }
                if let Scope::Track(which) = d.scope {
                    choices = choices.push(pick(format!("Every clip on {} ({})", which.name(), self.edit.track(which).clips.len()), d.scope));
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
                text(if *cancelled { "Stopping…".to_owned() } else { format!("{} of {total}", (*done + 1).min(*total)) }),
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

    /// How the dialog names a cut's sound or a sound clip.
    fn own_name(&self, own: Own) -> String {
        let file = self.edit.own_file(own, false).map(|(f, ..)| super::clip_name(&f)).unwrap_or_default();
        match own {
            Own::Cut(i) => format!("Cut {}: {file}", i + 1),
            Own::Clip(which, _) => format!("{}: {file}", which.name()),
        }
    }

    /// A right-click menu at the pointer, if open: a click outside or Esc
    /// closes it.
    pub(super) fn sound_menu_layer<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some((menu, at)) = self.sound_menu else { return below };
        let item = |label: String, msg: Message| -> Element<'a, Message> { button(text(label).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(msg).into() };
        let sound = |label: String, msg: SoundMsg| item(label, Message::Sound(msg));
        let mut items: Vec<Element<'a, Message>> = Vec::new();
        match menu {
            Menu::Own(own) => {
                let Some((keys, _)) = self.edit.own_line(own) else { return below };
                let (gain, mute, even, loudness, low_cut, clean) = match own {
                    Own::Cut(i) => self.edit.cuts.get(i).map(|c| (c.gain, c.mute, c.even, c.loudness, c.low_cut, c.clean)),
                    Own::Clip(which, i) => self.edit.track(which).clips.get(i).map(|c| (c.gain, c.mute, c.even, c.loudness, c.low_cut, c.clean)),
                }
                .unwrap_or_default();
                items.push(sound(format!("Lower for voice-over ({VOICE_OVER_DIP:+.0} dB)"), SoundMsg::Lower(own)));
                if (gain - normal_gain(loudness)).abs() > 0.05 {
                    items.push(sound(format!("Back to normal level ({:+.1} dB)", normal_gain(loudness)), SoundMsg::Restore(own)));
                }
                // The line shows once it has points; then a click on it adds one.
                if keys.is_empty() {
                    items.push(sound("Show volume line".into(), SoundMsg::ShowLine(own)));
                }
                if self.edit.before(own).is_some() {
                    items.push(sound("Continue the level of the one before".into(), SoundMsg::Continue(own)));
                }
                items.push(sound(if mute { "Unmute".into() } else { "Mute".into() }, SoundMsg::Mute(own, !mute)));
                items.push(sound(if even { "Stop evening out".into() } else { "Even out quiet and loud".into() }, SoundMsg::Even1(own, !even)));
                items.push(sound(if low_cut { "Stop cutting the low rumble".into() } else { "Cut the low rumble (wind)".into() }, SoundMsg::LowCut(own, !low_cut)));
                items.push(sound(if clean { "Stop cleaning up the noise".into() } else { "Clean up steady noise".into() }, SoundMsg::Clean(own, !clean)));
                items.push(sound("Normalize…".into(), SoundMsg::Open(Scope::Own(own))));
                items.push(sound("Back to the original".into(), SoundMsg::Original(own)));
                if let Own::Clip(which @ (SoundTrack::Sfx | SoundTrack::Sfx2), i) = own {
                    let to = if which == SoundTrack::Sfx { "SFX 2" } else { "SFX" };
                    items.push(sound(format!("Move to {to}"), SoundMsg::OtherSfx(which, i)));
                }
                // Auto-duck where a song is right-clicked too, not only on
                // the lane's empty stretch or in the Audio panel.
                if let Own::Clip(SoundTrack::Music, _) = own {
                    items.push(item(format!("Lower the music under speech (−{:.0} dB)", self.duck_db), Message::Duck));
                }
                items.push(rule::horizontal(1).into());
                let info = match own {
                    Own::Cut(i) => super::info::Of::Cut(i),
                    Own::Clip(which, i) => super::info::Of::Music(which, i),
                };
                items.push(item("Info".into(), Message::Info(info)));
            }
            Menu::Track(which) => {
                let track = self.edit.track(which);
                if !track.clips.is_empty() {
                    items.push(sound(format!("Normalize every clip on {}…", which.name()), SoundMsg::Open(Scope::Track(which))));
                }
                if which == SoundTrack::Music && !track.clips.is_empty() {
                    items.push(item(format!("Lower the music under speech (−{:.0} dB)", self.duck_db), Message::Duck));
                }
            }
            // On their own, so a misplaced right-click only opens a menu.
            Menu::Point(own, i) => items.push(sound("Delete point".into(), SoundMsg::DeletePoint(own, i))),
            Menu::Line(own) => items.push(sound("Delete the whole line".into(), SoundMsg::DeleteLine(own))),
        }
        if items.is_empty() {
            return below;
        }
        // Its bottom at the pointer (the timeline is low in the window),
        // by its own height: a row is 13 pt text, 5 px above and below.
        let height = items.len() as f32 * 26.0 + 8.0;
        let top = (at.y - height).max(0.0);
        let menu = container(column(items).width(260)).padding(4).style(container::rounded_box);
        let close = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::Sound(SoundMsg::Menu(None))).on_right_press(Message::Sound(SoundMsg::Menu(None)));
        stack![below, close, pin(opaque(menu)).position(Point::new(at.x, top))].into()
    }

}

/// What a run changed, in a sentence or two.
fn summary(done: usize, total: usize, gains: &[f64], silent: usize, failed: &[String], cancelled: bool) -> String {
    let cuts = |n: usize| if n == 1 { "clip" } else { "clips" };
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

    fn app() -> App {
        let opts = super::super::Options { osv_size: None, hw: Some(crate::ffmpeg::Hwaccel::Software), hw_forced: true, stabilize: crate::stab::Mode::Horizon, imu_axes: crate::stab::ImuAxes::X180, world_z_down: false };
        let mut app = App::new(opts, super::super::settings::Settings::default());
        // Files that aren't there: the dialog's measuring finds no sound,
        // and the results are fed in below by hand.
        app.edit.cuts = vec![crate::project::Cut::new("nowhere/a.mp4".into(), 0.0, 2.0), crate::project::Cut::new("nowhere/b.mp4".into(), 0.0, 2.0)];
        app
    }

    #[test]
    fn normalizing_every_cut_is_one_undo_step_and_says_what_it_did() {
        let mut app = app();
        let _ = app.sound_ui(SoundMsg::Open(Scope::All));
        let _ = app.sound_ui(SoundMsg::Start);
        assert!(matches!(app.normalize.as_ref().map(|d| &d.state), Some(State::Running { total: 2, .. })));
        let _ = app.sound_ui(SoundMsg::Measured(Own::Cut(0), Ok(Some((-20.0, 4.0)))));
        let _ = app.sound_ui(SoundMsg::Measured(Own::Cut(1), Ok(None)));
        let c = &app.edit.cuts[0];
        assert_eq!((c.gain, c.loudness, c.even), (4.0, Some(-20.0), true), "evened out by default");
        assert!(matches!(app.normalize.as_ref().map(|d| &d.state), Some(State::Done(s)) if s == "2 clips done. Level +4.0 dB. 1 without sound, left as they were."));
        // All of it undone at once.
        assert!(app.edit.undo());
        assert_eq!((app.edit.cuts[0].gain, app.edit.cuts[0].even), (0.0, false));
    }

    #[test]
    fn a_cancelled_run_stops_after_the_one_being_measured() {
        let mut app = app();
        let _ = app.sound_ui(SoundMsg::Open(Scope::All));
        let _ = app.sound_ui(SoundMsg::Start);
        let _ = app.sound_ui(SoundMsg::Cancel);
        let _ = app.sound_ui(SoundMsg::Measured(Own::Cut(0), Ok(Some((-18.0, 2.0)))));
        assert_eq!(app.edit.cuts[1].gain, 0.0, "the second never measured");
        assert!(matches!(app.normalize.as_ref().map(|d| &d.state), Some(State::Done(s)) if s.starts_with("Stopped after 1 of 2 clips")));
        // A sound clip's track, normalized the same way.
        app.edit.voice.clips.push(crate::project::MusicClip { input: "nowhere/take.wav".into(), at: 0.0, start: 0.0, end: 3.0, ..Default::default() });
        let _ = app.sound_ui(SoundMsg::Close);
        let _ = app.sound_ui(SoundMsg::Open(Scope::Track(SoundTrack::Voice)));
        let _ = app.sound_ui(SoundMsg::Start);
        let _ = app.sound_ui(SoundMsg::Measured(Own::Clip(SoundTrack::Voice, 0), Ok(Some((-30.0, 14.0)))));
        assert_eq!(app.edit.voice.clips[0].gain, 14.0);
    }

    #[test]
    fn says_what_changed() {
        assert_eq!(summary(3, 3, &[2.6, 7.7, 0.7], 0, &[], false), "3 clips done. Levels from +0.7 to +7.7 dB.");
        assert_eq!(summary(1, 1, &[16.1], 0, &[], false), "1 clip done. Level +16.1 dB.");
        assert_eq!(summary(2, 39, &[1.0, 2.0], 1, &[], true), "Stopped after 2 of 39 clips. Levels from +1.0 to +2.0 dB. 1 without sound, left as they were.");
    }
}
