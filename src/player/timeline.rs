//! The timeline under the view, top to bottom: the markers' flags, the
//! time ruler (click or drag it to seek), the text and overlay lanes, the
//! cuts, the Clip sound lane, the cuts' keyframe rows, a gap naming the
//! sound group, and the sound lanes.
//!
//! The cuts are blocks showing thumbnails of their footage: click one to
//! select it, drag it to move it, drag its edges to trim it. Under them is
//! the Clip sound lane: each cut's waveform (what it plays: its cleaned
//! voice or its clip's sound) with its volume line and its level tag; a
//! right-click opens the cut's sound menu. Then the cuts' keyframe rows:
//! effects and, while framing, View, each keyframe marked by its ease;
//! click one to jump to it, drag one to retime it.
//!
//! The sound lanes, one per `SoundTrack` (Music, SFX, SFX 2, Voice): their
//! clips with waveforms, moved and trimmed like cuts, crossfading with
//! Shift, a volume line each, and a line where the video ends (sound beyond
//! it fades out in the export). Every lane has its name pinned at the left.
//!
//! The text and overlay lanes are canvases of their own (`OverlayLane`):
//! their items as blocks, selected, moved and trimmed like sound clips.
//! Once a lane has items, two rows of their keyframes sit under it:
//! effects, then motion.
//!
//! It shows a window of the edit: the wheel zooms around the pointer, Shift
//! or a sideways wheel pans, and when zoomed in a scrollbar at the bottom
//! shows and moves the window.

use super::{Message, thumbs};
use crate::model::ops::End;
use crate::model::selection::Picked;
use crate::project::{Own, SoundTrack};
use crate::overlay::Track;
use crate::view::Ease;
use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::image::Handle;
use iced::{Point, Rectangle, Renderer, Size, Theme};
use std::sync::Arc;

mod auto;

/// Diamond half-size, and how close a click must land to pick a diamond or
/// a cut's edge.
const DIAMOND: f32 = 6.0;
const PICK: f32 = 7.0;
/// The header column at the left of every lane (gap 54): each lane's name,
/// solo and mute, fold, and its own level, fixed while the lanes scroll;
/// time starts after it. Wide enough for «Clip sound» with S and M.
pub const HEAD: f32 = 128.0;
/// How far inside a clip's end a press takes its edge (it was `PICK`, 7);
/// a cut's edge also reaches as far outside, so an edge is a 24 px target
/// (WCAG 2.5.8's least), however thin it is drawn (`TIMELINE-UX.md`).
const EDGE: f32 = 12.0;
/// The cut row's height at scale 1, and the gap between cut blocks.
const CUTS: f32 = 40.0;
/// The clips' own sound under them at scale 1: each cut's waveform and its
/// volume line.
const CLIP_SOUND: f32 = 30.0;
const GAP: f32 = 2.0;
/// Under a track, a row of its effect keyframes and one of its view (a
/// cut's) or motion (an overlay's) keyframes.
const EFFECTS: f32 = 16.0;
/// The transitions lane's height (gap 25f), open and folded.
const TRANS: f32 = 24.0;
const TRANS_FOLDED: f32 = 16.0;
/// An automation lane's height (gap 39).
const AUTO: f32 = 30.0;
const KEYS: f32 = 20.0;
/// The gap between the picture's lanes and the sound's, with the sound
/// group's name in it: the eye finds where one group ends (gap 50d).
const SEEK: f32 = 16.0;
/// The music lane at scale 1.
const MUSIC: f32 = 40.0;
/// The scrollbar's row at the bottom, and the bar's thickness.
const SCROLL: f32 = 8.0;
const SCROLL_BAR: f32 = 5.0;
/// How far the lanes may shrink or grow, as a share of their own height.
pub const SCALES: std::ops::RangeInclusive<f32> = 0.75..=3.0;

/// Where the rows sit, top to bottom: the cuts, the clip sound lane, the
/// keyframe rows, the gap before the sound, a lane per sound track (Music,
/// SFX, SFX 2, Voice) and the scrollbar. Dragging the divider above the
/// transport row scales the lanes that hold pictures and waveforms (cuts,
/// sound, overlays); the rows of marks and the gap keep their height, as
/// their marks do.
#[derive(Clone, Copy, Debug)]
pub struct Rows {
    cuts: f32,
    /// The clip sound lane's height; it starts where the cuts end.
    clip_sound: f32,
    /// The transitions lane: where it starts (under the clip sound) and how
    /// high it is.
    trans_top: f32,
    trans: f32,
    effects_top: f32,
    /// Where the automation lanes under the Effects row start (gap 39).
    auto_top: f32,
    /// The View key row's height: 0 while editing.
    keys: f32,
    keys_top: f32,
    /// Where the gap before the sound lanes starts (it was the seek bar).
    seek_top: f32,
    /// A sound lane's height, and where the first starts; a folded lane
    /// (SFX 2) is `FOLDED` high.
    music: f32,
    sounds_top: f32,
    /// Whether each sound lane is folded (`SoundTrack::ALL`'s order).
    folded: [bool; SOUND_LANES],
    height: f32,
    overlay: f32,
}

impl Rows {
    /// `framing`: the View key row shows (it holds the view's keyframes);
    /// editing, it's left out and the lanes below move up. `lanes`
    /// automation lanes open under the Effects row unless it's folded.
    pub fn new(scale: f32, framing: bool, folds: Folds, lanes: usize) -> Self {
        let scale = scale.clamp(*SCALES.start(), *SCALES.end());
        let (cuts, music) = ((CUTS * scale).round(), (MUSIC * scale).round());
        let clip_sound = (CLIP_SOUND * scale).round();
        let trans_top = cuts + clip_sound;
        let trans = if folds.transitions { TRANS_FOLDED } else { TRANS };
        let effects_top = trans_top + trans;
        let keys = if framing { KEYS } else { 0.0 };
        let auto_top = effects_top + EFFECTS;
        let auto = if folds.effects { 0.0 } else { lanes as f32 * AUTO };
        let seek_top = auto_top + auto + keys;
        let sounds_top = seek_top + SEEK;
        let folded = SoundTrack::ALL.map(|w| w == SoundTrack::Sfx2 && folds.sfx2);
        let lanes: f32 = folded.iter().map(|&f| if f { FOLDED } else { music }).sum();
        let height = sounds_top + lanes + SCROLL;
        Rows { cuts, clip_sound, trans_top, trans, effects_top, auto_top, keys_top: auto_top + auto, keys, seek_top, music, sounds_top, folded, height, overlay: (OVERLAY * scale).round() }
    }

    /// Where sound lane `i` starts.
    fn sound_top(self, i: usize) -> f32 {
        self.sounds_top + (0..i).map(|j| self.lane_h(j)).sum::<f32>()
    }

    /// Sound lane `i`'s height: thin while folded.
    fn lane_h(self, i: usize) -> f32 {
        if self.folded.get(i).copied().unwrap_or(false) { FOLDED } else { self.music }
    }

    /// The sound lanes' height together.
    fn sounds_h(self) -> f32 {
        (0..SOUND_LANES).map(|i| self.lane_h(i)).sum()
    }

    /// Where sound lane `i`'s volume line runs.
    fn sound_lane(self, i: usize) -> Lane {
        (self.sound_top(i) + 4.0, self.lane_h(i) - 8.0)
    }

    /// The sound lane at height `y`, if any.
    fn sound_at_y(self, y: f32) -> Option<usize> {
        (0..SOUND_LANES).find(|&i| y >= self.sound_top(i) && y < self.sound_top(i) + self.lane_h(i))
    }

    /// Where the cuts' volume lines run: over the clip sound lane.
    fn cut_lane(self) -> Lane {
        (self.cuts + 3.0, self.clip_sound - 6.0)
    }

    /// Whether `y` is on the clip sound lane.
    fn on_clip_sound(self, y: f32) -> bool {
        (self.cuts..self.trans_top).contains(&y)
    }

    /// Whether `y` is on the transitions lane.
    fn on_transitions(self, y: f32) -> bool {
        (self.trans_top..self.effects_top).contains(&y)
    }
}

/// The sound lanes, one per `SoundTrack`.
const SOUND_LANES: usize = SoundTrack::ALL.len();
/// The lanes' heights at scale 1, which a drag of the divider shares out:
/// the cuts, the sound lanes and the two overlay lanes (text and pictures).
pub const SCALABLE: f32 = CUTS + CLIP_SOUND + SOUND_LANES as f32 * MUSIC + 2.0 * OVERLAY;
/// A press on a cut that moves less than this is a click, not a drag.
const DRAG_FROM: f32 = 4.0;
/// An edge dragged this close to a cut boundary, the playhead or another
/// sound's edge snaps to it; Shift lets it go free, so the reach can be
/// generous (user, 2026-11-03: 8 px was fiddly).
const SNAP: f32 = 12.0;
/// The line where a drag snapped: bright, apart from the playhead's colour.
const SNAP_LINE: iced::Color = iced::Color::from_rgb(1.0, 0.85, 0.25);
/// The volume line's range, dB, top to bottom of its lane (the bottom is
/// silence), and a volume point's radius.
const VOLUME_TOP: f64 = 6.0;
const VOLUME_BOTTOM: f64 = crate::project::OFF_DB;
const POINT: f32 = 4.0;
/// Two presses on a level tag this close are a double-click.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);
/// A level dragged this close to 0 dB is 0 dB.
const SNAP_DB: f64 = 1.0;
const EFFECT_MARK: iced::Color = iced::Color::from_rgb(0.78, 0.55, 1.0);
/// Where a volume line runs: its top and height. A sound track's over its
/// lane, a cut's over the clip sound lane under it.
type Lane = (f32, f32);
/// How far down from a sound clip's top its fade handles take the mouse.
const FADE_BAND: f32 = 16.0;
/// How far either side of a fade's knob a press takes it (pack 1: it was
/// `PICK`, 7).
const FADE_REACH: f32 = 10.0;
/// A fade's knob stays this far inside its clip while the fade is shorter:
/// at a join the two clips' knobs used to sit on one spot, one over the
/// other (the user, 2026-11-02).
const FADE_INSET: f32 = 8.0;
/// The sound effects' clips (the second lane's a shade deeper); the
/// music's are the theme's green. From the colour-blind-safe Okabe-Ito
/// set, and every lane is named as well.
const SFX_COLOUR: iced::Color = iced::Color::from_rgb(0.80, 0.47, 0.65);
const SFX2_COLOUR: iced::Color = iced::Color::from_rgb(0.66, 0.36, 0.56);
const VOICE_COLOUR: iced::Color = iced::Color::from_rgb(0.34, 0.71, 0.91);
/// Waveform bars a gain pushes past full scale: the export's limiter
/// holds them, but loud enough to look at.
const OVER_COLOUR: iced::Color = iced::Color::from_rgb(1.0, 0.55, 0.2);
const SOUND_LINE: iced::Color = iced::Color::from_rgb(0.35, 0.85, 1.0);

/// An overlay lane's height at scale 1, above the cuts.
const OVERLAY: f32 = 30.0;
const OVERLAY_COLOUR: iced::Color = iced::Color::from_rgb(0.95, 0.55, 0.3);
const TEXT_COLOUR: iced::Color = iced::Color::from_rgb(0.3, 0.8, 0.75);

/// An overlay item as its lane shows it: the item itself, its name and
/// its small picture.
pub struct OverlayBlock {
    pub item: crate::overlay::Item,
    pub label: String,
    pub thumb: Option<Thumb>,
}

impl OverlayBlock {
    fn len(&self) -> f64 {
        self.item.end - self.item.start
    }
}

/// A text's or picture's small picture for its block, and its width to
/// its height, so it's drawn in its own shape.
#[derive(Clone)]
pub struct Thumb {
    pub image: Handle,
    pub aspect: f32,
}

/// One cut as the timeline shows it: the cut itself (its in point, sound,
/// fades, markers, effects, all as the project has them), where it starts
/// on the edit, and what the player has read or worked out for it.
pub struct Block {
    pub cut: crate::project::Cut,
    /// Its names, longest first; the first that fits is shown.
    pub labels: Vec<String>,
    /// Where it starts on the timeline.
    pub from: f64,
    /// Its clip's length (infinite if not known): its handles say whether
    /// there is footage beyond each edge.
    pub most: f64,
    /// Its keyframes within the cut: index into the cut's keyframes, clip
    /// time, how it moves on to the next (drawn as its shape), and whether
    /// it follows the stick.
    pub keys: Vec<(usize, f64, Ease, bool)>,
    /// Thumbnails of its clip by clip time, in order (see `thumbs`).
    pub thumbs: Vec<(f64, Handle)>,
    /// The waveform of the sound it plays (its cleaned voice, else its
    /// clip's), by clip time, once read; none for a silent clip.
    pub wave: Option<Arc<crate::waveform::Waveform>>,
    /// The same through the leveler, if the cut is evened out (and read).
    pub even_wave: Option<Arc<crate::waveform::Waveform>>,
    /// Its own sound level as a short tag ("0 dB", "−12 · even", "muted"),
    /// the knob a drag turns.
    pub level: String,
}

impl Block {
    /// How long it lasts on the edit.
    fn len(&self) -> f64 {
        self.cut.end - self.cut.start
    }

    /// The thumbnail nearest clip time `t`.
    fn thumb_at(&self, t: f64) -> Option<&Handle> {
        let i = self.thumbs.partition_point(|(at, _)| *at < t);
        let before = i.checked_sub(1).map(|j| &self.thumbs[j]);
        match (before, self.thumbs.get(i)) {
            (Some(a), Some(b)) => Some(if t - a.0 < b.0 - t { &a.1 } else { &b.1 }),
            (a, b) => a.or(b).map(|x| &x.1),
        }
    }

    /// Where its sound runs, which a J- or L-cut moves off its picture's
    /// edges: (edit time, clip time, length). Its sound's waveform, fades
    /// and volume line all live there.
    fn sound_span(&self) -> (f64, f64, f64) {
        let c = &self.cut;
        (self.from - c.sound_before, c.start - c.sound_before, self.len() + c.sound_before + c.sound_after)
    }
}

/// A sound track as its lane shows it: its clips and its volume points
/// (edit time, dB on top of the track's level).
pub struct SoundLane {
    pub which: SoundTrack,
    /// The track itself: its level, its line, its clips as they sound
    /// (`mixdown::clip_gain`).
    pub track: crate::project::Music,
    pub blocks: Vec<MusicBlock>,
}

/// A clip on a sound lane (a song, a sound effect) as it shows: the clip
/// itself and what the player has read for it.
pub struct MusicBlock {
    pub clip: crate::project::MusicClip,
    pub label: String,
    /// The file's length (infinite until its waveform is read).
    pub most: f64,
    /// The song's waveform, once read.
    pub wave: Option<Arc<crate::waveform::Waveform>>,
    /// Its waveform through the leveler, if it's evened out (and read).
    pub even_wave: Option<Arc<crate::waveform::Waveform>>,
    /// Its own level as a tag, the knob a drag turns (`Block::level`).
    pub level: String,
}

impl MusicBlock {
    fn len(&self) -> f64 {
        self.clip.end - self.clip.start
    }
}

pub struct Timeline {
    pub blocks: Vec<Block>,
    /// The sound lanes, in `SoundTrack::ALL`'s order.
    pub sounds: Vec<SoundLane>,
    /// What's selected, by place (one thing at a time).
    pub picked: Picked,
    /// Where the video ends; `duration` runs on to the music's end if later.
    pub video: f64,
    /// The clips' sound level in the mix (the Audio panel's «Clip sound»).
    pub clips_db: f64,
    pub duration: f64,
    /// The stretch of edit time shown: from `start`, `span` seconds.
    pub start: f64,
    pub span: f64,
    /// Playhead, in edit time.
    pub position: f64,
    /// The keyframe at the playhead (cut, keyframe), drawn highlighted.
    pub current: Option<(usize, usize)>,
    /// The overlay track's lane and the text track's.
    pub overlays: Vec<OverlayBlock>,
    pub texts: Vec<OverlayBlock>,
    /// Moves on when Esc cancels a drag, so the canvases drop theirs.
    pub drag_epoch: u64,
    /// The playhead's last key step, said by it for a moment (gap 55).
    pub step_note: Option<&'static str>,
    /// The rows' heights, from the divider's scale.
    pub rows: Rows,
    /// Which sound lanes are soloed or muted, for listening.
    pub listen: super::listen::Listen,
    /// The mix check's findings (from, to, one to fix), on the ruler.
    pub check_marks: Vec<(f64, f64, bool)>,
    /// The time ruler's height (`RULER_LARGE` or `RULER_COMPACT`).
    pub ruler: f32,
    /// Which foldable lanes are folded to a thin row.
    pub folds: Folds,
    /// Whether drags snap: N turns it off, Shift held lets one go free.
    pub snapping: bool,
    /// Where the drag under way has snapped, edit time: a line there
    /// through every lane.
    pub snap_line: Option<f64>,
    /// The automation lanes under the Effects row, by `grade::NAMES` index
    /// (gap 39).
    pub auto_lanes: Vec<super::automation_ui::Param>,
    /// The overlay and text lanes' own automation lanes.
    pub overlay_auto: Vec<super::automation_ui::Param>,
    pub text_auto: Vec<super::automation_ui::Param>,
    /// The keys held (Shift, Ctrl), for canvases that don't follow them.
    pub held: iced::keyboard::Modifiers,
}

/// What a snap target belongs to, so a drag never snaps to its own edges
/// (or to what moves with it).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Of {
    Edit,
    Playhead,
    /// A cut's picture edge, its sound's edge, its markers.
    Cut(usize),
    CutSound(usize),
    Mark(usize),
    Sound(usize, usize),
    Item(Track, usize),
}

/// The line where a drag has snapped, top to bottom of a canvas `h` high.
fn draw_snap_line(frame: &mut Frame, t: &Timeline, b: Rectangle, h: f32) {
    let Some(at) = t.snap_line else { return };
    let x = t.x(at, b);
    frame.fill(&Path::rectangle(Point::new(x - 1.0, 0.0), Size::new(2.0, h)), SNAP_LINE);
}

/// A message from a drag that may have snapped, with where (`Message::Snapped`).
fn snapped(at: Option<(f64, f64)>, message: impl FnOnce(f64) -> Message) -> Message {
    Message::Snapped(at.map(|(_, to)| to), Box::new(message(at.map_or(0.0, |(by, _)| by))))
}

#[derive(Default)]
pub enum Drag {
    #[default]
    None,
    Playhead,
    Key { cut: usize, key: usize },
    /// Trimming a cut's in (`start`) or out point. The scale is kept from
    /// the press: the timeline rescales as the edit's length changes.
    Trim { cut: usize, start: bool, x0: f32, from: f64, per_px: f64 },
    /// Moving a cut; `to` is where it would go.
    Move { cut: usize, x0: f32, moved: bool, to: usize },
    /// Dragging the scrollbar, which was at `start` when pressed at `x0`.
    Scroll { x0: f32, start: f64 },
    /// Moving clip `clip` of sound lane `lane`, which started at `at` when
    /// pressed at `x0`.
    MusicMove { lane: usize, clip: usize, x0: f32, at: f64, per_px: f64 },
    /// Trimming its in (`start`) or out edge, at file time `from` when
    /// pressed.
    MusicTrim { lane: usize, clip: usize, start: bool, x0: f32, from: f64, per_px: f64 },
    /// Shift-dragging cut `cut`'s start edge: its crossfade, `from` seconds
    /// when pressed at `x0`; dragging left lengthens it.
    Crossfade { cut: usize, x0: f32, from: f64, per_px: f64 },
    /// Dragging a fade handle, on a sound clip or a cut's sound (`own`), at
    /// its start (`in_edge`) or end; the fade was `from` seconds when
    /// pressed at `x0`.
    Fade { own: Own, in_edge: bool, x0: f32, from: f64, per_px: f64 },
    /// A point on a cut's or a sound clip's own volume line.
    Point { own: Own, key: usize },
    /// A level tag dragged up or down from `y0`, the level `from` then.
    Level { own: Own, y0: f32, from: f64 },
    /// Dragging cut `cut`'s effect keyframe `key` to another time.
    Effect { cut: usize, key: usize },
    /// Dragging cut `cut`'s sound's start (`start`) or end on the clip
    /// sound lane, at clip time `from` when pressed: a J- or L-cut.
    SoundEdge { cut: usize, start: bool, x0: f32, from: f64, per_px: f64 },
    /// Drawing a transition on its lane from `x0` to `x1` (gap 25f).
    Zone { x0: f32, x1: f32 },
    /// Dragging the inner edge of cut `cut`'s edge look on `end`: `from`
    /// seconds when pressed at `x0`.
    EdgeLength { cut: usize, end: End, x0: f32, from: f64, per_px: f64 },
    /// On an automation lane (gap 39).
    Auto(auto::LaneDrag),
}

#[derive(Default)]
pub struct State {
    drag: Drag,
    /// The sound lanes and the clip sound lane, drawn once and kept until
    /// what they show changes (`sound_key`): their waveforms took 40 ms a
    /// frame in a window and 79 ms fullscreen, on every frame while playing
    /// or dragging the playhead (`GPU-PLAYER.md`).
    sounds: canvas::Cache,
    sounds_key: std::cell::Cell<u64>,
    /// `Timeline::drag_epoch` when the drag began: moved on (Esc), the drag
    /// is dropped.
    epoch: u64,
    /// The last press on a level tag, for a double-click.
    tag_click: Option<(std::time::Instant, Own)>,
    /// Shift held: the wheel pans instead of zooming, and music may
    /// overlap. Ctrl held: a click on the music lane adds a volume point.
    shift: bool,
    ctrl: bool,
}

impl Timeline {
    fn width(b: Rectangle) -> f32 {
        (b.width - HEAD - 2.0 * PICK).max(1.0)
    }

    /// Edit time to x, with room either side for diamonds at the ends.
    /// Times outside the window land outside the widget.
    fn x(&self, t: f64, b: Rectangle) -> f32 {
        HEAD + PICK + ((t - self.start) / self.span.max(1e-9)) as f32 * Self::width(b)
    }

    fn t(&self, x: f32, b: Rectangle) -> f64 {
        (self.start + ((x - HEAD - PICK) / Self::width(b)) as f64 * self.span).clamp(0.0, self.duration)
    }

    fn zoomed(&self) -> bool {
        self.span < self.duration - 1e-6
    }

    /// The scrollbar's thumb, left and right x.
    fn scroll_thumb(&self, b: Rectangle) -> (f32, f32) {
        let w = Self::width(b) as f64 / self.duration.max(1e-9);
        (HEAD + PICK + (self.start * w) as f32, HEAD + PICK + ((self.start + self.span) * w) as f32)
    }

    fn block_at(&self, x: f32, b: Rectangle) -> Option<usize> {
        // Later first: in a crossfade the incoming cut lies over the one before.
        self.blocks.iter().rposition(|c| x >= self.x(c.from, b) && x <= self.x(c.from + c.len(), b))
    }

    /// The nearest diamond within reach of `x`: (cut, keyframe).
    fn key_at(&self, x: f32, b: Rectangle) -> Option<(usize, usize)> {
        let mut best = None;
        for (ci, c) in self.blocks.iter().enumerate() {
            for &(ki, t, _, _) in &c.keys {
                let d = (self.x(c.from + t - c.cut.start, b) - x).abs();
                if d <= PICK && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, (ci, ki)));
                }
            }
        }
        best.map(|(_, k)| k)
    }

    /// Whether `x` is on cut `i`'s in edge (`Some(true)`) or out edge.
    fn edge_at(&self, i: usize, x: f32, b: Rectangle) -> Option<bool> {
        let c = &self.blocks[i];
        edge_of(self.x(c.from, b), self.x(c.from + c.len(), b), x)
    }

    /// The effect keyframe mark under `p` (they sit in a row under the
    /// cuts): (cut, keyframe, clip time).
    fn effect_at(&self, p: Point, b: Rectangle) -> Option<(usize, usize, f64)> {
        if !(self.rows.effects_top..self.rows.auto_top).contains(&p.y) {
            return None;
        }
        // One key is the cut's value throughout (no mark); more animate it.
        self.blocks.iter().enumerate().rev().filter(|(_, c)| c.cut.effects.len() > 1).find_map(|(i, c)| {
            c.cut.effects.iter().map(|k| k.t).enumerate().find(|&(_, t)| t >= c.cut.start && t <= c.cut.start + c.len() && (self.x(c.from + t - c.cut.start, b) - p.x).abs() <= PICK).map(|(k, t)| (i, k, t))
        })
    }

    /// The join whose crossfade's X is under `x`: the cut after it.
    fn join_at(&self, x: f32, b: Rectangle) -> Option<usize> {
        self.blocks.iter().enumerate().skip(1).find(|(_, c)| c.cut.crossfade > 0.0 && x >= self.x(c.from, b) && x <= self.x(c.from + c.cut.crossfade, b)).map(|(i, _)| i)
    }

    /// The cut edge under `x`: (cut, whether it's the in edge). Later cuts
    /// first: a crossfading cut's in edge lies over the cut before it.
    fn edge_hit(&self, x: f32, b: Rectangle) -> Option<(usize, bool)> {
        (0..self.blocks.len()).rev().find_map(|i| {
            let (l, r) = self.block_x(&self.blocks[i], b);
            (x >= l - EDGE && x <= r + EDGE).then(|| self.edge_at(i, x, b)).flatten().map(|e| (i, e))
        })
    }

    /// What a drag has done so far, for a readout by the pointer (pack 1):
    /// how far the edge or clip went and its length or place now, from the
    /// edit as it is, so a trim stopped by the file's end or a neighbour
    /// says where it stopped.
    fn readout(&self, drag: &Drag) -> Option<String> {
        let edge = |start: f64, len: f64, at_in: bool| if at_in { start } else { start + len };
        match *drag {
            Drag::Trim { cut, start, from, .. } => self.blocks.get(cut).map(|c| format!("{} · {}", signed(edge(c.cut.start, c.len(), start) - from), seconds(c.len()))),
            // How far the sound runs past its picture's edge (or short of it).
            Drag::SoundEdge { cut, start, .. } => self.blocks.get(cut).map(|c| {
                let past = if start { c.cut.sound_before } else { c.cut.sound_after };
                if past >= 0.0 { format!("sound {} past the picture", seconds(past)) } else { format!("sound {} short", seconds(-past)) }
            }),
            Drag::MusicTrim { lane, clip, start, from, .. } => self.sounds.get(lane)?.blocks.get(clip).map(|m| format!("{} · {}", signed(edge(m.clip.start, m.len(), start) - from), seconds(m.len()))),
            Drag::MusicMove { lane, clip, at, .. } => self.sounds.get(lane)?.blocks.get(clip).map(|m| format!("{} · at {}", signed(m.clip.at - at), dailix_sheet::clock(m.clip.at, 1))),
            // At 0 it says which way: dragged into its own cut (as a sound's
            // fade in is) it stays 0, which looked like nothing working.
            Drag::Crossfade { cut, .. } => self.blocks.get(cut).map(|c| if c.cut.crossfade > 0.0 { format!("crossfade {}", seconds(c.cut.crossfade)) } else { "← onto the cut before".to_owned() }),
            Drag::Fade { own, in_edge, .. } => {
                let (fade_in, fade_out) = match own {
                    Own::Cut(i) => self.blocks.get(i).map(|c| (c.cut.sound_fade_in, c.cut.sound_fade_out))?,
                    Own::Clip(w, i) => self.sounds.iter().find(|l| l.which == w)?.blocks.get(i).map(|m| (m.clip.fade_in, m.clip.fade_out))?,
                };
                Some(format!("fade {}", seconds(if in_edge { fade_in } else { fade_out })))
            }
            _ => None,
        }
    }

    /// The markers' row's height: none without markers, a strip folded.
    pub fn flags_height(&self) -> f32 {
        if !self.blocks.iter().any(|c| c.cut.marks.iter().any(|m| m.t >= c.cut.start && m.t <= c.cut.start + c.len())) {
            0.0
        } else if self.folds.marks {
            FLAGS_FOLDED
        } else {
            FLAGS_OPEN
        }
    }

    /// The markers in view, in edit order: their cut, clip time, x, note.
    fn flags(&self, b: Rectangle) -> Vec<(usize, f64, f32, &str)> {
        let mut flags: Vec<_> = self
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(i, c)| c.cut.marks.iter().filter(move |m| m.t >= c.cut.start && m.t <= c.cut.start + c.len()).map(move |m| (i, m.t, c.from + m.t - c.cut.start, m.note.as_str())))
            .map(|(i, t, at, note)| (i, t, self.x(at, b), note))
            .filter(|f| f.2 >= 0.0 && f.2 <= b.width)
            .collect();
        flags.sort_by(|a, b| a.2.total_cmp(&b.2));
        flags
    }

    /// The marker under `x` (its cut and clip time), for pressing, the
    /// right button and the pointer alike; one scrolled under the header
    /// column isn't there.
    fn flag_at(&self, b: Rectangle, x: f32) -> Option<(usize, f64)> {
        self.flags(b).into_iter().filter(|f| f.2 >= HEAD).find(|f| (f.2 - x).abs() <= MARK_REACH).map(|(cut, t, ..)| (cut, t))
    }

    /// The selected cut's edge under `x`: only it has handles, so a press
    /// elsewhere on a cut moves or selects it.
    fn selected_edge(&self, x: f32, b: Rectangle) -> Option<(usize, bool)> {
        self.edge_hit(x, b).filter(|&(i, _)| Some(i) == self.picked.cut()).or_else(|| {
            // At a join the later cut's edge comes first: the selected one
            // may be the cut before.
            let i = self.picked.cut().filter(|&i| i < self.blocks.len())?;
            let (l, r) = self.block_x(&self.blocks[i], b);
            (x >= l - EDGE && x <= r + EDGE).then(|| self.edge_at(i, x, b)).flatten().map(|e| (i, e))
        })
    }

    fn music_at(&self, lane: usize, x: f32, b: Rectangle) -> Option<usize> {
        self.sounds[lane].blocks.iter().position(|m| x >= self.x(m.clip.at, b) && x <= self.x(m.clip.at + m.len(), b))
    }

    /// Whether `x` is on clip `i`'s in edge (`Some(true)`) or out edge.
    fn music_edge_at(&self, lane: usize, i: usize, x: f32, b: Rectangle) -> Option<bool> {
        let m = &self.sounds[lane].blocks[i];
        edge_of(self.x(m.clip.at, b), self.x(m.clip.at + m.len(), b), x)
    }

    /// A volume line's dB to y on `lane` (its top and height) and back.
    fn volume_y(lane: Lane, db: f64) -> f32 {
        lane.0 + ((VOLUME_TOP - db.clamp(VOLUME_BOTTOM, VOLUME_TOP)) / (VOLUME_TOP - VOLUME_BOTTOM)) as f32 * lane.1
    }

    /// y on `lane` to dB, snapping to 0 within `SNAP_DB`. In dB, not
    /// pixels: on the 24 px clip sound lane 3 px were ±5.7 dB, and −3 dB
    /// couldn't be set at all (found by the tests, 2026-11-01).
    fn volume_db(lane: Lane, y: f32) -> f64 {
        let db = (VOLUME_TOP - ((y - lane.0) / lane.1) as f64 * (VOLUME_TOP - VOLUME_BOTTOM)).clamp(VOLUME_BOTTOM, VOLUME_TOP);
        if db.abs() < SNAP_DB { 0.0 } else { db }
    }

    /// A fade handle under `p` on sound lane `lane`: (clip, at its start).
    /// The handles sit along the clip's top, where its fades end.
    fn fade_at(&self, lane: usize, p: Point, b: Rectangle) -> Option<(usize, bool)> {
        if self.rows.folded[lane] {
            return None;
        }
        let top = self.rows.sound_top(lane) + 2.0;
        if p.y > top + FADE_BAND {
            return None;
        }
        let per_px = self.span / Self::width(b) as f64;
        self.sounds[lane].blocks.iter().enumerate().find_map(|(i, m)| {
            let (l, r) = (self.x(m.clip.at, b), self.x(m.clip.at + m.len(), b));
            let fi = fade_knob_x((l, r), l + (m.clip.fade_in / per_px) as f32, true);
            let fo = fade_knob_x((l, r), r - (m.clip.fade_out / per_px) as f32, false);
            if (p.x - fi).abs() <= FADE_REACH && p.x >= l - 1.0 && p.x <= r + 1.0 {
                Some((i, true))
            } else if (p.x - fo).abs() <= FADE_REACH && p.x >= l - 1.0 && p.x <= r + 1.0 {
                Some((i, false))
            } else {
                None
            }
        })
    }

    /// The cut sound fade handle under `p` on the clip sound lane: the cut,
    /// and whether it's the fade in. Only along the lane's top, so the
    /// volume line and points below stay reachable.
    fn cut_fade_at(&self, p: Point, b: Rectangle) -> Option<(usize, bool)> {
        if p.y > self.rows.cuts + 1.0 + FADE_BAND {
            return None;
        }
        let per_px = self.span / Self::width(b) as f64;
        self.blocks.iter().enumerate().filter(|(_, c)| c.wave.is_some()).find_map(|(i, c)| {
            let (l, r) = self.sound_x(c, b);
            let fi = fade_knob_x((l, r), l + (c.cut.sound_fade_in / per_px) as f32, true);
            let fo = fade_knob_x((l, r), r - (c.cut.sound_fade_out / per_px) as f32, false);
            let inside = p.x >= l - 1.0 && p.x <= r + 1.0;
            if inside && (p.x - fi).abs() <= FADE_REACH {
                Some((i, true))
            } else if inside && (p.x - fo).abs() <= FADE_REACH {
                Some((i, false))
            } else {
                None
            }
        })
    }

    /// Where `own` is drawn: its lane, where it starts on the edit, its in
    /// point in its file, its length and its points.
    fn own_place(&self, own: Own) -> Option<(Lane, f64, f64, f64, &[crate::project::VolumeKey])> {
        match own {
            Own::Cut(i) => self.blocks.get(i).map(|c| {
                let (from, start, len) = c.sound_span();
                (self.rows.cut_lane(), from, start, len, &c.cut.volume[..])
            }),
            Own::Clip(which, i) => {
                let li = self.sounds.iter().position(|l| l.which == which)?;
                self.sounds[li].blocks.get(i).map(|m| (self.rows.sound_lane(li), m.clip.at, m.clip.start, m.len(), &m.clip.volume[..]))
            }
        }
    }

    /// The own lines under `p`'s row: the cuts' on the clip sound lane, a
    /// sound lane's clips' on it.
    fn owns_at_y(&self, y: f32) -> Vec<Own> {
        if self.rows.on_clip_sound(y) {
            (0..self.blocks.len()).map(Own::Cut).collect()
        } else if let Some(li) = self.rows.sound_at_y(y).filter(|&li| !self.rows.folded[li]) {
            let which = self.sounds[li].which;
            (0..self.sounds[li].blocks.len()).map(|i| Own::Clip(which, i)).collect()
        } else {
            Vec::new()
        }
    }

    /// The own line under `p`, where it's drawn (its block has points): a
    /// click there adds a point, as a rubber band.
    fn on_sound_line(&self, p: Point, b: Rectangle) -> Option<Own> {
        let t = self.t(p.x, b);
        self.owns_at_y(p.y).into_iter().find(|&own| {
            let Some((lane, from, start, len, keys)) = self.own_place(own) else { return false };
            !keys.is_empty() && t >= from && t <= from + len && (Self::volume_y(lane, crate::project::volume_at(keys, start + t - from)) - p.y).abs() <= PICK
        })
    }

    /// An own line's point under `p`, if any.
    fn sound_at(&self, p: Point, b: Rectangle) -> Option<(Own, usize)> {
        self.owns_at_y(p.y).into_iter().find_map(|own| {
            let (lane, from, start, len, keys) = self.own_place(own)?;
            let k = keys.iter().position(|k| k.t >= start && k.t <= start + len && (self.x(from + k.t - start, b) - p.x).abs() <= PICK && (Self::volume_y(lane, k.volume) - p.y).abs() <= PICK)?;
            Some((own, k))
        })
    }

    /// Adds a point to `own`'s line at `p` and drags it on from there.
    fn add_point(&self, drag: &mut Drag, own: Own, p: Point, b: Rectangle) -> Option<Message> {
        let (t, db) = self.own_point(own, p, b)?;
        let keys = self.own_place(own)?.4;
        *drag = Drag::Point { own, key: keys.partition_point(|k| k.t <= t) };
        Some(Message::PointAdd(own, t, db))
    }

    /// A new point on `own` at `p`: its file time and level.
    fn own_point(&self, own: Own, p: Point, b: Rectangle) -> Option<(f64, f64)> {
        let (lane, from, start, ..) = self.own_place(own)?;
        Some((start + self.t(p.x, b) - from, Self::volume_db(lane, p.y)))
    }

    /// Where things snap, edit times, and whose they are: the edit's ends,
    /// the playhead, the pictures' joins, cuts' sounds split from their
    /// pictures, markers, sound clips, overlays and text.
    fn snap_targets(&self) -> Vec<(f64, Of)> {
        let mut at = vec![(0.0, Of::Edit), (self.duration, Of::Edit), (self.position, Of::Playhead)];
        for (i, c) in self.blocks.iter().enumerate() {
            at.extend([(c.from, Of::Cut(i)), (c.from + c.len(), Of::Cut(i))]);
            if c.cut.sound_before != 0.0 || c.cut.sound_after != 0.0 {
                at.extend([(c.from - c.cut.sound_before, Of::CutSound(i)), (c.from + c.len() + c.cut.sound_after, Of::CutSound(i))]);
            }
            at.extend(c.cut.marks.iter().filter(|m| m.t >= c.cut.start && m.t <= c.cut.start + c.len()).map(|m| (c.from + m.t - c.cut.start, Of::Mark(i))));
        }
        for (li, l) in self.sounds.iter().enumerate() {
            at.extend(l.blocks.iter().enumerate().flat_map(|(i, m)| [(m.clip.at, Of::Sound(li, i)), (m.clip.at + m.len(), Of::Sound(li, i))]));
        }
        for (track, items) in [(Track::Overlay, &self.overlays), (Track::Text, &self.texts)] {
            at.extend(items.iter().enumerate().flat_map(|(i, o)| [(o.item.at, Of::Item(track, i)), (o.item.at + o.len(), Of::Item(track, i))]));
        }
        at
    }

    /// The nearest snap within `SNAP` pixels for any of `edges` (edit
    /// times), skipping targets `own` says move with the drag: how far to
    /// shift, and where it lands. None while snapping is off or Shift held.
    fn snap(&self, edges: &[f64], per_px: f64, own: impl Fn(Of) -> bool) -> Option<(f64, f64)> {
        if !self.snapping {
            return None;
        }
        let reach = SNAP as f64 * per_px;
        self.snap_targets()
            .into_iter()
            .filter(|&(_, of)| !own(of))
            .flat_map(|(t, _)| edges.iter().map(move |e| (t - e, t)))
            .filter(|(d, _)| d.abs() < reach)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    }

    /// Where a moved cut would land: before the first cut whose middle is
    /// right of `x`.
    fn drop_at(&self, x: f32, b: Rectangle) -> usize {
        self.blocks.iter().filter(|c| self.x(c.from + c.len() / 2.0, b) < x).count()
    }

    /// What's under `p`, in the order a press tries things; `ctrl` and
    /// `shift` as held.
    fn hit(&self, p: Point, b: Rectangle, ctrl: bool, shift: bool) -> Hit {
        if p.y >= self.rows.height - SCROLL {
            return if self.zoomed() { Hit::Scroll } else { Hit::Nothing };
        }
        if let Some((lane, solo)) = self.listen_button_at(p) {
            return Hit::Listen(lane, solo);
        }
        if let Some((lane, fold)) = self.fold_at(p) {
            return Hit::Fold(lane, fold);
        }
        if p.x < HEAD {
            return if self.head_menu(p).is_some() { Hit::Head } else { Hit::Nothing };
        }
        if self.rows.on_transitions(p.y) {
            return self.zone_hit(p.x, b);
        }
        if let Some(h) = self.cut_lanes().hit(p, b) {
            return Hit::Auto(h);
        }
        if let Some((cut, key, t)) = self.effect_at(p, b) {
            return Hit::Effect { cut, key, t };
        }
        let clip_sound = self.rows.on_clip_sound(p.y);
        if clip_sound {
            if let Some((cut, in_edge)) = self.cut_fade_at(p, b) {
                return Hit::CutFade { cut, in_edge };
            }
            if let Some((cut, start)) = self.sound_edge_at(p.x, b) {
                return Hit::SoundEdge { cut, start };
            }
        }
        let point = self.sound_at(p, b);
        // Ctrl on a point takes it away, on any lane.
        if let Some((own, key)) = point
            && (ctrl || clip_sound)
        {
            return Hit::Point { own, key, delete: ctrl };
        }
        if point.is_none()
            && let Some(own) = self.level_tag_hit(p, b)
        {
            return Hit::LevelTag(own);
        }
        if clip_sound {
            // A new point on the cut's line once it shows, or anywhere on
            // the cut with Ctrl; elsewhere a click selects the cut.
            let cut = self.block_at(p.x, b);
            return match self.on_sound_line(p, b).or_else(|| cut.filter(|_| ctrl).map(Own::Cut)) {
                Some(own) => Hit::NewPoint(own),
                None => cut.map_or(Hit::Nothing, Hit::CutSound),
            };
        }
        if p.y < self.rows.cuts {
            return match self.selected_edge(p.x, b) {
                Some((cut, start)) => Hit::CutEdge { cut, start, crossfade: start && shift && cut > 0 },
                None => self.join_at(p.x, b).map(Hit::Join).or_else(|| self.block_at(p.x, b).map(Hit::Cut)).unwrap_or(Hit::Nothing),
            };
        }
        if let Some(lane) = self.rows.sound_at_y(p.y) {
            let which = self.sounds[lane].which;
            if let Some((clip, in_edge)) = self.fade_at(lane, p, b) {
                return Hit::ClipFade { lane, clip, in_edge };
            }
            if let Some((own, key)) = point {
                return Hit::Point { own, key, delete: false };
            }
            if let Some(own) = self.on_sound_line(p, b) {
                return Hit::NewPoint(own);
            }
            let Some(clip) = self.music_at(lane, p.x, b) else { return Hit::Nothing };
            if ctrl {
                return Hit::NewPoint(Own::Clip(which, clip));
            }
            let edge = self.music_edge_at(lane, clip, p.x, b).filter(|_| self.picked.clip() == Some((which, clip)));
            return Hit::Clip { lane, clip, edge };
        }
        if (self.rows.keys_top..self.rows.seek_top).contains(&p.y)
            && let Some((cut, key)) = self.key_at(p.x, b)
        {
            return Hit::Key { cut, key };
        }
        Hit::Playhead
    }
}

/// What's under the pointer on the main lanes. A press acts on it and the
/// pointer's shape says what that press will do: both read this one
/// answer (`Timeline::hit`), so they can't disagree.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    /// Nothing a press does anything with.
    Nothing,
    /// The scrollbar, while zoomed in.
    Scroll,
    /// A lane's solo (`true`) or mute button.
    Listen(usize, bool),
    /// A folding lane's tag: folds it (`true`) or opens it.
    Fold(FoldLane, bool),
    /// A lane's header: its menu.
    Head,
    /// A cut's effect keyframe mark, at clip time `t`.
    Effect { cut: usize, key: usize, t: f64 },
    /// A fade knob on a cut's sound.
    CutFade { cut: usize, in_edge: bool },
    /// The selected cut's sound edge (a J- or L-cut).
    SoundEdge { cut: usize, start: bool },
    /// A point on a volume line; `delete` with Ctrl.
    Point { own: Own, key: usize, delete: bool },
    /// A clip's level tag.
    LevelTag(Own),
    /// A new point on `own`'s volume line.
    NewPoint(Own),
    /// A cut on the clip sound lane: selects it.
    CutSound(usize),
    /// The selected cut's edge: a trim, or with Shift on its in edge a
    /// crossfade.
    CutEdge { cut: usize, start: bool, crossfade: bool },
    /// A cut: moves it.
    Cut(usize),
    /// A crossfade's X, the join before cut `.0`: selects it, opening the
    /// Transitions panel on it.
    Join(usize),
    /// A fade knob on a sound clip.
    ClipFade { lane: usize, clip: usize, in_edge: bool },
    /// A sound clip: at the selected clip's edge (`Some(start)`), a trim;
    /// else a move.
    Clip { lane: usize, clip: usize, edge: Option<bool> },
    /// A view keyframe.
    Key { cut: usize, key: usize },
    /// Anywhere else under the cuts: the playhead goes there.
    Playhead,
    /// On the transitions lane (gap 25f): a zone (its join, the cut after
    /// it; `side` none the overlap, else that edge), its length's edge, or
    /// empty lane to draw a new one on.
    Zone { join: usize, side: Option<End> },
    ZoneEdge { cut: usize, side: Option<End> },
    ZoneDraw,
    /// On an automation lane (gap 39).
    Auto(auto::LaneHit),
}

impl Hit {
    /// The pointer's shape over it.
    fn pointer(self) -> mouse::Interaction {
        use mouse::Interaction as I;
        match self {
            Hit::Nothing => I::default(),
            Hit::Listen(..) | Hit::Fold(..) | Hit::Head | Hit::CutSound(_) | Hit::Playhead | Hit::Join(_) | Hit::Zone { .. } => I::Pointer,
            Hit::ZoneDraw => I::Crosshair,
            Hit::Auto(h) => h.pointer(),
            Hit::Scroll | Hit::Effect { .. } | Hit::Point { .. } | Hit::Cut(_) | Hit::Clip { edge: None, .. } | Hit::Key { .. } => I::Grab,
            Hit::CutFade { .. } | Hit::SoundEdge { .. } | Hit::CutEdge { .. } | Hit::ClipFade { .. } | Hit::Clip { edge: Some(_), .. } | Hit::ZoneEdge { .. } => I::ResizingHorizontally,
            Hit::LevelTag(_) => I::ResizingVertically,
            Hit::NewPoint(_) => I::Crosshair,
        }
    }
}

impl Timeline {
    /// The two layers: block backgrounds with their thumbnails, and over them
    /// everything else, which also takes the mouse. Within one layer iced
    /// draws images over shapes, so the labels and handles need their own.
    pub fn view<'a>(self) -> iced::Element<'a, Message> {
        let t = Arc::new(self);
        use iced::widget::canvas;
        let layers = iced::widget::stack![
            canvas(Pictures(t.clone())).width(iced::Length::Fill).height(t.rows.height),
            canvas(Marks(t.clone())).width(iced::Length::Fill).height(t.rows.height),
        ];
        // Zoomed in, what is outside the window is drawn outside the widget;
        // canvases don't clip, so the container does.
        let lane = |track| {
            let lane = OverlayLane(t.clone(), track);
            let height = lane.height();
            iced::widget::container(canvas(lane).width(iced::Length::Fill).height(height)).clip(true)
        };
        let ruler = canvas(Ruler(t.clone())).width(iced::Length::Fill).height(t.ruler);
        let flags = canvas(Flags(t.clone())).width(iced::Length::Fill).height(t.flags_height());
        iced::widget::column![iced::widget::container(flags).clip(true), iced::widget::container(ruler).clip(true), lane(Track::Text), lane(Track::Overlay), iced::widget::container(layers).clip(true)].into()
    }

    /// The sound lanes: each one's clips with their waveforms and handles,
    /// and where the video ends, what runs past it dimmed (it fades out
    /// there).
    fn draw_sounds(&self, frame: &mut Frame, theme: &Theme, b: Rectangle) {
        for lane in 0..self.sounds.len() {
            self.draw_sound_lane(frame, theme, b, lane);
        }
    }

    fn draw_sound_lane(&self, frame: &mut Frame, theme: &Theme, b: Rectangle, li: usize) {
        let palette = theme.extended_palette();
        let SoundLane { which, blocks, track } = &self.sounds[li];
        let colour = lane_colour(crate::mixcheck::Lane::Track(*which), theme);
        let (top, h) = (self.rows.sound_top(li) + 2.0, self.rows.lane_h(li) - 4.0);
        if self.rows.folded[li] {
            // Folded: where its clips are, thin, its name to open it.
            let mid = self.rows.sound_top(li) + self.rows.lane_h(li) / 2.0;
            folded_strips(frame, self, b, mid, blocks.iter().map(|m| (m.clip.at, m.len())), colour);
            return;
        }
        lane_ground(frame, self, b, (top, h), self.duration, 4.0, palette.background.weak.color.scale_alpha(0.5));
        for (i, m) in blocks.iter().enumerate() {
            let (l, r) = (self.x(m.clip.at, b) + GAP / 2.0, self.x(m.clip.at + m.len(), b) - GAP / 2.0);
            if r < 0.0 || l > b.width {
                continue;
            }
            let selected = Some((*which, i)) == self.picked.clip();
            coloured_block(frame, (l, r), (top, h), colour, selected);
            // The waveform as it's heard: the track's level and line, the
            // clip's own, its fades and crossfades (`mixdown::clip_gain`).
            if let Some(w) = &m.wave {
                let heard = |t: f64| crate::mixdown::clip_gain(track, i, m.clip.at + t - m.clip.start, self.video) as f32;
                let wave = Wave { left: l, right: r, top, height: h, start: m.clip.start, len: m.len() };
                wave.draw(frame, b, w, m.even_wave.as_ref().unwrap_or(w), heard, palette.background.base.text.scale_alpha(0.7));
            }
            draw_fades(frame, (l, r), top, h, m.len(), (m.clip.fade_in, m.clip.fade_out));
            if selected {
                selected_block(frame, theme, (l, r), (top, h), (m.clip.start > 1e-3, m.clip.start + m.len() < m.most - 1e-3));
            }
            // At the bottom: the volume line runs along the top.
            if r - l > m.label.chars().count() as f32 * 6.0 + 28.0 {
                name_tag(frame, (l + 9.0, top + h - 18.0), 16.0, &m.label, 11.0, 5.5);
            }
        }
        // Overlaps are crossfades.
        for (i, a) in blocks.iter().enumerate() {
            for c in &blocks[i + 1..] {
                let (t0, t1) = (a.clip.at.max(c.clip.at), (a.clip.at + a.len()).min(c.clip.at + c.len()));
                if t1 - t0 > 1e-6 {
                    crossfade(frame, (self.x(t0, b), self.x(t1, b)), (top, h), true);
                }
            }
        }
        // Past the video's end the sound fades out in the export: dim it,
        // and mark the end.
        if self.duration > self.video + 1e-6 {
            let x = self.x(self.video, b);
            let to = self.x(self.duration, b);
            frame.fill(&Path::rectangle(Point::new(x, top), Size::new((to - x).max(0.0), h)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55));
            frame.fill(&Path::rectangle(Point::new(x - 1.0, self.rows.sound_top(li)), Size::new(2.0, self.rows.lane_h(li))), palette.background.base.text.scale_alpha(0.8));
        }
        // Each clip's own line, inside it.
        for (i, m) in blocks.iter().enumerate() {
            self.draw_own_line(frame, b, Own::Clip(*which, i), self.rows.sound_lane(li), m.clip.at, m.clip.start, m.len(), &m.clip.volume);
        }
        for (i, m) in blocks.iter().enumerate() {
            draw_tag(frame, self.level_tag_at(Own::Clip(*which, i), b), &m.level);
        }
    }

    /// An own volume line (a cut's or a sound clip's) inside its block on
    /// `lane`, once it has points: the block starts at edit time `from`,
    /// at `start` in its file, for `len` seconds; points outside it aren't
    /// drawn (their indices still count).
    #[allow(clippy::too_many_arguments)]
    fn draw_own_line(&self, frame: &mut Frame, b: Rectangle, own: Own, lane: Lane, from: f64, start: f64, len: f64, keys: &[crate::project::VolumeKey]) {
        if keys.is_empty() {
            return;
        }
        let x = |t: f64| self.x(from + t - start, b);
        let end = start + len;
        let at = |t: f64| crate::project::volume_at(keys, t);
        let points: Vec<_> = keys.iter().map(|k| if k.t >= start && k.t <= end { (x(k.t), k.volume) } else { (f32::NAN, k.volume) }).collect();
        let selected = self.picked.point().filter(|p| p.0 == own).map(|p| p.1);
        let (l, r) = (x(start) + GAP / 2.0, x(end) - GAP / 2.0);
        draw_line(frame, b, lane, SOUND_LINE, ((l, at(start)), (r, at(end))), &points, selected);
    }

    /// The clip sound lane: under each cut, the waveform of what it plays,
    /// and the lane's name.
    fn draw_clip_sound(&self, frame: &mut Frame, theme: &Theme, b: Rectangle) {
        let palette = theme.extended_palette();
        let (top, h) = (self.rows.cuts + 1.0, self.rows.clip_sound - 2.0);
        lane_ground(frame, self, b, (top, h), self.video, 3.0, palette.background.weak.color.scale_alpha(0.35));
        for (i, c) in self.blocks.iter().enumerate() {
            let (l, r) = self.sound_x(c, b);
            if r < 0.0 || l > b.width {
                continue;
            }
            let block = Path::rounded_rectangle(Point::new(l, top), Size::new((r - l).max(1.0), h), 3.0.into());
            frame.fill(&block, SOUND_LINE.scale_alpha(if Some(i) == self.picked.cut() { 0.35 } else { 0.18 }));
            // A sound split from its picture (J, L or short) gets a frame of
            // its own: it no longer ends where the picture does.
            if c.cut.sound_before != 0.0 || c.cut.sound_after != 0.0 {
                frame.stroke(&block, Stroke::default().with_width(1.5).with_color(SOUND_LINE.scale_alpha(0.9)));
            }
            let Some(w) = &c.wave else { continue };
            // As it's heard: evened out (once that's read), its level and
            // points, the clips' level; what the file holds faint behind.
            let (_, from, len) = c.sound_span();
            let ramp = |t: f64| crate::project::fade_ramp(t - from, from + len - t, c.cut.sound_fade_in, c.cut.sound_fade_out);
            let heard = |t: f64| crate::mixdown::cut_gain(self.clips_db, c.cut.sound_db(), crate::project::volume_at(&c.cut.volume, t), ramp(t)) as f32;
            let wave = Wave { left: l, right: r, top, height: h, start: from, len };
            wave.draw(frame, b, w, c.even_wave.as_ref().unwrap_or(w), heard, palette.background.base.text.scale_alpha(0.6));
        }
        // Its sound's fades, a handle at each top corner as on sound clips.
        for c in &self.blocks {
            let (l, r) = self.sound_x(c, b);
            if r < 0.0 || l > b.width || c.wave.is_none() {
                continue;
            }
            draw_fades(frame, (l, r), top, h, c.sound_span().2, (c.cut.sound_fade_in, c.cut.sound_fade_out));
        }
        // The selected cut's sound edges, to drag apart from its picture:
        // yellow where its clip has more sound, red at the file's end.
        if let Some(c) = self.picked.cut().and_then(|i| self.blocks.get(i)) {
            let more = (c.cut.start - c.cut.sound_before > 0.01, c.cut.start + c.len() + c.cut.sound_after < c.most - 0.01);
            trim_handles(frame, self.sound_x(c, b), top, h, more);
        }
        // Each cut's level, where there's room: a tag at its left, so a
        // lowered or muted cut shows in words, not colour alone; dragged up
        // or down, the knob for its level.
        for (i, c) in self.blocks.iter().enumerate() {
            draw_tag(frame, self.level_tag_at(Own::Cut(i), b), &c.level);
        }
        // Through a crossfade the two cuts' sounds overlap, as their
        // pictures do.
        for c in self.blocks.iter().filter(|c| c.cut.crossfade > 0.0) {
            crossfade(frame, (self.x(c.from, b), self.x(c.from + c.cut.crossfade, b)), (top, h), false);
        }
        self.draw_sound(frame, b);

    }

    /// Each cut's sound line on the clip sound lane, for cuts with points:
    /// the points within the cut, and the line from its in to its out point.
    fn draw_sound(&self, frame: &mut Frame, b: Rectangle) {
        for (ci, c) in self.blocks.iter().enumerate() {
            let (from, start, len) = c.sound_span();
            self.draw_own_line(frame, b, Own::Cut(ci), self.rows.cut_lane(), from, start, len, &c.cut.volume);
        }
    }

    /// Where `own`'s level tag sits, if its block has room for it: at its
    /// left, past the lane's name, in the middle of its lane.
    fn level_tag_at(&self, own: Own, b: Rectangle) -> Option<Rectangle> {
        let (label, from, len, top, h) = match own {
            Own::Cut(i) => {
                let c = self.blocks.get(i)?;
                (&c.level, c.from, c.len(), self.rows.cuts + 1.0, self.rows.clip_sound - 2.0)
            }
            Own::Clip(which, i) => {
                let li = self.sounds.iter().position(|l| l.which == which).filter(|&li| !self.rows.folded[li])?;
                let m = self.sounds[li].blocks.get(i)?;
                (&m.level, m.clip.at, m.len(), self.rows.sound_top(li) + 2.0, self.rows.lane_h(li) - 4.0)
            }
        };
        let (l, r) = (self.x(from, b) + GAP / 2.0, self.x(from + len, b) - GAP / 2.0);
        let w = label.chars().count() as f32 * 6.0 + 8.0;
        if r - l < w + 12.0 || r < 0.0 || l > b.width {
            return None;
        }
        // Inside its clip, past the header column.
        let x = l.max(HEAD + PICK + 4.0).min(r - w - 4.0);
        Some(Rectangle::new(Point::new(x, top + h / 2.0 - 7.0), Size::new(w, 14.0)))
    }

    /// Sound lane `li`'s name tag: with its fold arrow for SFX 2 («▸ SFX 2
    /// (3)» folded, «▾ SFX 2» open), plain for the others.
    fn sound_tag(&self, li: usize) -> String {
        let lane = &self.sounds[li];
        match lane.which {
            SoundTrack::Sfx2 if self.rows.folded[li] => format!("▸ {} ({})", lane.which.name(), lane.blocks.len()),
            SoundTrack::Sfx2 => format!("▾ {}", lane.which.name()),
            w => w.name().to_owned(),
        }
    }

    /// The header column over the cut, clip sound, keyframe and sound
    /// lanes: each lane's name, solo and mute, and its level when not 0 dB.
    fn draw_heads(&self, frame: &mut Frame, theme: &Theme) {
        draw_head_ground(frame, theme, self.rows.height);
        let ink = theme.extended_palette().background.base.text.scale_alpha(0.45);
        let video = theme.extended_palette().primary.base.color;
        head_cell(frame, 0.0, self.rows.cuts, video);
        head_cell(frame, self.rows.cuts, self.rows.clip_sound, SOUND_LINE);
        head_cell(frame, self.rows.trans_top, self.rows.trans, TRANSITION);
        head_cell(frame, self.rows.effects_top, self.rows.seek_top - self.rows.effects_top, iced::Color::TRANSPARENT);
        for li in 0..self.sounds.len() {
            head_cell(frame, self.rows.sound_top(li), self.rows.lane_h(li), lane_colour(crate::mixcheck::Lane::Track(self.sounds[li].which), theme));
        }
        lane_tag(frame, "Video", video, self.rows.cuts / 2.0);
        lane_tag(frame, &self.trans_tag(), TRANSITION, self.rows.trans_top + self.rows.trans / 2.0);
        let (top, h) = (self.rows.cuts + 1.0, self.rows.clip_sound - 2.0);
        lane_tag(frame, "Clip sound", SOUND_LINE, tag_mid(top, h));
        draw_listen(frame, "Clip sound", tag_mid(top, h), &self.listen, super::listen::CLIP_SOUND);
        head_level(frame, top, h, self.clips_db);
        let lanes = self.cut_lanes();
        head_rows(frame, theme, (self.rows.effects_top, &lanes.tag()), (self.rows.keys > 0.0).then_some((self.rows.keys_top, "View")));
        lanes.draw_heads(frame, theme);
        frame.fill_text(Text { content: "SOUND".into(), position: Point::new(PICK + 4.0, self.rows.seek_top + SEEK / 2.0), color: ink, size: 9.5.into(), align_y: Vertical::Center, ..Default::default() });
        for (li, lane) in self.sounds.iter().enumerate() {
            let colour = lane_colour(crate::mixcheck::Lane::Track(lane.which), theme);
            let (top, h) = (self.rows.sound_top(li) + 2.0, self.rows.lane_h(li) - 4.0);
            let mid = if self.rows.folded[li] { self.rows.sound_top(li) + self.rows.lane_h(li) / 2.0 } else { tag_mid(top, h) };
            lane_tag(frame, &self.sound_tag(li), colour, mid);
            draw_listen(frame, &self.sound_tag(li), mid, &self.listen, super::listen::lane_of(lane.which));
            if !self.rows.folded[li] {
                head_level(frame, top, h, lane.track.volume);
            }
        }
    }

    /// The menu a sound lane's header opens: the lane's own level and what
    /// acts on the whole lane.
    fn head_menu(&self, p: Point) -> Option<super::sound_ui::Menu> {
        use super::sound_ui::Menu;
        if self.rows.on_clip_sound(p.y) {
            Some(Menu::ClipSound)
        } else {
            self.rows.sound_at_y(p.y).map(|li| Menu::Track(self.sounds[li].which))
        }
    }

    /// Everything the sound lanes and the clip sound lane are drawn from
    /// (`draw_sounds`, `draw_clip_sound`), hashed: while it holds, their
    /// kept drawing is right. The playhead isn't in it, so playing and
    /// dragging the playhead never redraw them.
    fn sound_key(&self, b: Rectangle, theme: &Theme) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let f = |h: &mut std::collections::hash_map::DefaultHasher, v: f64| v.to_bits().hash(h);
        for v in [self.start, self.span, self.clips_db, self.duration, self.video, b.width as f64, b.height as f64] {
            f(&mut h, v);
        }
        let ptr = |w: &Option<Arc<crate::waveform::Waveform>>| w.as_ref().map_or(0, |w| Arc::as_ptr(w) as usize);
        for c in &self.blocks {
            for v in [c.from, c.len(), c.cut.start, c.most, c.cut.sound_before, c.cut.sound_after, c.cut.sound_fade_in, c.cut.sound_fade_out, c.cut.sound_db(), c.cut.gain, c.cut.crossfade] {
                f(&mut h, v);
            }
            for k in &c.cut.volume {
                f(&mut h, k.t);
                f(&mut h, k.volume);
            }
            (ptr(&c.wave), ptr(&c.even_wave)).hash(&mut h);
            c.level.hash(&mut h);
        }
        for l in &self.sounds {
            format!("{:?}{:?}", l.which, l.track).hash(&mut h);
            for m in &l.blocks {
                for v in [m.clip.at, m.clip.start, m.len(), m.most, m.clip.fade_in, m.clip.fade_out] {
                    v.to_bits().hash(&mut h);
                }
                for k in &m.clip.volume {
                    (k.t.to_bits(), k.volume.to_bits()).hash(&mut h);
                }
                (ptr(&m.wave), ptr(&m.even_wave), &m.label, &m.level).hash(&mut h);
            }
        }
        // The sound lanes show a selected cut, clip or point; not an item.
        format!("{:?}{:?}{:?}{:?}{:?}", self.rows, self.picked.cut(), self.picked.clip(), self.picked.point(), theme.extended_palette().background.base.color).hash(&mut h);
        h.finish()
    }

    /// The middle of sound lane `li`'s name in the header (`tag_mid`).
    fn sound_mid(&self, li: usize) -> f32 {
        if self.rows.folded[li] { self.rows.sound_top(li) + self.rows.lane_h(li) / 2.0 } else { tag_mid(self.rows.sound_top(li) + 2.0, self.rows.lane_h(li) - 4.0) }
    }

    /// The fold toggle under `p`: a folded lane anywhere (but its solo and
    /// mute), an open one on its name tag.
    fn fold_at(&self, p: Point) -> Option<(FoldLane, bool)> {
        // The Effects row's name folds its automation lanes, while it has any.
        if self.cut_lanes().on_tag(p) {
            return Some((FoldLane::Effects, !self.folds.effects));
        }
        // The transitions lane: folded, anywhere opens it; open, its name.
        if self.rows.on_transitions(p.y) {
            let folded = self.folds.transitions;
            let mid = self.rows.trans_top + self.rows.trans / 2.0;
            return (folded || (p.x < HEAD && fold_tag(&self.trans_tag(), mid).contains(p))).then_some((FoldLane::Transitions, !folded));
        }
        let li = self.rows.sound_at_y(p.y)?;
        if self.sounds[li].which != SoundTrack::Sfx2 {
            return None;
        }
        let folded = self.rows.folded[li];
        (folded || fold_tag(&self.sound_tag(li), self.sound_mid(li)).contains(p)).then_some((FoldLane::Sfx2, !folded))
    }

    /// A sound lane's solo (`true`) or mute button under `p`: the lane
    /// (`listen::lane_of`) and which.
    fn listen_button_at(&self, p: Point) -> Option<(usize, bool)> {
        let (name, mid, lane) = if self.rows.on_clip_sound(p.y) {
            ("Clip sound".to_owned(), tag_mid(self.rows.cuts + 1.0, self.rows.clip_sound - 2.0), super::listen::CLIP_SOUND)
        } else {
            let li = self.rows.sound_at_y(p.y)?;
            let which = self.sounds[li].which;
            (self.sound_tag(li), self.sound_mid(li), super::listen::lane_of(which))
        };
        let [solo, mute] = listen_buttons(&name, mid);
        if solo.expand(1.0).contains(p) {
            Some((lane, true))
        } else {
            mute.expand(1.0).contains(p).then_some((lane, false))
        }
    }

    /// The level tag under `p`: whose it is.
    fn level_tag_hit(&self, p: Point, b: Rectangle) -> Option<Own> {
        let tag = |own: Own| self.level_tag_at(own, b).is_some_and(|r| r.expand(2.0).contains(p)).then_some(own);
        if self.rows.on_clip_sound(p.y) {
            (0..self.blocks.len()).find_map(|i| tag(Own::Cut(i)))
        } else {
            let li = self.rows.sound_at_y(p.y)?;
            let which = self.sounds[li].which;
            (0..self.sounds[li].blocks.len()).find_map(|i| tag(Own::Clip(which, i)))
        }
    }

    /// A press on `own`'s level tag: a second within `DOUBLE_CLICK` puts it
    /// back to its normal level; else a drag of the level starts.
    fn tag_press(&self, drag: &mut Drag, last: &mut Option<(std::time::Instant, Own)>, own: Own, p: Point) -> Option<Message> {
        let now = std::time::Instant::now();
        if last.take().is_some_and(|(at, o)| o == own && now - at < DOUBLE_CLICK) {
            return Some(Message::Sound(super::sound_ui::SoundMsg::Restore(own)));
        }
        *last = Some((now, own));
        *drag = Drag::Level { own, y0: p.y, from: self.level_of(own)? };
        Some(match own {
            Own::Cut(i) => Message::Select(i),
            Own::Clip(which, i) => Message::SelectMusic(which, i),
        })
    }

    /// The level `own` has now, dB.
    fn level_of(&self, own: Own) -> Option<f64> {
        match own {
            Own::Cut(i) => self.blocks.get(i).map(|c| c.cut.gain),
            Own::Clip(which, i) => self.sounds.iter().find(|l| l.which == which)?.track.clips.get(i).map(|c| c.gain),
        }
    }

    /// A cut block's left and right edges.
    fn block_x(&self, c: &Block, b: Rectangle) -> (f32, f32) {
        (self.x(c.from, b) + GAP / 2.0, self.x(c.from + c.len(), b) - GAP / 2.0)
    }

    /// Where a cut's sound is drawn on the clip sound lane: its picture's
    /// stretch, wider or narrower by what runs past it or stops short.
    fn sound_x(&self, c: &Block, b: Rectangle) -> (f32, f32) {
        let (from, _, len) = c.sound_span();
        (self.x(from, b) + GAP / 2.0, self.x(from + len, b) - GAP / 2.0)
    }

    /// The selected cut's sound edge under `x` on the clip sound lane: in
    /// (`true`) or out.
    fn sound_edge_at(&self, x: f32, b: Rectangle) -> Option<(usize, bool)> {
        let i = self.picked.cut().filter(|&i| i < self.blocks.len())?;
        let (l, r) = self.sound_x(&self.blocks[i], b);
        if x < l - EDGE || x > r + EDGE {
            return None;
        }
        edge_of(l, r, x).map(|start| (i, start))
    }
}

/// An overlay track's lane (the text's on top, then the pictures'), over
/// the cuts: a canvas of its own, so the rows under it keep their places.
struct OverlayLane(Arc<Timeline>, Track);

#[derive(Default)]
enum OverlayDrag {
    #[default]
    None,
    Move { item: usize, x0: f32, at: f64, per_px: f64 },
    /// An edge, at own time `from` when pressed.
    Trim { item: usize, start: bool, x0: f32, from: f64, per_px: f64 },
    /// A keyframe mark: item `item`'s effect keyframe `key` (`effects`) or
    /// motion keyframe, to another time.
    Key { item: usize, effects: bool, key: usize },
    /// On its automation lanes (its Motion row's if `true`).
    Auto(bool, auto::LaneDrag),
}

/// What's under the pointer on an overlay lane, read by a press and by
/// the pointer's shape alike (`OverlayLane::hit`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum OverlayHit {
    Nothing,
    /// The lane's name, or the folded lane: folds or opens it.
    Fold,
    /// A keyframe mark (see `OverlayLane::key_at`), at edit time `at`.
    Key { item: usize, effects: bool, key: usize, at: f64 },
    /// The keyframe rows away from a mark: the playhead goes there.
    Seek,
    /// The selected item's edge: a trim.
    Edge { item: usize, start: bool },
    /// An item: selects and moves it.
    Item(usize),
    /// The lane away from its items: nothing selected.
    Empty,
    /// Its automation lanes (gap 39), its Motion row's if `true`, and that
    /// row's name folding them.
    Auto(bool, auto::LaneHit),
    LanesFold(bool),
}

impl OverlayHit {
    /// The pointer's shape over it.
    fn pointer(self) -> mouse::Interaction {
        use mouse::Interaction as I;
        match self {
            OverlayHit::Nothing | OverlayHit::Empty => I::default(),
            OverlayHit::Fold | OverlayHit::Seek | OverlayHit::LanesFold(_) => I::Pointer,
            OverlayHit::Auto(_, h) => h.pointer(),
            OverlayHit::Key { .. } | OverlayHit::Item(_) => I::Grab,
            OverlayHit::Edge { .. } => I::ResizingHorizontally,
        }
    }
}

impl OverlayLane {
    fn blocks(&self) -> &[OverlayBlock] {
        match self.1 {
            Track::Overlay => &self.0.overlays,
            Track::Text => &self.0.texts,
        }
    }

    fn item_at(&self, x: f32, b: Rectangle) -> Option<usize> {
        self.blocks().iter().position(|o| x >= self.0.x(o.item.at, b) && x <= self.0.x(o.item.at + o.len(), b))
    }

    fn edge_at(&self, i: usize, x: f32, b: Rectangle) -> Option<bool> {
        let o = &self.blocks()[i];
        edge_of(self.0.x(o.item.at, b), self.0.x(o.item.at + o.len(), b), x)
    }

    fn folded(&self) -> bool {
        match self.1 {
            Track::Text => self.0.folds.text,
            Track::Overlay => self.0.folds.overlay,
        }
    }

    fn fold_lane(&self) -> FoldLane {
        match self.1 {
            Track::Text => FoldLane::Text,
            Track::Overlay => FoldLane::Overlay,
        }
    }

    /// The lane, and its keyframe rows once it has items; folded, a thin row.
    fn height(&self) -> f32 {
        if self.folded() {
            FOLDED
        } else if self.blocks().is_empty() {
            self.0.rows.overlay
        } else {
            self.motion_top() + KEYS + self.lanes(true).height()
        }
    }

    /// Its own automation lanes: its effects' under its Effects row, its
    /// place's (across, down, size, rotation) under its Motion row.
    fn lanes(&self, motion: bool) -> auto::Lanes<'_> {
        use super::automation_ui::{AutoOn, Param};
        let t = &self.0;
        let (all, folded) = match (self.1, motion) {
            (Track::Overlay, false) => (&t.overlay_auto, t.folds.overlay_effects),
            (Track::Overlay, true) => (&t.overlay_auto, t.folds.overlay_motion),
            (Track::Text, false) => (&t.text_auto, t.folds.text_effects),
            (Track::Text, true) => (&t.text_auto, t.folds.text_motion),
        };
        let params = all.iter().copied().filter(|p| matches!(p, Param::Place(_)) == motion).collect();
        let owners = self.blocks().iter().map(|o| auto::Owner { on: AutoOn::Item(o.item.id), from: o.item.at, start: o.item.start, len: o.len(), clips: &o.item.automation }).collect();
        let (name, row_top, row_h) = if motion { ("Motion", self.motion_top(), KEYS) } else { ("Effects", t.rows.overlay, EFFECTS) };
        auto::Lanes { t, params, name, row_top, row_h, top: row_top + row_h, folded, owners }
    }

    /// Where its Motion row starts: under its Effects row and lanes.
    fn motion_top(&self) -> f32 {
        self.0.rows.overlay + EFFECTS + self.lanes(false).height()
    }

    /// The row whose lanes a fold tag or a lane hit is in.
    fn fold_of(&self, motion: bool) -> FoldLane {
        match (self.1, motion) {
            (Track::Overlay, false) => FoldLane::OverlayEffects,
            (Track::Overlay, true) => FoldLane::OverlayMotion,
            (Track::Text, false) => FoldLane::TextEffects,
            (Track::Text, true) => FoldLane::TextMotion,
        }
    }

    /// What's under `p`, in the order a press tries things.
    fn hit(&self, p: Point, b: Rectangle) -> OverlayHit {
        // Folded, anywhere opens it; open, its name folds it.
        let (name, _) = self.name();
        if self.folded() || fold_tag(&self.tag_name(name), (self.0.rows.overlay - 4.0) / 2.0 + 2.0).contains(p) {
            return OverlayHit::Fold;
        }
        // The rest of the header column: nothing there.
        if p.x < HEAD {
            return OverlayHit::Nothing;
        }
        for motion in [false, true] {
            let lanes = self.lanes(motion);
            if !self.blocks().is_empty() && lanes.on_tag(p) {
                return OverlayHit::LanesFold(motion);
            }
            if let Some(h) = lanes.hit(p, b) {
                return OverlayHit::Auto(motion, h);
            }
        }
        if let Some((item, effects, key, at)) = self.key_at(p, b) {
            return OverlayHit::Key { item, effects, key, at };
        }
        let rows = |y: f32| (self.0.rows.overlay..self.0.rows.overlay + EFFECTS).contains(&y) || (self.motion_top()..self.motion_top() + KEYS).contains(&y);
        if rows(p.y) {
            return OverlayHit::Seek;
        }
        if p.y >= self.0.rows.overlay {
            return OverlayHit::Nothing;
        }
        match self.item_at(p.x, b) {
            None => OverlayHit::Empty,
            Some(item) => match self.edge_at(item, p.x, b).filter(|_| self.0.picked.item() == Some((self.1, item))) {
                Some(start) => OverlayHit::Edge { item, start },
                None => OverlayHit::Item(item),
            },
        }
    }

    /// The keyframe mark under `p` in its row: the item, whether it's in
    /// the effects row, the keyframe and its edit time.
    fn key_at(&self, p: Point, b: Rectangle) -> Option<(usize, bool, usize, f64)> {
        let effects = (self.0.rows.overlay..self.0.rows.overlay + EFFECTS).contains(&p.y);
        if !effects && !(self.motion_top()..self.motion_top() + KEYS).contains(&p.y) {
            return None;
        }
        self.blocks().iter().enumerate().find_map(|(item, o)| {
            let times: Vec<f64> = if effects { o.item.effects.iter().map(|k| k.t).collect() } else { o.item.motion.iter().map(|k| k.t).collect() };
            times
                .into_iter()
                .enumerate()
                .filter(|&(_, t)| t >= o.item.start && t <= o.item.start + o.len())
                .map(|(key, t)| (key, o.item.at + t - o.item.start))
                .find(|&(_, t)| (self.0.x(t, b) - p.x).abs() <= PICK)
                .map(|(key, t)| (item, effects, key, t))
        })
    }
}

impl OverlayLane {
    /// Its name and colour.
    fn name(&self) -> (&'static str, iced::Color) {
        match self.1 {
            Track::Overlay => ("Overlay", OVERLAY_COLOUR),
            Track::Text => ("Text", TEXT_COLOUR),
        }
    }

    /// Its tag's words: «▸ Text (2)» folded, «▾ Text» open.
    fn tag_name(&self, name: &str) -> String {
        if self.folded() { format!("▸ {name} ({})", self.blocks().len()) } else { format!("▾ {name}") }
    }
}

/// The wheel anywhere on the timeline (its lanes, ruler, markers): the
/// app zooms, or with Shift pans (`TimelineWheel`). Shift is the app's to
/// know: a lane learned it from its own events and missed it when Shift
/// was pressed elsewhere, and the ruler and markers had no wheel at all
/// (user, 2026-11-05).
fn wheel(delta: &mouse::ScrollDelta, t: &Timeline, cursor: mouse::Cursor, bounds: Rectangle) -> Option<canvas::Action<Message>> {
    let p = cursor.position_in(bounds)?;
    let (dx, dy) = match *delta {
        mouse::ScrollDelta::Lines { x, y } => (x, y),
        mouse::ScrollDelta::Pixels { x, y } => (x / 40.0, y / 40.0),
    };
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some(canvas::Action::publish(Message::TimelineWheel(dx, dy, t.t(p.x, bounds))).and_capture())
}

impl canvas::Program<Message> for OverlayLane {
    /// Its drag, and the drag epoch it began in (see `State::epoch`).
    type State = (OverlayDrag, u64);

    fn update(&self, state: &mut (OverlayDrag, u64), event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        let t = &self.0;
        if state.1 != t.drag_epoch {
            *state = (OverlayDrag::None, t.drag_epoch);
        }
        let drag = &mut state.0;
        match event {
            Mouse(mouse::Event::WheelScrolled { delta }) => wheel(delta, t, cursor, bounds),
            // Right-click on its automation lanes: a point's ease, a clip gone.
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) if cursor.position_in(bounds).is_some_and(|p| p.x >= HEAD && [false, true].iter().any(|&m| self.lanes(m).lane_at(p.y).is_some())) => {
                let p = cursor.position_in(bounds)?;
                let message = [false, true].into_iter().find_map(|m| Some(self.lanes(m)).filter(|l| l.lane_at(p.y).is_some())?.right(p, bounds))?;
                Some(canvas::Action::publish(message).and_capture())
            }
            // Right-click an overlay picture: what file it is (text has none).
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) if self.1 == Track::Overlay => {
                let p = cursor.position_in(bounds)?;
                let item = self.item_at(p.x, bounds).filter(|_| p.y < self.0.rows.overlay)?;
                Some(canvas::Action::publish(Message::Info(super::info::Of::Overlay((self.1, item)))).and_capture())
            }
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                let per_px = t.span / Timeline::width(bounds) as f64;
                let message = match self.hit(p, bounds) {
                    OverlayHit::Nothing => return None,
                    OverlayHit::Fold => Message::Fold(self.fold_lane(), !self.folded()),
                    OverlayHit::LanesFold(motion) => Message::Fold(self.fold_of(motion), !self.lanes(motion).folded),
                    OverlayHit::Auto(motion, h) => match self.lanes(motion).press(h, p, t.held.command()) {
                        Ok(d) => {
                            *drag = OverlayDrag::Auto(motion, d);
                            return Some(canvas::Action::request_redraw().and_capture());
                        }
                        Err(message) => message,
                    },
                    OverlayHit::Key { item, effects, key, at } => {
                        // A jump there, and a drag retimes it.
                        *drag = OverlayDrag::Key { item, effects, key };
                        Message::OverlayJump(at)
                    }
                    OverlayHit::Seek => Message::OverlayJump(t.t(p.x, bounds)),
                    OverlayHit::Empty => Message::OverlaySelect(None),
                    OverlayHit::Edge { item, start } => {
                        let o = &self.blocks()[item];
                        *drag = OverlayDrag::Trim { item, start, x0: p.x, from: if start { o.item.start } else { o.item.start + o.len() }, per_px };
                        Message::OverlaySelect(Some((self.1, item)))
                    }
                    OverlayHit::Item(item) => {
                        *drag = OverlayDrag::Move { item, x0: p.x, at: self.blocks()[item].item.at, per_px };
                        Message::OverlaySelect(Some((self.1, item)))
                    }
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            Mouse(mouse::Event::CursorMoved { position }) => {
                let x = position.x - bounds.x;
                let message = match drag {
                    OverlayDrag::None => return None,
                    OverlayDrag::Move { item, x0, at, per_px } => {
                        let (track, item) = (self.1, *item);
                        let at = *at + (x - *x0) as f64 * *per_px;
                        let len = self.blocks()[item].len();
                        snapped(t.snap(&[at, at + len], *per_px, |of| of == Of::Item(track, item)), |by| Message::OverlayMove((track, item), at + by))
                    }
                    OverlayDrag::Trim { item, start, x0, from, per_px } => {
                        let (track, item, start) = (self.1, *item, *start);
                        let own_t = *from + (x - *x0) as f64 * *per_px;
                        let o = &self.blocks()[item];
                        snapped(t.snap(&[o.item.at - o.item.start + own_t], *per_px, |of| of == Of::Item(track, item)), |by| Message::OverlayTrim((track, item), start, own_t + by))
                    }
                    OverlayDrag::Key { item, effects, key } => {
                        let o = &self.blocks()[*item];
                        let at = self.0.t(x, bounds).clamp(o.item.at, o.item.at + o.len());
                        Message::OverlayKeyMove((self.1, *item), *effects, *key, o.item.start + at - o.item.at)
                    }
                    OverlayDrag::Auto(motion, d) => {
                        let message = self.lanes(*motion).drag(d, Point::new(x, position.y - bounds.y), bounds, t.held.shift());
                        return Some(message.map_or_else(canvas::Action::request_redraw, canvas::Action::publish).and_capture());
                    }
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => match std::mem::take(drag) {
                OverlayDrag::None => None,
                OverlayDrag::Auto(motion, d) => Some(self.lanes(motion).release(d, bounds).map_or_else(canvas::Action::request_redraw, canvas::Action::publish).and_capture()),
                _ => Some(canvas::Action::publish(Message::ScrubDone).and_capture()),
            },
            _ => None,
        }
    }

    fn draw(&self, (drag, _): &(OverlayDrag, u64), renderer: &Renderer, theme: &Theme, bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry> {
        let t = &self.0;
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        let (name, colour) = self.name();
        if self.folded() {
            // Folded: where its items are, thin, and its name to open it.
            let mid = FOLDED / 2.0;
            folded_strips(&mut frame, t, bounds, mid, self.blocks().iter().map(|o| (o.item.at, o.len())), colour);
            let at = t.x(t.position, bounds);
            frame.fill(&Path::rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, FOLDED)), palette.primary.strong.color);
            draw_head_ground(&mut frame, theme, FOLDED);
            head_cell(&mut frame, 0.0, FOLDED, colour);
            lane_tag(&mut frame, &self.tag_name(name), colour, mid);
            return vec![frame.into_geometry()];
        }
        let (top, h) = (2.0, self.0.rows.overlay - 4.0);
        lane_ground(&mut frame, t, bounds, (top, h), t.duration, 4.0, palette.background.weak.color.scale_alpha(0.5));
        for (i, o) in self.blocks().iter().enumerate() {
            let (l, r) = (t.x(o.item.at, bounds) + GAP / 2.0, t.x(o.item.at + o.len(), bounds) - GAP / 2.0);
            if r < 0.0 || l > bounds.width {
                continue;
            }
            let selected = Some((self.1, i)) == t.picked.item();
            coloured_block(&mut frame, (l, r), (top, h), colour, selected);
            if selected {
                // A picture or text has no end of its own: footage both ways
                // but before its start.
                selected_block(&mut frame, theme, (l, r), (top, h), (o.item.start > 1e-3, true));
            }
            // The picture, as tall as the lane at most and as wide as the
            // block allows (user, 2026-11-07: shown at any size), then its
            // name where there's room.
            let mut text_x = l + 9.0;
            if let Some(thumb) = &o.thumb {
                let w = ((h - 4.0) * thumb.aspect).min(r - l - 16.0);
                if w >= 12.0 {
                    let th = w / thumb.aspect;
                    frame.draw_image(Rectangle::new(Point::new(l + 8.0, top + (h - th) / 2.0), Size::new(w, th)), canvas::Image::new(thumb.image.clone()));
                    text_x += w + 2.0;
                }
            }
            if r - text_x > o.label.chars().count() as f32 * 6.0 + 12.0 {
                frame.fill_text(Text {
                    content: o.label.clone(),
                    position: Point::new(text_x, top + h / 2.0),
                    color: iced::Color::BLACK,
                    size: 11.0.into(),
                    align_y: Vertical::Center,
                    ..Default::default()
                });
            }
        }
        if !self.blocks().is_empty() {
            draw_rows(&mut frame, theme, t, bounds, self.0.rows.overlay, Some(self.motion_top()));
            for motion in [false, true] {
                self.lanes(motion).draw(&mut frame, theme, bounds, match drag {
                    OverlayDrag::Auto(m, d) if *m == motion => Some(d),
                    _ => None,
                });
            }
            let inside = |o: &OverlayBlock, k: f64| k >= o.item.start && k <= o.item.start + o.len();
            for o in self.blocks() {
                for k in o.item.effects.iter().map(|k| k.t).filter(|&k| inside(o, k)) {
                    effect_mark(&mut frame, t.x(o.item.at + k - o.item.start, bounds), self.0.rows.overlay + EFFECTS / 2.0);
                }
                for (k, ease) in o.item.motion.iter().map(|k| (k.t, k.ease)).filter(|&(k, _)| inside(o, k)) {
                    let mark = key_mark(t.x(o.item.at + k - o.item.start, bounds), self.motion_top() + KEYS / 2.0, ease);
                    frame.fill(&mark, colour);
                    frame.stroke(&mark, Stroke::default().with_width(1.5).with_color(palette.background.base.color));
                }
            }
        }
        let at = t.x(t.position, bounds);
        frame.fill(&Path::rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, self.height())), palette.primary.strong.color);
        draw_head_ground(&mut frame, theme, self.height());
        head_cell(&mut frame, 0.0, self.height(), colour);
        lane_tag(&mut frame, &self.tag_name(name), colour, top + h / 2.0);
        if !self.blocks().is_empty() {
            let (effects, motion) = (self.lanes(false), self.lanes(true));
            head_rows(&mut frame, theme, (self.0.rows.overlay, &effects.tag()), Some((self.motion_top(), &motion.tag())));
            effects.draw_heads(&mut frame, theme);
            motion.draw_heads(&mut frame, theme);
        }
        // What the drag has done so far, by the pointer.
        let said = match *drag {
            OverlayDrag::Trim { item, start, from, .. } => self.blocks().get(item).map(|o| {
                let edge = if start { o.item.start } else { o.item.start + o.len() };
                format!("{} · {}", signed(edge - from), seconds(o.len()))
            }),
            OverlayDrag::Move { item, at, .. } => self.blocks().get(item).map(|o| format!("{} · at {}", signed(o.item.at - at), dailix_sheet::clock(o.item.at, 1))),
            _ => None,
        };
        if let (Some(p), Some(said)) = (cursor.position_in(bounds), said) {
            readout(&mut frame, bounds, p, &said);
        }
        draw_snap_line(&mut frame, t, bounds, bounds.height);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, (drag, _): &(OverlayDrag, u64), bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        match drag {
            OverlayDrag::Trim { .. } => mouse::Interaction::ResizingHorizontally,
            OverlayDrag::Move { .. } | OverlayDrag::Key { .. } | OverlayDrag::Auto(..) => mouse::Interaction::Grabbing,
            OverlayDrag::None => cursor.position_in(bounds).map_or_else(mouse::Interaction::default, |p| self.hit(p, bounds).pointer()),
        }
    }
}

struct Pictures(Arc<Timeline>);

impl canvas::Program<Message> for Pictures {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        for (i, c) in self.0.blocks.iter().enumerate() {
            let (l, r) = self.0.block_x(c, bounds);
            let block = Path::rounded_rectangle(Point::new(l, 0.0), Size::new((r - l).max(1.0), self.0.rows.cuts), 4.0.into());
            frame.fill(&block, if Some(i) == self.0.picked.cut() { palette.primary.weak.color } else { palette.background.strong.color });
            // Thumbnails side by side, each showing the moment at its middle.
            let (th, inset) = (self.0.rows.cuts - 4.0, 2.0);
            let tw = th * thumbs::SIZE.0 as f32 / thumbs::SIZE.1 as f32;
            if c.thumbs.is_empty() || r - l <= 2.0 * inset {
                continue;
            }
            // Within the widget too: iced clips a canvas's shapes to it but
            // not its images, and zoomed in the tiles at the ends ran on into
            // the window's margins.
            let (cl, cr) = ((l + inset).max(0.0), (r - inset).min(bounds.width));
            if cr <= cl {
                continue;
            }
            let clip = Rectangle::new(Point::new(cl, inset), Size::new(cr - cl, th));
            let per_px = c.len() / (r - l) as f64;
            frame.with_clip(clip, |f| {
                // From the first tile in sight: zoomed in, a block can be
                // far wider than the widget.
                let mut x = l + inset + ((-(l + inset)) / tw).floor().max(0.0) * tw;
                while x < clip.x + clip.width {
                    let t = c.cut.start + (x + tw / 2.0 - l) as f64 * per_px;
                    if let Some(h) = c.thumb_at(t) {
                        f.draw_image(Rectangle::new(Point::new(x, inset), Size::new(tw, th)), canvas::Image::new(h.clone()));
                    }
                    x += tw;
                }
            });
        }
        vec![frame.into_geometry()]
    }
}

/// A lane's name, pinned at the timeline's left edge over whatever is there,
/// on a dark tag with a dot of the lane's colour: the name says what the
/// lane is, the colour only helps (never colour alone).
fn lane_tag(frame: &mut Frame, name: &str, colour: iced::Color, mid: f32) {
    let (x, h) = (PICK + 2.0, 15.0);
    let w = name.chars().count() as f32 * 6.0 + 22.0;
    frame.fill(&Path::rounded_rectangle(Point::new(x, mid - h / 2.0), Size::new(w, h), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7));
    frame.fill(&Path::circle(Point::new(x + 8.0, mid), 3.5), colour);
    frame.fill_text(Text {
        content: name.to_owned(),
        position: Point::new(x + 15.0, mid),
        color: iced::Color::WHITE,
        size: 10.5.into(),
        align_y: Vertical::Center,
        ..Default::default()
    });
}

/// A signed change of time: «+0:01.2», «−0:00.4».
fn signed(d: f64) -> String {
    format!("{}{}", if d < -0.05 { '−' } else { '+' }, dailix_sheet::clock(d.abs(), 1))
}

/// A length: «4.3 s», or «1:02.5» from a minute.
fn seconds(s: f64) -> String {
    if s < 60.0 { format!("{s:.1} s") } else { dailix_sheet::clock(s, 1) }
}

/// `said` on a dark tag beside the pointer `p`, kept inside `b`: what a
/// drag has done so far (pack 1).
fn readout(frame: &mut Frame, b: Rectangle, p: Point, said: &str) {
    let w = said.chars().count() as f32 * 6.6 + 12.0;
    let x = (p.x + 14.0).min(b.width - w - 2.0).max(2.0);
    let y = (p.y - 26.0).max(2.0);
    frame.fill(&Path::rounded_rectangle(Point::new(x, y), Size::new(w, 18.0), 4.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8));
    frame.fill_text(Text { content: said.to_owned(), position: Point::new(x + 6.0, y + 9.0), color: iced::Color::WHITE, size: 12.0.into(), font: iced::Font::MONOSPACE, align_y: Vertical::Center, ..Default::default() });
}

/// A lane's ground under its blocks, from the edit's start to `end`
/// (edit time).
fn lane_ground(frame: &mut Frame, t: &Timeline, b: Rectangle, (top, h): (f32, f32), end: f64, radius: f32, colour: iced::Color) {
    let x0 = t.x(0.0, b);
    frame.fill(&Path::rounded_rectangle(Point::new(x0, top), Size::new((t.x(end, b) - x0).max(0.0), h), radius.into()), colour);
}

/// A folded lane: where its blocks are (`spans`: edit time, length), thin
/// strips about `mid`.
fn folded_strips(frame: &mut Frame, t: &Timeline, b: Rectangle, mid: f32, spans: impl IntoIterator<Item = (f64, f64)>, colour: iced::Color) {
    for (from, len) in spans {
        let (l, r) = (t.x(from, b), t.x(from + len, b));
        frame.fill(&Path::rounded_rectangle(Point::new(l, mid - 3.0), Size::new((r - l).max(2.0), 6.0), 2.0.into()), colour.scale_alpha(0.7));
    }
}

/// A block of a coloured lane (a sound clip, a text or picture): full
/// colour when selected.
fn coloured_block(frame: &mut Frame, (l, r): (f32, f32), (top, h): (f32, f32), colour: iced::Color, selected: bool) {
    frame.fill(&block_path((l, r), top, h), if selected { colour } else { colour.scale_alpha(0.55) });
}

fn block_path((l, r): (f32, f32), top: f32, h: f32) -> Path {
    Path::rounded_rectangle(Point::new(l, top), Size::new((r - l).max(1.0), h), 4.0.into())
}

/// The selected block's frame and trim handles, on any lane.
fn selected_block(frame: &mut Frame, theme: &Theme, (l, r): (f32, f32), (top, h): (f32, f32), more: (bool, bool)) {
    selection_frame(frame, &block_path((l, r), top, h), theme);
    trim_handles(frame, (l, r), top, h, more);
}

/// Where two blocks overlap (`l`..`r`): shaded, and with `cross` an X
/// across it, as editors draw a crossfade.
fn crossfade(frame: &mut Frame, (l, r): (f32, f32), (top, h): (f32, f32), cross: bool) {
    frame.fill(&Path::rectangle(Point::new(l, top), Size::new(r - l, h)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35));
    if cross {
        let x = Path::new(|p| {
            p.move_to(Point::new(l, top + h));
            p.line_to(Point::new(r, top));
            p.move_to(Point::new(l, top));
            p.line_to(Point::new(r, top + h));
        });
        frame.stroke(&x, Stroke::default().with_width(1.5).with_color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.85)));
    }
}

impl Timeline {
    /// The transitions lane's zones, left to right edges in pixels: each
    /// join's overlap and the edge looks on either side of a join (`side`
    /// as `Hit::Zone`), with the cut it's on.
    fn zones(&self, b: Rectangle) -> Vec<(usize, usize, Option<End>, f32, f32)> {
        let n = self.blocks.len();
        let mut zones = Vec::new();
        for (i, c) in self.blocks.iter().enumerate() {
            let (from, end) = (c.from, c.from + c.len());
            if i > 0 && c.cut.crossfade > 0.0 {
                zones.push((i, i, None, self.x(from, b), self.x(from + c.cut.crossfade, b)));
            }
            if let Some(e) = c.cut.edge_in.as_ref().filter(|_| i > 0) {
                zones.push((i, i, Some(End::In), self.x(from, b), self.x(from + e.seconds.min(c.len()), b)));
            }
            if let Some(e) = c.cut.edge_out.as_ref().filter(|_| i + 1 < n) {
                zones.push((i + 1, i, Some(End::Out), self.x(end - e.seconds.min(c.len()), b), self.x(end, b)));
            }
        }
        zones
    }

    /// What a press at `x` on the transitions lane meets: the edge that
    /// sets a zone's length (an overlap's start, an edge look's inner
    /// side), the zone, or empty lane.
    fn zone_hit(&self, x: f32, b: Rectangle) -> Hit {
        let zones = self.zones(b);
        let grip = |at: f32| (x - at).abs() <= 5.0;
        for &(_, cut, side, l, r) in &zones {
            if grip(if side == Some(End::In) { r } else { l }) {
                return Hit::ZoneEdge { cut, side };
            }
        }
        zones.iter().find(|z| x >= z.3 && x <= z.4).map_or(Hit::ZoneDraw, |&(join, _, side, ..)| Hit::Zone { join, side })
    }

    /// The lane's name: «▾ Transitions» open, «▸ Transitions (3)» folded.
    fn trans_tag(&self) -> String {
        if self.folds.transitions { format!("▸ Transitions ({})", self.zones(Rectangle::default()).len()) } else { "▾ Transitions".to_owned() }
    }

    /// The zones on the lane, the join picked framed, and a rectangle
    /// being drawn.
    fn draw_zones(&self, frame: &mut Frame, theme: &Theme, b: Rectangle, drag: &Drag) {
        let (top, h) = (self.rows.trans_top + 2.0, self.rows.trans - 4.0);
        for (join, cut, side, l, r) in self.zones(b) {
            let shape = Path::rectangle(Point::new(l, top), Size::new(r - l, h));
            frame.fill(&shape, TRANSITION.scale_alpha(if side.is_none() { 0.55 } else { 0.3 }));
            frame.stroke(&shape, Stroke::default().with_width(1.0).with_color(TRANSITION));
            if side.is_none() && self.picked.join() == Some(join) {
                selection_frame(frame, &block_path((l, r), top, h), theme);
            }
            // Its look's name, where it fits and the lane's open.
            let c = &self.blocks[cut].cut;
            let look = match side {
                None => c.transition.map_or("Dissolve", |l| l.name()),
                Some(End::In) => c.edge_in.as_ref().map_or("", |e| e.look.name()),
                Some(End::Out) => c.edge_out.as_ref().map_or("", |e| e.look.name()),
            };
            if !self.folds.transitions && r - l > look.chars().count() as f32 * 6.0 + 8.0 {
                frame.fill_text(Text { content: look.to_owned(), position: Point::new(l + 4.0, top + h / 2.0), color: iced::Color::WHITE, size: 11.0.into(), align_y: Vertical::Center, ..Default::default() });
            }
        }
        if let Drag::Zone { x0, x1 } = *drag {
            let shape = Path::rectangle(Point::new(x0.min(x1), top), Size::new((x1 - x0).abs(), h));
            frame.stroke(&shape, Stroke::default().with_width(1.5).with_color(TRANSITION));
        }
    }
}

/// Transitions' colour (`TRANSITIONS.md`: their zones orange).
const TRANSITION: iced::Color = iced::Color::from_rgb(1.0, 0.6, 0.15);

/// An edge look (gap 25b) from the cut's edge at `edge` to `inner`: a
/// wedge as high as the lane at the edge, none inside, as a fade reads.
fn edge_wedge(frame: &mut Frame, edge: f32, inner: f32, h: f32) {
    let wedge = Path::new(|p| {
        p.move_to(Point::new(edge, 0.0));
        p.line_to(Point::new(edge, h));
        p.line_to(Point::new(inner, h));
        p.close();
    });
    frame.fill(&wedge, TRANSITION.scale_alpha(0.4));
    let slope = Path::line(Point::new(edge, 0.0), Point::new(inner, h));
    frame.stroke(&slope, Stroke::default().with_width(1.5).with_color(iced::Color::from_rgba(1.0, 0.7, 0.3, 0.9)));
}

/// An effect keyframe: a small square at (`x`, `mid`).
fn effect_mark(frame: &mut Frame, x: f32, mid: f32) {
    let mark = Path::rectangle(Point::new(x - 3.5, mid - 3.5), Size::new(7.0, 7.0));
    frame.fill(&mark, EFFECT_MARK);
    frame.stroke(&mark, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
}

/// A block's name on a dark tag from (`x`, `top`), `h` high, so it reads
/// over a picture or a waveform; `size` the text's, `per_char` about how
/// wide a character is.
fn name_tag(frame: &mut Frame, (x, top): (f32, f32), h: f32, name: &str, size: f32, per_char: f32) {
    frame.fill(&Path::rounded_rectangle(Point::new(x, top), Size::new(name.chars().count() as f32 * per_char + 8.0, h), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6));
    frame.fill_text(Text {
        content: name.to_owned(),
        position: Point::new(x + 4.0, top + h / 2.0),
        color: iced::Color::WHITE,
        size: size.into(),
        align_y: Vertical::Center,
        ..Default::default()
    });
}

/// The selected clip's frame: thick in the accent colour, over a dark
/// line so it stands off any picture or waveform (gap 7; WCAG 1.4.11's
/// 3:1 against both). It was 2 px and easy to miss.
fn selection_frame(frame: &mut Frame, block: &Path, theme: &Theme) {
    frame.stroke(block, Stroke::default().with_width(5.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
    frame.stroke(block, Stroke::default().with_width(3.0).with_color(theme.extended_palette().primary.strong.color));
}

/// Trim handles at a selected clip's ends (`l`, `r`), only there, so a stray
/// drag on another clip moves it rather than trims it (iMovie, InShot).
/// Yellow where there's footage beyond that edge (`more`: in, out), red at
/// the file's end (FCP), each on a dark edge to show on any picture.
fn trim_handles(frame: &mut Frame, (l, r): (f32, f32), top: f32, h: f32, more: (bool, bool)) {
    const MORE: iced::Color = iced::Color::from_rgb(0.98, 0.80, 0.18);
    const END: iced::Color = iced::Color::from_rgb(0.92, 0.30, 0.30);
    for (x, more) in [(l + 5.0, more.0), (r - 5.0, more.1)] {
        frame.fill(&Path::rounded_rectangle(Point::new(x - 3.0, top + 5.0), Size::new(6.0, h - 10.0), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        frame.fill(&Path::rounded_rectangle(Point::new(x - 2.0, top + 6.0), Size::new(4.0, h - 12.0), 2.0.into()), if more { MORE } else { END });
    }
}

/// Whether `x` takes the in edge (`Some(true)`) or out edge of a block
/// drawn from `l` to `r`, or its body (`None`): `EDGE` inside each end,
/// and anything outside that reaches it.
/// A block narrower than three edges is split in thirds, so its middle
/// still moves it; it used to be all edge, and couldn't be moved.
fn edge_of(l: f32, r: f32, x: f32) -> Option<bool> {
    let reach = if r - l < 3.0 * EDGE { (r - l) / 3.0 } else { EDGE };
    if x - l <= reach {
        Some(true)
    } else if r - x <= reach {
        Some(false)
    } else {
        None
    }
}

/// A volume line on `lane` from `ends.0` to `ends.1` (x, dB) through
/// `points` (x, dB; NaN x for one not shown), the `selected` one labelled
/// with its level.
fn draw_line(frame: &mut Frame, b: Rectangle, lane: Lane, colour: iced::Color, ends: ((f32, f64), (f32, f64)), points: &[(f32, f64)], selected: Option<usize>) {
    let y = |db| Timeline::volume_y(lane, db);
    let line = Path::new(|p| {
        p.move_to(Point::new(ends.0.0, y(ends.0.1)));
        for &(x, db) in points.iter().filter(|p| !p.0.is_nan()) {
            p.line_to(Point::new(x, y(db)));
        }
        p.line_to(Point::new(ends.1.0, y(ends.1.1)));
    });
    frame.stroke(&line, Stroke::default().with_width(1.5).with_color(colour));
    for (i, &(x, db)) in points.iter().enumerate() {
        if x.is_nan() {
            continue;
        }
        let at = Point::new(x, y(db));
        let chosen = Some(i) == selected;
        let dot = Path::circle(at, if chosen { POINT + 1.0 } else { POINT });
        frame.fill(&dot, if chosen { iced::Color::WHITE } else { colour });
        frame.stroke(&dot, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6)));
        if chosen {
            let label = if db <= VOLUME_BOTTOM { "off".to_owned() } else { format!("{db:+.1} dB") };
            let (w, h) = (label.len() as f32 * 6.0 + 8.0, 14.0);
            let left = if at.x + 8.0 + w > b.width { at.x - 8.0 - w } else { at.x + 8.0 };
            let top = (at.y - h / 2.0).clamp(lane.0 - 4.0, lane.0 + lane.1 + 4.0 - h);
            frame.fill(&Path::rounded_rectangle(Point::new(left, top), Size::new(w, h), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7));
            frame.fill_text(Text {
                content: label,
                position: Point::new(left + 4.0, top + h / 2.0),
                color: iced::Color::WHITE,
                size: 11.0.into(),
                align_y: Vertical::Center,
                ..Default::default()
            });
        }
    }
}

/// The two keyframe rows under a track from `top`: faint bands over the
/// edit's length (`t`), like the lanes, not the widget's whole width;
/// effects above, `keys` (view or motion) below.
fn draw_rows(frame: &mut Frame, theme: &Theme, t: &Timeline, b: Rectangle, top: f32, keys: Option<f32>) {
    let palette = theme.extended_palette();
    let (l, r) = (t.x(0.0, b).max(0.0), t.x(t.duration, b).min(b.width));
    let rows = [Some((top, EFFECTS)), keys.map(|k| (k, KEYS))];
    for (y, h) in rows.into_iter().flatten() {
        frame.fill(&Path::rectangle(Point::new(l, y + 1.0), Size::new((r - l).max(0.0), h - 2.0)), palette.background.weak.color.scale_alpha(0.25));
    }
}

/// The header column's ground, over whatever lies under it as the lanes
/// scroll: the canvas's full height, a rule at its right.
fn draw_head_ground(frame: &mut Frame, theme: &Theme, h: f32) {
    let palette = theme.extended_palette();
    frame.fill(&Path::rectangle(Point::ORIGIN, Size::new(HEAD, h)), palette.background.base.color);
    frame.fill(&Path::rectangle(Point::new(HEAD - 1.0, 0.0), Size::new(1.0, h)), palette.background.strong.color);
}

/// A lane's cell in the header column, `top` to `top + h`: a light ground
/// in the lane's colour and a rule under it, so each lane's header reads
/// as its own (user, 2026-11-04).
fn head_cell(frame: &mut Frame, top: f32, h: f32, colour: iced::Color) {
    frame.fill(&Path::rectangle(Point::new(0.0, top + 1.0), Size::new(HEAD - 1.0, (h - 2.0).max(0.0))), colour.scale_alpha(0.13));
    frame.fill(&Path::rectangle(Point::new(0.0, top + h - 1.0), Size::new(HEAD - 1.0, 1.0)), iced::Color::from_rgba(1.0, 1.0, 1.0, 0.12));
}

/// The keyframe rows' names (`draw_rows`'s), in the header column: the
/// effects row's (`effects`) at `top`, the keys' row at its own top.
fn head_rows(frame: &mut Frame, theme: &Theme, (top, effects): (f32, &str), keys: Option<(f32, &str)>) {
    let ink = theme.extended_palette().background.base.text.scale_alpha(0.45);
    for (y, h, name) in [Some((top, EFFECTS, effects)), keys.map(|(k, name)| (k, KEYS, name))].into_iter().flatten() {
        frame.fill_text(Text { content: name.to_owned(), position: Point::new(PICK + 14.0, y + h / 2.0), color: ink, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
    }
}

/// Where a lane's name sits in the header: the middle of a short lane; in
/// a tall one a little up, its level under it.
fn tag_mid(top: f32, h: f32) -> f32 {
    if h >= 34.0 { top + h / 2.0 - 6.0 } else { top + h / 2.0 }
}

/// A lane's own level under its name in the header, when it isn't 0 dB
/// and the lane has room: so a clip reading «0 dB» on a lane at −6 dB
/// isn't a puzzle (the user, 2026-11-02).
fn head_level(frame: &mut Frame, top: f32, h: f32, db: f64) {
    if h < 34.0 || db.abs() < 0.05 {
        return;
    }
    let label = if db <= crate::project::OFF_DB { "lane off".to_owned() } else { format!("lane {db:+.1} dB").replace('-', "−") };
    let colour = if db < 0.0 { iced::Color::from_rgb(0.98, 0.80, 0.18) } else { iced::Color::from_rgb(1.0, 0.55, 0.3) };
    frame.fill_text(Text { content: label, position: Point::new(PICK + 4.0, tag_mid(top, h) + 15.0), color: colour, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
}

/// A keyframe mark at (`x`, `y`) shaped by its ease: smooth a diamond,
/// linear a circle, hold a square.
fn key_mark(x: f32, y: f32, ease: Ease) -> Path {
    match ease {
        Ease::Smooth => Path::new(|p| {
            p.move_to(Point::new(x, y - DIAMOND));
            p.line_to(Point::new(x + DIAMOND, y));
            p.line_to(Point::new(x, y + DIAMOND));
            p.line_to(Point::new(x - DIAMOND, y));
            p.close();
        }),
        Ease::Linear => Path::circle(Point::new(x, y), DIAMOND * 0.8),
        Ease::Hold => Path::rectangle(Point::new(x - DIAMOND * 0.7, y - DIAMOND * 0.7), Size::new(DIAMOND * 1.4, DIAMOND * 1.4)),
    }
}

struct Marks(Arc<Timeline>);

impl std::ops::Deref for Marks {
    type Target = Timeline;

    fn deref(&self) -> &Timeline {
        &self.0
    }
}

impl Timeline {
    /// A right-click: on a cut, what file it is; on a cut's sound or a
    /// sound clip, its menu where the pointer is (Info is in it); on a
    /// sound lane's empty stretch, the track's menu.
    fn right_press(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use super::sound_ui::{Menu, SoundMsg};
        let p = cursor.position_in(bounds)?;
        if let Some(message) = Some(p).filter(|p| p.x >= HEAD).and_then(|p| self.cut_lanes().right(p, bounds)) {
            return Some(canvas::Action::publish(message).and_capture());
        }
        // A lane's header first, then a point or a line: each has a
        // menu of its own.
        let menu = if p.x < HEAD {
            self.head_menu(p)?
        } else if let Some((own, i)) = self.sound_at(p, bounds) {
            Menu::Point(own, i)
        } else if let Some(own) = self.on_sound_line(p, bounds) {
            Menu::Line(own)
        } else if self.rows.on_clip_sound(p.y) {
            Menu::Own(Own::Cut(self.block_at(p.x, bounds)?))
        } else if let Some(lane) = self.rows.sound_at_y(p.y) {
            let which = self.sounds[lane].which;
            self.music_at(lane, p.x, bounds).map_or(Menu::Track(which), |i| Menu::Own(Own::Clip(which, i)))
        } else if p.y < self.rows.effects_top {
            return Some(canvas::Action::publish(Message::Info(super::info::Of::Cut(self.block_at(p.x, bounds)?))).and_capture());
        } else {
            return None;
        };
        Some(canvas::Action::publish(Message::Sound(SoundMsg::Menu(Some((menu, cursor.position()?))))).and_capture())
    }

    /// A left press: what's under it (`hit`) acted on, and the drag it
    /// starts kept in `state`.
    fn press(&self, state: &mut State, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        let p = cursor.position_in(bounds)?;
        let drag = &mut state.drag;
        let per_px = self.span / Timeline::width(bounds) as f64;
        let message = match self.hit(p, bounds, state.ctrl, state.shift) {
            Hit::Nothing => return None,
            Hit::Scroll => {
                // On the thumb, drag it; elsewhere on the bar, centre it there first.
                let (l, r) = self.scroll_thumb(bounds);
                let per_px = self.duration / Timeline::width(bounds) as f64;
                let start = if p.x >= l && p.x <= r { self.start } else { self.start + (p.x - (l + r) / 2.0) as f64 * per_px };
                *drag = Drag::Scroll { x0: p.x, start };
                Message::TimelinePan(start)
            }
            Hit::Listen(lane, solo) => Message::Listen(lane, solo),
            Hit::Fold(lane, folded) => Message::Fold(lane, folded),
            // A lane's header: its own settings in one place.
            Hit::Head => Message::Sound(super::sound_ui::SoundMsg::Menu(Some((self.head_menu(p)?, cursor.position()?)))),
            Hit::Effect { cut, key, t } => {
                // A jump there, and a drag retimes it.
                *drag = Drag::Effect { cut, key };
                Message::Effect(super::effects::EffectMsg::Jump(cut, t))
            }
            Hit::CutFade { cut, in_edge } => {
                let c = &self.blocks[cut];
                *drag = Drag::Fade { own: Own::Cut(cut), in_edge, x0: p.x, from: if in_edge { c.cut.sound_fade_in } else { c.cut.sound_fade_out }, per_px };
                Message::Select(cut)
            }
            Hit::SoundEdge { cut, start } => {
                // Its sound alone runs on past the picture or stops
                // short (a J- or L-cut).
                let c = &self.blocks[cut];
                let from = if start { c.cut.start - c.cut.sound_before } else { c.cut.start + c.len() + c.cut.sound_after };
                *drag = Drag::SoundEdge { cut, start, x0: p.x, from, per_px };
                Message::Select(cut)
            }
            Hit::Point { own, key, delete: true } => Message::Sound(super::sound_ui::SoundMsg::DeletePoint(own, key)),
            Hit::Point { own, key, .. } => {
                *drag = Drag::Point { own, key };
                Message::PointSelect(own, key)
            }
            // The clip's level knob.
            Hit::LevelTag(own) => self.tag_press(drag, &mut state.tag_click, own, p)?,
            // A sound point where clicked, dragged on.
            Hit::NewPoint(own) => self.add_point(drag, own, p, bounds)?,
            Hit::CutSound(cut) => Message::Select(cut),
            Hit::CutEdge { cut, crossfade: true, .. } => {
                // How far it overlaps the cut before.
                *drag = Drag::Crossfade { cut, x0: p.x, from: self.blocks[cut].cut.crossfade, per_px };
                Message::Select(cut)
            }
            Hit::CutEdge { cut, start, .. } => {
                let c = &self.blocks[cut];
                *drag = Drag::Trim { cut, start, x0: p.x, from: if start { c.cut.start } else { c.cut.start + c.len() }, per_px };
                Message::Select(cut)
            }
            Hit::Cut(cut) => {
                *drag = Drag::Move { cut, x0: p.x, moved: false, to: cut };
                Message::Select(cut)
            }
            Hit::Join(cut) => Message::SelectJoin(cut),
            Hit::Zone { join, side } => Message::TransitionZone(join, side),
            Hit::ZoneEdge { cut, side: None } => {
                *drag = Drag::Crossfade { cut, x0: p.x, from: self.blocks[cut].cut.crossfade, per_px };
                Message::TransitionZone(cut, None)
            }
            Hit::ZoneEdge { cut, side: Some(end) } => {
                let c = &self.blocks[cut].cut;
                let from = if end == End::In { c.edge_in.as_ref() } else { c.edge_out.as_ref() }.map_or(0.0, |e| e.seconds);
                *drag = Drag::EdgeLength { cut, end, x0: p.x, from, per_px };
                Message::TransitionZone(if end == End::Out { cut + 1 } else { cut }, Some(end))
            }
            Hit::Auto(h) => match self.cut_lanes().press(h, p, state.ctrl) {
                Ok(d) => {
                    *drag = Drag::Auto(d);
                    return Some(canvas::Action::request_redraw().and_capture());
                }
                Err(message) => message,
            },
            Hit::ZoneDraw => {
                *drag = Drag::Zone { x0: p.x, x1: p.x };
                return Some(canvas::Action::request_redraw().and_capture());
            }
            Hit::ClipFade { lane, clip, in_edge } => {
                let m = &self.sounds[lane].blocks[clip];
                *drag = Drag::Fade { own: Own::Clip(self.sounds[lane].which, clip), in_edge, x0: p.x, from: if in_edge { m.clip.fade_in } else { m.clip.fade_out }, per_px };
                Message::SelectMusic(self.sounds[lane].which, clip)
            }
            Hit::Clip { lane, clip, edge } => {
                let m = &self.sounds[lane].blocks[clip];
                *drag = match edge {
                    Some(start) => Drag::MusicTrim { lane, clip, start, x0: p.x, from: if start { m.clip.start } else { m.clip.start + m.len() }, per_px },
                    None => Drag::MusicMove { lane, clip, x0: p.x, at: m.clip.at, per_px },
                };
                Message::SelectMusic(self.sounds[lane].which, clip)
            }
            Hit::Key { cut, key } => {
                *drag = Drag::Key { cut, key };
                Message::KeyJump(cut, key)
            }
            Hit::Playhead => {
                *drag = Drag::Playhead;
                Message::Scrub(self.t(p.x, bounds))
            }
        };
        Some(canvas::Action::publish(message).and_capture())
    }

    /// The pointer moved during a drag: what the drag does at `position`.
    fn drag_to(&self, state: &mut State, position: Point, bounds: Rectangle) -> Option<canvas::Action<Message>> {
        let x = position.x - bounds.x;
        let drag = &mut state.drag;
        let message = match drag {
            Drag::None => return None,
            Drag::Playhead => {
                let t = self.t(x, bounds);
                snapped(self.snap(&[t], self.span / Timeline::width(bounds) as f64, |of| of == Of::Playhead), |by| Message::Scrub(t + by))
            }
            Drag::Key { cut, key } => {
                let c = &self.blocks[*cut];
                let edit_t = self.t(x, bounds).clamp(c.from, c.from + c.len());
                Message::KeyMove(*cut, *key, c.cut.start + edit_t - c.from)
            }
            Drag::Effect { cut, key } => {
                let c = &self.blocks[*cut];
                let edit_t = self.t(x, bounds).clamp(c.from, c.from + c.len());
                Message::Effect(super::effects::EffectMsg::KeyMove(*cut, *key, c.cut.start + edit_t - c.from))
            }
            Drag::Trim { cut, start, x0, from, per_px } => {
                let t = *from + (x - *x0) as f64 * *per_px;
                // Its out edge snaps; what comes after it moves with
                // it. Its in edge stays put on screen as it trims.
                let c = &self.blocks[*cut];
                let at = (!*start).then(|| self.snap(&[c.from + t - c.cut.start], *per_px, |of| matches!(of, Of::Cut(j) | Of::CutSound(j) | Of::Mark(j) if j >= *cut))).flatten();
                let (cut, start) = (*cut, *start);
                snapped(at, |by| Message::Trim(cut, start, t + by))
            }
            Drag::SoundEdge { cut, start, x0, from, per_px } => {
                // Onto a join (its own picture's too, back home) or a
                // neighbour's sound edge.
                let t = *from + (x - *x0) as f64 * *per_px;
                let c = &self.blocks[*cut];
                let at = self.snap(&[c.from + t - c.cut.start], *per_px, |of| of == Of::CutSound(*cut));
                let (cut, start) = (*cut, *start);
                snapped(at, |by| Message::SoundEdge(cut, start, t + by))
            }
            Drag::Move { x0, moved, to, .. } => {
                *moved |= (x - *x0).abs() > DRAG_FROM;
                *to = self.drop_at(x, bounds);
                return Some(canvas::Action::request_redraw().and_capture());
            }
            Drag::Scroll { x0, start } => Message::TimelinePan(*start + (x - *x0) as f64 * self.duration / Timeline::width(bounds) as f64),
            Drag::MusicMove { lane, clip, x0, at, per_px } => {
                let at = *at + (x - *x0) as f64 * *per_px;
                let len = self.sounds[*lane].blocks[*clip].len();
                let (lane, clip, overlap) = (*lane, *clip, state.shift);
                let to = self.snap(&[at, at + len], *per_px, |of| of == Of::Sound(lane, clip));
                snapped(to, |by| Message::MusicMove(self.sounds[lane].which, clip, at + by, overlap))
            }
            Drag::Crossfade { cut, x0, from, per_px } => Message::Crossfade(*cut, *from + (*x0 - x) as f64 * *per_px),
            Drag::Zone { x1, .. } => {
                *x1 = x;
                return Some(canvas::Action::request_redraw().and_capture());
            }
            Drag::Auto(d) => {
                let message = self.cut_lanes().drag(d, Point::new(x, position.y - bounds.y), bounds, state.shift);
                return Some(message.map_or_else(canvas::Action::request_redraw, canvas::Action::publish).and_capture());
            }
            // Inward lengthens: rightwards on a start, leftwards on an end.
            Drag::EdgeLength { cut, end, x0, from, per_px } => {
                let dx = (x - *x0) as f64 * *per_px;
                Message::EdgeSeconds(*cut, *end, *from + if *end == End::In { dx } else { -dx })
            }
            Drag::Fade { own, in_edge, x0, from, per_px } => {
                // Rightwards lengthens a fade in, leftwards a fade out.
                let dx = (x - *x0) as f64 * *per_px;
                Message::Fade(*own, *in_edge, *from + if *in_edge { dx } else { -dx })
            }
            Drag::Point { own, key } => {
                let (t, db) = self.own_point(*own, Point::new(x, position.y - bounds.y), bounds)?;
                Message::PointMove(*own, *key, t, db)
            }
            Drag::Level { own, y0, from } => {
                // Up is louder: 0.25 dB a pixel in 0.5 dB steps,
                // Shift a fifth of that in 0.1 dB steps.
                let (per_px, step) = if state.shift { (0.05, 0.1) } else { (0.25, 0.5) };
                let db = *from + ((*y0 - (position.y - bounds.y)) as f64 * per_px / step).round() * step;
                Message::Level(*own, db)
            }
            Drag::MusicTrim { lane, clip, start, x0, from, per_px } => {
                // File time to edit time is fixed while trimming:
                // either edge moves, the sound stays put.
                let m = &self.sounds[*lane].blocks[*clip];
                let t = *from + (x - *x0) as f64 * *per_px;
                let to_edit = m.clip.at - m.clip.start;
                let (lane, clip, start, overlap) = (*lane, *clip, *start, state.shift);
                let to = self.snap(&[t + to_edit], *per_px, |of| of == Of::Sound(lane, clip));
                snapped(to, |by| Message::MusicTrim(self.sounds[lane].which, clip, start, t + by, overlap))
            }
        };
        Some(canvas::Action::publish(message).and_capture())
    }
}

/// The left button let go: a moved cut lands, any other drag ends.
fn release(drag: Drag) -> Option<canvas::Action<Message>> {
    match drag {
        Drag::None => None,
        Drag::Move { cut, moved: true, to, .. } => Some(canvas::Action::publish(Message::MoveCut(cut, to)).and_capture()),
        Drag::Move { .. } | Drag::Scroll { .. } => Some(canvas::Action::request_redraw().and_capture()),
        // The drawn zone and the lanes' drags are let go in `Marks::update`,
        // which knows the times.
        Drag::Zone { .. } | Drag::Auto(_) => Some(canvas::Action::request_redraw().and_capture()),
        Drag::Playhead | Drag::Key { .. } | Drag::Effect { .. } | Drag::Trim { .. } | Drag::MusicMove { .. } | Drag::MusicTrim { .. } | Drag::Fade { .. } | Drag::Point { .. } | Drag::Level { .. } | Drag::Crossfade { .. } | Drag::SoundEdge { .. } | Drag::EdgeLength { .. } => {
            Some(canvas::Action::publish(Message::ScrubDone).and_capture())
        }
    }
}

impl canvas::Program<Message> for Marks {
    type State = State;

    fn update(&self, state: &mut State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::{Keyboard, Mouse};
        // Esc cancelled the drag in the app: nothing more of it, not even
        // its release (a cut's move lands then).
        if state.epoch != self.drag_epoch {
            state.epoch = self.drag_epoch;
            state.drag = Drag::None;
        }
        match event {
            Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                state.shift = m.shift();
                state.ctrl = m.command();
                None
            }
            Mouse(mouse::Event::WheelScrolled { delta }) => wheel(delta, self, cursor, bounds),
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => self.right_press(bounds, cursor),
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => self.press(state, bounds, cursor),
            Mouse(mouse::Event::CursorMoved { position }) => self.drag_to(state, *position, bounds),
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => match std::mem::take(&mut state.drag) {
                // A rectangle drawn on the transitions lane: a few pixels
                // at least, or it was a click.
                Drag::Zone { x0, x1 } if (x1 - x0).abs() > 4.0 => Some(canvas::Action::publish(Message::TransitionDraw(self.t(x0, bounds), self.t(x1, bounds))).and_capture()),
                Drag::Auto(d) => Some(self.cut_lanes().release(d, bounds).map_or_else(canvas::Action::request_redraw, canvas::Action::publish).and_capture()),
                drag => release(drag),
            },
            _ => None,
        }
    }

    fn draw(&self, state: &State, renderer: &Renderer, theme: &Theme, bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry> {
        let drag = &state.drag;
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        draw_rows(&mut frame, theme, self, bounds, self.rows.effects_top, (self.rows.keys > 0.0).then_some(self.rows.keys_top));
        self.cut_lanes().draw(&mut frame, theme, bounds, if let Drag::Auto(d) = drag { Some(d) } else { None });
        self.draw_zones(&mut frame, theme, bounds, drag);
        for (i, c) in self.blocks.iter().enumerate() {
            let (l, r) = self.block_x(c, bounds);
            if Some(i) == self.picked.cut() {
                selected_block(&mut frame, theme, (l, r), (0.0, self.rows.cuts), (c.cut.start > 1e-3, c.cut.start + c.len() < c.most - 1e-3));
            }
            // The longest label that fits (about 7 px a character).
            if let Some(label) = c.labels.iter().find(|s| r - l > s.chars().count() as f32 * 7.0 + 24.0) {
                name_tag(&mut frame, (l + 9.0, self.rows.cuts - 19.0), 15.0, label, 12.0, 5.9);
            }
        }
        // Effect keyframes: small squares along the bottom of their cut.
        for c in &self.blocks {
            for t in c.cut.effects.iter().filter(|_| c.cut.effects.len() > 1).map(|k| k.t).filter(|&t| t >= c.cut.start && t <= c.cut.start + c.len()) {
                effect_mark(&mut frame, self.x(c.from + t - c.cut.start, bounds), self.rows.effects_top + EFFECTS / 2.0);
            }
        }
        // Edge looks: an orange wedge, deepest at the cut's edge.
        for c in &self.blocks {
            let end = c.from + c.len();
            if let Some(e) = &c.cut.edge_in {
                edge_wedge(&mut frame, self.x(c.from, bounds), self.x(c.from + e.seconds.min(c.len()), bounds), self.rows.cuts);
            }
            if let Some(e) = &c.cut.edge_out {
                edge_wedge(&mut frame, self.x(end, bounds), self.x(end - e.seconds.min(c.len()), bounds), self.rows.cuts);
            }
        }
        // Crossfades: an X over each overlap, as on the music lane.
        for (i, c) in self.blocks.iter().enumerate().filter(|(_, c)| c.cut.crossfade > 0.0) {
            let (l, r) = (self.x(c.from, bounds), self.x(c.from + c.cut.crossfade, bounds));
            crossfade(&mut frame, (l, r), (0.0, self.rows.cuts), true);
            // The join picked: framed as a picked block is.
            if self.picked.join() == Some(i) {
                selection_frame(&mut frame, &block_path((l, r), 0.0, self.rows.cuts), theme);
            }
        }
        if let Drag::Move { moved: true, to, .. } = drag {
            let t = self.blocks.get(*to).map_or(self.duration, |c| c.from);
            let x = self.x(t, bounds);
            frame.fill(&Path::rectangle(Point::new(x - 1.5, 0.0), Size::new(3.0, self.rows.cuts)), palette.primary.strong.color);
        }

        let mid = self.rows.seek_top + SEEK / 2.0;
        let keys_mid = self.rows.keys_top + KEYS / 2.0;
        let (x0, x1) = (self.x(0.0, bounds), self.x(self.duration, bounds));
        // Where the picture's lanes end and the sound's begin: the sound
        // group's name, then a rule across the edit (gap 50d).
        let x0 = x0.max(HEAD + PICK);
        frame.fill(&Path::rectangle(Point::new(x0, mid - 0.5), Size::new((x1 - x0).max(0.0), 1.0)), palette.background.strong.color);
        let at = self.x(self.position, bounds);
        // The view's keyframes, while framing (their row is gone otherwise).
        for (ci, c) in self.blocks.iter().enumerate().filter(|_| self.rows.keys > 0.0) {
            for &(ki, t, ease, follows) in &c.keys {
                let (x, mid) = (self.x(c.from + t - c.cut.start, bounds), keys_mid);
                // Smooth is a diamond, linear a circle, hold a square.
                let mark = key_mark(x, mid, ease);
                let fill = if Some((ci, ki)) == self.current { palette.primary.strong.color } else { palette.background.base.text };
                frame.fill(&mark, fill);
                frame.stroke(&mark, Stroke::default().with_width(1.5).with_color(palette.background.base.color));
                // A keyframe that follows the stick wears a ring.
                if follows {
                    frame.stroke(&Path::circle(Point::new(x, mid), DIAMOND * 1.7), Stroke::default().with_width(1.5).with_color(fill));
                }
            }
        }
        frame.fill(&Path::rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, self.rows.seek_top)), palette.primary.strong.color);
        frame.fill(&Path::rectangle(Point::new(at - 1.0, self.rows.sounds_top), Size::new(2.0, self.rows.sounds_h())), palette.primary.strong.color);
        if self.zoomed() {
            let y = self.rows.height - SCROLL / 2.0 - SCROLL_BAR / 2.0;
            let pill = |l: f32, r: f32| Path::rounded_rectangle(Point::new(l, y), Size::new((r - l).max(SCROLL_BAR), SCROLL_BAR), (SCROLL_BAR / 2.0).into());
            frame.fill(&pill(HEAD + PICK, HEAD + PICK + Timeline::width(bounds)), palette.background.strong.color);
            let (l, r) = self.scroll_thumb(bounds);
            let active = matches!(drag, Drag::Scroll { .. });
            frame.fill(&pill(l, r), if active { palette.primary.strong.color } else { palette.background.base.text.scale_alpha(0.5) });
        }
        self.draw_heads(&mut frame, theme);
        if let (Some(p), Some(said)) = (cursor.position_in(bounds), self.readout(drag)) {
            readout(&mut frame, bounds, p, &said);
        }
        if let Some(note) = self.step_note {
            readout(&mut frame, bounds, Point::new(at - 14.0, 30.0), note);
        }
        draw_snap_line(&mut frame, self, bounds, self.rows.height);
        // The kept sound lanes under the rest: nothing else is drawn over
        // their stretch but the playhead, the headers and the readouts.
        let key = self.sound_key(bounds, theme);
        if state.sounds_key.replace(key) != key {
            state.sounds.clear();
        }
        let sounds = state.sounds.draw(renderer, bounds.size(), |frame| {
            self.draw_sounds(frame, theme, bounds);
            self.draw_clip_sound(frame, theme, bounds);
        });
        vec![sounds, frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        match state.drag {
            Drag::Trim { .. } | Drag::MusicTrim { .. } | Drag::Fade { .. } | Drag::Crossfade { .. } | Drag::SoundEdge { .. } => mouse::Interaction::ResizingHorizontally,
            Drag::Level { .. } => mouse::Interaction::ResizingVertically,
            Drag::None => cursor.position_in(bounds).map_or_else(mouse::Interaction::default, |p| self.hit(p, bounds, state.ctrl, state.shift).pointer()),
            _ => mouse::Interaction::Grabbing,
        }
    }
}

/// Where a clip's waveform goes: its block's sides, top and height, and
/// the stretch of the file it shows (in point and length, file time).
struct Wave {
    left: f32,
    right: f32,
    top: f32,
    height: f32,
    start: f64,
    len: f64,
}

impl Wave {
    /// Per two pixels, the loudest peak under them: `heard`'s peaks scaled
    /// by `gain` at that file time, drawn in `colour`; what goes over full
    /// scale (the limiter will hold it) in orange; and `file`'s own peaks
    /// faint behind, so a level change shows against what was there.
    fn draw(&self, frame: &mut Frame, b: Rectangle, file: &crate::waveform::Waveform, heard: &crate::waveform::Waveform, gain: impl Fn(f64) -> f32, colour: iced::Color) {
        let (l, r, h) = (self.left, self.right, self.height - 4.0);
        let per_px = self.len / (r - l).max(1.0) as f64;
        let mid = self.top + self.height / 2.0;
        let peak = |w: &crate::waveform::Waveform, t0: f64, t1: f64| {
            let (i0, i1) = ((t0 / crate::waveform::WAVE_STEP) as usize, ((t1 / crate::waveform::WAVE_STEP) as usize).max((t0 / crate::waveform::WAVE_STEP) as usize + 1));
            w.peaks.get(i0..i1.min(w.peaks.len())).map_or(0.0, |s| s.iter().fold(0.0f32, |a, &v| a.max(v)))
        };
        let (mut behind, mut bars, mut over) = (canvas::path::Builder::new(), canvas::path::Builder::new(), canvas::path::Builder::new());
        let mut x = l.max(0.0);
        while x < r.min(b.width) {
            let (t0, t1) = (self.start + (x - l) as f64 * per_px, self.start + (x + 2.0 - l) as f64 * per_px);
            let raw = peak(file, t0, t1);
            let bh = (raw * h).max(1.0);
            behind.rectangle(Point::new(x, mid - bh / 2.0), Size::new(1.5, bh));
            let loud = peak(heard, t0, t1) * gain((t0 + t1) / 2.0);
            let bh = (loud.min(1.0) * h).max(1.0);
            (if loud > 1.0 { &mut over } else { &mut bars }).rectangle(Point::new(x, mid - bh / 2.0), Size::new(1.5, bh));
            x += 2.0;
        }
        frame.fill(&behind.build(), colour.scale_alpha(0.2));
        frame.fill(&bars.build(), colour);
        frame.fill(&over.build(), OVER_COLOUR);
    }
}

/// Where a fade's knob is drawn and pressed: at the fade's end (`at`), but
/// at least `FADE_INSET` inside the clip's end (`l` for a fade in, `r` for
/// a fade out), so neighbours' knobs don't stack at a join.
fn fade_knob_x((l, r): (f32, f32), at: f32, fade_in: bool) -> f32 {
    if fade_in { at.max(l + FADE_INSET) } else { at.min(r - FADE_INSET) }
}

/// A clip's fades, `len` seconds long between `l` and `r`: shaded above a
/// line from its corner to where each fade ends, a handle there along the
/// top (a white knob, also where there's no fade yet, to grab one).
fn draw_fades(frame: &mut Frame, (l, r): (f32, f32), top: f32, h: f32, len: f64, (fade_in, fade_out): (f64, f64)) {
    for (fade, at_start) in [(fade_in, true), (fade_out, false)] {
        let w = (fade / (len.max(1e-9)) * (r - l) as f64) as f32;
        let (corner, end) = if at_start { (l, l + w) } else { (r, r - w) };
        if w > 0.5 {
            let shade = Path::new(|p| {
                p.move_to(Point::new(corner, top));
                p.line_to(Point::new(end, top));
                p.line_to(Point::new(corner, top + h));
                p.close();
            });
            frame.fill(&shade, iced::Color::from_rgba(0.0, 0.0, 0.0, 0.4));
            frame.stroke(&Path::line(Point::new(corner, top + h), Point::new(end, top)), Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.7)));
        }
        let knob = Path::circle(Point::new(fade_knob_x((l, r), end, at_start), top + 5.0), 4.5);
        frame.fill(&knob, iced::Color::WHITE);
        frame.stroke(&knob, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::VolumeKey;

    fn block(from: f64, len: f64, crossfade: f64) -> Block {
        let cut = crate::project::Cut { crossfade, ..crate::project::Cut::new("a.mp4".into(), 0.0, len) };
        Block { cut, labels: vec![], from, most: f64::INFINITY, keys: vec![], thumbs: vec![], wave: None, even_wave: None, level: "0 dB".into() }
    }

    /// Cuts a 0..4 and b 3..7 (crossfading 3..4), a 7..10; on Voice a clip
    /// 2..6 with a point; on a 1000 px window showing 0..10 s.
    fn timeline() -> (Timeline, Rectangle) {
        let mut sounds: Vec<SoundLane> = SoundTrack::ALL.iter().map(|&which| SoundLane { which, track: Default::default(), blocks: vec![] }).collect();
        let voice = sounds.iter_mut().find(|l| l.which == SoundTrack::Voice).unwrap();
        let clip = crate::project::MusicClip { input: "take.wav".into(), at: 2.0, start: 1.0, end: 5.0, fade_in: 1.0, volume: vec![VolumeKey { t: 3.0, volume: 0.0 }], ..Default::default() };
        voice.track.clips.push(clip.clone());
        voice.blocks.push(MusicBlock { clip, label: "take".into(), most: f64::INFINITY, wave: None, even_wave: None, level: "0 dB".into() });
        let t = Timeline {
            blocks: vec![block(0.0, 4.0, 0.0), block(3.0, 4.0, 1.0), block(7.0, 3.0, 0.0)],
            sounds,
            picked: Picked::None,
            video: 10.0, clips_db: 0.0, duration: 10.0, start: 0.0, span: 10.0, position: 0.0,
            current: None, overlays: vec![], texts: vec![], drag_epoch: 0, step_note: None,
            rows: Rows::new(1.0, false, Folds { sfx2: false, ..Folds::default() }, 0),
            listen: Default::default(),
            check_marks: vec![],
            ruler: RULER_LARGE,
            folds: Folds::default(),
            snapping: true,
            snap_line: None,
            auto_lanes: vec![],
            overlay_auto: vec![],
            text_auto: vec![],
            held: Default::default(),
        };
        (t, Rectangle::new(Point::ORIGIN, Size::new(1000.0 + 2.0 * PICK, 300.0)))
    }

    #[test]
    fn the_selected_cuts_sound_edges_are_where_its_sound_runs() {
        let (mut t, b) = timeline();
        // The last cut (7..10 s) selected, its sound from half a second
        // before its picture (a J-cut) to a second short of its end.
        t.picked = Picked::Cut(2);
        (t.blocks[2].cut.sound_before, t.blocks[2].cut.sound_after) = (0.5, -1.0);
        let (l, r) = t.sound_x(&t.blocks[2], b);
        assert!((l - t.x(6.5, b)).abs() < 2.0 && (r - t.x(9.0, b)).abs() < 2.0, "{l} {r}");
        assert_eq!(t.sound_edge_at(l + 2.0, b), Some((2, true)));
        assert_eq!(t.sound_edge_at(r - 2.0, b), Some((2, false)));
        // The picture's own edge is not the sound's, nor is the middle.
        assert_eq!(t.sound_edge_at(t.x(7.5, b), b), None);
        assert_eq!(t.sound_edge_at(t.x(10.0, b), b), None);
        // Nothing for a cut not selected.
        t.picked = Picked::Cut(0);
        assert_eq!(t.sound_edge_at(l + 2.0, b), None);
    }

    #[test]
    fn drags_snap_to_joins_sounds_and_markers_but_not_their_own_edges() {
        let (mut t, _) = timeline();
        // 100 px a second: a join 10 px off is taken, one 20 px off isn't.
        let per_px = 0.01;
        let any = |_: Of| false;
        let by = |s: Option<(f64, f64)>| s.map(|(by, _)| (by * 1000.0).round() / 1000.0);
        assert_eq!(by(t.snap(&[7.1], per_px, any)), Some(-0.1));
        assert_eq!(by(t.snap(&[7.25], per_px, any)), None);
        // The next cut's sound starting half a second early, at 6.5 s, and
        // a marker on the first cut at 1.5 s.
        t.blocks[2].cut.sound_before = 0.5;
        t.blocks[0].cut.marks = vec![crate::project::Mark { t: 1.5, note: String::new() }];
        assert_eq!(by(t.snap(&[6.47], per_px, any)), Some(0.03));
        assert_eq!(by(t.snap(&[1.45], per_px, any)), Some(0.05));
        // Its own edges aren't targets: the Voice clip (2..6) dragged by
        // its start near 2 doesn't snap back onto itself.
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        assert_eq!(by(t.snap(&[2.05], per_px, any)), Some(-0.05));
        assert_eq!(t.snap(&[2.05], per_px, |of| of == Of::Sound(li, 0)), None);
        // Off (N, or Shift held): nothing snaps.
        t.snapping = false;
        assert_eq!(t.snap(&[7.01], per_px, any), None);
    }

    #[test]
    fn the_kept_sound_lanes_redraw_for_what_they_show_never_for_the_playhead() {
        let (mut t, b) = timeline();
        let theme = Theme::KanagawaDragon;
        let key = t.sound_key(b, &theme);
        // Playing, dragging the playhead, snapping: the same drawing.
        t.position = 7.3;
        t.snap_line = Some(7.0);
        assert_eq!(t.sound_key(b, &theme), key);
        // What the lanes show: each redraws them.
        let changed = |f: &dyn Fn(&mut Timeline)| {
            let mut u = timeline().0;
            f(&mut u);
            u.sound_key(b, &theme) != key
        };
        assert!(changed(&|u| u.picked = Picked::Cut(1)));
        assert!(changed(&|u| u.start = 1.0));
        assert!(changed(&|u| u.clips_db = -3.0));
        assert!(changed(&|u| u.blocks[1].cut.sound_after = 0.5));
        assert!(changed(&|u| u.blocks[0].cut.volume.push(crate::project::VolumeKey { t: 1.0, volume: -6.0 })));
        assert!(changed(&|u| u.sounds[0].track.volume = -6.0));
        assert!(changed(&|u| u.picked = Picked::Point(Own::Cut(0), 0)));
        assert_ne!(t.sound_key(Rectangle::new(Point::ORIGIN, Size::new(b.width * 2.0, b.height)), &theme), key, "a wider window");
    }

    #[test]
    fn time_starts_after_the_header_column_whose_lanes_open_their_menus() {
        use super::super::sound_ui::Menu;
        let (t, b) = timeline();
        assert_eq!(t.x(0.0, b), HEAD + PICK);
        assert!((t.t(HEAD + PICK + Timeline::width(b), b) - 10.0).abs() < 1e-6);
        // A sound lane's header: its own menu; the clip sound's too.
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        let mid = t.rows.sound_top(li) + t.rows.lane_h(li) / 2.0;
        assert_eq!(t.head_menu(Point::new(HEAD - 10.0, mid)), Some(Menu::Track(SoundTrack::Voice)));
        assert_eq!(t.head_menu(Point::new(20.0, t.rows.cuts + t.rows.clip_sound / 2.0)), Some(Menu::ClipSound));
        // The picture's row has none.
        assert_eq!(t.head_menu(Point::new(20.0, t.rows.cuts / 2.0)), None);
        // In a tall lane the name and its S and M sit a little up, its
        // level under them; the buttons are where they're drawn.
        let at = t.sound_mid(li);
        let [solo, _] = listen_buttons(&t.sound_tag(li), at);
        assert_eq!(t.listen_button_at(solo.center()), Some((super::super::listen::lane_of(SoundTrack::Voice), true)));
    }

    #[test]
    fn solo_and_mute_sit_beside_each_sound_lanes_name() {
        let (t, _) = timeline();
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        let mid = t.rows.sound_top(li) + t.rows.lane_h(li) / 2.0;
        let [solo, mute] = listen_buttons("Voice", mid);
        let voice = super::super::listen::lane_of(SoundTrack::Voice);
        assert_eq!(t.listen_button_at(solo.center()), Some((voice, true)));
        assert_eq!(t.listen_button_at(mute.center()), Some((voice, false)));
        let [solo, _] = listen_buttons("Clip sound", t.rows.cuts + t.rows.clip_sound / 2.0);
        assert_eq!(t.listen_button_at(solo.center()), Some((super::super::listen::CLIP_SOUND, true)));
        // Past them, and the level tags start beyond.
        assert_eq!(t.listen_button_at(Point::new(mute.x + 40.0, mid)), None);
        assert!(mute.x + mute.width < PICK + 125.0);
    }

    #[test]
    fn sfx_2_folds_to_a_strip_that_opens_on_a_press() {
        let (mut t, b) = timeline();
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Sfx2).unwrap();
        let clip = crate::project::MusicClip { input: "boom.wav".into(), at: 2.0, start: 0.0, end: 1.0, ..Default::default() };
        t.sounds[li].blocks.push(MusicBlock { clip, label: "boom".into(), most: f64::INFINITY, wave: None, even_wave: None, level: "0 dB".into() });
        let open = t.rows;
        t.rows = Rows::new(1.0, false, Folds::default(), 0);
        assert_eq!(t.rows.lane_h(li), FOLDED);
        assert_eq!(open.height - t.rows.height, open.lane_h(li) - FOLDED, "only SFX 2 shrinks");
        assert_eq!(t.sound_tag(li), "▸ SFX 2 (1)");
        // Folded: a press anywhere on it opens it, nothing on it is hit.
        let mid = t.rows.sound_top(li) + FOLDED / 2.0;
        assert_eq!(t.fold_at(Point::new(t.x(2.5, b), mid)), Some((FoldLane::Sfx2, false)));
        assert_eq!(t.fade_at(li, Point::new(t.x(2.5, b), mid), b), None);
        // Open: only its name folds it; the other lanes never fold.
        t.rows = open;
        assert_eq!(t.sound_tag(li), "▾ SFX 2");
        let mid = t.rows.sound_top(li) + t.rows.lane_h(li) / 2.0;
        assert_eq!(t.fold_at(fold_tag(&t.sound_tag(li), mid).center()), Some((FoldLane::Sfx2, true)));
        assert_eq!(t.fold_at(Point::new(t.x(2.5, b), mid)), None);
        let voice = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        assert_eq!(t.fold_at(Point::new(PICK + 20.0, t.rows.sound_top(voice) + 5.0)), None);
    }

    #[test]
    fn the_ruler_marks_time_at_a_readable_step() {
        // 80 px apart at least, whatever the zoom.
        for per_px in [0.001, 0.01, 0.05, 0.2, 1.0, 5.0] {
            let (step, minors) = ruler_step(per_px);
            assert!(step / per_px >= 80.0 && minors >= 3, "{per_px}: {step}");
        }
        assert_eq!(ruler_step(0.01).0, 1.0);
        assert_eq!(ruler_label(65.0, 5.0), "1:05");
        assert_eq!(ruler_label(65.5, 0.5), "1:05.5");
        assert_eq!(ruler_label(3725.0, 60.0), "1:02:05");
    }

    #[test]
    fn an_automation_lane_finds_a_point_an_edge_a_clip_or_room_to_draw() {
        use super::auto::LaneHit;
        use crate::automation::{AutoClip, AutoPoint};
        let (mut t, b) = timeline();
        // Exposure's lane open; cut 0 (0..4 s, clip time 0..4) moves it over 1..3 s.
        (t.auto_lanes, t.folds.effects) = (vec![super::super::automation_ui::Param::Grade(0)], false);
        t.rows = Rows::new(1.0, false, t.folds, 1);
        t.blocks[0].cut.automation = vec![AutoClip { param: "exposure".into(), from: 1.0, to: 3.0, points: vec![AutoPoint { t: 2.0, v: 1.5, ease: Default::default() }], follows: None, animation: None }];
        let lanes = t.cut_lanes();
        let (top, h) = lanes.lane(0);
        let mid = top + h / 2.0;
        assert!((lanes.value(0, lanes.y(0, 1.5)) - 1.5).abs() < 1e-3, "height and value both ways");
        let at = |s: f64, y: f32| t.hit(Point::new(t.x(s, b), y), b, false, false);
        assert!(matches!(at(2.0, lanes.y(0, 1.5)), Hit::Auto(LaneHit::Point { lane: 0, owner: 0, t }) if t == 2.0));
        assert!(matches!(at(1.0, mid), Hit::Auto(LaneHit::Edge { start: true, .. })) && matches!(at(3.0, mid), Hit::Auto(LaneHit::Edge { start: false, .. })));
        assert!(matches!(at(2.5, top + h - 3.0), Hit::Auto(LaneHit::Clip { owner: 0, .. })));
        assert!(matches!(at(0.5, mid), Hit::Auto(LaneHit::Empty { lane: 0, owner: 0 })));
        // The point's value tag, beside it: drags its value.
        let tag = lanes.value_tag(0, t.x(2.0, b), lanes.y(0, 1.5), 1.5);
        assert!(matches!(t.hit(tag.center(), b, false, false), Hit::Auto(LaneHit::Value { owner: 0, t, v, .. }) if t == 2.0 && v == 1.5));
        assert_eq!(lanes.tag(), "▾ Effects");
    }

    #[test]
    fn an_overlay_lane_has_its_own_automation_lanes_with_its_place() {
        use super::auto::LaneHit;
        use super::super::automation_ui::{AutoOn, Param};
        use crate::automation::{AutoClip, AutoPoint};
        let (_, b) = timeline();
        let lane_with = |params: Vec<Param>, effects_open: bool| {
            let (mut t, _) = timeline();
            let x = AutoClip { param: "x".into(), from: 0.0, to: 4.0, points: vec![AutoPoint { t: 1.0, v: 0.25, ease: Default::default() }], follows: None, animation: None };
            let item = crate::overlay::Item { id: 9, input: "logo.png".into(), text: None, at: 2.0, start: 0.0, end: 4.0, motion: vec![], effects: vec![], bars: None, automation: vec![x] };
            t.overlays = vec![OverlayBlock { item, label: "logo".into(), thumb: None }];
            (t.overlay_auto, t.folds.overlay, t.folds.overlay_motion, t.folds.overlay_effects) = (params, false, false, !effects_open);
            OverlayLane(Arc::new(t), Track::Overlay)
        };
        let lane = lane_with(vec![Param::Place(0)], false);
        let lanes = lane.lanes(true);
        assert_eq!((lane.lanes(false).params.len(), lane.height()), (0, lane.0.rows.overlay + EFFECTS + KEYS + AUTO), "its place's lane under its Motion row");
        // The point at its own 1 s, edit 3 s, a quarter across.
        let p = Point::new(lane.0.x(3.0, b), lanes.y(0, 0.25));
        assert!(matches!(lane.hit(p, b), OverlayHit::Auto(true, LaneHit::Point { lane: 0, owner: 0, t }) if t == 1.0));
        assert_eq!(lanes.owners[0].on, AutoOn::Item(9));
        assert_eq!(Param::Place(0).label(0.25), "0.25");
        // An effect's lane open too: Motion moves down under it.
        let lane2 = lane_with(vec![Param::Grade(9), Param::Place(0)], true);
        assert_eq!(lane2.motion_top(), lane2.0.rows.overlay + EFFECTS + AUTO);
        assert_eq!((lane2.lanes(false).tag(), lane2.lanes(true).tag()), ("▾ Effects".to_owned(), "▾ Motion".to_owned()));
    }

    #[test]
    fn the_transitions_lane_finds_a_zone_its_length_edge_or_room_to_draw() {
        let (mut t, b) = timeline();
        // The overlap of cuts 0 and 1 is 3..4 s; an in edge on cut 2, 7..8 s.
        t.blocks[2].cut.edge_in = Some(crate::project::Edge { look: crate::transition::Look::EdgeBlur, seconds: 1.0, params: Default::default() });
        let y = t.rows.trans_top + t.rows.trans / 2.0;
        let hit = |s: f64| t.hit(Point::new(t.x(s, b), y), b, false, false);
        assert!(matches!(hit(3.5), Hit::Zone { join: 1, side: None }));
        assert!(matches!(hit(3.0), Hit::ZoneEdge { cut: 1, side: None }), "the overlap's start sets its length");
        assert!(matches!(hit(7.5), Hit::Zone { join: 2, side: Some(End::In) }));
        assert!(matches!(hit(8.0), Hit::ZoneEdge { cut: 2, side: Some(End::In) }), "an in's inner side");
        assert!(matches!(hit(5.5), Hit::ZoneDraw));
    }

    #[test]
    fn the_rows_stack_top_to_bottom_at_every_scale() {
        for scale in [0.5, 0.75, 1.0, 2.0, 3.0, 5.0] {
            for framing in [false, true] {
                let r = Rows::new(scale, framing, Folds { effects: false, ..Folds::default() }, 2);
                assert!(r.cuts < r.effects_top && r.effects_top < r.seek_top && r.seek_top < r.sounds_top && r.sounds_top < r.height, "{scale} {framing}: {r:?}");
                assert_eq!(r.keys > 0.0, framing, "the View row only while framing");
                for i in 0..SOUND_LANES {
                    let middle = r.sound_top(i) + r.lane_h(i) / 2.0;
                    assert_eq!(r.sound_at_y(middle), Some(i));
                }
                assert_eq!(r.sound_at_y(r.sounds_top - 1.0), None);
                assert_eq!(r.sound_at_y(r.height - SCROLL + 1.0), None, "the scrollbar isn't a lane");
                assert!(r.on_clip_sound(r.cuts + 1.0) && !r.on_clip_sound(r.trans_top));
                assert!(r.on_transitions(r.trans_top) && !r.on_transitions(r.effects_top) && r.trans_top < r.effects_top);
            }
        }
        // Scaled outside its range, it holds at the ends.
        assert_eq!(Rows::new(10.0, false, Folds::default(), 0).cuts, Rows::new(3.0, false, Folds::default(), 0).cuts);
    }

    #[test]
    fn a_press_finds_the_cut_the_edge_and_the_level_under_it() {
        let (t, b) = timeline();
        let x = |s: f64| t.x(s, b);
        // In the crossfade the incoming cut lies on top.
        assert_eq!(t.block_at(x(3.5), b), Some(1));
        assert_eq!(t.block_at(x(1.0), b), Some(0));
        assert_eq!(t.block_at(x(8.0), b), Some(2));
        // The join at 7 s: the later cut's in edge.
        assert_eq!(t.edge_hit(x(7.0) + 1.0, b), Some((2, true)));
        assert_eq!(t.edge_hit(x(5.0), b), None);
        // Only the selected cut's edges take a press; at the join it's the
        // selected one's, even when the cut after it lies over the join.
        let mut t2 = timeline().0;
        assert_eq!(t2.selected_edge(x(7.0) + 1.0, b), None, "nothing selected: no handles");
        t2.picked = Picked::Cut(1);
        assert_eq!(t2.selected_edge(x(7.0) - 1.0, b), Some((1, false)));
        assert_eq!(t2.selected_edge(x(7.0) + 1.0, b), Some((1, false)), "the selected cut's out edge, not the next one's in");
        // 12 px in from an end is still the edge; a narrow block is split
        // in thirds, so its middle moves it.
        assert_eq!(edge_of(100.0, 300.0, 111.0), Some(true));
        assert_eq!(edge_of(100.0, 300.0, 113.0), None);
        assert_eq!(edge_of(100.0, 130.0, 105.0), Some(true));
        assert_eq!(edge_of(100.0, 130.0, 115.0), None);
        assert_eq!(edge_of(100.0, 130.0, 125.0), Some(false));
        // Markers: in their cut's stretch only, in edit order.
        let mut t3 = timeline().0;
        t3.blocks[2].cut.marks = vec![crate::project::Mark { t: 1.0, note: "late".into() }];
        t3.blocks[0].cut.marks = vec![crate::project::Mark { t: 2.0, note: String::new() }, crate::project::Mark { t: 5.0, note: "past its out".into() }];
        let f = t3.flags(b);
        assert_eq!(f.iter().map(|f| (f.0, f.1)).collect::<Vec<_>>(), [(0, 2.0), (2, 1.0)]);
        assert!((f[1].2 - x(8.0)).abs() < 0.01, "cut 2 starts at 7 s: its 1 s is 8 s");
        assert_eq!((signed(1.24), signed(-0.4), signed(0.0), seconds(4.31), seconds(62.5), dailix_sheet::clock(59.96, 1)), ("+0:01.2".into(), "−0:00.4".into(), "+0:00.0".into(), "4.3 s".into(), "1:02.5".into(), "1:00.0".into()));
        // The level line: y back to dB, every level outside 0 dB's ±1 dB
        // reachable on the narrow clip sound lane too.
        let lane = t.rows.cut_lane();
        for db in [-30.0, -12.0, -3.0, 4.5] {
            assert!((Timeline::volume_db(lane, Timeline::volume_y(lane, db)) - db).abs() < 0.2, "{db}");
        }
        assert_eq!(Timeline::volume_db(lane, Timeline::volume_y(lane, 0.5)), 0.0, "0 dB caught");
        assert_eq!(Timeline::volume_db(lane, lane.0 + lane.1 + 50.0), VOLUME_BOTTOM, "below the lane is off");
    }

    #[test]
    fn a_cuts_volume_line_runs_where_its_sound_does() {
        let (mut t, b) = timeline();
        let ruler = timeline().0;
        let x = |s: f64| ruler.x(s, b);
        // The last cut (7..10 s) with a point at clip time 1 s, its sound
        // cut a second short (an L-cut the other way).
        t.blocks[2].cut.volume = vec![VolumeKey { t: 1.0, volume: 0.0 }];
        t.blocks[2].cut.sound_after = -1.0;
        let y = Timeline::volume_y(t.rows.cut_lane(), 0.0);
        assert_eq!(t.on_sound_line(Point::new(x(8.5), y), b), Some(Own::Cut(2)));
        assert_eq!(t.on_sound_line(Point::new(x(9.5), y), b), None, "past its sound's end: no line");
        // Run on half a second before its picture: the line reaches there.
        t.blocks[2].cut.sound_before = 0.5;
        assert_eq!(t.on_sound_line(Point::new(x(6.7), y), b), Some(Own::Cut(2)));
    }

    #[test]
    fn a_press_and_the_pointer_read_one_answer_of_whats_under_it() {
        use mouse::Interaction as I;
        let (mut t, b) = timeline();
        let ruler = timeline().0;
        let x = |s: f64| ruler.x(s, b);
        let at = |t: &Timeline, x: f32, y: f32, ctrl: bool, shift: bool| t.hit(Point::new(x, y), b, ctrl, shift);
        // A cut moves; the selected one's edges trim, Shift on its in edge
        // crossfades (not on the first cut, which has none before it).
        assert_eq!(at(&t, x(1.0), 10.0, false, false), Hit::Cut(0));
        t.picked = Picked::Cut(1);
        assert_eq!(at(&t, x(7.0) - 1.0, 10.0, false, false), Hit::CutEdge { cut: 1, start: false, crossfade: false });
        assert_eq!(at(&t, x(3.0) + 1.0, 10.0, false, true), Hit::CutEdge { cut: 1, start: true, crossfade: true });
        assert_eq!(Hit::CutEdge { cut: 1, start: true, crossfade: true }.pointer(), I::ResizingHorizontally);
        // On Voice: its point grabs, Ctrl takes it away; off the clip,
        // even with Ctrl, a press does nothing and the pointer says so.
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        let line = Timeline::volume_y(t.rows.sound_lane(li), 0.0);
        let own = Own::Clip(SoundTrack::Voice, 0);
        assert_eq!(at(&t, x(4.0), line, false, false), Hit::Point { own, key: 0, delete: false });
        assert_eq!(at(&t, x(4.0), line, true, false), Hit::Point { own, key: 0, delete: true });
        assert_eq!(at(&t, x(8.0), line, true, false), Hit::Nothing);
        assert_eq!(Hit::Nothing.pointer(), I::default());
        // Ctrl elsewhere on the clip: a new point, the crosshair.
        let low = t.rows.sound_top(li) + t.rows.lane_h(li) - 2.0;
        assert_eq!(at(&t, x(5.0), low, true, false), Hit::NewPoint(own));
        assert_eq!(Hit::NewPoint(own).pointer(), I::Crosshair);
        // The gap under the marks: the playhead goes there.
        assert_eq!(at(&t, x(5.0), t.rows.seek_top + 1.0, false, false), Hit::Playhead);
        // The scrollbar only takes a press while zoomed in.
        assert_eq!(at(&t, x(5.0), t.rows.height - 1.0, false, false), Hit::Nothing);
    }

    #[test]
    fn a_press_on_a_sound_clip_finds_its_fade_its_point_and_its_line() {
        let (t, b) = timeline();
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        let x = |s: f64| t.x(s, b);
        let lane = t.rows.sound_lane(li);
        // Its fade-in handle along the top, where the 1 s fade ends.
        assert_eq!(t.fade_at(li, Point::new(x(3.0), t.rows.sound_top(li) + 4.0), b), Some((0, true)));
        // Two clips meeting at 200 px with no fades: their knobs 16 px apart,
        // not one over the other.
        assert_eq!((fade_knob_x((100.0, 200.0), 200.0, false), fade_knob_x((200.0, 300.0), 200.0, true)), (192.0, 208.0));
        assert_eq!(fade_knob_x((100.0, 200.0), 150.0, false), 150.0, "a longer fade's knob at its end");
        // Its point (file time 3 s is edit time 4 s), and its line.
        let own = Own::Clip(SoundTrack::Voice, 0);
        assert_eq!(t.sound_at(Point::new(x(4.0), Timeline::volume_y(lane, 0.0)), b), Some((own, 0)));
        assert_eq!(t.on_sound_line(Point::new(x(5.0), Timeline::volume_y(lane, 0.0)), b), Some(own));
        assert_eq!(t.on_sound_line(Point::new(x(5.0), Timeline::volume_y(lane, -20.0)), b), None);
        // A new point there: the file's own time.
        let (at, db) = t.own_point(own, Point::new(x(5.0), Timeline::volume_y(lane, -6.0)), b).unwrap();
        assert!((at - 4.0).abs() < 0.02 && (db + 6.0).abs() < 0.2, "{at} {db}");
        // A new point on the clip's own line, in file time, dragged on.
        let mut drag = Drag::None;
        let y = Timeline::volume_y(lane, -6.0);
        let on_clip = t.add_point(&mut drag, own, Point::new(x(5.0), y), b);
        assert!(matches!(on_clip, Some(Message::PointAdd(o, at, db)) if o == own && (at - 4.0).abs() < 0.02 && (db + 6.0).abs() < 0.2));
        assert!(matches!(drag, Drag::Point { key: 1, .. }), "after its point at 3 s");
        // Its level tag: a press starts a drag of its level, a second
        // within a double-click's time puts it back to normal.
        let tag = t.level_tag_at(own, b).expect("room for the tag");
        let on_tag = Point::new(tag.x + 2.0, tag.y + 7.0);
        assert_eq!(t.level_tag_hit(on_tag, b), Some(own));
        let (mut drag, mut last) = (Drag::None, None);
        assert!(matches!(t.tag_press(&mut drag, &mut last, own, on_tag), Some(Message::SelectMusic(SoundTrack::Voice, 0))));
        assert!(matches!(drag, Drag::Level { from: 0.0, .. }));
        assert!(matches!(t.tag_press(&mut drag, &mut last, own, on_tag), Some(Message::Sound(super::super::sound_ui::SoundMsg::Restore(o))) if o == own));
        // Off the clips there's no clip, and its edges are its edges.
        assert_eq!(t.music_at(li, x(8.0), b), None);
        assert_eq!(t.music_edge_at(li, 0, x(6.0) - 1.0, b), Some(false));
    }
}

/// A level tag at `at` (if there's room): dark, white text, as on the cuts.
fn draw_tag(frame: &mut Frame, at: Option<Rectangle>, label: &str) {
    let Some(r) = at else { return };
    frame.fill(&Path::rounded_rectangle(r.position(), r.size(), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7));
    frame.fill_text(Text { content: label.to_owned(), position: Point::new(r.x + 4.0, r.y + r.height / 2.0), color: iced::Color::WHITE, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
}

/// A lane's solo and mute buttons, after its name tag (`lane_tag`).
fn listen_buttons(name: &str, mid: f32) -> [Rectangle; 2] {
    let x = PICK + 2.0 + name.chars().count() as f32 * 6.0 + 22.0 + 3.0;
    let b = |x: f32| Rectangle::new(Point::new(x, mid - 7.0), Size::new(14.0, 14.0));
    [b(x), b(x + 16.0)]
}

/// Lane `lane`'s «S» and «M», lit when on: yellow solo, red mute.
fn draw_listen(frame: &mut Frame, name: &str, mid: f32, listen: &super::listen::Listen, lane: usize) {
    let [solo, mute] = listen_buttons(name, mid);
    for (r, label, on, lit) in [(solo, "S", listen.solo[lane], iced::Color::from_rgb(0.95, 0.79, 0.3)), (mute, "M", listen.muted[lane], iced::Color::from_rgb(1.0, 0.3, 0.3))] {
        frame.fill(&Path::rounded_rectangle(r.position(), r.size(), 3.0.into()), if on { lit } else { iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7) });
        let ink = if on { iced::Color::BLACK } else { iced::Color::from_rgba(1.0, 1.0, 1.0, 0.75) };
        frame.fill_text(Text { content: label.into(), position: Point::new(r.x + 3.5, r.y + r.height / 2.0), color: ink, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
    }
}

/// A sound lane's colour, as the timeline draws it (the mix check too).
pub(super) fn lane_colour(lane: crate::mixcheck::Lane, theme: &Theme) -> iced::Color {
    match lane {
        crate::mixcheck::Lane::ClipSound => SOUND_LINE,
        crate::mixcheck::Lane::Track(SoundTrack::Music) => theme.extended_palette().success.base.color,
        crate::mixcheck::Lane::Track(SoundTrack::Sfx) => SFX_COLOUR,
        crate::mixcheck::Lane::Track(SoundTrack::Sfx2) => SFX2_COLOUR,
        crate::mixcheck::Lane::Track(SoundTrack::Voice) => VOICE_COLOUR,
    }
}

/// The time ruler over every lane (gap 50, as editors have it on top):
/// seconds marked at a step that suits the zoom, the playhead's head to
/// drag, the mix check's findings along its foot. `large` is the default;
/// the compact one is the old seek bar's height.
pub struct Ruler(pub Arc<Timeline>);

/// The ruler's height, large and compact.
pub const RULER_LARGE: f32 = 28.0;
pub const RULER_COMPACT: f32 = 16.0;

/// Between the major ticks: as many seconds as keep them 80 px apart or
/// more, and how many minor ticks each holds.
fn ruler_step(seconds_per_px: f64) -> (f64, u32) {
    const STEPS: [(f64, u32); 15] = [(0.1, 5), (0.2, 4), (0.5, 5), (1.0, 5), (2.0, 4), (5.0, 5), (10.0, 5), (15.0, 3), (30.0, 6), (60.0, 6), (120.0, 4), (300.0, 5), (600.0, 6), (1800.0, 6), (3600.0, 6)];
    STEPS.into_iter().find(|&(s, _)| s / seconds_per_px >= 80.0).unwrap_or((7200.0, 4))
}

/// A ruler label: «1:05», «1:05.5» under a second's step, «1:02:05» past
/// an hour.
fn ruler_label(t: f64, step: f64) -> String {
    let whole = t.round_ties_even() as u64;
    let (h, m, s) = (whole / 3600, whole / 60 % 60, whole % 60);
    if step < 1.0 {
        let tenth = ((t * 10.0).round() as u64) % 10;
        format!("{}:{:02}.{tenth}", t as u64 / 60, (t as u64) % 60)
    } else if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[derive(Default)]
pub struct RulerState {
    dragging: bool,
}

/// Markers' colour: apart from the playhead's and the lanes'.
const MARK: iced::Color = iced::Color::from_rgb(0.97, 0.40, 0.66);

/// How far either side of a marker's flag a press takes it.
const MARK_REACH: f32 = 7.0;

/// The markers' row over the ruler, open and folded (the user, 2026-11-03:
/// flags on the ruler were covered by the playhead's head).
pub const FLAGS_OPEN: f32 = 24.0;
pub const FLAGS_FOLDED: f32 = 8.0;

/// The markers' row: a flag on each marker's pole, its note beside it,
/// over the ruler where the playhead's head doesn't cover them. A click
/// selects one and puts the playhead there (Del then takes the marker,
/// not the cut), a drag moves it within its cut, a double- or right-click
/// opens its window; the name at the left folds the row to a strip.
pub struct Flags(pub Arc<Timeline>);

#[derive(Default)]
pub struct FlagsState {
    /// A marker being dragged: its cut and its clip time now.
    mark: Option<(usize, f64)>,
    /// The last press on a flag, for a double-click.
    click: Option<(std::time::Instant, usize, f64)>,
    /// `Timeline::drag_epoch` when the press began (Esc drops the drag).
    epoch: u64,
}

impl Flags {
    /// Whether `p` is on the row's name (which folds it).
    fn on_name(p: Point) -> bool {
        p.x <= PICK + 82.0
    }
}

impl canvas::Program<Message> for Flags {
    type State = FlagsState;

    fn draw(&self, _: &FlagsState, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let t = &self.0;
        let mut frame = Frame::new(renderer, bounds.size());
        let h = bounds.height;
        frame.fill(&Path::rectangle(Point::ORIGIN, bounds.size()), theme.extended_palette().background.weak.color.scale_alpha(0.4));
        let flags = t.flags(bounds);
        let outline = Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8));
        for (n, &(cut, mt, x, note)) in flags.iter().enumerate() {
            let chosen = t.picked.mark().is_some_and(|(c, m)| c == cut && (m - mt).abs() < crate::model::ops::cuts::SAME_MARK);
            frame.fill(&Path::rectangle(Point::new(x - 0.9, 0.0), Size::new(1.8, h)), MARK);
            if t.folds.marks {
                frame.fill(&Path::rectangle(Point::new(x - 2.5, 0.0), Size::new(5.0, h)), MARK);
                continue;
            }
            // 15 % bigger than they were on the ruler, outlined to stand off.
            let w = 9.5;
            let pennant = Path::new(|p| {
                p.move_to(Point::new(x, 2.0));
                p.line_to(Point::new(x + w, 2.0 + w * 0.6));
                p.line_to(Point::new(x, 2.0 + w * 1.2));
                p.close();
            });
            frame.fill(&pennant, if chosen { iced::Color::WHITE } else { MARK });
            frame.stroke(&pennant, if chosen { Stroke::default().with_width(2.0).with_color(MARK) } else { outline });
            let room = flags.get(n + 1).map_or(bounds.width, |f| f.2) - x - w - 8.0;
            let fits = (room / 7.0).max(0.0) as usize;
            if !note.is_empty() && fits >= 3 {
                let shown: String = if note.chars().count() > fits { note.chars().take(fits - 1).chain(std::iter::once('…')).collect() } else { note.to_owned() };
                let wide = shown.chars().count() as f32 * 6.9 + 8.0;
                frame.fill(&Path::rounded_rectangle(Point::new(x + w + 2.0, h / 2.0 - 8.0), Size::new(wide, 16.0), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.65));
                frame.fill_text(Text { content: shown, position: Point::new(x + w + 6.0, h / 2.0), color: MARK, size: 12.5.into(), align_y: Vertical::Center, ..Default::default() });
            }
        }
        draw_head_ground(&mut frame, theme, h);
        head_cell(&mut frame, 0.0, h, MARK);
        if !t.folds.marks {
            lane_tag(&mut frame, "▾ Markers", MARK, h / 2.0);
        } else {
            frame.fill_text(Text { content: "▸ Markers".into(), position: Point::new(PICK + 4.0, h / 2.0), color: MARK, size: 9.0.into(), align_y: Vertical::Center, ..Default::default() });
        }
        draw_snap_line(&mut frame, t, bounds, bounds.height);
        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut FlagsState, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        let t = &self.0;
        // Esc cancelled the drag in the app: let it go.
        if state.epoch != t.drag_epoch {
            (state.epoch, state.mark) = (t.drag_epoch, None);
        }
        if let Mouse(mouse::Event::WheelScrolled { delta }) = event {
            return wheel(delta, t, cursor, bounds);
        }
        match event {
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                let Some((cut, mt)) = t.flag_at(bounds, p.x) else {
                    return Flags::on_name(p).then(|| canvas::Action::publish(Message::Fold(FoldLane::Marks, !t.folds.marks)).and_capture());
                };
                // Twice on it: its window, to name or delete it.
                let again = state.click.is_some_and(|(at, c, m)| c == cut && m == mt && at.elapsed() < DOUBLE_CLICK);
                state.click = Some((std::time::Instant::now(), cut, mt));
                if again {
                    return Some(canvas::Action::publish(Message::MarkEdit(cut, mt)).and_capture());
                }
                state.mark = Some((cut, mt));
                Some(canvas::Action::publish(Message::MarkSelect(cut, mt)).and_capture())
            }
            Mouse(mouse::Event::CursorMoved { position }) => {
                let (cut, from) = state.mark?;
                let c = t.blocks.get(cut)?;
                let at = t.t(position.x - bounds.x, bounds);
                let snap = t.snap(&[at], t.span / Timeline::width(bounds) as f64, |of| of == Of::Mark(cut));
                let to = (c.cut.start + at + snap.map_or(0.0, |(by, _)| by) - c.from).clamp(c.cut.start, c.cut.start + c.len());
                state.mark = Some((cut, to));
                Some(canvas::Action::publish(snapped(snap, |_| Message::MarkMove(cut, from, to))).and_capture())
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.mark.take()?;
                Some(canvas::Action::publish(Message::ScrubDone).and_capture())
            }
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let p = cursor.position_in(bounds)?;
                let (cut, mt) = t.flag_at(bounds, p.x)?;
                Some(canvas::Action::publish(Message::MarkEdit(cut, mt)).and_capture())
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, state: &FlagsState, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let Some(p) = cursor.position_in(bounds) else { return mouse::Interaction::default() };
        if state.mark.is_some() {
            mouse::Interaction::Grabbing
        } else if self.0.flag_at(bounds, p.x).is_some() {
            mouse::Interaction::Grab
        } else if Flags::on_name(p) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl canvas::Program<Message> for Ruler {
    type State = RulerState;

    fn draw(&self, _state: &RulerState, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let t = &self.0;
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        let h = bounds.height;
        let large = h >= RULER_LARGE - 0.5;
        frame.fill(&Path::rectangle(Point::ORIGIN, bounds.size()), palette.background.weak.color.scale_alpha(0.6));
        let per_px = t.span.max(1e-9) / Timeline::width(bounds) as f64;
        let (step, minors) = ruler_step(per_px);
        let ink = palette.background.base.text;
        let first = (t.start / step).floor() * step;
        let mut m = first;
        while m <= t.start + t.span + step {
            let x = t.x(m, bounds);
            frame.fill(&Path::rectangle(Point::new(x, h * 0.45), Size::new(1.0, h * 0.55)), ink.scale_alpha(0.7));
            frame.fill_text(Text { content: ruler_label(m.max(0.0), step), position: Point::new(x + 3.0, if large { 3.0 } else { 1.0 }), color: ink.scale_alpha(0.85), size: (if large { 11.0 } else { 9.0 }).into(), ..Default::default() });
            for k in 1..minors {
                let x = t.x(m + step * k as f64 / minors as f64, bounds);
                frame.fill(&Path::rectangle(Point::new(x, h * 0.75), Size::new(1.0, h * 0.25)), ink.scale_alpha(0.35));
            }
            m += step;
        }
        // Where the video ends, when the music runs on past it.
        if t.duration > t.video + 1e-6 {
            let x = t.x(t.video, bounds);
            frame.fill(&Path::rectangle(Point::new(x, 0.0), Size::new(1.0, h)), ink.scale_alpha(0.5));
        }
        // The mix check's findings along its foot: red to fix, yellow a hint.
        for &(from, to, fix) in &t.check_marks {
            let colour = if fix { iced::Color::from_rgb(1.0, 0.3, 0.3) } else { iced::Color::from_rgb(0.95, 0.79, 0.3) };
            let (a, b) = (t.x(from, bounds), t.x(to, bounds).max(t.x(from, bounds) + 3.0));
            frame.fill(&Path::rectangle(Point::new(a, h - 3.0), Size::new(b - a, 3.0)), colour);
        }
        // The markers' poles: their flags fly in the row above (`Flags`),
        // clear of the playhead's head.
        for f in t.flags(bounds) {
            frame.fill(&Path::rectangle(Point::new(f.2 - 0.75, 0.0), Size::new(1.5, h)), MARK.scale_alpha(0.8));
        }
        // The playhead's head: a tab to drag, its line on down the lanes.
        let at = t.x(t.position, bounds);
        let head = Path::new(|p| {
            let w = if large { 6.0 } else { 4.5 };
            p.move_to(Point::new(at - w, 0.0));
            p.line_to(Point::new(at + w, 0.0));
            p.line_to(Point::new(at + w, h * 0.55));
            p.line_to(Point::new(at, h));
            p.line_to(Point::new(at - w, h * 0.55));
            p.close();
        });
        frame.fill(&head, palette.primary.strong.color);
        draw_head_ground(&mut frame, theme, h);
        draw_snap_line(&mut frame, t, bounds, bounds.height);
        vec![frame.into_geometry()]
    }

    fn update(&self, state: &mut RulerState, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        match event {
            Mouse(mouse::Event::WheelScrolled { delta }) => wheel(delta, &self.0, cursor, bounds),
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds).filter(|p| p.x >= HEAD)?;
                state.dragging = true;
                Some(canvas::Action::publish(Message::Scrub(self.0.t(p.x, bounds))).and_capture())
            }
            Mouse(mouse::Event::CursorMoved { position }) if state.dragging => Some(canvas::Action::publish(Message::Scrub(self.0.t(position.x - bounds.x, bounds))).and_capture()),
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                Some(canvas::Action::publish(Message::ScrubDone).and_capture())
            }
            _ => None,
        }
    }

    fn mouse_interaction(&self, state: &RulerState, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// The lanes that fold to a thin row (gap 50: folded by default, the
/// picture and its sound the lanes that matter most).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldLane {
    Text,
    Overlay,
    Sfx2,
    /// The markers' row over the ruler.
    Marks,
    /// The transitions lane over the effects row (gap 25f).
    Transitions,
    /// The Effects row's automation lanes (gap 39), the cuts', the
    /// overlays' and the text's.
    Effects,
    OverlayEffects,
    TextEffects,
    /// Their Motion rows' lanes (place).
    OverlayMotion,
    TextMotion,
}

/// Which of them are folded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Folds {
    pub text: bool,
    pub overlay: bool,
    pub sfx2: bool,
    pub marks: bool,
    pub transitions: bool,
    pub effects: bool,
    pub overlay_effects: bool,
    pub text_effects: bool,
    pub overlay_motion: bool,
    pub text_motion: bool,
}

impl Default for Folds {
    fn default() -> Self {
        Folds { text: true, overlay: true, sfx2: true, marks: false, transitions: false, effects: true, overlay_effects: true, text_effects: true, overlay_motion: true, text_motion: true }
    }
}

impl Folds {
    pub fn set(&mut self, lane: FoldLane, folded: bool) {
        *match lane {
            FoldLane::Text => &mut self.text,
            FoldLane::Overlay => &mut self.overlay,
            FoldLane::Sfx2 => &mut self.sfx2,
            FoldLane::Marks => &mut self.marks,
            FoldLane::Transitions => &mut self.transitions,
            FoldLane::Effects => &mut self.effects,
            FoldLane::OverlayEffects => &mut self.overlay_effects,
            FoldLane::TextEffects => &mut self.text_effects,
            FoldLane::OverlayMotion => &mut self.overlay_motion,
            FoldLane::TextMotion => &mut self.text_motion,
        } = folded;
    }
}

/// A folded lane's height.
const FOLDED: f32 = 18.0;

/// Where a lane's name tag is (`lane_tag`), centred on `mid`: a click
/// there folds or opens a foldable lane.
fn fold_tag(name: &str, mid: f32) -> Rectangle {
    Rectangle::new(Point::new(PICK + 2.0, mid - 7.5), Size::new(name.chars().count() as f32 * 6.0 + 22.0, 15.0))
}
