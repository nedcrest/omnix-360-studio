//! The export's sound: the cuts' stretches joined (each at its own level,
//! evened out, with its volume points, crossfaded into the next), the sound
//! tracks' clips placed, faded and levelled, all mixed and held under the
//! limiter, as one ffmpeg filter graph. Built as a string, so it is tested
//! without running ffmpeg. The player follows the same rules
//! (`mixdown`), and the tests here check the two sound alike.

use super::*;

/// A stretch of a clip's sound: from `start` seconds, for `duration` (to
/// the end if `None`).
pub struct Sound<'a> {
    pub input: &'a Path,
    pub start: f64,
    pub duration: Option<f64>,
    /// Volume points in clip time, applied only in a mix.
    pub volume: &'a [crate::project::VolumeKey],
    /// The cut's own level, dB (`Cut::sound_db`), applied only in a mix.
    pub gain: f64,
    /// The filters it goes through first (`loudness::sound_filter`: its
    /// rumble cut, cleaning, evening), only in a mix.
    pub filter: Option<String>,
    /// Seconds it crossfades with the stretch before, only in a mix.
    pub crossfade: f64,
    /// Its own fades in and out, seconds (`Cut::sound_fade_in`/`_out`),
    /// only in a mix; the fade out needs `duration`.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Seconds silent at its start and end: where its cut's sound stops
    /// short of the picture (`Cut::sound_before`/`_after` negative); its
    /// fades sit at the sound's edges inside them. Only in a mix.
    pub head: f64,
    pub tail: f64,
    /// How fast it reads its input: `duration`, the fades, the head and
    /// the tail are edit seconds, `start` and the volume points clip time.
    pub speed: &'a crate::speed::Speed,
}

/// The mix's last stage: peaks held under −1.2 dBFS (0.87), so cuts raised
/// by normalizing don't clip: −1.2 rather than −1, as the peaks between
/// samples (true peak) came out 0.2 dB over at −1 on a normalized Kos edit.
/// Without `level=0` it would raise the whole
/// mix by itself; without `latency=1` it would delay the sound by its 5 ms
/// lookahead.
pub const LIMITER: &str = "alimiter=limit=0.87:attack=5:release=50:level=0:latency=1";

/// What a whole edit's sound gets on top of joining its clips' stretches:
/// their level, and the sound tracks (the music, the sound effects).
pub struct Mix<'a> {
    /// The clips' sound, dB.
    pub sound_db: f64,
    /// The sound tracks, each with its level, clips and volume points.
    pub tracks: Vec<&'a crate::project::Music>,
    /// The edit's length: music running on is cut here, faded out over
    /// `project::END_FADE`.
    pub length: f64,
    /// The edit's fades from and to black, the sound with the picture.
    pub fades: crate::project::Fades,
}

impl<'a> Mix<'a> {
    /// The whole film's mix, as the export makes it: the clips' sound at
    /// their level, every track, and `beyond` (the cuts' sound past their
    /// pictures, `Project::sound_beyond`) as clips at the clips' level.
    pub fn whole(project: &'a crate::project::Project, beyond: &'a [crate::project::Music], length: f64) -> Self {
        let tracks = crate::project::SoundTrack::ALL.map(|t| project.track(t)).into_iter().chain(beyond).collect();
        Mix { sound_db: project.sound.volume, tracks, length, fades: project.fades }
    }
}

impl<'a> Sound<'a> {
    /// `duration` seconds of `input` from `start`, as recorded: no level,
    /// points, filters or fades.
    pub fn plain(input: &'a Path, start: f64, duration: Option<f64>) -> Self {
        Sound { input, start, duration, volume: &[], gain: 0.0, filter: None, crossfade: 0.0, fade_in: 0.0, fade_out: 0.0, head: 0.0, tail: 0.0, speed: &crate::speed::NORMAL }
    }

    /// Cut `c`'s stretch under its picture, from `input` at clip time
    /// `start` for `duration`: its level, points, filters and fades, and
    /// silence where its sound stops short. A fade at an edge its sound runs
    /// past belongs to the sound beyond (`Cut::sound_beyond`), not here.
    pub fn of_cut(c: &'a crate::project::Cut, input: &'a Path, start: f64, duration: Option<f64>, crossfade: f64) -> Sound<'a> {
        // Where its sound stops short, clip seconds, as edit seconds.
        let short = |from: f64, secs: f64| if c.speed.is_normal() { secs } else { c.speed.edit_secs(from, from + secs) };
        Sound {
            input,
            start,
            duration,
            volume: &c.volume,
            gain: c.sound_db(),
            filter: c.sound_filter(),
            crossfade,
            fade_in: c.sound_fades().0,
            fade_out: c.sound_fades().1,
            head: short(c.start, (-c.sound_before).max(0.0)),
            tail: short(c.end - (-c.sound_after).max(0.0), (-c.sound_after).max(0.0)),
            speed: &c.speed,
        }
    }

    /// How much of its input it reads, clip seconds: its `duration` at its
    /// speed.
    pub(super) fn reads(&self) -> Option<f64> {
        self.duration.map(|d| if self.speed.is_normal() { d } else { self.speed.clip_after(self.start, d) - self.start })
    }

    /// Its stream's time `t` (edit seconds from its start) as clip time,
    /// for its volume points: `(t+start)`, through its speed's stretches.
    fn clip_time(&self) -> String {
        if self.speed.is_normal() {
            return format!("(t+{})", self.start);
        }
        let mut e = 0.0;
        let pieces: Vec<(f64, f64, f64)> = self.speed.pieces(self.start, f64::INFINITY).into_iter().map(|(a, b, r)| {
            let at = e;
            e += (b - a) / r;
            (at, a, r)
        }).collect();
        let (last, rest) = pieces.split_last().expect("one stretch at least");
        let mut x = format!("({}+(t-{})*{})", last.1, last.0, last.2);
        for (w, next) in rest.iter().zip(&pieces[1..]).rev() {
            x = format!("if(lt(t,{}),{}+(t-{})*{},{x})", next.0, w.1, w.0, w.2);
        }
        x
    }

    /// Its sound at its speed, after its clean-up (`tempo`); `i` names
    /// its labels.
    fn tempo(&self, i: usize) -> String {
        let to = self.reads().map(|d| self.start + d);
        let tempo = tempo(self.speed, self.start, to, &i.to_string());
        if tempo.is_empty() { tempo } else { format!(",{tempo}") }
    }

    /// Its stream's filters after the resampling, as input `i` of a mix:
    /// its clean-up, its tempo, its pad, its level and points, its fades.
    fn chain(&self, i: usize) -> String {
        // Its volume points, in clip time: the stretch's time starts at 0.
        // Muted is silent whatever its points say (the player's rule).
        let level = if self.gain <= crate::project::OFF_DB {
            ",volume=0".to_owned()
        } else if self.volume.is_empty() && self.gain == 0.0 {
            String::new()
        } else {
            format!(",{}", level_filter(self.gain, self.volume, &self.clip_time()))
        };
        let even = self.filter.as_ref().map_or_else(String::new, |f| format!(",{f}"));
        // Straight, as the player's `Cut::sound_fade`, at the sound's
        // own edges; silent past them.
        let mut fades = String::new();
        if self.head > 0.0 {
            fades += &format!(",volume=0:enable='lt(t,{})'", self.head);
        }
        if self.fade_in > 0.0 {
            fades += &format!(",afade=t=in:st={}:d={}", self.head, self.fade_in);
        }
        if let Some(d) = self.duration {
            let end = d - self.tail;
            if self.fade_out > 0.0 {
                fades += &format!(",afade=t=out:st={}:d={}", (end - self.fade_out).max(0.0), self.fade_out.min(end));
            }
            if self.tail > 0.0 {
                fades += &format!(",volume=0:enable='gt(t,{end})'");
            }
        }
        format!("{even}{}{}{level}{fades}", self.tempo(i), pad(self))
    }

    /// Whether its input gives sound: its clip's, or silence in its place
    /// (`Encoder::new`) where the clip has none and its length is known.
    pub(super) fn sounds(&self) -> bool {
        self.duration.is_some() || has_audio(self.input).unwrap_or(false)
    }
}

impl Mix<'_> {
    /// Whether it changes anything.
    pub(super) fn matters(&self) -> bool {
        self.sound_db != 0.0 || self.tracks.iter().any(|t| !t.clips.is_empty()) || !self.fades.is_none()
    }
}

/// Sound read from clip time `from` (to `to`, else on) at `speed`, as
/// filters: a tempo per stretch of constant rate (`atempo`, which keeps
/// the pitch, twice below 0.5×; like tape, `asetrate` at 48 kHz), a curve's
/// stretches cut apart and joined, labelled by `label`. At normal speed,
/// nothing. The export's mix and the player's sound both use it.
pub fn tempo(speed: &crate::speed::Speed, from: f64, to: Option<f64>, label: &str) -> String {
    if speed.is_normal() {
        return String::new();
    }
    let rate = |r: f64| {
        if speed.tape {
            format!("aresample=48000,asetrate={},aresample=48000", 48000.0 * r)
        } else if r < 0.5 {
            format!("atempo=0.5,atempo={}", r / 0.5)
        } else {
            format!("atempo={r}")
        }
    };
    let pieces = speed.pieces(from, to.unwrap_or(f64::INFINITY));
    if let [(_, _, r)] = pieces[..] {
        return rate(r);
    }
    let n = pieces.len();
    let mut s = format!("asplit={n}{};", (0..n).map(|k| format!("[t{label}_{k}]")).collect::<String>());
    for (k, (a, b, r)) in pieces.iter().enumerate() {
        let end = if b.is_finite() { format!(":end={}", b - from) } else { String::new() };
        s += &format!("[t{label}_{k}]atrim=start={}{end},asetpts=PTS-STARTPTS,{}[u{label}_{k}];", a - from, rate(*r));
    }
    s + &(0..n).map(|k| format!("[u{label}_{k}]")).collect::<String>() + &format!("concat=n={n}:v=0:a=1")
}

/// Pads a stretch with silence to its frames' length, as a filter to append
/// (`,apad=…`), when that length is known. A sound shorter than its picture
/// would otherwise pull every later cut's sound early: a camera's own track
/// ends a few ms before its video (8 ms on an Action 2), a voice cleaned of
/// noise some 30 ms before its clip.
pub(super) fn pad(sound: &Sound<'_>) -> String {
    sound.duration.map_or_else(String::new, |d| format!(",apad=whole_dur={d}"))
}

/// `base` dB and the line through volume points `keys` as a `volume`
/// filter, the same line the player follows (straight in dB between
/// points), worked out per audio frame (~20 ms). `t` is the expression
/// for the points' time from the stream's.
fn level_filter(base: f64, keys: &[crate::project::VolumeKey], t: &str) -> String {
    if keys.is_empty() {
        return format!("volume={}", crate::project::gain(base));
    }
    // dB at `t`: nested ifs from the last stretch back to the first.
    let mut db = format!("{}", keys[keys.len() - 1].volume);
    for w in keys.windows(2).rev() {
        let (a, b) = (w[0], w[1]);
        db = format!("if(lt({t},{}),{}+({})*({t}-{})/{},{db})", b.t, a.volume, b.volume - a.volume, a.t, (b.t - a.t).max(1e-9));
    }
    db = format!("if(lt({t},{}),{},{db})", keys[0].t, keys[0].volume);
    let off = crate::project::OFF_DB;
    format!("volume=eval=frame:volume='if(lte({base}+{db},{off}),0,pow(10,({base}+{db})/20))'")
}

/// The filter graph for `Encoder::new` with a mix, adding the sound tracks'
/// inputs to `cmd`: the clips' stretches joined (or silence where they have
/// none) at their level, and each track's clips trimmed to their stretch and
/// to the edit, faded, placed at their time and at their track's level, all
/// mixed over the edit's length.
pub(super) fn mixed(cmd: &mut Command, audio: &[Sound<'_>], mix: &Mix<'_>) -> Result<String> {
    let stereo = "aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo";
    let mut graph = String::new();
    if !audio.is_empty() && audio.iter().all(Sound::sounds) {
        for (i, sound) in (1..).zip(audio) {
            graph += &format!("[{i}:a:0]{stereo}{}[c{i}];", sound.chain(i));
        }
        // Joined in turn: a crossfade overlaps a stretch's end with the
        // next's start (equal power, as the music's), else end to end.
        let mut joined = "[c1]".to_owned();
        for (i, sound) in (2..).zip(&audio[1..]) {
            let join = if sound.crossfade > 0.0 { format!("acrossfade=d={}:c1=qsin:c2=qsin", sound.crossfade) } else { "concat=n=2:v=0:a=1".to_owned() };
            graph += &format!("{joined}[c{i}]{join}[j{i}];");
            joined = format!("[j{i}]");
        }
        graph += &format!("{joined}volume={}[s];", crate::project::gain(mix.sound_db));
    } else {
        if !audio.is_empty() {
            eprintln!("not every clip has sound; exporting the music and sound effects alone");
        }
        graph += &format!("anullsrc=r=48000:cl=stereo,atrim=duration={}[s];", mix.length);
    }
    let mut parts = vec!["[s]".to_owned()];
    for (track, i, clip) in mix.tracks.iter().flat_map(|t| t.clips.iter().enumerate().map(move |(i, c)| (*t, i, c))) {
        // What's left of the edit from where it starts.
        let room = mix.length - clip.at;
        let len = (clip.end - clip.start).min(room);
        // A muted clip is silent whatever its points say: left out.
        if len <= 0.0 || clip.sound_db() <= crate::project::OFF_DB {
            continue;
        }
        let input = audio.len() + parts.len();
        cmd.args(["-ss", &clip.start.to_string(), "-t", &len.to_string()]).arg("-i").arg(&clip.input);
        let mut chain = format!("[{input}:a:0]{stereo}");
        // Its own sound, as a cut's: evened, then its level and points
        // (file time: the stretch starts at `clip.start`).
        if let Some(f) = clip.sound_filter() {
            chain += &format!(",{f}");
        }
        if clip.gain != 0.0 || !clip.volume.is_empty() {
            chain += &format!(",{}", level_filter(clip.gain, &clip.volume, &format!("(t+{})", clip.start)));
        }
        // Fades in at their own length, even where the edit's end cuts the
        // clip shorter: shortened, they'd rise faster than the player's.
        if clip.fade_in > 0.0 {
            chain += &format!(",afade=t=in:st=0:d={}", clip.fade_in);
        }
        // Its own fade-out and its crossfade out keep their place at its
        // own end, past `len` when the edit cuts it off; the edit's end
        // fade comes on top. Chained fades multiply, as the player's
        // `envelope` does.
        let whole = clip.end - clip.start;
        let mut fade_out = |d: f64, curve: &str| {
            let d = d.min(whole);
            if d > 0.0 && whole - d < len {
                chain += &format!(",afade=t=out:st={}:d={d}{curve}", whole - d);
            }
        };
        let (cross_in, cross_out) = track.crossfades(i);
        fade_out(clip.fade_out, "");
        // Equal power, as the player has them (a quarter sine each way).
        fade_out(cross_out, ":curve=qsin");
        if cross_in > 0.0 {
            chain += &format!(",afade=t=in:st=0:d={cross_in}:curve=qsin");
        }
        // The edit's end fade runs over its last `END_FADE` seconds whatever
        // the clip: one starting inside it starts already lowered, as the
        // player's `envelope` has it (an `afade` can't start part-way).
        if whole > room {
            chain += &format!(",volume=eval=frame:volume='clip(({len}-t)/{},0,1)'", crate::project::END_FADE);
        }
        let delay = (clip.at * 1000.0).round();
        graph += &format!("{chain},adelay=delays={delay}:all=1,{}[m{input}];", level_filter(track.volume, &[], "t"));
        parts.push(format!("[m{input}]"));
    }
    let mut fades = String::new();
    if mix.fades.fade_in > 0.0 {
        fades += &format!(",afade=t=in:st=0:d={}", mix.fades.fade_in.min(mix.length));
    }
    if mix.fades.fade_out > 0.0 {
        let d = mix.fades.fade_out.min(mix.length);
        fades += &format!(",afade=t=out:st={}:d={d}", mix.length - d);
    }
    graph += &format!("{}amix=inputs={}:duration=first:normalize=0{fades},{LIMITER}[a]", parts.concat(), parts.len());
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stretch<'a>(input: &'a Path, gain: f64, volume: &'a [crate::project::VolumeKey]) -> Sound<'a> {
        Sound { input, start: 2.0, duration: Some(4.0), volume, gain, filter: None, crossfade: 0.0, fade_in: 0.0, fade_out: 0.0, head: 0.0, tail: 0.0, speed: &crate::speed::NORMAL }
    }

    #[test]
    fn a_muted_cut_is_silent_in_the_mix_whatever_its_points() {
        let up = [crate::project::VolumeKey { t: 3.0, volume: 6.0 }];
        let music = crate::project::Music::default();
        let mix = Mix { sound_db: 0.0, tracks: vec![&music], length: 8.0, fades: Default::default() };
        let (a, b) = (Path::new("a.mp4"), Path::new("b.mp4"));
        let graph = mixed(&mut tool("ffmpeg"), &[stretch(a, crate::project::OFF_DB, &up), stretch(b, 3.0, &[])], &mix).unwrap();
        // The first stretch silenced, the second at +3 dB, the limiter last.
        assert!(graph.contains("[1:a:0]") && graph.split("[c1]").next().unwrap().contains("volume=0"), "{graph}");
        assert!(graph.contains(&format!("volume={}", crate::project::gain(3.0))), "{graph}");
        assert!(graph.ends_with(&format!("{LIMITER}[a]")), "{graph}");
    }

    /// A cut's sound at speed: its clip read for its span, its tempo
    /// before its pad (so the pad, its fades and its crossfade are edit
    /// seconds), its points read through its speed.
    #[test]
    fn a_cut_s_sound_plays_at_its_speed() {
        use crate::speed::{Speed, SpeedKey};
        let up = [crate::project::VolumeKey { t: 3.0, volume: 6.0 }, crate::project::VolumeKey { t: 5.0, volume: 0.0 }];
        let music = crate::project::Music::default();
        let mix = Mix { sound_db: 0.0, tracks: vec![&music], length: 8.0, fades: Default::default() };
        let (twice, quarter) = (Speed::constant(2.0), Speed::constant(0.25));
        let a = Path::new("a.mp4");
        // 4 edit seconds at 2× read 8 clip seconds.
        let fast = Sound { speed: &twice, ..stretch(a, 0.0, &up) };
        assert_eq!(fast.reads(), Some(8.0));
        let graph = mixed(&mut tool("ffmpeg"), &[fast], &mix).unwrap();
        let chain = graph.split("[c1]").next().unwrap();
        assert!(chain.find("atempo=2").unwrap() < chain.find("apad=whole_dur=4").unwrap(), "{graph}");
        assert!(chain.contains("(2+(t-0)*2)"), "the points' clock: {graph}");
        let slow = Sound { speed: &quarter, ..stretch(a, 0.0, &[]) };
        assert!(slow.tempo(1).contains("atempo=0.5,atempo=0.5"), "atempo goes no lower than 0.5");
        let tape = Speed { tape: true, ..Speed::constant(2.0) };
        assert_eq!(Sound { speed: &tape, ..stretch(a, 0.0, &[]) }.tempo(1), ",aresample=48000,asetrate=96000,aresample=48000");
        // The player's, from where it starts playing, on to the end.
        assert_eq!(tempo(&Speed { keys: vec![SpeedKey { t: 0.0, rate: 1.0 }, SpeedKey { t: 3.0, rate: 2.0 }], tape: false }, 4.0, None, "p"), "atempo=2");
        // A curve: 1× to clip second 3, then 2×: two stretches joined.
        let curve = Speed { keys: vec![SpeedKey { t: 0.0, rate: 1.0 }, SpeedKey { t: 3.0, rate: 2.0 }], tape: false };
        let ramp = Sound { speed: &curve, ..stretch(a, 0.0, &[]) };
        // 1 s at 1×, then 3 edit s at 2×: clip 2..9.
        assert_eq!(ramp.reads(), Some(7.0));
        assert_eq!(ramp.tempo(1), ",asplit=2[t1_0][t1_1];[t1_0]atrim=start=0:end=1,asetpts=PTS-STARTPTS,atempo=1[u1_0];[t1_1]atrim=start=1:end=7,asetpts=PTS-STARTPTS,atempo=2[u1_1];[u1_0][u1_1]concat=n=2:v=0:a=1");
        assert_eq!(ramp.clip_time(), "if(lt(t,1),2+(t-0)*1,(3+(t-1)*2))");
        // At normal speed the graph is as it was.
        assert_eq!(stretch(a, 0.0, &up).clip_time(), "(t+2)");
        assert_eq!(stretch(a, 0.0, &up).tempo(1), "");
    }

    #[test]
    fn a_sound_clips_own_level_and_evening_come_before_its_fades() {
        let mut voice = crate::project::Music::default();
        voice.clips.push(crate::project::MusicClip { input: "take1.wav".into(), at: 1.0, start: 0.5, end: 3.5, gain: 4.0, even: true, fade_in: 0.3, ..Default::default() });
        voice.clips.push(crate::project::MusicClip { input: "take2.wav".into(), at: 5.0, start: 0.0, end: 2.0, mute: true, ..Default::default() });
        let mix = Mix { sound_db: 0.0, tracks: vec![&voice], length: 8.0, fades: Default::default() };
        let mut cmd = tool("ffmpeg");
        let graph = mixed(&mut cmd, &[], &mix).unwrap();
        let chain = graph.split("[1:a:0]").nth(1).unwrap();
        let (even, level, fade) = (chain.find(crate::loudness::LEVELER).unwrap(), chain.find(&format!("volume={}", crate::project::gain(4.0))).unwrap(), chain.find("afade=t=in").unwrap());
        assert!(even < level && level < fade, "{graph}");
        // The muted take isn't even an input.
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!args.iter().any(|a| a == "take2.wav"), "{args:?}");
    }

    #[test]
    fn the_exports_volume_line_sounds_as_the_player_draws_it() {
        use crate::ffmpeg::testmedia;
        let dir = testmedia::dir("level-filter");
        let (plain, levelled) = (dir.join("plain.wav"), dir.join("levelled.wav"));
        testmedia::tone(&plain, 3.0);
        let keys = [crate::project::VolumeKey { t: 0.5, volume: 0.0 }, crate::project::VolumeKey { t: 1.5, volume: -12.0 }, crate::project::VolumeKey { t: 2.5, volume: 6.0 }];
        let filter = level_filter(-3.0, &keys, "t");
        testmedia::ffmpeg(&["-i", plain.to_str().unwrap(), "-af", &filter, levelled.to_str().unwrap()]);
        let (base, got) = (crate::ffmpeg::audio_levels(&plain, 0.1).unwrap(), crate::ffmpeg::audio_levels(&levelled, 0.1).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
        // Each tenth of a second, as `volume_at` says at its middle.
        for (k, ((b, _), (g, _))) in base.iter().zip(&got).enumerate().take(29) {
            let t = k as f64 * 0.1 + 0.05;
            let want = -3.0 + crate::project::volume_at(&keys, t);
            assert!((g - b - want).abs() < 0.6, "at {t:.2} s: {:.2} dB, want {want:.2}", g - b);
        }
        // At or below OFF_DB: silence, as the player has it.
        assert!(level_filter(crate::project::OFF_DB, &[], "t").ends_with("volume=0"));
    }

    /// Renders `track` through the export's mix over `length` seconds and
    /// compares its level every 0.1 s with the player's: the track's level
    /// times each clip's own level and envelope, clips adding by power.
    fn export_matches_player(name: &str, track: &mut crate::project::Music, length: f64) -> Vec<String> {
        use crate::ffmpeg::testmedia;
        let dir = testmedia::dir(name);
        for (k, clip) in track.clips.iter_mut().enumerate() {
            clip.input = dir.join(format!("{k}.wav"));
            testmedia::tone_at(&clip.input, clip.end + 1.0, 500 + 400 * k as u32);
        }
        let plain = dir.join("plain.wav");
        testmedia::tone(&plain, 1.0);
        let base = crate::ffmpeg::audio_levels(&plain, 0.5).unwrap()[0].0;
        let mix = Mix { sound_db: 0.0, tracks: vec![track], length, fades: Default::default() };
        // Input 0 stands for the picture, as in an export.
        let mut cmd = tool("ffmpeg");
        cmd.args(["-v", "error", "-nostdin", "-y", "-f", "lavfi", "-t", &length.to_string(), "-i", "anullsrc=r=48000:cl=stereo"]);
        let graph = mixed(&mut cmd, &[], &mix).unwrap();
        let out = dir.join("out.wav");
        assert!(cmd.args(["-filter_complex", &graph, "-map", "[a]"]).arg(&out).status().unwrap().success(), "{graph}");
        let got = crate::ffmpeg::audio_levels(&out, 0.1).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let mut wrong = Vec::new();
        for (k, (db, _)) in got.iter().enumerate() {
            let t = k as f64 * 0.1 + 0.05;
            if t >= length - 0.05 {
                break;
            }
            // The player's power averaged over the same 0.1 s the level is
            // measured on, so steep fades compare fairly.
            let power_at = |t: f64| -> f64 {
                (0..track.clips.len())
                    .map(|i| {
                        let g = crate::mixdown::clip_gain(track, i, t, length);
                        g * g
                    })
                    .sum()
            };
            let power = (0..20).map(|j| power_at(k as f64 * 0.1 + (j as f64 + 0.5) * 0.005)).sum::<f64>() / 20.0;
            let want = base + 10.0 * power.max(1e-12).log10();
            // Clearly audible levels only: the export steps a level once per
            // audio frame (~21 ms), which tells near silence.
            if want > base - 20.0 && (db - want).abs() > 0.7 {
                wrong.push(format!("{t:.2} s: export {db:.1} dB, player {want:.1}"));
            }
        }
        wrong
    }

    #[test]
    fn the_export_fades_sound_clips_as_the_player_does() {
        use crate::project::MusicClip;
        let clip = |at: f64, end: f64| MusicClip { at, start: 0.0, end, ..Default::default() };
        // Fades in and out, a crossfade, a clip at its own level.
        let mut track = crate::project::Music::default();
        track.clips = vec![MusicClip { fade_in: 1.0, ..clip(0.0, 3.0) }, MusicClip { fade_out: 1.0, gain: -6.0, ..clip(2.5, 3.0) }];
        let wrong = export_matches_player("mix-fades", &mut track, 6.0);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn the_export_cuts_a_clip_off_at_the_edits_end_as_the_player_does() {
        use crate::project::MusicClip;
        // A long fade-out of its own, cut off by the edit's end (and so
        // its end fade too), and a crossfade the end cuts into.
        let mut track = crate::project::Music::default();
        track.clips = vec![MusicClip { at: 0.0, start: 0.0, end: 6.0, fade_out: 6.0, ..Default::default() }];
        let mut wrong = export_matches_player("mix-cut-off", &mut track, 5.0);
        let mut track = crate::project::Music::default();
        track.clips = vec![MusicClip { at: 0.0, start: 0.0, end: 4.0, ..Default::default() }, MusicClip { at: 3.0, start: 0.0, end: 3.0, ..Default::default() }];
        wrong.extend(export_matches_player("mix-cross-cut-off", &mut track, 3.6));
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn cuts_join_end_to_end_or_crossfade_and_pad_to_their_pictures() {
        let music = crate::project::Music::default();
        let mix = Mix { sound_db: -6.0, tracks: vec![&music], length: 6.0, fades: Default::default() };
        let (a, b, c) = (Path::new("a.mp4"), Path::new("b.mp4"), Path::new("c.mp4"));
        let mut faded = stretch(b, 0.0, &[]);
        faded.crossfade = 0.5;
        // Lengths known: each padded to its pictures' length, the second
        // crossfading in, the third end to end; the clips' level last.
        let graph = mixed(&mut tool("ffmpeg"), &[stretch(a, 0.0, &[]), faded, stretch(c, 0.0, &[])], &mix).unwrap();
        assert!(graph.matches("apad=whole_dur=4").count() == 3, "{graph}");
        assert!(graph.contains("[c1][c2]acrossfade=d=0.5:c1=qsin:c2=qsin[j2]"), "{graph}");
        assert!(graph.contains("[j2][c3]concat=n=2:v=0:a=1[j3]"), "{graph}");
        assert!(graph.contains(&format!("[j3]volume={}[s]", crate::project::gain(-6.0))), "{graph}");
        // A clip of unknown length without sound: silence in place of the
        // whole edit's clip sound, the music still mixed.
        let mut silent = stretch(Path::new("no-such-file.mp4"), 0.0, &[]);
        silent.duration = None;
        let graph = mixed(&mut tool("ffmpeg"), &[silent], &mix).unwrap();
        assert!(graph.starts_with("anullsrc=r=48000:cl=stereo,atrim=duration=6[s];"), "{graph}");
    }

    #[test]
    fn a_cuts_sound_fades_in_and_out_over_its_own_stretch() {
        let music = crate::project::Music::default();
        let mix = Mix { sound_db: 0.0, tracks: vec![&music], length: 4.0, fades: Default::default() };
        let a = Path::new("a.mp4");
        let sound = Sound { fade_in: 0.5, fade_out: 1.5, ..stretch(a, 0.0, &[]) };
        let graph = mixed(&mut tool("ffmpeg"), &[sound], &mix).unwrap();
        // The 4 s stretch: in over its first 0.5 s, out over its last 1.5.
        let c1 = graph.split("[c1]").next().unwrap();
        assert!(c1.contains("afade=t=in:st=0:d=0.5") && c1.contains("afade=t=out:st=2.5:d=1.5"), "{graph}");
    }
}
