//! A clip as the player knows it (`Media`): its file, proxy, decoder, rig,
//! stabiliser, frame rate, sound; how it loads (`Media::load`, `cache_clip`);
//! and the launch options (`Launch`, `Options`).

use super::*;

#[derive(Clone)]
pub struct Media {
    pub input: PathBuf,
    /// Side-by-side proxy, if the clip has one.
    pub proxy: Option<PathBuf>,
    /// Stream the OSV at this size instead of the proxy.
    pub osv_size: Option<u32>,
    /// Pixels square of the OSV's lens tracks.
    pub osv_full: u32,
    pub hw: Hwaccel,
    /// Whether there's sound to play: the clip's own or its audio file's;
    /// false too once it failed to start.
    pub has_audio: bool,
    /// Whether the clip's file has sound of its own, as loaded.
    pub own_audio: bool,
    /// A sound file heard in place of the clip's own (`Cut::audio`), set on
    /// the copy a cut plays; the library's copy has none.
    pub audio: Option<PathBuf>,
    /// The filters the cut's sound goes through (`Cut::sound_filter`), on
    /// the copy it plays.
    pub filter: Option<String>,
    /// How fast the cut reads it (`Cut::speed`), on the copy it plays: its
    /// clock and its sound follow it.
    pub speed: crate::speed::Speed,
    pub rig: Arc<Rig>,
    pub stab: Stabilizer,
    pub readout_ms: f64,
    pub fps: f64,
    pub duration: f64,
    /// An ordinary video (`ffmpeg::flat`), not a 360 clip.
    pub flat: Option<ffmpeg::Flat>,
    /// The colour profile it was filmed in: D-Log M gets its LUT.
    pub color_mode: crate::dji::ColorMode,
}

impl Media {
    /// The file whose sound plays: the cut's own audio file, else the clip.
    pub fn heard(&self) -> &Path {
        self.audio.as_deref().unwrap_or(&self.input)
    }

    /// The filters its sound goes through played from clip time `t`:
    /// rumble cut, cleaning, evening, then its speed's tempo.
    pub fn filter_at(&self, t: f64) -> Option<String> {
        let tempo = ffmpeg::tempo(&self.speed, t, None, "p");
        match (self.filter.as_deref(), tempo.is_empty()) {
            (f, true) => f.map(str::to_owned),
            (None, false) => Some(tempo),
            (Some(f), false) => Some(format!("{f},{tempo}")),
        }
    }

    /// This clip as cut `cut` plays it: with the cut's audio file, if any.
    pub(super) fn for_cut(&self, cut: &Cut) -> Media {
        let mut media = self.clone();
        media.filter = cut.sound_filter();
        media.speed = cut.speed.clone();
        media.hear(heard_audio(cut));
        media
    }

    /// Whether this copy plays cut `cut` as it is: the same clip, sound
    /// file, filters and speed (`for_cut` made it so). Else it's made again.
    pub(super) fn plays(&self, cut: &Cut) -> bool {
        self.input == cut.input && self.audio == heard_audio(cut) && self.filter == cut.sound_filter() && self.speed == cut.speed
    }

    /// Its sound from `audio` (none: the clip's own): there's sound if
    /// either has it. Set again each time, so a voice that failed or a
    /// silent clip's voice taken away leaves no false answer (audit A15).
    pub(super) fn hear(&mut self, audio: Option<PathBuf>) {
        self.has_audio = self.own_audio || audio.is_some();
        self.audio = audio;
    }
}

impl Media {
    /// `progress` hears how far reading the clip's telemetry has got (0..1),
    /// which is most of the time a clip takes to load.
    pub(crate) fn load(input: PathBuf, opts: &Options, progress: &dyn Fn(f64)) -> Result<Media> {
        let stitch = crate::StitchOpts { stabilize: Some(opts.stabilize), imu_axes: opts.imu_axes, world_z_down: opts.world_z_down, ..Default::default() };
        let p = crate::Pipeline::load(&input, &stitch, opts.stabilize, progress)?;
        let proxy = if p.flat.is_some() { None } else { ffmpeg::proxy(&input) };
        // An ordinary video or picture has its own (`Pipeline::load_flat`):
        // a GPU decoder takes no JPEG, and a picture loaded first made every
        // later OSV decode in software, crawling at 1920 px.
        let hw = match opts.hw {
            _ if p.flat.is_some() && !opts.hw_forced => p.hw,
            Some(hw) => hw,
            None => {
                let hw = Hwaccel::pick(&input, p.tracks[0])?;
                eprintln!("decoding with {hw:?}");
                hw
            }
        };
        let own_audio = ffmpeg::has_audio(&input)?;
        Ok(Media {
            duration: ffmpeg::duration(&input)?,
            osv_full: match p.flat {
                Some(f) => f.size().0.max(f.size().1),
                None => ffmpeg::dimensions(&input, p.tracks[0])?.0 as u32,
            },
            has_audio: own_audio,
            own_audio,
            audio: None,
            filter: None,
            speed: Default::default(),
            proxy,
            osv_size: opts.osv_size,
            hw,
            rig: Arc::new(match p.flat {
                Some(flat) => Rig::flat(p.tracks[0], flat.hdr),
                None => Rig::new(p.lenses, p.tracks, false),
            }),
            stab: p.stab,
            readout_ms: p.readout_ms,
            fps: p.fps,
            flat: p.flat,
            color_mode: p.color_mode,
            input,
        })
    }
}

impl fmt::Debug for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Media").field("input", &self.input).finish_non_exhaustive()
    }
}

/// The audio file cut `cut` plays in place of its clip's sound: its own, if
/// it has one that's there; a missing one leaves the clip's sound playing
/// (the project's opening says which are missing).
pub(super) fn heard_audio(cut: &Cut) -> Option<PathBuf> {
    cut.audio.clone().filter(|a| a.exists())
}

/// What the command line asked for; what it leaves out comes from the
/// remembered settings.
pub struct Launch {
    pub osv_size: Option<u32>,
    pub hw: Option<Hwaccel>,
    pub stabilize: Option<Mode>,
    pub imu_axes: ImuAxes,
    pub world_z_down: bool,
}

/// Settings for loading and playing clips; the GUI updates them so the
/// next clip opens the way the last one was left.
#[derive(Clone)]
pub struct Options {
    pub osv_size: Option<u32>,
    /// Forced decoder, or the one found by probing the first Osmo clip.
    pub hw: Option<Hwaccel>,
    /// `hw` was asked for (`--hwaccel`), not found.
    pub hw_forced: bool,
    pub stabilize: Mode,
    pub imu_axes: ImuAxes,
    pub world_z_down: bool,
}

impl Default for Options {
    /// As a viewer opens a clip: the decoder probed, the horizon kept, the
    /// IMU as AGENTS.md's facts have it.
    fn default() -> Self {
        Options { osv_size: None, hw: None, hw_forced: false, stabilize: Mode::Horizon, imu_axes: ImuAxes::X180, world_z_down: false }
    }
}

impl Options {
    /// Decoded in software, nothing probed: for work without a player (the
    /// cache, tests), the same on every machine.
    pub fn software() -> Self {
        Options { hw: Some(Hwaccel::Software), hw_forced: true, ..Options::default() }
    }
}

/// Caches `input` the way opening it in the player would: its telemetry and
/// timeline thumbnails (`omnix cache add`).
pub fn cache_clip(input: &Path) -> Result<()> {
    thumbs::make(&Media::load(input.to_owned(), &Options::software(), &|_| {})?, |_, _| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cut's copy plays at its speed: its sound's filters end in its
    /// tempo from where it starts, and a change of speed makes it again.
    #[test]
    fn a_cut_at_speed_plays_through_its_tempo() {
        use crate::speed::{Speed, SpeedKey};
        let mut cut = Cut::new("a.mp4".into(), 0.0, 10.0);
        cut.speed = Speed { keys: vec![SpeedKey::held(0.0, 1.0), SpeedKey::held(4.0, 2.0)], tape: false };
        let library = Media { input: "a.mp4".into(), ..test_media() };
        let m = library.for_cut(&cut);
        assert_eq!(m.filter_at(5.0).as_deref(), Some("atempo=2"));
        assert!(m.filter_at(1.0).unwrap().starts_with("asplit=2"));
        assert!(m.plays(&cut));
        cut.speed = Speed::constant(0.5);
        assert!(!m.plays(&cut), "another speed: made again");
        assert_eq!(library.for_cut(&Cut::new("a.mp4".into(), 0.0, 10.0)).filter_at(3.0), None, "at 1×, its filters only");
    }

    /// A media with nothing loaded, for what reads its fields only.
    fn test_media() -> Media {
        Media {
            input: PathBuf::new(),
            proxy: None,
            osv_size: None,
            osv_full: 0,
            hw: Hwaccel::Software,
            has_audio: false,
            own_audio: false,
            audio: None,
            filter: None,
            speed: Default::default(),
            rig: Arc::new(Rig::flat(0, crate::hdr::Hdr::None)),
            stab: Stabilizer::new(&[], Mode::Off, ImuAxes::X180, -1.0, 0.0),
            readout_ms: 0.0,
            fps: 30.0,
            duration: 10.0,
            flat: None,
            color_mode: Default::default(),
        }
    }

    #[test]
    fn a_silent_clips_voice_taken_away_leaves_it_silent() {
        let dir = std::env::temp_dir().join(format!("omnix-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("silent.mp4");
        let made = std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=gray:s=64x36:d=0.5", "-an"]).arg(&clip).status();
        if !made.is_ok_and(|s| s.success()) {
            return;
        }
        let mut m = Media::load(clip, &Options::default(), &|_| {}).unwrap();
        assert!(!m.has_audio && !m.own_audio);
        m.hear(Some(dir.join("voice.flac")));
        assert!(m.has_audio, "a voice to play");
        m.hear(None);
        assert!(!m.has_audio, "back to the clip's own: none");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
