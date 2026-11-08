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
    /// Edge looks (gap 25b): on one cut's start or end alone, no overlap.
    EdgeFade,
    EdgeBlur,
    EdgeZoom,
    EdgeGlitch,
    EdgeFlash,
}

/// The library's groups, in the order the panel shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Camera,
    Light,
    Flat,
    Sphere,
    Edge,
}

impl Group {
    /// The overlap's groups (the Edges tab has its looks in one).
    pub const OVERLAP: [Group; 4] = [Group::Camera, Group::Light, Group::Flat, Group::Sphere];

    pub fn name(self) -> &'static str {
        match self {
            Group::Camera => "Camera moves",
            Group::Light => "Light and film",
            Group::Flat => "Picture",
            Group::Sphere => "360",
            Group::Edge => "Edges",
        }
    }
}

impl Look {
    /// The looks across a join's overlap.
    pub const OVERLAP: [Look; 8] = [Look::WhipPan, Look::ZoomPunch, Look::SpinBlur, Look::Flash, Look::LightLeak, Look::LumaFade, Look::Glitch, Look::SphereWhip];
    /// The looks on a cut's edge (`Cut::edge_in`, `edge_out`).
    pub const EDGES: [Look; 5] = [Look::EdgeFade, Look::EdgeBlur, Look::EdgeZoom, Look::EdgeGlitch, Look::EdgeFlash];

    pub fn is_edge(self) -> bool {
        self.group() == Group::Edge
    }

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
            Look::EdgeFade => "Fade",
            Look::EdgeBlur => "Blur",
            Look::EdgeZoom => "Zoom",
            Look::EdgeGlitch => "Glitch",
            Look::EdgeFlash => "Flash",
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
            Look::EdgeFade => "To black or white (from it at a start); to black, the sound fades too",
            Look::EdgeBlur => "Out of focus, or into it at a start",
            Look::EdgeZoom => "A push in or a pull out with a blur along it",
            Look::EdgeGlitch => "Torn strips and shifted colours growing to the end, or settling at a start",
            Look::EdgeFlash => "Blown out to light, or out of it at a start",
        }
    }

    pub fn group(self) -> Group {
        match self {
            Look::WhipPan | Look::ZoomPunch | Look::SpinBlur => Group::Camera,
            Look::Flash | Look::LightLeak => Group::Light,
            Look::LumaFade | Look::Glitch => Group::Flat,
            Look::SphereWhip => Group::Sphere,
            Look::EdgeFade | Look::EdgeBlur | Look::EdgeZoom | Look::EdgeGlitch | Look::EdgeFlash => Group::Edge,
        }
    }
}

/// One of a look's own settings (gap 25g): a slider, or a choice among
/// `choices` held as its index. A join stores only those the user moved
/// (`Cut::transition_params`, by `key`); the rest are `default`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knob {
    pub key: &'static str,
    pub name: &'static str,
    pub least: f32,
    pub most: f32,
    pub default: f32,
    pub step: f32,
    pub choices: &'static [&'static str],
}

const fn slider(key: &'static str, name: &'static str, least: f32, most: f32, default: f32, step: f32) -> Knob {
    Knob { key, name, least, most, default, step, choices: &[] }
}

const fn choice(key: &'static str, name: &'static str, choices: &'static [&'static str]) -> Knob {
    Knob { key, name, least: 0.0, most: (choices.len() - 1) as f32, default: 0.0, step: 1.0, choices }
}

/// The most knobs a look has: the shader's two `vec4f`s.
pub const MOST_KNOBS: usize = 8;

/// A look's knob values in its `knobs` order, as the shader takes them.
pub type Values = [f32; MOST_KNOBS];

impl Knob {
    /// `v` within the range, on the steps.
    pub fn settle(&self, v: f32) -> f32 {
        (((v - self.least) / self.step).round() * self.step + self.least).clamp(self.least, self.most)
    }
}

impl Look {
    /// Its own settings; the defaults are gl-transitions' (the look as
    /// first built).
    pub fn knobs(self) -> &'static [Knob] {
        const WHIP: [Knob; 2] = [choice("direction", "Direction", &["Left", "Right", "Up", "Down"]), slider("streak", "Streak", 0.0, 2.0, 1.0, 0.1)];
        const ZOOM: [Knob; 2] = [slider("strength", "Strength", 0.1, 1.0, 0.4, 0.05), choice("centre", "Centre", &["Drifting", "Still"])];
        const SPIN: [Knob; 5] = [
            slider("turns", "Turns", 0.5, 3.0, 1.0, 0.25),
            choice("direction", "Direction", &["Clockwise", "Anticlockwise"]),
            slider("blur", "Blur", 0.0, 1.0, 0.5, 0.05),
            slider("split", "Colour split", 0.0, 1.0, 0.6, 0.05),
            slider("zoom", "Zoom", 0.0, 1.0, 0.25, 0.05),
        ];
        const FLASH: [Knob; 2] = [slider("strength", "Strength", 0.2, 1.5, 0.6, 0.05), choice("colour", "Colour", &["White", "Warm"])];
        const LEAK: [Knob; 1] = [slider("strength", "Strength", 0.0, 2.0, 1.0, 0.1)];
        const LUMA: [Knob; 2] = [slider("softness", "Softness", 0.05, 1.0, 0.25, 0.05), choice("order", "First", &["Bright parts", "Dark parts"])];
        const GLITCH: [Knob; 2] = [slider("strength", "Strength", 0.0, 2.0, 1.0, 0.1), slider("strips", "Strips", 8.0, 80.0, 36.0, 1.0)];
        const FADE: [Knob; 1] = [choice("colour", "Colour", &["Black", "White"])];
        const BLUR: [Knob; 1] = [slider("strength", "Strength", 0.2, 2.0, 1.0, 0.1)];
        const PUSH: [Knob; 2] = [choice("direction", "Direction", &["In", "Out"]), slider("strength", "Strength", 0.1, 1.0, 0.3, 0.05)];
        const BLOW: [Knob; 1] = [slider("strength", "Strength", 0.2, 1.5, 0.8, 0.05)];
        match self {
            Look::WhipPan => &WHIP,
            Look::ZoomPunch => &ZOOM,
            Look::SpinBlur => &SPIN,
            Look::Flash => &FLASH,
            Look::LightLeak => &LEAK,
            Look::LumaFade => &LUMA,
            Look::Glitch => &GLITCH,
            Look::SphereWhip => &[],
            Look::EdgeFade => &FADE,
            Look::EdgeBlur => &BLUR,
            Look::EdgeZoom => &PUSH,
            Look::EdgeGlitch => &GLITCH,
            Look::EdgeFlash => &BLOW,
        }
    }

    pub fn knob(self, key: &str) -> Option<&'static Knob> {
        self.knobs().iter().find(|k| k.key == key)
    }
}

/// A join's transition as the player and the export draw it: the look
/// and its knobs' values (`Cut::transition_spec`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Spec {
    pub look: Option<Look>,
    pub values: Values,
}

impl Spec {
    /// `look` at its defaults.
    pub fn of(look: Option<Look>) -> Spec {
        Spec { look, values: values(look, &Default::default()) }
    }

    pub fn code(&self) -> u32 {
        Look::code(self.look)
    }
}

/// The values `look` plays with: those `set`, the rest by default.
pub fn values(look: Option<Look>, set: &std::collections::BTreeMap<String, f32>) -> Values {
    let mut v = [0.0; MOST_KNOBS];
    for (slot, k) in v.iter_mut().zip(look.map_or(&[][..], Look::knobs)) {
        *slot = set.get(k.key).map_or(k.default, |&x| k.settle(x));
    }
    v
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
            Some(Look::LightLeak) => 5,
            Some(Look::LumaFade) => 6,
            Some(Look::Glitch) => 7,
            Some(Look::EdgeFade) => 8,
            Some(Look::EdgeBlur) => 9,
            Some(Look::EdgeZoom) => 10,
            Some(Look::EdgeGlitch) => 11,
            Some(Look::EdgeFlash) => 12,
            _ => 0,
        }
    }
}

/// A transition's picture at `uv` (0..1 across the view, y down), `p` of
/// the way through, from the outgoing picture (`from`) and the incoming
/// (`to`), each read at any point (clamped to its edges); `aspect` the
/// view's width over its height, `pixel` seeding the jitter that hides a
/// blur's sample steps. An edge look (`Look::is_edge`) has one picture,
/// `from`, and `p` is how much of it: 0 the picture as it is, 1 all the
/// way (black, out of focus); a cut's start runs it backwards. The
/// shader's `transition` (`transition.wgsl`) is
/// this, line for line. The looks are ports of gl-transitions (MIT):
/// WhipPan and WhipSpin by gre, CrossZoom by rectalogic, Overexposure by
/// Ben Zhang, their parameters the knobs (`Look::knobs`, `spec.values`); light leak after FilmBurn by
/// Anastasia Dunbar and glitch after StripDatamoshGlitch by bread and
/// GlitchMemories by Gunnar Roth, their sine noise put on `hash` (a sine's
/// last digits differ between GPU and CPU); luma fade is ours.
pub fn colour(spec: &Spec, p: f32, uv: [f32; 2], aspect: f32, pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3], to: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let clamp01 = |q: [f32; 2]| [q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0)];
    let v = &spec.values;
    match spec.code() {
        // Whip pan: both pictures slide the knob's way (left by default),
        // streaked along the move; the incoming arrives from the other
        // side. Slow, whip, slow.
        1 => {
            let d = whip_direction(v[0]);
            let h = 1.0 - (2.0 * p - 1.0).abs();
            let e = 0.5 * h * (0.1 + 0.9 * h * h);
            let pos = if p < 0.5 { e } else { 1.0 - e };
            let len = 0.4 * h * (0.1 + 2.7 * h * h) / 2.8 * v[1];
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
            let strength = v[0];
            let centre = if v[1] < 0.5 { [0.25 + 0.5 * p, 0.5] } else { [0.5, 0.5] };
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
            let (turns, way, blur, chroma, zoom) = (v[0], 1.0 - 2.0 * v[1], v[2], v[3], v[4]);
            let h = 1.0 - (2.0 * p - 1.0).abs();
            let shutter = blur * 0.1 * h;
            let scale = 1.0 + zoom * h * h * h;
            let q = [(uv[0] - 0.5) * aspect, uv[1] - 0.5];
            let jitter = hash(pixel, 3);
            let (mut sum, mut weight) = ([0.0f32; 3], [0.0f32; 3]);
            for i in 0..SAMPLES {
                let s = (i as f32 + jitter) / SAMPLES as f32 - 0.5;
                let t = (p + s * shutter).clamp(0.0, 1.0);
                let half = std::f32::consts::PI * turns * way;
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
        // Flash: both pictures pushed past white at the middle, or past a
        // warm white.
        4 => {
            let lift = (std::f32::consts::PI * p).sin() * v[0];
            let tint = mix([1.0, 1.0, 1.0], [1.0, 0.8, 0.55], v[1]);
            let (a, b) = (from(uv), to(uv));
            [0, 1, 2].map(|k| a[k] * (1.0 - p) + b[k] * p + (a[k] + b[k]) * lift * tint[k])
        }
        5 => light_leak(v, p, uv, aspect, from, to),
        6 => {
            // The incoming shows first where the outgoing is bright (or
            // dark): each pixel turns when the edge, sweeping down the
            // brightness, passes its own.
            let (a, b) = (from(uv), to(uv));
            let soft = v[0];
            let luma = 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2];
            let luma = if v[1] < 0.5 { luma } else { 1.0 - luma };
            let w = ((p * (1.0 + soft) - (1.0 - luma)) / soft).clamp(0.0, 1.0);
            mix(a, b, w * w * (3.0 - 2.0 * w))
        }
        7 => glitch(v, p, uv, pixel, from, to),
        8 => {
            let c = if v[0] < 0.5 { 0.0 } else { 1.0 };
            mix(from(uv), [c; 3], p)
        }
        9 => edge_blur(v, p, uv, aspect, pixel, from),
        10 => edge_zoom(v, p, uv, pixel, from),
        11 => tear(v[1], p.sqrt() * v[0], 0.0, (p * 24.0).floor() as u32, uv, pixel, from, from),
        12 => {
            let lift = p * v[0];
            from(uv).map(|c| c * (1.0 + 2.0 * lift) + lift * lift)
        }
        _ => mix(from(uv), to(uv), p),
    }
}

/// Light leak: the pictures switch fast at the middle under warm glows
/// that drift across and fade with it.
fn light_leak(v: &Values, p: f32, uv: [f32; 2], aspect: f32, from: &dyn Fn([f32; 2]) -> [f32; 3], to: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let glow = (std::f32::consts::PI * p).sin().max(0.0);
    let strength = v[0];
    let base = mix(from(uv), to(uv), sigmoid(p, 10.0));
    let mut f = 0.0;
    for i in 0..6u32 {
        let r = [0, 1, 2, 3].map(|k| hash([i, k], 5));
        let centre = [r[0] * 1.4 - 0.2 + (r[1] - 0.5) * p, r[2]];
        let radius = 0.15 + 0.35 * r[3];
        let d = [(uv[0] - centre[0]) * aspect, uv[1] - centre[1]];
        f += (-(d[0] * d[0] + d[1] * d[1]) / (radius * radius)).exp();
        f += 0.06 * (1.0 + (uv[0] * 6.0 * r[1] + p * 8.0 + r[2] * 6.0).sin() * (uv[1] * 6.0 * r[3] + p * 6.0 + r[0] * 6.0).sin());
    }
    let f = f / 2.2;
    let warm = [f, (f * 0.7).powf(2.0 - glow), (f * 0.6).powf(1.3)];
    [0, 1, 2].map(|k| base[k] + warm[k] * glow * strength)
}

/// Glitch: strips of rows torn sideways, some showing the other picture,
/// their colours split (the rest's a little), a grain over it, all strongest at the middle;
/// the tear changes 24 times through.
fn glitch(v: &Values, p: f32, uv: [f32; 2], pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3], to: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let burst = (std::f32::consts::PI * p).sin().max(0.0).sqrt() * v[0];
    tear(v[1], burst, smoothstep(0.3, 0.7, p), (p * 24.0).floor() as u32, uv, pixel, from, to)
}

/// The glitch's tearing, `burst` strong, over `strips` rows; `w` of the way
/// from `from` to `to`, a torn strip the other way round; `frame` picks
/// the tear (the glitch and its edge look both).
#[allow(clippy::too_many_arguments)]
fn tear(strips: f32, burst: f32, w: f32, frame: u32, uv: [f32; 2], pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3], to: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let row = (uv[1] * strips).floor() as u32;
    let torn = hash([row, frame], 7) < 0.35 * burst;
    let shift = if torn { (hash([row, frame], 8) - 0.5) * 0.25 * burst } else { 0.0 };
    let w = if torn && hash([row, frame], 9) < 0.5 { 1.0 - w } else { w };
    let split = if torn { 0.025 } else { 0.004 } * burst;
    let at = |dx: f32| [(uv[0] + shift + dx).clamp(0.0, 1.0), uv[1]];
    let grain = (hash(pixel, 10) - 0.5) * 0.12 * burst;
    let (r, g, b) = (at(split), at(0.0), at(-split));
    [mix(from(r), to(r), w)[0] + grain, mix(from(g), to(g), w)[1] + grain, mix(from(b), to(b), w)[2] + grain]
}

/// The edge blur: a disc of samples (a golden-angle spiral, turned per
/// pixel) as wide as `p` and the strength make it.
fn edge_blur(v: &Values, p: f32, uv: [f32; 2], aspect: f32, pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let radius = p * 0.04 * v[0];
    let turn = hash(pixel, 11) * std::f32::consts::TAU;
    let mut sum = [0.0f32; 3];
    for i in 0..SAMPLES {
        let r = radius * ((i as f32 + 0.5) / SAMPLES as f32).sqrt();
        let a = i as f32 * 2.399_963 + turn;
        let c = from([(uv[0] + a.cos() * r / aspect).clamp(0.0, 1.0), (uv[1] + a.sin() * r).clamp(0.0, 1.0)]);
        sum = [0, 1, 2].map(|k| sum[k] + c[k]);
    }
    sum.map(|c| c / SAMPLES as f32)
}

/// The edge zoom: pushed in (or pulled out, the frame mirrored past its
/// edges) by up to the strength, blurred along the move.
fn edge_zoom(v: &Values, p: f32, uv: [f32; 2], pixel: [u32; 2], from: &dyn Fn([f32; 2]) -> [f32; 3]) -> [f32; 3] {
    let jitter = hash(pixel, 12);
    let mut sum = [0.0f32; 3];
    for i in 0..SAMPLES {
        let s = (i as f32 + jitter) / SAMPLES as f32;
        let k = 1.0 + v[1] * p * (1.0 - 0.3 * s);
        let f = if v[0] < 0.5 { 1.0 / k } else { k };
        let c = from([mirror(0.5 + (uv[0] - 0.5) * f), mirror(0.5 + (uv[1] - 0.5) * f)]);
        sum = [0, 1, 2].map(|k| sum[k] + c[k]);
    }
    sum.map(|c| c / SAMPLES as f32)
}

/// The whip's move: left, right, up, down (`Look::knobs`' order).
fn whip_direction(i: f32) -> [f32; 2] {
    match i as u32 {
        1 => [1.0, 0.0],
        2 => [0.0, -1.0],
        3 => [0.0, 1.0],
        _ => [-1.0, 0.0],
    }
}

/// FilmBurn's ease: steep through the middle, `a` how steep.
fn sigmoid(x: f32, a: f32) -> f32 {
    if x > 0.5 { 1.0 - (2.0 - 2.0 * x).powf(a) / 2.0 } else { (2.0 * x).powf(a) / 2.0 }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
pub fn blend(spec: &Spec, p: f32, a: &crate::stitch::Frame, b: &crate::stitch::Frame) -> crate::stitch::Frame {
    let (w, h, deep) = (a.width, a.height, a.deep);
    let aspect = w as f32 / h as f32;
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
            for v in colour(spec, p, uv, aspect, [x as u32, y as u32], &from, &to) {
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
        assert!(Group::OVERLAP.iter().all(|g| Look::OVERLAP.iter().any(|l| l.group() == *g)));
        assert!(Look::OVERLAP.iter().all(|l| !l.is_edge()) && Look::EDGES.iter().all(|l| l.is_edge()));
        assert_eq!((stepped(0.6), stepped(0.0), stepped(1.13)), (0.5, 0.25, 1.25));
        assert_eq!(serde_json::to_string(&Look::WhipPan).unwrap(), "\"whip_pan\"");
    }

    #[test]
    fn an_edge_starts_as_the_picture_and_ends_changed() {
        let a = |uv: [f32; 2]| [uv[0] * uv[0], 0.4, 0.5 + 0.5 * (uv[1] * 40.0).sin()];
        for look in Look::EDGES {
            let ends = look.knobs().iter().enumerate().map(|(i, k)| (i, k.most));
            for (i, value) in std::iter::once((MOST_KNOBS, 0.0)).chain(ends) {
                let mut s = Spec::of(Some(look));
                if i < MOST_KNOBS {
                    s.values[i] = value;
                }
                let (mut still, mut moved) = (0.0f32, 0.0f32);
                for n in 0..64u32 {
                    let at = [0.1 + 0.1 * (n % 8) as f32, 0.1 + 0.1 * (n / 8) as f32];
                    let diff = |p: f32| colour(&s, p, at, 16.0 / 9.0, [n, 3 * n], &a, &a).iter().zip(a(at)).map(|(g, w)| (g - w).abs()).sum::<f32>();
                    still = still.max(diff(0.0));
                    moved += diff(1.0) / 64.0;
                }
                assert!(still < 1e-4, "{s:?} at 0 is off by {still}");
                assert!(moved > 0.01, "{s:?} at 1 is near the picture: {moved}");
            }
        }
    }

    #[test]
    fn knobs_fit_the_shader_and_fill_by_default() {
        assert!(Look::OVERLAP.iter().chain(&Look::EDGES).all(|l| l.knobs().len() <= MOST_KNOBS && l.knobs().iter().all(|k| k.least <= k.default && k.default <= k.most)));
        let mut set = std::collections::BTreeMap::new();
        set.insert("turns".to_owned(), 2.1);
        let v = values(Some(Look::SpinBlur), &set);
        assert_eq!(&v[..5], &[2.0, 0.0, 0.5, 0.6, 0.25], "on its steps, the rest by default");
        assert_eq!(values(None, &set), [0.0; MOST_KNOBS]);
    }

    #[test]
    fn each_look_starts_on_the_outgoing_and_ends_on_the_incoming() {
        let (a, b) = (|_: [f32; 2]| [0.2, 0.4, 0.6], |_: [f32; 2]| [0.9, 0.1, 0.5]);
        // Every look, at its defaults and with each knob at either end.
        let mut specs = vec![Spec::default()];
        for look in Look::OVERLAP {
            specs.push(Spec::of(Some(look)));
            for (i, k) in look.knobs().iter().enumerate() {
                for end in [k.least, k.most] {
                    let mut s = Spec::of(Some(look));
                    s.values[i] = end;
                    specs.push(s);
                }
            }
        }
        for s in specs {
            for (p, want) in [(0.0, [0.2, 0.4, 0.6]), (1.0, [0.9, 0.1, 0.5])] {
                let got = colour(&s, p, [0.3, 0.7], 16.0 / 9.0, [5, 9], &a, &b);
                assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-3), "{s:?} at {p}: {got:?}");
            }
        }
        // A flat picture stays flat through a blur's middle; the flash
        // goes past white.
        let mid = colour(&Spec::of(Some(Look::WhipPan)), 0.5, [0.5, 0.5], 1.0, [3, 3], &a, &a);
        assert!(mid.iter().zip([0.2, 0.4, 0.6]).all(|(g, w)| (g - w).abs() < 1e-3), "{mid:?}");
        assert!(colour(&Spec::of(Some(Look::Flash)), 0.5, [0.5, 0.5], 1.0, [0, 0], &a, &b)[0] > 0.9);
    }
}
