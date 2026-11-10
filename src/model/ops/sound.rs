//! Operations on sound: a cut's or a sound clip's own level, switches,
//! fades and line, and the auto-duck. The rules moved here from the
//! player's sound menu and level tag (`OPS-API.md` phase 2).

use super::{End, OpError, Outcome, SoundRef, said};
use crate::project::{OFF_DB, Own, OwnSound, Project, SoundTrack};
use serde::{Deserialize, Serialize};

/// How far «Lower for voice-over» takes a sound under its normal level:
/// the usual depth of a dip under speech.
pub const VOICE_OVER_DIP: f64 = -12.0;

/// The level a sound has when nothing has lowered it: what normalizing
/// gave it (from what it measured), or 0 dB as recorded.
pub fn normal_gain(loudness: Option<f64>) -> f64 {
    loudness.map_or(0.0, crate::model::level::normalizing_gain)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum SoundOp {
    /// Its own level, dB (the level tag), from `OFF_DB` (silent) to
    /// `model::level::MOST_GAIN`.
    Level { sound: SoundRef, db: f64 },
    /// Its fade in or out, seconds; kept within its length less the other.
    Fade { sound: SoundRef, end: End, seconds: f64 },
    /// Silenced (`on`), or heard again at its level.
    Mute { sound: SoundRef, on: bool },
    /// Its low rumble cut (`loudness::LOW_CUT`).
    LowCut { sound: SoundRef, on: bool },
    /// Its steady noise cleaned (`loudness::CLEAN`).
    Clean { sound: SoundRef, on: bool },
    /// Its line shown: one point in its middle at the level there (0 dB),
    /// so nothing sounds different until it's moved. The line runs flat
    /// from its first and last points to its edges, wherever they go.
    ShowLine { sound: SoundRef },
    /// Its line removed, all its points.
    DeleteLine { sound: SoundRef },
    /// Back to the file as it is: no level, evening, mute, points or fades.
    Original { sound: SoundRef },
    /// `VOICE_OVER_DIP` under its normal level, whatever it was.
    LowerForVoice { sound: SoundRef },
    /// Its normal level.
    Restore { sound: SoundRef },
    /// A point on its line at `t` (its file's time), at `db`; points are
    /// told apart by time (`project::SAME_POINT`).
    AddPoint { sound: SoundRef, t: f64, db: f64 },
    /// A point on its line at `t` at the level the line has there, or the
    /// one already there: what K does.
    PointAt { sound: SoundRef, t: f64 },
    /// The point at `from` moved to `t` and `db`, kept within its stretch
    /// and between its neighbours.
    MovePoint { sound: SoundRef, from: f64, t: f64, db: f64 },
    /// The point at `t` (file time) on its line gone.
    DeletePoint { sound: SoundRef, t: f64 },
    /// Its line started where what plays before it ends
    /// (`Project::continue_level`).
    ContinueLevel { sound: SoundRef },
    /// What normalizing measured, applied: evened out or not, and given the
    /// level its loudness asks for (`normal_gain`). `lufs` is the
    /// loudness measured as it will sound (`loudness::measure_stretch`,
    /// evened if `even`); `None` for silence or not measured, which only
    /// sets the evening.
    Normalized { sound: SoundRef, lufs: Option<f64>, even: bool },
    /// The music lowered `depth` dB over these stretches of speech (edit
    /// time, as `duck::find_speech` measures them), replacing a duck before.
    Duck { depth: f64, speech: Vec<(f64, f64)> },
    /// A track's level, dB (the Audio panel's), or the clips' own sound's
    /// without a `track`: from `OFF_DB` (silent) to `TRACK_MOST`.
    TrackLevel { track: Option<SoundTrack>, db: f64 },
    /// The whole edit's fade from or to black, its sound with it: seconds,
    /// 0 to `EDIT_FADE_MOST`.
    EditFade { end: End, seconds: f64 },
}

/// The loudest a track's level goes, dB.
pub const TRACK_MOST: f64 = 6.0;
/// The longest fade of the whole edit, seconds.
pub const EDIT_FADE_MOST: f64 = 5.0;

pub(super) fn apply(p: &mut Project, op: &SoundOp) -> Result<Outcome, OpError> {
    match op {
        &SoundOp::Level { sound, db } => {
            let most = crate::model::level::MOST_GAIN;
            if !(OFF_DB..=most).contains(&db) {
                return Err(OpError::OutOfRange { what: "level", value: db, least: OFF_DB, most });
            }
            let was = change(p, sound, |s| std::mem::replace(s.gain, db))?;
            said(format!("{sound}: level {was:+.1} → {db:+.1} dB"))
        }
        &SoundOp::Fade { sound, end, seconds } => {
            let now = change(p, sound, |mut s| {
                s.set_fade(end == End::In, seconds);
                if end == End::In { *s.fade_in } else { *s.fade_out }
            })?;
            said(format!("{sound}: fade {} {now:.2} s", if end == End::In { "in" } else { "out" }))
        }
        &SoundOp::Mute { sound, on } => switch(p, sound, on, "muted", |s| s.mute),
        &SoundOp::LowCut { sound, on } => switch(p, sound, on, "low cut", |s| s.low_cut),
        &SoundOp::Clean { sound, on } => switch(p, sound, on, "cleaned", |s| s.clean),
        &SoundOp::ShowLine { sound } => {
            change(p, sound, |s| {
                // Not at its edges: there a point stayed behind when the
                // clip was trimmed longer (user, 2026-11-07). A line already
                // there is shown: shown twice is shown once.
                if s.volume.is_empty() {
                    crate::project::volume_key_at(s.volume, (s.range.0 + s.range.1) / 2.0);
                }
            })?;
            said(format!("{sound}: line shown"))
        }
        &SoundOp::DeleteLine { sound } => {
            let n = change(p, sound, |s| std::mem::take(s.volume).len())?;
            forget_duck(p, sound);
            said(format!("{sound}: line removed ({n} points)"))
        }
        &SoundOp::Original { sound } => {
            change(p, sound, |mut s| s.original())?;
            forget_duck(p, sound);
            said(format!("{sound}: back to the original"))
        }
        &SoundOp::LowerForVoice { sound } => {
            let db = change(p, sound, |s| {
                *s.gain = normal_gain(*s.loudness) + VOICE_OVER_DIP;
                *s.gain
            })?;
            said(format!("{sound}: lowered for voice-over, level {db:+.1} dB"))
        }
        &SoundOp::Restore { sound } => {
            let db = change(p, sound, |s| {
                *s.gain = normal_gain(*s.loudness);
                *s.gain
            })?;
            said(format!("{sound}: normal level {db:+.1} dB"))
        }
        &SoundOp::AddPoint { sound, t, db } => {
            within(p, sound, t)?;
            change(p, sound, |s| crate::project::add_volume_key(s.volume, t, db))?;
            said(format!("{sound}: point at {t:.2} s, {db:+.1} dB"))
        }
        &SoundOp::PointAt { sound, t } => {
            within(p, sound, t)?;
            let db = change(p, sound, |s| {
                let i = crate::project::volume_key_at(s.volume, t);
                s.volume[i].volume
            })?;
            said(format!("{sound}: point at {t:.2} s, {db:+.1} dB"))
        }
        &SoundOp::MovePoint { sound, from, t, db } => {
            let now = change(p, sound, |s| {
                let i = point(s.volume, from)?;
                crate::project::move_volume_key(s.volume, i, t, db, s.range);
                Some(s.volume[i])
            })?
            .ok_or_else(|| OpError::NotFound(format!("point at {from:.2} s on {sound}")))?;
            said(format!("{sound}: point {from:.2} s → {:.2} s, {:+.1} dB", now.t, now.volume))
        }
        &SoundOp::DeletePoint { sound, t } => {
            change(p, sound, |s| point(s.volume, t).map(|i| s.volume.remove(i)))?.ok_or_else(|| OpError::NotFound(format!("point at {t:.2} s on {sound}")))?;
            said(format!("{sound}: point at {t:.2} s removed"))
        }
        &SoundOp::ContinueLevel { sound } => {
            let own = sound.own(p).ok_or_else(|| OpError::NotFound(sound.to_string()))?;
            if !p.continue_level(own) {
                return Err(OpError::Invalid(format!("{sound}: nothing sounds before it to continue")));
            }
            said(format!("{sound}: line continues the level before it"))
        }
        &SoundOp::Normalized { sound, lufs, even } => {
            if lufs.is_some_and(|l| !l.is_finite()) {
                return Err(OpError::OutOfRange { what: "loudness", value: lufs.unwrap(), least: f64::MIN, most: f64::MAX });
            }
            let db = change(p, sound, |s| {
                *s.even = even;
                if lufs.is_some() {
                    *s.loudness = lufs;
                    *s.gain = normal_gain(lufs);
                }
                *s.gain
            })?;
            let evened = if even { ", evened out" } else { "" };
            said(match lufs {
                Some(l) => format!("{sound}: {l:.1} LUFS, level {db:+.1} dB{evened}"),
                None => format!("{sound}: level kept {db:+.1} dB{evened}"),
            })
        }
        SoundOp::Duck { depth, speech } => {
            if !(0.0..=60.0).contains(depth) {
                return Err(OpError::OutOfRange { what: "depth", value: *depth, least: 0.0, most: 60.0 });
            }
            crate::duck::duck_music(&mut p.music, speech, *depth);
            said(format!("music {depth:.0} dB down under {} stretches of speech", speech.len()))
        }
        &SoundOp::TrackLevel { track, db } => {
            if !(OFF_DB..=TRACK_MOST).contains(&db) {
                return Err(OpError::OutOfRange { what: "track level", value: db, least: OFF_DB, most: TRACK_MOST });
            }
            let level = match track {
                Some(w) => &mut p.track_mut(w).volume,
                None => &mut p.sound.volume,
            };
            let was = std::mem::replace(level, db);
            said(format!("{}: level {was:+.1} → {db:+.1} dB", track.map_or("clip sound", SoundTrack::name)))
        }
        &SoundOp::EditFade { end, seconds } => {
            if !(0.0..=EDIT_FADE_MOST).contains(&seconds) {
                return Err(OpError::OutOfRange { what: "edit fade", value: seconds, least: 0.0, most: EDIT_FADE_MOST });
            }
            let fade = if end == End::In { &mut p.fades.fade_in } else { &mut p.fades.fade_out };
            *fade = seconds;
            said(format!("the edit fades {} over {seconds:.1} s", if end == End::In { "in" } else { "out" }))
        }
    }
}

/// Runs `f` on `sound`'s settings.
fn change<T>(p: &mut Project, sound: SoundRef, f: impl FnOnce(OwnSound<'_>) -> T) -> Result<T, OpError> {
    let own: Own = sound.own(p).ok_or_else(|| OpError::NotFound(sound.to_string()))?;
    Ok(f(p.own_sound(own).expect("found just now")))
}

/// Refuses a time outside `sound`'s stretch of its file.
fn within(p: &mut Project, sound: SoundRef, t: f64) -> Result<(), OpError> {
    let (least, most) = change(p, sound, |s| s.range)?;
    if (least..=most).contains(&t) { Ok(()) } else { Err(OpError::OutOfRange { what: "point time", value: t, least, most }) }
}

/// The point at `t` on `keys`, told apart by `SAME_POINT`.
fn point(keys: &[crate::project::VolumeKey], t: f64) -> Option<usize> {
    keys.iter().position(|k| (k.t - t).abs() < crate::project::SAME_POINT)
}

fn switch(p: &mut Project, sound: SoundRef, on: bool, what: &str, flag: impl for<'a> FnOnce(OwnSound<'a>) -> &'a mut bool) -> Result<Outcome, OpError> {
    change(p, sound, |s| *flag(s) = on)?;
    said(format!("{sound}: {what} {}", if on { "on" } else { "off" }))
}

fn forget_duck(p: &mut Project, sound: SoundRef) {
    if let Some(own) = sound.own(p) {
        p.forget_duck(own);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;
    use crate::project::Cut;

    /// Two cuts and a song, ids given.
    fn edit() -> Edit {
        let mut p = Project::default();
        p.cuts = vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut::new("b.mp4".into(), 0.0, 3.0)];
        p.music.append_part("song.wav".into(), 0.0, 0.0, 10.0);
        p.settle_ids();
        let mut e = Edit::default();
        e.replace(p);
        e
    }

    fn song(e: &Edit) -> SoundRef {
        SoundRef::Clip(e.track(SoundTrack::Music).clips[0].id)
    }

    fn op(o: SoundOp) -> Op {
        Op::Sound(o)
    }

    #[test]
    fn track_levels_and_the_edits_fades_are_ops() {
        let mut e = edit();
        e.apply(&op(SoundOp::TrackLevel { track: Some(SoundTrack::Sfx), db: -6.0 })).unwrap();
        e.apply(&op(SoundOp::TrackLevel { track: None, db: 3.0 })).unwrap();
        e.apply(&op(SoundOp::EditFade { end: End::Out, seconds: 2.5 })).unwrap();
        assert_eq!((e.track(SoundTrack::Sfx).volume, e.sound.volume, e.fades.fade_in, e.fades.fade_out), (-6.0, 3.0, 0.0, 2.5));
        assert!(e.apply(&op(SoundOp::TrackLevel { track: None, db: 7.0 })).is_err());
        assert!(e.apply(&op(SoundOp::EditFade { end: End::In, seconds: -1.0 })).is_err());
        assert!(e.undo() && e.fades.fade_out == 0.0, "one undo step each");
    }

    #[test]
    fn an_op_is_one_undo_step_and_says_what_it_did() {
        let mut e = edit();
        let s = song(&e);
        let done = e.apply(&op(SoundOp::Level { sound: s, db: -8.0 })).unwrap();
        assert_eq!(done.summary, format!("sound clip #{}: level +0.0 → -8.0 dB", 3));
        assert_eq!(e.music.clips[0].gain, -8.0);
        check(&e).unwrap();
        assert!(e.undo() && e.music.clips[0].gain == 0.0 && !e.undo());
    }

    #[test]
    fn refused_it_changes_nothing() {
        let mut e = edit();
        let before = Project::clone(&e);
        assert!(matches!(e.apply(&op(SoundOp::Level { sound: song(&e), db: 40.0 })), Err(OpError::OutOfRange { .. })));
        assert!(matches!(e.apply(&op(SoundOp::Mute { sound: SoundRef::Cut(99), on: true })), Err(OpError::NotFound(_))));
        // A batch is all or none.
        let ops = [op(SoundOp::Mute { sound: SoundRef::Cut(1), on: true }), op(SoundOp::Mute { sound: SoundRef::Cut(99), on: true })];
        assert_eq!(e.batch(&ops).unwrap_err().0, 1);
        assert_eq!(*e, before);
        assert!(!e.undo(), "no step taken");
    }

    #[test]
    fn ops_from_the_sound_twice_are_once() {
        let s = |e: &Edit| song(e);
        for o in [|s| SoundOp::LowerForVoice { sound: s }, |s| SoundOp::Restore { sound: s }, |s| SoundOp::ShowLine { sound: s }, |_| SoundOp::Duck { depth: 18.0, speech: vec![(2.0, 5.0)] }, |s| SoundOp::Normalized { sound: s, lufs: Some(-25.0), even: true }] {
            let (mut once, mut twice) = (edit(), edit());
            once.apply(&op(o(s(&once)))).unwrap();
            twice.apply(&op(o(s(&twice)))).unwrap();
            twice.apply(&op(o(s(&twice)))).unwrap();
            assert_eq!(Project::clone(&once), Project::clone(&twice), "{:?}", o(s(&once)));
        }
    }

    #[test]
    fn a_shown_line_has_no_point_at_an_edge_to_stay_behind() {
        let mut e = edit();
        let s = song(&e);
        e.apply(&op(SoundOp::ShowLine { sound: s })).unwrap();
        let c = &e.music.clips[0];
        assert_eq!(c.volume.len(), 1);
        assert!((c.volume[0].t - (c.start + c.end) / 2.0).abs() < 1e-9 && c.volume[0].volume == 0.0);
    }

    #[test]
    fn points_are_found_by_time() {
        let mut e = edit();
        let s = song(&e);
        e.apply(&op(SoundOp::AddPoint { sound: s, t: 2.0, db: -6.0 })).unwrap();
        e.apply(&op(SoundOp::PointAt { sound: s, t: 6.0 })).unwrap();
        assert_eq!(e.music.clips[0].volume.len(), 2);
        assert_eq!(e.music.clips[0].volume[1].volume, -6.0, "K keeps the line's level");
        // K on a point already there adds none.
        e.apply(&op(SoundOp::PointAt { sound: s, t: 6.005 })).unwrap();
        assert_eq!(e.music.clips[0].volume.len(), 2);
        // A move stops at its neighbour.
        e.apply(&op(SoundOp::MovePoint { sound: s, from: 6.0, t: 1.0, db: -3.0 })).unwrap();
        assert!(e.music.clips[0].volume[1].t > 2.0);
        assert!(matches!(e.apply(&op(SoundOp::DeletePoint { sound: s, t: 4.0 })), Err(OpError::NotFound(_))));
        assert!(matches!(e.apply(&op(SoundOp::AddPoint { sound: s, t: 11.0, db: 0.0 })), Err(OpError::OutOfRange { .. })));
        e.apply(&op(SoundOp::DeletePoint { sound: s, t: 2.0 })).unwrap();
        assert_eq!(e.music.clips[0].volume.len(), 1);
        check(&e).unwrap();
    }

    #[test]
    fn deleting_a_ducked_line_forgets_the_duck() {
        let mut e = edit();
        let s = song(&e);
        e.apply(&op(SoundOp::Duck { depth: 18.0, speech: vec![(2.0, 5.0)] })).unwrap();
        e.apply(&op(SoundOp::DeleteLine { sound: s })).unwrap();
        assert!(e.music.clips[0].ducked.is_none());
        e.apply(&op(SoundOp::Duck { depth: 18.0, speech: vec![(2.0, 5.0)] })).unwrap();
        assert!(e.music.clips[0].volume.iter().all(|k| k.volume <= 1e-9), "not raised");
    }
}
