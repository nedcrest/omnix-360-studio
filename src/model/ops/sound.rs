//! Operations on sound: a cut's or a sound clip's own level, switches,
//! fades and line, and the auto-duck. The rules moved here from the
//! player's sound menu and level tag (`OPS-API.md` phase 2).

use super::{OpError, Outcome, SoundRef};
use crate::project::{OFF_DB, Own, OwnSound, Project};
use serde::{Deserialize, Serialize};

/// How far «Lower for voice-over» takes a sound under its normal level:
/// the usual depth of a dip under speech.
pub const VOICE_OVER_DIP: f64 = -12.0;

/// The level a sound has when nothing has lowered it: what normalizing
/// gave it (from what it measured), or 0 dB as recorded.
pub fn normal_gain(loudness: Option<f64>) -> f64 {
    loudness.map_or(0.0, crate::loudness::normalizing_gain)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    In,
    Out,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum SoundOp {
    /// Its own level, dB (the level tag), from `OFF_DB` (silent) to
    /// `loudness::MOST_GAIN`.
    Level { sound: SoundRef, db: f64 },
    /// Its fade in or out, seconds; kept within its length less the other.
    Fade { sound: SoundRef, end: End, seconds: f64 },
    Mute { sound: SoundRef, on: bool },
    /// Its low rumble cut (`loudness::LOW_CUT`).
    LowCut { sound: SoundRef, on: bool },
    /// Its steady noise cleaned (`loudness::CLEAN`).
    Clean { sound: SoundRef, on: bool },
    /// Its line shown: points at its in and out at 0 dB, so nothing sounds
    /// different until one is moved.
    ShowLine { sound: SoundRef },
    /// Its line removed, all its points.
    DeleteLine { sound: SoundRef },
    /// Back to the file as it is: no level, evening, mute, points or fades.
    Original { sound: SoundRef },
    /// `VOICE_OVER_DIP` under its normal level, whatever it was.
    LowerForVoice { sound: SoundRef },
    /// Its normal level.
    Restore { sound: SoundRef },
    /// The music lowered `depth` dB over these stretches of speech (edit
    /// time, as `duck::find_speech` measures them), replacing a duck before.
    Duck { depth: f64, speech: Vec<(f64, f64)> },
}

pub(super) fn apply(p: &mut Project, op: &SoundOp) -> Result<Outcome, OpError> {
    let said = |summary: String| Ok(Outcome { summary });
    match op {
        &SoundOp::Level { sound, db } => {
            let most = crate::loudness::MOST_GAIN;
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
                // The level there kept (0 dB on a line with no points), and
                // a point already there reused: shown twice is shown once.
                for t in [s.range.0, s.range.1] {
                    crate::project::volume_key_at(s.volume, t);
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
        SoundOp::Duck { depth, speech } => {
            if !(0.0..=60.0).contains(depth) {
                return Err(OpError::OutOfRange { what: "depth", value: *depth, least: 0.0, most: 60.0 });
            }
            crate::duck::duck_music(&mut p.music, speech, *depth);
            said(format!("music {depth:.0} dB down under {} stretches of speech", speech.len()))
        }
    }
}

/// Runs `f` on `sound`'s settings.
fn change<T>(p: &mut Project, sound: SoundRef, f: impl FnOnce(OwnSound<'_>) -> T) -> Result<T, OpError> {
    let own: Own = sound.own(p).ok_or_else(|| OpError::NotFound(sound.to_string()))?;
    Ok(f(p.own_sound(own).expect("found just now")))
}

fn switch(p: &mut Project, sound: SoundRef, on: bool, what: &str, flag: impl for<'a> FnOnce(OwnSound<'a>) -> &'a mut bool) -> Result<Outcome, OpError> {
    change(p, sound, |s| *flag(s) = on)?;
    Ok(Outcome { summary: format!("{sound}: {what} {}", if on { "on" } else { "off" }) })
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
    use crate::project::{Cut, SoundTrack};

    /// Two cuts and a song, ids given.
    fn edit() -> Edit {
        let mut p = Project::default();
        p.cuts = vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut::new("b.mp4".into(), 0.0, 3.0)];
        p.music.append("song.wav".into(), 0.0, 10.0);
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
        for o in [|s| SoundOp::LowerForVoice { sound: s }, |s| SoundOp::Restore { sound: s }, |s| SoundOp::ShowLine { sound: s }, |_| SoundOp::Duck { depth: 18.0, speech: vec![(2.0, 5.0)] }] {
            let (mut once, mut twice) = (edit(), edit());
            once.apply(&op(o(s(&once)))).unwrap();
            twice.apply(&op(o(s(&twice)))).unwrap();
            twice.apply(&op(o(s(&twice)))).unwrap();
            assert_eq!(Project::clone(&once), Project::clone(&twice), "{:?}", o(s(&once)));
        }
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
