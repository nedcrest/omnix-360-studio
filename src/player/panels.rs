//! Panels and settings: which side panel shows, the Info panel, the export
//! and LUT settings, the interface's size, the divider, the window's
//! fullscreen, sound volume and shape.

use super::*;

impl App {
    /// Panels and settings, the window: this area's messages, as `handle` passes them on.
    pub(super) fn on_panels(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Resolution(r) => self.resolution = r,
            Message::ExportClips(on) => self.export_clips = on,
            Message::ExportGraded(on) => self.export_graded = on,
            Message::ExportFormat(f) => self.export_format = f,
            Message::ExportLoudness(l) => self.export_loudness = l.0,
            Message::Info(of) => {
                let path = match of {
                    info::Of::Cut(i) => self.edit.cuts.get(i).map(|c| c.input.clone()),
                    info::Of::Music(which, i) => self.edit.track(which).clips.get(i).map(|m| m.input.clone()),
                    info::Of::Overlay(r) => self.edit.item(r).filter(|it| it.text.is_none()).map(|it| it.input.clone()),
                    // The bin opens its files with their path (`open_info`).
                    info::Of::File(_) => None,
                };
                let Some(path) = path else { return Task::none() };
                return self.open_info(of, path);
            }
            Message::InfoRead(path, file) => {
                if let Some(info) = &mut self.info
                    && info.path == path
                {
                    info.file = Some(file);
                }
            }
            Message::InfoClose => self.info = None,
            Message::Reveal(path) => {
                std::thread::spawn(move || {
                    if let Err(e) = info::reveal(&path) {
                        eprintln!("couldn't show {}: {e:#}", path.display());
                    }
                });
            }
            Message::CopyPath(path) => {
                self.save_status = Some("Copied the file's path".to_owned());
                return iced::clipboard::write(path.display().to_string());
            }
            Message::ExportPanel => self.toggle_side(Side::Export),
            Message::AudioPanel => self.toggle_side(Side::Audio),
            Message::Shortcuts => self.shortcuts = !self.shortcuts,
            Message::SettingsPanel => {
                self.settings_open = !self.settings_open;
                if self.settings_open {
                    self.info = None;
                    // Dailix may have moved its place since the start.
                    self.dailix_projects = crate::dailix_projects();
                }
            }
            Message::AutoKey(on) => self.auto_key = on,
            Message::SplitDrag(dy) => self.split = (self.split_kept - dy / timeline::SCALABLE).clamp(*timeline::SCALES.start(), *timeline::SCALES.end()),
            Message::SplitDone => self.split_kept = self.split,
            Message::SplitReset => (self.split, self.split_kept) = (1.0, 1.0),
            Message::TenBit(on) => {
                self.ten_bit = on;
                source::set_deep(on);
            }
            Message::AddLut => {
                return Task::perform(
                    async { rfd::AsyncFileDialog::new().set_title("Add a LUT").add_filter("3D LUT", &["cube", "CUBE"]).pick_file().await.map(|f| f.path().to_owned()) },
                    Message::LutPicked,
                );
            }
            Message::LutPicked(None) => {}
            Message::LutPicked(Some(path)) => match crate::lut::add_to_library(&path) {
                Ok(name) => {
                    self.luts = crate::lut::library_names();
                    self.save_status = Some("LUT added".to_owned());
                    // DJI's LUT added with no choice made yet: D-Log M
                    // clips get it, those in the edit too.
                    let squashed = name.to_lowercase().replace([' ', '-', '_'], "");
                    if self.dlog_m_lut.is_none() && squashed.contains("dlogm") {
                        self.dlog_m_lut = Some(name);
                        self.lut_dlog_m_cuts();
                    }
                    self.mark_flat();
                }
                Err(e) => self.error = Some(format!("Couldn't add the LUT: {e:#}")),
            },
            Message::RemoveLut(name) => {
                if let Err(e) = crate::lut::remove_from_library(&name) {
                    self.error = Some(format!("{e:#}"));
                }
                if self.dlog_m_lut.as_ref() == Some(&name) {
                    self.dlog_m_lut = None;
                }
                self.luts = crate::lut::library_names();
                self.mark_flat();
            }
            Message::DlogMLut(choice) => self.dlog_m_lut = choice.name(),
            Message::UiScale(steps) => {
                self.ui_scale = if steps == 0 {
                    settings::UI_DEFAULT
                } else {
                    // Rounded to the step, so repeated clicks land on whole percents.
                    ((self.ui_scale + steps as f32 * settings::UI_STEP) / settings::UI_STEP).round() * settings::UI_STEP
                }
                .clamp(*settings::UI_SCALE.start(), *settings::UI_SCALE.end());
            }
            Message::ShowAngles(on) => self.angles_shown = on,
            Message::RulerLarge(on) => self.ruler_large = on,
            Message::ProjectsChoose => {
                let start = self.projects_place();
                return Task::perform(
                    async move {
                        let mut d = rfd::AsyncFileDialog::new().set_title("The projects folder");
                        if let Some(s) = start {
                            d = d.set_directory(s);
                        }
                        d.pick_folder().await.map(|f| f.path().to_owned())
                    },
                    Message::ProjectsPicked,
                );
            }
            Message::ProjectsPicked(Some(folder)) => self.projects = Some(folder),
            Message::ProjectsPicked(None) => {}
            Message::ProjectsFromDailix => self.projects = None,
            Message::BarLabels => self.bar_labels = !self.bar_labels,
            Message::Snapping => self.snapping = !self.snapping,
            Message::Fold(lane, folded) => self.folds.set(lane, folded),
            Message::Modifiers(m) => self.modifiers = m,
            Message::ViewSize(size) => self.view_size = size,
            Message::Aspect(a) => self.aspect = a,
            Message::Theme(theme) => self.theme = theme,
            Message::Mute => {
                self.muted = !self.muted;
                audio::set_listening(self.volume());
            }
            Message::Volume(v) => {
                self.volume = v;
                self.muted = false;
                audio::set_listening(self.volume());
            }
            Message::Fullscreen => {
                self.fullscreen = !self.fullscreen;
                let mode = if self.fullscreen { window::Mode::Fullscreen } else { window::Mode::Windowed };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
            _ => {}
        }
        Task::none()
    }

    /// The Info panel on `path`, as `of` names it.
    pub(super) fn open_info(&mut self, of: info::Of, path: PathBuf) -> Task<Message> {
        self.settings_open = false;
        // Read once per file: another cut of it shows at once.
        let file = self.info.take().filter(|i| i.path == path).and_then(|i| i.file).filter(|f| f.is_ok());
        let read = file.is_none();
        self.info = Some(info::Info { of, path: path.clone(), file });
        if !read {
            return Task::none();
        }
        let song = matches!(of, info::Of::Music(..) | info::Of::File(true));
        let p = path.clone();
        Task::perform(blocking(move || info::read(&p, song).map(Arc::new).map_err(|e| format!("{e:#}"))), move |r| Message::InfoRead(path.clone(), r))
    }
}
