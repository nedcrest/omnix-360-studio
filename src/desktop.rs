//! A program's icon and desktop entry, in the menu and on the desktop,
//! starting this very binary: `omnix install`, `dailix install`. Linux only
//! for now (freedesktop's icon theme and `.desktop` files); elsewhere it
//! says so. One function for both programs.

use std::path::PathBuf;

/// The entry `packaging/omnix.desktop` describes, starting `exe`: its own
/// `Exec` line swapped for one that runs this binary's player.
pub fn omnix_entry(exe: &std::path::Path) -> String {
    include_str!("../packaging/omnix.desktop").lines().map(|l| if l.starts_with("Exec=") { format!("Exec=\"{}\" play %F", exe.display()) } else { l.to_owned() }).collect::<Vec<_>>().join("\n") + "\n"
}

/// Omnix's icon.
pub const OMNIX_ICON: &str = include_str!("../packaging/omnix.svg");

/// Writes the icon `svg` as `<id>.svg` into the icon theme and `entry` as
/// `<id>.desktop` into the menu and onto the desktop; what was written.
#[cfg(target_os = "linux")]
pub fn install(id: &str, entry: &str, svg: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("no HOME")?);
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let mut written = Vec::new();
    let icons = data.join("icons/hicolor/scalable/apps");
    std::fs::create_dir_all(&icons)?;
    std::fs::write(icons.join(format!("{id}.svg")), svg)?;
    written.push(icons.join(format!("{id}.svg")));
    let desktop = std::process::Command::new("xdg-user-dir").arg("DESKTOP").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).map(|s| PathBuf::from(s.trim())).filter(|p| p.is_dir()).unwrap_or_else(|| home.join("Desktop"));
    for dir in [data.join("applications"), desktop] {
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!("{id}.desktop"));
        std::fs::write(&file, entry)?;
        // GNOME launches a desktop file only once it's executable and
        // marked trusted (the system's gio: a Homebrew one lacks gvfs).
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        let gio = if std::path::Path::new("/usr/bin/gio").exists() { "/usr/bin/gio" } else { "gio" };
        let _ = std::process::Command::new(gio).args(["set", "-t", "string"]).arg(&file).args(["metadata::trusted", "true"]).status();
        written.push(file);
    }
    let _ = std::process::Command::new("gtk-update-icon-cache").args(["-f", "-t"]).arg(data.join("icons/hicolor")).status();
    Ok(written)
}

#[cfg(not(target_os = "linux"))]
pub fn install(id: &str, _entry: &str, _svg: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    Err(format!("{id} install makes a Linux desktop entry; on this system, pin the binary yourself").into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn omnixs_entry_runs_this_binary_and_keeps_the_rest() {
        let e = super::omnix_entry(std::path::Path::new("/opt/x/omnix"));
        assert!(e.contains("Exec=\"/opt/x/omnix\" play %F\n"), "{e}");
        assert!(e.contains("Icon=omnix\n") && e.contains("StartupWMClass=Omnix\n") && e.contains("MimeType="));
        assert_eq!(e.matches("Exec=").count(), 1);
    }
}
