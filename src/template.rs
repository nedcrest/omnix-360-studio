//! Transition templates (gap 25c, user, 2026-11-08): a join's transitions
//! set up once (the overlap's look, length and settings, the edge looks on
//! either side) and saved under a name, in one library for the program,
//! to put on any join of any edit (`CutOp::Template`).
//!
//! The library is `<config>/transitions/<name>.json`, beside the LUTs'.
//! Omnix makes its folder when it starts (`make_library`), so a fresh
//! install has it without anyone making it by hand; saving makes it again
//! if it was taken away, and reading a missing one finds no templates.

use crate::project::{Cut, Edge};
use crate::transition::Look;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a join's transitions are: an overlap, an out and an in, each or
/// none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// None: a plain cut, no overlap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap: Option<Overlap>,
    /// On the end of the cut before the join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_out: Option<Edge>,
    /// On the start of the cut after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_in: Option<Edge>,
}

/// A join's overlap: its look (none: the dissolve), length and the
/// settings moved from the look's own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overlap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<Look>,
    pub seconds: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f32>,
}

impl Template {
    /// The join before cut `i` (1 or more) as it stands.
    pub fn of_join(cuts: &[Cut], i: usize) -> Template {
        let c = &cuts[i];
        let overlap = (c.crossfade > 0.0).then(|| Overlap { look: c.transition, seconds: c.crossfade, params: c.transition_params.clone() });
        Template { overlap, edge_out: cuts[i - 1].edge_out.clone(), edge_in: c.edge_in.clone() }
    }
}

/// The library's folder.
pub fn library() -> Option<PathBuf> {
    Some(crate::fsutil::config_dir()?.join("transitions"))
}

/// Makes the library's folder if it isn't there (at the program's start).
pub fn make_library() -> Result<()> {
    let dir = library().context("no configuration folder (HOME is not set)")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))
}

/// The templates by name, sorted; none if the folder is missing.
pub fn names() -> Vec<String> {
    library().map_or_else(Vec::new, |d| names_in(&d))
}

pub fn load(name: &str) -> Result<Template> {
    load_in(&library().context("no configuration folder")?, name)
}

/// Saves `t` as `name`, over one of that name.
pub fn save(name: &str, t: &Template) -> Result<()> {
    save_in(&library().context("no configuration folder (HOME is not set)")?, name, t)
}

pub fn remove(name: &str) -> Result<()> {
    let path = path_in(&library().context("no configuration folder")?, name)?;
    std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            path.extension().is_some_and(|x| x == "json").then(|| Some(path.file_stem()?.to_string_lossy().into_owned())).flatten()
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn load_in(dir: &Path, name: &str) -> Result<Template> {
    let path = path_in(dir, name)?;
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
}

fn save_in(dir: &Path, name: &str, t: &Template) -> Result<()> {
    let path = path_in(dir, name)?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    crate::fsutil::replace_file(&path, serde_json::to_string_pretty(t)?.as_bytes())
}

/// A template's file: its name, trimmed, as the file's; one that would
/// leave the folder or hide in it is refused.
fn path_in(dir: &Path, name: &str) -> Result<PathBuf> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
        bail!("{name:?} can't be a template's name");
    }
    Ok(dir.join(format!("{name}.json")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_is_saved_listed_read_back_and_named_safely() {
        let dir = std::env::temp_dir().join(format!("omnix-templates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(names_in(&dir).is_empty(), "no folder, no templates");
        let t = Template {
            overlap: Some(Overlap { look: Some(Look::WhipPan), seconds: 0.5, params: BTreeMap::from([("streak".to_owned(), 1.5)]) }),
            edge_out: None,
            edge_in: Some(Edge { look: Look::EdgeBlur, seconds: 0.25, params: BTreeMap::new() }),
        };
        save_in(&dir, " Fast whip ", &t).unwrap();
        save_in(&dir, "a calm one", &Template::default()).unwrap();
        assert_eq!(names_in(&dir), ["a calm one", "Fast whip"]);
        assert_eq!(load_in(&dir, "Fast whip").unwrap(), t);
        assert!(["", "../x", "a/b", ".hidden"].iter().all(|n| save_in(&dir, n, &t).is_err()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_join_reads_as_its_template() {
        let mut cuts = vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut::new("b.mp4".into(), 0.0, 4.0)];
        cuts[1].crossfade = 0.75;
        cuts[1].transition = Some(Look::Flash);
        cuts[0].edge_out = Some(Edge { look: Look::EdgeFade, seconds: 0.5, params: BTreeMap::new() });
        let t = Template::of_join(&cuts, 1);
        assert_eq!((t.overlap.map(|o| (o.look, o.seconds)), t.edge_out.map(|e| e.look), t.edge_in), (Some((Some(Look::Flash), 0.75)), Some(Look::EdgeFade), None));
    }
}
