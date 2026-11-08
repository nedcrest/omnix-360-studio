//! A cut's style look (gap 87, with 26's glow and soft focus; user,
//! 2026-11-09): old film, VHS, neo-cyberpunk, glow, soft focus, put over
//! the cut's graded picture. Each reads the picture around a pixel, so it
//! is a pass over the finished picture (the transition pass's, one
//! picture), not a value in the per-pixel grade. Living ones (grain,
//! scratches, tracking lines) change with the clip's time `t`.
//!
//! `transition::colour` calls these by the look's code; `style.wgsl` is
//! each, line for line (the player's twin), noise only from the integer
//! `transition::hash`. Ours, not ports. Every look's first knob is how
//! much of it (0: the picture as it is).

use crate::transition::{Values, disc, hash, luma, mix, tear};

type Picture<'a> = &'a dyn Fn([f32; 2]) -> [f32; 3];

fn clamp01(q: [f32; 2]) -> [f32; 2] {
    [q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0)]
}

/// Old film: the gate's weave, a faded sepia, flicker, darker corners,
/// vertical scratches and dust, grain; at 24 frames a second.
pub fn old_film(v: &Values, t: f32, uv: [f32; 2], pixel: [u32; 2], from: Picture) -> [f32; 3] {
    let [amount, grain, scratches, flicker, fade, weave] = [v[0], v[1], v[2], v[3], v[4], v[5]];
    let frame = (t * 24.0).floor() as u32;
    let wob = [(hash([frame, 0], 20) - 0.5) * 0.006 * weave, (hash([frame, 1], 20) - 0.5) * 0.004 * weave];
    let c = from(uv);
    let moved = from(clamp01([uv[0] + wob[0], uv[1] + wob[1]]));
    let grey = luma(moved);
    let mut o = mix(moved, [grey * 1.07 + 0.03, grey * 0.95 + 0.02, grey * 0.75 + 0.01], fade);
    let dim = (1.0 + (hash([frame, 2], 20) - 0.5) * 0.16 * flicker) * (1.0 - 0.7 * ((uv[0] - 0.5).powi(2) + (uv[1] - 0.5).powi(2)));
    o = o.map(|x| x * dim);
    for k in 0..3u32 {
        let x = hash([frame, k + 3], 21);
        if hash([frame, k + 3], 22) < scratches * 0.8 && (uv[0] - x).abs() < 0.0012 {
            o = mix(o, [0.9, 0.88, 0.8], 0.6);
        }
    }
    if hash([pixel[0] / 3, pixel[1] / 3], 23 + frame) > 1.0 - 0.002 * scratches {
        o = o.map(|x| x * 0.3);
    }
    let g = (hash(pixel, 30 + frame) - 0.5) * 0.18 * grain;
    mix(c, o.map(|x| x + g), amount)
}

/// VHS: a tracking band rolling down, jittered lines, the colour bled
/// sideways under a sharper brightness, lifted blacks, scanlines, noise;
/// at 30 frames a second.
pub fn vhs(v: &Values, t: f32, uv: [f32; 2], pixel: [u32; 2], from: Picture) -> [f32; 3] {
    let [amount, bleed, tracking, noise] = [v[0], v[1], v[2], v[3]];
    let frame = (t * 30.0).floor() as u32;
    let row = (uv[1] * 240.0).floor() as u32;
    let in_band = ((uv[1] - (t * 0.13).fract()).abs() < 0.04) as u32 as f32;
    let x = uv[0] + in_band * (hash([row, frame], 24) - 0.5) * 0.03 * tracking + (hash([row, frame], 25) - 0.5) * 0.002 * noise;
    let c = from(uv);
    let sharp = from(clamp01([x, uv[1]]));
    let mut soft = [0.0f32; 3];
    for i in 0..5 {
        let s = from(clamp01([x + (i as f32 - 2.0) * 0.003 * bleed + 0.004 * bleed, uv[1]]));
        soft = [0, 1, 2].map(|k| soft[k] + s[k] / 5.0);
    }
    let lift = luma(sharp) - luma(soft);
    let mut o = soft.map(|s| s + lift);
    o = mix(o, [luma(o); 3], 0.15).map(|s| s * 0.9 + 0.05);
    if pixel[1] % 2 == 1 {
        o = o.map(|s| s * 0.92);
    }
    let n = (hash(pixel, 40 + frame) - 0.5) * 0.12 * noise * (1.0 + 2.0 * in_band * tracking);
    mix(c, o.map(|s| s + n), amount)
}

/// Neo-cyberpunk: colours split from the middle out, now and then a tear,
/// shadows pushed teal and highlights magenta with more contrast,
/// scanlines.
pub fn cyberpunk(v: &Values, t: f32, uv: [f32; 2], pixel: [u32; 2], from: Picture) -> [f32; 3] {
    let [amount, neon, split, scanlines, glitch] = [v[0], v[1], v[2], v[3], v[4]];
    let frame = (t * 24.0).floor() as u32;
    let c = from(uv);
    let d = [uv[0] - 0.5, uv[1] - 0.5];
    let at = |k: f32| clamp01([0.5 + d[0] * (1.0 + k * 0.012 * split), 0.5 + d[1] * (1.0 + k * 0.012 * split)]);
    let mut base = [from(at(1.0))[0], c[1], from(at(-1.0))[2]];
    // A tear on about one burst in four, a quarter second each.
    let burst = if hash([frame / 6, 0], 26) < 0.25 { glitch } else { 0.0 };
    if burst > 0.0 {
        base = tear(36.0, burst, 0.0, frame, uv, pixel, from, from);
    }
    let l = luma(base);
    let s = ((l - 0.2) / 0.6).clamp(0.0, 1.0);
    let tint = mix([0.1, 0.9, 1.0], [1.0, 0.3, 0.9], s * s * (3.0 - 2.0 * s));
    let mut o = [0, 1, 2].map(|k| ((base[k] * (1.0 - 0.5 * neon) + tint[k] * l * neon) - 0.5) * (1.0 + 0.3 * neon) + 0.5);
    if pixel[1].is_multiple_of(3) {
        o = o.map(|x| x * (1.0 - 0.3 * scanlines));
    }
    mix(c, o, amount)
}

/// Glow: the parts brighter than the threshold, blurred, added back.
pub fn glow(v: &Values, uv: [f32; 2], aspect: f32, pixel: [u32; 2], from: Picture) -> [f32; 3] {
    let [strength, radius, threshold] = [v[0], v[1], v[2]];
    let c = from(uv);
    let bright = disc(0.005 + 0.04 * radius, 27, uv, aspect, pixel, &|q| from(q).map(|x| (x - threshold).max(0.0)));
    [0, 1, 2].map(|k| c[k] + bright[k] * strength * 2.0)
}

/// Soft focus: the picture blurred into itself, its highlights blooming.
pub fn soft_focus(v: &Values, uv: [f32; 2], aspect: f32, pixel: [u32; 2], from: Picture) -> [f32; 3] {
    let [amount, radius] = [v[0], v[1]];
    let c = from(uv);
    let b = disc(0.003 + 0.03 * radius, 28, uv, aspect, pixel, from);
    [0, 1, 2].map(|k| c[k] + (b[k] - c[k]) * amount * 0.7 + (b[k] - c[k]).max(0.0) * amount * 0.3)
}
