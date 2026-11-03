//! A blank stretch on the main lane (gap 32): a black picture added as a
//! cut like any other, so the edit stays magnetic and a gap is something
//! you can see, trim and move. Room for a title, an overlay or a sound.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The file's name: its stem is what the timeline writes on the cut.
pub const FILE: &str = "Blank.png";

/// Small: it is scaled to the frame and is black all over anyway.
const SIZE: (u32, u32) = (320, 180);

/// `Blank.png` in `dir`, written if it is not there yet. One file serves
/// every blank cut; its length is the cut's, as for any picture.
pub fn ensure(dir: &Path) -> Result<PathBuf> {
    let path = dir.join(FILE);
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    let black = image::RgbImage::new(SIZE.0, SIZE.1);
    let mut png = Vec::new();
    black.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    crate::fsutil::replace_file(&path, &png)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_blank_is_written_once_and_is_black() {
        let dir = std::env::temp_dir().join(format!("omnix-blank-{}", std::process::id()));
        let path = super::ensure(&dir).unwrap();
        let picture = image::open(&path).unwrap().to_rgb8();
        assert!(picture.pixels().all(|p| p.0 == [0, 0, 0]));
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(super::ensure(&dir).unwrap(), path);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), written, "kept, not rewritten");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
