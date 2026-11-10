//! Sound on the timeline: waveforms (sound clips' and what each cut plays),
//! adding a sound file to a track, selecting volume points, and how a cut's
//! level is tagged on the Clip sound lane.

use super::*;
use crate::model::ops::clips::ClipOp;
use crate::model::ops::{End, Op};
use crate::model::ops::sound::SoundOp;

/// The most a level tag turns a clip up, dB: as far as normalizing goes.
const LEVEL_MOST: f64 = crate::model::level::MOST_GAIN;

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
                move || crate::waveform::Waveform::read(&path, false).map(Arc::new).map_err(|e| format!("{e:#}"))
            }),
            move |r| Message::Waveform(path.clone(), r),
        )
    }

    /// How far music clip `i` would move for its nearest shown beat to
    /// land on the playhead (`beats::pull_to`): none without one in reach.
    pub(super) fn beat_pull(&self, i: usize) -> Option<f64> {
        let c = self.edit.music.clips.get(i)?;
        let shown = c.beats?;
        let hits = self.hits.get(&c.input)?.as_ref()?;
        crate::beats::pull_to(hits, shown.threshold, c.local(self.position()))
    }

    /// Reads the beats of every song whose clip shows them (gap 13), each
    /// file once, off the UI thread.
    pub(super) fn read_beats(&mut self) -> Task<Message> {
        let mut wanted: Vec<PathBuf> = self.edit.sound_clips().filter(|c| c.beats.is_some()).map(|c| c.input.clone()).filter(|p| !self.hits.contains_key(p)).collect();
        wanted.dedup();
        Task::batch(wanted.into_iter().map(|path| {
            self.hits.insert(path.clone(), None);
            Task::perform(blocking({
                let path = path.clone();
                move || crate::beats::read(&path).map(Arc::new).map_err(|e| format!("{e:#}"))
            }), move |r| Message::Beats(path.clone(), r))
        }).collect::<Vec<_>>())
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
            let file = c.sound_file();
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
                    move || crate::waveform::Waveform::read(&path, even).map(Arc::new).ok()
                }),
                move |wave| Message::CutWave(path.clone(), even, wave),
            )
        }))
    }

    /// Selects the point at `t` on `own`'s line, if there is one.
    pub(super) fn select_point_at(&mut self, own: Own, t: f64) {
        if let Some(i) = self.edit.own_line(own).and_then(|(keys, _)| keys.iter().position(|k| (k.t - t).abs() < crate::project::SAME_POINT)) {
            self.select_point(own, i);
        }
    }

    /// Selects point `i` of a volume line, and nothing else.
    pub(super) fn select_point(&mut self, own: Own, i: usize) {
        self.selection = Selection::point_at(&self.edit, own, i);
    }
}

impl App {
    /// Sound tracks and volume points: this area's messages, as `handle` passes them on.
    pub(super) fn on_tracks(&mut self, message: Message) -> Task<Message> {
        match message {
            m @ (Message::AddSound(_) | Message::SoundPicked(..) | Message::SoundProbed(..) | Message::Waveform(..) | Message::Beats(..) | Message::CutWave(..)) => return self.on_sound_files(m),
            Message::SelectMusic(which, i) => {
                self.pick(Selection::clip_at(&self.edit, which, i));
            }
            m @ (Message::PointAdd(..) | Message::PointSelect(..) | Message::PointMove(..)) => self.on_points(m),
            // A stale index (the clip gone mid-drag) is no undo step.
            Message::MusicMove(which, i, _, _) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicTrim(which, i, ..) if i >= self.edit.track(which).clips.len() => {}
            Message::MusicMove(which, i, want, overlap) => {
                self.edit.begin();
                let (clip, was) = (self.edit.track(which).clips[i].id, self.edit.track(which).clips[i].at);
                self.op(Op::Clip(ClipOp::Move { clip, at: want.min(was + self.also_room()), overlap }));
                let now = self.edit.clip_index(clip).map_or(was, |(w, i)| self.edit.track(w).clips[i].at);
                self.move_also(now - was);
            }
            Message::MusicTrim(which, i, in_edge, t, overlap) => {
                self.edit.begin();
                // The file's length comes with its waveform.
                let c = &self.edit.track(which).clips[i];
                let (clip, most) = (c.id, self.waveforms.get(&c.input).map_or(f64::INFINITY, |w| w.duration));
                self.op(Op::Clip(ClipOp::Trim { clip, end: if in_edge { End::In } else { End::Out }, t, most, overlap }));
            }
            Message::Listen(lane, solo) => self.listen.press(lane, solo),
            // Drags: one undo step each, from press to release.
            Message::Level(own, db) => {
                self.edit.begin();
                let db = db.clamp(crate::project::OFF_DB, LEVEL_MOST);
                self.sound_op(own, |sound| SoundOp::Level { sound, db });
            }
            Message::Fade(own, in_edge, seconds) => {
                self.edit.begin();
                let end = if in_edge { End::In } else { End::Out };
                self.sound_op(own, |sound| SoundOp::Fade { sound, end, seconds });
            }
            Message::DuckDepth(db) => self.duck_db = db,
            Message::Duck if !self.ducking => return self.find_speech(),
            // The edit changed while the speech was found (a take moved):
            // found again, so the music isn't lowered where speech was.
            Message::Ducked(version, _) if version != self.edit.version => {
                self.ducking = false;
                return self.on_tracks(Message::Duck);
            }
            Message::Duck => {}
            Message::Ducked(_, result) => self.ducked(result),
            Message::SoundLevel(db) => {
                self.edit.begin();
                self.op(Op::Sound(SoundOp::TrackLevel { track: None, db }));
                let v = self.clip_volume();
                self.with_playback(|p| p.set_volume(v));
            }
            Message::TrackLevel(which, db) => {
                self.edit.begin();
                self.op(Op::Sound(SoundOp::TrackLevel { track: Some(which), db }));
            }
            Message::LevelDone => self.edit.commit(),
            Message::TrackSync(track, on) => {
                self.op(Op::Sound(SoundOp::SyncLock { track, on }));
            }
            Message::PullBeat(i) => {
                self.sound_menu = None;
                if let Some(shift) = self.beat_pull(i) {
                    let clip = self.edit.music.clips[i].id;
                    let at = self.edit.music.clips[i].at + shift;
                    self.op(Op::Clip(ClipOp::Move { clip, at, overlap: false }));
                    self.move_also(shift);
                }
            }
            Message::ClipBeats(clip, beats, press) => {
                self.edit.begin();
                self.op(Op::Sound(SoundOp::Beats { clip, beats }));
                if press {
                    self.edit.commit();
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// A volume point added, selected or dragged on a sound's own line.
    fn on_points(&mut self, message: Message) {
        match message {
            Message::PointAdd(own, t, db) => {
                let Some((_, (start, end))) = self.edit.own_line(own) else { return };
                // The drag that follows moves it: one undo step for both.
                self.edit.begin();
                let t = t.clamp(start, end);
                self.sound_op(own, |sound| SoundOp::AddPoint { sound, t, db });
                self.select_point_at(own, t);
            }
            Message::PointSelect(own, i) => self.select_point(own, i),
            // A point gone mid-drag (an undo) is no undo step, nor an index panic.
            Message::PointMove(own, i, ..) if self.edit.own_line(own).is_none_or(|(keys, _)| i >= keys.len()) => {}
            Message::PointMove(own, i, t, db) => {
                self.edit.begin();
                let from = self.edit.own_line(own).map_or(t, |(keys, _)| keys[i].t);
                self.sound_op(own, |sound| SoundOp::MovePoint { sound, from, t, db });
                // The selection names the point by its time, which moved;
                // its place on the line didn't (points keep their order).
                self.select_point(own, i);
            }
            _ => {}
        }
    }

    /// A sound file asked for, read, its waveform or beats come back.
    fn on_sound_files(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddSound(which) => return self.pick_sound_file(which),
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
            Message::SoundProbed(which, path, at, Ok(len)) => return self.sound_added(which, path, at, len),
            Message::Waveform(path, Ok(wave)) => {
                self.waveforms.insert(path, wave);
            }
            Message::Waveform(path, Err(e)) => self.error = Some(format!("Can't read {}: {e}", path.display())),
            Message::Beats(path, read) => match read {
                Ok(beats) => {
                    self.hits.insert(path, Some(beats));
                }
                // Left as being read: not tried again over and over.
                Err(e) => self.error = Some(format!("Can't find the beats of {}: {e}", path.display())),
            },
            Message::CutWave(path, even, wave) => {
                self.cut_waves_reading = self.cut_waves_reading.saturating_sub(1);
                if let Some(wave) = wave {
                    (if even { &mut self.even_waves } else { &mut self.waveforms }).insert(path, wave);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// The file dialog for a sound on track `which`, in its folder of the
    /// project.
    fn pick_sound_file(&self, which: SoundTrack) -> Task<Message> {
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
        Task::perform(
            async move {
                let mut dialog = rfd::AsyncFileDialog::new().set_title(title).add_filter(filter, &["mp3", "MP3", "flac", "wav", "WAV", "m4a", "aac", "ogg", "opus", "wma"]);
                if let Some(dir) = dir {
                    dialog = dialog.set_directory(dir);
                }
                dialog.pick_file().await.map(|f| f.path().to_owned())
            },
            move |picked| Message::SoundPicked(which, picked),
        )
    }

    /// A sound read, `len` long, added to track `which` at `at` (none:
    /// after the music there), selected; its waveform read.
    fn sound_added(&mut self, which: SoundTrack, path: PathBuf, at: Option<f64>, len: f64) -> Task<Message> {
        let at = at.unwrap_or_else(|| self.edit.track(which).end());
        // An SFX over another lands on SFX 2 (the op's rule): open it.
        let added = self.op(Op::Clip(ClipOp::Add { track: which, input: path.clone(), at, start: 0.0, end: len }));
        if let Some((w, i)) = added.and_then(|o| o.made).and_then(|id| self.edit.clip_index(id)) {
            if w == SoundTrack::Sfx2 {
                self.folds.sfx2 = false;
            }
            self.selection = Selection::clip_at(&self.edit, w, i);
        }
        self.load_waveform(&path)
    }

    /// Auto-duck: the speech found off the UI thread, each voice's
    /// waveform cached or read once.
    fn find_speech(&mut self) -> Task<Message> {
        self.sound_menu = None;
        self.ducking = true;
        // Each voice's waveform (cached, or read once), off the UI thread.
        let (project, version) = ((*self.edit).clone(), self.edit.version);
        Task::perform(blocking(move || crate::duck::find_speech(&project).map_err(|e| format!("{e:#}"))), move |r| Message::Ducked(version, r))
    }

    /// The speech found: the music lowered under it, one undo step.
    fn ducked(&mut self, result: Result<Vec<(f64, f64)>, String>) {
        self.ducking = false;
        match result {
            Err(e) => self.error = Some(format!("Can't find the speech: {e}")),
            Ok(spans) if spans.is_empty() => self.error = Some("No speech to lower the music under: it's found in cleaned voices and on the Voice lane.".into()),
            Ok(speech) => {
                // One undo step; the line stays editable point by point.
                match self.edit.apply(&Op::Sound(SoundOp::Duck { depth: self.duck_db, speech })) {
                    Ok(_) => self.selection = self.selection.still_valid(&self.edit),
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        }
    }
}
