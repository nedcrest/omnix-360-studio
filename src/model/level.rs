//! How loud speech is brought to, and the gain that does it: plain numbers,
//! so the core's sound ops and the measuring in `loudness` share them
//! without the core reaching ffmpeg (it used to, through `loudness`).

/// What a cut's speech is brought to: web video and podcasts sit around
/// −14 to −16 LUFS; −16 leaves room for music under it.
pub const TARGET_LUFS: f64 = -16.0;
/// The most a cut is raised or lowered when normalized, dB.
pub const MOST_GAIN: f64 = 24.0;

/// The gain, dB, that brings `lufs` to the target (unbounded;
/// `normalizing_gain` holds it within `MOST_GAIN`).
pub fn gain_to_target(lufs: f64) -> f64 {
    ((TARGET_LUFS - lufs) * 10.0).round() / 10.0
}

/// `gain_to_target` held within what normalizing may do.
pub fn normalizing_gain(lufs: f64) -> f64 {
    gain_to_target(lufs).clamp(-MOST_GAIN, MOST_GAIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gain_brings_speech_to_the_target_within_bounds() {
        assert_eq!(gain_to_target(-20.04), 4.0);
        assert_eq!(normalizing_gain(-60.0), MOST_GAIN);
        assert_eq!(normalizing_gain(20.0), -MOST_GAIN);
    }
}
