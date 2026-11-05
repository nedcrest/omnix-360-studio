//! The contact sheet's core: reading and changing its marking file,
//! `notes.json`, one way for every program that does (Dailix itself,
//! Omnix's bin, an AI, the tests). Design: `docs/SHEET-CORE.md`.
//!
//! The file is kept as it was read ([`Doc`]): the order of its keys, a
//! time written `26` rather than `26.0`, and every field this crate
//! doesn't know, so a file read and written back unchanged is the same
//! byte for byte. The sheet's other writers (the web page it replaces,
//! for now) keep adding fields; none may be lost here. What a program
//! reads is the typed view over it ([`Sheet`]).

pub mod footage;
pub mod layout;
pub mod marking;
pub mod ops;
pub mod project;
pub mod store;

pub use layout::Layout;
pub use store::{Store, StoreError};

pub use ops::{ClipOp, DayOp, MarkOp, Op, OpError, Outcome, RoleOp};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

/// Whether clip time `t` falls in one of `marks`' segments: a flag or a
/// point outside every segment goes into no edit.
pub fn in_segment(marks: &[Mark], t: f64) -> bool {
    marks.iter().any(|s| s.kind == MarkKind::Segment && s.end.is_some_and(|e| t >= s.t && t <= e))
}

/// Whether `name` is a day as the sheet keys days: `YYYY-MM-DD`.
pub fn is_day(name: &str) -> bool {
    name.len() == 10 && name.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
}

/// Seconds as `m:ss`, with `places` decimals (`m:ss.s` for one), rounded
/// to them first: 59.96 s with one place is «1:00.0», never «0:60.0».
/// One clock for both programs' readouts and the sheet's text view.
pub fn clock(t: f64, places: u32) -> String {
    let scale = 10u64.pow(places);
    let units = (t.max(0.0) * scale as f64).round() as u64;
    let (s, frac) = (units / scale, units % scale);
    match places {
        0 => format!("{}:{:02}", s / 60, s % 60),
        p => format!("{}:{:02}.{frac:0w$}", s / 60, s % 60, w = p as usize),
    }
}

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
        let mut next = self.highest_id();
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
    pub(crate) fn marks(&self) -> impl Iterator<Item = &serde_json::Map<String, Value>> {
        self.value.get("clips").and_then(Value::as_object).into_iter().flat_map(|c| c.values()).filter_map(|c| c.get("marks")?.as_array()).flatten().filter_map(Value::as_object)
    }

    fn marks_mut(&mut self) -> impl Iterator<Item = &mut serde_json::Map<String, Value>> {
        self.value.get_mut("clips").and_then(Value::as_object_mut).into_iter().flat_map(|c| c.values_mut()).filter_map(|c| c.get_mut("marks")?.as_array_mut()).flatten().filter_map(Value::as_object_mut)
    }

    /// Drops the web sheet's view state (`ui`: which days are folded): the
    /// page's, not the marking's; a Dailix project doesn't carry it.
    pub fn forget_view_state(&mut self) {
        if let Some(top) = self.value.as_object_mut() {
            top.shift_remove("ui");
        }
    }

    /// The typed view of the file as it stands.
    pub fn sheet(&self) -> Result<Sheet, Error> {
        Sheet::deserialize(&self.value).map_err(|e| Error::Shape(e.to_string()))
    }
}

impl schemars::JsonSchema for Rating {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Rating".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type": "integer", "minimum": 0, "maximum": 2, "description": "2 take (✓), 1 think (?), 0 junk (✕)"})
    }
}

/// A mark as the file holds it: `end` only for a segment, `kind: "flag"`
/// only for a flag. Read as this, then typed (`Mark`); the schema shows it.
#[derive(Deserialize, schemars::JsonSchema)]
struct MarkShape {
    /// Given by the core, kept forever: `m` and a number.
    id: Option<String>,
    /// Seconds into the clip.
    t: f64,
    /// A segment's end; absent for a point or a flag.
    end: Option<f64>,
    /// "flag" for a note to self, never material.
    kind: Option<String>,
    /// One of the eight (talk, establish, broll, detail, cut, action,
    /// reveal, reaction) or the sheet's own (`roles`).
    role: Option<String>,
    #[serde(default)]
    note: String,
}

impl schemars::JsonSchema for Mark {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Mark".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        MarkShape::json_schema(g)
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
        let r = MarkShape::deserialize(d)?;
        let kind = match (r.kind.as_deref(), r.end) {
            (Some("flag"), _) => MarkKind::Flag,
            (_, Some(_)) => MarkKind::Segment,
            _ => MarkKind::Point,
        };
        Ok(Mark { id: r.id, t: r.t, end: r.end, kind, role: r.role.filter(|r| !r.is_empty()), note: r.note })
    }
}

/// A clip's marking.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Default, Deserialize, schemars::JsonSchema)]
pub struct Day {
    #[serde(default)]
    pub note: String,
}

/// A role the user added to the sheet's eight.
#[derive(Debug, Clone, PartialEq, Deserialize, schemars::JsonSchema)]
pub struct Role {
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub en: String,
    /// Its colour on the sheet's strips.
    #[serde(default)]
    pub color: Option<String>,
}

/// The typed view of a sheet: days and clips in the file's order, a clip
/// keyed `<day>/<file>`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, schemars::JsonSchema)]
pub struct Sheet {
    #[serde(default)]
    pub days: IndexMap<String, Day>,
    #[serde(default)]
    pub clips: IndexMap<String, Clip>,
    #[serde(default)]
    pub roles: Vec<Role>,
}

/// The sheet's own roles: the function of a shot in the edit, not what's
/// in it. Key, name, the trade's English word, and the colour the web sheet
/// gave it (footage-agent's `report.html`), so a segment reads the same in
/// every program.
const BUILT_IN_ROLES: [(&str, &str, &str, &str); 8] = [
    ("talk", "розмова", "sync", "#c0563a"),
    ("establish", "загальний", "establishing", "#4f8a6a"),
    ("broll", "фон", "B-roll", "#6b7f9e"),
    ("detail", "деталь", "insert", "#c98a5e"),
    ("cut", "перебивка", "cutaway", "#b07aa8"),
    ("action", "дія", "action", "#d29a5e"),
    ("reveal", "ревіл", "reveal", "#7a6bab"),
    ("reaction", "реакція", "reaction", "#d18fa5"),
];

/// A mark with no role, or one whose colour is unknown.
pub const NO_ROLE_RGB: [u8; 3] = [0x9a, 0x9a, 0x9a];

/// Whether `key` is one of the sheet's own roles.
pub fn is_built_in_role(key: &str) -> bool {
    BUILT_IN_ROLES.iter().any(|r| r.0 == key)
}

impl Role {
    /// How it reads in a list: "розмова / sync", or as much as it has.
    pub fn name(&self) -> String {
        let title = if self.title.is_empty() { &self.key } else { &self.title };
        if self.en.is_empty() { title.clone() } else { format!("{title} / {}", self.en) }
    }

    /// Its colour (`#rrggbb`), grey when it has none it can read.
    pub fn rgb(&self) -> [u8; 3] {
        let hex = self.color.as_deref().and_then(|c| c.strip_prefix('#')).filter(|h| h.len() == 6);
        hex.and_then(|h| {
            let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
            Some([byte(0)?, byte(2)?, byte(4)?])
        })
        .unwrap_or(NO_ROLE_RGB)
    }
}

impl Sheet {
    /// The sheet as text, a day at a time: the day's note, then each clip
    /// rated, noted or marked, then its marks by time. What `dailix sheet
    /// describe` prints, for a person, a test or an AI.
    pub fn describe(&self, only: Option<&str>) -> String {
        let mut out = String::new();
        let mut days: Vec<&str> = self.days.keys().map(String::as_str).chain(self.clips.keys().filter_map(|k| k.split_once('/').map(|(d, _)| d))).collect();
        days.sort_unstable();
        days.dedup();
        for day in days.into_iter().filter(|d| only.is_none_or(|o| o == *d)) {
            let note = self.days.get(day).map(|d| d.note.replace('\n', " / ")).unwrap_or_default();
            out.push_str(&format!("{day}  {note}\n"));
            for (key, clip) in self.clips.iter().filter(|(k, _)| k.split_once('/').is_some_and(|(d, _)| d == day)) {
                if clip.rating.is_none() && clip.note.is_empty() && clip.marks.is_empty() {
                    continue;
                }
                let file = key.split_once('/').map_or(key.as_str(), |(_, f)| f);
                let rating = match clip.rating {
                    Some(Rating::Take) => "✓",
                    Some(Rating::Think) => "?",
                    Some(Rating::Junk) => "✕",
                    None => "·",
                };
                let voice = clip.audio.as_ref().map(|a| format!("  voice {a}")).unwrap_or_default();
                out.push_str(&format!("  {file}  {rating}  {}{voice}\n", clip.note.replace('\n', " / ")));
                let mut marks: Vec<_> = clip.marks.iter().collect();
                marks.sort_by(|a, b| a.t.total_cmp(&b.t));
                for m in marks {
                    let span = match (m.kind, m.end) {
                        (MarkKind::Segment, Some(end)) => format!("{}–{}", clock(m.t, 1), clock(end, 1)),
                        (MarkKind::Flag, _) => format!("{} flag", clock(m.t, 1)),
                        _ => clock(m.t, 1),
                    };
                    let role = m.role.as_deref().unwrap_or("-");
                    out.push_str(&format!("    {}  {span}  {role}  {}\n", m.id.as_deref().unwrap_or("-"), m.note.replace('\n', " / ")));
                }
            }
        }
        out
    }

    /// Every role a mark can have: the sheet's own, then the user's added
    /// in it (one with a built-in's key is the built-in).
    pub fn all_roles(&self) -> Vec<Role> {
        let built_in = BUILT_IN_ROLES.iter().map(|&(key, title, en, color)| Role { key: key.into(), title: title.into(), en: en.into(), color: Some(color.into()) });
        built_in.chain(self.roles.iter().filter(|r| !is_built_in_role(&r.key)).cloned()).collect()
    }

    /// A role by its key, the user's included.
    pub fn role(&self, key: &str) -> Option<Role> {
        self.all_roles().into_iter().find(|r| r.key == key)
    }

    /// A mark's colour by its role: its own, grey for none or one unknown.
    pub fn role_rgb(&self, key: Option<&str>) -> [u8; 3] {
        key.and_then(|k| self.role(k)).map_or(NO_ROLE_RGB, |r| r.rgb())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_reads_as_a_line_per_day_clip_and_mark() {
        let doc = Doc::parse(
            r#"{"days": {"2030-05-01": {"note": "море"}}, "clips": {
                "2030-05-01/a.mp4": {"rating": 2, "note": "кіт", "audio": "dnn6", "marks": [
                    {"t": 26, "kind": "flag", "note": "мем", "role": "cut", "id": "m2"},
                    {"t": 1.5, "end": 4, "role": "broll", "note": "фон", "id": "m1"}]},
                "2030-05-01/b.mp4": {"rating": null, "note": "", "marks": []},
                "2030-05-02/c.mp4": {"rating": 0}}}"#,
        )
        .unwrap();
        let text = doc.sheet().unwrap().describe(None);
        assert_eq!(
            text,
            "2030-05-01  море\n  a.mp4  ✓  кіт  voice dnn6\n    m1  0:01.5–0:04.0  broll  фон\n    m2  0:26.0 flag  cut  мем\n2030-05-02  \n  c.mp4  ✕  \n"
        );
        assert_eq!(doc.sheet().unwrap().describe(Some("2030-05-02")), "2030-05-02  \n  c.mp4  ✕  \n");
    }

    #[test]
    fn roles_built_in_and_the_users_with_their_colours() {
        let sheet = Doc::parse(r##"{"roles": [{"key": "meme", "title": "мем", "color": "#8fae7a"}, {"key": "odd", "color": "nope"}]}"##).unwrap().sheet().unwrap();
        let keys: Vec<String> = sheet.all_roles().into_iter().map(|r| r.key).collect();
        assert_eq!((keys.len(), keys[0].as_str(), keys[8].as_str()), (10, "talk", "meme"));
        assert_eq!(sheet.role("talk").unwrap().name(), "розмова / sync");
        assert_eq!(sheet.role("meme").unwrap().name(), "мем");
        assert_eq!([sheet.role_rgb(Some("talk")), sheet.role_rgb(Some("meme")), sheet.role_rgb(Some("odd")), sheet.role_rgb(None)], [[0xc0, 0x56, 0x3a], [0x8f, 0xae, 0x7a], NO_ROLE_RGB, NO_ROLE_RGB]);
    }

    #[test]
    fn a_clock_rounds_before_it_carries() {
        assert_eq!([clock(65.4, 0), clock(59.96, 1), clock(136.4, 1), clock(59.996, 2), clock(-1.0, 1)], ["1:05", "1:00.0", "2:16.4", "1:00.00", "0:00.0"]);
    }

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
