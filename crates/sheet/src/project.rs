//! A project's `dailix/project.json`: what the project is (its name as
//! people say it, where it came from, where its originals are). Its
//! fields are kept as found, in their order: a field one writer doesn't
//! know (an import, the home view, a later version) survives the others.

use crate::Layout;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The file's fields; none when it isn't there.
pub fn read(layout: &Layout) -> std::io::Result<Map<String, Value>> {
    match std::fs::read(layout.project()) {
        Ok(b) => serde_json::from_slice(&b).map_err(std::io::Error::other),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e),
    }
}

/// Changes some fields, keeping the rest; `version` set when new. Written
/// whole or not at all.
pub fn update(layout: &Layout, change: impl FnOnce(&mut Map<String, Value>)) -> std::io::Result<()> {
    let mut fields = read(layout)?;
    fields.entry("version").or_insert(1.into());
    change(&mut fields);
    let text = serde_json::to_string_pretty(&fields).map_err(std::io::Error::other)?;
    crate::store::replace(&layout.project(), text.as_bytes())
}

/// The project's name as people say it ("Sea, May 2030"): its `title`,
/// else its folder's name. The folder keeps its name, so paths into it
/// (edits, the sources) never change.
pub fn title(root: &Path) -> String {
    let named = read(&Layout::new(root)).ok().and_then(|p| p.get("title").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned));
    named.unwrap_or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// Names the project; an empty name gives it back its folder's.
pub fn set_title(root: &Path, title: &str) -> std::io::Result<()> {
    update(&Layout::new(root), |p| {
        match title.trim() {
            "" => p.remove("title"),
            t => p.insert("title".to_owned(), t.into()),
        };
    })
}

/// What the user wrote about a project (user, 2026-11-07: a description
/// and tags, to find and tell trips apart): `project.json`'s
/// `description` and `tags`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct About {
    pub description: String,
    pub tags: Vec<String>,
}

/// The project's description and tags; empty when none.
pub fn about(root: &Path) -> About {
    let p = read(&Layout::new(root)).unwrap_or_default();
    About {
        description: p.get("description").and_then(Value::as_str).unwrap_or_default().to_owned(),
        tags: p.get("tags").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),
    }
}

/// Sets them, keeping every other field; an empty one is taken out.
pub fn set_about(root: &Path, about: &About) -> std::io::Result<()> {
    update(&Layout::new(root), |p| {
        let description = about.description.trim();
        match description.is_empty() {
            true => p.remove("description"),
            false => p.insert("description".into(), description.into()),
        };
        match about.tags.is_empty() {
            true => p.remove("tags"),
            false => p.insert("tags".into(), about.tags.iter().map(|t| Value::from(t.as_str())).collect()),
        };
    })
}

/// Tags as typed, comma-separated: trimmed, empty ones and repeats left out.
pub fn parse_tags(typed: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for t in typed.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            tags.push(t.to_owned());
        }
    }
    tags
}

/// A name as people say it as a folder's: Ukrainian letters spelled in
/// Latin ones, lower case, anything else a dash, 40 letters at most;
/// empty when nothing is left.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        let latin = match c {
            'а' => "a", 'б' => "b", 'в' => "v", 'г' => "h", 'ґ' => "g", 'д' => "d", 'е' => "e", 'є' => "ie", 'ж' => "zh", 'з' => "z",
            'и' => "y", 'і' => "i", 'ї' => "i", 'й' => "i", 'к' => "k", 'л' => "l", 'м' => "m", 'н' => "n", 'о' => "o", 'п' => "p",
            'р' => "r", 'с' => "s", 'т' => "t", 'у' => "u", 'ф' => "f", 'х' => "kh", 'ц' => "ts", 'ч' => "ch", 'ш' => "sh", 'щ' => "shch",
            'ь' | '\'' | '’' => "", 'ю' => "iu", 'я' => "ia",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => "-",
        };
        out.push_str(latin);
    }
    let joined = out.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    joined.chars().take(40).collect::<String>().trim_end_matches('-').to_owned()
}

/// A trip's folder name from its name as people say it and its month
/// (`YYYY-MM`): `2026-11_motopokatushky-lystopad`, as the trips made so far
/// are named (user, 2026-11-08).
pub fn trip_folder(month: &str, title: &str) -> String {
    let s = slug(title);
    format!("{month}_{}", if s.is_empty() { "trip" } else { &s })
}

/// The month (`YYYY-MM`, UTC) of a time in seconds since 1970: a new
/// trip's folder starts with it.
pub fn month_of(unix_secs: u64) -> String {
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}

/// A new trip in the place `projects`: its folder (`folder`, or `-2`, …
/// when taken) made in the fixed layout, named `title`, with what was
/// written about it. Its folder.
pub fn new_trip(projects: &Path, folder: &str, title: &str, about: &About) -> std::io::Result<PathBuf> {
    let free = (1..).map(|n| if n == 1 { folder.to_owned() } else { format!("{folder}-{n}") }).find(|n| !projects.join(n).exists()).expect("a free name");
    let root = projects.join(free);
    let l = Layout::new(&root);
    l.create()?;
    update(&l, |_| {})?;
    // An empty marking: a trip is read with one (its days come later).
    if !l.sheet().exists() {
        std::fs::write(l.sheet(), "{}")?;
    }
    set_title(&root, title)?;
    set_about(&root, about)?;
    Ok(root)
}

/// The projects in the place `projects`: its folders that are Dailix
/// projects, by name. The home view and Omnix's start window both list
/// them.
pub fn all(projects: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(projects) else { return Vec::new() };
    let mut roots: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| Layout::new(p).is_project()).collect();
    roots.sort();
    roots
}

/// A project's videos' edits (`videos/<name>/edit.json`), the latest
/// changed first.
pub fn edits(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(Layout::new(root).videos()) else { return Vec::new() };
    let mut found: Vec<_> = entries.flatten().map(|e| e.path().join(crate::layout::EDIT)).filter(|p| p.is_file()).map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).ok(), p)).collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found.into_iter().map(|(_, p)| p).collect()
}

/// The name of a video of every day of a trip (a day's is the day).
pub const ALL_DAYS: &str = "all-days";

/// The trip's edits of videos called `name` or `name-<n>` (made from the
/// same day, or all days), newest first (Omnix gap 104: «→ Omnix» offers
/// them before making another).
pub fn videos_named(root: &Path, name: &str) -> Vec<PathBuf> {
    let numbered = |v: &str| v.strip_prefix(name).and_then(|r| r.strip_prefix('-')).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    edits(root).into_iter().filter(|e| video_of(e).is_some_and(|(_, v)| v == name || numbered(&v))).collect()
}

/// The project and the video an edit file is (`<root>/videos/<name>/edit.json`).
pub fn of_edit(edit: &Path) -> Option<PathBuf> {
    video_of(edit).map(|(root, _)| root)
}

/// The project and the video's name of an edit file, if it's a video's.
pub fn video_of(edit: &Path) -> Option<(PathBuf, String)> {
    if edit.file_name()? != crate::layout::EDIT {
        return None;
    }
    let folder = edit.parent()?;
    let root = folder.parent()?.parent()?;
    let l = Layout::new(root);
    let name = folder.file_name()?.to_str()?.to_owned();
    (folder.parent() == Some(l.videos().as_path()) && l.is_project()).then(|| (root.to_owned(), name))
}

/// What an edit is called in lists: its video's name, else its file's.
pub fn edit_name(edit: &Path) -> String {
    video_of(edit).map(|(_, name)| name).or_else(|| edit.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_default()
}

/// Where a video's edit exports to by default (user, 2026-11-08).
pub fn export_dir(edit: &Path) -> Option<PathBuf> {
    video_of(edit).map(|(root, name)| Layout::new(root).video(&name).join(crate::layout::EXPORT))
}

/// A new video's folder, made: `name`, or `name-2`, `name-3`, … when taken.
/// Its edit's path (not written). The folder is claimed by making it: two
/// edits made at once (Dailix's → Omnix, off its window's thread) could
/// both find a name free and one write over the other (the audit,
/// 2026-11-14).
pub fn new_video(root: &Path, name: &str) -> std::io::Result<PathBuf> {
    let l = Layout::new(root);
    if let Some(videos) = l.video(name).parent() {
        std::fs::create_dir_all(videos)?;
    }
    for n in 1.. {
        let free = if n == 1 { name.to_owned() } else { format!("{name}-{n}") };
        match std::fs::create_dir(l.video(&free)) {
            Ok(()) => return Ok(l.video_edit(&free)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    unreachable!("names run out")
}

/// What becomes of an edit lying loose in the old `04_project`.
#[derive(Debug, Clone, PartialEq)]
pub enum Tidy {
    /// Moved into a video of its own (its name: the file's, `omnix-` off).
    Move { from: PathBuf, to: PathBuf },
    /// The same bytes as one moved: to the trash.
    Same { path: PathBuf, as_: PathBuf },
    /// Nothing in it (no cuts, no sounds, no items): to the trash.
    Empty { path: PathBuf },
    /// Not an Omnix edit: left where it is.
    Other { path: PathBuf },
}

/// What tidying the old `04_project` into videos does (user, 2026-11-08:
/// one copy of each edit kept, each in its video; copies and empty ones
/// go). Only planned: the caller moves and trashes.
pub fn tidy_old_edits(root: &Path) -> Vec<Tidy> {
    let l = Layout::new(root);
    let Ok(entries) = std::fs::read_dir(l.old_edits()) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    // The first made first: `omnix-x` before `omnix-x-2`.
    files.sort_by_key(|p| (p.file_stem().map_or(0, |s| s.len()), p.clone()));
    let mut kept: Vec<(Vec<u8>, PathBuf)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for f in files {
        let bytes = std::fs::read(&f).unwrap_or_default();
        let json: Option<Value> = (f.extension().is_some_and(|x| x == "json")).then(|| serde_json::from_slice(&bytes).ok()).flatten();
        let Some(Value::Object(fields)) = json.filter(|v| v.get("cuts").is_some()) else {
            out.push(Tidy::Other { path: f });
            continue;
        };
        let empty = fields.iter().filter(|(k, _)| *k != "aspect").all(|(_, v)| match v {
            Value::Array(a) => a.is_empty(),
            Value::Object(o) => o.values().all(|x| x.as_array().is_none_or(|a| a.is_empty())),
            _ => true,
        });
        if empty {
            out.push(Tidy::Empty { path: f });
        } else if let Some((_, first)) = kept.iter().find(|(b, _)| *b == bytes) {
            out.push(Tidy::Same { path: f.clone(), as_: first.clone() });
        } else {
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
            let base = stem.strip_prefix("omnix-").unwrap_or(stem).to_owned();
            let name = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|n| !names.contains(n) && !l.video(n).exists()).expect("a free name");
            names.push(name.clone());
            kept.push((bytes, f.clone()));
            out.push(Tidy::Move { from: f, to: l.video_edit(&name) });
        }
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn edits_made_at_once_each_get_a_folder_of_their_own() {
        let trip = std::env::temp_dir().join(format!("sheet-new-video-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&trip);
        let made: Vec<PathBuf> = (0..8).map(|_| { let t = trip.clone(); std::thread::spawn(move || new_video(&t, "2030-05-01").unwrap()) }).collect::<Vec<_>>().into_iter().map(|h| h.join().unwrap()).collect();
        let mut names: Vec<&PathBuf> = made.iter().collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 8, "{made:?}");
        std::fs::remove_dir_all(&trip).unwrap();
    }

    use super::*;

    #[test]
    fn a_description_and_tags_are_kept_beside_the_other_fields() {
        let root = std::env::temp_dir().join(format!("dailix-about-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.data()).unwrap();
        set_title(&root, "Sea").unwrap();
        assert_eq!(about(&root), About::default());
        let a = About { description: " Scooters and cats ".into(), tags: parse_tags("scooter, Cats, , cats ,sea") };
        assert_eq!(a.tags, ["scooter", "Cats", "sea"]);
        set_about(&root, &a).unwrap();
        assert_eq!(about(&root), About { description: "Scooters and cats".into(), tags: a.tags.clone() });
        assert_eq!(title(&root), "Sea");
        set_about(&root, &About::default()).unwrap();
        let p = read(&l).unwrap();
        assert!(!p.contains_key("description") && !p.contains_key("tags") && p.contains_key("title"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_place_lists_its_projects_and_each_its_edits() {
        let place = std::env::temp_dir().join(format!("dailix-all-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&place);
        for name in ["b_trip", "a_trip"] {
            let l = Layout::new(place.join(name));
            std::fs::create_dir_all(l.data()).unwrap();
            update(&l, |_| {}).unwrap();
        }
        std::fs::create_dir_all(place.join("not_a_project")).unwrap();
        assert_eq!(all(&place), [place.join("a_trip"), place.join("b_trip")]);
        let trip = place.join("a_trip");
        let old = new_video(&trip, "2030-05-01").unwrap();
        std::fs::write(&old, "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let new = new_video(&trip, "2030-05-01").unwrap();
        std::fs::write(&new, "{}").unwrap();
        std::fs::create_dir_all(Layout::new(&trip).video("not-yet")).unwrap();
        assert_eq!(new, Layout::new(&trip).video_edit("2030-05-01-2"));
        assert_eq!(edits(&trip), [new.clone(), old.clone()]);
        let other = new_video(&trip, "2030-05-01-short").unwrap();
        std::fs::write(&other, "{}").unwrap();
        assert_eq!(videos_named(&trip, "2030-05-01"), [new.clone(), old.clone()], "not «-short»: another video");
        assert!(videos_named(&trip, ALL_DAYS).is_empty());
        assert!(edits(&place.join("b_trip")).is_empty());
        assert_eq!(video_of(&new), Some((trip.clone(), "2030-05-01-2".to_owned())));
        assert_eq!(export_dir(&new), Some(trip.join("videos/2030-05-01-2/export")));
        assert_eq!((edit_name(&new), edit_name(Path::new("/x/my-edit.json"))), ("2030-05-01-2".to_owned(), "my-edit".to_owned()));
        assert_eq!(of_edit(&trip.join("videos/x/other.json")), None);
        assert_eq!(of_edit(&trip.join("04_project/edit.json")), None);
        assert!(all(&place.join("gone")).is_empty());
        std::fs::remove_dir_all(&place).unwrap();
    }

    #[test]
    fn old_edits_go_one_copy_each_into_videos_and_the_rest_out() {
        let root = std::env::temp_dir().join(format!("dailix-tidy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let l = Layout::new(&root);
        std::fs::create_dir_all(l.data()).unwrap();
        update(&l, |_| {}).unwrap();
        let old = l.old_edits();
        std::fs::create_dir_all(&old).unwrap();
        let edit = r#"{"cuts": [{"input": "a.mp4", "in": 0, "out": 2}]}"#;
        for (f, text) in [("omnix-2030-05-01.json", edit), ("omnix-2030-05-01-2.json", edit), ("omnix-all-days.json", r#"{"aspect": "16:9", "cuts": []}"#), ("omnix-test.json", r#"{"cuts": [], "music": {"clips": [{"input": "s.wav"}]}}"#), ("den1.kdenlive", "<mlt/>")] {
            std::fs::write(old.join(f), text).unwrap();
        }
        let plan = tidy_old_edits(&root);
        assert!(plan.contains(&Tidy::Move { from: old.join("omnix-2030-05-01.json"), to: l.video_edit("2030-05-01") }), "{plan:?}");
        assert!(plan.contains(&Tidy::Same { path: old.join("omnix-2030-05-01-2.json"), as_: old.join("omnix-2030-05-01.json") }));
        assert!(plan.contains(&Tidy::Empty { path: old.join("omnix-all-days.json") }));
        assert!(plan.contains(&Tidy::Move { from: old.join("omnix-test.json"), to: l.video_edit("test") }), "music alone is not empty");
        assert!(plan.contains(&Tidy::Other { path: old.join("den1.kdenlive") }));
        assert_eq!(plan.len(), 5);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_new_trip_is_named_by_its_month_and_made_in_the_layout() {
        assert_eq!(trip_folder("2026-11", "Мотопокатушки, листопад!"), "2026-11_motopokatushky-lystopad");
        assert_eq!(trip_folder("2026-08", "Кос'2026 — Греція"), "2026-08_kos2026-hretsiia");
        assert_eq!(trip_folder("2026-08", "  …  "), "2026-08_trip");
        assert_eq!(slug("Шортс: павичі"), "shorts-pavychi");
        assert_eq!((month_of(0), month_of(1_793_836_800), month_of(951_782_400)), ("1970-01".to_owned(), "2026-11".to_owned(), "2000-02".to_owned()));
        let place = std::env::temp_dir().join(format!("dailix-new-trip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&place);
        let about = About { description: "Hills".into(), tags: vec!["moto".into()] };
        let a = new_trip(&place, "2026-11_moto", "Moto", &about).unwrap();
        let b = new_trip(&place, "2026-11_moto", "Moto again", &About::default()).unwrap();
        assert_eq!((a.clone(), b.clone()), (place.join("2026-11_moto"), place.join("2026-11_moto-2")));
        assert!(Layout::new(&a).is_project() && Layout::new(&a).videos().is_dir() && Layout::new(&a).footage().is_dir());
        assert!(crate::Doc::parse(&std::fs::read_to_string(Layout::new(&a).sheet()).unwrap()).is_ok(), "an empty marking, readable");
        assert_eq!((title(&a), self::about(&a)), ("Moto".to_owned(), about));
        assert_eq!(all(&place), [a, b]);
        std::fs::remove_dir_all(&place).unwrap();
    }

    #[test]
    fn fields_one_writer_does_not_know_survive_it() {
        let root = std::env::temp_dir().join(format!("dailix-project-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let layout = Layout::new(root.join("2030-05_sea"));
        std::fs::create_dir_all(layout.data()).unwrap();
        assert_eq!(title(&layout.root), "2030-05_sea");
        update(&layout, |p| {
            p.insert("sources".into(), "/s".into());
        })
        .unwrap();
        set_title(&layout.root, "  Sea, May 2030 ").unwrap();
        // An import run again doesn't name it back.
        update(&layout, |p| {
            p.insert("sources".into(), "/s2".into());
        })
        .unwrap();
        let p = read(&layout).unwrap();
        assert_eq!(p.keys().collect::<Vec<_>>(), ["version", "sources", "title"]);
        assert_eq!((title(&layout.root).as_str(), p["sources"].as_str()), ("Sea, May 2030", Some("/s2")));
        set_title(&layout.root, "").unwrap();
        assert_eq!(title(&layout.root), "2030-05_sea");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
