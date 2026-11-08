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
}

pub const OMNIX: Program = Program { bin: "omnix", var: "OMNIX_BIN", repo: "omnix-360-studio" };
pub const DAILIX: Program = Program { bin: "dailix", var: "DAILIX_BIN", repo: "dailix" };

/// `p`'s binary: its variable, else on PATH, else a release build in its
/// repo beside this binary's (`<repos>/<repo>/target/release`).
pub fn locate(p: Program) -> Option<PathBuf> {
    locate_from(p, std::env::var_os(p.var).map(PathBuf::from), std::env::var_os("PATH"), std::env::current_exe().ok())
}

fn locate_from(p: Program, env_bin: Option<PathBuf>, path: Option<OsString>, exe: Option<PathBuf>) -> Option<PathBuf> {
    let name = format!("{}{}", p.bin, std::env::consts::EXE_SUFFIX);
    let on_path = || path.as_ref().and_then(|ps| std::env::split_paths(ps).map(|d| d.join(&name)).find(|f| f.is_file()));
    let beside = || {
        let repo = exe.as_deref()?.parent()?.parent()?.parent()?;
        Some(repo.parent()?.join(p.repo).join("target/release").join(&name)).filter(|f| f.is_file())
    };
    env_bin.filter(|f| f.is_file()).or_else(on_path).or_else(beside)
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
        assert_eq!(locate_from(OMNIX, Some(set.clone()), path.clone(), exe.clone()), Some(set));
        assert_eq!(locate_from(OMNIX, Some(base.join("gone")), path, exe.clone()), Some(on_path), "a variable naming nothing is passed over");
        assert_eq!(locate_from(OMNIX, None, None, exe), Some(beside));
        assert_eq!(locate_from(OMNIX, None, None, None), None);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
