//! A sound track in the player (the music, the sound effects: one `Player`
//! each): its files' waveforms for the timeline, and its sound kept in step
//! with the edit while it plays.
//!
//! A track has an audio stream of its own, apart from the clips' sound, so it
//! plays on through cut changes untouched. It follows edit time: started at
//! the song's place when playback reaches a music clip, restarted only after
//! a jump (or if it drifts), and set to its level every frame, which carries
//! the clip's fades and the fade-out where the video ends. The next clip is
//! opened `PRIME` seconds ahead and let through one device latency early, so
//! one song hands over to the next without the gap of starting ffmpeg and the
//! device; a clip that carries on in the same song just plays on.

use super::audio::Audio;
use crate::project::{self, MusicClip};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Seconds per waveform peak.
pub const WAVE_STEP: f64 = 0.05;
/// Restart the music when it is this far from where the edit is.
const DRIFT: f64 = 0.15;
/// Open the next clip's sound this long before it starts.
const PRIME: f64 = 1.0;

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

/// A clip's sound, open: its song, where the clip sits on the edit, from
/// where in the song the stream started (so it's at `from + elapsed` once
/// let through), whether it has been, and the filters it's decoded through
/// (`MusicClip::sound_filter`; a stream with others isn't reused).
struct Open {
    input: PathBuf,
    at: f64,
    from: f64,
    audio: Audio,
    started: bool,
    filter: Option<String>,
}

impl Open {
    fn is(&self, clip: &MusicClip) -> bool {
        self.input == clip.input && self.at == clip.at && self.from == clip.start && self.filter == clip.sound_filter()
    }

    /// `clip`'s sound from `from` in its file, evened out if it is.
    fn new(clip: &MusicClip, from: f64, volume: f32) -> Result<Open> {
        let filter = clip.sound_filter();
        let audio = Audio::prepare(&clip.input, from, volume, filter.as_deref())?;
        Ok(Open { input: clip.input.clone(), at: clip.at, from, audio, started: false, filter })
    }

    fn play(&mut self) {
        self.audio.play();
        self.started = true;
    }
}

/// The music's open streams: the clips playing now (two through a
/// crossfade), and the next one's, ready.
#[derive(Default)]
pub struct Player {
    open: Vec<Open>,
}

impl Player {
    /// Keeps the music in step with edit time `t` while the edit plays (stops
    /// it otherwise): the clips under `t`, each at the track's level there
    /// (with its volume points), its own level and points, times the clip's
    /// envelope.
    /// `heard`: 0 when the lane is muted or another soloed (`listen`).
    pub fn follow(&mut self, track: &project::Music, t: f64, video_end: f64, playing: bool, heard: f32) {
        if !playing {
            self.open.clear();
            return;
        }
        let level = |i: usize, t: f64| crate::mixdown::clip_gain(track, i, t, video_end) as f32 * heard;
        let mut old = std::mem::take(&mut self.open);
        let now: Vec<usize> = (0..track.clips.len()).filter(|&i| t >= track.clips[i].at && t < track.clips[i].ends() && t < video_end).collect();
        for &i in &now {
            let clip = &track.clips[i];
            let want = clip.start + (t - clip.at);
            // Playing and in step (a split song's second half plays on in its
            // first half's stream), else made ready for it, else started.
            let in_step = old.iter().position(|o| o.started && o.input == clip.input && o.filter == clip.sound_filter() && (o.from + o.audio.elapsed() - want).abs() < DRIFT);
            let ready = || old.iter().position(|o| !o.started && o.is(clip));
            let mut open = match in_step.or_else(ready) {
                Some(k) => old.swap_remove(k),
                None => match Open::new(clip, want, 0.0) {
                    Ok(open) => open,
                    Err(e) => {
                        eprintln!("no music: {e:#}");
                        continue;
                    }
                },
            };
            if !open.started {
                open.play();
            }
            open.audio.set_volume(level(i, t));
            self.open.push(open);
        }
        // The clips starting within `PRIME`, opened ahead and let through a
        // device latency early so they're heard on time; not one the song
        // playing runs straight on into.
        for (i, next) in track.clips.iter().enumerate() {
            if next.at <= t || next.at >= t + PRIME || next.at >= video_end {
                continue;
            }
            let carries_on = now.iter().any(|&j| {
                let c = &track.clips[j];
                c.input == next.input && (c.ends() - next.at).abs() < 1e-3 && (c.end - next.start).abs() < 1e-3
            });
            if carries_on {
                continue;
            }
            let mut open = match old.iter().position(|o| o.is(next)) {
                Some(k) => old.swap_remove(k),
                None => match Open::new(next, next.start, level(i, next.at)) {
                    Ok(open) => open,
                    Err(e) => {
                        eprintln!("no music: {e:#}");
                        continue;
                    }
                },
            };
            if !open.started && t >= next.at - open.audio.latency().min(0.25) {
                open.audio.set_volume(level(i, next.at));
                open.play();
            }
            self.open.push(open);
        }
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
