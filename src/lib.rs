//! The contact sheet's core: reading and changing its marking file,
//! `notes.json`, one way for every program that does (Rushix itself,
//! Omnix's bin, an AI, the tests). Design: `docs/SHEET-CORE.md`.
//!
//! The file is kept as it was read ([`Doc`]): the order of its keys, a
//! time written `26` rather than `26.0`, and every field this crate
//! doesn't know, so a file read and written back unchanged is the same
//! byte for byte. The sheet's other writers (the web page it replaces,
//! for now) keep adding fields; none may be lost here. What a program
//! reads is the typed view over it ([`Sheet`]).

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

/// The marking file as read: changed in place, written back in the form
/// the sheet has always written it.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    value: Value,
}

/// Why a file couldn't be read as a sheet.
#[derive(Debug)]
pub enum Error {
    /// Not JSON.
    Json(serde_json::Error),
    /// JSON, but not the sheet's shape: what was wrong and where.
    Shape(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "not JSON: {e}"),
            Error::Shape(e) => write!(f, "not a contact sheet: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl Doc {
    /// Reads a sheet's text. An empty object is an empty sheet.
    pub fn parse(text: &str) -> Result<Doc, Error> {
        let value: Value = serde_json::from_str(text).map_err(Error::Json)?;
        if !value.is_object() {
            return Err(Error::Shape("the file isn't an object".into()));
        }
        let doc = Doc { value };
        // The typed view must hold, or the file isn't one.
        doc.sheet()?;
        Ok(doc)
    }

    /// The text to write: two spaces of indent, other scripts' letters as
    /// they are, no newline at the end. The form of Python's
    /// `json.dumps(indent=2, ensure_ascii=False)`, which the sheet's
    /// server has always written.
    pub fn text(&self) -> String {
        serde_json::to_string_pretty(&self.value).expect("a JSON value always serialises")
    }

    /// Gives every mark without an id one, and the later of two marks
    /// sharing an id a new one; says how many it gave. An id is `m` and a
    /// number, past the highest in the file, in the file's order, so the
    /// same file always gets the same ids; it goes last among the mark's
    /// keys, so nothing else in the file moves. Ids are kept forever after.
    ///
    /// An id given here means something only once it's written: until
    /// then, another writer adding a mark before this one would get the
    /// same id at the next read. So a program settles and saves before
    /// it addresses a mark by id (the store does, step 5).
    pub fn settle_ids(&mut self) -> usize {
        let number = |id: &str| id.strip_prefix('m').and_then(|n| n.parse::<u64>().ok());
        let mut next = self.marks().filter_map(|m| m.get("id").and_then(Value::as_str).and_then(number)).max().unwrap_or(0);
        let mut seen = std::collections::HashSet::new();
        let mut given = 0;
        for mark in self.marks_mut() {
            let fresh = mark.get("id").and_then(Value::as_str).is_some_and(|id| seen.insert(id.to_owned()));
            if !fresh {
                next += 1;
                let id = format!("m{next}");
                seen.insert(id.clone());
                // A duplicate keeps its place among the keys; a new one goes last.
                mark.insert("id".into(), Value::String(id));
                given += 1;
            }
        }
        given
    }

    /// Every mark's object, clip by clip in the file's order.
    fn marks(&self) -> impl Iterator<Item = &serde_json::Map<String, Value>> {
        self.value.get("clips").and_then(Value::as_object).into_iter().flat_map(|c| c.values()).filter_map(|c| c.get("marks")?.as_array()).flatten().filter_map(Value::as_object)
    }

    fn marks_mut(&mut self) -> impl Iterator<Item = &mut serde_json::Map<String, Value>> {
        self.value.get_mut("clips").and_then(Value::as_object_mut).into_iter().flat_map(|c| c.values_mut()).filter_map(|c| c.get_mut("marks")?.as_array_mut()).flatten().filter_map(Value::as_object_mut)
    }

    /// The typed view of the file as it stands.
    pub fn sheet(&self) -> Result<Sheet, Error> {
        Sheet::deserialize(&self.value).map_err(|e| Error::Shape(e.to_string()))
    }
}

/// A clip's rating: the sheet's ✓, ? and ✕.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    /// ✕: junk.
    Junk,
    /// ?: to think about.
    Think,
    /// ✓: take it.
    Take,
}

impl<'de> Deserialize<'de> for Rating {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Rating, D::Error> {
        match u8::deserialize(d)? {
            0 => Ok(Rating::Junk),
            1 => Ok(Rating::Think),
            2 => Ok(Rating::Take),
            n => Err(serde::de::Error::custom(format!("a rating is 0, 1 or 2, not {n}"))),
        }
    }
}

/// What a mark is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// From `t` to `end`: material.
    Segment,
    /// At `t` only (the sheet's first version, or shorter than it keeps
    /// as a segment): material from there.
    Point,
    /// A note to self ("a joke could go here"), never material.
    Flag,
}

/// A mark on a clip, as the typed view reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// Its id, once the core has given it one (step 2).
    pub id: Option<String>,
    pub t: f64,
    pub end: Option<f64>,
    pub kind: MarkKind,
    pub role: Option<String>,
    pub note: String,
}

impl<'de> Deserialize<'de> for Mark {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Mark, D::Error> {
        // As written: `kind` only for a flag, `end` only for a segment.
        #[derive(Deserialize)]
        struct Raw {
            id: Option<String>,
            t: f64,
            end: Option<f64>,
            kind: Option<String>,
            role: Option<String>,
            #[serde(default)]
            note: String,
        }
        let r = Raw::deserialize(d)?;
        let kind = match (r.kind.as_deref(), r.end) {
            (Some("flag"), _) => MarkKind::Flag,
            (_, Some(_)) => MarkKind::Segment,
            _ => MarkKind::Point,
        };
        Ok(Mark { id: r.id, t: r.t, end: r.end, kind, role: r.role.filter(|r| !r.is_empty()), note: r.note })
    }
}

/// A clip's marking.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Clip {
    /// None while unrated.
    #[serde(default)]
    pub rating: Option<Rating>,
    #[serde(default)]
    pub note: String,
    /// The cleaned voice chosen for it (a recipe's name, `dnn6`); none:
    /// the clip's own sound.
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub marks: Vec<Mark>,
}

/// A day's story and ideas.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Day {
    #[serde(default)]
    pub note: String,
}

/// A role the user added to the sheet's eight.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Role {
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub hint: String,
}

/// The typed view of a sheet: days and clips in the file's order, a clip
/// keyed `<day>/<file>`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Sheet {
    #[serde(default)]
    pub days: IndexMap<String, Day>,
    #[serde(default)]
    pub clips: IndexMap<String, Clip>,
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_told_apart_by_their_fields() {
        let doc = Doc::parse(
            r#"{"clips": {"2026-01-02/a.mp4": {"rating": 2, "marks": [
                {"t": 1.5, "end": 4, "role": "broll", "note": "x"},
                {"t": 7, "note": "p"},
                {"t": 9, "kind": "flag", "role": "", "note": "f"}]}}}"#,
        )
        .unwrap();
        let sheet = doc.sheet().unwrap();
        let clip = &sheet.clips["2026-01-02/a.mp4"];
        assert_eq!(clip.rating, Some(Rating::Take));
        let kinds: Vec<_> = clip.marks.iter().map(|m| m.kind).collect();
        assert_eq!(kinds, [MarkKind::Segment, MarkKind::Point, MarkKind::Flag]);
        assert_eq!(clip.marks[0].end, Some(4.0));
        // An empty role is none.
        assert_eq!(clip.marks[2].role, None);
    }

    #[test]
    fn a_wrong_shape_is_refused_with_where() {
        assert!(matches!(Doc::parse("[]"), Err(Error::Shape(_))));
        assert!(matches!(Doc::parse("{"), Err(Error::Json(_))));
        let e = Doc::parse(r#"{"clips": {"d/a": {"rating": 5}}}"#).unwrap_err();
        assert!(e.to_string().contains("0, 1 or 2"), "{e}");
    }

    #[test]
    fn ids_go_to_marks_without_one_past_the_highest_and_stay() {
        let mut doc = Doc::parse(
            r#"{"clips": {"d/a": {"marks": [{"t": 1, "note": ""}, {"t": 2, "note": "", "id": "m7"}]},
                          "d/b": {"marks": [{"t": 3, "note": "", "id": "m7"}, {"t": 4, "note": ""}]}}}"#,
        )
        .unwrap();
        assert_eq!(doc.settle_ids(), 3);
        let ids: Vec<_> = doc.sheet().unwrap().clips.values().flat_map(|c| c.marks.iter().map(|m| m.id.clone().unwrap())).collect();
        // In the file's order, past m7; the second m7 is a duplicate.
        assert_eq!(ids, ["m8", "m7", "m9", "m10"]);
        // Settled, nothing more to give, and the text doesn't change.
        let text = doc.text();
        assert_eq!(doc.settle_ids(), 0);
        assert_eq!(doc.text(), text);
        // The same file gets the same ids.
        let mut again = Doc::parse(r#"{"clips": {"d/a": {"marks": [{"t": 1, "note": ""}]}}}"#).unwrap();
        again.settle_ids();
        assert_eq!(again.sheet().unwrap().clips["d/a"].marks[0].id.as_deref(), Some("m1"));
    }

    #[test]
    fn a_new_id_goes_last_and_nothing_else_moves() {
        let mut doc = Doc::parse("{\n  \"clips\": {\n    \"d/a\": {\n      \"marks\": [\n        {\n          \"t\": 26,\n          \"note\": \"x\"\n        }\n      ]\n    }\n  }\n}").unwrap();
        doc.settle_ids();
        assert_eq!(doc.text(), "{\n  \"clips\": {\n    \"d/a\": {\n      \"marks\": [\n        {\n          \"t\": 26,\n          \"note\": \"x\",\n          \"id\": \"m1\"\n        }\n      ]\n    }\n  }\n}");
    }

    #[test]
    fn an_empty_file_is_an_empty_sheet() {
        assert_eq!(Doc::parse("{}").unwrap().sheet().unwrap(), Sheet::default());
    }
}
