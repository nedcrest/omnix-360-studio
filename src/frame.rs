//! An ordinary (flat) clip's frame (gap 107): which part of its picture
//! the film shows, in plain numbers — its centre across and down from the
//! picture's middle and its zoom, in % of the picture's height, and its
//! rotation in degrees. Always the film's shape, so no black bars at 100 %
//! and more. A cut's frame is its base (`Cut::frame`); the Frame lanes
//! animate it (step b). The same crop the 360 view gives a flat clip
//! (`View::camera`), so the player, the export and the shader need nothing
//! new.

use crate::view::{FLAT_DEGREES, Projection, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// The frame's centre right of the picture's middle, % of its height.
    #[serde(default)]
    pub x: f64,
    /// The frame's centre below the picture's middle, % of its height.
    #[serde(default)]
    pub y: f64,
    /// 100 shows the picture's whole height; 200 half of it, twice as big.
    #[serde(default = "whole")]
    pub zoom: f64,
    /// Turned clockwise, degrees.
    #[serde(default)]
    pub rotation: f64,
}

fn whole() -> f64 {
    100.0
}

impl Default for Frame {
    fn default() -> Frame {
        Frame { x: 0.0, y: 0.0, zoom: whole(), rotation: 0.0 }
    }
}

/// The frame's values by name, as ops, `describe` and the lanes call them.
pub const NAMES: [&str; 4] = ["x", "y", "zoom", "rotation"];

/// The values as automation clips name them (gap 107, step b): `frame.`
/// first, as an overlay's place has an `x` of its own.
pub const PARAMS: [&str; 4] = ["frame.x", "frame.y", "frame.zoom", "frame.rotation"];

/// The values as the lanes and the panel call them.
pub const LABELS: [&str; 4] = ["Across %", "Down %", "Zoom %", "Rotation °"];

/// Each value's range, in `NAMES` order: the centre within a picture's
/// height either way, from the whole picture to an eighth of it, a turn
/// either way.
pub const RANGES: [(f64, f64); 4] = [(-100.0, 100.0), (-100.0, 100.0), (100.0, 800.0), (-180.0, 180.0)];

impl Frame {
    /// Value `name` (one of `NAMES`).
    pub fn get(&self, name: &str) -> Option<f64> {
        Some(match name {
            "x" => self.x,
            "y" => self.y,
            "zoom" => self.zoom,
            "rotation" => self.rotation,
            _ => return None,
        })
    }

    /// Its values in `NAMES` order.
    pub fn values(&self) -> [f64; 4] {
        [self.x, self.y, self.zoom, self.rotation]
    }

    /// The frame of `values` (`NAMES` order).
    pub fn from_values([x, y, zoom, rotation]: [f64; 4]) -> Frame {
        Frame { x, y, zoom, rotation }
    }

    /// Whether every value is within its range, or the first that isn't.
    pub fn check(&self) -> Result<(), String> {
        for (name, (lo, hi)) in NAMES.iter().zip(RANGES) {
            let v = self.get(name).expect("a name of NAMES");
            if !(lo..=hi).contains(&v) {
                return Err(format!("the frame's {name} {v} is outside {lo}..{hi}"));
            }
        }
        Ok(())
    }

    /// The view that shows it: a flat clip's crop is a perspective view
    /// read as a crop (`View::camera`, `FLAT_DEGREES` a picture's height).
    pub fn view(&self) -> View {
        let d = FLAT_DEGREES;
        View { yaw: self.x / 100.0 * d, pitch: -self.y / 100.0 * d, roll: self.rotation, fov: d * 100.0 / self.zoom, ..View::home(Projection::Perspective) }
    }

    /// The frame moved by `screen` (% of the picture's height, across and
    /// down the screen: turned with the frame onto the picture) and turned
    /// `turn` degrees more: what a drag over the view does (the audit,
    /// 2026-11-12: a turned frame moved the wrong way).
    pub fn moved(&self, [dx, dy]: [f64; 2], turn: f64) -> Frame {
        let (s, c) = self.rotation.to_radians().sin_cos();
        Frame { x: self.x + c * dx - s * dy, y: self.y + s * dx + c * dy, rotation: self.rotation + turn, ..*self }
    }

    /// The frame in words, for `describe`.
    pub fn said(&self) -> String {
        format!("x {:.1} %, y {:.1} %, zoom {:.1} %, rotation {:.1}°", self.x, self.y, self.zoom, self.rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_the_crop_it_says() {
        let crop = |f: Frame| f.view().camera().crop;
        let whole = crop(Frame::default());
        assert_eq!((whole.centre, whole.half), ([0.0, 0.0], 0.5), "the whole height");
        let zoomed = crop(Frame { x: 10.0, y: -20.0, zoom: 200.0, rotation: 0.0 });
        assert!((zoomed.half - 0.25).abs() < 1e-6, "twice as big: half the height");
        assert!((zoomed.centre[0] - 0.1).abs() < 1e-6 && (zoomed.centre[1] + 0.2).abs() < 1e-6, "right 10 %, up 20 %: {:?}", zoomed.centre);
        assert!(Frame { zoom: 50.0, ..Frame::default() }.check().is_err() && Frame::default().check().is_ok());
        // Dragged across the screen: across the picture, or down it when turned a quarter.
        let f = Frame::default().moved([10.0, 0.0], 0.0);
        assert_eq!((f.x, f.y), (10.0, 0.0));
        let turned = Frame { rotation: 90.0, ..Frame::default() }.moved([10.0, 0.0], 5.0);
        assert!(turned.x.abs() < 1e-9 && (turned.y - 10.0).abs() < 1e-9 && turned.rotation == 95.0, "{turned:?}");
    }
}
