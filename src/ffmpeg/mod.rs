//! Video and sound through the ffmpeg CLI, always as a separate program
//! (never linked: see AGENTS.md): finding the tools, probing files, the
//! decode commands (hardware decoders tried in turn), the encoder, and the
//! export's sound mix (`mix`).

use crate::stitch::Frame;
use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use crate::hdr::Hdr;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

mod mix;
pub use mix::{Mix, Sound};
use mix::{mixed, pad};

/// A command running ffmpeg's `tool` (`ffmpeg` or `ffprobe`): one bundled
/// beside our executable first (in `ffmpeg/` there, or right beside it, as a
/// Windows zip or a Mac build carries them), else the one on the PATH. On
/// macOS also Homebrew's, since an app started from the Finder doesn't get
/// the shell's PATH.
pub fn tool(tool: &str) -> Command {
    static FOUND: std::sync::OnceLock<Vec<(String, PathBuf)>> = std::sync::OnceLock::new();
    let found = FOUND.get_or_init(|| {
        let file = |name: &str| format!("{name}{}", std::env::consts::EXE_SUFFIX);
        let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
        ["ffmpeg", "ffprobe"]
            .into_iter()
            .map(|name| {
                let mut places: Vec<PathBuf> = beside.iter().flat_map(|dir| [dir.join("ffmpeg").join(file(name)), dir.join(file(name))]).collect();
                if cfg!(target_os = "macos") && std::env::var_os("PATH").is_none_or(|p| !std::env::split_paths(&p).any(|d| d.join(name).is_file())) {
                    places.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(|d| Path::new(d).join(name)));
                }
                (name.to_owned(), places.into_iter().find(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(name)))
            })
            .collect()
    });
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = match found.iter().find(|(name, _)| name == tool) {
        Some((_, path)) => Command::new(path),
        None => Command::new(tool),
    };
    // CREATE_NO_WINDOW: on Windows each ffmpeg would otherwise flash a
    // console window of its own over the player.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    cmd
}

/// Everything ffprobe says about `input`'s container and streams, as JSON.
pub fn probe_all(input: &Path) -> Result<serde_json::Value> {
    let out = tool("ffprobe").args(["-v", "error", "-show_format", "-show_streams", "-of", "json"]).arg(input).output().context("running ffprobe")?;
    if !out.status.success() {
        bail!("ffprobe couldn't read {}", input.display());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

pub fn probe(input: &Path, stream: &str, entries: &str) -> Result<String> {
    let out = tool("ffprobe")
        .args(["-v", "error", "-select_streams", stream, "-show_entries", entries, "-of", "csv=p=0"])
        .arg(input)
        .output()
        .context("running ffprobe")?;
    // A stream with side data (a phone's rotation) ends each line with an
    // empty field for it: "30000/1001," didn't parse as a frame rate.
    Ok(String::from_utf8_lossy(&out.stdout).trim().lines().map(|l| l.trim_end_matches(',')).collect::<Vec<_>>().join("\n"))
}

fn probe_format(input: &Path) -> Result<String> {
    let out = tool("ffprobe").args(["-v", "error", "-show_entries", "format=format_name", "-of", "csv=p=0"]).arg(input).output().context("running ffprobe")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub fn frame_rate(input: &Path) -> Result<f64> {
    let rate = probe(input, "v:0", "stream=r_frame_rate")?;
    let (n, d) = rate.split_once('/').context("ffprobe gave no frame rate")?;
    Ok(n.parse::<f64>()? / d.parse::<f64>()?)
}

/// How long the clip runs; a picture is `PICTURE_MOST`, as long as its cut
/// may be stretched.
pub fn duration(input: &Path) -> Result<f64> {
    let out = tool("ffprobe")
        .args(["-v", "error", "-show_entries", "format=format_name,duration", "-of", "csv=p=0"])
        .arg(input)
        .output()
        .context("running ffprobe")?;
    let out = String::from_utf8_lossy(&out.stdout);
    // The format name may hold commas ("mov,mp4,m4a,...").
    let (format, duration) = out.trim().rsplit_once(',').context("ffprobe gave no duration")?;
    if is_picture(format) {
        return Ok(PICTURE_MOST);
    }
    Ok(duration.parse()?)
}

/// A picture's cut when it's added, and its export on its own.
pub const PICTURE_SECONDS: f64 = 5.0;
/// The longest a picture's cut can be trimmed out to.
pub const PICTURE_MOST: f64 = 3600.0;

/// Whether ffprobe's format name is a still picture's: `image2` (JPEG and
/// files named like a picture) or a `*_pipe` (PNG, WebP, ...). An animated
/// GIF is `gif`, a video.
fn is_picture(format: &str) -> bool {
    format == "image2" || format.ends_with("_pipe")
}

/// The clip's recording time as the camera tagged it, if it did.
pub fn creation_time(input: &Path) -> Result<Option<String>> {
    let out = tool("ffprobe")
        .args(["-v", "error", "-show_entries", "format_tags=creation_time", "-of", "csv=p=0"])
        .arg(input)
        .output()
        .context("running ffprobe")?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok((!t.is_empty()).then_some(t))
}

/// The low-resolution proxy DJI records next to the clip, if it is there:
/// the lens tracks side by side, each the full circle scaled down.
pub fn proxy(input: &Path) -> Option<PathBuf> {
    ["LRF", "lrf"].iter().map(|ext| input.with_extension(ext)).find(|p| p.exists())
}

/// Cuts a side-by-side proxy frame into one square picture per track.
pub fn split_proxy(frame: &Frame, tracks: &[u32]) -> Vec<Frame> {
    let side = frame.height;
    tracks
        .iter()
        .map(|&t| {
            let x0 = t as usize * side;
            let rgb = (0..side).flat_map(|y| &frame.rgb[(y * frame.width + x0) * 3..(y * frame.width + x0 + side) * 3]).copied().collect();
            Frame { width: side, height: side, rgb, deep: false }
        })
        .collect()
}

/// Decodes only `track`'s keyframes, scaled to `size`, and hands each to
/// `each` with its time until it returns false. Skipping the other frames
/// makes this cheap (0037's 55 s proxy: 84 frames in about 1 s); the cost
/// is mostly reading the file.
pub fn key_frames(input: &Path, track: u32, (w, h): (usize, usize), out: Out, mut each: impl FnMut(f64, Frame) -> bool) -> Result<()> {
    let mut filters = Vec::new();
    Out { size: Some((w as u32, h as u32)), ..out }.filters(&mut filters);
    out.rgb_filters(&mut filters);
    filters.push("showinfo".to_owned());
    let mut cmd = tool("ffmpeg");
    cmd.args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "info", "-skip_frame", "nokey"]);
    out.input_args(&mut cmd);
    let mut child = cmd
        .arg("-i")
        .arg(input)
        .args(["-map", &format!("0:v:{track}"), "-vf", &filters.join(","), "-fps_mode", "passthrough"])
        .args(["-f", "rawvideo", "-pix_fmt", out.rgb_format(), "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running ffmpeg")?;
    // showinfo logs each frame's time; read it on its own thread, or a
    // full stderr pipe would stall the decode.
    let stderr = child.stderr.take().unwrap();
    let (tx, times) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(t) = line.split("pts_time:").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|t| t.parse::<f64>().ok())
                && tx.send(t).is_err()
            {
                break;
            }
        }
    });
    let mut stdout = child.stdout.take().unwrap();
    let result = (|| -> Result<()> {
        loop {
            let mut data = vec![0; w * h * out.rgb_bytes()];
            match stdout.read_exact(&mut data) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e.into()),
            }
            let t = times.recv().context("ffmpeg gave a frame without its time")?;
            if !each(t, Frame { width: w, height: h, rgb: out.rgb24(data), deep: false }) {
                return Ok(());
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// RMS and peak level in dBFS of the first audio stream, per `window`
/// seconds from the start.
pub fn audio_levels(input: &Path, window: f64) -> Result<Vec<(f64, f64)>> {
    const RATE: f64 = 16000.0;
    let out = tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-ac", "1", "-ar", &RATE.to_string(), "-f", "f32le", "-"])
        .stderr(Stdio::inherit())
        .output()
        .context("running ffmpeg")?;
    let samples: Vec<f32> = out.stdout.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let db = |v: f64| 20.0 * v.max(1e-6).log10();
    Ok(samples
        .chunks(((window * RATE).round() as usize).max(1))
        .map(|w| {
            let rms = (w.iter().map(|&s| s as f64 * s as f64).sum::<f64>() / w.len() as f64).sqrt();
            (db(rms), db(w.iter().fold(0.0f64, |m, &s| m.max(s.abs() as f64))))
        })
        .collect())
}

/// An ordinary (flat) video's picture track: phones, drones and action cams
/// beside the Osmo's 360 clips. `stored` is its size as encoded; phones film
/// portrait as landscape and tag a rotation, which `turns` undoes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flat {
    /// Index among the file's video streams.
    pub track: u32,
    pub stored: (u32, u32),
    /// Quarter turns clockwise to stand it upright.
    pub turns: u8,
    /// A still picture: one frame, shown for as long as its cut lasts.
    pub picture: bool,
    /// HDR (a phone's HLG): brought down to SDR as it's shown (`hdr.rs`).
    pub hdr: Hdr,
    /// Full-range samples (0-255, `yuvj420p`, as Nikon cameras write).
    pub full_range: bool,
    /// Tagged with BT.601's matrix (SD video, some cameras).
    pub bt601: bool,
}

impl Flat {
    /// Its size upright.
    pub fn size(&self) -> (u32, u32) {
        if self.turns % 2 == 1 { (self.stored.1, self.stored.0) } else { self.stored }
    }

    /// Decoded upright, at `size` (upright) if given.
    pub fn out(&self, size: Option<(u32, u32)>, rate: Option<f64>) -> Out {
        Out { size, turns: Some(self.turns), rate, picture: self.picture, hdr: self.hdr, full_range: self.full_range, bt601: self.bt601 }
    }
}

/// `None` for an OSV (two square HEVC lens tracks), else the flat video
/// track to show. Errors if the file has no video at all.
pub fn flat(input: &Path) -> Result<Option<Flat>> {
    let streams = probe(input, "v", "stream=codec_name,width,height:stream_disposition=attached_pic")?;
    let mut lenses = 0;
    let mut first = None;
    for (i, line) in streams.lines().enumerate() {
        let f: Vec<&str> = line.split(',').collect();
        let (Some(codec), Some(w), Some(h)) = (f.first(), f.get(1).and_then(|v| v.parse::<u32>().ok()), f.get(2).and_then(|v| v.parse::<u32>().ok())) else { continue };
        // Cover art and thumbnails ride along as one-picture video streams.
        if f.get(3) == Some(&"1") {
            continue;
        }
        if *codec == "hevc" && w == h && w >= 1500 { // 100 fps mode records 1920×1920 lenses
            lenses += 1;
        }
        first.get_or_insert((i as u32, (w, h)));
    }
    if lenses >= 2 {
        return Ok(None);
    }
    let (track, stored) = first.with_context(|| format!("{} has no video", input.display()))?;
    // ffprobe gives the display matrix's angle counterclockwise; ffmpeg's
    // own autorotation turns -90 into a clockwise quarter turn.
    let rotation = probe(input, &format!("v:{track}"), "stream_side_data=rotation")?.lines().next().and_then(|r| r.trim().parse::<f64>().ok()).unwrap_or(0.0);
    let turns = ((-rotation / 90.0).round() as i64).rem_euclid(4) as u8;
    let picture = is_picture(&probe_format(input)?);
    // ffprobe gives them in its own order: format, range, matrix, transfer.
    let colour = probe(input, &format!("v:{track}"), "stream=pix_fmt,color_range,color_space,color_transfer")?;
    let colour: Vec<&str> = colour.lines().next().unwrap_or("").split(',').collect();
    let hdr = Hdr::from_transfer(colour.get(3).unwrap_or(&""));
    let full_range = colour.first().is_some_and(|f| f.starts_with("yuvj")) || colour.get(1) == Some(&"pc");
    let bt601 = matches!(colour.get(2), Some(&"smpte170m" | &"bt470bg"));
    Ok(Some(Flat { track, stored, turns, picture, hdr, full_range, bt601 }))
}

/// How a decode delivers frames, beyond the track as stored.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Out {
    /// Scaled to this size.
    pub size: Option<(u32, u32)>,
    /// `Some`: ffmpeg's autorotation off and this many quarter turns
    /// clockwise instead, so the size is known before the first frame (and
    /// it works after a hardware download). `None`: as ffmpeg does it.
    pub turns: Option<u8>,
    /// Frames repeated or dropped to this rate (a 60 fps phone clip in a
    /// 29.97 fps edit).
    pub rate: Option<f64>,
    /// A still picture: decoded once in software, laid over black where
    /// it's transparent, and never seeked (it has only its one frame).
    pub picture: bool,
    /// HDR video: an RGB decode delivers the BT.2020 signal at 16 bits
    /// (`rgb_filters`), which `rgb24` brings down to SDR (`rgb24`). The GPU's NV12
    /// and P010 are left as stored; the shader does the same there.
    pub hdr: Hdr,
    /// Full-range video: NV12 and P010 are brought to video range
    /// (`yuv_filters`), the only range the shader reads. Left as is, a
    /// Nikon's 0-255 came out crushed and dark (the user's DSC_0412,
    /// 2026-10-14). RGB decodes convert by the stream's own tag already.
    pub full_range: bool,
    /// A BT.601 video: NV12 and P010 are converted to BT.709's matrix
    /// (`yuv_filters`), the only one the shader reads.
    pub bt601: bool,
}

impl Out {
    fn input_args(&self, cmd: &mut Command) {
        if self.turns.is_some() {
            cmd.arg("-noautorotate");
        }
    }

    /// `-ss t`, except for a picture: at any time it's its one frame.
    fn seek(&self, cmd: &mut Command, t: f64) {
        if !self.picture {
            cmd.args(["-ss", &t.to_string()]);
        }
    }

    /// The decoder: a picture in software, as GPU decoders take only video.
    fn hw(&self, hw: Hwaccel) -> Hwaccel {
        if self.picture { Hwaccel::Software } else { hw }
    }

    /// For a stream of frames at another rate: each output frame takes the
    /// source frame nearest its time, counted from the first one after the
    /// seek, as frames are timed (start + n / fps). ffmpeg's `-r` took
    /// frames about 4 early going from 100 to 30 fps (40 ms: the picture
    /// ran ahead of the attitude, tilting the steadied view a few degrees
    /// in fast turns) and drifted going up from the 25 fps proxy; the `fps`
    /// filter alone keeps the last frame in each slot, 1-2 late. Shifting
    /// by half a source frame and rounding up makes the last in each slot
    /// the nearest. After any hardware download: before it, VA-API froze
    /// (the filter holds frames from its small surface pool).
    fn retime(&self, filters: &mut Vec<String>) {
        if let Some(rate) = self.rate.filter(|_| !self.picture) {
            filters.push(format!("setpts=PTS-STARTPTS-0.5/FRAME_RATE/TB,fps={rate}:round=up:start_time=0"));
        }
    }

    /// For a stream of frames: a picture's one frame repeated at the rate
    /// (30 fps without one), decoded and scaled only once.
    fn repeat(&self, filters: &mut Vec<String>) {
        if self.picture {
            filters.push(format!("loop=loop=-1:size=1,setpts=N/{}/TB", self.rate.unwrap_or(30.0)));
        }
    }

    /// Filters for after decoding: over black (a picture), the turn, then
    /// the scale.
    fn filters(&self, filters: &mut Vec<String>) {
        if self.picture {
            // Where a PNG is see-through its colours are anything; times
            // alpha they're over black, as decided for the main track.
            filters.push("format=rgba,premultiply=inplace=1".to_owned());
        }
        match self.turns {
            Some(1) => filters.push("transpose=clock".to_owned()),
            Some(2) => filters.push("hflip,vflip".to_owned()),
            Some(3) => filters.push("transpose=cclock".to_owned()),
            _ => {}
        }
        if let Some((w, h)) = self.size {
            // A picture is scaled once, so it may as well be scaled well.
            filters.push(format!("scale={w}:{h}:flags={}", if self.picture { "lanczos" } else { "bilinear" }));
        }
    }

    /// For an NV12 or P010 decode: full range brought to video range, and
    /// a picture (RGBA by now) or a BT.601 video given BT.709's matrix.
    /// swscale's default for both is BT.601, which the shader read as 709:
    /// colour bars' green 210 came out 190 with red's 15 at 0 (in the player
    /// and GPU export only; the CPU's RGB decode was right). ffmpeg 4.4
    /// (Ubuntu 22.04's) converts YUV to YUV by these options as 9.0 does.
    fn yuv_filters(&self, filters: &mut Vec<String>) {
        let mut opts = Vec::new();
        if self.full_range {
            opts.push("in_range=pc");
        }
        if self.bt601 && !self.picture {
            opts.push("in_color_matrix=bt601");
        }
        if self.bt601 || self.picture {
            opts.push("out_color_matrix=bt709");
        }
        if !opts.is_empty() {
            opts.push("out_range=tv");
            filters.push(format!("scale={}", opts.join(":")));
        }
    }

    /// For an RGB decode: an HDR video's BT.2020 matrix, read in limited
    /// range and given in full, for `rgb24`.
    fn rgb_filters(&self, filters: &mut Vec<String>) {
        if self.hdr != Hdr::None {
            filters.push("scale=in_color_matrix=bt2020:in_range=tv:out_range=pc".to_owned());
        }
    }

    fn rgb_format(&self) -> &'static str {
        if self.hdr == Hdr::None { "rgb24" } else { "rgb48le" }
    }

    /// Bytes an RGB decode gives a pixel.
    fn rgb_bytes(&self) -> usize {
        if self.hdr == Hdr::None { 3 } else { 6 }
    }

    /// An RGB decode's pixels as 8-bit SDR RGB.
    fn rgb24(&self, data: Vec<u8>) -> Vec<u8> {
        use rayon::prelude::*;
        if self.hdr == Hdr::None {
            return data;
        }
        let mut rgb = vec![0u8; data.len() / 2];
        rgb.par_chunks_mut(3).zip(data.par_chunks(6)).for_each(|(px, d)| {
            let v = |i: usize| u16::from_le_bytes([d[2 * i], d[2 * i + 1]]) as f32 / 65535.0;
            let sdr = crate::hdr::to_sdr([v(0), v(1), v(2)], self.hdr);
            for c in 0..3 {
                px[c] = (sdr[c] * 255.0).round() as u8;
            }
        });
        rgb
    }

}

pub fn has_audio(input: &Path) -> Result<bool> {
    Ok(!probe(input, "a", "stream=index")?.is_empty())
}

pub fn dimensions(input: &Path, track: u32) -> Result<(usize, usize)> {
    let dims = probe(input, &format!("v:{track}"), "stream=width,height")?;
    let (w, h) = dims.split_once(',').context("ffprobe gave no dimensions")?;
    Ok((w.parse()?, h.parse()?))
}

/// Hardware decoders, in the order they are tried. Vulkan Video works on every
/// vendor with a recent driver; VA-API and CUDA cover stacks without it.
/// VideoToolbox is macOS's, and the only one there.
#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum Hwaccel {
    Vulkan,
    Vaapi,
    Cuda,
    #[value(name = "videotoolbox")]
    VideoToolbox,
    D3d11va,
    Software,
}

impl Hwaccel {
    fn name(self) -> Option<&'static str> {
        match self {
            Hwaccel::Vulkan => Some("vulkan"),
            Hwaccel::Vaapi => Some("vaapi"),
            Hwaccel::Cuda => Some("cuda"),
            Hwaccel::VideoToolbox => Some("videotoolbox"),
            Hwaccel::D3d11va => Some("d3d11va"),
            Hwaccel::Software => None,
        }
    }

    /// The pixel format its frames stay on the GPU as, for `hwdownload`.
    fn output_format(self) -> Option<&'static str> {
        match self {
            Hwaccel::VideoToolbox => Some("videotoolbox_vld"),
            Hwaccel::D3d11va => Some("d3d11"),
            hw => hw.name(),
        }
    }

    /// The first decoder that gets a frame of `track` out of `input`.
    pub fn pick(input: &Path, track: u32) -> Result<Hwaccel> {
        // Windows: Direct3D 11 decodes on any GPU vendor's driver; CUDA and
        // Vulkan stay as fallbacks.
        let tried: &[Hwaccel] = if cfg!(target_os = "macos") {
            &[Hwaccel::VideoToolbox]
        } else if cfg!(windows) {
            &[Hwaccel::D3d11va, Hwaccel::Cuda, Hwaccel::Vulkan]
        } else {
            &[Hwaccel::Vulkan, Hwaccel::Vaapi, Hwaccel::Cuda]
        };
        for &hw in tried {
            if hw.works(input, track)? {
                return Ok(hw);
            }
        }
        Ok(Hwaccel::Software)
    }

    /// Whether this decoder gets a full-size frame of `track` out of `input`.
    /// `hwdownload` rejects software frames, so a probe only succeeds if the
    /// GPU really decoded; ffmpeg's own fallback would hide a failure.
    ///
    /// Each probe takes 0.5-1 s, and every Osmo 360 clip's tracks are the
    /// same (HEVC Main10, 3840²), so each decoder is probed once a run per
    /// kind of track (codec, profile, sampling, size): an export of several
    /// clips otherwise spent seconds probing. Probed once for all, a 10-bit
    /// 4:2:2 H.264 video (as Sony and Panasonic record) after an OSV was
    /// given the OSV's decoder, which can't decode it: "no frames decoded".
    pub fn works(self, input: &Path, track: u32) -> Result<bool> {
        static PROBED: std::sync::Mutex<Vec<(Hwaccel, String, bool)>> = std::sync::Mutex::new(Vec::new());
        let kind = probe(input, &format!("v:{track}"), "stream=codec_name,profile,pix_fmt,width,height")?;
        if let Some(&(_, _, ok)) = PROBED.lock().unwrap().iter().find(|(hw, k, _)| *hw == self && *k == kind) {
            return Ok(ok);
        }
        let ok = nv12(input, track, 0.0, Out::default(), self)?
            .args(["-frames:v", "1", "-f", "null", "-"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("running ffmpeg")?
            .success();
        PROBED.lock().unwrap().push((self, kind, ok));
        Ok(ok)
    }
}

/// An ffmpeg command, missing only its output, that decodes `track` from
/// `start` to NV12, scaled to `size` pixels square if given. Scaling and
/// conversion run on the CPU: the GPU filters differ per backend, and timing
/// showed the download, not the conversion, is the cost.
///
/// Except on CUDA at full size: `scale_cuda` converts the 10-bit frames to
/// NV12 before the download, which halves the bytes. Both 3840² lenses of
/// 0037 then decode at 1.9× real time instead of 0.9× on Vulkan.
pub fn nv12(input: &Path, track: u32, start: f64, out: Out, hw: Hwaccel) -> Result<Command> {
    Ok(yuv(input, track, start, out, hw, false)?.0)
}

/// `nv12`, or with `deep` and a 10-bit track, P010: the same layout with
/// 16-bit samples, the 10 bits in the high ones. Says which it is. A 10-bit
/// track's frames come off Vulkan as P010 anyway (keeping them was a little
/// faster than converting: 0037's lenses at 1.19× real time against 1.10×);
/// on CUDA they cost twice the download (1.33× against 2.3×).
pub fn yuv(input: &Path, track: u32, start: f64, out: Out, hw: Hwaccel, deep: bool) -> Result<(Command, bool)> {
    let hw = out.hw(hw);
    // Asked only where it matters: a software decode to NV12 converts
    // whatever it gets (the proxy never paid for a probe).
    let ten_bit = !out.picture && (deep || hw.name().is_some()) && ten_bit(input, track)?;
    let deep = deep && ten_bit;
    let format = if deep { "p010le" } else { "nv12" };
    let mut filters = Vec::new();
    let mut cmd = tool("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"]);
    if let Some(name) = hw.name() {
        cmd.args(["-hwaccel", name, "-hwaccel_output_format", hw.output_format().unwrap_or(name)]);
        if hw == Hwaccel::Cuda && out.size.is_none() {
            filters.push(format!("scale_cuda=format={format},hwdownload,format={format}"));
        } else {
            filters.push(format!("hwdownload,format={}", if ten_bit { "p010le" } else { "nv12" }));
        }
    }
    out.retime(&mut filters);
    out.filters(&mut filters);
    out.repeat(&mut filters);
    out.yuv_filters(&mut filters);
    filters.push(format!("format={format}"));
    out.input_args(&mut cmd);
    out.seek(&mut cmd, start);
    cmd.arg("-i").arg(input);
    cmd.args(["-map", &format!("0:v:{track}"), "-vf", &filters.join(",")]);
    Ok((cmd, deep))
}

/// Whether `track` holds more than 8 bits a sample, asked of ffprobe once
/// per track a run.
fn ten_bit(input: &Path, track: u32) -> Result<bool> {
    static KNOWN: std::sync::Mutex<Vec<(PathBuf, u32, bool)>> = std::sync::Mutex::new(Vec::new());
    if let Some(&(_, _, deep)) = KNOWN.lock().unwrap().iter().find(|(p, t, _)| p == input && *t == track) {
        return Ok(deep);
    }
    let deep = probe(input, &format!("v:{track}"), "stream=pix_fmt")?.contains("10");
    KNOWN.lock().unwrap().push((input.to_owned(), track, deep));
    Ok(deep)
}

/// One RGB24 frame of a video track at `t`, decoded on `hw` when it can be.
/// Asking for exactly one frame lets ffmpeg finish cleanly instead of
/// complaining about a closed pipe.
pub fn still(input: &Path, track: u32, t: f64, hw: Hwaccel) -> Result<Frame> {
    still_out(input, track, t, hw, dimensions(input, track)?, Out::default())
}

/// A picture's one frame as RGBA, upright and premultiplied (`Out`'s
/// picture filters), at `(w, h)`.
pub fn picture_rgba(input: &Path, flat: Flat, (w, h): (u32, u32)) -> Result<Vec<u8>> {
    let out = flat.out(Some((w, h)), None);
    let mut cmd = tool("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"]);
    out.input_args(&mut cmd);
    let mut filters = Vec::new();
    out.filters(&mut filters);
    cmd.arg("-i").arg(input).args(["-map", &format!("0:v:{}", flat.track)]);
    if !filters.is_empty() {
        cmd.args(["-vf", &filters.join(",")]);
    }
    let out = cmd.args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"]).stderr(Stdio::inherit()).output().context("running ffmpeg")?;
    if out.stdout.len() != (w * h * 4) as usize {
        bail!("couldn't read {}", input.display());
    }
    Ok(out.stdout)
}

/// `still` delivered as `out` says, which comes out `(width, height)`.
pub fn still_out(input: &Path, track: u32, t: f64, hw: Hwaccel, (width, height): (usize, usize), out: Out) -> Result<Frame> {
    let mut cmd = tool("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"]);
    if let Some(name) = out.hw(hw).name() {
        cmd.args(["-hwaccel", name]);
    }
    out.input_args(&mut cmd);
    let mut filters = Vec::new();
    out.filters(&mut filters);
    out.rgb_filters(&mut filters);
    out.seek(&mut cmd, t);
    cmd.arg("-i").arg(input).args(["-map", &format!("0:v:{track}")]);
    if !filters.is_empty() {
        cmd.args(["-vf", &filters.join(",")]);
    }
    let decoded = cmd
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", out.rgb_format(), "-"])
        .stderr(Stdio::inherit())
        .output()
        .context("running ffmpeg")?;
    if decoded.stdout.len() != width * height * out.rgb_bytes() {
        bail!("no frame at {t:.2} s");
    }
    Ok(Frame { width, height, rgb: out.rgb24(decoded.stdout), deep: false })
}

/// The last lines a child wrote to stderr, read on a thread of its own (a
/// full pipe would stall it), for saying why it failed.
pub struct Tail(Option<std::thread::JoinHandle<Vec<String>>>);

impl Tail {
    const LINES: usize = 6;

    /// Takes over `child`'s stderr, which must be piped.
    pub fn of(child: &mut Child) -> Self {
        let stderr = child.stderr.take().expect("stderr piped");
        Tail(Some(std::thread::spawn(move || {
            use std::io::BufRead;
            let mut lines = std::collections::VecDeque::new();
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                if lines.len() == Self::LINES {
                    lines.pop_front();
                }
                lines.push_back(line);
            }
            lines.into()
        })))
    }

    /// Why a decode that exited with `status` failed, with what it wrote.
    pub fn failure(&mut self, status: std::process::ExitStatus) -> String {
        let lines = self.0.take().and_then(|h| h.join().ok()).unwrap_or_default();
        let mut why = format!("ffmpeg failed decoding ({status})");
        if !lines.is_empty() {
            why = format!("{why}: {}", lines.join("; "));
        }
        why
    }
}

/// Streams RGB24 frames of one video track, starting at the first frame at or
/// after `start` seconds. Decodes on `hw` when it can (3.5× faster for a
/// 3840² HEVC still on Vulkan) and quietly in software when it can't.
pub struct Decoder {
    child: Child,
    stdout: ChildStdout,
    stderr: Tail,
    input: PathBuf,
    width: usize,
    height: usize,
    /// How it was asked to deliver, for `Out::rgb24`.
    out: Out,
}

impl Decoder {
    /// Decodes `track` of `input` from `start`, delivered as `out` says,
    /// which comes out `(width, height)`.
    pub fn with(input: &Path, track: u32, start: f64, hw: Hwaccel, (width, height): (usize, usize), out: Out) -> Result<Self> {
        let mut cmd = tool("ffmpeg");
        cmd.args(["-v", "error"]);
        if let Some(name) = out.hw(hw).name() {
            cmd.args(["-hwaccel", name]);
        }
        out.input_args(&mut cmd);
        let mut filters = Vec::new();
        out.retime(&mut filters);
        out.filters(&mut filters);
        out.rgb_filters(&mut filters);
        out.repeat(&mut filters);
        out.seek(&mut cmd, start);
        cmd.arg("-i").arg(input).args(["-map", &format!("0:v:{track}")]);
        if !filters.is_empty() {
            cmd.args(["-vf", &filters.join(",")]);
        }
        let mut child = cmd
            .args(["-f", "rawvideo", "-pix_fmt", out.rgb_format(), "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("running ffmpeg")?;
        let stdout = child.stdout.take().unwrap();
        let stderr = Tail::of(&mut child);
        Ok(Decoder { child, stdout, stderr, input: input.to_owned(), width, height, out })
    }

    /// The next frame, or `None` at the end of the stream. Fails if ffmpeg
    /// did: a read error or a dying decoder mid-clip otherwise looked like
    /// the clip's end, and the export held its last frame without a word.
    pub fn next(&mut self) -> Result<Option<Frame>> {
        let mut data = vec![0; self.width * self.height * self.out.rgb_bytes()];
        match self.stdout.read_exact(&mut data) {
            Ok(()) => Ok(Some(Frame { width: self.width, height: self.height, rgb: self.out.rgb24(data), deep: false })),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                let status = self.child.wait()?;
                if !status.success() {
                    bail!("{}: {}", self.input.display(), self.stderr.failure(status));
                }
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What `Encoder` writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// H.264 and AAC in MP4: plays anywhere, small.
    #[default]
    Mp4,
    /// 10-bit DNxHR HQX (4:2:2) and 16-bit PCM in MOV, for DaVinci
    /// Resolve: its free Linux version decodes neither H.264/H.265 nor AAC.
    /// About 28 MB a second at 1080p, as 8-bit HQ was; an editing
    /// intermediate, with the bits for grading. A GPU export feeds it 16-bit
    /// frames (`Frame::deep`).
    Resolve,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Resolve => "mov",
        }
    }

    fn codecs(self) -> &'static [&'static str] {
        match self {
            Format::Mp4 => &["-c:v", "libx264", "-crf", "18", "-preset", "medium", "-pix_fmt", "yuv420p", "-c:a", "aac"],
            Format::Resolve => &["-c:v", "dnxhd", "-profile:v", "dnxhr_hqx", "-pix_fmt", "yuv422p10le", "-c:a", "pcm_s16le", "-ar", "48000"],
        }
    }
}

/// Encodes RGB frames (8 or 16 bits a sample), muxing the source's audio
/// over the same span.
pub struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
    /// It takes 16-bit frames.
    deep: bool,
}

/// Where rendered frames go: an encoder, or something blending them first.
pub trait FrameSink {
    fn write(&mut self, frame: &Frame) -> Result<()>;
}

impl FrameSink for Encoder {
    fn write(&mut self, frame: &Frame) -> Result<()> {
        Encoder::write(self, frame)
    }
}

/// Writes `duration` seconds of `input`'s sound from `start` to `output` as
/// a 48 kHz 24-bit WAV, untouched otherwise: music for another editor, in a
/// form any of them reads (Resolve's free version on Linux reads no AAC or
/// MP3).
pub fn wav(input: &Path, start: f64, duration: f64, output: &Path) -> Result<()> {
    let status = tool("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y", "-ss", &start.to_string(), "-t", &duration.to_string(), "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-ar", "48000", "-c:a", "pcm_s24le"])
        .arg(output)
        .status()
        .context("running ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg couldn't write {}", output.display());
    }
    Ok(())
}

impl Encoder {
    /// The sound is `audio`'s stretches back to back. Each stretch should
    /// last exactly as long as the frames written for it, so later cuts stay
    /// in sync. Several stretches are joined only if every clip has sound.
    /// No `-shortest`: the sound already ends with the frames, and with the
    /// frames arriving slower than the sound it cut the last frame off.
    /// With `mix`, the joined sound is set to its level and the music mixed
    /// in.
    /// `deep`: frames come with 16-bit samples (`Frame::deep`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(output: &Path, (w, h): (usize, usize), fps: f64, audio: &[Sound<'_>], mix: Option<&Mix<'_>>, format: Format, deep: bool) -> Result<Self> {
        let mut cmd = tool("ffmpeg");
        cmd.args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", if deep { "rgb48le" } else { "rgb24" }]).args(["-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
        for s in audio {
            match s.duration {
                // A picture or a silent video: silence as long as its frames.
                Some(d) if !has_audio(s.input).unwrap_or(false) => {
                    cmd.args(["-f", "lavfi", "-t", &d.to_string(), "-i", "anullsrc=r=48000:cl=stereo"]);
                }
                _ => {
                    cmd.args(["-ss", &s.start.to_string()]);
                    if let Some(d) = s.duration {
                        cmd.args(["-t", &d.to_string()]);
                    }
                    cmd.arg("-i").arg(s.input);
                }
            }
        }
        // The music's inputs, before any output option.
        let points = audio.iter().any(|s| !s.volume.is_empty() || s.crossfade > 0.0 || s.gain != 0.0 || s.filter.is_some());
        let graph = mix.filter(|m| m.matters() || points).map(|m| mixed(&mut cmd, audio, m)).transpose()?;
        cmd.args(["-map", "0:v"]);
        if let Some(graph) = graph {
            cmd.args(["-filter_complex", &graph, "-map", "[a]"]);
        } else if let [_] = audio {
            cmd.args(["-map", "1:a:0?"]);
        } else if audio.iter().all(Sound::sounds) {
            // Same camera, but resample anyway: concat needs matching formats.
            let parts: String = (1..).zip(audio).map(|(i, s)| format!("[{i}:a:0]aresample=48000,aformat=channel_layouts=stereo{}[a{i}];", pad(s))).collect();
            let inputs: String = (1..=audio.len()).map(|i| format!("[a{i}]")).collect();
            cmd.args(["-filter_complex", &format!("{parts}{inputs}concat=n={}:v=0:a=1[a]", audio.len()), "-map", "[a]"]);
        } else if !audio.is_empty() {
            eprintln!("not every clip has sound; exporting without");
        }
        let mut child = cmd
            .args(format.codecs())
            .arg(output)
            .stdin(Stdio::piped())
            .spawn()
            .context("running ffmpeg")?;
        let stdin = child.stdin.take();
        Ok(Encoder { child, stdin, deep })
    }

    pub fn write(&mut self, frame: &Frame) -> Result<()> {
        if frame.deep != self.deep {
            bail!("a {}-bit frame for a {}-bit encoder", if frame.deep { 16 } else { 8 }, if self.deep { 16 } else { 8 });
        }
        self.stdin.as_mut().unwrap().write_all(&frame.rgb)?;
        Ok(())
    }

    /// Stops encoding, leaving whatever was written.
    pub fn abort(mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin.take());
        if !self.child.wait()?.success() {
            bail!("ffmpeg encoder failed");
        }
        Ok(())
    }
}


/// Media made for tests by ffmpeg's own generators: tiny, in a folder of
/// the test's own (tests run side by side), never real footage.
#[cfg(test)]
pub mod testmedia {
    use std::path::{Path, PathBuf};

    /// A fresh folder for test `name`, removed by the caller.
    pub fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omnix-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Runs ffmpeg with `args` (after `-v error -y`), panicking if it fails.
    pub fn ffmpeg(args: &[&str]) {
        let ok = super::tool("ffmpeg").args(["-v", "error", "-nostdin", "-y"]).args(args).status().unwrap().success();
        assert!(ok, "ffmpeg {args:?}");
    }

    /// A steady 1 kHz tone, `secs` long, 48 kHz mono WAV at `path`.
    pub fn tone(path: &Path, secs: f64) {
        tone_at(path, secs, 1000);
    }

    /// The same at `hz`: tones of different pitches add up by power, so
    /// two sounds over each other can be told apart in a level.
    pub fn tone_at(path: &Path, secs: f64, hz: u32) {
        ffmpeg(&["-f", "lavfi", "-i", &format!("sine=f={hz}:d={secs}:sample_rate=48000"), path.to_str().unwrap()]);
    }
}
