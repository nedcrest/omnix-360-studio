//! A Dailix project's footage as the contact sheet sees it: the clips the
//! analysis found (`dailix/analysis/<day>/<clip>.footage.json`, a clip's
//! sidecar: its file and length), day by day, each with its file under
//! `01_footage` and its poster's place (`Layout`).

use crate::Layout;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    technical: Length,
}

/// The part of a sidecar's technical facts this reads.
#[derive(Deserialize)]
struct Length {
    duration_s: f64,
}

/// What a clip added to a trip (`dailix add`) says of itself in its
/// sidecar, in the shape footage-agent's analysis wrote: enough for the
/// contact sheet (its length) and for a later analysis to fill in.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Technical {
    pub duration_s: f64,
    pub fps: f64,
    /// `<width>x<height>`, as stored (a phone's portrait video is landscape
    /// turned by its metadata).
    pub resolution: String,
    pub codec: String,
    pub size_bytes: u64,
    pub has_audio: bool,
    /// When it was filmed, as the file says (ISO 8601), if it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Where clip `source` of `day` keeps its sidecar.
pub fn sidecar_path(layout: &Layout, day: &str, source: &str) -> PathBuf {
    let stem = Path::new(source).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    layout.analysis().join(day).join(format!("{stem}.footage.json"))
}

/// Writes clip `source`'s sidecar for `day`: the file, the day, `tech`,
/// and one shot over the whole of it (no analysis yet). Where it went.
pub fn write_sidecar(layout: &Layout, day: &str, source: &str, tech: &Technical) -> std::io::Result<PathBuf> {
    let path = sidecar_path(layout, day, source);
    let shot = serde_json::json!({ "start_s": 0.0, "end_s": tech.duration_s, "scene": [], "people": [], "objects": [], "speech": null, "quality": null });
    let doc = serde_json::json!({ "source": source, "technical": tech, "proxy": null, "day": day, "shots": [shot], "samples": [] });
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&doc).map_err(std::io::Error::other)?)?;
    Ok(path)
}

/// The day a clip was filmed: from its name first (cameras and phones
/// write the local date there: `DJI_20260819_110507…`, `20260818_202527`),
/// else from the date it says it was made (`2026-08-19T…`, UTC, so near
/// midnight a day off: the name is better).
pub fn day_of(file_name: &str, created_at: Option<&str>) -> Option<String> {
    let digits: Vec<char> = file_name.chars().collect();
    for i in 0..digits.len().saturating_sub(7) {
        let w: String = digits[i..i + 8].iter().collect();
        let before = i == 0 || !digits[i - 1].is_ascii_digit();
        let after = digits.get(i + 8).is_none_or(|c| !c.is_ascii_digit());
        if before && after && w.chars().all(|c| c.is_ascii_digit()) && (w.starts_with("19") || w.starts_with("20")) {
            let (y, m, d) = (&w[..4], &w[4..6], &w[6..8]);
            if ("01"..="12").contains(&m) && ("01"..="31").contains(&d) {
                return Some(format!("{y}-{m}-{d}"));
            }
        }
    }
    let c = created_at?;
    (c.len() >= 10 && c.as_bytes()[4] == b'-' && c.as_bytes()[7] == b'-').then(|| c[..10].to_owned())
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
    fn a_clips_day_comes_from_its_name_else_from_when_it_was_made() {
        assert_eq!(day_of("DJI_20260819_110507_13_null_video.mp4", None).as_deref(), Some("2026-08-19"));
        assert_eq!(day_of("20260818_202527.mp4", Some("2026-08-18T23:59:00Z")).as_deref(), Some("2026-08-18"));
        assert_eq!(day_of("DJI_0021.MP4", Some("2026-08-22T09:15:00.000000Z")).as_deref(), Some("2026-08-22"));
        assert_eq!(day_of("dji_mimo_20260821_094704_1_1787309763736_video.MP4", None).as_deref(), Some("2026-08-21"), "not the long number");
        assert_eq!(day_of("clip.mp4", None), None);
    }

    #[test]
    fn a_written_sidecar_is_read_back_as_a_clip() {
        let root = std::env::temp_dir().join(format!("dailix-sidecar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::new(&root);
        let tech = Technical { duration_s: 12.5, fps: 29.97, resolution: "1920x1080".into(), codec: "hevc".into(), size_bytes: 9, has_audio: true, created_at: None };
        write_sidecar(&layout, "2030-05-01", "A.MP4", &tech).unwrap();
        let (footage, skipped) = read(&layout).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        assert_eq!((footage.days[0].day.as_str(), footage.days[0].clips[0].key.as_str(), footage.days[0].clips[0].duration), ("2030-05-01", "2030-05-01/A.MP4", 12.5));
        std::fs::remove_dir_all(&root).unwrap();
    }

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
