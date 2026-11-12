//! Sound by hand: the «Normalize» dialog, and the menu a right-click opens
//! on a cut's sound (the Clip sound lane), on a sound clip, or on a sound
//! lane's empty stretch (the track's own menu). A cut's sound and a sound
//! clip's are changed alike, through `project::Own` (gap 33f).
//!
//! Normalizing measures what each cut plays (`loudness::measure_stretch`) and
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

use super::{App, Message, Selection, blocking, tip};
use crate::model::ops::sound::{SoundOp, VOICE_OVER_DIP, normal_gain};
use crate::model::ops::{Op, SoundRef};
use crate::project::{Id, Own, SoundTrack};
use iced::widget::{button, checkbox, column, container, mouse_area, opaque, pin, progress_bar, radio, row, rule, space, stack, text, tooltip};
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
    /// The clips' own sound as a lane (its header).
    ClipSound,
    Point(Own, usize),
    Line(Own),
    /// A cut that's one of a clip's pieces («This was one clip»).
    Pieces(usize),
    /// A text or picture lane's head: its name and its sync lock.
    Lane(crate::overlay::Track, usize),
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
    Running { todo: Vec<SoundRef>, total: usize, done: usize, gains: Vec<f64>, silent: usize, failed: Vec<String>, cancelled: bool },
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
    Measured(SoundRef, Result<Option<(f64, f64)>, String>),
    Cancel,
    Close,
    /// A right-click menu, where the pointer is (window coordinates), or
    /// closed.
    Menu(Option<(Menu, Point)>),
    /// Its level down for a voice-over; back to its normal level.
    Lower(Own),
    Restore(Own),
    Mute(Own, bool),
    /// Its volume line shown (`SoundOp::ShowLine`).
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
    /// Cut `i`'s sound back to its picture's edges (no J- or L-cut).
    ToPicture(usize),
    /// A volume line's point `i` removed (Ctrl+click on it, or its menu).
    DeletePoint(Own, usize),
    /// A volume line removed, all its points.
    DeleteLine(Own),
}

impl App {
    /// The Normalize dialog's messages: opened, set up, run, measured,
    /// stopped, closed.
    fn on_normalize(&mut self, msg: SoundMsg) -> Task<Message> {
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
                // By id: an edit while it runs doesn't send a result to
                // another cut. One undo step for the whole run (`begin`,
                // ended in `measure_next`); each is applied as it's
                // measured, so a stopped run leaves the rest as they were.
                let todo: Vec<SoundRef> = todo.into_iter().filter_map(|o| SoundRef::of(o, &self.edit)).collect();
                self.edit.begin();
                let total = todo.len();
                d.state = State::Running { todo: todo.into_iter().rev().collect(), total, done: 0, gains: Vec::new(), silent: 0, failed: Vec::new(), cancelled: false };
                return self.measure_next();
            }
            SoundMsg::Measured(sound, result) => {
                let even = self.normalize.as_ref().is_some_and(|d| d.even);
                if let Ok(measured) = &result {
                    // Gone since (deleted mid-run): nothing to apply it to.
                    let _ = self.edit.apply(&Op::Sound(SoundOp::Normalized { sound, lufs: measured.map(|(lufs, _)| lufs), even }));
                }
                let name = sound.own(&self.edit).and_then(|own| self.edit.own_file(own, false)).map(|(f, ..)| super::clip_name(&f)).unwrap_or_default();
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
            _ => {}
        }
        Task::none()
    }

    pub(super) fn sound_ui(&mut self, msg: SoundMsg) -> Task<Message> {
        match msg {
            m @ (SoundMsg::Open(_) | SoundMsg::Scope(_) | SoundMsg::Even(_) | SoundMsg::Start | SoundMsg::Measured(..) | SoundMsg::Cancel | SoundMsg::Close) => return self.on_normalize(m),
            SoundMsg::Menu(at) => {
                // A cut's sound: the voices Dailix made for its clip, read now.
                self.menu_voices = match at {
                    Some((Menu::Own(Own::Cut(i)), _)) => self.cut_voices(i),
                    _ => Default::default(),
                };
                self.sound_menu = at;
            }
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
            SoundMsg::ToPicture(i) => {
                self.sound_menu = None;
                if let Some(cut) = self.edit.cuts.get(i).map(|c| c.id) {
                    self.op(Op::Cut(crate::model::ops::cuts::CutOp::SoundToPicture { cut }));
                }
            }
            SoundMsg::OtherSfx(from, i) => {
                self.sound_menu = None;
                let Some(clip) = self.edit.track(from).clips.get(i).map(|c| c.id) else { return Task::none() };
                if self.edit.apply(&Op::Clip(crate::model::ops::clips::ClipOp::OtherSfx { clip })).is_ok()
                    && let Some((w, j)) = self.edit.clip_index(clip)
                {
                    self.selection = Selection::clip_at(&self.edit, w, j);
                }
            }
            SoundMsg::Continue(own) => self.sound_op(own, |sound| SoundOp::ContinueLevel { sound }),
            SoundMsg::DeletePoint(own, i) => {
                self.sound_menu = None;
                if let Some(t) = self.edit.own_line(own).and_then(|(keys, _)| keys.get(i)).map(|k| k.t) {
                    self.sound_op(own, |sound| SoundOp::DeletePoint { sound, t });
                }
                if self.selection.point(&self.edit).is_some() {
                    self.selection = Selection::None;
                }
            }
            SoundMsg::DeleteLine(own) => {
                self.sound_menu = None;
                if self.selection.point(&self.edit).is_some() {
                    self.selection = Selection::None;
                }
                self.sound_op(own, |sound| SoundOp::DeleteLine { sound });
            }
            SoundMsg::Even1(own, on) => {
                // Evening out changes how loud it is: measured again, as
                // normalizing would.
                if on || self.edit.own_loudness(own).is_some() {
                    self.sound_menu = None;
                    self.normalize = Some(Dialog { scope: Scope::Own(own), even: on, state: State::Ready });
                    return self.sound_ui(SoundMsg::Start);
                }
                self.sound_op(own, |sound| SoundOp::Normalized { sound, lufs: None, even: on });
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

    /// Starts measuring the next cut, or ends the run with what changed.
    fn measure_next(&mut self) -> Task<Message> {
        let Some(Dialog { state, even, .. }) = self.normalize.as_mut() else { return Task::none() };
        let even = *even;
        let State::Running { todo, total, gains, silent, failed, cancelled, done } = state else { return Task::none() };
        let next = if *cancelled { None } else { todo.pop() };
        let Some(sound) = next else {
            *state = State::Done(summary(*done, *total, gains, *silent, failed, *cancelled));
            self.edit.commit();
            return Task::none();
        };
        // Measured as it will sound: evened out or not as asked.
        let Some((file, range, filter)) = sound.own(&self.edit).and_then(|own| self.edit.own_file(own, even)) else {
            return Task::done(Message::Sound(SoundMsg::Measured(sound, Err("gone from the edit".into()))));
        };
        Task::perform(blocking(move || crate::loudness::measure_stretch(&file, range, filter.as_deref()).map_err(|e| format!("{e:#}"))), move |r| Message::Sound(SoundMsg::Measured(sound, r)))
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
                    _ => self.selection.cut(&self.edit).filter(|&i| i < self.edit.cuts.len()).map(Own::Cut),
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
                    text(format!("Speech to {:.0} LUFS; peaks held under −1 dB.", crate::model::level::TARGET_LUFS)).size(13).style(text::secondary),
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
        stack![below, super::ask::modal(card, None)].into()
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
        // The menu's rows, by kind, for its height: items, rules, headings.
        let mut rows = (0usize, 0usize, 0usize);
        match menu {
            Menu::Own(own) => {
                let Some(groups) = own_entries(&self.edit, own, self.duck_db) else { return below };
                for (heading, group) in groups {
                    push_group(&mut items, &mut rows, heading, group.into_iter().map(|(label, msg)| item(label, msg)).collect(), 0);
                }
                // A cut's sound: its own or a cleaned voice, and whether its
                // own is speech (gap 103).
                if let Own::Cut(i) = own
                    && let Some(c) = self.edit.cuts.get(i)
                {
                    let voices = voice_entries(c, &self.menu_voices).into_iter().map(|(label, msg)| item(label, msg)).collect();
                    push_group(&mut items, &mut rows, "Voice", voices, 0);
                }
                // A song's beats, under their heading (gap 13): per song, not
                // per track (the user, 2026-11-11).
                if let Own::Clip(SoundTrack::Music, i) = own
                    && let Some(c) = self.edit.music.clips.get(i)
                {
                    let mut beats = beat_entries(c.id, c.beats);
                    if let Some(shift) = self.beat_pull(i) {
                        beats.push(item(format!("Pull the nearest beat to the playhead ({:+.0} ms)", shift * 1000.0), Message::PullBeat(i)));
                    }
                    // Its slider is about two rows tall.
                    push_group(&mut items, &mut rows, "Beats", beats, 1);
                }
            }
            Menu::Track(which) => {
                let track = self.edit.track(which);
                items.push(lane_level(which.name(), track.volume, move |db| Message::TrackLevel(which, db)));
                let lock = button(text(format!("{}Sync lock", if track.sync { "✓ " } else { "" })).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(Message::TrackSync(which, !track.sync));
                items.push(tip(lock, text("Trimming, deleting or adding a cut keeps what plays under each picture with it: this lane's clips are cut where the picture is and move with it"), tooltip::Position::Right));
                if !track.clips.is_empty() {
                    items.push(sound(format!("Normalize every clip on {}…", which.name()), SoundMsg::Open(Scope::Track(which))));
                }
                let past: Vec<Id> = self.edit.past_end().into_iter().map(|(id, _)| id).filter(|&id| self.edit.clip_index(id).is_some_and(|(w, _)| w == which)).collect();
                if !past.is_empty() {
                    items.push(item(format!("Trim to the film's end ({})", past.len()), Message::TrimToEnd(past)));
                }
                if which == SoundTrack::Music && !track.clips.is_empty() {
                    items.push(item(format!("Lower the music under speech (−{:.0} dB)", self.duck_db), Message::Duck));
                }
            }
            Menu::ClipSound => {
                items.push(lane_level("Clip sound", self.edit.sound.volume, Message::SoundLevel));
                if !self.edit.cuts.is_empty() {
                    items.push(sound(format!("Normalize every cut ({})…", self.edit.cuts.len()), SoundMsg::Open(Scope::All)));
                }
            }
            // On their own, so a misplaced right-click only opens a menu.
            Menu::Point(own, i) => items.push(sound("Delete point".into(), SoundMsg::DeletePoint(own, i))),
            Menu::Line(own) => items.push(sound("Delete the whole line".into(), SoundMsg::DeleteLine(own))),
            Menu::Lane(track, lane) => {
                let sync = self.edit.lane(track, lane).is_some_and(|l| l.sync);
                items.push(item("Rename…".into(), Message::LaneRename(track, lane)));
                let lock = button(text(format!("{}Sync lock", if sync { "✓ " } else { "" })).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(Message::LaneSync(track, lane, !sync));
                items.push(tip(lock, text("Trimming, deleting or adding a cut keeps the text and pictures over each picture with it: shortened where it goes"), tooltip::Position::Right));
            }
            Menu::Pieces(i) => {
                items.push(item(format!("Join the pieces back ({})", self.edit.pieces(i).len()), Message::JoinPieces(i)));
                items.push(item("Info".into(), Message::Info(super::info::Of::Cut(i))));
            }
        }
        if items.is_empty() {
            return below;
        }
        // Its bottom at the pointer (the timeline is low in the window),
        // by its own height: an item is 13 pt text, 5 px above and below; a
        // heading is smaller, a rule thin. Menus without groups count their
        // items alone.
        if rows == (0, 0, 0) {
            rows.0 = items.len();
        }
        let height = rows.0 as f32 * 26.0 + rows.1 as f32 * 2.0 + rows.2 as f32 * 22.0 + 8.0;
        let top = (at.y - height).max(0.0);
        let menu = container(column(items).width(260)).padding(4).style(container::rounded_box);
        let close = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::Sound(SoundMsg::Menu(None))).on_right_press(Message::Sound(SoundMsg::Menu(None)));
        stack![below, close, pin(opaque(menu)).position(Point::new(at.x, top))].into()
    }

}

/// A menu group put in: a rule before it (not at the top), its heading if
/// it has one, its entries; `rows` counts them for the menu's height
/// (items, rules, headings), `taller` the rows its entries take beyond one
/// each (a slider).
fn push_group<'a>(items: &mut Vec<Element<'a, Message>>, rows: &mut (usize, usize, usize), heading: &'static str, group: Vec<Element<'a, Message>>, taller: usize) {
    if !items.is_empty() {
        items.push(rule::horizontal(1).into());
        rows.1 += 1;
    }
    if !heading.is_empty() {
        items.push(container(text(heading).size(11).style(text::secondary)).width(Length::Fill).padding([4, 10]).into());
        rows.2 += 1;
    }
    rows.0 += group.len() + taller;
    items.extend(group);
}

/// A cut's «Voice» entries (gap 103): its own sound, each voice Dailix
/// made for its clip (the marking's choice named), the one it plays
/// ticked; and «Its own sound is speech».
fn voice_entries(c: &crate::project::Cut, voices: &crate::from_sheet::Voices) -> Vec<(String, Message)> {
    let tick = |on: bool, label: &str| format!("{}{label}", if on { "✓ " } else { "" });
    let mut entries = vec![(tick(c.audio.is_none(), "Its own sound"), Message::CutVoice(c.id, None))];
    for (name, file) in &voices.made {
        let chosen = if voices.chosen.as_deref() == Some(name.as_str()) { " · chosen in Dailix" } else { "" };
        entries.push((tick(c.audio.as_deref() == Some(file.as_path()), &format!("Cleaned voice {name}{chosen}")), Message::CutVoice(c.id, Some(file.clone()))));
    }
    entries.push((tick(c.speech, "Its own sound is speech"), Message::CutSpeech(c.id, !c.speech)));
    entries
}

impl App {
    /// The voices Dailix made for cut `i`'s clip, if this edit is a trip's.
    fn cut_voices(&self, i: usize) -> crate::from_sheet::Voices {
        let root = self.project.as_deref().and_then(dailix_sheet::project::of_edit);
        match (root, self.edit.cuts.get(i)) {
            (Some(root), Some(c)) => crate::from_sheet::voices(&root, &c.input),
            _ => Default::default(),
        }
    }

    /// Voices chosen in Dailix since the edit was made (gap 103), asked
    /// when it opens: none asked when there are none.
    pub(super) fn offer_sheet_voices(&mut self) -> Task<Message> {
        let Some(root) = self.project.as_deref().and_then(dailix_sheet::project::of_edit) else { return Task::none() };
        let n = crate::from_sheet::newer_voices(&root, &self.edit).len();
        if n == 0 {
            return Task::none();
        }
        self.ask(super::ask::Ask {
            title: "Voices chosen in Dailix".to_owned(),
            text: format!("{n} cut{} of this edit got a cleaned voice chosen in Dailix since it was made. Use {}?", if n == 1 { "" } else { "s" }, if n == 1 { "it" } else { "them" }),
            answers: vec![("Use them".to_owned(), Message::SheetVoices(Some(true))), ("Keep as it is".to_owned(), Message::SheetVoices(Some(false)))],
            cancel: Message::SheetVoices(None),
            field: None,
        })
    }

    /// The answer: the voices measured over their cuts and set, one undo
    /// step, or let pass so they aren't offered again.
    fn on_sheet_voices(&mut self, message: Message) -> Task<Message> {
        use crate::model::ops::cuts::CutOp;
        match message {
            Message::SheetVoices(Some(take)) => {
                let Some(root) = self.project.as_deref().and_then(dailix_sheet::project::of_edit) else { return Task::none() };
                let newer = crate::from_sheet::newer_voices(&root, &self.edit);
                if !take {
                    let ops: Vec<Op> = newer.into_iter().map(|(cut, name, _)| Op::Cut(CutOp::SheetVoice { cut, name: Some(name) })).collect();
                    if let Err((_, e)) = self.edit.batch(&ops) {
                        self.error = Some(e.to_string());
                    }
                    return Task::none();
                }
                let spans: Vec<_> = newer.into_iter().filter_map(|(cut, name, file)| self.edit.cut_index(cut).map(|i| (cut, file, name, (self.edit.cuts[i].start, self.edit.cuts[i].end)))).collect();
                let measure = move || spans.into_iter().map(|(cut, file, name, span)| (cut, file.clone(), name, crate::from_sheet::voice_loudness(&file, span))).collect();
                return Task::perform(blocking(measure), Message::SheetVoicesMeasured);
            }
            Message::SheetVoicesMeasured(found) => {
                // A cut gone meanwhile left out, not the whole batch refused.
                let ops: Vec<Op> = found.into_iter().filter(|f| self.edit.cut_index(f.0).is_some()).flat_map(|(cut, file, name, lufs)| [Op::Cut(CutOp::Voice { cut, audio: Some(file), lufs }), Op::Cut(CutOp::SheetVoice { cut, name: Some(name) })]).collect();
                let at = self.position();
                match self.edit.batch(&ops) {
                    Ok(_) => self.after_edit(at),
                    Err((_, e)) => self.error = Some(e.to_string()),
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// A cut's voice chosen (measured over the cut first, then set) or its
    /// own sound marked as speech; the menu closes (gap 103).
    pub(super) fn on_cut_voice(&mut self, message: Message) -> Task<Message> {
        use crate::model::ops::cuts::CutOp;
        self.sound_menu = None;
        match message {
            Message::CutVoice(cut, Some(file)) => {
                self.voice_wanted = Some((cut, Some(file.clone())));
                let Some(c) = self.edit.cut_index(cut).map(|i| &self.edit.cuts[i]) else { return Task::none() };
                let span = (c.start, c.end);
                let measured = file.clone();
                return Task::perform(blocking(move || crate::from_sheet::voice_loudness(&measured, span)), move |lufs| Message::CutVoiceMeasured(cut, Some(file.clone()), lufs));
            }
            Message::CutVoice(cut, None) => {
                self.voice_wanted = Some((cut, None));
                return self.on_cut_voice(Message::CutVoiceMeasured(cut, None, None));
            }
            Message::CutVoiceMeasured(cut, audio, _) if self.voice_wanted.as_ref() != Some(&(cut, audio.clone())) => {}
            Message::CutVoiceMeasured(cut, audio, lufs) => {
                self.voice_wanted = None;
                let at = self.position();
                if self.op(Op::Cut(CutOp::Voice { cut, audio, lufs })).is_some() {
                    // The player heard the old sound: opened again.
                    self.after_edit(at);
                }
            }
            Message::CutSpeech(cut, on) => {
                self.op(Op::Cut(CutOp::Speech { cut, on }));
            }
            m @ (Message::SheetVoices(_) | Message::SheetVoicesMeasured(_)) => return self.on_sheet_voices(m),
            _ => {}
        }
        Task::none()
    }
}

/// A group of a menu: its heading (none: empty) and each entry's words
/// and what it does.
type MenuGroup = (&'static str, Vec<(String, Message)>);

/// The right-click menu of `own`'s sound, in groups under their
/// headings (gap 47), the empty ones left out: each entry's words and
/// what it does. A toggle keeps its words and shows its state by a tick.
/// None if `own` isn't there.
fn own_entries(p: &crate::project::Project, own: Own, duck_db: f64) -> Option<Vec<MenuGroup>> {
    let (keys, _) = p.own_line(own)?;
    let (gain, mute, even, loudness, low_cut, clean) = match own {
        Own::Cut(i) => p.cuts.get(i).map(|c| (c.gain, c.mute, c.even, c.loudness, c.low_cut, c.clean)),
        Own::Clip(which, i) => p.track(which).clips.get(i).map(|c| (c.gain, c.mute, c.even, c.loudness, c.low_cut, c.clean)),
    }?;
    let sound = |label: String, msg: SoundMsg| (label, Message::Sound(msg));
    let tick = |on: bool, label: &str| format!("{}{label}", if on { "✓ " } else { "" });
    let mut level = vec![sound(format!("Lower for voice-over ({VOICE_OVER_DIP:+.0} dB)"), SoundMsg::Lower(own))];
    if (gain - normal_gain(loudness)).abs() > 0.05 {
        level.push(sound(format!("Back to normal level ({:+.1} dB)", normal_gain(loudness)), SoundMsg::Restore(own)));
    }
    level.push(sound(tick(mute, "Mute"), SoundMsg::Mute(own, !mute)));
    level.push(sound("Normalize…".into(), SoundMsg::Open(Scope::Own(own))));
    let mut line = Vec::new();
    // The line shows once it has points; then a click on it adds one.
    if keys.is_empty() {
        line.push(sound("Show volume line".into(), SoundMsg::ShowLine(own)));
    }
    if p.before(own).is_some() {
        line.push(sound("Continue the level of the one before".into(), SoundMsg::Continue(own)));
    }
    line.push(sound("Back to the original".into(), SoundMsg::Original(own)));
    let clean_up = vec![
        sound(tick(even, "Even out quiet and loud"), SoundMsg::Even1(own, !even)),
        sound(tick(low_cut, "Cut the low rumble (wind)"), SoundMsg::LowCut(own, !low_cut)),
        sound(tick(clean, "Clean up steady noise"), SoundMsg::Clean(own, !clean)),
    ];
    let mut place = Vec::new();
    // A sound split from its picture (a J- or L-cut, or short).
    if let Own::Cut(i) = own
        && p.cuts.get(i).is_some_and(|c| c.sound_before != 0.0 || c.sound_after != 0.0)
    {
        place.push(sound("Sound back to the picture's edges".into(), SoundMsg::ToPicture(i)));
    }
    if let Own::Clip(which @ (SoundTrack::Sfx | SoundTrack::Sfx2), i) = own {
        let to = if which == SoundTrack::Sfx { "SFX 2" } else { "SFX" };
        place.push(sound(format!("Move to {to}"), SoundMsg::OtherSfx(which, i)));
    }
    // Auto-duck where a song is right-clicked too, not only on the lane's
    // empty stretch or in the Audio panel.
    let mut music = Vec::new();
    if let Own::Clip(SoundTrack::Music, _) = own {
        music.push((format!("Lower the music under speech (−{duck_db:.0} dB)"), Message::Duck));
    }
    let info = match own {
        Own::Cut(i) => super::info::Of::Cut(i),
        Own::Clip(which, i) => super::info::Of::Music(which, i),
    };
    let groups = vec![("Level", level), ("Volume line", line), ("Clean up", clean_up), ("Placement", place), ("Music", music), ("", vec![("Info".to_owned(), Message::Info(info))])];
    Some(groups.into_iter().filter(|(_, g)| !g.is_empty()).collect())
}

/// A song's beats in its menu (gap 13): shown or not; shown, how strong a
/// hit must be to show (a slider, one undo step a drag) and snapping.
fn beat_entries<'a>(clip: Id, beats: Option<crate::project::Beats>) -> Vec<Element<'a, Message>> {
    let entry = |label: String, msg: Message| -> Element<'a, Message> { button(text(label).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(msg).into() };
    let tick = |on: bool, label: &str| format!("{}{label}", if on { "✓ " } else { "" });
    let Some(b) = beats else {
        return vec![entry(tick(false, "Show the beats"), Message::ClipBeats(clip, Some(crate::project::Beats { threshold: BEATS_FIRST, snap: false }), true))];
    };
    let slider = column![
        text(format!("Beats at {:.0} % of the strongest and up", b.threshold * 100.0)).size(13),
        iced::widget::slider(0.0..=1.0, b.threshold, move |threshold| Message::ClipBeats(clip, Some(crate::project::Beats { threshold, ..b }), false)).step(0.05).on_release(Message::LevelDone),
    ]
    .spacing(4)
    .padding([5, 10]);
    vec![
        entry(tick(true, "Show the beats"), Message::ClipBeats(clip, None, true)),
        tip(slider, text("Lower: every beat; higher: the strong hits only"), tooltip::Position::Right),
        entry(tick(b.snap, "Snap to the beats"), Message::ClipBeats(clip, Some(crate::project::Beats { snap: !b.snap, ..b }), true)),
    ]
}

/// A threshold the beats start at once shown: about the main beats of a
/// song, the off-beats left out (on the songs tried).
const BEATS_FIRST: f64 = 0.4;

/// A lane's own level in its menu: the name and level, and a slider (the
/// Audio panel's, one undo step a drag).
fn lane_level<'a>(name: &str, db: f64, set: impl Fn(f64) -> Message + 'a) -> Element<'a, Message> {
    column![
        text(format!("{name}: whole lane {}", super::db_label(db))).size(13),
        iced::widget::slider(crate::project::OFF_DB..=6.0, db, set).step(0.5).on_release(Message::LevelDone),
    ]
    .spacing(4)
    .padding([5, 10])
    .into()
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
    use super::super::tests::app;

    #[test]
    fn a_songs_menu_turns_its_beats_on_sets_them_and_snaps_in_steps() {
        use super::super::Message;
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), end: 9.0, ..Default::default() });
        app.edit.settle_ids();
        let song = app.edit.music.clips[0].id;
        let on = crate::project::Beats { threshold: BEATS_FIRST, snap: false };
        let _ = app.update(Message::ClipBeats(song, Some(on), true));
        // A drag of the threshold, then snapping pressed.
        for threshold in [0.5, 0.6, 0.7] {
            let _ = app.update(Message::ClipBeats(song, Some(crate::project::Beats { threshold, ..on }), false));
        }
        let _ = app.update(Message::LevelDone);
        let _ = app.update(Message::ClipBeats(song, Some(crate::project::Beats { threshold: 0.7, snap: true }), true));
        let beats = |app: &App| app.edit.music.clips[0].beats;
        assert_eq!(beats(&app), Some(crate::project::Beats { threshold: 0.7, snap: true }));
        assert!(app.edit.undo() && beats(&app).is_some_and(|b| !b.snap && b.threshold == 0.7), "snapping its own step");
        assert!(app.edit.undo() && beats(&app) == Some(on), "the drag one step");
        assert_eq!(beat_entries(song, None).len(), 1, "off: «Show the beats» alone");
        assert_eq!(beat_entries(song, Some(on)).len(), 3);
    }

    #[test]
    fn the_music_lanes_sync_lock_keeps_the_song_under_the_cuts() {
        use super::super::Message;
        use crate::model::ops::{End, Op, cuts::CutOp};
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), end: 9.0, ..Default::default() });
        let _ = app.update(Message::TrackSync(SoundTrack::Music, true));
        assert!(app.edit.music.sync);
        let cut = app.edit.cuts[0].id;
        app.op(Op::Cut(CutOp::Trim { cut, end: End::In, t: 0.5, most: 2.0 }));
        assert_eq!((app.edit.music.clips[0].at, app.edit.music.clips[0].start), (0.0, 0.5), "the song gave up its head with the picture");
        assert!(app.edit.undo() && app.edit.music.clips[0].start == 0.0, "one undo step with the trim");
    }

    #[test]
    fn a_songs_beat_is_pulled_onto_the_playhead_and_nudged_by_keys() {
        use super::super::Message;
        use crate::beats::Beat;
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 0.0, end: 2.0, beats: Some(crate::project::Beats { threshold: 0.0, snap: true }), ..Default::default() });
        app.edit.settle_ids();
        app.hits.insert("song.wav".into(), Some(std::sync::Arc::new(vec![Beat { t: 0.013, strength: 1.0 }])));
        // The playhead at the edit's start (no clip open): the beat 13 ms in.
        assert!((app.beat_pull(0).unwrap() + 0.013).abs() < 1e-12);
        let _ = app.update(Message::PullBeat(0));
        assert!(app.edit.music.clips[0].at.abs() < 1e-12, "at the edit's start: it can't go before it");
        app.edit.music.clips[0].at = 1.0;
        app.selection = crate::model::selection::Selection::clip_at(&app.edit, SoundTrack::Music, 0);
        let _ = app.key(iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight), iced::keyboard::Modifiers::ALT);
        let _ = app.key(iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight), iced::keyboard::Modifiers::ALT | iced::keyboard::Modifiers::SHIFT);
        assert!((app.edit.music.clips[0].at - 1.011).abs() < 1e-9, "{}", app.edit.music.clips[0].at);
    }

    #[test]
    fn beats_are_read_once_a_song_shows_them() {
        let mut app = app();
        app.edit.music.clips.push(crate::project::MusicClip { input: "nowhere/song.wav".into(), end: 9.0, ..Default::default() });
        let _ = app.read_beats();
        assert!(app.hits.is_empty(), "off: nothing read");
        app.edit.music.clips[0].beats = Some(crate::project::Beats { threshold: 0.4, snap: false });
        let _ = app.read_beats();
        assert_eq!(app.hits.get(std::path::Path::new("nowhere/song.wav")), Some(&None), "being read, once");
    }

    #[test]
    fn the_sound_menu_offers_what_fits_the_sound() {
        let mut app = app();
        app.edit.cuts[1].sound_after = 0.5;
        app.edit.cuts[1].mute = true;
        app.edit.sfx.clips.push(crate::project::MusicClip { input: "bell.wav".into(), end: 1.0, ..Default::default() });
        app.edit.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), end: 9.0, ..Default::default() });
        let labels = |own| own_entries(&app.edit, own, 18.0).unwrap().into_iter().map(|(h, g)| (h, g.into_iter().map(|(l, _)| l).collect::<Vec<_>>())).collect::<Vec<_>>();
        let has = |own, words: &str| labels(own).iter().any(|(_, g)| g.iter().any(|l| l == words));
        assert!(has(Own::Cut(1), "Sound back to the picture's edges") && has(Own::Cut(1), "✓ Mute") && !has(Own::Cut(0), "Sound back to the picture's edges"));
        assert!(has(Own::Clip(SoundTrack::Sfx, 0), "Move to SFX 2") && has(Own::Clip(SoundTrack::Music, 0), "Lower the music under speech (−18 dB)"));
        assert_eq!(labels(Own::Cut(0)).iter().map(|(h, _)| *h).collect::<Vec<_>>(), ["Level", "Volume line", "Clean up", ""], "empty groups left out");
        assert!(own_entries(&app.edit, Own::Cut(9), 18.0).is_none());
    }

    #[test]
    fn normalizing_every_cut_is_one_undo_step_and_says_what_it_did() {
        let mut app = app();
        let _ = app.sound_ui(SoundMsg::Open(Scope::All));
        let _ = app.sound_ui(SoundMsg::Start);
        assert!(matches!(app.normalize.as_ref().map(|d| &d.state), Some(State::Running { total: 2, .. })));
        let _ = app.sound_ui(SoundMsg::Measured(SoundRef::Cut(1), Ok(Some((-20.0, 4.0)))));
        let _ = app.sound_ui(SoundMsg::Measured(SoundRef::Cut(2), Ok(None)));
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
        let _ = app.sound_ui(SoundMsg::Measured(SoundRef::Cut(1), Ok(Some((-18.0, 2.0)))));
        assert_eq!(app.edit.cuts[1].gain, 0.0, "the second never measured");
        assert!(matches!(app.normalize.as_ref().map(|d| &d.state), Some(State::Done(s)) if s.starts_with("Stopped after 1 of 2 clips")));
        // A sound clip's track, normalized the same way.
        app.edit.voice.clips.push(crate::project::MusicClip { input: "nowhere/take.wav".into(), at: 0.0, start: 0.0, end: 3.0, ..Default::default() });
        app.edit.settle_ids();
        let _ = app.sound_ui(SoundMsg::Close);
        let _ = app.sound_ui(SoundMsg::Open(Scope::Track(SoundTrack::Voice)));
        let _ = app.sound_ui(SoundMsg::Start);
        let _ = app.sound_ui(SoundMsg::Measured(SoundRef::Clip(3), Ok(Some((-30.0, 14.0)))));
        assert_eq!(app.edit.voice.clips[0].gain, 14.0);
    }

    #[test]
    fn says_what_changed() {
        assert_eq!(summary(3, 3, &[2.6, 7.7, 0.7], 0, &[], false), "3 clips done. Levels from +0.7 to +7.7 dB.");
        assert_eq!(summary(1, 1, &[16.1], 0, &[], false), "1 clip done. Level +16.1 dB.");
        assert_eq!(summary(2, 39, &[1.0, 2.0], 1, &[], true), "Stopped after 2 of 39 clips. Levels from +1.0 to +2.0 dB. 1 without sound, left as they were.");
    }

    #[test]
    fn a_lanes_menu_trims_what_runs_past_the_film() {
        use super::super::Message;
        let mut app = app();
        app.edit.sfx.clips.push(crate::project::MusicClip { input: "boom.wav".into(), at: 3.0, end: 2.0, ..Default::default() });
        app.edit.settle_ids();
        let boom = app.edit.sfx.clips[0].id;
        assert_eq!(app.edit.past_end(), [(boom, 1.0)], "the film is 4 s");
        let _ = app.update(Message::TrimToEnd(vec![boom]));
        assert_eq!(app.edit.sfx.clips[0].ends(), 4.0);
        assert!(app.edit.past_end().is_empty());
    }

    #[test]
    fn a_pieces_menu_joins_them_back() {
        use super::super::Message;
        let mut app = app();
        let a = app.edit.cuts[0].id;
        app.op(Op::Cut(crate::model::ops::cuts::CutOp::Split { cut: a, t: 1.0 }));
        assert_eq!(app.edit.pieces(1), [0, 1]);
        app.sound_menu = Some((Menu::Pieces(1), iced::Point::ORIGIN));
        let _ = app.update(Message::JoinPieces(1));
        assert_eq!((app.edit.cuts.len(), app.edit.cuts[0].end, app.sound_menu), (2, 2.0, None));
    }

    #[test]
    fn a_cuts_voice_menu_offers_its_voices_and_sets_one() {
        use super::super::Message;
        let mut app = app();
        let cut = app.edit.cuts[0].id;
        let voices = crate::from_sheet::Voices { made: vec![("dnn6".into(), "v/dnn6.flac".into()), ("eq".into(), "v/eq.flac".into())], chosen: Some("eq".into()) };
        let labels: Vec<String> = voice_entries(&app.edit.cuts[0], &voices).into_iter().map(|(l, _)| l).collect();
        assert_eq!(labels, ["✓ Its own sound", "Cleaned voice dnn6", "Cleaned voice eq · chosen in Dailix", "Its own sound is speech"]);
        // Chosen, then measured; an older choice measured later is dropped.
        let _ = app.update(Message::CutVoice(cut, Some("v/dnn6.flac".into())));
        let _ = app.update(Message::CutVoice(cut, Some("v/eq.flac".into())));
        let _ = app.update(Message::CutVoiceMeasured(cut, Some("v/dnn6.flac".into()), Some(-20.0)));
        assert!(app.edit.cuts[0].audio.is_none(), "dnn6 was chosen before eq");
        let _ = app.update(Message::CutVoiceMeasured(cut, Some("v/eq.flac".into()), Some(-26.0)));
        let _ = app.update(Message::CutSpeech(cut, true));
        let c = &app.edit.cuts[0];
        assert_eq!((c.audio.as_deref(), c.loudness, c.speech), (Some(std::path::Path::new("v/eq.flac")), Some(-26.0), true));
        assert!(voice_entries(c, &voices)[2].0.starts_with("✓ "), "the one it plays ticked");
    }

    #[test]
    fn voices_chosen_in_dailix_are_set_in_one_step_and_not_offered_again() {
        use super::super::Message;
        let mut app = app();
        let cut = app.edit.cuts[0].id;
        let _ = app.update(Message::SheetVoicesMeasured(vec![(cut, "v/dnn6.flac".into(), "dnn6".into(), Some(-26.0))]));
        let c = &app.edit.cuts[0];
        assert_eq!((c.audio.as_deref(), c.sheet_voice.as_deref()), (Some(std::path::Path::new("v/dnn6.flac")), Some("dnn6")));
        assert!(app.edit.undo() && app.edit.cuts[0].audio.is_none() && app.edit.cuts[0].sheet_voice.is_none(), "one undo step");
    }
}
