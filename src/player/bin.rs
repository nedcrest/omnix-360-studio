//! The bin: the media of the project's folder, sorted by kind, in a panel at
//! the left of the view. A click adds a file where its kind goes: a clip as a
//! cut, a sound on the music lane at the playhead, a picture over the picture.
//!
//! The project's folder is the one holding the project file, or the one
//! above it when the file sits in a `04_project` folder: the layout
//! footage-agent makes (`footage_agent/layout.py` there), where footage,
//! music and sounds live in numbered folders beside `04_project`.
//!
//! It reads the folders when it opens and on Refresh, not on every frame:
//! a few hundred names, read in a blink.

use super::{App, Message, tip};
use iced::widget::{button, column, container, row, rule, scrollable, text, tooltip};
use iced::{Alignment, Element, Length};
use std::path::{Path, PathBuf};

const WIDTH: f32 = 300.0;

/// What a file is for, and so where a click puts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Clip,
    Sound,
    Picture,
}

/// A heading and the folders under the project's folder it gathers.
/// Cleaned voice is two levels deeper (`voice/<day>/<clip>/<recipe>.flac`),
/// so its folder is walked down.
const SECTIONS: [(&str, Kind, &[&str]); 7] = [
    ("Footage", Kind::Clip, &["01_footage"]),
    ("External video", Kind::Clip, &["06_external"]),
    ("Music", Kind::Sound, &["02_audio/music"]),
    ("Voice-over", Kind::Sound, &["02_audio/voiceover"]),
    ("Cleaned voice", Kind::Sound, &["voice"]),
    ("Sound effects", Kind::Sound, &["07_sfx"]),
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

/// The project's folder for a project file at `project`.
pub fn root(project: &Path) -> Option<PathBuf> {
    let dir = project.parent()?;
    if dir.file_name().is_some_and(|n| n == "04_project") { dir.parent().map(Path::to_owned) } else { Some(dir.to_owned()) }
}

fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let is = |list: &[&str]| list.contains(&ext.as_str());
    if is(CLIP_EXT) {
        Some(Kind::Clip)
    } else if is(SOUND_EXT) {
        Some(Kind::Sound)
    } else if is(PICTURE_EXT) {
        Some(Kind::Picture)
    } else {
        None
    }
}

/// The files under `dir` of `kind`, down `depth` more folders, by path.
fn files(dir: &Path, kind: Kind, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.') || n.starts_with('_')) {
            continue;
        }
        if path.is_dir() {
            if depth > 0 {
                files(&path, kind, depth - 1, out);
            }
        } else if kind_of(&path) == Some(kind) {
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
                let depth = if *dir == "voice" { 2 } else { 0 };
                files(&root.join(dir), kind, depth, &mut found);
            }
            found.sort();
            (!found.is_empty()).then_some(Section { title, kind, files: found })
        })
        .collect()
}

/// A file's name as the bin lists it: cleaned voice by its clip and recipe
/// (`DJI_0001 · dnn6`), as its own name is only the recipe.
fn label(section: &Section, path: &Path) -> String {
    let name = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if section.title == "Cleaned voice"
        && let Some(clip) = path.parent().and_then(Path::file_name)
    {
        return format!("{} · {name}", clip.to_string_lossy());
    }
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name)
}

impl App {
    /// The project's folder, read afresh.
    pub(super) fn rescan_bin(&mut self) {
        self.bin = self.project.as_deref().and_then(root).map(|r| (scan(&r), r));
    }

    /// The bin panel, while it's open.
    pub(super) fn bin_panel(&self) -> Option<Element<'_, Message>> {
        if !self.bin_open {
            return None;
        }
        let close = tip(button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(Message::BinPanel), text("Close (B)"), tooltip::Position::Bottom);
        let refresh = tip(button(text("↻").size(14)).padding([0, 6]).style(button::text).on_press(Message::BinRefresh), text("Read the folder again"), tooltip::Position::Bottom);
        let mut body = column![row![text("Bin").size(18).width(Length::Fill), refresh, close].align_y(Alignment::Center)].spacing(10);

        // The clips already loaded, wherever they live: a project's footage
        // is often in an archive outside its folder.
        let mut loaded: Vec<&Path> = self.library.iter().map(|m| m.input.as_path()).collect();
        loaded.sort();
        loaded.dedup();

        match &self.bin {
            None => body = body.push(text("Save the project to see its folder here.").size(13).style(text::secondary)),
            Some((sections, root)) => {
                body = body.push(text(root.display().to_string()).size(12).style(text::secondary));
                if sections.is_empty() {
                    body = body.push(text("No media in the project's folders yet.").size(13).style(text::secondary));
                }
                for section in sections {
                    body = body.push(rule::horizontal(1)).push(text(section.title).size(15));
                    for path in &section.files {
                        body = body.push(item(label(section, path), path, section.kind));
                    }
                }
            }
        }
        if !loaded.is_empty() {
            body = body.push(rule::horizontal(1)).push(text("Loaded clips").size(15));
            for path in loaded {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                body = body.push(item(name, path, Kind::Clip));
            }
        }
        Some(container(scrollable(body.padding(12))).width(WIDTH).height(Length::Fill).into())
    }
}

/// A file as a row: click to add it.
fn item<'a>(name: String, path: &Path, kind: Kind) -> Element<'a, Message> {
    let hint = match kind {
        Kind::Clip => "Add at the end of the edit",
        Kind::Sound => "Add on the music lane at the playhead",
        Kind::Picture => "Add over the picture at the playhead",
    };
    tip(
        button(text(name).size(13)).width(Length::Fill).style(button::text).padding([2, 4]).on_press(Message::BinAdd(path.to_owned(), kind)),
        text(hint),
        tooltip::Position::Right,
    )
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
        assert_eq!(titles, ["Footage", "Music", "Cleaned voice", "Sound effects", "Graphics"]);
        assert_eq!(s[0].files, [dir.join("01_footage/DJI_0001.MP4")]);
        assert_eq!(label(&s[2], &s[2].files[0]), "DJI_0001 · dnn6");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
