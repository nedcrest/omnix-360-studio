//! Files: a replacement that lands whole or not at all, and where a
//! program keeps its settings and cache on each platform.

use crate::stitch;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The name of Omnix's own folders (the binary's; gap 76 renames both).
const APP: &str = "rosmo360";

/// Which of a program's folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppDir {
    /// Settings: `$XDG_CONFIG_HOME` or `~/.config` on Linux, `~/Library/
    /// Application Support` on macOS, `%APPDATA%` on Windows.
    Config,
    /// What can be made again: `$XDG_CACHE_HOME` or `~/.cache`,
    /// `~/Library/Caches`, `%LOCALAPPDATA%`.
    Cache,
}

/// The folder `app` keeps its settings or cache in, by the platform's
/// custom; none when the system says no home.
pub fn app_dir(kind: AppDir, app: &str) -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = match kind {
        _ if cfg!(windows) => env(if kind == AppDir::Config { "APPDATA" } else { "LOCALAPPDATA" })?,
        AppDir::Config if cfg!(target_os = "macos") => env("HOME")?.join("Library/Application Support"),
        AppDir::Cache if cfg!(target_os = "macos") => env("HOME")?.join("Library/Caches"),
        AppDir::Config => env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?,
        AppDir::Cache => env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))?,
    };
    Some(base.join(app))
}

/// Omnix's settings folder (settings, LUTs, the cache's place).
pub(crate) fn config_dir() -> Option<PathBuf> {
    app_dir(AppDir::Config, APP)
}

/// Omnix's cache folder when none is chosen.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    app_dir(AppDir::Cache, APP)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replacement_lands_whole_keeps_permissions_and_follows_a_link() {
        let dir = std::env::temp_dir().join(format!("omnix-replace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("edit.json");
        std::fs::write(&file, "old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
            std::os::unix::fs::symlink(&file, dir.join("link.json")).unwrap();
            // Through the link: the file it points at is replaced, the link stays.
            replace_file(&dir.join("link.json"), b"new").unwrap();
            assert!(std::fs::symlink_metadata(dir.join("link.json")).unwrap().file_type().is_symlink());
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o640);
        }
        #[cfg(not(unix))]
        replace_file(&file, b"new").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new");
        // A file that can't be written (its folder is gone): an error, and
        // no temporary file left behind.
        assert!(replace_file(&dir.join("gone/edit.json"), b"x").is_err());
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).filter(|n| n.to_string_lossy().ends_with(".tmp")).collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(left.is_empty(), "{left:?}");
    }
}
