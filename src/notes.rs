//! The contact sheet's marks: footage-agent's `report/notes.json`, read so
//! the bin's player shows a clip's segments, and changed when one is set,
//! described or deleted here (gaps 71a, 72e; user, 2026-11-04: one list,
//! both ways).
//!
//! Reading and changing go through the sheet's core, Dailix's
//! `dailix-sheet` (`../dailix`, `docs/SHEET-CORE.md`): the file is
//! read afresh for every change, marks are named by their ids, every op
//! is checked, and the file keeps its form and every field it has. What
//! stays here is Omnix's side: where a project's sheet is, and which
//! sheet key a clip file has.

use anyhow::{Context, Result, bail};
use dailix_sheet::{MarkKind, MarkOp, Op, Store};
use std::path::{Path, PathBuf};

/// A mark of the sheet on a clip: where it is and what it's for.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Its place in the clip's marks as read, for the strip.
    pub index: usize,
    /// The sheet's id for it: how it's changed. None only for a segment
    /// kept in Omnix (a project without a sheet).
    pub id: Option<String>,
    pub start: f64,
    /// `None` for a point or a flag.
    pub end: Option<f64>,
    pub role: Option<String>,
    pub note: String,
}

/// A role a segment can have: its key in the sheet, its name there
/// ("перебивка / cutaway") and its colour, as the sheet shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub key: String,
    pub name: String,
    pub rgb: [u8; 3],
}

impl From<dailix_sheet::Role> for Role {
    fn from(r: dailix_sheet::Role) -> Self {
        Role { name: r.name(), rgb: r.rgb(), key: r.key }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// A project folder's sheet: its file and how its clips are keyed.
#[derive(Debug, Clone)]
pub struct Sheet {
    store: Store,
    /// The folders a clip's key is relative to: the archive the sheet was
    /// made from, then the project's own footage folder.
    bases: Vec<PathBuf>,
}

impl Sheet {
    /// The sheet of the project folder `root`, if it has one: a Dailix
    /// project's (`dailix/sheet.json`, clips keyed under `01_footage`),
    /// else footage-agent's (`report/notes.json`, keyed under the archive
    /// it names, then `01_footage`).
    pub fn of(root: &Path) -> Option<Sheet> {
        let dailix = dailix_sheet::Layout::new(root);
        if dailix.is_project() && dailix.sheet().is_file() {
            return Some(Sheet { store: Store::new(dailix.sheet()), bases: vec![dailix.footage()] });
        }
        let file = root.join("report").join("notes.json");
        if !file.is_file() {
            return None;
        }
        let source: Option<serde_json::Value> = std::fs::read(root.join("report").join("_source.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let archive = source.as_ref().and_then(|s| s["archive"].as_str()).map(PathBuf::from);
        let bases = archive.into_iter().chain([root.join("01_footage")]).collect();
        Some(Sheet { store: Store::new(file), bases })
    }

    /// The sheet's key for `file`: `<day>/<name>`, by the first folder it
    /// is under.
    pub fn key(&self, file: &Path) -> Option<String> {
        let rel = self.bases.iter().find_map(|b| file.strip_prefix(b).ok())?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        (!parts.is_empty()).then(|| parts.join("/"))
    }

    fn key_of(&self, file: &Path) -> Result<String> {
        match self.key(file) {
            Some(k) => Ok(k),
            None => bail!("{} isn't in the contact sheet's archive", file.display()),
        }
    }

    /// The marks on `file`, in the sheet's order. Reading gives the
    /// sheet's marks their ids, written into it first if any had none.
    pub fn segments(&self, file: &Path) -> Result<Vec<Segment>> {
        let Some(key) = self.key(file) else { return Ok(Vec::new()) };
        let sheet = self.store.read().context("reading the contact sheet")?.sheet()?;
        let Some(clip) = sheet.clips.get(&key) else { return Ok(Vec::new()) };
        Ok(clip
            .marks
            .iter()
            .enumerate()
            .map(|(index, m)| Segment { index, id: m.id.clone(), start: m.t, end: (m.kind == MarkKind::Segment).then_some(m.end).flatten(), role: m.role.clone(), note: m.note.clone() })
            .collect())
    }

    /// The sheet's own roles, for a project without a sheet.
    pub fn default_roles() -> Vec<Role> {
        dailix_sheet::Sheet::default().all_roles().into_iter().map(Role::from).collect()
    }

    /// The roles to pick from: the sheet's own, then the user's added in it.
    pub fn roles(&self) -> Vec<Role> {
        match self.store.read().map_err(anyhow::Error::from).and_then(|d| Ok(d.sheet()?)) {
            Ok(sheet) => sheet.all_roles().into_iter().map(Role::from).collect(),
            Err(_) => Self::default_roles(),
        }
    }

    /// Applies `ops` to the sheet as it is now; the id the first made, if
    /// it made one.
    fn apply(&self, ops: &[Op]) -> Result<Option<String>> {
        let (_, outcomes) = self.store.apply(ops).context("writing the contact sheet")?;
        Ok(outcomes.into_iter().find_map(|o| o.made))
    }

    /// The segment from `start` to `end` on `file`: the mark `was` moved,
    /// or a new one. Says where it now is in the clip's list.
    pub fn set(&self, file: &Path, was: Option<&Segment>, start: f64, end: f64) -> Result<usize> {
        let id = match was.and_then(|w| w.id.clone()) {
            Some(mark) => {
                self.apply(&[Op::Mark(MarkOp::Move { mark: mark.clone(), t: start, end: Some(end) })])?;
                mark
            }
            None => {
                let clip = self.key_of(file)?;
                let made = self.apply(&[Op::Mark(MarkOp::Add { clip, t: start, end: Some(end), flag: false, role: None, note: String::new() })])?;
                made.context("the sheet made no mark")?
            }
        };
        self.segments(file)?.iter().position(|s| s.id.as_deref() == Some(id.as_str())).context("the mark isn't in the sheet after writing it")
    }

    /// Gives the mark `was` names a role (none removes it) and a note.
    pub fn describe(&self, was: &Segment, role: Option<&str>, note: &str) -> Result<()> {
        let mark = was.id.clone().context("the mark has no id")?;
        self.apply(&[Op::Mark(MarkOp::Role { mark: mark.clone(), role: role.map(str::to_owned) }), Op::Mark(MarkOp::Note { mark, note: note.to_owned() })])?;
        Ok(())
    }

    /// Removes the mark `was` names.
    pub fn remove(&self, was: &Segment) -> Result<()> {
        let mark = was.id.clone().context("the mark has no id")?;
        self.apply(&[Op::Mark(MarkOp::Remove { mark })])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet_in(dir: &Path, notes: &str) -> Sheet {
        std::fs::create_dir_all(dir.join("report")).unwrap();
        std::fs::write(dir.join("report/notes.json"), notes).unwrap();
        std::fs::write(dir.join("report/_source.json"), format!(r#"{{"archive": "{}"}}"#, dir.join("archive").display())).unwrap();
        Sheet::of(dir).unwrap()
    }

    #[test]
    fn a_clip_is_keyed_by_its_day_and_name_under_the_archive_or_footage() {
        let s = Sheet { store: Store::new("/x"), bases: vec![PathBuf::from("/a"), PathBuf::from("/p/01_footage")] };
        assert_eq!(s.key(Path::new("/a/2026-08-21/x.MP4")).as_deref(), Some("2026-08-21/x.MP4"));
        assert_eq!(s.key(Path::new("/p/01_footage/2026-08-22/y.MP4")).as_deref(), Some("2026-08-22/y.MP4"));
        assert_eq!(s.key(Path::new("/elsewhere/z.MP4")), None);
    }

    #[test]
    fn a_dailix_project_is_found_first() {
        let dir = std::env::temp_dir().join(format!("omnix-notes-dailix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let layout = dailix_sheet::Layout::new(&dir);
        layout.create().unwrap();
        std::fs::write(layout.project(), "{}").unwrap();
        std::fs::write(layout.sheet(), r#"{"clips": {"2030-05-01/a.mp4": {"marks": [{"t": 1, "end": 2, "note": "", "id": "m1"}]}}}"#).unwrap();
        // An old sheet beside it is ignored.
        sheet_in(&dir, r#"{"clips": {}}"#);
        let s = Sheet::of(&dir).unwrap();
        assert_eq!(s.segments(&dir.join("01_footage/2030-05-01/a.mp4")).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn segments_are_read_moved_described_and_removed_through_the_core() {
        let dir = std::env::temp_dir().join(format!("omnix-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = sheet_in(
            &dir,
            r#"{"days": {"2026-08-21": {"note": "день"}}, "clips": {"2026-08-21/x.MP4": {"rating": 2, "note": "кіт", "marks": [
                {"t": 1.5, "end": 4.0, "role": "broll", "note": "фон"},
                {"t": 7.0, "note": "точка"}]}}, "ui": {"shut": []}}"#,
        );
        let file = dir.join("archive/2026-08-21/x.MP4");
        // Read: ids given, and written.
        let marks = s.segments(&file).unwrap();
        assert_eq!(marks.iter().map(|m| m.id.as_deref()).collect::<Vec<_>>(), [Some("m1"), Some("m2")]);
        assert_eq!((marks[0].start, marks[0].end, marks[0].role.as_deref()), (1.5, Some(4.0), Some("broll")));
        assert_eq!((marks[1].start, marks[1].end), (7.0, None));
        let text = std::fs::read_to_string(dir.join("report/notes.json")).unwrap();
        assert!(text.contains("\"id\": \"m1\""));

        // Moved: the same mark, its role and note kept, the rest untouched.
        assert_eq!(s.set(&file, Some(&marks[0]), 2.0, 5.25).unwrap(), 0);
        let after = s.segments(&file).unwrap();
        assert_eq!((after[0].start, after[0].end, after[0].note.as_str()), (2.0, Some(5.25), "фон"));
        let all: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("report/notes.json")).unwrap()).unwrap();
        assert_eq!(all["days"]["2026-08-21"]["note"], "день");
        assert_eq!(all["clips"]["2026-08-21/x.MP4"]["rating"], 2);
        assert!(dir.join("report/notes.json.bak").is_file());

        // A point given an end becomes a segment; a new one is added.
        assert_eq!(s.set(&file, Some(&after[1]), 7.0, 9.0).unwrap(), 1);
        assert_eq!(s.set(&file, None, 10.0, 12.0).unwrap(), 2);

        // Described, and a role taken away.
        let m = &s.segments(&file).unwrap()[0];
        s.describe(m, Some("cut"), "крик").unwrap();
        let m = &s.segments(&file).unwrap()[0];
        assert_eq!((m.role.as_deref(), m.note.as_str()), (Some("cut"), "крик"));
        s.describe(m, None, "крик").unwrap();
        assert_eq!(s.segments(&file).unwrap()[0].role, None);
        assert!(s.roles().iter().any(|r| r.key == "cut" && r.name == "перебивка / cutaway"));

        // Removed by id: the others stay; removed again, refused.
        let before = s.segments(&file).unwrap();
        s.remove(&before[1]).unwrap();
        let after = s.segments(&file).unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[1].id, before[2].id);
        assert!(s.remove(&before[1]).is_err());

        // A clip the sheet has no entry for gets one.
        let new = dir.join("archive/2026-08-22/y.MP4");
        assert_eq!(s.set(&new, None, 0.0, 1.0).unwrap(), 0);
        assert_eq!(s.segments(&new).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
