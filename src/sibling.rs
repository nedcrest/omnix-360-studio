//! Finding and starting the other program of the pair (Omnix from Dailix,
//! Dailix from Omnix): one way for both.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A program of the pair: its binary's name, the variable that may name
/// it, and its repo's folder beside this one's (for working on both).
#[derive(Clone, Copy, Debug)]
pub struct Program {
    pub bin: &'static str,
    pub var: &'static str,
    pub repo: &'static str,
    /// Its AppImage's name, as the release names it.
    pub appimage: &'static str,
}

pub const OMNIX: Program = Program { bin: "omnix", var: "OMNIX_BIN", repo: "omnix-360-studio", appimage: "Omnix-x86_64.AppImage" };
pub const DAILIX: Program = Program { bin: "dailix", var: "DAILIX_BIN", repo: "dailix", appimage: "Dailix-x86_64.AppImage" };

/// Where this program runs from: its variable for the other, PATH, its
/// own binary, and the AppImage it was started from (`APPIMAGE`).
struct Here {
    env_bin: Option<PathBuf>,
    path: Option<OsString>,
    exe: Option<PathBuf>,
    appimage: Option<PathBuf>,
}

/// `p`'s binary: its variable; else beside this binary (install.sh puts
/// both in one folder, on PATH or not); else beside this AppImage (the
/// two AppImages downloaded into one folder: gap 120); else on PATH;
/// else a release build in its repo beside this one's
/// (`<repos>/<repo>/target/release`, for working on both).
pub fn locate(p: Program) -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    locate_from(p, &Here { env_bin: var(p.var).map(PathBuf::from), path: var("PATH"), exe: std::env::current_exe().ok(), appimage: var("APPIMAGE").map(PathBuf::from) })
}

fn locate_from(p: Program, here: &Here) -> Option<PathBuf> {
    let name = format!("{}{}", p.bin, std::env::consts::EXE_SUFFIX);
    let exe = here.exe.as_deref();
    let own_dir = || Some(exe?.parent()?.join(&name)).filter(|f| f.is_file());
    let by_appimage = || Some(here.appimage.as_deref()?.parent()?.join(p.appimage)).filter(|f| f.is_file());
    let on_path = || here.path.as_ref().and_then(|ps| std::env::split_paths(ps).map(|d| d.join(&name)).find(|f| f.is_file()));
    let repo = || {
        let repo = exe?.parent()?.parent()?.parent()?;
        Some(repo.parent()?.join(p.repo).join("target/release").join(&name)).filter(|f| f.is_file())
    };
    here.env_bin.clone().filter(|f| f.is_file()).or_else(own_dir).or_else(by_appimage).or_else(on_path).or_else(repo)
}

/// Starts `p` with `args`, in a session of its own so it stays open when
/// this one closes; what went wrong, said plainly.
pub fn start(p: Program, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let bin = locate(p).ok_or_else(|| format!("{} wasn't found ({})", p.bin, p.var))?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.spawn().map(drop).map_err(|e| format!("{}: {e}", bin.display()))
}

/// Opens `edit` in Omnix.
pub fn open_in_omnix(edit: &Path) -> Result<(), String> {
    start(OMNIX, &["play".as_ref(), edit.as_os_str()]).map_err(|e| format!("{} not opened: {e}", edit.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_other_is_looked_for_in_its_variable_then_path_then_beside() {
        let base = std::env::temp_dir().join(format!("omnix-sibling-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let name = format!("omnix{}", std::env::consts::EXE_SUFFIX);
        let file = |p: PathBuf| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"").unwrap();
            p
        };
        let set = file(base.join("set").join(&name));
        let on_path = file(base.join("bin").join(&name));
        let beside = file(base.join("omnix-360-studio/target/release").join(&name));
        let exe = Some(base.join("dailix/target/release/dailix"));
        let path = Some(std::env::join_paths([base.join("empty"), base.join("bin")]).unwrap());
        let here = |env_bin: Option<PathBuf>, path: Option<OsString>, exe: Option<PathBuf>, appimage: Option<PathBuf>| Here { env_bin, path, exe, appimage };
        assert_eq!(locate_from(OMNIX, &here(Some(set.clone()), path.clone(), exe.clone(), None)), Some(set));
        assert_eq!(locate_from(OMNIX, &here(Some(base.join("gone")), path, exe.clone(), None)), Some(on_path), "a variable naming nothing is passed over");
        assert_eq!(locate_from(OMNIX, &here(None, None, exe, None)), Some(beside));
        assert_eq!(locate_from(OMNIX, &here(None, None, None, None)), None);
        // Installed side by side (install.sh: ~/.local/bin, maybe off PATH).
        let installed = file(base.join("local/bin").join(&name));
        assert_eq!(locate_from(OMNIX, &here(None, None, Some(base.join("local/bin/dailix")), None)), Some(installed));
        // Two AppImages in one folder: Dailix's runs from its mount, APPIMAGE says where it is.
        let image = file(base.join("Downloads/Omnix-x86_64.AppImage"));
        assert_eq!(locate_from(OMNIX, &here(None, None, Some(base.join("mount/usr/bin/dailix")), Some(base.join("Downloads/Dailix-x86_64.AppImage")))), Some(image));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
