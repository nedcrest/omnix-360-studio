//! The bin: the media of the project's folder, sorted by kind, in a panel
//! of its own at the far left. It folds to a narrow strip; pinned, it stays
//! open, also at the next start.
//!
//! Each file shows a picture (a frame of a clip, the picture itself) and
//! its length. A click previews it: a clip's frames in the bin's preview
//! strip, a sound heard. "+" adds it where its kind goes: a clip as a cut,
//! a sound on the music lane at the playhead, a picture over the picture.
//!
//! The project's folder is the one holding the project file, or the one
//! above it when the file sits in a `04_project` folder: the layout
//! footage-agent makes (`footage_agent/layout.py` there).
//!
//! Sounds and clips are heard through `ffplay`, ffmpeg's player, as a
//! separate process like every ffmpeg tool here; a clip plays in a small
//! window of its own.

use super::{App, Message, blocking, tip};
use super::tracks::probe_sound;
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
    fn alike(self, other: Kind) -> bool {
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

/// A `Place` made definite: the playhead's edit time when it was pressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spot {
    End,
    At(f64),
    Join(f64),
}

/// The add buttons' icons.
const ADD_END: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h13M11 7l5 5-5 5M20 5v14"/></svg>"#;
const ADD_HERE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v13"/><path d="M7 3h10l-5 6z" fill="white"/></svg>"#;
const ADD_JOIN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="11" width="8" height="10" rx="1"/><rect x="14" y="11" width="8" height="10" rx="1"/><path d="M12 2v7M9 6l3 3 3-3"/></svg>"#;

/// A heading and the folders under the project's folder it gathers.
/// Cleaned voice (`voice/`) isn't listed: it belongs to its clip, and comes
/// in with the project footage-agent writes, not by hand.
const SECTIONS: [(&str, Kind, &[&str]); 6] = [
    ("Footage", Kind::Clip, &["01_footage"]),
    ("External video", Kind::Clip, &["06_external"]),
    ("Music", Kind::Sound(SoundTrack::Music), &[SoundTrack::Music.folder()]),
    ("Voice-over", Kind::Sound(SoundTrack::Voice), &[SoundTrack::Voice.folder()]),
    ("Sound effects", Kind::Sound(SoundTrack::Sfx), &[SoundTrack::Sfx.folder()]),
    ("Graphics", Kind::Picture, &["08_graphics"]),
];

const CLIP_EXT: &[&str] = &["mp4", "mov", "mkv", "webm", "osv", "insv"];
const SOUND_EXT: &[&str] = &["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma"];
const PICTURE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff"];

/// One heading's files, by name.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: &'static str,
    pub kind: Kind,
    pub files: Vec<PathBuf>,
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
    player: Option<Player>,
}

/// An `ffplay` process, stopped when dropped: a preview never outlives the
/// bin's hold on it.
struct Player(Child);

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
}

/// The project's folder for a project file at `project`.
pub fn root(project: &Path) -> Option<PathBuf> {
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
/// have files, in a fixed order. DJI's `.LRF` proxies aren't clips of their
/// own, so they're never listed.
pub fn scan(root: &Path) -> Vec<Section> {
    SECTIONS
        .iter()
        .filter_map(|&(title, kind, dirs)| {
            let mut found = Vec::new();
            for dir in dirs {
                files(&root.join(dir), kind, &mut found);
            }
            found.sort();
            (!found.is_empty()).then_some(Section { title, kind, files: found })
        })
        .collect()
}

/// A file's name as the bin lists it.
fn label(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(super) fn clock(seconds: f64) -> String {
    let s = seconds.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// One frame of `path` at `t` seconds, `width` wide, as a JPEG.
fn frame(path: &Path, t: f64, width: u32) -> Option<image::Handle> {
    let out = crate::ffmpeg::tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss", &format!("{t:.2}"), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &format!("scale={width}:-2"), "-f", "image2pipe", "-c:v", "mjpeg", "-"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (out.status.success() && !out.stdout.is_empty()).then(|| image::Handle::from_bytes(out.stdout))
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
        let Some((sections, _)) = &self.bin.read else { return Task::none() };
        let todo: Vec<(PathBuf, Kind)> = sections.iter().flat_map(|s| s.files.iter().map(move |f| (f.clone(), s.kind))).filter(|(f, _)| !self.bin.meta.contains_key(f)).collect();
        for (f, _) in &todo {
            self.bin.meta.insert(f.clone(), Meta::default());
        }
        Task::batch(todo.chunks(4).map(|chunk| {
            let chunk = chunk.to_vec();
            Task::perform(blocking(move || chunk.into_iter().map(|(f, k)| (read_meta(&f, k), f)).map(|(m, f)| (f, m)).collect::<Vec<_>>()), Message::BinMeta)
        }))
    }

    /// Previews `path`: a sound or a clip heard (a clip in a window of its
    /// own), a clip's frames read into the strip. The same file again stops it.
    pub(super) fn bin_preview(&mut self, path: PathBuf, kind: Kind) -> Task<Message> {
        let same = self.bin.preview.as_mut().is_some_and(|p| p.path == path && p.playing());
        self.stop_preview();
        if same {
            return Task::none();
        }
        let player = (kind != Kind::Picture).then(|| play(&path, kind)).flatten();
        if player.is_none() && kind != Kind::Picture {
            self.error = Some("Can't preview: ffplay isn't installed (it comes with ffmpeg)".to_owned());
        }
        self.bin.preview = Some(Preview { path: path.clone(), kind, frames: Vec::new(), player });
        if kind != Kind::Clip {
            return Task::none();
        }
        let duration = self.bin.meta.get(&path).and_then(|m| m.duration).unwrap_or(10.0);
        Task::perform(
            blocking({
                let path = path.clone();
                // All at once: a frame of 2.7K HEVC takes about a second.
                move || {
                    use rayon::prelude::*;
                    (1..=PREVIEW_FRAMES).into_par_iter().filter_map(|i| frame(&path, duration * (i as f64 - 0.5) / PREVIEW_FRAMES as f64, 300)).collect::<Vec<_>>()
                }
            }),
            move |frames| Message::BinFrames(path.clone(), frames),
        )
    }

    /// Stops any sound or clip the bin is playing, keeping what it shows.
    pub(super) fn stop_preview(&mut self) {
        if let Some(p) = &mut self.bin.preview {
            p.player = None;
        }
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

        // The clips already loaded, wherever they live: a project's footage
        // is often in an archive outside its folder.
        let mut loaded: Vec<PathBuf> = self.library.iter().map(|m| m.input.clone()).collect();
        loaded.sort();
        loaded.dedup();

        match &self.bin.read {
            None => body = body.push(text("Save the project to see its folder here.").size(13).style(text::secondary)),
            Some((sections, root)) => {
                body = body.push(text(root.display().to_string()).size(12).style(text::secondary));
                for section in sections {
                    body = body.push(rule::horizontal(1)).push(text(section.title).size(15));
                    for path in &section.files {
                        body = body.push(self.item(label(path), path, section.kind));
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
        let length = meta.and_then(|m| m.duration).map(clock).unwrap_or_default();
        let selected = self.bin.preview.as_ref().is_some_and(|p| p.path == path);
        let info = column![text(name).size(13), text(length).size(11).style(text::secondary)].spacing(1).width(Length::Fill);
        let body = button(row![picture, info].spacing(8).align_y(Alignment::Center))
            .width(Length::Fill)
            .padding([3, 4])
            .style(if selected { button::secondary } else { button::text })
            .on_press(Message::BinPreview(path.to_owned(), kind));
        let add = |icon: &'static [u8], place: Place, hint: &'static str| -> Element<'a, Message> {
            let icon = iced::widget::svg(iced::widget::svg::Handle::from_memory(icon)).width(16).height(16);
            tip(button(icon).padding(4).on_press(Message::BinAdd(path.to_owned(), kind, place)), text(hint), tooltip::Position::Left)
        };
        let buttons: Element<'a, Message> = match kind {
            Kind::Clip => row![
                add(ADD_END, Place::End, "Add at the end of the edit"),
                add(ADD_HERE, Place::Playhead, "Add at the playhead: the cut there is split around it"),
                add(ADD_JOIN, Place::Join, "Add at the join of cuts nearest the playhead"),
            ]
            .spacing(2)
            .into(),
            Kind::Sound(_) => row![
                add(ADD_END, Place::End, "Add on the music lane after the last song"),
                add(ADD_HERE, Place::Playhead, "Add on the music lane at the playhead"),
                add(ADD_JOIN, Place::Join, "Add on the music lane at the join of cuts nearest the playhead"),
            ]
            .spacing(2)
            .into(),
            Kind::Picture => add(ADD_HERE, Place::Playhead, "Add over the picture at the playhead"),
        };
        row![body, buttons].spacing(4).align_y(Alignment::Center).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_above_04_project() {
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
        make("_trash/old.mp3");
        let s = scan(&dir);
        let titles: Vec<_> = s.iter().map(|s| s.title).collect();
        // Cleaned voice belongs to its clip: not listed.
        assert_eq!(titles, ["Footage", "Music", "Sound effects", "Graphics"]);
        assert_eq!(s[0].files, [dir.join("01_footage/DJI_0001.MP4")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn length_reads_as_minutes() {
        assert_eq!(clock(65.4), "1:05");
        assert_eq!(clock(3.0), "0:03");
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
            Message::BinPreview(path, kind) => return self.bin_preview(path, kind),
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
                return self.update(Message::Picked(vec![path]));
            }
            Message::BinAdd(path, Kind::Picture, _) => return self.update(Message::OverlayPicked(vec![path])),
            Message::BinAdd(path, Kind::Sound(which), place) => {
                // Onto its section's track, where it fits nearest the spot,
                // decided now from the playhead now.
                let at = match self.spot(place) {
                    Spot::End => None,
                    Spot::At(t) => Some(t),
                    Spot::Join(t) => Some(crate::project::offsets(&self.edit.cuts).into_iter().chain([self.edit.duration()]).nth(self.edit.nearest_join(t)).unwrap_or(t)),
                };
                return probe_sound(which, path, at);
            }
            _ => {}
        }
        Task::none()
    }
}
