//! Overlay kinds (gap 87, step 2c, the user, 2026-11-09): a picture
//! dressed as a sticker (a white cut-out edge round its shape), a polaroid
//! (a card, wide at the foot) or a picture in picture (rounded, a thin
//! border), each with a soft shadow. Made into its picture when it's
//! loaded (`Item::picture`), so the player and the export draw the same.

use crate::mask::{blur, distance, shift};
use crate::overlay::Picture;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// How a picture on the overlay track is dressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Sticker,
    Polaroid,
    /// Picture in picture.
    Inset,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Sticker, Kind::Polaroid, Kind::Inset];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Sticker => "Sticker",
            Kind::Polaroid => "Polaroid",
            Kind::Inset => "Picture in picture",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Kind::Sticker => "a white cut-out edge round its shape, even a PNG's, and a shadow",
            Kind::Polaroid => "on a white card wide at the foot, and a shadow",
            Kind::Inset => "rounded corners, a thin white border and a shadow",
        }
    }
}

// Sizes as shares of the picture's short side.
const STICKER_EDGE: f64 = 0.035;
const POLAROID_SIDE: f64 = 0.06;
const POLAROID_FOOT: f64 = 0.24;
const INSET_ROUND: f64 = 0.05;
const INSET_BORDER: f64 = 0.012;
const SHADOW_DROP: f64 = 0.02;
const SHADOW_SOFT: f64 = 0.02;
const SHADOW_DARK: f32 = 0.45;
/// A polaroid's card, a little warm.
const CARD: [f32; 3] = [0.97, 0.96, 0.93];

/// `picture` dressed as `kind`: a bigger picture, with room for its edge
/// or card and its shadow.
pub fn dress(picture: &Picture, kind: Kind) -> Picture {
    let (w, h) = (picture.width, picture.height);
    let s = w.min(h) as f64;
    let (side, foot) = match kind {
        Kind::Sticker => (STICKER_EDGE * s, STICKER_EDGE * s),
        Kind::Polaroid => (POLAROID_SIDE * s, POLAROID_FOOT * s),
        Kind::Inset => (INSET_BORDER * s, INSET_BORDER * s),
    };
    let (drop, soft) = (SHADOW_DROP * s, SHADOW_SOFT * s);
    let shade_room = (drop + 2.0 * soft).ceil() as usize + 1;
    let at = side.ceil() as usize + shade_room;
    let size = (at + w + side.ceil() as usize + shade_room, at + h + foot.ceil() as usize + shade_room);
    let (cw, ch) = size;
    // The picture's place, and the card's, in the new picture (pixels).
    let inner = (at as f64, at as f64, (at + w) as f64, (at + h) as f64);
    let outer = (inner.0 - side, inner.1 - side, inner.2 + side, inner.3 + foot);
    let card: Vec<f32> = match kind {
        Kind::Sticker => sticker_edge(picture, at, size, side),
        Kind::Polaroid => coverage(size, |x, y| rounded(x, y, outer, 0.0)),
        Kind::Inset => coverage(size, |x, y| rounded(x, y, outer, INSET_ROUND * s + side)),
    };
    let clip = |x: usize, y: usize| if kind == Kind::Inset { rounded(x as f64 + 0.5, y as f64 + 0.5, inner, INSET_ROUND * s) } else { 1.0 };
    let shade = blur(&shift(&card, size, drop), size, soft);
    let colour = if kind == Kind::Polaroid { CARD } else { [1.0; 3] };
    let mut rgba = vec![0u8; cw * ch * 4];
    rgba.par_chunks_mut(4).enumerate().for_each(|(i, px)| {
        let (x, y) = (i % cw, i / cw);
        // Shadow, then the card or edge, then the picture, each over what's below.
        let (mut c, mut a) = ([0f32; 3], SHADOW_DARK * shade[i]);
        let mut over = |src: [f32; 3], alpha: f32| {
            (0..3).for_each(|k| c[k] = src[k] + c[k] * (1.0 - alpha));
            a = alpha + a * (1.0 - alpha);
        };
        over(colour.map(|v| v * card[i]), card[i]);
        if (at..at + w).contains(&x) && (at..at + h).contains(&y) {
            let j = ((y - at) * w + x - at) * 4;
            let k = clip(x, y);
            let p = &picture.rgba[j..j + 4];
            over([0, 1, 2].map(|n| p[n] as f32 / 255.0 * k), p[3] as f32 / 255.0 * k);
        }
        (0..3).for_each(|k| px[k] = (c[k] * 255.0).round() as u8);
        px[3] = (a * 255.0).round() as u8;
    });
    Picture { width: cw, height: ch, rgba, natural: picture.natural, order: None }
}

/// A sticker's edge: `r` pixels round the picture's shape (where it's
/// at least half opaque), soft at its rim; the picture sits `at` in.
fn sticker_edge(picture: &Picture, at: usize, (cw, ch): (usize, usize), r: f64) -> Vec<f32> {
    let (w, h) = (picture.width, picture.height);
    let inside: Vec<bool> = (0..cw * ch)
        .map(|i| {
            let (x, y) = (i % cw, i / cw);
            (at..at + w).contains(&x) && (at..at + h).contains(&y) && picture.rgba[((y - at) * w + x - at) * 4 + 3] >= 128
        })
        .collect();
    distance(&inside, (cw, ch)).into_iter().map(|d| (r as f32 + 0.5 - d).clamp(0.0, 1.0)).collect()
}

/// `inside` at each pixel's centre of a `w`×`h` picture.
fn coverage((w, h): (usize, usize), inside: impl Fn(f64, f64) -> f32 + Sync) -> Vec<f32> {
    (0..w * h).into_par_iter().map(|i| inside((i % w) as f64 + 0.5, (i / w) as f64 + 0.5)).collect()
}

/// How much of a pixel at (`x`, `y`) a rectangle `x0, y0, x1, y1` with
/// corners rounded `r` covers, soft over a pixel at its edge.
fn rounded(x: f64, y: f64, (x0, y0, x1, y1): (f64, f64, f64, f64), r: f64) -> f32 {
    let (hx, hy) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let r = r.min(hx).min(hy);
    let (qx, qy) = ((x - (x0 + hx)).abs() - (hx - r), (y - (y0 + hy)).abs() - (hy - r));
    let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An opaque grey picture, `w`×`h`.
    fn grey(w: usize, h: usize) -> Picture {
        Picture { width: w, height: h, rgba: [100u8, 100, 100, 255].repeat(w * h), natural: None, order: None }
    }

    #[test]
    fn each_kind_frames_the_picture_and_keeps_it() {
        let p = grey(200, 100);
        for kind in Kind::ALL {
            let d = dress(&p, kind);
            assert!(d.width > 200 && d.height > 100, "{kind:?} has room");
            assert!(d.rgba.chunks(4).filter(|px| *px == [100, 100, 100, 255]).count() > 150 * 80, "{kind:?}: the picture shows");
            assert_eq!(d.rgba[3], 0, "{kind:?}: clear at its corner");
        }
        // A polaroid's card is wider at its foot than at its head.
        let d = dress(&p, Kind::Polaroid);
        let card = |y: usize| d.rgba[(y * d.width + d.width / 2) * 4] > 200;
        let head = (0..d.height / 2).filter(|&y| card(y)).count();
        let foot = (d.height / 2..d.height).filter(|&y| card(y)).count();
        assert!(foot > 3 * head, "{head} {foot}");
    }
}
