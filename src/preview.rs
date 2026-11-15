//! Contact sheets of a project (`omnix preview`): a row per cut, with the music's waveform.

use crate::{overlay, project, sheet, stitch, view};
use crate::{pipeline::{Shot, StitchOpts, Carry}, cut_pipelines};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::path::Path;

/// A tile `tile` wide in `aspect`'s shape (16:9 without one): its height,
/// even and at least 8.
fn tile_height(tile: usize, aspect: Option<view::Aspect>) -> usize {
    let view::Aspect(w, h) = aspect.unwrap_or(view::Aspect(16, 9));
    ((tile * h as usize / w as usize) / 2).max(8) * 2
}

/// A contact sheet of one clip (`omnix preview <clip>`): a tile every
/// `every` seconds, or at each of `reframe`'s keyframes, or 24 spread over
/// its `span` (start, duration); each the whole sphere with its grid, or
/// framed by `reframe`; labelled with its time. The sheet and the times.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_clip(input: &Path, reframe: Option<&view::Reframe>, opts: &StitchOpts, (start, duration): (Option<f64>, Option<f64>), every: Option<f64>, tile: usize, columns: usize, proxy: bool) -> Result<(stitch::Frame, Vec<f64>)> {
    let mut p = crate::Pipeline::new(input, opts, crate::stab::Mode::Horizon)?;
    if proxy {
        p.use_proxy();
    }
    let times: Vec<f64> = match (every, reframe) {
        (Some(every), _) => {
            let (a, b) = p.span(start, duration, reframe)?;
            (0..).map(|n| a + n as f64 * every.max(1.0 / p.fps)).take_while(|&t| t < b).collect()
        }
        (None, Some(r)) if start.is_none() && duration.is_none() && r.keyframes.len() > 1 => r.keyframes.iter().map(|k| k.t).collect(),
        _ => {
            let (a, b) = p.span(start, duration, None)?;
            (0..24).map(|n| a + (n as f64 + 0.5) * (b - a) / 24.0).collect()
        }
    };
    let times: Vec<f64> = times.into_iter().map(|t| p.snap(t)).collect();
    let shot = |t: f64| match reframe {
        None => Shot::Equirect(tile),
        Some(r) => Shot::View { camera: Box::new(r.camera_at(t, &|f, t| p.aim(f, t))), size: (tile, tile_height(tile, r.aspect)) },
    };
    let tiles = times
        .par_iter()
        .map(|&t| {
            let mut f = p.render(&p.stills(t)?, t, &mut Carry::default(), &shot(t));
            if reframe.is_none() {
                sheet::equirect_grid(&mut f);
            }
            sheet::label(&mut f, 4, 4, &format!("{t:.1}s"));
            Ok(f)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((sheet::compose(&tiles, columns), times))
}

/// A contact sheet of a project: per cut, `columns` moments spread across
/// it (or one every `every` seconds), framed as `export` frames them. Each
/// cut starts a row, headed by its edit time, number, clip and span
/// (`2:16 #23 0036 18.0-25.0`); each tile says its edit and clip time and
/// shows the overlays and text there, and with music the mix's waveform
/// runs under each row, lined up with the
/// tiles. Blank tiles fill out the last. Returns the sheet and how many
/// tiles it shows.
pub(crate) fn preview_project(project: &project::Project, opts: &StitchOpts, every: Option<f64>, tile: usize, columns: usize, proxy: bool) -> Result<(stitch::Frame, usize)> {
    let mut project = project.clone();
    let (pipelines, of_cut) = cut_pipelines(&mut project, opts, proxy)?;
    let project = &project;
    let size = (tile, tile_height(tile, project.aspect));
    let offsets = project::offsets(&project.cuts);
    let music = sheet_music(project);
    let width = sheet::width(tile, columns);
    let mut shots = Vec::new();
    let mut bands = Vec::new();
    for (n, (cut, &i)) in project.cuts.iter().zip(&of_cut).enumerate() {
        let p = &pipelines[i];
        let len = cut.end - cut.start;
        let (times, step): (Vec<f64>, f64) = match every {
            Some(every) => {
                let every = every.max(1.0 / p.fps);
                ((0..).map(|k| cut.start + k as f64 * every).take_while(|&t| t < cut.end).collect(), every)
            }
            None => ((0..columns).map(|k| cut.start + (k as f64 + 0.5) * len / columns as f64).collect(), len / columns as f64),
        };
        let edit = |t: f64| offsets[n] + cut.edit_at(t);
        for (r, row) in times.chunks(columns).enumerate() {
            let heading = if r == 0 {
                format!("{} #{} {} {:.1}-{:.1}", dailix_sheet::clock(offsets[n], 1), n + 1, clip_number(&cut.input), cut.start, cut.end)
            } else {
                format!("{} #{}", dailix_sheet::clock(edit(row[0]), 1), n + 1)
            };
            // Each pixel across the row stands for the edit time between
            // its tiles' times, as far as the cut goes.
            let wave = music.as_ref().map(|level| {
                let at = |x: f64| edit(row[0]) + (x - sheet::tile_middle(tile, 0)) / (sheet::tile_middle(tile, 1) - sheet::tile_middle(tile, 0)).max(1.0) * step;
                (0..width)
                    .map(|x| {
                        let (a, b) = (at(x as f64), at(x as f64 + 1.0));
                        let (from, to) = (offsets[n], offsets[n] + len);
                        (b > from && a < to).then(|| {
                            let k = ((b - a) / crate::waveform::WAVE_STEP).ceil().max(1.0) as usize;
                            (0..k).map(|j| level(a + (b - a) * j as f64 / k as f64)).fold(0.0, f32::max)
                        })
                    })
                    .collect()
            });
            bands.push(sheet::Band { heading, wave });
        }
        let row_end = times.len().div_ceil(columns) * columns;
        shots.extend(times.iter().map(|&t| Some((n, i, p.snap(t).clamp(cut.start, cut.end)))));
        shots.extend((times.len()..row_end).map(|_| None));
    }
    // Overlays and text as the export burns them in, so a title's place
    // shows on the sheet; the tiles leave out the edit's fades, so do they.
    let overlays = overlay::Overlays::new(project.shown_lanes().collect(), size, Default::default(), project.duration())?;
    let tiles = shots
        .par_iter()
        .map(|shot| {
            let Some((n, i, t)) = *shot else { return Ok(stitch::Frame { width: size.0, height: size.1, rgb: vec![0; size.0 * size.1 * 3], deep: false }) };
            let p = &pipelines[i];
            let mut f = p.render(&p.stills(t)?, t, &mut Carry::default(), &Shot::View { camera: Box::new(project::camera_at(&project.cuts, n, t, &|f, t| p.aim(f, t))), size });
            let cut = &project.cuts[n];
            let edit = offsets[n] + cut.edit_at(t);
            overlays.draw(&mut f, edit);
            sheet::label(&mut f, 4, 4, &format!("{} {t:.1}s", dailix_sheet::clock(edit, 1)));
            Ok(f)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((sheet::compose_rows(&tiles, columns, &bands), shots.iter().flatten().count()))
}

/// The edit's frame at edit time `t`, `width` wide in the edit's aspect,
/// as the export makes it (audit C10: the sheet's tiles leave out styles,
/// edge looks and fades): framed by its keys, graded with its LUT, styled,
/// its edge look, faded, with the overlays and text. A join's overlap
/// shows the cut under `t` alone. Only that cut's clip is opened.
pub(crate) fn frame_at(project: &project::Project, opts: &StitchOpts, t: f64, width: usize, proxy: bool) -> Result<stitch::Frame> {
    let mut project = project.clone();
    let (n, at) = project.locate(t).context("an edit with no cuts")?;
    let cut = &mut project.cuts[n];
    let mode = cut.stabilize.or(opts.stabilize).unwrap_or(crate::stab::Mode::Horizon);
    let mut p = crate::Pipeline::new(&cut.input, &StitchOpts { stabilize: Some(mode), ..opts.clone() }, mode)?;
    if proxy {
        p.use_proxy();
    }
    cut.flat = p.flat.is_some();
    if let Err(e) = cut.resolve_lut() {
        eprintln!("warning: {e:#}; {} is left as filmed", cut.input.display());
    }
    let project = &project;
    let cut = &project.cuts[n];
    let at = p.snap(at).clamp(cut.start, cut.end);
    let size = (width.max(2) / 2 * 2, tile_height(width, project.aspect));
    let mut camera = project::camera_at(&project.cuts, n, at, &|f, t| p.aim(f, t));
    camera.grade.black = project.black_at(n, at);
    let mut f = p.render(&p.stills(at)?, at, &mut Carry::default(), &Shot::View { camera: Box::new(camera), size });
    if let Some(s) = &cut.style {
        f = crate::transition::blend(&s.spec(), at as f32, &f, &f);
    }
    if let Some((spec, q)) = cut.edge_at(at) {
        f = crate::transition::blend(&spec, q, &f, &f);
    }
    overlay::Overlays::new(project.shown_lanes().collect(), size, project.fades, project.duration())?.draw(&mut f, t);
    Ok(f)
}

/// A clip's number for a sheet's headings, which draw only digits: the
/// last run of them in its name (`CAM_20260713153034_0037_D` → 0037).
pub(crate) fn clip_number(input: &Path) -> String {
    let stem = input.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let digits: Vec<&str> = stem.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).collect();
    digits.last().map_or_else(String::new, |d| d.to_string())
}

/// The music's and sound effects' loudness at edit time, 0..1, as the
/// export mixes them (each file's peaks at its track's level, fades and
/// crossfades), or `None` without any. Files that can't be read are left
/// out, with a note.
pub(crate) fn sheet_music(project: &project::Project) -> Option<impl Fn(f64) -> f32 + '_> {
    let tracks: Vec<_> = project::SoundTrack::ALL.iter().map(|&t| project.track(t)).filter(|t| !t.clips.is_empty()).collect();
    if tracks.is_empty() {
        return None;
    }
    let waves: Vec<Vec<_>> = tracks
        .iter()
        .map(|track| track.clips.iter().map(|c| crate::waveform::Waveform::read(&c.input, false).inspect_err(|e| eprintln!("no waveform for {}: {e:#}", c.input.display())).ok()).collect())
        .collect();
    let end = project::edit_length(&project.cuts);
    Some(move |t: f64| {
        let mut loudest = 0.0f32;
        for (track, waves) in tracks.iter().zip(&waves) {
            for (i, (clip, wave)) in track.clips.iter().zip(waves).enumerate() {
                let Some(wave) = wave else { continue };
                if t < clip.at || t >= clip.ends() {
                    continue;
                }
                let peak = wave.peaks.get((clip.local(t) / crate::waveform::WAVE_STEP) as usize).copied().unwrap_or(0.0);
                let level = crate::mixdown::clip_gain(track, i, t, end) as f32;
                loudest = loudest.max(peak * level);
            }
        }
        loudest
    })
}
