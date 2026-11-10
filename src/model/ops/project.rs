//! The edit as a whole: its picture's shape (audit C3: it was the
//! player's alone, so an agent could neither see nor set it).

use super::{OpError, Outcome, said};
use crate::project::Project;
use crate::view::Aspect;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectOp {
    /// The picture's shape, "16:9" (each side up to eight times the
    /// other); none: free, the view fills the window.
    Aspect { aspect: Option<Aspect> },
}

pub(super) fn apply(p: &mut Project, op: &ProjectOp) -> Result<Outcome, OpError> {
    match op {
        ProjectOp::Aspect { aspect } => {
            if let Some(a) = aspect {
                // The same rule as reading "16:9".
                String::from(*a).parse::<Aspect>().map_err(|e| OpError::Invalid(format!("{e:#}")))?;
            }
            p.aspect = *aspect;
            said(format!("aspect {}", aspect.map_or("free".to_owned(), String::from)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_aspect_is_set_freed_and_kept_in_range() {
        let mut p = Project::default();
        apply(&mut p, &ProjectOp::Aspect { aspect: Some(Aspect(9, 16)) }).unwrap();
        assert_eq!(p.aspect, Some(Aspect(9, 16)));
        assert!(apply(&mut p, &ProjectOp::Aspect { aspect: Some(Aspect(20, 1)) }).is_err() && p.aspect == Some(Aspect(9, 16)));
        apply(&mut p, &ProjectOp::Aspect { aspect: None }).unwrap();
        assert_eq!(p.aspect, None);
        let op: super::super::Op = serde_json::from_str(r#"{"area": "project", "op": "aspect", "aspect": "4:3"}"#).unwrap();
        assert_eq!(op, super::super::Op::Project(ProjectOp::Aspect { aspect: Some(Aspect(4, 3)) }));
    }
}
