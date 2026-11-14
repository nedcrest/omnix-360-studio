//! Project cards (gap 110, `docs/omnix/PROJECT-CARDS.md`): what an edit
//! holds, shown on the start pages as frames. An edit's 16 frames are
//! rendered as the film plays (stitched, framed, graded, its titles in)
//! and kept as one JPEG strip, side by side: beside its trip's data for a
//! video of a trip, in Omnix's cache otherwise. Made off the window's
//! thread, again whenever the edit is newer than its strip. Dailix makes
//! and reads them through here too.

pub mod view;

use crate::{fsutil, project};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Frames on a card.
pub const FRAMES: usize = 16;

/// The frames a card shows at rest, one from each quarter of the film.
pub const AT_REST: [usize; 4] = [1, 5, 9, 13];

/// A frame's width in the strip.
const WIDTH: usize = 320;

/// The edit times of a film's frames: the middle of each sixteenth, so
/// none falls on a fade from black at either end.
pub fn times(duration: f64) -> Vec<f64> {
    (0..FRAMES).map(|k| duration * (k as f64 + 0.5) / FRAMES as f64).collect()
}

/// Where edit `edit`'s strip is kept: beside its trip's data when it's a
/// video of a trip; else in Omnix's cache, named after the edit's path.
pub fn strip_path(edit: &Path) -> Option<PathBuf> {
    if let Some((root, video)) = dailix_sheet::project::video_of(edit) {
        return Some(dailix_sheet::layout::Layout::new(root).card(&video));
    }
    let edit = std::fs::canonicalize(edit).unwrap_or_else(|_| edit.to_owned());
    let name: String = edit.to_string_lossy().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect();
    Some(fsutil::app_dir(fsutil::AppDir::Cache, "omnix")?.join("cards").join(format!("{name}.jpg")))
}

/// Whether a strip changed at `strip` (none: there's none) is to be made
/// again for an edit changed at `edit`.
pub fn stale(edit: Option<SystemTime>, strip: Option<SystemTime>) -> bool {
    match (edit, strip) {
        (_, None) => true,
        (Some(e), Some(s)) => e > s,
        (None, Some(_)) => false,
    }
}

/// Whether edit `edit`'s strip is missing or older than the edit.
pub fn needs_making(edit: &Path) -> bool {
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    strip_path(edit).is_some_and(|s| stale(modified(edit), modified(&s)))
}

/// Renders edit `edit`'s frames and writes its strip (whole or not at
/// all); where it went. Slow (each frame is stitched): off the window's
/// thread.
pub fn make(edit: &Path) -> Result<PathBuf> {
    let path = strip_path(edit).context("no place to keep the card's frames")?;
    let p = project::Project::load(edit)?;
    let opts = crate::pipeline::StitchOpts::default();
    let frames: Vec<crate::stitch::Frame> = times(p.duration()).into_iter().map(|t| crate::preview::frame_at(&p, &opts, t, WIDTH, true)).collect::<Result<_>>()?;
    let jpeg = strip(&frames)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fsutil::replace_file(&path, &jpeg)?;
    Ok(path)
}

/// `frames` side by side, as JPEG: each at the first's size.
fn strip(frames: &[crate::stitch::Frame]) -> Result<Vec<u8>> {
    let (w, h) = frames.first().map(|f| (f.width, f.height)).context("no frames")?;
    let mut rgb = vec![0u8; w * frames.len() * h * 3];
    for (k, f) in frames.iter().enumerate() {
        anyhow::ensure!((f.width, f.height, f.deep) == (w, h, false), "the frames differ in size");
        for y in 0..h {
            let row = (y * w * frames.len() + k * w) * 3;
            rgb[row..row + w * 3].copy_from_slice(&f.rgb[y * w * 3..(y + 1) * w * 3]);
        }
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode(&rgb, (w * frames.len()) as u32, h as u32, image::ExtendedColorType::Rgb8)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cards_frames_are_spread_over_the_film_clear_of_its_ends() {
        let t = times(16.0);
        assert_eq!((t.len(), t[0], t[15]), (FRAMES, 0.5, 15.5));
        assert!(AT_REST.iter().all(|&k| k < FRAMES) && AT_REST.windows(2).all(|w| w[1] - w[0] == FRAMES / 4), "one from each quarter");
    }

    #[test]
    fn a_strip_is_made_again_only_when_the_edit_is_newer() {
        let (old, new) = (SystemTime::UNIX_EPOCH, SystemTime::now());
        assert!(stale(Some(new), None) && stale(Some(new), Some(old)));
        assert!(!stale(Some(old), Some(new)) && !stale(None, Some(new)));
    }

    #[test]
    fn a_trips_video_keeps_its_card_beside_the_trips_data() {
        let root = std::env::temp_dir().join(format!("omnix-cards-{}", std::process::id()));
        let l = dailix_sheet::layout::Layout::new(&root);
        l.create().unwrap();
        std::fs::write(l.project(), "{}").unwrap();
        assert_eq!(strip_path(&l.video_edit("Day one")), Some(root.join("dailix/cards/Day one.jpg")));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn frames_go_side_by_side_into_one_picture() {
        let frame = |v: u8| crate::stitch::Frame { width: 4, height: 2, rgb: vec![v; 4 * 2 * 3], deep: false };
        let jpeg = strip(&[frame(0), frame(255)]).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (8, 2));
        assert!(img.get_pixel(1, 1)[0] < 30 && img.get_pixel(6, 1)[0] > 225, "dark left, light right");
    }
}
