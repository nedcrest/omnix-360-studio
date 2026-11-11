//! The cuts: putting a clip into the edit (where, `spot`: the end, the
//! playhead, the nearest join; the cut made of it, `add_cut`), the cuts'
//! labels on the timeline, and `on_cuts`, the messages that edit them:
//! select, trim, move, split, delete, crossfades, marks, undo and redo,
//! and the toasts that say what happened.

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
    pub(super) fn spot(&self, place: bin::Place) -> Place {
        match place {
            bin::Place::End => Place::End,
            bin::Place::Playhead => Place::At(self.position()),
            bin::Place::Join => Place::Join(self.position()),
        }
    }

    /// Trims the selected (else playing) cut's in point (`start`) or out
    /// point to edit time `at`, if it falls inside the cut; its index then.
    pub(super) fn trim_to(&mut self, start: bool, at: f64) -> Option<usize> {
        let i = self.selection.cut(&self.edit).unwrap_or(self.current);
        let c = self.edit.cuts.get(i)?;
        let t = c.start + at - self.edit.offset(i);
        if t <= c.start || t >= c.end {
            return None;
        }
        let most = self.library.iter().find(|m| m.input == c.input).map_or(f64::INFINITY, |m| m.duration);
        let cut = c.id;
        self.op(Op::Cut(CutOp::Trim { cut, end: if start { End::In } else { End::Out }, t, most }))?;
        self.selection = Selection::cut_at(&self.edit, i);
        Some(i)
    }

    /// Adds the whole clip as a cut where the bin's button said (else at
    /// the end) and selects it. The first clip starts playing, as opening a
    /// clip always did; later ones leave playback where it is.
    pub(super) fn add_cut(&mut self, input: PathBuf, duration: f64) {
        // A picture has no length of its own: a few seconds, trimmed at will.
        let picture = self.library.iter().find(|m| m.input == input).and_then(|m| m.flat).is_some_and(|f| f.picture);
        let (start, end) = match self.bin.spans.remove(&input) {
            Some((s, e)) if !picture => (s.clamp(0.0, duration), e.clamp(0.0, duration)),
            _ => (0.0, if picture { ffmpeg::PICTURE_SECONDS } else { duration }),
        };
        let lut = self.auto_lut(&input);
        let place = self.placing.remove(&input).unwrap_or(Place::End);
        let added = self.op(Op::Cut(CutOp::Add { input, start, end, place, stabilize: Some(self.opts.stabilize), lut }));
        let Some(i) = added.and_then(|o| o.made).and_then(|id| self.edit.cut_index(id)) else { return };
        self.mark_flat();
        self.selection = Selection::cut_at(&self.edit, i);
        if self.clip.is_none() {
            self.follow = true;
            self.goto(i, 0.0, true);
        } else if place != Place::End {
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
                self.pick(Selection::cut_at(&self.edit, i));
            }
            Message::SelectOne(i) => self.pick_one(i),
            Message::AddBlank => return self.add_blank(),
            m @ (Message::SoundEdge(..) | Message::Trim(..) | Message::TrimToPlayhead(_) | Message::Crossfade(..) | Message::ToggleCrossfade) => return self.on_trims(m),
            Message::MoveCut(from, to) if self.move_together(from, to) => {}
            Message::MoveCut(from, to) => {
                if to != from && to != from + 1 && from < self.edit.cuts.len() {
                    let (cut, before) = (self.edit.cuts[from].id, self.edit.cuts.get(to).map(|c| c.id));
                    if self.op(Op::Cut(CutOp::Move { cut, before })).is_some() {
                        // The selection names the cut by id: it follows by itself.
                        self.current = crate::project::Project::moved_index(from, to, self.current);
                        self.selection = Selection::Cut(cut);
                    }
                }
            }
            Message::Split if let Some((which, i)) = self.selection.clip(&self.edit) => {
                let (Some(clip), at) = (self.edit.track(which).clips.get(i).map(|c| c.id), self.position()) else { return Task::none() };
                // Not where the playhead is: nothing to split, said quietly.
                let Ok(done) = self.edit.apply(&Op::Clip(ClipOp::Split { clip, at })) else { return Task::none() };
                if let Some(j) = done.made.and_then(|id| self.edit.track(which).clips.iter().position(|c| c.id == id)) {
                    self.selection = Selection::clip_at(&self.edit, which, j);
                }
            }
            Message::Split => {
                let Some((_, t)) = self.playing_cut() else { return Task::none() };
                let (i, cut) = (self.current, self.edit.cuts[self.current].id);
                if self.edit.apply(&Op::Cut(CutOp::Split { cut, t })).is_ok() {
                    // The playhead is now at the start of the second half.
                    self.current = i + 1;
                    self.selection = Selection::cut_at(&self.edit, i + 1);
                }
            }
            Message::DeleteCut => return self.delete_selected(),
            Message::SelectAtPlayhead => {
                // The cut playing there (the one shown through a crossfade).
                if self.playing_cut().is_some() {
                    self.selection = Selection::cut_at(&self.edit, self.current);
                    return self.step_note("selected");
                }
            }
            Message::StepsRested(n) => {
                if n == self.steps {
                    self.with_playback(|p| p.settle());
                }
            }
            Message::StepNoteGone(n) => {
                if self.step_note.is_some_and(|(_, m)| m == n) {
                    self.step_note = None;
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

    /// A cut's edges trimmed (by a drag, or to the playhead), its sound's
    /// edges, its crossfade.
    fn on_trims(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SoundEdge(i, start, t) => {
                let Some(c) = self.edit.cuts.get(i) else { return Task::none() };
                let most = self.library.iter().find(|m| m.input == c.input).map_or(f64::INFINITY, |m| m.duration);
                let cut = c.id;
                // One undo step for the drag, ended by `ScrubDone`.
                self.edit.begin();
                self.op(Op::Cut(CutOp::SoundEdge { cut, end: if start { End::In } else { End::Out }, t, most }));
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
                let t = if start { c.start } else { c.end - self.frame_len() };
                self.follow = true;
                self.scrub_to(i, t);
            }
            Message::TrimToPlayhead(start) => {
                if let Some(i) = self.trim_to(start, self.position()) {
                    // On the frame trimmed to: the cut's new start, or its
                    // end, where the playhead already was.
                    self.follow = true;
                    let at = if start { self.edit.cuts[i].start } else { self.edit.cuts[i].end - self.frame_len() };
                    self.goto(i, at, false);
                }
            }
            Message::Crossfade(i, seconds) => {
                let Some(cut) = self.edit.cuts.get(i).map(|c| c.id) else { return Task::none() };
                self.edit.begin();
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
            }
            Message::ToggleCrossfade => {
                let i = self.selection.cut(&self.edit).unwrap_or(self.current);
                let Some(c) = self.edit.cuts.get(i) else { return Task::none() };
                let (cut, seconds) = (c.id, if c.crossfade > 0.0 { 0.0 } else { 1.0 });
                self.op(Op::Cut(CutOp::Crossfade { cut, seconds }));
            }
            _ => {}
        }
        Task::none()
    }

    /// A blank (black) cut at the playhead: its picture made in the
    /// project's pictures folder (before the first save, in the cache).
    fn add_blank(&mut self) -> Task<Message> {
        // In the project's pictures folder, so it travels with the
        // project; before the first save, in the cache.
        let pictures = self.project_root().and_then(|r| super::bin_view::folder_for(&r, bin::Kind::Picture, None));
        let dir = pictures.or_else(|| crate::cache::dir().map(|d| d.join("blank"))).unwrap_or_else(std::env::temp_dir);
        match crate::blank::ensure(&dir) {
            Ok(path) => {
                // Placed from the playhead now, as a bin button does.
                self.placing.insert(path.clone(), Place::At(self.position()));
                return self.update(Message::Picked(vec![path]));
            }
            Err(e) => self.error = Some(format!("Can't make the blank picture: {e:#}")),
        }
        Task::none()
    }

    /// Del: what's selected deleted (with nothing selected, the cut under
    /// the playhead), said in a toast with Undo a moment.
    fn delete_selected(&mut self) -> Task<Message> {
        self.sound_menu = None;
        // A frame key picked on the Frame row (gap 107): it alone.
        if let Some((cut, t)) = self.frame_key.take()
            && self.selection == Selection::Cut(cut)
        {
            self.op(Op::Auto(crate::model::ops::auto::AutoOp::FrameKeyDelete { cut, t, tolerance: 1e-6 }));
            return Task::none();
        }
        if self.delete_picked() {
            return Task::none();
        }
        // With nothing selected, the cut under the playhead.
        let s = match self.selection {
            Selection::None if self.clip.is_some() => Selection::cut_at(&self.edit, self.current),
            s => s,
        };
        let what = match s {
            Selection::Cut(_) => "Cut deleted",
            Selection::Clip(..) => "Sound deleted",
            Selection::Point(..) => "Point deleted",
            Selection::Item(_) => "Overlay deleted",
            Selection::Mark(..) => "Marker deleted",
            Selection::Join(_) => "Transition taken away",
            Selection::None => return Task::none(),
        };
        let at = s.cut(&self.edit);
        let done = self.edit.delete(s);
        if let Some(i) = at {
            // The cut after it, now at `i`, selected: Del again
            // takes the next.
            if done {
                self.selection = Selection::cut_at(&self.edit, i);
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
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::app;
    use super::*;

    /// Pack 2's key routes for drags: Alt+] trims the selected cut's end
    /// to the playhead, Alt+[ its start; Alt+→ moves it one place.
    #[test]
    fn a_cut_trims_to_the_playhead_and_moves_by_keys() {
        let mut app = app();
        app.selection = Selection::cut_at(&app.edit, 0);
        // The playhead 1.5 s into cut 0 (0..2 s).
        assert_eq!(app.trim_to(false, 1.5), Some(0));
        assert_eq!(app.edit.cuts[0].end, 1.5);
        assert_eq!(app.trim_to(true, 9.0), None, "the playhead outside the cut: nothing");
        let first = app.edit.cuts[0].id;
        let _ = app.update(Message::MoveCut(0, 2));
        assert_eq!(app.edit.cuts[1].id, first);
        assert!(app.edit.undo() && app.edit.undo() && app.edit.cuts[0].end == 2.0 && !app.edit.undo());
    }

    /// After a delete, «… deleted · Undo» shows; Undo takes it back and the
    /// note goes; an older note's timer leaves a newer one alone.
    #[test]
    fn a_delete_offers_its_undo() {
        let mut app = app();
        app.edit.voice.clips.push(crate::project::MusicClip { input: "nowhere/take.wav".into(), at: 0.0, start: 0.0, end: 3.0, ..Default::default() });
        app.edit.voice.clips.push(crate::project::MusicClip { input: "nowhere/take.wav".into(), at: 4.0, start: 0.0, end: 3.0, ..Default::default() });
        app.edit.settle_ids();
        app.selection = Selection::clip_at(&app.edit, SoundTrack::Voice, 0);
        let _ = app.update(Message::DeleteCut);
        assert_eq!(app.toast.map(|t| t.0), Some("Sound deleted"));
        let first = app.toast.unwrap().1;
        app.selection = Selection::clip_at(&app.edit, SoundTrack::Voice, 0);
        let _ = app.update(Message::DeleteCut);
        let _ = app.update(Message::ToastGone(first));
        assert!(app.toast.is_some(), "the newer note stays");
        let _ = app.update(Message::Undo);
        assert!(app.toast.is_none() && app.edit.voice.clips.len() == 1);
    }

    #[test]
    fn a_deleted_cut_comes_back_with_undo() {
        let mut app = app();
        app.selection = Selection::cut_at(&app.edit, 0);
        let _ = app.update(Message::DeleteCut);
        assert_eq!(app.edit.cuts.len(), 1);
        let _ = app.update(Message::Undo);
        assert_eq!(app.edit.cuts.len(), 2, "back");
        // Through a marker's drag before it, too.
        let cut = app.edit.cuts[0].id;
        app.edit.apply(&crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Mark { cut, t: 1.0, note: String::new() })).unwrap();
        let _ = app.update(Message::MarkMove(0, 1.0, 1.5));
        let _ = app.update(Message::ScrubDone);
        app.selection = Selection::cut_at(&app.edit, 1);
        let _ = app.update(Message::DeleteCut);
        let _ = app.update(Message::Undo);
        assert_eq!(app.edit.cuts.len(), 2, "back after a marker drag");
    }
}
