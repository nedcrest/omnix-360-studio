//! A join's transition: how one cut gives way to the next across their
//! crossfade (`Cut::crossfade`, its length). The looks are the library the
//! user chose from (2026-11-08, `docs/omnix/TRANSITIONS.md`): non-banal
//! ones, in groups. None set: the plain dissolve.

use serde::{Deserialize, Serialize};

/// A transition's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Look {
    WhipPan,
    ZoomPunch,
    SpinBlur,
    Flash,
    LightLeak,
    LumaFade,
    Glitch,
    SphereWhip,
}

/// The library's groups, in the order the panel shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Camera,
    Light,
    Flat,
    Sphere,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Camera, Group::Light, Group::Flat, Group::Sphere];

    pub fn name(self) -> &'static str {
        match self {
            Group::Camera => "Camera moves",
            Group::Light => "Light and film",
            Group::Flat => "Picture",
            Group::Sphere => "360",
        }
    }
}

impl Look {
    pub const ALL: [Look; 8] = [Look::WhipPan, Look::ZoomPunch, Look::SpinBlur, Look::Flash, Look::LightLeak, Look::LumaFade, Look::Glitch, Look::SphereWhip];

    pub fn name(self) -> &'static str {
        match self {
            Look::WhipPan => "Whip pan",
            Look::ZoomPunch => "Zoom punch",
            Look::SpinBlur => "Spin blur",
            Look::Flash => "Flash",
            Look::LightLeak => "Light leak",
            Look::LumaFade => "Luma fade",
            Look::Glitch => "Glitch",
            Look::SphereWhip => "Sphere whip",
        }
    }

    /// What it looks like, for its tile's hint.
    pub fn about(self) -> &'static str {
        match self {
            Look::WhipPan => "A fast pan with motion blur; the cut hides in the blur",
            Look::ZoomPunch => "Into the picture with a radial blur, out of the next",
            Look::SpinBlur => "The frame spins faster, the next takes over mid-spin",
            Look::Flash => "A quick wash of light with blown highlights",
            Look::LightLeak => "Warm light glows wash over, the next under them",
            Look::LumaFade => "The next shows first through the bright parts",
            Look::Glitch => "Torn strips and shifted colours",
            Look::SphereWhip => "The view spins round the sphere into the next 360 clip",
        }
    }

    pub fn group(self) -> Group {
        match self {
            Look::WhipPan | Look::ZoomPunch | Look::SpinBlur => Group::Camera,
            Look::Flash | Look::LightLeak => Group::Light,
            Look::LumaFade | Look::Glitch => Group::Flat,
            Look::SphereWhip => Group::Sphere,
        }
    }
}

/// A transition's length steps (TRANSITIONS.md), and its length when one
/// is put on a join with none.
pub const STEP: f64 = 0.25;
pub const DEFAULT: f64 = 0.5;

/// `seconds` on the steps, one step at least.
pub fn stepped(seconds: f64) -> f64 {
    ((seconds / STEP).round() * STEP).max(STEP)
}

/// How many samples a blur takes along its move (gl-transitions' 24).
const SAMPLES: usize = 24;

impl Look {
    /// Its number in the shader (`transition.wgsl`'s `look`); 0 the
    /// dissolve, which the looks not built yet show as (the panel says so).
    pub fn code(look: Option<Look>) -> u32 {
        match look {
            Some(Look::WhipPan) => 1,
            Some(Look::ZoomPunch) => 2,
            Some(Look::SpinBlur) => 3,
            Some(Look::Flash) => 4,
            _ => 0,
        }
    }
}

/// A transition's picture at `uv` (0..1 across the view, y down), `p` of
/// the way through, from the outgoing picture (`from`) and the incoming
/// (`to`), each read at any point (clamped to its edges); `aspect` the
/// view's width over its height, `pixel` seeding the jitter that hides a
/// blur's sample steps. The shader's `transition` (`transition.wgsl`) is
/// this, line for line. The looks are ports of gl-transitions (MIT):
/// WhipPan and WhipSpin by gre, CrossZoom by rectalogic, Overexposure by
/// Ben Zhang, at their default parameters.
pub fn colour(code: u32, p: f32, uv: [f32; 2], aspect: f32, pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3], to: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let clamp01 = |q: [f32; 2]| [q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0)];
    match code {
        // Whip pan: both pictures slide left, streaked along the move; the
        // incoming arrives from the right. Slow, whip, slow.
        1 => {
            let d = [-1.0f32, 0.0];
            let h = 1.0 - (2.0 * p - 1.0).abs();
            let e = 0.5 * h * (0.1 + 0.9 * h * h);
            let pos = if p < 0.5 { e } else { 1.0 - e };
            let len = 0.4 * h * (0.1 + 2.7 * h * h) / 2.8;
            let q = [uv[0] - d[0] * pos, uv[1] - d[1] * pos];
            let jitter = hash(pixel, 1);
            let mut sum = [0.0f32; 3];
            for i in 0..SAMPLES {
                let t = (i as f32 + jitter) / SAMPLES as f32 - 0.5;
                let s = [q[0] + d[0] * t * len, q[1] + d[1] * t * len];
                let c = if (s[0] - 0.5) * -d[0] + (s[1] - 0.5) * -d[1] < 0.5 * (d[0] * d[0] + d[1] * d[1]) { from(clamp01(s)) } else { to(clamp01([s[0] + d[0], s[1] + d[1]])) };
                sum = [0, 1, 2].map(|k| sum[k] + c[k]);
            }
            sum.map(|v| v / SAMPLES as f32)
        }
        // Zoom punch: a crossfade through a strong zoom blur, its centre
        // drifting across the middle.
        2 => {
            let strength = 0.4;
            let centre = [0.25 + 0.5 * p, 0.5];
            let dissolve = expo_in_out(p);
            let s = strength / 2.0 * (1.0 - (2.0 * std::f32::consts::PI * p).cos());
            let to_centre = [centre[0] - uv[0], centre[1] - uv[1]];
            let offset = hash(pixel, 2);
            let (mut sum, mut total) = ([0.0f32; 3], 0.0);
            for t in 0..=40 {
                let percent = (t as f32 + offset) / 40.0;
                let weight = 4.0 * (percent - percent * percent);
                let at = clamp01([uv[0] + to_centre[0] * percent * s, uv[1] + to_centre[1] * percent * s]);
                let c = mix(from(at), to(at), dissolve);
                sum = [0, 1, 2].map(|k| sum[k] + c[k] * weight);
                total += weight;
            }
            sum.map(|v| v / total)
        }
        // Spin blur: the frame rolls faster round the middle, the incoming
        // takes over mid-spin and slows to rest; a little zoom, the
        // channels splitting with the speed.
        3 => {
            let (turns, blur, chroma, zoom) = (1.0f32, 0.5f32, 0.6f32, 0.25f32);
            let h = 1.0 - (2.0 * p - 1.0).abs();
            let shutter = blur * 0.1 * h;
            let scale = 1.0 + zoom * h * h * h;
            let q = [(uv[0] - 0.5) * aspect, uv[1] - 0.5];
            let jitter = hash(pixel, 3);
            let (mut sum, mut weight) = ([0.0f32; 3], [0.0f32; 3]);
            for i in 0..SAMPLES {
                let s = (i as f32 + jitter) / SAMPLES as f32 - 0.5;
                let t = (p + s * shutter).clamp(0.0, 1.0);
                let half = std::f32::consts::PI * turns;
                let a = if t < 0.5 { half * (2.0 * t).powi(3) } else { -half * (2.0 - 2.0 * t).powi(3) };
                let (c, sn) = (a.cos(), a.sin());
                let r = [(c * q[0] + sn * q[1]) / scale, (-sn * q[0] + c * q[1]) / scale];
                let suv = [mirror(r[0] / aspect + 0.5), mirror(r[1] + 0.5)];
                let col = if t < 0.5 { from(suv) } else { to(suv) };
                let w = [(1.0 - 4.0 * chroma * s).max(0.0), 1.0, (1.0 + 4.0 * chroma * s).max(0.0)];
                for k in 0..3 {
                    sum[k] += col[k] * w[k];
                    weight[k] += w[k];
                }
            }
            [0, 1, 2].map(|k| sum[k] / weight[k].max(1e-5))
        }
        // Flash: both pictures pushed past white at the middle.
        4 => {
            let lift = (std::f32::consts::PI * p).sin() * 0.6;
            let (a, b) = (from(uv), to(uv));
            [0, 1, 2].map(|k| a[k] * (1.0 - p + lift) + b[k] * (p + lift))
        }
        _ => mix(from(uv), to(uv), p),
    }
}

/// gl-transitions' exponential ease in and out over 0..1.
fn expo_in_out(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let t = t * 2.0;
    if t < 1.0 { 0.5 * 2f32.powf(10.0 * (t - 1.0)) } else { 0.5 * (2.0 - 2f32.powf(-10.0 * (t - 1.0))) }
}

/// A coordinate folded back into 0..1, so a turned frame stays filled.
fn mirror(v: f32) -> f32 {
    let m = v.rem_euclid(2.0);
    if m > 1.0 { 2.0 - m } else { m }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * t)
}

/// A number in 0..1 for a pixel and a salt, the same on the GPU (`hash`
/// in the shader): integers only, so no float rounding sets them apart.
pub fn hash(pixel: [u32; 2], salt: u32) -> f32 {
    let mut v = pixel[0].wrapping_mul(1_973).wrapping_add(pixel[1].wrapping_mul(9_277)).wrapping_add(salt.wrapping_mul(26_699)) | 1;
    v ^= v << 13;
    v ^= v >> 17;
    v ^= v << 5;
    (v >> 8) as f32 / 16_777_216.0
}

/// The transition between two finished frames of one size, as the export
/// writes it: each pixel by `colour`, the pictures read bilinearly as the
/// GPU's sampler reads them.
pub fn blend(look: Option<Look>, p: f32, a: &crate::stitch::Frame, b: &crate::stitch::Frame) -> crate::stitch::Frame {
    let (w, h, deep) = (a.width, a.height, a.deep);
    let (code, aspect) = (Look::code(look), w as f32 / h as f32);
    let scale = if deep { 65_535.0 } else { 255.0 };
    let get = |f: &crate::stitch::Frame, x: usize, y: usize, c: usize| -> f32 {
        let i = (y * w + x) * 3 + c;
        if deep { u16::from_le_bytes([f.rgb[2 * i], f.rgb[2 * i + 1]]) as f32 / scale } else { f.rgb[i] as f32 / scale }
    };
    let sample = |f: &crate::stitch::Frame, uv: [f32; 2]| -> [f32; 3] {
        let x = (uv[0] * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
        let y = (uv[1] * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1, fx, fy) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1), x.fract(), y.fract());
        [0, 1, 2].map(|c| {
            let top = get(f, x0, y0, c) + (get(f, x1, y0, c) - get(f, x0, y0, c)) * fx;
            let bot = get(f, x0, y1, c) + (get(f, x1, y1, c) - get(f, x0, y1, c)) * fx;
            top + (bot - top) * fy
        })
    };
    let (from, to) = (|uv: [f32; 2]| sample(a, uv), |uv: [f32; 2]| sample(b, uv));
    let mut rgb = Vec::with_capacity(a.rgb.len());
    for y in 0..h {
        for x in 0..w {
            let uv = [(x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32];
            for v in colour(code, p, uv, aspect, [x as u32, y as u32], &from, &to) {
                let v = (v.clamp(0.0, 1.0) * scale).round();
                if deep {
                    rgb.extend((v as u16).to_le_bytes());
                } else {
                    rgb.push(v as u8);
                }
            }
        }
    }
    crate::stitch::Frame { width: w, height: h, rgb, deep }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_in_a_group_and_lengths_go_by_steps() {
        assert!(Group::ALL.iter().all(|g| Look::ALL.iter().any(|l| l.group() == *g)));
        assert_eq!((stepped(0.6), stepped(0.0), stepped(1.13)), (0.5, 0.25, 1.25));
        assert_eq!(serde_json::to_string(&Look::WhipPan).unwrap(), "\"whip_pan\"");
    }

    #[test]
    fn each_look_starts_on_the_outgoing_and_ends_on_the_incoming() {
        let (a, b) = (|_: [f32; 2]| [0.2, 0.4, 0.6], |_: [f32; 2]| [0.9, 0.1, 0.5]);
        for code in 0..=4 {
            for (p, want) in [(0.0, [0.2, 0.4, 0.6]), (1.0, [0.9, 0.1, 0.5])] {
                let got = colour(code, p, [0.3, 0.7], 16.0 / 9.0, [5, 9], &a, &b);
                assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-3), "look {code} at {p}: {got:?}");
            }
        }
        // A flat picture stays flat through a blur's middle; the flash
        // goes past white.
        let mid = colour(1, 0.5, [0.5, 0.5], 1.0, [3, 3], &a, &a);
        assert!(mid.iter().zip([0.2, 0.4, 0.6]).all(|(g, w)| (g - w).abs() < 1e-3), "{mid:?}");
        assert!(colour(4, 0.5, [0.5, 0.5], 1.0, [0, 0], &a, &b)[0] > 0.9);
    }
}
