//! The player's settings, remembered between runs: saved whenever one
//! changes, in the OS's usual place for configuration.

use crate::Resolution;
use crate::ffmpeg::Format;
use crate::stab::Mode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub stabilize: Mode,
    /// The OSV size to play, or the proxy if `None`.
    pub osv_size: Option<u32>,
    pub rolling_shutter: bool,
    pub full_on_pause: bool,
    pub volume: f32,
    pub muted: bool,
    pub resolution: Resolution,
    /// The aspect lock, width and height; free if `None`.
    pub aspect: Option<[u16; 2]>,
    /// Export each cut to its own file.
    pub export_clips: bool,
    /// Apply the cuts' colour effects to those files.
    pub export_graded: bool,
    pub export_format: Format,
    /// The whole film brought to this loudness on export, LUFS; as mixed
    /// if `None`.
    pub export_loudness: Option<f64>,
    /// Looking around while paused keys the view at the playhead.
    pub auto_key: bool,
    /// The colour theme, by iced's name for it (`settings_panel::theme_named`).
    pub theme: String,
    /// Decode 10-bit tracks at 10 bits (`source::set_deep`).
    pub ten_bit: bool,
    /// The library LUT D-Log M clips get when they're added.
    pub dlog_m_lut: Option<String>,
    /// The bin is pinned open (`bin.rs`).
    pub bin_pinned: bool,
    /// The bin view's tile width (`bin_view.rs`).
    pub bin_tile: f32,
    /// The bin view's right side (the file shown): its width.
    pub bin_side: f32,
    /// How tall the timeline's lanes are, as the divider above the
    /// transport row left them (`timeline::Rows`).
    pub timeline_scale: f32,
    /// The view-angles strip shows under the transport row.
    pub view_angles: bool,
    /// The whole interface's size: text, buttons, the timeline (`ui_scale`
    /// in the app, iced's scale factor).
    pub ui_scale: f32,
    /// The voice-over input, by its source name (`capture::Input::name`);
    /// the system's default if `None` or gone.
    pub record_input: Option<String>,
    pub record_channel: crate::capture::Channel,
    /// Three seconds' count before recording.
    pub record_countdown: bool,
    /// The edit plays while recording.
    pub record_play: bool,
    /// How far auto-duck lowers the music under speech, dB.
    pub duck_db: f64,
    /// The timeline's time ruler large (labels you can read) or compact.
    pub ruler_large: bool,
    /// The bars' groups named above their buttons.
    pub bar_labels: bool,
    /// Timeline drags snap to joins, markers and edges (N).
    pub snapping: bool,
    /// The Text, Overlay and SFX 2 lanes folded to a thin row.
    pub fold_text: bool,
    pub fold_overlay: bool,
    pub fold_sfx2: bool,
    /// The markers' row over the ruler folded to a strip.
    pub fold_marks: bool,
    /// The transitions lane folded to a thin row.
    pub fold_transitions: bool,
    /// The Effects row's automation lanes folded away.
    pub fold_effects: bool,
    pub fold_overlay_effects: bool,
    pub fold_text_effects: bool,
    pub fold_overlay_motion: bool,
    pub fold_text_motion: bool,
    /// The Effects panel's groups folded, by name (`effects_view::Group`).
    pub effects_folded: Vec<String>,
    /// The Effects panel wide, its groups framed side by side (gap 98).
    pub effects_wide: bool,
    /// The projects place, chosen in Omnix; none: Dailix's.
    pub projects: Option<PathBuf>,
    /// The edits opened or saved last, the latest first (`RECENT` at most).
    pub recent: Vec<PathBuf>,
    /// The start window's browser of the projects place open.
    pub browse: bool,
}

/// How many edits the start window offers.
pub const RECENT: usize = 5;

/// `edit` the latest of `recent`: once, first, `RECENT` kept.
pub fn remember(recent: &mut Vec<PathBuf>, edit: &std::path::Path) {
    recent.retain(|p| p != edit);
    recent.insert(0, edit.to_owned());
    recent.truncate(RECENT);
}

/// The interface's size range and steps (A− / A+ in the top bar), and its
/// default: a little bigger than iced's own, which read small on the
/// user's 43" screen (2026-10-24).
pub const UI_SCALE: std::ops::RangeInclusive<f32> = 0.75..=2.0;
pub const UI_STEP: f32 = 0.05;
pub const UI_DEFAULT: f32 = 1.15;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            stabilize: Mode::Horizon,
            osv_size: None,
            rolling_shutter: true,
            full_on_pause: true,
            volume: 1.0,
            muted: false,
            resolution: Resolution::P1080,
            aspect: None,
            export_clips: false,
            export_graded: false,
            export_format: Format::Mp4,
            export_loudness: None,
            auto_key: false,
            theme: iced::Theme::KanagawaDragon.to_string(),
            ten_bit: false,
            dlog_m_lut: None,
            bin_pinned: false,
            bin_tile: super::bin_view::TILE_DEFAULT,
            bin_side: super::bin_view::SIDE_DEFAULT,
            timeline_scale: 1.0,
            view_angles: true,
            ui_scale: UI_DEFAULT,
            record_input: None,
            record_channel: crate::capture::Channel::First,
            record_countdown: true,
            record_play: true,
            duck_db: 18.0,
            ruler_large: true,
            bar_labels: false,
            snapping: true,
            fold_text: true,
            fold_overlay: true,
            fold_sfx2: true,
            fold_marks: false,
            fold_transitions: false,
            fold_effects: true,
            fold_overlay_effects: true,
            fold_text_effects: true,
            fold_overlay_motion: true,
            fold_text_motion: true,
            effects_folded: Vec::new(),
            effects_wide: false,
            projects: None,
            recent: Vec::new(),
            browse: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(crate::fsutil::config_dir()?.join("settings.json"))
}

impl Settings {
    /// The saved settings, or the defaults if there are none (or they can't
    /// be read: a broken file shouldn't stop the player).
    pub fn load() -> Settings {
        let Some(path) = path() else { return Settings::default() };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("ignoring {}: {e}", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        let write = || -> anyhow::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
            crate::replace_file(&path, (serde_json::to_string_pretty(self)? + "\n").as_bytes())?;
            Ok(())
        };
        if let Err(e) = write() {
            eprintln!("couldn't save the settings to {}: {e:#}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_edit_comes_first_once_and_five_are_kept() {
        let mut r = Vec::new();
        for n in ["a", "b", "c", "d", "e", "f"] {
            remember(&mut r, std::path::Path::new(n));
        }
        assert_eq!(r, ["f", "e", "d", "c", "b"].map(PathBuf::from));
        remember(&mut r, std::path::Path::new("c"));
        assert_eq!(r, ["c", "f", "e", "d", "b"].map(PathBuf::from));
    }
}
