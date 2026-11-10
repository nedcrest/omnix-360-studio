//! The edit as a whole: its picture's shape (audit C3: it was the
//! player's alone, so an agent could neither see nor set it).

use super::{OpError, Outcome, said};
use crate::project::{Group, Id, Project};
use crate::text::Colour;
use crate::view::Aspect;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectOp {
    /// The picture's shape, "16:9" (each side up to eight times the
    /// other); none: free, the view fills the window.
    Aspect { aspect: Option<Aspect> },
    /// Clips (cuts, sound clips, text, pictures; two at least, by id) made
    /// a group of their own (gap 8), taken from any they were in: its
    /// colour the next of `project::GROUP_COLOURS` unless given. Makes the
    /// group's id.
    Group { members: Vec<Id>, #[serde(default)] name: Option<String>, #[serde(default)] colour: Option<Colour> },
    /// The group gone, its clips as they are.
    Ungroup { group: Id },
    /// The group's colour, and its name if given.
    GroupLook { group: Id, colour: Colour, #[serde(default)] name: Option<String> },
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
        ProjectOp::Group { members, name, colour } => group(p, members, name.clone(), *colour),
        &ProjectOp::Ungroup { group } => {
            if !p.groups.iter().any(|g| g.id == group) {
                return Err(OpError::NotFound(format!("group #{group}")));
            }
            for id in p.members(group) {
                p.set_group(id, None);
            }
            p.settle_groups();
            said(format!("group #{group} undone"))
        }
        ProjectOp::GroupLook { group, colour, name } => {
            let g = p.groups.iter_mut().find(|g| g.id == *group).ok_or_else(|| OpError::NotFound(format!("group #{group}")))?;
            g.colour = *colour;
            if let Some(n) = name {
                g.name = n.trim().to_owned();
            }
            said(format!("group #{group}: {}", colour.hex()))
        }
    }
}

/// `ProjectOp::Group`.
fn group(p: &mut Project, members: &[Id], name: Option<String>, colour: Option<Colour>) -> Result<Outcome, OpError> {
    let mut ids: Vec<Id> = members.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() < 2 {
        return Err(OpError::Invalid("a group is two clips at least".into()));
    }
    let id = p.groups.iter().map(|g| g.id).max().unwrap_or(0) + 1;
    for &m in &ids {
        if !p.set_group(m, Some(id)) {
            return Err(OpError::NotFound(format!("clip #{m}")));
        }
    }
    let colour = colour.unwrap_or(Colour(crate::project::GROUP_COLOURS[p.groups.len() % crate::project::GROUP_COLOURS.len()]));
    p.groups.push(Group { id, name: name.unwrap_or_default().trim().to_owned(), colour });
    // Groups left with one clip go.
    p.settle_groups();
    Ok(Outcome { summary: format!("group #{id} of {} clips", ids.len()), made: Some(id) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_are_grouped_regrouped_and_ungrouped() {
        use crate::model::edit::Edit;
        use crate::project::{Cut, MusicClip};
        let mut p = Project { cuts: (0..3).map(|_| Cut::new("a.mp4".into(), 0.0, 4.0)).collect(), ..Default::default() };
        p.music.clips.push(MusicClip { input: "song.wav".into(), end: 9.0, ..Default::default() });
        p.settle_ids();
        let mut e = Edit::new(p);
        let op = |members: Vec<Id>| super::super::Op::Project(ProjectOp::Group { members, name: None, colour: None });
        let g = e.apply(&op(vec![1, 2, 4])).unwrap().made.unwrap();
        assert_eq!((e.members(g), e.group_of(4), e.groups[0].colour.0), (vec![1, 2, 4], Some(g), crate::project::GROUP_COLOURS[0]));
        assert!(e.apply(&op(vec![1])).is_err() && e.apply(&op(vec![1, 99])).is_err(), "two at least, all there");
        // Two of them into a new group: the first keeps one and goes.
        let h = e.apply(&op(vec![2, 4])).unwrap().made.unwrap();
        assert_eq!((e.groups.len(), e.group_of(1), e.members(h)), (1, None, vec![2, 4]));
        // A member deleted leaves one: the group goes.
        e.apply(&super::super::Op::Cut(super::super::cuts::CutOp::Delete { cut: 2 })).unwrap();
        assert!(e.groups.is_empty() && e.group_of(4).is_none());
        let g = e.apply(&op(vec![1, 3])).unwrap().made.unwrap();
        e.apply(&super::super::Op::Project(ProjectOp::Ungroup { group: g })).unwrap();
        assert!(e.groups.is_empty() && e.group_of(1).is_none());
        super::super::check(&e).unwrap();
    }

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
