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
//! - the cleaned voice chosen for a clip plays in its cuts, each cut's
//!   level set from the loudness of the stretch it plays (gap 77; audit
//!   B11: Dailix's number is the whole file's);
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
        use dailix_sheet::voice;
        let chosen = mark.audio.as_ref().zip(voice::dir(&layout, &clip.key)).filter(|(r, dir)| voice::track(dir, r).is_file());
        let voice = chosen.as_ref().map(|(r, dir)| voice::track(dir, r));
        // The whole file's loudness as Dailix measured it: what's used for a
        // voice that can't be read here.
        let whole = chosen.as_ref().and_then(|(r, dir)| voice::made(dir).get(*r)?.lufs);
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
            cut.sheet_voice = chosen.as_ref().map(|(r, _)| (*r).clone());
            if let Some(v) = &voice {
                cut.loudness = stretch_loudness(v, (start, end), whole);
                cut.gain = crate::model::ops::sound::normal_gain(cut.loudness);
            }
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

/// The cleaned voices Dailix made for clip `clip` of the trip at `root`,
/// by name with their files, and the one its marking chose (gap 103):
/// what a cut's «Voice» offers. Nothing for a clip outside the trip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Voices {
    pub made: Vec<(String, PathBuf)>,
    pub chosen: Option<String>,
}

pub fn voices(root: &Path, clip: &Path) -> Voices {
    let layout = Layout::new(root);
    voices_in(&layout, read_sheet(&layout).as_ref(), clip)
}

/// The trip's marking, if it can be read.
fn read_sheet(layout: &Layout) -> Option<dailix_sheet::Sheet> {
    std::fs::read_to_string(layout.sheet()).ok().and_then(|t| Doc::parse(&t).ok()?.sheet().ok())
}

/// `voices` with the marking read already (`newer_voices` reads it once
/// for every clip; the audit, 2026-11-12).
fn voices_in(layout: &Layout, sheet: Option<&dailix_sheet::Sheet>, clip: &Path) -> Voices {
    let Ok(rel) = clip.strip_prefix(layout.footage()) else { return Voices::default() };
    let key = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
    let Some(dir) = dailix_sheet::voice::dir(layout, &key) else { return Voices::default() };
    let made = dailix_sheet::voice::variants(&dir).into_iter().map(|n| (n.clone(), dailix_sheet::voice::track(&dir, &n))).collect();
    let chosen = sheet.and_then(|s| s.clips.get(&key)?.audio.clone());
    Voices { made, chosen }
}

/// The cuts of `project` whose clip has a voice chosen in the marking of
/// the trip at `root` (and made) that the cut hasn't seen yet
/// (`Cut::sheet_voice`): (cut, the voice's name, its file), to offer
/// when the edit opens (gap 103).
pub fn newer_voices(root: &Path, project: &Project) -> Vec<(crate::project::Id, String, PathBuf)> {
    let layout = Layout::new(root);
    let sheet = read_sheet(&layout);
    let mut known: std::collections::HashMap<&Path, Voices> = std::collections::HashMap::new();
    let mut newer = Vec::new();
    for c in &project.cuts {
        let v = known.entry(&c.input).or_insert_with(|| voices_in(&layout, sheet.as_ref(), &c.input));
        let Some(name) = v.chosen.clone().filter(|n| Some(n) != c.sheet_voice.as_ref()) else { continue };
        // A cut that plays it already isn't offered it (an edit made from
        // the marking before `sheet_voice` was kept; the audit).
        if let Some((_, file)) = v.made.iter().find(|(n, _)| *n == name).filter(|(_, f)| c.audio.as_deref() != Some(f.as_path())) {
            newer.push((c.id, name, file.clone()));
        }
    }
    newer
}

/// A voice's loudness over a cut's `span`, LUFS (`stretch_loudness`),
/// with the whole file's as Dailix measured it when it can't be read:
/// what `CutOp::Voice` sets the level from.
pub fn voice_loudness(voice: &Path, span: (f64, f64)) -> Option<f64> {
    let whole = voice.parent().and_then(|dir| dailix_sheet::voice::made(dir).get(voice.file_stem()?.to_str()?)?.lufs);
    stretch_loudness(voice, span, whole)
}

/// What a cut playing `voice` over `span` (the voice is timed like its
/// clip) plays, LUFS, as Normalize measures it; none where it's silent.
/// A voice that can't be read here gets `whole`, the file's own loudness
/// Dailix measured, and a warning.
fn stretch_loudness(voice: &Path, span: (f64, f64), whole: Option<f64>) -> Option<f64> {
    match crate::ffmpeg::has_audio(voice).map(|sound| sound.then(|| crate::loudness::measure_stretch(voice, span, None))) {
        Ok(Some(Ok(measured))) => measured.map(|(lufs, _)| lufs),
        Ok(None) => None,
        Ok(Some(Err(e))) | Err(e) => {
            eprintln!("warning: {} couldn't be measured ({e:#}); its whole file's loudness is used", voice.display());
            whole
        }
    }
}

/// Builds the edit and writes it as a new video of the trip
/// (`videos/<day>/edit.json`, `videos/<day>-2/…` when taken; user,
/// 2026-11-08): never over an edit already there. Where it went.
pub fn write(root: &Path, day: Option<&str>) -> Result<PathBuf> {
    save(root, day, &build(root, day)?)
}

/// An empty edit as a new video, to build from the bin (user, 2026-11-05:
/// an idea made afresh). Of one day, the bin shows that day's clips (the
/// video is named for it); of none, every day's (`all-days`: the whole
/// trip's footage). Where it went.
pub fn write_empty(root: &Path, day: Option<&str>) -> Result<PathBuf> {
    save(root, day, &Project { aspect: Some(Aspect(16, 9)), ..Default::default() })
}

/// An empty edit as a new video called `name` (Omnix's «New video»).
pub fn write_named(root: &Path, name: &str) -> Result<PathBuf> {
    let slug = dailix_sheet::project::slug(name);
    save(root, Some(if slug.is_empty() { "video" } else { &slug }), &Project { aspect: Some(Aspect(16, 9)), ..Default::default() })
}

fn save(root: &Path, name: Option<&str>, project: &Project) -> Result<PathBuf> {
    let path = dailix_sheet::project::new_video(root, name.unwrap_or("all-days"))?;
    project.save(&path)?;
    Ok(path)
}

/// The day a video is named for (`videos/<day>…/edit.json`), if it is.
pub fn day_of(edit: &Path) -> Option<String> {
    let (_, name) = dailix_sheet::project::video_of(edit)?;
    let day = name.get(..10)?;
    dailix_sheet::is_day(day).then(|| day.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_gets_the_loudness_of_the_stretch_it_plays() {
        let dir = std::env::temp_dir().join(format!("omnix-stretch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let voice = dir.join("dnn6.flac");
        // Loud for 10 s, then 20 dB down.
        let made = std::process::Command::new("ffmpeg").args(["-v", "error", "-y", "-f", "lavfi", "-i", "sine=f=440:d=20:sample_rate=48000", "-af", "volume='if(lt(t,10),1,0.1)':eval=frame"]).arg(&voice).status();
        if !made.is_ok_and(|s| s.success()) {
            return;
        }
        let (loud, quiet) = (stretch_loudness(&voice, (1.0, 8.0), Some(-26.0)).unwrap(), stretch_loudness(&voice, (12.0, 19.0), Some(-26.0)).unwrap());
        let broken = dir.join("broken.flac");
        std::fs::write(&broken, b"not sound").unwrap();
        let unread = stretch_loudness(&broken, (1.0, 8.0), Some(-26.0));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!((loud - quiet - 20.0).abs() < 1.0, "{loud} against {quiet}");
        assert_eq!(unread, Some(-26.0), "can't be read: Dailix's number");
    }

    #[test]
    fn a_marking_becomes_an_edit_by_the_sheets_rules() {
        let root = std::env::temp_dir().join(format!("omnix-from-sheet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::new(&root);
        layout.create().unwrap();
        dailix_sheet::project::update(&layout, |_| {}).unwrap();
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
        let made = r#"{"dnn6": {"recipe": "dnn6", "kind": "dnn", "args": ["-D", "-a", "6"], "note": "", "source": "", "source_size": 1, "source_mtime": 0, "lufs": -26.0}}"#;
        std::fs::write(layout.voice().join("2030-05-01/A/_recipes.json"), made).unwrap();
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
        assert_eq!((p.cuts[1].loudness, p.cuts[1].gain), (Some(-26.0), 10.0), "its level from the voice's loudness");
        assert_eq!(p.cuts[1].marks.iter().map(|m| (m.t, m.note.as_str())).collect::<Vec<_>>(), [(15.0, "cut · a meme here")]);
        assert_eq!(p.cuts[2].marks.len(), 1, "a whole clip keeps its flags as markers too");
        assert_eq!(build(&root, None).unwrap().cuts.len(), 4, "all days: D whole too");
        // What a cut's «Voice» offers: the voices made and the marking's choice.
        let v = voices(&root, &layout.clip("2030-05-01/A.MP4"));
        assert_eq!((v.made, v.chosen.as_deref()), (vec![("dnn6".to_owned(), layout.voice().join("2030-05-01/A/dnn6.flac"))], Some("dnn6")));
        assert_eq!(voices(&root, &layout.clip("2030-05-01/B.MP4")), Voices::default(), "none made");
        assert_eq!(voices(&root, Path::new("/elsewhere/A.MP4")), Voices::default());
        // An edit made from the marking has seen its choice; one chosen later is offered.
        assert!(newer_voices(&root, &p).is_empty());
        let mut seen_none = p.clone();
        seen_none.cuts[1].sheet_voice = None;
        assert!(newer_voices(&root, &seen_none).is_empty(), "it plays it already");
        seen_none.cuts[1].audio = None;
        assert_eq!(newer_voices(&root, &seen_none).iter().map(|(id, n, _)| (*id, n.as_str())).collect::<Vec<_>>(), [(p.cuts[1].id, "dnn6")]);

        // Written beside the others, never over one.
        let a = write(&root, Some("2030-05-01")).unwrap();
        let b = write(&root, Some("2030-05-01")).unwrap();
        assert_eq!((a.clone(), b.clone()), (layout.video_edit("2030-05-01"), layout.video_edit("2030-05-01-2")));
        assert_eq!(day_of(&a).as_deref(), Some("2030-05-01"));
        assert_eq!(day_of(&b).as_deref(), Some("2030-05-01"));
        assert_eq!(day_of(Path::new("/x/my-edit.json")), None);
        // An empty day: the player reads it, the command line's tools don't.
        let empty = write_empty(&root, Some("2030-05-02")).unwrap();
        assert!(Project::read(&empty).unwrap().cuts.is_empty() && Project::load(&empty).is_err());
        // Of every day: named for none, so the bin filters none out.
        let all = write_empty(&root, None).unwrap();
        assert_eq!((all.clone(), day_of(&all)), (layout.video_edit("all-days"), None));
        let back = Project::load(&a).unwrap();
        assert_eq!(back.cuts.len(), 3);
        // The trip's own clips, kept whole, as Omnix saves them.
        assert_eq!(back.cuts[0].input, root.join("01_footage/2030-05-01/A.MP4"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
