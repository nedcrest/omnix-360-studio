//! Colour effects on the framed picture: exposure, white balance, tone,
//! saturation, a vignette and opacity, keyframed per cut, plus the edit's
//! fades from and to black.
//!
//! They act on the stitched view, after framing, so the player and both
//! export paths grade the same picture and a crossfade blends two graded
//! cuts. The shader's `grade` mirrors `Grade::apply`; change them together.
//! Exposure and white balance work on light (the display values decoded
//! with a 2.2 gamma), the rest on the display values themselves, as a
//! colourist's controls feel.

use crate::view::Ease;
use serde::{Deserialize, Serialize};

/// The effects' values; the default (all 0, opacity 1) is the picture
/// untouched.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Grade {
    /// Stops, −3..3.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exposure: f64,
    /// −1..1, each a stop of contrast around mid-grey.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub contrast: f64,
    /// −1..1: pull back or lift the bright and the dark parts.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub highlights: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shadows: f64,
    /// −1..1: cooler to warmer, and green to magenta.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub temperature: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tint: f64,
    /// −1..1: −1 is black and white, 1 double.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub saturation: f64,
    /// −1..1: like saturation, but mostly on dull colours.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vibrance: f64,
    /// 0..1: how dark the corners go.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vignette: f64,
    /// The vignette's shape (gap 6): 0 round, 1 the top (a graduated
    /// filter on a sky), 2 the bottom, 3 the sides (`VIGNETTES`); held
    /// from key to key, not blended.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub vignette_shape: f64,
    /// 0..1: how far in from its edge it reaches.
    #[serde(default = "reach", skip_serializing_if = "is_reach")]
    pub vignette_reach: f64,
    /// 0..1: how soft its inner edge is.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub vignette_soft: f64,
    /// 0..1: how much of the picture shows. On the main track what's left
    /// is black, as in a fade; an overlay will show what's beneath.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    /// 0..1: how much of the cut's LUT (`Cut::lut`), applied before the
    /// rest; keyframed with them, so a LUT can fade in or out in a cut.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub lut: f64,
    /// 0..1, text and pictures only (gap 87, step 2b): how soft it's
    /// drawn, 1 past reading (`overlay::blur_reach`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub blur: f64,
    /// 0..1, text and pictures only: how much of it has come on, a
    /// picture wiped in from the left, text typed letter by letter
    /// (`overlay::revealed`).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub reveal: f64,
    /// 0..0.5: the level that becomes black (gap 14, input levels): a
    /// picture whose darkest is 14 of 255 gets black at about 0.055.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub black_in: f64,
    /// 0.5..1: the level that becomes white.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub white_in: f64,
    /// 0..1: how far faded to black, from the edit's fades; not keyframed.
    #[serde(skip)]
    pub black: f64,
}

impl Default for Grade {
    fn default() -> Self {
        Grade::from_values(NEUTRAL)
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn one() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

/// The vignette's reach as it was before it could be set.
const REACH: f64 = 0.7;

fn reach() -> f64 {
    REACH
}

fn is_reach(v: &f64) -> bool {
    *v == REACH
}

/// The vignette's shapes, by `Grade::vignette_shape`.
pub const VIGNETTES: [&str; 4] = ["Round", "Top", "Bottom", "Sides"];

/// One of `Grade::values`: its name in the project (`KeyOp::EffectValue`),
/// as the Effects panel shows it, what it does, its range (the panel's
/// slider, an automation lane's height) and its value untouched.
#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub name: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub range: (f64, f64),
    pub neutral: f64,
}

const fn effect(name: &'static str, label: &'static str, hint: &'static str, range: (f64, f64), neutral: f64) -> Effect {
    Effect { name, label, hint, range, neutral }
}

/// Where each sits in `Grade::values`.
pub const EXPOSURE: usize = 0;
pub const CONTRAST: usize = 1;
pub const HIGHLIGHTS: usize = 2;
pub const SHADOWS: usize = 3;
pub const TEMPERATURE: usize = 4;
pub const TINT: usize = 5;
pub const SATURATION: usize = 6;
pub const VIBRANCE: usize = 7;
/// The vignette's amount, its shape (held, not blended), its reach and
/// softness.
pub const VIGNETTE: usize = 8;
pub const OPACITY: usize = 9;
/// How much of the cut's LUT.
pub const LUT: usize = 10;
pub const SHAPE: usize = 11;
pub const VIGNETTE_REACH: usize = 12;
pub const VIGNETTE_SOFT: usize = 13;
/// A text's or a picture's, never a cut's (`ITEM_ONLY`).
pub const BLUR: usize = 14;
pub const REVEAL: usize = 15;
/// The input levels (gap 14).
pub const BLACK_IN: usize = 16;
pub const WHITE_IN: usize = 17;
pub const ITEM_ONLY: [usize; 2] = [BLUR, REVEAL];

/// Each of `Grade::values`, in its order.
pub const EFFECTS: [Effect; 18] = [
    effect("exposure", "Exposure", "Brighter or darker, in stops, like the camera's EV", (-3.0, 3.0), 0.0),
    effect("contrast", "Contrast", "More or less contrast, around mid-grey", (-1.0, 1.0), 0.0),
    effect("highlights", "Highlights", "Pull back (−) or lift the bright parts, such as the sky", (-1.0, 1.0), 0.0),
    effect("shadows", "Shadows", "Lift (+) or deepen the dark parts", (-1.0, 1.0), 0.0),
    effect("temperature", "Temperature", "Cooler (−) or warmer", (-1.0, 1.0), 0.0),
    effect("tint", "Tint", "Greener (−) or more magenta", (-1.0, 1.0), 0.0),
    effect("saturation", "Saturation", "Less or more colour; −1 is black and white", (-1.0, 1.0), 0.0),
    effect("vibrance", "Vibrance", "More colour mostly where there is little, sparing skin and sky", (-1.0, 1.0), 0.0),
    effect("vignette", "Vignette", "Darker edges: round, or from the top, the bottom or the sides", (0.0, 1.0), 0.0),
    effect("opacity", "Opacity", "How much of the picture shows; on the main track it fades towards black", (0.0, 1.0), 1.0),
    effect("lut", "Strength", "How much of the LUT: 100 % is all of it, less mixes in the picture as filmed", (0.0, 1.0), 1.0),
    effect("vignette_shape", "Shape", "The vignette's shape: round, from the top, the bottom or the sides", (0.0, 3.0), 0.0),
    effect("vignette_reach", "Reach", "How far in the vignette reaches", (0.0, 1.0), REACH),
    effect("vignette_soft", "Softness", "How soft the vignette's inner edge is", (0.0, 1.0), 1.0),
    effect("blur", "Blur", "How soft it's drawn; at 100 % past reading", (0.0, 1.0), 0.0),
    effect("reveal", "Reveal", "How much has come on: a picture wiped in from the left, text typed letter by letter", (0.0, 1.0), 1.0),
    effect("black_in", "Black", "The level that becomes black, of 255: the picture's darkest (Scopes' Black) makes its black black", (0.0, 0.5), 0.0),
    effect("white_in", "White", "The level that becomes white, of 255: the picture's brightest makes its white white", (0.5, 1.0), 1.0),
];

/// `EFFECTS`' names, ranges and neutral values each as a list.
pub const NAMES: [&str; 18] = {
    let (mut a, mut i) = ([""; 18], 0);
    while i < EFFECTS.len() {
        a[i] = EFFECTS[i].name;
        i += 1;
    }
    a
};
pub const RANGES: [(f64, f64); 18] = {
    let (mut a, mut i) = ([(0.0, 0.0); 18], 0);
    while i < EFFECTS.len() {
        a[i] = EFFECTS[i].range;
        i += 1;
    }
    a
};
/// `Grade::values` of the picture untouched.
pub const NEUTRAL: [f64; 18] = {
    let (mut a, mut i) = ([0.0; 18], 0);
    while i < EFFECTS.len() {
        a[i] = EFFECTS[i].neutral;
        i += 1;
    }
    a
};

/// Stops of colour per unit of temperature or tint: at ±1, red and blue
/// are four times apart, enough to neutralise shade or low sun.
const WB_STOPS: f64 = 1.0;

impl Grade {
    pub fn is_neutral(&self) -> bool {
        *self == Grade::default()
    }

    /// The fields as a list, for blending and the panel.
    pub fn values(&self) -> [f64; 18] {
        [
            self.exposure,
            self.contrast,
            self.highlights,
            self.shadows,
            self.temperature,
            self.tint,
            self.saturation,
            self.vibrance,
            self.vignette,
            self.opacity,
            self.lut,
            self.vignette_shape,
            self.vignette_reach,
            self.vignette_soft,
            self.blur,
            self.reveal,
            self.black_in,
            self.white_in,
        ]
    }

    pub fn from_values(v: [f64; 18]) -> Grade {
        let [exposure, contrast, highlights, shadows, temperature, tint, saturation, vibrance, vignette, opacity, lut, vignette_shape, vignette_reach, vignette_soft, blur, reveal, black_in, white_in] = v;
        Grade { exposure, contrast, highlights, shadows, temperature, tint, saturation, vibrance, vignette, vignette_shape, vignette_reach, vignette_soft, opacity, lut, blur, reveal, black_in, white_in, black: 0.0 }
    }

    /// What colours an overlay's pixels: its opacity, fade, blur and
    /// reveal left out, each applied on its own.
    pub fn shade(&self) -> Grade {
        Grade { opacity: 1.0, black: 0.0, blur: 0.0, reveal: 1.0, ..*self }
    }

    /// How far toward the vignette's edge `at` is (−1..1 each way, y down),
    /// 0..1 by its shape, and where its darkening starts and ends.
    fn vignette_mask(&self, at: [f64; 2], aspect: f64) -> f64 {
        let d = match self.vignette_shape.round() as u32 {
            1 => (1.0 - at[1]) / 2.0,
            2 => (1.0 + at[1]) / 2.0,
            3 => at[0].abs(),
            _ => ((at[0] * aspect).powi(2) + at[1].powi(2)).sqrt() / (aspect * aspect + 1.0).sqrt(),
        };
        let from = 1.0 - self.vignette_reach;
        let to = from + (1.0 - from) * self.vignette_soft.max(0.05);
        let t = ((d - from) / (to - from)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Grades display-encoded `rgb` (0..1) at screen point `at` (−1..1 each
    /// way) of a picture `aspect` wide to its height.
    pub fn apply(&self, rgb: [f64; 3], at: [f64; 2], aspect: f64) -> [f64; 3] {
        let luma = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        // Levels first (gap 14): `black_in` to black, `white_in` to white,
        // on the display values, as an editor's input levels.
        let span = (self.white_in - self.black_in).max(1e-3);
        let rgb = rgb.map(|v| ((v - self.black_in) / span).clamp(0.0, 1.0));
        // Light: exposure and white balance.
        let gain = 2f64.powf(self.exposure);
        let wb = [2f64.powf(WB_STOPS * self.temperature), 2f64.powf(-WB_STOPS * self.tint), 2f64.powf(-WB_STOPS * self.temperature)];
        let mut c = [0.0; 3];
        for i in 0..3 {
            c[i] = shoulder(rgb[i].max(0.0).powf(2.2) * gain * wb[i], gain * wb[i]).powf(1.0 / 2.2);
        }
        // Tone: highlights and shadows move the brightness, keeping the hue.
        // Shadows works on a bump that is 0 at black (black stays black)
        // and greatest at 0.2. The colour grows only by the square root of
        // the brightness gain: multiplied by the whole gain (6× for a pixel
        // lifted from 5% to 30%), shadows' slight warm cast turned the rock
        // in them red (user, 2026-10-14; a shadowed rock face at +0.8: its
        // colour spread 0.075 against 0.043 now, from 0.027).
        let l = luma(c);
        let smooth = |a: f64, b: f64, x: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let x = (l / 0.6).clamp(0.0, 1.0);
        let lift = 0.25 * (self.shadows * 6.75 * x * (1.0 - x).powi(2) + self.highlights * smooth(0.4, 1.0, l));
        if l > 1e-4 {
            let to = (l + lift).max(0.0);
            let g = (to / l).sqrt();
            c = c.map(|v| to + (v - l) * g);
        } else {
            c = c.map(|v| (v + lift).max(0.0));
        }
        let k = 2f64.powf(self.contrast);
        c = c.map(|v| (v - 0.5) * k + 0.5);
        // Colour: saturation, and vibrance most on what's least saturated.
        let l = luma(c);
        let spread = c.iter().fold(f64::MIN, |a, &v| a.max(v)) - c.iter().fold(f64::MAX, |a, &v| a.min(v));
        let s = (1.0 + self.saturation) * (1.0 + self.vibrance * (1.0 - spread.clamp(0.0, 1.0)));
        c = c.map(|v| l + (v - l) * s);
        // The vignette, by distance from the centre (1 at the corners), the
        // opacity over black and the fade to black.
        let keep = (1.0 - self.vignette * self.vignette_mask(at, aspect)) * self.opacity * (1.0 - self.black);
        c.map(|v| (v * keep).clamp(0.0, 1.0))
    }

    /// For the shader: (exposure gain, temperature and tint in stops,
    /// contrast gain),
    /// (highlights, shadows, saturation, vibrance), (vignette, black, 1 if
    /// it does anything, opacity), the vignette's (shape, from, to, 0).
    pub fn uniforms(&self) -> [[f32; 4]; 5] {
        let f = |v: f64| v as f32;
        [
            [f(2f64.powf(self.exposure)), f(WB_STOPS * self.temperature), f(WB_STOPS * self.tint), f(2f64.powf(self.contrast))],
            [f(self.highlights), f(self.shadows), f(self.saturation), f(self.vibrance)],
            [f(self.vignette), f(self.black), if self.is_neutral() { 0.0 } else { 1.0 }, f(self.opacity)],
            {
                let from = 1.0 - self.vignette_reach;
                [self.vignette_shape.round() as f32, f(from), f(from + (1.0 - from) * self.vignette_soft.max(0.05)), 0.0]
            },
            [f(self.black_in), f((self.white_in - self.black_in).max(1e-3)), 0.0, 0.0],
        ]
    }
}

/// Where brightening light starts to roll off, in linear light (about
/// 0.66 displayed).
const KNEE: f64 = 0.4;

/// Linear light `x` brightened by `gain`, rolled off above `KNEE` so the
/// source's white (now `gain`) lands on white instead of everything above
/// 1/gain clipping: at +0.5 EV 23% of 0037's summit view had a channel
/// burnt out (user, 2026-10-15: "it burns out highlights very quick").
/// Below the knee, and for gains up to 1, it's the plain gain. The curve
/// k + t/(1 + a·t) leaves the knee at slope 1 and meets white at `gain`.
pub fn shoulder(x: f64, gain: f64) -> f64 {
    if gain <= 1.0 || x <= KNEE {
        return x;
    }
    let a = 1.0 / (1.0 - KNEE) - 1.0 / (gain - KNEE);
    let t = x - KNEE;
    KNEE + t / (1.0 + a * t)
}

/// The temperature and tint that make `rgb` (display-encoded, of something
/// that should be grey or white) neutral, within −1..1.
pub fn white_balance(rgb: [f64; 3]) -> (f64, f64) {
    let [r, g, b] = rgb.map(|v| v.max(1e-4).powf(2.2));
    // r·2^(sT) = b·2^(−sT), and then g·2^(−s·tint) their geometric mean.
    let temperature = ((b / r).log2() / (2.0 * WB_STOPS)).clamp(-1.0, 1.0);
    let (r, b) = (r * 2f64.powf(WB_STOPS * temperature), b * 2f64.powf(-WB_STOPS * temperature));
    let tint = ((g / (r * b).sqrt()).log2() / WB_STOPS).clamp(-1.0, 1.0);
    (temperature, tint)
}

/// A cut's effects from clip time `t`, eased like camera keyframes into the
/// next.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectKey {
    pub t: f64,
    #[serde(flatten)]
    pub grade: Grade,
    #[serde(default, skip_serializing_if = "is_smooth")]
    pub ease: Ease,
}

impl crate::view::Timed for EffectKey {
    fn time(&self) -> f64 {
        self.t
    }
    fn set_time(&mut self, t: f64) {
        self.t = t;
    }
}

fn is_smooth(e: &Ease) -> bool {
    *e == Ease::Smooth
}

/// The effects at clip time `t` from `keys` (in time order): held before
/// the first and after the last, none without any.
pub fn grade_at(keys: &[EffectKey], t: f64) -> Grade {
    let i = keys.partition_point(|k| k.t <= t);
    if i == 0 || i == keys.len() {
        return keys.get(i.saturating_sub(1)).map_or_else(Grade::default, |k| k.grade);
    }
    let (a, b) = (&keys[i - 1], &keys[i]);
    let Some(u) = a.ease.shape(((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0)) else { return a.grade };
    let (va, vb) = (a.grade.values(), b.grade.values());
    Grade::from_values(std::array::from_fn(|k| if k == SHAPE { va[k] } else { va[k] + (vb[k] - va[k]) * u }))
}

#[cfg(test)]
mod tests {
    /// Exposure up rolls the highlights off instead of clipping them, and
    /// leaves everything else as the plain gain did.
    #[test]
    fn levels_make_their_black_black_and_their_white_white() {
        let g = Grade { black_in: 14.0 / 255.0, white_in: 0.9, ..Grade::default() };
        let at = |v: f64| g.apply([v; 3], [0.0, 0.0], 1.0)[0];
        assert!(at(14.0 / 255.0) < 1e-9 && at(0.02) == 0.0, "the darkest is black");
        assert!((at(0.9) - 1.0).abs() < 1e-9 && at(0.97) == 1.0, "the white point is white");
        assert!(at(0.5) > 0.5, "the middle opens up");
        assert_eq!(Grade::default().apply([0.3; 3], [0.0, 0.0], 1.0), [0.3; 3].map(|v: f64| v.powf(2.2).powf(1.0 / 2.2)), "none: as it was");
    }

    #[test]
    fn exposure_rolls_highlights_off() {
        for gain in [0.5, 1.0] {
            for x in [0.0, 0.3, 0.7, 1.0] {
                assert_eq!(shoulder(x * gain, gain), x * gain);
            }
        }
        for gain in [1.2, 2.0, 4.0] {
            assert_eq!(shoulder(0.3, gain), 0.3);
            assert!((shoulder(gain, gain) - 1.0).abs() < 1e-12, "white lands on white");
            let mut last = 0.0;
            for i in 0..=1000 {
                let x = i as f64 / 1000.0 * gain;
                let y = shoulder(x, gain);
                assert!(y >= last && y <= 1.0 + 1e-12);
                last = y;
            }
            // Smooth at the knee: no step in slope.
            let (d, k) = (1e-6, KNEE);
            assert!(((shoulder(k + d, gain) - shoulder(k, gain)) / d - 1.0).abs() < 1e-4);
        }
    }

    /// A dragged effect keyframe stops a frame short of its neighbours, so
    /// each keeps what it holds.
    #[test]
    fn effect_keys_retime_between_neighbours() {
        let key = |t, exposure| EffectKey { t, grade: Grade { exposure, ..Default::default() }, ease: Ease::Smooth };
        let mut keys = vec![key(1.0, 0.1), key(2.0, 0.2), key(3.0, 0.3)];
        crate::view::retime(&mut keys, 1, 2.5, 0.1);
        assert_eq!(keys[1].t, 2.5);
        crate::view::retime(&mut keys, 1, 9.0, 0.1);
        assert!((keys[1].t - 2.9).abs() < 1e-9 && keys[1].grade.exposure == 0.2);
        crate::view::retime(&mut keys, 0, -5.0, 0.1);
        assert_eq!(keys[0].t, -5.0);
    }

    use super::*;

    #[test]
    fn neutral_leaves_the_picture_and_each_control_moves_it_its_way() {
        let grey = [0.5, 0.5, 0.5];
        let mid = [0.0, 0.0];
        let out = Grade::default().apply(grey, mid, 1.6);
        assert!(out.iter().all(|v| (v - 0.5).abs() < 1e-9), "{out:?}");
        let g = |grade: Grade, c: [f64; 3]| grade.apply(c, mid, 1.6);
        // A stop more light: 2^(1/2.2) brighter in display values.
        assert!((g(Grade { exposure: 1.0, ..Default::default() }, [0.3; 3])[0] - 0.3 * 2f64.powf(1.0 / 2.2)).abs() < 1e-9);
        let warm = g(Grade { temperature: 0.5, ..Default::default() }, grey);
        assert!(warm[0] > warm[1] && warm[1] > warm[2]);
        let bw = g(Grade { saturation: -1.0, ..Default::default() }, [0.8, 0.4, 0.2]);
        assert!((bw[0] - bw[1]).abs() < 1e-9 && (bw[1] - bw[2]).abs() < 1e-9);
        let punchy = g(Grade { contrast: 1.0, ..Default::default() }, [0.75; 3]);
        assert!((punchy[0] - 1.0).abs() < 1e-9);
        // Corners darken, the centre doesn't; a fade takes it all to black.
        let v = Grade { vignette: 1.0, ..Default::default() };
        assert_eq!(v.apply(grey, mid, 1.6), grey);
        assert!(v.apply(grey, [1.0, 1.0], 1.6)[0] < 0.01);
        assert_eq!(Grade { black: 1.0, ..Default::default() }.apply(grey, mid, 1.6), [0.0; 3]);
        // Opacity goes towards black, and its default is the picture whole.
        assert!(Grade::default().opacity == 1.0 && Grade::default().is_neutral());
        assert!((Grade { opacity: 0.25, ..Default::default() }.apply(grey, mid, 1.6)[0] - 0.125).abs() < 1e-9);
    }

    #[test]
    fn white_balance_neutralises_a_colour_cast() {
        let cast = [0.62, 0.55, 0.42];
        let (temperature, tint) = white_balance(cast);
        let out = Grade { temperature, tint, ..Default::default() }.apply(cast, [0.0, 0.0], 1.0);
        assert!((out[0] - out[1]).abs() < 1e-6 && (out[1] - out[2]).abs() < 1e-6, "{out:?}");
    }

    #[test]
    fn effect_keys_ease_and_hold() {
        let key = |t, exposure, ease| EffectKey { t, grade: Grade { exposure, ..Default::default() }, ease };
        let keys = [key(10.0, 0.0, Ease::Linear), key(20.0, 1.0, Ease::Hold), key(30.0, -1.0, Ease::Smooth)];
        assert_eq!(grade_at(&keys, 5.0).exposure, 0.0);
        assert_eq!(grade_at(&keys, 15.0).exposure, 0.5);
        assert_eq!(grade_at(&keys, 25.0).exposure, 1.0);
        assert_eq!(grade_at(&keys, 40.0).exposure, -1.0);
        assert!(grade_at(&[], 3.0).is_neutral());
        // Only what's set is written.
        let json = serde_json::to_string(&key(10.0, 0.5, Ease::Smooth)).unwrap();
        assert_eq!(json, r#"{"t":10.0,"exposure":0.5}"#);
        let old: EffectKey = serde_json::from_str(r#"{"t":1.0,"vignette":0.5}"#).unwrap();
        assert_eq!(old.grade.opacity, 1.0);
        let faded = EffectKey { t: 2.0, grade: Grade { opacity: 0.5, ..Default::default() }, ease: Ease::Smooth };
        assert_eq!(serde_json::to_string(&faded).unwrap(), r#"{"t":2.0,"opacity":0.5}"#);
        // Eased like the rest.
        let keys = [EffectKey { t: 0.0, ..faded }, EffectKey { t: 10.0, grade: Grade::default(), ease: Ease::Linear }];
        assert!((grade_at(&[EffectKey { ease: Ease::Linear, ..keys[0] }, keys[1]], 5.0).opacity - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_vignette_keeps_its_old_look_by_default_and_takes_a_shape() {
        // An old project's vignette reads as it was drawn before shapes.
        let old: EffectKey = serde_json::from_str(r#"{"t":1.0,"vignette":0.5}"#).unwrap();
        assert_eq!((old.grade.vignette_shape, old.grade.vignette_reach, old.grade.vignette_soft), (0.0, 0.7, 1.0));
        let smooth = |a: f64, b: f64, x: f64| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        for at in [[0.0, 0.0], [0.9, -0.8], [-1.0, 1.0], [0.4, 0.2]] {
            let r = ((at[0] * 1.6f64).powi(2) + at[1].powi(2)).sqrt() / (1.6f64 * 1.6 + 1.0).sqrt();
            assert!((old.grade.vignette_mask(at, 1.6) - smooth(0.3, 1.0, r)).abs() < 1e-12);
        }
        // The top: the sky darkened, the ground not; held from key to key.
        let top = Grade { vignette: 1.0, vignette_shape: 1.0, ..Default::default() };
        let grey = [0.5; 3];
        assert!(top.apply(grey, [0.0, -0.95], 1.6)[0] < 0.2 && (top.apply(grey, [0.0, 0.9], 1.6)[0] - 0.5).abs() < 1e-9);
        let keys = [EffectKey { t: 0.0, grade: top, ease: Ease::Linear }, EffectKey { t: 2.0, grade: Grade { vignette_shape: 3.0, ..top }, ease: Ease::Linear }];
        assert_eq!(grade_at(&keys, 1.5).vignette_shape, 1.0);
        assert!(serde_json::to_string(&top).unwrap().contains("vignette_shape") && !serde_json::to_string(&Grade::default()).unwrap().contains("vignette"));
    }
}
