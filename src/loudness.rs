//! How loud a stretch of sound is, as the ear hears it: integrated loudness
//! in LUFS (EBU R 128, ffmpeg's `ebur128`), and the gain that brings a cut's
//! sound to the edit's target.
//!
//! Why: the user's footage (Kos, 2026-10) measured 15 dB under what web
//! video is heard at. 0 dB on a level means "as recorded", not "loud
//! enough", so a cut is measured and given its own level (`Cut::gain`);
//! the export's limiter (`ffmpeg::mix::LIMITER`) keeps the raised peaks under
//! full scale.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Stdio;

/// What a cut's speech is brought to: web video and podcasts sit around
/// −14 to −16 LUFS; −16 leaves room for music under it.
pub const TARGET_LUFS: f64 = -16.0;
/// The most a cut is raised or lowered when normalized, dB.
pub const MOST_GAIN: f64 = 24.0;
/// The speech leveler a cut gets with `Cut::even`: over 150 ms frames
/// with a short window, quiet stretches are lifted towards the loud ones
/// (at most 12x), loud ones left alone. `b=1` (alternative boundary mode)
/// takes the first and last frames' own gain: without it the gain ramps up
/// from 1 at every cut's start and the first second or two came out 13-15
/// dB quiet. Measured on Kos (2026-10-28): a cleaned voice's quiet words
/// +5.5 dB against plain gain, its spread quiet→loud 18.0 → 13.0 dB; a
/// clip's own sound 11.0 → 4.3. The gentler `f=250:g=15:m=8` lifted the
/// voice only 2.5 dB, which the user couldn't hear. Samples for choosing by
/// ear: EDITOR-GAPS gap 11.
pub const LEVELER: &str = "dynaudnorm=f=150:g=7:p=0.9:m=12:b=1";

/// «Cut the low rumble»: a high-pass at 120 Hz, two poles. Wind on a
/// scooter and handling rumble live under ~150 Hz; voices start around
/// 85-180 Hz, so little of them goes (gap 46c).
pub const LOW_CUT: &str = "highpass=f=120:p=2";

/// «Clean up steady noise»: FFT denoising that follows the noise floor as
/// it changes (`tn`), 12 dB of reduction (ffmpeg's default), floor −40 dB.
/// For hiss, hum, an engine's drone; not for gusts (gap 46c).
pub const CLEAN: &str = "afftdn=nr=12:nf=-40:tn=1";

/// The filters a cut's or a sound clip's sound goes through, in order: the
/// rumble cut first, then the denoising, the leveler last (it should even
/// out the voice, not the wind or the hiss). `None` when there are none.
pub fn sound_filter(even: bool, low_cut: bool, clean: bool) -> Option<String> {
    let parts: Vec<&str> = [(low_cut, LOW_CUT), (clean, CLEAN), (even, LEVELER)].into_iter().filter_map(|(on, f)| on.then_some(f)).collect();
    (!parts.is_empty()).then(|| parts.join(","))
}
/// Quieter than this is silence, not something to raise.
const SILENT_LUFS: f64 = -70.0;

/// The integrated loudness of `duration` seconds of `input`'s first sound
/// track from `start`, LUFS, after `filter` if any (as the cut will sound);
/// `None` if it's silent there.
pub fn measure(input: &Path, start: f64, duration: f64, filter: Option<&str>) -> Result<Option<f64>> {
    let chain = filter.map_or("ebur128=framelog=quiet".to_owned(), |f| format!("{f},ebur128=framelog=quiet"));
    let out = crate::ffmpeg::tool("ffmpeg")
        .args(["-hide_banner", "-nostats", "-nostdin", "-ss", &start.to_string(), "-t", &duration.to_string(), "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-af", &chain, "-f", "null", "-"])
        .stdout(Stdio::null())
        .output()
        .context("running ffmpeg")?;
    if !out.status.success() {
        bail!("ffmpeg couldn't read the sound of {}", input.display());
    }
    // The summary comes last: "I:         -30.7 LUFS".
    let text = String::from_utf8_lossy(&out.stderr);
    let lufs = text
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("I:").and_then(|v| v.trim().trim_end_matches("LUFS").trim().parse::<f64>().ok()))
        .with_context(|| format!("no loudness measured for {}", input.display()))?;
    Ok((lufs > SILENT_LUFS).then_some(lufs))
}

/// The gain, dB, that brings `lufs` to the target (unbounded;
/// `normalizing_gain` holds it within `MOST_GAIN`).
pub fn gain_to_target(lufs: f64) -> f64 {
    ((TARGET_LUFS - lufs) * 10.0).round() / 10.0
}

/// `gain_to_target` held within what normalizing may do.
pub fn normalizing_gain(lufs: f64) -> f64 {
    gain_to_target(lufs).clamp(-MOST_GAIN, MOST_GAIN)
}

/// Measures what cut `cut` plays over its own stretch: its loudness and
/// the gain that normalizes it, or `None` where it's silent (a picture, a
/// clip without sound).
pub fn measure_cut(cut: &crate::project::Cut) -> Result<Option<(f64, f64)>> {
    measure_stretch(&cut.sound_file(), (cut.start, cut.end), cut.sound_filter().as_deref())
}

/// Measures `file` from `start` to `end` through `filter` (its evening and
/// cleaning, `sound_filter`): its
/// loudness and the gain that normalizes it, or `None` where it's silent.
/// A cut's sound and a sound clip's alike.
pub fn measure_stretch(file: &std::path::Path, (start, end): (f64, f64), filter: Option<&str>) -> Result<Option<(f64, f64)>> {
    if !crate::ffmpeg::has_audio(file).unwrap_or(false) {
        return Ok(None);
    }
    Ok(measure(file, start, end - start, filter)?.map(|lufs| (lufs, normalizing_gain(lufs))))
}

/// `rosmo360 normalize`: measures each cut (all, or those numbered in
/// `only`, from 1), evened out first if `even`, and gives it the gain to the
/// target, printing a line each; writes the project to `output` if given.
pub fn normalize(project: &std::path::Path, only: &[usize], even: bool, output: Option<&std::path::Path>) -> Result<()> {
    let mut p = crate::project::Project::load(project)?;
    for (n, cut) in p.cuts.iter_mut().enumerate() {
        if !only.is_empty() && !only.contains(&(n + 1)) {
            continue;
        }
        let name = cut.input.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        // Measured as it will sound; changed only once measured, so a cut
        // that can't be read is left as it was and the rest go on, as in
        // the app's dialog.
        let mut trial = cut.clone();
        trial.even = even;
        match measure_cut(&trial) {
            Ok(Some((lufs, gain))) => {
                println!("#{:<3} {name}  {lufs:.1} LUFS  {:+.1} dB", n + 1, gain);
                cut.even = even;
                cut.gain = gain;
                cut.loudness = Some(lufs);
            }
            Ok(None) => println!("#{:<3} {name}  silent, left as it is", n + 1),
            Err(e) => println!("#{:<3} {name}  couldn't measure, left as it is: {e:#}", n + 1),
        }
    }
    if let Some(out) = output {
        p.save(out)?;
        eprintln!("wrote {}", out.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_run_rumble_cut_then_cleaning_then_evening() {
        assert_eq!(sound_filter(false, false, false), None);
        assert_eq!(sound_filter(true, true, true).unwrap(), format!("{LOW_CUT},{CLEAN},{LEVELER}"));
        assert_eq!(sound_filter(true, false, true).unwrap(), format!("{CLEAN},{LEVELER}"));
    }

    #[test]
    fn measures_a_level_and_a_change_in_it() {
        let dir = std::env::temp_dir().join(format!("rosmo360-loud-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let make = |name: &str, filter: &str| {
            let path = dir.join(name);
            let ok = crate::ffmpeg::tool("ffmpeg")
                .args(["-v", "error", "-y", "-f", "lavfi", "-i", filter, "-ar", "48000"])
                .arg(&path)
                .status()
                .unwrap()
                .success();
            assert!(ok);
            path
        };
        let quiet = make("quiet.wav", "sine=f=440:d=6,volume=-30dB");
        let louder = make("louder.wav", "sine=f=440:d=6,volume=-24dB");
        let silent = make("silent.wav", "anullsrc=r=48000:cl=mono,atrim=duration=3");
        let (a, b) = (measure(&quiet, 0.0, 6.0, None).unwrap().unwrap(), measure(&louder, 0.0, 6.0, None).unwrap().unwrap());
        // 6 dB louder reads 6 LU more, and the gain to the target follows.
        assert!(((b - a) - 6.0).abs() < 0.2, "{a} {b}");
        assert!((gain_to_target(a) - (TARGET_LUFS - a)).abs() < 0.06);
        // Just a stretch of it measures the same, and silence is none.
        assert!((measure(&quiet, 2.0, 3.0, None).unwrap().unwrap() - a).abs() < 0.3);
        assert_eq!(measure(&silent, 0.0, 3.0, None).unwrap(), None);
        // The leveler lifts a steady quiet tone towards its peak.
        assert!(measure(&quiet, 0.0, 6.0, Some(LEVELER)).unwrap().unwrap() > a + 6.0);
        assert_eq!(normalizing_gain(-60.0), MOST_GAIN);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
