//! A Dailix project's footage as the contact sheet sees it: the clips the
//! analysis found (`dailix/analysis/<day>/<clip>.footage.json`, a clip's
//! sidecar: its file and length), day by day, each with its file under
//! `01_footage` and its poster's place (`Layout`).

use crate::Layout;
use serde::Deserialize;
use std::path::PathBuf;

/// One clip: its sheet key, its file, its length, its poster's place.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// `<day>/<file>`, as the sheet keys it.
    pub key: String,
    pub file: String,
    pub path: PathBuf,
    pub duration: f64,
    /// Made when first shown; maybe not there yet.
    pub poster: PathBuf,
}

/// A day's clips, in the order they were filmed (by name: cameras number
/// and time-stamp their files).
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub day: String,
    pub clips: Vec<Clip>,
}

/// A project's footage.
#[derive(Debug, Clone, PartialEq)]
pub struct Footage {
    pub layout: Layout,
    pub days: Vec<Day>,
}

/// The parts of a sidecar this reads.
#[derive(Deserialize)]
struct Sidecar {
    source: String,
    day: String,
    technical: Technical,
}

#[derive(Deserialize)]
struct Technical {
    duration_s: f64,
}

/// Reads the footage of the project `layout`. A sidecar that can't be
/// read is skipped (and said, in the second value), not fatal: one bad
/// file shouldn't hide a trip.
pub fn read(layout: &Layout) -> std::io::Result<(Footage, Vec<String>)> {
    let analysis = layout.analysis();
    let mut days: Vec<Day> = Vec::new();
    let mut skipped = Vec::new();
    let mut day_dirs: Vec<PathBuf> = std::fs::read_dir(&analysis)?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    day_dirs.sort();
    for dir in day_dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(".footage.json")).collect();
        files.sort();
        let mut clips = Vec::new();
        for f in files {
            let sidecar: Sidecar = match std::fs::read(&f).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())) {
                Ok(s) => s,
                Err(e) => {
                    skipped.push(format!("{}: {e}", f.display()));
                    continue;
                }
            };
            let key = format!("{}/{}", sidecar.day, sidecar.source);
            clips.push(Clip { path: layout.clip(&key), poster: layout.poster(&key), key, file: sidecar.source, duration: sidecar.technical.duration_s });
        }
        if let Some(day) = clips.first().map(|c| c.key.split('/').next().unwrap_or_default().to_owned()) {
            days.push(Day { day, clips });
        }
    }
    Ok((Footage { layout: layout.clone(), days }, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn days_and_clips_come_from_the_sidecars() {
        let root = std::env::temp_dir().join(format!("dailix-footage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::new(&root);
        let sidecar = |day: &str, src: &str, d: f64| {
            let dir = layout.analysis().join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let stem = Path::new(src).file_stem().unwrap().to_string_lossy().into_owned();
            std::fs::write(dir.join(format!("{stem}.footage.json")), format!(r#"{{"source": "{src}", "day": "{day}", "technical": {{"duration_s": {d}}}}}"#)).unwrap();
        };
        sidecar("2030-05-02", "B.MP4", 3.0);
        sidecar("2030-05-01", "A2.MP4", 12.5);
        sidecar("2030-05-01", "A1.MP4", 7.0);
        std::fs::write(layout.analysis().join("2030-05-01/broken.footage.json"), "{").unwrap();
        let (f, skipped) = read(&layout).unwrap();
        assert_eq!(f.days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(), ["2030-05-01", "2030-05-02"]);
        let first = &f.days[0].clips[0];
        assert_eq!((first.key.as_str(), first.duration), ("2030-05-01/A1.MP4", 7.0));
        assert_eq!(first.path, root.join("01_footage/2030-05-01/A1.MP4"));
        assert_eq!(first.poster, root.join("dailix/thumbs/2030-05-01/A1.jpg"));
        assert_eq!(f.days[0].clips[1].file, "A2.MP4");
        assert_eq!(skipped.len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
