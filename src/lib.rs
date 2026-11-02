//! Omnix (built on rOsmo360): a home-video and DJI Osmo 360 editor. The
//! library holds everything; the `rosmo360` binary is its command line
//! (`cli::main`), and `play` opens the player.
//!
//! Map and plan: `docs/omnix/ARCHITECTURE.md`.

mod align;
mod apply;
mod cache;
mod capture;
mod check;
mod dissolve;
mod dji;
mod duck;
mod mixdown;
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
mod project;
mod ring;
mod sheet;
mod sky;
mod stab;
mod stitch;
mod text;
mod track;
mod view;
pub mod cli;
mod analyse;
mod export;
mod fsutil;
mod pipeline;
mod preview;

// What the modules reach for at the crate's root, as when it was all main.rs.
pub(crate) use analyse::track_to;
pub(crate) use export::{ExportOpts, Resolution, Target, cut_pipelines, export};
pub(crate) use fsutil::replace_file;
pub(crate) use pipeline::{Pipeline, StitchOpts, load_input};
