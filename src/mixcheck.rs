//! The mix check (gap 45): what in the sound breaks the rules a platform
//! and a listener set, from measured numbers, in words, each pointing at
//! the clip to fix. Pure: the numbers are measured elsewhere (`measure`);
//! here only the rules, so they're tested without ffmpeg.
//!
//! Sources disagree on the numbers (`EDITOR-GAPS.md` gap 45); the ones
//! used: the whole film at the platform's target (YouTube −14 LUFS),
//! peaks under −1 dBFS, speech steady within 4 LU, music at least 15 dB
//! under the voice while it speaks (WCAG asks 20 for speech-led video),
//! sound effects not more than 6 dB over the speech, nor so far under it
//! they're lost.

use crate::project::{Own, SoundTrack};

/// A lane the check looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    ClipSound,
    Track(SoundTrack),
}

/// One cut's sound or sound clip, measured as heard (its level, line,
/// fades and filters, the lane's level): where it plays and how loud.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub lane: Lane,
    pub own: Own,
    /// Where it plays on the edit, seconds.
    pub at: f64,
    pub len: f64,
    /// Integrated loudness over it, LUFS; `None` if silent.
    pub lufs: Option<f64>,
    /// Its loudest 400 ms, LUFS; `None` if silent.
    pub loudest: Option<f64>,
    /// Speech: a cleaned voice or a Voice take (`duck::sources`).
    pub speech: bool,
}

/// A stretch of speech, and how loud the speech and each other sound
/// under it are there, LUFS.
#[derive(Clone, Debug, PartialEq)]
pub struct Speech {
    pub from: f64,
    pub to: f64,
    pub voice: f64,
    pub under: Vec<(Own, f64)>,
}

/// What was measured of a project.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    pub items: Vec<Item>,
    pub speech: Vec<Speech>,
    /// The whole film's integrated loudness, LUFS, and its loudest sample,
    /// dBFS.
    pub mix: Option<f64>,
    pub peak: f64,
    /// Where the film ends, seconds (gap 101); none without pictures.
    pub end: Option<f64>,
}

/// The rules' numbers: a platform's target and the mixing guides'.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    pub target: f64,
    pub peak: f64,
    pub speech_spread: f64,
    pub music_under: f64,
    pub sfx_over: f64,
    pub sfx_lost: f64,
}

impl Rules {
    /// The guides' numbers at a platform's `target` LUFS.
    pub fn at(target: f64) -> Rules {
        Rules { target, peak: -1.0, speech_spread: 4.0, music_under: 15.0, sfx_over: 6.0, sfx_lost: 25.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Worth knowing; the export or a later step may handle it.
    Hint,
    /// Likely heard as a fault.
    Fix,
}

/// Something that breaks a rule: where (from `at` to `until` on the edit;
/// the whole film's at 0 for nothing), which clip (none for the whole
/// film), how bad, and what it is in words.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub at: f64,
    pub until: f64,
    pub own: Option<Own>,
    pub severity: Severity,
    pub text: String,
}

/// The middle value.
fn median(mut v: Vec<f64>) -> Option<f64> {
    v.sort_by(f64::total_cmp);
    (!v.is_empty()).then(|| if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 })
}

/// The findings for `m` under `rules`, in edit order, the whole film's
/// first.
pub fn check(m: &Measured, rules: &Rules) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut say = |(at, until): (f64, f64), own: Option<Own>, severity: Severity, text: String| found.push(Finding { at, until, own, severity, text });
    if let Some(mix) = m.mix {
        let off = mix - rules.target;
        if off < -1.0 {
            say((0.0, 0.0), None, Severity::Hint, format!("The film is {mix:.1} LUFS, {:.1} LU under {:.0}: export with Loudness at {:.0} to bring it up.", -off, rules.target, rules.target));
        } else if off > 1.0 {
            say((0.0, 0.0), None, Severity::Hint, format!("The film is {mix:.1} LUFS, {off:.1} LU over {:.0}: the platform will turn it down.", rules.target));
        }
    }
    if m.peak > rules.peak {
        say((0.0, 0.0), None, Severity::Fix, format!("Peaks reach {:.1} dBFS, over {:.0}: they may distort on some players.", m.peak, rules.peak));
    }
    // Speech against the rest of the speech.
    let speech: Vec<&Item> = m.items.iter().filter(|i| i.speech && i.lufs.is_some()).collect();
    let typical = median(speech.iter().filter_map(|i| i.lufs).collect());
    if let Some(typical) = typical {
        for i in &speech {
            let off = i.lufs.unwrap() - typical;
            if off < -rules.speech_spread {
                say((i.at, i.at + i.len), Some(i.own), Severity::Fix, format!("Speech {:.1} LU quieter than the rest: raise it, or normalize it.", -off));
            } else if off > rules.speech_spread {
                say((i.at, i.at + i.len), Some(i.own), Severity::Hint, format!("Speech {off:.1} LU louder than the rest."));
            }
        }
    }
    // Sound effects against the speech's loudest moments.
    let speech_peak = median(speech.iter().filter_map(|i| i.loudest).collect());
    for i in m.items.iter().filter(|i| matches!(i.lane, Lane::Track(SoundTrack::Sfx | SoundTrack::Sfx2))) {
        match (i.loudest, speech_peak) {
            (Some(l), Some(s)) if l - s > rules.sfx_over => say((i.at, i.at + i.len), Some(i.own), Severity::Fix, format!("Sound effect {:.1} dB over the speech: lower it.", l - s)),
            (Some(l), Some(s)) if s - l > rules.sfx_lost => say((i.at, i.at + i.len), Some(i.own), Severity::Hint, format!("Sound effect {:.1} dB under the speech: it may be lost.", s - l)),
            _ => {}
        }
    }
    // Music under each stretch of speech.
    for s in &m.speech {
        for &(own, level) in &s.under {
            let is_music = matches!(own, Own::Clip(SoundTrack::Music, _));
            let gap = s.voice - level;
            if is_music && gap < rules.music_under {
                say((s.from, s.to), Some(own), Severity::Fix, format!("Music only {gap:.1} dB under the voice at {:.1}-{:.1} s: lower it {:.0} dB more (or «Lower the music under speech»).", s.from, s.to, rules.music_under - gap));
            }
        }
    }
    // Sound past the pictures (gap 101).
    if let Some(end) = m.end {
        for i in m.items.iter().filter(|i| matches!(i.lane, Lane::Track(_)) && i.at + i.len > end + 1e-3) {
            say((end, i.at + i.len), Some(i.own), Severity::Fix, format!("Runs {:.1} s past the film's end: «Trim to the film's end» on its lane's menu.", i.at + i.len - end));
        }
    }
    found.sort_by(|a, b| a.at.total_cmp(&b.at).then(b.severity.cmp(&a.severity)));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(lane: Lane, own: Own, at: f64, lufs: f64, loudest: f64, speech: bool) -> Item {
        Item { lane, own, at, len: 2.0, lufs: Some(lufs), loudest: Some(loudest), speech }
    }

    fn says(found: &[Finding], own: Option<Own>, words: &str) -> bool {
        found.iter().any(|f| f.own == own && f.text.contains(words))
    }

    #[test]
    fn a_tidy_mix_has_nothing_to_say() {
        let voice = Own::Clip(SoundTrack::Voice, 0);
        let m = Measured {
            items: vec![item(Lane::Track(SoundTrack::Voice), voice, 1.0, -16.0, -12.0, true), item(Lane::Track(SoundTrack::Sfx), Own::Clip(SoundTrack::Sfx, 0), 4.0, -20.0, -14.0, false)],
            speech: vec![Speech { from: 1.0, to: 3.0, voice: -16.0, under: vec![(Own::Clip(SoundTrack::Music, 0), -33.0)] }],
            mix: Some(-14.3),
            peak: -1.5,
            end: Some(6.0),
        };
        assert_eq!(check(&m, &Rules::at(-14.0)), vec![]);
    }

    #[test]
    fn each_rule_points_at_the_clip_to_fix() {
        let (quiet, loud_sfx, lost_sfx, song) = (Own::Cut(2), Own::Clip(SoundTrack::Sfx, 0), Own::Clip(SoundTrack::Sfx2, 0), Own::Clip(SoundTrack::Music, 0));
        let m = Measured {
            items: vec![
                item(Lane::ClipSound, Own::Cut(0), 0.0, -16.0, -12.0, true),
                item(Lane::Track(SoundTrack::Voice), Own::Clip(SoundTrack::Voice, 0), 5.0, -17.0, -13.0, true),
                item(Lane::ClipSound, quiet, 9.0, -23.0, -19.0, true),
                item(Lane::Track(SoundTrack::Sfx), loud_sfx, 3.0, -8.0, -4.0, false),
                item(Lane::Track(SoundTrack::Sfx2), lost_sfx, 7.0, -45.0, -42.0, false),
            ],
            speech: vec![Speech { from: 5.0, to: 7.0, voice: -17.0, under: vec![(song, -25.0)] }],
            mix: Some(-19.0),
            peak: -0.4,
            end: Some(10.0),
        };
        let found = check(&m, &Rules::at(-14.0));
        assert!(says(&found, None, "5.0 LU under -14"), "{found:#?}");
        assert!(says(&found, None, "Peaks reach -0.4"));
        assert!(says(&found, Some(quiet), "Speech 6.0 LU quieter"));
        assert!(says(&found, Some(loud_sfx), "9.0 dB over the speech"), "against the speech's typical loudest, -13");
        assert!(says(&found, Some(lost_sfx), "may be lost"));
        assert!(!found.iter().any(|f| f.text.contains("past the film")), "all within 10 s");
        let m = Measured { end: Some(8.0), ..m };
        assert!(says(&check(&m, &Rules::at(-14.0)), Some(lost_sfx), "1.0 s past the film's end"), "7-9 s against 8: the sound clip's tail");
        assert!(!says(&check(&m, &Rules::at(-14.0)), Some(quiet), "past the film"), "a cut's own sound ends with it");
        assert!(says(&found, Some(song), "only 8.0 dB under the voice") && says(&found, Some(song), "7 dB more"));
        // In edit order, the whole film's first.
        assert!(found.windows(2).all(|w| w[0].at <= w[1].at));
        assert_eq!(found.len(), 6);
    }
}
