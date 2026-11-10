//! How loud a stretch of sound is, as the ear hears it: integrated loudness
//! in LUFS (EBU R 128, measured by our own meter, `lufs.rs`, as the player
//! and the mix check do), and the gain that brings a cut's sound to the
//! edit's target.
//!
//! Why: the user's footage (Kos, 2026-10) measured 15 dB under what web
//! video is heard at. 0 dB on a level means "as recorded", not "loud
//! enough", so a cut is measured and given its own level (`Cut::gain`);
//! the export's limiter (`ffmpeg::LIMITER`) keeps the raised peaks under
//! full scale.

use crate::model::level::normalizing_gain;
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Stdio;

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
/// `None` if it's silent there. Our meter, as the player's and the mix
/// check's (user, 2026-11-05: one meter, not ffmpeg's beside it).
pub fn measure(input: &Path, start: f64, duration: f64, filter: Option<&str>) -> Result<Option<f64>> {
    const RATE: f64 = 48000.0;
    // A mono track is one channel to BS.1770 (and to ffmpeg's ebur128):
    // copied into both sides it would read 3 LU louder.
    let mono = crate::ffmpeg::probe(input, "a:0", "stream=channels")?.lines().next() == Some("1");
    let stereo = if mono { "pan=stereo|c0=c0" } else { "aformat=channel_layouts=stereo" };
    let chain = filter.map_or(stereo.to_owned(), |f| format!("{f},{stereo}"));
    let out = crate::ffmpeg::tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss", &start.to_string(), "-t", &duration.to_string(), "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-af", &chain, "-ar", &RATE.to_string(), "-f", "f32le", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("running ffmpeg")?;
    if !out.status.success() {
        bail!("ffmpeg couldn't read the sound of {}", input.display());
    }
    let frames: Vec<[f32; 2]> = out.stdout.as_chunks::<8>().0.iter().map(|b| [f32::from_le_bytes([b[0], b[1], b[2], b[3]]), f32::from_le_bytes([b[4], b[5], b[6], b[7]])]).collect();
    let mut meter = crate::lufs::Loudness::default();
    meter.feed(RATE, &frames);
    Ok(meter.integrated().filter(|&lufs| lufs > SILENT_LUFS))
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

/// Loudness targets for a whole film, LUFS integrated, by where it goes
/// (gap 46a). Sources disagree on the social ones; these are the most
/// cited (`EDITOR-GAPS.md` gap 45). Each turns down what's louder;
/// YouTube doesn't turn up what's quieter.
pub const TARGETS: [(&str, f64); 4] = [("YouTube, Spotify, TikTok, Instagram", -14.0), ("Apple, podcasts", -16.0), ("Broadcast (EBU R128)", -23.0), ("Louder (social, −11)", -11.0)];

/// Brings exported film `path`'s sound to `target` LUFS integrated: it's
/// measured, turned up or down by the difference and held under the
/// export's limiter again (peaks under −1.2 dBFS), the picture copied as
/// it is. Returns the loudness before and the gain, or `None` when it was
/// silent or already within 0.1 LU.
pub fn bring_to(path: &Path, target: f64, format: crate::ffmpeg::Format) -> Result<Option<(f64, f64)>> {
    let length = crate::ffmpeg::duration(path)?;
    let Some(before) = measure(path, 0.0, length, None)? else { return Ok(None) };
    let gain = target - before;
    if gain.abs() < 0.1 {
        return Ok(None);
    }
    let ext = path.extension().map_or_else(String::new, |e| e.to_string_lossy().into_owned());
    let tmp = path.with_extension(format!("loudness.{ext}"));
    let ok = crate::ffmpeg::tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(path)
        .args(["-map", "0", "-c", "copy"])
        .args(format.audio_codec())
        .args(["-af", &format!("volume={gain}dB,{}", crate::ffmpeg::LIMITER)])
        .arg(&tmp)
        .status()
        .context("running ffmpeg")?
        .success();
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        bail!("ffmpeg couldn't set the loudness of {}", path.display());
    }
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(Some((before, gain)))
}

/// `omnix normalize`: measures each cut (all, or those numbered in
/// `only`, from 1), or every clip on `track`, as it will sound (evened out
/// first if `even`), and applies what it measured as one step of ops
/// (`SoundOp::Normalized`, as the player's dialog does), printing a line
/// each; writes the project to `output` if given. One that can't be read
/// is left as it was and the rest go on.
pub fn normalize(project: &std::path::Path, only: &[crate::project::Id], track: Option<crate::project::SoundTrack>, even: bool, output: Option<&std::path::Path>) -> Result<()> {
    use crate::model::ops::{Op, SoundRef, sound::SoundOp};
    use crate::project::Own;
    let p = crate::project::Project::load(project)?;
    let owns: Vec<Own> = match track {
        Some(w) => (0..p.track(w).clips.len()).map(|i| Own::Clip(w, i)).collect(),
        None => p.cuts_by_id(only)?.into_iter().map(Own::Cut).collect(),
    };
    let mut ops = Vec::new();
    for own in owns {
        let (Some(sound), Some((file, range, filter))) = (SoundRef::of(own, &p), p.own_file(own, even)) else { continue };
        let name = file.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        match measure_stretch(&file, range, filter.as_deref()) {
            Ok(measured) => {
                match measured {
                    Some((lufs, gain)) => println!("{sound:<14} {name}  {lufs:.1} LUFS  {gain:+.1} dB"),
                    None => println!("{sound:<14} {name}  silent, level kept"),
                }
                ops.push(Op::Sound(SoundOp::Normalized { sound, lufs: measured.map(|(lufs, _)| lufs), even }));
            }
            Err(e) => println!("{sound:<14} {name}  couldn't measure, left as it is: {e:#}"),
        }
    }
    let mut edit = crate::model::edit::Edit::default();
    edit.replace(p);
    edit.batch(&ops).map_err(|(n, e)| anyhow::anyhow!("op {} of {}: {e}", n + 1, ops.len()))?;
    if let Some(out) = output {
        edit.save(out)?;
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
        let dir = std::env::temp_dir().join(format!("omnix-loud-{}", std::process::id()));
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
        use crate::model::level::{TARGET_LUFS, gain_to_target};
        assert!((gain_to_target(a) - (TARGET_LUFS - a)).abs() < 0.06);
        // Just a stretch of it measures the same, and silence is none.
        assert!((measure(&quiet, 2.0, 3.0, None).unwrap().unwrap() - a).abs() < 0.3);
        assert_eq!(measure(&silent, 0.0, 3.0, None).unwrap(), None);
        // The leveler lifts a steady quiet tone towards its peak.
        assert!(measure(&quiet, 0.0, 6.0, Some(LEVELER)).unwrap().unwrap() > a + 6.0);
        // As ffmpeg's ebur128 reads them, a mono and a stereo recording:
        // stored levels don't move for the change of meter.
        let theirs = |path: &Path| {
            let out = crate::ffmpeg::tool("ffmpeg").args(["-hide_banner", "-nostdin", "-i"]).arg(path).args(["-af", "ebur128=framelog=quiet", "-f", "null", "-"]).output().unwrap();
            String::from_utf8_lossy(&out.stderr).lines().rev().find_map(|l| l.trim().strip_prefix("I:").and_then(|v| v.trim().trim_end_matches("LUFS").trim().parse::<f64>().ok())).unwrap()
        };
        let stereo = make("stereo.wav", "anoisesrc=d=6:c=pink:a=0.1,aformat=channel_layouts=stereo");
        for f in [&quiet, &stereo] {
            let (ours, ff) = (measure(f, 0.0, 6.0, None).unwrap().unwrap(), theirs(f));
            assert!((ours - ff).abs() < 0.3, "{}: ours {ours:.2}, ffmpeg {ff:.2}", f.display());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
