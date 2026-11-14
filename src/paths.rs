//! A folder as the window shows it (gap 115, 2026-11-14): the home folder
//! as `~`, a long path cut in the middle. Omnix's bin and settings showed
//! a trip's whole path (`/home/<user>/…`), long and personal in a
//! screenshot; Dailix had these and uses them from here.

use std::path::Path;

/// `path` as people say it: the home folder as `~`.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => Path::new("~").join(rest).display().to_string(),
        _ => path.display().to_string(),
    }
}

/// `path` cut to `most` characters in the middle, so its start and its
/// last folder (the part people know) stay: `/home/someone/…/projects`.
pub fn middle(path: &str, most: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= most || most < 5 {
        return path.to_owned();
    }
    let tail = (most - 1) / 2 + (most - 1) % 2;
    let head = most - 1 - tail;
    chars[..head].iter().chain(['…'].iter()).chain(&chars[chars.len() - tail..]).collect()
}

/// `path` for the window: from this user's home as `~`.
pub fn shown(path: &Path) -> String {
    tilde(path, &std::env::home_dir().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_folder_is_said_as_a_tilde() {
        let home = Path::new("/home/someone");
        assert_eq!(tilde(Path::new("/home/someone/Dailix/projects"), home), "~/Dailix/projects");
        assert_eq!(tilde(Path::new("/media/disk/trips"), home), "/media/disk/trips");
        assert_eq!(tilde(Path::new("/home/someone2/x"), home), "/home/someone2/x", "not a folder of home");
    }

    #[test]
    fn a_long_path_is_cut_in_the_middle_keeping_its_last_folder() {
        assert_eq!(middle("/home/someone/Dailix/projects", 40), "/home/someone/Dailix/projects");
        let cut = middle("/media/someone/Big Disk/Trips/2026/everything/projects", 30);
        assert_eq!(cut.chars().count(), 30);
        assert!(cut.starts_with("/media/") && cut.ends_with("/projects") && cut.contains('…'), "{cut}");
    }
}
