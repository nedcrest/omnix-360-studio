//! The edit as a whole: its picture's shape (audit C3: it was the
//! player's alone, so an agent could neither see nor set it).

use super::{OpError, Outcome, said};
use crate::model::edit::MIN_CUT;
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
    /// Each of `things` (sound clips, text, pictures, by id; none: all)
    /// that runs past the film's end trimmed to it, and one that starts
    /// there or later taken away (gap 101).
    TrimToEnd { #[serde(default)] things: Vec<Id> },
    /// Sound clips, text and pictures (by id; no cuts: a cut's place is its
    /// order) moved `by` seconds together, as far as all of them fit: none
    /// before the edit, none past the film's end, none further into a clip
    /// that isn't moving. How a group's others follow a dragged one.
    Shift { things: Vec<Id>, by: f64 },
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
        ProjectOp::TrimToEnd { things } => trim_to_end(p, things),
        ProjectOp::Shift { things, by } => {
            if let Some(id) = things.iter().find(|&&id| p.clip_index(id).is_none() && p.item_index(id).is_none()) {
                return Err(OpError::NotFound(format!("sound clip or item #{id}")));
            }
            let went = p.shift(things, *by);
            said(format!("{} moved {went:+.2} s{}", things.len(), if (went - by).abs() > 1e-6 { format!(" of {by:+.2}") } else { String::new() }))
        }
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

/// `TrimToEnd`: what of `things` (all, if none) runs past the film's
/// end, trimmed to it or, starting there, taken away.
fn trim_to_end(p: &mut Project, things: &[Id]) -> Result<Outcome, OpError> {
    let end = p.film_end().ok_or_else(|| OpError::Invalid("no pictures yet: the film has no end to trim to".into()))?;
    if let Some(id) = things.iter().find(|&&id| p.clip_index(id).is_none() && p.item_index(id).is_none()) {
        return Err(OpError::NotFound(format!("sound clip or item #{id}")));
    }
    let past: Vec<Id> = p.past_end().into_iter().map(|(id, _)| id).filter(|id| things.is_empty() || things.contains(id)).collect();
    let (mut trimmed, mut gone) = (0, 0);
    for id in past {
        if let Some((w, i)) = p.clip_index(id) {
            let clips = &mut p.track_mut(w).clips;
            if clips[i].at >= end - MIN_CUT {
                clips.remove(i);
                gone += 1;
            } else {
                let c = &mut clips[i];
                c.end = c.start + (end - c.at);
                // Its fades within what's left (the audit, 2026-11-11).
                c.fade_out = c.fade_out.min(c.end - c.start);
                c.fade_in = c.fade_in.min(c.end - c.start - c.fade_out);
                trimmed += 1;
            }
        } else if let Some((track, lane, i)) = p.item_index(id) {
            let items = &mut p.track_lanes_mut(track)[lane].items;
            if items[i].at >= end - MIN_CUT {
                items.remove(i);
                gone += 1;
            } else {
                let it = &mut items[i];
                let was = (it.start, it.end);
                it.end = it.local(end);
                crate::animate::keep_on_edges(&mut it.automation, was, (it.start, it.end));
                trimmed += 1;
            }
        }
    }
    p.settle_groups();
    said(format!("{trimmed} trimmed to the film's end at {end:.2} s, {gone} past it taken away"))
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

    #[test]
    fn what_runs_past_the_film_is_trimmed_to_its_end_or_taken_away() {
        use super::super::{Op, check};
        use crate::model::edit::Edit;
        use crate::project::{Cut, MusicClip, SoundTrack};
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 10.0)], ..Default::default() };
        p.music.clips.push(MusicClip { input: "song.wav".into(), at: 4.0, end: 30.0, ..Default::default() });
        p.sfx.clips.push(MusicClip { input: "boom.wav".into(), at: 12.0, end: 1.0, ..Default::default() });
        p.sfx2.clips.push(MusicClip { input: "bell.wav".into(), at: 1.0, end: 1.0, ..Default::default() });
        let mut e = Edit::default();
        e.replace(p);
        let (song, boom) = (e.music.clips[0].id, e.sfx.clips[0].id);
        assert_eq!(e.past_end(), [(song, 24.0), (boom, 3.0)], "the bell inside the film");
        let said = e.apply(&Op::Project(ProjectOp::TrimToEnd { things: vec![song] })).unwrap().summary;
        assert_eq!((e.music.clips[0].ends(), e.sfx.clips.len()), (10.0, 1), "{said}: the song alone, as asked");
        e.apply(&Op::Project(ProjectOp::TrimToEnd { things: vec![] })).unwrap();
        assert!(e.sfx.clips.is_empty() && e.past_end().is_empty() && e.track(SoundTrack::Sfx2).clips.len() == 1);
        check(&e).unwrap();
        assert!(e.undo() && e.sfx.clips.len() == 1);
        assert!(e.apply(&Op::Project(ProjectOp::TrimToEnd { things: vec![999] })).is_err(), "an unknown id says so");
    }

    #[test]
    fn a_trimmed_songs_fades_stay_within_it() {
        use super::super::Op;
        use crate::model::edit::Edit;
        use crate::project::{Cut, MusicClip};
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 10.0)], ..Default::default() };
        p.music.clips.push(MusicClip { input: "song.wav".into(), at: 9.0, end: 10.0, fade_in: 0.8, fade_out: 3.0, ..Default::default() });
        let mut e = Edit::default();
        e.replace(p);
        e.apply(&Op::Project(ProjectOp::TrimToEnd { things: vec![] })).unwrap();
        let c = &e.music.clips[0];
        assert!(c.fade_in + c.fade_out <= c.end - c.start + 1e-9, "{c:?}");
    }

    #[test]
    fn a_set_moves_together_as_far_as_all_of_it_fits() {
        use super::super::{Op, check};
        use crate::model::edit::Edit;
        use crate::project::{Cut, MusicClip};
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 10.0)], ..Default::default() };
        // Two songs edge to edge and a bell further on the same lane.
        for (at, len) in [(0.0, 2.0), (2.0, 2.0), (7.0, 1.0)] {
            p.music.clips.push(MusicClip { input: "s.wav".into(), at, end: len, ..Default::default() });
        }
        let mut e = Edit::default();
        e.replace(p);
        let ids: Vec<Id> = e.music.clips.iter().map(|c| c.id).collect();
        let said = e.apply(&Op::Project(ProjectOp::Shift { things: vec![ids[0], ids[1]], by: 5.0 })).unwrap().summary;
        let ats: Vec<f64> = e.music.clips.iter().map(|c| c.at).collect();
        assert!((ats[0] - 3.0).abs() < 1e-6 && (ats[1] - 5.0).abs() < 1e-6, "stopped whole at the bell: {ats:?} ({said})");
        e.apply(&Op::Project(ProjectOp::Shift { things: vec![ids[0], ids[1]], by: -9.0 })).unwrap();
        assert!(e.music.clips[0].at.abs() < 1e-6 && (e.music.clips[1].at - 2.0).abs() < 1e-6, "not before the edit, still edge to edge");
        e.apply(&Op::Project(ProjectOp::Shift { things: vec![ids[2]], by: 5.0 })).unwrap();
        assert!((e.music.clips[2].ends() - 10.0).abs() < 1e-6, "held at the film's end");
        assert!(e.apply(&Op::Project(ProjectOp::Shift { things: vec![e.cuts[0].id], by: 1.0 })).is_err(), "a cut isn't shifted");
        check(&e).unwrap();
    }

    #[test]
    fn check_sees_a_group_without_its_clips_or_a_clip_without_its_group() {
        use crate::project::{Cut, Group};
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 1.0), Cut::new("b.mp4".into(), 0.0, 1.0)], ..Default::default() };
        p.settle_ids();
        p.cuts[0].group = Some(9);
        assert!(super::super::check(&p).unwrap_err().contains("group #9"), "a member of no group");
        p.groups.push(Group { id: 9, name: String::new(), colour: crate::text::Colour([1, 2, 3]) });
        assert!(super::super::check(&p).unwrap_err().contains("fewer than two"));
        p.cuts[1].group = Some(9);
        super::super::check(&p).unwrap();
    }
}
