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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_in_a_group_and_lengths_go_by_steps() {
        assert!(Group::ALL.iter().all(|g| Look::ALL.iter().any(|l| l.group() == *g)));
        assert_eq!((stepped(0.6), stepped(0.0), stepped(1.13)), (0.5, 0.25, 1.25));
        assert_eq!(serde_json::to_string(&Look::WhipPan).unwrap(), "\"whip_pan\"");
    }
}
