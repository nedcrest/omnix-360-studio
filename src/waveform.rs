//! A sound file's waveform: the loudest sample in each short step, for the
//! timeline's lanes, the contact sheet's music strip and finding speech
//! (`duck`). Kept in the clip cache.

use anyhow::Result;
use std::path::Path;

/// Seconds per waveform peak.
pub const WAVE_STEP: f64 = 0.05;

/// A song's waveform: the loudest sample in each `WAVE_STEP`, 0..1, and the
/// song's length.
#[derive(Debug)]
pub struct Waveform {
    pub peaks: Vec<f32>,
    pub duration: f64,
}

impl Waveform {
    /// From the clip cache, else decoded from `path` to mono at 8 kHz, which
    /// is plenty for a picture of its loudness: a 6-minute song takes about
    /// a second. `even`: as the leveler gives it (`loudness::LEVELER`), so an
    /// evened-out cut is drawn as it's heard.
    pub fn read(path: &Path, even: bool) -> Result<Waveform> {
        let kind = if even { "wave-even" } else { "wave" };
        if let Some((peaks, duration)) = crate::cache::waveform(path, kind, WAVE_STEP) {
            return Ok(Waveform { peaks, duration });
        }
        const RATE: usize = 8000;
        let samples: Vec<f32> = crate::ffmpeg::mono(path, RATE as u32, even.then_some(crate::loudness::LEVELER))?.into_iter().map(f32::abs).collect();
        let per = (WAVE_STEP * RATE as f64) as usize;
        if samples.is_empty() {
            anyhow::bail!("{} has no sound", path.display());
        }
        let peaks: Vec<f32> = samples.chunks(per).map(|c| c.iter().fold(0.0f32, |m, &v| m.max(v)).min(1.0)).collect();
        let duration = samples.len() as f64 / RATE as f64;
        crate::cache::store_waveform(path, kind, WAVE_STEP, &peaks, duration);
        Ok(Waveform { peaks, duration })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_evened_waveform_is_read_through_the_leveler() {
        crate::cache::disable();
        let dir = std::env::temp_dir().join(format!("omnix-wave-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let quiet = dir.join("quiet.wav");
        let ok = crate::ffmpeg::tool("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=f=440:d=6,volume=-30dB", "-ar", "48000"]).arg(&quiet).status().unwrap().success();
        assert!(ok);
        let (plain, even) = (Waveform::read(&quiet, false).unwrap(), Waveform::read(&quiet, true).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
        let middle = |w: &Waveform| w.peaks[w.peaks.len() / 2];
        // −30 dB is 0.03 of full scale; the leveler lifts a steady tone
        // well up (as `loudness`'s test measures it).
        assert!(middle(&plain) < 0.04, "{}", middle(&plain));
        assert!(middle(&even) > 2.0 * middle(&plain), "{} vs {}", middle(&even), middle(&plain));
    }
}
