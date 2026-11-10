//! The bin: the media of the project's folder, sorted by kind, in a panel
//! of its own at the far left. It folds to a narrow strip; pinned, it stays
//! open, also at the next start.
//!
//! Each file shows a picture (a frame of a clip, the picture itself) and
//! its length. A click previews it: a clip's frames in the bin's preview
//! strip, a sound heard. "+" adds it where its kind goes: a clip as a cut,
//! a sound on its section's lane (music, SFX, voice-over), a picture over
//! the picture.
//!
//! The project's folder (`root`): a Dailix trip's for one of its videos
//! (`videos/<name>/`), the one above an older project's `04_project`, or
//! the one holding the project file.
//!
//! Sounds and clips are heard through `ffplay`, ffmpeg's player, as a
//! separate process like every ffmpeg tool here; a clip plays in a small
//! window of its own.

use super::{App, Message, blocking, tip};
use super::tracks::probe_sound;
use crate::model::ops::cuts::Place as CutPlace;
use crate::project::SoundTrack;
use iced::widget::{button, column, container, image, row, rule, scrollable, text, tooltip};
use iced::{Alignment, Element, Length, Task};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const WIDTH: f32 = 360.0;
/// The folded bin: a strip with the button that opens it.
const FOLDED: f32 = 34.0;
/// Wide enough for the bin view's biggest tiles; the panel shows it small.
const THUMB_W: u32 = 420;
/// Frames in the preview strip of a clip.
const PREVIEW_FRAMES: usize = 4;

/// What a file is for, and so where "+" puts it: a sound goes onto its
/// section's sound track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Clip,
    Sound(SoundTrack),
    Picture,
}

impl Kind {
    /// The same kind of file, whichever track a sound is for.
    pub(super) fn alike(self, other: Kind) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// Which of the bin's add buttons: at the end, at the playhead (splitting
/// the cut there), or at the join of cuts nearest the playhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    End,
    Playhead,
    Join,
}

/// The add buttons' icons.
const ADD_END: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h13M11 7l5 5-5 5M20 5v14"/></svg>"#;
const ADD_HERE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v13"/><path d="M7 3h10l-5 6z" fill="white"/></svg>"#;
const ADD_JOIN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="11" width="8" height="10" rx="1"/><rect x="14" y="11" width="8" height="10" rx="1"/><path d="M12 2v7M9 6l3 3 3-3"/></svg>"#;

/// A heading and the folders under the project's folder it gathers.
/// Cleaned voice (`voice/`) isn't listed: it belongs to its clip, and comes
/// in with the project footage-agent writes, not by hand.
pub(super) const SECTIONS: [(&str, Kind, &[&str]); 6] = [
    ("Footage", Kind::Clip, &["01_footage"]),
    ("External video", Kind::Clip, &["06_external"]),
    ("Music", Kind::Sound(SoundTrack::Music), &[SoundTrack::Music.folder()]),
    ("Voice-over", Kind::Sound(SoundTrack::Voice), &[SoundTrack::Voice.folder()]),
    ("Sound effects", Kind::Sound(SoundTrack::Sfx), &[SoundTrack::Sfx.folder()]),
    ("Pictures", Kind::Picture, &[dailix_sheet::layout::PICTURES, dailix_sheet::layout::OLD_PICTURES]),
];

pub(super) const CLIP_EXT: &[&str] = &["mp4", "mov", "mkv", "m4v", "avi", "mts", "webm", "osv", "insv"];
pub(super) const SOUND_EXT: &[&str] = &["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma"];
pub(super) const PICTURE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"];

/// Extensions for a file dialog's filter: upper case too, as the filter is
/// exact on Linux.
pub(super) fn exts(of: &[&str]) -> Vec<String> {
    of.iter().flat_map(|e| [e.to_string(), e.to_uppercase()]).collect()
}

/// One heading's files, by name: a folder's own, or one of its
/// subfolders' (`sub`: a day's, as Dailix files a project's clips under
/// `01_footage/<day>/`).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: &'static str,
    pub sub: Option<String>,
    pub kind: Kind,
    pub files: Vec<PathBuf>,
}

impl Section {
    /// Its heading: the section's name, and its subfolder's.
    pub fn heading(&self) -> String {
        match &self.sub {
            Some(sub) => format!("{} · {sub}", self.title),
            None => self.title.to_owned(),
        }
    }
}

/// What the bin read about a file: its length (none for a picture) and a
/// picture of it (none for a sound).
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub duration: Option<f64>,
    pub thumb: Option<image::Handle>,
}

/// The file being previewed: its frames (a clip's, once read) and the
/// player heard, if one is playing.
pub struct Preview {
    pub path: PathBuf,
    pub kind: Kind,
    pub frames: Vec<image::Handle>,
    pub(super) player: Option<Player>,
}

/// An `ffplay` process, stopped when dropped: a preview never outlives the
/// bin's hold on it.
pub(super) struct Player(Child);

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Preview {
    pub fn playing(&mut self) -> bool {
        self.player.as_mut().is_some_and(|p| matches!(p.0.try_wait(), Ok(None)))
    }
}

/// The bin's state in the app.
#[derive(Default)]
pub struct Bin {
    /// Open, or folded to a strip.
    pub open: bool,
    /// Kept open, now and at the next start.
    pub pinned: bool,
    /// What it read of the project's folder: its sections and the folder;
    /// `None` with no saved project.
    pub read: Option<(Vec<Section>, PathBuf)>,
    pub meta: HashMap<PathBuf, Meta>,
    pub preview: Option<Preview>,
    /// Shown as tiles in the picture's place (`bin_view.rs`).
    pub view: bool,
    /// The tiles' width there.
    pub tile: f32,
    /// The pointer over the view, and a tile's menu open at it.
    pub pointer: iced::Point,
    pub menu: Option<(PathBuf, Kind, iced::Point)>,
    /// Files being copied into the project's folder, and what the last
    /// copy did.
    pub copying: usize,
    pub copied: Option<String>,
    /// The bin view's right side (the file shown, its Info): its width,
    /// whether its edge is being dragged, and the view's size last drawn
    /// (to turn the pointer into a width, and to keep the pictures at the
    /// side from pushing its Info out of sight).
    pub side: f32,
    pub gripping: bool,
    pub size: std::cell::Cell<iced::Size>,
    /// What Info read of the file shown there (`None` while reading).
    pub about: Option<(PathBuf, Option<Result<std::sync::Arc<super::info::File>, String>>)>,
    /// Clips being added as one segment of them (the bin player's picked
    /// one), till they're in.
    pub spans: HashMap<PathBuf, (f64, f64)>,
    /// The project folder's contact sheet, if it has one (`notes.rs`), and
    /// the roles a segment can have (its own list, or the default one).
    pub sheet: Option<crate::notes::Sheet>,
    pub roles: Vec<crate::notes::Role>,
    /// The tile the pointer rests on, since when, and whether its frames
    /// show; the frames read for hovering, by clip.
    pub hover: Option<(PathBuf, std::time::Instant, bool)>,
    pub hover_frames: HashMap<PathBuf, Vec<image::Handle>>,
    /// Every day's clips shown, not only the edit's days'.
    pub all_days: bool,
    /// The headings folded to their line (user, 2026-11-05: a project's
    /// every day in the bin is a long list); none at first.
    pub folded: std::collections::HashSet<String>,
    /// The clip in the bin view's own player (`bin_player.rs`).
    pub source: Option<super::bin_player::Source>,
    /// Loaded clips from outside the folder taken out of the bin's list
    /// (the clips stay loaded: an undo may bring back a cut of one).
    pub hidden: std::collections::HashSet<PathBuf>,
}

/// The project's folder for a project file at `project`: a Dailix trip's
/// for one of its videos; above an older project's `04_project`; else
/// the file's own folder.
pub fn root(project: &Path) -> Option<PathBuf> {
    if let Some(trip) = dailix_sheet::project::of_edit(project) {
        return Some(trip);
    }
    let dir = project.parent()?;
    if dir.file_name().is_some_and(|n| n == "04_project") { dir.parent().map(Path::to_owned) } else { Some(dir.to_owned()) }
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let is = |list: &[&str]| list.contains(&ext.as_str());
    if is(CLIP_EXT) {
        Some(Kind::Clip)
    } else if is(SOUND_EXT) {
        Some(Kind::Sound(SoundTrack::Music))
    } else if is(PICTURE_EXT) {
        Some(Kind::Picture)
    } else {
        None
    }
}

/// The files in `dir` of `kind`, by path.
fn files(dir: &Path, kind: Kind, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.') || n.starts_with('_'));
        if !hidden && path.is_file() && kind_of(&path).is_some_and(|k| k.alike(kind)) {
            out.push(path);
        }
    }
}

/// What the bin shows for the project's folder `root`: the sections that
/// have files, in a fixed order; a section's folder's own files, then a
/// section per subfolder (a day's, in a Dailix project), by name. DJI's
/// `.LRF` proxies aren't clips of their own, so they're never listed.
pub fn scan(root: &Path) -> Vec<Section> {
    let mut out = Vec::new();
    for &(title, kind, dirs) in &SECTIONS {
        let mut own = Vec::new();
        let mut subs: Vec<(String, PathBuf)> = Vec::new();
        for dir in dirs {
            let dir = root.join(dir);
            files(&dir, kind, &mut own);
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.path().is_dir() && !name.starts_with('.') && !name.starts_with('_') {
                    subs.push((name, e.path()));
                }
            }
        }
        own.sort();
        if !own.is_empty() {
            out.push(Section { title, sub: None, kind, files: own });
        }
        subs.sort();
        for (name, dir) in subs {
            let mut found = Vec::new();
            files(&dir, kind, &mut found);
            found.sort();
            if !found.is_empty() {
                out.push(Section { title, sub: Some(name), kind, files: found });
            }
        }
    }
    out
}

/// A file's name as the bin lists it.
fn label(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A clip's frames spread over its length, `width` wide, read at once (a
/// frame of 2.7K HEVC takes about a second).
pub(super) fn frames(path: &Path, duration: f64, width: u32) -> Vec<image::Handle> {
    use rayon::prelude::*;
    (1..=PREVIEW_FRAMES).into_par_iter().filter_map(|i| frame(path, duration * (i as f64 - 0.5) / PREVIEW_FRAMES as f64, width)).collect()
}

/// One frame of `path` at `t` seconds, `width` wide.
fn frame(path: &Path, t: f64, width: u32) -> Option<image::Handle> {
    crate::ffmpeg::jpeg_frame(path, t, width).map(image::Handle::from_bytes)
}

/// What the bin shows of a file: its length and, for a clip or a picture,
/// a small frame of it.
fn read_meta(path: &Path, kind: Kind) -> Meta {
    let duration = (kind != Kind::Picture).then(|| crate::ffmpeg::duration(path).ok()).flatten();
    let thumb = match kind {
        Kind::Sound(_) => None,
        Kind::Picture => frame(path, 0.0, THUMB_W),
        // A third in: past a clip's often shaky start.
        Kind::Clip => frame(path, duration.unwrap_or(3.0) * 0.3, THUMB_W),
    };
    Meta { duration, thumb }
}

/// `ffplay` on `path`: a sound with no window, a clip in a small one.
fn play(path: &Path, kind: Kind) -> Option<Player> {
    let mut cmd = Command::new("ffplay");
    cmd.args(["-autoexit", "-loglevel", "quiet"]);
    match kind {
        Kind::Clip => cmd.args(["-x", "640", "-y", "360", "-window_title", "Preview"]),
        _ => cmd.arg("-nodisp"),
    };
    cmd.arg(path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok().map(Player)
}

impl App {
    /// The project's folder, read afresh, and a task reading the files'
    /// pictures and lengths not read yet, a few files per thread.
    pub(super) fn rescan_bin(&mut self) -> Task<Message> {
        self.bin.read = self.project.as_deref().and_then(root).map(|r| (scan(&r), r));
        self.bin.sheet = self.bin.read.as_ref().and_then(|(_, r)| crate::notes::Sheet::of(r));
        self.bin.roles = self.bin.sheet.as_ref().map_or_else(crate::notes::Sheet::default_roles, crate::notes::Sheet::roles);
        self.read_bin_meta()
    }

    /// Reads the pictures and lengths not read yet: the folder's files and
    /// the clips loaded from outside it (an archive's), a few per thread.
    pub(super) fn read_bin_meta(&mut self) -> Task<Message> {
        let mut todo: Vec<(PathBuf, Kind)> = self.bin.read.iter().flat_map(|(sections, _)| sections.iter().flat_map(|s| s.files.iter().map(move |f| (f.clone(), s.kind)))).collect();
        todo.extend(self.loaded_outside().into_iter().map(|f| (f, Kind::Clip)));
        todo.retain(|(f, _)| !self.bin.meta.contains_key(f));
        for (f, _) in &todo {
            self.bin.meta.insert(f.clone(), Meta::default());
        }
        Task::batch(todo.chunks(4).map(|chunk| {
            let chunk = chunk.to_vec();
            Task::perform(blocking(move || chunk.into_iter().map(|(f, k)| (read_meta(&f, k), f)).map(|(m, f)| (f, m)).collect::<Vec<_>>()), Message::BinMeta)
        }))
    }

    /// Shows `path`: a clip's frames read, what Info says of it read; and,
    /// with `start`, a sound or a clip heard (a clip in a window of its
    /// own). Started again while it plays, it stops; the file already shown
    /// keeps what was read of it.
    pub(super) fn bin_preview(&mut self, path: PathBuf, kind: Kind, start: bool) -> Task<Message> {
        let same = self.bin.preview.as_ref().is_some_and(|p| p.path == path);
        let was_playing = same && self.bin.preview.as_mut().is_some_and(Preview::playing);
        if same && !start {
            return Task::none();
        }
        self.stop_preview();
        if was_playing {
            return Task::none();
        }
        let player = (start && kind != Kind::Picture).then(|| play(&path, kind)).flatten();
        if start && player.is_none() && kind != Kind::Picture {
            self.error = Some("Can't preview: ffplay isn't installed (it comes with ffmpeg)".to_owned());
        }
        if same {
            if let Some(p) = &mut self.bin.preview {
                p.player = player;
            }
            return Task::none();
        }
        self.bin.preview = Some(Preview { path: path.clone(), kind, frames: Vec::new(), player });
        let about = self.read_about(path.clone(), kind);
        if kind != Kind::Clip {
            return about;
        }
        // The bin view shows a clip in its player (its frames on hovering a
        // tile); the panel shows them in its strip.
        if !start {
            return about;
        }
        let duration = self.bin.meta.get(&path).and_then(|m| m.duration).unwrap_or(10.0);
        let frames = Task::perform(
            blocking({
                let path = path.clone();
                move || frames(&path, duration, 640)
            }),
            move |frames| Message::BinFrames(path.clone(), frames),
        );
        Task::batch([frames, about])
    }

    /// Reads what Info says of `path`, for the bin view's side.
    fn read_about(&mut self, path: PathBuf, kind: Kind) -> Task<Message> {
        if self.bin.about.as_ref().is_some_and(|(p, f)| *p == path && f.as_ref().is_some_and(Result::is_ok)) {
            return Task::none();
        }
        self.bin.about = Some((path.clone(), None));
        let song = matches!(kind, Kind::Sound(_));
        Task::perform(
            blocking({
                let path = path.clone();
                move || super::info::read(&path, song).map(std::sync::Arc::new).map_err(|e| format!("{e:#}"))
            }),
            move |r| Message::BinView(super::bin_view::BinMsg::About(path.clone(), r)),
        )
    }

    /// Stops any sound or clip the bin is playing, keeping what it shows.
    pub(super) fn stop_preview(&mut self) {
        if let Some(p) = &mut self.bin.preview {
            p.player = None;
        }
    }

    /// The days this edit is of: its cuts' (clips under `01_footage/<day>/`)
    /// and its file's name's (`omnix-<day>`, an empty day). None for none:
    /// then every day shows.
    pub(super) fn edit_days(&self) -> Option<std::collections::HashSet<String>> {
        let (_, root) = self.bin.read.as_ref()?;
        let footage = root.join("01_footage");
        let mut days: std::collections::HashSet<String> = self.edit.cuts.iter().filter_map(|c| c.input.strip_prefix(&footage).ok()?.components().next().map(|d| d.as_os_str().to_string_lossy().into_owned())).collect();
        days.extend(self.project.as_deref().and_then(crate::from_sheet::day_of));
        (!days.is_empty()).then_some(days)
    }

    /// Whether a section shows: a day's clips only for the edit's days,
    /// unless every day is asked for (user, 2026-11-05: a day's edit showed
    /// every day's clips); music, effects and the rest always.
    pub(super) fn day_shown(&self, s: &Section) -> bool {
        if self.bin.all_days || s.kind != Kind::Clip {
            return true;
        }
        match (&s.sub, self.edit_days()) {
            (Some(day), Some(days)) => days.contains(day),
            _ => true,
        }
    }

    /// A heading that folds its files away and back when clicked, with how
    /// many there are.
    pub(super) fn fold_heading(&self, heading: String, files: usize, size: u32) -> Element<'_, Message> {
        let mark = if self.bin.folded.contains(&heading) { "▸" } else { "▾" };
        let label = text(format!("{mark} {heading}  ·  {files}")).size(size);
        button(label).padding(0).style(button::text).on_press(Message::BinView(super::bin_view::BinMsg::Fold(heading))).into()
    }

    /// The «all days» switch, when the edit is of some days and the folder
    /// has others.
    pub(super) fn days_toggle(&self) -> Option<Element<'_, Message>> {
        let days = self.edit_days()?;
        let (sections, _) = self.bin.read.as_ref()?;
        let others = sections.iter().any(|s| s.kind == Kind::Clip && s.sub.as_ref().is_some_and(|d| !days.contains(d)));
        others.then(|| {
            let mut v: Vec<&String> = days.iter().collect();
            v.sort();
            let label = v.into_iter().cloned().collect::<Vec<_>>().join(", ");
            tip(
                iced::widget::toggler(self.bin.all_days).label("All days").text_size(13).on_toggle(|_| Message::BinView(super::bin_view::BinMsg::AllDays)),
                text(format!("Every day's clips, not only this edit's ({label})")),
                tooltip::Position::Bottom,
            )
        })
    }

    /// The bin at the far left: open, or the strip it folds to.
    pub(super) fn bin_panel(&self) -> Element<'_, Message> {
        if !self.bin.open {
            let open = tip(button(super::svg_icon(super::BIN)).padding(5).on_press(Message::BinPanel), text("Open the bin (B)"), tooltip::Position::Right);
            return container(column![open].padding([8, 4])).width(FOLDED).height(Length::Fill).into();
        }
        let small = |s: &'static str| button(text(s).size(14)).padding([0, 6]).style(button::text);
        let pin = tip(
            small(if self.bin.pinned { "🔒" } else { "🔓" }).on_press(Message::BinPin),
            text(if self.bin.pinned { "Pinned: stays open, also next time. Click to unpin" } else { "Pin the bin open" }),
            tooltip::Position::Bottom,
        );
        let refresh = tip(small("↻").on_press(Message::BinRefresh), text("Read the folder again"), tooltip::Position::Bottom);
        let fold = tip(small("«").on_press_maybe((!self.bin.pinned).then_some(Message::BinPanel)), text("Fold the bin to the side (B)"), tooltip::Position::Bottom);
        let mut body = column![row![text("Bin").size(18).width(Length::Fill), pin, refresh, fold].spacing(2).align_y(Alignment::Center)].spacing(8);

        if let Some(p) = &self.bin.preview {
            body = body.push(self.preview_box(p));
        }

        // The clips already loaded from outside the folder (an archive's):
        // the folder's own are listed under their sections.
        let loaded = self.loaded_outside();

        match &self.bin.read {
            None => body = body.push(text("Save the project to see its folder here.").size(13).style(text::secondary)),
            Some((sections, root)) => {
                body = body.push(text(root.display().to_string()).size(12).style(text::secondary));
                if let Some(days) = self.days_toggle() {
                    body = body.push(days);
                }
                for section in sections.iter().filter(|s| self.day_shown(s)) {
                    let heading = section.heading();
                    let open = !self.bin.folded.contains(&heading);
                    body = body.push(rule::horizontal(1)).push(self.fold_heading(heading, section.files.len(), 15));
                    if open {
                        for path in &section.files {
                            body = body.push(self.item(label(path), path, section.kind));
                        }
                    }
                }
            }
        }
        if !loaded.is_empty() {
            body = body.push(rule::horizontal(1)).push(text("Loaded clips").size(15));
            for path in &loaded {
                body = body.push(self.item(label(path), path, Kind::Clip));
            }
        }
        container(scrollable(body.padding(10))).width(WIDTH).height(Length::Fill).into()
    }

    /// The file being previewed: its frames, and Stop while it plays.
    pub(super) fn preview_box<'a>(&'a self, p: &'a Preview) -> Element<'a, Message> {
        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut col = column![row![text(name).size(13).width(Length::Fill), button(text("■").size(13)).padding([0, 6]).style(button::text).on_press(Message::BinStop)].align_y(Alignment::Center)].spacing(4);
        match p.kind {
            Kind::Clip if p.frames.is_empty() => col = col.push(text("Reading frames…").size(12).style(text::secondary)),
            Kind::Clip => {
                let mut grid = column![].spacing(2);
                for pair in p.frames.chunks(2) {
                    grid = grid.push(row(pair.iter().map(|h| image(h.clone()).width(Length::FillPortion(1)).into())).spacing(2));
                }
                col = col.push(grid);
            }
            Kind::Picture => {
                if let Some(h) = self.bin.meta.get(&p.path).and_then(|m| m.thumb.clone()) {
                    col = col.push(image(h).width(Length::Fill));
                }
            }
            Kind::Sound(_) => col = col.push(text("♪ playing").size(12).style(text::secondary)),
        }
        container(col).padding(6).style(container::rounded_box).into()
    }

    /// A file as a row: its picture, name and length; click previews, "+" adds.
    fn item<'a>(&self, name: String, path: &Path, kind: Kind) -> Element<'a, Message> {
        let meta = self.bin.meta.get(path);
        let picture: Element<'a, Message> = match meta.and_then(|m| m.thumb.clone()) {
            Some(h) => image(h).width(64).height(36).into(),
            None => container(text(if matches!(kind, Kind::Sound(_)) { "♪" } else { "…" }).size(16)).width(64).height(36).center_x(64).center_y(36).into(),
        };
        let length = meta.and_then(|m| m.duration).map(|d| dailix_sheet::clock(d, 0)).unwrap_or_default();
        let selected = self.bin.preview.as_ref().is_some_and(|p| p.path == path);
        // A camera's names have no spaces to break at: broken anywhere, so
        // a long one wraps under itself rather than under the add buttons.
        let info = column![text(name).size(13).wrapping(text::Wrapping::WordOrGlyph), text(length).size(11).style(text::secondary)].spacing(1).width(Length::Fill);
        let body = super::bin_view::carried(container(row![picture, info].spacing(8).align_y(Alignment::Center)).padding([3, 4]).into(), (path, kind), Length::Fill, selected, Message::BinPreview(path.to_owned(), kind));
        row![body, add_buttons(path, kind)].spacing(4).align_y(Alignment::Center).into()
    }
}

/// A file's add buttons: at the end, at the playhead, at the join (a
/// picture only at the playhead).
pub(super) fn add_buttons<'a>(path: &Path, kind: Kind) -> Element<'a, Message> {
    let add = |icon: &'static [u8], place: Place, hint: String| -> Element<'a, Message> {
        let icon = iced::widget::svg(iced::widget::svg::Handle::from_memory(icon)).width(16).height(16);
        tip(button(icon).padding(4).on_press(Message::BinAdd(path.to_owned(), kind, place)), text(hint), tooltip::Position::Left)
    };
    match kind {
        Kind::Clip => row![
            add(ADD_END, Place::End, "Add at the end of the edit".into()),
            add(ADD_HERE, Place::Playhead, "Add at the playhead: the cut there is split around it".into()),
            add(ADD_JOIN, Place::Join, "Add at the join of cuts nearest the playhead".into()),
        ]
        .spacing(2)
        .into(),
        Kind::Sound(which) => row![
            add(ADD_END, Place::End, format!("Add on the {} lane after its last sound", which.name())),
            add(ADD_HERE, Place::Playhead, format!("Add on the {} lane at the playhead", which.name())),
            add(ADD_JOIN, Place::Join, format!("Add on the {} lane at the join of cuts nearest the playhead", which.name())),
        ]
        .spacing(2)
        .into(),
        Kind::Picture => add(ADD_HERE, Place::Playhead, "Add over the picture at the playhead".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_the_trip_or_above_04_project() {
        let trip = std::env::temp_dir().join(format!("omnix-bin-root-{}", std::process::id()));
        let l = dailix_sheet::Layout::new(&trip);
        std::fs::create_dir_all(l.data()).unwrap();
        dailix_sheet::project::update(&l, |_| {}).unwrap();
        assert_eq!(root(&l.video_edit("2030-05-01")), Some(trip.clone()), "a video's edit: its trip");
        std::fs::remove_dir_all(&trip).unwrap();
        assert_eq!(root(Path::new("/v/kos/04_project/den2.json")), Some(PathBuf::from("/v/kos")));
        assert_eq!(root(Path::new("/v/edit.json")), Some(PathBuf::from("/v")));
    }

    #[test]
    fn scan_sorts_files_into_sections() {
        let dir = std::env::temp_dir().join(format!("omnix-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let make = |rel: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        };
        make("01_footage/DJI_0001.MP4");
        make("01_footage/DJI_0001.LRF");
        make("02_audio/music/song.wav");
        make("02_audio/music/LICENSES.md");
        make("07_sfx/boom.mp3");
        make("voice/2026-08-22/DJI_0001/dnn6.flac");
        make("08_graphics/logo.png");
        // A Dailix project files its clips by day.
        make("01_footage/2030-05-02/B.MP4");
        make("01_footage/2030-05-01/A.MP4");
        make("01_footage/_old/skip.MP4");
        make("_trash/old.mp3");
        let s = scan(&dir);
        let titles: Vec<_> = s.iter().map(|s| s.title).collect();
        // Cleaned voice belongs to its clip: not listed.
        assert_eq!(titles, ["Footage", "Footage", "Footage", "Music", "Sound effects", "Pictures"]);
        assert_eq!(s[0].files, [dir.join("01_footage/DJI_0001.MP4")]);
        assert_eq!(s.iter().map(Section::heading).take(3).collect::<Vec<_>>(), ["Footage", "Footage · 2030-05-01", "Footage · 2030-05-02"]);
        assert_eq!(s[1].files, [dir.join("01_footage/2030-05-01/A.MP4")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

impl App {
    /// The bin: this area's messages, as `handle` passes them on.
    pub(super) fn on_bin(&mut self, message: Message) -> Task<Message> {
        match message {
            // Pinned, it stays open.
            Message::BinPanel if self.bin.open && self.bin.pinned => {}
            Message::BinPanel => self.bin.open = !self.bin.open,
            Message::BinPin => {
                self.bin.pinned = !self.bin.pinned;
                self.bin.open |= self.bin.pinned;
            }
            Message::BinRefresh => return self.rescan_bin(),
            Message::BinPreview(path, kind) => return self.bin_preview(path, kind, true),
            Message::BinStop => self.stop_preview(),
            Message::BinMeta(read) => self.bin.meta.extend(read),
            Message::BinFrames(path, frames) => {
                if let Some(p) = self.bin.preview.as_mut().filter(|p| p.path == path) {
                    p.frames = frames;
                }
            }
            Message::BinAdd(path, Kind::Clip, place) => {
                // Where it goes is decided now, from the playhead now: the
                // clip may take a while to load.
                let spot = self.spot(place);
                self.placing.insert(path.clone(), spot);
                // The segment picked in the bin's player, if it's this clip's.
                if let Some(s) = self.bin.source.as_ref().filter(|s| s.path == path)
                    && let Some(m) = s.picked.and_then(|i| s.marks.get(i))
                    && let Some(end) = m.end
                {
                    self.bin.spans.insert(path.clone(), (m.start, end));
                }
                return self.update(Message::Picked(vec![path]));
            }
            Message::BinAdd(path, Kind::Picture, _) => return self.update(Message::OverlayPicked(vec![path])),
            Message::BinAdd(path, Kind::Sound(which), place) => {
                // Onto its section's track, where it fits nearest the spot,
                // decided now from the playhead now.
                let at = match self.spot(place) {
                    CutPlace::End => None,
                    CutPlace::At(t) => Some(t),
                    CutPlace::Join(t) => Some(crate::project::offsets(&self.edit.cuts).into_iter().chain([self.edit.duration()]).nth(self.edit.nearest_join(t)).unwrap_or(t)),
                };
                return probe_sound(which, path, at);
            }
            _ => {}
        }
        Task::none()
    }
}
