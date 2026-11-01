//! The timeline under the view. The top row holds the edit's cuts as
//! blocks showing thumbnails of their footage: click one to select it, drag
//! it to move it, drag its edges to trim it. Below is the seek bar with each
//! cut's keyframes as diamonds (shaped by their ease): click the bar to seek,
//! click a diamond to jump to it, drag one to retime it.
//!
//! Under the cuts is the Clip sound lane: each cut's waveform (what it
//! plays: its cleaned voice or its clip's sound) with its volume line and
//! its level tag; a right-click opens the cut's sound menu. Under the seek
//! bar are the sound lanes, one per `SoundTrack` (Music, SFX): their clips
//! with waveforms, moved and trimmed like cuts, crossfading with Shift, a
//! volume line each, and a line where the video ends (sound beyond it
//! fades out in the export). Every lane has its name pinned at the left.
//! The View key row shows only while framing.
//!
//! Above the cuts is the overlay lane, a canvas of its own (`OverlayLane`):
//! its pictures as blocks, selected, moved and trimmed like music. Under
//! each track (the overlays, once there are any, and the cuts) are two rows
//! of its keyframes: effects, then motion (an overlay's) or view (a cut's),
//! each marked by its ease; click one to jump there.
//!
//! It shows a window of the edit: the wheel zooms around the pointer, Shift
//! or a sideways wheel pans, and when zoomed in a scrollbar at the bottom
//! shows and moves the window.

use super::{Message, music, thumbs};
use crate::project::{Own, SoundTrack};
use crate::overlay::Track;
use crate::view::Ease;
use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::image::Handle;
use iced::{Point, Rectangle, Renderer, Size, Theme};
use std::sync::Arc;

/// Diamond half-size, and how close a click must land to pick a diamond or
/// a cut's edge.
const DIAMOND: f32 = 6.0;
const PICK: f32 = 7.0;
const BAR: f32 = 8.0;
/// The cut row's height at scale 1, and the gap between cut blocks.
const CUTS: f32 = 40.0;
/// The clips' own sound under them at scale 1: each cut's waveform and its
/// volume line.
const CLIP_SOUND: f32 = 30.0;
const GAP: f32 = 2.0;
/// Under a track, a row of its effect keyframes and one of its view (a
/// cut's) or motion (an overlay's) keyframes.
const EFFECTS: f32 = 16.0;
const KEYS: f32 = 20.0;
const SEEK: f32 = 18.0;
/// The music lane at scale 1.
const MUSIC: f32 = 40.0;
/// The scrollbar's row at the bottom, and the bar's thickness.
const SCROLL: f32 = 8.0;
const SCROLL_BAR: f32 = 5.0;
/// How far the lanes may shrink or grow, as a share of their own height.
pub const SCALES: std::ops::RangeInclusive<f32> = 0.75..=3.0;

/// Where the rows sit, top to bottom: the cuts, their keyframe rows, the
/// seek bar, a lane per sound track (music, sound effects) and the
/// scrollbar. Dragging the divider above
/// the transport row scales the lanes that hold pictures and waveforms
/// (cuts, music, overlays); the rows of marks and the seek bar keep their
/// height, as their marks do.
#[derive(Clone, Copy, Debug)]
pub struct Rows {
    cuts: f32,
    /// The clip sound lane's height; it starts where the cuts end.
    clip_sound: f32,
    effects_top: f32,
    /// The View key row's height: 0 while editing.
    keys: f32,
    keys_top: f32,
    seek_top: f32,
    /// A sound lane's height, and where the first starts.
    music: f32,
    sounds_top: f32,
    height: f32,
    overlay: f32,
}

impl Rows {
    /// `framing`: the View key row shows (it holds the view's keyframes);
    /// editing, it's left out and the lanes below move up.
    pub fn new(scale: f32, framing: bool) -> Self {
        let scale = scale.clamp(*SCALES.start(), *SCALES.end());
        let (cuts, music) = ((CUTS * scale).round(), (MUSIC * scale).round());
        let clip_sound = (CLIP_SOUND * scale).round();
        let effects_top = cuts + clip_sound;
        let keys = if framing { KEYS } else { 0.0 };
        let seek_top = effects_top + EFFECTS + keys;
        let sounds_top = seek_top + SEEK;
        let height = sounds_top + SOUND_LANES as f32 * music + SCROLL;
        Rows { cuts, clip_sound, effects_top, keys_top: effects_top + EFFECTS, keys, seek_top, music, sounds_top, height, overlay: (OVERLAY * scale).round() }
    }

    /// Where sound lane `i` starts.
    fn sound_top(self, i: usize) -> f32 {
        self.sounds_top + i as f32 * self.music
    }

    /// Where sound lane `i`'s volume line runs.
    fn sound_lane(self, i: usize) -> Lane {
        (self.sound_top(i) + 4.0, self.music - 8.0)
    }

    /// The sound lane at height `y`, if any.
    fn sound_at_y(self, y: f32) -> Option<usize> {
        let i = ((y - self.sounds_top) / self.music).floor();
        (y >= self.sounds_top && i < SOUND_LANES as f32).then_some(i as usize)
    }

    /// Where the cuts' volume lines run: over the clip sound lane.
    fn cut_lane(self) -> Lane {
        (self.cuts + 3.0, self.clip_sound - 6.0)
    }

    /// Whether `y` is on the clip sound lane.
    fn on_clip_sound(self, y: f32) -> bool {
        (self.cuts..self.effects_top).contains(&y)
    }
}

/// The sound lanes, one per `SoundTrack`.
const SOUND_LANES: usize = SoundTrack::ALL.len();
/// The lanes' heights at scale 1, which a drag of the divider shares out:
/// the cuts, the sound lanes and the two overlay lanes (text and pictures).
pub const SCALABLE: f32 = CUTS + CLIP_SOUND + SOUND_LANES as f32 * MUSIC + 2.0 * OVERLAY;
/// A press on a cut that moves less than this is a click, not a drag.
const DRAG_FROM: f32 = 4.0;
/// A music edge dragged this close to a cut boundary, the playhead or
/// another song's edge snaps to it.
const SNAP: f32 = 8.0;
/// The volume line's range, dB, top to bottom of its lane (the bottom is
/// silence), and a volume point's radius.
const VOLUME_TOP: f64 = 6.0;
const VOLUME_BOTTOM: f64 = crate::project::OFF_DB;
const POINT: f32 = 4.0;
/// A level dragged this close to 0 dB is 0 dB.
const SNAP_DB: f64 = 1.0;
const EFFECT_MARK: iced::Color = iced::Color::from_rgb(0.78, 0.55, 1.0);
/// Where a volume line runs: its top and height. A sound track's over its
/// lane, a cut's over the clip sound lane under it.
type Lane = (f32, f32);
const MUSIC_LINE: iced::Color = iced::Color::from_rgb(1.0, 0.78, 0.25);
/// The sound effects' clips; the music's are the theme's green. From the
/// colour-blind-safe Okabe-Ito set, and every lane is named as well.
/// How far down from a sound clip's top its fade handles take the mouse.
const FADE_BAND: f32 = 12.0;
const SFX_COLOUR: iced::Color = iced::Color::from_rgb(0.80, 0.47, 0.65);
const VOICE_COLOUR: iced::Color = iced::Color::from_rgb(0.34, 0.71, 0.91);
/// Waveform bars a gain pushes past full scale: the export's limiter
/// holds them, but loud enough to look at.
const OVER_COLOUR: iced::Color = iced::Color::from_rgb(1.0, 0.55, 0.2);
const SOUND_LINE: iced::Color = iced::Color::from_rgb(0.35, 0.85, 1.0);

/// An overlay lane's height at scale 1, above the cuts.
const OVERLAY: f32 = 30.0;
const OVERLAY_COLOUR: iced::Color = iced::Color::from_rgb(0.95, 0.55, 0.3);
const TEXT_COLOUR: iced::Color = iced::Color::from_rgb(0.3, 0.8, 0.75);

/// An overlay item as its lane shows it.
pub struct OverlayBlock {
    pub label: String,
    /// Where it starts on the edit, its in point in its own time, and its
    /// length.
    pub from: f64,
    pub start: f64,
    pub len: f64,
    /// Its effect keyframes' and motion keyframes' own times.
    pub effects: Vec<f64>,
    pub motion: Vec<(f64, Ease)>,
    pub thumb: Option<Handle>,
}

/// One cut as the timeline shows it.
pub struct Block {
    /// Its names, longest first; the first that fits is shown.
    pub labels: Vec<String>,
    /// Where it starts on the timeline, and how long it lasts.
    pub from: f64,
    pub len: f64,
    /// Its in point in clip time.
    pub start: f64,
    /// Its keyframes within the cut: index into the cut's keyframes, clip
    /// time, and how it moves on to the next (drawn as its shape).
    /// Keyframes: index, clip time, ease, and whether it follows the stick.
    pub keys: Vec<(usize, f64, Ease, bool)>,
    /// Thumbnails of its clip by clip time, in order (see `thumbs`).
    pub thumbs: Vec<(f64, Handle)>,
    /// Its sound's volume points, clip time (some may lie outside the cut).
    pub sound: Vec<crate::project::VolumeKey>,
    /// The waveform of the sound it plays (its cleaned voice, else its
    /// clip's), by clip time, once read; none for a silent clip.
    pub wave: Option<Arc<music::Waveform>>,
    /// The same through the leveler, if the cut is evened out (and read).
    pub even_wave: Option<Arc<music::Waveform>>,
    /// Its own level, dB (`Cut::sound_db`: `OFF_DB` when muted).
    pub own_db: f64,
    /// Its sound's fades in and out, seconds.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Its own sound level as a short tag ("+16.1", "−12 · even", "muted"),
    /// shown on the clip sound lane.
    pub level: Option<String>,
    /// How long it overlaps the cut before, dissolving in.
    pub crossfade: f64,
    /// Its effect keyframes' clip times.
    pub effects: Vec<f64>,
}

impl Block {
    /// The thumbnail nearest clip time `t`.
    fn thumb_at(&self, t: f64) -> Option<&Handle> {
        let i = self.thumbs.partition_point(|(at, _)| *at < t);
        let before = i.checked_sub(1).map(|j| &self.thumbs[j]);
        match (before, self.thumbs.get(i)) {
            (Some(a), Some(b)) => Some(if t - a.0 < b.0 - t { &a.1 } else { &b.1 }),
            (a, b) => a.or(b).map(|x| &x.1),
        }
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

/// A clip on a sound lane (a song, a sound effect) as it shows.
pub struct MusicBlock {
    pub label: String,
    /// Where it starts on the edit, its in point in the song, and its length.
    pub from: f64,
    pub start: f64,
    pub len: f64,
    /// The song's waveform, once read.
    pub wave: Option<Arc<music::Waveform>>,
    /// Its fades in and out, seconds: a handle at each top corner.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Its own volume points in the file's time (gap 33e).
    pub volume: Vec<crate::project::VolumeKey>,
    /// Its waveform through the leveler, if it's evened out (and read).
    pub even_wave: Option<Arc<music::Waveform>>,
}

pub struct Timeline {
    pub blocks: Vec<Block>,
    /// The sound lanes, in `SoundTrack::ALL`'s order.
    pub sounds: Vec<SoundLane>,
    /// The sound clip selected, and the sound track's volume point.
    pub selected_music: Option<(SoundTrack, usize)>,

    /// A point on a volume line, selected.
    pub selected_point: Option<(crate::project::Line, usize)>,
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
    pub selected: Option<usize>,
    /// The keyframe at the playhead (cut, keyframe), drawn highlighted.
    pub current: Option<(usize, usize)>,
    /// The overlay track's lane and the text track's.
    pub overlays: Vec<OverlayBlock>,
    pub texts: Vec<OverlayBlock>,
    pub selected_overlay: Option<crate::overlay::ItemRef>,
    /// The rows' heights, from the divider's scale.
    pub rows: Rows,
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
    /// Dragging the fade handle at clip `clip`'s start (`in_edge`) or end on
    /// sound lane `lane`; the fade was `from` seconds when pressed at `x0`.
    /// A fade handle, on a sound clip or a cut's sound.
    Fade { own: Own, in_edge: bool, x0: f32, from: f64, per_px: f64 },
    /// A point on a volume line.
    Point { line: crate::project::Line, key: usize },
    /// Dragging cut `cut`'s effect keyframe `key` to another time.
    Effect { cut: usize, key: usize },
}

#[derive(Default)]
pub struct State {
    drag: Drag,
    /// Shift held: the wheel pans instead of zooming, and music may
    /// overlap. Ctrl held: a click on the music lane adds a volume point.
    shift: bool,
    ctrl: bool,
}

impl Timeline {
    fn width(b: Rectangle) -> f32 {
        (b.width - 2.0 * PICK).max(1.0)
    }

    /// Edit time to x, with room either side for diamonds at the ends.
    /// Times outside the window land outside the widget.
    fn x(&self, t: f64, b: Rectangle) -> f32 {
        PICK + ((t - self.start) / self.span.max(1e-9)) as f32 * Self::width(b)
    }

    fn t(&self, x: f32, b: Rectangle) -> f64 {
        (self.start + ((x - PICK) / Self::width(b)) as f64 * self.span).clamp(0.0, self.duration)
    }

    fn zoomed(&self) -> bool {
        self.span < self.duration - 1e-6
    }

    /// The scrollbar's thumb, left and right x.
    fn scroll_thumb(&self, b: Rectangle) -> (f32, f32) {
        let w = Self::width(b) as f64 / self.duration.max(1e-9);
        (PICK + (self.start * w) as f32, PICK + ((self.start + self.span) * w) as f32)
    }

    fn block_at(&self, x: f32, b: Rectangle) -> Option<usize> {
        // Later first: in a crossfade the incoming cut lies over the one before.
        self.blocks.iter().rposition(|c| x >= self.x(c.from, b) && x <= self.x(c.from + c.len, b))
    }

    /// The nearest diamond within reach of `x`: (cut, keyframe).
    fn key_at(&self, x: f32, b: Rectangle) -> Option<(usize, usize)> {
        let mut best = None;
        for (ci, c) in self.blocks.iter().enumerate() {
            for &(ki, t, _, _) in &c.keys {
                let d = (self.x(c.from + t - c.start, b) - x).abs();
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
        let (l, r) = (self.x(c.from, b), self.x(c.from + c.len, b));
        if r - l < 3.0 * PICK {
            // Too narrow for a body: the nearer edge.
            return Some(x - l < r - x);
        }
        if x - l <= PICK {
            Some(true)
        } else if r - x <= PICK {
            Some(false)
        } else {
            None
        }
    }

    /// The effect keyframe mark under `p` (they sit in a row under the
    /// cuts): (cut, keyframe, clip time).
    fn effect_at(&self, p: Point, b: Rectangle) -> Option<(usize, usize, f64)> {
        if !(self.rows.effects_top..self.rows.keys_top).contains(&p.y) {
            return None;
        }
        self.blocks.iter().enumerate().rev().find_map(|(i, c)| {
            c.effects.iter().enumerate().find(|&(_, &t)| t >= c.start && t <= c.start + c.len && (self.x(c.from + t - c.start, b) - p.x).abs() <= PICK).map(|(k, &t)| (i, k, t))
        })
    }

    /// The cut edge under `x`: (cut, whether it's the in edge). Later cuts
    /// first: a crossfading cut's in edge lies over the cut before it.
    fn edge_hit(&self, x: f32, b: Rectangle) -> Option<(usize, bool)> {
        (0..self.blocks.len()).rev().find_map(|i| {
            let (l, r) = self.block_x(&self.blocks[i], b);
            (x >= l - PICK && x <= r + PICK).then(|| self.edge_at(i, x, b)).flatten().map(|e| (i, e))
        })
    }

    fn music_at(&self, lane: usize, x: f32, b: Rectangle) -> Option<usize> {
        self.sounds[lane].blocks.iter().position(|m| x >= self.x(m.from, b) && x <= self.x(m.from + m.len, b))
    }

    /// Whether `x` is on clip `i`'s in edge (`Some(true)`) or out edge.
    fn music_edge_at(&self, lane: usize, i: usize, x: f32, b: Rectangle) -> Option<bool> {
        let m = &self.sounds[lane].blocks[i];
        let (l, r) = (self.x(m.from, b), self.x(m.from + m.len, b));
        if r - l < 3.0 * PICK {
            Some(x - l < r - x)
        } else if x - l <= PICK {
            Some(true)
        } else if r - x <= PICK {
            Some(false)
        } else {
            None
        }
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
        let top = self.rows.sound_top(lane) + 2.0;
        if p.y > top + FADE_BAND {
            return None;
        }
        let per_px = self.span / Self::width(b) as f64;
        self.sounds[lane].blocks.iter().enumerate().find_map(|(i, m)| {
            let (l, r) = (self.x(m.from, b), self.x(m.from + m.len, b));
            let (fi, fo) = (l + (m.fade_in / per_px) as f32, r - (m.fade_out / per_px) as f32);
            if (p.x - fi).abs() <= PICK && p.x >= l - 1.0 && p.x <= r + 1.0 {
                Some((i, true))
            } else if (p.x - fo).abs() <= PICK && p.x >= l - 1.0 && p.x <= r + 1.0 {
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
            let (l, r) = self.block_x(c, b);
            let (fi, fo) = (l + (c.fade_in / per_px) as f32, r - (c.fade_out / per_px) as f32);
            let inside = p.x >= l - 1.0 && p.x <= r + 1.0;
            if inside && (p.x - fi).abs() <= PICK {
                Some((i, true))
            } else if inside && (p.x - fo).abs() <= PICK {
                Some((i, false))
            } else {
                None
            }
        })
    }

    /// Whether `p` is on sound lane `lane`'s track line, where it's drawn
    /// (the track has points).
    fn on_track_line(&self, lane: usize, p: Point, b: Rectangle) -> bool {
        let keys = &self.sounds[lane].track.keyframes;
        !keys.is_empty() && (Self::volume_y(self.rows.sound_lane(lane), crate::project::volume_at(keys, self.t(p.x, b))) - p.y).abs() <= PICK
    }

    /// Sound lane `lane`'s volume point under `p`, if any.
    fn volume_at(&self, lane: usize, p: Point, b: Rectangle) -> Option<usize> {
        self.sounds[lane].track.keyframes.iter().position(|k| (self.x(k.t, b) - p.x).abs() <= PICK && (Self::volume_y(self.rows.sound_lane(lane), k.volume) - p.y).abs() <= PICK)
    }

    /// Where `own` is drawn: its lane, where it starts on the edit, its in
    /// point in its file, its length and its points.
    fn own_place(&self, own: Own) -> Option<(Lane, f64, f64, f64, &[crate::project::VolumeKey])> {
        match own {
            Own::Cut(i) => self.blocks.get(i).map(|c| (self.rows.cut_lane(), c.from, c.start, c.len, &c.sound[..])),
            Own::Clip(which, i) => {
                let li = self.sounds.iter().position(|l| l.which == which)?;
                self.sounds[li].blocks.get(i).map(|m| (self.rows.sound_lane(li), m.from, m.start, m.len, &m.volume[..]))
            }
        }
    }

    /// The own lines under `p`'s row: the cuts' on the clip sound lane, a
    /// sound lane's clips' on it.
    fn owns_at_y(&self, y: f32) -> Vec<Own> {
        if self.rows.on_clip_sound(y) {
            (0..self.blocks.len()).map(Own::Cut).collect()
        } else if let Some(li) = self.rows.sound_at_y(y) {
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

    /// A point on `line` at `p`: its time (edit time on a track's line,
    /// file time on an own line) and level.
    fn line_point(&self, line: crate::project::Line, p: Point, b: Rectangle) -> Option<(f64, f64)> {
        match line {
            crate::project::Line::Own(own) => self.own_point(own, p, b),
            crate::project::Line::Track(which) => {
                let li = self.sounds.iter().position(|l| l.which == which)?;
                Some((self.t(p.x, b), Self::volume_db(self.rows.sound_lane(li), p.y)))
            }
        }
    }

    /// Adds a point to `line` at `p` and drags it on from there.
    fn add_point(&self, drag: &mut Drag, line: crate::project::Line, p: Point, b: Rectangle) -> Option<Message> {
        let (t, db) = self.line_point(line, p, b)?;
        let keys = match line {
            crate::project::Line::Own(own) => self.own_place(own)?.4,
            crate::project::Line::Track(which) => &self.sounds.iter().find(|l| l.which == which)?.track.keyframes[..],
        };
        *drag = Drag::Point { line, key: keys.partition_point(|k| k.t <= t) };
        Some(Message::PointAdd(line, t, db))
    }

    /// A new point on `own` at `p`: its file time and level.
    fn own_point(&self, own: Own, p: Point, b: Rectangle) -> Option<(f64, f64)> {
        let (lane, from, start, ..) = self.own_place(own)?;
        Some((start + self.t(p.x, b) - from, Self::volume_db(lane, p.y)))
    }

    /// Edit times a sound clip's edges snap to, but for clip `clip` of lane
    /// `lane`'s own: the start, the cut boundaries, the playhead, and every
    /// other sound clip's edges on any lane (an effect lines up with a beat
    /// of the song).
    fn snap_targets(&self, lane: usize, clip: usize) -> Vec<f64> {
        let mut at = vec![0.0, self.position];
        at.extend(self.blocks.iter().flat_map(|c| [c.from, c.from + c.len]));
        for (li, l) in self.sounds.iter().enumerate() {
            at.extend(l.blocks.iter().enumerate().filter(|&(i, _)| (li, i) != (lane, clip)).flat_map(|(_, m)| [m.from, m.from + m.len]));
        }
        at
    }

    /// How far to shift any of `edges` (edit times) onto the nearest snap
    /// target within `SNAP` pixels, or 0.
    fn snap(&self, lane: usize, clip: usize, edges: &[f64], per_px: f64) -> f64 {
        let reach = SNAP as f64 * per_px;
        self.snap_targets(lane, clip)
            .iter()
            .flat_map(|t| edges.iter().map(move |e| t - e))
            .filter(|d| d.abs() < reach)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(0.0)
    }

    /// Where a moved cut would land: before the first cut whose middle is
    /// right of `x`.
    fn drop_at(&self, x: f32, b: Rectangle) -> usize {
        self.blocks.iter().filter(|c| self.x(c.from + c.len / 2.0, b) < x).count()
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
        iced::widget::column![lane(Track::Text), lane(Track::Overlay), iced::widget::container(layers).clip(true)].into()
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
        let colour = match which {
            SoundTrack::Music => palette.success.base.color,
            SoundTrack::Sfx => SFX_COLOUR,
            SoundTrack::Voice => VOICE_COLOUR,
        };
        let (top, h) = (self.rows.sound_top(li) + 2.0, self.rows.music - 4.0);
        let lane = Path::rounded_rectangle(Point::new(self.x(0.0, b), top), Size::new((self.x(self.duration, b) - self.x(0.0, b)).max(0.0), h), 4.0.into());
        frame.fill(&lane, palette.background.weak.color.scale_alpha(0.5));
        for (i, m) in blocks.iter().enumerate() {
            let (l, r) = (self.x(m.from, b) + GAP / 2.0, self.x(m.from + m.len, b) - GAP / 2.0);
            if r < 0.0 || l > b.width {
                continue;
            }
            let block = Path::rounded_rectangle(Point::new(l, top), Size::new((r - l).max(1.0), h), 4.0.into());
            let selected = Some((*which, i)) == self.selected_music;
            frame.fill(&block, if selected { colour } else { colour.scale_alpha(0.55) });
            // The waveform as it's heard: the track's level and line, the
            // clip's own, its fades and crossfades (`mixdown::clip_gain`).
            if let Some(w) = &m.wave {
                let heard = |t: f64| crate::mixdown::clip_gain(track, i, m.from + t - m.start, self.video) as f32;
                let wave = Wave { left: l, right: r, top, height: h, start: m.start, len: m.len };
                wave.draw(frame, b, w, m.even_wave.as_ref().unwrap_or(w), heard, palette.background.base.text.scale_alpha(0.7));
            }
            draw_fades(frame, (l, r), top, h, m.len, (m.fade_in, m.fade_out));
            if selected {
                frame.stroke(&block, Stroke::default().with_width(2.0).with_color(palette.primary.strong.color));
            }
            for x in [l + 4.0, r - 4.0] {
                frame.fill(&Path::rounded_rectangle(Point::new(x - 1.0, top + 4.0), Size::new(2.0, h - 8.0), 1.0.into()), iced::Color::from_rgba(1.0, 1.0, 1.0, 0.8));
            }
            // On a dark tag, as on the cuts, so it reads over the waveform.
            if r - l > m.label.chars().count() as f32 * 6.0 + 28.0 {
                // At the bottom: the volume line runs along the top.
                let tag = Path::rounded_rectangle(Point::new(l + 9.0, top + h - 18.0), Size::new(m.label.chars().count() as f32 * 5.5 + 8.0, 16.0), 3.0.into());
                frame.fill(&tag, iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6));
                frame.fill_text(Text {
                    content: m.label.clone(),
                    position: Point::new(l + 13.0, top + h - 10.0),
                    color: iced::Color::WHITE,
                    size: 11.0.into(),
                    align_y: Vertical::Center,
                    ..Default::default()
                });
            }
        }
        // Overlaps are crossfades: an X across each, as editors draw them.
        for (i, a) in blocks.iter().enumerate() {
            for c in &blocks[i + 1..] {
                let (t0, t1) = (a.from.max(c.from), (a.from + a.len).min(c.from + c.len));
                if t1 - t0 <= 1e-6 {
                    continue;
                }
                let (l, r) = (self.x(t0, b), self.x(t1, b));
                frame.fill(&Path::rectangle(Point::new(l, top), Size::new(r - l, h)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35));
                let x = Path::new(|p| {
                    p.move_to(Point::new(l, top + h));
                    p.line_to(Point::new(r, top));
                    p.move_to(Point::new(l, top));
                    p.line_to(Point::new(r, top + h));
                });
                frame.stroke(&x, Stroke::default().with_width(1.5).with_color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.85)));
            }
        }
        // Past the video's end the sound fades out in the export: dim it,
        // and mark the end.
        if self.duration > self.video + 1e-6 {
            let x = self.x(self.video, b);
            let to = self.x(self.duration, b);
            frame.fill(&Path::rectangle(Point::new(x, top), Size::new((to - x).max(0.0), h)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55));
            frame.fill(&Path::rectangle(Point::new(x - 1.0, self.rows.sound_top(li)), Size::new(2.0, self.rows.music)), palette.background.base.text.scale_alpha(0.8));
        }
        // Each clip's own line, inside it; the track's over the whole lane.
        for (i, m) in blocks.iter().enumerate() {
            self.draw_own_line(frame, b, Own::Clip(*which, i), self.rows.sound_lane(li), m.from, m.start, m.len, &m.volume);
        }
        self.draw_volume(frame, b, li);
        lane_tag(frame, which.name(), colour, top + h / 2.0);
    }

    /// A sound lane's volume line over it, once there are points: a flat
    /// line along every lane would only be in the way.
    fn draw_volume(&self, frame: &mut Frame, b: Rectangle, li: usize) {
        let lane = &self.sounds[li];
        let keys = &lane.track.keyframes;
        let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return };
        let points: Vec<_> = keys.iter().map(|k| (self.x(k.t, b), k.volume)).collect();
        let ends = ((self.x(0.0, b), first.volume), (self.x(self.duration, b), last.volume));
        let selected = self.selected_point.filter(|p| p.0 == crate::project::Line::Track(lane.which)).map(|p| p.1);
        // Dimmer than a clip's own line: it belongs to the whole track.
        draw_line(frame, b, self.rows.sound_lane(li), MUSIC_LINE.scale_alpha(0.55), ends, &points, selected);
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
        let selected = self.selected_point.filter(|p| p.0 == crate::project::Line::Own(own)).map(|p| p.1);
        let (l, r) = (x(start) + GAP / 2.0, x(end) - GAP / 2.0);
        draw_line(frame, b, lane, SOUND_LINE, ((l, at(start)), (r, at(end))), &points, selected);
    }

    /// The clip sound lane: under each cut, the waveform of what it plays,
    /// and the lane's name.
    fn draw_clip_sound(&self, frame: &mut Frame, theme: &Theme, b: Rectangle) {
        let palette = theme.extended_palette();
        let (top, h) = (self.rows.cuts + 1.0, self.rows.clip_sound - 2.0);
        let lane = Path::rounded_rectangle(Point::new(self.x(0.0, b), top), Size::new((self.x(self.video, b) - self.x(0.0, b)).max(0.0), h), 3.0.into());
        frame.fill(&lane, palette.background.weak.color.scale_alpha(0.35));
        for (i, c) in self.blocks.iter().enumerate() {
            let (l, r) = self.block_x(c, b);
            if r < 0.0 || l > b.width {
                continue;
            }
            let block = Path::rounded_rectangle(Point::new(l, top), Size::new((r - l).max(1.0), h), 3.0.into());
            frame.fill(&block, SOUND_LINE.scale_alpha(if Some(i) == self.selected { 0.35 } else { 0.18 }));
            let Some(w) = &c.wave else { continue };
            // As it's heard: evened out (once that's read), its level and
            // points, the clips' level; what the file holds faint behind.
            let ramp = |t: f64| crate::project::fade_ramp(t - c.start, c.start + c.len - t, c.fade_in, c.fade_out);
            let heard = |t: f64| crate::mixdown::cut_gain(self.clips_db, c.own_db, crate::project::volume_at(&c.sound, t), ramp(t)) as f32;
            let wave = Wave { left: l, right: r, top, height: h, start: c.start, len: c.len };
            wave.draw(frame, b, w, c.even_wave.as_ref().unwrap_or(w), heard, palette.background.base.text.scale_alpha(0.6));
        }
        // Its sound's fades, a handle at each top corner as on sound clips.
        for c in &self.blocks {
            let (l, r) = self.block_x(c, b);
            if r < 0.0 || l > b.width || c.wave.is_none() {
                continue;
            }
            draw_fades(frame, (l, r), top, h, c.len, (c.fade_in, c.fade_out));
        }
        // Each cut's level, where there's room: a tag at its left, so
        // a lowered or muted cut shows in words, not colour alone.
        for c in &self.blocks {
            let (l, r) = self.block_x(c, b);
            let Some(label) = &c.level else { continue };
            let w = label.chars().count() as f32 * 6.0 + 8.0;
            if r - l < w + 12.0 || r < 0.0 || l > b.width {
                continue;
            }
            let x = l.max(PICK + 80.0).min(r - w - 4.0);
            frame.fill(&Path::rounded_rectangle(Point::new(x, top + h / 2.0 - 7.0), Size::new(w, 14.0), 3.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7));
            frame.fill_text(Text { content: label.clone(), position: Point::new(x + 4.0, top + h / 2.0), color: iced::Color::WHITE, size: 10.0.into(), align_y: Vertical::Center, ..Default::default() });
        }
        // Through a crossfade the two cuts' sounds overlap, as their
        // pictures do.
        for c in self.blocks.iter().filter(|c| c.crossfade > 0.0) {
            let (l, r) = (self.x(c.from, b), self.x(c.from + c.crossfade, b));
            frame.fill(&Path::rectangle(Point::new(l, top), Size::new(r - l, h)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35));
        }
        self.draw_sound(frame, b);
        lane_tag(frame, "Clip sound", SOUND_LINE, top + h / 2.0);
    }

    /// Each cut's sound line on the clip sound lane, for cuts with points:
    /// the points within the cut, and the line from its in to its out point.
    fn draw_sound(&self, frame: &mut Frame, b: Rectangle) {
        for (ci, c) in self.blocks.iter().enumerate() {
            self.draw_own_line(frame, b, Own::Cut(ci), self.rows.cut_lane(), c.from, c.start, c.len, &c.sound);
        }
    }

    /// A cut block's left and right edges.
    fn block_x(&self, c: &Block, b: Rectangle) -> (f32, f32) {
        (self.x(c.from, b) + GAP / 2.0, self.x(c.from + c.len, b) - GAP / 2.0)
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
}

impl OverlayLane {
    fn blocks(&self) -> &[OverlayBlock] {
        match self.1 {
            Track::Overlay => &self.0.overlays,
            Track::Text => &self.0.texts,
        }
    }

    fn item_at(&self, x: f32, b: Rectangle) -> Option<usize> {
        self.blocks().iter().position(|o| x >= self.0.x(o.from, b) && x <= self.0.x(o.from + o.len, b))
    }

    fn edge_at(&self, i: usize, x: f32, b: Rectangle) -> Option<bool> {
        let o = &self.blocks()[i];
        let (l, r) = (self.0.x(o.from, b), self.0.x(o.from + o.len, b));
        if r - l < 3.0 * PICK {
            Some(x - l < r - x)
        } else if x - l <= PICK {
            Some(true)
        } else if r - x <= PICK {
            Some(false)
        } else {
            None
        }
    }

    /// The lane, and its keyframe rows once it has items.
    fn height(&self) -> f32 {
        if self.blocks().is_empty() { self.0.rows.overlay } else { self.0.rows.overlay + EFFECTS + KEYS }
    }

    /// The keyframe mark under `p` in its row: the item, whether it's in
    /// the effects row, the keyframe and its edit time.
    fn key_at(&self, p: Point, b: Rectangle) -> Option<(usize, bool, usize, f64)> {
        let effects = (self.0.rows.overlay..self.0.rows.overlay + EFFECTS).contains(&p.y);
        if !effects && !(self.0.rows.overlay + EFFECTS..self.0.rows.overlay + EFFECTS + KEYS).contains(&p.y) {
            return None;
        }
        self.blocks().iter().enumerate().find_map(|(item, o)| {
            let times: Vec<f64> = if effects { o.effects.clone() } else { o.motion.iter().map(|k| k.0).collect() };
            times
                .into_iter()
                .enumerate()
                .filter(|&(_, t)| t >= o.start && t <= o.start + o.len)
                .map(|(key, t)| (key, o.from + t - o.start))
                .find(|&(_, t)| (self.0.x(t, b) - p.x).abs() <= PICK)
                .map(|(key, t)| (item, effects, key, t))
        })
    }
}

impl canvas::Program<Message> for OverlayLane {
    type State = OverlayDrag;

    fn update(&self, drag: &mut OverlayDrag, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::Mouse;
        let t = &self.0;
        match event {
            Mouse(mouse::Event::WheelScrolled { delta }) => {
                let p = cursor.position_in(bounds)?;
                let (dx, dy) = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => (x, y),
                    mouse::ScrollDelta::Pixels { x, y } => (x / 40.0, y / 40.0),
                };
                let message = if dy != 0.0 {
                    Message::TimelineZoom(1.25f64.powf(dy as f64), Some(t.t(p.x, bounds)))
                } else if dx != 0.0 {
                    Message::TimelinePan(t.start - dx as f64 * t.span * 0.1)
                } else {
                    return None;
                };
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
                if let Some((item, effects, key, at)) = self.key_at(p, bounds) {
                    // A jump there, and a drag retimes it.
                    *drag = OverlayDrag::Key { item, effects, key };
                    return Some(canvas::Action::publish(Message::OverlayJump(at)).and_capture());
                }
                // Elsewhere in the keyframe rows: seek there.
                if p.y >= self.0.rows.overlay {
                    return Some(canvas::Action::publish(Message::OverlayJump(t.t(p.x, bounds))).and_capture());
                }
                let Some(item) = self.item_at(p.x, bounds) else {
                    return Some(canvas::Action::publish(Message::OverlaySelect(None)).and_capture());
                };
                let o = &self.blocks()[item];
                let per_px = t.span / Timeline::width(bounds) as f64;
                *drag = match self.edge_at(item, p.x, bounds) {
                    Some(start) => OverlayDrag::Trim { item, start, x0: p.x, from: if start { o.start } else { o.start + o.len }, per_px },
                    None => OverlayDrag::Move { item, x0: p.x, at: o.from, per_px },
                };
                Some(canvas::Action::publish(Message::OverlaySelect(Some((self.1, item)))).and_capture())
            }
            Mouse(mouse::Event::CursorMoved { position }) => {
                let x = position.x - bounds.x;
                let message = match drag {
                    OverlayDrag::None => return None,
                    OverlayDrag::Move { item, x0, at, per_px } => Message::OverlayMove((self.1, *item), *at + (x - *x0) as f64 * *per_px),
                    OverlayDrag::Trim { item, start, x0, from, per_px } => Message::OverlayTrim((self.1, *item), *start, *from + (x - *x0) as f64 * *per_px),
                    OverlayDrag::Key { item, effects, key } => {
                        let o = &self.blocks()[*item];
                        let at = self.0.t(x, bounds).clamp(o.from, o.from + o.len);
                        Message::OverlayKeyMove((self.1, *item), *effects, *key, o.start + at - o.from)
                    }
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => match std::mem::take(drag) {
                OverlayDrag::None => None,
                _ => Some(canvas::Action::publish(Message::ScrubDone).and_capture()),
            },
            _ => None,
        }
    }

    fn draw(&self, _: &OverlayDrag, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let t = &self.0;
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        let (top, h) = (2.0, self.0.rows.overlay - 4.0);
        let lane = Path::rounded_rectangle(Point::new(t.x(0.0, bounds), top), Size::new((t.x(t.duration, bounds) - t.x(0.0, bounds)).max(0.0), h), 4.0.into());
        frame.fill(&lane, palette.background.weak.color.scale_alpha(0.5));
        let (name, colour) = match self.1 {
            Track::Overlay => ("Overlay", OVERLAY_COLOUR),
            Track::Text => ("Text", TEXT_COLOUR),
        };
        for (i, o) in self.blocks().iter().enumerate() {
            let (l, r) = (t.x(o.from, bounds) + GAP / 2.0, t.x(o.from + o.len, bounds) - GAP / 2.0);
            if r < 0.0 || l > bounds.width {
                continue;
            }
            let block = Path::rounded_rectangle(Point::new(l, top), Size::new((r - l).max(1.0), h), 4.0.into());
            let selected = Some((self.1, i)) == t.selected_overlay;
            frame.fill(&block, if selected { colour } else { colour.scale_alpha(0.55) });
            if selected {
                frame.stroke(&block, Stroke::default().with_width(2.0).with_color(palette.primary.strong.color));
            }
            for x in [l + 4.0, r - 4.0] {
                frame.fill(&Path::rounded_rectangle(Point::new(x - 1.0, top + 4.0), Size::new(2.0, h - 8.0), 1.0.into()), iced::Color::from_rgba(1.0, 1.0, 1.0, 0.8));
            }
            // The picture, then its name, where there's room.
            let mut text_x = l + 9.0;
            if let Some(thumb) = &o.thumb
                && r - l > 3.0 * h
            {
                frame.draw_image(Rectangle::new(Point::new(l + 8.0, top + 2.0), Size::new(h * 1.5, h - 4.0)), canvas::Image::new(thumb.clone()));
                text_x += h * 1.5 + 2.0;
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
            draw_rows(&mut frame, theme, t, bounds, self.0.rows.overlay, Some("Motion"));
            let inside = |o: &OverlayBlock, k: f64| k >= o.start && k <= o.start + o.len;
            for o in self.blocks() {
                for &k in o.effects.iter().filter(|&&k| inside(o, k)) {
                    let x = t.x(o.from + k - o.start, bounds);
                    let mark = Path::rectangle(Point::new(x - 3.5, self.0.rows.overlay + EFFECTS / 2.0 - 3.5), Size::new(7.0, 7.0));
                    frame.fill(&mark, EFFECT_MARK);
                    frame.stroke(&mark, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
                }
                for &(k, ease) in o.motion.iter().filter(|(k, _)| inside(o, *k)) {
                    let mark = key_mark(t.x(o.from + k - o.start, bounds), self.0.rows.overlay + EFFECTS + KEYS / 2.0, ease);
                    frame.fill(&mark, colour);
                    frame.stroke(&mark, Stroke::default().with_width(1.5).with_color(palette.background.base.color));
                }
            }
        }
        lane_tag(&mut frame, name, colour, top + h / 2.0);
        let at = t.x(t.position, bounds);
        frame.fill(&Path::rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, self.height())), palette.primary.strong.color);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, drag: &OverlayDrag, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        match drag {
            OverlayDrag::Trim { .. } => mouse::Interaction::ResizingHorizontally,
            OverlayDrag::Move { .. } | OverlayDrag::Key { .. } => mouse::Interaction::Grabbing,
            OverlayDrag::None => {
                let Some(p) = cursor.position_in(bounds) else { return mouse::Interaction::default() };
                if p.y >= self.0.rows.overlay {
                    // A mark can be dragged; elsewhere in the rows a click seeks.
                    return if self.key_at(p, bounds).is_some() { mouse::Interaction::Grab } else { mouse::Interaction::Pointer };
                }
                match self.item_at(p.x, bounds) {
                    Some(i) if self.edge_at(i, p.x, bounds).is_some() => mouse::Interaction::ResizingHorizontally,
                    Some(_) => mouse::Interaction::Grab,
                    None => mouse::Interaction::default(),
                }
            }
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
            frame.fill(&block, if Some(i) == self.0.selected { palette.primary.weak.color } else { palette.background.strong.color });
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
            let per_px = c.len / (r - l) as f64;
            frame.with_clip(clip, |f| {
                // From the first tile in sight: zoomed in, a block can be
                // far wider than the widget.
                let mut x = l + inset + ((-(l + inset)) / tw).floor().max(0.0) * tw;
                while x < clip.x + clip.width {
                    let t = c.start + (x + tw / 2.0 - l) as f64 * per_px;
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

/// The two keyframe rows under a track from `top`: faint bands with their
/// names at the left, effects above, `keys` (view or motion) below.
/// The keyframe rows' bands under a track, over the edit's length (`t`)
/// like the lanes and the seek bar, not the widget's whole width.
fn draw_rows(frame: &mut Frame, theme: &Theme, t: &Timeline, b: Rectangle, top: f32, keys: Option<&str>) {
    let palette = theme.extended_palette();
    let (l, r) = (t.x(0.0, b).max(0.0), t.x(t.duration, b).min(b.width));
    let rows = [Some((top, EFFECTS, "Effects")), keys.map(|k| (top + EFFECTS, KEYS, k))];
    for (y, h, name) in rows.into_iter().flatten() {
        frame.fill(&Path::rectangle(Point::new(l, y + 1.0), Size::new((r - l).max(0.0), h - 2.0)), palette.background.weak.color.scale_alpha(0.25));
        frame.fill_text(Text {
            content: name.to_owned(),
            position: Point::new(l.max(PICK) + 12.0, y + h / 2.0),
            color: palette.background.base.text.scale_alpha(0.35),
            size: 10.0.into(),
            align_y: Vertical::Center,
            ..Default::default()
        });
    }
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

impl canvas::Program<Message> for Marks {
    type State = State;

    fn update(&self, state: &mut State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<canvas::Action<Message>> {
        use iced::Event::{Keyboard, Mouse};
        let drag = &mut state.drag;
        match event {
            Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                state.shift = m.shift();
                state.ctrl = m.command();
                None
            }
            Mouse(mouse::Event::WheelScrolled { delta }) => {
                let p = cursor.position_in(bounds)?;
                let (dx, dy) = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => (x, y),
                    mouse::ScrollDelta::Pixels { x, y } => (x / 40.0, y / 40.0),
                };
                let (pan, zoom) = if state.shift { (dx + dy, 0.0) } else { (dx, dy) };
                let message = if zoom != 0.0 {
                    Message::TimelineZoom(1.25f64.powf(zoom as f64), Some(self.t(p.x, bounds)))
                } else if pan != 0.0 {
                    Message::TimelinePan(self.start - pan as f64 * self.span * 0.1)
                } else {
                    return None;
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            // Right-click a cut: what file it is; a cut's sound or a sound
            // clip: its menu, where the pointer is (Info is in it); a sound
            // lane's empty stretch: the track's menu.
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                use super::sound_ui::{Menu, SoundMsg};
                use crate::project::Line;
                let p = cursor.position_in(bounds)?;
                // A point or a line first: each has a menu of its own.
                let track = self.rows.sound_at_y(p.y).map(|lane| (lane, self.sounds[lane].which));
                let menu = if let Some((own, i)) = self.sound_at(p, bounds) {
                    Menu::Point(Line::Own(own), i)
                } else if let Some((i, which)) = track.and_then(|(lane, which)| Some((self.volume_at(lane, p, bounds)?, which))) {
                    Menu::Point(Line::Track(which), i)
                } else if let Some(own) = self.on_sound_line(p, bounds) {
                    Menu::Line(Line::Own(own))
                } else if let Some((_, which)) = track.filter(|&(lane, _)| self.on_track_line(lane, p, bounds)) {
                    Menu::Line(Line::Track(which))
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
            Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                let message = if p.y >= self.rows.height - SCROLL {
                    if !self.zoomed() {
                        return None;
                    }
                    // On the thumb, drag it; elsewhere on the bar, centre it there first.
                    let (l, r) = self.scroll_thumb(bounds);
                    let per_px = self.duration / Timeline::width(bounds) as f64;
                    let start = if p.x >= l && p.x <= r { self.start } else { self.start + (p.x - (l + r) / 2.0) as f64 * per_px };
                    *drag = Drag::Scroll { x0: p.x, start };
                    Message::TimelinePan(start)
                } else if let Some((cut, key, t)) = self.effect_at(p, bounds) {
                    // A jump there, and a drag retimes it.
                    *drag = Drag::Effect { cut, key };
                    Message::Effect(super::effects::EffectMsg::Jump(cut, t))
                } else if self.rows.on_clip_sound(p.y) && let Some((cut, in_edge)) = self.cut_fade_at(p, bounds) {
                    let c = &self.blocks[cut];
                    *drag = Drag::Fade { own: Own::Cut(cut), in_edge, x0: p.x, from: if in_edge { c.fade_in } else { c.fade_out }, per_px: self.span / Timeline::width(bounds) as f64 };
                    Message::Select(cut)
                } else if state.ctrl && let Some((own, key)) = self.sound_at(p, bounds) {
                    // Ctrl+click on a point takes it away, on any lane.
                    Message::Sound(super::sound_ui::SoundMsg::DeletePoint(crate::project::Line::Own(own), key))
                } else if state.ctrl && let Some(lane) = self.rows.sound_at_y(p.y) && let Some(key) = self.volume_at(lane, p, bounds) {
                    Message::Sound(super::sound_ui::SoundMsg::DeletePoint(crate::project::Line::Track(self.sounds[lane].which), key))
                } else if self.rows.on_clip_sound(p.y) && let Some((own, key)) = self.sound_at(p, bounds) {
                    let line = crate::project::Line::Own(own);
                    *drag = Drag::Point { line, key };
                    Message::PointSelect(line, key)
                } else if self.rows.on_clip_sound(p.y) && let Some(own) = self.on_sound_line(p, bounds).or_else(|| state.ctrl.then(|| self.block_at(p.x, bounds).map(Own::Cut)).flatten()) {
                    // A sound point on the cut where clicked (Ctrl, or on
                    // its line once it shows), dragged on.
                    self.add_point(drag, crate::project::Line::Own(own), p, bounds)?
                } else if self.rows.on_clip_sound(p.y) {
                    // Elsewhere on the lane: that cut, as a click on it.
                    Message::Select(self.block_at(p.x, bounds)?)
                } else if p.y < self.rows.cuts {
                    let per_px = self.span / Timeline::width(bounds) as f64;
                    let cut = match self.edge_hit(p.x, bounds) {
                        // Shift on a start edge: how far it overlaps the
                        // cut before, a crossfade.
                        Some((cut, true)) if state.shift && cut > 0 => {
                            *drag = Drag::Crossfade { cut, x0: p.x, from: self.blocks[cut].crossfade, per_px };
                            cut
                        }
                        Some((cut, start)) => {
                            let c = &self.blocks[cut];
                            *drag = Drag::Trim { cut, start, x0: p.x, from: if start { c.start } else { c.start + c.len }, per_px };
                            cut
                        }
                        None => {
                            let cut = self.block_at(p.x, bounds)?;
                            *drag = Drag::Move { cut, x0: p.x, moved: false, to: cut };
                            cut
                        }
                    };
                    Message::Select(cut)
                } else if let Some(lane) = self.rows.sound_at_y(p.y) {
                    let which = self.sounds[lane].which;
                    if let Some((clip, in_edge)) = self.fade_at(lane, p, bounds) {
                        let m = &self.sounds[lane].blocks[clip];
                        let per_px = self.span / Timeline::width(bounds) as f64;
                        *drag = Drag::Fade { own: Own::Clip(which, clip), in_edge, x0: p.x, from: if in_edge { m.fade_in } else { m.fade_out }, per_px };
                        Message::SelectMusic(which, clip)
                    } else if let Some(line) = self.sound_at(p, bounds).map(|(own, k)| (crate::project::Line::Own(own), k)).or_else(|| Some((crate::project::Line::Track(which), self.volume_at(lane, p, bounds)?))) {
                        // A point, a clip's own first: drag it.
                        let (line, key) = line;
                        *drag = Drag::Point { line, key };
                        Message::PointSelect(line, key)
                    } else if let Some(own) = self.on_sound_line(p, bounds) {
                        self.add_point(drag, crate::project::Line::Own(own), p, bounds)?
                    } else if state.ctrl {
                        // Ctrl on a clip: a point on its own line; off the
                        // clips, on the track's. Dragged on from there.
                        let line = self.music_at(lane, p.x, bounds).map_or(crate::project::Line::Track(which), |clip| crate::project::Line::Own(Own::Clip(which, clip)));
                        self.add_point(drag, line, p, bounds)?
                    } else {
                        let clip = self.music_at(lane, p.x, bounds)?;
                        let m = &self.sounds[lane].blocks[clip];
                        let per_px = self.span / Timeline::width(bounds) as f64;
                        *drag = match self.music_edge_at(lane, clip, p.x, bounds) {
                            Some(start) => Drag::MusicTrim { lane, clip, start, x0: p.x, from: if start { m.start } else { m.start + m.len }, per_px },
                            None => Drag::MusicMove { lane, clip, x0: p.x, at: m.from, per_px },
                        };
                        Message::SelectMusic(which, clip)
                    }
                } else if (self.rows.keys_top..self.rows.seek_top).contains(&p.y) && let Some((cut, key)) = self.key_at(p.x, bounds) {
                    *drag = Drag::Key { cut, key };
                    Message::KeyJump(cut, key)
                } else {
                    *drag = Drag::Playhead;
                    Message::Scrub(self.t(p.x, bounds))
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            Mouse(mouse::Event::CursorMoved { position }) => {
                let x = position.x - bounds.x;
                let message = match drag {
                    Drag::None => return None,
                    Drag::Playhead => Message::Scrub(self.t(x, bounds)),
                    Drag::Key { cut, key } => {
                        let c = &self.blocks[*cut];
                        let edit_t = self.t(x, bounds).clamp(c.from, c.from + c.len);
                        Message::KeyMove(*cut, *key, c.start + edit_t - c.from)
                    }
                    Drag::Effect { cut, key } => {
                        let c = &self.blocks[*cut];
                        let edit_t = self.t(x, bounds).clamp(c.from, c.from + c.len);
                        Message::Effect(super::effects::EffectMsg::KeyMove(*cut, *key, c.start + edit_t - c.from))
                    }
                    Drag::Trim { cut, start, x0, from, per_px } => Message::Trim(*cut, *start, *from + (x - *x0) as f64 * *per_px),
                    Drag::Move { x0, moved, to, .. } => {
                        *moved |= (x - *x0).abs() > DRAG_FROM;
                        *to = self.drop_at(x, bounds);
                        return Some(canvas::Action::request_redraw().and_capture());
                    }
                    Drag::Scroll { x0, start } => Message::TimelinePan(*start + (x - *x0) as f64 * self.duration / Timeline::width(bounds) as f64),
                    Drag::MusicMove { lane, clip, x0, at, per_px } => {
                        let at = *at + (x - *x0) as f64 * *per_px;
                        let len = self.sounds[*lane].blocks[*clip].len;
                        Message::MusicMove(self.sounds[*lane].which, *clip, at + self.snap(*lane, *clip, &[at, at + len], *per_px), state.shift)
                    }
                    Drag::Crossfade { cut, x0, from, per_px } => Message::Crossfade(*cut, *from + (*x0 - x) as f64 * *per_px),
                    Drag::Fade { own, in_edge, x0, from, per_px } => {
                        // Rightwards lengthens a fade in, leftwards a fade out.
                        let dx = (x - *x0) as f64 * *per_px;
                        Message::Fade(*own, *in_edge, *from + if *in_edge { dx } else { -dx })
                    }
                    Drag::Point { line, key } => {
                        let (t, db) = self.line_point(*line, Point::new(x, position.y - bounds.y), bounds)?;
                        Message::PointMove(*line, *key, t, db)
                    }
                    Drag::MusicTrim { lane, clip, start, x0, from, per_px } => {
                        // File time to edit time is fixed while trimming:
                        // either edge moves, the sound stays put.
                        let m = &self.sounds[*lane].blocks[*clip];
                        let t = *from + (x - *x0) as f64 * *per_px;
                        let to_edit = m.from - m.start;
                        Message::MusicTrim(self.sounds[*lane].which, *clip, *start, t + self.snap(*lane, *clip, &[t + to_edit], *per_px), state.shift)
                    }
                };
                Some(canvas::Action::publish(message).and_capture())
            }
            Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => match std::mem::take(drag) {
                Drag::None => None,
                Drag::Move { cut, moved: true, to, .. } => Some(canvas::Action::publish(Message::MoveCut(cut, to)).and_capture()),
                Drag::Move { .. } | Drag::Scroll { .. } => Some(canvas::Action::request_redraw().and_capture()),
                Drag::Playhead | Drag::Key { .. } | Drag::Effect { .. } | Drag::Trim { .. } | Drag::MusicMove { .. } | Drag::MusicTrim { .. } | Drag::Fade { .. } | Drag::Point { .. } | Drag::Crossfade { .. } => {
                    Some(canvas::Action::publish(Message::ScrubDone).and_capture())
                }
            },
            _ => None,
        }
    }

    fn draw(&self, state: &State, renderer: &Renderer, theme: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let drag = &state.drag;
        let palette = theme.extended_palette();
        let mut frame = Frame::new(renderer, bounds.size());
        draw_rows(&mut frame, theme, self, bounds, self.rows.effects_top, (self.rows.keys > 0.0).then_some("View"));
        for (i, c) in self.blocks.iter().enumerate() {
            let (l, r) = self.block_x(c, bounds);
            let block = Path::rounded_rectangle(Point::new(l, 0.0), Size::new((r - l).max(1.0), self.rows.cuts), 4.0.into());
            let selected = Some(i) == self.selected;
            if selected {
                frame.stroke(&block, Stroke::default().with_width(2.0).with_color(palette.primary.strong.color));
            }
            // Trim handles, light on a dark edge so they show on any picture.
            for x in [l + 4.0, r - 4.0] {
                frame.fill(&Path::rounded_rectangle(Point::new(x - 2.0, 5.0), Size::new(4.0, self.rows.cuts - 10.0), 2.0.into()), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5));
                frame.fill(&Path::rounded_rectangle(Point::new(x - 1.0, 6.0), Size::new(2.0, self.rows.cuts - 12.0), 1.0.into()), iced::Color::from_rgba(1.0, 1.0, 1.0, 0.8));
            }
            // The longest label that fits (about 7 px a character), on a
            // dark tag, readable over any picture.
            if let Some(label) = c.labels.iter().find(|s| r - l > s.chars().count() as f32 * 7.0 + 24.0) {
                let tag = Path::rounded_rectangle(Point::new(l + 9.0, self.rows.cuts - 19.0), Size::new(label.chars().count() as f32 * 5.9 + 8.0, 15.0), 3.0.into());
                frame.fill(&tag, iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6));
                frame.fill_text(Text {
                    content: label.clone(),
                    position: Point::new(l + 13.0, self.rows.cuts - 11.5),
                    color: iced::Color::WHITE,
                    size: 12.0.into(),
                    align_y: Vertical::Center,
                    ..Default::default()
                });
            }
        }
        // Effect keyframes: small squares along the bottom of their cut.
        for c in &self.blocks {
            for &t in c.effects.iter().filter(|&&t| t >= c.start && t <= c.start + c.len) {
                let x = self.x(c.from + t - c.start, bounds);
                let mark = Path::rectangle(Point::new(x - 3.5, self.rows.effects_top + EFFECTS / 2.0 - 3.5), Size::new(7.0, 7.0));
                frame.fill(&mark, EFFECT_MARK);
                frame.stroke(&mark, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
            }
        }
        // Crossfades: an X over each overlap, as on the music lane.
        for c in self.blocks.iter().filter(|c| c.crossfade > 0.0) {
            let (l, r) = (self.x(c.from, bounds), self.x(c.from + c.crossfade, bounds));
            frame.fill(&Path::rectangle(Point::new(l, 0.0), Size::new(r - l, self.rows.cuts)), iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35));
            let x = Path::new(|p| {
                p.move_to(Point::new(l, self.rows.cuts));
                p.line_to(Point::new(r, 0.0));
                p.move_to(Point::new(l, 0.0));
                p.line_to(Point::new(r, self.rows.cuts));
            });
            frame.stroke(&x, Stroke::default().with_width(1.5).with_color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.85)));
        }
        if let Drag::Move { moved: true, to, .. } = drag {
            let t = self.blocks.get(*to).map_or(self.duration, |c| c.from);
            let x = self.x(t, bounds);
            frame.fill(&Path::rectangle(Point::new(x - 1.5, 0.0), Size::new(3.0, self.rows.cuts)), palette.primary.strong.color);
        }

        self.draw_sounds(&mut frame, theme, bounds);
        let mid = self.rows.seek_top + SEEK / 2.0;
        let keys_mid = self.rows.keys_top + KEYS / 2.0;
        let (x0, x1) = (self.x(0.0, bounds), self.x(self.duration, bounds));
        let bar = |from: f32, to: f32| Path::rounded_rectangle(Point::new(from, mid - BAR / 2.0), Size::new((to - from).max(0.0), BAR), (BAR / 2.0).into());
        frame.fill(&bar(x0, x1), palette.background.strong.color);
        let at = self.x(self.position, bounds);
        frame.fill(&bar(x0, at), palette.primary.base.color);
        // Where one cut ends and the next begins.
        for c in self.blocks.iter().skip(1) {
            let x = self.x(c.from, bounds);
            frame.fill(&Path::rectangle(Point::new(x - 1.0, mid - BAR), Size::new(2.0, 2.0 * BAR)), palette.background.base.color);
        }
        // The view's keyframes, while framing (their row is gone otherwise).
        for (ci, c) in self.blocks.iter().enumerate().filter(|_| self.rows.keys > 0.0) {
            for &(ki, t, ease, follows) in &c.keys {
                let (x, mid) = (self.x(c.from + t - c.start, bounds), keys_mid);
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
        self.draw_clip_sound(&mut frame, theme, bounds);
        frame.fill(&Path::rectangle(Point::new(at - 1.0, 0.0), Size::new(2.0, self.rows.seek_top)), palette.primary.strong.color);
        frame.fill(&Path::rectangle(Point::new(at - 1.0, self.rows.sounds_top), Size::new(2.0, SOUND_LANES as f32 * self.rows.music)), palette.primary.strong.color);
        frame.fill(&Path::circle(Point::new(at, mid), BAR), palette.primary.strong.color);
        if self.zoomed() {
            let y = self.rows.height - SCROLL / 2.0 - SCROLL_BAR / 2.0;
            let pill = |l: f32, r: f32| Path::rounded_rectangle(Point::new(l, y), Size::new((r - l).max(SCROLL_BAR), SCROLL_BAR), (SCROLL_BAR / 2.0).into());
            frame.fill(&pill(PICK, PICK + Timeline::width(bounds)), palette.background.strong.color);
            let (l, r) = self.scroll_thumb(bounds);
            let active = matches!(drag, Drag::Scroll { .. });
            frame.fill(&pill(l, r), if active { palette.primary.strong.color } else { palette.background.base.text.scale_alpha(0.5) });
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        match state.drag {
            Drag::Trim { .. } | Drag::MusicTrim { .. } | Drag::Fade { .. } | Drag::Crossfade { .. } => mouse::Interaction::ResizingHorizontally,
            Drag::None => {
                let Some(p) = cursor.position_in(bounds) else { return mouse::Interaction::default() };
                if p.y >= self.rows.height - SCROLL {
                    return if self.zoomed() { mouse::Interaction::Grab } else { mouse::Interaction::default() };
                }
                if let Some(lane) = self.rows.sound_at_y(p.y) {
                    if self.fade_at(lane, p, bounds).is_some() {
                        return mouse::Interaction::ResizingHorizontally;
                    }
                    if self.sound_at(p, bounds).is_some() || self.volume_at(lane, p, bounds).is_some() {
                        return mouse::Interaction::Grab;
                    }
                    if state.ctrl || self.on_sound_line(p, bounds).is_some() {
                        return mouse::Interaction::Crosshair;
                    }
                    return match self.music_at(lane, p.x, bounds) {
                        Some(i) if self.music_edge_at(lane, i, p.x, bounds).is_some() => mouse::Interaction::ResizingHorizontally,
                        Some(_) => mouse::Interaction::Grab,
                        None => mouse::Interaction::default(),
                    };
                }
                if self.rows.on_clip_sound(p.y) {
                    return if self.cut_fade_at(p, bounds).is_some() {
                        mouse::Interaction::ResizingHorizontally
                    } else if self.sound_at(p, bounds).is_some() {
                        mouse::Interaction::Grab
                    } else if (state.ctrl && self.block_at(p.x, bounds).is_some()) || self.on_sound_line(p, bounds).is_some() {
                        mouse::Interaction::Crosshair
                    } else if self.block_at(p.x, bounds).is_some() {
                        mouse::Interaction::Pointer
                    } else {
                        mouse::Interaction::default()
                    };
                }
                if p.y >= self.rows.cuts {
                    // Marks in either row drag to another time.
                    let on = if p.y < self.rows.keys_top { self.effect_at(p, bounds).is_some() } else { p.y < self.rows.seek_top && self.key_at(p.x, bounds).is_some() };
                    return if on { mouse::Interaction::Grab } else { mouse::Interaction::Pointer };
                }
                if self.edge_hit(p.x, bounds).is_some() {
                    return mouse::Interaction::ResizingHorizontally;
                }
                match self.block_at(p.x, bounds) {
                    Some(_) => mouse::Interaction::Grab,
                    None => mouse::Interaction::default(),
                }
            }
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
    fn draw(&self, frame: &mut Frame, b: Rectangle, file: &music::Waveform, heard: &music::Waveform, gain: impl Fn(f64) -> f32, colour: iced::Color) {
        let (l, r, h) = (self.left, self.right, self.height - 4.0);
        let per_px = self.len / (r - l).max(1.0) as f64;
        let mid = self.top + self.height / 2.0;
        let peak = |w: &music::Waveform, t0: f64, t1: f64| {
            let (i0, i1) = ((t0 / music::WAVE_STEP) as usize, ((t1 / music::WAVE_STEP) as usize).max((t0 / music::WAVE_STEP) as usize + 1));
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
        let knob = Path::circle(Point::new(end, top + 4.0), 3.5);
        frame.fill(&knob, iced::Color::WHITE);
        frame.stroke(&knob, Stroke::default().with_width(1.0).with_color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Line, VolumeKey};

    fn block(from: f64, len: f64, crossfade: f64) -> Block {
        Block {
            labels: vec![], from, len, start: 0.0, keys: vec![], thumbs: vec![], sound: vec![], wave: None, even_wave: None,
            own_db: 0.0, fade_in: 0.0, fade_out: 0.0, level: None, crossfade, effects: vec![],
        }
    }

    /// Cuts a 0..4 and b 3..7 (crossfading 3..4), a 7..10; on Voice a clip
    /// 2..6 with a point; on a 1000 px window showing 0..10 s.
    fn timeline() -> (Timeline, Rectangle) {
        let mut sounds: Vec<SoundLane> = SoundTrack::ALL.iter().map(|&which| SoundLane { which, track: Default::default(), blocks: vec![] }).collect();
        let voice = sounds.iter_mut().find(|l| l.which == SoundTrack::Voice).unwrap();
        voice.blocks.push(MusicBlock { label: "take".into(), from: 2.0, start: 1.0, len: 4.0, wave: None, fade_in: 1.0, fade_out: 0.0, volume: vec![VolumeKey { t: 3.0, volume: 0.0 }], even_wave: None });
        let t = Timeline {
            blocks: vec![block(0.0, 4.0, 0.0), block(3.0, 4.0, 1.0), block(7.0, 3.0, 0.0)],
            sounds,
            selected_music: None, selected_point: None,
            video: 10.0, clips_db: 0.0, duration: 10.0, start: 0.0, span: 10.0, position: 0.0,
            selected: None, current: None, overlays: vec![], texts: vec![], selected_overlay: None,
            rows: Rows::new(1.0, false),
        };
        (t, Rectangle::new(Point::ORIGIN, Size::new(1000.0 + 2.0 * PICK, 300.0)))
    }

    #[test]
    fn the_rows_stack_top_to_bottom_at_every_scale() {
        for scale in [0.5, 0.75, 1.0, 2.0, 3.0, 5.0] {
            for framing in [false, true] {
                let r = Rows::new(scale, framing);
                assert!(r.cuts < r.effects_top && r.effects_top < r.seek_top && r.seek_top < r.sounds_top && r.sounds_top < r.height, "{scale} {framing}: {r:?}");
                assert_eq!(r.keys > 0.0, framing, "the View row only while framing");
                for i in 0..SOUND_LANES {
                    let middle = r.sound_top(i) + r.music / 2.0;
                    assert_eq!(r.sound_at_y(middle), Some(i));
                }
                assert_eq!(r.sound_at_y(r.sounds_top - 1.0), None);
                assert_eq!(r.sound_at_y(r.height - SCROLL + 1.0), None, "the scrollbar isn't a lane");
                assert!(r.on_clip_sound(r.cuts + 1.0) && !r.on_clip_sound(r.effects_top));
            }
        }
        // Scaled outside its range, it holds at the ends.
        assert_eq!(Rows::new(10.0, false).cuts, Rows::new(3.0, false).cuts);
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
    fn a_press_on_a_sound_clip_finds_its_fade_its_point_and_its_line() {
        let (t, b) = timeline();
        let li = t.sounds.iter().position(|l| l.which == SoundTrack::Voice).unwrap();
        let x = |s: f64| t.x(s, b);
        let lane = t.rows.sound_lane(li);
        // Its fade-in handle along the top, where the 1 s fade ends.
        assert_eq!(t.fade_at(li, Point::new(x(3.0), t.rows.sound_top(li) + 4.0), b), Some((0, true)));
        // Its point (file time 3 s is edit time 4 s), and its line.
        let own = Own::Clip(SoundTrack::Voice, 0);
        assert_eq!(t.sound_at(Point::new(x(4.0), Timeline::volume_y(lane, 0.0)), b), Some((own, 0)));
        assert_eq!(t.on_sound_line(Point::new(x(5.0), Timeline::volume_y(lane, 0.0)), b), Some(own));
        assert_eq!(t.on_sound_line(Point::new(x(5.0), Timeline::volume_y(lane, -20.0)), b), None);
        // A new point there: the file's own time.
        let (at, db) = t.own_point(own, Point::new(x(5.0), Timeline::volume_y(lane, -6.0)), b).unwrap();
        assert!((at - 4.0).abs() < 0.02 && (db + 6.0).abs() < 0.2, "{at} {db}");
        // A new point: on the clip, its own line, in file time; off the
        // clips, the track's line, in edit time; both dragged on.
        let mut drag = Drag::None;
        let y = Timeline::volume_y(lane, -6.0);
        let on_clip = t.add_point(&mut drag, Line::Own(own), Point::new(x(5.0), y), b);
        assert!(matches!(on_clip, Some(Message::PointAdd(Line::Own(o), at, _)) if o == own && (at - 4.0).abs() < 0.02));
        assert!(matches!(drag, Drag::Point { line: Line::Own(_), key: 1 }), "after its point at 3 s");
        let off = t.add_point(&mut drag, Line::Track(SoundTrack::Voice), Point::new(x(8.0), y), b);
        assert!(matches!(off, Some(Message::PointAdd(Line::Track(SoundTrack::Voice), at, db)) if (at - 8.0).abs() < 0.02 && (db + 6.0).abs() < 0.2));
        // Off the clips there's no clip, and its edges are its edges.
        assert_eq!(t.music_at(li, x(8.0), b), None);
        assert_eq!(t.music_edge_at(li, 0, x(6.0) - 1.0, b), Some(false));
    }
}
