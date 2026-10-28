//! Real-time 360 player: an iced window with the stitched view, an open-file
//! dialog and playback controls. Free look on all three axes, stabilisation
//! and rolling-shutter correction run in the view's shader.

mod audio;
mod edit;
mod export;
mod gpu;
mod audio_panel;
mod bin;
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
mod framing;
mod jobs;
mod file;
mod tracks;
mod overlay_ui;
mod cuts;

use crate::ffmpeg::{self, Hwaccel};
use crate::stab::{ImuAxes, Mode, Stabilizer};
use crate::project::{Cut, Project, SoundTrack};
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
use overlay_ui::*;
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
    /// A song's waveform, read off the UI thread.
    Waveform(PathBuf, Result<Arc<music::Waveform>, String>),
    /// The waveform of what a cut plays, if it has sound.
    CutWave(PathBuf, Option<Arc<music::Waveform>>),
    /// An overlay's picture ready (by `Item::key`), and the font its text
    /// missed.
    OverlayPicture(String, Result<(Arc<crate::overlay::Picture>, Option<String>), String>),
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
    /// A track's volume point added at this edit time and level (dB).
    VolumeAdd(SoundTrack, f64, f64),
    VolumeSelect(SoundTrack, usize),
    /// A track's volume point `i` dragged to this edit time and level.
    VolumeMove(SoundTrack, usize, f64, f64),
    /// A sound point on cut `cut` at this clip time and level (dB).
    SoundKeyAdd(usize, f64, f64),
    SoundKeySelect(usize, usize),
    /// Cut `cut`'s sound point dragged to this clip time and level.
    SoundKeyMove(usize, usize, f64, f64),
    /// The mix: the clips' sound and each sound track, in dB.
    SoundLevel(f64),
    TrackLevel(SoundTrack, f64),
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
    /// Split the cut at the playhead.
    Split,
    /// Open or close the bin (the project folder's media) at the left.
    BinPanel,
    /// Read the project's folder again.
    BinRefresh,
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
    /// The cuts' sound files whose waveforms were asked for: each once, a
    /// silent one too (pictures, clips without sound).
    cut_waves_asked: std::collections::HashSet<PathBuf>,
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

    /// The overlay or text item selected, in the lane shown of its track.
    selected_overlay: Option<crate::overlay::ItemRef>,
    /// The «Framing» switch: off (editing) by default, each start.
    framing: bool,
    /// The interface's size (iced's scale factor), kept in the settings.
    ui_scale: f32,
    /// The Normalize dialog, while open.
    normalize: Option<sound_ui::Dialog>,
    /// The right-click menu on a cut's sound: which cut, where.
    sound_menu: Option<(usize, iced::Point)>,
    /// The clip selected on a sound track.
    selected_music: Option<(SoundTrack, usize)>,
    /// A sound track's volume point selected, and the cut sound point:
    /// (cut, point).
    selected_volume: Option<(SoundTrack, usize)>,
    selected_sound: Option<(usize, usize)>,
    /// Each sound track's streams, in `SoundTrack::ALL`'s order.
    music: [music::Player; 2],
    /// Through a crossfade, the other cut of it alongside: its index and
    /// clip. Usually the next one, coming in over the playing cut; while
    /// paused with the incoming cut selected for editing, that one is the
    /// current cut and this is the one before (`follow_crossfade`).
    partner: Option<(usize, Clip)>,
    edit: edit::Edit,
    /// The cut being played, and the one selected on the timeline.
    current: usize,
    selected: Option<usize>,
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
    /// A running export: its cancel flag and how far it has got.
    exporting: Option<(Arc<std::sync::atomic::AtomicBool>, f32)>,
    /// A trim or keyframe drag has saved the edit for undo already.
    dragging: bool,
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
    /// The Effects panel shows beside the view.
    /// The panel docked at the right of the view, if one is open.
    side: Option<Side>,
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

            selected_overlay: None,
            framing: false,
            normalize: None,
            sound_menu: None,
            selected_music: None,
            selected_volume: None,
            cut_waves_asked: Default::default(),
            cut_waves_reading: 0,
            selected_sound: None,
            music: Default::default(),
            partner: None,
            edit: edit::Edit::default(),
            current: 0,
            selected: None,
            dragging: false,
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
            angle_draft: None,
            modifiers: keyboard::Modifiers::default(),
            scrubbing: None,
            settings_open: false,
            bin: bin::Bin { open: saved.bin_pinned, pinned: saved.bin_pinned, ..Default::default() },
            fullscreen: false,
            volume: saved.volume,
            muted: saved.muted,
            view_size: Size::new(16.0, 9.0),
            aspect: saved.aspect.map_or(Aspect::Free, |[w, h]| Aspect::Ratio(w, h)),
            save_status: None,
            picking: None,
            picking_white: false,
            side: None,
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
            auto_key: self.auto_key,
            theme: self.theme.to_string(),
            ten_bit: self.ten_bit,
            dlog_m_lut: self.dlog_m_lut.clone(),
            bin_pinned: self.bin.pinned,
            timeline_scale: self.split_kept,
            view_angles: self.angles_shown,
            ui_scale: self.ui_scale,
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
        let quiet = matches!(message, Message::Tick | Message::LoadingTick | Message::ViewSize(_) | Message::Modifiers(_));
        if !looking && !quiet {
            self.auto_key_at = None;
        }
        // A field shows what's typed only while it's being typed.
        if !quiet && !matches!(message, Message::AngleInput(..)) {
            self.angle_draft = None;
        }
        let task = self.handle(message);
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
        let now = self.settings();
        if now != self.saved_settings {
            now.save();
            self.saved_settings = now;
        }
        task
    }

    /// Reads `path`'s waveform for the music lane, unless it has it.
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
            keyboard::Event::KeyPressed { key, modifiers, .. } => Some(Message::Key(key, modifiers)),
            keyboard::Event::ModifiersChanged(m) => Some(Message::Modifiers(m)),
            _ => None,
        });
        // The crossfade's partner too: paused, its frame came in only with
        // the next event.
        let busy = self.clip.iter().chain(self.partner.as_ref().map(|(_, c)| c)).any(|c| c.playback.busy());
        let close = window::close_requests().map(|_| Message::CloseRequested);
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
        match message {
            Message::Open => {
                let dir = self.clip.as_ref().and_then(|c| c.playback.media.input.parent().map(Path::to_owned));
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .set_title("Add clips")
                            .add_filter(
                                "Videos and pictures",
                                &[
                                    "OSV", "osv", "mp4", "MP4", "mov", "MOV", "mkv", "MKV", "m4v", "M4V", "avi", "AVI", "webm", "WEBM", "mts", "MTS", "jpg", "JPG", "jpeg", "JPEG", "png", "PNG", "webp", "WEBP", "bmp", "BMP", "tif", "TIF", "tiff", "TIFF",
                                ],
                            )
                            .add_filter("Osmo 360 video", &["OSV", "osv"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_files().await.unwrap_or_default().iter().map(|f| f.path().to_owned()).collect()
                    },
                    Message::Picked,
                );
            }
            Message::Picked(paths) => {
                self.queue.extend(paths.into_iter().map(|p| (p, true)));
                if self.loading.is_none() {
                    return self.open_next();
                }
            }
            Message::Loaded(result) => {
                self.loading = None;
                self.loaded += 1;
                match Arc::into_inner(result).expect("loaded clip is not shared") {
                    Ok(media) => {
                        if media.flat.is_none() {
                            self.opts.hw = Some(media.hw);
                        }
                        let (input, duration) = (media.input.clone(), media.duration);
                        let thumbs = (!self.thumbs.contains_key(&input)).then(|| Task::run(thumbs::stream(media.clone()), |m| m));
                        self.thumbs.entry(input.clone()).or_default();
                        self.library.push(media);
                        self.mark_flat();
                        if self.loading_adds {
                            self.add_cut(input, duration);
                        }
                        return Task::batch([self.open_next(), thumbs.unwrap_or_else(Task::none)]);
                    }
                    Err(e) => self.error = Some(e),
                }
                return self.open_next();
            }
            Message::OpenProject if self.unsaved() => return self.confirm(Then::OpenProject),
            Message::NewProject if self.unsaved() => return self.confirm(Then::NewProject),
            Message::NewProject => {
                // Back to the start screen with an empty edit. Loaded clips
                // stay in the library, so adding them again is instant; one
                // still loading finishes there without becoming a cut.
                self.edit.replace(Project::default());
                self.forget_edit_state();
                self.saved_version = self.edit.version;
                self.saved_aspect = self.aspect;
                self.project = None;
                self.queue.clear();
                self.loading_adds = false;
                self.error = None;
            }
            Message::CloseRequested if self.busy() => return self.confirm_stop(),
            Message::StopAndClose(stop) => {
                if stop {
                    self.cancel_work();
                    self.closing = self.busy();
                    if !self.closing {
                        return self.update(Message::CloseRequested);
                    }
                    self.save_status = Some("Stopping, then quitting…".to_owned());
                }
            }
            Message::CloseRequested if self.unsaved() => return self.confirm(Then::Close),
            Message::CloseRequested => {
                self.stop_preview();
                return iced::exit();
            }
            Message::Confirmed(then, choice) => match choice {
                Choice::Save => {
                    self.after_save = Some(then);
                    return self.update(Message::SaveProject(false));
                }
                Choice::Discard => {
                    // Let the changes go: the next save starts fresh.
                    self.saved_version = self.edit.version;
                    self.saved_aspect = self.aspect;
                    return self.proceed(then);
                }
                Choice::Cancel => {}
            },
            Message::OpenProject => {
                let dir = self.project.as_ref().and_then(|p| p.parent().map(Path::to_owned));
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title("Open project").add_filter("Omnix project", &["json"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_file().await.map(|f| f.path().to_owned())
                    },
                    Message::ProjectPicked,
                );
            }
            Message::ProjectPicked(None) => {}
            Message::SaveProjectTo(None) => self.after_save = None,
            Message::ProjectPicked(Some(path)) => {
                let project = match Project::load(&path) {
                    Ok(p) => p,
                    Err(e) => {
                        self.error = Some(format!("{e:#}"));
                        return Task::none();
                    }
                };
                // Cuts of clips that are gone can't play; say so and leave them out.
                let (mut cuts, missing): (Vec<_>, Vec<_>) = project.cuts.into_iter().partition(|c| c.input.exists());
                // Each cut gets a mode of its own, so changing one leaves the rest.
                for c in &mut cuts {
                    c.stabilize.get_or_insert(self.opts.stabilize);
                }
                self.error = (!missing.is_empty()).then(|| {
                    let names: Vec<_> = missing.iter().map(|c| c.input.display().to_string()).collect();
                    format!("Left out cuts of missing clips: {}", names.join(", "))
                });
                // A cut's audio file that's gone: its clip's own sound plays,
                // and the file stays named, so it comes back once found.
                let mut deaf: Vec<String> = cuts.iter().filter_map(|c| c.audio.as_ref()).filter(|a| !a.exists()).map(|a| a.display().to_string()).collect();
                deaf.dedup();
                if !deaf.is_empty() {
                    let said = self.error.take().map_or(String::new(), |e| e + ". ");
                    self.error = Some(format!("{said}Missing sound files, playing the clips' own: {}", deaf.join(", ")));
                }
                self.aspect = project.aspect.map_or(Aspect::Free, |a| Aspect::Ratio(a.0 as u16, a.1 as u16));
                self.forget_edit_state();
                // Music and sound effects that are gone are left out the same way.
                let (mut music, mut sfx) = (project.music, project.sfx);
                let mut gone = Vec::new();
                for (track, word) in [(&mut music, "music"), (&mut sfx, "sound effects")] {
                    let (here, lost): (Vec<_>, Vec<_>) = std::mem::take(&mut track.clips).into_iter().partition(|c| c.input.exists());
                    track.clips = here;
                    if !lost.is_empty() {
                        let names: Vec<_> = lost.iter().map(|c| c.input.display().to_string()).collect();
                        let said = self.error.take().map_or(String::new(), |e| e + ". ");
                        self.error = Some(format!("{said}Left out missing {word}: {}", names.join(", ")));
                    }
                    gone.extend(lost);
                }
                // And overlay pictures.
                let (mut overlays, texts) = (project.overlays, project.text);
                let mut lost = Vec::new();
                for lane in &mut overlays {
                    let (here, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut lane.items).into_iter().partition(|it| it.text.is_some() || it.input.exists());
                    lane.items = here;
                    lost.extend(gone.into_iter().map(|it| it.input.display().to_string()));
                }
                if !lost.is_empty() {
                    let said = self.error.take().map_or(String::new(), |e| e + ". ");
                    self.error = Some(format!("{said}Left out missing overlays: {}", lost.join(", ")));
                }
                let waves: Vec<_> = music.clips.iter().chain(&sfx.clips).map(|c| self.load_waveform(&c.input)).collect();
                self.queue.extend(cuts.iter().map(|c| (c.input.clone(), false)));
                let complete = missing.is_empty() && gone.is_empty() && lost.is_empty();
                self.edit.replace(Project { aspect: None, cuts, sound: project.sound, music, sfx, fades: project.fades, overlays, text: texts });
                self.mark_flat();
                self.saved_version = if complete { self.edit.version } else { 0 };
                self.saved_aspect = self.aspect;
                self.project = Some(path);
                let next = if self.loading.is_none() { self.open_next() } else { Task::none() };
                return Task::batch(waves.into_iter().chain([next]));
            }
            Message::SaveProject(as_new) => {
                if !self.has_edit() {
                    return Task::none();
                }
                if let (Some(path), false) = (&self.project, as_new) {
                    return self.update(Message::SaveProjectTo(Some(path.clone())));
                }
                // Named after the first clip, or the song (or sound effect) a
                // project starts from: `has_edit` holds one of them.
                let Some(first) = self.edit.cuts.first().map(|c| &c.input).or(self.edit.music.clips.first().map(|c| &c.input)).or(self.edit.sfx.clips.first().map(|c| &c.input)) else {
                    return Task::none();
                };
                let dir = self.project.as_ref().unwrap_or(first).parent().map(Path::to_owned);
                let name = self.project.as_ref().and_then(|p| p.file_name()).map_or_else(
                    || format!("{}_edit.json", first.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
                    |n| n.to_string_lossy().into_owned(),
                );
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title("Save project").add_filter("Omnix project", &["json"]).set_file_name(name);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.save_file().await.map(|f| f.path().to_owned())
                    },
                    Message::SaveProjectTo,
                );
            }
            Message::Resolution(r) => self.resolution = r,
            Message::ExportClips(on) => self.export_clips = on,
            Message::ExportGraded(on) => self.export_graded = on,
            Message::ExportFormat(f) => self.export_format = f,
            Message::Info(of) => {
                let path = match of {
                    info::Of::Cut(i) => self.edit.cuts.get(i).map(|c| c.input.clone()),
                    info::Of::Music(which, i) => self.edit.track(which).clips.get(i).map(|m| m.input.clone()),
                    info::Of::Overlay(r) => self.edit.item(r).filter(|it| it.text.is_none()).map(|it| it.input.clone()),
                };
                let Some(path) = path else { return Task::none() };
                self.settings_open = false;
                // Read once per file: another cut of it shows at once.
                let file = self.info.take().filter(|i| i.path == path).and_then(|i| i.file).filter(|f| f.is_ok());
                let read = file.is_none();
                self.info = Some(info::Info { of, path: path.clone(), file });
                if read {
                    let song = matches!(of, info::Of::Music(..));
                    let p = path.clone();
                    return Task::perform(blocking(move || info::read(&p, song).map(Arc::new).map_err(|e| format!("{e:#}"))), move |r| Message::InfoRead(path.clone(), r));
                }
            }
            Message::InfoRead(path, file) => {
                if let Some(info) = &mut self.info
                    && info.path == path
                {
                    info.file = Some(file);
                }
            }
            Message::InfoClose => self.info = None,
            Message::Reveal(path) => {
                std::thread::spawn(move || {
                    if let Err(e) = info::reveal(&path) {
                        eprintln!("couldn't show {}: {e:#}", path.display());
                    }
                });
            }
            Message::CopyPath(path) => {
                self.save_status = Some("Copied the file's path".to_owned());
                return iced::clipboard::write(path.display().to_string());
            }
            Message::ExportPanel => self.toggle_side(Side::Export),
            Message::AudioPanel => self.toggle_side(Side::Audio),
            Message::SettingsPanel => {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    self.info = None;
                }
            }
            // Pinned, it stays open.
            Message::BinPanel if self.bin.open && self.bin.pinned => {}
            Message::BinPanel => self.bin.open = !self.bin.open,
            Message::BinPin => {
                self.bin.pinned = !self.bin.pinned;
                self.bin.open |= self.bin.pinned;
            }
            Message::BinRefresh => return self.rescan_bin(),
            Message::BinPreview(path, kind) => return self.bin_preview(path, kind),
            Message::BinStop => self.stop_preview(),
            Message::BinMeta(read) => self.bin.meta.extend(read),
            Message::BinFrames(path, frames) => {
                if let Some(p) = self.bin.preview.as_mut().filter(|p| p.path == path) {
                    p.frames = frames;
                }
            }
            Message::BinAdd(path, bin::Kind::Clip, place) => {
                // Where it goes is decided now, from the playhead now: the
                // clip may take a while to load.
                let spot = self.spot(place);
                self.placing.insert(path.clone(), spot);
                return self.update(Message::Picked(vec![path]));
            }
            Message::BinAdd(path, bin::Kind::Picture, _) => return self.update(Message::OverlayPicked(vec![path])),
            Message::BinAdd(path, bin::Kind::Sound(which), place) => {
                // Onto its section's track, where it fits nearest the spot,
                // decided now from the playhead now.
                let at = match self.spot(place) {
                    bin::Spot::End => None,
                    bin::Spot::At(t) => Some(t),
                    bin::Spot::Join(t) => Some(crate::project::offsets(&self.edit.cuts).into_iter().chain([self.edit.duration()]).nth(self.edit.nearest_join(t)).unwrap_or(t)),
                };
                return probe_sound(which, path, at);
            }
            Message::AutoKey(on) => self.auto_key = on,
            Message::SplitDrag(dy) => self.split = (self.split_kept - dy / timeline::SCALABLE).clamp(*timeline::SCALES.start(), *timeline::SCALES.end()),
            Message::SplitDone => self.split_kept = self.split,
            Message::SplitReset => (self.split, self.split_kept) = (1.0, 1.0),
            Message::TenBit(on) => {
                self.ten_bit = on;
                source::set_deep(on);
            }
            Message::AddLut => {
                return Task::perform(
                    async { rfd::AsyncFileDialog::new().set_title("Add a LUT").add_filter("3D LUT", &["cube", "CUBE"]).pick_file().await.map(|f| f.path().to_owned()) },
                    Message::LutPicked,
                );
            }
            Message::LutPicked(None) => {}
            Message::LutPicked(Some(path)) => match crate::lut::add_to_library(&path) {
                Ok(name) => {
                    self.luts = crate::lut::library_names();
                    self.save_status = Some("LUT added".to_owned());
                    // DJI's LUT added with no choice made yet: D-Log M
                    // clips get it, those in the edit too.
                    let squashed = name.to_lowercase().replace([' ', '-', '_'], "");
                    if self.dlog_m_lut.is_none() && squashed.contains("dlogm") {
                        self.dlog_m_lut = Some(name);
                        self.lut_dlog_m_cuts();
                    }
                    self.mark_flat();
                }
                Err(e) => self.error = Some(format!("Couldn't add the LUT: {e:#}")),
            },
            Message::RemoveLut(name) => {
                if let Err(e) = crate::lut::remove_from_library(&name) {
                    self.error = Some(format!("{e:#}"));
                }
                if self.dlog_m_lut.as_ref() == Some(&name) {
                    self.dlog_m_lut = None;
                }
                self.luts = crate::lut::library_names();
                self.mark_flat();
            }
            Message::DlogMLut(choice) => self.dlog_m_lut = choice.name(),
            Message::LookDone => {}
            Message::Export => {
                if self.edit.cuts.is_empty() || self.exporting.is_some() {
                    return Task::none();
                }
                let dir = self.export_base().parent().map(Path::to_owned);
                let (clips, ext) = (self.export_clips, self.export_format.extension());
                let name = format!("{}_{}.{ext}", self.export_name(), self.resolution);
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new();
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        if clips {
                            dialog.set_title("Export the cuts as clips, into the folder").pick_folder().await.map(|f| f.path().to_owned())
                        } else {
                            let kind = if ext == "mov" { "QuickTime video" } else { "MP4 video" };
                            dialog.set_title("Export video").add_filter(kind, &[ext]).set_file_name(name).save_file().await.map(|f| f.path().to_owned())
                        }
                    },
                    Message::ExportTo,
                );
            }
            Message::ExportTo(None) => {}
            Message::ExportTo(Some(path)) => return self.start_export(path),
            Message::ExportProgress(f) => {
                if let Some((_, done)) = &mut self.exporting {
                    *done = f;
                }
            }
            Message::ExportCancel => {
                if let Some((cancel, _)) = &self.exporting {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::Effect(e) => self.effect_message(e),
            Message::PickColour if self.on_flat() => {}
            Message::PickColour => {
                self.picking_white = false;
                if self.picking.take().is_none() && self.clip.is_some() {
                    self.with_playback(Playback::pause);
                    self.picking = Some(None);
                    self.save_status = Some("Draw a box over the subject to track its colour (Esc cancels)".to_owned());
                } else {
                    self.save_status = None;
                }
            }
            Message::PickArea(area) => {
                if self.picking.is_some() {
                    self.picking = Some(Some(area));
                }
            }
            Message::AreaPicked(area, aspect) => {
                self.picking = None;
                self.save_status = None;
                if area.width < 0.004 || area.height < 0.004 {
                    return Task::none();
                }
                if std::mem::take(&mut self.picking_white) {
                    if let Some((rgb, _)) = self.sample_area(area, aspect) {
                        self.white_balance(rgb);
                    }
                } else if let Some((rgb, dir)) = self.sample_area(area, aspect) {
                    match crate::track::Colour::from_rgb(rgb, 18.0) {
                        Ok(colour) => return self.start_track(colour, crate::track::hex(rgb), dir),
                        Err(e) => self.error = Some(format!("{e:#}")),
                    }
                }
            }
            Message::TrackProgress(f) => {
                if let Some((_, done)) = &mut self.tracking {
                    *done = f;
                }
            }
            Message::TrackCancel => {
                if let Some((cancel, _)) = &self.tracking {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::Tracked(result) => {
                let cancelled = self.tracking.take().is_some_and(|(c, _)| c.load(std::sync::atomic::Ordering::Relaxed));
                match result {
                    Ok(done) => self.follow_track(done),
                    Err(_) if cancelled => self.save_status = Some("Tracking cancelled".to_owned()),
                    Err(e) => self.error = Some(format!("Tracking failed: {e}")),
                }
                return self.close_when_stopped();
            }
            Message::ExportDone(result) => {
                let cancelled = self.exporting.take().is_some_and(|(c, _)| c.load(std::sync::atomic::Ordering::Relaxed));
                match result {
                    Ok(paths) => {
                        let name = |p: &PathBuf| p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        self.save_status = Some(match paths.as_slice() {
                            [one] => format!("Exported {}", name(one)),
                            [first, ..] => format!("Exported {} clips to {}", paths.len(), first.parent().map_or_else(String::new, |d| d.display().to_string())),
                            [] => "Exported nothing".to_owned(),
                        });
                    }
                    Err(_) if cancelled => self.save_status = Some("Export cancelled".to_owned()),
                    Err(e) => self.error = Some(format!("Export failed: {e}")),
                }
                return self.close_when_stopped();
            }
            Message::SaveProjectTo(Some(path)) => match self.to_project().save(&path) {
                Ok(()) => {
                    self.saved_version = self.edit.version;
                    self.saved_aspect = self.aspect;
                    // No "Saved …" (user: just annoying): the title's unsaved
                    // mark going is enough.
                    self.save_status = None;
                    self.project = Some(path);
                    if let Some(then) = self.after_save.take() {
                        return self.proceed(then);
                    }
                }
                Err(e) => {
                    self.after_save = None;
                    self.error = Some(format!("Saving the project failed: {e:#}"));
                }
            },
            Message::LoadingTick => {}
            Message::Tick => {
                self.tick();
                if self.clip.as_ref().is_some_and(|c| c.playback.playing) {
                    self.reveal_playhead();
                }
            }
            Message::Key(key, mods) => {
                let task = self.key(key, mods);
                // Keys can seek out of sight (arrows, [ ]): follow.
                self.reveal_playhead();
                return task;
            }
            Message::TimelineZoom(factor, anchor) => self.zoom_timeline(self.timeline_zoom * factor, anchor),
            Message::TimelineZoomTo(v) => self.zoom_timeline(self.timeline_most_zoom().powf(v), None),
            Message::TimelinePan(start) => self.timeline_start = start,
            Message::Toggle => {
                self.follow = true;
                let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
                if !playing && self.position() >= self.edit.duration() - 1.5 / self.fps() {
                    // From the end of the edit, play again from the start.
                    let start = self.edit.cuts.first().map_or(0.0, |c| c.start);
                    self.goto(0, start, true);
                } else {
                    self.with_playback(Playback::toggle);
                }
            }
            Message::Scrub(t) => {
                self.follow = true;
                // In an overlap, the selected cut's time, if it's one of the two.
                let selected = self.selected.filter(|&i| i < self.edit.cuts.len()).and_then(|i| {
                    let (from, c) = (self.edit.offset(i), &self.edit.cuts[i]);
                    (t >= from && t < from + c.len()).then(|| (i, c.start + t - from))
                });
                if let Some((i, t)) = selected.or_else(|| self.edit.locate(t)) {
                    self.scrub_to(i, t);
                }
            }
            Message::ScrubDone => {
                self.dragging = false;
                self.with_playback(Playback::scrub_end);
                if let Some((_, inc)) = &mut self.partner {
                    inc.playback.scrub_end();
                }
            }
            Message::Select(i) => {
                // The other cut of a crossfade selected while paused: it
                // takes the overlap over, to be framed there.
                if self.partner.as_ref().is_some_and(|(j, _)| *j == i) && self.clip.as_ref().is_some_and(|c| !c.playback.playing) {
                    self.swap_partner();
                }
                self.selected = Some(i);
                self.selected_music = None;
                self.selected_volume = None;
                self.selected_sound = None;
                self.selected_overlay = None;
            }
            Message::AddOverlay => {
                let dir = self.project.as_ref().and_then(|p| p.parent().map(Path::to_owned));
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .set_title("Add overlay pictures")
                            .add_filter("Pictures", &["png", "PNG", "jpg", "JPG", "jpeg", "JPEG", "webp", "WEBP", "bmp", "BMP", "tif", "TIF", "tiff", "TIFF"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_files().await.unwrap_or_default().iter().map(|f| f.path().to_owned()).collect()
                    },
                    Message::OverlayPicked,
                );
            }
            Message::OverlayPicked(paths) if !paths.is_empty() => {
                self.edit.checkpoint();
                // One after another from the playhead, each where it fits.
                let mut at = self.position();
                let lane = self.edit.lane_mut(crate::overlay::Track::Overlay);
                for input in paths {
                    lane.items.push(crate::overlay::Item { input, text: None, at, start: 0.0, end: ffmpeg::PICTURE_SECONDS, motion: Vec::new(), effects: Vec::new() });
                    let i = lane.items.len() - 1;
                    lane.items[i].at = lane.place(i, at);
                    at = lane.items[i].ends();
                    self.selected_overlay = Some((crate::overlay::Track::Overlay, i));
                }
                self.selected_music = None;
                self.selected_volume = None;
                self.selected_sound = None;
            }
            Message::OverlayPicked(_) => {}
            Message::OverlaySelect(i) => {
                // Typing in another overlay is a change of its own for undo.
                self.dragging = false;
                self.selected_overlay = i;
                if i.is_some() {
                    self.selected_music = None;
                    self.selected_volume = None;
                    self.selected_sound = None;
                }
            }
            Message::OverlayMove((track, i), at) => {
                if self.edit.item((track, i)).is_some() {
                    self.begin_drag();
                    let lane = self.edit.lane_mut(track);
                    lane.items[i].at = lane.place(i, at);
                }
            }
            Message::OverlayTrim((track, i), start, t) => {
                if self.edit.item((track, i)).is_some() {
                    self.begin_drag();
                    let lane = self.edit.lane_mut(track);
                    let (lo, hi) = lane.room(i);
                    let item = &mut lane.items[i];
                    if start {
                        // Its picture stays where it is on the edit; a
                        // picture has nothing before its own start.
                        let earliest = (item.start - (item.at - lo)).max(0.0);
                        let t = t.clamp(earliest, item.end - edit::MIN_CUT);
                        item.at += t - item.start;
                        item.start = t;
                    } else {
                        let latest = (item.start + (hi - item.at)).min(item.start + ffmpeg::PICTURE_MOST);
                        item.end = t.clamp(item.start + edit::MIN_CUT, latest);
                    }
                }
            }
            Message::OverlayJump(at) => {
                self.follow = true;
                if let Some((i, t)) = self.edit.locate(at) {
                    self.goto(i, t, false);
                }
            }
            Message::OverlayKeyMove(r, effects, key, t) => {
                self.follow = true;
                self.begin_drag();
                let gap = 1.0 / self.fps();
                let Some(item) = self.edit.item_mut(r) else { return Task::none() };
                let local = if effects {
                    crate::view::retime(&mut item.effects, key, t, gap);
                    item.effects.get(key).map(|k| k.t)
                } else {
                    crate::view::retime(&mut item.motion, key, t, gap);
                    item.motion.get(key).map(|k| k.t)
                };
                // Follow the mark, so what it holds shows as it moves.
                if let Some(at) = local.map(|t| item.at + t - item.start)
                    && let Some((i, t)) = self.edit.locate(at)
                {
                    self.scrub_to(i, t);
                }
            }
            Message::AddSound(which) => {
                let dir = self.project.as_ref().and_then(|p| p.parent().map(Path::to_owned));
                let (title, filter) = match which {
                    SoundTrack::Music => ("Add music", "Music"),
                    SoundTrack::Sfx => ("Add a sound effect", "Sounds"),
                };
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title(title).add_filter(filter, &["mp3", "MP3", "flac", "wav", "WAV", "m4a", "aac", "ogg", "opus", "wma"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_file().await.map(|f| f.path().to_owned())
                    },
                    move |picked| Message::SoundPicked(which, picked),
                );
            }
            Message::SoundPicked(_, None) => {}
            Message::SoundPicked(which, Some(path)) => {
                // A song after the music already there, whole; a sound
                // effect at the playhead, where it's wanted.
                let at = match which {
                    SoundTrack::Music => None,
                    SoundTrack::Sfx => Some(self.position()),
                };
                return probe_sound(which, path, at);
            }
            Message::SoundProbed(_, path, _, Err(e)) => self.error = Some(format!("Can't add {}: {e}", path.display())),
            Message::SoundProbed(which, path, at, Ok(len)) => {
                self.edit.checkpoint();
                let at = at.unwrap_or_else(|| self.edit.track(which).end());
                let i = self.edit.track_mut(which).append(path.clone(), at, len);
                self.selected_music = Some((which, i));
                self.selected = None;
                return self.load_waveform(&path);
            }
            Message::Sound(msg) => return self.sound_ui(msg),
            Message::UiScale(steps) => {
                self.ui_scale = if steps == 0 {
                    settings::UI_DEFAULT
                } else {
                    // Rounded to the step, so repeated clicks land on whole percents.
                    ((self.ui_scale + steps as f32 * settings::UI_STEP) / settings::UI_STEP).round() * settings::UI_STEP
                }
                .clamp(*settings::UI_SCALE.start(), *settings::UI_SCALE.end());
            }
            Message::Framing(on) => {
                self.framing = on;
                if !on {
                    self.picking = None;
                }
            }
            Message::OverlayPicture(key, Ok((picture, missing))) => {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                if let Some(font) = missing {
                    self.error = Some(format!("The font {font} isn't on this computer; its text shows in Inter"));
                }
                self.overlay_thumbs.insert(key.clone(), overlay_thumb(&picture));
                self.overlay_pictures.insert(key, Some((NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), picture)));
            }
            Message::OverlayPicture(key, Err(e)) => {
                self.error = Some(format!("Couldn't read the overlay {}: {e}", key.trim_start_matches("file:")));
            }
            Message::AddText => {
                self.edit.checkpoint();
                let at = self.position();
                let lane = self.edit.lane_mut(crate::overlay::Track::Text);
                lane.items.push(crate::overlay::Item { input: PathBuf::new(), text: Some(Default::default()), at, start: 0.0, end: ffmpeg::PICTURE_SECONDS, motion: Vec::new(), effects: Vec::new() });
                let i = lane.items.len() - 1;
                lane.items[i].at = lane.place(i, at);
                self.selected_overlay = Some((crate::overlay::Track::Text, i));
                self.selected_music = None;
                self.selected_volume = None;
                self.selected_sound = None;
                // Its words and look are set in the text panel, open now.
                self.text_of = None;
            }
            Message::Waveform(path, Ok(wave)) => {
                self.waveforms.insert(path, wave);
            }
            Message::Waveform(path, Err(e)) => self.error = Some(format!("Can't read {}: {e}", path.display())),
            Message::CutWave(path, wave) => {
                self.cut_waves_reading = self.cut_waves_reading.saturating_sub(1);
                if let Some(wave) = wave {
                    self.waveforms.insert(path, wave);
                }
            }
            Message::SelectMusic(which, i) => {
                self.selected_music = Some((which, i));
                self.selected_overlay = None;
                self.selected = None;
                self.selected_volume = None;
                self.selected_sound = None;
            }
            Message::VolumeAdd(which, t, db) => {
                self.begin_drag();
                let keys = &mut self.edit.track_mut(which).keyframes;
                let i = keys.partition_point(|k| k.t <= t);
                keys.insert(i, crate::project::VolumeKey { t, volume: db });
                self.select_volume(which, i);
            }
            Message::VolumeSelect(which, i) => self.select_volume(which, i),
            Message::SoundKeyAdd(cut, t, db) => {
                let Some(c) = self.edit.cuts.get(cut) else { return Task::none() };
                let t = t.clamp(c.start, c.end);
                self.begin_drag();
                let keys = &mut self.edit.cuts[cut].volume;
                let i = keys.partition_point(|k| k.t <= t);
                keys.insert(i, crate::project::VolumeKey { t, volume: db });
                self.select_sound(cut, i);
            }
            Message::SoundKeySelect(cut, i) => self.select_sound(cut, i),
            // A point gone mid-drag (an undo) is no undo step, nor an index panic.
            Message::VolumeMove(which, i, ..) if i >= self.edit.track(which).keyframes.len() => {}
            Message::SoundKeyMove(cut, i, ..) if self.edit.cuts.get(cut).is_none_or(|c| i >= c.volume.len()) => {}
            Message::SoundKeyMove(cut, i, t, db) => {
                let Some(c) = self.edit.cuts.get(cut) else { return Task::none() };
                let (start, end) = (c.start, c.end);
                self.begin_drag();
                let keys = &mut self.edit.cuts[cut].volume;
                // Within the cut and between its neighbours.
                let lo = i.checked_sub(1).map_or(start, |j| keys[j].t + 0.01).max(start);
                let hi = keys.get(i + 1).map_or(end, |k| k.t - 0.01).min(end);
                if let Some(k) = keys.get_mut(i) {
                    *k = crate::project::VolumeKey { t: t.clamp(lo, hi.max(lo)), volume: db };
                }
            }
            Message::VolumeMove(which, i, t, db) => {
                self.begin_drag();
                let keys = &mut self.edit.track_mut(which).keyframes;
                // Between its neighbours: points keep their order.
                let lo = i.checked_sub(1).map_or(0.0, |j| keys[j].t + 0.01);
                let hi = keys.get(i + 1).map_or(f64::INFINITY, |k| k.t - 0.01);
                if let Some(k) = keys.get_mut(i) {
                    *k = crate::project::VolumeKey { t: t.clamp(lo, hi.max(lo)), volume: db };
                }
            }
            // A stale index (the clip gone mid-drag) is no undo step.
            Message::MusicMove(which, i, _, _) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicTrim(which, i, ..) if i >= self.edit.track(which).clips.len() => {}
            Message::KeyMove(i, k, _) if self.edit.cuts.get(i).is_none_or(|c| k >= c.keyframes.len()) => {}
            Message::MusicMove(which, i, want, overlap) => {
                self.begin_drag();
                if i < self.edit.track(which).clips.len() {
                    let m = self.edit.track(which);
                    let c = &m.clips[i];
                    let len = c.end - c.start;
                    // A plain drag never overlaps: it stops edge to edge, or
                    // hops past. With Shift it slides in as far as it may.
                    let at = if overlap || m.crossfades(i) != (0.0, 0.0) {
                        crate::project::Music::reach(c.at, want.max(0.0), |at| m.fits(i, at, len, overlap))
                    } else {
                        m.place(i, want)
                    };
                    self.edit.track_mut(which).clips[i].at = at;
                }
            }
            Message::MusicTrim(which, i, in_edge, t, overlap) => {
                self.begin_drag();
                let song = self.waveforms.get(&self.edit.track(which).clips[i].input).map_or(f64::INFINITY, |w| w.duration);
                let m = self.edit.track(which);
                let c = &m.clips[i];
                // Song time to edit time on this clip: the song stays put on
                // the edit, so trimming its start moves where the clip
                // begins, not what plays when. Within the song and the edit,
                // and into a neighbour only with Shift.
                let to_edit = c.at - c.start;
                let (start, end) = (c.start, c.end);
                if in_edge {
                    let want = t.max(start - c.at).max(0.0).min(end - edit::MIN_CUT);
                    let start = crate::project::Music::reach(start, want, |s| m.fits(i, s + to_edit, end - s, overlap));
                    let c = &mut self.edit.track_mut(which).clips[i];
                    c.at = start + to_edit;
                    c.start = start;
                } else {
                    let want = t.min(song).max(start + edit::MIN_CUT);
                    let end = crate::project::Music::reach(end, want, |e| m.fits(i, c.at, e - start, overlap));
                    self.edit.track_mut(which).clips[i].end = end;
                }
            }
            Message::SoundLevel(db) => {
                self.begin_drag();
                self.edit.sound.volume = db;
                let v = self.clip_volume();
                self.with_playback(|p| p.set_volume(v));
            }
            Message::TrackLevel(which, db) => {
                self.begin_drag();
                self.edit.track_mut(which).volume = db;
            }
            Message::LevelDone => self.dragging = false,
            Message::Trim(i, start, t) => {
                let Some(clip_len) = self.edit.cuts.get(i).and_then(|c| self.library.iter().find(|m| m.input == c.input)).map(|m| m.duration) else {
                    return Task::none();
                };
                self.begin_drag();
                self.edit.trim(i, start, t, clip_len);
                let c = &self.edit.cuts[i];
                // Show the frame at the edge being trimmed.
                let t = if start { c.start } else { c.end - 1.0 / self.fps() };
                self.follow = true;
                self.scrub_to(i, t);
            }
            Message::MoveCut(from, to) => {
                if to != from && to != from + 1 {
                    self.edit.checkpoint();
                    self.selected_sound = None;
                    let at = |i: usize| {
                        let landed = if to > from { to - 1 } else { to };
                        if i == from {
                            return landed;
                        }
                        let j = if i > from { i - 1 } else { i };
                        if j >= landed { j + 1 } else { j }
                    };
                    self.edit.move_cut(from, to);
                    self.current = at(self.current);
                    self.selected = Some(at(from));
                }
            }
            Message::Crossfade(i, seconds) => {
                self.begin_drag();
                self.edit.set_crossfade(i, seconds);
            }
            Message::ToggleCrossfade => {
                let i = self.selected.unwrap_or(self.current);
                let Some(c) = self.edit.cuts.get(i) else { return Task::none() };
                let to = if c.crossfade > 0.0 { 0.0 } else { 1.0 };
                self.edit.checkpoint();
                self.edit.set_crossfade(i, to);
            }
            Message::Split if self.selected_music.is_some() => {
                let (which, i) = self.selected_music.unwrap();
                let at = self.position();
                if self.edit.attempt(|e| e.track_mut(which).split(i, at, edit::MIN_CUT)) {
                    self.selected_music = Some((which, i + 1));
                }
            }
            Message::Split => {
                let Some((_, t)) = self.playing_cut() else { return Task::none() };
                let i = self.current;
                if self.edit.attempt(|e| e.split(i, t)) {
                    // The playhead is now at the start of the second half.
                    self.current = i + 1;
                    self.selected = Some(i + 1);
                    // The cuts after it moved up one: a sound point selected
                    // by (cut, point) would now name another cut's.
                    self.selected_sound = None;
                }
            }
            Message::DeleteCut if self.selected_overlay.is_some() => {
                let (track, i) = self.selected_overlay.take().unwrap();
                if self.edit.item((track, i)).is_some() {
                    self.edit.checkpoint();
                    self.edit.lane_mut(track).items.remove(i);
                }
            }
            Message::DeleteCut if self.selected_sound.is_some() => {
                let (cut, i) = self.selected_sound.take().unwrap();
                if self.edit.cuts.get(cut).is_some_and(|c| i < c.volume.len()) {
                    self.edit.checkpoint();
                    self.edit.cuts[cut].volume.remove(i);
                }
            }
            Message::DeleteCut if self.selected_volume.is_some() => {
                let (which, i) = self.selected_volume.take().unwrap();
                if i < self.edit.track(which).keyframes.len() {
                    self.edit.checkpoint();
                    self.edit.track_mut(which).keyframes.remove(i);
                }
            }
            // With a song or sound effect selected, delete that, not the
            // cut under the playhead (Shift+Del used to, being the key for
            // cuts).
            Message::DeleteCut if self.selected_music.is_some() => {
                let (which, i) = self.selected_music.take().unwrap();
                if i < self.edit.track(which).clips.len() {
                    self.edit.checkpoint();
                    self.edit.track_mut(which).clips.remove(i);
                }
            }
            Message::DeleteCut => {
                let Some(i) = self.selected.or(self.clip.as_ref().map(|_| self.current)) else { return Task::none() };
                if i >= self.edit.cuts.len() {
                    return Task::none();
                }
                self.edit.checkpoint();
                self.edit.remove(i);
                self.after_edit(self.edit.offset(i.min(self.edit.cuts.len())));
            }
            Message::Undo | Message::Redo => {
                let at = self.position();
                let changed = if matches!(message, Message::Undo) { self.edit.undo() } else { self.edit.redo() };
                if changed {
                    self.mark_flat();
                    self.after_edit(at);
                }
            }
            Message::KeyAdd => {
                if self.playing_cut().is_some() {
                    self.edit.checkpoint();
                    self.key_view();
                }
            }
            Message::KeyDelete => {
                if let Some(k) = self.current_key() {
                    self.edit.checkpoint();
                    self.edit.cuts[self.current].keyframes.remove(k);
                }
            }
            Message::Thumb(input, t, handle) => {
                let thumbs = self.thumbs.entry(input).or_default();
                let i = thumbs.partition_point(|(at, _)| *at < t);
                thumbs.insert(i, (t, handle));
            }
            Message::KeyEase(ease) => {
                if let Some(k) = self.current_key()
                    && self.edit.cuts[self.current].keyframes[k].ease != ease
                {
                    self.edit.checkpoint();
                    self.edit.cuts[self.current].keyframes.set_ease(k, ease);
                    self.follow = true;
                }
            }
            Message::KeyFollow(_) | Message::Stab(_) | Message::Projection(_) if self.on_flat() => {}
            Message::KeyFollow(aiming) => {
                let (Some(k), Some((_, t))) = (self.current_key(), self.playing_cut()) else { return Task::none() };
                if Aiming::of(&self.edit.cuts[self.current].keyframes[k].follow) == aiming {
                    return Task::none();
                }
                let follow = match aiming {
                    Aiming::Fixed => None,
                    Aiming::Stick => Some(Follow::Stick),
                    // A track comes from picking its colour; the keyframe
                    // here follows it once tracked.
                    Aiming::Colour => return self.update(Message::PickColour),
                };
                let aims = self.aims(t);
                self.edit.checkpoint();
                self.edit.cuts[self.current].keyframes.set_follow(k, follow, &|f, _| aims.iter().find(|a| &a.0 == f).and_then(|a| a.1));
                self.follow = true;
            }
            Message::KeyJump(i, k) => {
                self.follow = true;
                self.selected = Some(i);
                if let Some(t) = self.edit.cuts.get(i).and_then(|c| c.keyframes.get(k)).map(|k| k.t) {
                    let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
                    self.goto(i, t, playing);
                }
            }
            Message::KeyMove(i, k, t) => {
                self.follow = true;
                self.begin_drag();
                let gap = 1.0 / self.fps();
                let Some(cut) = self.edit.cuts.get_mut(i) else { return Task::none() };
                cut.keyframes.retime(k, t, gap);
                let t = cut.keyframes[k].t;
                self.scrub_to(i, t);
            }
            Message::Look { yaw, pitch, roll } => {
                let look = self.free_look();
                look.yaw += yaw.to_degrees();
                look.pitch += pitch.to_degrees();
                look.roll += roll.to_degrees();
            }
            Message::Zoom(deg) => self.free_look().zoom_by(deg),
            Message::Fov(deg) => self.free_look().fov = deg,
            Message::AngleInput(angle, typed) => self.type_angle(angle, typed),
            Message::AngleDone => {}
            Message::AngleScrubStart(angle) => self.scrubbing = Some(angle),
            Message::AngleScrubDone => self.scrubbing = None,
            Message::AngleScrub(angle, dx) => self.scrub_angle(angle, dx),
            Message::AngleNudge(angle, dir) => self.nudge_angle(angle, dir),
            Message::AngleArrow(Some(angle), dir) => self.nudge_angle(angle, dir),
            Message::AngleArrow(None, dir) => self.free_look().pitch += 5.0 * dir,
            Message::ShowAngles(on) => self.angles_shown = on,
            Message::Modifiers(m) => self.modifiers = m,
            Message::Stab(mode) => {
                // The playing cut's, and the one new cuts get.
                self.opts.stabilize = mode;
                if self.edit.cuts.get(self.current).is_some_and(|c| c.stabilize != Some(mode)) {
                    self.edit.checkpoint();
                    self.edit.cuts[self.current].stabilize = Some(mode);
                }
                self.with_playback(|p| p.media.stab.set_mode(mode));
            }
            Message::Projection(p) => {
                let look = self.free_look();
                look.projection = p;
                look.reset();
            }
            Message::Source(s) => {
                let size = match s {
                    Source::Proxy => None,
                    Source::Osv(size) => Some(size),
                };
                self.opts.osv_size = size;
                self.with_playback(|p| p.set_osv_size(size));
            }
            Message::RollingShutter(on) => self.rolling_shutter = on,
            Message::FullOnPause(on) => {
                self.full_on_pause = on;
                self.with_playback(|p| p.set_full_on_pause(on));
            }
            Message::ViewSize(size) => self.view_size = size,
            Message::Aspect(a) => self.aspect = a,
            Message::Theme(theme) => self.theme = theme,
            Message::Save(what) => {
                let Some(c) = &self.clip else { return Task::none() };
                let media = &c.playback.media;
                let stem = media.input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let name = format!("{stem}_{:.2}s_{}.png", c.playback.shown_pts, if what == Saving::View { "view" } else { "360" });
                let dir = media.input.parent().map(Path::to_owned);
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title("Save photo").add_filter("PNG image", &["png"]).set_file_name(name);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.save_file().await.map(|f| f.path().to_owned())
                    },
                    move |path| Message::SaveTo(what, path),
                );
            }
            Message::SaveTo(_, None) => {}
            Message::SaveTo(what, Some(path)) => {
                let Some(c) = &self.clip else { return Task::none() };
                let media = &c.playback.media;
                let shot = match what {
                    Saving::Sphere => export::Shot::Sphere { width: SAVE_SPHERE_PX },
                    Saving::View => {
                        let k = SAVE_VIEW_PX / self.view_size.width.max(self.view_size.height).max(1.0);
                        // Even sizes keep image tools that halve chroma happy.
                        let px = |v: f32| ((v * k / 2.0).round() as usize).max(1) * 2;
                        export::Shot::View {
                            camera: Box::new(self.camera()),
                            size: (px(self.view_size.width), px(self.view_size.height)),
                        }
                    }
                };
                let request = export::Request {
                    input: media.input.clone(),
                    flat: media.flat,
                    hw: media.hw,
                    rig: media.rig.clone(),
                    stab: media.stab.clone(),
                    readout_ms: media.readout_ms,
                    t: c.playback.shown_pts,
                    rolling_shutter: self.rolling_shutter,
                    shot,
                    path,
                };
                self.save_status = Some("Saving photo…".to_owned());
                return Task::perform(blocking(move || export::save(request).map_err(|e| format!("{e:#}"))), Message::Saved);
            }
            Message::Saved(result) => match result {
                Ok(_) => self.save_status = None,
                Err(e) => {
                    self.save_status = None;
                    self.error = Some(format!("Saving failed: {e}"));
                }
            },
            Message::Mute => {
                self.muted = !self.muted;
                audio::set_listening(self.volume());
            }
            Message::Volume(v) => {
                self.volume = v;
                self.muted = false;
                audio::set_listening(self.volume());
            }
            Message::ResetView => self.free_look().reset(),
            Message::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen { window::Mode::Fullscreen } else { window::Mode::Windowed };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
        }
        Task::none()
    }

    fn begin_drag(&mut self) {
        if !self.dragging {
            self.edit.checkpoint();
            self.dragging = true;
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let open_hint = if self.edit.cuts.is_empty() { "Open clips (Ctrl+O)" } else { "Add clips to the edit as cuts (Ctrl+O)" };
        let error = self.error.as_ref().map(|e| text(e.clone()).style(text::danger));
        let Some(clip) = &self.clip else {
            // Music first: the song is chosen and sets the length to fill.
            let song = self.edit.music.clips.first().map(|c| {
                let name = c.input.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                format!("Music: {name} ({}). Add clips to fill it.", clock(self.edit.music.end()))
            });
            let status: Element<'_, Message> = match (&self.loading, song) {
                (Some(_), _) => self.loading_bar(280.0),
                (None, Some(song)) => text(song).into(),
                (None, None) => text("Open DJI Osmo 360 .OSV clips, other videos or pictures, or a saved project, to start. Or start from a song.").into(),
            };
            let idle = self.loading.is_none();
            let buttons = row![
                labelled_button(ADD_CLIP, "Open clips…", idle.then_some(Message::Open)),
                labelled_button(OPEN_PROJECT, "Open project…", idle.then_some(Message::OpenProject)),
                labelled_button(ADD_MUSIC, "Add music…", idle.then_some(Message::AddSound(SoundTrack::Music))),
            ]
            .spacing(12);
            let start = center(column![self.logo(), status, buttons].push(error).spacing(16).align_x(Alignment::Center));
            return column![start, credit()].into();
        };
        let p = &clip.playback;
        let media = &p.media;

        let quality = p.quality();
        let showing = if quality.starts_with("loading") {
            status_icon(SHOWING_LOADING, format!("Showing the playback picture, {quality}"))
        } else if quality == "proxy" {
            status_icon(SHOWING_PROXY, "Showing the proxy (LRF): quick to decode, less detail".to_owned())
        } else {
            status_icon(SHOWING_FULL, format!("Showing the {}", if quality == "full res" { "OSV at full resolution".to_owned() } else { quality.clone() }))
        };
        let decode = match media.hw {
            Hwaccel::Software => status_icon(DECODE_CPU, "Decoding in software: no GPU decoder worked here".to_owned()),
            hw => status_icon(DECODE_GPU, format!("Decoding on the GPU with {}", match hw {
                Hwaccel::Vulkan => "Vulkan Video",
                Hwaccel::Vaapi => "VA-API",
                Hwaccel::VideoToolbox => "VideoToolbox",
                Hwaccel::D3d11va => "Direct3D 11",
                _ => "CUDA",
            })),
        };
        // One line: wrapped, a long one stood the whole top bar on end.
        let message = self.save_status.as_ref().map(|s| text(s.clone()).style(text::secondary).wrapping(text::Wrapping::None));
        let status: Element<'_, Message> = match &self.loading {
            Some(_) => self.loading_bar(140.0),
            // What decodes and shows matters for 360 footage: shown while framing.
            None => row![].push(message).push(self.framing.then_some(showing)).push(self.framing.then_some(decode)).spacing(10).align_y(Alignment::Center).into(),
        };
        let shown_view = self.shown_view();
        let (lo, hi) = shown_view.fov_range();
        let mut sources = Vec::new();
        if media.proxy.is_some() {
            sources.push(Source::Proxy);
        }
        sources.push(Source::Osv(1920.min(media.osv_full)));
        sources.push(Source::Osv(media.osv_full));
        sources.dedup();
        let source = match (media.osv_size, &media.proxy) {
            (None, Some(_)) => Source::Proxy,
            (size, _) => Source::Osv(size.unwrap_or(1920)),
        };
        // Groups: settings | the project file | what to add | the export;
        // then, while framing a 360 cut, how it is steadied and drawn.
        let top = row![
            tip(
                icon_button(SETTINGS, Some(Message::SettingsPanel)),
                text(if self.settings_open { "Close the Settings panel (Ctrl+,)" } else { "Settings (Ctrl+,)" }),
                tooltip::Position::Bottom,
            ),
            separator(),
            tip(icon_button(NEW_PROJECT, Some(Message::NewProject)), text("Start a new, empty project (Ctrl+N)"), tooltip::Position::Bottom),
            tip(icon_button(OPEN_PROJECT, Some(Message::OpenProject)), text("Open a saved edit (Ctrl+Shift+O)"), tooltip::Position::Bottom),
            tip(
                icon_button(SAVE, self.unsaved().then_some(Message::SaveProject(false))),
                text("Save the edit: cuts, keyframes and shape (Ctrl+S)"),
                tooltip::Position::Bottom,
            ),
            tip(
                icon_button(SAVE_AS, (!self.edit.cuts.is_empty()).then_some(Message::SaveProject(true))),
                text("Save the edit as a new project file (Ctrl+Shift+S)"),
                tooltip::Position::Bottom,
            ),
            separator(),
            tip(icon_button(ADD_CLIP, Some(Message::Open)), text(open_hint), tooltip::Position::Bottom),
            tip(
                icon_button(ADD_MUSIC, Some(Message::AddSound(SoundTrack::Music))),
                text(if self.edit.music.clips.is_empty() { "Add music under the edit" } else { "Add more music, after the last song" }),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(ADD_SFX, Some(Message::AddSound(SoundTrack::Sfx))), text("Add a sound effect at the playhead, on the SFX lane"), tooltip::Position::Bottom),
            tip(icon_button(ADD_OVERLAY, Some(Message::AddOverlay)), text("Add pictures over the picture, at the playhead (PNG transparency shows through)"), tooltip::Position::Bottom),
            tip(icon_button(ADD_TEXT, Some(Message::AddText)), text("Add text over the picture, at the playhead (typed in the Text panel at the left)"), tooltip::Position::Bottom),
            separator(),
            self.export_controls(),
        ]
        // How a 360 clip is steadied and drawn: only while framing one; an
        // ordinary video or a picture has neither.
        .push((self.framing && media.flat.is_none()).then(|| labelled("Stabilisation", pick_list(&Mode::ALL[..], Some(media.stab.mode()), Message::Stab))))
        .push((self.framing && media.flat.is_none()).then(|| labelled("Projection", pick_list(&Projection::ALL[..], Some(shown_view.projection), Message::Projection))))
        .push((self.framing && media.flat.is_none()).then(|| {
            row![
                tip(labelled("Source", pick_list(sources, Some(source), Message::Source)), text("Playback source: the proxy is quick, the OSV sharper"), tooltip::Position::Bottom),
                tip(
                    toggler(self.full_on_pause).label("Full res").on_toggle(Message::FullOnPause),
                    text("Full res when paused: a paused frame is decoded from the OSV at full size; off, it stays at the playback source's quality"),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        }))
        .push(row![
            space().width(Length::Fill),
            // Beside what's playing: the transport row is full at the
            // window's default width, and cut it off.
            tip(
                row![
                    canvas(self.meter.view()).width(meter::WIDTH).height(meter::HEIGHT),
                    // The loudest since play started, red past 0 dBFS.
                    text(self.meter.loudest.map_or("–".to_string(), |l| if l > -48.0 { format!("{l:+.1}") } else { "–".to_string() }))
                        .size(12)
                        .width(34)
                        .color_maybe(self.meter.loudest.filter(|&l| l >= 0.0).map(|_| iced::Color::from_rgb8(0xe5, 0x3e, 0x3e))),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                text("The mix as it plays, left and right: peak (faint) and RMS, red past 0 dBFS; the number is the loudest peak since play started, dBFS. It sums the clip sound and music at their levels, as the export mixes them, whatever the speakers are set to."),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(TEXT_SMALLER, (self.ui_scale > *settings::UI_SCALE.start() + 1e-3).then_some(Message::UiScale(-1))), text("Make the interface smaller"), tooltip::Position::Bottom),
            tip(
                button(text(format!("{:.0}%", self.ui_scale * 100.0)).size(12)).style(button::text).padding([2, 2]).on_press(Message::UiScale(0)),
                text(format!("Interface size; click for {:.0}%", settings::UI_DEFAULT * 100.0)),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(TEXT_BIGGER, (self.ui_scale < *settings::UI_SCALE.end() - 1e-3).then_some(Message::UiScale(1))), text("Make the interface bigger"), tooltip::Position::Bottom),
            tip(
                icon_button(if self.fullscreen { FULLSCREEN_EXIT } else { FULLSCREEN }, Some(Message::Fullscreen)),
                text(if self.fullscreen { "Leave fullscreen (F)" } else { "Fullscreen (F)" }),
                tooltip::Position::Bottom,
            ),
            status
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill))
        .spacing(12)
        .padding([6, 10])
        .align_y(Alignment::Center);

        let view: Element<'_, Message> = match self.held.clone().or_else(|| self.sphere()) {
            Some(sphere) => {
                let (aspect, picking) = (self.aspect, self.picking);
                let handles = self.overlay_handles();
                // A locked aspect is the largest box of that shape that fits.
                responsive(move |room| {
                    let (w, h) = match aspect.ratio() {
                        None => (room.width, room.height),
                        Some(r) => if room.width / room.height > r { (room.height * r, room.height) } else { (room.width, room.width / r) },
                    };
                    let sphere = shader(sphere.clone()).width(w).height(h);
                    // The box's layer is there from the start of picking: adding it
                    // mid-drag would rebuild the view's state and lose the drag.
                    // The handles' layer is always there for the same reason.
                    match picking {
                        Some(area) => center(stack![sphere, canvas(PickBox(area)).width(w).height(h)]).into(),
                        None => center(stack![sphere, canvas(handles.clone()).width(w).height(h)]).into(),
                    }
                })
                .into()
            }
            None => center(column![self.logo(), text("Decoding…")].spacing(16).align_x(Alignment::Center)).into(),
        };
        let view = container(view).width(Length::Fill).height(Length::Fill).style(|_| container::background(iced::Color::BLACK));
        let view: Element<'_, Message> = match self.side {
            Some(Side::Effects) => row![view, self.effects_panel()].into(),
            Some(Side::Export) => row![view, self.export_panel()].into(),
            Some(Side::Audio) => row![view, self.audio_panel()].into(),
            None => view.into(),
        };
        // A text item's words and look, at the left while it's selected.
        // The Info and Settings panels win the left side over the Text panel
        // while open.
        let view: Element<'_, Message> = match self.info_panel().or_else(|| self.settings_panel()).or_else(|| self.text_panel()) {
            Some(panel) => row![panel, view].into(),
            None => view,
        };
        // The bin, a panel of its own at the far left, beside whichever.
        let view: Element<'_, Message> = row![self.bin_panel(), view].into();

        let current = self.current_key();
        let aiming = current.map(|k| Aiming::of(&self.edit.cuts[self.current].keyframes[k].follow));
        let tolerance = self.tolerance();
        let offsets = crate::project::offsets(&self.edit.cuts);
        let blocks = self
            .edit
            .cuts
            .iter()
            .zip(offsets)
            .map(|(c, from)| {
                let keys = c.keyframes.iter().enumerate().filter(|(_, k)| k.t >= c.start - tolerance && k.t <= c.end + tolerance).map(|(i, k)| (i, k.t, k.ease, k.follow.is_some())).collect();
                // Only those in the cut, and one either side for its ends.
                let thumbs = self.thumbs.get(&c.input).map_or_else(Vec::new, |all| {
                    let from = all.partition_point(|(t, _)| *t < c.start).saturating_sub(1);
                    let to = (all.partition_point(|(t, _)| *t <= c.end) + 1).min(all.len());
                    all[from..to].to_vec()
                });
                let wave = self.waveforms.get(&cut_wave_file(c)).cloned();
                let level = cut_level_tag(c);
                timeline::Block { labels: cut_labels(c, self.opts.stabilize), from, len: c.end - c.start, start: c.start, keys, sound: c.volume.clone(), wave, level, muted: c.mute, thumbs, crossfade: c.crossfade, effects: c.effects.iter().map(|k| k.t).collect() }
            })
            .collect();
        let (timeline_start, timeline_span) = self.timeline_window();
        let sounds = SoundTrack::ALL
            .map(|which| {
                let track = self.edit.track(which);
                let blocks = track
                    .clips
                    .iter()
                    .map(|c| timeline::MusicBlock {
                        label: c.input.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
                        from: c.at,
                        start: c.start,
                        len: c.end - c.start,
                        wave: self.waveforms.get(&c.input).cloned(),
                    })
                    .collect();
                timeline::SoundLane { which, blocks, volume: track.keyframes.clone() }
            })
            .into_iter()
            .collect();
        let timeline = timeline::Timeline {
            blocks,
            sounds,
            selected_music: self.selected_music,
            selected_volume: self.selected_volume,
            selected_sound: self.selected_sound,
            video: self.edit.duration(),
            duration: self.edit.frame(),
            start: timeline_start,
            span: timeline_span,
            position: self.position(),
            selected: self.selected,
            current: current.map(|k| (self.current, k)),
            overlays: self.overlay_blocks(crate::overlay::Track::Overlay),
            texts: self.overlay_blocks(crate::overlay::Track::Text),
            selected_overlay: self.selected_overlay,
            rows: timeline::Rows::new(self.split, self.framing),
        }
        .view();
        // With a song selected, C splits that instead.
        let music = self.selected_music.and_then(|(which, i)| self.edit.track(which).clips.get(i));
        let point = self.selected_volume.is_some() || self.selected_sound.is_some();
        // Any cut but the first can crossfade in from the one before.
        let crossfade_cut = Some(self.selected.unwrap_or(self.current)).filter(|&i| i > 0).and_then(|i| self.edit.cuts.get(i));
        let can_split = match music {
            Some(c) => self.position() > c.at + edit::MIN_CUT && self.position() < c.ends() - edit::MIN_CUT,
            None => self.edit.cuts.get(self.current).is_some_and(|c| p.target() > c.start + edit::MIN_CUT && p.target() < c.end - edit::MIN_CUT),
        };
        // Groups: play | cutting | undo | effects | the framing switch, and
        // while it's on the view's keyframes and angles (for a 360 cut also
        // following and colour tracking); then the timeline's zoom, the mix.
        let flat = self.on_flat();
        let framing_tools = self.framing.then(|| {
            row![
                tip(
                    icon_button(KEY_ADD, Some(Message::KeyAdd)),
                    text(if current.is_some() { "Update this keyframe with the view (K)" } else { "Add a keyframe with this view (K)" }),
                    tooltip::Position::Top,
                ),
                tip(icon_button(KEY_DELETE, current.map(|_| Message::KeyDelete)), text("Delete this keyframe (Del)"), tooltip::Position::Top),
                tip(
                    pick_list(&Ease::ALL[..], current.map(|k| self.edit.cuts[self.current].keyframes[k].ease), Message::KeyEase).placeholder("Ease").width(96),
                    text("How this keyframe moves on to the next (J cycles)"),
                    // Below: the list opens upwards and the hint would cover it.
                    tooltip::Position::Bottom,
                ),
                tip(labelled("FOV", slider(lo..=hi, shown_view.fov, Message::Fov).width(110)), text("Field of view: how wide the view is (scroll on the view)"), tooltip::Position::Top),
                tip(icon_button(RESET_VIEW, Some(Message::ResetView)), text("Reset the view: straight ahead, level (0)"), tooltip::Position::Top),
            ]
            .push((!flat).then(|| {
                tip(
                    pick_list(&Aiming::ALL[..], aiming, Message::KeyFollow).placeholder("Follow").width(140),
                    text("What this keyframe looks at: fixed angles, the selfie stick (whoever holds it), or a colour track (pick a new one by drawing a box over the subject)"),
                    tooltip::Position::Bottom,
                )
            }))
            .push((!flat).then(|| {
                tip(
                    icon_button(PICK, (self.clip.is_some() && self.tracking.is_none()).then_some(Message::PickColour)),
                    text("Track a colour: draw a box over a subject (a jacket, a tent); the keyframe here then follows it"),
                    tooltip::Position::Top,
                )
            }))
            .push((!flat).then(|| tip(icon_button(SAVE_SPHERE, (!p.playing).then_some(Message::Save(Saving::Sphere))), text("Save the whole sphere as an equirect PNG (pause first)"), tooltip::Position::Top)))
            .spacing(12)
            .align_y(Alignment::Center)
        });
        let transport = row![
            tip(
                icon_button(if p.playing { PAUSE } else { PLAY }, Some(Message::Toggle)),
                text(if p.playing { "Pause (Space)" } else { "Play (Space)" }),
                tooltip::Position::Top,
            ),
            // A fixed width: the digits' widths differ in most fonts (macOS's
            // system font especially), and the row after it jittered as it ran.
            text(format!("{} / {}", clock(self.position()), clock(self.edit.duration()))).width(170).center(),
            tip(
                icon_button(SAVE_VIEW, (!p.playing).then_some(Message::Save(Saving::View))),
                text("Save this frame as a PNG (pause first)"),
                tooltip::Position::Top,
            ),
            separator(),
            tip(icon_button(SPLIT, can_split.then_some(Message::Split)), text(if music.is_some() { "Split the selected sound at the playhead (C)" } else { "Split the cut at the playhead (C)" }), tooltip::Position::Top),
            tip(
                icon_button(DELETE_CUT, (self.selected.is_some() || music.is_some() || point || self.selected_overlay.is_some()).then_some(Message::DeleteCut)),
                text(if point {
                    "Delete the selected volume point (Del)"
                } else if music.is_some() {
                    "Delete the selected sound (Del)"
                } else if self.framing {
                    "Delete the selected cut (Shift+Del)"
                } else {
                    "Delete the selected cut (Del)"
                }),
                tooltip::Position::Top,
            ),
            tip(
                icon_button(CROSSFADE, crossfade_cut.map(|_| Message::ToggleCrossfade)),
                text(match crossfade_cut {
                    Some(c) if c.crossfade > 0.0 => "Take away this cut's crossfade (Shift-drag its start edge to change its length)",
                    _ => "Crossfade into this cut from the one before, 1 s (Shift-drag its start edge to change its length)",
                }),
                tooltip::Position::Top,
            ),
            separator(),
            tip(icon_button(UNDO, Some(Message::Undo)), text("Undo (Ctrl+Z)"), tooltip::Position::Top),
            tip(icon_button(REDO, Some(Message::Redo)), text("Redo (Ctrl+Shift+Z)"), tooltip::Position::Top),
            separator(),
            tip(
                icon_button(EFFECTS, Some(Message::Effect(effects::EffectMsg::Toggle))),
                text(if self.side == Some(Side::Effects) { "Close the Effects panel (G)" } else { "Effects: exposure, colour and tone of the cut, keyframed; fades for the edit (G)" }),
                tooltip::Position::Top,
            ),
            separator(),
            tip(
                toggler(self.framing).label("Framing").on_toggle(Message::Framing),
                text("Framing: the view's keyframes, field of view and angles; for a 360 clip also stabilisation, projection and tracking. Off: editing only, and the view can't be turned by accident."),
                tooltip::Position::Top,
            ),
        ]
        .push(framing_tools)
        .push(space().width(Length::Fill))
        .push(tip(icon_button(ZOOM_OUT, Some(Message::TimelineZoom(1.0 / 1.5, None))), text("Zoom the timeline out (Ctrl+−, or scroll on it)"), tooltip::Position::Top))
        .push(tip(
            slider(0.0..=1.0, self.timeline_zoom.ln() / self.timeline_most_zoom().ln().max(1e-9), Message::TimelineZoomTo).step(0.001).width(90),
            text("Timeline zoom"),
            tooltip::Position::Top,
        ))
        .push(tip(icon_button(ZOOM_IN, Some(Message::TimelineZoom(1.5, None))), text("Zoom the timeline in (Ctrl+=, or scroll on it)"), tooltip::Position::Top))
        .push(tip(icon_button(ZOOM_FIT, Some(Message::TimelineZoomTo(0.0))), text("Show the whole edit (Ctrl+0)"), tooltip::Position::Top))
        .push(separator())
        .push(tip(
            icon_button(if self.muted { SPEAKER_MUTED } else { SPEAKER }, Some(Message::AudioPanel)),
            text(if self.side == Some(Side::Audio) { "Close the Audio panel" } else { "Audio: listening volume and the mix levels (M mutes)" }),
            tooltip::Position::Top,
        ))
        .spacing(12)
        .padding([4, 10])
        .align_y(Alignment::Center);

        let timeline = container(timeline).padding([2, 10]);
        let divider = tip(
            canvas(divider::Divider).width(Length::Fill).height(divider::HEIGHT),
            text("Drag to give the timeline or the picture more room (double-click: as it was)"),
            tooltip::Position::Top,
        );
        let window: Element<'_, Message> = column![top, view, divider, transport].push(self.angles_strip().filter(|_| self.framing)).push(timeline).push(error.map(|e| container(e).padding([0, 10]))).push(credit()).into();
        self.normalize_dialog(self.sound_menu_layer(window))
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
