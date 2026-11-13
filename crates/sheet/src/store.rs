//! The sheet on disk. Other programs write it too (for now the web sheet
//! it replaces saves the whole file from a browser), so the store holds no
//! copy of its own between writes: each write reads the file afresh,
//! gives ids to marks that came without (another writer's new ones),
//! applies the ops to what is there now (they name marks by id, so they
//! land where they should), checks, and replaces the file whole, the last
//! version kept beside it as `notes.json.bak` (the sheet's server keeps
//! the same). The web sheet's server refuses a save from a page older
//! than the file (footage-agent `3a67dba`), so its page reloads rather
//! than undoing a write made here.

use crate::{Doc, Error, Op, OpError, Outcome};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A sheet's file.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

/// Why the store couldn't read or write.
#[derive(Debug)]
pub enum StoreError {
    Io(PathBuf, std::io::Error),
    Sheet(Error),
    /// The op at this place in the list was refused; nothing was written.
    Op(usize, OpError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            StoreError::Sheet(e) => e.fmt(f),
            StoreError::Op(i, e) => write!(f, "op {}: {e}", i + 1),
        }
    }
}

impl std::error::Error for StoreError {}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Store {
        Store { path: path.into() }
    }

    /// The sheet as it is now, with every mark given an id, written first
    /// if any had none: an id counts only once it's in the file.
    pub fn read(&self) -> Result<Doc, StoreError> {
        let mut doc = self.load()?;
        if doc.settle_ids() > 0 {
            self.write(&doc)?;
        }
        Ok(doc)
    }

    /// Applies `ops` to the sheet as it is now, all or none, and writes it.
    /// What each did, and the sheet as written.
    pub fn apply(&self, ops: &[Op]) -> Result<(Doc, Vec<Outcome>), StoreError> {
        let mut doc = self.load()?;
        let settled = doc.settle_ids() > 0;
        let mut outcomes = Vec::with_capacity(ops.len());
        for (i, op) in ops.iter().enumerate() {
            outcomes.push(doc.apply(op).map_err(|e| StoreError::Op(i, e))?);
        }
        if settled || !ops.is_empty() {
            self.write(&doc)?;
        }
        Ok((doc, outcomes))
    }

    fn load(&self) -> Result<Doc, StoreError> {
        let text = std::fs::read_to_string(&self.path).map_err(|e| StoreError::Io(self.path.clone(), e))?;
        Doc::parse(&text).map_err(StoreError::Sheet)
    }

    /// Replaces the file whole or not at all: written beside it, synced,
    /// renamed over it; the last version copied to `.bak` first.
    fn write(&self, doc: &Doc) -> Result<(), StoreError> {
        let io = |e| StoreError::Io(self.path.clone(), e);
        let dir = self.path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        std::fs::copy(&self.path, dir.join(format!("{name}.bak"))).map_err(io)?;
        replace(&self.path, doc.text().as_bytes()).map_err(io)
    }
}

/// Replaces the file at `path` whole or not at all: written beside it
/// (never through a link someone left at that name), synced, renamed over
/// it, keeping its permissions.
pub(crate) fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        if let Ok(meta) = std::fs::metadata(path) {
            file.set_permissions(meta.permissions())?;
        }
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipOp, MarkOp};

    const CLIP: &str = "2030-05-01/a.mp4";

    /// A sheet file in a folder of its own, removed after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str, text: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("dailix-store-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("notes.json"), text).unwrap();
            Scratch(dir)
        }
        fn file(&self) -> PathBuf {
            self.0.join("notes.json")
        }
        fn text(&self) -> String {
            std::fs::read_to_string(self.file()).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reading_gives_ids_and_writes_them_first() {
        let s = Scratch::new("read", r#"{"clips": {"2030-05-01/a.mp4": {"marks": [{"t": 1, "note": ""}]}}}"#);
        let doc = Store::new(s.file()).read().unwrap();
        assert_eq!(doc.sheet().unwrap().clips[CLIP].marks[0].id.as_deref(), Some("m1"));
        assert!(s.text().contains("\"id\": \"m1\""));
        assert!(s.0.join("notes.json.bak").is_file());
        // Read again: nothing to give, nothing written.
        let before = std::fs::metadata(s.file()).unwrap().modified().unwrap();
        Store::new(s.file()).read().unwrap();
        assert_eq!(std::fs::metadata(s.file()).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn ops_land_on_the_file_as_it_is_now() {
        let s = Scratch::new("two", "{}");
        let store = Store::new(s.file());
        let (_, out) = store.apply(&[Op::Mark(MarkOp::Add { clip: CLIP.into(), t: 1.0, end: Some(3.0), flag: false, role: None, note: "ours".into() })]).unwrap();
        let ours = out[0].made.clone().unwrap();
        // Another writer meanwhile: a whole file saved with a mark of its
        // own (no id) before ours, and a rating.
        let mut theirs: serde_json::Value = serde_json::from_str(&s.text()).unwrap();
        let marks = theirs["clips"][CLIP]["marks"].as_array_mut().unwrap();
        marks.insert(0, serde_json::json!({"t": 0.5, "note": "theirs"}));
        theirs["clips"][CLIP]["rating"] = serde_json::json!(2);
        std::fs::write(s.file(), serde_json::to_string_pretty(&theirs).unwrap()).unwrap();
        // Ours moved by id: it lands on ours, theirs gets an id, both kept.
        let (doc, _) = store.apply(&[Op::Mark(MarkOp::Move { mark: ours.clone(), t: 1.0, end: Some(4.0) })]).unwrap();
        let clip = &doc.sheet().unwrap().clips[CLIP];
        assert_eq!(clip.rating, Some(crate::Rating::Take));
        assert_eq!((clip.marks[0].note.as_str(), clip.marks[0].id.as_deref()), ("theirs", Some("m2")));
        assert_eq!((clip.marks[1].id.as_deref(), clip.marks[1].end), (Some(ours.as_str()), Some(4.0)));
    }

    #[test]
    fn a_refused_op_writes_nothing_and_leaves_nothing_behind() {
        let s = Scratch::new("refused", r#"{"clips": {}}"#);
        let text = s.text();
        let err = Store::new(s.file()).apply(&[Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(1) }), Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(7) })]).unwrap_err();
        assert!(matches!(err, StoreError::Op(1, OpError::Invalid(_))), "{err}");
        assert_eq!(s.text(), text);
        let left: Vec<_> = std::fs::read_dir(&s.0).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["notes.json"]);
    }
}
