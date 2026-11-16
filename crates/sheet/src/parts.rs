//! A trip's parts (Omnix gap 117): named lists of its clips, from any of
//! its days («Rides on Kos»: only the scooter's videos, every day). A
//! part holds clips' keys, never copies; a clip can be in several. The
//! file keeps them at its top, in the order they were made:
//! `"parts": [{"name": "Rides on Kos", "clips": ["2026-08-22/DJI_0001.MP4"]}]`.

use crate::ops::{OpError, Outcome};
use crate::Doc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A part as the typed view reads it.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, schemars::JsonSchema)]
pub struct Part {
    pub name: String,
    /// Its clips' keys (`<day>/<file>`), in the order they were put in.
    #[serde(default)]
    pub clips: Vec<String>,
}

/// A change to the parts. A part is named by its name, the same however
/// it's written in capitals («rides on kos» is «Rides on Kos»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PartOp {
    /// A new, empty part; its name is what it made.
    Add { name: String },
    Rename { part: String, name: String },
    /// Clips into the part; those in it already stay where they are.
    Put { part: String, clips: Vec<String> },
    /// Clips out of the part; one not in it is no error.
    Take { part: String, clips: Vec<String> },
    /// The part gone; its clips stay in the trip.
    Remove { part: String },
}

/// Whether two names are one part's.
pub fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// What's wrong with `parts`, a line each.
pub(crate) fn check(parts: &[Part]) -> Vec<String> {
    let mut problems = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if p.name.trim().is_empty() {
            problems.push(format!("part {}: no name", i + 1));
        }
        if parts[..i].iter().any(|q| same_name(&q.name, &p.name)) {
            problems.push(format!("part {}: the name is used twice", p.name));
        }
        for (j, clip) in p.clips.iter().enumerate() {
            if !clip.split_once('/').is_some_and(|(d, f)| crate::is_day(d) && !f.is_empty()) {
                problems.push(format!("part {}: {clip} isn't <day>/<file>", p.name));
            }
            if p.clips[..j].contains(clip) {
                problems.push(format!("part {}: {clip} is in it twice", p.name));
            }
        }
    }
    problems
}

impl Doc {
    pub(crate) fn change_part(&mut self, op: &PartOp) -> Result<Outcome, OpError> {
        match op {
            PartOp::Add { name } => {
                let name = name.trim();
                if name.is_empty() {
                    return Err(OpError::Invalid("a part needs a name".into()));
                }
                if self.part_at(name).is_ok() {
                    return Err(OpError::Invalid(format!("the part «{name}» is there already")));
                }
                self.parts_mut()?.push(json!({"name": name, "clips": []}));
                Ok(Outcome { made: Some(name.to_owned()) })
            }
            PartOp::Rename { part, name } => {
                let (i, name) = (self.part_at(part)?, name.trim());
                if name.is_empty() {
                    return Err(OpError::Invalid("a part needs a name".into()));
                }
                if self.part_at(name).is_ok_and(|j| j != i) {
                    return Err(OpError::Invalid(format!("the part «{name}» is there already")));
                }
                self.parts_mut()?[i]["name"] = json!(name);
                Ok(Outcome::default())
            }
            PartOp::Put { part, clips } => {
                let list = self.clips_of(part)?;
                for clip in clips {
                    if !list.iter().any(|c| c.as_str() == Some(clip.as_str())) {
                        list.push(json!(clip));
                    }
                }
                Ok(Outcome::default())
            }
            PartOp::Take { part, clips } => {
                self.clips_of(part)?.retain(|c| !c.as_str().is_some_and(|c| clips.iter().any(|t| t == c)));
                Ok(Outcome::default())
            }
            PartOp::Remove { part } => {
                let i = self.part_at(part)?;
                self.parts_mut()?.remove(i);
                Ok(Outcome::default())
            }
        }
    }

    /// The parts' list, made if absent.
    fn parts_mut(&mut self) -> Result<&mut Vec<Value>, OpError> {
        let top = self.value.as_object_mut().ok_or_else(|| OpError::Invalid("the file isn't an object".into()))?;
        top.entry("parts").or_insert_with(|| json!([])).as_array_mut().ok_or_else(|| OpError::Invalid("parts aren't a list".into()))
    }

    /// Where the part named `name` is in the list.
    fn part_at(&self, name: &str) -> Result<usize, OpError> {
        let parts = self.value.get("parts").and_then(Value::as_array);
        parts.and_then(|ps| ps.iter().position(|p| p.get("name").and_then(Value::as_str).is_some_and(|n| same_name(n, name)))).ok_or_else(|| OpError::NotFound(format!("part {name}")))
    }

    /// The clips' list of the part named `name`, made if absent.
    fn clips_of(&mut self, name: &str) -> Result<&mut Vec<Value>, OpError> {
        let i = self.part_at(name)?;
        let part = self.parts_mut()?[i].as_object_mut().ok_or_else(|| OpError::Invalid(format!("part {name} isn't an object")))?;
        part.entry("clips").or_insert_with(|| json!([])).as_array_mut().ok_or_else(|| OpError::Invalid(format!("part {name}: its clips aren't a list")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Op;

    fn doc() -> Doc {
        Doc::parse(r#"{"days": {}, "clips": {}}"#).unwrap()
    }

    fn apply(d: &mut Doc, op: PartOp) -> Result<Outcome, OpError> {
        d.apply(&Op::Part(op))
    }

    const A: &str = "2030-05-01/a.mp4";
    const B: &str = "2030-05-02/b.mp4";

    #[test]
    fn a_part_is_made_filled_and_emptied_by_its_name() {
        let mut d = doc();
        assert_eq!(apply(&mut d, PartOp::Add { name: " Rides on Kos ".into() }).unwrap().made.as_deref(), Some("Rides on Kos"));
        // Its name in any capitals; a clip put twice is there once.
        apply(&mut d, PartOp::Put { part: "rides on kos".into(), clips: vec![B.into(), A.into(), B.into()] }).unwrap();
        assert_eq!(d.sheet().unwrap().parts, [Part { name: "Rides on Kos".into(), clips: vec![B.into(), A.into()] }]);
        apply(&mut d, PartOp::Take { part: "Rides on Kos".into(), clips: vec![B.into(), "2030-05-03/none.mp4".into()] }).unwrap();
        assert_eq!(d.sheet().unwrap().parts[0].clips, [A]);
        assert!(d.text().contains("\"parts\": [\n    {\n      \"name\": \"Rides on Kos\",\n      \"clips\": [\n        \"2030-05-01/a.mp4\"\n      ]"), "{}", d.text());
        apply(&mut d, PartOp::Rename { part: "Rides on Kos".into(), name: "Scooter".into() }).unwrap();
        apply(&mut d, PartOp::Remove { part: "scooter".into() }).unwrap();
        assert!(d.sheet().unwrap().parts.is_empty());
    }

    #[test]
    fn a_part_is_refused_without_a_name_twice_or_with_a_clip_not_of_a_day() {
        let mut d = doc();
        assert!(matches!(apply(&mut d, PartOp::Add { name: "  ".into() }), Err(OpError::Invalid(_))));
        apply(&mut d, PartOp::Add { name: "Beach".into() }).unwrap();
        apply(&mut d, PartOp::Add { name: "Town".into() }).unwrap();
        assert!(matches!(apply(&mut d, PartOp::Add { name: "BEACH".into() }), Err(OpError::Invalid(_))));
        assert!(matches!(apply(&mut d, PartOp::Rename { part: "Town".into(), name: "beach".into() }), Err(OpError::Invalid(_))));
        // Its own name in other capitals is a rename, not a clash.
        apply(&mut d, PartOp::Rename { part: "Town".into(), name: "TOWN".into() }).unwrap();
        assert_eq!(apply(&mut d, PartOp::Put { part: "Sea".into(), clips: vec![A.into()] }), Err(OpError::NotFound("part Sea".into())));
        assert!(matches!(apply(&mut d, PartOp::Put { part: "Beach".into(), clips: vec!["a.mp4".into()] }), Err(OpError::Invalid(_))));
        assert!(d.sheet().unwrap().parts[0].clips.is_empty());
    }

    #[test]
    fn parts_are_described_and_found_by_clip() {
        let mut d = doc();
        apply(&mut d, PartOp::Add { name: "Beach".into() }).unwrap();
        apply(&mut d, PartOp::Add { name: "Town".into() }).unwrap();
        apply(&mut d, PartOp::Put { part: "Beach".into(), clips: vec![A.into(), B.into()] }).unwrap();
        let s = d.sheet().unwrap();
        assert!(s.describe(None).ends_with("part «Beach»  2 clips  2030-05-01/a.mp4, 2030-05-02/b.mp4\npart «Town»  0 clips  \n"), "{}", s.describe(None));
        assert!(s.describe(Some("2030-05-02")).ends_with("part «Beach»  1 clips  2030-05-02/b.mp4\n"));
        assert_eq!((s.parts_of(A), s.part("beach").map(|p| p.clips.len())), (vec!["Beach"], Some(2)));
    }

    #[test]
    fn parts_are_data_in_and_out() {
        let op = Op::Part(PartOp::Put { part: "Beach".into(), clips: vec![A.into()] });
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(json, r#"{"area":"part","op":"put","part":"Beach","clips":["2030-05-01/a.mp4"]}"#);
        assert_eq!(serde_json::from_str::<Op>(&json).unwrap(), op);
    }
}
