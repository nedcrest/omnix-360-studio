//! Long work on threads, with progress and Cancel in the top bar: the
//! export, colour tracking and the box drawn to pick a colour; and asking
//! before closing while one runs.

use super::*;

/// A finished colour track: its CSV, where it starts (cut and clip time),
/// how many samples found the colour, and the colour.
#[derive(Debug, Clone)]
pub struct TrackDone {
    pub(super) path: PathBuf,
    pub(super) cut: usize,
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
        use iced::futures::{SinkExt, StreamExt};
        let project = self.to_project();
        let [w, h] = project.aspect.unwrap_or(crate::view::Aspect(16, 9)).size(self.resolution.short_side());
        let target = if self.export_clips {
            crate::Target::Clips { dir: path, name: self.export_name(), handles: 0.0, graded: self.export_graded }
        } else {
            crate::Target::One(path.with_extension(self.export_format.extension()))
        };
        let out = crate::ExportOpts { size: (w as usize, h as usize), draft: false, cpu: false, format: self.export_format, target };
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
        let work = iced::stream::channel(8, async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
            let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
            std::thread::spawn(move || {
                let mut progress = |done: usize, total: usize| {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        anyhow::bail!("cancelled");
                    }
                    if done.is_multiple_of(5) || done == total {
                        let _ = tx.unbounded_send(Message::ExportProgress(done as f32 / total.max(1) as f32));
                    }
                    Ok(())
                };
                let result = crate::export(&project, &opts, &out, &mut progress);
                let _ = tx.unbounded_send(Message::ExportDone(result.map_err(|e| format!("{e:#}"))));
            });
            while let Some(m) = rx.next().await {
                if output.send(m).await.is_err() {
                    break;
                }
            }
        });
        Task::run(work, |m| m)
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
        use iced::futures::{SinkExt, StreamExt};
        let Some((cut, t)) = self.playing_cut() else { return Task::none() };
        let (input, end, i) = (cut.input.clone(), cut.end, self.current);
        let folder = self.project.as_deref().unwrap_or(&input).parent().unwrap_or(Path::new("")).to_owned();
        let stem = input.file_stem().map_or_else(|| "clip".to_owned(), |s| s.to_string_lossy().into_owned());
        let path = folder.join(format!("{stem}-colour-{t:.1}s.csv"));
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.tracking = Some((cancel.clone(), 0.0));
        let work = iced::stream::channel(8, async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
            let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
            std::thread::spawn(move || {
                let mut progress = |done: usize, total: usize| {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        anyhow::bail!("cancelled");
                    }
                    let _ = tx.unbounded_send(Message::TrackProgress(done as f32 / total.max(1) as f32));
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
                let _ = tx.unbounded_send(Message::Tracked(result.map_err(|e| format!("{e:#}"))));
            });
            while let Some(m) = rx.next().await {
                if output.send(m).await.is_err() {
                    break;
                }
            }
        });
        Task::run(work, |m| m)
    }

    /// Makes the keyframe where a finished track starts follow it, with the
    /// subject in the middle of the view.
    pub(super) fn follow_track(&mut self, done: TrackDone) {
        self.save_status = Some(format!("Tracked {}: found in {} of {} samples", done.colour, done.found, done.rows));
        if done.cut >= self.edit.cuts.len() {
            return;
        }
        self.followed.borrow_mut().remove(&done.path);
        let follow = Follow::Track(done.path);
        let here = done.cut == self.current;
        let mut aims = if here { self.aims(done.t) } else { Vec::new() };
        let subject = if here { self.aim(&follow, done.t) } else { None };
        aims.push((follow.clone(), subject));
        let mut view = if here { self.shown_view() } else { View::home(Projection::Perspective) };
        if let Some((yaw, pitch)) = subject {
            (view.yaw, view.pitch) = (yaw, pitch);
        }
        let aim = |f: &Follow, _| aims.iter().find(|a| &a.0 == f).and_then(|a| a.1);
        let tolerance = self.tolerance();
        self.edit.checkpoint();
        let keyframes = &mut self.edit.cuts[done.cut].keyframes;
        let k = keyframes.set(done.t, view, tolerance, &aim);
        keyframes.set_follow(k, Some(follow), &aim);
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

    /// Export button, resolution and options, or a running export's progress.
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
                    icon_button(EXPORT, (!self.edit.cuts.is_empty()).then_some(Message::ExportPanel)),
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
    pub(super) fn confirm_stop(&self) -> Task<Message> {
        let what = if self.exporting.is_some() { "An export" } else { "Colour tracking" };
        let ask = rfd::AsyncMessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Still working")
            .set_description(format!("{what} is still running. Stop it and quit?"))
            .set_buttons(rfd::MessageButtons::OkCancelCustom("Stop and quit".into(), "Keep working".into()));
        Task::perform(ask.show(), |answer| {
            Message::StopAndClose(matches!(answer, rfd::MessageDialogResult::Ok) || matches!(&answer, rfd::MessageDialogResult::Custom(l) if l == "Stop and quit"))
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
