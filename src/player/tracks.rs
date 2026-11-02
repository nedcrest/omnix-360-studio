//! Sound on the timeline: waveforms (sound clips' and what each cut plays),
//! adding a sound file to a track, selecting volume points, and how a cut's
//! level is tagged on the Clip sound lane.

use super::*;

/// The most a level tag turns a clip up, dB: as far as normalizing goes.
const LEVEL_MOST: f64 = crate::loudness::MOST_GAIN;

/// A cut's or a sound clip's own level as its lane tags it ("0 dB",
/// "+1.4 · even", "muted"): always shown, as it is the knob a drag up or
/// down turns (gap 44).
pub(super) fn level_tag(gain: f64, mute: bool, filters: [(bool, &str); 3]) -> String {
    let level = if mute { "muted".to_owned() } else if gain == 0.0 { "0 dB".to_owned() } else { format!("{gain:+.1}") };
    std::iter::once(level).chain(filters.into_iter().filter(|f| f.0).map(|f| f.1.to_owned())).collect::<Vec<_>>().join(" · ")
}

/// What a level tag says of a sound's filters, in the order they run.
pub(super) fn filter_tags(low_cut: bool, clean: bool, even: bool) -> [(bool, &'static str); 3] {
    [(low_cut, "low cut"), (clean, "clean"), (even, "even")]
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
    /// Reads `path`'s waveform for its lane, unless it has it.
    pub(super) fn load_waveform(&self, path: &Path) -> Task<Message> {
        if self.waveforms.contains_key(path) {
            return Task::none();
        }
        let path = path.to_owned();
        Task::perform(
            blocking({
                let path = path.clone();
                move || music::Waveform::read(&path, false).map(Arc::new).map_err(|e| format!("{e:#}"))
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
        // The plain waveform of each, and the evened one of those evened;
        // a sound clip's plain one comes when it's added (`load_waveform`).
        let wants = self.edit.cuts.iter().flat_map(|c| {
            let file = cut_wave_file(c);
            std::iter::once((file.clone(), false)).chain(c.even.then_some((file, true)))
        });
        let wants = wants.chain(self.edit.sound_clips().filter(|c| c.even).map(|c| (c.input.clone(), true)));
        let mut wanted: Vec<(PathBuf, bool)> = wants
            .filter(|(p, even)| !(if *even { &self.even_waves } else { &self.waveforms }).contains_key(p))
            .filter(|w| !self.cut_waves_asked.contains(w) && seen.insert(w.clone()))
            .collect();
        wanted.truncate(AT_ONCE.saturating_sub(self.cut_waves_reading));
        self.cut_waves_reading += wanted.len();
        Task::batch(wanted.into_iter().map(|(path, even)| {
            self.cut_waves_asked.insert((path.clone(), even));
            Task::perform(
                blocking({
                    let path = path.clone();
                    move || music::Waveform::read(&path, even).map(Arc::new).ok()
                }),
                move |wave| Message::CutWave(path.clone(), even, wave),
            )
        }))
    }

    /// Selects point `i` of a volume line, and nothing else.
    pub(super) fn select_point(&mut self, own: Own, i: usize) {
        self.selected_point = Some((own, i));
        self.selected_music = None;
        self.selected = None;
    }
}

impl App {
    /// Sound tracks and volume points: this area's messages, as `handle` passes them on.
    pub(super) fn on_tracks(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddSound(which) => {
                // The track's own folder in the project (made if missing:
                // the project's tree is footage-agent's layout), else the
                // project's folder.
                let dir = self.project.as_deref().and_then(bin::root).map(|root| {
                    let dir = root.join(which.folder());
                    if std::fs::create_dir_all(&dir).is_ok() { dir } else { root }
                });
                let (title, filter) = match which {
                    SoundTrack::Music => ("Add music", "Music"),
                    SoundTrack::Sfx | SoundTrack::Sfx2 => ("Add a sound effect", "Sounds"),
                    SoundTrack::Voice => ("Add a voice-over", "Sounds"),
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
                // effect or a voice-over at the playhead, where it's wanted.
                let at = match which {
                    SoundTrack::Music => None,
                    SoundTrack::Sfx | SoundTrack::Sfx2 | SoundTrack::Voice => Some(self.position()),
                };
                return probe_sound(which, path, at);
            }
            Message::SoundProbed(_, path, _, Err(e)) => self.error = Some(format!("Can't add {}: {e}", path.display())),
            Message::SoundProbed(which, path, at, Ok(len)) => {
                self.edit.checkpoint();
                let at = at.unwrap_or_else(|| self.edit.track(which).end());
                // A sound effect where one already plays goes on the second
                // lane, so the two sound together (gap 46e).
                let busy = |t: SoundTrack| self.edit.track(t).clips.iter().any(|c| at < c.ends() - 1e-6 && at + len > c.at + 1e-6);
                let which = if which == SoundTrack::Sfx && busy(SoundTrack::Sfx) && !busy(SoundTrack::Sfx2) { SoundTrack::Sfx2 } else { which };
                if which == SoundTrack::Sfx2 {
                    self.folds.sfx2 = false;
                }
                let i = self.edit.track_mut(which).append(path.clone(), at, len);
                self.selected_music = Some((which, i));
                self.selected = None;
                return self.load_waveform(&path);
            }
            Message::Waveform(path, Ok(wave)) => {
                self.waveforms.insert(path, wave);
            }
            Message::Waveform(path, Err(e)) => self.error = Some(format!("Can't read {}: {e}", path.display())),
            Message::CutWave(path, even, wave) => {
                self.cut_waves_reading = self.cut_waves_reading.saturating_sub(1);
                if let Some(wave) = wave {
                    (if even { &mut self.even_waves } else { &mut self.waveforms }).insert(path, wave);
                }
            }
            Message::SelectMusic(which, i) => {
                self.selected_music = Some((which, i));
                self.selected_overlay = None;
                self.selected = None;
                self.selected_point = None;
            }
            Message::PointAdd(own, t, db) => {
                if self.edit.own_line(own).is_none() {
                    return Task::none();
                }
                self.begin_drag();
                let Some((keys, (start, end))) = self.edit.own_line_mut(own) else { return Task::none() };
                let i = crate::project::add_volume_key(keys, t.clamp(start, end), db);
                self.select_point(own, i);
            }
            Message::PointSelect(own, i) => self.select_point(own, i),
            // A point gone mid-drag (an undo) is no undo step, nor an index panic.
            Message::PointMove(own, i, ..) if self.edit.own_line(own).is_none_or(|(keys, _)| i >= keys.len()) => {}
            Message::PointMove(own, i, t, db) => {
                self.begin_drag();
                // Within its stretch of its file and between its
                // neighbours: points keep their order.
                if let Some((keys, range)) = self.edit.own_line_mut(own) {
                    crate::project::move_volume_key(keys, i, t, db, range);
                }
            }
            // A stale index (the clip gone mid-drag) is no undo step.
            Message::MusicMove(which, i, _, _) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicTrim(which, i, ..) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicMove(which, i, want, overlap) => {
                self.begin_drag();
                self.edit.track_mut(which).drag_to(i, want, overlap);
            }
            Message::MusicTrim(which, i, in_edge, t, overlap) => {
                self.begin_drag();
                // The file's length comes with its waveform.
                let song = self.waveforms.get(&self.edit.track(which).clips[i].input).map_or(f64::INFINITY, |w| w.duration);
                self.edit.track_mut(which).trim(i, in_edge, t, overlap, song, edit::MIN_CUT);
            }
            Message::Listen(lane, solo) => {
                let l = &mut self.listen;
                let flag = if solo { &mut l.solo[lane] } else { &mut l.muted[lane] };
                *flag = !*flag;
            }
            Message::Level(own, db) => {
                if self.edit.own_line(own).is_some() {
                    self.begin_drag();
                    if let Some(s) = self.edit.own_sound(own) {
                        *s.gain = db.clamp(crate::project::OFF_DB, LEVEL_MOST);
                    }
                }
            }
            Message::Fade(own, in_edge, seconds) => {
                if self.edit.own_line(own).is_some() {
                    self.begin_drag();
                    if let Some(mut s) = self.edit.own_sound(own) {
                        s.set_fade(in_edge, seconds);
                    }
                }
            }
            Message::DuckDepth(db) => self.duck_db = db,
            Message::Duck if !self.ducking => {
                self.sound_menu = None;
                self.ducking = true;
                // Each voice's waveform (cached, or read once), off the UI thread.
                let project = (*self.edit).clone();
                return Task::perform(blocking(move || crate::duck::find_speech(&project).map_err(|e| format!("{e:#}"))), Message::Ducked);
            }
            Message::Duck => {}
            Message::Ducked(result) => {
                self.ducking = false;
                match result {
                    Err(e) => self.error = Some(format!("Can't find the speech: {e}")),
                    Ok(spans) if spans.is_empty() => self.error = Some("No speech to lower the music under: it's found in cleaned voices and on the Voice lane.".into()),
                    Ok(spans) => {
                        // One undo step; the line stays editable point by point.
                        self.edit.checkpoint();
                        crate::duck::duck_music(&mut self.edit.music, &spans, self.duck_db);
                        self.selected_point = None;
                    }
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
