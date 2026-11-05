//! A clip ready to stitch: its rig, lenses, stabiliser and frame rate (`Pipeline`), and
//! rendering its frames (`encode`, `grade_frame`), shared by the CLI, the export and the player.

use crate::{align, cache, dji, exposure, ffmpeg, lut, stab, stitch, track, view};
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use clap::Args;
use std::fs::File;
use std::path::{Path, PathBuf};
use crate::view::Reframe;
use telemetry_parser::rig::{RigPicture, geometry::ProjectionKind};

#[derive(Args, Clone)]
pub(crate) struct StitchOpts {
    /// Use this alternative calibration (see `rig`'s `variants`) instead of the default.
    #[arg(long)]
    pub(crate) variant: Option<usize>,
    /// Render only this lens (0 or 1), for checking overlap alignment.
    #[arg(long)]
    pub(crate) only: Option<usize>,
    /// Stabilisation mode, from the camera's attitude data [default: off
    /// for stitch and render, horizon for export and preview].
    #[arg(long, value_enum)]
    pub(crate) stabilize: Option<stab::Mode>,
    /// Heading smoothing window for `horizon`, in ms either side.
    #[arg(long, default_value_t = 500.0)]
    pub(crate) smooth: f64,
    /// How the IMU's axes sit relative to the rig's.
    #[arg(long, value_enum, default_value_t = stab::ImuAxes::X180)]
    pub(crate) imu_axes: stab::ImuAxes,
    /// Treat the attitude's world z as pointing down rather than up.
    #[arg(long)]
    pub(crate) world_z_down: bool,
    /// Skip rolling-shutter correction.
    #[arg(long)]
    pub(crate) no_rs: bool,
    /// Even out the lenses' exposure at the seam. Off by default: where the
    /// sun veils one lens it dulls the other to meet it.
    #[arg(long)]
    pub(crate) exposure: bool,
    /// Skip shifting the lenses to line up close subjects at the seam.
    #[arg(long)]
    pub(crate) no_align: bool,
    /// Blend everywhere instead of cutting where the lenses disagree.
    #[arg(long)]
    pub(crate) no_cut: bool,
    /// Give each lens the other's video track. Which track a lens was
    /// recorded to isn't in the clip (telemetry-parser goes by the camera
    /// model), and swapped back-to-back lenses still stitch cleanly, only
    /// half a turn round the stick: horizon mode then swings by twice the
    /// tilt and stick follow looks away from the holder (issue #4).
    #[arg(long)]
    pub(crate) swap_lenses: bool,
}

impl Default for StitchOpts {
    fn default() -> Self {
        StitchOpts {
            variant: None,
            only: None,
            stabilize: None,
            smooth: 500.0,
            imu_axes: stab::ImuAxes::X180,
            world_z_down: false,
            no_rs: false,
            exposure: false,
            no_align: false,
            no_cut: false,
            swap_lenses: false,
        }
    }
}

/// What to draw from the stitched sphere.
pub(crate) enum Shot {
    /// The whole sphere, this wide.
    Equirect(usize),
    View { camera: Box<view::Camera>, size: (usize, usize) },
}

/// Everything needed to turn decoded lens frames at a given time into a
/// stitched picture.
pub(crate) struct Pipeline {
    pub(crate) input: PathBuf,
    pub(crate) lenses: Vec<stitch::Lens>,
    pub(crate) tracks: Vec<u32>,
    pub(crate) stab: stab::Stabilizer,
    pub(crate) readout_ms: f64,
    pub(crate) fps: f64,
    pub(crate) only: Option<usize>,
    pub(crate) rs_steps: usize,
    pub(crate) seam: Option<exposure::Seam>,
    pub(crate) aligner: Option<align::Aligner>,
    pub(crate) cut: bool,
    pub(crate) hw: ffmpeg::Hwaccel,
    /// Decode this side-by-side proxy instead of the lens tracks.
    pub(crate) proxy: Option<PathBuf>,
    /// An ordinary video: no lenses, rig or attitude; drawn by cropping.
    pub(crate) flat: Option<ffmpeg::Flat>,
    /// Decode at this rate instead of the clip's own (an ordinary video in
    /// an edit at another rate); `fps` is then this.
    pub(crate) rate: Option<f64>,
    /// The colour profile it was filmed in (D-Log M wants a LUT).
    pub(crate) color_mode: dji::ColorMode,
    /// Tracks that keyframes follow, read once each (`None`: unreadable).
    pub(crate) followed: std::sync::Mutex<std::collections::HashMap<PathBuf, Option<std::sync::Arc<track::Track>>>>,
}

/// Seam state carried from frame to frame so it eases instead of flickering.
#[derive(Default)]
pub(crate) struct Carry {
    pub(crate) gains: Option<exposure::Gains>,
    pub(crate) field: Option<align::Field>,
}

impl Pipeline {
    /// `stabilize` is the mode when the options don't name one.
    pub(crate) fn new(input: &Path, opts: &StitchOpts, stabilize: stab::Mode) -> Result<Self> {
        Self::load(input, opts, stabilize, &|_| {})
    }

    /// `new`, telling `progress` how far reading the telemetry has got (0..1).
    pub(crate) fn load(input: &Path, opts: &StitchOpts, stabilize: stab::Mode, progress: &dyn Fn(f64)) -> Result<Self> {
        if let Some(flat) = ffmpeg::flat(input)? {
            return Self::load_flat(input, flat);
        }
        let telemetry = cache::telemetry(input, progress)?;
        let rig = telemetry.rig.context("no camera rig in file")?;
        let projection = match opts.variant {
            Some(i) => &rig.variants.get(i).context("no such variant")?.projection,
            None => &rig.projection,
        };
        let ProjectionKind::Rig { cameras, .. } = &projection.kind else {
            bail!("not a multi-lens rig");
        };
        let mut lenses = cameras.iter().map(stitch::Lens::new).collect::<Result<Vec<_>>>()?;
        if opts.swap_lenses {
            for lens in &mut lenses {
                lens.input = 1 - lens.input;
            }
        }
        let tracks = rig
            .pictures
            .iter()
            .map(|p| match p {
                RigPicture::VideoTrack(n) => Ok(*n),
                RigPicture::Sibling { .. } => bail!("multi-file rigs are not supported"),
            })
            .collect::<Result<Vec<_>>>()?;
        let mode = opts.stabilize.unwrap_or(stabilize);
        let stab = stab::Stabilizer::new(&telemetry.quats, mode, opts.imu_axes, if opts.world_z_down { 1.0 } else { -1.0 }, opts.smooth);
        if !stab.has_data() && mode != stab::Mode::Off {
            bail!("no attitude data to stabilise with");
        }
        let readout_ms = cameras[0].readout.map_or(0.0, |r| r.time_us / 1000.0);
        let rs_steps = if opts.no_rs || readout_ms == 0.0 || !stab.has_data() { 1 } else { 33 };
        let seam = if opts.exposure && opts.only.is_none() { exposure::Seam::new(&lenses) } else { None };
        let aligner = if opts.no_align || opts.only.is_some() { None } else { align::Aligner::new(&lenses) };
        let hw = ffmpeg::Hwaccel::pick(input, tracks[0])?;
        Ok(Pipeline {
            input: input.to_owned(),
            lenses,
            tracks,
            stab,
            readout_ms,
            fps: ffmpeg::frame_rate(input)?,
            only: opts.only,
            rs_steps,
            seam,
            aligner,
            cut: !opts.no_cut && opts.only.is_none(),
            hw,
            proxy: None,
            flat: None,
            rate: None,
            color_mode: telemetry.color_mode,
            followed: Default::default(),
        })
    }

    /// An ordinary video: nothing to stitch, steady or follow.
    pub(crate) fn load_flat(input: &Path, flat: ffmpeg::Flat) -> Result<Self> {
        Ok(Pipeline {
            input: input.to_owned(),
            lenses: Vec::new(),
            tracks: vec![flat.track],
            stab: stab::Stabilizer::new(&[], stab::Mode::Off, stab::ImuAxes::X180, -1.0, 0.0),
            readout_ms: 0.0,
            fps: ffmpeg::frame_rate(input)?,
            only: None,
            rs_steps: 1,
            seam: None,
            aligner: None,
            cut: false,
            hw: if flat.picture { ffmpeg::Hwaccel::Software } else { ffmpeg::Hwaccel::pick(input, flat.track)? },
            proxy: None,
            flat: Some(flat),
            rate: None,
            color_mode: dji::ColorMode::Normal,
            followed: Default::default(),
        })
    }

    /// Where `follow` points in the view at clip time `t` (seconds).
    pub(crate) fn aim(&self, follow: &view::Follow, t: f64) -> Option<(f64, f64)> {
        if self.flat.is_some() {
            return None;
        }
        match follow {
            view::Follow::Stick => self.stab.stick(t * 1000.0, self.readout_ms),
            view::Follow::Track(path) => {
                let track = self
                    .followed
                    .lock()
                    .unwrap()
                    .entry(path.clone())
                    .or_insert_with(|| track::Track::load(path).inspect_err(|e| eprintln!("{e:#}; keyframes following it look ahead")).ok().map(std::sync::Arc::new))
                    .clone()?;
                track.aim(&self.stab, t, self.readout_ms)
            }
        }
    }

    /// Decode from the clip's proxy from now on, if it has one.
    pub(crate) fn use_proxy(&mut self) {
        if self.flat.is_some() {
            return;
        }
        self.proxy = ffmpeg::proxy(&self.input);
        if self.proxy.is_none() {
            eprintln!("no proxy (LRF) next to the clip; using the full-resolution tracks");
        }
    }

    /// Rounds to the nearest frame so attitude lookups match decoded pictures.
    pub(crate) fn snap(&self, t: f64) -> f64 {
        (t * self.fps).round() / self.fps
    }

    /// Stitches one frame, measuring the seam afresh and easing towards it.
    pub(crate) fn render(&self, frames: &[stitch::Frame], t: f64, carry: &mut Carry, shot: &Shot) -> stitch::Frame {
        if self.flat.is_some() {
            let picture = &frames[0];
            return match *shot {
                // The whole picture, as the home view shows it.
                Shot::Equirect(width) => {
                    let size = (width, (width * picture.height / picture.width).max(2) / 2 * 2);
                    stitch::flat(picture, size, &view::View::home(view::Projection::Perspective).camera())
                }
                Shot::View { ref camera, size } => {
                    let mut frame = stitch::flat(picture, size, camera);
                    grade_frame(&mut frame, camera);
                    frame
                }
            };
        }
        let views = if self.stab.has_data() { self.stab.rig_from_view(t * 1000.0, self.readout_ms, self.rs_steps) } else { vec![stitch::IDENTITY] };
        let rgb = |i: usize, u: f64, v: f64| self.lenses[i].sample(frames, u, v).map(|c| c / 255.0);
        let exposure = self.seam.as_ref().map(|seam| {
            let now = seam.measure(|i, u, v| Some(rgb(i, u, v)));
            let g = carry.gains.get_or_insert(now);
            g.ease(&now, exposure::EASE);
            (seam, *g)
        });
        let align = self.aligner.as_ref().map(|aligner| {
            let now = aligner.measure(&aligner.strips(|i, u, v| Some(rgb(i, u, v).iter().sum::<f64>() / 3.0)), carry.field.as_ref());
            let f = carry.field.get_or_insert_with(|| now.clone());
            f.ease(&now, align::EASE);
            (aligner, f.clone())
        });
        let seam = stitch::Seam { exposure: exposure.as_ref().map(|(s, g)| (*s, g)), align: align.as_ref().map(|(a, f)| (*a, f)), cut: self.cut };
        match *shot {
            Shot::Equirect(width) => stitch::equirect(&self.lenses, frames, width, self.only, &views, &seam),
            Shot::View { ref camera, size: (w, h) } => {
                let ray = |x: f64, y: f64| stitch::apply(camera.user, camera.ray(w as f64 / h as f64, [x / w as f64 * 2.0 - 1.0, y / h as f64 * 2.0 - 1.0]));
                let mut frame = stitch::render(&self.lenses, frames, (w, h), ray, self.only, &views, &seam);
                grade_frame(&mut frame, camera);
                frame
            }
        }
    }

    /// Every lens's frame at `t`.
    pub(crate) fn stills(&self, t: f64) -> Result<Vec<stitch::Frame>> {
        if let Some(flat) = &self.flat {
            let (w, h) = flat.size();
            return Ok(vec![ffmpeg::still_out(&self.input, flat.track, t, self.hw, (w as usize, h as usize), flat.out(None, None))?]);
        }
        match &self.proxy {
            Some(proxy) => Ok(ffmpeg::split_proxy(&ffmpeg::still(proxy, 0, t, self.hw)?, &self.tracks)),
            None => self.tracks.iter().map(|&n| ffmpeg::still(&self.input, n, t, self.hw)).collect(),
        }
    }

    /// Says so when an export holds this clip's last frame for more than a
    /// frame or two: the cut runs past what decoded (a duration rounded up
    /// costs a frame), so its picture freezes.
    pub fn warn_held(&self, frames: usize) {
        if frames > 2 {
            eprintln!(
                "\nwarning: {} ran out {frames} frames ({:.2} s) before the cut's end; its last frame is held",
                self.input.display(),
                frames as f64 / self.fps
            );
        }
    }

    pub(crate) fn stream(&self, start: f64) -> Result<Stream> {
        if let Some(flat) = &self.flat {
            let (w, h) = flat.size();
            return Ok(Stream::Tracks(vec![ffmpeg::Decoder::with(&self.input, flat.track, start, self.hw, (w as usize, h as usize), flat.out(None, self.rate))?]));
        }
        Ok(match &self.proxy {
            // At the clip's rate: in the 100 fps mode the proxy is 25 fps,
            // and frames are timed n / fps.
            Some(proxy) => {
                let out = ffmpeg::Out { rate: Some(self.fps), ..Default::default() };
                Stream::Proxy(ffmpeg::Decoder::with(proxy, 0, start, self.hw, ffmpeg::dimensions(proxy, 0)?, out)?, self.tracks.clone())
            }
            // At the edit's rate where it's the clip's own (`cut_pipelines`).
            None => Stream::Tracks(
                self.tracks
                    .iter()
                    .map(|&n| ffmpeg::Decoder::with(&self.input, n, start, self.hw, ffmpeg::dimensions(&self.input, n)?, ffmpeg::Out { rate: self.rate, ..Default::default() }))
                    .collect::<Result<_>>()?,
            ),
        })
    }

    /// The span to render: `start`/`duration` if given, else the keyframes'
    /// span, else from `start` to the end of the clip.
    pub(crate) fn span(&self, start: Option<f64>, duration: Option<f64>, reframe: Option<&Reframe>) -> Result<(f64, f64)> {
        let (start, end) = span(&self.input, start, duration, reframe)?;
        Ok((self.snap(start), end))
    }
}

/// The span of `input` to render: `start`/`duration` if given, else the
/// keyframes' span, else from `start` to the end of the clip.
pub(crate) fn span(input: &Path, start: Option<f64>, duration: Option<f64>, reframe: Option<&Reframe>) -> Result<(f64, f64)> {
    let keyed = reframe.and_then(Reframe::span);
    let start = start.or(keyed.map(|k| k.0)).unwrap_or(0.0);
    let end = match (duration, keyed) {
        (Some(d), _) => start + d,
        (None, Some((_, end))) if end > start => end,
        // A picture has no end of its own.
        _ => Some(ffmpeg::duration(input)?).filter(|&d| d < ffmpeg::PICTURE_MOST).unwrap_or(start + ffmpeg::PICTURE_SECONDS),
    };
    if end <= start {
        bail!("nothing to render between {start} and {end} s");
    }
    Ok((start, end))
}

/// Decoded lens frames in order, from the lens tracks or the proxy.
pub(crate) enum Stream {
    Tracks(Vec<ffmpeg::Decoder>),
    Proxy(ffmpeg::Decoder, Vec<u32>),
}

impl Stream {
    /// One frame per lens; `None` once the clip ends.
    pub(crate) fn next(&mut self) -> Result<Option<Vec<stitch::Frame>>> {
        match self {
            Stream::Tracks(decoders) => decoders.iter_mut().map(|d| d.next()).collect::<Result<Option<Vec<_>>>>(),
            Stream::Proxy(decoder, tracks) => Ok(decoder.next()?.map(|f| ffmpeg::split_proxy(&f, tracks))),
        }
    }
}

/// Stitches `total` frames (or to the end) from `start` into `encoder`,
/// framing each as `shot` says for its time. If the clip ends early, the
/// last frame is repeated up to `total`, so the sound stays in step.
pub(crate) fn encode(p: &Pipeline, start: f64, total: Option<usize>, encoder: &mut dyn ffmpeg::FrameSink, shot: impl Fn(f64) -> Shot, tick: &mut dyn FnMut(usize) -> Result<()>) -> Result<()> {
    let mut stream = p.stream(start)?;
    let mut carry = Carry::default();
    let mut last = None;
    let mut n = 0;
    while total.is_none_or(|total| n < total) {
        let Some(frames) = stream.next()? else { break };
        let t = start + n as f64 / p.fps;
        let frame = p.render(&frames, t, &mut carry, &shot(t));
        encoder.write(&frame)?;
        last = Some(frame);
        n += 1;
        tick(n)?;
    }
    if let Some(total) = total {
        // A cut with no frames would write none while its sound still
        // counted, and every later cut would run behind its sound.
        let last = last.context("no frames decoded")?;
        p.warn_held(total - n);
        for k in n..total {
            encoder.write(&last)?;
            tick(k + 1)?;
        }
    }
    Ok(())
}

/// Grades a rendered frame on the CPU as the shader's `finish` does on the
/// GPU: its LUT, then its effects.
pub(crate) fn grade_frame(frame: &mut stitch::Frame, camera: &view::Camera) {
    let (grade, lut) = (&camera.grade, camera.lut.map(|id| (lut::get(id), camera.grade.lut.clamp(0.0, 1.0))));
    if grade.is_neutral() && lut.is_none() {
        return;
    }
    let (w, h) = (frame.width, frame.height);
    let aspect = w as f64 / h as f64;
    frame.rgb.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for (x, px) in row.chunks_exact_mut(3).enumerate() {
            // Pixel centres, as the shader's fragment positions.
            let at = [(x as f64 + 0.5) / w as f64 * 2.0 - 1.0, (y as f64 + 0.5) / h as f64 * 2.0 - 1.0];
            let mut rgb = [px[0], px[1], px[2]].map(|v| v as f64 / 255.0);
            if let Some((lut, strength)) = &lut {
                rgb = lut.blend(rgb, *strength);
            }
            let out = if grade.is_neutral() { rgb } else { grade.apply(rgb, at, aspect) };
            for (p, v) in px.iter_mut().zip(out) {
                *p = (v * 255.0).round() as u8;
            }
        }
    });
}

pub(crate) fn load_input(input: &Path, progress: &dyn Fn(f64)) -> Result<telemetry_parser::Input> {
    let mut f = File::open(input)?;
    let size = f.metadata()?.len() as usize;
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    Ok(telemetry_parser::Input::from_stream(&mut f, size, input, progress, cancel)?)
}
