//! A Dailix project folder: the numbered folders are the user's, made for
//! people to browse in a file manager; everything a program keeps (the
//! marking, the analysis, thumbnails, cleaned voices) lives in one folder
//! beside them, `dailix/` (user, 2026-11-04: the projects of his earlier
//! pipeline, footage-agent,
//! scattered `report/`, `analysis/`, `voice/` among the numbered ones).
//!
//! ```text
//! <project>/
//!   01_footage/<day>/<clip>   the clips, copied in (originals stay in the archive)
//!   02_audio/music, 02_audio/voiceover
//!   03_proxies/<day>/<clip>   light copies for quick editing (none made yet)
//!   06_external … 09_luts
//!   videos/<name>/            a video made from the trip (user, 2026-11-08):
//!     edit.json                 its edit, in Omnix
//!     export/                   what it was exported as
//!   dailix/
//!     project.json            what the project is and where it came from
//!     sheet.json              the contact sheet's marking (`Doc`)
//!     analysis/<day>/<clip>.footage.json
//!     thumbs/<day>/<clip>.jpg  a poster per clip, made when first shown
//!     voice/<day>/<clip>/<recipe>.flac
//! ```
//!
//! A clip's sheet key is its path under `01_footage`: `<day>/<file>`.

use std::path::{Path, PathBuf};

/// Pictures and photos to lay over the video (Omnix gap 91: named simply,
/// as the user asked); projects made before it have `OLD_PICTURES`,
/// still read.
pub const PICTURES: &str = "08_pictures";
pub const OLD_PICTURES: &str = "08_graphics";

/// The numbered folders (the same tree as footage-agent's, the user's
/// earlier pipeline).
pub const FOLDERS: [&str; 9] = [
    "01_footage",         // the clips, copied in; the originals stay in the sources
    "02_audio/music",     // music
    "02_audio/voiceover", // voice-over recorded on its own
    "03_proxies",         // light copies for quick editing (none made yet)
    "videos",             // a folder per video: its edit and its exports
    "06_external",        // outside video: memes, inserts, transitions
    "07_sfx",             // sound effects
    PICTURES,             // pictures and photos over the video: logos, captions, arrows
    "09_luts",            // LUT files (.cube) for this project's colour
];

/// A video's edit, in its folder.
pub const EDIT: &str = "edit.json";
/// A video's exports, in its folder.
pub const EXPORT: &str = "export";

/// The folders whose files come from elsewhere: each gets a `LICENSES.md`
/// to note where a file came from and on what terms.
const LICENSED: [(&str, &str); 4] = [("02_audio/music", "Music"), ("06_external", "Outside video"), ("07_sfx", "Sound effects"), (PICTURES, "Pictures")];

/// Where the parts of the project folder `root` are.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Layout {
        Layout { root: root.into() }
    }

    /// The folder of everything Dailix keeps.
    pub fn data(&self) -> PathBuf {
        self.root.join("dailix")
    }
    pub fn project(&self) -> PathBuf {
        self.data().join("project.json")
    }
    pub fn sheet(&self) -> PathBuf {
        self.data().join("sheet.json")
    }
    pub fn analysis(&self) -> PathBuf {
        self.data().join("analysis")
    }
    pub fn thumbs(&self) -> PathBuf {
        self.data().join("thumbs")
    }
    /// A clip's poster: `thumbs/<day>/<clip stem>.jpg`.
    pub fn poster(&self, key: &str) -> PathBuf {
        let p = Path::new(key);
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.thumbs().join(p.parent().unwrap_or(Path::new(""))).join(format!("{stem}.jpg"))
    }
    /// A video's card frames (Omnix's project cards, gap 110): a strip
    /// of its edit's frames, made by Omnix.
    pub fn card(&self, video: &str) -> PathBuf {
        self.data().join("cards").join(format!("{video}.jpg"))
    }
    pub fn voice(&self) -> PathBuf {
        self.data().join("voice")
    }
    pub fn footage(&self) -> PathBuf {
        self.root.join("01_footage")
    }
    /// The trip's videos, a folder each.
    pub fn videos(&self) -> PathBuf {
        self.root.join("videos")
    }
    /// Video `name`'s folder, its edit and where it exports to.
    pub fn video(&self, name: &str) -> PathBuf {
        self.videos().join(name)
    }
    pub fn video_edit(&self, name: &str) -> PathBuf {
        self.video(name).join(EDIT)
    }
    /// Where edits lay loose before the videos' folders (gap 82): only
    /// read, to move them in (`project::tidy_old_edits`).
    pub fn old_edits(&self) -> PathBuf {
        self.root.join("04_project")
    }

    /// Whether `root` is a Dailix project (it has `dailix/project.json`).
    pub fn is_project(&self) -> bool {
        self.project().is_file()
    }

    /// The clip file a sheet key names.
    pub fn clip(&self, key: &str) -> PathBuf {
        self.footage().join(key)
    }

    /// Makes the folders (and the licence notes) not there yet; touches
    /// nothing that is, so it's safe on a project made before a folder
    /// was added. What it made.
    pub fn create(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut made = Vec::new();
        let dirs = FOLDERS.iter().map(|d| self.root.join(d)).chain([self.data(), self.analysis(), self.thumbs(), self.voice()]);
        for d in dirs {
            if !d.is_dir() {
                std::fs::create_dir_all(&d)?;
                made.push(d);
            }
        }
        for (dir, what) in LICENSED {
            let f = self.root.join(dir).join("LICENSES.md");
            if !f.exists() {
                std::fs::write(&f, format!("# {what}\n\nA line per file: where it came from, its licence, whether it needs credit.\n\n| file | source | licence | credit |\n|------|--------|---------|--------|\n"))?;
                made.push(f);
            }
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clips_key_names_its_file_under_the_footage() {
        let l = Layout::new("/p");
        assert_eq!(l.clip("2030-05-01/A.MP4"), PathBuf::from("/p/01_footage/2030-05-01/A.MP4"));
        assert_eq!(l.sheet(), PathBuf::from("/p/dailix/sheet.json"));
        assert_eq!(l.poster("2030-05-01/A.MP4"), PathBuf::from("/p/dailix/thumbs/2030-05-01/A.jpg"));
        assert_eq!(l.card("Day one"), PathBuf::from("/p/dailix/cards/Day one.jpg"));
    }

    #[test]
    fn creating_makes_what_is_missing_and_keeps_the_rest() {
        let root = std::env::temp_dir().join(format!("dailix-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("06_external")).unwrap();
        std::fs::write(root.join("06_external/LICENSES.md"), "mine").unwrap();
        let l = Layout::new(&root);
        let made = l.create().unwrap();
        assert!(root.join("02_audio/voiceover").is_dir() && root.join("dailix/thumbs").is_dir());
        assert_eq!(std::fs::read_to_string(root.join("06_external/LICENSES.md")).unwrap(), "mine");
        assert!(!made.contains(&root.join("06_external")));
        assert!(l.create().unwrap().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
