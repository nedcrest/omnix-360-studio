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
