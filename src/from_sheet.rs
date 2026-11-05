//! A Dailix project's marking made into an Omnix edit (`omnix
//! from-sheet`, and Dailix's «→ Omnix»): what footage-agent's «→ Omnix»
//! made from its web contact sheet (`export/omnix.py`, `cut.py
//! --from-marks --whole-unmarked`), now from Dailix's `dailix/sheet.json`,
//! with the same rules:
//! - clips rated ✓ or ? go in, junk and unrated ones don't;
//! - a segment is a cut, its role and note the cut's note; a point (the
//!   sheet's first version) a cut of `POINT` seconds from it;
//! - a clip with no segment goes in whole (a flag is no marking: a note
//!   about a meme doesn't mean the clip was gone through);
//! - the cleaned voice chosen for a clip plays in its cuts;
//! - by day, by file, by time; 16:9 (the player shows the export's shape).
//!
//! New here (gap 75): a clip's flags inside a cut become the cut's
//! markers, their notes kept, in clip time so they travel with the cut.

use crate::project::{Cut, Mark, Project};
use crate::view::Aspect;
use anyhow::{Context, Result};
use dailix_sheet::{Doc, Layout, MarkKind, Rating};
use std::path::{Path, PathBuf};

/// A point mark's cut length, as footage-agent made it.
const POINT: f64 = 6.0;
/// Shorter than Omnix's shortest cut, a mark makes none.
const MIN_CUT: f64 = crate::model::edit::MIN_CUT;

/// A mark's label for a cut's note: its role and note.
fn label(role: Option<&str>, note: &str) -> String {
    [role.unwrap_or_default(), note].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// The edit the marking of the Dailix project at `root` makes, of one day
/// or of all.
pub fn build(root: &Path, day: Option<&str>) -> Result<Project> {
    let layout = Layout::new(root);
    let text = std::fs::read_to_string(layout.sheet()).with_context(|| format!("reading {}", layout.sheet().display()))?;
    let sheet = Doc::parse(&text)?.sheet()?;
    let mut cuts = Vec::new();
    // A sidecar that can't be read leaves its clip out, as Dailix's sheet
    // does (and says).
    let (footage, _) = dailix_sheet::footage::read(&layout).with_context(|| format!("reading {}", layout.analysis().display()))?;
    for clip in footage.days.into_iter().filter(|d| day.is_none_or(|only| d.day == only)).flat_map(|d| d.clips) {
        let Some(mark) = sheet.clips.get(&clip.key) else { continue };
        if !matches!(mark.rating, Some(Rating::Take | Rating::Think)) {
            continue;
        }
        let (d, file) = clip.key.split_once('/').unwrap_or_default();
        let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let voice = mark.audio.as_ref().map(|r| layout.voice().join(d).join(&stem).join(format!("{r}.flac"))).filter(|v| v.is_file());
        let mut spans: Vec<(f64, f64, String)> = mark
            .marks
            .iter()
            .filter(|m| m.kind != MarkKind::Flag)
            .map(|m| (m.t, m.end.unwrap_or(m.t + POINT).min(clip.duration), label(m.role.as_deref(), &m.note)))
            .collect();
        if spans.is_empty() {
            spans.push((0.0, clip.duration, mark.note.chars().take(80).collect()));
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (start, end, note) in spans {
            if end - start < MIN_CUT {
                continue;
            }
            let mut cut = Cut::new(clip.path.clone(), start, end);
            cut.note = (!note.is_empty()).then_some(note);
            cut.audio = voice.clone();
            cut.marks = mark
                .marks
                .iter()
                .filter(|m| m.kind == MarkKind::Flag && m.t >= start && m.t <= end)
                .map(|m| Mark { t: m.t, note: label(m.role.as_deref(), &m.note) })
                .collect();
            cuts.push(cut);
        }
    }
    let mut project = Project { aspect: Some(Aspect(16, 9)), cuts, ..Default::default() };
    project.settle_ids();
    Ok(project)
}

/// Builds the edit and writes it into the project's `04_project`, under a
/// name not taken (`omnix-<day>.json`, `omnix-<day>-2.json`, …): never over
/// an edit already there. Where it went.
pub fn write(root: &Path, day: Option<&str>) -> Result<PathBuf> {
    save(root, day, &build(root, day)?)
}

/// An empty edit in the project's `04_project`, to build from the bin
/// (user, 2026-11-05: an idea made afresh). Of one day, the bin shows that
/// day's clips (`omnix-<day>` names it); of none, every day's
/// (`omnix-all-days`: the whole project's footage). Where it went.
pub fn write_empty(root: &Path, day: Option<&str>) -> Result<PathBuf> {
    save(root, day, &Project { aspect: Some(Aspect(16, 9)), ..Default::default() })
}

fn save(root: &Path, day: Option<&str>, project: &Project) -> Result<PathBuf> {
    let folder = Layout::new(root).edits();
    std::fs::create_dir_all(&folder)?;
    let stem = format!("omnix-{}", day.unwrap_or("all-days"));
    let path = (1..).map(|n| folder.join(if n == 1 { format!("{stem}.json") } else { format!("{stem}-{n}.json") })).find(|p| !p.exists()).expect("a free name");
    project.save(&path)?;
    Ok(path)
}

/// The day an edit's file is named for (`omnix-<day>…`), if it is.
pub fn day_of(file: &Path) -> Option<String> {
    let name = file.file_stem()?.to_str()?;
    let day = name.strip_prefix("omnix-")?.get(..10)?;
    dailix_sheet::is_day(day).then(|| day.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marking_becomes_an_edit_by_the_sheets_rules() {
        let root = std::env::temp_dir().join(format!("omnix-from-sheet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::new(&root);
        layout.create().unwrap();
        let clip = |day: &str, file: &str, d: f64| {
            let dir = layout.analysis().join(day);
            std::fs::create_dir_all(&dir).unwrap();
            let stem = file.split('.').next().unwrap();
            std::fs::write(dir.join(format!("{stem}.footage.json")), format!(r#"{{"source": "{file}", "day": "{day}", "technical": {{"duration_s": {d}}}}}"#)).unwrap();
        };
        clip("2030-05-01", "A.MP4", 30.0);
        clip("2030-05-01", "B.MP4", 10.0);
        clip("2030-05-01", "C.MP4", 10.0);
        clip("2030-05-02", "D.MP4", 8.0);
        std::fs::create_dir_all(layout.voice().join("2030-05-01/A")).unwrap();
        std::fs::write(layout.voice().join("2030-05-01/A/dnn6.flac"), b"v").unwrap();
        std::fs::write(
            layout.sheet(),
            r#"{"clips": {
                "2030-05-01/A.MP4": {"rating": 2, "audio": "dnn6", "marks": [
                    {"t": 12, "end": 20, "role": "talk", "note": "the tree"},
                    {"t": 15, "kind": "flag", "role": "cut", "note": "a meme here"},
                    {"t": 25, "kind": "flag", "note": "outside"},
                    {"t": 2, "note": "a point"}]},
                "2030-05-01/B.MP4": {"rating": 1, "note": "the whole of it", "marks": [{"t": 3, "kind": "flag", "note": "only a flag"}]},
                "2030-05-01/C.MP4": {"rating": 0},
                "2030-05-02/D.MP4": {"rating": 2}}}"#,
        )
        .unwrap();
        let p = build(&root, Some("2030-05-01")).unwrap();
        let cuts: Vec<(String, f64, f64, Option<&str>)> = p.cuts.iter().map(|c| (c.input.file_name().unwrap().to_string_lossy().into_owned(), c.start, c.end, c.note.as_deref())).collect();
        assert_eq!(
            cuts,
            [
                ("A.MP4".to_owned(), 2.0, 8.0, Some("a point")),
                ("A.MP4".to_owned(), 12.0, 20.0, Some("talk · the tree")),
                ("B.MP4".to_owned(), 0.0, 10.0, Some("the whole of it")),
            ]
        );
        // The voice, and the flag inside the segment as its marker.
        assert_eq!(p.cuts[1].audio.as_deref(), Some(layout.voice().join("2030-05-01/A/dnn6.flac").as_path()));
        assert_eq!(p.cuts[1].marks.iter().map(|m| (m.t, m.note.as_str())).collect::<Vec<_>>(), [(15.0, "cut · a meme here")]);
        assert_eq!(p.cuts[2].marks.len(), 1, "a whole clip keeps its flags as markers too");
        assert_eq!(build(&root, None).unwrap().cuts.len(), 4, "all days: D whole too");

        // Written beside the others, never over one.
        let a = write(&root, Some("2030-05-01")).unwrap();
        let b = write(&root, Some("2030-05-01")).unwrap();
        assert_eq!((a.file_name().unwrap(), b.file_name().unwrap()), ("omnix-2030-05-01.json".as_ref(), "omnix-2030-05-01-2.json".as_ref()));
        assert_eq!(day_of(&a).as_deref(), Some("2030-05-01"));
        assert_eq!(day_of(&b).as_deref(), Some("2030-05-01"));
        assert_eq!(day_of(Path::new("/x/my-edit.json")), None);
        // An empty day: the player reads it, the command line's tools don't.
        let empty = write_empty(&root, Some("2030-05-02")).unwrap();
        assert!(Project::read(&empty).unwrap().cuts.is_empty() && Project::load(&empty).is_err());
        // Of every day: named for none, so the bin filters none out.
        let all = write_empty(&root, None).unwrap();
        assert_eq!((all.file_name().unwrap(), day_of(&all)), ("omnix-all-days.json".as_ref(), None));
        let back = Project::load(&a).unwrap();
        assert_eq!(back.cuts.len(), 3);
        // Beside 04_project, not under it: kept whole, as Omnix saves them.
        assert_eq!(back.cuts[0].input, root.join("01_footage/2030-05-01/A.MP4"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
