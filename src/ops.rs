//! Operations: every change to a sheet, as data. The studio's window,
//! Omnix's bin, the CLI and an AI make the same changes the same way, and
//! tests can replay them (as Omnix's `OPS-API.md` does for projects).
//!
//! An op says what to set, never "change by": replayed, it gives the same
//! sheet (Omnix's «from the original» rule). It names what it changes by
//! a clip's key, a mark's id, a day: no hidden context such as "the mark
//! picked". It is applied whole or not at all, and it may not leave a
//! problem `check` finds that wasn't there before; one already in the
//! file (an old segment shorter than allowed) blocks nothing.
//!
//! New things are written in the shapes the web sheet writes them, so the
//! file reads the same whoever wrote it: a mark `{t, note, end, role}`, a
//! flag `{t, kind, note, role}`, a clip `{rating, note, marks}`, a time
//! with nothing after the point as a whole number.

use crate::Doc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fmt;

/// The shortest segment: as Omnix's shortest cut, so any segment can
/// become a cut.
pub const MIN_SEGMENT: f64 = 0.2;

/// The sheet's own roles; the user's added ones are in its `roles`.
pub const ROLES: [&str; 8] = ["talk", "establish", "broll", "detail", "cut", "action", "reveal", "reaction"];

/// The colours the web sheet gives the user's roles, in turn.
const EXTRA_COLORS: [&str; 6] = ["#8fae7a", "#3f8fb0", "#a3799a", "#7d8a6a", "#e08b4a", "#5fb3c9"];

/// A change to a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "area", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Mark(MarkOp),
    Clip(ClipOp),
    Day(DayOp),
    Role(RoleOp),
}

/// Marks on a clip: segments (material), points, flags (notes to self).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarkOp {
    /// A new mark on `clip`: a segment with an `end`, a point without, a
    /// flag (never with an end). The clip's entry is made if it has none.
    Add {
        clip: String,
        t: f64,
        #[serde(default)]
        end: Option<f64>,
        #[serde(default)]
        flag: bool,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        note: String,
    },
    /// The mark's place: a segment's ends, or a point's or a flag's time
    /// (`end` none makes a segment a point).
    Move { mark: String, t: f64, #[serde(default)] end: Option<f64> },
    /// Its role; none takes it away.
    Role { mark: String, #[serde(default)] role: Option<String> },
    Note { mark: String, note: String },
    Remove { mark: String },
}

/// A clip's own marking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipOp {
    /// ✓ 2, ? 1, ✕ 0, or none: unrated.
    Rate { clip: String, #[serde(default)] rating: Option<u8> },
    Note { clip: String, note: String },
    /// The cleaned voice chosen for it (a recipe's name); none: its own.
    Voice { clip: String, #[serde(default)] audio: Option<String> },
}

/// A day's story.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DayOp {
    Note { day: String, note: String },
}

/// The user's own roles, beside the eight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoleOp {
    /// A role named `title` (and its trade word `en`), keyed as the web
    /// sheet keys one: `x_` and the title, lower case, other characters
    /// as `_`, its first 24 characters.
    Add { title: String, #[serde(default)] en: String },
}

/// What an op did: the mark or role it made, if it made one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    pub made: Option<String>,
}

/// Why an op was refused; the sheet is unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum OpError {
    /// No such mark, clip or day.
    NotFound(String),
    /// Its values can't be: what's wrong.
    Invalid(String),
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::NotFound(what) => write!(f, "not found: {what}"),
            OpError::Invalid(why) => write!(f, "refused: {why}"),
        }
    }
}

impl std::error::Error for OpError {}

/// A time as the sheet writes it: to a thousandth, a whole number without
/// a point.
fn time(t: f64) -> Value {
    let t = (t * 1000.0).round() / 1000.0;
    if t.fract() == 0.0 && t.abs() < 1e15 { json!(t as i64) } else { json!(t) }
}

fn finite(t: f64, what: &str) -> Result<(), OpError> {
    if t.is_finite() && t >= 0.0 { Ok(()) } else { Err(OpError::Invalid(format!("{what} {t} isn't a time"))) }
}

/// The web sheet's key for a role titled `title`.
fn role_key(title: &str) -> String {
    let lower = title.to_lowercase();
    let mut key = String::new();
    let mut gap = false;
    for c in lower.chars() {
        let kept = c.is_ascii_lowercase() || c.is_ascii_digit() || ('а'..='я').contains(&c) || "їієґ".contains(c);
        if kept {
            key.push(c);
            gap = false;
        } else if !gap {
            key.push('_');
            gap = true;
        }
    }
    // `"x_" + ….slice(0, 24)`: the title's first 24, in characters (each
    // kept one is a single UTF-16 unit, as JavaScript counts). Checked
    // against JavaScript itself (node), 2026-11-04.
    format!("x_{}", key.chars().take(24).collect::<String>())
}

impl Doc {
    /// Applies `op`: whole, checked, or not at all.
    pub fn apply(&mut self, op: &Op) -> Result<Outcome, OpError> {
        let before = self.check();
        let mut next = self.clone();
        let outcome = next.change(op)?;
        let new: Vec<String> = next.check().into_iter().filter(|p| !before.contains(p)).collect();
        if !new.is_empty() {
            return Err(OpError::Invalid(new.join("; ")));
        }
        *self = next;
        Ok(outcome)
    }

    /// What's wrong with the sheet, a line each, naming where; none when
    /// it's sound.
    pub fn check(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let Ok(sheet) = self.sheet() else { return vec!["the file isn't a contact sheet".into()] };
        let date = |d: &str| d.len() == 10 && d.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
        for day in sheet.days.keys().filter(|d| !date(d)) {
            problems.push(format!("day {day}: not a date"));
        }
        let mut ids = std::collections::HashSet::new();
        for (key, clip) in &sheet.clips {
            match key.split_once('/') {
                Some((day, file)) if date(day) && !file.is_empty() => {}
                _ => problems.push(format!("clip {key}: not <day>/<file>")),
            }
            for m in &clip.marks {
                let at = format!("{key} {}", m.id.as_deref().unwrap_or("(no id)"));
                if !(m.t.is_finite() && m.t >= 0.0) {
                    problems.push(format!("{at}: starts at {}", m.t));
                }
                match (m.kind, m.end) {
                    (crate::MarkKind::Flag, Some(_)) => problems.push(format!("{at}: a flag has no end")),
                    (_, Some(end)) if end < m.t + MIN_SEGMENT - 1e-9 => problems.push(format!("{at}: shorter than {MIN_SEGMENT} s")),
                    _ => {}
                }
                if let Some(id) = &m.id
                    && !ids.insert(id.clone())
                {
                    problems.push(format!("{at}: the id is used twice"));
                }
                if let Some(role) = &m.role
                    && !ROLES.contains(&role.as_str())
                    && !sheet.roles.iter().any(|r| &r.key == role)
                {
                    problems.push(format!("{at}: no role {role}"));
                }
            }
        }
        problems
    }

    /// The op's change, unchecked.
    fn change(&mut self, op: &Op) -> Result<Outcome, OpError> {
        match op {
            Op::Mark(MarkOp::Add { clip, t, end, flag, role, note }) => {
                finite(*t, "the start")?;
                if let Some(end) = end {
                    finite(*end, "the end")?;
                }
                // The web sheet's shapes: {t, note, end, role}; a flag
                // {t, kind, note, role}.
                let mut m = Map::new();
                m.insert("t".into(), time(*t));
                if *flag {
                    if end.is_some() {
                        return Err(OpError::Invalid("a flag has no end".into()));
                    }
                    m.insert("kind".into(), json!("flag"));
                }
                m.insert("note".into(), json!(note));
                if let Some(end) = end {
                    m.insert("end".into(), time(*end));
                }
                if let Some(role) = role.as_ref().filter(|r| !r.is_empty()) {
                    m.insert("role".into(), json!(role));
                }
                let id = format!("m{}", self.highest_id() + 1);
                m.insert("id".into(), json!(id));
                self.clip_mut(clip)?.entry("marks").or_insert_with(|| json!([])).as_array_mut().ok_or_else(|| OpError::Invalid(format!("{clip}: its marks aren't a list")))?.push(Value::Object(m));
                Ok(Outcome { made: Some(id) })
            }
            Op::Mark(MarkOp::Move { mark, t, end }) => {
                finite(*t, "the start")?;
                let m = self.mark_mut(mark)?;
                if end.is_some() && m.get("kind").and_then(Value::as_str) == Some("flag") {
                    return Err(OpError::Invalid("a flag has no end".into()));
                }
                m.insert("t".into(), time(*t));
                match end {
                    Some(end) => {
                        finite(*end, "the end")?;
                        m.insert("end".into(), time(*end));
                    }
                    None => {
                        m.shift_remove("end");
                    }
                }
                Ok(Outcome::default())
            }
            Op::Mark(MarkOp::Role { mark, role }) => {
                let m = self.mark_mut(mark)?;
                match role.as_ref().filter(|r| !r.is_empty()) {
                    Some(r) => m.insert("role".into(), json!(r)),
                    None => m.shift_remove("role"),
                };
                Ok(Outcome::default())
            }
            Op::Mark(MarkOp::Note { mark, note }) => {
                self.mark_mut(mark)?.insert("note".into(), json!(note));
                Ok(Outcome::default())
            }
            Op::Mark(MarkOp::Remove { mark }) => {
                let (clip, i) = self.find_mark(mark)?;
                if let Some(marks) = self.value["clips"][clip.as_str()]["marks"].as_array_mut() {
                    marks.remove(i);
                }
                Ok(Outcome::default())
            }
            Op::Clip(ClipOp::Rate { clip, rating }) => {
                if rating.is_some_and(|r| r > 2) {
                    return Err(OpError::Invalid(format!("a rating is 0, 1 or 2, not {}", rating.unwrap_or_default())));
                }
                self.clip_mut(clip)?.insert("rating".into(), rating.map_or(Value::Null, |r| json!(r)));
                Ok(Outcome::default())
            }
            Op::Clip(ClipOp::Note { clip, note }) => {
                self.clip_mut(clip)?.insert("note".into(), json!(note));
                Ok(Outcome::default())
            }
            Op::Clip(ClipOp::Voice { clip, audio }) => {
                let c = self.clip_mut(clip)?;
                match audio.as_ref().filter(|a| !a.is_empty()) {
                    Some(a) => c.insert("audio".into(), json!(a)),
                    None => c.shift_remove("audio"),
                };
                Ok(Outcome::default())
            }
            Op::Day(DayOp::Note { day, note }) => {
                let days = self.object_mut("days")?;
                let d = days.entry(day.clone()).or_insert_with(|| json!({}));
                d.as_object_mut().ok_or_else(|| OpError::Invalid(format!("day {day} isn't an object")))?.insert("note".into(), json!(note));
                Ok(Outcome::default())
            }
            Op::Role(RoleOp::Add { title, en }) => {
                let title = title.trim();
                if title.is_empty() {
                    return Err(OpError::Invalid("a role needs a title".into()));
                }
                let key = role_key(title);
                let known = self.sheet().map(|s| s.roles.iter().any(|r| r.key == key)).unwrap_or(false);
                if known || ROLES.contains(&key.as_str()) {
                    return Err(OpError::Invalid(format!("the role {key} is there already")));
                }
                let roles = self.value.as_object_mut().ok_or_else(|| OpError::Invalid("the file isn't an object".into()))?.entry("roles").or_insert_with(|| json!([]));
                let roles = roles.as_array_mut().ok_or_else(|| OpError::Invalid("roles aren't a list".into()))?;
                let color = EXTRA_COLORS[roles.len() % EXTRA_COLORS.len()];
                roles.push(json!({"key": key, "title": title, "en": en.trim(), "color": color}));
                Ok(Outcome { made: Some(key) })
            }
        }
    }

    /// The highest number among the marks' ids (`m<n>`).
    fn highest_id(&self) -> u64 {
        self.marks().filter_map(|m| m.get("id")?.as_str()?.strip_prefix('m')?.parse().ok()).max().unwrap_or(0)
    }

    /// A top-level object, made if absent.
    fn object_mut(&mut self, key: &str) -> Result<&mut Map<String, Value>, OpError> {
        let top = self.value.as_object_mut().ok_or_else(|| OpError::Invalid("the file isn't an object".into()))?;
        top.entry(key).or_insert_with(|| json!({})).as_object_mut().ok_or_else(|| OpError::Invalid(format!("{key} isn't an object")))
    }

    /// A clip's entry, made as the web sheet makes one if absent.
    fn clip_mut(&mut self, clip: &str) -> Result<&mut Map<String, Value>, OpError> {
        if !clip.contains('/') {
            return Err(OpError::Invalid(format!("{clip}: a clip is <day>/<file>")));
        }
        let clips = self.object_mut("clips")?;
        let c = clips.entry(clip.to_owned()).or_insert_with(|| json!({"rating": null, "note": "", "marks": []}));
        c.as_object_mut().ok_or_else(|| OpError::Invalid(format!("{clip} isn't an object")))
    }

    /// Where the mark with `id` is: its clip and place.
    fn find_mark(&self, id: &str) -> Result<(String, usize), OpError> {
        let clips = self.value.get("clips").and_then(Value::as_object);
        clips
            .into_iter()
            .flatten()
            .find_map(|(key, c)| c.get("marks")?.as_array()?.iter().position(|m| m.get("id").and_then(Value::as_str) == Some(id)).map(|i| (key.clone(), i)))
            .ok_or_else(|| OpError::NotFound(format!("mark {id}")))
    }

    fn mark_mut(&mut self, id: &str) -> Result<&mut Map<String, Value>, OpError> {
        let (clip, i) = self.find_mark(id)?;
        self.value["clips"][clip.as_str()]["marks"][i].as_object_mut().ok_or_else(|| OpError::Invalid(format!("mark {id} isn't an object")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarkKind, Rating};

    const CLIP: &str = "2030-05-01/a.mp4";

    fn doc() -> Doc {
        Doc::parse(r#"{"days": {}, "clips": {}}"#).unwrap()
    }

    fn add(doc: &mut Doc, t: f64, end: Option<f64>) -> String {
        doc.apply(&Op::Mark(MarkOp::Add { clip: CLIP.into(), t, end, flag: false, role: Some("broll".into()), note: "x".into() })).unwrap().made.unwrap()
    }

    #[test]
    fn a_segment_is_added_in_the_sheets_shape_with_an_id() {
        let mut d = doc();
        assert_eq!(add(&mut d, 1.0, Some(4.25)), "m1");
        assert_eq!(add(&mut d, 5.0, None), "m2");
        // The clip made as the web sheet makes one; the mark in its key
        // order; whole numbers without a point.
        let text = d.text();
        assert!(text.contains("\"rating\": null,\n      \"note\": \"\",\n      \"marks\": ["), "{text}");
        assert!(text.contains("\"t\": 1,\n          \"note\": \"x\",\n          \"end\": 4.25,\n          \"role\": \"broll\",\n          \"id\": \"m1\""), "{text}");
        let kinds: Vec<_> = d.sheet().unwrap().clips[CLIP].marks.iter().map(|m| m.kind).collect();
        assert_eq!(kinds, [MarkKind::Segment, MarkKind::Point]);
    }

    #[test]
    fn marks_are_moved_named_and_removed_by_id() {
        let mut d = doc();
        let a = add(&mut d, 1.0, Some(4.0));
        let b = add(&mut d, 6.0, Some(8.0));
        d.apply(&Op::Mark(MarkOp::Move { mark: a.clone(), t: 2.0005, end: Some(3.5) })).unwrap();
        d.apply(&Op::Mark(MarkOp::Role { mark: a.clone(), role: None })).unwrap();
        d.apply(&Op::Mark(MarkOp::Note { mark: a.clone(), note: "крик".into() })).unwrap();
        let m = &d.sheet().unwrap().clips[CLIP].marks[0];
        // To a thousandth.
        assert_eq!((m.t, m.end, m.role.clone(), m.note.as_str()), (2.001, Some(3.5), None, "крик"));
        d.apply(&Op::Mark(MarkOp::Remove { mark: a.clone() })).unwrap();
        let marks = &d.sheet().unwrap().clips[CLIP].marks;
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].id.as_deref(), Some(b.as_str()));
        assert_eq!(d.apply(&Op::Mark(MarkOp::Remove { mark: a })), Err(OpError::NotFound("mark m1".into())));
        // A new id is past the highest, never one removed.
        assert_eq!(add(&mut d, 9.0, None), "m3");
    }

    #[test]
    fn a_refused_op_changes_nothing() {
        let mut d = doc();
        let a = add(&mut d, 1.0, Some(4.0));
        let text = d.text();
        // Too short; ends before it starts; a flag with an end; no such
        // role; a bad time; a bad rating.
        for op in [
            Op::Mark(MarkOp::Move { mark: a.clone(), t: 1.0, end: Some(1.1) }),
            Op::Mark(MarkOp::Move { mark: a.clone(), t: 4.0, end: Some(1.0) }),
            Op::Mark(MarkOp::Add { clip: CLIP.into(), t: 1.0, end: Some(3.0), flag: true, role: None, note: String::new() }),
            Op::Mark(MarkOp::Role { mark: a.clone(), role: Some("nonsense".into()) }),
            Op::Mark(MarkOp::Move { mark: a.clone(), t: f64::NAN, end: None }),
            Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(3) }),
            Op::Clip(ClipOp::Note { clip: "no-day".into(), note: String::new() }),
        ] {
            assert!(matches!(d.apply(&op), Err(OpError::Invalid(_))), "{op:?}");
            assert_eq!(d.text(), text, "{op:?} changed the sheet");
        }
    }

    #[test]
    fn a_problem_already_there_blocks_nothing() {
        // An old segment shorter than allowed: other ops still go through.
        let mut d = Doc::parse(r#"{"clips": {"2030-05-01/a.mp4": {"marks": [{"t": 1, "end": 1.1, "note": "", "id": "m1"}]}}}"#).unwrap();
        assert_eq!(d.check().len(), 1);
        d.apply(&Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(2) })).unwrap();
        d.apply(&Op::Mark(MarkOp::Note { mark: "m1".into(), note: "ok".into() })).unwrap();
        assert_eq!(d.sheet().unwrap().clips[CLIP].rating, Some(Rating::Take));
    }

    #[test]
    fn a_flag_a_rating_a_voice_and_a_days_note() {
        let mut d = doc();
        let f = d.apply(&Op::Mark(MarkOp::Add { clip: CLIP.into(), t: 26.0, end: None, flag: true, role: Some("cut".into()), note: "мем".into() })).unwrap().made.unwrap();
        assert!(d.text().contains("\"t\": 26,\n          \"kind\": \"flag\",\n          \"note\": \"мем\",\n          \"role\": \"cut\""));
        assert!(matches!(d.apply(&Op::Mark(MarkOp::Move { mark: f, t: 20.0, end: Some(22.0) })), Err(OpError::Invalid(_))));
        d.apply(&Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: Some(1) })).unwrap();
        d.apply(&Op::Clip(ClipOp::Voice { clip: CLIP.into(), audio: Some("dnn6".into()) })).unwrap();
        d.apply(&Op::Day(DayOp::Note { day: "2030-05-01".into(), note: "море".into() })).unwrap();
        let s = d.sheet().unwrap();
        assert_eq!((s.clips[CLIP].rating, s.clips[CLIP].audio.as_deref()), (Some(Rating::Think), Some("dnn6")));
        assert_eq!(s.days["2030-05-01"].note, "море");
        d.apply(&Op::Clip(ClipOp::Voice { clip: CLIP.into(), audio: None })).unwrap();
        d.apply(&Op::Clip(ClipOp::Rate { clip: CLIP.into(), rating: None })).unwrap();
        let s = d.sheet().unwrap();
        assert_eq!((s.clips[CLIP].rating, s.clips[CLIP].audio.as_deref()), (None, None));
    }

    #[test]
    fn a_role_is_keyed_as_the_web_sheet_keys_it_and_then_usable() {
        assert_eq!(role_key("Мем"), "x_мем");
        assert_eq!(role_key("Жарт / Smile!!"), "x_жарт_smile_");
        assert_eq!(role_key("дуже довга назва ролі для перевірки"), "x_дуже_довга_назва_ролі_дл");
        let mut d = doc();
        let key = d.apply(&Op::Role(RoleOp::Add { title: "Мем".into(), en: "meme".into() })).unwrap().made.unwrap();
        assert!(matches!(d.apply(&Op::Role(RoleOp::Add { title: "мем".into(), en: String::new() })), Err(OpError::Invalid(_))));
        let a = add(&mut d, 1.0, Some(3.0));
        d.apply(&Op::Mark(MarkOp::Role { mark: a, role: Some(key) })).unwrap();
        assert!(d.text().contains("\"color\": \"#8fae7a\""));
    }

    #[test]
    fn ops_are_data_in_and_out() {
        let op: Op = serde_json::from_str(r#"{"area": "mark", "op": "move", "mark": "m3", "t": 1.5, "end": 4}"#).unwrap();
        assert_eq!(op, Op::Mark(MarkOp::Move { mark: "m3".into(), t: 1.5, end: Some(4.0) }));
        assert!(serde_json::from_str::<Op>(r#"{"area": "mark", "op": "move", "mark": "m3", "t": 1, "by": 2}"#).is_err());
    }
}
