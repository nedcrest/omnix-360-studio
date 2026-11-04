//! The contact sheet's marks: footage-agent's `report/notes.json`, read so
//! the bin's player shows a clip's segments, and written back when one is
//! set or moved in Omnix (gaps 71a, 72e; user, 2026-11-04: one list, both
//! ways).
//!
//! The sheet keys a clip by `<day>/<file>` under its archive (named in
//! `report/_source.json`); a mark is `{t, end, role, note}`, a segment, or
//! a point without `end`. Everything else in the file is the sheet's and
//! is kept as it is: Omnix reads the file afresh before each write and
//! changes only the mark it set, so the sheet's own edits on disk survive.
//! A page of the sheet left open still holds its own copy and saves it
//! whole; footage-agent's side refuses a stale save (see its
//! `CONTACT-SHEET-NEXT.md`).

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// A mark of the sheet on a clip: where it is and what it's for.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Its place in the clip's `marks`, to write it back.
    pub index: usize,
    pub start: f64,
    /// `None` for a point.
    pub end: Option<f64>,
    pub role: Option<String>,
    pub note: String,
}

/// A role a segment can have: its key in the sheet and its name there
/// ("перебивка / cutaway"), as the sheet's own list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub key: String,
    pub name: String,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// The sheet's own roles (footage-agent's `report.html`): the function of
/// a shot in the edit, not what's in it. The English word is the trade's.
const ROLES: [(&str, &str, &str); 8] = [
    ("talk", "розмова", "sync"),
    ("establish", "загальний", "establishing"),
    ("broll", "фон", "B-roll"),
    ("detail", "деталь", "insert"),
    ("cut", "перебивка", "cutaway"),
    ("action", "дія", "action"),
    ("reveal", "ревіл", "reveal"),
    ("reaction", "реакція", "reaction"),
];

/// A project folder's sheet: where its file is and how its clips are keyed.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub file: PathBuf,
    /// The folders a clip's key is relative to: the archive the sheet was
    /// made from, then the project's own footage folder.
    bases: Vec<PathBuf>,
}

impl Sheet {
    /// The sheet of the project folder `root`, if it has one.
    pub fn of(root: &Path) -> Option<Sheet> {
        let file = root.join("report").join("notes.json");
        if !file.is_file() {
            return None;
        }
        let source: Option<Value> = std::fs::read(root.join("report").join("_source.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let archive = source.as_ref().and_then(|s| s["archive"].as_str()).map(PathBuf::from);
        let bases = archive.into_iter().chain([root.join("01_footage")]).collect();
        Some(Sheet { file, bases })
    }

    /// The sheet's key for `file`: `<day>/<name>`, by the first folder it
    /// is under.
    pub fn key(&self, file: &Path) -> Option<String> {
        let rel = self.bases.iter().find_map(|b| file.strip_prefix(b).ok())?;
        let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        (!parts.is_empty()).then(|| parts.join("/"))
    }

    fn read(&self) -> Result<Value> {
        let bytes = std::fs::read(&self.file).with_context(|| format!("reading {}", self.file.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("reading {}", self.file.display()))
    }

    /// The marks on `file`, in the sheet's order.
    pub fn segments(&self, file: &Path) -> Result<Vec<Segment>> {
        let Some(key) = self.key(file) else { return Ok(Vec::new()) };
        Ok(marks_of(&self.read()?, &key))
    }

    /// The sheet's own roles, for a project without a sheet.
    pub fn default_roles() -> Vec<Role> {
        ROLES.iter().map(|(k, t, en)| Role { key: (*k).to_owned(), name: format!("{t} / {en}") }).collect()
    }

    /// The roles to pick from: the sheet's own, then the user's added in it
    /// (`roles`: key, title, en).
    pub fn roles(&self) -> Vec<Role> {
        let mut roles = Self::default_roles();
        if let Ok(notes) = self.read()
            && let Some(own) = notes["roles"].as_array()
        {
            for r in own {
                let Some(key) = r["key"].as_str() else { continue };
                let title = r["title"].as_str().unwrap_or(key);
                let name = match r["en"].as_str().filter(|e| !e.is_empty()) {
                    Some(en) => format!("{title} / {en}"),
                    None => title.to_owned(),
                };
                if !roles.iter().any(|x| x.key == key) {
                    roles.push(Role { key: key.to_owned(), name });
                }
            }
        }
        roles
    }

    /// Gives the mark `was` names a role (none removes it) and a note, and
    /// saves the sheet. Says where the mark is; an error if the sheet moved
    /// or removed it meanwhile.
    pub fn describe(&self, file: &Path, was: &Segment, role: Option<&str>, note: &str) -> Result<usize> {
        let Some(key) = self.key(file) else { bail!("{} isn't in the contact sheet's archive", file.display()) };
        let mut notes = self.read()?;
        let m = notes["clips"][key.as_str()]["marks"].get_mut(was.index).filter(|m| m["t"].as_f64() == Some(was.start) && m["end"].as_f64() == was.end);
        let Some(m) = m.and_then(Value::as_object_mut) else { bail!("the mark was changed in the contact sheet meanwhile: pick it again") };
        match role {
            Some(r) => m.insert("role".into(), json!(r)),
            None => m.remove("role"),
        };
        m.insert("note".into(), json!(note));
        self.save(&notes)?;
        Ok(was.index)
    }

    /// Removes the mark `was` names from `file`'s list and saves the sheet;
    /// an error if the sheet moved or removed it meanwhile.
    pub fn remove(&self, file: &Path, was: &Segment) -> Result<()> {
        let Some(key) = self.key(file) else { bail!("{} isn't in the contact sheet's archive", file.display()) };
        let mut notes = self.read()?;
        let marks = notes["clips"][key.as_str()]["marks"].as_array_mut();
        let Some(marks) = marks.filter(|m| m.get(was.index).is_some_and(|m| m["t"].as_f64() == Some(was.start) && m["end"].as_f64() == was.end)) else {
            bail!("the mark was changed in the contact sheet meanwhile: pick it again")
        };
        marks.remove(was.index);
        self.save(&notes)
    }

    /// Writes `notes` over the sheet, its last version kept beside it.
    fn save(&self, notes: &Value) -> Result<()> {
        let mut text = serde_json::to_string_pretty(notes)?;
        text.push('\n');
        // The sheet's server keeps the last version beside it; so do we.
        std::fs::copy(&self.file, self.file.with_extension("json.bak")).with_context(|| format!("backing up {}", self.file.display()))?;
        crate::fsutil::replace_file(&self.file, text.as_bytes())
    }

    /// Sets a segment on `file` from `start` to `end` and saves the sheet:
    /// the mark `was` names moved, if it's still there as it was, else a new
    /// mark. Says where the mark now is in the clip's list.
    pub fn set(&self, file: &Path, was: Option<&Segment>, start: f64, end: f64) -> Result<usize> {
        let Some(key) = self.key(file) else { bail!("{} isn't in the contact sheet's archive", file.display()) };
        let mut notes = self.read()?;
        let index = set_mark(&mut notes, &key, was, start, end)?;
        self.save(&notes)?;
        Ok(index)
    }
}

/// Seconds rounded as the sheet writes them (a tenth of a millisecond).
fn round(t: f64) -> f64 {
    (t * 1000.0).round() / 1000.0
}

fn marks_of(notes: &Value, key: &str) -> Vec<Segment> {
    let Some(marks) = notes["clips"][key]["marks"].as_array() else { return Vec::new() };
    marks
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            Some(Segment {
                index,
                start: m["t"].as_f64()?,
                end: m["end"].as_f64(),
                role: m["role"].as_str().filter(|r| !r.is_empty()).map(str::to_owned),
                note: m["note"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// Puts the segment into `notes`: over the mark `was` names when that mark
/// is still there unchanged (the sheet may have been edited meanwhile),
/// else as a new mark at the end of the clip's list.
fn set_mark(notes: &mut Value, key: &str, was: Option<&Segment>, start: f64, end: f64) -> Result<usize> {
    let (start, end) = (round(start.min(end)), round(start.max(end)));
    let clips = notes.as_object_mut().context("the sheet isn't an object")?.entry("clips").or_insert_with(|| json!({}));
    let clip = clips.as_object_mut().context("the sheet's clips aren't an object")?.entry(key).or_insert_with(|| json!({}));
    let marks = clip.as_object_mut().context("a clip in the sheet isn't an object")?.entry("marks").or_insert_with(|| json!([]));
    let marks = marks.as_array_mut().context("a clip's marks aren't a list")?;
    let same = |m: &Value, w: &Segment| m["t"].as_f64() == Some(w.start) && m["end"].as_f64() == w.end;
    if let Some(w) = was
        && let Some(m) = marks.get_mut(w.index).filter(|m| same(m, w))
        && let Some(m) = m.as_object_mut()
    {
        m.insert("t".into(), json!(start));
        m.insert("end".into(), json!(end));
        return Ok(w.index);
    }
    let mut m = Map::new();
    m.insert("t".into(), json!(start));
    m.insert("end".into(), json!(end));
    m.insert("note".into(), json!(""));
    marks.push(Value::Object(m));
    Ok(marks.len() - 1)
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
        let s = Sheet { file: PathBuf::new(), bases: vec![PathBuf::from("/a"), PathBuf::from("/p/01_footage")] };
        assert_eq!(s.key(Path::new("/a/2026-08-21/x.MP4")).as_deref(), Some("2026-08-21/x.MP4"));
        assert_eq!(s.key(Path::new("/p/01_footage/2026-08-22/y.MP4")).as_deref(), Some("2026-08-22/y.MP4"));
        assert_eq!(s.key(Path::new("/elsewhere/z.MP4")), None);
    }

    #[test]
    fn segments_and_points_are_read_and_a_moved_one_is_written_back_alone() {
        let dir = std::env::temp_dir().join(format!("omnix-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = sheet_in(
            &dir,
            r#"{"days": {"2026-08-21": {"note": "день"}}, "clips": {"2026-08-21/x.MP4": {"rating": 2, "note": "кіт", "marks": [
                {"t": 1.5, "end": 4.0, "role": "broll", "note": "фон"},
                {"t": 7.0, "note": "точка"}]}}, "ui": {"shut": []}}"#,
        );
        let file = dir.join("archive/2026-08-21/x.MP4");
        let marks = s.segments(&file).unwrap();
        assert_eq!(marks.len(), 2);
        assert_eq!((marks[0].start, marks[0].end, marks[0].role.as_deref()), (1.5, Some(4.0), Some("broll")));
        assert_eq!((marks[1].start, marks[1].end), (7.0, None));

        // Moved: the same mark, its role and note kept, the rest untouched.
        assert_eq!(s.set(&file, Some(&marks[0]), 2.0, 5.25).unwrap(), 0);
        let after = s.segments(&file).unwrap();
        assert_eq!((after[0].start, after[0].end, after[0].note.as_str()), (2.0, Some(5.25), "фон"));
        let all: Value = serde_json::from_slice(&std::fs::read(&s.file).unwrap()).unwrap();
        assert_eq!(all["days"]["2026-08-21"]["note"], "день");
        assert_eq!(all["clips"]["2026-08-21/x.MP4"]["rating"], 2);
        assert!(dir.join("report/notes.json.bak").is_file());
        // The sheet's own order of fields is kept.
        let text = std::fs::read_to_string(&s.file).unwrap();
        assert!(text.find("\"days\"").unwrap() < text.find("\"clips\"").unwrap() && text.find("\"clips\"").unwrap() < text.find("\"ui\"").unwrap());
        assert!(text.contains("\"t\": 2.0,\n") || text.contains("\"t\": 2.0\n"));

        // Moved by the sheet meanwhile: Omnix's change becomes a new mark
        // rather than overwriting the sheet's.
        assert_eq!(s.set(&file, Some(&marks[0]), 9.0, 8.0).unwrap(), 2);
        let after = s.segments(&file).unwrap();
        assert_eq!((after[2].start, after[2].end), (8.0, Some(9.0)));

        // A role and a note given; a role taken away.
        let m = &s.segments(&file).unwrap()[0];
        s.describe(&file, m, Some("cut"), "крик").unwrap();
        let m = &s.segments(&file).unwrap()[0];
        assert_eq!((m.role.as_deref(), m.note.as_str()), (Some("cut"), "крик"));
        s.describe(&file, m, None, "крик").unwrap();
        assert_eq!(s.segments(&file).unwrap()[0].role, None);
        assert!(s.roles().iter().any(|r| r.key == "cut" && r.name == "перебивка / cutaway"));

        // Removed: the others stay, in order.
        let before = s.segments(&file).unwrap();
        s.remove(&file, &before[1]).unwrap();
        let after = s.segments(&file).unwrap();
        assert_eq!(after.len(), before.len() - 1);
        assert_eq!(after[1].start, before[2].start);
        // The same mark again: it isn't there as it was, so it's refused.
        assert!(s.remove(&file, &before[1]).is_err());

        // A clip the sheet has no entry for gets one.
        let new = dir.join("archive/2026-08-22/y.MP4");
        assert_eq!(s.set(&new, None, 0.0, 1.0).unwrap(), 0);
        assert_eq!(s.segments(&new).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
