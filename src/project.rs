//! An edit: cuts from one or more clips played one after another, each with
//! its own keyframed views. The player's timeline edits it and `export`
//! renders it.
//!
//! Keyframes belong to their cut and are pinned to clip time, not to the
//! timeline: moving a cut keeps its camera moves, and trimming one keeps
//! each keyframe on the same moment of footage. Keyframes outside the cut's
//! in and out points are kept, so extending the cut again brings them back.
//!
//! The sound tracks (`SoundTrack`: the music, the sound effects) are the
//! other way round: their clips sit at times on the edit, so cutting the
//! video to the music leaves the music where it is. Each is cut on its own
//! lane, and the edit ends with the last video cut, a track faded out there
//! if it runs on. A cut's own sound has its level (`gain`, normalized to
//! −16 LUFS by `loudness`), `mute` and `even` (evened out), and plays a
//! cleaned voice in place of the clip's where `audio` names one.
//!
//! This is the public file format: new fields are optional, so an older
//! project always loads.

use crate::stab::Mode;
use crate::view::{Aim, Aspect, Camera, Keyframes, Projection, View};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The picture's shape, width:height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect: Option<Aspect>,
    /// None yet when a project starts from its music.
    #[serde(default)]
    pub cuts: Vec<Cut>,
    /// The clips' own sound in the mix.
    #[serde(default, skip_serializing_if = "Sound::is_default")]
    pub sound: Sound,
    #[serde(default, skip_serializing_if = "Music::is_empty")]
    pub music: Music,
    /// Sound effects: a track like the music's, mixed in beside it.
    #[serde(default, skip_serializing_if = "Music::is_empty")]
    pub sfx: Music,
    /// More sound effects, playing over those on `sfx`.
    #[serde(default, skip_serializing_if = "Music::is_empty")]
    pub sfx2: Music,
    /// A voice-over: a track like the music's.
    #[serde(default, skip_serializing_if = "Music::is_empty")]
    pub voice: Music,
    /// Fades from black at the start and to black at the end.
    #[serde(default, skip_serializing_if = "Fades::is_none")]
    pub fades: Fades,
    /// Lanes of pictures over the picture, the first lowest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<crate::overlay::Lane>,
    /// Lanes of text, over the overlays.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<crate::overlay::Lane>,
}

/// The edit's fades from black at its start and to black at its end, in
/// seconds, picture and sound together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fades {
    #[serde(default, rename = "in", skip_serializing_if = "is_zero")]
    pub fade_in: f64,
    #[serde(default, rename = "out", skip_serializing_if = "is_zero")]
    pub fade_out: f64,
}

impl Fades {
    pub fn is_none(&self) -> bool {
        *self == Fades::default()
    }

    /// How far faded to black at edit time `t` of an edit `length` long,
    /// 0..1.
    pub fn black(&self, t: f64, length: f64) -> f64 {
        let from = |d: f64, x: f64| if d > 0.0 { 1.0 - (x / d).clamp(0.0, 1.0) } else { 0.0 };
        from(self.fade_in, t).max(from(self.fade_out, length - t))
    }
}

/// The clips' own sound, as one track of the mix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sound {
    /// Level in dB; 0 is as recorded.
    #[serde(default)]
    pub volume: f64,
}

impl Sound {
    fn is_default(&self) -> bool {
        *self == Sound::default()
    }
}

/// One of the edit's sound tracks besides the clips' own sound. Each is a
/// `Music`: files placed on the edit's time, a level and volume points.
///
/// To add one (a voice-over track): a variant here and in `ALL`, a field on
/// `Project` (serde default, skipped when empty, so old projects load), and
/// its arms in the `match`es the compiler then points to (`track`,
/// `track_mut`, its name and colour, where a new clip goes). Everything else
/// goes over `ALL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SoundTrack {
    Music,
    Sfx,
    /// A second lane of sound effects, for two at once (gap 46e).
    Sfx2,
    Voice,
}

impl SoundTrack {
    /// Top to bottom, as the timeline shows them and the export mixes them.
    pub const ALL: [SoundTrack; 4] = [SoundTrack::Music, SoundTrack::Sfx, SoundTrack::Sfx2, SoundTrack::Voice];

    /// The lane's name on the timeline.
    pub fn name(self) -> &'static str {
        match self {
            SoundTrack::Music => "Music",
            SoundTrack::Sfx => "SFX",
            SoundTrack::Sfx2 => "SFX 2",
            SoundTrack::Voice => "Voice",
        }
    }

    /// Its files' folder under a project's folder, in footage-agent's
    /// layout (`footage_agent/layout.py`, the source of truth): where the
    /// add button opens and the bin looks.
    pub const fn folder(self) -> &'static str {
        match self {
            SoundTrack::Music => "02_audio/music",
            SoundTrack::Sfx | SoundTrack::Sfx2 => "07_sfx",
            SoundTrack::Voice => "02_audio/voiceover",
        }
    }

    /// The word for its files in a Clips export: "<name> - music 001.wav".
    pub fn file_word(self) -> &'static str {
        match self {
            SoundTrack::Music => "music",
            SoundTrack::Sfx => "sfx",
            SoundTrack::Sfx2 => "sfx2",
            SoundTrack::Voice => "voice",
        }
    }
}

/// A sound track: clips of songs (or sound effects) placed on the edit's
/// timeline. The music is one, the sound effects another (`SoundTrack`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Music {
    /// The track's level in dB; 0 is as the files are.
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub clips: Vec<MusicClip>,
    /// The track's own volume line of older projects (edit times), read
    /// and folded into its clips' lines on load (`fold_track_line`), never
    /// written: it lay over the clips' own lines and the two got in each
    /// other's way (user, 2026-11-01, gap 33i).
    #[serde(default, rename = "keyframes", skip_serializing)]
    old_line: Vec<VolumeKey>,
}

/// A straight fade's level `into` seconds after a start and `left` before
/// an end, with fades of `fade_in` and `fade_out` seconds there, 0..1.
pub fn fade_ramp(into: f64, left: f64, fade_in: f64, fade_out: f64) -> f64 {
    ramp(into, fade_in) * ramp(left, fade_out)
}

/// One straight fade `secs` long, `x` seconds into it, 0..1 (1 with none).
/// Fades that meet multiply, as the export's chained `afade`s do.
pub fn ramp(x: f64, secs: f64) -> f64 {
    if secs > 0.0 { (x / secs).clamp(0.0, 1.0) } else { 1.0 }
}

/// A volume point: `volume` dB at time `t`, straight lines (in dB) between
/// points, held before the first and after the last.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeKey {
    pub t: f64,
    pub volume: f64,
}

/// The level `keys` give at `t`, dB; 0 without any.
pub fn volume_at(keys: &[VolumeKey], t: f64) -> f64 {
    let i = keys.partition_point(|k| k.t <= t);
    match (i.checked_sub(1).map(|j| keys[j]), keys.get(i).copied()) {
        (Some(a), Some(b)) => a.volume + (b.volume - a.volume) * (t - a.t) / (b.t - a.t).max(1e-9),
        (Some(k), None) | (None, Some(k)) => k.volume,
        (None, None) => 0.0,
    }
}

/// Two volume points closer than this are one: K at a point selects it.
pub const SAME_POINT: f64 = 0.01;

/// Adds a volume point at `t`, `db`, in time order, and returns its index.
/// A sound track's points and a cut's are both lists of these.
pub fn add_volume_key(keys: &mut Vec<VolumeKey>, t: f64, db: f64) -> usize {
    let i = keys.partition_point(|k| k.t <= t);
    keys.insert(i, VolumeKey { t, volume: db });
    i
}

/// The point at `t` (within `SAME_POINT`), or a new one there on the line,
/// so the level doesn't jump: what K does on a sound.
pub fn volume_key_at(keys: &mut Vec<VolumeKey>, t: f64) -> usize {
    match keys.iter().position(|k| (k.t - t).abs() < SAME_POINT) {
        Some(i) => i,
        None => add_volume_key(keys, t, volume_at(keys, t)),
    }
}

/// Point `i` dragged to `t`, `db`: kept between its neighbours (points
/// keep their order) and within `range` (a cut's own stretch, or the edit
/// from 0). Nothing if there's no point `i`.
pub fn move_volume_key(keys: &mut [VolumeKey], i: usize, t: f64, db: f64, range: (f64, f64)) {
    if i >= keys.len() {
        return;
    }
    let lo = i.checked_sub(1).map_or(range.0, |j| keys[j].t + SAME_POINT).max(range.0);
    let hi = keys.get(i + 1).map_or(range.1, |k| k.t - SAME_POINT).min(range.1);
    keys[i] = VolumeKey { t: t.clamp(lo, hi.max(lo)), volume: db };
}

impl Music {
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty() && self.volume == 0.0
    }

    /// An older project's track line moved into its clips' own lines, so
    /// they sound as before: each clip that the line changes gets points at
    /// its in and out and at every track point inside it, each its own
    /// level there plus the track line's.
    fn fold_track_line(&mut self) {
        let line = std::mem::take(&mut self.old_line);
        if line.is_empty() {
            return;
        }
        for c in &mut self.clips {
            let edit = |file_t: f64| c.at + file_t - c.start;
            let mut times: Vec<f64> = [c.start, c.end].into_iter().chain(line.iter().map(|k| c.start + k.t - c.at).filter(|&t| t > c.start && t < c.end)).chain(c.volume.iter().map(|k| k.t)).collect();
            times.sort_by(f64::total_cmp);
            times.dedup_by(|a, b| (*a - *b).abs() < SAME_POINT);
            if times.iter().all(|&t| volume_at(&line, edit(t)).abs() < 1e-9) {
                continue;
            }
            c.volume = times.into_iter().map(|t| VolumeKey { t, volume: volume_at(&c.volume, t) + volume_at(&line, edit(t)) }).collect();
        }
    }

    /// Where the last clip ends on the edit, in seconds.
    pub fn end(&self) -> f64 {
        self.clips.iter().map(MusicClip::ends).fold(0.0, f64::max)
    }

    /// The start nearest `want` where clip `i` fits without overlapping the
    /// others: a plain drag never makes an overlap (a crossfade is made with
    /// Shift, `fits`), so one pushed into another stops edge to edge
    /// against it, and hops past it once `want` is nearer the far side.
    pub fn place(&self, i: usize, want: f64) -> f64 {
        let len = self.clips[i].end - self.clips[i].start;
        let others: Vec<(f64, f64)> = self.clips.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, c)| (c.at, c.ends())).collect();
        nearest_free(&others, len, want).unwrap_or(self.end())
    }

    /// Whether clip `i` fits from `at` for `len` seconds. It may overlap
    /// another clip only partly, at one of that clip's ends (the overlap is
    /// their crossfade), and never where a third clip plays; with `overlap`
    /// false, by no more than it overlaps that clip already (a plain drag
    /// never makes a crossfade, nor grows one).
    pub fn fits(&self, i: usize, at: f64, len: f64, overlap: bool) -> bool {
        const E: f64 = 1e-6;
        let (s, e) = (at, at + len);
        let me = &self.clips[i];
        let others = || self.clips.iter().enumerate().filter(move |&(j, _)| j != i).map(|(_, c)| c);
        at >= -E
            && others().all(|c| {
                let o = overlap_of(s, e, c.at, c.ends());
                if o <= E {
                    return true;
                }
                let partly = (s > c.at + E && e > c.ends() + E) || (s < c.at - E && e < c.ends() - E);
                let (o0, o1) = (s.max(c.at), e.min(c.ends()));
                let alone = others().all(|d| std::ptr::eq(d, c) || overlap_of(o0, o1, d.at, d.ends()) <= E);
                partly && alone && (overlap || o <= overlap_of(me.at, me.ends(), c.at, c.ends()) + E)
            })
    }

    /// How far from `from` towards `to` a value can go while `ok` holds
    /// (it holds at `from`): an edge dragged into a neighbour stops at it.
    pub fn reach(from: f64, to: f64, ok: impl Fn(f64) -> bool) -> f64 {
        if ok(to) {
            return to;
        }
        let (mut good, mut bad) = (from, to);
        for _ in 0..40 {
            let mid = (good + bad) / 2.0;
            if ok(mid) {
                good = mid;
            } else {
                bad = mid;
            }
        }
        good
    }

    /// Clip `i`'s crossfades: how long it overlaps the clip before it, and
    /// the clip after it.
    pub fn crossfades(&self, i: usize) -> (f64, f64) {
        let me = &self.clips[i];
        let (mut fade_in, mut fade_out) = (0.0, 0.0);
        for (j, c) in self.clips.iter().enumerate() {
            let o = overlap_of(me.at, me.ends(), c.at, c.ends());
            if j == i || o <= 1e-6 {
                continue;
            }
            if c.at < me.at {
                fade_in = o;
            } else {
                fade_out = o;
            }
        }
        (fade_in, fade_out)
    }

    /// Clip `i` dragged to start at edit time `want`. A plain drag never
    /// overlaps: it stops edge to edge, or hops past (`place`). With
    /// `overlap` (Shift) it slides into a neighbour as far as it may, and
    /// so does a clip already crossfading.
    pub fn drag_to(&mut self, i: usize, want: f64, overlap: bool) {
        let Some(c) = self.clips.get(i) else { return };
        let len = c.end - c.start;
        let at = if overlap || self.crossfades(i) != (0.0, 0.0) {
            Music::reach(c.at, want.max(0.0), |at| self.fits(i, at, len, overlap))
        } else {
            self.place(i, want)
        };
        self.clips[i].at = at;
    }

    /// Clip `i`'s in edge (`in_edge`) or out edge dragged to time `t` in its
    /// file, `file_len` seconds long. The file stays put on the edit, so
    /// trimming the in edge moves where the clip begins, not what plays
    /// when. It stays within the file and the edit, at least `min` long, and
    /// goes into a neighbour only with `overlap`.
    pub fn trim(&mut self, i: usize, in_edge: bool, t: f64, overlap: bool, file_len: f64, min: f64) {
        let Some(c) = self.clips.get(i) else { return };
        let to_edit = c.at - c.start;
        let (start, end, at) = (c.start, c.end, c.at);
        if in_edge {
            let want = t.max(start - at).max(0.0).min(end - min);
            let start = Music::reach(start, want, |s| self.fits(i, s + to_edit, end - s, overlap));
            let c = &mut self.clips[i];
            c.at = start + to_edit;
            c.start = start;
        } else {
            let want = t.min(file_len).max(start + min);
            self.clips[i].end = Music::reach(end, want, |e| self.fits(i, at, e - start, overlap));
        }
    }

    /// Adds the file's `start`..`end` as a clip as near `at` as fits: a
    /// recorded take, its lead-in before the playback started left out.
    pub fn append_part(&mut self, input: PathBuf, at: f64, start: f64, end: f64) -> usize {
        self.clips.push(MusicClip { input, at, start, end, ..Default::default() });
        let i = self.clips.len() - 1;
        self.clips[i].at = self.place(i, at);
        i
    }

    /// Splits clip `i` at edit time `t`, each half staying where it was on the
    /// edit; false if `t` leaves either half shorter than `min`.
    pub fn split(&mut self, i: usize, t: f64, min: f64) -> bool {
        let Some(c) = self.clips.get(i) else { return false };
        let song = c.start + t - c.at;
        if song - c.start < min || c.end - song < min {
            return false;
        }
        let second = MusicClip { at: t, start: song, fade_in: 0.0, ..c.clone() };
        let first = &mut self.clips[i];
        first.end = song;
        first.fade_out = 0.0;
        self.clips.insert(i + 1, second);
        true
    }
}

/// A stretch of a song at a place on the edit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicClip {
    /// Names it for operations and their undo (`OPS-API.md`): unique in
    /// the project, kept through moves and edits; 0 until given one
    /// (`Project::settle_ids`).
    #[serde(default, skip_serializing_if = "no_id")]
    pub id: Id,
    /// The file; a relative path is relative to the project file.
    pub input: PathBuf,
    /// Where it starts on the edit, in seconds.
    pub at: f64,
    /// In and out points in the song, in seconds.
    #[serde(rename = "in")]
    pub start: f64,
    #[serde(rename = "out")]
    pub end: f64,
    /// Fades at its own start and end, in seconds.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade_in: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fade_out: f64,
    /// Its own level, dB, as a cut's (`Cut::gain`): set by normalizing or
    /// from its menu. 0 is as the file is.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gain: f64,
    /// Evened out (`loudness::LEVELER`) before its level, as a cut's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub even: bool,
    /// Off; `gain` kept for when it's back on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    /// Its low rumble cut and steady noise cleaned, as a cut's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub low_cut: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clean: bool,
    /// What normalizing measured of its stretch, LUFS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness: Option<f64>,
    /// Its own volume points, in the file's time, so they move and trim
    /// with it; on top of its level, under the track's line (gap 33e).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume: Vec<VolumeKey>,
    /// The auto-duck folded into `volume`, so pressing it again starts from
    /// the line without it instead of lowering twice (user, 2026-11-02).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ducked: Option<Ducked>,
}

/// An auto-duck as applied to a clip's own line: how far down, and where,
/// in the file's time (`duck::duck_music`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ducked {
    pub depth: f64,
    pub speech: Vec<(f64, f64)>,
}

/// How long the spans `a0..a1` and `b0..b1` share.
fn overlap_of(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

impl MusicClip {
    /// Where it ends on the edit.
    pub fn ends(&self) -> f64 {
        self.at + self.end - self.start
    }

    /// Its own level, dB: `OFF_DB` when muted.
    pub fn sound_db(&self) -> f64 {
        if self.mute { OFF_DB } else { self.gain }
    }

    /// The filters its sound goes through (`loudness::sound_filter`).
    pub fn sound_filter(&self) -> Option<String> {
        crate::loudness::sound_filter(self.even, self.low_cut, self.clean)
    }

    /// Its own level at edit time `t`, dB: its level and its volume
    /// points (in the file's time), off when muted (`mixdown::own_level`).
    pub fn own_db_at(&self, t: f64) -> f64 {
        crate::mixdown::own_level(self.sound_db(), volume_at(&self.volume, self.start + t - self.at))
    }
}

/// A marker on a cut: a time in its clip and a note (empty: just a mark).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub t: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// What names a cut, a sound clip or an overlay item (`OPS-API.md`).
pub type Id = u32;

pub(crate) fn no_id(id: &Id) -> bool {
    *id == 0
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// The folder a project file's relative paths are relative to, absolute.
/// Opened by a relative path (`edit.json`), `parent()` is "", and joined
/// to it the clips stayed relative to the working folder: a copy saved
/// elsewhere then named them as if they were beside it, and couldn't find
/// them (found 2026-11-01).
fn project_dir(path: &Path) -> Result<PathBuf> {
    let file = std::path::absolute(path).with_context(|| format!("finding {}", path.display()))?;
    Ok(file.parent().map(Path::to_owned).unwrap_or_default())
}

/// Music running on past the last cut fades out over this long before it.
pub const END_FADE: f64 = 3.0;

/// A level at or below this is off: the sliders' bottom is silence.
pub const OFF_DB: f64 = -40.0;

/// A mix level in dB as a gain factor.
pub fn gain(db: f64) -> f64 {
    if db <= OFF_DB { 0.0 } else { 10f64.powf(db / 20.0) }
}

/// A look on a cut's start or end alone, with no overlap (gap 25b): how
/// long, and its settings the user moved (`Look::knobs`). Each half of a
/// join belongs to its own cut and goes where the cut goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub look: crate::transition::Look,
    pub seconds: f64,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub params: std::collections::BTreeMap<String, f32>,
}

impl Edge {
    pub fn spec(&self) -> crate::transition::Spec {
        crate::transition::Spec { look: Some(self.look), values: crate::transition::values(Some(self.look), &self.params) }
    }

    /// A fade to (or from) black: the sound fades with it (user, 2026-11-08).
    pub fn blacks(&self) -> bool {
        self.look == crate::transition::Look::EdgeFade && self.spec().values[0] < 0.5
    }
}

/// A style look over a cut's whole picture (gap 87: old film, VHS, glow…)
/// and its settings the user moved (`Look::knobs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub look: crate::transition::Look,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub params: std::collections::BTreeMap<String, f32>,
}

impl Style {
    pub fn spec(&self) -> crate::transition::Spec {
        crate::transition::Spec { look: Some(self.look), values: crate::transition::values(Some(self.look), &self.params) }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cut {
    /// Names it for operations and their undo (`OPS-API.md`): unique in
    /// the project, kept through moves and edits; 0 until given one
    /// (`Project::settle_ids`).
    #[serde(default, skip_serializing_if = "no_id")]
    pub id: Id,
    /// The clip; a relative path is relative to the project file.
    pub input: PathBuf,
    /// In and out points, clip time in seconds.
    #[serde(rename = "in")]
    pub start: f64,
    #[serde(rename = "out")]
    pub end: f64,
    #[serde(default)]
    pub keyframes: Keyframes,
    /// Its stabilisation, or the export's `--stabilize` (horizon by
    /// default) if it doesn't set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stabilize: Option<Mode>,
    /// What the cut is for, shown on the timeline: written by whoever
    /// assembled the edit, such as an automatic highlight pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// A sound file heard in place of the clip's own, timed like the clip
    /// (its second 0 is the clip's): the clip's voice cleaned of noise, say.
    /// The cut's volume points and the clips' level apply to it as they
    /// would to the clip's sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<PathBuf>,
    /// Its sound's volume points, clip time, in dB on top of the project's
    /// `sound` level. Pinned to the footage like the keyframes, so a shout
    /// keeps its level wherever the cut goes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume: Vec<VolumeKey>,
    /// Seconds it overlaps the cut before it, dissolving in from it
    /// (picture and sound); the edit is that much shorter.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub crossfade: f64,
    /// How that crossfade looks (`transition::Look`); none: a dissolve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<crate::transition::Look>,
    /// That look's own settings the user moved (`Look::knobs`, by key);
    /// the rest play at their defaults. Cleared when the look changes.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub transition_params: std::collections::BTreeMap<String, f32>,
    /// The look on its start, from its in point (`Edge`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_in: Option<Edge>,
    /// The look on its end, up to its out point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_out: Option<Edge>,
    /// A style look over its whole picture, after its grade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// Its effects moved over time (`automation::AutoClip`), clip time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automation: Vec<crate::automation::AutoClip>,
    /// Objects followed through its footage (`automation::ObjectTrack`),
    /// for overlays and text to follow (gap 37's contract).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<crate::automation::ObjectTrack>,
    /// Its colour effects' keyframes, clip time, apart from the camera's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<crate::grade::EffectKey>,
    /// Its markers, clip time in order, so they move with its footage as
    /// keyframes do (gap 5): what to find, a note, a beat later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
    /// A LUT applied before the effects: a name in the LUT library, or a
    /// `.cube` file (relative to the project). D-Log M needs DJI's to look
    /// like Rec.709; a missing one leaves the cut as filmed, with a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lut: Option<String>,
    /// The LUT's strength as projects saved it for a day (2026-10-15)
    /// before it became an effect: read, moved into the effect keyframes
    /// (`Project::load`), never written.
    #[serde(default, rename = "lut_strength", skip_serializing)]
    pub old_lut_strength: Option<f64>,
    /// Its own sound level, dB, on top of the edit's clip sound level and
    /// under its volume points: set by normalizing (to
    /// `model::level::TARGET_LUFS`) or from the menu ("lower for voice-over").
    /// 0 is as recorded.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gain: f64,
    /// Its sound evened out: quiet stretches lifted, loud ones left alone
    /// (`loudness::LEVELER`), before its level. Normalizing measures it so.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub even: bool,
    /// Its sound is off; `gain` is kept for when it's back on.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    /// Its low rumble cut (`loudness::LOW_CUT`) and its steady noise
    /// cleaned (`loudness::CLEAN`), before evening.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub low_cut: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clean: bool,
    /// Its sound fades in from its start and out to its end, seconds
    /// (the picture doesn't): straight, as the sound tracks' fades.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sound_fade_in: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sound_fade_out: f64,
    /// Its sound starts this many seconds before its picture, under the
    /// cut before (a J-cut), or after its picture starts when negative
    /// (silence there). The picture's edges still make the joins.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sound_before: f64,
    /// Its sound runs on this long after its picture, under the cut after
    /// (an L-cut), or stops short when negative.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sound_after: f64,
    /// What normalizing measured of what the cut plays, LUFS, shown in
    /// Info; it says nothing about the sound once trimmed again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness: Option<f64>,
    /// Its clip is an ordinary video (framed by cropping, not a 360 view):
    /// found when the clip is opened, not saved.
    #[serde(skip)]
    pub flat: bool,
    /// Its LUT, loaded (`resolve_lut`): found when the clip is opened.
    #[serde(skip)]
    pub lut_applied: Option<usize>,
}

impl Cut {
    /// A cut of `input` from `start` to `end` seconds, nothing else set.
    /// The one place a cut is spelled out field by field, so a new field
    /// means one edit here, not one at every cut made.
    pub fn new(input: PathBuf, start: f64, end: f64) -> Cut {
        Cut {
            id: 0,
            input,
            start,
            end,
            keyframes: Default::default(),
            stabilize: None,
            note: None,
            audio: None,
            volume: Vec::new(),
            crossfade: 0.0,
            transition: None,
            transition_params: Default::default(),
            edge_in: None,
            edge_out: None,
            style: None,
            automation: Vec::new(),
            tracks: Vec::new(),
            effects: Vec::new(),
            marks: Vec::new(),
            lut: None,
            old_lut_strength: None,
            gain: 0.0,
            even: false,
            mute: false,
            low_cut: false,
            clean: false,
            sound_fade_in: 0.0,
            sound_fade_out: 0.0,
            sound_before: 0.0,
            sound_after: 0.0,
            loudness: None,
            flat: false,
            lut_applied: None,
        }
    }

    /// Its sound's own level, dB: `gain`, or silence when muted.
    pub fn sound_db(&self) -> f64 {
        if self.mute { OFF_DB } else { self.gain }
    }

    /// The filters its sound goes through (`loudness::sound_filter`).
    pub fn sound_filter(&self) -> Option<String> {
        crate::loudness::sound_filter(self.even, self.low_cut, self.clean)
    }

    /// The file whose sound the cut plays (its cleaned voice if it has one
    /// and it exists, else its clip), and for measuring or drawing it the
    /// clip's `.LRF` stands in for the clip: the same sound, a sixteenth of
    /// the file.
    pub fn sound_file(&self) -> PathBuf {
        self.audio.clone().filter(|a| a.exists()).or_else(|| crate::ffmpeg::proxy(&self.input)).unwrap_or_else(|| self.input.clone())
    }

    /// Loads the LUT it names (a library name, or a file) for `camera_at`.
    /// Says why when it can't: the cut then shows as filmed.
    pub fn resolve_lut(&mut self) -> Result<()> {
        self.lut_applied = None;
        let Some(name) = &self.lut else { return Ok(()) };
        let path = crate::lut::find(name).with_context(|| format!("the LUT \"{name}\" isn't in the LUT library (or isn't there as a file)"))?;
        self.lut_applied = Some(crate::lut::shared(&path)?);
        Ok(())
    }

    /// How loud its sound is at clip time `t` under its picture, from its
    /// fades, 0..1: silent where its sound stops short of the picture. A
    /// fade at an edge past the picture belongs to the sound beyond it
    /// (`sound_beyond`).
    pub fn sound_fade(&self, t: f64) -> f64 {
        let (from, to) = self.sound_span();
        if t < from || t > to {
            return 0.0;
        }
        let (fade_in, fade_out) = self.sound_fades();
        fade_ramp(t - from, to - t, fade_in, fade_out)
    }

    /// Its sound's fades at its picture's ends, seconds: its own, as long
    /// as an edge fading to black where that's longer; none at an end
    /// whose sound runs past the picture (that fade is the sound beyond's).
    pub fn sound_fades(&self) -> (f64, f64) {
        let black = |e: &Option<Edge>| e.as_ref().filter(|e| e.blacks()).map_or(0.0, |e| e.seconds);
        let fade_in = if self.sound_before > 0.0 { 0.0 } else { self.sound_fade_in.max(black(&self.edge_in)) };
        let fade_out = if self.sound_after > 0.0 { 0.0 } else { self.sound_fade_out.max(black(&self.edge_out)) };
        (fade_in, fade_out)
    }

    /// The edge look at clip time `t` and how much of it (0..1,
    /// `transition::colour`'s `p`): growing to the out point, ebbing from
    /// the in point; where the two meet on a short cut, the stronger.
    pub fn edge_at(&self, t: f64) -> Option<(crate::transition::Spec, f32)> {
        let out = self.edge_out.as_ref().map(|e| (e, (t - (self.end - e.seconds)) / e.seconds));
        let start = self.edge_in.as_ref().map(|e| (e, 1.0 - (t - self.start) / e.seconds));
        [start, out].into_iter().flatten().map(|(e, q)| (e, q.clamp(0.0, 1.0))).filter(|&(_, q)| q > 0.0).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(e, q)| (e.spec(), q as f32))
    }

    /// Where its sound starts and ends, clip time.
    pub fn sound_span(&self) -> (f64, f64) {
        (self.start - self.sound_before, self.end + self.sound_after)
    }

    /// Its sound beyond its picture, for a cut whose picture starts at edit
    /// time `at`: the stretch before it (a J-cut) and after it (an L-cut),
    /// as sound clips on the edit with the cut's own level, points and
    /// filters, and its fade at the sound's outer edge. The player and the
    /// export play them as they play sound clips, so the two sound alike.
    pub fn sound_beyond(&self, at: f64) -> Vec<MusicClip> {
        let like = |at: f64, start: f64, end: f64| MusicClip {
            input: self.sound_file(),
            at,
            start,
            end,
            gain: self.gain,
            even: self.even,
            mute: self.mute,
            low_cut: self.low_cut,
            clean: self.clean,
            volume: self.volume.clone(),
            ..Default::default()
        };
        let mut beyond = Vec::new();
        if self.sound_before > 0.0 {
            beyond.push(MusicClip { fade_in: self.sound_fade_in.min(self.sound_before), ..like(at - self.sound_before, self.start - self.sound_before, self.start) });
        }
        if self.sound_after > 0.0 {
            beyond.push(MusicClip { fade_out: self.sound_fade_out.min(self.sound_after), ..like(at + self.len(), self.end, self.end + self.sound_after) });
        }
        beyond
    }

    /// Keeps its sound within its clip (`0..most`) and at least `MIN_CUT`
    /// long, after its picture's edges moved.
    pub fn settle_sound(&mut self, most: f64) {
        self.sound_before = self.sound_before.min(self.start);
        self.sound_after = self.sound_after.min((most - self.end).max(0.0));
        let (from, to) = self.sound_span();
        if to - from < crate::model::edit::MIN_CUT {
            self.sound_before = self.sound_before.max(0.0);
            self.sound_after = self.sound_after.max(0.0);
        }
    }

    pub fn len(&self) -> f64 {
        self.end - self.start
    }

    /// Its colour effects at clip time `t`: the values set, its automation
    /// over them (the one answer the player and the export read).
    pub fn grade_at(&self, t: f64) -> crate::grade::Grade {
        crate::automation::graded(crate::grade::grade_at(&self.effects, t), &self.automation, t)
    }

    /// Its join's transition, as the player and the export draw it.
    pub fn transition_spec(&self) -> crate::transition::Spec {
        crate::transition::Spec { look: self.transition, values: crate::transition::values(self.transition, &self.transition_params) }
    }
}

/// The shortest stretch of a cut its crossfades leave to it alone.
const SOLO: f64 = 0.1;

/// Where each of `cuts` starts on the edit: after the one before, less the
/// crossfade into it.
pub fn offsets(cuts: &[Cut]) -> Vec<f64> {
    let mut from = 0.0;
    cuts.iter()
        .map(|c| {
            from -= c.crossfade;
            let at = from;
            from += c.len();
            at
        })
        .collect()
}

/// The edit's length: the cuts' less their crossfades.
pub fn edit_length(cuts: &[Cut]) -> f64 {
    cuts.iter().map(|c| c.len() - c.crossfade).sum()
}

/// The longest crossfade into cut `i`: none into the first, and leaving
/// both cuts a moment of their own with the crossfade before.
pub fn max_crossfade(cuts: &[Cut], i: usize) -> f64 {
    if i == 0 || i >= cuts.len() {
        return 0.0;
    }
    let before = &cuts[i - 1];
    (before.len() - before.crossfade).min(cuts[i].len()) - SOLO
}

/// Keeps every crossfade within `max_crossfade` after cuts were trimmed,
/// moved or deleted.
pub fn settle_crossfades(cuts: &mut [Cut]) {
    for i in 0..cuts.len() {
        cuts[i].crossfade = cuts[i].crossfade.clamp(0.0, max_crossfade(cuts, i).max(0.0));
    }
}

/// The start nearest `want` where something `len` long fits among
/// `others` (each `(start, end)`) without overlapping one: `want` itself,
/// or edge to edge against one. None when nowhere fits. A sound clip's and
/// an overlay item's placing alike.
pub(crate) fn nearest_free(others: &[(f64, f64)], len: f64, want: f64) -> Option<f64> {
    let free = |at: f64| at >= 0.0 && others.iter().all(|&(a, b)| at + len <= a + 1e-6 || at >= b - 1e-6);
    std::iter::once(want.max(0.0))
        .chain(others.iter().flat_map(|&(a, b)| [b, a - len]))
        .filter(|&at| free(at))
        .min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs()))
}

/// What `Project::drop_missing` left out (cuts, sound clips, overlay
/// pictures whose files are gone), and the sound files gone whose cuts
/// play their clip's own sound instead (they stay named, to come back).
#[derive(Debug, Default, PartialEq)]
pub struct Missing {
    pub cuts: Vec<PathBuf>,
    pub sounds: Vec<(SoundTrack, PathBuf)>,
    pub overlays: Vec<PathBuf>,
    pub deaf: Vec<PathBuf>,
}

impl Missing {
    /// Nothing was left out (a missing sound file leaves nothing out).
    pub fn is_complete(&self) -> bool {
        self.cuts.is_empty() && self.sounds.is_empty() && self.overlays.is_empty()
    }

    /// What to tell the user, if anything.
    pub fn said(&self) -> Option<String> {
        let names = |v: &mut dyn Iterator<Item = &PathBuf>| v.map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ");
        let mut parts = Vec::new();
        if !self.cuts.is_empty() {
            parts.push(format!("Left out cuts of missing clips: {}", names(&mut self.cuts.iter())));
        }
        if !self.deaf.is_empty() {
            parts.push(format!("Missing sound files, playing the clips' own: {}", names(&mut self.deaf.iter())));
        }
        for which in SoundTrack::ALL {
            let mut of = self.sounds.iter().filter(|(w, _)| *w == which).map(|(_, p)| p).peekable();
            if of.peek().is_some() {
                parts.push(format!("Left out missing {}: {}", which.name(), names(&mut of)));
            }
        }
        if !self.overlays.is_empty() {
            parts.push(format!("Left out missing overlays: {}", names(&mut self.overlays.iter())));
        }
        (!parts.is_empty()).then(|| parts.join(". "))
    }
}

impl Project {
    /// Leaves out what can't play because its file is gone (as `exists`
    /// says): cuts, sound clips, overlay pictures (text stays). What went,
    /// and the cuts' own sound files gone, are said in the result.
    pub fn drop_missing(&mut self, exists: impl Fn(&Path) -> bool) -> Missing {
        let mut missing = Missing::default();
        self.cuts.retain(|c| exists(&c.input) || {
            missing.cuts.push(c.input.clone());
            false
        });
        missing.deaf = self.cuts.iter().filter_map(|c| c.audio.clone()).filter(|a| !exists(a)).collect();
        missing.deaf.sort();
        missing.deaf.dedup();
        for which in SoundTrack::ALL {
            self.track_mut(which).clips.retain(|c| exists(&c.input) || {
                missing.sounds.push((which, c.input.clone()));
                false
            });
        }
        for lane in &mut self.overlays {
            lane.items.retain(|it| it.file().is_none_or(&exists) || {
                missing.overlays.push(it.input.clone());
                false
            });
        }
        missing
    }

    /// The cuts' sound past their pictures (`Cut::sound_beyond`), a track
    /// each at the clips' level: one clip to a track, so the tail of one
    /// cut and the head of the next both play where they overlap rather
    /// than crossfade as a track's clips do.
    pub fn sound_beyond(&self) -> Vec<Music> {
        self.cuts
            .iter()
            .zip(offsets(&self.cuts))
            .flat_map(|(c, at)| c.sound_beyond(at))
            .filter(|c| c.at + c.end - c.start > 0.0)
            .map(|c| {
                // Before the edit's start (a J-cut on the first cut): only
                // what falls inside it.
                let early = (-c.at).max(0.0);
                Music { volume: self.sound.volume, clips: vec![MusicClip { at: c.at + early, start: c.start + early, fade_in: (c.fade_in - early).max(0.0), ..c }], ..Default::default() }
            })
            .collect()
    }

    /// How far faded to black cut `n` is at clip time `t`, 0..1.
    pub fn black_at(&self, n: usize, t: f64) -> f64 {
        let at = offsets(&self.cuts)[n] + t - self.cuts[n].start;
        self.fades.black(at, self.duration())
    }

    /// A project to work on: it must have cuts, or music to fill (the
    /// command line's tools have nothing to do with an empty one).
    pub fn load(path: &Path) -> Result<Project> {
        let project = Self::read(path)?;
        // Music alone is a start: its length is the frame to fill.
        if project.cuts.is_empty() && project.sound_clips().next().is_none() {
            bail!("{} has no cuts", path.display());
        }
        Ok(project)
    }

    /// A project file read, empty or not: the player opens an empty one (a
    /// day to build from its bin, Dailix's «empty day»).
    pub fn read(path: &Path) -> Result<Project> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut project: Project = serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
        let dir = &project_dir(path)?;
        for (n, cut) in project.cuts.iter_mut().enumerate() {
            if cut.end <= cut.start {
                bail!("cut {} ends ({} s) before it starts ({} s)", n + 1, cut.end, cut.start);
            }
            cut.input = dir.join(&cut.input);
            if let Some(audio) = cut.audio.as_mut() {
                *audio = dir.join(&*audio);
            }
            for track in cut.keyframes.tracks_mut() {
                *track = dir.join(&*track);
            }
            if let Some(lut) = cut.lut.as_mut().filter(|l| crate::lut::is_file(l)) {
                *lut = dir.join(&*lut).to_string_lossy().into_owned();
            }
            if let Some(strength) = cut.old_lut_strength.take().filter(|s| *s != 1.0) {
                if cut.effects.is_empty() {
                    cut.effects.push(crate::grade::EffectKey { t: cut.start, grade: Default::default(), ease: Default::default() });
                }
                for key in &mut cut.effects {
                    key.grade.lut = strength.clamp(0.0, 1.0);
                }
            }
            // Effect keys from before automation clips become clips (gap 39).
            cut.effects.sort_by(|a, b| a.t.total_cmp(&b.t));
            let clips = crate::automation::from_keys(&mut cut.effects, cut.start, cut.end);
            crate::automation::adopt(&mut cut.automation, clips);
        }
        for which in SoundTrack::ALL {
            for (n, clip) in project.track_mut(which).clips.iter_mut().enumerate() {
                if clip.end <= clip.start || clip.at < 0.0 {
                    bail!("{} clip {} is empty or starts before the edit", which.file_word(), n + 1);
                }
                clip.input = dir.join(&clip.input);
            }
        }
        for (n, item) in project.overlays.iter_mut().chain(&mut project.text).flat_map(|l| &mut l.items).enumerate() {
            if item.end <= item.start || item.at < 0.0 {
                bail!("overlay {} is empty or starts before the edit", n + 1);
            }
            // One of a picture, text or bars.
            let kinds = [item.file().is_some(), item.text.is_some(), item.bars.is_some()].into_iter().filter(|&k| k).count();
            if kinds != 1 {
                bail!("overlay {} needs one of a picture (\"input\"), \"text\" or \"bars\"", n + 1);
            }
            if item.motion.iter().any(|k| !k.t.is_finite() || k.place.values().iter().any(|v| !v.is_finite())) {
                bail!("an overlay's motion keyframe isn't a number");
            }
            if item.effects.iter().any(|k| !k.t.is_finite() || k.grade.values().iter().any(|v| !v.is_finite())) {
                bail!("an overlay's effect keyframe isn't a number");
            }
            if item.file().is_some() {
                item.input = dir.join(&item.input);
            }
            item.motion.sort_by(|a, b| a.t.total_cmp(&b.t));
            item.effects.sort_by(|a, b| a.t.total_cmp(&b.t));
            // And an item's effect and motion keys.
            let clips = [crate::automation::from_keys(&mut item.effects, item.start, item.end), crate::automation::from_keys(&mut item.motion, item.start, item.end)].concat();
            crate::automation::adopt(&mut item.automation, clips);
        }
        if project.cuts.iter().any(|c| !c.crossfade.is_finite()) {
            bail!("a crossfade isn't a number");
        }
        settle_crossfades(&mut project.cuts);
        let tracks = [&mut project.music, &mut project.sfx, &mut project.sfx2, &mut project.voice];
        let keys = project.cuts.iter_mut().map(|c| &mut c.volume).chain(tracks.into_iter().flat_map(|t| std::iter::once(&mut t.old_line).chain(t.clips.iter_mut().map(|c| &mut c.volume))));
        for keys in keys {
            if keys.iter().any(|k| !k.t.is_finite() || !k.volume.is_finite()) {
                bail!("a volume point isn't a number");
            }
            keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        }
        for which in SoundTrack::ALL {
            project.track_mut(which).fold_track_line();
        }
        for cut in &mut project.cuts {
            if cut.effects.iter().any(|k| !k.t.is_finite() || k.grade.values().iter().any(|v| !v.is_finite())) {
                bail!("an effect keyframe isn't a number");
            }
            cut.effects.sort_by(|a, b| a.t.total_cmp(&b.t));
            if cut.marks.iter().any(|m| !m.t.is_finite()) {
                bail!("a marker's time isn't a number");
            }
            cut.marks.sort_by(|a, b| a.t.total_cmp(&b.t));
        }
        project.settle_ids();
        Ok(project)
    }

    /// Writes the project as JSON. Clips in the project's folder or below
    /// are stored relative to it, so the folder can move as a whole.
    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = &project_dir(path)?;
        let mut project = self.clone();
        for cut in &mut project.cuts {
            if let Ok(rel) = cut.input.strip_prefix(dir) {
                cut.input = rel.to_owned();
            }
            if let Some(audio) = cut.audio.as_mut()
                && let Ok(rel) = audio.strip_prefix(dir)
            {
                *audio = rel.to_owned();
            }
            for track in cut.keyframes.tracks_mut() {
                if let Ok(rel) = track.strip_prefix(dir) {
                    *track = rel.to_owned();
                }
            }
            if let Some(lut) = cut.lut.as_mut().filter(|l| crate::lut::is_file(l))
                && let Ok(rel) = Path::new(lut.as_str()).strip_prefix(dir)
            {
                *lut = rel.to_string_lossy().into_owned();
            }
        }
        for which in SoundTrack::ALL {
            for clip in &mut project.track_mut(which).clips {
                if let Ok(rel) = clip.input.strip_prefix(dir) {
                    clip.input = rel.to_owned();
                }
            }
        }
        for item in project.overlays.iter_mut().chain(&mut project.text).flat_map(|l| &mut l.items) {
            if let Ok(rel) = item.input.strip_prefix(dir) {
                item.input = rel.to_owned();
            }
        }
        project.settle_ids();
        crate::replace_file(path, (serde_json::to_string_pretty(&project)? + "\n").as_bytes())
    }

    /// Gives an id to every cut, sound clip and overlay item without one,
    /// or sharing one with something before it (the second half of a
    /// split): one past the highest, so ids aren't handed out twice while
    /// the highest stays. Order: cuts, each sound track, overlays, text.
    pub fn settle_ids(&mut self) {
        let mut next = self.next_id();
        let mut ids: Vec<&mut Id> = Vec::new();
        ids.extend(self.cuts.iter_mut().map(|c| &mut c.id));
        let [music, sfx, sfx2, voice] = [&mut self.music, &mut self.sfx, &mut self.sfx2, &mut self.voice];
        for t in [music, sfx, sfx2, voice] {
            ids.extend(t.clips.iter_mut().map(|c| &mut c.id));
        }
        ids.extend(self.overlays.iter_mut().chain(&mut self.text).flat_map(|l| &mut l.items).map(|i| &mut i.id));
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            if *id == 0 || !seen.insert(*id) {
                *id = next;
                seen.insert(next);
                next += 1;
            }
        }
    }

    /// The id the next new thing gets: one past the highest.
    pub fn next_id(&self) -> Id {
        let tracks = SoundTrack::ALL.iter().flat_map(|&w| self.track(w).clips.iter().map(|c| c.id));
        let items = self.overlays.iter().chain(&self.text).flat_map(|l| &l.items).map(|i| i.id);
        self.cuts.iter().map(|c| c.id).chain(tracks).chain(items).max().unwrap_or(0) + 1
    }

    /// Length of the edit in seconds.
    pub fn duration(&self) -> f64 {
        edit_length(&self.cuts)
    }

    /// The project as a file should hold what the player shows: each sound
    /// track's clips in edit order (a lane keeps the order they were added
    /// and split in), and the views looked at by hand held as keyframes.
    /// A 360 cut with no keyframes of its own nor any before it shows the
    /// free look `look_360`; the file alone would frame it straight ahead,
    /// so the first such cut gets it at its in point (later 360 cuts carry
    /// it on). An ordinary video without keyframes shows its whole picture
    /// as the file frames it, except cut `framing` (one being framed by
    /// hand right now), which gets `look_flat`. The home view is never held:
    /// an untouched project gets no extra keyframes.
    pub fn as_shown(&self, look_360: View, look_flat: View, framing: Option<usize>) -> Project {
        let mut p = self.clone();
        for which in SoundTrack::ALL {
            p.track_mut(which).clips.sort_by(|a, b| a.at.total_cmp(&b.at));
        }
        let hold = |cut: &mut Cut, look: View| {
            if cut.keyframes.is_empty() && look != View::home(Projection::Perspective) {
                cut.keyframes.set(cut.start, look, 0.0, &|_, _| None);
            }
        };
        if let Some(cut) = p.cuts.iter_mut().find(|c| !c.flat) {
            hold(cut, look_360);
        }
        if let Some(cut) = framing.and_then(|i| p.cuts.get_mut(i)).filter(|c| c.flat) {
            hold(cut, look_flat);
        }
        p
    }

    /// Every clip on every sound track.
    pub fn sound_clips(&self) -> impl Iterator<Item = &MusicClip> {
        SoundTrack::ALL.into_iter().flat_map(|t| &self.track(t).clips)
    }

    /// Where the cut with id `id` is.
    pub fn cut_index(&self, id: Id) -> Option<usize> {
        self.cuts.iter().position(|c| c.id == id)
    }

    /// The places of the cuts with ids `ids` (every cut if none), in the
    /// edit's order: a command names cuts as ops and `describe` do (audit
    /// C8). An id that isn't there is refused.
    pub fn cuts_by_id(&self, ids: &[Id]) -> Result<Vec<usize>> {
        if let Some(gone) = ids.iter().find(|&&id| self.cut_index(id).is_none()) {
            bail!("no cut #{gone}");
        }
        Ok((0..self.cuts.len()).filter(|&i| ids.is_empty() || ids.contains(&self.cuts[i].id)).collect())
    }

    /// Which sound track the clip with id `id` is on, and where.
    pub fn clip_index(&self, id: Id) -> Option<(SoundTrack, usize)> {
        SoundTrack::ALL.into_iter().find_map(|w| self.track(w).clips.iter().position(|c| c.id == id).map(|i| (w, i)))
    }

    /// Which track (overlays or text), lane and place the item with id `id`
    /// is at.
    pub fn item_index(&self, id: Id) -> Option<(crate::overlay::Track, usize, usize)> {
        crate::overlay::Track::ALL.into_iter().find_map(|track| self.track_lanes(track).iter().enumerate().find_map(|(l, lane)| lane.items.iter().position(|it| it.id == id).map(|i| (track, l, i))))
    }

    pub fn track(&self, which: SoundTrack) -> &Music {
        match which {
            SoundTrack::Music => &self.music,
            SoundTrack::Sfx => &self.sfx,
            SoundTrack::Sfx2 => &self.sfx2,
            SoundTrack::Voice => &self.voice,
        }
    }

    pub fn track_mut(&mut self, which: SoundTrack) -> &mut Music {
        match which {
            SoundTrack::Music => &mut self.music,
            SoundTrack::Sfx => &mut self.sfx,
            SoundTrack::Sfx2 => &mut self.sfx2,
            SoundTrack::Voice => &mut self.voice,
        }
    }

    /// `own`'s volume points and the stretch of its file they apply in
    /// (a cut's in to out, a sound clip's), if it's there.
    pub fn own_line(&self, own: Own) -> Option<(&[VolumeKey], (f64, f64))> {
        match own {
            Own::Cut(i) => self.cuts.get(i).map(|c| (&c.volume[..], (c.start, c.end))),
            Own::Clip(which, i) => self.track(which).clips.get(i).map(|c| (&c.volume[..], (c.start, c.end))),
        }
    }

    /// `own`'s sound settings, to change.
    pub fn own_sound(&mut self, own: Own) -> Option<OwnSound<'_>> {
        match own {
            Own::Cut(i) => self.cuts.get_mut(i).map(|c| OwnSound { range: (c.start, c.end), gain: &mut c.gain, even: &mut c.even, mute: &mut c.mute, low_cut: &mut c.low_cut, clean: &mut c.clean, loudness: &mut c.loudness, volume: &mut c.volume, fade_in: &mut c.sound_fade_in, fade_out: &mut c.sound_fade_out }),
            Own::Clip(which, i) => self.track_mut(which).clips.get_mut(i).map(|c| OwnSound { range: (c.start, c.end), gain: &mut c.gain, even: &mut c.even, mute: &mut c.mute, low_cut: &mut c.low_cut, clean: &mut c.clean, loudness: &mut c.loudness, volume: &mut c.volume, fade_in: &mut c.fade_in, fade_out: &mut c.fade_out }),
        }
    }

    /// The file `own`'s sound comes from (a cut's cleaned voice or proxy,
    /// as `Cut::sound_file`), its stretch of it, and the filters it would go
    /// through evened out or not as `even` says (normalizing asks).
    pub fn own_file(&self, own: Own, even: bool) -> Option<(PathBuf, (f64, f64), Option<String>)> {
        match own {
            Own::Cut(i) => self.cuts.get(i).map(|c| (c.sound_file(), (c.start, c.end), crate::loudness::sound_filter(even, c.low_cut, c.clean))),
            Own::Clip(which, i) => self.track(which).clips.get(i).map(|c| (c.input.clone(), (c.start, c.end), crate::loudness::sound_filter(even, c.low_cut, c.clean))),
        }
    }

    /// Sound clip `i` of `from` moved to track `to`, everything of it kept,
    /// nearest its place where it fits there (`Music::place`): a sound
    /// effect onto the other SFX lane, to play over another. Its index on
    /// `to`, or `None` if there's no such clip.
    pub fn move_clip(&mut self, from: SoundTrack, i: usize, to: SoundTrack) -> Option<usize> {
        if i >= self.track(from).clips.len() || from == to {
            return None;
        }
        let clip = self.track_mut(from).clips.remove(i);
        let want = clip.at;
        let track = self.track_mut(to);
        track.clips.push(clip);
        let j = track.clips.len() - 1;
        track.clips[j].at = track.place(j, want);
        Some(j)
    }

    /// What plays before `own` on its lane: the cut before, or the sound
    /// clip on the same track ending nearest before it starts.
    pub fn before(&self, own: Own) -> Option<Own> {
        match own {
            Own::Cut(i) => i.checked_sub(1).filter(|&j| j < self.cuts.len()).map(Own::Cut),
            Own::Clip(which, i) => {
                let clips = &self.track(which).clips;
                let at = clips.get(i)?.at;
                (0..clips.len()).filter(|&j| j != i && clips[j].ends() <= at + 1e-6).max_by(|&a, &b| clips[a].ends().total_cmp(&clips[b].ends())).map(|j| Own::Clip(which, j))
            }
        }
    }

    /// Starts `own`'s volume line at the level what plays before it ends
    /// at (its own level and points; the track's line is the track's), so
    /// a second take or a split carries on without a jump. A line with no
    /// points becomes flat at that level; otherwise its first point is set
    /// at its start. False if there's nothing before it, or it's silent.
    pub fn continue_level(&mut self, own: Own) -> bool {
        let Some(end) = self.before(own).and_then(|prev| self.own_sound(prev)).map(|p| p.db_at(p.range.1)) else { return false };
        if end <= OFF_DB {
            return false;
        }
        let Some(s) = self.own_sound(own) else { return false };
        let (start, stop) = s.range;
        let v = end - *s.gain;
        if s.volume.is_empty() {
            add_volume_key(s.volume, start, v);
            add_volume_key(s.volume, stop, v);
        } else {
            let i = volume_key_at(s.volume, start);
            s.volume[i].volume = v;
        }
        true
    }

    /// Forgets a clip's auto-duck once its line is gone (deleted, back to
    /// the original): taking it out of a line without it would raise the
    /// music instead.
    pub fn forget_duck(&mut self, own: Own) {
        if let Own::Clip(which, i) = own
            && let Some(c) = self.track_mut(which).clips.get_mut(i)
        {
            c.ducked = None;
        }
    }

}

/// The sound settings a cut and a sound clip share, borrowed to change
/// them alike (the menu, normalizing: gap 33f).
pub struct OwnSound<'a> {
    pub gain: &'a mut f64,
    pub even: &'a mut bool,
    pub mute: &'a mut bool,
    pub low_cut: &'a mut bool,
    pub clean: &'a mut bool,
    pub loudness: &'a mut Option<f64>,
    pub volume: &'a mut Vec<VolumeKey>,
    pub fade_in: &'a mut f64,
    pub fade_out: &'a mut f64,
    /// Its stretch of its file.
    pub range: (f64, f64),
}

impl OwnSound<'_> {
    /// Its fade in (`in_edge`) or out set to `seconds`: from 0 to what
    /// its length leaves beside the other fade. A cut's sound and a sound
    /// clip's alike.
    pub fn set_fade(&mut self, in_edge: bool, seconds: f64) {
        let len = self.range.1 - self.range.0;
        let (this, other) = if in_edge { (&mut *self.fade_in, *self.fade_out) } else { (&mut *self.fade_out, *self.fade_in) };
        *this = seconds.clamp(0.0, (len - other).max(0.0));
    }

    /// Back to the file as it is: no level, evening, mute, points or fades.
    pub fn original(&mut self) {
        (*self.gain, *self.even, *self.mute, *self.loudness) = (0.0, false, false, None);
        (*self.low_cut, *self.clean) = (false, false);
        self.volume.clear();
        (*self.fade_in, *self.fade_out) = (0.0, 0.0);
    }

    /// Its level at file time `t`, dB: its own and its points', `OFF_DB`
    /// when muted.
    fn db_at(&self, t: f64) -> f64 {
        crate::mixdown::own_level(if *self.mute { OFF_DB } else { *self.gain }, volume_at(self.volume, t))
    }
}

/// Whose own volume line: a cut's sound or a sound clip's. Both are points
/// in the file's time, so one set of operations serves both (gap 33e).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Own {
    Cut(usize),
    Clip(SoundTrack, usize),
}

/// The keyframes that frame cut `n` of `cuts` (in edit order) at clip time
/// `t`, and the time to read them at. A 360 cut without keyframes holds the
/// view the edit had last: the end of the nearest earlier 360 cut with some,
/// as the player shows it when playing on. An ordinary video without them
/// shows its whole picture (a view's angles mean another thing there), so
/// neither kind holds the other's. `None` before any keyframe.
pub fn keyframed(cuts: &[Cut], n: usize, t: f64) -> Option<(&Keyframes, f64)> {
    if !cuts[n].keyframes.is_empty() {
        return Some((&cuts[n].keyframes, t));
    }
    if cuts[n].flat {
        return None;
    }
    cuts[..n].iter().rev().find(|c| !c.flat && !c.keyframes.is_empty()).map(|c| (&c.keyframes, c.end))
}

/// The view of cut `n` at clip time `t` (see `keyframed`), or straight ahead
/// before any keyframe. `aim` is where following keyframes point in this
/// cut's clip at `t`, so a cut that holds an earlier cut's following view
/// goes on following in its own footage.
pub fn camera_at(cuts: &[Cut], n: usize, t: f64, aim: Aim) -> Camera {
    let camera = keyframed(cuts, n, t).and_then(|(k, at)| k.camera_at(at, &held(aim, at == t, t))).unwrap_or_else(|| View::home(Projection::Perspective).camera());
    Camera { grade: cuts[n].grade_at(t), lut: cuts[n].lut_applied, ..camera }
}

/// `aim` for reading keyframes from `keyframed`: as asked in the cut's own
/// keyframes, else always at the cut's time `t`, as the keyframes held
/// from an earlier cut are read at that cut's end.
pub fn held(aim: Aim<'_>, own: bool, t: f64) -> impl Fn(&crate::view::Follow, f64) -> Option<(f64, f64)> + '_ {
    move |f, at| aim(f, if own { at } else { t })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_grows_to_the_out_point_and_ebbs_from_the_in_point_and_black_takes_the_sound() {
        use crate::transition::Look;
        let edge = |look, seconds| Some(Edge { look, seconds, params: Default::default() });
        let mut c = Cut::new("a.mp4".into(), 10.0, 14.0);
        (c.edge_in, c.edge_out) = (edge(Look::EdgeBlur, 1.0), edge(Look::EdgeFade, 2.0));
        let q = |t| c.edge_at(t).map(|(s, q)| (s.look.unwrap(), q));
        assert_eq!((q(10.0), q(10.5), q(11.5), q(13.0), q(14.0)), (Some((Look::EdgeBlur, 1.0)), Some((Look::EdgeBlur, 0.5)), None, Some((Look::EdgeFade, 0.5)), Some((Look::EdgeFade, 1.0))));
        c.sound_fade_out = 0.5;
        assert_eq!(c.sound_fades(), (0.0, 2.0), "the blur keeps the sound, the black takes it");
        c.edge_out.as_mut().unwrap().params.insert("colour".into(), 1.0);
        assert_eq!(c.sound_fades(), (0.0, 0.5), "a fade to white leaves it");
    }

    #[test]
    fn a_command_names_cuts_by_id() {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 1.0), Cut::new("b.mp4".into(), 0.0, 1.0), Cut::new("c.mp4".into(), 0.0, 1.0)], ..Default::default() };
        p.settle_ids();
        p.cuts.swap(0, 2);
        // Ids 3, 2, 1 in the edit's order: by id, not by place.
        assert_eq!(p.cuts_by_id(&[1]).unwrap(), [2]);
        assert_eq!(p.cuts_by_id(&[]).unwrap(), [0, 1, 2]);
        assert!(p.cuts_by_id(&[1, 9]).unwrap_err().to_string().contains("no cut #9"));
    }

    #[test]
    fn what_is_gone_is_left_out_and_said() {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut { audio: Some("voice.flac".into()), ..Cut::new("b.mp4".into(), 0.0, 3.0) }, Cut { audio: Some("voice.flac".into()), ..Cut::new("b.mp4".into(), 3.0, 5.0) }], ..Default::default() };
        p.music.append_part("song.wav".into(), 0.0, 0.0, 10.0);
        p.sfx.append_part("bang.wav".into(), 0.0, 0.0, 1.0);
        let item = |input: &str, text| crate::overlay::Item { id: 0, input: input.into(), text, at: 0.0, start: 0.0, end: 5.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None };
        p.overlays.push(crate::overlay::Lane::default());
        let bars = crate::overlay::Item { bars: Some(Default::default()), ..item("", None) };
        p.overlays[0].items = vec![item("logo.png", None), item("", Some(Default::default())), bars];
        let here = ["b.mp4", "song.wav"];
        let m = p.drop_missing(|f| here.iter().any(|h| f == Path::new(h)));
        assert_eq!((p.cuts.len(), p.music.clips.len(), p.sfx.clips.len(), p.overlays[0].items.len()), (2, 1, 0, 2), "the text and bars items stay");
        // Bars read back from a project file: neither a picture nor text.
        let path = std::env::temp_dir().join(format!("omnix-bars-{}.json", std::process::id()));
        std::fs::write(&path, serde_json::to_string(&Project { cuts: vec![Cut::new("x.mp4".into(), 0.0, 2.0)], overlays: p.overlays.clone(), ..Default::default() }).unwrap()).unwrap();
        let read = Project::read(&path);
        let _ = std::fs::remove_file(&path);
        assert!(read.unwrap().overlays[0].items.iter().any(|it| it.bars.is_some() && it.file().is_none()));
        assert_eq!(m.deaf, [PathBuf::from("voice.flac")], "named once");
        assert!(!m.is_complete());
        assert_eq!(m.said().unwrap(), "Left out cuts of missing clips: a.mp4. Missing sound files, playing the clips' own: voice.flac. Left out missing SFX: bang.wav. Left out missing overlays: logo.png");
        assert_eq!(Project::default().drop_missing(|_| false), Missing::default());
    }

    #[test]
    fn everything_gets_an_id_once_and_a_split_half_a_new_one() {
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut { id: 7, ..Cut::new("b.mp4".into(), 0.0, 3.0) }], ..Default::default() };
        p.music.append_part("song.wav".into(), 0.0, 0.0, 10.0);
        p.text.push(crate::overlay::Lane::default());
        p.text[0].items.push(crate::overlay::Item { id: 0, input: PathBuf::new(), text: Some(Default::default()), at: 0.0, start: 0.0, end: 5.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None });
        p.settle_ids();
        assert_eq!(p.cuts[1].id, 7, "an id given stays");
        assert_eq!((p.cuts[0].id, p.music.clips[0].id, p.text[0].items[0].id), (8, 9, 10), "past the highest, in order");
        // The second half of a split is a copy: it gets its own.
        let half = p.cuts[1].clone();
        p.cuts.push(half);
        p.settle_ids();
        assert_eq!((p.cuts[1].id, p.cuts[2].id), (7, 11));
        // Settled, nothing moves; and the ids survive the file.
        let before = p.clone();
        p.settle_ids();
        assert_eq!(p, before);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"id\":7"), "{json}");
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
    }

    #[test]
    fn loads_cuts_relative_to_the_file() {
        let dir = std::env::temp_dir().join(format!("omnix-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edit.json");
        std::fs::write(
            &path,
            r#"{"aspect": "9:16", "cuts": [
                {"input": "a.OSV", "in": 28, "out": 37, "keyframes": [{"t": 28, "yaw": -20}, {"t": 37, "projection": "little-planet"}, {"t": 37.5, "follow": "friend.csv"}]},
                {"input": "/clips/b.OSV", "in": 5, "out": 12, "stabilize": "lock", "note": "the pan"}
            ]}"#,
        )
        .unwrap();
        let p = Project::load(&path).unwrap();
        assert_eq!(p.cuts[0].input, dir.join("a.OSV"));
        assert_eq!(p.cuts[1].input, PathBuf::from("/clips/b.OSV"));
        assert_eq!(p.duration(), 16.0);
        assert_eq!(p.cuts[1].keyframes.len(), 0);
        assert_eq!((p.cuts[0].stabilize, p.cuts[1].stabilize), (None, Some(Mode::Lock)));
        assert_eq!((p.cuts[0].note.as_deref(), p.cuts[1].note.as_deref()), (None, Some("the pan")));
        assert_eq!(p.cuts[0].keyframes[1].view.projection, Projection::LittlePlanet);
        assert_eq!(p.cuts[0].keyframes[2].follow, Some(crate::view::Follow::Track(dir.join("friend.csv"))));
        let saved = dir.join("again.json");
        p.save(&saved).unwrap();
        let text = std::fs::read_to_string(&saved).unwrap();
        assert!(text.contains(r#""input": "a.OSV""#) && text.contains(r#""follow": "friend.csv""#));
        assert_eq!(Project::load(&saved).unwrap(), p);
        std::fs::write(&path, r#"{"cuts": [{"input": "a.OSV", "in": 5, "out": 2}]}"#).unwrap();
        assert!(Project::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A project saved while the LUT's strength was the cut's own opens,
    /// the strength moved into its effects; it isn't written back.
    #[test]
    fn a_cut_s_own_audio_file_loads_and_saves_like_its_clip() {
        let dir = std::env::temp_dir().join(format!("omnix-audio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edit.json");
        std::fs::write(
            &path,
            r#"{"cuts": [{"input": "a.MP4", "in": 0, "out": 4, "audio": "voice/a/dnn6.flac"},
                         {"input": "b.MP4", "in": 0, "out": 2, "audio": "/elsewhere/b.flac"},
                         {"input": "c.MP4", "in": 0, "out": 2}]}"#,
        )
        .unwrap();
        let p = Project::load(&path).unwrap();
        assert_eq!(p.cuts[0].audio, Some(dir.join("voice/a/dnn6.flac")));
        assert_eq!(p.cuts[1].audio, Some(PathBuf::from("/elsewhere/b.flac")));
        assert_eq!(p.cuts[2].audio, None);
        p.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""audio": "voice/a/dnn6.flac""#) && text.contains(r#""audio": "/elsewhere/b.flac""#));
        // A cut without one writes none: old projects read and write as before.
        assert_eq!(text.matches(r#""audio""#).count(), 2);
        assert_eq!(Project::load(&path).unwrap(), p);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_lut_strength_becomes_an_effect() {
        let dir = std::env::temp_dir().join(format!("omnix-lut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edit.json");
        std::fs::write(
            &path,
            r#"{"cuts": [{"input": "a.OSV", "in": 2, "out": 5, "lut": "x", "lut_strength": 0.84},
                         {"input": "a.OSV", "in": 6, "out": 9, "lut": "x", "lut_strength": 0.5, "effects": [{"t": 7, "exposure": 1}, {"t": 8}]}]}"#,
        )
        .unwrap();
        let p = Project::load(&path).unwrap();
        assert_eq!(p.cuts[0].effects.iter().map(|k| (k.t, k.grade.lut)).collect::<Vec<_>>(), [(2.0, 0.84)]);
        // Its two keys read as one and an exposure clip (gap 39), looking the same.
        assert_eq!(p.cuts[1].effects.iter().map(|k| (k.grade.exposure, k.grade.lut)).collect::<Vec<_>>(), [(1.0, 0.5)]);
        assert_eq!(p.cuts[1].automation.iter().map(|c| (c.param.as_str(), c.points.len())).collect::<Vec<_>>(), [("exposure", 2)]);
        assert_eq!((p.cuts[1].grade_at(7.0).exposure, p.cuts[1].grade_at(8.5).exposure, p.cuts[1].grade_at(8.5).lut), (1.0, 0.0, 0.5));
        p.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("lut_strength") && text.contains(r#""lut": 0.84"#));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn music_sits_on_the_edit_relative_to_the_file() {
        let dir = std::env::temp_dir().join(format!("omnix-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edit.json");
        // Music alone is a project: the song is the frame to fill.
        std::fs::write(&path, r#"{"music": {"volume": -3, "clips": [{"input": "song.mp3", "at": 2, "in": 60, "out": 200, "fade_in": 1.5}]}}"#).unwrap();
        let p = Project::load(&path).unwrap();
        assert_eq!((p.cuts.len(), p.sound.volume, p.music.volume), (0, 0.0, -3.0));
        assert_eq!(p.music.clips[0].input, dir.join("song.mp3"));
        assert_eq!(p.music.end(), 142.0);
        let keys = [VolumeKey { t: 10.0, volume: -6.0 }, VolumeKey { t: 20.0, volume: 0.0 }];
        assert_eq!([5.0, 10.0, 15.0, 25.0].map(|t| volume_at(&keys, t)), [-6.0, -6.0, -3.0, 0.0]);
        assert_eq!(volume_at(&[], 3.0), 0.0);
        p.save(&dir.join("again.json")).unwrap();
        let text = std::fs::read_to_string(dir.join("again.json")).unwrap();
        assert!(text.contains(r#""input": "song.mp3""#) && !text.contains("fade_out") && !text.contains("sound"));
        assert_eq!(Project::load(&dir.join("again.json")).unwrap(), p);
        // Without music or sound settings nothing is written for them.
        assert!(!serde_json::to_string(&Project::default()).unwrap().contains("music"));
        std::fs::write(&path, r#"{"music": {"clips": [{"input": "song.mp3", "at": 0, "in": 9, "out": 9}]}}"#).unwrap();
        assert!(Project::load(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_holds_what_was_looked_at_by_hand_and_no_more() {
        let flat = |name: &str| Cut { flat: true, ..Cut::new(name.into(), 0.0, 5.0) };
        let mut p = Project { cuts: vec![Cut::new("a.osv".into(), 0.0, 5.0), Cut::new("b.osv".into(), 0.0, 5.0), flat("c.mp4"), flat("d.mp4")], ..Default::default() };
        p.music.clips = ["late.wav", "early.wav"].iter().zip([9.0, 1.0]).map(|(n, at)| MusicClip { input: n.into(), at, start: 0.0, end: 2.0, ..Default::default() }).collect();
        let home = View::home(Projection::Perspective);
        let turned = View { yaw: 40.0, ..home };
        // Nothing looked at: nothing held; sound clips in edit order.
        let same = p.as_shown(home, home, None);
        assert!(same.cuts.iter().all(|c| c.keyframes.is_empty()));
        assert_eq!(same.music.clips[0].at, 1.0);
        // The 360 look on the first 360 cut only; the hand-framed video's
        // look on it alone, not on the other video.
        let held = p.as_shown(turned, turned, Some(3));
        let keyed: Vec<bool> = held.cuts.iter().map(|c| !c.keyframes.is_empty()).collect();
        assert_eq!(keyed, [true, false, false, true]);
    }

    #[test]
    fn sound_effects_are_a_track_of_their_own() {
        let dir = std::env::temp_dir().join(format!("omnix-sfx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edit.json");
        // Sound effects alone are a start, as music is; their files load
        // relative to the project, and an old track line's points (out of
        // order) go into the clip's own line.
        let json = r#"{"sfx": {"volume": -6, "clips": [{"input": "sfx/whoosh.wav", "at": 4, "in": 0, "out": 1.5}],
                       "keyframes": [{"t": 5, "volume": -3}, {"t": 4, "volume": 0}]}}"#;
        std::fs::write(&path, json).unwrap();
        let p = Project::load(&path).unwrap();
        assert!(p.music.is_empty());
        assert_eq!(p.sfx.clips[0].input, dir.join("sfx/whoosh.wav"));
        // The clip plays 0..1.5 s of its file at 4..5.5 s: the line's 4 s
        // (0 dB) and 5 s (−3 dB) are its 0 and 1 s.
        let line: Vec<_> = p.sfx.clips[0].volume.iter().map(|k| (k.t, k.volume)).collect();
        assert_eq!(line, [(0.0, 0.0), (1.0, -3.0), (1.5, -3.0)]);
        assert_eq!(p.track(SoundTrack::Sfx).end(), 5.5);
        p.save(&dir.join("again.json")).unwrap();
        let text = std::fs::read_to_string(dir.join("again.json")).unwrap();
        assert!(text.contains(r#""input": "sfx/whoosh.wav""#) && !text.contains("music"));
        assert!(!text.contains("keyframes"), "the track line isn't written back");
        assert_eq!(Project::load(&dir.join("again.json")).unwrap(), p);
        // An older project has no "sfx", and saving it writes none.
        assert!(!serde_json::to_string(&Project::default()).unwrap().contains("sfx"));
        std::fs::write(&path, r#"{"sfx": {"clips": [{"input": "a.wav", "at": -1, "in": 0, "out": 1}]}}"#).unwrap();
        assert!(Project::load(&path).unwrap_err().to_string().contains("sfx clip 1"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sound_clips_drag_and_trim_without_overlapping_unless_asked() {
        let clip = |at: f64, start: f64, end: f64| MusicClip { input: "s.wav".into(), at, start, end, ..Default::default() };
        let mut m = Music { clips: vec![clip(0.0, 0.0, 10.0), clip(12.0, 0.0, 10.0)], ..Default::default() };
        // Dragged into the second, nearer its start: stops edge to edge.
        m.drag_to(0, 4.0, false);
        assert_eq!(m.clips[0].at, 2.0);
        // With Shift it overlaps (a crossfade), partly.
        m.drag_to(0, 6.0, true);
        assert_eq!(m.clips[0].at, 6.0);
        m.drag_to(0, 0.0, false);
        // Its in edge to 3 s into the file: it starts 3 s later on the
        // edit, what plays when unchanged.
        m.trim(0, true, 3.0, false, 60.0, 0.2);
        assert_eq!((m.clips[0].at, m.clips[0].start), (3.0, 3.0));
        // The out edge stops at the neighbour, and at the file's end.
        m.trim(0, false, 30.0, false, 60.0, 0.2);
        assert!((m.clips[0].end - 12.0).abs() < 1e-6, "{}", m.clips[0].end);
        m.trim(1, false, 99.0, false, 15.0, 0.2);
        assert_eq!(m.clips[1].end, 15.0);
        // A clip gone (an undo mid-drag) is nothing.
        m.drag_to(5, 1.0, false);
        m.trim(5, true, 1.0, false, 9.0, 0.2);
    }

    #[test]
    fn fades_stay_within_the_clip() {
        let mut p = Project::default();
        p.music.append_part("a.wav".into(), 0.0, 0.0, 10.0);
        let song = Own::Clip(SoundTrack::Music, 0);
        p.own_sound(song).unwrap().set_fade(true, 3.0);
        p.own_sound(song).unwrap().set_fade(false, 9.0);
        assert_eq!((p.music.clips[0].fade_in, p.music.clips[0].fade_out), (3.0, 7.0));
        p.own_sound(song).unwrap().set_fade(true, -2.0);
        assert_eq!(p.music.clips[0].fade_in, 0.0);
        assert!(p.own_sound(Own::Clip(SoundTrack::Music, 4)).is_none());
    }

    #[test]
    fn volume_points_stay_in_order() {
        let mut keys = Vec::new();
        assert_eq!(add_volume_key(&mut keys, 4.0, -6.0), 0);
        assert_eq!(add_volume_key(&mut keys, 2.0, 0.0), 0);
        assert_eq!(add_volume_key(&mut keys, 8.0, -12.0), 2);
        // K at a point selects it; between two, a new one on the line.
        assert_eq!(volume_key_at(&mut keys, 4.005), 1);
        let i = volume_key_at(&mut keys, 6.0);
        assert_eq!((i, keys[i].volume, keys.len()), (2, -9.0, 4));
        // Dragged past a neighbour, it stops just before it; within range.
        move_volume_key(&mut keys, 1, 9.0, 3.0, (0.0, 20.0));
        assert_eq!((keys[1].t, keys[1].volume), (5.99, 3.0));
        move_volume_key(&mut keys, 0, -5.0, 0.0, (1.0, 20.0));
        assert_eq!(keys[0].t, 1.0);
        move_volume_key(&mut keys, 9, 1.0, 0.0, (0.0, 20.0));
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn a_sound_appended_lands_where_it_fits() {
        let mut m = Music::default();
        assert_eq!(m.append_part("a.wav".into(), 2.0, 0.0, 5.0), 0);
        assert_eq!((m.clips[0].at, m.clips[0].end), (2.0, 5.0));
        // Wanted inside the first, nearer its end: it goes after it.
        let i = m.append_part("b.wav".into(), 5.0, 0.0, 1.0);
        assert_eq!(m.clips[i].at, 7.0);
        // Nearer the first's start: it stops edge to edge before it.
        let j = m.append_part("c.wav".into(), 2.5, 0.0, 1.0);
        assert_eq!(m.clips[j].at, 1.0);
    }

    #[test]
    fn a_cuts_sound_fades_within_its_length() {
        let mut p = Project::default();
        p.cuts.push(Cut::new("a.mp4".into(), 10.0, 14.0));
        p.own_sound(Own::Cut(0)).unwrap().set_fade(true, 1.0);
        p.own_sound(Own::Cut(0)).unwrap().set_fade(false, 9.0);
        let c = &p.cuts[0];
        // The fade out takes what the fade in leaves of the 4 s.
        assert_eq!((c.sound_fade_in, c.sound_fade_out), (1.0, 3.0));
        assert_eq!(c.sound_fade(10.5), 0.5);
        assert_eq!(c.sound_fade(11.0), 1.0);
        assert_eq!(c.sound_fade(12.5), 0.5);
        assert_eq!(c.sound_fade(14.0), 0.0);
    }

    #[test]
    fn a_sound_clips_own_level_follows_its_file() {
        let mut c = MusicClip { input: "take.wav".into(), at: 10.0, start: 2.0, end: 8.0, gain: 3.0, ..Default::default() };
        c.volume = vec![VolumeKey { t: 4.0, volume: 0.0 }, VolumeKey { t: 6.0, volume: -6.0 }];
        // Edit 13 s is the file's 5 s, half way down the points' ramp.
        assert_eq!(c.own_db_at(13.0), 0.0);
        // Moved, its points go with it.
        c.at = 20.0;
        assert_eq!(c.own_db_at(23.0), 0.0);
        c.mute = true;
        assert_eq!(c.own_db_at(23.0), OFF_DB);
        // Old projects' clips load with none of it.
        let old: MusicClip = serde_json::from_str(r#"{"input": "s.wav", "at": 0, "in": 0, "out": 2}"#).unwrap();
        assert_eq!((old.gain, old.even, old.mute, old.volume.len()), (0.0, false, false, 0));
    }

    #[test]
    fn a_cut_and_a_sound_clip_share_one_kind_of_own_line() {
        let mut p = Project::default();
        p.cuts.push(Cut::new("a.mp4".into(), 2.0, 6.0));
        p.voice.clips.push(MusicClip { input: "take.wav".into(), at: 1.0, start: 0.5, end: 3.0, ..Default::default() });
        for (own, range) in [(Own::Cut(0), (2.0, 6.0)), (Own::Clip(SoundTrack::Voice, 0), (0.5, 3.0))] {
            let s = p.own_sound(own).unwrap();
            assert_eq!(s.range, range);
            add_volume_key(s.volume, range.0 + 1.0, -6.0);
            assert_eq!(p.own_line(own).unwrap().0.len(), 1);
        }
        assert!(p.own_line(Own::Clip(SoundTrack::Sfx, 0)).is_none());
    }

    #[test]
    fn a_second_take_carries_on_at_the_first_ones_level() {
        let mut p = Project::default();
        let take = |at: f64| MusicClip { input: "take.wav".into(), at, start: 0.0, end: 4.0, ..Default::default() };
        p.voice.clips = vec![take(10.0), take(0.0), take(20.0)];
        // Clip 0 (at 10) follows clip 1 (at 0, ending at 4), not clip 2.
        assert_eq!(p.before(Own::Clip(SoundTrack::Voice, 0)), Some(Own::Clip(SoundTrack::Voice, 1)));
        assert_eq!(p.before(Own::Clip(SoundTrack::Voice, 1)), None);
        // The first take normalized up 5 dB and pulled down 2 at its end.
        p.voice.clips[1].gain = 5.0;
        p.voice.clips[1].volume = vec![VolumeKey { t: 3.0, volume: 0.0 }, VolumeKey { t: 4.0, volume: -2.0 }];
        p.voice.clips[0].gain = 1.0;
        assert!(p.continue_level(Own::Clip(SoundTrack::Voice, 0)));
        // It starts at 3 dB in all, flat: 1 of its own and +2 of points.
        let c = &p.voice.clips[0];
        assert_eq!(c.volume, vec![VolumeKey { t: 0.0, volume: 2.0 }, VolumeKey { t: 4.0, volume: 2.0 }]);
        // And back to the file as it is.
        p.own_sound(Own::Clip(SoundTrack::Voice, 0)).unwrap().original();
        assert_eq!((p.voice.clips[0].gain, p.voice.clips[0].volume.len()), (0.0, 0));
        assert!(!p.continue_level(Own::Clip(SoundTrack::Voice, 1)));
    }

    #[test]
    fn a_project_saved_elsewhere_still_finds_its_clips() {
        // Opened by a relative path, its folder is still absolute.
        assert!(project_dir(Path::new("edit.json")).unwrap().is_absolute());
        let dir = std::env::temp_dir().join(format!("omnix-paths-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("edit.json"), r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 2}], "voice": {"clips": [{"input": "take.wav", "at": 0, "in": 0, "out": 1}]}}"#).unwrap();
        let p = Project::load(&dir.join("edit.json")).unwrap();
        assert_eq!(p.cuts[0].input, dir.join("a.mp4"));
        // Saved one folder down: the clips are named from there.
        p.save(&dir.join("sub/copy.json")).unwrap();
        let copy = Project::load(&dir.join("sub/copy.json")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(copy.cuts[0].input, dir.join("a.mp4"));
        assert_eq!(copy.voice.clips[0].input, dir.join("take.wav"));
    }

    fn song(at: f64, len: f64) -> MusicClip {
        MusicClip { input: "s.wav".into(), at, start: 0.0, end: len, ..Default::default() }
    }

    #[test]
    fn a_third_clip_never_lands_in_an_overlap() {
        // A 0..10 and B 8..18 crossfade over 8..10.
        let m = Music { clips: vec![song(0.0, 10.0), song(8.0, 10.0), song(30.0, 4.0)], ..Default::default() };
        // C dragged so it would cover the crossfade, even partly: no.
        assert!(!m.fits(2, 7.0, 4.0, true));
        assert!(!m.fits(2, 9.0, 2.0, true));
        // C wholly inside A: no.
        assert!(!m.fits(2, 2.0, 3.0, true));
        // A plain drag (no Shift) doesn't grow B's crossfade with A; Shift does.
        assert!(!m.fits(1, 7.0, 10.0, false));
        assert!(m.fits(1, 7.0, 10.0, true));
        assert!(m.fits(1, 8.5, 10.0, false), "shrinking it is fine");
        // Before the edit's start: never.
        assert!(!m.fits(2, -1.0, 4.0, true));
    }

    #[test]
    fn reach_stops_at_the_last_value_that_holds() {
        let ok = |x: f64| x <= 3.25;
        let r = Music::reach(0.0, 10.0, ok);
        assert!(ok(r) && (r - 3.25).abs() < 1e-9, "{r}");
        assert_eq!(Music::reach(0.0, 2.0, ok), 2.0);
        // Downwards too.
        let r = Music::reach(5.0, -5.0, |x| x >= 1.5);
        assert!(r >= 1.5 && r - 1.5 < 1e-9, "{r}");
    }

    #[test]
    fn a_sound_with_no_room_goes_after_the_last() {
        let m = Music { clips: vec![song(0.0, 5.0), song(5.0, 5.0), song(0.0, 20.0)], ..Default::default() };
        // The 20 s clip fits nowhere between the two: after the last.
        assert_eq!(m.place(2, 3.0), 10.0);
    }

    #[test]
    fn volume_points_at_their_edges() {
        let keys = [VolumeKey { t: 1.0, volume: -6.0 }, VolumeKey { t: 1.0, volume: 6.0 }, VolumeKey { t: 3.0, volume: 0.0 }];
        // Two points at one time: the later one wins from there on, no NaN.
        assert!(volume_at(&keys, 1.0).is_finite());
        assert_eq!(volume_at(&keys, 0.0), -6.0);
        assert_eq!(volume_at(&[], 5.0), 0.0);
        // A point near another is that point, not a new one.
        let mut keys = vec![VolumeKey { t: 2.0, volume: -3.0 }];
        assert_eq!(volume_key_at(&mut keys, 2.005), 0);
        assert_eq!(keys.len(), 1);
        // Moved within a range narrower than its neighbours leave: held in it.
        let mut keys = vec![VolumeKey { t: 1.0, volume: 0.0 }, VolumeKey { t: 2.0, volume: 0.0 }, VolumeKey { t: 3.0, volume: 0.0 }];
        move_volume_key(&mut keys, 1, 9.0, -4.0, (0.0, 10.0));
        assert!(keys[1].t < 3.0 && keys[1].t > 1.0 && keys[1].volume == -4.0);
        move_volume_key(&mut keys, 1, 2.5, 0.0, (2.2, 2.3));
        assert!((2.2..=2.3).contains(&keys[1].t));
        move_volume_key(&mut keys, 7, 1.0, 0.0, (0.0, 1.0));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn continue_level_counts_edit_order_not_list_order_and_skips_silence() {
        let mut p = Project::default();
        // Stored out of order: clip 0 plays last.
        p.sfx.clips = vec![song(20.0, 2.0), song(0.0, 2.0), song(10.0, 2.0)];
        assert_eq!(p.before(Own::Clip(SoundTrack::Sfx, 0)), Some(Own::Clip(SoundTrack::Sfx, 2)));
        assert_eq!(p.before(Own::Clip(SoundTrack::Sfx, 2)), Some(Own::Clip(SoundTrack::Sfx, 1)));
        // The one before muted: nothing to carry on from.
        p.sfx.clips[2].mute = true;
        assert!(!p.continue_level(Own::Clip(SoundTrack::Sfx, 0)));
        assert!(p.sfx.clips[0].volume.is_empty());
        // Cuts: the one before in the edit.
        p.cuts = vec![Cut::new("a.mp4".into(), 0.0, 2.0), Cut::new("b.mp4".into(), 0.0, 2.0)];
        p.cuts[0].gain = -4.0;
        assert!(p.continue_level(Own::Cut(1)));
        assert_eq!(volume_at(&p.cuts[1].volume, 1.0), -4.0);
        assert_eq!(p.before(Own::Cut(0)), None);
    }

    #[test]
    fn fades_at_their_bounds() {
        // No fades: always full; fades longer than the clip: they meet, never above 1.
        assert_eq!(fade_ramp(0.0, 0.0, 0.0, 0.0), 1.0);
        assert_eq!(fade_ramp(1.0, 1.0, 4.0, 4.0), 0.0625, "fades that meet multiply");
        assert_eq!(fade_ramp(-1.0, 5.0, 2.0, 0.0), 0.0);
        let mut p = Project::default();
        p.cuts.push(Cut::new("a.mp4".into(), 0.0, 2.0));
        let mut fade = |in_edge: bool, s: f64| p.own_sound(Own::Cut(0)).unwrap().set_fade(in_edge, s);
        fade(true, -3.0);
        fade(true, 5.0);
        fade(false, 1.0);
        assert_eq!((p.cuts[0].sound_fade_in, p.cuts[0].sound_fade_out), (2.0, 0.0), "nothing left after the fade in");
    }

    /// Old project files, one per shape the format has had, in
    /// `tests/fixtures/`: `deny_unknown_fields` makes a renamed field break
    /// them loudly, so a format change that would shows here first.
    #[test]
    fn old_project_files_still_load() {
        let dir = std::env::temp_dir().join(format!("omnix-fixtures-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let load = |name: &str, json: &str| {
            let path = dir.join(name);
            std::fs::write(&path, json).unwrap();
            Project::load(&path).unwrap_or_else(|e| panic!("{name}: {e:#}"))
        };
        // rOsmo360 0.7: 360 and flat cuts, music, effects, the old LUT
        // strength, overlays, text, fades.
        let p = load("rosmo360-0.7.json", include_str!("../tests/fixtures/rosmo360-0.7.json"));
        assert_eq!((p.cuts.len(), p.cuts[1].crossfade, p.sound.volume, p.music.clips[0].start), (2, 1.0, -3.0, 12.0));
        assert_eq!(p.cuts[0].effects[0].grade.lut, 0.6, "lut_strength moved into the effects");
        assert_eq!(p.cuts[1].audio, Some(dir.join("voice/phone.flac")));
        assert_eq!((p.overlays[0].items.len(), p.text[0].items.len(), p.fades.fade_out), (1, 1, 2.0));
        // The music's old track line (0 dB at 2 s, −12 dB at 5 s) is now the
        // song's own (it plays its 12..24 s from 0 s), sounding the same.
        let song = &p.music.clips[0];
        assert_eq!(song.volume.iter().map(|k| (k.t, k.volume)).collect::<Vec<_>>(), [(12.0, 0.0), (14.0, 0.0), (17.0, -12.0), (24.0, -12.0)]);
        assert_eq!(song.own_db_at(3.5), -6.0);
        // Omnix, October 2026: normalized cuts, SFX, a muted cut.
        let p = load("omnix-2026-10.json", include_str!("../tests/fixtures/omnix-2026-10.json"));
        assert_eq!((p.cuts[0].gain, p.cuts[0].even, p.cuts[1].mute), (2.6, true, true));
        assert_eq!((p.sfx.clips.len(), p.voice.clips.len()), (1, 0));
        assert_eq!(p.sfx.clips[0].input, dir.join("07_sfx/bell.wav"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_names_paths_from_the_project_and_reads_back_the_same() {
        let dir = std::env::temp_dir().join(format!("omnix-roundtrip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.json"), include_str!("../tests/fixtures/rosmo360-0.7.json")).unwrap();
        let p = Project::load(&dir.join("old.json")).unwrap();
        p.save(&dir.join("new.json")).unwrap();
        let text = std::fs::read_to_string(dir.join("new.json")).unwrap();
        let again = Project::load(&dir.join("new.json")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // Every kind of path inside the folder is written relative to it.
        for name in ["\"0037.OSV\"", "\"voice/phone.flac\"", "\"song.mp3\"", "\"logo.png\""] {
            assert!(text.contains(name), "{name} in {text}");
        }
        assert!(!text.contains(dir.to_str().unwrap()), "no absolute path inside the folder");
        assert_eq!(again, p);
    }

    #[test]
    fn a_sound_effect_moves_to_the_other_lane_whole() {
        let mut p = Project::default();
        p.sfx.clips.push(MusicClip { input: "bell.wav".into(), at: 3.0, start: 0.5, end: 2.0, gain: -4.0, fade_in: 0.2, ..Default::default() });
        p.sfx2.clips.push(MusicClip { input: "boom.wav".into(), at: 0.0, start: 0.0, end: 3.5, ..Default::default() });
        // Onto SFX 2, where a boom plays till 3.5 s: right after it.
        assert_eq!(p.move_clip(SoundTrack::Sfx, 0, SoundTrack::Sfx2), Some(1));
        let bell = &p.sfx2.clips[1];
        assert_eq!((bell.at, bell.start, bell.gain, bell.fade_in), (3.5, 0.5, -4.0, 0.2));
        assert!(p.sfx.clips.is_empty());
        assert_eq!(p.move_clip(SoundTrack::Sfx, 0, SoundTrack::Sfx2), None);
        // Saved and read back, the second lane is there.
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"sfx2\"") && !json.contains("\"sfx\":"), "{json}");
    }

    #[test]
    fn a_take_lands_without_its_lead_in() {
        let mut m = Music::default();
        m.append_part("song.wav".into(), 10.0, 0.0, 2.0);
        // 3.4 s recorded, the first 0.9 before the playback started.
        let i = m.append_part("take.wav".into(), 4.0, 0.9, 3.4);
        assert_eq!((m.clips[i].at, m.clips[i].start, m.clips[i].end), (4.0, 0.9, 3.4));
        // Its 2.5 s heard, not 3.4, decide where it fits: edge to edge
        // before the song.
        let j = m.append_part("take2.wav".into(), 7.6, 0.9, 3.4);
        assert_eq!(m.clips[j].at, 7.5);
    }

    #[test]
    fn music_clips_split_in_place_and_never_overlap() {
        let clip = |at: f64, start: f64, end: f64| MusicClip { input: "song.mp3".into(), at, start, end, fade_in: 1.0, fade_out: 2.0, ..Default::default() };
        let mut m = Music { clips: vec![clip(0.0, 0.0, 30.0)], ..Default::default() };
        assert!(m.split(0, 10.0, 0.1));
        assert_eq!((m.clips[0].end, m.clips[0].fade_out), (10.0, 0.0));
        assert_eq!((m.clips[1].at, m.clips[1].start, m.clips[1].end, m.clips[1].fade_in, m.clips[1].fade_out), (10.0, 10.0, 30.0, 0.0, 2.0));
        assert!(!m.split(0, 10.05, 0.1));
        // The second half pushed onto the first stops against it (there's
        // no room before it); the first dragged over the second stays before
        // it until it's nearer the far side.
        assert_eq!(m.place(1, 5.0), 10.0);
        assert_eq!(m.place(0, 12.0), 0.0);
        assert_eq!(m.place(0, 18.0), 30.0);
        m.clips[1].at = 40.0;
        assert_eq!(m.place(0, 35.0), 30.0);
        // A crossfade is a partial overlap of two, made only on purpose.
        assert!(!m.fits(0, 35.0, 10.0, false));
        assert!(m.fits(0, 35.0, 10.0, true));
        assert_eq!(m.crossfades(1), (0.0, 0.0));
        m.clips[0].at = 35.0;
        assert_eq!((m.crossfades(0), m.crossfades(1)), ((0.0, 5.0), (5.0, 0.0)));
        // Not inside the other, nor over a third.
        assert!(!m.fits(0, 45.0, 10.0, true));
        m.clips.push(clip(58.0, 0.0, 10.0));
        assert!(!m.fits(2, 50.0, 10.0, true));
        assert!(m.fits(2, 46.0, 30.0, true));
        assert!(!m.fits(2, 43.0, 30.0, true));
        // An edge dragged into a neighbour without Shift stops against it.
        let len = m.clips[2].end - m.clips[2].start;
        let at = Music::reach(58.0, 40.0, |at| m.fits(2, at, len, false));
        assert!((at - 58.0).abs() < 1e-6, "{at}");
        m.clips[2].at = 70.0;
        let at = Music::reach(70.0, 40.0, |at| m.fits(2, at, len, false));
        assert!((at - 60.0).abs() < 1e-6, "{at}");
    }

    #[test]
    fn cuts_without_keyframes_hold_the_last_view() {
        let p: Project = serde_json::from_str(
            r#"{"cuts": [
                {"input": "a.OSV", "in": 0, "out": 4},
                {"input": "a.OSV", "in": 10, "out": 20, "keyframes": [{"t": 10, "yaw": 0}, {"t": 14, "yaw": 40}, {"t": 30, "yaw": 200}]},
                {"input": "b.OSV", "in": 50, "out": 60},
                {"input": "b.OSV", "in": 70, "out": 80}
            ]}"#,
        )
        .unwrap();
        let yaw = |n, t| camera_at(&p.cuts, n, t, &|_, _| None).user;
        assert_eq!(yaw(0, 2.0), View::home(Projection::Perspective).camera().user);
        assert_eq!(keyframed(&p.cuts, 1, 12.0).map(|k| k.1), Some(12.0));
        // An ordinary video in between holds nothing and passes nothing on.
        let mut mixed = p.cuts.clone();
        mixed[2].flat = true;
        assert_eq!(keyframed(&mixed, 2, 55.0), None);
        assert_eq!(keyframed(&mixed, 3, 75.0).map(|k| k.1), Some(20.0));
        // The view at the earlier cut's out point, not at its last keyframe.
        let held = p.cuts[1].keyframes.camera_at(20.0, &|_, _| None).unwrap().user;
        assert_eq!((yaw(2, 55.0), yaw(3, 75.0)), (held, held));
    }
}
