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
    /// Adds the whole clip as a cut at the end of the edit and selects it.
    /// The first clip starts playing, as opening a clip always did; later
    /// ones leave playback where it is.
    /// A bin button's place as a spot on the edit, from the playhead now.
    pub(super) fn spot(&self, place: bin::Place) -> bin::Spot {
        match place {
            bin::Place::End => bin::Spot::End,
            bin::Place::Playhead => bin::Spot::At(self.position()),
            bin::Place::Join => bin::Spot::Join(self.position()),
        }
    }

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
