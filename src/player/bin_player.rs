//! The bin view's own player (gap 60, the start of 71a): the clip shown at
//! the view's right side plays there, not in a window of its own, with a
//! strip under it to put the playhead anywhere in the file.
//!
//! It is a second `Playback`, the same the edit plays with, so it seeks,
//! scrubs from the quick copy (gap 63) and plays its sound alike. Only one
//! of the two plays at a time: starting one pauses the other. While the bin
//! fills the picture's place the edit's picture isn't drawn, so the GPU
//! pipeline, which holds one picture (and a crossfade's second), draws this
//! one alone.

use super::gpu::{self, Rig, Shown};
use super::{App, Clip, Message, blocking};
use super::media::Media;
use super::playback::Playback;
use crate::stitch::M3;
use crate::view::{Camera, Projection, View};
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::shader;
use iced::{Color, Point, Rectangle, Renderer, Size, Task, Theme, mouse};
use std::path::PathBuf;
use std::sync::Arc;

/// The clip in the bin's player: its file, and its playback once loaded.
pub struct Source {
    pub path: PathBuf,
    pub clip: Option<Clip>,
}

/// The player's messages.
#[derive(Debug, Clone)]
pub enum SourceMsg {
    /// The clip read (`Media::load`), to play.
    Loaded(Arc<Result<Media, String>>),
    /// Play or pause (Space while the bin is shown).
    Toggle,
    /// The strip pressed or dragged: to this time, a frame at a time.
    Scrub(f64),
    /// The strip let go.
    ScrubDone,
}

impl App {
    /// Opens `path` in the bin's player: at once if it's loaded, else read
    /// off the UI thread first. Paused at its start.
    pub(super) fn source_open(&mut self, path: PathBuf) -> Task<Message> {
        if self.bin.source.as_ref().is_some_and(|s| s.path == path) {
            return Task::none();
        }
        if let Some(media) = self.library.iter().find(|m| m.input == path) {
            let clip = self.source_clip(media.clone());
            self.bin.source = Some(Source { path, clip: Some(clip) });
            return Task::none();
        }
        self.bin.source = Some(Source { path: path.clone(), clip: None });
        let opts = self.opts.clone();
        Task::perform(blocking(move || Media::load(path, &opts, &|_| {}).map_err(|e| format!("{e:#}"))), |r| Message::BinPlayer(SourceMsg::Loaded(Arc::new(r))))
    }

    fn source_clip(&self, media: Media) -> Clip {
        let mut playback = Playback::new(media, 1.0, self.full_on_pause);
        playback.jump(0.0, false);
        Clip { playback, shown: None }
    }

    pub(super) fn on_source(&mut self, msg: SourceMsg) -> Task<Message> {
        match msg {
            SourceMsg::Loaded(result) => {
                let result = Arc::unwrap_or_clone(result);
                let media = match result {
                    Ok(media) => media,
                    Err(e) => {
                        self.error = Some(e);
                        return Task::none();
                    }
                };
                // Still the file shown (another may have been picked since).
                if self.bin.source.as_ref().is_none_or(|s| s.path != media.input || s.clip.is_some()) {
                    return Task::none();
                }
                let clip = self.source_clip(media.clone());
                if let Some(s) = &mut self.bin.source {
                    s.clip = Some(clip);
                }
                // Into the library too: added to the edit later, it's there.
                if !self.library.iter().any(|m| m.input == media.input) {
                    return self.take_in(media);
                }
            }
            SourceMsg::Toggle => {
                let Some(p) = self.source_playback() else { return Task::none() };
                if p.playing {
                    p.pause();
                    return Task::none();
                }
                // From its end, again from the start.
                if p.ended || p.position() >= p.media.duration - 1.5 / p.media.fps {
                    p.jump(0.0, true);
                } else {
                    p.play();
                }
                self.pause_edit();
            }
            SourceMsg::Scrub(t) => {
                if let Some(p) = self.source_playback() {
                    p.scrub(t);
                }
            }
            SourceMsg::ScrubDone => {
                if let Some(p) = self.source_playback() {
                    p.scrub_end();
                }
            }
        }
        Task::none()
    }

    fn source_playback(&mut self) -> Option<&mut Playback> {
        self.bin.source.as_mut()?.clip.as_mut().map(|c| &mut c.playback)
    }

    /// Pauses the edit (and a crossfade's partner): the bin's player is
    /// starting.
    fn pause_edit(&mut self) {
        for c in self.clip.iter_mut().chain(self.partner.as_mut().map(|(_, c)| c)) {
            c.playback.pause();
        }
    }

    /// Pauses the bin's player: the edit is starting.
    pub(super) fn pause_source(&mut self) {
        if let Some(p) = self.source_playback() {
            p.pause();
        }
    }

    /// Whether the bin's player needs frames drawn (playing, seeking).
    pub(super) fn source_busy(&self) -> bool {
        self.bin.source.as_ref().and_then(|s| s.clip.as_ref()).is_some_and(|c| c.playback.busy())
    }

    /// Takes the bin's player's newest frame, every screen frame.
    pub(super) fn tick_source(&mut self) {
        if let Some(c) = self.bin.source.as_mut().and_then(|s| s.clip.as_mut())
            && let Some((frame, fix)) = c.playback.tick()
        {
            c.shown = Some(Arc::new(Shown::new(frame, fix)));
        }
    }

    /// The picture to draw, its shape (width over height), and where it
    /// is: the time and the clip's length. `None` until a frame is in.
    pub(super) fn source_picture(&self) -> Option<(Picture, f32, f64, f64)> {
        let c = self.bin.source.as_ref()?.clip.as_ref()?;
        let p = &c.playback;
        let flat = p.media.flat;
        // An ordinary video fills its own shape; a 360 clip looks ahead.
        let (view, aspect) = match flat {
            Some(f) => {
                let (w, h) = if f.turns % 2 == 1 { (f.stored.1, f.stored.0) } else { f.stored };
                (View::home(Projection::Perspective), w as f32 / h.max(1) as f32)
            }
            None => (View::default(), 16.0 / 9.0),
        };
        let picture = Picture { rig: p.media.rig.clone(), shown: c.shown.clone()?, camera: view.camera(), rig_from_view: p.rig_from_view(self.rolling_shutter) };
        Some((picture, aspect, p.position(), p.media.duration))
    }

    pub(super) fn source_playing(&self) -> bool {
        self.bin.source.as_ref().and_then(|s| s.clip.as_ref()).is_some_and(|c| c.playback.playing)
    }
}

/// The bin player's frame, drawn by the player's GPU pipeline; it takes no
/// input (a look around belongs to the edit's view).
#[derive(Clone)]
pub struct Picture {
    rig: Arc<Rig>,
    shown: Arc<Shown>,
    camera: Camera,
    rig_from_view: Vec<M3>,
}

impl shader::Program<Message> for Picture {
    type State = ();
    type Primitive = gpu::Primitive;

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> gpu::Primitive {
        gpu::Primitive { rig: self.rig.clone(), shown: self.shown.clone(), camera: self.camera, rig_from_view: self.rig_from_view.clone(), incoming: None, overlays: Vec::new() }
    }
}

/// The strip under the bin's player: the clip's length, how far it is, and
/// its playhead; pressed or dragged, the playhead goes there.
pub struct Strip {
    pub at: f64,
    pub length: f64,
}

impl canvas::Program<Message> for Strip {
    /// Being dragged.
    type State = bool;

    fn update(&self, dragging: &mut bool, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let time = |x: f32| (((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64) * self.length;
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_over(bounds)?;
                *dragging = true;
                Some(canvas::Action::publish(Message::BinPlayer(SourceMsg::Scrub(time(p.x)))).and_capture())
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { position }) if *dragging => Some(canvas::Action::publish(Message::BinPlayer(SourceMsg::Scrub(time(position.x)))).and_capture()),
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if *dragging => {
                *dragging = false;
                Some(canvas::Action::publish(Message::BinPlayer(SourceMsg::ScrubDone)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(&self, _: &bool, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = theme.extended_palette();
        let (w, h) = (bounds.width, bounds.height);
        let bar = h * 0.4;
        let top = (h - bar) / 2.0;
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(w, bar), (bar / 2.0).into()), palette.background.strong.color);
        let x = if self.length > 0.0 { (self.at / self.length).clamp(0.0, 1.0) as f32 * w } else { 0.0 };
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(x.max(bar), bar), (bar / 2.0).into()), palette.primary.base.color);
        frame.fill(&Path::rectangle(Point::new((x - 1.0).clamp(0.0, w - 2.0), 0.0), Size::new(2.0, h)), Color::WHITE);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, dragging: &bool, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if *dragging || cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}
