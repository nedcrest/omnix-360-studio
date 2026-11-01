//! The binary end to end, on clips ffmpeg makes for the test: tiny
//! (320×180, 30 fps, a few seconds), in a folder per test, never real
//! footage. CPU export, no cache, so they run anywhere ffmpeg does; each
//! takes a few seconds. Skipped, saying so, where ffmpeg isn't installed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A fresh folder for test `name`.
fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnix-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn has_ffmpeg() -> bool {
    let ok = Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("ffmpeg isn't installed: skipped");
    }
    ok
}

fn ffmpeg(dir: &Path, args: &[&str]) {
    let ok = Command::new("ffmpeg").current_dir(dir).args(["-v", "error", "-nostdin", "-y"]).args(args).status().unwrap().success();
    assert!(ok, "ffmpeg {args:?}");
}

/// A `secs` long 320×180 30 fps clip with a tone at `hz`, as `name`.
fn clip(dir: &Path, name: &str, secs: f64, hz: u32) {
    ffmpeg(dir, &[
        "-f", "lavfi", "-i", &format!("testsrc2=s=320x180:r=30:d={secs}"),
        "-f", "lavfi", "-i", &format!("sine=f={hz}:d={secs}:sample_rate=48000"),
        "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", name,
    ]);
}

/// Runs the binary in `dir`, panicking with its output if it fails.
fn omnix(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rosmo360")).current_dir(dir).arg("--no-cache").args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "rosmo360 {args:?} failed:\n{text}");
    text
}

/// ffprobe's value of `entries` for `file`'s first stream of `kind`.
fn probe(dir: &Path, file: &str, kind: &str, entries: &str) -> String {
    let out = Command::new("ffprobe")
        .current_dir(dir)
        .args(["-v", "error", "-select_streams", &format!("{kind}:0"), "-count_frames", "-show_entries", &format!("stream={entries}"), "-of", "csv=p=0", file])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// The loudest sample of `file`, dBFS.
fn peak(dir: &Path, file: &str) -> f64 {
    let out = Command::new("ffmpeg").current_dir(dir).args(["-hide_banner", "-nostdin", "-i", file, "-af", "astats=measure_perchannel=none:measure_overall=Peak_level", "-vn", "-f", "null", "-"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    text.lines().find_map(|l| l.split("Peak level dB:").nth(1)?.trim().parse().ok()).expect("a peak level")
}

#[test]
fn exports_a_small_edit_with_a_crossfade_and_music() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("export");
    clip(&d, "a.mp4", 3.0, 440);
    clip(&d, "b.mp4", 3.0, 660);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=220:d=5:sample_rate=48000", "music.wav"]);
    // 2 s + 2 s, 0.5 s of them crossfading: 3.5 s, 105 frames at 30 fps.
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0.5, "out": 2.5}, {"input": "b.mp4", "in": 0, "out": 2, "crossfade": 0.5, "gain": 12}],
        "music": {"volume": 6, "clips": [{"input": "music.wav", "at": 0, "in": 0, "out": 5, "fade_in": 0.5}]}}"#).unwrap();
    omnix(&d, &["export", "edit.json", "--cpu", "-o", "out.mp4", "--size", "320x180"]);
    let frames: u32 = probe(&d, "out.mp4", "v", "nb_read_frames").parse().unwrap();
    let length: f64 = probe(&d, "out.mp4", "a", "duration").parse().unwrap();
    // Raised 12 dB and the music 6 dB over a tone: the limiter holds the
    // peaks at −1.2 dBFS (a little over after AAC).
    let top = peak(&d, "out.mp4");
    std::fs::remove_dir_all(&d).unwrap();
    assert_eq!(frames, 105);
    assert!((length - 3.5).abs() < 1.0 / 30.0, "{length}");
    assert!(top < -0.5, "peak {top} dBFS");
}

#[test]
fn exports_a_file_per_cut_with_handles_and_the_music_apart() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("clips");
    clip(&d, "a.mp4", 4.0, 440);
    clip(&d, "b.mp4", 4.0, 660);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=220:d=5:sample_rate=48000", "music.wav"]);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 1, "out": 2}, {"input": "b.mp4", "in": 0, "out": 1}],
        "music": {"clips": [{"input": "music.wav", "at": 0, "in": 1, "out": 3}]}}"#).unwrap();
    std::fs::create_dir_all(d.join("out")).unwrap();
    omnix(&d, &["export", "edit.json", "--cpu", "--clips", "--handles", "0.5", "-o", "out", "--size", "320x180"]);
    let mut names: Vec<String> = std::fs::read_dir(d.join("out")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    // a: 1..2 with half a second either side; b: 0..1, nothing before 0.
    let a: f64 = probe(&d.join("out"), &names[0], "v", "duration").parse().unwrap();
    let b: f64 = probe(&d.join("out"), &names[1], "v", "duration").parse().unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names[2].contains("music") && names[2].ends_with(".wav"), "{names:?}");
    assert!((a - 2.0).abs() < 0.05, "{a}");
    assert!((b - 1.5).abs() < 0.05, "{b}");
}

#[test]
fn check_says_what_looks_like_a_mistake() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("check");
    clip(&d, "a.mp4", 4.0, 440);
    // A cut under a second, and one showing what the cut before showed.
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 2}, {"input": "a.mp4", "in": 1, "out": 1.5}]}"#).unwrap();
    let said = omnix(&d, &["check", "edit.json"]);
    std::fs::remove_dir_all(&d).unwrap();
    assert!(said.contains("short"), "{said}");
    assert!(said.contains("repeat"), "{said}");
}

/// The integrated loudness of `file`, LUFS.
fn loudness(dir: &Path, file: &str) -> f64 {
    let out = Command::new("ffmpeg").current_dir(dir).args(["-hide_banner", "-nostdin", "-i", file, "-af", "ebur128=framelog=quiet", "-vn", "-f", "null", "-"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    text.lines().filter_map(|l| l.trim().strip_prefix("I:")?.trim().strip_suffix("LUFS")?.trim().parse().ok()).next_back().expect("a loudness")
}

#[test]
fn exports_the_whole_film_at_a_loudness_target() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("loudness");
    clip(&d, "a.mp4", 3.0, 440);
    clip(&d, "b.mp4", 3.0, 660);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 3}, {"input": "b.mp4", "in": 0, "out": 3}]}"#).unwrap();
    omnix(&d, &["export", "edit.json", "--cpu", "-o", "plain.mp4", "--size", "320x180"]);
    let said = omnix(&d, &["export", "edit.json", "--cpu", "-o", "loud.mp4", "--size", "320x180", "--loudness", "-14"]);
    let (plain, loud) = (loudness(&d, "plain.mp4"), loudness(&d, "loud.mp4"));
    let frames: u32 = probe(&d, "loud.mp4", "v", "nb_read_frames").parse().unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    // The tones come out near −21 LUFS as mixed; brought to −14, the
    // picture untouched.
    assert!(plain < -17.0, "as mixed {plain}");
    assert!((loud + 14.0).abs() < 0.5, "brought to {loud}: {said}");
    assert_eq!(frames, 180);
}
