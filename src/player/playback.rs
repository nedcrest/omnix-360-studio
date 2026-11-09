//! The playback clock: plays from the stream (proxy or scaled OSV) and, if
//! asked to, swaps in a full-resolution OSV frame whenever playback is paused.

use super::gpu::RS_STEPS_MAX;
use super::source::{self, Frame, Kind, Stream};
use super::Media;
use super::audio::Audio;
use super::gpu::SeamFix;
use crate::stitch::M3;
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

/// How far ahead of the frame shown a parked preview decoder is taken on
/// rather than a fresh one started at the time: decoding up to this many
/// seconds of frames is about what a fresh start costs.
const PARKED_REACH: f64 = 2.0;

/// How many paused frames are kept to step back to without decoding
/// (about 3 MB each at playback size).
const RECENT: usize = 48;

/// A step back to a frame not kept starts decoding this much earlier and
/// keeps the frames on the way (the decoder goes through them from the
/// keyframe anyway), so the steps back after it are at once: the
/// playhead's buffer (the user, 2026-11-03).
const BACK_FILL: f64 = 1.0;

pub struct Playback {
    pub media: Media,
    pub playing: bool,
    /// Playback clock: `anchor_pts` at `anchor_at`.
    anchor_pts: f64,
    anchor_at: Instant,
    /// When the stream playing was started, until its first frame; and how
    /// long that took last time, to start a crossfade's partner that far
    /// ahead (`startup`).
    started_at: Option<Instant>,
    startup: f64,
    /// A crossfade partner made `primed` to catch up: `go` once the edit
    /// reaches this clip time, and left alone (not ticked) until then.
    pub go_at: Option<f64>,
    pub shown_pts: f64,
    stream: Option<Stream>,
    pending: Option<Frame>,
    /// While paused, a stream is only kept for its first frame.
    preview_only: bool,
    /// The paused frame asked for while a preview was still on its way
    /// (stepping, a crossfade's partner following): started once that one
    /// shows. Restarted on every step instead, a preview never got its
    /// first frame out while the keys repeated, and nothing showed.
    preview_next: Option<f64>,
    /// The paused preview's decoder after its frame showed, kept with that
    /// frame's time: a step on takes its next frames instead of starting a
    /// decoder again, which decoded from the keyframe before every step
    /// and held a core busy (the user, 2026-11-03). Its queue is bounded,
    /// so parked it waits, decoding nothing more.
    parked: Option<(Stream, f64)>,
    /// A preview's frames before this time are passed over (a parked
    /// decoder taken on to a later frame).
    preview_from: f64,
    /// The last paused frames shown, newest last: a step back to one of
    /// them shows it at once, as no decoder can step backwards.
    recent: std::collections::VecDeque<Frame>,
    /// A frame from `recent` to show at the next tick.
    recalled: Option<Frame>,
    /// A sharper frame for while paused, being decoded: its time and size
    /// (`None` for full resolution).
    still: Option<(f64, Option<u32>, Receiver<Result<Frame>>)>,
    /// A still decoding (it can outlive `still`, which a seek drops), and
    /// the time of the one asked for since: one decodes at a time, the
    /// latest asked for after it. A still a frame (stepping, a held key)
    /// started a full-resolution decode each, dozens at once.
    still_busy: Arc<AtomicBool>,
    still_next: Option<f64>,
    /// The frame on screen is such a still, at this size.
    showing_still: Option<Option<u32>>,
    /// Swap in a full-resolution frame when paused. Off, a paused frame is
    /// at the playback source's quality, so stopping doesn't change the
    /// picture.
    full_on_pause: bool,
    /// While the seek bar is dragged: whether to resume playing on release,
    /// and the latest position asked for while a preview was still decoding.
    scrub: Option<(bool, Option<f64>)>,
    /// Seam corrections, eased from frame to frame while playing.
    fix: SeamFix,
    /// Plays while playing, and is then the clock.
    audio: Option<Audio>,
    /// Sound opened and buffered ahead (`primed`), from this time, to play
    /// once the first frame shows instead of opening the device then.
    ready: Option<(f64, Audio)>,
    /// Sound starts with the next frame shown. Sound is ready in
    /// milliseconds but a fresh HEVC decode takes far longer, so starting both
    /// together left the picture trailing the sound after every seek or
    /// source switch.
    sound_waits: bool,
    volume: f32,
    pub error: Option<String>,
    /// Playback reached the end of the clip and stopped.
    pub ended: bool,
}

/// An ordinary video plays fitted inside this many pixels on its long side
/// (a phone's 4K would be as heavy as the OSV); paused, "Full res when
/// paused" shows it whole.
pub const FLAT_PLAY: u32 = 1920;

/// `size` scaled to fit `long` on its long side (never up), sides even.
pub fn fit((w, h): (u32, u32), long: u32) -> (u32, u32) {
    let k = (long as f64 / w.max(h) as f64).min(1.0);
    let even = |v: u32| ((v as f64 * k / 2.0).round() as u32).max(1) * 2;
    if k >= 1.0 { (w / 2 * 2, h / 2 * 2) } else { (even(w), even(h)) }
}

impl Playback {
    /// A playback with nothing decoding yet: `jump` (or `seek`) says where.
    /// It used to seek to 0 first, which started a preview decode and a
    /// full-resolution still there only to throw them away.
    pub fn new(media: Media, volume: f32, full_on_pause: bool) -> Self {
        Playback {
            media,
            playing: false,
            anchor_pts: 0.0,
            anchor_at: Instant::now(),
            started_at: None,
            startup: 0.0,
            go_at: None,
            shown_pts: 0.0,
            stream: None,
            pending: None,
            preview_only: false,
            preview_next: None,
            parked: None,
            preview_from: 0.0,
            recent: Default::default(),
            recalled: None,
            still: None,
            still_busy: Default::default(),
            still_next: None,
            showing_still: None,
            full_on_pause,
            scrub: None,
            fix: SeamFix::default(),
            audio: None,
            ready: None,
            sound_waits: false,
            volume,
            error: None,
            ended: false,
        }
    }

    /// A cut's playback made ready ahead of time while another plays: the
    /// decoder started at `t` (it fills its few frames of buffer and waits)
    /// and the sound opened and buffered, so `go` shows its first frame and
    /// starts its sound at once, without the decoder's seek or the device's
    /// start-up in between.
    pub fn primed(media: Media, t: f64, volume: f32, full_on_pause: bool) -> Self {
        let mut p = Playback::new(media, volume, full_on_pause);
        let t = p.snap(t);
        (p.anchor_pts, p.shown_pts) = (t, t);
        p.playing = true;
        p.start_stream(t);
        p.playing = false;
        if p.media.has_audio {
            p.ready = Audio::prepare(p.media.heard(), t, volume, p.media.filter()).inspect_err(|e| eprintln!("no sound ready: {e:#}")).ok().map(|a| (t, a));
        }
        p
    }

    /// How long before `go` its sound should start: the device's latency,
    /// so the first sample is heard just as the cut before ends. `None`
    /// without sound ready (or once it's started). The device reports it
    /// (cpal's callback and playback timestamps; ~50 ms on PipeWire, more on
    /// Bluetooth); a device reporting none gets the old small gap, and one
    /// reporting nonsense at most a quarter second of overlap.
    pub fn sound_lead(&self) -> Option<f64> {
        self.ready.as_ref().map(|(_, a)| a.latency().min(0.25))
    }

    /// Starts a `primed` playback's sound ahead of `go` (see `sound_lead`).
    pub fn start_sound(&mut self) {
        if let Some((_, ready)) = self.ready.take() {
            ready.play();
            self.audio = Some(ready);
        }
    }

    /// Starts playing a `primed` playback. With its sound already started
    /// the picture runs on at once; else the sound starts with the first
    /// frame, and the picture waits for it to be heard.
    pub fn go(&mut self) {
        // Its decoder started long before: no start-up to measure.
        self.started_at = None;
        self.go_at = None;
        self.playing = true;
        self.ended = false;
        self.anchor_at = Instant::now();
        self.sound_waits = self.media.has_audio && self.audio.is_none();
    }

    fn kind(&self) -> Kind {
        if let Some(flat) = self.media.flat {
            // A picture repeats at the clip's rate, as the frames are timed.
            return Kind::Flat { path: self.media.input.clone(), flat, size: Some(fit(flat.size(), FLAT_PLAY)), rate: flat.picture.then_some(self.media.fps) };
        }
        match (&self.media.proxy, self.media.osv_size) {
            (Some(p), None) => Kind::Lrf(p.clone()),
            (_, size) => Kind::Osv { path: self.media.input.clone(), tracks: self.media.rig.tracks.clone(), size: Some(size.unwrap_or(1920)) },
        }
    }

    /// Streams at this OSV size, or from the proxy if `None`.
    pub fn set_osv_size(&mut self, size: Option<u32>) {
        self.media.osv_size = size;
        self.seek(self.shown_pts);
    }

    fn snap(&self, t: f64) -> f64 {
        let last = (self.media.duration - 1.0 / self.media.fps).max(0.0);
        ((t * self.media.fps).round() / self.media.fps).clamp(0.0, last)
    }

    /// The playback clock: where playback is now, between frames too.
    pub fn now(&self) -> f64 {
        match (self.playing, &self.audio) {
            (false, _) => self.anchor_pts,
            _ if self.sound_waits => self.anchor_pts,
            (true, Some(audio)) => self.anchor_pts + audio.elapsed(),
            (true, None) => self.anchor_pts + self.anchor_at.elapsed().as_secs_f64(),
        }
    }

    /// Its sound from `audio` (none: the clip's own) through `filter`,
    /// on from where it is without stopping the picture: two voices of a
    /// clip heard against each other (Dailix, gap 77).
    pub fn hear(&mut self, audio: Option<std::path::PathBuf>, filter: Option<String>) {
        let t = self.position();
        self.media.has_audio |= audio.is_some();
        (self.media.audio, self.media.filter, self.ready) = (audio, filter, None);
        if self.playing {
            (self.anchor_pts, self.anchor_at) = (t, Instant::now());
            self.start_audio(t);
        } else {
            self.audio = None;
        }
    }

    /// Stops the sound until the next frame is shown, and the clock with it.
    fn hold_sound(&mut self) {
        self.audio = None;
        self.sound_waits = self.playing && self.media.has_audio;
    }

    /// Starts sound at `t`. Without a usable device the clip plays silently
    /// on the wall clock; that is reported once, not on every play.
    fn start_audio(&mut self, t: f64) {
        self.audio = None;
        if !self.media.has_audio {
            return;
        }
        if let Some((at, ready)) = self.ready.take()
            && (at - t).abs() < 0.5 / self.media.fps
        {
            ready.play();
            self.audio = Some(ready);
            return;
        }
        match Audio::start(self.media.heard(), t, self.volume, self.media.filter()) {
            Ok(a) => self.audio = Some(a),
            Err(e) => {
                eprintln!("no sound: {e:#}");
                self.media.has_audio = false;
            }
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        for a in self.audio.iter().chain(self.ready.as_ref().map(|(_, a)| a)) {
            a.set_volume(volume);
        }
    }

    /// Starts decoding at `t`. A paused preview only needs one frame, so it
    /// comes from the proxy when there is one: it starts fastest.
    fn start_stream(&mut self, t: f64) {
        self.started_at = self.playing.then(Instant::now);
        self.pending = None;
        self.preview_next = None;
        self.parked = None;
        self.preview_from = t;
        // Paused, the quick copies: a 360 clip's proxy, an ordinary video's
        // copy for the playhead (`scrub`), decoded in software.
        let scrub = self.media.flat.filter(|_| !self.playing).and_then(|flat| super::scrub::ready(&self.media.input).map(|p| (p, super::scrub::flat(&flat))));
        let (kind, hw) = match (&self.media.proxy, self.playing, scrub) {
            (Some(p), false, _) => (Kind::Lrf(p.clone()), self.media.hw),
            (_, _, Some((path, flat))) => (Kind::Flat { path, flat, size: None, rate: None }, crate::ffmpeg::Hwaccel::Software),
            _ => (self.kind(), self.media.hw),
        };
        self.stream = match source::spawn(kind, hw, t, self.media.fps) {
            Ok(s) => Some(s),
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                None
            }
        };
    }

    pub fn toggle(&mut self) {
        if self.playing { self.pause() } else { self.play() }
    }

    pub fn play(&mut self) {
        if self.shown_pts >= self.snap(self.media.duration) {
            self.shown_pts = 0.0;
        }
        self.playing = true;
        self.ended = false;
        self.preview_only = false;
        self.still = None;
        self.anchor_pts = self.shown_pts;
        self.anchor_at = Instant::now();
        self.start_stream(self.shown_pts);
        self.hold_sound();
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.hold_sound();
        self.anchor_pts = self.shown_pts;
        self.stream = None;
        self.pending = None;
        self.request_still(self.shown_pts);
    }

    /// Jumps to `t`. While paused a proxy frame shows first, then the
    /// full-resolution one.
    pub fn seek(&mut self, t: f64) {
        self.ended = false;
        let t = self.snap(t);
        self.anchor_pts = t;
        if self.playing {
            self.anchor_at = Instant::now();
            self.start_stream(t);
            self.hold_sound();
        } else {
            self.preview(t);
            self.request_still(t);
        }
    }

    /// The quick picture at `t` while paused (from the proxy where there
    /// is one), one decode in flight, the latest asked for next.
    fn preview(&mut self, t: f64) {
        // Shown a moment ago: at once, and whatever was decoding is dropped.
        if let Some(f) = self.recent.iter().find(|f| (f.pts - t).abs() < 0.5 / self.media.fps) {
            self.recalled = Some(f.clone());
            self.preview_only = true;
            self.stream = None;
            self.preview_next = None;
            return;
        }
        if self.stream.is_some() && self.preview_only {
            self.preview_next = Some(t);
            return;
        }
        self.preview_only = true;
        // A little ahead of the frame shown: its decoder, parked, goes on.
        if let Some((stream, at)) = self.parked.take()
            && t > at
            && t - at < PARKED_REACH
        {
            self.stream = Some(stream);
            self.pending = None;
            self.preview_from = t;
            return;
        }
        // Behind the frame shown: from a second before, keeping the frames
        // on the way for the steps back that follow.
        let back = t < self.shown_pts - 0.5 / self.media.fps;
        self.start_stream(if back { (t - BACK_FILL).max(0.0) } else { t });
        self.preview_from = t;
    }

    /// Keeps a paused frame for steps back (`recent`), the oldest let go.
    fn keep(&mut self, f: Frame) {
        if self.recent.iter().any(|r| r.pts == f.pts) {
            return;
        }
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(f);
    }

    /// The full-resolution frame where it has come to rest, once quick
    /// steps stop (`step` shows only the quick picture).
    pub fn settle(&mut self) {
        if !self.playing && self.scrub.is_none() {
            self.request_still(self.anchor_pts);
        }
    }

    /// Goes to `t`, then plays or stays paused.
    pub fn jump(&mut self, t: f64, play: bool) {
        match (play, self.playing) {
            (true, true) => self.seek(t),
            (true, false) => {
                self.shown_pts = self.snap(t);
                self.play();
            }
            (false, playing) => {
                if playing {
                    self.playing = false;
                    self.hold_sound();
                    self.stream = None;
                    self.pending = None;
                }
                self.seek(t);
            }
        }
    }

    /// How long the last stream started playing took to show its first
    /// frame: about a second for the OSV at 1920 px.
    pub fn startup(&self) -> f64 {
        self.startup
    }

    /// A playback of the same clip, `primed` at `t` to `go` there (see
    /// `go_at`), knowing how long its decoder takes to start.
    pub fn primed_again(&self, t: f64) -> Self {
        let mut p = Playback::primed(self.media.clone(), t, self.volume, self.full_on_pause);
        p.startup = self.startup;
        p.go_at = Some(p.anchor_pts);
        p
    }

    /// Whether the clock is running: playing, and not waiting for the first
    /// frame (and the sound with it) after a start or seek.
    pub fn running(&self) -> bool {
        self.playing && !self.sound_waits
    }

    /// From where the last step or seek asked, not from the frame shown:
    /// pressed faster than frames decode, each step used to start again
    /// from the old frame, asking for the same one, and the playhead stood
    /// still (the user, 2026-11-03).
    ///
    /// Only the quick picture: a full-resolution frame for every step kept
    /// the decoder busy on frames already passed; `settle` asks for it
    /// once the steps stop.
    pub fn step(&mut self, frames: f64) {
        if !self.playing {
            self.ended = false;
            let t = self.snap(self.target() + frames / self.media.fps);
            self.anchor_pts = t;
            self.still_next = None;
            self.preview(t);
        }
    }

    /// Follows the seek bar while it is dragged: playback holds, and each
    /// preview starts only once the last one has shown, so a fast drag never
    /// keeps restarting the decoder before anything appears.
    pub fn scrub(&mut self, t: f64) {
        let t = self.snap(t);
        if self.scrub.is_none() {
            self.scrub = Some((self.playing, None));
            self.playing = false;
            self.hold_sound();
            self.still = None;
        }
        self.anchor_pts = t;
        // The still on screen is of another time now.
        self.showing_still = None;
        if self.stream.is_some() && self.preview_only {
            self.scrub = self.scrub.map(|(resume, _)| (resume, Some(t)));
        } else {
            self.preview_only = true;
            self.start_stream(t);
        }
    }

    pub fn scrub_end(&mut self) {
        let Some((resume, _)) = self.scrub.take() else { return };
        if resume {
            self.shown_pts = self.anchor_pts;
            self.play();
        } else {
            self.seek(self.anchor_pts);
        }
    }

    /// Where playback is headed: the frame on screen while playing, else
    /// the last seek's time, whose frame may still be decoding.
    pub fn target(&self) -> f64 {
        if self.playing { self.shown_pts } else { self.anchor_pts }
    }

    /// Where the seek bar should sit: under the pointer while dragging, and
    /// paused, where it was put, before its frame has decoded (a click on
    /// another clip showed the playhead at that clip's start a moment
    /// first: the user, 2026-11-03).
    pub fn position(&self) -> f64 {
        if self.scrub.is_some() || !self.playing { self.anchor_pts } else { self.shown_pts }
    }

    /// Turns the full-resolution paused frame on or off, swapping the one on
    /// screen now if paused.
    pub fn set_full_on_pause(&mut self, on: bool) {
        self.full_on_pause = on;
        if !self.playing && self.scrub.is_none() {
            self.seek(self.shown_pts);
        }
    }

    /// Decodes the paused frame at `t` sharper than the proxy preview: at
    /// full resolution, or else at the OSV size being played (none when
    /// playing the proxy, whose preview is already that quality).
    fn request_still(&mut self, t: f64) {
        // What to decode, and at what size (none: full); nothing when
        // playing already shows it as sharp.
        let size = match (self.media.flat, self.full_on_pause, self.media.osv_size) {
            // Playing already shows up to `FLAT_PLAY`; only full size adds.
            (Some(flat), full, _) if !full || fit(flat.size(), FLAT_PLAY) == flat.size() => None,
            (Some(_), ..) => Some(None),
            (None, true, _) => Some(None),
            (None, false, Some(s)) => Some(Some(s)),
            (None, false, None) => None,
        };
        let Some(size) = size else {
            self.still = None;
            return;
        };
        if self.still_busy.load(Ordering::Relaxed) {
            self.still_next = Some(t);
            return;
        }
        let (tx, rx) = channel();
        let (path, flat, tracks, hw, busy) = (self.media.input.clone(), self.media.flat, self.media.rig.tracks.clone(), self.media.hw, self.still_busy.clone());
        busy.store(true, Ordering::Relaxed);
        std::thread::spawn(move || {
            let frame = match flat {
                Some(flat) => source::still_flat(&path, flat, t, hw),
                None => source::still(&path, &tracks, t, size, hw),
            };
            busy.store(false, Ordering::Relaxed);
            let _ = tx.send(frame);
        });
        self.still = Some((t, size, rx));
    }

    /// Whether the clock or a decode needs polling every display frame.
    pub fn busy(&self) -> bool {
        self.playing || self.stream.is_some() || self.still.is_some() || self.recalled.is_some()
    }

    /// Pulls whatever the decoders have ready for the current clock and
    /// returns the frame to show now, if it changed, with its seam corrections.
    pub fn tick(&mut self) -> Option<(Frame, SeamFix)> {
        let mut show = self.recalled.take().map(|f| (f, None));
        // A slow machine at full resolution can fall behind the sound; then
        // the sound restarts from the picture rather than drifting from it.
        if self.audio.is_some() && self.now() - self.shown_pts > 0.5 {
            self.anchor_pts = self.shown_pts;
            self.hold_sound();
        }
        if self.stream.is_some() {
            let due = if self.playing && !self.sound_waits { self.now() + 0.5 / self.media.fps } else { f64::INFINITY };
            let mut latest = None;
            let mut ended = false;
            loop {
                let next = match self.pending.take() {
                    Some(f) => f,
                    None => match self.stream.as_ref().unwrap().frames.try_recv() {
                        Ok(f) => f,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            ended = true;
                            break;
                        }
                    },
                };
                // Before the one asked for (a parked decoder taken on, or a
                // step back's buffer filling): kept for steps back, not shown.
                if self.preview_only && next.pts < self.preview_from - 0.5 / self.media.fps {
                    self.keep(next);
                    continue;
                }
                if next.pts <= due {
                    latest = Some(next);
                    if self.preview_only || self.sound_waits {
                        break;
                    }
                } else {
                    self.pending = Some(next);
                    break;
                }
            }
            if let Some(f) = latest {
                if let Some(at) = self.started_at.take() {
                    self.startup = at.elapsed().as_secs_f64();
                }
                if self.sound_waits && self.playing {
                    self.sound_waits = false;
                    self.anchor_pts = f.pts;
                    self.start_audio(f.pts);
                }
                // A paused preview never replaces the full-resolution still.
                let pts = f.pts;
                if self.preview_only {
                    self.keep(f.clone());
                }
                if !(self.preview_only && self.showing_still.is_some() && self.still.is_none()) {
                    show = Some((f, None));
                }
                if self.preview_only {
                    self.parked = self.stream.take().map(|s| (s, pts));
                    if let Some((resume, Some(t))) = self.scrub {
                        self.scrub = Some((resume, None));
                        self.preview(t);
                    } else if let Some(t) = self.preview_next.take() {
                        self.preview(t);
                    }
                }
            } else if self.playing && self.audio.is_none() && self.shown_pts < self.now() - 0.25 {
                // Decoding is slower than real time: let the clock wait for
                // it. Sound can't wait, so with sound the picture skips.
                self.anchor_pts = self.shown_pts;
                self.anchor_at = Instant::now();
            }
            if ended && self.playing {
                self.pause();
                self.ended = true;
            } else if ended && self.preview_only {
                // No frame there (past the end): the next one asked for.
                self.stream = None;
                if let Some(t) = self.preview_next.take() {
                    self.start_stream(t);
                }
            }
        }
        if let Some((t, size, rx)) = &self.still {
            match rx.try_recv() {
                Ok(Ok(frame)) => {
                    let current = !self.playing && (*t - self.anchor_pts).abs() < 0.5 / self.media.fps;
                    let size = *size;
                    self.still = None;
                    if current {
                        show = Some((frame, Some(size)));
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("full-resolution frame failed: {e:#}");
                    self.still = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.still = None,
            }
        }
        // The still asked for while another decoded, if it's still wanted.
        if !self.still_busy.load(Ordering::Relaxed)
            && let Some(t) = self.still_next.take()
            && !self.playing
            && (t - self.anchor_pts).abs() < 0.5 / self.media.fps
        {
            self.request_still(t);
        }
        let (frame, still) = show?;
        // Playing frames follow on from each other; anything else is a jump.
        let continuous = self.playing && still.is_none() && (frame.pts - self.shown_pts).abs() < 2.0 / self.media.fps;
        self.shown_pts = frame.pts;
        self.showing_still = still;
        self.fix.follow(&self.media.rig, &frame, continuous);
        Some((frame, self.fix.clone()))
    }

    pub fn rig_from_view(&self, rolling_shutter: bool) -> Vec<M3> {
        let steps = if rolling_shutter && self.media.readout_ms > 0.0 { RS_STEPS_MAX } else { 1 };
        if self.media.stab.has_data() {
            self.media.stab.rig_from_view(self.shown_pts * 1000.0, self.media.readout_ms, steps)
        } else {
            vec![[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]]
        }
    }

    /// What is on screen: a still, the stream, or a still on its way.
    pub fn quality(&self) -> String {
        let osv = |size: Option<u32>| size.map_or_else(|| "full res".to_owned(), |s| format!("OSV {s} px"));
        match (self.showing_still, &self.still) {
            (Some(size), _) => osv(size),
            (None, Some((_, size, _))) => format!("loading {}…", osv(*size)),
            (None, None) if !self.playing && self.media.proxy.is_some() => "proxy".to_owned(),
            (None, None) => match self.kind() {
                Kind::Lrf(_) => "proxy".to_owned(),
                Kind::Osv { size, .. } => osv(size),
                Kind::Flat { size, .. } => size.map_or_else(|| "full res".to_owned(), |(w, h)| format!("{w}×{h}")),
            },
        }
    }
}
