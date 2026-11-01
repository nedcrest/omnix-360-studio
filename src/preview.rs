//! Contact sheets of a project (`rosmo360 preview`): a row per cut, with the music's waveform.

use crate::{check, overlay, player, project, sheet, stitch, view};
use crate::{pipeline::{Shot, StitchOpts, Carry}, cut_pipelines};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

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
    let view::Aspect(w, h) = project.aspect.unwrap_or(view::Aspect(16, 9));
    let size = (tile, ((tile * h as usize / w as usize) / 2).max(8) * 2);
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
        let edit = |t: f64| offsets[n] + t - cut.start;
        for (r, row) in times.chunks(columns).enumerate() {
            let heading = if r == 0 {
                format!("{} #{} {} {:.1}-{:.1}", check::clock(offsets[n]), n + 1, clip_number(&cut.input), cut.start, cut.end)
            } else {
                format!("{} #{}", check::clock(edit(row[0])), n + 1)
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
                            let k = ((b - a) / player::music::WAVE_STEP).ceil().max(1.0) as usize;
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
    let overlays = overlay::Overlays::new(project.overlays.iter().chain(&project.text).collect(), size, Default::default(), project.duration())?;
    let tiles = shots
        .par_iter()
        .map(|shot| {
            let Some((n, i, t)) = *shot else { return Ok(stitch::Frame { width: size.0, height: size.1, rgb: vec![0; size.0 * size.1 * 3], deep: false }) };
            let p = &pipelines[i];
            let mut f = p.render(&p.stills(t)?, t, &mut Carry::default(), &Shot::View { camera: Box::new(project::camera_at(&project.cuts, n, t, &|f, t| p.aim(f, t))), size });
            let cut = &project.cuts[n];
            let edit = offsets[n] + t - cut.start;
            overlays.draw(&mut f, edit);
            sheet::label(&mut f, 4, 4, &format!("{} {t:.1}s", check::clock(edit)));
            Ok(f)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((sheet::compose_rows(&tiles, columns, &bands), shots.iter().flatten().count()))
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
        .map(|track| track.clips.iter().map(|c| player::music::Waveform::read(&c.input, false).inspect_err(|e| eprintln!("no waveform for {}: {e:#}", c.input.display())).ok()).collect())
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
                let peak = wave.peaks.get(((clip.start + t - clip.at) / player::music::WAVE_STEP) as usize).copied().unwrap_or(0.0);
                let level = player::music::gain(track.level_at(t)) * crate::mixdown::envelope(clip, track.crossfades(i), t, end) as f32;
                loudest = loudest.max(peak * level);
            }
        }
        loudest
    })
}
