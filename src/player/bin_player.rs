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
use crate::notes::Segment;
use crate::stitch::M3;
use crate::view::{Camera, Projection, View};
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::widget::{shader, text_editor};
use iced::{Color, Point, Rectangle, Renderer, Size, Task, Theme, mouse};
use std::path::PathBuf;
use std::sync::Arc;

/// The clip in the bin's player: its file, and its playback once loaded.
pub struct Source {
    pub path: PathBuf,
    pub clip: Option<Clip>,
    /// The contact sheet's marks on it (`notes.rs`), the one picked on the
    /// strip, and an in point set with I waiting for its O.
    pub marks: Vec<Segment>,
    pub picked: Option<usize>,
    pub mark_in: Option<f64>,
    /// The picked segment's note as being typed, till it's saved.
    pub draft: Option<String>,
    /// The note's field (it wraps, so a long note reads whole), and the
    /// segment it was filled from (its place, start and end).
    pub note: text_editor::Content,
    note_for: Option<(usize, f64, Option<f64>)>,
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
    /// A segment picked on the strip (its place in `marks`), or none.
    Pick(Option<usize>),
    /// I: the picked segment's start to the playhead, else an in point
    /// there; false (Alt+I) forgets the in point and the pick.
    In(bool),
    /// O: the picked segment's end to the playhead, else a new segment from
    /// the in point (or the start) to it; false (Alt+O) as Alt+I.
    Out(bool),
    /// The picked segment removed (Delete, or its ✕).
    Remove,
    /// The picked segment's role chosen (none: «—»), saved at once.
    Role(Option<String>),
    /// Its note typed, and saved (Enter, or picking another segment).
    Note(text_editor::Action),
    NoteSave,
}

impl App {
    /// Opens `path` in the bin's player: at once if it's loaded, else read
    /// off the UI thread first. Paused at its start.
    pub(super) fn source_open(&mut self, path: PathBuf) -> Task<Message> {
        if self.bin.source.as_ref().is_some_and(|s| s.path == path) {
            return Task::none();
        }
        let marks = self.sheet_marks(&path);
        let source = |clip| Source { path: path.clone(), clip, marks, picked: None, mark_in: None, draft: None, note: text_editor::Content::new(), note_for: None };
        if let Some(media) = self.library.iter().find(|m| m.input == path) {
            let clip = self.source_clip(media.clone());
            self.bin.source = Some(source(Some(clip)));
            return Task::none();
        }
        self.bin.source = Some(source(None));
        let opts = self.opts.clone();
        Task::perform(blocking(move || Media::load(path, &opts, &|_| {}).map_err(|e| format!("{e:#}"))), |r| Message::BinPlayer(SourceMsg::Loaded(Arc::new(r))))
    }

    /// The contact sheet's marks on `path` (none without a sheet). Read
    /// here, on the UI thread: the sheet is a small file.
    fn sheet_marks(&mut self, path: &std::path::Path) -> Vec<Segment> {
        let Some(sheet) = &self.bin.sheet else { return Vec::new() };
        sheet.segments(path).unwrap_or_else(|e| {
            self.error = Some(format!("{e:#}"));
            Vec::new()
        })
    }

    fn source_clip(&self, media: Media) -> Clip {
        let mut playback = Playback::new(media, 1.0, self.full_on_pause);
        playback.jump(0.0, false);
        Clip { playback, shown: None }
    }

    pub(super) fn on_source(&mut self, msg: SourceMsg) -> Task<Message> {
        // A note being typed is saved before a segment's ends move.
        if matches!(msg, SourceMsg::In(true) | SourceMsg::Out(true)) {
            self.save_draft();
        }
        let task = self.source_msg(msg);
        self.sync_note();
        task
    }

    /// Fills the note's field from the picked segment when the pick (or
    /// the segment) changed.
    fn sync_note(&mut self) {
        let Some(s) = &mut self.bin.source else { return };
        let now = s.picked.and_then(|i| s.marks.get(i)).map(|m| (m.index, m.start, m.end));
        if now != s.note_for {
            s.note_for = now;
            s.draft = None;
            let text = s.picked.and_then(|i| s.marks.get(i)).map(|m| m.note.clone()).unwrap_or_default();
            s.note = text_editor::Content::with_text(&text);
        }
    }

    fn source_msg(&mut self, msg: SourceMsg) -> Task<Message> {
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
            SourceMsg::In(false) | SourceMsg::Out(false) => {
                if let Some(s) = &mut self.bin.source {
                    s.mark_in = None;
                    s.picked = None;
                }
            }
            SourceMsg::In(true) => {
                let Some(now) = self.source_playback().map(|p| p.position()) else { return Task::none() };
                let Some(s) = &mut self.bin.source else { return Task::none() };
                match s.picked.and_then(|i| s.marks.get(i)).cloned() {
                    Some(m @ Segment { end: Some(end), .. }) if now < end => self.set_segment(Some(m), now, end),
                    _ => {
                        s.picked = None;
                        s.mark_in = Some(now);
                    }
                }
            }
            SourceMsg::Out(true) => {
                let Some(now) = self.source_playback().map(|p| p.position()) else { return Task::none() };
                let Some(s) = &self.bin.source else { return Task::none() };
                match s.picked.and_then(|i| s.marks.get(i)).cloned() {
                    // A segment's end moved; a point picked becomes a segment.
                    Some(m) if now > m.start => {
                        let start = m.start;
                        self.set_segment(Some(m), start, now);
                    }
                    _ => {
                        let start = s.mark_in.unwrap_or(0.0);
                        self.set_segment(None, start, now);
                    }
                }
            }
            SourceMsg::Role(role) => {
                let note = self.bin.source.as_ref().and_then(|s| s.draft.clone().or_else(|| s.picked.and_then(|i| s.marks.get(i)).map(|m| m.note.clone())));
                self.describe_picked(role, note);
            }
            SourceMsg::Note(action) => {
                if let Some(s) = &mut self.bin.source {
                    let edit = action.is_edit();
                    s.note.perform(action);
                    if edit {
                        // One line, as the contact sheet keeps it.
                        s.draft = Some(s.note.text().replace('\n', " ").trim_end().to_owned());
                    }
                }
            }
            SourceMsg::NoteSave => self.save_draft(),
            SourceMsg::Remove => {
                let Some(s) = &self.bin.source else { return Task::none() };
                let Some(m) = s.picked.and_then(|i| s.marks.get(i)).cloned() else { return Task::none() };
                let path = s.path.clone();
                let removed = match &self.bin.sheet {
                    Some(sheet) if sheet.key(&path).is_some() => Some(sheet.remove(&m)),
                    _ => None,
                };
                if let Some(Err(e)) = removed {
                    self.error = Some(format!("Couldn't write the contact sheet: {e:#}"));
                    return Task::none();
                }
                let fresh = removed.is_some().then(|| self.sheet_marks(&path));
                let Some(s) = &mut self.bin.source else { return Task::none() };
                match fresh {
                    Some(marks) => s.marks = marks,
                    None => {
                        s.marks.remove(m.index);
                        for (i, x) in s.marks.iter_mut().enumerate() {
                            x.index = i;
                        }
                    }
                }
                s.picked = None;
                s.draft = None;
                // Its cuts on the timeline go too: those of this clip with
                // its in and out to a frame (one trimmed since is the
                // user's own, and stays). One undo step.
                let Some(end) = m.end else { return Task::none() };
                let frame = self.library.iter().find(|x| x.input == path).map_or(1.0 / 30.0, |x| 1.0 / x.fps);
                let near = |a: f64, b: f64| (a - b).abs() <= frame;
                let cuts: Vec<(usize, crate::project::Id)> = self.edit.cuts.iter().enumerate().filter(|(_, c)| c.input == path && near(c.start, m.start) && near(c.end, end)).map(|(i, c)| (i, c.id)).collect();
                let Some(&(first, _)) = cuts.first() else { return Task::none() };
                self.edit.commit();
                self.edit.begin();
                for (_, id) in &cuts {
                    self.op(crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Delete { cut: *id }));
                }
                self.edit.commit();
                self.selection = crate::model::selection::Selection::None;
                self.after_edit(self.edit.offset(first.min(self.edit.cuts.len())));
            }
            SourceMsg::Pick(i) => {
                self.save_draft();
                let Some(s) = &mut self.bin.source else { return Task::none() };
                s.draft = None;
                s.picked = i.filter(|&i| i < s.marks.len());
                // To its start, as the strip shows it picked.
                if let Some(start) = s.picked.map(|i| s.marks[i].start)
                    && let Some(p) = self.source_playback()
                {
                    let playing = p.playing;
                    p.jump(start, playing);
                }
            }
        }
        Task::none()
    }

    /// Sets a segment from `start` to `end` on the player's clip: the
    /// picked one moved (`was`), or a new one; written back to the contact
    /// sheet, then picked. Without a sheet, or for a clip outside its
    /// archive, it's kept here only, for adding.
    fn set_segment(&mut self, was: Option<Segment>, start: f64, end: f64) {
        let Some(s) = &self.bin.source else { return };
        let path = s.path.clone();
        let (start, end) = (start.min(end), start.max(end));
        if end - start < crate::model::edit::MIN_CUT {
            self.error = Some("The segment is too short: set the out point further from the in point".to_owned());
            return;
        }
        let written = match &self.bin.sheet {
            Some(sheet) if sheet.key(&path).is_some() => Some(sheet.set(&path, was.as_ref(), start, end)),
            _ => None,
        };
        let marks = match written {
            Some(Err(e)) => {
                self.error = Some(format!("Couldn't write the contact sheet: {e:#}"));
                return;
            }
            Some(Ok(index)) => Some((self.sheet_marks(&path), index)),
            None => None,
        };
        let Some(s) = &mut self.bin.source else { return };
        s.mark_in = None;
        match marks {
            Some((marks, index)) => {
                s.marks = marks;
                s.picked = Some(index);
            }
            // Here only: over the picked one, or a new one at the end.
            None => {
                let at = was.map(|w| w.index).filter(|&i| i < s.marks.len());
                let index = at.unwrap_or(s.marks.len());
                let (role, note) = at.map(|i| (s.marks[i].role.clone(), s.marks[i].note.clone())).unwrap_or_default();
                let m = Segment { index, id: None, start, end: Some(end), role, note };
                match at {
                    Some(i) => s.marks[i] = m,
                    None => s.marks.push(m),
                }
                s.picked = Some(index);
            }
        }
    }

    /// Saves the note being typed for the picked segment, if any.
    fn save_draft(&mut self) {
        let Some(s) = &self.bin.source else { return };
        let Some(draft) = s.draft.clone() else { return };
        let role = s.picked.and_then(|i| s.marks.get(i)).and_then(|m| m.role.clone());
        self.describe_picked(role, Some(draft));
    }

    /// Gives the picked segment `role` and `note` (its own kept if none):
    /// in the contact sheet, or here only without one.
    fn describe_picked(&mut self, role: Option<String>, note: Option<String>) {
        let Some(s) = &self.bin.source else { return };
        let Some(m) = s.picked.and_then(|i| s.marks.get(i)).cloned() else { return };
        let note = note.unwrap_or_else(|| m.note.clone());
        let path = s.path.clone();
        let written = match &self.bin.sheet {
            Some(sheet) if sheet.key(&path).is_some() => Some(sheet.describe(&m, role.as_deref(), &note)),
            _ => None,
        };
        if let Some(Err(e)) = written {
            self.error = Some(format!("Couldn't write the contact sheet: {e:#}"));
            return;
        }
        let fresh = written.is_some().then(|| self.sheet_marks(&path));
        let Some(s) = &mut self.bin.source else { return };
        s.draft = None;
        match fresh {
            Some(marks) => s.marks = marks,
            None => {
                if let Some(x) = s.marks.get_mut(m.index) {
                    x.role = role;
                    x.note = note;
                }
            }
        }
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
    /// The sheet's segments (start, end or none for a point, colour), the
    /// picked one's place among them, and an in point waiting for its out.
    pub marks: Vec<(f64, Option<f64>, Color)>,
    pub picked: Option<usize>,
    pub mark_in: Option<f64>,
}

/// The band at the strip's top where segments show and are picked.
const BAND: f32 = 12.0;
/// How near a point mark a press picks it, in pixels.
const NEAR: f32 = 5.0;

impl Strip {
    /// The segment under `x` in the band: the shortest one there, so one
    /// inside another can still be picked; a point within `NEAR`.
    fn mark_at(&self, x: f32, width: f32) -> Option<usize> {
        let px = |t: f64| (t / self.length.max(1e-9)) as f32 * width;
        self.marks
            .iter()
            .enumerate()
            .filter(|(_, (s, e, _))| match e {
                Some(e) => x >= px(*s) - 2.0 && x <= px(*e) + 2.0,
                None => (x - px(*s)).abs() <= NEAR,
            })
            .min_by(|a, b| {
                let len = |(s, e, _): &(f64, Option<f64>, Color)| e.map_or(0.0, |e| e - s);
                len(a.1).total_cmp(&len(b.1))
            })
            .map(|(i, _)| i)
    }
}

impl canvas::Program<Message> for Strip {
    /// Being dragged.
    type State = bool;

    fn update(&self, dragging: &mut bool, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let time = |x: f32| (((x - bounds.x) / bounds.width).clamp(0.0, 1.0) as f64) * self.length;
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_over(bounds)?;
                // The band picks a segment (or, pressed where none is, none).
                if !self.marks.is_empty() && p.y - bounds.y < BAND {
                    return Some(canvas::Action::publish(Message::BinPlayer(SourceMsg::Pick(self.mark_at(p.x - bounds.x, bounds.width)))).and_capture());
                }
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
        let px = |t: f64| if self.length > 0.0 { (t / self.length).clamp(0.0, 1.0) as f32 * w } else { 0.0 };
        // The segments in the band, the picked one full height and outlined
        // down over the bar.
        let band = if self.marks.is_empty() { 0.0 } else { BAND };
        for (i, (s, e, colour)) in self.marks.iter().enumerate() {
            let picked = self.picked == Some(i);
            let (x0, x1) = (px(*s), e.map_or(px(*s) + 3.0, px));
            let top = if picked { 0.0 } else { 3.0 };
            frame.fill(&Path::rectangle(Point::new(x0, top), Size::new((x1 - x0).max(3.0), band - 2.0 - top)), *colour);
            if picked {
                frame.fill(&Path::rectangle(Point::new(x0, band), Size::new((x1 - x0).max(3.0), h - band)), Color { a: 0.25, ..*colour });
            }
        }
        if let Some(t) = self.mark_in {
            let x = px(t);
            frame.fill(&Path::new(|b| {
                b.move_to(Point::new(x, 0.0));
                b.line_to(Point::new(x + 7.0, 0.0));
                b.line_to(Point::new(x, band.max(8.0)));
                b.close();
            }), palette.primary.strong.color);
        }
        let bar = (h - band) * 0.4;
        let top = band + (h - band - bar) / 2.0;
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(w, bar), (bar / 2.0).into()), palette.background.strong.color);
        let x = px(self.at);
        frame.fill(&Path::rounded_rectangle(Point::new(0.0, top), Size::new(x.max(bar), bar), (bar / 2.0).into()), palette.primary.base.color);
        frame.fill(&Path::rectangle(Point::new((x - 1.0).clamp(0.0, w - 2.0), band), Size::new(2.0, h - band)), Color::WHITE);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, dragging: &bool, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if *dragging || cursor.is_over(bounds) { mouse::Interaction::Pointer } else { mouse::Interaction::default() }
    }
}
