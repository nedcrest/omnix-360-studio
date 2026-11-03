//! Quick copies of ordinary videos for moving the playhead (user,
//! 2026-11-04: dragging it made the machine work hard). A phone's or a
//! camera's file often has a keyframe only every few seconds (the test
//! clips: 8 s), so each jump of a paused playhead had ffmpeg decode up to
//! 240 full frames from the keyframe before, a core at 100 %. A copy at
//! 960 px with a keyframe every 6 frames finds any frame in 0.05 s
//! against 0.43 s. Made once per file into the clip cache, on a thread when
//! the clip opens; paused frames come from it once it's there, and the full
//! frame from the file itself once the playhead rests, as a 360 clip's
//! proxy does.

use crate::ffmpeg::Flat;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// The copy's long side, pixels.
const LONG: u32 = 960;
/// A keyframe every so many frames: any frame is at most this many decodes
/// away.
const KEY_EVERY: u32 = 6;

/// The copies made so far, by the file they stand for.
static READY: LazyLock<Mutex<HashMap<PathBuf, PathBuf>>> = LazyLock::new(Default::default);

/// The copy of `input`, once it's made.
pub fn ready(input: &Path) -> Option<PathBuf> {
    READY.lock().ok()?.get(input).cloned()
}

/// The copy's size as stored (not turned upright): `flat`'s, long side
/// `LONG` at most, even.
fn size(flat: &Flat) -> (u32, u32) {
    super::playback::fit(flat.stored, LONG)
}

/// How the copy decodes: as its file does (turned, its colours as they
/// were), at its own size, its one video track.
pub fn flat(flat: &Flat) -> Flat {
    Flat { track: 0, stored: size(flat), ..*flat }
}

/// Makes `input`'s copy on a thread if the cache hasn't one (a picture
/// needs none), and registers it once there.
pub fn make(input: PathBuf, flat: Flat) {
    if flat.picture {
        return;
    }
    std::thread::spawn(move || {
        let Some(entry) = crate::cache::scrub(&input) else { return };
        if !entry.is_file()
            && let Err(e) = write(&input, &flat, &entry)
        {
            eprintln!("no quick copy of {} for the playhead: {e:#}", input.display());
            return;
        }
        if let Ok(mut ready) = READY.lock() {
            ready.insert(input, entry);
        }
    });
}

/// Encodes the copy beside `entry` and renames it there, so a half-written
/// one is never used: upright as stored (its turn is applied on decoding,
/// as the file's), no sound, H.264 at its fastest.
fn write(input: &Path, flat: &Flat, entry: &Path) -> anyhow::Result<()> {
    let part = entry.with_extension("scrub-part");
    let (w, h) = size(flat);
    let ok = crate::ffmpeg::tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y", "-noautorotate", "-i"])
        .arg(input)
        .args(["-map", &format!("0:v:{}", flat.track), "-an", "-sn", "-dn", "-vf", &format!("scale={w}:{h}")])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-crf", "26", "-g", &KEY_EVERY.to_string(), "-bf", "0", "-f", "mp4"])
        .arg(&part)
        .status()?
        .success();
    if !ok {
        let _ = std::fs::remove_file(&part);
        anyhow::bail!("ffmpeg failed");
    }
    std::fs::rename(&part, entry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ffmpeg::testmedia;
    use std::process::Command;

    /// ffprobe's answer for `file`'s video, csv without names.
    fn probe(file: &std::path::Path, args: &[&str]) -> String {
        let out = Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0"]).args(args).args(["-of", "csv=p=0"]).arg(file).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// A frame at `t` as a 16×9 grey picture, each pixel the mean of its
    /// area (a sampled one aliases the fine test pattern).
    fn look(file: &std::path::Path, t: f64) -> Vec<u8> {
        let out = Command::new("ffmpeg").args(["-v", "error", "-nostdin", "-ss", &t.to_string(), "-i"]).arg(file).args(["-frames:v", "1", "-vf", "scale=16:9:flags=area,format=gray", "-f", "rawvideo", "-"]).output().unwrap();
        out.stdout
    }

    #[test]
    fn the_copy_is_small_finds_any_frame_quickly_and_shows_the_same() {
        let dir = testmedia::dir("scrub");
        // A camera-like file: 1280×720, a keyframe only every 4 s.
        let input = dir.join("cam.mp4");
        testmedia::ffmpeg(&["-f", "lavfi", "-i", "testsrc2=s=1280x720:r=30:d=4", "-c:v", "libx264", "-g", "120", "-pix_fmt", "yuv420p", input.to_str().unwrap()]);
        let flat = crate::ffmpeg::flat(&input).unwrap().unwrap();
        let copy = dir.join("cam.scrub");
        super::write(&input, &flat, &copy).unwrap();
        let size = probe(&copy, &["-show_entries", "stream=width,height"]);
        let keys: Vec<f64> = probe(&copy, &["-skip_frame", "nokey", "-show_entries", "frame=pts_time"]).lines().filter_map(|l| l.trim_end_matches(',').parse().ok()).collect();
        let (a, b) = (look(&input, 2.5), look(&copy, 2.5));
        // And the player decodes it as it decodes the file: the frame asked
        // for, at the copy's size.
        let kind = super::super::source::Kind::Flat { path: copy.clone(), flat: super::flat(&flat), size: None, rate: None };
        let stream = super::super::source::spawn(kind, crate::ffmpeg::Hwaccel::Software, 2.5, 30.0).unwrap();
        let frame = stream.frames.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        drop(stream);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!((frame.pts - 2.5).abs() < 0.02, "{}", frame.pts);
        assert_eq!((frame.pictures[0].width, frame.pictures[0].height), (960, 540));
        assert_eq!(size, "960,540");
        assert_eq!(super::flat(&flat).stored, (960, 540));
        // A keyframe every 6 frames: 20 in 4 s.
        assert!(keys.len() >= 20 && keys.windows(2).all(|w| w[1] - w[0] < 0.21), "{keys:?}");
        // The same picture at the same time, near enough for a glance: on
        // average within a few levels (a frame early or late would be far
        // more on this moving pattern).
        let off = a.iter().zip(&b).map(|(x, y)| (*x as f64 - *y as f64).abs()).sum::<f64>() / a.len().max(1) as f64;
        assert!(a.len() == 144 && off < 3.0, "off by {off:.2} on average");
    }
}
