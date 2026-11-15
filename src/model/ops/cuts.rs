//! Operations on the cuts: add, trim, move, split, crossfade, framing
//! settings, delete (`OPS-API.md` step 3). What the player read from its
//! clip library (a clip's length, its colour mode) comes in the op.

use super::{End, OpError, Outcome, said};
use crate::model::edit::MIN_CUT;
use crate::project::{Cut, Id, Project};
use crate::stab::Mode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where a new cut goes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CutPlace")]
pub enum Place {
    /// After the last cut.
    End,
    /// At edit time `t`: the cut playing there is split and the new one
    /// goes between the halves (within `MIN_CUT` of a join, at the join).
    At(f64),
    /// At the join of two cuts nearest edit time `t`.
    Join(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum CutOp {
    /// A cut of `input` from `start` to `end` (seconds in the clip), with
    /// its stabilisation and LUT (a library name or a `.cube` file).
    Add {
        input: PathBuf,
        start: f64,
        end: f64,
        place: Place,
        #[serde(default)]
        stabilize: Option<Mode>,
        #[serde(default)]
        lut: Option<String>,
    },
    /// Its in or out point to `t` (clip time); `most` is the clip's length.
    Trim { cut: Id, end: End, t: f64, most: f64 },
    /// Moved to play just before cut `before`, or last if none.
    Move { cut: Id, before: Option<Id> },
    /// Several cuts moved together (a group's, gap 8): side by side, in the
    /// order they were, just before cut `before` (not one of them), or last.
    MoveTogether { cuts: Vec<Id>, before: Option<Id> },
    /// The sound it plays: a cleaned voice's file (one Dailix made for its
    /// clip, timed like it), or none for the clip's own; with the voice's
    /// loudness over the cut, LUFS, measured by the caller, its level is
    /// set from it as Normalize would (gap 103).
    Voice { cut: Id, #[serde(default)] audio: Option<std::path::PathBuf>, #[serde(default)] lufs: Option<f64> },
    /// The voice Dailix's marking chose for its clip, as this cut has now
    /// seen it (taken in or let pass): it isn't offered again (gap 103).
    SheetVoice { cut: Id, #[serde(default)] name: Option<String> },
    /// An ordinary clip's frame (gap 107): which part of its picture the
    /// film shows, in % of its height (`frame::Frame`); none: the whole
    /// picture, or its view keys. Several cuts at once by a batch («for all
    /// cuts of this clip», «for the whole film»).
    Frame { cut: Id, #[serde(default)] frame: Option<crate::frame::Frame> },
    /// Its own sound is speech (or not): auto-duck and the mix check count
    /// it as a voice (gap 103).
    Speech { cut: Id, on: bool },
    /// Split at `t` (clip time); the second half is a new cut.
    Split { cut: Id, t: f64 },
    /// Cut `cut`'s pieces (what one clip was split into) joined back into
    /// one where the first is, from the earliest start to the latest end:
    /// its join and effects from the first, its out end's from the last.
    JoinPieces { cut: Id },
    /// How far it overlaps the cut before, seconds; kept within both.
    Crossfade { cut: Id, seconds: f64 },
    /// How its crossfade looks (`transition::Look`); none: a dissolve.
    /// The first cut has no join before it.
    Transition { cut: Id, look: Option<crate::transition::Look> },
    /// One of its look's own settings (`Look::knobs`, by key) to `value`;
    /// none: back to the default.
    TransitionKnob { cut: Id, key: String, value: Option<f32> },
    /// The look on its start (`In`) or end (`Out`) alone (`Cut::edge_in`,
    /// `edge_out`), `seconds` long (on the transitions' steps, within the
    /// cut); none: taken off. Its settings stay while the look does.
    Edge { cut: Id, end: End, look: Option<crate::transition::Look>, seconds: f64 },
    /// One of an edge look's own settings, as `TransitionKnob`.
    EdgeKnob { cut: Id, end: End, key: String, value: Option<f32> },
    /// The join before it as `template` has it (`template::Template`): the
    /// overlap, the out on the cut before, the in on this one; what the
    /// template leaves out is taken off.
    Template { cut: Id, template: crate::template::Template },
    /// A style look over its whole picture (`Cut::style`: old film, VHS,
    /// glow…); none: off. Its settings stay while the look does.
    Style { cut: Id, look: Option<crate::transition::Look> },
    /// One of its style's own settings, as `TransitionKnob`.
    StyleKnob { cut: Id, key: String, value: Option<f32> },
    /// An object followed through its footage (a tracker's, or written by
    /// hand), replacing one of that name (gap 37's contract).
    Track { cut: Id, track: crate::automation::ObjectTrack },
    /// Its track named `name` taken away (clips made from it stay).
    Untrack { cut: Id, name: String },
    /// How its 360 view is held steady (none: the player's choice).
    Stabilize { cut: Id, mode: Option<Mode> },
    /// Its LUT: a library name (`omnix looks` lists them) or a `.cube`
    /// file; none: as filmed.
    Lut { cut: Id, lut: Option<String> },
    /// The cut gone; the cuts after it close up.
    Delete { cut: Id },
    /// A marker at `t` (clip time) with `note`: the one there (within
    /// `SAME_MARK`) gets the note, else a new one.
    Mark { cut: Id, t: f64, #[serde(default)] note: String },
    /// The marker at `from` to `t`, within the cut.
    MoveMark { cut: Id, from: f64, t: f64 },
    /// The marker at `t` (clip time) gone.
    Unmark { cut: Id, t: f64 },
    /// Its sound's edge (`in`: where it starts, `out`: where it ends) to
    /// clip time `t`, the picture where it is: past the picture's edge the
    /// sound plays under the cut beside (a J- or L-cut), inside it the
    /// picture goes on in silence. Kept within the clip (`most` is its
    /// length) and at least `MIN_CUT` long.
    SoundEdge { cut: Id, end: End, t: f64, most: f64 },
    /// Its sound back to its picture's edges.
    SoundToPicture { cut: Id },
    /// How fast it reads its clip, one rate throughout (gap 31): 0.5 is
    /// half speed, 2 twice; 1 back to normal. Its clip span stays, its
    /// length on the edit changes and the cuts after it move.
    Speed { cut: Id, rate: f64 },
    /// Its speed as a curve (31c): each key's rate from its clip time on,
    /// in clip-time order; none: normal speed.
    SpeedCurve { cut: Id, keys: Vec<crate::speed::SpeedKey> },
    /// Its sound at speed like tape, slower is lower (on), or stretched
    /// keeping its pitch (off, as it starts).
    TapePitch { cut: Id, on: bool },
}

/// Two markers closer than this (seconds) are one.
pub const SAME_MARK: f64 = 1.0 / 60.0;

pub(super) fn apply(p: &mut Project, op: &CutOp) -> Result<Outcome, OpError> {
    match op {
        CutOp::Add { input, start, end, place, stabilize, lut } => add(p, input, start, end, place, stabilize, lut),
        &CutOp::Trim { cut, end, t, most } => {
            let i = find(p, cut)?;
            p.trim(i, end == End::In, t, most);
            let c = &p.cuts[i];
            said(format!("cut #{cut}: {:.2}-{:.2} s", c.start, c.end))
        }
        &CutOp::Move { cut, before } => move_cut(p, cut, before),
        CutOp::MoveTogether { cuts, before } => move_together(p, cuts, before),
        &CutOp::Split { cut, t } => split(p, cut, t),
        &CutOp::JoinPieces { cut } => join_pieces(p, cut),
        CutOp::Voice { cut, audio, lufs } => set_voice(p, *cut, audio.clone(), *lufs),
        CutOp::SheetVoice { cut, name } => {
            let i = find(p, *cut)?;
            p.cuts[i].sheet_voice = name.clone();
            said(format!("cut #{cut}: Dailix's voice {} seen", name.as_deref().unwrap_or("(none)")))
        }
        &CutOp::Frame { cut, frame } => set_frame(p, cut, frame),
        &CutOp::Speech { cut, on } => {
            let i = find(p, cut)?;
            p.cuts[i].speech = on;
            said(format!("cut #{cut}: its own sound {} speech", if on { "is" } else { "isn't" }))
        }
        &CutOp::Transition { cut, look } => transition(p, cut, look),
        CutOp::TransitionKnob { cut, key, value } => {
            let i = find(p, *cut)?;
            let look = p.cuts[i].transition.ok_or_else(|| OpError::Invalid(format!("cut #{cut}'s join has no look to set")))?;
            let said_ = set_knob(look, &mut p.cuts[i].transition_params, key, *value)?;
            said(format!("cut #{cut}: {said_}"))
        }
        &CutOp::Edge { cut, end, look, seconds } => edge(p, cut, end, look, seconds),
        CutOp::Template { cut, template } => set_template(p, cut, template),
        &CutOp::Style { cut, look } => {
            let i = find(p, cut)?;
            set_style(&mut p.cuts[i].style, look)?;
            said(format!("cut #{cut}: style {}", look.map_or("none", |l| l.name())))
        }
        CutOp::Track { cut, track } => add_track(p, cut, track),
        CutOp::Untrack { cut, name } => untrack(p, cut, name),
        CutOp::StyleKnob { cut, key, value } => {
            let i = find(p, *cut)?;
            let style = p.cuts[i].style.as_mut().ok_or_else(|| OpError::Invalid(format!("cut #{cut} has no style to set")))?;
            said(format!("cut #{cut}: {}", set_knob(style.look, &mut style.params, key, *value)?))
        }
        CutOp::EdgeKnob { cut, end, key, value } => edge_knob(p, *cut, *end, key, *value),
        &CutOp::Crossfade { cut, seconds } => {
            let i = find(p, cut)?;
            p.set_crossfade(i, seconds);
            said(format!("cut #{cut}: crossfade {:.2} s", p.cuts[i].crossfade))
        }
        &CutOp::Stabilize { cut, mode } => {
            let i = find(p, cut)?;
            p.cuts[i].stabilize = mode;
            said(format!("cut #{cut}: stabilisation {}", mode.map_or("as the export's".to_owned(), |m| format!("{m:?}").to_lowercase())))
        }
        CutOp::Lut { cut, lut } => {
            let i = find(p, *cut)?;
            p.cuts[i].lut = lut.clone();
            said(format!("cut #{cut}: LUT {}", lut.as_deref().unwrap_or("none")))
        }
        &CutOp::Delete { cut } => {
            let i = find(p, cut)?;
            p.remove(i);
            said(format!("cut #{cut} removed"))
        }
        CutOp::Mark { cut, t, note } => mark(p, cut, t, note),
        &CutOp::MoveMark { cut, from, t } => move_mark(p, cut, from, t),
        &CutOp::SoundEdge { cut, end, t, most } => sound_edge(p, cut, end, t, most),
        &CutOp::SoundToPicture { cut } => sound_to_picture(p, cut),
        &CutOp::Speed { cut, rate } => set_rate(p, cut, rate),
        CutOp::SpeedCurve { cut, keys } => set_speed(p, *cut, keys.clone()),
        &CutOp::TapePitch { cut, on } => tape_pitch(p, cut, on),
        &CutOp::Unmark { cut, t } => unmark(p, cut, t),
    }
}

/// Cut `cut` moved to play just before `before`, or last.
fn move_cut(p: &mut Project, cut: Id, before: Option<Id>) -> Result<Outcome, OpError> {
    let from = find(p, cut)?;
    let to = match before {
        Some(b) => find(p, b)?,
        None => p.cuts.len(),
    };
    p.move_cut(from, to);
    said(format!("cut #{cut} moved, {}", position(find(p, cut)?, p.cuts.len())))
}

/// Cut `cut`'s sound back to its picture's edges.
fn sound_to_picture(p: &mut Project, cut: Id) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    (c.sound_before, c.sound_after) = (0.0, 0.0);
    said(format!("cut #{cut}: sound back to its picture"))
}

/// Cut `cut`'s marker at `t` gone.
fn unmark(p: &mut Project, cut: Id, t: f64) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    let k = mark_at(&c.marks, t).ok_or_else(|| OpError::NotFound(format!("marker at {t:.2} s on cut #{cut}")))?;
    c.marks.remove(k);
    said(format!("cut #{cut}: marker at {t:.2} s removed"))
}

/// A cut added.
fn add(p: &mut Project, input: &std::path::Path, start: &f64, end: &f64, place: &Place, stabilize: &Option<Mode>, lut: &Option<String>) -> Result<Outcome, OpError> {
    if !(start.is_finite() && end - start >= MIN_CUT) {
        return Err(OpError::Invalid(format!("a cut from {start} to {end} s is shorter than {MIN_CUT} s")));
    }
    let id = p.next_id();
    let cut = Cut { id, stabilize: *stabilize, lut: lut.clone(), ..Cut::new(input.to_path_buf(), *start, *end) };
    let at = match *place {
        Place::End => p.insert_cut(usize::MAX, cut),
        Place::At(t) => p.insert_at(t, cut),
        Place::Join(t) => {
            let join = p.nearest_join(t);
            p.insert_cut(join, cut)
        }
    };
    // Put in among the cuts, it may have split one: its second half
    // gets an id of its own.
    p.settle_ids();
    Ok(Outcome { summary: format!("cut #{id} added, {}", position(at, p.cuts.len())), made: Some(id) })
}

/// Several cuts moved together.
fn move_together(p: &mut Project, cuts: &[Id], before: &Option<Id>) -> Result<Outcome, OpError> {
    for &c in cuts {
        find(p, c)?;
    }
    if let Some(b) = before {
        find(p, *b)?;
        if cuts.contains(b) {
            return Err(OpError::Invalid(format!("cut #{b} is one of those moved")));
        }
    }
    p.move_cuts(cuts, *before);
    said(format!("{} cuts moved together", cuts.len()))
}

/// Cut `cut` split at clip time `t`: its second half a new id.
fn split(p: &mut Project, cut: Id, t: f64) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    if !p.split(i, t) {
        let c = &p.cuts[i];
        return Err(OpError::OutOfRange { what: "split time", value: t, least: c.clip_at(MIN_CUT), most: c.clip_before_end(MIN_CUT) });
    }
    let id = p.next_id();
    p.cuts[i + 1].id = id;
    Ok(Outcome { summary: format!("cut #{cut} split at {t:.2} s; the second half is cut #{id}"), made: Some(id) })
}

/// `Voice`: the cut's sound, a voice's file or its own, and its level
/// from the voice's loudness as Normalize would set it.
fn set_voice(p: &mut Project, cut: Id, audio: Option<std::path::PathBuf>, lufs: Option<f64>) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    let said_as = audio.as_ref().map_or("its own sound".to_owned(), |a| format!("voice {}", a.display()));
    // Its own sound: from the original (no level, as the file is). A voice
    // with its loudness: the level from it; without: the level kept (the
    // audit, 2026-11-12).
    match (&audio, lufs) {
        (None, _) => (c.gain, c.loudness) = (0.0, None),
        (Some(_), Some(_)) => (c.gain, c.loudness) = (super::sound::normal_gain(lufs), lufs),
        (Some(_), None) => {}
    }
    c.audio = audio;
    said(format!("cut #{cut}: {said_as}"))
}

/// `Frame`: an ordinary clip's frame, each value within its range.
fn set_frame(p: &mut Project, cut: Id, frame: Option<crate::frame::Frame>) -> Result<Outcome, OpError> {
    if let Some(f) = frame {
        f.check().map_err(OpError::Invalid)?;
    }
    let i = find(p, cut)?;
    p.cuts[i].frame = frame;
    said(format!("cut #{cut}: {}", frame.map_or("framed whole".to_owned(), |f| format!("frame {}", f.said()))))
}

/// `JoinPieces`: cut `cut`'s pieces one cut again.
fn join_pieces(p: &mut Project, cut: Id) -> Result<Outcome, OpError> {
    let pieces = p.pieces(find(p, cut)?);
    let (Some(&first), Some(&last)) = (pieces.first(), pieces.iter().max_by(|&&a, &&b| p.cuts[a].end.total_cmp(&p.cuts[b].end))) else {
        return Err(OpError::Invalid(format!("cut #{cut} isn't one of several pieces: nothing to join")));
    };
    let start = pieces.iter().map(|&k| p.cuts[k].start).fold(f64::INFINITY, f64::min);
    let tail = p.cuts[last].clone();
    // What was set on any piece since the split stays (the audit,
    // 2026-11-11: the others' marks and points were dropped).
    let kept = owned_lists(&pieces.iter().map(|&k| p.cuts[k].clone()).collect::<Vec<_>>());
    for &k in pieces[1..].iter().rev() {
        p.cuts.remove(k);
    }
    let c = &mut p.cuts[first];
    (c.start, c.end, c.origin) = (start, tail.end, None);
    (c.volume, c.automation, c.tracks, c.effects, c.marks) = kept;
    (c.sound_fade_out, c.sound_after, c.edge_out) = (tail.sound_fade_out, tail.sound_after, tail.edge_out);
    c.settle_sound(f64::INFINITY);
    let (id, end) = (c.id, c.end);
    crate::project::settle_crossfades(&mut p.cuts);
    said(format!("cut #{id}: {} pieces joined back, {start:.2}-{end:.2} s of its clip", pieces.len()))
}

/// The clip-time lists of a clip's `pieces` as one cut keeps them: each
/// entry from the piece whose span holds it (outside all, the first's), so
/// what a split copied to both halves isn't doubled. View keys stay the
/// first piece's.
#[allow(clippy::type_complexity)]
fn owned_lists(pieces: &[crate::project::Cut]) -> (Vec<crate::project::VolumeKey>, Vec<crate::automation::AutoClip>, Vec<crate::automation::ObjectTrack>, Vec<crate::grade::EffectKey>, Vec<crate::project::Mark>) {
    let owner = |t: f64| pieces.iter().position(|c| t >= c.start - 1e-9 && t <= c.end + 1e-9).unwrap_or(0);
    fn gather<T: Clone>(pieces: &[crate::project::Cut], owner: &dyn Fn(f64) -> usize, list: fn(&crate::project::Cut) -> &[T], t: fn(&T) -> f64) -> Vec<T> {
        let mut out: Vec<T> = pieces.iter().enumerate().flat_map(|(k, c)| list(c).iter().filter(move |e| owner(t(e)) == k).cloned()).collect();
        out.sort_by(|a, b| t(a).total_cmp(&t(b)));
        out
    }
    let mut tracks: Vec<crate::automation::ObjectTrack> = Vec::new();
    for (k, c) in pieces.iter().enumerate() {
        for tr in &c.tracks {
            let own = tr.samples.iter().filter(|s| owner(s.t) == k).cloned();
            match tracks.iter_mut().find(|t| t.name == tr.name) {
                Some(t) => t.samples.extend(own),
                None => tracks.push(crate::automation::ObjectTrack { samples: own.collect(), ..tr.clone() }),
            }
        }
    }
    for t in &mut tracks {
        t.samples.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    (
        gather(pieces, &owner, |c| &c.volume, |k| k.t),
        gather(pieces, &owner, |c| &c.automation, |a| a.from),
        tracks,
        gather(pieces, &owner, |c| &c.effects, |e| e.t),
        gather(pieces, &owner, |c| &c.marks, |m| m.t),
    )
}

/// The join before cut `cut`: its look.
fn transition(p: &mut Project, cut: Id, look: Option<crate::transition::Look>) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    if i == 0 {
        return Err(OpError::Invalid(format!("cut #{cut} is the first: no join before it")));
    }
    if look.is_some_and(|l| l.is_edge()) {
        return Err(OpError::Invalid(format!("{} is an edge look: Edge puts it on a cut's start or end", look.unwrap().name())));
    }
    // The settings were the old look's.
    if p.cuts[i].transition != look {
        p.cuts[i].transition_params.clear();
    }
    p.cuts[i].transition = look;
    said(format!("cut #{cut}: transition {}", look.map_or("dissolve", |l| l.name())))
}

/// A look on cut `cut`'s start or end.
fn edge(p: &mut Project, cut: Id, end: End, look: Option<crate::transition::Look>, seconds: f64) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    // An edge look lasts edit seconds: at most the cut's length on the edit.
    let longest = c.edit_len();
    let edge = if end == End::In { &mut c.edge_in } else { &mut c.edge_out };
    let Some(look) = look else {
        *edge = None;
        return said(format!("cut #{cut}: no look on its {}", if end == End::In { "start" } else { "end" }));
    };
    if !look.is_edge() {
        return Err(OpError::Invalid(format!("{} goes across a join (Transition), not on an edge", look.name())));
    }
    let seconds = crate::transition::stepped(seconds).min(longest);
    // The settings were the old look's.
    let params = edge.take().filter(|e| e.look == look).map(|e| e.params).unwrap_or_default();
    *edge = Some(crate::project::Edge { look, seconds, params });
    said(format!("cut #{cut}: {} {seconds:.2} s on its {}", look.name(), if end == End::In { "start" } else { "end" }))
}

/// The join before cut `cut` set whole from a template.
fn set_template(p: &mut Project, cut: &Id, template: &crate::template::Template) -> Result<Outcome, OpError> {
    let i = find(p, *cut)?;
    if i == 0 {
        return Err(OpError::Invalid(format!("cut #{cut} is the first: no join before it")));
    }
    let before = p.cuts[i - 1].id;
    let overlap = template.overlap.as_ref();
    apply(p, &CutOp::Transition { cut: *cut, look: None })?;
    apply(p, &CutOp::Transition { cut: *cut, look: overlap.and_then(|o| o.look) })?;
    apply(p, &CutOp::Crossfade { cut: *cut, seconds: overlap.map_or(0.0, |o| o.seconds) })?;
    for (key, &value) in overlap.iter().flat_map(|o| &o.params) {
        apply(p, &CutOp::TransitionKnob { cut: *cut, key: key.clone(), value: Some(value) })?;
    }
    for (on, end, edge) in [(before, End::Out, &template.edge_out), (*cut, End::In, &template.edge_in)] {
        apply(p, &CutOp::Edge { cut: on, end, look: None, seconds: 0.0 })?;
        let Some(e) = edge else { continue };
        apply(p, &CutOp::Edge { cut: on, end, look: Some(e.look), seconds: e.seconds })?;
        for (key, &value) in &e.params {
            apply(p, &CutOp::EdgeKnob { cut: on, end, key: key.clone(), value: Some(value) })?;
        }
    }
    said(format!("the join before cut #{cut} set from a template"))
}

/// An object's track on cut `cut`, over one of its name.
fn add_track(p: &mut Project, cut: &Id, track: &crate::automation::ObjectTrack) -> Result<Outcome, OpError> {
    let i = find(p, *cut)?;
    if track.name.trim().is_empty() || track.samples.iter().any(|s| ![s.t, s.x, s.y].iter().all(|v| v.is_finite())) || track.samples.windows(2).any(|w| w[0].t > w[1].t) {
        return Err(OpError::Invalid("a track needs a name and samples that are numbers in time order".into()));
    }
    let tracks = &mut p.cuts[i].tracks;
    tracks.retain(|t| t.name != track.name);
    tracks.push(track.clone());
    said(format!("cut #{cut}: track «{}», {} samples", track.name, track.samples.len()))
}

/// Cut `cut`'s track `name` gone.
fn untrack(p: &mut Project, cut: &Id, name: &str) -> Result<Outcome, OpError> {
    let i = find(p, *cut)?;
    let before = p.cuts[i].tracks.len();
    p.cuts[i].tracks.retain(|t| t.name != *name);
    if p.cuts[i].tracks.len() == before {
        return Err(OpError::NotFound(format!("track «{name}» on cut #{cut}")));
    }
    said(format!("cut #{cut}: track «{name}» gone"))
}

/// A marker on cut `cut`.
fn mark(p: &mut Project, cut: &Id, t: &f64, note: &str) -> Result<Outcome, OpError> {
    let i = find(p, *cut)?;
    let c = &mut p.cuts[i];
    if !t.is_finite() {
        return Err(OpError::Invalid(format!("a marker at {t} s")));
    }
    match mark_at(&c.marks, *t) {
        Some(k) => c.marks[k].note = note.to_owned(),
        None => {
            let k = c.marks.partition_point(|m| m.t <= *t);
            c.marks.insert(k, crate::project::Mark { t: *t, note: note.to_owned() });
        }
    }
    said(format!("cut #{cut}: marker at {t:.2} s{}", if note.is_empty() { String::new() } else { format!(" «{note}»") }))
}

/// Cut `cut`'s marker at `from` moved to `t`.
fn move_mark(p: &mut Project, cut: Id, from: f64, t: f64) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    let k = mark_at(&c.marks, from).ok_or_else(|| OpError::NotFound(format!("marker at {from:.2} s on cut #{cut}")))?;
    let mut m = c.marks.remove(k);
    m.t = t.clamp(c.start, c.end);
    let to = c.marks.partition_point(|n| n.t <= m.t);
    let at = m.t;
    c.marks.insert(to, m);
    said(format!("cut #{cut}: marker {from:.2} s → {at:.2} s"))
}

/// Cut `cut`'s sound's own start or end (a J- or L-cut).
fn sound_edge(p: &mut Project, cut: Id, end: End, t: f64, most: f64) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    if !t.is_finite() {
        return Err(OpError::Invalid(format!("a sound edge at {t} s")));
    }
    let c = &mut p.cuts[i];
    let past = match end {
        End::In => t < c.start,
        End::Out => t > c.end,
    };
    if past && !c.speed.is_normal() {
        return Err(OpError::Invalid(format!("cut #{cut} plays at speed: its sound can't run past its picture yet (a J- or L-cut)")));
    }
    let (from, to) = c.sound_span();
    match end {
        // At least MIN_CUT of sound on the edit, at the cut's speed.
        End::In => c.sound_before = c.start - t.clamp(0.0, c.speed.clip_after(to, -MIN_CUT).max(0.0)),
        End::Out => {
            let least = c.speed.clip_after(from, MIN_CUT);
            c.sound_after = t.clamp(least, most.max(least)) - c.end;
        }
    }
    let (from, to) = c.sound_span();
    said(format!("cut #{cut}: sound {from:.2}-{to:.2} s (picture {:.2}-{:.2})", c.start, c.end))
}

/// One of cut `cut`'s edge look's settings, as `TransitionKnob`.
fn edge_knob(p: &mut Project, cut: Id, end: End, key: &str, value: Option<f32>) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    let edge = if end == End::In { &mut c.edge_in } else { &mut c.edge_out };
    let edge = edge.as_mut().ok_or_else(|| OpError::Invalid(format!("cut #{cut} has no look on that edge")))?;
    let said_ = set_knob(edge.look, &mut edge.params, key, value)?;
    said(format!("cut #{cut}: {said_}"))
}

/// Cut `cut` at one `rate` throughout, in `speed::RANGE`; 1 is normal.
fn set_rate(p: &mut Project, cut: Id, rate: f64) -> Result<Outcome, OpError> {
    let (lo, hi) = crate::speed::RANGE;
    if !(lo..=hi).contains(&rate) {
        return Err(OpError::OutOfRange { what: "speed", value: rate, least: lo, most: hi });
    }
    let keys = if rate == 1.0 { Vec::new() } else { crate::speed::Speed::constant(rate).keys };
    set_speed(p, cut, keys)
}

fn tape_pitch(p: &mut Project, cut: Id, on: bool) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    p.cuts[i].speed.tape = on;
    said(format!("cut #{cut}: its sound at speed {}", if on { "like tape" } else { "keeps its pitch" }))
}

/// Cut `cut`'s speed keys to `keys` (none: normal), if they're in order
/// and range and leave it at least `MIN_CUT` on the edit. A J- or L-cut
/// keeps normal speed: its sound beyond the picture plays as it is.
fn set_speed(p: &mut Project, cut: Id, keys: Vec<crate::speed::SpeedKey>) -> Result<Outcome, OpError> {
    let i = find(p, cut)?;
    let c = &mut p.cuts[i];
    let speed = crate::speed::Speed { keys, tape: c.speed.tape };
    speed.check().map_err(OpError::Invalid)?;
    if !speed.is_normal() && (c.sound_before > 0.0 || c.sound_after > 0.0) {
        return Err(OpError::Invalid(format!("cut #{cut} is a J- or L-cut: its sound beyond the picture can't play at speed yet, so its speed stays normal")));
    }
    // Its picture and its sound (shorter where it stops short) at least
    // MIN_CUT on the edit.
    let (from, to) = c.sound_span();
    let len = speed.edit_secs(c.start, c.end).min(speed.edit_secs(from, to));
    if len < MIN_CUT {
        return Err(OpError::Invalid(format!("at that speed cut #{cut} would last {len:.2} s, less than {MIN_CUT} s")));
    }
    c.speed = speed;
    crate::project::settle_crossfades(&mut p.cuts);
    let c = &p.cuts[i];
    said(format!("cut #{cut}: {}, {:.2} s on the edit", c.speed.describe(), c.edit_len()))
}

/// The marker within `SAME_MARK` of `t`.
fn mark_at(marks: &[crate::project::Mark], t: f64) -> Option<usize> {
    marks.iter().position(|m| (m.t - t).abs() < SAME_MARK)
}

/// Where cut `id` is.
fn find(p: &Project, id: Id) -> Result<usize, OpError> {
    p.cut_index(id).ok_or_else(|| OpError::NotFound(format!("cut #{id}")))
}

fn position(i: usize, of: usize) -> String {
    format!("{} of {of}", i + 1)
}

/// A cut's or an item's style to `look` (none: off), its settings kept
/// while the look stays.
pub(super) fn set_style(style: &mut Option<crate::project::Style>, look: Option<crate::transition::Look>) -> Result<(), OpError> {
    if let Some(l) = look.filter(|l| l.group() != crate::transition::Group::Style) {
        return Err(OpError::Invalid(format!("{} isn't a style look", l.name())));
    }
    // The settings were the old look's.
    let params = style.take().filter(|s| Some(s.look) == look).map(|s| s.params).unwrap_or_default();
    *style = look.map(|look| crate::project::Style { look, params });
    Ok(())
}

/// One of `look`'s settings in `params` to `value` (none: the default);
/// only what differs from the default is kept. What it says it did.
pub(super) fn set_knob(look: crate::transition::Look, params: &mut std::collections::BTreeMap<String, f32>, key: &str, value: Option<f32>) -> Result<String, OpError> {
    let knob = look.knob(key).ok_or_else(|| OpError::NotFound(format!("setting {key:?} of {}", look.name())))?;
    match value {
        Some(v) if !(knob.least..=knob.most).contains(&v) => {
            return Err(OpError::OutOfRange { what: knob.name, value: v as f64, least: knob.least as f64, most: knob.most as f64 });
        }
        Some(v) if knob.settle(v) != knob.default => {
            params.insert(knob.key.to_owned(), knob.settle(v));
        }
        _ => {
            params.remove(knob.key);
        }
    }
    Ok(format!("{} {}", knob.name, params.get(knob.key).copied().unwrap_or(knob.default)))
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;

    fn add(input: &str, len: f64, place: Place) -> Op {
        Op::Cut(CutOp::Add { input: input.into(), start: 0.0, end: len, place, stabilize: None, lut: None })
    }

    fn names(e: &Edit) -> Vec<String> {
        e.cuts.iter().map(|c| format!("{}#{}", c.input.display(), c.id)).collect()
    }

    #[test]
    fn several_cuts_move_together_in_their_order() {
        let mut p = Project { cuts: (0..5).map(|k| Cut::new(format!("{k}.mp4").into(), 0.0, 2.0)).collect(), ..Default::default() };
        p.settle_ids();
        let mut e = crate::model::edit::Edit::new(p);
        // Cuts #2 and #4 before #1: 2, 4, 1, 3, 5.
        e.apply(&Op::Cut(CutOp::MoveTogether { cuts: vec![4, 2], before: Some(1) })).unwrap();
        assert_eq!(e.cuts.iter().map(|c| c.id).collect::<Vec<_>>(), [2, 4, 1, 3, 5]);
        // To the end; refused before one of themselves.
        e.apply(&Op::Cut(CutOp::MoveTogether { cuts: vec![2, 4], before: None })).unwrap();
        assert_eq!(e.cuts.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 3, 5, 2, 4]);
        assert!(e.apply(&Op::Cut(CutOp::MoveTogether { cuts: vec![2, 4], before: Some(4) })).is_err());
    }

    #[test]
    fn a_cuts_sound_runs_past_its_picture_or_stops_short_within_its_clip() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::In, t: 1.0, most: 4.0 })).unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::Out, t: 3.0, most: 4.0 })).unwrap();
        let edge = |end, t| Op::Cut(CutOp::SoundEdge { cut: a, end, t, most: 4.0 });
        // An L-cut a second past the picture, and a J-cut further back than
        // the clip has footage: stopped at its start.
        e.apply(&edge(End::Out, 4.0)).unwrap();
        e.apply(&edge(End::In, -2.0)).unwrap();
        assert_eq!(e.cuts[0].sound_span(), (0.0, 4.0));
        assert_eq!((e.cuts[0].sound_before, e.cuts[0].sound_after), (1.0, 1.0));
        // The sound past the picture: before it at edit time -1 (the cut is
        // first), after it from 2 s.
        let beyond = e.cuts[0].sound_beyond(0.0);
        assert_eq!(beyond.iter().map(|c| (c.at, c.start, c.end)).collect::<Vec<_>>(), [(-1.0, 0.0, 1.0), (2.0, 3.0, 4.0)]);
        // Inside the picture: silence after 2.5 s, the picture going on.
        e.apply(&edge(End::Out, 2.5)).unwrap();
        assert_eq!(e.cuts[0].sound_fade(2.4), 1.0);
        assert_eq!(e.cuts[0].sound_fade(2.6), 0.0);
        // Never shorter than a cut can be, and back to the picture.
        e.apply(&edge(End::In, 2.45)).unwrap();
        let (from, to) = e.cuts[0].sound_span();
        assert!((to - from - MIN_CUT).abs() < 1e-9, "{from}-{to}");
        e.apply(&Op::Cut(CutOp::SoundToPicture { cut: a })).unwrap();
        assert_eq!(e.cuts[0].sound_span(), (1.0, 3.0));
        super::super::check(&e).unwrap();
    }

    #[test]
    fn cuts_are_added_split_moved_and_deleted_by_id() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let b = e.apply(&add("b", 3.0, Place::End)).unwrap().made.unwrap();
        // At 2 s: a is split, c goes between its halves; a's second half
        // gets its own id.
        let c = e.apply(&add("c", 1.0, Place::At(2.0))).unwrap().made.unwrap();
        assert_eq!(names(&e), ["a#1", "c#3", "a#4", "b#2"]);
        check(&e).unwrap();
        // Moved before a, by id, whatever the indices are now.
        e.apply(&Op::Cut(CutOp::Move { cut: b, before: Some(a) })).unwrap();
        e.apply(&Op::Cut(CutOp::Move { cut: c, before: None })).unwrap();
        assert_eq!(names(&e), ["b#2", "a#1", "a#4", "c#3"]);
        let half = e.apply(&Op::Cut(CutOp::Split { cut: b, t: 1.0 })).unwrap().made.unwrap();
        assert_eq!(names(&e), ["b#2", format!("b#{half}").as_str(), "a#1", "a#4", "c#3"]);
        assert!(matches!(e.apply(&Op::Cut(CutOp::Split { cut: b, t: 0.1 })), Err(OpError::OutOfRange { .. })));
        e.apply(&Op::Cut(CutOp::Delete { cut: a })).unwrap();
        assert!(matches!(e.apply(&Op::Cut(CutOp::Delete { cut: a })), Err(OpError::NotFound(_))));
        check(&e).unwrap();
        // Every op was its own step.
        for _ in 0..7 {
            assert!(e.undo());
            check(&e).unwrap();
        }
        assert!(e.cuts.is_empty() && !e.undo());
    }

    #[test]
    fn markers_are_set_noted_moved_and_removed_by_time() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let mark = |t: f64, note: &str| Op::Cut(CutOp::Mark { cut: a, t, note: note.into() });
        e.apply(&mark(3.0, "")).unwrap();
        e.apply(&mark(1.0, "")).unwrap();
        e.apply(&mark(1.005, "the castle")).unwrap();
        assert_eq!(e.cuts[0].marks.iter().map(|m| (m.t, m.note.as_str())).collect::<Vec<_>>(), [(1.0, "the castle"), (3.0, "")], "one at 1 s, noted; in order");
        e.apply(&Op::Cut(CutOp::MoveMark { cut: a, from: 1.0, t: 9.0 })).unwrap();
        assert_eq!(e.cuts[0].marks.last().map(|m| m.t), Some(4.0), "kept within the cut, and in order");
        e.apply(&Op::Cut(CutOp::Unmark { cut: a, t: 3.0 })).unwrap();
        assert!(matches!(e.apply(&Op::Cut(CutOp::Unmark { cut: a, t: 3.0 })), Err(OpError::NotFound(_))));
        // A split's halves both carry them; each shows its own stretch's.
        e.apply(&Op::Cut(CutOp::Split { cut: a, t: 2.0 })).unwrap();
        assert_eq!((e.cuts[0].marks.len(), e.cuts[1].marks.len()), (1, 1));
        check(&e).unwrap();
    }

    #[test]
    fn a_template_sets_a_join_whole_and_reads_back_the_same() {
        use crate::transition::Look;
        let mut e = Edit::default();
        let ids: Vec<Id> = (0..4).map(|n| e.apply(&add(&n.to_string(), 4.0, Place::End)).unwrap().made.unwrap()).collect();
        // The join before cut 1, set up by hand.
        e.apply(&Op::Cut(CutOp::Transition { cut: ids[1], look: Some(Look::SpinBlur) })).unwrap();
        e.apply(&Op::Cut(CutOp::Crossfade { cut: ids[1], seconds: 0.75 })).unwrap();
        e.apply(&Op::Cut(CutOp::TransitionKnob { cut: ids[1], key: "turns".into(), value: Some(2.0) })).unwrap();
        e.apply(&Op::Cut(CutOp::Edge { cut: ids[0], end: End::Out, look: Some(Look::EdgeFade), seconds: 0.5 })).unwrap();
        let template = crate::template::Template::of_join(&e.cuts, 1);
        // The join before cut 3 had its own: what the template leaves out goes.
        e.apply(&Op::Cut(CutOp::Edge { cut: ids[3], end: End::In, look: Some(Look::EdgeBlur), seconds: 1.0 })).unwrap();
        e.apply(&Op::Cut(CutOp::Template { cut: ids[3], template: template.clone() })).unwrap();
        assert_eq!(crate::template::Template::of_join(&e.cuts, 3), template);
        assert!(e.cuts[3].edge_in.is_none());
        let wrong = crate::template::Template { overlap: Some(crate::template::Overlap { look: Some(Look::EdgeFade), seconds: 0.5, params: Default::default() }), ..Default::default() };
        let before = e.cuts.clone();
        assert!(e.apply(&Op::Cut(CutOp::Template { cut: ids[2], template: wrong })).is_err());
        assert_eq!(e.cuts, before, "refused whole");
        check(&e).unwrap();
    }

    #[test]
    fn trims_and_crossfades_keep_within_the_clip() {
        let mut e = Edit::default();
        let a = e.apply(&add("a", 4.0, Place::End)).unwrap().made.unwrap();
        let b = e.apply(&add("b", 3.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Trim { cut: a, end: End::Out, t: 9.0, most: 6.0 })).unwrap();
        assert_eq!(e.cuts[0].end, 6.0, "not past the clip's length");
        e.apply(&Op::Cut(CutOp::Transition { cut: b, look: Some(crate::transition::Look::WhipPan) })).unwrap();
        assert_eq!(e.cuts[1].transition, Some(crate::transition::Look::WhipPan));
        assert!(e.apply(&Op::Cut(CutOp::Transition { cut: a, look: Some(crate::transition::Look::Flash) })).is_err(), "the first cut has no join before it");
        let knob = |key: &str, value| Op::Cut(CutOp::TransitionKnob { cut: b, key: key.to_owned(), value });
        e.apply(&knob("streak", Some(1.5))).unwrap();
        e.apply(&knob("direction", Some(1.0))).unwrap();
        assert!(e.apply(&knob("streak", Some(5.0))).is_err() && e.apply(&knob("turns", Some(1.0))).is_err(), "out of range; not the whip's");
        e.apply(&knob("direction", Some(0.0))).unwrap();
        assert_eq!(e.cuts[1].transition_params.len(), 1, "the default isn't kept");
        assert_eq!(e.cuts[1].transition_spec().values[..2], [0.0, 1.5]);
        e.apply(&Op::Cut(CutOp::Transition { cut: b, look: Some(crate::transition::Look::Flash) })).unwrap();
        assert!(e.cuts[1].transition_params.is_empty(), "another look starts at its defaults");
        let edge = |end, look, seconds| Op::Cut(CutOp::Edge { cut: b, end, look, seconds });
        assert!(e.apply(&edge(End::Out, Some(crate::transition::Look::Flash), 1.0)).is_err(), "an overlap look isn't an edge's");
        assert!(e.apply(&Op::Cut(CutOp::Transition { cut: b, look: Some(crate::transition::Look::EdgeFade) })).is_err(), "nor the other way");
        e.apply(&edge(End::Out, Some(crate::transition::Look::EdgeZoom), 0.6)).unwrap();
        e.apply(&Op::Cut(CutOp::EdgeKnob { cut: b, end: End::Out, key: "strength".into(), value: Some(0.8) })).unwrap();
        e.apply(&edge(End::Out, Some(crate::transition::Look::EdgeZoom), 9.0)).unwrap();
        let out = e.cuts[1].edge_out.clone().unwrap();
        assert_eq!((out.seconds, out.params.len()), (3.0, 1), "within the cut, the settings kept: {out:?}");
        e.apply(&edge(End::Out, Some(crate::transition::Look::EdgeBlur), 0.5)).unwrap();
        assert!(e.cuts[1].edge_out.as_ref().unwrap().params.is_empty(), "another look starts at its defaults");
        e.apply(&edge(End::Out, None, 0.0)).unwrap();
        assert!(e.cuts[1].edge_out.is_none());
        let style = |look| Op::Cut(CutOp::Style { cut: a, look });
        assert!(e.apply(&style(Some(crate::transition::Look::EdgeFade))).is_err(), "an edge look isn't a style");
        e.apply(&style(Some(crate::transition::Look::OldFilm))).unwrap();
        e.apply(&Op::Cut(CutOp::StyleKnob { cut: a, key: "grain".into(), value: Some(0.9) })).unwrap();
        e.apply(&style(Some(crate::transition::Look::OldFilm))).unwrap();
        assert_eq!(e.cuts[0].style.as_ref().unwrap().params.len(), 1, "the same look keeps its settings");
        e.apply(&style(Some(crate::transition::Look::Glow))).unwrap();
        assert!(e.cuts[0].style.as_ref().unwrap().params.is_empty(), "another starts at its defaults");
        e.apply(&style(None)).unwrap();
        assert!(e.cuts[0].style.is_none());
        e.apply(&Op::Cut(CutOp::Crossfade { cut: b, seconds: 10.0 })).unwrap();
        assert!(e.cuts[1].crossfade < 3.0 && e.cuts[1].crossfade > 0.0, "within the shorter cut: {}", e.cuts[1].crossfade);
        e.apply(&Op::Cut(CutOp::Stabilize { cut: a, mode: Some(Mode::Level) })).unwrap();
        assert_eq!(e.cuts[0].stabilize, Some(Mode::Level));
        check(&e).unwrap();
    }

    /// Gap 31: a speed set on a cut makes it shorter or longer on the
    /// edit, the cuts after it and the sync-locked music following; out of
    /// range, too short, or a J- or L-cut, it's refused.
    #[test]
    fn a_cut_s_speed_changes_its_length_on_the_edit() {
        let mut e = Edit::default();
        let a = e.apply(&add("a.mp4", 4.0, Place::End)).unwrap().made.unwrap();
        e.apply(&add("b.mp4", 2.0, Place::End)).unwrap();
        e.music.sync = true;
        e.music.clips.push(crate::project::MusicClip { input: "song.mp3".into(), at: 4.5, start: 0.0, end: 1.0, ..Default::default() });
        let speed = |rate| Op::Cut(CutOp::Speed { cut: a, rate });
        e.apply(&speed(2.0)).unwrap();
        assert_eq!((e.cuts[0].clip_len(), e.cuts[0].edit_len(), e.duration()), (4.0, 2.0, 4.0));
        assert_eq!(e.music.clips[0].at, 2.5, "the locked song under b moved with it");
        assert!(crate::apply::describe_lines(&e).join("\n").contains("speed: 2× speed; 4.00 s of its clip, 2.00 s on the edit"));
        e.apply(&speed(1.0)).unwrap();
        assert!(e.cuts[0].speed.is_default() && e.duration() == 6.0);
        for rate in [0.1, 5.0, f64::NAN] {
            assert!(e.apply(&speed(rate)).is_err(), "{rate}");
        }
        // 0.2 s of clip at 4× is 0.05 s: under MIN_CUT.
        let short = e.apply(&add("c.mp4", 0.2, Place::End)).unwrap().made.unwrap();
        assert!(e.apply(&Op::Cut(CutOp::Speed { cut: short, rate: 4.0 })).is_err());
        // A curve, out of order: refused; in order, kept.
        let key = |t, rate| crate::speed::SpeedKey { t, rate };
        assert!(e.apply(&Op::Cut(CutOp::SpeedCurve { cut: a, keys: vec![key(2.0, 1.0), key(1.0, 2.0)] })).is_err());
        e.apply(&Op::Cut(CutOp::SpeedCurve { cut: a, keys: vec![key(0.0, 1.0), key(2.0, 2.0)] })).unwrap();
        assert_eq!(e.cuts[0].edit_len(), 3.0);
        e.apply(&Op::Cut(CutOp::TapePitch { cut: a, on: true })).unwrap();
        assert!(e.cuts[0].speed.tape);
        // Its sound can't run past its picture at speed; a J-cut keeps 1×.
        assert!(e.apply(&Op::Cut(CutOp::SoundEdge { cut: a, end: End::Out, t: 4.5, most: 10.0 })).is_err());
        e.apply(&speed(1.0)).unwrap();
        e.apply(&Op::Cut(CutOp::SoundEdge { cut: a, end: End::Out, t: 4.5, most: 10.0 })).unwrap();
        assert!(e.apply(&speed(2.0)).is_err(), "an L-cut keeps normal speed");
        check(&e).unwrap();
    }

    #[test]
    fn a_clips_pieces_are_joined_back_and_undo_splits_them_again() {
        let mut e = Edit::default();
        let a = e.apply(&add("a.mp4", 9.0, Place::End)).unwrap().made.unwrap();
        e.apply(&add("b.mp4", 2.0, Place::End)).unwrap();
        e.apply(&Op::Cut(CutOp::Edge { cut: a, end: End::Out, look: Some(crate::transition::Look::EdgeFade), seconds: 0.5 })).unwrap();
        let mid = e.apply(&Op::Cut(CutOp::Split { cut: a, t: 3.0 })).unwrap().made.unwrap();
        let last = e.apply(&Op::Cut(CutOp::Split { cut: mid, t: 6.0 })).unwrap().made.unwrap();
        assert_eq!(e.pieces(0), [0, 1, 2], "three pieces of one clip");
        assert!(e.pieces(3).is_empty(), "b.mp4 was never split");
        // The middle one trimmed, and moved after b.mp4.
        e.apply(&Op::Cut(CutOp::Trim { cut: mid, end: End::Out, t: 5.0, most: 9.0 })).unwrap();
        e.apply(&Op::Cut(CutOp::Move { cut: last, before: None })).unwrap();
        let before = e.cuts.len();
        e.apply(&Op::Cut(CutOp::JoinPieces { cut: mid })).unwrap();
        let c = &e.cuts[0];
        assert_eq!((e.cuts.len(), c.id, c.start, c.end, c.origin), (before - 2, a, 0.0, 9.0, None), "all of it again, where the first was");
        assert!(c.edge_out.is_some(), "its out end's look back from the last piece");
        check(&e).unwrap();
        assert!(e.undo() && e.cuts.len() == before && e.pieces(0).len() == 3);
        assert!(e.apply(&Op::Cut(CutOp::JoinPieces { cut: e.cuts[1].id })).is_ok());
        let b = e.cuts.iter().find(|c| c.input == std::path::Path::new("b.mp4")).unwrap().id;
        assert!(e.apply(&Op::Cut(CutOp::JoinPieces { cut: b })).is_err(), "one piece: nothing to join");
    }

    #[test]
    fn joined_pieces_keep_what_was_set_on_each_since_the_split() {
        let mut e = Edit::default();
        let a = e.apply(&add("a.mp4", 9.0, Place::End)).unwrap().made.unwrap();
        let second = e.apply(&Op::Cut(CutOp::Split { cut: a, t: 3.0 })).unwrap().made.unwrap();
        // A marker and a volume point on the second piece only.
        e.apply(&Op::Cut(CutOp::Mark { cut: second, t: 7.0, note: "hit".into() })).unwrap();
        e.apply(&Op::Sound(crate::model::ops::sound::SoundOp::AddPoint { sound: super::super::SoundRef::Cut(second), t: 6.0, db: -12.0 })).unwrap();
        e.apply(&Op::Cut(CutOp::JoinPieces { cut: a })).unwrap();
        let c = &e.cuts[0];
        assert_eq!((c.marks.len(), c.marks[0].t), (1, 7.0), "{:?}", c.marks);
        assert!(c.volume.iter().any(|k| k.t == 6.0), "{:?}", c.volume);
        check(&e).unwrap();
    }

    #[test]
    fn a_cuts_voice_and_its_own_speech_are_set_by_ops() {
        let mut e = Edit::default();
        let a = e.apply(&add("a.mp4", 4.0, Place::End)).unwrap().made.unwrap();
        e.apply(&Op::Cut(CutOp::Voice { cut: a, audio: Some("voice/a/dnn6.flac".into()), lufs: Some(-26.0) })).unwrap();
        assert_eq!((e.cuts[0].audio.is_some(), e.cuts[0].loudness, e.cuts[0].gain), (true, Some(-26.0), super::super::sound::normal_gain(Some(-26.0))), "its level from the voice");
        e.apply(&Op::Cut(CutOp::Voice { cut: a, audio: Some("voice/a/eq.flac".into()), lufs: None })).unwrap();
        assert_eq!(e.cuts[0].loudness, Some(-26.0), "no loudness: the level kept");
        e.apply(&Op::Cut(CutOp::Voice { cut: a, audio: None, lufs: None })).unwrap();
        assert_eq!((e.cuts[0].audio.as_ref(), e.cuts[0].gain), (None, 0.0), "back to its own sound");
        e.apply(&Op::Cut(CutOp::Speech { cut: a, on: true })).unwrap();
        assert!(e.cuts[0].speech);
        let sources = crate::duck::sources(&e);
        assert_eq!(sources.iter().map(|s| s.file.clone()).collect::<Vec<_>>(), [std::path::PathBuf::from("a.mp4")], "its own sound counted as speech");
        check(&e).unwrap();
    }

    #[test]
    fn a_cuts_frame_is_set_by_an_op_within_its_ranges_and_is_what_it_shows() {
        use crate::frame::Frame;
        let mut e = Edit::default();
        let a = e.apply(&add("a.mp4", 4.0, Place::End)).unwrap().made.unwrap();
        let frame = Frame { x: 10.0, y: 0.0, zoom: 150.0, rotation: 0.0 };
        e.apply(&Op::Cut(CutOp::Frame { cut: a, frame: Some(frame) })).unwrap();
        let mut cuts = e.cuts.clone();
        cuts[0].flat = true;
        let crop = crate::project::camera_at(&cuts, 0, 1.0, &|_, _| None).crop;
        assert!((crop.half - 1.0 / 3.0).abs() < 1e-6 && (crop.centre[0] - 0.1).abs() < 1e-6, "{crop:?}");
        assert!(e.apply(&Op::Cut(CutOp::Frame { cut: a, frame: Some(Frame { zoom: 20.0, ..frame }) })).is_err(), "less than the whole picture");
        cuts[0].flat = false;
        assert_eq!(crate::project::camera_at(&cuts, 0, 1.0, &|_, _| None).crop.half, 0.5, "a 360 cut: its view, not a frame");
        e.apply(&Op::Cut(CutOp::Frame { cut: a, frame: None })).unwrap();
        check(&e).unwrap();
    }
}
