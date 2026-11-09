//! Export on the GPU: the player's stitch shader (`gpu.rs`) rendering into
//! an off-screen texture on a wgpu device of its own, read back for the
//! encoder. The CPU stitcher in `stitch.rs` stays the reference (`export
//! --cpu`), and the two must match.
//!
//! The work overlaps in stages, each on its own thread: ffmpeg decodes, one
//! thread samples each frame's seam, the next matches it against the frame
//! before and works out the attitude, and the GPU renders a frame while the
//! last one is read back and encoded. Done one after another on 0037 at
//! 1080p the steps took 22 ms (seam), 14 ms (upload), 10 ms (GPU) and 6 ms
//! (conversion) a frame, and the export ran at 11 fps; overlapped, with
//! CUDA's NV12 decode, 21 fps.

use super::gpu::{self, Rig, SeamFix, SeamSample, Shown};
use super::source::{self, Frame, Kind};
use crate::ffmpeg::{self, Hwaccel};
use crate::stitch::{self, M3};
use crate::view::Camera;
use anyhow::{Context, Result, bail};
use iced::Rectangle;
use iced::wgpu;
use rayon::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel, sync_channel};

/// Display-encoded RGBA, as the shader writes it for a non-sRGB target; for
/// a 10-bit export 16-bit floats (a core format, blending included; its 11
/// bits of precision near white are half a 10-bit step).
fn format(deep: bool) -> wgpu::TextureFormat {
    if deep { wgpu::TextureFormat::Rgba16Float } else { wgpu::TextureFormat::Rgba8Unorm }
}
/// Frames in flight on the GPU: one renders while the other is read back.
const IN_FLIGHT: usize = 2;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    stitcher: gpu::Pipeline,
    size: (usize, usize),
    targets: Vec<Target>,
    /// Submitted frames, oldest first: the target each renders into, and
    /// word of its readback being mapped.
    pending: VecDeque<(usize, Receiver<Result<(), wgpu::BufferAsyncError>>)>,
    next: usize,
    /// Frames submitted, numbering the dither noise so it changes each frame.
    frames: u32,
    /// Renders and hands back 16-bit frames (`stitch::Frame::deep`).
    deep: bool,
    /// The adapter's name, for the log.
    pub name: String,
}

/// What a frame renders into, and the buffer it is read back through.
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    /// Bytes per row in `readback`, padded as copies require.
    row: usize,
}

impl Gpu {
    /// A device of our own on the fastest GPU. A software rasteriser
    /// doesn't count: the CPU stitcher is quicker than that.
    /// `deep`: 16-bit frames, for a 10-bit export.
    pub fn new(deep: bool) -> Result<Gpu> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let options = wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() };
        let adapter = block_on(instance.request_adapter(&options)).context("no GPU adapter")?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            bail!("only a software renderer ({})", info.name);
        }
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("export"), ..Default::default() }))?;
        let stitcher = gpu::Pipeline::create(&device, format(deep));
        Ok(Gpu { device, queue, stitcher, size: (0, 0), targets: Vec::new(), pending: VecDeque::new(), next: 0, frames: 0, deep, name: info.name })
    }

    /// Starts rendering `shown` through `camera` at `size`; `finish` hands
    /// the frames back in order. At most `IN_FLIGHT` may be waiting, so
    /// `finish` one first when that many are.
    pub fn submit(&mut self, rig: &Rig, shown: &Shown, camera: &Camera, views: &[M3], (w, h): (usize, usize)) {
        assert!(self.pending.len() < IN_FLIGHT, "finish a frame before submitting another");
        if self.size != (w, h) {
            assert!(self.pending.is_empty(), "the size changed with frames in flight");
            self.targets = (0..IN_FLIGHT).map(|_| self.make_target(w, h)).collect();
            self.size = (w, h);
        }
        let slot = self.next;
        self.next = (self.next + 1) % IN_FLIGHT;
        // A 16-bit target keeps the precision itself.
        self.stitcher.dither = (!self.deep).then_some(self.frames);
        self.frames = self.frames.wrapping_add(1);
        let target = &self.targets[slot];
        self.stitcher.prepare_frame(&self.device, &self.queue, rig, shown, camera, views, [w as f32, h as f32], [0.0, 0.0]);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.stitcher.draw(&mut encoder, &target.view, &Rectangle { x: 0, y: 0, width: w as u32, height: h as u32 });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &target.readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(target.row as u32), rows_per_image: Some(h as u32) },
            },
            wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = channel();
        target.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.pending.push_back((slot, rx));
    }

    /// The oldest submitted frame as RGB, waiting for the GPU if need be;
    /// `None` if none is in flight.
    pub fn finish(&mut self) -> Result<Option<stitch::Frame>> {
        let Some((slot, mapped)) = self.pending.pop_front() else { return Ok(None) };
        let (w, h) = self.size;
        let target = &self.targets[slot];
        // Callbacks fire only from a poll; wait until this one has.
        let result = loop {
            match mapped.try_recv() {
                Ok(r) => break r,
                Err(_) => {
                    self.device.poll(wgpu::PollType::wait_indefinitely())?;
                }
            }
        };
        result.context("reading the frame back from the GPU")?;
        let data = target.readback.slice(..).get_mapped_range();
        let rgb = if self.deep {
            let mut rgb = vec![0; w * h * 6];
            rgb.par_chunks_mut(w * 6).zip(data.par_chunks(target.row)).for_each(|(out, row)| {
                for (o, px) in out.as_chunks_mut::<6>().0.iter_mut().zip(row.as_chunks::<8>().0) {
                    for c in 0..3 {
                        let v = half(u16::from_le_bytes([px[2 * c], px[2 * c + 1]]));
                        o[2 * c..2 * c + 2].copy_from_slice(&((v.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes());
                    }
                }
            });
            rgb
        } else {
            let mut rgb = vec![0; w * h * 3];
            rgb.par_chunks_mut(w * 3).zip(data.par_chunks(target.row)).for_each(|(out, row)| {
                for (o, px) in out.as_chunks_mut::<3>().0.iter_mut().zip(row.as_chunks::<4>().0) {
                    o.copy_from_slice(&px[..3]);
                }
            });
            rgb
        };
        drop(data);
        target.readback.unmap();
        Ok(Some(stitch::Frame { width: w, height: h, rgb, deep: self.deep }))
    }

    fn make_target(&self, w: usize, h: usize) -> Target {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("export"),
            size: wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format(self.deep),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let row = (w * if self.deep { 8 } else { 4 }).div_ceil(align) * align;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Target { texture, view, readback, row }
    }
}

/// What the GPU needs of a clip besides `crate::Pipeline`.
pub struct Clip {
    /// The GPU's copy of the rig, without the seam corrections the
    /// pipeline was set up without.
    rig: Arc<Rig>,
    /// What decodes the lens tracks: CUDA if it can, which converts to NV12
    /// on the GPU (see `ffmpeg::nv12`), else the pipeline's decoder.
    hw: Hwaccel,
}

impl Clip {
    pub fn new(p: &crate::Pipeline) -> Result<Clip> {
        if p.flat.is_some() {
            return Ok(Clip { rig: Arc::new(Rig::flat(p.tracks[0], p.flat.map_or(crate::hdr::Hdr::None, |f| f.hdr))), hw: p.hw });
        }
        let mut rig = Rig::new(p.lenses.clone(), p.tracks.clone(), p.seam.is_some());
        if p.aligner.is_none() {
            rig.aligner = None;
        }
        let cuda = p.proxy.is_none() && p.hw != Hwaccel::Software && p.hw != Hwaccel::Cuda && Hwaccel::Cuda.works(&p.input, p.tracks[0])?;
        Ok(Clip { rig: Arc::new(rig), hw: if cuda { Hwaccel::Cuda } else { p.hw } })
    }
}

/// A decoded frame with what the shader needs besides the picture.
struct Measured {
    frame: Frame,
    fix: SeamFix,
    views: Vec<M3>,
    t: f64,
}

/// Stitches `total` frames of `p`'s clip from `start` into `encoder` on the
/// GPU, as `crate::encode` does on the CPU: the same frame times, attitude
/// and rolling-shutter steps, and the last frame repeated if the clip ends
/// early so the sound stays in step.
#[allow(clippy::too_many_arguments)]
pub fn encode(
    gpu: &mut Gpu,
    p: &crate::Pipeline,
    clip: &Clip,
    start: f64,
    total: usize,
    encoder: &mut dyn ffmpeg::FrameSink,
    camera: impl Fn(f64) -> Camera,
    size: (usize, usize),
    tick: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<()> {
    let kind = match (&p.flat, &p.proxy) {
        (Some(flat), _) => Kind::Flat { path: p.input.clone(), flat: *flat, size: None, rate: p.rate },
        (None, Some(proxy)) => Kind::Lrf(proxy.clone()),
        (None, None) => Kind::Osv { path: p.input.clone(), tracks: p.tracks.clone(), size: None },
    };
    let stream = source::spawn(kind, clip.hw, start, p.fps)?;
    let failed = stream.error.clone();
    let rig = &clip.rig;
    // Seam and attitude for the next frames while this one renders. The
    // threads end with the stream, or when this returns and drops `measured`.
    let (sampled_tx, sampled) = sync_channel::<(Frame, SeamSample)>(2);
    let sample_rig = rig.clone();
    std::thread::spawn(move || {
        for frame in stream.frames.iter().take(total) {
            let sample = SeamSample::new(&sample_rig, &frame);
            if sampled_tx.send((frame, sample)).is_err() {
                break;
            }
        }
    });
    let (tx, measured) = sync_channel::<Measured>(2);
    let (seam_rig, stab, readout_ms, rs_steps, cut, fps) = (rig.clone(), p.stab.clone(), p.readout_ms, p.rs_steps, p.cut, p.fps);
    std::thread::spawn(move || {
        let mut fix = SeamFix::default();
        for (n, (frame, sample)) in sampled.iter().enumerate() {
            let t = start + n as f64 / fps;
            fix.apply(&seam_rig, sample, n > 0);
            if !cut {
                fix.field.cut.fill(0.0);
            }
            let views = if stab.has_data() { stab.rig_from_view(t * 1000.0, readout_ms, rs_steps) } else { vec![stitch::IDENTITY] };
            if tx.send(Measured { frame, fix: fix.clone(), views, t }).is_err() {
                break;
            }
        }
    });
    let mut last: Option<stitch::Frame> = None;
    let mut done = 0;
    let mut write = |frame: &stitch::Frame| -> Result<usize> {
        encoder.write(frame)?;
        done += 1;
        tick(done)?;
        Ok(done)
    };
    let mut written = 0;
    for m in measured.iter() {
        if gpu.pending.len() == IN_FLIGHT
            && let Some(frame) = gpu.finish()?
        {
            written = write(&frame)?;
            last = Some(frame);
        }
        gpu.submit(rig, &Shown::new(m.frame, m.fix), &camera(m.t), &m.views, size);
    }
    while let Some(frame) = gpu.finish()? {
        written = write(&frame)?;
        last = Some(frame);
    }
    // The clip ran out before the cut did: a decoder that failed says so,
    // else the last frame is held.
    if let Some(why) = failed.lock().unwrap().take() {
        bail!("{}: {why}", p.input.display());
    }
    let last = last.context("no frames decoded")?;
    p.warn_held(total - written);
    for _ in written..total {
        write(&last)?;
    }
    Ok(())
}

/// An IEEE half-precision float's value.
fn half(bits: u16) -> f32 {
    let (sign, exp, man) = ((bits >> 15) as u32, ((bits >> 10) & 0x1f) as u32, (bits & 0x3ff) as u32);
    let magnitude = match exp {
        0 => man as f32 / 1024.0 / 16384.0,
        31 => f32::INFINITY,
        _ => f32::from_bits(((exp + 112) << 23) | (man << 13)),
    };
    if sign == 1 { -magnitude } else { magnitude }
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    iced::futures::executor::block_on(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::{self, Place, Quad};

    /// `w` × `h` 10-bit pictures of one Y′ code a column (`y(x)`) and
    /// these Cb, Cr codes, as P010.
    fn p010(w: usize, h: usize, y: impl Fn(usize) -> u16, cb: u16, cr: u16) -> Frame {
        let mut data = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                data.extend((y(x) << 6).to_le_bytes());
            }
        }
        for _ in 0..w * h / 4 {
            data.extend((cb << 6).to_le_bytes());
            data.extend((cr << 6).to_le_bytes());
        }
        Frame { pts: 0.0, layout: source::Layout::PerTrack, pictures: vec![source::Picture { width: w as u32, height: h as u32, deep: true, data }] }
    }

    /// An ordinary video's `frame` drawn whole into a `w` × `h` target, as
    /// an export draws it.
    fn draw_flat(gpu: &mut Gpu, frame: Frame, hdr: crate::hdr::Hdr, size: (usize, usize)) -> stitch::Frame {
        draw_flat_as(gpu, frame, hdr, size, crate::view::View::home(crate::view::Projection::Perspective).camera())
    }

    /// `draw_flat` through `camera` (its grade and LUT).
    fn draw_flat_as(gpu: &mut Gpu, frame: Frame, hdr: crate::hdr::Hdr, (w, h): (usize, usize), camera: Camera) -> stitch::Frame {
        let shown = Shown::new(frame, SeamFix::default());
        let rig = Rig::flat(0, hdr);
        gpu.size = (0, 0);
        gpu.targets = vec![gpu.make_target(w, h)];
        gpu.stitcher.prepare_frame(&gpu.device, &gpu.queue, &rig, &shown, &camera, &[stitch::IDENTITY], [w as f32, h as f32], [0.0, 0.0]);
        gpu.stitcher.prepare_overlays(&gpu.device, &gpu.queue, &[], [w as f32, h as f32], [0.0, 0.0], [w as f32, h as f32]);
        let target = &gpu.targets[0];
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        gpu.stitcher.draw(&mut encoder, &target.view, &Rectangle { x: 0, y: 0, width: w as u32, height: h as u32 });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(target.row as u32), rows_per_image: Some(h as u32) } },
            wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = channel();
        target.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu.pending.push_back((0, rx));
        gpu.size = (w, h);
        gpu.finish().unwrap().unwrap()
    }

/// Two ordinary videos' frames drawn as a join: `a` going, `b` coming,
    /// mixed by the transition's pass (`look`'s code) `p` of the way through.
    /// `a` and `b` drawn as a join, or `a` alone under an edge look (no `b`).
    fn draw_join(gpu: &mut Gpu, a: Frame, b: Option<Frame>, spec: &crate::transition::Spec, p: f32, styles: [Option<(crate::transition::Spec, f32)>; 2], (w, h): (usize, usize)) -> stitch::Frame {
        let camera = crate::view::View::home(crate::view::Projection::Perspective).camera();
        let (ra, rb) = (Rig::flat(0, crate::hdr::Hdr::None), Rig::flat(0, crate::hdr::Hdr::None));
        gpu.size = (0, 0);
        gpu.targets = vec![gpu.make_target(w, h)];
        let size = [w as f32, h as f32];
        gpu.stitcher.prepare_frame(&gpu.device, &gpu.queue, &ra, &Shown::new(a, SeamFix::default()), &camera, &[stitch::IDENTITY], size, [0.0, 0.0]);
        match b {
            Some(b) => gpu.stitcher.prepare_incoming(&gpu.device, &gpu.queue, &rb, &Shown::new(b, SeamFix::default()), &camera, &[stitch::IDENTITY], size, [0.0, 0.0], spec, p),
            None => gpu.stitcher.prepare_edge(&gpu.device, &gpu.queue, size, [0.0, 0.0], spec, p),
        }
        for (layer, (style, t)) in styles.iter().enumerate().filter_map(|(i, s)| Some((i, s.as_ref()?))) {
            gpu.stitcher.prepare_style(&gpu.queue, layer, style, *t);
        }
        gpu.stitcher.prepare_overlays(&gpu.device, &gpu.queue, &[], size, [0.0, 0.0], size);
        let target = &gpu.targets[0];
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        gpu.stitcher.draw(&mut encoder, &target.view, &Rectangle { x: 0, y: 0, width: w as u32, height: h as u32 });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(target.row as u32), rows_per_image: Some(h as u32) } },
            wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = channel();
        target.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu.pending.push_back((0, rx));
        gpu.size = (w, h);
        gpu.finish().unwrap().unwrap()
    }

    /// A join drawn by the player's transition pass is what the export's
    /// `transition::blend` makes of the two cuts drawn alone: every look,
    /// at the start, the middle and near the end; an edge look of one.
    #[test]
    fn gpu_transitions_match_the_cpu() {
        use crate::transition::{self, Look};
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (64, 32);
        // Stripes: a blur shows on them, as on footage.
        let a = || p010(w, h, |x| if (x / 4) % 2 == 0 { 250 } else { 800 }, 512, 512);
        let b = || p010(w, h, |x| if (x / 6) % 2 == 0 { 900 } else { 350 }, 600, 440);
        let (fa, fb) = (draw_flat(&mut gpu, a(), crate::hdr::Hdr::None, (w, h)), draw_flat(&mut gpu, b(), crate::hdr::Hdr::None, (w, h)));
        // Each built look at its defaults, then with every knob at its far
        // end (a choice's last), through the middle only.
        let built: Vec<Option<Look>> = std::iter::once(None).chain(Look::OVERLAP.into_iter().filter(|&l| Look::code(Some(l)) != 0).map(Some)).collect();
        let far = built.iter().flatten().map(|&l| {
            let mut s = transition::Spec::of(Some(l));
            for (v, k) in s.values.iter_mut().zip(l.knobs()) {
                *v = k.most;
            }
            (s, vec![0.5])
        });
        let cases: Vec<(transition::Spec, Vec<f32>)> = built.iter().map(|&l| (transition::Spec::of(l), vec![0.2, 0.5, 0.8])).chain(far).collect();
        // The edge looks on `a` alone, the same way.
        let edges = Look::EDGES.into_iter().flat_map(|l| {
            let mut far = transition::Spec::of(Some(l));
            for (v, k) in far.values.iter_mut().zip(l.knobs()) {
                *v = k.most;
            }
            [(transition::Spec::of(Some(l)), vec![0.3, 0.7, 1.0]), (far, vec![0.7])]
        });
        for (spec, ps) in cases.into_iter().chain(edges) {
            let (look, edge) = (spec.look, spec.look.is_some_and(Look::is_edge));
            for p in ps {
                let got = draw_join(&mut gpu, a(), (!edge).then(b), &spec, p, [None, None], (w, h));
                let want = transition::blend(&spec, p, &fa, if edge { &fa } else { &fb });
                let diffs: Vec<i32> = got.rgb.iter().zip(&want.rgb).map(|(&g, &c)| (g as i32 - c as i32).abs()).collect();
                let off = diffs.iter().filter(|&&d| d > 3).count();
                eprintln!("{spec:?} at {p}: most {}, {off} of {} off by more than 3", diffs.iter().max().unwrap(), diffs.len());
                assert!(off * 100 <= diffs.len(), "{look:?} at {p}: {off} samples off");
                // And a look is its own, not the dissolve again.
                if look.is_some() && !edge && p == 0.5 && spec == transition::Spec::of(look) {
                    let plain = transition::blend(&transition::Spec::default(), p, &fa, &fb);
                    let apart = want.rgb.iter().zip(&plain.rgb).filter(|(x, y)| x.abs_diff(**y) > 8).count();
                    assert!(apart * 10 > want.rgb.len(), "{look:?} looks like a dissolve");
                }
            }
        }
    }

    /// A style drawn by the player's passes is what the export's
    /// `transition::blend` makes: each look alone, at its defaults and its
    /// knobs' far ends; under an edge look; on both cuts of a join.
    #[test]
    fn gpu_styles_match_the_cpu() {
        use crate::transition::{Spec, Look, blend};
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (64, 32);
        let a = || p010(w, h, |x| if (x / 4) % 2 == 0 { 250 } else { 800 }, 512, 512);
        let b = || p010(w, h, |x| if (x / 6) % 2 == 0 { 900 } else { 350 }, 600, 440);
        let (fa, fb) = (draw_flat(&mut gpu, a(), crate::hdr::Hdr::None, (w, h)), draw_flat(&mut gpu, b(), crate::hdr::Hdr::None, (w, h)));
        let close = |got: &stitch::Frame, want: &stitch::Frame, what: &str| {
            let off = got.rgb.iter().zip(&want.rgb).filter(|(g, c)| g.abs_diff(**c) > 3).count();
            assert!(off * 100 <= got.rgb.len(), "{what}: {off} samples off");
        };
        for look in Look::STYLES {
            let mut far = Spec::of(Some(look));
            for (v, k) in far.values.iter_mut().zip(look.knobs()) {
                *v = k.most;
            }
            for style in [Spec::of(Some(look)), far] {
                let got = draw_join(&mut gpu, a(), None, &Spec::default(), 0.0, [Some((style, 1.3)), None], (w, h));
                close(&got, &blend(&style, 1.3, &fa, &fa), &format!("{style:?}"));
            }
        }
        let (film, edge) = (Spec::of(Some(Look::OldFilm)), Spec::of(Some(Look::EdgeBlur)));
        let got = draw_join(&mut gpu, a(), None, &edge, 0.6, [Some((film, 2.0)), None], (w, h));
        let styled = blend(&film, 2.0, &fa, &fa);
        close(&got, &blend(&edge, 0.6, &styled, &styled), "a style under an edge look");
        let (vhs, glow, whip) = (Spec::of(Some(Look::Vhs)), Spec::of(Some(Look::Glow)), Spec::of(Some(Look::WhipPan)));
        let got = draw_join(&mut gpu, a(), Some(b()), &whip, 0.5, [Some((vhs, 0.7)), Some((glow, 0.2))], (w, h));
        close(&got, &blend(&whip, 0.5, &blend(&vhs, 0.7, &fa, &fa), &blend(&glow, 0.2, &fb, &fb)), "styles on both cuts of a join");
    }

    /// Each vignette shape (gap 6) darkens where `Grade::apply` does, its
    /// reach and softness too.
    #[test]
    fn gpu_vignette_shapes_match_the_cpu() {
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (64, 32);
        let mut camera = crate::view::View::home(crate::view::Projection::Perspective).camera();
        for shape in 0..crate::grade::VIGNETTES.len() {
            camera.grade = crate::grade::Grade { vignette: 0.8, vignette_shape: shape as f64, vignette_reach: 0.6, vignette_soft: 0.5, ..Default::default() };
            let got = draw_flat_as(&mut gpu, p010(w, h, |_| 700, 512, 512), crate::hdr::Hdr::None, (w, h), camera);
            let grey = ((700.0 - 64.0) / 876.0f64).clamp(0.0, 1.0);
            for (x, y) in [(1, 1), (32, 1), (32, 30), (1, 16), (32, 16), (60, 25)] {
                let at = [(x as f64 + 0.5) / w as f64 * 2.0 - 1.0, (y as f64 + 0.5) / h as f64 * 2.0 - 1.0];
                let want = (camera.grade.apply([grey; 3], at, w as f64 / h as f64)[0] * 255.0).round() as i32;
                let px = got.rgb[(y * w + x) * 3] as i32;
                assert!((px - want).abs() <= 2, "shape {shape} at ({x}, {y}): GPU {px}, CPU {want}");
            }
        }
    }

    /// The shader brings a 10-bit HLG picture to SDR as `hdr::to_sdr` does.
    #[test]
    fn gpu_hdr_matches_the_cpu() {
        use crate::hdr::{self, Hdr};
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (64, 32);
        // Y′CbCr as 10-bit limited-range codes: grey, sky blue, green, near
        // peak white.
        for [y, cb, cr] in [[502, 512, 512], [600, 640, 440], [420, 420, 470], [900, 512, 512]] {
            let got = draw_flat(&mut gpu, p010(w, h, |_| y, cb, cr), Hdr::Hlg, (w, h));
            let ycc = [(y as f32 - 64.0) / 876.0, (cb as f32 - 512.0) / 896.0, (cr as f32 - 512.0) / 896.0];
            let want = hdr::to_sdr(hdr::bt2020_rgb(ycc), Hdr::Hlg).map(|v| (v * 255.0).round() as i32);
            let mid = ((h / 2) * w + w / 2) * 3;
            let px = [0, 1, 2].map(|c| got.rgb[mid + c] as i32);
            eprintln!("{ycc:?}: GPU {px:?}, CPU {want:?}");
            assert!(px.iter().zip(want).all(|(a, b)| (a - b).abs() <= 2), "GPU {px:?} against CPU {want:?}");
        }
    }

    /// The shader's LUT (a 3D texture, filtered by the hardware) gives what
    /// `Lut::blend` gives, at part strength too.
    #[test]
    fn gpu_lut_matches_the_cpu() {
        let Ok(mut gpu) = Gpu::new(false) else { return };
        // Bent, and mixing the channels, so the interpolation is tested.
        let mut cube = String::from("LUT_3D_SIZE 17\n");
        for b in 0..17 {
            for g in 0..17 {
                for r in 0..17 {
                    let [r, g, b] = [r, g, b].map(|v| v as f64 / 16.0);
                    cube += &format!("{} {} {}\n", r.powf(0.6), 0.2 * r + 0.8 * g * g, (1.0 - b).sqrt());
                }
            }
        }
        let path = std::env::temp_dir().join(format!("omnix-test-{}.cube", std::process::id()));
        std::fs::write(&path, cube).unwrap();
        let id = crate::lut::shared(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let lut = crate::lut::get(id);
        let (w, h) = (64, 32);
        let mut camera = crate::view::View::home(crate::view::Projection::Perspective).camera();
        camera.lut = Some(id);
        camera.grade.lut = 0.7;
        for [y, cb, cr] in [[502, 512, 512], [600, 640, 440], [420, 420, 470], [800, 540, 560]] {
            let got = draw_flat_as(&mut gpu, p010(w, h, |_| y, cb, cr), crate::hdr::Hdr::None, (w, h), camera);
            // As the shader decodes it: BT.709, video range.
            let [yy, u, v] = [(y as f64 - 64.0) / 876.0, (cb as f64 - 512.0) / 896.0, (cr as f64 - 512.0) / 896.0];
            let rgb = [yy + 1.5748 * v, yy - 0.1873 * u - 0.4681 * v, yy + 1.8556 * u].map(|c| c.clamp(0.0, 1.0));
            let want = lut.blend(rgb, 0.7).map(|v| (v * 255.0).round() as i32);
            let mid = ((h / 2) * w + w / 2) * 3;
            let px = [0, 1, 2].map(|c| got.rgb[mid + c] as i32);
            eprintln!("{rgb:?}: GPU {px:?}, CPU {want:?}");
            assert!(px.iter().zip(want).all(|(a, b)| (a - b).abs() <= 2), "GPU {px:?} against CPU {want:?}");
        }
    }

    /// Dithered, a 10-bit grey ramp finer than 8 bits holds its shape in the
    /// 8-bit export: each column's average follows it rather than stepping.
    #[test]
    fn dither_keeps_a_smooth_ramp() {
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (256, 64);
        // 20 codes across: steps of a quarter of an 8-bit level.
        let code = |x: usize| 400 + (x * 20 / w) as u16;
        let mut errors = Vec::new();
        for dither in [None, Some(7)] {
            gpu.stitcher.dither = dither;
            let got = draw_flat(&mut gpu, p010(w, h, code, 512, 512), crate::hdr::Hdr::None, (w, h));
            let mut squares = 0.0;
            for x in 0..w {
                let mean = (0..h).map(|y| got.rgb[(y * w + x) * 3 + 1] as f64).sum::<f64>() / h as f64;
                let want = (code(x) as f64 - 64.0) / 876.0 * 255.0;
                squares += (mean - want).powi(2);
            }
            errors.push((squares / w as f64).sqrt());
        }
        gpu.stitcher.dither = None;
        eprintln!("RMS error of the column averages: {:.3} levels plain, {:.3} dithered", errors[0], errors[1]);
        assert!(errors[0] > 0.2 && errors[1] < 0.12, "{errors:?}");
    }

    /// A 16-bit render (a 10-bit export) keeps a 10-bit ramp's quarter-level
    /// steps, pixel by pixel, where 8 bits round them away.
    #[test]
    fn deep_render_keeps_ten_bits() {
        let Ok(mut gpu) = Gpu::new(true) else { return };
        let (w, h) = (256, 4);
        let code = |x: usize| 400 + (x * 20 / w) as u16;
        let got = draw_flat(&mut gpu, p010(w, h, code, 512, 512), crate::hdr::Hdr::None, (w, h));
        assert!(got.deep);
        let worst = (0..w)
            .map(|x| {
                let i = (2 * w + x) * 6 + 2;
                let g = u16::from_le_bytes([got.rgb[i], got.rgb[i + 1]]) as f64 / 65535.0 * 255.0;
                (g - (code(x) as f64 - 64.0) / 876.0 * 255.0).abs()
            })
            .fold(0.0, f64::max);
        eprintln!("worst error {worst:.3} of an 8-bit level");
        assert!(worst < 0.1, "{worst}");
    }

    /// The player's overlay pass draws what `overlay::composite` does.
    #[test]
    fn gpu_overlays_match_the_cpu() {
        let Ok(mut gpu) = Gpu::new(false) else { return };
        let (w, h) = (320, 180);
        // A mid-grey ordinary video frame (limited-range NV12).
        let mut data = vec![126u8; w * h];
        data.extend(vec![128u8; w * h / 2]);
        let frame = Frame { pts: 0.0, layout: source::Layout::PerTrack, pictures: vec![source::Picture { width: w as u32, height: h as u32, deep: false, data }] };
        let shown = Shown::new(frame, SeamFix::default());
        let rig = Rig::flat(0, crate::hdr::Hdr::None);
        let camera = crate::view::View::home(crate::view::Projection::Perspective).camera();
        // A 4×2 picture: opaque red left, half-transparent green right.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            rgba.extend([255, 0, 0, 255, 255, 0, 0, 255, 0, 128, 0, 128, 0, 128, 0, 128]);
        }
        let picture = Arc::new(overlay::Picture { width: 4, height: 2, rgba, natural: None, order: None });
        let place = Place { x: 0.4, y: 0.6, size: 0.5, rotation: 20.0 };
        let grade = crate::grade::Grade { opacity: 0.8, saturation: -0.5, ..Default::default() };
        let draw = gpu::OverlayDraw { id: 1, picture: picture.clone(), spot: crate::overlay::Spot::Placed(place), grade, black: 0.25 };
        let mut render = |overlays: &[gpu::OverlayDraw]| {
            gpu.size = (0, 0);
            gpu.targets = vec![gpu.make_target(w, h)];
            gpu.stitcher.prepare_frame(&gpu.device, &gpu.queue, &rig, &shown, &camera, &[stitch::IDENTITY], [w as f32, h as f32], [0.0, 0.0]);
            gpu.stitcher.prepare_overlays(&gpu.device, &gpu.queue, overlays, [w as f32, h as f32], [0.0, 0.0], [w as f32, h as f32]);
            let target = &gpu.targets[0];
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            gpu.stitcher.draw(&mut encoder, &target.view, &Rectangle { x: 0, y: 0, width: w as u32, height: h as u32 });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(target.row as u32), rows_per_image: Some(h as u32) } },
                wgpu::Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
            );
            gpu.queue.submit([encoder.finish()]);
            let (tx, rx) = channel();
            target.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            gpu.pending.push_back((0, rx));
            gpu.size = (w, h);
            gpu.finish().unwrap().unwrap()
        };
        let mut want = render(&[]);
        let got = render(&[draw]);
        overlay::composite(&mut want, &picture, &Quad::new(&place, &picture, (w, h)), &grade, 0.25);
        let diff: Vec<i32> = want.rgb.iter().zip(&got.rgb).map(|(a, b)| (*a as i32 - *b as i32).abs()).collect();
        let big = diff.iter().filter(|&&d| d > 8).count();
        let mean = diff.iter().sum::<i32>() as f64 / diff.len() as f64;
        eprintln!("mean diff {mean:.3}, {big} of {} values off by more than 8", diff.len());
        // Edge pixels differ by filtering; the inside must agree.
        assert!(mean < 0.3 && big < 50, "mean {mean}, {big} off");
        // Bars (gap 15): two bands of their colour, half way in.
        let item = overlay::Item { id: 0, input: Default::default(), text: None, at: 0.0, start: 0.0, end: 4.0, motion: vec![], effects: vec![], bars: Some(overlay::Bars { colour: [0.1, 0.2, 0.6], ..Default::default() }), automation: vec![] };
        let (bars, _) = item.picture(0, h).unwrap();
        let bars = Arc::new(bars);
        let spots = overlay::spots(&item, 0.5);
        let draws: Vec<gpu::OverlayDraw> = spots.iter().map(|&(spot, grade)| gpu::OverlayDraw { id: 2, picture: bars.clone(), spot, grade, black: 0.0 }).collect();
        let mut want = render(&[]);
        let got = render(&draws);
        for (spot, grade) in &spots {
            overlay::composite(&mut want, &bars, &spot.quad(&bars, (w, h)), grade, 0.0);
        }
        let big = want.rgb.iter().zip(&got.rgb).filter(|(a, b)| a.abs_diff(**b) > 8).count();
        assert!(big < 4 * w, "bars: {big} values off");
        // Blurred and half wiped in; then typed, its letters' order set.
        let blurred = crate::grade::Grade { blur: 0.7, reveal: 0.5, ..Default::default() };
        let typed = Arc::new(overlay::Picture { order: Some(vec![64, 128, 192, 255, 64, 128, 192, 255]), ..(*picture).clone() });
        for (id, pic, grade) in [(3, picture.clone(), blurred), (4, typed, crate::grade::Grade { reveal: 0.6, ..Default::default() })] {
            let mut want = render(&[]);
            let got = render(&[gpu::OverlayDraw { id, picture: pic.clone(), spot: crate::overlay::Spot::Placed(place), grade, black: 0.0 }]);
            overlay::composite(&mut want, &pic, &Quad::new(&place, &pic, (w, h)), &grade, 0.0);
            let diff: Vec<i32> = want.rgb.iter().zip(&got.rgb).map(|(a, b)| (*a as i32 - *b as i32).abs()).collect();
            let (big, mean) = (diff.iter().filter(|&&d| d > 8).count(), diff.iter().sum::<i32>() as f64 / diff.len() as f64);
            assert!(mean < 0.3 && big < 80, "picture {id}: mean {mean}, {big} off");
        }
    }
}
