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
    fn an_empty_file_is_an_empty_sheet() {
        assert_eq!(Doc::parse("{}").unwrap().sheet().unwrap(), Sheet::default());
    }
}
