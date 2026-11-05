//! Long work on threads, with progress and Cancel in the top bar: the
//! export, colour tracking and the box drawn to pick a colour; and asking
//! before closing while one runs.

use super::*;

/// A finished colour track: its CSV, where it starts (cut and clip time),
/// how many samples found the colour, and the colour.
#[derive(Debug, Clone)]
pub struct TrackDone {
    pub(super) path: PathBuf,
    /// The tracked cut, by id: tracking takes a minute or two, and the cuts
    /// may be moved or deleted meanwhile.
    pub(super) cut: crate::project::Id,
    pub(super) t: f64,
    pub(super) found: usize,
    pub(super) rows: usize,
    pub(super) colour: String,
}

impl App {
    /// Renders the edit to `path` on its own thread with the export the
    /// command line uses, at the chosen resolution, in the aspect lock's
    /// shape (16:9 when free), stabilised as the player is.
    pub(super) fn start_export(&mut self, path: PathBuf) -> Task<Message> {
        let project = self.to_project();
        let [w, h] = project.aspect.unwrap_or(crate::view::Aspect(16, 9)).size(self.resolution.short_side());
        let target = if self.export_clips {
            crate::Target::Clips { dir: path, name: self.export_name(), handles: 0.0, graded: self.export_graded }
        } else {
            crate::Target::One(path.with_extension(self.export_format.extension()))
        };
        let out = crate::ExportOpts { size: (w as usize, h as usize), draft: false, cpu: false, format: self.export_format, target, loudness: self.export_loudness };
        let opts = crate::StitchOpts {
            stabilize: Some(self.opts.stabilize),
            imu_axes: self.opts.imu_axes,
            world_z_down: self.opts.world_z_down,
            no_rs: !self.rolling_shutter,
            ..Default::default()
        };
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.exporting = Some((cancel.clone(), 0.0));
        self.save_status = None;
        background(move |say| {
            let mut progress = |done: usize, total: usize| {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    anyhow::bail!("cancelled");
                }
                if done.is_multiple_of(5) || done == total {
                    say(Message::ExportProgress(done as f32 / total.max(1) as f32));
                }
                Ok(())
            };
            Message::ExportDone(crate::export(&project, &opts, &out, &mut progress).map_err(|e| format!("{e:#}")))
        })
    }

    /// What exports are named after: the project, else the first clip.
    pub(super) fn export_base(&self) -> &Path {
        self.project.as_deref().unwrap_or(&self.edit.cuts[0].input)
    }

    pub(super) fn export_name(&self) -> String {
        self.export_base().file_stem().map_or_else(|| "export".to_owned(), |s| s.to_string_lossy().into_owned())
    }

    /// Tracks `colour` from the playhead to the end of the cut on a thread,
    /// starting from `dir` (where the picked box was, in the rig), into a
    /// CSV beside the project (or the clip, before it is saved).
    pub(super) fn start_track(&mut self, colour: crate::track::Colour, hex: String, dir: [f64; 3]) -> Task<Message> {
        let Some((cut, t)) = self.playing_cut() else { return Task::none() };
        let (input, end, i) = (cut.input.clone(), cut.end, cut.id);
        let folder = self.project.as_deref().unwrap_or(&input).parent().unwrap_or(Path::new("")).to_owned();
        let stem = input.file_stem().map_or_else(|| "clip".to_owned(), |s| s.to_string_lossy().into_owned());
        let path = folder.join(format!("{stem}-colour-{t:.1}s.csv"));
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.tracking = Some((cancel.clone(), 0.0));
        background(move |say| {
            let mut progress = |done: usize, total: usize| {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    anyhow::bail!("cancelled");
                }
                say(Message::TrackProgress(done as f32 / total.max(1) as f32));
                Ok(())
            };
            let result = (|| {
                use std::io::Write;
                let mut file = std::io::BufWriter::new(std::fs::File::create(&path).with_context(|| format!("writing {}", path.display()))?);
                let near = Some(crate::track::Near::Rig(dir));
                let (found, rows) = crate::track_to(&input, &colour, near, Some(t), Some(end - t), 0.1, &mut file, &mut progress)?;
                file.flush()?;
                Ok::<_, anyhow::Error>(TrackDone { path: path.clone(), cut: i, t, found, rows, colour: hex })
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            Message::Tracked(result.map_err(|e| format!("{e:#}")))
        })
    }

    /// Makes the keyframe where a finished track starts follow it, with the
    /// subject in the middle of the view.
    pub(super) fn follow_track(&mut self, done: TrackDone) {
        self.save_status = Some(format!("Tracked {}: found in {} of {} samples", done.colour, done.found, done.rows));
        // Gone from the edit meanwhile: the track's file stays, unused.
        let Some(at) = self.edit.cut_index(done.cut) else { return };
        self.followed.borrow_mut().remove(&done.path);
        let follow = Follow::Track(done.path);
        let here = at == self.current;
        let mut aims = if here { self.aims(done.t) } else { Vec::new() };
        let subject = if here { self.aim(&follow, done.t) } else { None };
        aims.push((follow.clone(), subject));
        let mut view = if here { self.shown_view() } else { View::home(Projection::Perspective) };
        if let Some((yaw, pitch)) = subject {
            (view.yaw, view.pitch) = (yaw, pitch);
        }
        use crate::model::ops::{Op, keys::KeyOp};
        let (cut, t, tolerance) = (done.cut, done.t, self.tolerance());
        // Keyed and made to follow: one undo step.
        self.ops(&[
            Op::Key(KeyOp::View { cut, t, view, tolerance, aims: aims.clone() }),
            Op::Key(KeyOp::Follow { cut, t, follow: Some(follow), tolerance, aims }),
        ]);
        self.follow = true;
    }

    /// The median colour inside `area` (fractions of the view, `aspect`
    /// wide over tall) of the frame on screen, and the rig direction of its
    /// middle. Sampled from the lens pictures as the shader maps them.
    pub(super) fn sample_area(&self, area: Rectangle, aspect: f64) -> Option<([f64; 3], [f64; 3])> {
        const N: usize = 16;
        let c = self.clip.as_ref()?;
        let frame = c.shown.as_ref()?.frame();
        let rig = &c.playback.media.rig;
        let camera = self.camera();
        let views = c.playback.rig_from_view(self.rolling_shutter);
        let rig_from_view = views[views.len() / 2];
        let dir = |fx: f64, fy: f64| crate::stitch::apply(rig_from_view, crate::stitch::apply(camera.user, camera.ray(aspect, [2.0 * fx - 1.0, 2.0 * fy - 1.0])));
        let mut channels: [Vec<f64>; 3] = Default::default();
        // An ordinary video or picture has no lenses: its frame's one
        // picture, cropped as the shader crops it. The eyedropper used to
        // find nothing there and silently do nothing (the user's Nikon clip).
        let flat = c.playback.media.flat;
        let picture = frame.pictures.first().map_or(1.0, |p| p.width as f64 / p.height as f64);
        for gy in 0..N {
            for gx in 0..N {
                let fx = area.x as f64 + (gx as f64 + 0.5) / N as f64 * area.width as f64;
                let fy = area.y as f64 + (gy as f64 + 0.5) / N as f64 * area.height as f64;
                if let Some(flat) = flat {
                    if let Some([u, v]) = camera.flat_uv(aspect, picture, [2.0 * fx - 1.0, 2.0 * fy - 1.0])
                        && let Some(rgb) = frame.flat_rgb_at(u, v, flat.hdr)
                    {
                        for k in 0..3 {
                            channels[k].push(rgb[k]);
                        }
                    }
                    continue;
                }
                let d = dir(fx, fy);
                // The lens that weighs most there, as the blend would.
                let best = rig.lenses.iter().filter_map(|l| l.project(d).map(|h| (l, h))).max_by(|a, b| (a.0.blend.weight(a.1.2) * a.1.3).total_cmp(&(b.0.blend.weight(b.1.2) * b.1.3)));
                if let Some((lens, (u, v, _, _))) = best
                    && let Some(rgb) = frame.rgb_at(lens, rig.tracks[lens.input], u, v)
                {
                    for k in 0..3 {
                        channels[k].push(rgb[k]);
                    }
                }
            }
        }
        if channels[0].is_empty() {
            return None;
        }
        let median = channels.map(|mut c| {
            c.sort_by(f64::total_cmp);
            c[c.len() / 2]
        });
        Some((median, dir(area.x as f64 + area.width as f64 / 2.0, area.y as f64 + area.height as f64 / 2.0)))
    }

    /// The aspect and the button that opens the Export panel, or a running
    /// export's or colour track's progress.
    pub(super) fn export_controls(&self) -> Element<'_, Message> {
        if let Some((_, done)) = &self.tracking {
            return row![
                text(format!("Tracking {:.0}%", done * 100.0)),
                progress_bar(0.0..=1.0, *done).length(140).girth(8),
                tip(icon_button(CANCEL, Some(Message::TrackCancel)), text("Stop tracking"), tooltip::Position::Bottom),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into();
        }
        match &self.exporting {
            Some((_, done)) => row![
                text(format!("Exporting {:.0}%", done * 100.0)),
                progress_bar(0.0..=1.0, *done).length(140).girth(8),
                tip(icon_button(CANCEL, Some(Message::ExportCancel)), text("Stop the export"), tooltip::Position::Bottom),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            None => row![
                tip(
                    labelled("Aspect", pick_list(&Aspect::ALL[..], Some(self.aspect), Message::Aspect)),
                    text("The shape of the view, of saved views and of the export (16:9 when free)"),
                    tooltip::Position::Bottom,
                ),
                tip(
                    super::tint_button(super::Tint::Global, EXPORT, (!self.edit.cuts.is_empty()).then_some(Message::ExportPanel)),
                    text(if self.side == Some(Side::Export) { "Close the Export panel (Ctrl+E)" } else { "Export: resolution, format and clips, then the edit with sound (Ctrl+E)" }),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into(),
        }
    }

    /// An export or colour track is running on its thread.
    pub(super) fn busy(&self) -> bool {
        self.exporting.is_some() || self.tracking.is_some()
    }

    /// Asks whether to stop the running export or tracking and quit:
    /// quitting under it left a half-written video, and the crossfade's
    /// temporary frames (GBs at 2160p) in the temp folder.
    pub(super) fn confirm_stop(&mut self) -> Task<Message> {
        let what = if self.exporting.is_some() { "An export" } else { "Colour tracking" };
        self.ask(super::ask::Ask {
            title: "Still working".to_owned(),
            text: format!("{what} is still running. Stop it and quit?"),
            answers: vec![("Stop and quit".to_owned(), Message::StopAndClose(true))],
            cancel: Message::StopAndClose(false),
        })
    }

    pub(super) fn cancel_work(&self) {
        for (cancel, _) in self.exporting.iter().chain(&self.tracking) {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Goes on quitting once the work stopped for it has finished
    /// (cleaning up after itself), asking about unsaved changes as usual.
    pub(super) fn close_when_stopped(&mut self) -> Task<Message> {
        if self.closing && !self.busy() {
            self.closing = false;
            return self.update(Message::CloseRequested);
        }
        Task::none()
    }
}

impl App {
    /// Long work: export, colour tracking, saving a photo: this area's messages, as `handle` passes them on.
    pub(super) fn on_jobs(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Export => {
                if self.edit.cuts.is_empty() || self.exporting.is_some() {
                    return Task::none();
                }
                let dir = self.export_base().parent().map(Path::to_owned);
                let (clips, ext) = (self.export_clips, self.export_format.extension());
                let name = format!("{}_{}.{ext}", self.export_name(), self.resolution);
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new();
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        if clips {
                            dialog.set_title("Export the cuts as clips, into the folder").pick_folder().await.map(|f| f.path().to_owned())
                        } else {
                            let kind = if ext == "mov" { "QuickTime video" } else { "MP4 video" };
                            dialog.set_title("Export video").add_filter(kind, &[ext]).set_file_name(name).save_file().await.map(|f| f.path().to_owned())
                        }
                    },
                    Message::ExportTo,
                );
            }
            Message::ExportTo(None) => {}
            Message::ExportTo(Some(path)) => return self.start_export(path),
            Message::ExportProgress(f) => {
                if let Some((_, done)) = &mut self.exporting {
                    *done = f;
                }
            }
            Message::ExportCancel => {
                if let Some((cancel, _)) = &self.exporting {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::PickColour if self.on_flat() => {}
            Message::PickColour => {
                self.picking_white = false;
                if self.picking.take().is_none() && self.clip.is_some() {
                    self.with_playback(Playback::pause);
                    self.picking = Some(None);
                    self.save_status = Some("Draw a box over the subject to track its colour (Esc cancels)".to_owned());
                } else {
                    self.save_status = None;
                }
            }
            Message::PickArea(area) => {
                if self.picking.is_some() {
                    self.picking = Some(Some(area));
                }
            }
            Message::AreaPicked(area, aspect) => {
                self.picking = None;
                self.save_status = None;
                if area.width < 0.004 || area.height < 0.004 {
                    return Task::none();
                }
                if std::mem::take(&mut self.picking_white) {
                    if let Some((rgb, _)) = self.sample_area(area, aspect) {
                        self.white_balance(rgb);
                    }
                } else if let Some((rgb, dir)) = self.sample_area(area, aspect) {
                    match crate::track::Colour::from_rgb(rgb, 18.0) {
                        Ok(colour) => return self.start_track(colour, crate::track::hex(rgb), dir),
                        Err(e) => self.error = Some(format!("{e:#}")),
                    }
                }
            }
            Message::TrackProgress(f) => {
                if let Some((_, done)) = &mut self.tracking {
                    *done = f;
                }
            }
            Message::TrackCancel => {
                if let Some((cancel, _)) = &self.tracking {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::Tracked(result) => {
                let cancelled = self.tracking.take().is_some_and(|(c, _)| c.load(std::sync::atomic::Ordering::Relaxed));
                match result {
                    Ok(done) => self.follow_track(done),
                    Err(_) if cancelled => self.save_status = Some("Tracking cancelled".to_owned()),
                    Err(e) => self.error = Some(format!("Tracking failed: {e}")),
                }
                return self.close_when_stopped();
            }
            Message::ExportDone(result) => {
                let cancelled = self.exporting.take().is_some_and(|(c, _)| c.load(std::sync::atomic::Ordering::Relaxed));
                match result {
                    Ok(paths) => {
                        let name = |p: &PathBuf| p.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        self.save_status = Some(match paths.as_slice() {
                            [one] => format!("Exported {}", name(one)),
                            [first, ..] => format!("Exported {} clips to {}", paths.len(), first.parent().map_or_else(String::new, |d| d.display().to_string())),
                            [] => "Exported nothing".to_owned(),
                        });
                    }
                    Err(_) if cancelled => self.save_status = Some("Export cancelled".to_owned()),
                    Err(e) => self.error = Some(format!("Export failed: {e}")),
                }
                return self.close_when_stopped();
            }
            Message::Save(what) => {
                let Some(c) = &self.clip else { return Task::none() };
                let media = &c.playback.media;
                let stem = media.input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let name = format!("{stem}_{:.2}s_{}.png", c.playback.shown_pts, if what == Saving::View { "view" } else { "360" });
                let dir = media.input.parent().map(Path::to_owned);
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title("Save photo").add_filter("PNG image", &["png"]).set_file_name(name);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.save_file().await.map(|f| f.path().to_owned())
                    },
                    move |path| Message::SaveTo(what, path),
                );
            }
            Message::SaveTo(_, None) => {}
            Message::SaveTo(what, Some(path)) => {
                let Some(c) = &self.clip else { return Task::none() };
                let media = &c.playback.media;
                let shot = match what {
                    Saving::Sphere => export::Shot::Sphere { width: SAVE_SPHERE_PX },
                    Saving::View => {
                        let k = SAVE_VIEW_PX / self.view_size.width.max(self.view_size.height).max(1.0);
                        // Even sizes keep image tools that halve chroma happy.
                        let px = |v: f32| ((v * k / 2.0).round() as usize).max(1) * 2;
                        export::Shot::View {
                            camera: Box::new(self.camera()),
                            size: (px(self.view_size.width), px(self.view_size.height)),
                        }
                    }
                };
                let request = export::Request {
                    input: media.input.clone(),
                    flat: media.flat,
                    hw: media.hw,
                    rig: media.rig.clone(),
                    stab: media.stab.clone(),
                    readout_ms: media.readout_ms,
                    t: c.playback.shown_pts,
                    rolling_shutter: self.rolling_shutter,
                    shot,
                    path,
                };
                self.save_status = Some("Saving photo…".to_owned());
                return Task::perform(blocking(move || export::save(request).map_err(|e| format!("{e:#}"))), Message::Saved);
            }
            Message::Saved(result) => match result {
                Ok(_) => self.save_status = None,
                Err(e) => {
                    self.save_status = None;
                    self.error = Some(format!("Saving failed: {e}"));
                }
            },
            _ => {}
        }
        Task::none()
    }
}

/// Runs `work` on a thread of its own, what it says (`say`: progress) and
/// the message it ends with relayed to the window as they come: every long
/// job's (the export, colour tracking).
fn background(work: impl FnOnce(&dyn Fn(Message)) -> Message + Send + 'static) -> Task<Message> {
    use iced::futures::{SinkExt, StreamExt};
    let relay = iced::stream::channel(8, async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            let say = |m: Message| {
                let _ = tx.unbounded_send(m);
            };
            let last = work(&say);
            say(last);
        });
        while let Some(m) = rx.next().await {
            if output.send(m).await.is_err() {
                break;
            }
        }
    });
    Task::run(relay, |m| m)
}
