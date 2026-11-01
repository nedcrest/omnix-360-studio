//! Putting a clip into the edit: where (`spot`: the end, the playhead, the
//! nearest join), and the cut made of it (`add_cut`); the cut's labels on
//! the timeline.

use super::*;

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

    /// Adds the whole clip as a cut where the bin's button said (else at
    /// the end) and selects it. The first clip starts playing, as opening a
    /// clip always did; later ones leave playback where it is.
    pub(super) fn add_cut(&mut self, input: PathBuf, duration: f64) {
        self.edit.checkpoint();
        // A picture has no length of its own: a few seconds, trimmed at will.
        let picture = self.library.iter().find(|m| m.input == input).and_then(|m| m.flat).is_some_and(|f| f.picture);
        let end = if picture { ffmpeg::PICTURE_SECONDS } else { duration };
        let lut = self.auto_lut(&input);
        let spot = self.placing.remove(&input).unwrap_or(bin::Spot::End);
        let cut = Cut { stabilize: Some(self.opts.stabilize), lut, ..Cut::new(input, 0.0, end) };
        let i = match spot {
            bin::Spot::End => self.edit.insert_cut(usize::MAX, cut),
            bin::Spot::At(t) => self.edit.insert_at(t, cut),
            bin::Spot::Join(t) => {
                let at = self.edit.nearest_join(t);
                self.edit.insert_cut(at, cut)
            }
        };
        self.mark_flat();
        self.selected = Some(i);
        // Cuts after it moved: a sound point selected by (cut, point) or a
        // track's would now name the wrong one.
        self.selected_sound = None;
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
                self.selected = Some(i);
                self.selected_music = None;
                self.selected_volume = None;
                self.selected_sound = None;
                self.selected_overlay = None;
            }
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
                    let at = |i: usize| edit::Edit::moved_index(from, to, i);
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
                let (own, i) = self.selected_sound.take().unwrap();
                if self.edit.own_line(own).is_some_and(|(keys, _)| i < keys.len()) {
                    self.edit.checkpoint();
                    if let Some((keys, _)) = self.edit.own_line_mut(own) {
                        keys.remove(i);
                    }
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
            _ => {}
        }
        Task::none()
    }
}
