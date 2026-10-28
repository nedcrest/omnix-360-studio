//! Writing files safely: a replacement lands whole or not at all.

use crate::stitch;
use anyhow::{Context, Result};
use std::path::Path;

/// Replaces `path` with `bytes` whole or not at all: written to a new file
/// beside it, synced, then renamed over it. Written in place, a crash or a
/// full disk mid-save left a project (or the settings) cut short. The new
/// file keeps the old one's permissions, and a symlink's target is what
/// gets replaced, not the link.
pub(crate) fn replace_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().context("no file name")?.to_string_lossy();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        // create_new: never through a link someone left at that name.
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        if let Ok(meta) = std::fs::metadata(&path) {
            file.set_permissions(meta.permissions())?;
        }
        file.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.with_context(|| format!("writing {}", path.display()))
}

pub(crate) fn save_png(path: &Path, f: &stitch::Frame) -> Result<()> {
    image::save_buffer(path, &f.rgb, f.width as u32, f.height as u32, image::ColorType::Rgb8).with_context(|| format!("writing {}", path.display()))
}
