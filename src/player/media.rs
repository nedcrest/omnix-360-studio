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
    pub has_audio: bool,
    /// A sound file heard in place of the clip's own (`Cut::audio`), set on
    /// the copy a cut plays; the library's copy has none.
    pub audio: Option<PathBuf>,
    /// The cut's sound is evened out (`Cut::even`), on the copy it plays.
    pub even: bool,
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

    /// The filter its sound goes through: the leveler when evened out.
    pub fn leveler(&self) -> Option<&'static str> {
        self.even.then_some(crate::loudness::LEVELER)
    }

    /// This clip as cut `cut` plays it: with the cut's audio file, if any.
    pub(super) fn for_cut(&self, cut: &Cut) -> Media {
        let mut media = self.clone();
        media.audio = heard_audio(cut);
        media.even = cut.even;
        media.has_audio |= media.audio.is_some();
        media
    }
}

impl Media {
    /// `progress` hears how far reading the clip's telemetry has got (0..1),
    /// which is most of the time a clip takes to load.
    pub(super) fn load(input: PathBuf, opts: &Options, progress: &dyn Fn(f64)) -> Result<Media> {
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
        Ok(Media {
            duration: ffmpeg::duration(&input)?,
            osv_full: match p.flat {
                Some(f) => f.size().0.max(f.size().1),
                None => ffmpeg::dimensions(&input, p.tracks[0])?.0 as u32,
            },
            has_audio: ffmpeg::has_audio(&input)?,
            audio: None,
            even: false,
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

/// Caches `input` the way opening it in the player would: its telemetry and
/// timeline thumbnails (`rosmo360 cache add`).
pub fn cache_clip(input: &Path) -> Result<()> {
    let opts = Options { osv_size: None, hw: Some(Hwaccel::Software), hw_forced: true, stabilize: Mode::Horizon, imu_axes: ImuAxes::X180, world_z_down: false };
    thumbs::make(&Media::load(input.to_owned(), &opts, &|_| {})?, |_, _| true)
}
