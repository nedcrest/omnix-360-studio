//! Putting a clip into the edit: where (`spot`: the end, the playhead, the
//! nearest join), and the cut made of it (`add_cut`); the cut's labels on
//! the timeline.

use super::*;
use crate::model::ops::clips::ClipOp;
use crate::model::ops::cuts::{CutOp, Place};
use crate::model::ops::{End, Op};

/// A cut's names on the timeline, longest first, for the block to show the
/// first that fits: its clip, length, stabilisation when that isn't horizon
/// (the usual), and note.
pub(super) fn cut_labels(c: &Cut, default: Mode) -> Vec<String> {
    let mut label = format!("{} · {:.1} s", clip_name(&c.input), c.end - c.start);
    // Its sound from a file of its own: say which, by its name.
    if let Some(audio) = &c.audio {
        label += &format!(" · ♪ {}", audio.file_stem().map_or_else(String::new, |n| n.to_string_lossy().into_owned()));
    }
    match c.stabilize.unwrap_or(default) {
        Mode::Horizon => {}
        Mode::Off => label += " · unstabilised",
        Mode::Level => label += " · level",
        Mode::Lock => label += " · world lock",
    }
    match c.note.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(note) => vec![format!("{label} · {note}"), label],
        None => vec![label],
    }
}

impl App {
    /// A bin button's place as a spot on the edit, from the playhead now.
    pub(super) fn spot(&self, place: bin::Place) -> bin::Spot {
        match place {
            bin::Place::End => bin::Spot::End,
            bin::Place::Playhead => bin::Spot::At(self.position()),
            bin::Place::Join => bin::Spot::Join(self.position()),
        }
    }

    /// Trims the selected (else playing) cut's in point (`start`) or out
    /// point to edit time `at`, if it falls inside the cut; its index then.
    pub(super) fn trim_to(&mut self, start: bool, at: f64) -> Option<usize> {
        let i = self.selection.cut().unwrap_or(self.current);
        let c = self.edit.cuts.get(i)?;
        let t = c.start + at - self.edit.offset(i);
        if t <= c.start || t >= c.end {
            return None;
        }
        let most = self.library.iter().find(|m| m.input == c.input).map_or(f64::INFINITY, |m| m.duration);
        let cut = c.id;
        self.op(Op::Cut(CutOp::Trim { cut, end: if start { End::In } else { End::Out }, t, most }))?;
        self.selection = Selection::Cut(i);
        Some(i)
    }

    /// Adds the whole clip as a cut where the bin's button said (else at
    /// the end) and selects it. The first clip starts playing, as opening a
    /// clip always did; later ones leave playback where it is.
    pub(super) fn add_cut(&mut self, input: PathBuf, duration: f64) {
        // A picture has no length of its own: a few seconds, trimmed at will.
        let picture = self.library.iter().find(|m| m.input == input).and_then(|m| m.flat).is_some_and(|f| f.picture);
        let end = if picture { ffmpeg::PICTURE_SECONDS } else { duration };
        let lut = self.auto_lut(&input);
        let spot = self.placing.remove(&input).unwrap_or(bin::Spot::End);
        let place = match spot {
            bin::Spot::End => Place::End,
            bin::Spot::At(t) => Place::At(t),
            bin::Spot::Join(t) => Place::Join(t),
        };
        let added = self.op(Op::Cut(CutOp::Add { input, start: 0.0, end, place, stabilize: Some(self.opts.stabilize), lut }));
        let Some(i) = added.and_then(|o| o.made).and_then(|id| self.cut_index(id)) else { return };
        self.mark_flat();
        self.selection = Selection::Cut(i);
        if self.clip.is_none() {
            self.follow = true;
            self.goto(i, 0.0, true);
        } else if spot != bin::Spot::End {
            // Put in among the cuts, it shifts the ones after it: show it,
            // paused at its start, so the playhead stays on what's shown.
            self.follow = true;
            self.goto(i, 0.0, false);
        }
    }
}

impl App {
    /// Editing cuts: this area's messages, as `handle` passes them on.
    pub(super) fn on_cuts(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Select(i) => {
                // The other cut of a crossfade selected while paused: it
                // takes the overlap over, to be framed there.
                if self.partner.as_ref().is_some_and(|(j, _)| *j == i) && self.clip.as_ref().is_some_and(|c| !c.playback.playing) {
                    self.swap_partner();
                }
                self.selection = Selection::Cut(i);
            }
            Message::Trim(i, start, t) => {
                let Some(clip_len) = self.edit.cuts.get(i).and_then(|c| self.library.iter().find(|m| m.input == c.input)).map(|m| m.duration) else {
                    return Task::none();
                };
                self.edit.begin();
                let cut = self.edit.cuts[i].id;
                self.op(Op::Cut(CutOp::Trim { cut, end: if start { End::In } else { End::Out }, t, most: clip_len }));
                let c = &self.edit.cuts[i];
                // Show the frame at the edge being trimmed.
                let t = if start { c.start } else { c.end - 1.0 / self.fps() };
                self.follow = true;
                self.scrub_to(i, t);
            }
            Message::MoveCut(from, to) => {
                if to != from && to != from + 1 && from < self.edit.cuts.len() {
                    let (cut, before) = (self.edit.cuts[from].id, self.edit.cuts.get(to).map(|c| c.id));
                    if self.op(Op::Cut(CutOp::Move { cut, before })).is_some() {
                        let at = |i: usize| crate::project::Project::moved_index(from, to, i);
                        self.current = at(self.current);
                        self.selection = Selection::Cut(at(from));
                    }
                }
            }
            Message::TrimToPlayhead(start) => {
                if let Some(i) = self.trim_to(start, self.position()) {
                    // On the frame trimmed to: the cut's new start, or its
                    // end, where the playhead already was.
                    self.follow = true;
                    let at = if start { self.edit.cuts[i].start } else { self.edit.cuts[i].end - 1.0 / self.fps() };
                    self.goto(i, at, false);
                }
            }
            Message::Crossfade(i, seconds) => {
                let Some(cut) = self.edit.cuts.get(i).map(|c| c.id) else { return Task::none() };
                self.edit.begin();
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
            }
            Message::ToggleCrossfade => {
                let i = self.selection.cut().unwrap_or(self.current);
                let Some(c) = self.edit.cuts.get(i) else { return Task::none() };
                let (cut, seconds) = (c.id, if c.crossfade > 0.0 { 0.0 } else { 1.0 });
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
            }
            Message::Split if let Selection::Clip(which, i) = self.selection => {
                let (Some(clip), at) = (self.edit.track(which).clips.get(i).map(|c| c.id), self.position()) else { return Task::none() };
                // Not where the playhead is: nothing to split, said quietly.
                let Ok(done) = self.edit.apply(&Op::Clip(ClipOp::Split { clip, at })) else { return Task::none() };
                if let Some(j) = done.made.and_then(|id| self.edit.track(which).clips.iter().position(|c| c.id == id)) {
                    self.selection = Selection::Clip(which, j);
                }
            }
            Message::Split => {
                let Some((_, t)) = self.playing_cut() else { return Task::none() };
                let (i, cut) = (self.current, self.edit.cuts[self.current].id);
                if self.edit.apply(&Op::Cut(CutOp::Split { cut, t })).is_ok() {
                    // The playhead is now at the start of the second half.
                    self.current = i + 1;
                    self.selection = Selection::Cut(i + 1);
                }
            }
            Message::DeleteCut => {
                self.sound_menu = None;
                // With nothing selected, the cut under the playhead.
                let s = match self.selection {
                    Selection::None if self.clip.is_some() => Selection::Cut(self.current),
                    s => s,
                };
                let what = match s {
                    Selection::Cut(_) => "Cut deleted",
                    Selection::Clip(..) => "Sound deleted",
                    Selection::Point(..) => "Point deleted",
                    Selection::Item(_) => "Overlay deleted",
                    Selection::None => return Task::none(),
                };
                let done = self.edit.delete(s);
                if let Selection::Cut(i) = s {
                    // The cut after it, now at `i`, stays selected.
                    if done {
                        self.after_edit(self.edit.offset(i.min(self.edit.cuts.len())));
                    }
                } else {
                    self.selection = Selection::None;
                }
                // Undo a moment away, without knowing Ctrl+Z (pack 2).
                if done {
                    let n = self.toast.map_or(0, |(_, n)| n + 1);
                    self.toast = Some((what, n));
                    return Task::perform(super::blocking(|| std::thread::sleep(std::time::Duration::from_secs(6))), move |_| Message::ToastGone(n));
                }
            }
            Message::ToastGone(n) => {
                if self.toast.is_some_and(|(_, m)| m == n) {
                    self.toast = None;
                }
            }
            Message::Undo | Message::Redo => {
                self.toast = None;
                let at = self.position();
                let changed = if matches!(message, Message::Undo) { self.edit.undo() } else { self.edit.redo() };
                if changed {
                    self.mark_flat();
                    self.after_edit(at);
                }
            }
            _ => {}
        }
        Task::none()
    }
}
