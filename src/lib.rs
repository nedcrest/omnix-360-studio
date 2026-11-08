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
mod grade;
mod ffmpeg;
mod hdr;
mod imu;
mod loudness;
mod lut;
mod overlay;
mod player;
/// Omnix's player for another program's window (Dailix).
pub use player::viewer;
mod project;
mod ring;
mod sheet;
mod sky;
mod stab;
mod stitch;
mod text;
mod track;
mod view;
mod waveform;
pub mod cli;
mod analyse;
mod export;
pub mod desktop;
pub mod sibling;
pub mod transition;
mod fsutil;
mod pipeline;
mod preview;

// What the modules reach for at the crate's root, as when it was all main.rs.
pub(crate) use analyse::track_to;
pub(crate) use export::{ExportOpts, Resolution, Target, cut_pipelines, export};
pub(crate) use fsutil::replace_file;
pub use fsutil::{AppDir, DAILIX, app_dir, dailix_projects, dailix_settings, projects_place_in};
pub(crate) use pipeline::{Pipeline, StitchOpts, load_input};
