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
    let out = Command::new(env!("CARGO_BIN_EXE_omnix")).current_dir(dir).arg("--no-cache").args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "omnix {args:?} failed:\n{text}");
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

#[test]
fn mixcheck_finds_music_too_loud_under_the_voice() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("mixcheck");
    clip(&d, "a.mp4", 6.0, 440);
    // A voice-over and a song as loud as it.
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=660:d=3:sample_rate=48000,volume=-6dB", "take.wav"]);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=220:d=6:sample_rate=48000,volume=-6dB", "song.wav"]);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 6, "mute": true}],
        "voice": {"clips": [{"input": "take.wav", "at": 1, "in": 0, "out": 3}]},
        "music": {"clips": [{"input": "song.wav", "at": 0, "in": 0, "out": 6}]}}"#).unwrap();
    let said = omnix(&d, &["mixcheck", "edit.json"]);
    // Quieted 20 dB, the song sits well under the voice: nothing about it.
    std::fs::write(d.join("quiet.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 6, "mute": true}],
        "voice": {"clips": [{"input": "take.wav", "at": 1, "in": 0, "out": 3}]},
        "music": {"clips": [{"input": "song.wav", "at": 0, "in": 0, "out": 6, "gain": -20}]}}"#).unwrap();
    let quiet = omnix(&d, &["mixcheck", "quiet.json"]);
    std::fs::remove_dir_all(&d).unwrap();
    assert!(said.contains("Music only") && said.contains("1.0-4.0 s"), "{said}");
    assert!(said.contains("Voice") && said.contains("speech"), "{said}");
    assert!(!quiet.contains("Music only"), "{quiet}");
}

#[test]
fn ops_from_a_file_duck_the_music_and_twice_is_once() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("apply");
    clip(&d, "a.mp4", 6.0, 440);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=660:d=3:sample_rate=48000,volume=-6dB", "take.wav"]);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=220:d=6:sample_rate=48000,volume=-6dB", "song.wav"]);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 6, "mute": true}],
        "voice": {"clips": [{"input": "take.wav", "at": 1, "in": 0, "out": 3}]},
        "music": {"clips": [{"input": "song.wav", "at": 0, "in": 0, "out": 6}]}}"#).unwrap();
    // Ids are given on load: the cut, then the song, then the take.
    let held = omnix(&d, &["describe", "edit.json"]);
    let duck = |depth: u32| format!(r#"[{{"area": "sound", "op": "duck", "depth": {depth}, "speech": [[1, 4]]}}]"#);
    std::fs::write(d.join("20.json"), duck(20)).unwrap();
    std::fs::write(d.join("18.json"), duck(18)).unwrap();
    // 20, then 18, against 18 alone (user, 2026-11-02: from the original).
    omnix(&d, &["apply", "edit.json", "20.json", "-o", "a.json"]);
    let said = omnix(&d, &["apply", "a.json", "18.json", "-o", "b.json"]);
    omnix(&d, &["apply", "edit.json", "18.json", "-o", "c.json"]);
    let twice = omnix(&d, &["mixcheck", "b.json"]);
    let once = omnix(&d, &["mixcheck", "c.json"]);
    // An op on what isn't there is refused, and nothing is written.
    std::fs::write(d.join("bad.json"), r#"[{"area": "sound", "op": "mute", "sound": {"cut": 42}, "on": true}]"#).unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_omnix")).current_dir(&d).args(["apply", "edit.json", "bad.json", "-o", "d.json"]).output().unwrap();
    let wrote_bad = d.join("d.json").exists();
    std::fs::remove_dir_all(&d).unwrap();
    assert!(held.contains("cut #1") && held.contains("clip #2") && held.contains("clip #3"), "{held}");
    assert!(said.contains("music 18 dB down under 1 stretches"), "{said}");
    let speech = |s: &str| s.lines().find(|l| l.contains("speech to")).map(str::to_owned);
    assert_eq!(speech(&twice), speech(&once), "{twice}\n{once}");
    assert!(!twice.contains("Music only"), "{twice}");
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("no cut #42") && !wrote_bad);
}

#[test]
fn normalize_a_sound_track_through_ops() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("normalize");
    clip(&d, "a.mp4", 4.0, 440);
    // A song: normalizing gives it a level towards -16 LUFS.
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=220:d=4:sample_rate=48000,volume=-3dB", "song.wav"]);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 4}],
        "music": {"clips": [{"input": "song.wav", "at": 0, "in": 0, "out": 4}]}}"#).unwrap();
    let said = omnix(&d, &["normalize", "--track", "music", "edit.json", "-o", "n.json"]);
    let held = omnix(&d, &["describe", "n.json"]);
    // Applied again on the result, it's where it was: from the measure, not the level.
    omnix(&d, &["normalize", "--track", "music", "n.json", "-o", "n2.json"]);
    let again = omnix(&d, &["describe", "n2.json"]);
    std::fs::remove_dir_all(&d).unwrap();
    assert!(said.contains("sound clip #2") && said.contains("LUFS"), "{said}");
    let level = |s: &str| s.lines().find(|l| l.contains("clip #2")).and_then(|l| l.split("level ").nth(1)).map(str::to_owned).unwrap();
    assert!(!level(&held).starts_with("+0.0"), "levelled: {held}");
    assert_eq!(level(&held), level(&again));
    assert!(held.contains("cut #1") && held.contains("level +0.0 dB"), "the cut left alone: {held}");
}

#[test]
fn ops_cut_and_rearrange_the_edit_and_the_export_follows() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("cutops");
    clip(&d, "a.mp4", 4.0, 440);
    clip(&d, "b.mp4", 3.0, 660);
    ffmpeg(&d, &["-f", "lavfi", "-i", "sine=f=880:d=0.5:sample_rate=48000", "bell.wav"]);
    // Cut #1 a (4 s), cut #2 b (3 s).
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 4}, {"input": "b.mp4", "in": 0, "out": 3}]}"#).unwrap();
    // Split a at 1.5 s (its second half is #3), put b first, drop a's
    // first half, trim b to 2 s, crossfade a's half in over 0.5 s, and a
    // bell over SFX twice: the second lands on SFX 2.
    std::fs::write(d.join("ops.json"), r#"[
        {"area": "cut", "op": "split", "cut": 1, "t": 1.5},
        {"area": "cut", "op": "move", "cut": 2, "before": 1},
        {"area": "cut", "op": "delete", "cut": 1},
        {"area": "cut", "op": "trim", "cut": 2, "end": "out", "t": 2, "most": 3},
        {"area": "cut", "op": "crossfade", "cut": 3, "seconds": 0.5},
        {"area": "clip", "op": "add", "track": "sfx", "input": "bell.wav", "at": 1, "start": 0, "end": 0.5},
        {"area": "clip", "op": "add", "track": "sfx", "input": "bell.wav", "at": 1.2, "start": 0, "end": 0.5}
    ]"#).unwrap();
    let said = omnix(&d, &["apply", "edit.json", "ops.json", "-o", "out.json"]);
    let held = omnix(&d, &["describe", "out.json"]);
    // b 2 s, then a 1.5..4 (2.5 s) overlapping 0.5 s: 4 s, 120 frames.
    omnix(&d, &["export", "out.json", "--cpu", "-o", "out.mp4", "--size", "320x180"]);
    let frames: u32 = probe(&d, "out.mp4", "v", "nb_read_frames").parse().unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    assert!(said.contains("the second half is cut #3"), "{said}");
    let cuts: Vec<&str> = held.lines().filter(|l| l.contains("cut #")).collect();
    assert!(cuts.len() == 2 && cuts[0].contains("cut #2") && cuts[0].contains("b.mp4 0.00-2.00") && cuts[1].contains("cut #3") && cuts[1].contains("a.mp4 1.50-4.00"), "{held}");
    assert!(held.contains("SFX:") && held.contains("SFX 2:"), "{held}");
    assert_eq!(frames, 120);
}

#[test]
fn a_blank_cut_goes_in_at_a_join_and_exports_black() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("blank");
    clip(&d, "a.mp4", 2.0, 440);
    clip(&d, "b.mp4", 2.0, 660);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 2}, {"input": "b.mp4", "in": 0, "out": 2}]}"#).unwrap();
    let path = omnix(&d, &["blank", "08_graphics"]);
    assert!(path.trim().ends_with("Blank.png"), "{path}");
    // A second blank second at the join (2 s): a, blank, b.
    std::fs::write(d.join("ops.json"), r#"[{"area": "cut", "op": "add", "input": "08_graphics/Blank.png", "start": 0, "end": 1, "place": {"at": 2}}]"#).unwrap();
    omnix(&d, &["apply", "edit.json", "ops.json", "-o", "out.json"]);
    let held = omnix(&d, &["describe", "out.json"]);
    omnix(&d, &["export", "out.json", "--cpu", "-o", "out.mp4", "--size", "320x180"]);
    let frames: u32 = probe(&d, "out.mp4", "v", "nb_read_frames").parse().unwrap();
    // The middle of the blank second, as luma: black is video range's 16.
    ffmpeg(&d, &["-ss", "2.5", "-i", "out.mp4", "-frames:v", "1", "-vf", "format=gray", "-f", "rawvideo", "mid.gray"]);
    let mid = std::fs::read(d.join("mid.gray")).unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    let cuts: Vec<&str> = held.lines().filter(|l| l.contains("cut #")).collect();
    assert!(cuts.len() == 3 && cuts[1].contains("Blank.png") && cuts[1].contains("2.00-   3.00"), "{held}");
    assert_eq!(frames, 150);
    let most = mid.iter().copied().max().unwrap();
    assert!(most <= 20, "the blank shows black, brightest {most}");
}

/// `file`'s sound from `from` to `to` seconds, RMS dBFS.
fn rms(dir: &Path, file: &str, from: f64, to: f64) -> f64 {
    let filter = format!("atrim={from}:{to},astats=measure_perchannel=none:measure_overall=RMS_level");
    let out = Command::new("ffmpeg").current_dir(dir).args(["-hide_banner", "-nostdin", "-i", file, "-af", &filter, "-vn", "-f", "null", "-"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    text.lines().find_map(|l| l.split("RMS level dB:").nth(1)?.trim().parse().ok()).unwrap_or(-120.0)
}

#[test]
fn a_cuts_sound_runs_under_the_next_picture_or_stops_short() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("jl");
    // a: 3 s of tone; b: 2 s of silence. The edit: a's first 2 s, then b.
    clip(&d, "a.mp4", 3.0, 440);
    ffmpeg(&d, &[
        "-f", "lavfi", "-i", "testsrc2=s=320x180:r=30:d=2", "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
        "-t", "2", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "b.mp4",
    ]);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 2}, {"input": "b.mp4", "in": 0, "out": 2}]}"#).unwrap();
    // An L-cut: a's sound a second on, under b's picture.
    std::fs::write(d.join("l.json"), r#"[{"area": "cut", "op": "sound_edge", "cut": 1, "end": "out", "t": 3, "most": 3}]"#).unwrap();
    omnix(&d, &["apply", "edit.json", "l.json", "-o", "l-edit.json"]);
    omnix(&d, &["export", "l-edit.json", "--cpu", "-o", "l.mp4", "--size", "320x180"]);
    // Stopping short instead: a silent after 1 s, its picture going on.
    std::fs::write(d.join("short.json"), r#"[{"area": "cut", "op": "sound_edge", "cut": 1, "end": "out", "t": 1, "most": 3}]"#).unwrap();
    omnix(&d, &["apply", "edit.json", "short.json", "-o", "short-edit.json"]);
    omnix(&d, &["export", "short-edit.json", "--cpu", "-o", "short.mp4", "--size", "320x180"]);
    omnix(&d, &["export", "edit.json", "--cpu", "-o", "plain.mp4", "--size", "320x180"]);
    let tone = rms(&d, "plain.mp4", 0.2, 1.8);
    let (plain_b, l_b, l_later) = (rms(&d, "plain.mp4", 2.2, 2.8), rms(&d, "l.mp4", 2.2, 2.8), rms(&d, "l.mp4", 3.2, 3.8));
    let (short_a, short_after) = (rms(&d, "short.mp4", 0.2, 0.8), rms(&d, "short.mp4", 1.2, 1.8));
    let frames: u32 = probe(&d, "l.mp4", "v", "nb_read_frames").parse().unwrap();
    std::fs::remove_dir_all(&d).unwrap();
    // The picture keeps its joins: 4 s either way.
    assert_eq!(frames, 120);
    assert!(tone > -30.0, "a's tone {tone:.1} dB");
    // Under b's first second: a's tone with the L-cut, silence without it
    // or after it.
    assert!(plain_b < -60.0 && l_later < -60.0, "plain {plain_b:.1}, after the L {l_later:.1}");
    assert!((l_b - tone).abs() < 1.5, "L {l_b:.1} against the tone {tone:.1}");
    assert!((short_a - tone).abs() < 1.5 && short_after < -60.0, "short: {short_a:.1} then {short_after:.1}");
}

/// The mean of a PNG's bytes, 0..255.
fn mean(dir: &Path, file: &str) -> f64 {
    let img = image::open(dir.join(file)).unwrap().to_rgb8();
    img.as_raw().iter().map(|&v| v as f64).sum::<f64>() / img.as_raw().len() as f64
}

#[test]
fn a_frame_of_the_edit_is_graded_and_faded_as_the_export_makes_it() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("frame");
    clip(&d, "a.mp4", 4.0, 440);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 4}], "fades": {"in": 1, "out": 0}}"#).unwrap();
    std::fs::write(d.join("dark.json"), r#"[{"area": "key", "op": "effect_value", "on": {"cut": 1}, "param": "exposure", "value": -3}]"#).unwrap();
    omnix(&d, &["apply", "edit.json", "dark.json", "-o", "dark.edit.json"]);
    omnix(&d, &["frame", "edit.json", "--at", "0", "--width", "320", "-o", "start.png"]);
    omnix(&d, &["frame", "edit.json", "--at", "2", "--width", "320", "-o", "plain.png"]);
    omnix(&d, &["frame", "dark.edit.json", "--at", "2", "--width", "320", "-o", "dark.png"]);
    let (start, plain, dark) = (mean(&d, "start.png"), mean(&d, "plain.png"), mean(&d, "dark.png"));
    std::fs::remove_dir_all(&d).unwrap();
    assert!(start < 2.0, "faded in from black: {start}");
    assert!(plain > 40.0 && dark < plain / 2.0, "graded: {dark} against {plain}");
}

#[test]
fn mixcheck_says_the_films_loudness_over_time() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("mixevery");
    clip(&d, "a.mp4", 3.0, 440);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 3}]}"#).unwrap();
    let said = omnix(&d, &["mixcheck", "edit.json", "--every", "1"]);
    std::fs::remove_dir_all(&d).unwrap();
    let lines: Vec<&str> = said.lines().filter(|l| l.contains(" s  film ")).collect();
    assert_eq!(lines.len(), 3, "{said}");
    assert!(lines.iter().all(|l| l.contains("LUFS") && !l.contains("silent")), "{said}");
}

#[test]
fn a_measuring_command_prints_the_ops_it_would_apply() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("printops");
    clip(&d, "a.mp4", 3.0, 440);
    std::fs::write(d.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 3}]}"#).unwrap();
    // On stdout the ops alone; what it measured on stderr.
    let out = Command::new(env!("CARGO_BIN_EXE_omnix")).current_dir(&d).args(["--no-cache", "normalize", "edit.json", "--ops"]).output().unwrap();
    let ops: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is an ops file");
    std::fs::write(d.join("ops.json"), serde_json::to_string(&ops).unwrap()).unwrap();
    omnix(&d, &["apply", "edit.json", "ops.json", "-o", "applied.json"]);
    omnix(&d, &["normalize", "edit.json", "-o", "direct.json"]);
    let (applied, direct) = (std::fs::read_to_string(d.join("applied.json")).unwrap(), std::fs::read_to_string(d.join("direct.json")).unwrap());
    let untouched = !d.join("edit.json.bak").exists() && std::fs::read_to_string(d.join("edit.json")).unwrap().len() < 80;
    std::fs::remove_dir_all(&d).unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("LUFS") && ops[0]["op"] == "normalized", "{ops}");
    assert_eq!(applied, direct);
    assert!(untouched, "--ops writes nothing");
}

/// PSNR (dB) of the frame at `a_t` in `a` against the one at `b_t` in `b`:
/// over 30 the same picture through the encoder, far under it another.
fn psnr(dir: &Path, (a, a_t): (&str, f64), (b, b_t): (&str, f64)) -> f64 {
    for (file, t, png) in [(a, a_t, "a.png"), (b, b_t, "b.png")] {
        ffmpeg(dir, &["-ss", &t.to_string(), "-i", file, "-frames:v", "1", "-s", "320x180", png]);
    }
    let out = Command::new("ffmpeg").current_dir(dir).args(["-hide_banner", "-nostdin", "-i", "a.png", "-i", "b.png", "-lavfi", "psnr", "-f", "null", "-"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr);
    text.split("average:").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or_else(|| panic!("no PSNR in: {text}"))
}

/// Gap 31: a cut at twice the speed exports half its clip's frames and
/// sound, each frame its clip's at twice the time; at half the speed,
/// twice as many; its sound's pitch kept or, like tape, shifted.
#[test]
fn exports_cuts_at_speed() {
    if !has_ffmpeg() {
        return;
    }
    let d = dir("speed");
    clip(&d, "a.mp4", 5.0, 440);
    std::fs::write(d.join("fast.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 4, "speed": {"keys": [{"t": 0, "rate": 2}]}}]}"#).unwrap();
    std::fs::write(d.join("slow.json"), r#"{"cuts": [{"input": "a.mp4", "in": 1, "out": 2, "speed": {"keys": [{"t": 0, "rate": 0.5}]}}, {"input": "a.mp4", "in": 3, "out": 4}]}"#).unwrap();
    omnix(&d, &["export", "fast.json", "--cpu", "-o", "fast.mp4", "--size", "320x180"]);
    omnix(&d, &["export", "slow.json", "--cpu", "-o", "slow.mp4", "--size", "320x180"]);
    let fast: (u32, f64) = (probe(&d, "fast.mp4", "v", "nb_read_frames").parse().unwrap(), probe(&d, "fast.mp4", "a", "duration").parse().unwrap());
    let slow: (u32, f64) = (probe(&d, "slow.mp4", "v", "nb_read_frames").parse().unwrap(), probe(&d, "slow.mp4", "a", "duration").parse().unwrap());
    // At 1 s into the fast export, its clip's second 2, not 1.
    let (same, other) = (psnr(&d, ("fast.mp4", 1.0), ("a.mp4", 2.0)), psnr(&d, ("fast.mp4", 1.0), ("a.mp4", 1.0)));
    // At 1 s into the slow one, its clip's 1.5 s; the cut after it still in step.
    let (slow_same, after) = (psnr(&d, ("slow.mp4", 1.0), ("a.mp4", 1.5)), psnr(&d, ("slow.mp4", 2.5), ("a.mp4", 3.5)));
    std::fs::remove_dir_all(&d).unwrap();
    assert_eq!(fast.0, 60);
    assert!((fast.1 - 2.0).abs() < 1.0 / 30.0, "fast sound {}", fast.1);
    assert!(same > 30.0 && other < same - 6.0, "fast picture: {same} dB at clip 2 s, {other} dB at 1 s");
    assert_eq!(slow.0, 90);
    assert!((slow.1 - 3.0).abs() < 1.0 / 30.0, "slow sound {}", slow.1);
    assert!(slow_same > 30.0 && after > 30.0, "slow picture {slow_same} dB, the cut after {after} dB");
}
