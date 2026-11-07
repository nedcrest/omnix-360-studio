//! Files: a replacement that lands whole or not at all, the clips under
//! folders given, and where a program keeps its settings and cache.

use crate::stitch;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// The name of Omnix's own folders, and the one they had before the
/// rename (gap 76): moved over once, so nothing set or cached is lost.
const APP: &str = "omnix";
const OLD_APP: &str = "rosmo360";

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

/// Dailix's name for its folders: Omnix reads its projects place.
pub const DAILIX: &str = "dailix";

/// Dailix's settings file. Dailix saves it here and Omnix reads it: one
/// place says where.
pub fn dailix_settings() -> Option<PathBuf> {
    Some(app_dir(AppDir::Config, DAILIX)?.join("settings.json"))
}

/// The projects place in Dailix's settings, as its file's bytes hold it
/// (Dailix's test checks it reads what Dailix writes).
pub fn projects_place_in(settings: &[u8]) -> Option<PathBuf> {
    serde_json::from_slice::<serde_json::Value>(settings).ok()?.get("projects")?.as_str().map(PathBuf::from)
}

/// Dailix's projects place: Omnix's own until one is chosen in Omnix
/// (user, 2026-11-07: each program its own, Omnix from Dailix's at first).
pub fn dailix_projects() -> Option<PathBuf> {
    projects_place_in(&std::fs::read(dailix_settings()?).ok()?)
}

/// Omnix's settings folder (settings, LUTs, the cache's place).
pub(crate) fn config_dir() -> Option<PathBuf> {
    own_dir(AppDir::Config)
}

/// Omnix's cache folder when none is chosen.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    own_dir(AppDir::Cache)
}

/// Omnix's folder of `kind`, the old name's moved to it the first time
/// (once a run); if the move fails, the old one is used as it is.
fn own_dir(kind: AppDir) -> Option<PathBuf> {
    let (new, old) = (app_dir(kind, APP)?, app_dir(kind, OLD_APP)?);
    Some(moved_over(&old, new))
}

/// `new`, with `old` renamed to it when only `old` is there; `old` if that
/// fails.
fn moved_over(old: &Path, new: PathBuf) -> PathBuf {
    if !new.exists() && old.is_dir() && std::fs::rename(old, &new).is_err() {
        return old.to_owned();
    }
    new
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
    fn the_old_folder_is_moved_to_the_new_name_once() {
        let base = std::env::temp_dir().join(format!("omnix-rename-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (old, new) = (base.join("rosmo360"), base.join("omnix"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("settings.json"), b"{}").unwrap();
        assert_eq!(moved_over(&old, new.clone()), new);
        assert!(new.join("settings.json").is_file() && !old.exists());
        // Both there (an old copy came back): the new one wins, untouched.
        std::fs::create_dir_all(&old).unwrap();
        assert_eq!(moved_over(&old, new.clone()), new);
        assert!(old.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

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

/// Each path given that is a file, and every .OSV in the folders given (and
/// the folders inside), in name order.
pub(crate) fn osv_under(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    fn walk(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        if path.is_dir() {
            let mut inside: Vec<_> = std::fs::read_dir(path).with_context(|| format!("reading {}", path.display()))?.flatten().map(|e| e.path()).collect();
            inside.sort();
            for p in inside {
                if p.is_dir() || p.extension().is_some_and(|e| e.eq_ignore_ascii_case("osv")) {
                    walk(&p, out)?;
                }
            }
        } else if path.exists() {
            out.push(path.to_owned());
        } else {
            bail!("{} does not exist", path.display());
        }
        Ok(())
    }
    let mut clips = Vec::new();
    for p in paths {
        walk(p, &mut clips)?;
    }
    Ok(clips)
}
