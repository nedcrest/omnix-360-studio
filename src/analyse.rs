//! Reading clips: `info`, `scan` (heading, shake, loudness, sky per second), `seam`, and
//! colour tracking (`track_to`).

use crate::{align, cache, ffmpeg, sky, stab, track};
use crate::pipeline::{Pipeline, Shot, StitchOpts, Carry};
use crate::fsutil::osv_under;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Fails with a plain reason when `input` is an ordinary video, which has
/// no `what` (only an Osmo 360's .OSV does).
pub(crate) fn needs_360(input: &Path, what: &str) -> Result<()> {
    if ffmpeg::flat(input)?.is_some() {
        bail!("{} is an ordinary video or picture: it has no {what}, only an Osmo 360 clip (.OSV) does", input.display());
    }
    Ok(())
}

pub(crate) fn info(input: &Path) -> Result<()> {
    if let Some(flat) = ffmpeg::flat(input)? {
        let info = if flat.picture {
            serde_json::json!({ "input": input, "kind": "picture", "size": flat.size() })
        } else {
            serde_json::json!({
            "input": input,
            "kind": "video",
            "duration": ffmpeg::duration(input)?,
            "fps": ffmpeg::frame_rate(input)?,
            "size": flat.size(),
            "recorded": ffmpeg::creation_time(input)?,
            "audio": ffmpeg::has_audio(input)?,
            })
        };
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    let telemetry = cache::telemetry(input, &|_| {})?;
    let quats = telemetry.quats;
    let info = serde_json::json!({
        "input": input,
        "kind": "osv",
        "colour": telemetry.color_mode.name(),
        "proxy": ffmpeg::proxy(input),
        "duration": ffmpeg::duration(input)?,
        "fps": ffmpeg::frame_rate(input)?,
        "lens_size": ffmpeg::dimensions(input, 0)?,
        "recorded": ffmpeg::creation_time(input)?,
        "audio": ffmpeg::has_audio(input)?,
        "attitude": !quats.is_empty(),
    });
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}

/// Tracks `colour` through `input` from `start` for `duration` seconds (to
/// the end if `None`), writing `Cmd::Track`'s CSV to `out`. `progress(done,
/// total)` hears of each sample and cancels by failing. Returns how many of
/// how many samples found it.
pub(crate) fn track_to(
    input: &Path,
    colour: &track::Colour,
    near: Option<track::Near>,
    start: Option<f64>,
    duration: Option<f64>,
    every: f64,
    out: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(usize, usize) -> Result<()>,
) -> Result<(usize, usize)> {
    // Small, unstabilised (so its directions are the rig's) and from the
    // proxy: enough to find a jacket, and fast.
    const WIDTH: usize = 512;
    let mut p = Pipeline::new(input, &StitchOpts::default(), stab::Mode::Off)?;
    if !p.stab.has_data() {
        bail!("{} has no attitude data to track with", input.display());
    }
    p.use_proxy();
    let (start, end) = p.span(start, duration, None)?;
    let total = ((end - start) / every).ceil().max(1.0) as usize;
    let mut stream = p.stream(start)?;
    let mut carry = Carry::default();
    let mut last: Option<(f64, [f64; 3])> = near.map(|near| {
        let d = match near {
            track::Near::Level(yaw, pitch) => track::from_level(&p.stab, start * 1000.0, p.readout_ms, yaw, pitch),
            track::Near::Rig(d) => d,
        };
        (start, d)
    });
    let (mut next, mut n, mut found, mut rows) = (start, 0usize, 0usize, 0usize);
    writeln!(out, "t,x,y,z,yaw,pitch,size")?;
    while let Some(frames) = stream.next()? {
        let t = start + n as f64 / p.fps;
        n += 1;
        if t > end {
            break;
        }
        if t + 0.5 / p.fps < next {
            continue;
        }
        next += every;
        rows += 1;
        let pic = p.render(&frames, t, &mut carry, &Shot::Equirect(WIDTH));
        // Look where it was last, carried through the camera's turning since;
        // after a few seconds lost, anywhere again.
        let guess = last.filter(|(at, _)| t - at < 3.0).map(|(at, d)| p.stab.to_rig(t * 1000.0, p.stab.to_world(at * 1000.0, d)));
        match track::find(&pic.rgb, WIDTH, colour, guess) {
            Some((d, share)) => {
                let (yaw, pitch) = track::level_angles(&p.stab, t * 1000.0, p.readout_ms, d);
                writeln!(out, "{t:.3},{:.5},{:.5},{:.5},{yaw:.1},{pitch:.1},{:.3}", d[0], d[1], d[2], share * 100.0)?;
                last = Some((t, d));
                found += 1;
            }
            None => writeln!(out, "{t:.3},,,,,,")?,
        }
        progress(rows.min(total), total)?;
    }
    Ok((found, rows))
}

/// `scan` of each clip under `inputs` into `dir/<clip>.csv`, `jobs` at once,
/// a line each on how it went; a clip that fails is reported and skipped.
pub(crate) fn scan_many(inputs: &[PathBuf], dir: &Path, jobs: usize, every: f64, image: bool) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let clips = osv_under(inputs)?;
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    eprintln!("scanning {} clips into {}", clips.len(), dir.display());
    let (next, failed) = (AtomicUsize::new(0), AtomicUsize::new(0));
    std::thread::scope(|s| {
        for _ in 0..jobs.clamp(1, clips.len().max(1)) {
            s.spawn(|| {
                while let Some(clip) = clips.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let at = std::time::Instant::now();
                    let name = clip.file_stem().map_or_else(|| clip.display().to_string(), |n| n.to_string_lossy().into_owned());
                    let out = dir.join(format!("{name}.csv"));
                    // To a temporary file first, so a failed scan leaves no half CSV.
                    let tmp = dir.join(format!(".{name}.csv.part"));
                    let result = needs_360(clip, "motion data").and_then(|_| {
                        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
                        scan(clip, every, image, &mut w)?;
                        std::io::Write::flush(&mut w)?;
                        drop(w);
                        std::fs::rename(&tmp, &out)?;
                        Ok(())
                    });
                    match result {
                        Ok(()) => println!("{name}: {} in {:.0} s", out.display(), at.elapsed().as_secs_f64()),
                        Err(e) => {
                            let _ = std::fs::remove_file(&tmp);
                            failed.fetch_add(1, Ordering::Relaxed);
                            println!("{name}: failed: {e:#}");
                        }
                    }
                }
            });
        }
    });
    let failed = failed.into_inner();
    eprintln!("{} scanned, {failed} failed", clips.len() - failed);
    Ok(())
}

/// One CSV row per `every` seconds into `out`; see `Cmd::Scan` and the header.
pub(crate) fn scan(input: &Path, every: f64, image: bool, out: &mut dyn std::io::Write) -> Result<()> {
    let skies = if image { skies(input)? } else { Vec::new() };
    let quats = cache::telemetry(input, &|_| {})?.quats;
    let duration = ffmpeg::duration(input)?;
    // The proxy carries the same sound (decoded, sample for sample) at a
    // sixteenth of the file: reading the clip's own from a network share
    // made a folder of clips take minutes.
    let sound = ffmpeg::proxy(input).filter(|p| ffmpeg::has_audio(p).unwrap_or(false)).unwrap_or_else(|| input.to_owned());
    let levels = if ffmpeg::has_audio(&sound)? { ffmpeg::audio_levels(&sound, every)? } else { Vec::new() };
    let stab = (!quats.is_empty()).then(|| stab::Stabilizer::new(&quats, stab::Mode::Off, stab::ImuAxes::X180, -1.0, 0.0));
    // Turning is sampled every 20 ms: fine enough for hand shake, coarse
    // enough that the attitude's held-then-jumping samples average out.
    const STEP: f64 = 0.02;
    writeln!(out, "t,heading,tilt,turn,rotation,shake,loudness,peak{}", if image { ",sky,skyline,open" } else { "" })?;
    let mut heading = None::<f64>;
    let rows = (duration / every).ceil() as usize;
    for n in 0..rows {
        let t = n as f64 * every;
        let mut level = levels.get(n).map_or(String::from(","), |(rms, peak)| format!("{rms:.1},{peak:.1}"));
        if image {
            // The keyframe nearest the row's middle, if there's one in it.
            let mid = t + every / 2.0;
            match skies.iter().filter(|(at, _)| (at - mid).abs() <= every.max(1.0) / 2.0).min_by(|a, b| (a.0 - mid).abs().total_cmp(&(b.0 - mid).abs())) {
                Some((_, s)) => level += &format!(",{:.1},{:.1},{:.0}", s.sky, s.skyline, s.open),
                None => level += ",,,",
            }
        }
        let Some(stab) = &stab else {
            writeln!(out, "{t:.2},,,,,,{level}")?;
            continue;
        };
        let poses: Vec<_> = (0..=(every / STEP).round() as usize).map(|k| stab.pose((t + k as f64 * STEP) * 1000.0)).collect();
        let (first, last) = (&poses[0], &poses[poses.len() - 1]);
        let path: f64 = poses.windows(2).map(|w| w[0].turn_to(&w[1])).sum();
        // Unwrapped, so a full turn reads 360 rather than jumping back.
        let mut h = first.heading;
        if let Some(prev) = heading {
            h = prev + (h - prev + 540.0).rem_euclid(360.0) - 180.0;
        }
        heading = Some(h);
        let turn = (last.heading - first.heading + 540.0).rem_euclid(360.0) - 180.0;
        let rotation = path / every;
        let shake = (path - first.turn_to(last)) / every;
        writeln!(out, "{t:.2},{h:.1},{:.1},{:.1},{rotation:.1},{shake:.1},{level}", first.tilt, turn / every)?;
    }
    Ok(())
}

/// The sky around the camera at the proxy's keyframes (about every 0.7 s),
/// level. Keyframes alone decode fast; the nearest one stands for a row.
pub(crate) fn skies(input: &Path) -> Result<Vec<(f64, sky::Sky)>> {
    const WIDTH: usize = 360;
    const LENS: usize = 256;
    // The seam corrections measure at full strip size whatever the output's;
    // at 1° a pixel their few pixels at the seam don't change the sky.
    let opts = StitchOpts { no_rs: true, no_align: true, no_cut: true, ..StitchOpts::default() };
    let p = Pipeline::new(input, &opts, stab::Mode::Horizon)?;
    let proxy = ffmpeg::proxy(input).context("scan --image reads the LRF proxy beside the clip, and there is none")?;
    let mut skies = Vec::new();
    ffmpeg::key_frames(&proxy, 0, (LENS * p.tracks.len(), LENS), ffmpeg::Out::default(), |t, frame| {
        let frames = ffmpeg::split_proxy(&frame, &p.tracks);
        let pic = p.render(&frames, t, &mut Carry::default(), &Shot::Equirect(WIDTH));
        skies.push((t, sky::measure(&pic.rgb, WIDTH)));
        true
    })?;
    Ok(skies)
}

pub(crate) fn seam_report(input: &Path, start: f64, duration: f64, dump: Option<&Path>) -> Result<()> {
    let opts = StitchOpts { no_rs: true, ..StitchOpts::default() };
    let p = Pipeline::new(input, &opts, stab::Mode::Off)?;
    let aligner = p.aligner.as_ref().context("not a two-lens rig")?;
    let start = p.snap(start);
    let mut stream = p.stream(start)?;
    let (mut eased, mut prev_raw): (Option<align::Field>, Option<align::Field>) = (None, None);
    let mut sums = [0.0; 7];
    // The selfie stick runs down the rig's y axis.
    let stick = aligner.column([0.0, 1.0, 0.0]);
    let mut n = 0;
    let jitter = |a: &align::Field, b: &align::Field| a.shift.iter().zip(&b.shift).map(|(x, y)| (x - y).abs() as f64).sum::<f64>() / a.shift.len() as f64;
    println!("t,mismatch_none,mismatch_raw,mismatch_eased,jitter_raw_deg,jitter_eased_deg,stick_none,stick_eased");
    while n < (duration * p.fps).round() as usize {
        let Some(frames) = stream.next()? else { break };
        let strips = aligner.strips(|i, u, v| Some(p.lenses[i].sample(&frames, u, v).iter().sum::<f64>() / (3.0 * 255.0)));
        let raw = aligner.measure(&strips, eased.as_ref());
        let before = eased.clone();
        let e = eased.get_or_insert_with(|| raw.clone());
        e.ease(&raw, align::EASE);
        let row = [
            aligner.mismatch(&strips, &align::Field::default(), None),
            aligner.mismatch(&strips, &raw, None),
            aligner.mismatch(&strips, e, None),
            prev_raw.as_ref().map_or(0.0, |r| jitter(r, &raw).to_degrees()),
            before.as_ref().map_or(0.0, |b| jitter(b, e).to_degrees()),
            aligner.mismatch(&strips, &align::Field::default(), Some(stick)),
            aligner.mismatch(&strips, e, Some(stick)),
        ];
        println!("{:.3},{:.5},{:.5},{:.5},{:.4},{:.4},{:.5},{:.5}", start + n as f64 / p.fps, row[0], row[1], row[2], row[3], row[4], row[5], row[6]);
        for (s, v) in sums.iter_mut().zip(row) {
            *s += v;
        }
        if n == 0
            && let Some(dir) = dump
        {
            aligner.dump(&strips, &raw, dir)?;
            eprintln!("rig down is strip column {}, rig up column {}", aligner.column([0.0, 1.0, 0.0]), aligner.column([0.0, -1.0, 0.0]));
        }
        prev_raw = Some(raw);
        n += 1;
    }
    let m = sums.map(|s| s / n.max(1) as f64);
    eprintln!(
        "{n} frames: mismatch none {:.5} raw {:.5} eased {:.5}; jitter raw {:.4}° eased {:.4}°; stick zone none {:.5} eased {:.5}",
        m[0], m[1], m[2], m[3], m[4], m[5], m[6]
    );
    Ok(())
}
