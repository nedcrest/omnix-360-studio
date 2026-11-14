//! The command line: the `omnix` subcommands and how each runs; `play` opens the player.

use crate::{cache, check, ffmpeg, imu, loudness, player, project, stab, track, view};
use crate::{ExportOpts, Resolution, Target, StitchOpts, Pipeline, export, track_to};
use crate::analyse::{info, needs_360, scan, scan_many, seam_report};
use crate::fsutil::save_png;
use crate::pipeline::{Carry, Shot, encode, load_input, span};
use crate::preview::preview_project;
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use crate::view::{Projection, Reframe, View};

#[derive(Parser)]
#[command(name = player::NAME, version, about = "Omnix: edit home video and DJI Osmo 360 .OSV files (built on rOsmo360)")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
    /// Read clips afresh instead of from the cache of their telemetry and
    /// thumbnails, and cache nothing.
    #[arg(long, global = true)]
    pub(crate) no_cache: bool,
}

/// What `check` reads of a clip: only the attitude (where the camera and
/// the view turn), so only the options that change it; the stitch's own
/// (one lens, rolling shutter, the seam) would do nothing there.
#[derive(clap::Args, Clone)]
pub(crate) struct CheckOpts {
    /// Use this alternative calibration (see `rig`'s `variants`).
    #[arg(long)]
    variant: Option<usize>,
    /// Stabilisation mode [default: each cut's own, else horizon].
    #[arg(long, value_enum)]
    stabilize: Option<crate::stab::Mode>,
    /// Heading smoothing window for `horizon`, in ms either side.
    #[arg(long, default_value_t = 500.0)]
    smooth: f64,
    /// How the IMU's axes sit relative to the rig's.
    #[arg(long, value_enum, default_value_t = crate::stab::ImuAxes::X180)]
    imu_axes: crate::stab::ImuAxes,
    /// Treat the attitude's world z as pointing down rather than up.
    #[arg(long)]
    world_z_down: bool,
    /// Give each lens the other's video track (see `export --help`).
    #[arg(long)]
    swap_lenses: bool,
}

impl From<CheckOpts> for StitchOpts {
    fn from(c: CheckOpts) -> Self {
        StitchOpts { variant: c.variant, stabilize: c.stabilize, smooth: c.smooth, imu_axes: c.imu_axes, world_z_down: c.world_z_down, swap_lenses: c.swap_lenses, ..StitchOpts::default() }
    }
}

/// `omnix export`'s arguments (see `Cmd::Export`).
#[derive(clap::Args)]
pub(crate) struct ExportArgs {
    /// A clip (.OSV), or a project (.json): {"aspect": "9:16", "cuts":
    /// [{"input": "a.OSV", "in": 28, "out": 37, "keyframes": [...]}, ...]}.
    input: PathBuf,
    #[command(flatten)]
    framing: Framing,
    /// Start time in seconds.
    #[arg(short, long)]
    start: Option<f64>,
    /// Duration in seconds.
    #[arg(short, long)]
    duration: Option<f64>,
    /// Picture height for landscape, width for portrait: 1080p is
    /// 1920×1080 at 16:9 and 1080×1920 at 9:16.
    #[arg(short, long, value_enum, default_value_t = Resolution::P1080)]
    resolution: Resolution,
    /// Shape, W:H (16:9, 9:16, 1:1, 4:5, ...); else the reframe's, else 16:9.
    #[arg(short, long)]
    aspect: Option<view::Aspect>,
    /// Exact size, WIDTHxHEIGHT, instead of a resolution and aspect.
    #[arg(long, value_parser = parse_size, conflicts_with_all = ["resolution", "aspect"])]
    size: Option<[u32; 2]>,
    /// The file to write; with --clips, the folder for the clips.
    /// [default: clip.mp4, clip.mov for --format resolve, or the
    /// project's or clip's name for --clips]
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// mp4: H.264 and AAC, plays anywhere. resolve: 10-bit DNxHR HQX and
    /// PCM in MOV, which DaVinci Resolve's free Linux version can read
    /// (about 28 MB a second at 1080p).
    #[arg(long, value_enum, default_value_t = ffmpeg::Format::Mp4)]
    format: ffmpeg::Format,
    /// Write each cut to its own file for another editor, named after
    /// the project: "<name> - 001.mp4" and on, in the folder -o; and each
    /// music clip as "<name> - music 001.wav", each sound effect as
    /// "<name> - sfx 001.wav" (48 kHz 24-bit, unfaded, at the file's
    /// own level).
    #[arg(long)]
    clips: bool,
    /// With --clips, seconds of extra footage either side of each cut
    /// (and song either side of each music clip), where there is some,
    /// for the other editor's transitions.
    #[arg(long, default_value_t = 0.0, requires = "clips")]
    handles: f64,
    /// With --clips, apply the cuts' colour effects too (the whole edit
    /// always has them; left off, the clips are as shot, to grade in
    /// the other editor).
    #[arg(long, requires = "clips")]
    graded: bool,
    /// Bring the whole film's sound to this loudness, LUFS integrated
    /// (YouTube and most streaming: -14; Apple and podcasts: -16;
    /// broadcast: -23). Not with --clips.
    #[arg(long, allow_negative_numbers = true, conflicts_with = "clips")]
    loudness: Option<f64>,
    /// Stitch from the low-resolution proxy (LRF): a quick, soft draft.
    #[arg(long)]
    draft: bool,
    /// Stitch on the CPU, the reference the GPU shader must match.
    /// Without a usable GPU this happens anyway.
    #[arg(long)]
    cpu: bool,
    #[command(flatten)]
    opts: StitchOpts,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Dump the camera rig (DJI Studio's default stitch geometry) as JSON.
    Rig { input: PathBuf },
    /// Where the cache of clips' telemetry and thumbnails is and what it
    /// holds (it makes clips open in a fraction of a second after the first
    /// time); `cache clear` empties it.
    Cache {
        #[command(subcommand)]
        action: Option<CacheAction>,
    },
    /// Dump orientation (q), gyro (g) and accelerometer (a) samples as CSV.
    Imu { input: PathBuf },
    /// The clip's length, frame rate, recording time and what it holds, as JSON.
    Info { input: PathBuf },
    /// List what in a project is likely a mistake, one line each (edit
    /// time, cut, what): cuts out of the order they were filmed in, a cut
    /// repeating footage the cut before shows, crossfades between cuts of one
    /// clip seconds apart (one scene: it stutters), pans faster than 30°/s
    /// (and whether keyframes or the camera turn the view),
    /// cuts under a second, and music ending before the edit or running long
    /// past it.
    Check {
        project: PathBuf,
        /// Pans faster than this, degrees a second over a second, are
        /// listed: they smear and are hard to follow.
        #[arg(long, default_value_t = 30.0)]
        fast_pan: f64,
        #[command(flatten)]
        opts: CheckOpts,
    },
    /// Auto colour (gap 89): each cut measured over a dozen frames across
    /// it (its LUT on), and its levels, exposure and white balance set to
    /// put it right; a line each says what was measured and set.
    Autocolour {
        project: PathBuf,
        /// Only the cuts with these ids, as `describe` says them (all if
        /// none).
        #[arg(long, value_delimiter = ',')]
        cuts: Vec<u32>,
        /// The levels alone (black and white points).
        #[arg(long)]
        levels: bool,
        /// Print the ops it would apply (an ops file for `apply`) instead
        /// of applying them; what it measured goes to stderr.
        #[arg(long)]
        ops: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Bring each cut's sound to -16 LUFS: measure what it plays over its
    /// own stretch and give it the gain (the export's limiter holds the
    /// peaks). Prints a line per cut; writes the project with -o.
    Normalize {
        project: PathBuf,
        /// Only the cuts with these ids, as `describe` says them (all if
        /// none).
        #[arg(long, value_delimiter = ',')]
        cuts: Vec<u32>,
        /// Every clip on this sound track instead of the cuts: music, sfx,
        /// sfx2 or voice.
        #[arg(long, value_parser = sound_track, conflicts_with = "cuts")]
        track: Option<crate::project::SoundTrack>,
        /// Even out each cut's sound first: quiet stretches lifted, loud
        /// ones left alone.
        #[arg(long)]
        even: bool,
        /// Print the ops it would apply (an ops file for `apply`) instead
        /// of applying them; what it measured goes to stderr.
        #[arg(long)]
        ops: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Check the sound against a platform's loudness and the mixing rules:
    /// each clip measured as the export makes it, the whole film, and what
    /// to fix (music too loud under speech, a sound effect over it, quiet
    /// speech, the film off the target).
    Mixcheck {
        project: PathBuf,
        /// The platform's target, LUFS integrated (YouTube -14).
        #[arg(long, allow_negative_numbers = true, default_value_t = -14.0)]
        target: f64,
        /// Also the whole film's loudness every this many seconds.
        #[arg(long)]
        every: Option<f64>,
    },
    /// A song's beats (gap 13): a line per hit, its time in the file and
    /// its strength (0..1 of the song's strongest), those at --threshold
    /// or stronger.
    Beats {
        song: PathBuf,
        #[arg(long, default_value_t = 0.0)]
        threshold: f64,
    },
    /// Lower the music under speech (auto-duck): find where cleaned voices
    /// and Voice takes speak and put volume points on the music's line, this
    /// far down there. Prints where; writes the project with -o.
    Duck {
        project: PathBuf,
        /// How far down, dB.
        #[arg(long, default_value_t = 18.0)]
        depth: f64,
        /// Print the ops it would apply (an ops file for `apply`) instead
        /// of applying them; what it measured goes to stderr.
        #[arg(long)]
        ops: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Apply operations to a project: a JSON list of them, as the player
    /// makes them (`docs/omnix/OPS-API.md`), one step, all or none. Prints
    /// what each did; writes the project with -o. Without -o it is a dry
    /// run: what they would do, or which is refused and why.
    Apply {
        project: PathBuf,
        ops: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// What a project holds, by id: the cuts, sound clips and overlays that
    /// operations name.
    Describe { project: PathBuf },
    /// Makes an edit's card frames (the start pages' project cards):
    /// 16 frames of the film, kept beside its trip's data (or in Omnix's
    /// cache for an edit outside a trip); prints where.
    Card { edit: PathBuf },
    /// Every key and gesture of the window, as Markdown tables (the
    /// guide's reference page, the F1 window's list).
    Keys,
    /// The JSON Schema of an ops file for `apply`: every operation, its
    /// fields and what they mean (for an agent's tools, or an editor).
    OpsSchema,
    /// The looks an operation can set (a join's, an edge's, a cut's
    /// style): every one by its key, with its settings (key, range,
    /// default, choices), and the saved transition templates; with
    /// --template NAME, that template as a `template` op takes it; with
    /// --save-template NAME --project FILE --cut ID, the join before that
    /// cut (its overlap and the edges on both sides) saved as NAME.
    Looks {
        #[arg(long)]
        template: Option<String>,
        #[arg(long, requires_all = ["project", "cut"])]
        save_template: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        cut: Option<u32>,
    },
    /// Omnix's icon and a desktop entry, in the menu and on the desktop,
    /// starting this very binary (Linux).
    Install,
    /// Makes the black picture a blank cut uses (`Blank.png` in DIR, kept
    /// if there) and prints its path, for a cut `add` op: a gap in the
    /// edit for a title, an overlay or a sound.
    Blank { dir: PathBuf },
    /// Makes an edit from a Dailix project's marking (its folder): clips
    /// rated ✓ or ?, their segments as cuts (a clip with none whole), the
    /// voice chosen, flags inside a cut as its markers; written into the
    /// project's 04_project under a new name, printed.
    FromSheet {
        project: PathBuf,
        /// One day only (YYYY-MM-DD).
        #[arg(long)]
        day: Option<String>,
        /// An empty edit, to build from the bin: of the day (--day), or of every day.
        #[arg(long)]
        empty: bool,
    },
    /// Stitch one frame to an equirectangular PNG.
    Stitch {
        input: PathBuf,
        /// Time in seconds.
        #[arg(short, long, default_value_t = 0.0)]
        time: f64,
        #[arg(short, long, default_value = "equirect.png")]
        output: PathBuf,
        /// Output width; height is width / 2.
        #[arg(short, long, default_value_t = 4096)]
        width: usize,
        #[command(flatten)]
        opts: StitchOpts,
    },
    /// Play clips in a window with free look, stabilisation and rolling-shutter correction.
    ///
    /// ← / →: a frame · editing: Shift+← / →: a second, ↑ / ↓: the previous / next cut or marker, Home / End ·
    /// framing: ↑ / ↓: pitch, Shift+← / →: yaw · drag: look around · right-drag or Q/E: roll ·
    /// scroll: zoom · space: play/pause · , .: frame step · S: stabilisation ·
    /// R: rolling shutter · P: projection · M: a marker · Shift+M: mute · 0: reset view · F: fullscreen · Ctrl+O: open ·
    /// K: keyframe the view here · [ ]: previous/next keyframe · Del: delete keyframe · J: its ease ·
    /// C: split the cut · D: select the cut under the playhead · Shift+Del: delete the selected cut · Alt+[ / Alt+]: trim its start / end to
    /// the playhead · Alt+← / Alt+→: move it one place · Ctrl+Z / Ctrl+Shift+Z: undo / redo ·
    /// Ctrl+= / Ctrl+− / Ctrl+0: zoom the timeline in / out / to the whole edit (or scroll on it)
    Play {
        /// Clips to open at start, one cut each in this order, or a saved
        /// project (.json); otherwise use the Open buttons.
        input: Vec<PathBuf>,
        /// Stream the OSV itself at this size (pixels square) instead of the
        /// LRF proxy [default: the last source used].
        #[arg(long)]
        osv_size: Option<u32>,
        /// Video decoder; the first that works, in this order, if omitted.
        #[arg(long, value_enum)]
        hwaccel: Option<ffmpeg::Hwaccel>,
        /// Stabilisation mode [default: the last one used, at first horizon].
        #[arg(long, value_enum)]
        stabilize: Option<stab::Mode>,
        #[arg(long, value_enum, default_value_t = stab::ImuAxes::X180)]
        imu_axes: stab::ImuAxes,
        #[arg(long)]
        world_z_down: bool,
    },
    /// Report how well the seam alignment works over a span: what ghosting
    /// is left (lens disagreement across the blend band) and how much the
    /// shift wobbles from frame to frame.
    Seam {
        input: PathBuf,
        /// Start time in seconds.
        #[arg(short, long, default_value_t = 0.0)]
        start: f64,
        /// Duration in seconds.
        #[arg(short, long, default_value_t = 5.0)]
        duration: f64,
        /// Write the first frame's seam strips, differences and shift field
        /// as images into this directory.
        #[arg(long)]
        dump: Option<PathBuf>,
    },
    /// Stitch a span of the clip to an equirectangular MP4.
    Render {
        input: PathBuf,
        /// Start time in seconds.
        #[arg(short, long, default_value_t = 0.0)]
        start: f64,
        /// Duration in seconds; the rest of the clip if omitted.
        #[arg(short, long)]
        duration: Option<f64>,
        #[arg(short, long, default_value = "equirect.mp4")]
        output: PathBuf,
        /// Output width; height is width / 2.
        #[arg(short, long, default_value_t = 4096)]
        width: usize,
        #[command(flatten)]
        opts: StitchOpts,
    },
    /// Render a framed (reframed) video with sound: a clip with a fixed view
    /// or keyframed views from a JSON file, or a project (.json) of cuts from
    /// several clips, each with its own keyframes.
    ///
    /// For a clip without -s/-d, the keyframes' span is rendered.
    /// Stabilisation defaults to horizon.
    Export(ExportArgs),
    /// Stitch evenly spaced moments onto one contact-sheet PNG, each tile
    /// labelled with its time, to skim a clip. Tiles show the whole sphere,
    /// marked at yaw −90/0/90° and the horizon, unless a framing is given.
    ///
    /// Given a project (.json), one row per cut (more with --every), each
    /// tile framed as the export frames it and labelled with the cut's
    /// number and the clip time.
    ///
    /// Uses the proxy (LRF) unless --full. Stabilisation defaults to horizon.
    Preview {
        input: PathBuf,
        #[command(flatten)]
        framing: Framing,
        /// Start time in seconds.
        #[arg(short, long)]
        start: Option<f64>,
        /// Duration in seconds.
        #[arg(short, long)]
        duration: Option<f64>,
        /// Seconds between tiles; else 24 tiles, or the keyframes' times.
        #[arg(long)]
        every: Option<f64>,
        /// Tile width in pixels.
        #[arg(long, default_value_t = 640)]
        tile: usize,
        #[arg(long, default_value_t = 4)]
        columns: usize,
        #[arg(short, long, default_value = "preview.png")]
        output: PathBuf,
        /// Stitch from the full-resolution lens tracks instead of the proxy.
        #[arg(long)]
        full: bool,
        #[command(flatten)]
        opts: StitchOpts,
    },
    /// One frame of a project's edit at --at seconds, as the export makes
    /// it: framed, graded with its LUT, styled, its edge look, faded, with
    /// overlays and text. From the proxy unless --full.
    Frame {
        project: PathBuf,
        /// Edit time, seconds.
        #[arg(long)]
        at: f64,
        #[arg(long, default_value_t = 1280)]
        width: usize,
        #[arg(short, long, default_value = "frame.png")]
        output: PathBuf,
        #[arg(long)]
        full: bool,
        #[command(flatten)]
        opts: StitchOpts,
    },
    /// The scopes of one frame of a project's edit at --at seconds, as
    /// `frame` draws it: its darkest and brightest, what's crushed and
    /// blown, the luma in sixteen bands.
    Scopes {
        project: PathBuf,
        #[arg(long)]
        at: f64,
        #[arg(long)]
        full: bool,
        #[command(flatten)]
        opts: StitchOpts,
    },
    /// Summarise the clip over time as CSV, to find moments worth a look:
    /// the camera's heading and tilt, how fast it turns or shakes, and how
    /// loud it is.
    ///
    /// Given several clips or folders (every .OSV in them and the folders
    /// inside), writes a CSV per clip into the folder --output, named after
    /// the clip, a few clips at a time.
    Scan {
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// The folder for a CSV per clip; one clip without it goes to the
        /// standard output.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Clips scanned at once into --output.
        #[arg(long, default_value_t = 4)]
        jobs: usize,
        /// Seconds per row.
        #[arg(long, default_value_t = 1.0)]
        every: f64,
        /// Add columns from the picture: sky (% of the upper half that is
        /// sky), skyline (its median elevation, degrees) and open (% of
        /// directions with a view out to the horizon). Stitches the proxy's
        /// keyframes small, which takes seconds to a minute where plain scan
        /// reads only telemetry.
        #[arg(long)]
        image: bool,
    },
    /// Track a coloured subject (a jacket, a tent) through a clip, as CSV for
    /// keyframes to follow ("follow": "track.csv"): per sample its direction
    /// in the camera (x, y, z), its yaw and pitch as a level view would look
    /// at it (directions in the world, like scan's heading), and its size in
    /// % of the sphere. Empty where it was lost.
    Track {
        input: PathBuf,
        /// The subject's colour, #rrggbb (from a preview or a photo of it).
        #[arg(long)]
        colour: String,
        /// How far the hue may differ, degrees either side.
        #[arg(long, default_value_t = 18.0)]
        tolerance: f64,
        /// Where the subject is at the start, as level yaw,pitch (it may not
        /// be the biggest patch of its colour); else it starts from that.
        #[arg(long, value_parser = parse_pair, allow_hyphen_values = true)]
        near: Option<(f64, f64)>,
        /// Start time in seconds.
        #[arg(short, long)]
        start: Option<f64>,
        /// Seconds to track; to the end if omitted.
        #[arg(short, long)]
        duration: Option<f64>,
        /// Seconds between samples.
        #[arg(long, default_value_t = 0.1)]
        every: f64,
    },
}

#[derive(Subcommand)]
pub(crate) enum CacheAction {
    /// Cache clips ahead of time, so they open at once later: each .OSV
    /// given, and every .OSV in the folders given (and folders inside).
    Add {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Read every clip afresh, even those already cached.
        #[arg(long)]
        force: bool,
    },
    /// Move the cache to another folder (a disk with room, say), and keep it
    /// there from now on; OMNIX_CACHE, if set, still wins for a run.
    Move {
        #[arg(required_unless_present = "default")]
        folder: Option<PathBuf>,
        /// Back to the system's cache folder (~/.cache/omnix on Linux).
        #[arg(long, conflicts_with = "folder")]
        default: bool,
    },
    /// Delete the cached entries: all of them, or those unused for a while.
    Clear {
        /// Only entries no clip has used for this many days.
        #[arg(long)]
        older_than: Option<f64>,
    },
}

/// `cache add`: caches each clip under `paths` that isn't yet (all of them
/// with `force`), saying how it went one line a clip. A clip that fails is
/// reported and skipped.
pub(crate) fn cache_clips(paths: &[PathBuf], force: bool) -> Result<()> {
    let clips = crate::fsutil::osv_under(paths)?;
    let dir = cache::dir().context("no cache folder (HOME is not set)")?;
    eprintln!("caching {} clips in {}", clips.len(), dir.display());
    let (mut added, mut failed) = (0, 0);
    for (i, clip) in clips.iter().enumerate() {
        let name = clip.file_name().map_or_else(|| clip.display().to_string(), |n| n.to_string_lossy().into_owned());
        let at = std::time::Instant::now();
        let was = !force && cache::has(clip);
        if force {
            cache::forget(clip)?;
        }
        match player::cache_clip(clip) {
            Ok(()) if was => println!("[{}/{}] {name}: already cached", i + 1, clips.len()),
            Ok(()) => {
                added += 1;
                println!("[{}/{}] {name}: cached in {:.0} s", i + 1, clips.len(), at.elapsed().as_secs_f64());
            }
            Err(e) => {
                failed += 1;
                println!("[{}/{}] {name}: failed: {e:#}", i + 1, clips.len());
            }
        }
    }
    eprintln!("{added} added, {} already there, {failed} failed", clips.len() - added - failed);
    Ok(())
}

pub(crate) fn parse_pair(s: &str) -> Result<(f64, f64), String> {
    let (a, b) = s.split_once(',').ok_or("give YAW,PITCH")?;
    Ok((a.trim().parse().map_err(|e| format!("{a:?}: {e}"))?, b.trim().parse().map_err(|e| format!("{b:?}: {e}"))?))
}

/// How `export` and `preview` frame the sphere: keyframes from a file or a
/// fixed view.
#[derive(Args)]
pub(crate) struct Framing {
    /// Keyframed views as JSON: {"aspect": "9:16", "keyframes": [{"t": 12,
    /// "yaw": 40, "pitch": -5, "fov": 80, "projection": "perspective", "ease":
    /// "smooth"}, ...]}. Keyframes may leave out what doesn't change.
    #[arg(long, conflicts_with = "view")]
    pub(crate) reframe: Option<PathBuf>,
    /// A fixed view: YAW,PITCH[,ROLL[,FOV]] in degrees.
    #[arg(long, value_parser = parse_view, allow_hyphen_values = true)]
    pub(crate) view: Option<Angles>,
    /// Projection for --view.
    #[arg(long, value_enum, default_value_t = Projection::Perspective, requires = "view")]
    pub(crate) projection: Projection,
}

impl Framing {
    pub(crate) fn reframe(&self) -> Result<Option<Reframe>> {
        if let Some(path) = &self.reframe {
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let mut reframe: Reframe = serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
            // Tracks it follows are named relative to it, as in a project.
            let dir = path.parent().unwrap_or(Path::new(""));
            for track in reframe.keyframes.tracks_mut() {
                *track = dir.join(&*track);
            }
            return Ok(Some(reframe));
        }
        Ok(self.view.as_ref().map(|Angles(v)| {
            let home = View::home(self.projection);
            Reframe::fixed(View { yaw: v[0], pitch: v[1], roll: v.get(2).copied().unwrap_or(home.roll), fov: v.get(3).copied().unwrap_or(home.fov), ..home })
        }))
    }
}

/// --view's YAW,PITCH[,ROLL[,FOV]]. A newtype, since clap takes a bare
/// `Vec` for a flag given several times.
#[derive(Clone)]
pub(crate) struct Angles(Vec<f64>);

pub(crate) fn parse_view(s: &str) -> Result<Angles, String> {
    let v = s.split(',').map(|x| x.trim().parse::<f64>().map_err(|e| format!("{x:?}: {e}"))).collect::<Result<Vec<_>, _>>()?;
    if !(2..=4).contains(&v.len()) {
        return Err("expected YAW,PITCH[,ROLL[,FOV]]".into());
    }
    Ok(Angles(v))
}

pub(crate) fn parse_size(s: &str) -> Result<[u32; 2], String> {
    let (w, h) = s.split_once(['x', 'X']).ok_or("expected WIDTHxHEIGHT")?;
    let (w, h): (u32, u32) = (w.parse().map_err(|e| format!("{e}"))?, h.parse().map_err(|e| format!("{e}"))?);
    if w < 16 || h < 16 || w % 2 == 1 || h % 2 == 1 {
        return Err("width and height must be even and at least 16".into());
    }
    Ok([w, h])
}

/// A project sets its own cuts and views: the options for one clip are
/// refused with it rather than ignored.
fn clip_options_unused(framing: &Framing, start: Option<f64>, duration: Option<f64>) -> Result<()> {
    if framing.reframe.is_some() || framing.view.is_some() || start.is_some() || duration.is_some() {
        bail!("a project sets its own cuts and views; leave out --reframe, --view, -s and -d");
    }
    Ok(())
}

/// Progress on the terminal: frames done, out of how many, and the rate.
pub(crate) fn print_progress() -> impl FnMut(usize, usize) -> Result<()> {
    let began = Instant::now();
    move |done, total| {
        if done.is_multiple_of(10) || done == total {
            eprint!("\rframe {done}/{total}  {:.2} fps", done as f64 / began.elapsed().as_secs_f64().max(1e-3));
        }
        if done == total {
            eprintln!();
        }
        Ok(())
    }
}

/// The command line, as the player wants it when started as a macOS app:
/// Finder starts it with nothing (or, on some versions, only a `-psn_`
/// process serial number), which opens the player, as the AppImage's AppRun
/// does on Linux. From a terminal nothing still prints the help.
/// On Windows, double-clicked in Explorer (or a clip or project opened with
/// it) the same: the player, with the file if there is one.
pub(crate) fn app_args() -> Vec<std::ffi::OsString> {
    let mut args: Vec<_> = std::env::args_os().filter(|a| !a.to_string_lossy().starts_with("-psn_")).collect();
    let in_app = std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"));
    let explorer = from_explorer();
    if (cfg!(target_os = "macos") && in_app || explorer) && args.len() == 1 {
        args.push("play".into());
    } else if explorer && args.len() == 2 {
        args.insert(1, "play".into());
    }
    args
}

/// Whether Windows started us from Explorer rather than a terminal: then the
/// console it made is ours alone (a terminal shares its own), and it's
/// closed, so the player doesn't sit beside a black window. Errors then go
/// to a message box (`main`), there being nowhere to print them.
#[cfg(windows)]
pub(crate) fn from_explorer() -> bool {
    unsafe extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    static ALONE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALONE.get_or_init(|| {
        let mut ids = [0u32; 2];
        let alone = unsafe { GetConsoleProcessList(ids.as_mut_ptr(), 2) } == 1;
        if alone {
            unsafe { FreeConsole() };
        }
        alone
    })
}

#[cfg(not(windows))]
pub(crate) fn from_explorer() -> bool {
    false
}

pub fn main() -> Result<()> {
    // Output piped into something that stopped reading (`| head`) is no
    // failure to report: the reader has what it wanted.
    match run() {
        Err(e) if e.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe)) => Ok(()),
        Err(e) if from_explorer() => {
            rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_title(player::NAME).set_description(format!("{e:#}")).show();
            Err(e)
        }
        r => r,
    }
}

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse_from(app_args());
    if cli.no_cache {
        cache::disable();
    }
    match cli.cmd {
        Cmd::Cache { action: None } => cache::report()?,
        Cmd::Cache { action: Some(CacheAction::Clear { older_than }) } => cache::clear(older_than)?,
        Cmd::Cache { action: Some(CacheAction::Add { paths, force }) } => cache_clips(&paths, force)?,
        Cmd::Cache { action: Some(CacheAction::Move { folder, .. }) } => cache::move_to(folder)?,
        Cmd::Rig { input } => {
            needs_360(&input, "a camera rig")?;
            let rig = cache::telemetry(&input, &|_| {})?.rig.context("no camera rig in file")?;
            println!("{}", serde_json::to_string_pretty(&rig)?);
        }
        Cmd::Imu { input } => imu_csv(&input)?,
        Cmd::Info { input } => info(&input)?,
        Cmd::Check { project, fast_pan, opts } => check::check(&project, &opts.into(), fast_pan)?,
        Cmd::Autocolour { project, cuts, levels, ops, output } => crate::autocolour::run(&project, &cuts, levels, ops, output.as_deref())?,
        Cmd::Normalize { project, cuts, track, even, ops, output } => loudness::normalize(&project, &cuts, track, even, ops, output.as_deref())?,
        Cmd::Beats { song, threshold } => print_beats(&song, threshold)?,
        Cmd::Duck { project, depth, ops, output } => crate::duck::run(&project, depth, ops, output.as_deref())?,
        Cmd::Apply { project, ops, output } => crate::apply::run(&project, &ops, output.as_deref())?,
        Cmd::Describe { project } => crate::apply::describe(&project)?,
        Cmd::Card { edit } => println!("{}", crate::cards::make(&edit)?.display()),
        Cmd::OpsSchema => println!("{}", crate::apply::schema()),
        Cmd::Keys => print!("{}", crate::keys::omnix_page()),
        Cmd::Looks { save_template: Some(name), project: Some(project), cut: Some(cut), .. } => crate::apply::save_template(&project, cut, &name)?,
        Cmd::Looks { template, .. } => print!("{}", crate::apply::looks(template.as_deref())?),
        Cmd::Install => install()?,
        Cmd::Blank { dir } => println!("{}", crate::blank::ensure(&dir)?.display()),
        Cmd::FromSheet { project, day, empty: true } => println!("{}", crate::from_sheet::write_empty(&project, day.as_deref())?.display()),
        Cmd::FromSheet { project, day, empty: false } => println!("{}", crate::from_sheet::write(&project, day.as_deref())?.display()),
        Cmd::Mixcheck { project, target, every } => crate::mixmeasure::run_cli(&project, target, every)?,
        Cmd::Play { input, osv_size, hwaccel, stabilize, imu_axes, world_z_down } => {
            player::run(input, player::Launch { osv_size, hw: hwaccel, stabilize, imu_axes, world_z_down })?;
        }
        Cmd::Seam { input, start, duration, dump } => needs_360(&input, "a seam").and_then(|_| seam_report(&input, start, duration, dump.as_deref()))?,
        Cmd::Stitch { input, time, output, width, opts } => stitch_png(&input, time, &output, width, &opts)?,
        Cmd::Render { input, start, duration, output, width, opts } => render_equirect(&input, start, duration, &output, width, &opts)?,
        Cmd::Export(args) => export_cmd(args)?,
        Cmd::Scopes { project, at, full, opts } => {
            let frame = crate::preview::frame_at(&project::Project::load(&project)?, &opts, at, 640, !full)?;
            print!("{}", crate::scopes::describe(&crate::scopes::of_frame(&frame)));
        }
        Cmd::Frame { project, at, width, output, full, opts } => frame_png(&project, at, width, &output, full, &opts)?,
        Cmd::Preview { input, framing, start, duration, every, tile, columns, output, full, opts } if is_project(&input) => {
            clip_options_unused(&framing, start, duration)?;
            let project = project::Project::load(&input)?;
            let (sheet, tiles) = preview_project(&project, &opts, every, tile, columns.max(1), !full)?;
            save_png(&output, &sheet)?;
            eprintln!("wrote {}: {tiles} tiles of {} cuts", output.display(), project.cuts.len());
        }
        Cmd::Preview { input, framing, start, duration, every, tile, columns, output, full, opts } => {
            let (sheet, times) = crate::preview::preview_clip(&input, framing.reframe()?.as_ref(), &opts, (start, duration), every, tile, columns, !full)?;
            save_png(&output, &sheet)?;
            eprintln!("wrote {}: {} tiles at {}", output.display(), times.len(), times.iter().map(|t| format!("{t:.1}")).collect::<Vec<_>>().join(" "));
        }
        Cmd::Scan { inputs, output, jobs, every, image } => match (&inputs[..], output) {
            ([input], None) if !input.is_dir() => needs_360(input, "motion data").and_then(|_| scan(input, every, image, &mut std::io::stdout().lock()))?,
            (_, None) => bail!("several clips (or a folder) need --output, a folder for a CSV per clip"),
            (_, Some(dir)) => scan_many(&inputs, &dir, jobs, every, image)?,
        },
        Cmd::Track { input, colour, tolerance, near, start, duration, every } => track_cmd(&input, &colour, tolerance, near, (start, duration), every)?,
    }
    Ok(())
}

/// `omnix imu`: the clip's motion data as CSV.
fn imu_csv(input: &Path) -> Result<()> {
    needs_360(input, "motion data")?;
    let imu = imu::read(&load_input(input, &|_| {})?);
    println!("kind,t,w,x,y,z");
    for (t, q) in &imu.quats {
        println!("q,{t},{},{},{},{}", q[0], q[1], q[2], q[3]);
    }
    for (kind, series) in [("g", &imu.gyro), ("a", &imu.accel)] {
        for (t, v) in series {
            println!("{kind},{t},,{},{},{}", v[0], v[1], v[2]);
        }
    }
    Ok(())
}

/// `omnix beats`: a song's hits at `threshold` or stronger, a line each.
fn print_beats(song: &Path, threshold: f64) -> Result<()> {
    let beats = crate::beats::read(song)?;
    for b in crate::beats::at_least(&beats, threshold) {
        println!("{:8.3} s  {:.2}", b.t, b.strength);
    }
    eprintln!("{} of {} hits at {threshold} or stronger", crate::beats::at_least(&beats, threshold).count(), beats.len());
    Ok(())
}

/// `omnix install`: the icon and a desktop entry starting this binary.
fn install() -> Result<()> {
    let written = crate::desktop::install("omnix", &crate::desktop::omnix_entry(&std::env::current_exe()?), crate::desktop::OMNIX_ICON).map_err(|e| anyhow::anyhow!("{e}"))?;
    for f in written {
        println!("{}", f.display());
    }
    Ok(())
}

/// `omnix stitch`: one equirect frame as a PNG.
fn stitch_png(input: &Path, time: f64, output: &Path, width: usize, opts: &StitchOpts) -> Result<()> {
    let p = Pipeline::new(input, opts, stab::Mode::Off)?;
    let time = p.snap(time);
    let out = p.render(&p.stills(time)?, time, &mut Carry::default(), &Shot::Equirect(width));
    save_png(output, &out)?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

/// `omnix render`: an equirect video with its sound.
fn render_equirect(input: &Path, start: f64, duration: Option<f64>, output: &Path, width: usize, opts: &StitchOpts) -> Result<()> {
    let p = Pipeline::new(input, opts, stab::Mode::Off)?;
    let start = p.snap(start);
    let total = duration.map(|d| (d * p.fps).round() as usize);
    let mut encoder = ffmpeg::Encoder::new(output, (width, width / 2), p.fps, &[ffmpeg::Sound::plain(input, start, duration)], None, ffmpeg::Format::Mp4, false)?;
    let began = Instant::now();
    let mut tick = |n: usize| {
        if n.is_multiple_of(10) {
            eprint!("\rframe {n}{}  {:.2} fps", total.map_or(String::new(), |t| format!("/{t}")), n as f64 / began.elapsed().as_secs_f64());
        }
        Ok(())
    };
    encode(&p, start, total, &mut encoder, |_| Shot::Equirect(width), &mut tick)?;
    eprintln!();
    encoder.finish()?;
    eprintln!("wrote {}", output.display());
    Ok(())
}

/// `omnix export`: a clip or a project framed and graded, to one file or
/// a file per cut.
fn export_cmd(args: ExportArgs) -> Result<()> {
    let ExportArgs { input, framing, start, duration, resolution, aspect, size, output, format, clips, handles, graded, loudness, draft, cpu, opts } = args;
    let name = input.file_stem().map_or_else(|| "export".to_owned(), |s| s.to_string_lossy().into_owned());
    let project = if is_project(&input) {
        clip_options_unused(&framing, start, duration)?;
        project::Project::load(&input)?
    } else {
        // One clip is a project of one cut.
        let reframe = framing.reframe()?.unwrap_or_else(|| Reframe::fixed(View::home(Projection::Perspective)));
        let (start, end) = span(&input, start, duration, Some(&reframe))?;
        project::Project { aspect: reframe.aspect, cuts: vec![project::Cut { keyframes: reframe.keyframes, ..project::Cut::new(input, start, end) }], ..Default::default() }
    };
    let aspect = aspect.or(project.aspect).unwrap_or(view::Aspect(16, 9));
    let [w, h] = size.unwrap_or_else(|| aspect.size(resolution.short_side())).map(|v| v as usize);
    let target = if clips {
        Target::Clips { dir: output.unwrap_or_else(|| PathBuf::from(&name)), name, handles, graded }
    } else {
        Target::One(output.unwrap_or_else(|| PathBuf::from(format!("clip.{}", format.extension()))))
    };
    let written = export(&project, &opts, &ExportOpts { size: (w, h), draft, cpu, format, target, loudness }, &mut print_progress())?;
    for path in &written {
        // The music and sound effects beside clips are sound only.
        let size = if path.extension().is_some_and(|e| e == "wav") { String::new() } else { format!(" ({w}×{h})") };
        eprintln!("wrote {}{size}", path.display());
    }
    eprintln!("{} file(s), {:.2} s of edit", written.len(), project.duration());
    Ok(())
}

/// `omnix frame`: one frame of the edit at `at` as the export makes it.
fn frame_png(project: &Path, at: f64, width: usize, output: &Path, full: bool, opts: &StitchOpts) -> Result<()> {
    let frame = crate::preview::frame_at(&project::Project::load(project)?, opts, at, width, !full)?;
    save_png(output, &frame)?;
    eprintln!("wrote {} ({}×{})", output.display(), frame.width, frame.height);
    Ok(())
}

/// `omnix track`: a colour followed through a 360 clip, a CSV line per
/// sample.
fn track_cmd(input: &Path, colour: &str, tolerance: f64, near: Option<(f64, f64)>, (start, duration): (Option<f64>, Option<f64>), every: f64) -> Result<()> {
    needs_360(input, "a sphere to track in")?;
    let near = near.map(|(yaw, pitch)| track::Near::Level(yaw, pitch));
    let (found, rows) = track_to(input, &track::Colour::parse(colour, tolerance)?, near, start, duration, every, &mut std::io::stdout().lock(), &mut |_, _| Ok(()))?;
    eprintln!("found the colour in {found} of {rows} samples");
    Ok(())
}

pub(crate) fn is_project(input: &Path) -> bool {
    input.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// A sound track by its lane's name, any case, spaces left out ("sfx2").
fn sound_track(s: &str) -> Result<crate::project::SoundTrack, String> {
    let squash = |t: &str| t.to_lowercase().replace(' ', "");
    crate::project::SoundTrack::ALL.into_iter().find(|w| squash(w.name()) == squash(s)).ok_or_else(|| format!("no sound track {s:?}: music, sfx, sfx2 or voice"))
}
