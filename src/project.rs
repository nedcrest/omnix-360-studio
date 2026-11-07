//! A project's `dailix/project.json`: what the project is (its name as
//! people say it, where it came from, where its originals are). Its
//! fields are kept as found, in their order: a field one writer doesn't
//! know (an import, the home view, a later version) survives the others.

use crate::Layout;
use serde_json::{Map, Value};
use std::path::Path;

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

/// The projects in the place `projects`: its folders that are Dailix
/// projects, by name. The home view and Omnix's start window both list
/// them.
pub fn all(projects: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(projects) else { return Vec::new() };
    let mut roots: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| Layout::new(p).is_project()).collect();
    roots.sort();
    roots
}

/// A project's edits (`04_project/*.json`), the latest changed first.
pub fn edits(root: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(Layout::new(root).edits()) else { return Vec::new() };
    let mut found: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).ok(), p)).collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found.into_iter().map(|(_, p)| p).collect()
}

/// The project an edit file is one of (it lies in its `04_project`).
pub fn of_edit(edit: &Path) -> Option<std::path::PathBuf> {
    let root = edit.parent()?.parent()?;
    let l = Layout::new(root);
    (edit.parent() == Some(l.edits().as_path()) && l.is_project()).then(|| root.to_owned())
}

#[cfg(test)]
mod tests {
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
        let edits_dir = Layout::new(place.join("a_trip")).edits();
        std::fs::create_dir_all(&edits_dir).unwrap();
        std::fs::write(edits_dir.join("old.json"), "{}").unwrap();
        std::fs::write(edits_dir.join("notes.txt"), "").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(edits_dir.join("new.json"), "{}").unwrap();
        assert_eq!(edits(&place.join("a_trip")), [edits_dir.join("new.json"), edits_dir.join("old.json")]);
        assert!(edits(&place.join("b_trip")).is_empty());
        assert_eq!(of_edit(&edits_dir.join("new.json")), Some(place.join("a_trip")));
        assert_eq!(of_edit(&place.join("a_trip/elsewhere.json")), None);
        assert!(all(&place.join("gone")).is_empty());
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
