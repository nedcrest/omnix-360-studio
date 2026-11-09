//! Omnix's player for another program's window: Dailix's contact sheet
//! plays clips with it (`../dailix`). A small surface over the
//! player the edit and the bin use: a clip read (`load`, blocking: run it
//! off the UI thread), played, paused, sought and scrubbed (from the quick
//! copy, gap 63, made on load), and its newest frame drawn by Omnix's GPU
//! pipeline as a `shader` widget in the host's own message type.
//!
//! The pipeline holds one picture at a time (and a crossfade's second):
//! show one `Picture` per window.

use super::gpu::{self, Rig, Shown};
use super::media::{Media, Options};
use super::playback::Playback;
use crate::stitch::M3;
use crate::view::{Camera, Projection, View};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use std::path::Path;
use std::sync::Arc;

/// A clip read, ready to play: made by `load`, off the UI thread.
pub struct Loaded(Media);

/// Reads the clip at `path` for playing (its decoder probed, its quick
/// copy for scrubbing started). Blocking: ffprobe, and an Osmo 360 clip's
/// telemetry.
pub fn load(path: &Path) -> anyhow::Result<Loaded> {
    let media = Media::load(path.to_owned(), &Options::default(), &|_| {})?;
    if let Some(flat) = media.flat {
        super::scrub::make(media.input.clone(), flat, media.hw);
    }
    Ok(Loaded(media))
}

/// What the clip at `path` is, as lines (its kind first): length,
/// picture, frame rate, codecs, when recorded, size and bitrate, as
/// Omnix's Info panel shows them. Blocking (ffprobe).
pub fn describe(path: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let file = super::info::read(path, false)?;
    Ok(std::iter::once(("Kind".to_owned(), file.kind.to_owned())).chain(file.rows.into_iter().map(|(k, v)| (k.to_owned(), v))).collect())
}

/// One frame of the clip at `path`, `t` seconds in, `width` wide, as
/// JPEG bytes (a poster); ffmpeg found as Omnix finds it. Blocking.
pub fn jpeg_frame(path: &Path, t: f64, width: u32) -> Option<Vec<u8>> {
    crate::ffmpeg::jpeg_frame(path, t, width)
}

/// A clip playing (or paused) and its newest frame.
pub struct Viewer {
    playback: Playback,
    shown: Option<Arc<Shown>>,
}

impl Viewer {
    /// Paused at the clip's start.
    pub fn new(loaded: Loaded) -> Viewer {
        let mut playback = Playback::new(loaded.0, 1.0, true);
        playback.jump(0.0, false);
        Viewer { playback, shown: None }
    }

    pub fn duration(&self) -> f64 {
        self.playback.media.duration
    }
    /// Where it is, in seconds into the clip.
    pub fn position(&self) -> f64 {
        self.playback.position()
    }
    pub fn playing(&self) -> bool {
        self.playback.playing
    }

    /// Plays or pauses; from its end, plays from the start.
    pub fn toggle(&mut self) {
        let p = &mut self.playback;
        if p.playing {
            p.pause();
        } else if p.ended || p.position() >= p.media.duration - 1.5 / p.media.fps {
            p.jump(0.0, true);
        } else {
            p.play();
        }
    }
    /// To `t`, playing on if it was.
    pub fn jump(&mut self, t: f64) {
        let playing = self.playback.playing;
        self.playback.jump(t, playing);
    }
    /// A frame at `t` while a strip is dragged; `scrub_end` when let go.
    pub fn scrub(&mut self, t: f64) {
        self.playback.scrub(t);
    }
    pub fn scrub_end(&mut self) {
        self.playback.scrub_end();
    }
    /// Its sound from `audio` (none: the clip's own), `gain_db` louder,
    /// on from where it is: a clip's voices compared at one loudness.
    pub fn hear(&mut self, audio: Option<&Path>, gain_db: f64) {
        let filter = (gain_db.abs() > 0.05).then(|| format!("volume={gain_db:.2}dB"));
        self.playback.hear(audio.map(Path::to_path_buf), filter);
    }

    /// One frame on (`frames` > 0) or back.
    pub fn step(&mut self, frames: f64) {
        self.playback.step(frames);
    }

    /// Whether it needs a tick each screen frame (playing, seeking).
    pub fn busy(&self) -> bool {
        self.playback.busy()
    }

    /// Takes the newest decoded frame: call every screen frame while busy.
    pub fn tick(&mut self) {
        if let Some((frame, fix)) = self.playback.tick() {
            self.shown = Some(Arc::new(Shown::new(frame, fix)));
        }
    }

    /// The picture's shape, width over height.
    pub fn aspect(&self) -> f32 {
        aspect(&self.playback)
    }

    /// The frame to draw (a `shader` widget), once one is decoded.
    pub fn picture(&self) -> Option<Picture> {
        picture(&self.playback, self.shown.clone()?, true)
    }
}

/// A clip's shape, width over height: an ordinary video's own (turned if
/// the camera was), a 360 clip's view 16:9. Here and in the bin's player.
pub(super) fn aspect(playback: &Playback) -> f32 {
    playback.media.flat.map_or(16.0 / 9.0, |f| {
        let (w, h) = f.size();
        w as f32 / h.max(1) as f32
    })
}

/// A clip's frame as a viewer shows it: an ordinary video whole, a 360
/// clip looking ahead.
pub(super) fn picture(playback: &Playback, shown: Arc<Shown>, rolling_shutter: bool) -> Option<Picture> {
    let view = if playback.media.flat.is_some() { View::home(Projection::Perspective) } else { View::default() };
    Some(Picture { rig: playback.media.rig.clone(), shown, camera: view.camera(), rig_from_view: playback.rig_from_view(rolling_shutter) })
}

/// A frame drawn by Omnix's GPU pipeline; it takes no input.
#[derive(Clone)]
pub struct Picture {
    rig: Arc<Rig>,
    shown: Arc<Shown>,
    camera: Camera,
    rig_from_view: Vec<M3>,
}

impl<M> shader::Program<M> for Picture {
    type State = ();
    type Primitive = gpu::Primitive;

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> gpu::Primitive {
        gpu::Primitive { rig: self.rig.clone(), shown: self.shown.clone(), camera: self.camera, rig_from_view: self.rig_from_view.clone(), incoming: None, edge: None, style: None, overlays: Vec::new() }
    }
}
