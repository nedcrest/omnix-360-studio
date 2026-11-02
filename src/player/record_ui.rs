//! The Record panel: a voice-over recorded straight onto the Voice lane
//! (gap 36), opened by the microphone button in the top bar.
//!
//! While the panel is open, the chosen input is listened to for the meter
//! alone. Record starts ffmpeg writing the take at once (so its start-up
//! is over by the time it matters), counts three seconds, then plays the
//! edit from the playhead. Stop finishes the file, puts the take on the
//! Voice lane where the playback started, its lead-in left out, as one
//! undo step, and takes the playhead back there.
//!
//! The take lines up with the picture by clocks: when the first sound came
//! in (`Recorder::first_sound`) against when the edit was at the playhead
//! (taken from the playback itself, which starts a moment after Play),
//! plus the delay of the sound out and back in (`latency`).
//!
//! The user's design (2026-10-29): input picker with its kind, warnings
//! for Bluetooth and webcam mics, a live meter, channel choice, countdown,
//! play along on headphones, and takes in `02_audio/voiceover`.

use super::{App, Message, Selection, Side, bin, tip};
use crate::capture::{Channel, Input, Kind, Level, Recorder};
use crate::project::SoundTrack;
use iced::widget::{button, checkbox, column, container, pick_list, progress_bar, row, rule, scrollable, slider, text, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const WIDTH: f32 = 300.0;

/// The countdown before recording.
const COUNTDOWN: Duration = Duration::from_secs(3);

/// The meter's floor, dBFS.
const FLOOR_DB: f32 = -60.0;

/// How long a level reading covers (`astats` reset every 5 frames of 10
/// ms): the first sound came in about this much before its level did.
const LEVEL_WINDOW: f64 = 0.05;

/// The sound's way out to the ears and back in from the mic, seconds: what
/// a take is shifted by so the voice lands where it was spoken to.
/// Measured figures are typical, not this machine's: a USB interface's
/// round trip is a few tens of milliseconds, a Bluetooth headset's a
/// quarter of a second.
fn latency(kind: Kind) -> f64 {
    match kind {
        Kind::Bluetooth => 0.25,
        _ => 0.04,
    }
}

/// The panel's state; recorders run only while it's open.
#[derive(Default)]
pub struct Panel {
    inputs: Vec<Input>,
    /// The chosen input's source name; the default if `None` or gone.
    pub input: Option<String>,
    pub channel: Channel,
    pub countdown: bool,
    pub play: bool,
    /// The chosen input's own volume in the system, percent, as read when
    /// it was chosen; `None` if it couldn't be.
    volume: Option<u32>,
    /// Listening for the meter, between takes.
    meter: Option<Recorder>,
    take: Option<Take>,
    /// Stopping: the file being finished and placed.
    stopping: bool,
    error: Option<String>,
}

/// A take being recorded.
struct Take {
    rec: Recorder,
    /// The playhead when Record was pressed: where the take goes.
    at: f64,
    /// When the countdown ends.
    rolls: Instant,
    /// The playback was started (once: at the edit's end it won't go).
    started: bool,
    /// When the edit was at `at`, by the playback's own clock; `rolls` if
    /// nothing plays.
    anchor: Option<Instant>,
    kind: Kind,
}

#[derive(Clone, Debug)]
pub enum RecordMsg {
    /// The panel opened or closed by the microphone button.
    Toggle,
    Inputs(Result<Vec<Input>, String>),
    Pick(Input),
    Channel(Channel),
    /// The input's own volume, percent.
    Volume(u32),
    Countdown(bool),
    Play(bool),
    Record,
    Stop,
    /// The take's file finished and its length read: the file, its length,
    /// where it goes on the edit, and its in point.
    Stopped(Result<(PathBuf, f64), String>, f64, f64),
    ClearClip,
    /// A screen frame while the panel is open: the meter, the countdown.
    Tick,
}

impl Panel {
    pub fn new(input: Option<String>, channel: Channel, countdown: bool, play: bool) -> Panel {
        Panel { input, channel, countdown, play, ..Default::default() }
    }

    /// The input to record from: the chosen one if it's there, else the
    /// default (listed first).
    fn chosen(&self) -> Option<&Input> {
        self.input.as_ref().and_then(|name| self.inputs.iter().find(|i| &i.name == name)).or(self.inputs.first())
    }

    pub fn recording(&self) -> bool {
        self.take.is_some()
    }

    /// Anything listening to an input.
    pub fn active(&self) -> bool {
        self.take.is_some() || self.meter.is_some()
    }

    /// Whatever is listening, the take's recorder or the meter's.
    fn listening(&self) -> Option<&Recorder> {
        self.take.as_ref().map(|t| &t.rec).or(self.meter.as_ref())
    }

    /// Listens to the chosen input for the meter (again, after a change),
    /// and reads its volume.
    fn listen(&mut self) {
        self.meter = None;
        if self.take.is_some() || self.stopping {
            return;
        }
        self.volume = self.chosen().and_then(|i| crate::capture::input_volume(i).ok());
        if let Some(input) = self.chosen() {
            match Recorder::start(input, self.channel, None) {
                Ok(rec) => self.meter = Some(rec),
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
    }
}

/// The next take's file in `dir`: `VO_<date>_<time>_take<NN>.wav`, NN one
/// more than the takes already there.
fn take_file(dir: &Path) -> PathBuf {
    let n = std::fs::read_dir(dir).map_or(0, |d| d.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with("VO_")).count());
    dir.join(format!("VO_{}_take{:02}.wav", chrono::Local::now().format("%Y-%m-%d_%H%M%S"), n + 1))
}

impl App {
    pub(super) fn record_ui(&mut self, msg: RecordMsg) -> Task<Message> {
        match msg {
            RecordMsg::Toggle => {
                self.toggle_side(Side::Record);
                if self.side == Some(Side::Record) {
                    self.record.error = None;
                    // pactl, off the UI thread: the inputs as they are now.
                    return Task::perform(super::blocking(|| crate::capture::inputs().map_err(|e| format!("{e:#}"))), |r| Message::Record(RecordMsg::Inputs(r)));
                }
                return self.close_record();
            }
            RecordMsg::Inputs(Err(e)) => self.record.error = Some(e),
            RecordMsg::Inputs(Ok(inputs)) => {
                self.record.inputs = inputs;
                if self.side == Some(Side::Record) {
                    self.record.listen();
                }
            }
            RecordMsg::Pick(input) => {
                self.record.input = Some(input.name);
                self.record.listen();
            }
            RecordMsg::Channel(c) => {
                self.record.channel = c;
                self.record.listen();
            }
            RecordMsg::Volume(percent) => {
                if let Some(input) = self.record.chosen() {
                    match crate::capture::set_input_volume(input, percent) {
                        Ok(()) => self.record.volume = Some(percent),
                        Err(e) => self.record.error = Some(format!("{e:#}")),
                    }
                }
            }
            RecordMsg::Countdown(on) => self.record.countdown = on,
            RecordMsg::Play(on) => self.record.play = on,
            RecordMsg::ClearClip => {
                if let Some(rec) = self.record.listening() {
                    rec.clear_clip();
                }
            }
            RecordMsg::Record => self.start_take(),
            RecordMsg::Stop => return self.stop_take(),
            RecordMsg::Stopped(result, at, start) => {
                self.record.stopping = false;
                // The meter back on while the panel is open.
                if self.side == Some(Side::Record) {
                    self.record.listen();
                }
                match result {
                    Err(e) => self.record.error = Some(format!("The take wasn't saved: {e}")),
                    Ok((_, len)) if len - start < 0.1 => self.record.error = Some("Nothing was recorded after the countdown.".into()),
                    Ok((path, len)) => {
                        use crate::model::ops::{Op, clips::ClipOp};
                        let take = self.op(Op::Clip(ClipOp::Add { track: SoundTrack::Voice, input: path.clone(), at, start, end: len }));
                        if let Some((w, i)) = take.and_then(|o| o.made).and_then(|id| self.clip_index(id)) {
                            self.selection = Selection::clip_at(&self.edit, w, i);
                        }
                        return self.load_waveform(&path);
                    }
                }
            }
            RecordMsg::Tick => self.record_tick(),
        }
        Task::none()
    }

    /// Stops what the panel runs once it's closed (by its button or by
    /// another panel taking its place); a take being recorded is kept.
    pub(super) fn close_record(&mut self) -> Task<Message> {
        self.record.meter = None;
        if self.record.take.is_some() { self.stop_take() } else { Task::none() }
    }

    fn playing(&self) -> bool {
        self.clip.as_ref().is_some_and(|c| c.playback.playing)
    }

    fn start_take(&mut self) {
        self.record.error = None;
        let Some(root) = self.project.as_deref().and_then(bin::root) else {
            self.record.error = Some("Save the project first: takes go in its folder.".into());
            return;
        };
        let dir = root.join(SoundTrack::Voice.folder());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.record.error = Some(format!("Can't make {}: {e}", dir.display()));
            return;
        }
        let Some(input) = self.record.chosen().cloned() else {
            self.record.error = Some("No sound input found.".into());
            return;
        };
        if self.playing() {
            self.with_playback(super::playback::Playback::toggle);
        }
        // One recorder at a time: the take's feeds the meter now.
        self.record.meter = None;
        match Recorder::start(&input, self.record.channel, Some(&take_file(&dir))) {
            Ok(rec) => {
                let now = Instant::now();
                let rolls = if self.record.countdown { now + COUNTDOWN } else { now };
                self.record.take = Some(Take { rec, at: self.position(), rolls, started: false, anchor: None, kind: input.kind });
                self.record_tick();
            }
            Err(e) => {
                self.record.error = Some(format!("{e:#}"));
                self.record.listen();
            }
        }
    }

    /// The countdown, the playback started when it ends, and the anchor
    /// kept to the playback's clock; a recorder that stopped by itself
    /// reported.
    fn record_tick(&mut self) {
        let now = Instant::now();
        let play = self.record.play && !self.edit.cuts.is_empty();
        let position = self.position();
        let playing = self.playing();
        let Some(take) = self.record.take.as_mut() else {
            if let Some(problem) = self.record.meter.as_mut().and_then(Recorder::problem) {
                self.record.meter = None;
                self.record.error = Some(format!("The input stopped: {problem}"));
            }
            return;
        };
        if let Some(problem) = take.rec.problem() {
            self.record.take = None;
            self.record.error = Some(format!("Recording stopped: {problem}"));
            return;
        }
        if now < take.rolls {
            return;
        }
        if play && !take.started {
            take.started = true;
            if !playing {
                self.follow = true;
                self.with_playback(super::playback::Playback::toggle);
            }
            return;
        }
        // The playback's start takes a moment: the edit's clock, in its
        // first second, says when it was at the playhead. Later, a cut's
        // change could jump it.
        let ahead = position - take.at;
        if take.started && playing && ahead > 0.0 && ahead < 1.0 {
            take.anchor = Some(now - Duration::from_secs_f64(ahead));
        }
    }

    fn stop_take(&mut self) -> Task<Message> {
        let Some(take) = self.record.take.take() else { return Task::none() };
        if self.playing() {
            self.with_playback(super::playback::Playback::toggle);
        }
        if let Some((i, t)) = self.edit.locate(take.at) {
            self.goto(i, t, false);
        }
        // The file's time where the edit was at the playhead.
        let start = match take.rec.first_sound() {
            // The playhead's moment before the first sound (no countdown)
            // counts as none.
            Some(first) => take.anchor.unwrap_or(take.rolls).saturating_duration_since(first).as_secs_f64() + LEVEL_WINDOW + latency(take.kind),
            None => 0.0,
        };
        let at = take.at;
        self.record.stopping = true;
        Task::perform(
            super::blocking(move || -> Result<(PathBuf, f64), String> {
                let file = take.rec.stop().map_err(|e| format!("{e:#}"))?.ok_or("no file")?;
                let len = crate::ffmpeg::duration(&file).map_err(|e| format!("{e:#}"))?;
                Ok((file, len))
            }),
            move |r| Message::Record(RecordMsg::Stopped(r, at, start)),
        )
    }

    pub(super) fn record_panel(&self) -> Element<'_, Message> {
        let p = &self.record;
        let note = |s: String| text(s).size(13).style(text::secondary);
        let busy = p.recording() || p.stopping;
        let chosen = p.chosen();
        let mut body = column![
            text("Voice-over").size(18),
            button(text("Add from disk…")).on_press(Message::AddSound(SoundTrack::Voice)),
            rule::horizontal(1),
            pick_list(p.inputs.clone(), chosen.cloned(), |i| Message::Record(RecordMsg::Pick(i)))
                .placeholder("No sound input found")
                .text_size(13)
                .width(Length::Fill),
        ]
        .spacing(12)
        .padding(12);
        if let Some(warning) = chosen.and_then(|i| i.kind.warning()) {
            body = body.push(text(warning).size(13).style(text::warning));
        }
        if chosen.is_some_and(|i| i.channels >= 2) {
            let pick = |c: Channel| {
                let style = if p.channel == c { button::primary } else { button::secondary };
                button(text(c.to_string()).size(13)).style(style).on_press_maybe((!busy).then_some(Message::Record(RecordMsg::Channel(c))))
            };
            body = body.push(tip(
                row![text("Channel").size(13).width(Length::Fill)].extend(Channel::ALL.map(|c| pick(c).into())).spacing(4).align_y(Alignment::Center),
                text("Which of the input's two channels the mic is on: an interface's mic input is usually 1"),
                tooltip::Position::Bottom,
            ));
        }
        if let Some(volume) = p.volume {
            body = body.push(tip(
                row![
                    text("Gain").size(13).width(48),
                    slider(0.0..=crate::capture::MOST_VOLUME as f64, volume as f64, |v| Message::Record(RecordMsg::Volume(v.round() as u32))).step(1.0),
                    text(format!("{volume}%")).size(13).width(44),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text("The input's own volume, as in the system's sound settings. Above 100% it's digital: the noise rises with the voice"),
                tooltip::Position::Bottom,
            ));
        }
        body = body.push(self.record_meter());
        body = body.push(column![
            checkbox(p.countdown).label("Count down 3 s").on_toggle_maybe((!busy).then_some(|on| Message::Record(RecordMsg::Countdown(on)))),
            tip(
                checkbox(p.play).label("Play the edit while recording").on_toggle_maybe((!busy).then_some(|on| Message::Record(RecordMsg::Play(on)))),
                text("On headphones: through speakers, the mic records the edit too"),
                tooltip::Position::Bottom,
            ),
        ]
        .spacing(8));
        let (label, action) = match &p.take {
            Some(t) if Instant::now() < t.rolls => (format!("{}…  Stop", (t.rolls - Instant::now()).as_secs() + 1), RecordMsg::Stop),
            Some(t) => {
                let s = (Instant::now() - t.rolls).as_secs();
                (format!("Stop  {}:{:02}", s / 60, s % 60), RecordMsg::Stop)
            }
            None => ("Record".to_owned(), RecordMsg::Record),
        };
        let can = !p.stopping && (p.recording() || (chosen.is_some() && self.project.is_some()));
        let record = button(text(label).size(16).width(Length::Fill).center()).style(button::danger).width(Length::Fill).padding(10).on_press_maybe(can.then_some(Message::Record(action)));
        body = body.push(if self.project.is_none() { tip(record, text("Save the project first: takes go in its folder"), tooltip::Position::Bottom) } else { record.into() });
        if let Some(e) = &p.error {
            body = body.push(note(e.clone()).style(text::danger));
        }
        container(scrollable(body)).width(WIDTH).height(Length::Fill).into()
    }

    /// The input's level: a bar to −60 dBFS, the peak in numbers, and a
    /// CLIP light that stays on until clicked.
    fn record_meter(&self) -> Element<'_, Message> {
        let level = self.record.listening().map(Recorder::level).unwrap_or(Level::default());
        let peak = (level.peak as f32).max(FLOOR_DB);
        let db = if level.peak.is_finite() { format!("{:.0} dB", level.peak) } else { "—".to_owned() };
        let clip = button(text("CLIP").size(12)).style(if level.clipped { button::danger } else { button::text }).padding([2, 6]).on_press_maybe(level.clipped.then_some(Message::Record(RecordMsg::ClearClip)));
        row![progress_bar(FLOOR_DB..=0.0, peak).girth(10), text(db).size(13).width(48), clip].spacing(8).align_y(Alignment::Center).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_are_numbered_after_those_in_the_folder() {
        let dir = std::env::temp_dir().join(format!("omnix-takes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("VO_2026-10-29_101500_take01.wav"), b"").unwrap();
        std::fs::write(dir.join("narration.wav"), b"").unwrap();
        let name = take_file(&dir).file_name().unwrap().to_string_lossy().into_owned();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(name.starts_with("VO_") && name.ends_with("_take02.wav"), "{name}");
    }
}
