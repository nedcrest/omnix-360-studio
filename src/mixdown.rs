//! How loud a sound is at a moment of the edit: the rules the player, the
//! timeline's waveforms and the contact sheet share, and that the export's
//! ffmpeg graph (`ffmpeg/mix.rs`) is tested against. One place, so the
//! five copies that drifted apart (`CORE-API.md`, A) can't again.

use crate::project::{END_FADE, Music, MusicClip, OFF_DB, fade_ramp, gain, ramp};

/// An equal-power crossfade's share at `x` of the way through (0..1): a
/// quarter sine, so two sounds trading places stay as loud as one.
pub fn equal_power(x: f64) -> f64 {
    (x.clamp(0.0, 1.0) * std::f64::consts::FRAC_PI_2).sin()
}

/// A sound's own level with its volume points, dB: off at `OFF_DB` (muted)
/// whatever the points say.
pub fn own_level(own_db: f64, points_db: f64) -> f64 {
    if own_db <= OFF_DB { OFF_DB } else { own_db + points_db }
}

/// A cut's sound as the speakers get it: its own level and points
/// (`own_level`), times the clips' level, times its fades (`fade`, 0..1).
pub fn cut_gain(clips_db: f64, own_db: f64, points_db: f64, fade: f64) -> f64 {
    gain(own_level(own_db, points_db)) * gain(clips_db) * fade
}

/// How loud sound clip `clip` is at edit time `t` from its own fades, its
/// `crossfades` with the clips before and after it (equal power, so the
/// loudness holds through the middle), and the fade-out before `video_end`
/// when it runs on past it, 0..1. All multiply, as the export's chained
/// fades do.
pub fn envelope(clip: &MusicClip, (cross_in, cross_out): (f64, f64), t: f64, video_end: f64) -> f64 {
    let into = t - clip.at;
    let left = clip.ends() - t;
    let mut g = fade_ramp(into, left, clip.fade_in, clip.fade_out);
    if clip.ends() > video_end {
        g *= ramp(video_end - t, END_FADE);
    }
    if cross_in > 0.0 {
        g *= equal_power(into / cross_in);
    }
    if cross_out > 0.0 {
        g *= equal_power(left / cross_out);
    }
    g
}

/// Sound clip `i` of `track` at edit time `t`, as the speakers get it: the
/// track's level and line, the clip's own level and line, its envelope.
pub fn clip_gain(track: &Music, i: usize, t: f64, video_end: f64) -> f64 {
    let clip = &track.clips[i];
    gain(track.level_at(t)) * gain(clip.own_db_at(t)) * envelope(clip, track.crossfades(i), t, video_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_in_out_and_before_the_video_ends() {
        let clip = MusicClip { input: "song.mp3".into(), at: 10.0, start: 0.0, end: 100.0, fade_in: 2.0, ..Default::default() };
        let none = (0.0, 0.0);
        assert_eq!(envelope(&clip, none, 11.0, 1000.0), 0.5);
        assert_eq!(envelope(&clip, none, 50.0, 1000.0), 1.0);
        // The video ends at 60 s, so it fades over the last 3 s before it.
        assert_eq!(envelope(&clip, none, 58.5, 60.0), 0.5);
        assert_eq!(envelope(&clip, none, 56.0, 60.0), 1.0);
        let short = MusicClip { fade_out: 4.0, end: 20.0, ..clip.clone() };
        assert_eq!(envelope(&short, none, 28.0, 1000.0), 0.5);
        // Halfway through a 4 s crossfade into the next clip: equal power,
        // so the two together are as loud as one.
        let out = envelope(&clip, (0.0, 4.0), 108.0, 1000.0);
        assert!((out * out - 0.5).abs() < 1e-9, "{out}");
    }

    #[test]
    fn a_cuts_sound_as_the_export_mixes_it() {
        // Muted is silent even with a point above 0 dB.
        assert_eq!(cut_gain(0.0, OFF_DB, 6.0, 1.0), 0.0);
        // Its own level and points go off at OFF_DB on their own, before
        // the clips' level, as each stretch's volume filter does.
        assert_eq!(cut_gain(10.0, -30.0, -12.0, 1.0), 0.0);
        assert!((cut_gain(-6.0, 3.0, 3.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((cut_gain(0.0, 0.0, 0.0, 0.25) - 0.25).abs() < 1e-9);
    }
}
