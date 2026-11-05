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

#[cfg(test)]
mod tests {
    use super::*;

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
