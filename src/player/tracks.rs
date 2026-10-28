//! Sound on the timeline: waveforms (sound clips' and what each cut plays),
//! adding a sound file to a track, selecting volume points, and how a cut's
//! level is tagged on the Clip sound lane.

use super::*;

/// A cut's own sound level as the clip sound lane tags it: "muted", its
/// gain, "even" when evened out; none at the clip's own level.
pub(super) fn cut_level_tag(cut: &Cut) -> Option<String> {
    let mut parts = Vec::new();
    if cut.mute {
        parts.push("muted".to_owned());
    } else if cut.gain != 0.0 {
        parts.push(format!("{:+.1}", cut.gain));
    }
    if cut.even {
        parts.push("even".to_owned());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The file to draw cut `cut`'s waveform from (`Cut::sound_file`).
pub(super) fn cut_wave_file(cut: &Cut) -> PathBuf {
    cut.sound_file()
}

/// Runs `f` on its own thread so blocking work (parsing, ffprobe) stays off
/// the executor.
/// Reads a sound file's length on a thread (ffprobe can take seconds on a
/// network or spinning drive, and froze the window), then adds it to track
/// `which` at `at` (`SoundProbed`). A file that can't be read is said so,
/// not added as a minute of nothing.
pub(super) fn probe_sound(which: SoundTrack, path: PathBuf, at: Option<f64>) -> Task<Message> {
    Task::perform(
        blocking({
            let path = path.clone();
            move || ffmpeg::duration(&path).map_err(|e| format!("{e:#}"))
        }),
        move |len| Message::SoundProbed(which, path.clone(), at, len),
    )
}

impl App {
    pub(super) fn load_waveform(&self, path: &Path) -> Task<Message> {
        if self.waveforms.contains_key(path) {
            return Task::none();
        }
        let path = path.to_owned();
        Task::perform(
            blocking({
                let path = path.clone();
                move || music::Waveform::read(&path).map(Arc::new).map_err(|e| format!("{e:#}"))
            }),
            move |r| Message::Waveform(path.clone(), r),
        )
    }

    /// Reads the waveforms of what the cuts play, each file once, two at a
    /// time (a project of 30 clips on a spinning drive would otherwise read
    /// them all at once): a cut's clip sound lane shows it once read. The
    /// first read of a long clip takes a few seconds (3 s for an 8-minute
    /// 3.8 GB one), later opens come from the cache. A file without sound
    /// gives none, quietly: a picture's cut is simply silent.
    pub(super) fn load_cut_waves(&mut self) -> Task<Message> {
        const AT_ONCE: usize = 2;
        // Each file once, however many cuts use it.
        let mut seen = std::collections::HashSet::new();
        let mut wanted: Vec<PathBuf> = self.edit.cuts.iter().map(cut_wave_file).filter(|p| !self.waveforms.contains_key(p) && !self.cut_waves_asked.contains(p) && seen.insert(p.clone())).collect();
        wanted.truncate(AT_ONCE.saturating_sub(self.cut_waves_reading));
        self.cut_waves_reading += wanted.len();
        Task::batch(wanted.into_iter().map(|path| {
            self.cut_waves_asked.insert(path.clone());
            Task::perform(
                blocking({
                    let path = path.clone();
                    move || music::Waveform::read(&path).map(Arc::new).ok()
                }),
                move |wave| Message::CutWave(path.clone(), wave),
            )
        }))
    }

    /// Saves the edit for undo once per drag.
    /// Selects a sound track's volume point `i`, and nothing else.
    pub(super) fn select_volume(&mut self, which: SoundTrack, i: usize) {
        self.selected_volume = Some((which, i));
        self.selected_music = None;
        self.selected = None;
        self.selected_sound = None;
    }

    /// Selects cut `cut`'s sound point `i`, and nothing else.
    pub(super) fn select_sound(&mut self, cut: usize, i: usize) {
        self.selected_sound = Some((cut, i));
        self.selected_volume = None;
        self.selected_music = None;
        self.selected = None;
    }
}

impl App {
    /// Sound tracks and volume points: this area's messages, as `handle` passes them on.
    pub(super) fn on_tracks(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddSound(which) => {
                let dir = self.project.as_ref().and_then(|p| p.parent().map(Path::to_owned));
                let (title, filter) = match which {
                    SoundTrack::Music => ("Add music", "Music"),
                    SoundTrack::Sfx => ("Add a sound effect", "Sounds"),
                };
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new().set_title(title).add_filter(filter, &["mp3", "MP3", "flac", "wav", "WAV", "m4a", "aac", "ogg", "opus", "wma"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_file().await.map(|f| f.path().to_owned())
                    },
                    move |picked| Message::SoundPicked(which, picked),
                );
            }
            Message::SoundPicked(_, None) => {}
            Message::SoundPicked(which, Some(path)) => {
                // A song after the music already there, whole; a sound
                // effect at the playhead, where it's wanted.
                let at = match which {
                    SoundTrack::Music => None,
                    SoundTrack::Sfx => Some(self.position()),
                };
                return probe_sound(which, path, at);
            }
            Message::SoundProbed(_, path, _, Err(e)) => self.error = Some(format!("Can't add {}: {e}", path.display())),
            Message::SoundProbed(which, path, at, Ok(len)) => {
                self.edit.checkpoint();
                let at = at.unwrap_or_else(|| self.edit.track(which).end());
                let i = self.edit.track_mut(which).append(path.clone(), at, len);
                self.selected_music = Some((which, i));
                self.selected = None;
                return self.load_waveform(&path);
            }
            Message::Waveform(path, Ok(wave)) => {
                self.waveforms.insert(path, wave);
            }
            Message::Waveform(path, Err(e)) => self.error = Some(format!("Can't read {}: {e}", path.display())),
            Message::CutWave(path, wave) => {
                self.cut_waves_reading = self.cut_waves_reading.saturating_sub(1);
                if let Some(wave) = wave {
                    self.waveforms.insert(path, wave);
                }
            }
            Message::SelectMusic(which, i) => {
                self.selected_music = Some((which, i));
                self.selected_overlay = None;
                self.selected = None;
                self.selected_volume = None;
                self.selected_sound = None;
            }
            Message::VolumeAdd(which, t, db) => {
                self.begin_drag();
                let keys = &mut self.edit.track_mut(which).keyframes;
                let i = keys.partition_point(|k| k.t <= t);
                keys.insert(i, crate::project::VolumeKey { t, volume: db });
                self.select_volume(which, i);
            }
            Message::VolumeSelect(which, i) => self.select_volume(which, i),
            Message::SoundKeyAdd(cut, t, db) => {
                let Some(c) = self.edit.cuts.get(cut) else { return Task::none() };
                let t = t.clamp(c.start, c.end);
                self.begin_drag();
                let keys = &mut self.edit.cuts[cut].volume;
                let i = keys.partition_point(|k| k.t <= t);
                keys.insert(i, crate::project::VolumeKey { t, volume: db });
                self.select_sound(cut, i);
            }
            Message::SoundKeySelect(cut, i) => self.select_sound(cut, i),
            // A point gone mid-drag (an undo) is no undo step, nor an index panic.
            Message::VolumeMove(which, i, ..) if i >= self.edit.track(which).keyframes.len() => {}
            Message::SoundKeyMove(cut, i, ..) if self.edit.cuts.get(cut).is_none_or(|c| i >= c.volume.len()) => {}
            Message::SoundKeyMove(cut, i, t, db) => {
                let Some(c) = self.edit.cuts.get(cut) else { return Task::none() };
                let (start, end) = (c.start, c.end);
                self.begin_drag();
                let keys = &mut self.edit.cuts[cut].volume;
                // Within the cut and between its neighbours.
                let lo = i.checked_sub(1).map_or(start, |j| keys[j].t + 0.01).max(start);
                let hi = keys.get(i + 1).map_or(end, |k| k.t - 0.01).min(end);
                if let Some(k) = keys.get_mut(i) {
                    *k = crate::project::VolumeKey { t: t.clamp(lo, hi.max(lo)), volume: db };
                }
            }
            Message::VolumeMove(which, i, t, db) => {
                self.begin_drag();
                let keys = &mut self.edit.track_mut(which).keyframes;
                // Between its neighbours: points keep their order.
                let lo = i.checked_sub(1).map_or(0.0, |j| keys[j].t + 0.01);
                let hi = keys.get(i + 1).map_or(f64::INFINITY, |k| k.t - 0.01);
                if let Some(k) = keys.get_mut(i) {
                    *k = crate::project::VolumeKey { t: t.clamp(lo, hi.max(lo)), volume: db };
                }
            }
            // A stale index (the clip gone mid-drag) is no undo step.
            Message::MusicMove(which, i, _, _) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicTrim(which, i, ..) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicMove(which, i, want, overlap) => {
                self.begin_drag();
                if i < self.edit.track(which).clips.len() {
                    let m = self.edit.track(which);
                    let c = &m.clips[i];
                    let len = c.end - c.start;
                    // A plain drag never overlaps: it stops edge to edge, or
                    // hops past. With Shift it slides in as far as it may.
                    let at = if overlap || m.crossfades(i) != (0.0, 0.0) {
                        crate::project::Music::reach(c.at, want.max(0.0), |at| m.fits(i, at, len, overlap))
                    } else {
                        m.place(i, want)
                    };
                    self.edit.track_mut(which).clips[i].at = at;
                }
            }
            Message::MusicTrim(which, i, in_edge, t, overlap) => {
                self.begin_drag();
                let song = self.waveforms.get(&self.edit.track(which).clips[i].input).map_or(f64::INFINITY, |w| w.duration);
                let m = self.edit.track(which);
                let c = &m.clips[i];
                // Song time to edit time on this clip: the song stays put on
                // the edit, so trimming its start moves where the clip
                // begins, not what plays when. Within the song and the edit,
                // and into a neighbour only with Shift.
                let to_edit = c.at - c.start;
                let (start, end) = (c.start, c.end);
                if in_edge {
                    let want = t.max(start - c.at).max(0.0).min(end - edit::MIN_CUT);
                    let start = crate::project::Music::reach(start, want, |s| m.fits(i, s + to_edit, end - s, overlap));
                    let c = &mut self.edit.track_mut(which).clips[i];
                    c.at = start + to_edit;
                    c.start = start;
                } else {
                    let want = t.min(song).max(start + edit::MIN_CUT);
                    let end = crate::project::Music::reach(end, want, |e| m.fits(i, c.at, e - start, overlap));
                    self.edit.track_mut(which).clips[i].end = end;
                }
            }
            Message::SoundLevel(db) => {
                self.begin_drag();
                self.edit.sound.volume = db;
                let v = self.clip_volume();
                self.with_playback(|p| p.set_volume(v));
            }
            Message::TrackLevel(which, db) => {
                self.begin_drag();
                self.edit.track_mut(which).volume = db;
            }
            Message::LevelDone => self.dragging = false,
            _ => {}
        }
        Task::none()
    }
}
