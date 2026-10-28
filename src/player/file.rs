//! The project file and the clips' loading: opening clips one after another,
//! asking about unsaved changes, the window's title, the loading bar, and
//! what the edit is saved as (`to_project`).

use super::*;

/// What waits on the edit being saved or let go.
#[derive(Debug, Clone, Copy)]
pub(super) enum Then {
    Close,
    OpenProject,
    NewProject,
}

/// An answer to "save changes first?".
#[derive(Debug, Clone, Copy)]
pub(super) enum Choice {
    Save,
    Discard,
    Cancel,
}

impl App {
    /// Adds the next queued clip: straight away if it is in the library
    /// already, else once it has loaded.
    pub(super) fn open_next(&mut self) -> Task<Message> {
        // A project's first cut plays once its clip is in, while the rest load.
        if self.clip.is_none() && self.edit.cuts.first().is_some_and(|c| self.library.iter().any(|m| m.input == c.input)) {
            let start = self.edit.cuts[0].start;
            self.follow = true;
            self.goto(0, start, false);
        }
        while let Some((path, add)) = self.queue.pop_front() {
            match self.library.iter().find(|m| m.input == path).map(|m| m.duration) {
                Some(duration) if add => self.add_cut(path, duration),
                Some(_) => {}
                None => {
                    self.loading_adds = add;
                    return self.open(path);
                }
            }
        }
        self.loaded = 0;
        Task::none()
    }

    /// The edit as a project file holds it.
    pub(super) fn to_project(&self) -> Project {
        let aspect = match self.aspect {
            Aspect::Free => None,
            Aspect::Ratio(w, h) => Some(crate::view::Aspect(w.into(), h.into())),
        };
        let (look_360, look_flat) = if self.look_flat { (self.other_look, self.look) } else { (self.look, self.other_look) };
        // The video being framed by hand right now, if any (`Project::as_shown`).
        let framing = (!self.follow).then_some(self.current);
        Project { aspect, ..self.edit.as_shown(look_360, look_flat, framing) }
    }

    /// Asks whether to save the edit before doing `then`.
    pub(super) fn confirm(&self, then: Then) -> Task<Message> {
        let name = self.project.as_ref().and_then(|p| p.file_name()).map_or_else(|| "this edit".to_owned(), |n| n.to_string_lossy().into_owned());
        let ask = rfd::AsyncMessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Unsaved changes")
            .set_description(format!("Save the changes to {name} first?"))
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom("Save".into(), "Don't save".into(), "Cancel".into()));
        Task::perform(ask.show(), move |answer| {
            // Backends report custom buttons by label, some as yes/no.
            let choice = match answer {
                rfd::MessageDialogResult::Yes => Choice::Save,
                rfd::MessageDialogResult::No => Choice::Discard,
                rfd::MessageDialogResult::Custom(label) if label == "Save" => Choice::Save,
                rfd::MessageDialogResult::Custom(label) if label == "Don't save" => Choice::Discard,
                _ => Choice::Cancel,
            };
            Message::Confirmed(then, choice)
        })
    }

    /// Does what waited on the edit being saved or let go.
    pub(super) fn proceed(&mut self, then: Then) -> Task<Message> {
        match then {
            Then::Close => {
                self.stop_preview();
                iced::exit()
            }
            Then::OpenProject => self.update(Message::OpenProject),
            Then::NewProject => self.update(Message::NewProject),
        }
    }

    pub(super) fn unsaved(&self) -> bool {
        (self.edit.version != self.saved_version || self.aspect != self.saved_aspect) && self.has_edit()
    }

    /// Whether there is anything to save: cuts, or music to fill.
    pub(super) fn has_edit(&self) -> bool {
        !self.edit.cuts.is_empty() || !self.edit.music.clips.is_empty() || !self.edit.sfx.clips.is_empty()
    }

    /// Parses the clip and probes the decoder off the UI thread.
    pub(super) fn open(&mut self, path: PathBuf) -> Task<Message> {
        self.loading = Some(path.clone());
        self.error = None;
        let opts = self.opts.clone();
        let part = self.loading_part.clone();
        part.store(0f64.to_bits(), std::sync::atomic::Ordering::Relaxed);
        let progress = move |f: f64| part.store(f.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
        Task::perform(blocking(move || Media::load(path, &opts, &progress).map_err(|e| format!("{e:#}"))), |r| Message::Loaded(Arc::new(r)))
    }

    pub(super) fn title(&self) -> String {
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // A dot marks unsaved changes, as in many editors.
        let project = match &self.project {
            Some(p) => format!(" · {}{}", name(p), if self.unsaved() { " •" } else { "" }),
            None if self.unsaved() => " · unsaved edit •".to_owned(),
            None => String::new(),
        };
        match (&self.loading, &self.clip) {
            (Some(_), _) => format!("{NAME}{project} · loading {:.0}%", self.load_progress() * 100.0),
            (None, Some(c)) => format!("{NAME}{project} · {}", name(&c.playback.media.input)),
            (None, None) => format!("{NAME}{project}"),
        }
    }

    /// Clips loaded and to load since the queue was last empty: a clip
    /// that several cuts use loads once.
    pub(super) fn load_total(&self) -> usize {
        let waiting: std::collections::HashSet<&PathBuf> =
            self.queue.iter().map(|(p, _)| p).filter(|p| self.loading.as_ref() != Some(*p) && !self.library.iter().any(|m| &m.input == *p)).collect();
        self.loaded + waiting.len() + usize::from(self.loading.is_some())
    }

    /// How far loading the queued clips has got, 0..1.
    pub(super) fn load_progress(&self) -> f32 {
        let part = f64::from_bits(self.loading_part.load(std::sync::atomic::Ordering::Relaxed));
        ((self.loaded as f64 + part) / self.load_total().max(1) as f64) as f32
    }

    /// A bar for loading clips, rather than their names going by.
    pub(super) fn loading_bar(&self, length: f32) -> Element<'_, Message> {
        let total = self.load_total();
        let label = if total > 1 { format!("Loading clip {} of {total}", self.loaded + 1) } else { "Loading…".to_owned() };
        row![text(label), progress_bar(0.0..=1.0, self.load_progress()).length(length).girth(8)].spacing(8).align_y(Alignment::Center).into()
    }

    /// The logo shown while a clip loads or decodes its first frame.
    pub(super) fn logo(&self) -> Element<'_, Message> {
        image(self.logo.clone()).width(Length::Fixed(420.0)).into()
    }

    /// Forgets what pointed into the edit being left: selections, the
    /// playing clip, menus and dialogs, places waiting for clips, the
    /// timeline's window. New and Open share it: each used to clear its own
    /// list, and both missed some (an overlay still selected after opening
    /// a project, so Del removed an item of the new one).
    pub(super) fn forget_edit_state(&mut self) {
        self.selected = None;
        self.selected_overlay = None;
        self.selected_music = None;
        self.selected_volume = None;
        self.selected_sound = None;
        self.hand_cut = None;
        self.info = None;
        self.text_of = None;
        self.clip = None;
        self.held = None;
        self.current = 0;
        self.sound_menu = None;
        self.normalize = None;
        self.placing.clear();
        self.picking = None;
        self.auto_key_at = None;
        self.dragging = false;
        self.save_status = None;
        self.timeline_zoom = 1.0;
        self.timeline_start = 0.0;
    }
}
