//! Omnix (built on rOsmo360): a home-video and DJI Osmo 360 editor. The
//! library holds everything; the `omnix` binary is its command line
//! (`cli::main`), and `play` opens the player.
//!
//! Map and plan: `docs/omnix/ARCHITECTURE.md`.

mod align;
mod apply;
mod blank;
mod cache;
mod capture;
mod check;
mod dissolve;
mod dress;
mod dji;
mod duck;
mod mixdown;
mod notes;
/// A Dailix project's marking made into an edit (`omnix from-sheet`).
pub mod from_sheet;
mod model;
mod lufs;
mod mixcheck;
mod mixmeasure;
mod exposure;
mod frame;
mod speed;
mod grade;
mod ffmpeg;
mod hdr;
mod imu;
mod loudness;
mod lut;
mod mask;
mod overlay;
mod player;
/// Omnix's player for another program's window (Dailix).
pub use player::viewer;
mod project;
mod ring;
mod sheet;
mod scopes;
mod sky;
mod stab;
mod stitch;
mod text;
mod track;
mod view;
mod waveform;
mod beats;
pub mod cli;
pub mod keys;
pub mod reference;
mod analyse;
mod export;
pub mod desktop;
pub mod sibling;
pub mod transition;
mod template;
mod style;
mod autocolour;
mod automation;
mod animate;
mod fsutil;
mod pipeline;
mod preview;
/// Project cards (gap 110): an edit's frames, for the start pages.
pub mod cards;
#[cfg(test)]
mod time_snapshot;

// What the modules reach for at the crate's root, as when it was all main.rs.
pub(crate) use analyse::track_to;
pub(crate) use export::{ExportOpts, Resolution, Target, cut_pipelines, export};
pub(crate) use fsutil::replace_file;
pub use fsutil::{AppDir, DAILIX, app_dir, dailix_projects, dailix_settings, projects_place_in};

/// Omnix's sound tools as Dailix uses them (gap 77): ffmpeg as Omnix finds
/// it, and its one loudness meter, so a voice measured there reads the
/// same here.
pub mod sound {
    pub use crate::ffmpeg::tool;

    /// `file`'s whole loudness, LUFS; none where it's silent.
    pub fn loudness(file: &std::path::Path) -> anyhow::Result<Option<f64>> {
        crate::loudness::measure(file, 0.0, 86_400.0, None)
    }

    /// Where a cleaned voice holds speech, seconds into it: the finder the
    /// auto-duck uses (gap 83), its waveform cached as the timeline's.
    pub fn speech(file: &std::path::Path) -> anyhow::Result<Vec<(f64, f64)>> {
        use crate::waveform::{WAVE_STEP, Waveform};
        let wave = Waveform::read(file, false)?;
        let whole = crate::duck::Source { file: file.to_owned(), start: 0.0, end: wave.peaks.len() as f64 * WAVE_STEP, at: 0.0 };
        Ok(crate::duck::speech(&wave.peaks, WAVE_STEP, &whole))
    }
}
pub(crate) use pipeline::{Pipeline, StitchOpts, load_input};
