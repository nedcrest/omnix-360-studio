//! Real-time 360 player: an iced window with the stitched view, an open-file
//! dialog and playback controls. Free look on all three axes, stabilisation
//! and rolling-shutter correction run in the view's shader.

mod audio;
use crate::model::edit;
mod export;
mod gpu;
mod audio_panel;
mod bin;
mod bin_view;
mod bin_player;
pub mod viewer;
mod ask;
mod export_panel;
mod handles;
mod info;
mod settings_panel;
mod meter;

mod playback;
pub mod render;
mod settings;
mod sound_ui;
mod source;
mod thumbs;
mod effects;
pub mod music;
mod timeline;
mod divider;
mod angles;
mod icons;
mod sphere;
mod media;
mod keys;
mod transport;
mod panels;
mod bars;
mod framing;
mod jobs;
mod file;
mod tracks;
mod overlay_ui;
mod cuts;
mod record_ui;
mod scrub;
mod shortcuts;
mod marks;
use crate::model::selection::Selection;
mod listen;
mod mixcheck_ui;

use crate::ffmpeg::{self, Hwaccel};
use crate::stab::{ImuAxes, Mode, Stabilizer};
use crate::project::{Cut, Own, Project, SoundTrack};
use crate::stitch::M3;
use crate::view::{Camera, Ease, Follow, Keyframes, Projection, View};
use anyhow::{Context, Result};
use gpu::{Rig, Shown};
use iced::keyboard::{self, Key, key::Named};
use iced::mouse;
use iced::widget::{button, canvas, center, column, progress_bar, container, image, pick_list, responsive, row, rule, shader, slider, space, stack, svg, text, toggler, tooltip};
use iced::window;
use iced::{Alignment, Element, Length, Point, Rectangle, Size, Subscription, Task, Theme};
use playback::Playback;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use icons::*;
use sphere::*;
use media::*;
pub use media::{Launch, cache_clip};
use keys::*;
use framing::*;
use jobs::*;
use file::*;
use tracks::*;
use cuts::*;

pub use settings::config_dir;

pub fn run(inputs: Vec<PathBuf>, launch: Launch) -> Result<()> {
    // Finding the system's fonts takes a moment: done ahead, the font list
    // opens at once.
    std::thread::spawn(crate::text::families);
    let settings = settings::Settings::load();
    // The interface's size grows the window with it, or the bars' ends
    // would be cut off as they were at 1280 px.
    let ui = settings.ui_scale.clamp(*settings::UI_SCALE.start(), *settings::UI_SCALE.end());
    let opts = Options {
        osv_size: launch.osv_size.or(settings.osv_size),
        hw: launch.hw,
        hw_forced: launch.hw.is_some(),
        stabilize: launch.stabilize.unwrap_or(settings.stabilize),
        imu_axes: launch.imu_axes,
        world_z_down: launch.world_z_down,
    };
    iced::application(
        move || {
            let mut app = App::new(opts.clone(), settings.clone());
            let is_project = |p: &PathBuf| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
            let task = match inputs.iter().find(|p| is_project(p)) {
                Some(project) => app.update(Message::ProjectPicked(Some(project.clone()))),
                None => {
                    app.queue.extend(inputs.iter().map(|p| (p.clone(), true)));
                    app.open_next()
                }
            };
            (app, task)
        },
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(|app: &App| app.theme.clone())
    .scale_factor(|app: &App| app.ui_scale)
    .window(window::Settings {
        // Wide enough for the top bar and the transport row (at 1280 the
        // ends of both were cut off).
        size: iced::Size::new(1640.0 * ui, 900.0 * ui),
        // Closing asks first when the edit has unsaved changes.
        exit_on_close_request: false,
        // Wayland matches the window to its menu entry and icon by this id: in a
        // Flatpak that's the Flatpak's own (io.github.ravelund.rOsmo360).
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: std::env::var("FLATPAK_ID").unwrap_or_else(|_| NAME.to_owned()),
            ..Default::default()
        },
        // Windows takes the title bar's and taskbar's icon from the window,
        // not the .exe's resource (which Explorer shows).
        #[cfg(windows)]
        icon: window::icon::from_file_data(include_bytes!("../../packaging/rosmo360.png"), None).ok(),
        ..Default::default()
    })
    .run()
    .context("running the player window")
}

/// What "save" writes: the view as framed, or the whole sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Saving {
    View,
    Sphere,
}

/// Longest side of a saved view, and width of a saved sphere: the lenses'
/// full resolution holds about this much detail.
const SAVE_VIEW_PX: f32 = 3840.0;
const SAVE_SPHERE_PX: usize = 7680;

/// The panels that dock at the right of the view, one at a time.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Effects,
    Export,
    Audio,
    Record,
}

/// The shape of the view, and so of a saved view: free fills the window.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Aspect {
    Free,
    Ratio(u16, u16),
}

impl Aspect {
    const ALL: [Aspect; 6] = [Aspect::Free, Aspect::Ratio(16, 9), Aspect::Ratio(9, 16), Aspect::Ratio(4, 3), Aspect::Ratio(1, 1), Aspect::Ratio(2, 1)];

    /// Width over height, if locked.
    fn ratio(self) -> Option<f32> {
        match self {
            Aspect::Free => None,
            Aspect::Ratio(w, h) => Some(w as f32 / h as f32),
        }
    }
}

impl fmt::Display for Aspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aspect::Free => f.write_str("Free"),
            Aspect::Ratio(w, h) => write!(f, "{w}:{h}"),
        }
    }
}

/// What the player streams while playing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    Proxy,
    Osv(u32),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Proxy => f.write_str("Proxy (LRF)"),
            Source::Osv(s) => write!(f, "OSV {s} px"),
        }
    }
}

/// The playing cut's clip: its playback plus the frame the view draws.
struct Clip {
    playback: Playback,
    shown: Option<Arc<Shown>>,
}

#[derive(Debug, Clone)]
enum Message {
    Open,
    /// Clips to add to the edit, in order.
    Picked(Vec<PathBuf>),
    OpenProject,
    ProjectPicked(Option<PathBuf>),
    /// The window's close button was pressed.
    CloseRequested,
    /// The answer to "stop the export (or tracking) and quit?".
    StopAndClose(bool),
    /// The answer to "save changes first?" before doing `.0`.
    Confirmed(Then, Choice),
    /// Save the project; to a new file if true or never saved.
    SaveProject(bool),
    SaveProjectTo(Option<PathBuf>),
    Resolution(crate::Resolution),
    /// Export each cut to its own file.
    ExportClips(bool),
    /// Grade those files with the cuts' effects.
    ExportGraded(bool),
    ExportFormat(ffmpeg::Format),
    /// The export's loudness target picked (Export panel).
    ExportLoudness(export_panel::Loudness),
    /// Open or close the Export panel.
    ExportPanel,
    /// Open or close the Audio panel.
    AudioPanel,
    /// Open or close the Settings panel.
    SettingsPanel,
    AutoKey(bool),
    /// The divider above the transport row is this many pixels below
    /// where it was pressed (the lanes shrink), was let go, or was
    /// double-clicked (back to 1).
    SplitDrag(f32),
    SplitDone,
    SplitReset,
    Theme(Theme),
    TenBit(bool),
    /// Add a `.cube` to the LUT library (a file dialog), the file picked,
    /// take one out, and choose the one D-Log M clips get.
    AddLut,
    LutPicked(Option<PathBuf>),
    RemoveLut(String),
    DlogMLut(effects::LutChoice),
    /// A drag on the view let go.
    LookDone,
    /// Open the Info panel on a file on the timeline (right-click).
    Info(info::Of),
    /// What was read about a file for the Info panel.
    InfoRead(PathBuf, Result<Arc<info::File>, String>),
    InfoClose,
    /// Show a file in the file manager.
    Reveal(PathBuf),
    CopyPath(PathBuf),
    /// Export the edit: ask where (a file, or a folder for the clips), then
    /// render in the background.
    Export,
    ExportTo(Option<PathBuf>),
    /// Fraction of the export done.
    ExportProgress(f32),
    /// The files written.
    ExportDone(Result<Vec<PathBuf>, String>),
    ExportCancel,
    /// Starts (or stops) drawing a box over a subject to track its colour.
    PickColour,
    /// Pick a file for a sound track: a song for the music, or a sound
    /// effect.
    AddSound(SoundTrack),
    SoundPicked(SoundTrack, Option<PathBuf>),
    /// A sound file's length read off the UI thread, to add it to a track
    /// at a time (`None`: after the track's last clip).
    SoundProbed(SoundTrack, PathBuf, Option<f64>, Result<f64, String>),
    /// The «Framing» switch: the view's keyframes, angles and (for a 360
    /// cut) stabilisation and projection, shown and active; off, the bars
    /// hold editing only and the view can't be turned by accident.
    Framing(bool),
    /// The interface bigger or smaller by this many steps; 0 resets it.
    UiScale(i32),
    /// A cut's sound by hand: the Normalize dialog and the right-click menu.
    Sound(sound_ui::SoundMsg),
    /// The Record panel: a voice-over recorded onto the Voice lane.
    Record(record_ui::RecordMsg),
    /// The mix check in the viewer's place (gap 45).
    Check(mixcheck_ui::CheckMsg),
    /// A song's waveform, read off the UI thread.
    Waveform(PathBuf, Result<Arc<music::Waveform>, String>),
    /// The waveform of what a cut plays, if it has sound.
    CutWave(PathBuf, bool, Option<Arc<music::Waveform>>),
    /// A cut's sound's or a sound clip's fade in (`true`) or out dragged to
    /// this many seconds.
    Fade(Own, bool, f64),
    /// A cut's sound's or a sound clip's own level dragged to this, dB
    /// (its level tag, gap 44).
    Level(Own, f64),
    /// Sound lane `i` (`listen::lane_of`) soloed (`true`) or muted,
    /// toggled: for listening only.
    Listen(usize, bool),
    /// An overlay's picture ready (by `Item::key`), and the font its text
    /// missed.
    OverlayPicture(String, Result<(Arc<crate::overlay::Picture>, Option<String>), String>),
    /// Add a blank (black) cut at the playhead (gap 32).
    AddBlank,
    /// Cut `i`'s sound's start (`true`) or end dragged to clip time `t`,
    /// its picture left where it is: a J- or L-cut (gap 4B).
    SoundEdge(usize, bool, f64),
    /// Add a text overlay at the playhead.
    AddText,
    /// Add pictures to the overlay lane at the playhead.
    AddOverlay,
    OverlayPicked(Vec<PathBuf>),
    OverlaySelect(Option<crate::overlay::ItemRef>),
    /// An overlay dragged to start at edit time `at`.
    OverlayMove(crate::overlay::ItemRef, f64),
    /// An overlay's in (`true`) or out edge dragged to own time `t`.
    OverlayTrim(crate::overlay::ItemRef, bool, f64),
    /// To an overlay keyframe's edit time.
    OverlayJump(f64),
    /// An overlay's effect keyframe (`true`) or motion keyframe dragged to
    /// another time in the item's own.
    OverlayKeyMove(crate::overlay::ItemRef, bool, usize, f64),
    /// A clip on a sound track (the music, the sound effects) selected.
    SelectMusic(SoundTrack, usize),
    /// Clip `i` of a sound track dragged to start at this edit time; with
    /// Shift held (`true`) it may overlap a neighbour, crossfading.
    MusicMove(SoundTrack, usize, f64, bool),
    /// Clip `i`'s in (`true`) or out edge dragged to this time in its file,
    /// with Shift held or not.
    MusicTrim(SoundTrack, usize, bool, f64, bool),
    /// A point on a cut's or a sound clip's own volume line, added at this
    /// time in its file and level (dB); selected; dragged.
    PointAdd(Own, f64, f64),
    PointSelect(Own, usize),
    PointMove(Own, usize, f64, f64),
    /// The mix: the clips' sound and each sound track, in dB.
    SoundLevel(f64),
    TrackLevel(SoundTrack, f64),
    /// Auto-duck: how far, dB; lower the music under speech now; the
    /// speech found (edit time), read off the UI thread.
    DuckDepth(f64),
    Duck,
    /// Where speech was found, in the edit as it was at `version`.
    Ducked(u64, Result<Vec<(f64, f64)>, String>),
    /// A level slider let go.
    LevelDone,
    /// Start an empty edit (asking first if there are unsaved changes).
    NewProject,
    /// A redraw while clips load, for the loading bar.
    LoadingTick,
    /// The box being drawn, in fractions of the view.
    PickArea(Rectangle),
    /// The box drawn, and the view's width over its height.
    AreaPicked(Rectangle, f64),
    TrackProgress(f32),
    TrackCancel,
    Tracked(Result<TrackDone, String>),
    /// Never cloned in flight, so the clip can be taken back out of the `Arc`.
    Loaded(Arc<Result<Media, String>>),
    Tick,
    Key(Key, keyboard::Modifiers),
    Toggle,
    Scrub(f64),
    ScrubDone,
    /// Add a keyframe with the current view at the playhead, or update the
    /// one there.
    KeyAdd,
    /// Delete the keyframe at the playhead.
    KeyDelete,
    /// Zoom the timeline by a factor, keeping edit time `.1` (the playhead
    /// if `None`) where it is on screen.
    TimelineZoom(f64, Option<f64>),
    /// Zoom the timeline to slider position `.0` (0 whole edit, 1 closest).
    TimelineZoomTo(f64),
    /// Scroll the timeline so it starts at edit time `.0`.
    TimelinePan(f64),
    /// A thumbnail of clip `.0` at clip time `.1`.
    Thumb(PathBuf, f64, image::Handle),
    /// How the keyframe at the playhead moves on to the next.
    KeyEase(Ease),
    KeyFollow(Aiming),
    /// Jump to keyframe `.1` of cut `.0`.
    KeyJump(usize, usize),
    /// Drag keyframe `.1` of cut `.0` to clip time `.2`.
    KeyMove(usize, usize, f64),
    Select(usize),
    /// Drag cut `.0`'s in (`.1` true) or out point to clip time `.2`.
    Trim(usize, bool, f64),
    /// Move cut `.0` to position `.1`.
    MoveCut(usize, usize),
    /// A delete's «Undo» note has had its time (its number).
    ToastGone(u64),
    /// The shortcuts' window opened or closed (F1, its button).
    Shortcuts,
    /// A marker at the playhead on the cut there (M).
    AddMark,
    /// The cut under the playhead selected (D, Premiere's key).
    SelectAtPlayhead,
    /// Cut `.0`'s marker at clip time `.1` dragged to `.2`.
    MarkMove(usize, f64, f64),
    /// Cut `.0`'s marker at clip time `.1` clicked: selected (Del takes it,
    /// not the cut), the playhead to it.
    MarkSelect(usize, f64),
    /// Cut `.0`'s marker at clip time `.1` opened to name or delete
    /// (a double-click or a right-click on its flag); its note typed;
    /// saved; deleted; the window closed.
    MarkEdit(usize, f64),
    MarkNote(String),
    MarkSave,
    MarkDelete,
    MarkClose,
    /// Frame step `.0` has rested: the full-resolution frame, if it was the
    /// last.
    StepsRested(u64),
    /// The playhead's step note has had its time (its number).
    StepNoteGone(u64),
    /// The selected (else playing) cut's in point (true) or out point to
    /// the playhead: Alt+[ and Alt+] (FCP), trimming without a drag.
    TrimToPlayhead(bool),
    /// Split the cut at the playhead.
    Split,
    /// The wheel on the timeline: across, up or down (lines), and the
    /// time under the pointer. Zooms there; with Shift, or across, pans.
    TimelineWheel(f32, f32, f64),
    /// Open or close the bin (the project folder's media) at the left.
    BinPanel,
    /// Read the project's folder again.
    BinRefresh,
    /// The bin as a view of tiles in the picture's place.
    BinView(bin_view::BinMsg),
    /// A question in the window answered (`ask.rs`).
    Answer(Box<Message>),
    /// The bin view's own player.
    BinPlayer(bin_player::SourceMsg),
    /// Add a file from the bin where its kind goes.
    BinAdd(PathBuf, bin::Kind, bin::Place),
    /// Pin the bin open, or unpin it.
    BinPin,
    /// Preview a file in the bin: see a clip's frames, hear a sound.
    BinPreview(PathBuf, bin::Kind),
    BinStop,
    /// Lengths and pictures of the bin's files, read.
    BinMeta(Vec<(PathBuf, bin::Meta)>),
    /// The previewed clip's frames, read.
    BinFrames(PathBuf, Vec<image::Handle>),
    /// Cut `i`'s crossfade from the cut before dragged to this many seconds.
    Crossfade(usize, f64),
    /// Give the selected (or playing) cut a 1 s crossfade, or take its away.
    ToggleCrossfade,
    /// From the Effects panel.
    Effect(effects::EffectMsg),
    /// Delete the selected cut, or the selected music clip.
    DeleteCut,
    Undo,
    Redo,
    Look { yaw: f64, pitch: f64, roll: f64 },
    /// Scroll notches; positive zooms out.
    Zoom(f64),
    Fov(f64),
    /// Typed into a field of the view-angles strip.
    AngleInput(angles::Angle, String),
    /// Enter in one: show the value as set.
    AngleDone,
    /// Scrolled on a field: a step up (1) or down (−1).
    AngleNudge(angles::Angle, f64),
    /// A name dragged: starting, by this many pixels, let go.
    AngleScrubStart(angles::Angle),
    AngleScrub(angles::Angle, f64),
    AngleScrubDone,
    /// ↑ or ↓ pressed: the field that has the keyboard, if one does, and
    /// the direction (`angles::arrow`).
    AngleArrow(Option<angles::Angle>, f64),
    /// Show the view-angles strip.
    ShowAngles(bool),
    /// The timeline's time ruler large or compact.
    RulerLarge(bool),
    /// The bars' group names shown or hidden.
    BarLabels,
    /// Snapping on the timeline on or off (N).
    Snapping,
    /// A timeline drag's message, and where it snapped (edit time), for
    /// the line drawn there; `None` when it didn't.
    Snapped(Option<f64>, Box<Message>),
    /// A foldable lane folded (`true`) or opened.
    Fold(timeline::FoldLane, bool),
    Modifiers(keyboard::Modifiers),
    Stab(Mode),
    Projection(Projection),
    Source(Source),
    RollingShutter(bool),
    FullOnPause(bool),
    Mute,
    Volume(f32),
    Aspect(Aspect),
    /// The 360 view's size on screen, so a saved view keeps its shape.
    ViewSize(Size),
    Save(Saving),
    SaveTo(Saving, Option<PathBuf>),
    Saved(Result<PathBuf, String>),
    ResetView,
    Fullscreen,
}

struct App {
    opts: Options,
    /// Tracks that keyframes follow, read once each (`None`: unreadable).
    followed: std::cell::RefCell<std::collections::HashMap<PathBuf, Option<Arc<crate::track::Track>>>>,
    /// The colour theme (the Settings panel's; Kanagawa Dragon unless chosen).
    theme: Theme,
    logo: image::Handle,
    /// Every clip opened so far, ready for the cuts that use it.
    library: Vec<Media>,
    /// Each clip's timeline thumbnails by clip time, as they arrive.
    thumbs: std::collections::HashMap<PathBuf, Vec<(f64, image::Handle)>>,
    /// Waveforms of sound clips and of what cuts play, by file.
    waveforms: std::collections::HashMap<PathBuf, Arc<music::Waveform>>,
    /// The same through the leveler, for cuts evened out.
    even_waves: std::collections::HashMap<PathBuf, Arc<music::Waveform>>,
    /// The cuts' sound files whose waveforms were asked for, plain or
    /// evened: each once, a silent one too (pictures, clips without sound).
    cut_waves_asked: std::collections::HashSet<(PathBuf, bool)>,
    /// How many of them are being read now.
    cut_waves_reading: usize,
    /// The mix's level as it plays.
    meter: meter::Meter,
    /// The overlays' pictures, scaled for the view: `None` while loading.
    overlay_pictures: std::collections::HashMap<String, Option<(u64, Arc<crate::overlay::Picture>)>>,
    /// Their timeline thumbnails.
    overlay_thumbs: std::collections::HashMap<String, iced::widget::image::Handle>,
    /// The selected text overlay's words as edited in the panel, and which
    /// overlay they're of.
    text_content: iced::widget::text_editor::Content,
    text_of: Option<crate::overlay::ItemRef>,

    /// The «Framing» switch: off (editing) by default, each start.
    framing: bool,
    /// The interface's size (iced's scale factor), kept in the settings.
    ui_scale: f32,
    /// The Normalize dialog, while open.
    normalize: Option<sound_ui::Dialog>,
    /// The right-click menu on a cut's sound: which cut, where.
    sound_menu: Option<(sound_ui::Menu, iced::Point)>,
    /// Each sound track's streams, in `SoundTrack::ALL`'s order.
    music: [music::Player; SoundTrack::ALL.len()],
    /// The cuts' sound past their pictures (J- and L-cuts), a stream each,
    /// as `Project::sound_beyond` lists them.
    beyond: Vec<music::Player>,
    /// Through a crossfade, the other cut of it alongside: its index and
    /// clip. Usually the next one, coming in over the playing cut; while
    /// paused with the incoming cut selected for editing, that one is the
    /// current cut and this is the one before (`follow_crossfade`).
    partner: Option<(usize, Clip)>,
    edit: edit::Edit,
    /// The cut being played.
    current: usize,
    /// What's selected on the timeline.
    selection: Selection,
    /// Counts drags cancelled by Esc; the timeline drops a drag begun before.
    drag_epoch: u64,
    /// «… deleted · Undo» in the top bar for a few seconds after a delete,
    /// and which one it is, so an older one's timer doesn't clear a newer.
    toast: Option<(&'static str, u64)>,
    /// A question up in the window (`ask.rs`).
    asking: Option<ask::Ask>,
    /// The keyboard and mouse shortcuts' window, open (gap 56).
    shortcuts: bool,
    /// A marker being named in its window (gap 5).
    mark_edit: Option<marks::Editing>,
    /// Counts frame steps by key; the full-resolution frame is asked for
    /// when the last one has rested a moment.
    steps: u64,
    /// The playhead's last key step («+1 f», «join →») shown by it for a
    /// moment, and which one (gap 55).
    step_note: Option<(&'static str, u64)>,
    /// Clips waiting to be loaded, in order, and whether each is added as a
    /// cut (opened) or only loaded for a project's cuts.
    queue: std::collections::VecDeque<(PathBuf, bool)>,
    /// Where a clip added from the bin goes once it's loaded, by its file:
    /// at the end if it has no entry.
    placing: std::collections::HashMap<PathBuf, bin::Spot>,
    /// Whether the clip loading now is added as a cut.
    loading_adds: bool,
    /// The next cut's playback, primed in the last `PREROLL` seconds of the
    /// one playing, with the cut it was made for: its clip and in point, so
    /// one made stale by an edit is never used.
    preroll: Option<(usize, PathBuf, f64, Playback)>,
    /// Clips loaded since the queue was last empty, and how far the one
    /// loading now has got (an f64's bits, set from its loading thread):
    /// together with the queue, a project's loading progress.
    loaded: usize,
    loading_part: Arc<std::sync::atomic::AtomicU64>,
    /// The project file, once opened or saved, and the edit version saved.
    project: Option<PathBuf>,
    saved_version: u64,
    /// The aspect lock when last saved: it's in the project, so changing it
    /// is a change to save.
    saved_aspect: Aspect,
    /// What to do once the project has been saved.
    after_save: Option<Then>,
    /// Quitting once the export or tracking stopped for it has cleaned up.
    closing: bool,
    /// Exports are this many pixels on the short side.
    resolution: crate::Resolution,
    /// Export each cut to its own file, and in which format.
    export_clips: bool,
    export_graded: bool,
    export_format: ffmpeg::Format,
    /// The export's loudness target, LUFS, or as mixed.
    export_loudness: Option<f64>,
    /// A running export: its cancel flag and how far it has got.
    exporting: Option<(Arc<std::sync::atomic::AtomicBool>, f32)>,
    /// Where an effect or overlay drag writes its key (gap 38).
    effect_pin: effects::Pin,
    /// Timeline zoom (1 shows the whole edit) and the edit time at its left
    /// edge; `timeline_window` keeps them in range.
    timeline_zoom: f64,
    timeline_start: f64,
    clip: Option<Clip>,
    /// The picture as it was drawn when the playhead crossed into another
    /// cut, kept until that cut's first frame arrives. Drawing the old frame
    /// with the new cut's stabilisation and keyframes showed it at a wrong
    /// angle for a moment, like a ghost of the previous clip.
    held: Option<Sphere>,
    loading: Option<PathBuf>,
    error: Option<String>,
    /// The free-look view: what is shown unless following keyframes.
    look: View,
    /// The free look kept for the other kind of clip (360 or ordinary
    /// video), swapped in when the playing cut is of that kind: angles mean
    /// another thing there, and a little planet's would push a video's
    /// picture off screen.
    other_look: View,
    /// The cut (index and clip) looked around in by hand since the view last
    /// followed keyframes: one without keyframes of its own keeps that view
    /// (`keep_hand_view`).
    hand_cut: Option<(usize, PathBuf)>,
    /// Whether `look` belongs to ordinary videos.
    look_flat: bool,
    /// Show the keyframes' view rather than `look`. Playing, seeking and
    /// keyframe edits turn it on; looking around by hand turns it off.
    follow: bool,
    rolling_shutter: bool,
    /// Show a full-resolution frame when paused.
    full_on_pause: bool,
    /// Looking around while paused keys the view at the playhead (the
    /// Settings panel).
    auto_key: bool,
    /// The cut and clip time automatic keyframing is writing to, so one
    /// look-around there is one undo step; `None` once it's over (the mouse
    /// let go, or anything else done).
    auto_key_at: Option<(usize, f64)>,
    /// The timeline lanes' scale as the divider has it now, and as last let
    /// go, which is what's saved: a drag would otherwise write the settings
    /// file at every step.
    split: f32,
    split_kept: f32,
    /// The Settings panel is open (at the left).
    settings_open: bool,
    /// The bin, at the far left (`bin.rs`).
    bin: bin::Bin,
    /// 10-bit tracks decode at 10 bits (`source::deep`).
    ten_bit: bool,
    /// The library LUT new D-Log M cuts get, and the library's LUTs.
    dlog_m_lut: Option<String>,
    luts: Vec<String>,
    fullscreen: bool,
    volume: f32,
    muted: bool,
    view_size: Size,
    aspect: Aspect,
    /// The view-angles strip shows under the transport row.
    angles_shown: bool,
    /// A field of it as typed so far, until Enter or another message.
    angle_draft: Option<(angles::Angle, String)>,
    /// The keyboard's modifiers, for Shift on a scroll.
    modifiers: keyboard::Modifiers,
    /// The field whose name is being dragged, from its press to its
    /// release (`angles::Scrub`).
    scrubbing: Option<angles::Angle>,
    /// What the last save did, or that one is running.
    save_status: Option<String>,
    /// Picking a colour to track: `Some(None)` until a box is drawn, then
    /// the box in fractions of the view.
    picking: Option<Option<Rectangle>>,
    /// The box picks a white balance, not a colour to track.
    picking_white: bool,
    /// The panel docked at the right of the view, if one is open.
    side: Option<Side>,
    /// The Record panel's choices and recorders.
    record: record_ui::Panel,
    /// Solo and mute per sound lane: what's heard while mixing.
    listen: listen::Listen,
    /// The mix check, and whether it's in the viewer's place.
    check: mixcheck_ui::Check,
    /// Auto-duck's depth, dB, and whether it's reading the voices now.
    duck_db: f64,
    /// The timeline's time ruler large or compact (Settings).
    ruler_large: bool,
    /// The bars' groups named above their buttons.
    bar_labels: bool,
    /// Timeline drags snap (N toggles; Shift held lets one go free).
    snapping: bool,
    /// Where the drag under way snapped, edit time: a line drawn there.
    snap_line: Option<f64>,
    /// Which foldable lanes are folded.
    folds: timeline::Folds,
    ducking: bool,
    /// The Info panel (at the left, in the Text panel's place).
    info: Option<info::Info>,
    /// A running colour track: its cancel flag and progress, 0..1.
    tracking: Option<(Arc<std::sync::atomic::AtomicBool>, f32)>,
    /// The settings as last written, to save only when they change.
    saved_settings: settings::Settings,
}

impl App {
    fn new(opts: Options, saved: settings::Settings) -> Self {
        audio::set_listening(if saved.muted { 0.0 } else { saved.volume });
        source::set_deep(saved.ten_bit);
        App {
            opts,
            followed: Default::default(),
            theme: settings_panel::theme_named(&saved.theme),
            ten_bit: saved.ten_bit,
            dlog_m_lut: saved.dlog_m_lut.clone(),
            luts: crate::lut::library_names(),
            logo: image::Handle::from_bytes(LOGO),
            library: Vec::new(),
            thumbs: Default::default(),
            waveforms: Default::default(),
            meter: Default::default(),
            overlay_pictures: Default::default(),
            overlay_thumbs: Default::default(),
            text_content: Default::default(),
            text_of: None,

            framing: false,
            normalize: None,
            sound_menu: None,
            cut_waves_asked: Default::default(),
            even_waves: Default::default(),
            cut_waves_reading: 0,
            music: Default::default(),
            beyond: Vec::new(),
            partner: None,
            edit: edit::Edit::default(),
            current: 0,
            selection: Selection::None,
            drag_epoch: 0,
            toast: None,
            asking: None,
            shortcuts: false,
            mark_edit: None,
            steps: 0,
            step_note: None,
            effect_pin: effects::Pin::default(),
            timeline_zoom: 1.0,
            timeline_start: 0.0,
            queue: Default::default(),
            placing: Default::default(),
            loading_adds: true,
            loaded: 0,
            loading_part: Arc::default(),
            preroll: None,
            project: None,
            saved_version: 0,
            saved_aspect: Aspect::Free,
            after_save: None,
            closing: false,
            resolution: saved.resolution,
            export_clips: saved.export_clips,
            export_graded: saved.export_graded,
            export_format: saved.export_format,
            export_loudness: saved.export_loudness,
            exporting: None,
            clip: None,
            held: None,
            loading: None,
            error: None,
            look: View::default(),
            other_look: View::home(Projection::Perspective),
            hand_cut: None,
            look_flat: false,
            follow: true,
            rolling_shutter: saved.rolling_shutter,
            full_on_pause: saved.full_on_pause,
            auto_key: saved.auto_key,
            auto_key_at: None,
            split: saved.timeline_scale,
            split_kept: saved.timeline_scale,
            angles_shown: saved.view_angles,
            ui_scale: saved.ui_scale.clamp(*settings::UI_SCALE.start(), *settings::UI_SCALE.end()),
            duck_db: saved.duck_db.clamp(3.0, 30.0),
            ruler_large: saved.ruler_large,
            bar_labels: saved.bar_labels,
            snapping: saved.snapping,
            snap_line: None,
            folds: timeline::Folds { text: saved.fold_text, overlay: saved.fold_overlay, sfx2: saved.fold_sfx2, marks: saved.fold_marks },
            ducking: false,
            listen: Default::default(),
            check: Default::default(),
            angle_draft: None,
            modifiers: keyboard::Modifiers::default(),
            scrubbing: None,
            settings_open: false,
            bin: bin::Bin { open: saved.bin_pinned, pinned: saved.bin_pinned, tile: saved.bin_tile, side: saved.bin_side, ..Default::default() },
            fullscreen: false,
            volume: saved.volume,
            muted: saved.muted,
            view_size: Size::new(16.0, 9.0),
            aspect: saved.aspect.map_or(Aspect::Free, |[w, h]| Aspect::Ratio(w, h)),
            save_status: None,
            picking: None,
            picking_white: false,
            side: None,
            record: record_ui::Panel::new(saved.record_input.clone(), saved.record_channel, saved.record_countdown, saved.record_play),
            info: None,
            tracking: None,
            saved_settings: saved,
        }
    }

    /// The settings to remember, as they are now.
    fn settings(&self) -> settings::Settings {
        settings::Settings {
            stabilize: self.opts.stabilize,
            osv_size: self.opts.osv_size,
            rolling_shutter: self.rolling_shutter,
            full_on_pause: self.full_on_pause,
            volume: self.volume,
            muted: self.muted,
            resolution: self.resolution,
            aspect: match self.aspect {
                Aspect::Free => None,
                Aspect::Ratio(w, h) => Some([w, h]),
            },
            export_clips: self.export_clips,
            export_graded: self.export_graded,
            export_format: self.export_format,
            export_loudness: self.export_loudness,
            auto_key: self.auto_key,
            theme: self.theme.to_string(),
            ten_bit: self.ten_bit,
            dlog_m_lut: self.dlog_m_lut.clone(),
            bin_pinned: self.bin.pinned,
            bin_tile: self.bin.tile,
            bin_side: self.bin.side,
            timeline_scale: self.split_kept,
            view_angles: self.angles_shown,
            ui_scale: self.ui_scale,
            record_input: self.record.input.clone(),
            record_channel: self.record.channel,
            record_countdown: self.record.countdown,
            record_play: self.record.play,
            duck_db: self.duck_db,
            ruler_large: self.ruler_large,
            bar_labels: self.bar_labels,
            snapping: self.snapping,
            fold_text: self.folds.text,
            fold_overlay: self.folds.overlay,
            fold_sfx2: self.folds.sfx2,
            fold_marks: self.folds.marks,
        }
    }

    /// Handles `message`, then saves the settings if it changed any.
    fn update(&mut self, message: Message) -> Task<Message> {
        // Editing: what turns or keys the view does nothing (its controls
        // are hidden too; the mouse on the picture and the keys still send).
        if !self.framing && framing_only(&message) {
            return Task::none();
        }
        let before = if self.look_flat { (self.other_look, self.look) } else { (self.look, self.other_look) };
        // What may look around (a key may turn the view); a let-go drag or
        // anything else done ends a look-around's undo step. Frames and
        // the view's size go by without ending it.
        let looking = matches!(
            message,
            Message::Look { .. } | Message::Zoom(_) | Message::Fov(_) | Message::ResetView | Message::Key(..) | Message::AngleInput(..) | Message::AngleNudge(..) | Message::AngleArrow(..) | Message::AngleScrub(..)
        );
        let quiet = matches!(message, Message::Tick | Message::LoadingTick | Message::ViewSize(_) | Message::Modifiers(_) | Message::Record(record_ui::RecordMsg::Tick));
        // A look-around keyed by Auto keyframe ends: its undo step closes.
        if !looking && !quiet && self.auto_key_at.take().is_some() {
            self.edit.commit();
        }
        // A field shows what's typed only while it's being typed.
        if !quiet && !matches!(message, Message::AngleInput(..)) {
            self.angle_draft = None;
        }
        let task = self.handle(message);
        // What the message added or split gets its id (`OPS-API.md`; the
        // operations' `Editor` will do it as it applies them).
        self.edit.settle_ids();
        if looking {
            self.auto_key(before);
        }
        self.keep_hand_view(before);
        let task = Task::batch([task, self.load_overlay_pictures(), self.load_cut_waves()]);
        self.settle_look();
        self.settle_text();
        self.follow_crossfade();
        self.follow_music();
        let task = Task::batch([task, self.follow_bin()]);
        // Another panel took the Record panel's place: stop listening.
        let task = if self.side != Some(Side::Record) && self.record.active() { Task::batch([task, self.close_record()]) } else { task };
        let now = self.settings();
        if now != self.saved_settings {
            now.save();
            self.saved_settings = now;
        }
        task
    }

    /// Reads the bin again when the project's folder changed: another
    /// project opened, or this one saved somewhere else.
    fn follow_bin(&mut self) -> Task<Message> {
        let want = self.project.as_deref().and_then(bin::root);
        if want.as_ref() == self.bin.read.as_ref().map(|(_, r)| r) {
            return Task::none();
        }
        self.rescan_bin()
    }

    /// What the speakers get: nothing while muted, so unmuting restores
    /// the level that was set.
    fn volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    /// Opens `side` at the right (closing whichever was there), or closes it.
    fn toggle_side(&mut self, side: Side) {
        self.side = if self.side == Some(side) { None } else { Some(side) };
    }

    fn subscription(&self) -> Subscription<Message> {
        let keys = keyboard::listen().filter_map(|e| match e {
            keyboard::Event::KeyPressed { key, physical_key, modifiers, .. } => Some(Message::Key(keys::latin(key, physical_key, modifiers.shift()), modifiers)),
            keyboard::Event::ModifiersChanged(m) => Some(Message::Modifiers(m)),
            _ => None,
        });
        // The crossfade's partner too: paused, its frame came in only with
        // the next event.
        let busy = self.clip.iter().chain(self.partner.as_ref().map(|(_, c)| c)).any(|c| c.playback.busy()) || self.source_busy();
        let close = window::close_requests().map(|_| Message::CloseRequested);
        // Files dropped from a file manager go into the bin (X11 and other
        // systems; winit doesn't pass them on Wayland yet).
        // The wait before a hovered tile's frames show, a screen frame at a time.
        let close = if self.bin.view && self.bin.hover.as_ref().is_some_and(|(_, _, shown)| !shown) {
            Subscription::batch([close, window::frames().map(|_| Message::BinView(bin_view::BinMsg::HoverTick))])
        } else {
            close
        };
        let close = if self.bin.view || self.bin.open {
            Subscription::batch([close, iced::event::listen_with(|e, _, _| match e {
                iced::Event::Window(window::Event::FileDropped(path)) => Some(Message::BinView(bin_view::BinMsg::Dropped(path))),
                _ => None,
            })])
        } else {
            close
        };
        // The Record panel's meter and countdown, every screen frame.
        let keys = if self.side == Some(Side::Record) { Subscription::batch([keys, window::frames().map(|_| Message::Record(record_ui::RecordMsg::Tick))]) } else { keys };
        if busy {
            Subscription::batch([keys, close, window::frames().map(|_| Message::Tick)])
        } else if self.loading.is_some() {
            // Only to redraw the loading bar as it fills.
            Subscription::batch([keys, close, window::frames().map(|_| Message::LoadingTick)])
        } else {
            Subscription::batch([keys, close])
        }
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        // Each area handles its own messages (`on_*` in its module).
        match message {
            m @ (Message::Open { .. } | Message::Picked { .. } | Message::Loaded { .. } | Message::OpenProject { .. } | Message::NewProject { .. } | Message::CloseRequested { .. } | Message::StopAndClose { .. } | Message::Confirmed { .. } | Message::ProjectPicked { .. } | Message::SaveProjectTo { .. } | Message::SaveProject { .. } | Message::LoadingTick { .. } | Message::Thumb { .. }) => self.on_file(m),
            m @ (Message::Export { .. } | Message::ExportTo { .. } | Message::ExportProgress { .. } | Message::ExportCancel { .. } | Message::PickColour { .. } | Message::PickArea { .. } | Message::AreaPicked { .. } | Message::TrackProgress { .. } | Message::TrackCancel { .. } | Message::Tracked { .. } | Message::ExportDone { .. } | Message::Save { .. } | Message::SaveTo { .. } | Message::Saved { .. }) => self.on_jobs(m),
            m @ (Message::Resolution { .. } | Message::ExportClips { .. } | Message::ExportGraded { .. } | Message::ExportFormat { .. } | Message::ExportLoudness { .. } | Message::Info { .. } | Message::InfoRead { .. } | Message::InfoClose { .. } | Message::Reveal { .. } | Message::CopyPath { .. } | Message::ExportPanel { .. } | Message::AudioPanel { .. } | Message::SettingsPanel { .. } | Message::Shortcuts { .. } | Message::AutoKey { .. } | Message::SplitDrag { .. } | Message::SplitDone { .. } | Message::SplitReset { .. } | Message::TenBit { .. } | Message::AddLut { .. } | Message::LutPicked { .. } | Message::RemoveLut { .. } | Message::DlogMLut { .. } | Message::UiScale { .. } | Message::ShowAngles { .. } | Message::RulerLarge { .. } | Message::BarLabels | Message::Snapping | Message::Fold { .. } | Message::Modifiers { .. } | Message::ViewSize { .. } | Message::Aspect { .. } | Message::Theme { .. } | Message::Mute { .. } | Message::Volume { .. } | Message::Fullscreen { .. }) => self.on_panels(m),
            m @ (Message::BinPanel { .. } | Message::BinPin { .. } | Message::BinRefresh { .. } | Message::BinPreview { .. } | Message::BinStop { .. } | Message::BinMeta { .. } | Message::BinFrames { .. } | Message::BinAdd { .. }) => self.on_bin(m),
            Message::BinView(msg) => self.on_bin_view(msg),
            Message::BinPlayer(msg) => self.on_source(msg),
            Message::Answer(msg) => self.answer(*msg),
            m @ (Message::MarkEdit { .. } | Message::MarkNote { .. } | Message::MarkSave | Message::MarkDelete | Message::MarkClose) => self.on_marks(m),
            m @ (Message::Tick { .. } | Message::TimelineZoom { .. } | Message::TimelineWheel { .. } | Message::TimelineZoomTo { .. } | Message::TimelinePan { .. } | Message::Toggle { .. } | Message::Scrub { .. } | Message::ScrubDone { .. }) => self.on_transport(m),
            m @ (Message::Select { .. } | Message::AddBlank | Message::SoundEdge { .. } | Message::Trim { .. } | Message::MoveCut { .. } | Message::AddMark { .. } | Message::SelectAtPlayhead | Message::MarkMove { .. } | Message::MarkSelect { .. } | Message::StepsRested { .. } | Message::ToastGone { .. } | Message::StepNoteGone { .. } | Message::TrimToPlayhead { .. } | Message::Crossfade { .. } | Message::ToggleCrossfade { .. } | Message::Split { .. } | Message::DeleteCut { .. } | Message::Undo { .. } | Message::Redo { .. }) => self.on_cuts(m),
            m @ (Message::LookDone { .. } | Message::Framing { .. } | Message::KeyMove { .. } | Message::KeyAdd { .. } | Message::KeyDelete { .. } | Message::KeyEase { .. } | Message::KeyFollow { .. } | Message::Stab { .. } | Message::Projection { .. } | Message::KeyJump { .. } | Message::Look { .. } | Message::Zoom { .. } | Message::Fov { .. } | Message::AngleInput { .. } | Message::AngleDone { .. } | Message::AngleScrubStart { .. } | Message::AngleScrubDone { .. } | Message::AngleScrub { .. } | Message::AngleNudge { .. } | Message::AngleArrow { .. } | Message::Source { .. } | Message::RollingShutter { .. } | Message::FullOnPause { .. } | Message::ResetView { .. }) => self.on_framing(m),
            m @ (Message::AddOverlay { .. } | Message::OverlayPicked { .. } | Message::OverlaySelect { .. } | Message::OverlayMove { .. } | Message::OverlayTrim { .. } | Message::OverlayJump { .. } | Message::OverlayKeyMove { .. } | Message::OverlayPicture { .. } | Message::AddText { .. }) => self.on_overlay_ui(m),
            m @ (Message::AddSound { .. } | Message::SoundPicked { .. } | Message::SoundProbed { .. } | Message::Waveform { .. } | Message::CutWave { .. } | Message::SelectMusic { .. } | Message::PointAdd { .. } | Message::PointSelect { .. } | Message::PointMove { .. } | Message::MusicMove { .. } | Message::MusicTrim { .. } | Message::Fade { .. } | Message::Level { .. } | Message::Listen { .. } | Message::SoundLevel { .. } | Message::TrackLevel { .. } | Message::DuckDepth { .. } | Message::Duck { .. } | Message::Ducked { .. } | Message::LevelDone { .. }) => self.on_tracks(m),
            Message::Snapped(at, inner) => {
                self.snap_line = at;
                self.update(*inner)
            }
            Message::Effect(e) => {
                self.effect_message(e);
                Task::none()
            }
            Message::Key(key, mods) => {
                let task = self.key(key, mods);
                // Keys can seek out of sight (arrows, [ ]): follow.
                self.reveal_playhead();
                task
            }
            Message::Sound(msg) => self.sound_ui(msg),
            Message::Record(msg) => self.record_ui(msg),
            Message::Check(msg) => self.mix_check(msg),
        }
    }

    /// Applies `op` to the edit (`OPS-API.md`): one undo step, or part of
    /// the drag under way; a refusal is shown. What it did, if it did.
    fn op(&mut self, op: crate::model::ops::Op) -> Option<crate::model::ops::Outcome> {
        self.edit.apply(&op).map_err(|e| self.error = Some(e.to_string())).ok()
    }

    /// Which track the sound clip with id `id` is on, and where.
    fn clip_index(&self, id: crate::project::Id) -> Option<(SoundTrack, usize)> {
        SoundTrack::ALL.into_iter().find_map(|w| self.edit.track(w).clips.iter().position(|c| c.id == id).map(|i| (w, i)))
    }

    /// Where the cut with id `id` is.
    fn cut_index(&self, id: crate::project::Id) -> Option<usize> {
        self.edit.cuts.iter().position(|c| c.id == id)
    }

    /// The window, and a question over it when one is asked (`ask.rs`).
    fn view(&self) -> Element<'_, Message> {
        stack![self.window_view(), self.ask_layer()].into()
    }

    fn window_view(&self) -> Element<'_, Message> {
        let Some(clip) = &self.clip else { return self.start_screen() };
        let error = self.error.as_ref().map(|e| text(e.clone()).style(text::danger));
        let timeline = self.timeline_view();
        let timeline = container(timeline).padding([2, 10]);
        let divider = tip(
            canvas(divider::Divider).width(Length::Fill).height(divider::HEIGHT),
            text("Drag to give the timeline or the picture more room (double-click: as it was)"),
            tooltip::Position::Top,
        );
        let window: Element<'_, Message> = column![self.top_bar(clip), self.picture(), divider, self.transport_bar(clip)].push(self.framing.then(|| self.framing_row(clip))).push(timeline).push(error.map(|e| container(e).padding([0, 10]))).push(credit()).into();
        self.shortcuts_dialog(self.mark_dialog(self.normalize_dialog(self.sound_menu_layer(window))))
    }
}

/// The name people see: Omnix, this fork of rOsmo360 (whose name was Osmo
/// 360 with an r for its Rust core). The binary, the config and cache
/// folders keep rOsmo360's names: scripts, footage-agent and the clip cache
/// find them there, and merges from upstream stay simple.
pub const NAME: &str = "Omnix";

/// A short name for a clip on the timeline: DJI's clip number (the digits
/// in CAM_20260713153034_0037_D), else the file name.
fn clip_name(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    stem.split('_').rev().find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit())).map_or(stem.clone(), str::to_owned)
}

/// The bin: a box of files.
pub(super) const BIN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="5" rx="1"/><path d="M5 9v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V9"/><path d="M10 13h4"/></svg>"#;

fn clock(t: f64) -> String {
    format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0)
}

/// Runs `f` on a thread of its own and gives its result as a future: for
/// work that blocks (ffprobe, decoding a waveform) without holding up the
/// window.
fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = T> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    async move { rx.await.expect("loader thread panicked") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_keeps_the_editing_keys() {
        let none = keyboard::Modifiers::empty();
        let ch = |c: &str| Key::Character(c.into());
        let named = |n| Key::Named(n);
        // Seeking, playing, cutting and the file keys stay.
        for key in [named(Named::ArrowLeft), named(Named::ArrowRight), named(Named::Space), ch("c"), ch("m"), ch("g"), ch("b"), ch("f")] {
            assert!(!framing_key(&key, none, false), "{key:?}");
        }
        assert!(!framing_key(&ch("0"), keyboard::Modifiers::CTRL, false), "Ctrl+0 fits the timeline");
        assert!(!framing_key(&ch("s"), keyboard::Modifiers::CTRL, false), "Ctrl+S saves");
        // Turning, framing and keying the view go.
        for key in [named(Named::ArrowUp), named(Named::Home), ch("q"), ch("e"), ch("0"), ch("k"), ch("j"), ch("["), ch("s"), ch("p"), ch("r")] {
            assert!(framing_key(&key, none, false), "{key:?}");
        }
        assert!(framing_key(&named(Named::ArrowLeft), keyboard::Modifiers::SHIFT, false), "Shift+← turns the view");
        // K on a selected sound adds a volume point, which editing keeps.
        assert!(!framing_key(&ch("k"), none, true));
    }
}
