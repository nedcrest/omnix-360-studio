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
        !self.edit.cuts.is_empty() || self.edit.sound_clips().next().is_some()
    }

    /// A clip loaded, into the library: its thumbnails for the timeline
    /// read (the task), its quick copy for the playhead made. Shared by
    /// clips added to the edit and those opened in the bin's player, so a
    /// clip played there adds at once.
    pub(super) fn take_in(&mut self, media: Media) -> Task<Message> {
        if media.flat.is_none() {
            self.opts.hw = Some(media.hw);
        }
        let input = media.input.clone();
        let thumbs = (!self.thumbs.contains_key(&input)).then(|| Task::run(thumbs::stream(media.clone()), |m| m));
        self.thumbs.entry(input.clone()).or_default();
        if let Some(flat) = media.flat {
            super::scrub::make(input, flat, media.hw);
        }
        self.library.push(media);
        self.mark_flat();
        thumbs.unwrap_or_else(Task::none)
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
        self.selection = Selection::None;
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
        self.edit.commit();
        self.save_status = None;
        self.timeline_zoom = 1.0;
        self.timeline_start = 0.0;
    }
}

impl App {
    /// The project file and loading clips: this area's messages, as `handle` passes them on.
    pub(super) fn on_file(&mut self, message: Message) -> Task<Message> {
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
                        let (input, duration) = (media.input.clone(), media.duration);
                        let thumbs = self.take_in(media);
                        if self.loading_adds {
                            self.add_cut(input, duration);
                        }
                        return Task::batch([self.open_next(), thumbs]);
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
                let mut project = match Project::load(&path) {
                    Ok(p) => p,
                    Err(e) => {
                        self.error = Some(format!("{e:#}"));
                        return Task::none();
                    }
                };
                // Cuts of clips that are gone can't play; say so and leave them out.
                let (mut cuts, missing): (Vec<_>, Vec<_>) = std::mem::take(&mut project.cuts).into_iter().partition(|c| c.input.exists());
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
                // Sound clips that are gone are left out the same way.
                let mut gone = Vec::new();
                for which in SoundTrack::ALL {
                    let word = which.name();
                    let track = project.track_mut(which);
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
                let (mut overlays, texts) = (std::mem::take(&mut project.overlays), std::mem::take(&mut project.text));
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
                let waves: Vec<_> = project.sound_clips().map(|c| self.load_waveform(&c.input)).collect();
                self.queue.extend(cuts.iter().map(|c| (c.input.clone(), false)));
                let complete = missing.is_empty() && gone.is_empty() && lost.is_empty();
                self.edit.replace(Project { aspect: None, cuts, overlays, text: texts, ..project });
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
                let Some(first) = self.edit.cuts.first().map(|c| &c.input).or(self.edit.sound_clips().next().map(|c| &c.input)) else {
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
            Message::Thumb(input, t, handle) => {
                let thumbs = self.thumbs.entry(input).or_default();
                let i = thumbs.partition_point(|(at, _)| *at < t);
                thumbs.insert(i, (t, handle));
            }
            _ => {}
        }
        Task::none()
    }
}
