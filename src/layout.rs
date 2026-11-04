//! A Dailix project folder: the numbered folders are the user's, made for
//! people to browse in a file manager; everything a program keeps (the
//! marking, the analysis, thumbnails, cleaned voices) lives in one folder
//! beside them, `dailix/` (user, 2026-11-04: footage-agent's projects
//! scattered `report/`, `analysis/`, `voice/` among the numbered ones).
//!
//! ```text
//! <project>/
//!   01_footage/<day>/<clip>   the clips, copied in (originals stay in the archive)
//!   02_audio/music, 02_audio/voiceover
//!   03_proxies/<day>/<clip>   the cameras' own light copies (DJI's LRF)
//!   04_project … 09_luts
//!   dailix/
//!     project.json            what the project is and where it came from
//!     sheet.json              the contact sheet's marking (`Doc`)
//!     analysis/<day>/<clip>.footage.json
//!     thumbs/<day>/<clip>/0001.jpg …
//!     voice/<day>/<clip>/<recipe>.flac
//! ```
//!
//! A clip's sheet key is its path under `01_footage`: `<day>/<file>`.

use std::path::{Path, PathBuf};

/// The numbered folders and what each is for (footage-agent's
/// `layout.py`, the same tree).
pub const FOLDERS: [(&str, &str); 10] = [
    ("01_footage", "the clips, copied in; the originals stay in the archive"),
    ("02_audio/music", "music"),
    ("02_audio/voiceover", "voice-over recorded on its own"),
    ("03_proxies", "the cameras' own light copies, for quick editing"),
    ("04_project", "edits: .json for Omnix, .mlt for Kdenlive"),
    ("05_export", "finished videos"),
    ("06_external", "outside video: memes, inserts, transitions"),
    ("07_sfx", "sound effects"),
    ("08_graphics", "pictures over the video: logos, captions, arrows"),
    ("09_luts", "LUT files (.cube) for this project's colour"),
];

/// The folders whose files come from elsewhere: each gets a `LICENSES.md`
/// to note where a file came from and on what terms.
const LICENSED: [(&str, &str); 4] = [("02_audio/music", "Music"), ("06_external", "Outside video"), ("07_sfx", "Sound effects"), ("08_graphics", "Graphics")];

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
    pub fn voice(&self) -> PathBuf {
        self.data().join("voice")
    }
    pub fn footage(&self) -> PathBuf {
        self.root.join("01_footage")
    }
    pub fn proxies(&self) -> PathBuf {
        self.root.join("03_proxies")
    }

    /// Whether `root` is a Dailix project (it has `dailix/project.json`).
    pub fn is_project(&self) -> bool {
        self.project().is_file()
    }

    /// The clip file a sheet key names.
    pub fn clip(&self, key: &str) -> PathBuf {
        self.footage().join(key)
    }

    /// The sheet key of a clip file under `01_footage`.
    pub fn key(&self, clip: &Path) -> Option<String> {
        let rel = clip.strip_prefix(self.footage()).ok()?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        (parts.len() >= 2).then(|| parts.join("/"))
    }

    /// Makes the folders (and the licence notes) not there yet; touches
    /// nothing that is, so it's safe on a project made before a folder
    /// was added. What it made.
    pub fn create(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut made = Vec::new();
        let dirs = FOLDERS.iter().map(|(d, _)| self.root.join(d)).chain([self.data(), self.analysis(), self.thumbs(), self.voice()]);
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
    fn a_clips_key_is_its_path_under_the_footage() {
        let l = Layout::new("/p");
        assert_eq!(l.key(Path::new("/p/01_footage/2030-05-01/A.MP4")).as_deref(), Some("2030-05-01/A.MP4"));
        assert_eq!(l.key(Path::new("/p/01_footage/A.MP4")), None);
        assert_eq!(l.key(Path::new("/q/01_footage/2030-05-01/A.MP4")), None);
        assert_eq!(l.clip("2030-05-01/A.MP4"), PathBuf::from("/p/01_footage/2030-05-01/A.MP4"));
        assert_eq!(l.sheet(), PathBuf::from("/p/dailix/sheet.json"));
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
