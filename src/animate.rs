//! Ready-made ins and outs for text and pictures (gap 87, the effects
//! milestone's step 2a, the user, 2026-11-10): Fade, Slide four ways, Pop,
//! Zoom, Spin. One is made into automation clips at the item's start
//! (`In`) or end (`Out`) — its opacity, place, size, turn — so it plays as
//! any automation does and can be adjusted on the lanes; each clip says
//! which animation it came from (`AutoClip::animation`), and a trim keeps
//! them on their edge (`keep_on_edges`).

use crate::automation::{AutoClip, AutoPoint};
use crate::model::ops::End;
use crate::view::Ease;
use serde::{Deserialize, Serialize};

/// An in or an out, by how it moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Anim {
    Fade,
    /// Moving leftwards (in: from the right; out: off to the left), and so on.
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    /// Out of nothing past its size and back, as a sticker pops on.
    Pop,
    /// From bigger, faded in.
    Zoom,
    /// Turning once as it grows.
    Spin,
}

impl Anim {
    pub const ALL: [Anim; 8] = [Anim::Fade, Anim::SlideLeft, Anim::SlideRight, Anim::SlideUp, Anim::SlideDown, Anim::Pop, Anim::Zoom, Anim::Spin];

    pub fn name(self) -> &'static str {
        match self {
            Anim::Fade => "Fade",
            Anim::SlideLeft => "Slide ←",
            Anim::SlideRight => "Slide →",
            Anim::SlideUp => "Slide ↑",
            Anim::SlideDown => "Slide ↓",
            Anim::Pop => "Pop",
            Anim::Zoom => "Zoom",
            Anim::Spin => "Spin",
        }
    }
}

/// Where a clip made by an animation came from: which, on which edge, how
/// long.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub anim: Anim,
    pub edge: End,
    pub seconds: f64,
}

/// How far a slide travels, in frames.
const SLIDE: f64 = 0.75;

/// The clips `anim` makes on `edge` of an item running `start`..`end` (own
/// time), `seconds` long, from its values where it holds still: its
/// opacity and its place (x, y, size, rotation).
pub fn clips(animation: Animation, (start, end): (f64, f64), opacity: f64, place: crate::overlay::Place) -> Vec<AutoClip> {
    let d = animation.seconds.clamp(0.05, (end - start).max(0.05));
    let (from, to) = if animation.edge == End::In { (start, start + d) } else { (end - d, end) };
    // A value's run on this edge: its values from the start of the in (or
    // the out), the still value at the other end.
    let run = |param: &str, values: &[(f64, f64)]| {
        let points = values.iter().map(|&(k, v)| AutoPoint { t: from + k * (to - from), v, ease: Ease::Smooth }).collect();
        AutoClip { param: param.to_owned(), from, to, points, follows: None, animation: Some(animation) }
    };
    // `away` is the value off-stage; an in goes away → still, an out the reverse.
    let swing = |param: &str, still: f64, away: f64| {
        let v = if animation.edge == End::In { [(0.0, away), (1.0, still)] } else { [(0.0, still), (1.0, away)] };
        run(param, &v)
    };
    let pop = |param: &str, still: f64, peak: f64, away: f64| {
        let v = if animation.edge == End::In { [(0.0, away), (0.7, peak), (1.0, still)] } else { [(0.0, still), (0.3, peak), (1.0, away)] };
        run(param, &v)
    };
    // Moving leftwards: in from the right of where it is, out to its left.
    let side = |still: f64, sign: f64| if animation.edge == End::In { still - sign * SLIDE } else { still + sign * SLIDE };
    match animation.anim {
        Anim::Fade => vec![swing("opacity", opacity, 0.0)],
        Anim::SlideLeft => vec![swing("x", place.x, side(place.x, -1.0))],
        Anim::SlideRight => vec![swing("x", place.x, side(place.x, 1.0))],
        Anim::SlideUp => vec![swing("y", place.y, side(place.y, -1.0))],
        Anim::SlideDown => vec![swing("y", place.y, side(place.y, 1.0))],
        Anim::Pop => vec![pop("size", place.size, place.size * 1.15, 0.0)],
        Anim::Zoom => vec![swing("size", place.size, place.size * 1.6), swing("opacity", opacity, 0.0)],
        Anim::Spin => vec![swing("rotation", place.rotation, place.rotation - 360.0), swing("size", place.size, 0.0)],
    }
}

/// An item trimmed from `was` to `now` (own time): its animations' clips
/// moved with the edge they belong to.
pub fn keep_on_edges(clips: &mut [AutoClip], was: (f64, f64), now: (f64, f64)) {
    for c in clips {
        let by = match c.animation.map(|a| a.edge) {
            Some(End::In) => now.0 - was.0,
            Some(End::Out) => now.1 - was.1,
            None => continue,
        };
        (c.from, c.to) = (c.from + by, c.to + by);
        c.points.iter_mut().for_each(|q| q.t += by);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::Place;

    fn at(clips: &[AutoClip], param: &str, t: f64) -> Option<f64> {
        crate::automation::value(clips, param, t)
    }

    #[test]
    fn an_in_comes_to_the_still_value_and_an_out_leaves_it() {
        let place = Place { x: 0.5, y: 0.8, size: 1.0, rotation: 0.0 };
        let fade_in = clips(Animation { anim: Anim::Fade, edge: End::In, seconds: 1.0 }, (2.0, 8.0), 0.9, place);
        assert_eq!((at(&fade_in, "opacity", 2.0), at(&fade_in, "opacity", 3.0), at(&fade_in, "opacity", 5.0)), (Some(0.0), Some(0.9), None), "covered only over its second");
        let slide_out = clips(Animation { anim: Anim::SlideLeft, edge: End::Out, seconds: 0.5 }, (2.0, 8.0), 1.0, place);
        assert_eq!((at(&slide_out, "x", 7.5), at(&slide_out, "x", 8.0)), (Some(0.5), Some(0.5 - SLIDE)), "off to the left");
        let slide_in = clips(Animation { anim: Anim::SlideLeft, edge: End::In, seconds: 0.5 }, (2.0, 8.0), 1.0, place);
        assert_eq!(at(&slide_in, "x", 2.0), Some(0.5 + SLIDE), "in from the right, moving left");
        let pop = clips(Animation { anim: Anim::Pop, edge: End::In, seconds: 1.0 }, (0.0, 4.0), 1.0, place);
        assert!(at(&pop, "size", 0.7).unwrap() > 1.1 && at(&pop, "size", 0.0) == Some(0.0));
        assert!(Anim::ALL.iter().all(|&anim| clips(Animation { anim, edge: End::Out, seconds: 9.0 }, (0.0, 4.0), 1.0, place).iter().all(|c| c.from >= 0.0 && c.to <= 4.0)), "never longer than the item");
    }

    #[test]
    fn a_trim_keeps_an_animation_on_its_edge() {
        let place = Place::default();
        let mut all = [clips(Animation { anim: Anim::Fade, edge: End::In, seconds: 1.0 }, (0.0, 6.0), 1.0, place), clips(Animation { anim: Anim::Fade, edge: End::Out, seconds: 1.0 }, (0.0, 6.0), 1.0, place)].concat();
        keep_on_edges(&mut all, (0.0, 6.0), (0.5, 4.0));
        assert_eq!((all[0].from, all[0].to, all[1].from, all[1].to), (0.5, 1.5, 3.0, 4.0));
        assert_eq!(all[1].points.last().unwrap().t, 4.0);
    }
}
