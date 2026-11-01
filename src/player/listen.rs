//! Solo and mute per sound lane, for listening while mixing (gap 46d):
//! hear the Voice alone, or everything but the music. Only what's heard
//! changes; the project and the export don't (as a DAW's mute and solo
//! while mixing), and it's not saved.

use crate::project::SoundTrack;

/// The lanes it covers: the clips' own sound, then each sound track in
/// `SoundTrack::ALL`'s order.
pub const LANES: usize = 1 + SoundTrack::ALL.len();

/// The clip sound lane's place among them.
pub const CLIP_SOUND: usize = 0;

/// A sound track's place among them.
pub fn lane_of(which: SoundTrack) -> usize {
    1 + SoundTrack::ALL.iter().position(|&w| w == which).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Listen {
    pub muted: [bool; LANES],
    pub solo: [bool; LANES],
}

impl Listen {
    /// How much of lane `lane` is heard, 0 or 1: nothing when muted, or
    /// when another lane is soloed and it isn't.
    pub fn heard(&self, lane: usize) -> f32 {
        let soloing = self.solo.iter().any(|&s| s);
        if self.muted[lane] || (soloing && !self.solo[lane]) { 0.0 } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_and_mute_choose_what_is_heard() {
        let mut l = Listen::default();
        assert!((0..LANES).all(|i| l.heard(i) == 1.0));
        let voice = lane_of(SoundTrack::Voice);
        l.solo[voice] = true;
        assert_eq!((l.heard(voice), l.heard(CLIP_SOUND), l.heard(lane_of(SoundTrack::Music))), (1.0, 0.0, 0.0));
        // Two soloed: both heard; a soloed lane muted is still silent.
        l.solo[CLIP_SOUND] = true;
        l.muted[voice] = true;
        assert_eq!((l.heard(voice), l.heard(CLIP_SOUND)), (0.0, 1.0));
        assert_ne!(lane_of(SoundTrack::Music), lane_of(SoundTrack::Sfx));
    }
}
