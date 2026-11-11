//! Operations on the sound tracks' clips: add, move, trim, split, to the
//! other SFX lane, delete (`OPS-API.md` step 3). A clip's own level, line
//! and fades are sound ops (`sound.rs`).

use super::{End, OpError, Outcome, held, said};
use crate::model::edit::MIN_CUT;
use crate::project::{Id, Project, SoundTrack};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipOp {
    /// `input` from `start` to `end` (its own time) on `track`, playing
    /// from edit time `at`. A sound effect where one already plays goes on
    /// SFX 2, so the two sound together (gap 46e).
    Add { track: SoundTrack, input: PathBuf, at: f64, start: f64, end: f64 },
    /// To start at edit time `at`; with `overlap` it may overlap a
    /// neighbour (a crossfade), else it stops edge to edge or hops past.
    Move { clip: Id, at: f64, #[serde(default)] overlap: bool },
    /// Its in or out edge to `t` (its own time), kept in place on the
    /// edit; `most` is its file's length.
    Trim { clip: Id, end: End, t: f64, most: f64, #[serde(default)] overlap: bool },
    /// Split at edit time `at`; the second half is a new clip.
    Split { clip: Id, at: f64 },
    /// A sound effect to the other SFX lane, nearest its place there.
    OtherSfx { clip: Id },
    /// The sound clip gone.
    Delete { clip: Id },
}

pub(super) fn apply(p: &mut Project, op: &ClipOp) -> Result<Outcome, OpError> {
    match op {
        ClipOp::Add { track, input, at, start, end } => {
            // Short is fine (a click); MIN_CUT is for splits and trims.
            if !(*at >= 0.0 && end > start) {
                return Err(OpError::Invalid(format!("a clip at {at} s from {start} to {end} s is empty or before the edit")));
            }
            let busy = |p: &Project, t: SoundTrack| p.track(t).clips.iter().any(|c| *at < c.ends() - 1e-6 && at + (end - start) > c.at + 1e-6);
            let track = if *track == SoundTrack::Sfx && busy(p, SoundTrack::Sfx) && !busy(p, SoundTrack::Sfx2) { SoundTrack::Sfx2 } else { *track };
            let id = p.next_id();
            let i = p.track_mut(track).append_part(input.clone(), *at, *start, *end);
            p.track_mut(track).clips[i].id = id;
            Ok(Outcome { summary: format!("sound clip #{id} added on {}", track.name()), made: Some(id) })
        }
        &ClipOp::Move { clip, at, overlap } => {
            let (w, i) = find(p, clip)?;
            let c = &p.track(w).clips[i];
            let (latest, was) = (crate::project::latest_start(p.film_end(), c.end - c.start).max(c.at), (c.at, c.ends()));
            p.track_mut(w).drag_to(i, at.min(latest), overlap);
            let pushed = crate::project::pushed_past(p.film_end(), was.1, p.track(w).clips[i].ends());
            if pushed {
                p.track_mut(w).clips[i].at = was.0;
            }
            said(format!("sound clip #{clip} at {:.2} s{}", p.track(w).clips[i].at, held(at > latest || pushed)))
        }
        &ClipOp::Trim { clip, end, t, most, overlap } => {
            let (w, i) = find(p, clip)?;
            let c = &p.track(w).clips[i];
            let latest = if end == End::Out { crate::project::latest_out(p.film_end(), c.at, c.start).max(c.end) } else { f64::INFINITY };
            p.track_mut(w).trim(i, end == End::In, t.min(latest), overlap, most, MIN_CUT);
            let c = &p.track(w).clips[i];
            said(format!("sound clip #{clip}: {:.2}-{:.2} s of its file{}", c.start, c.end, held(t > latest)))
        }
        &ClipOp::Split { clip, at } => {
            let (w, i) = find(p, clip)?;
            if !p.track_mut(w).split(i, at, MIN_CUT) {
                let c = &p.track(w).clips[i];
                return Err(OpError::OutOfRange { what: "split time", value: at, least: c.at + MIN_CUT, most: c.ends() - MIN_CUT });
            }
            let id = p.next_id();
            p.track_mut(w).clips[i + 1].id = id;
            Ok(Outcome { summary: format!("sound clip #{clip} split at {at:.2} s; the second half is #{id}"), made: Some(id) })
        }
        &ClipOp::OtherSfx { clip } => {
            let (w, i) = find(p, clip)?;
            let to = match w {
                SoundTrack::Sfx => SoundTrack::Sfx2,
                SoundTrack::Sfx2 => SoundTrack::Sfx,
                _ => return Err(OpError::Invalid(format!("sound clip #{clip} is on {}, not a sound effects lane", w.name()))),
            };
            p.move_clip(w, i, to);
            said(format!("sound clip #{clip} moved to {}", to.name()))
        }
        &ClipOp::Delete { clip } => {
            let (w, i) = find(p, clip)?;
            p.track_mut(w).clips.remove(i);
            said(format!("sound clip #{clip} removed"))
        }
    }
}

/// Which track clip `id` is on, and where.
fn find(p: &Project, id: Id) -> Result<(SoundTrack, usize), OpError> {
    p.clip_index(id).ok_or_else(|| OpError::NotFound(format!("sound clip #{id}")))
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;

    fn add(e: &mut Edit, track: SoundTrack, at: f64, len: f64) -> Id {
        e.apply(&Op::Clip(ClipOp::Add { track, input: "s.wav".into(), at, start: 0.0, end: len })).unwrap().made.unwrap()
    }

    #[test]
    fn a_sound_effect_over_another_goes_on_sfx_2_and_back() {
        let mut e = Edit::default();
        let bell = add(&mut e, SoundTrack::Sfx, 1.0, 2.0);
        let boom = add(&mut e, SoundTrack::Sfx, 2.0, 2.0);
        assert_eq!((e.sfx.clips.len(), e.sfx2.clips[0].id), (1, boom));
        e.apply(&Op::Clip(ClipOp::OtherSfx { clip: boom })).unwrap();
        assert_eq!(e.sfx.clips.len(), 2, "moved back, nearest its place where it fits");
        let song = add(&mut e, SoundTrack::Music, 0.0, 5.0);
        assert!(matches!(e.apply(&Op::Clip(ClipOp::OtherSfx { clip: song })), Err(OpError::Invalid(_))));
        e.apply(&Op::Clip(ClipOp::Delete { clip: bell })).unwrap();
        check(&e).unwrap();
    }

    #[test]
    fn clips_move_trim_and_split_by_id() {
        let mut e = Edit::default();
        let a = add(&mut e, SoundTrack::Music, 0.0, 4.0);
        let b = add(&mut e, SoundTrack::Music, 4.0, 4.0);
        // A plain move stops edge to edge: a can't go into b.
        e.apply(&Op::Clip(ClipOp::Move { clip: a, at: 1.0, overlap: false })).unwrap();
        assert!(e.music.clips[0].ends() <= e.music.clips[1].at + 1e-9);
        e.apply(&Op::Clip(ClipOp::Trim { clip: b, end: End::Out, t: 9.0, most: 6.0, overlap: false })).unwrap();
        assert_eq!(e.music.clips[1].end, 6.0, "not past its file");
        let half = e.apply(&Op::Clip(ClipOp::Split { clip: b, at: 6.0 })).unwrap().made.unwrap();
        assert_eq!(e.music.clips.iter().map(|c| c.id).collect::<Vec<_>>(), [a, b, half]);
        check(&e).unwrap();
        assert!(e.undo() && e.music.clips.len() == 2);
    }

    #[test]
    fn a_sound_clip_stops_at_the_films_end_unless_there_is_no_film() {
        let mut e = Edit::default();
        let song = add(&mut e, SoundTrack::Music, 0.0, 3.0);
        e.apply(&Op::Clip(ClipOp::Move { clip: song, at: 20.0, overlap: false })).unwrap();
        assert_eq!(e.music.clips[0].at, 20.0, "no pictures yet: no end to keep to");
        e.apply(&Op::Cut(super::super::cuts::CutOp::Add { input: "a.mp4".into(), start: 0.0, end: 10.0, place: super::super::cuts::Place::End, stabilize: None, lut: None })).unwrap();
        e.apply(&Op::Clip(ClipOp::Move { clip: song, at: 2.0, overlap: false })).unwrap();
        assert_eq!(e.music.clips[0].at, 2.0, "back from past the end: earlier is free");
        let said = e.apply(&Op::Clip(ClipOp::Move { clip: song, at: 9.0, overlap: false })).unwrap().summary;
        assert_eq!((e.music.clips[0].at, said.contains("held")), (7.0, true), "ends with the film");
        e.apply(&Op::Clip(ClipOp::Trim { clip: song, end: End::In, t: 1.0, most: 60.0, overlap: false })).unwrap();
        e.apply(&Op::Clip(ClipOp::Trim { clip: song, end: End::Out, t: 30.0, most: 60.0, overlap: false })).unwrap();
        assert!((e.music.clips[0].ends() - 10.0).abs() < 1e-9, "its tail stops there too: {:?}", e.music.clips[0]);
        // A song longer than the film: earlier and shorter, never later.
        let long = add(&mut e, SoundTrack::Sfx, 0.0, 30.0);
        e.apply(&Op::Clip(ClipOp::Move { clip: long, at: 2.0, overlap: false })).unwrap();
        assert_eq!(e.sfx.clips[0].at, 0.0, "not later");
        e.apply(&Op::Clip(ClipOp::Trim { clip: long, end: End::Out, t: 20.0, most: 60.0, overlap: false })).unwrap();
        e.apply(&Op::Clip(ClipOp::Trim { clip: long, end: End::Out, t: 25.0, most: 60.0, overlap: false })).unwrap();
        assert_eq!(e.sfx.clips[0].end, 20.0, "shorter, then not longer again");
        // A lane full to the end: its placement would push a clip past it.
        let mut e2 = Edit::default();
        e2.apply(&Op::Cut(super::super::cuts::CutOp::Add { input: "a.mp4".into(), start: 0.0, end: 10.0, place: super::super::cuts::Place::End, stabilize: None, lut: None })).unwrap();
        add(&mut e2, SoundTrack::Sfx, 1.0, 9.0);
        let b = add(&mut e2, SoundTrack::Sfx, 0.0, 1.0);
        let said = e2.apply(&Op::Clip(ClipOp::Move { clip: b, at: 9.0, overlap: false })).unwrap().summary;
        let (w, i) = e2.clip_index(b).unwrap();
        assert_eq!((w, e2.track(w).clips[i].at, said.contains("held")), (SoundTrack::Sfx, 0.0, true), "kept where it was");
        check(&e).unwrap();
    }
}
