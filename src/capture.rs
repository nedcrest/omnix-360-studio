//! Recording sound from a microphone: the inputs the system has (by name
//! and kind: Bluetooth, USB, built-in, webcam), and a take written to a WAV
//! while its level is read live for a meter. For the voice-over recorder
//! (`player::record_ui`).
//!
//! Through PipeWire's Pulse interface and ffmpeg, as a separate program
//! like every ffmpeg use here: `pactl -f json list sources` lists the
//! inputs, `ffmpeg -f pulse -i <source>` records one. One ffmpeg both
//! writes the take and prints the level (`astats` metadata in its log on
//! stderr: as a file, `/dev/stderr` sat in a 32 KB buffer for minutes), so
//! the meter shows exactly what is recorded. Writing `q` to its stdin
//! stops it with the WAV finished; killed, the header's sizes would be
//! wrong.
//!
//! Checked on the user's machine (2026-10-29, PipeWire 1.0.5, ffmpeg
//! 6.1.1): a Scarlett Solo (USB, two channels, the mic on the first), a
//! webcam's mic (16 kHz mono), the built-in card's profile off.

use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What kind of device an input is, for its label and its warnings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bluetooth,
    Usb,
    Webcam,
    BuiltIn,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Bluetooth => "Bluetooth",
            Kind::Usb => "USB",
            Kind::Webcam => "Webcam",
            Kind::BuiltIn => "Built-in",
            Kind::Other => "Input",
        }
    }

    /// What to know before recording from one: a Bluetooth headset's mic
    /// switches it to its call profile (mono, 8 or 16 kHz), and what plays
    /// in it drops to that quality too while the mic is open; a webcam's
    /// mic is a small one, made for calls.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            Kind::Bluetooth => Some("Bluetooth mic: phone quality (16 kHz), and the headset's playback drops to it while recording. A USB mic sounds far better."),
            Kind::Webcam => Some("A webcam's mic: made for calls, it sounds thin."),
            _ => None,
        }
    }
}

/// A sound input: its source name (what ffmpeg opens), how people know it,
/// its kind, channels, and whether it's the system's default.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub name: String,
    pub description: String,
    pub kind: Kind,
    pub channels: u32,
    pub default: bool,
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} · {}", self.description, self.kind.label())
    }
}

/// The inputs to record from, the default first. Through PipeWire or
/// PulseAudio, so Linux only for now: Windows' and macOS' inputs would come
/// from ffmpeg's dshow and avfoundation devices.
#[cfg(not(target_os = "linux"))]
pub fn inputs() -> Result<Vec<Input>> {
    bail!("recording a voice-over works on Linux only so far")
}

/// The inputs to record from, the default first.
#[cfg(target_os = "linux")]
pub fn inputs() -> Result<Vec<Input>> {
    let list = std::process::Command::new("pactl").args(["-f", "json", "list", "sources"]).output().context("running pactl (PipeWire or PulseAudio)")?;
    if !list.status.success() {
        bail!("pactl couldn't list the sound inputs");
    }
    let default = std::process::Command::new("pactl").arg("get-default-source").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
    parse_sources(&String::from_utf8_lossy(&list.stdout), &default)
}

/// The most an input's volume goes: 200 % is PipeWire's cubic volume ×8,
/// +18 dB, enough to bring a webcam mic peaking at −12 dB up; above 100 %
/// it is digital and raises the noise as much as the voice.
pub const MOST_VOLUME: u32 = 200;

/// An input's own volume in the system, percent (what the system's sound
/// settings show).
#[cfg(target_os = "linux")]
pub fn input_volume(input: &Input) -> Result<u32> {
    let out = std::process::Command::new("pactl").args(["get-source-volume", &input.name]).output().context("running pactl")?;
    parse_volume(&String::from_utf8_lossy(&out.stdout)).context("reading pactl's volume")
}

/// Sets an input's own volume in the system, percent: the take and the
/// meter both get it, as from the system's sound settings.
#[cfg(target_os = "linux")]
pub fn set_input_volume(input: &Input, percent: u32) -> Result<()> {
    let ok = std::process::Command::new("pactl").args(["set-source-volume", &input.name, &format!("{}%", percent.min(MOST_VOLUME))]).status().context("running pactl")?.success();
    if !ok {
        bail!("pactl couldn't set {}'s volume", input.description);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn input_volume(_: &Input) -> Result<u32> {
    bail!("an input's volume is set on Linux only so far")
}

#[cfg(not(target_os = "linux"))]
pub fn set_input_volume(_: &Input, _: u32) -> Result<()> {
    bail!("an input's volume is set on Linux only so far")
}

/// The first channel's percent in `pactl get-source-volume`'s
/// "Volume: front-left: 65536 / 100% / 0.00 dB, …".
fn parse_volume(text: &str) -> Option<u32> {
    text.split('/').find_map(|part| part.trim().strip_suffix('%')?.trim().parse().ok())
}

/// `pactl -f json list sources` as inputs: real sources only (not the
/// monitors of outputs), the default first.
fn parse_sources(json: &str, default: &str) -> Result<Vec<Input>> {
    let all: Vec<serde_json::Value> = serde_json::from_str(json).context("reading pactl's list")?;
    let mut inputs: Vec<Input> = all
        .iter()
        .filter_map(|s| {
            let props = &s["properties"];
            let prop = |k: &str| props[k].as_str().unwrap_or("");
            if prop("media.class") != "Audio/Source" || prop("device.class") == "monitor" {
                return None;
            }
            let name = s["name"].as_str()?.to_owned();
            let kind = match (prop("device.bus"), prop("device.form_factor")) {
                ("bluetooth", _) => Kind::Bluetooth,
                (_, "webcam") => Kind::Webcam,
                ("usb", _) => Kind::Usb,
                ("pci", _) | (_, "internal") => Kind::BuiltIn,
                _ if name.starts_with("bluez_") => Kind::Bluetooth,
                _ => Kind::Other,
            };
            let channels = s["sample_specification"].as_str().and_then(|spec| spec.split_whitespace().find_map(|w| w.strip_suffix("ch")?.parse().ok())).unwrap_or(1);
            Some(Input { description: s["description"].as_str().unwrap_or(&name).to_owned(), default: name == default, name, kind, channels })
        })
        .collect();
    inputs.sort_by_key(|i| !i.default);
    Ok(inputs)
}

/// Which channel of a two-channel input to record: an interface's mic is
/// usually on the first alone, and averaging both would lose 6 dB and mix
/// in the other input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Channel {
    #[default]
    First,
    Second,
    Both,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::First, Channel::Second, Channel::Both];

    fn pan(self, channels: u32) -> &'static str {
        match (self, channels) {
            (_, 0 | 1) | (Channel::First, _) => "pan=mono|c0=c0",
            (Channel::Second, _) => "pan=mono|c0=c1",
            (Channel::Both, _) => "pan=mono|c0=0.5*c0+0.5*c1",
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Channel::First => "1",
            Channel::Second => "2",
            Channel::Both => "1 + 2",
        })
    }
}

/// The peak of the last 50 ms (`astats` reset every 5 frames of 10 ms),
/// dBFS, and whether anything has hit full scale since the meter was last
/// cleared. The peak alone: the meter draws nothing else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub peak: f64,
    pub clipped: bool,
}

impl Default for Level {
    fn default() -> Self {
        Level { peak: f64::NEG_INFINITY, clipped: false }
    }
}

/// Over this, dBFS, a take has clipped.
const CLIP_DB: f64 = -0.1;

/// An input being listened to: for the meter alone, or recorded to a file.
pub struct Recorder {
    child: Child,
    stdin: Option<ChildStdin>,
    level: Arc<Mutex<Level>>,
    /// When ffmpeg's first level arrived: the take's first sound, near
    /// enough (to a tenth of a second at most).
    first: Arc<Mutex<Option<Instant>>>,
    file: Option<PathBuf>,
    /// ffmpeg's last words that weren't levels: why it stopped, if it did.
    said: Arc<Mutex<Vec<String>>>,
}

impl Recorder {
    /// Starts listening to `input`'s `channel`; with `file`, recording it
    /// there as 48 kHz 24-bit mono WAV.
    pub fn start(input: &Input, channel: Channel, file: Option<&Path>) -> Result<Recorder> {
        let meter = "astats=metadata=1:reset=5:measure_perchannel=none:measure_overall=Peak_level,ametadata=mode=print";
        let pan = channel.pan(input.channels);
        let mut cmd = crate::ffmpeg::tool("ffmpeg");
        cmd.args(["-hide_banner", "-nostats", "-loglevel", "info", "-f", "pulse", "-sample_rate", "48000", "-channels", &input.channels.max(1).to_string(), "-fragment_size", "1920", "-i", &input.name]);
        match file {
            Some(f) => {
                cmd.args(["-filter_complex", &format!("[0:a]{pan},asplit=2[rec][m];[m]{meter}[mo]"), "-map", "[rec]", "-c:a", "pcm_s24le", "-ar", "48000", "-rf64", "auto", "-y"]).arg(f);
                cmd.args(["-map", "[mo]", "-f", "null", "-"]);
            }
            None => {
                cmd.args(["-filter_complex", &format!("[0:a]{pan},{meter}[mo]"), "-map", "[mo]", "-f", "null", "-"]);
            }
        }
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().context("running ffmpeg")?;
        let stdin = child.stdin.take();
        let (level, first) = (Arc::new(Mutex::new(Level::default())), Arc::new(Mutex::new(None)));
        let err = child.stderr.take().unwrap();
        let said = Arc::new(Mutex::new(Vec::new()));
        let (l, f, s) = (level.clone(), first.clone(), said.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let Some(db) = parse_peak(&line) else {
                    if !line.contains("frame:") {
                        let mut s = s.lock().unwrap();
                        s.push(line);
                        let n = s.len();
                        s.drain(..n.saturating_sub(4));
                    }
                    continue;
                };
                f.lock().unwrap().get_or_insert_with(Instant::now);
                let mut level = l.lock().unwrap();
                level.peak = db;
                level.clipped |= db >= CLIP_DB;
            }
        });
        Ok(Recorder { child, stdin, level, first, file: file.map(Path::to_owned), said })
    }

    pub fn level(&self) -> Level {
        *self.level.lock().unwrap()
    }

    /// Clears the clip light.
    pub fn clear_clip(&self) {
        self.level.lock().unwrap().clipped = false;
    }

    /// What ffmpeg said that wasn't a level, if it stopped: the reason.
    pub fn problem(&mut self) -> Option<String> {
        let ended = self.child.try_wait().ok().flatten().is_some();
        let said = self.said.lock().unwrap().join("; ");
        (ended && !said.is_empty()).then_some(said)
    }

    /// When the first sound came in, once it has.
    pub fn first_sound(&self) -> Option<Instant> {
        *self.first.lock().unwrap()
    }

    /// Stops it and returns the take's file (finished: ffmpeg was asked to
    /// quit, not killed, unless it didn't within two seconds).
    pub fn stop(mut self) -> Result<Option<PathBuf>> {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q");
        }
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            if self.child.try_wait()?.is_some() {
                return Ok(self.file.take());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(self.file.take())
    }
}

impl Drop for Recorder {
    /// A recorder let go (the panel closed) stops listening.
    fn drop(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q");
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The peak in one of ffmpeg's metadata lines,
/// `lavfi.astats.Overall.Peak_level=-12.3`, dBFS.
fn parse_peak(line: &str) -> Option<f64> {
    // The log prefixes each line with "[Parsed_ametadata_N @ 0x…] ".
    let line = line.rsplit_once("] ").map_or(line, |(_, l)| l);
    let value = line.trim().strip_prefix("lavfi.astats.Overall.Peak_level=")?;
    match value.trim() {
        "-inf" => Some(f64::NEG_INFINITY),
        v => v.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_inputs_volume() {
        assert_eq!(parse_volume("Volume: mono: 65536 / 100% / 0.00 dB\n        balance 0.00"), Some(100));
        assert_eq!(parse_volume("Volume: front-left: 98304 / 150% / 10.57 dB,   front-right: 98304 / 150% / 10.57 dB"), Some(150));
        assert_eq!(parse_volume("No such entity"), None);
    }

    #[test]
    fn lists_real_inputs_with_their_kinds_default_first() {
        // Shaped as this machine's pactl gave it (trimmed).
        let json = r#"[
          {"name":"alsa_output.usb-Focusrite.monitor","description":"Monitor of Scarlett Solo","sample_specification":"s32le 2ch 48000Hz",
           "properties":{"media.class":"Audio/Sink","device.class":"monitor","device.bus":"usb"}},
          {"name":"alsa_input.usb-Xiongmai_web_camera.mono-fallback","description":"web camera Mono","sample_specification":"s16le 1ch 16000Hz",
           "properties":{"media.class":"Audio/Source","device.bus":"usb","device.form_factor":"webcam"}},
          {"name":"alsa_input.usb-Focusrite_Scarlett_Solo.analog-stereo","description":"Scarlett Solo (3rd Gen.) Analog Stereo","sample_specification":"s32le 2ch 48000Hz",
           "properties":{"media.class":"Audio/Source","device.bus":"usb"}},
          {"name":"bluez_input.AA_BB","description":"Headset","sample_specification":"s16le 1ch 16000Hz",
           "properties":{"media.class":"Audio/Source","device.bus":"bluetooth"}}
        ]"#;
        let inputs = parse_sources(json, "alsa_input.usb-Focusrite_Scarlett_Solo.analog-stereo").unwrap();
        let got: Vec<_> = inputs.iter().map(|i| (i.kind, i.channels, i.default)).collect();
        assert_eq!(got, [(Kind::Usb, 2, true), (Kind::Webcam, 1, false), (Kind::Bluetooth, 1, false)]);
        assert_eq!(inputs[0].to_string(), "Scarlett Solo (3rd Gen.) Analog Stereo · USB");
        assert!(Kind::Bluetooth.warning().is_some() && Kind::Usb.warning().is_none());
    }

    /// Records two seconds from the default input: run by hand on a machine
    /// with one (`cargo test --release -- --ignored records_a_take`).
    #[test]
    #[ignore]
    fn records_a_take() {
        let input = inputs().unwrap().into_iter().next().expect("an input");
        let file = std::env::temp_dir().join(format!("omnix-take-{}.wav", std::process::id()));
        let rec = Recorder::start(&input, Channel::First, Some(&file)).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        let mut rec = rec;
        let level = rec.level();
        assert!(rec.first_sound().is_some(), "levels arrived: {:?}", rec.problem());
        let path = rec.stop().unwrap().unwrap();
        let len = crate::ffmpeg::duration(&path).unwrap();
        eprintln!("{}: {len:.2} s, peak {:.1} dBFS", input, level.peak);
        assert!(len > 1.5 && len < 2.5, "{len}");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_the_level_lines() {
        assert_eq!(parse_peak("lavfi.astats.Overall.Peak_level=-12.5"), Some(-12.5));
        assert_eq!(parse_peak("lavfi.astats.Overall.Peak_level=-inf"), Some(f64::NEG_INFINITY));
        assert_eq!(parse_peak("[Parsed_ametadata_2 @ 0x57a1] lavfi.astats.Overall.Peak_level=-90.3"), Some(-90.3));
        assert_eq!(parse_peak("frame:3    pts:3072    pts_time:0.064"), None);
        // A mono input records its one channel whatever is chosen.
        assert_eq!(Channel::Second.pan(1), "pan=mono|c0=c0");
        assert_eq!(Channel::Second.pan(2), "pan=mono|c0=c1");
    }
}
