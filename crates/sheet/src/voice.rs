//! Cleaned voices (Omnix gap 77): the camera's or the phone's microphone
//! sits behind whoever talks, so traffic, sea and wind come through louder
//! than the words. A clip's voice is made again from its original by a
//! named recipe into `dailix/voice/<day>/<clip>/<recipe>.flac`, beside
//! `_recipes.json` saying what it was made from and how (and, once
//! measured, how loud it is). Everything there can be deleted and made
//! again; the marking's `audio` field names the one the edit uses.
//!
//! A recipe's name means the same forever: the marking stores the name, so
//! other settings are a new name, never the old one changed (footage-agent's
//! `docs/VOICE.md`, measured on real footage: `dnn6` the default).
//!
//! This is the pure part: the recipes, the files' places and what they
//! say. Running `deep-filter` and ffmpeg is the app's (`dailix voice`).

use crate::layout::Layout;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// How a recipe works: DeepFilterNet (`deep-filter`, its args), or an
/// ffmpeg filter chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dnn,
    Ffmpeg,
}

impl Kind {
    /// As `_recipes.json` writes it.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Dnn => "dnn",
            Kind::Ffmpeg => "ffmpeg",
        }
    }
}

/// A named way of making a clip's voice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub name: &'static str,
    pub kind: Kind,
    /// `deep-filter`'s arguments, or the ffmpeg filter chain (one).
    pub args: &'static [&'static str],
    pub note: &'static str,
}

/// For wind: nothing taken away, what's left lifted: the rumble cut, +5 dB
/// where words are told apart, the dynamics pressed.
pub const EQ_CHAIN: &str = "highpass=f=95,equalizer=f=2600:t=q:w=1.2:g=5,equalizer=f=250:t=q:w=1.0:g=-4,acompressor=threshold=-26dB:ratio=3:attack=8:release=200:makeup=3";

/// The recipes, as footage-agent named them (`-D` keeps the sound in step
/// with the picture; `-a` caps how much a moment is pressed down, so a
/// quiet phrase taken for noise isn't wiped).
pub const RECIPES: [Recipe; 4] = [
    Recipe { name: "dnn6", kind: Kind::Dnn, args: &["-D", "-a", "6"], note: "DeepFilterNet, pressing 6 dB at most: the default" },
    Recipe { name: "dnn12", kind: Kind::Dnn, args: &["-D", "-a", "12"], note: "DeepFilterNet, 12 dB at most: cleaner, a word lost more often" },
    Recipe { name: "dnn24", kind: Kind::Dnn, args: &["-D", "-a", "24"], note: "DeepFilterNet, 24 dB at most: clearly cleaner, quiet phrases go" },
    Recipe { name: "eq", kind: Kind::Ffmpeg, args: &[EQ_CHAIN], note: "nothing taken away, the words lifted: for wind" },
];

/// The recipe a voice is made with unless another is asked for.
pub const DEFAULT: &str = "dnn6";

pub fn recipe(name: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.name == name)
}

/// What a variant was made from and how: an entry of `_recipes.json`, as
/// footage-agent wrote it, and its loudness once measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Made {
    pub recipe: String,
    pub kind: String,
    pub args: Vec<String>,
    pub note: String,
    pub source: String,
    pub source_size: u64,
    pub source_mtime: i64,
    /// Integrated loudness, LUFS (gap 77: Omnix sets the voice's level
    /// from it, nothing is rendered again).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lufs: Option<f64>,
}

impl Made {
    /// `recipe` made from `source` now (its loudness not yet measured).
    pub fn stamp(recipe: &Recipe, source: &Path) -> std::io::Result<Made> {
        let meta = std::fs::metadata(source)?;
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
        Ok(Made {
            recipe: recipe.name.to_owned(),
            kind: recipe.kind.key().to_owned(),
            args: recipe.args.iter().map(|a| a.to_string()).collect(),
            note: recipe.note.to_owned(),
            source: source.display().to_string(),
            source_size: meta.len(),
            source_mtime: mtime,
            lufs: None,
        })
    }
}

/// Voices are compared at this loudness, LUFS: a louder one would pass for
/// better (footage-agent's VOICE.md). Only for choosing between them: the
/// edit's level is Omnix's (`model::level::TARGET_LUFS`, −16, set from
/// the voice when the edit is built); two purposes, two rules (Omnix
/// audit B12, decided by the user 2026-11-11).
pub const HEARD_AT: f64 = -20.0;
/// None is lifted more than this, dB, while comparing: a near-silent one
/// would roar (Omnix lifts up to 24 for the film).
const MOST_LIFT: f64 = 15.0;

/// The gain, dB, that brings a voice `lufs` loud to `HEARD_AT`; none
/// unmeasured.
pub fn compare_gain(lufs: Option<f64>) -> f64 {
    lufs.map_or(0.0, |l| (HEARD_AT - l).min(MOST_LIFT))
}

/// The recipe a Make makes for a clip with voices `made`: the one
/// `picked`, else the first not made yet; none when all are.
pub fn to_make<'a>(picked: Option<&str>, made: impl IntoIterator<Item = &'a str> + Clone) -> Option<&'static Recipe> {
    picked.and_then(recipe).or_else(|| RECIPES.iter().find(|r| !made.clone().into_iter().any(|n| n == r.name)))
}

/// Which made voice speech is looked for in (gap 83): one DeepFilterNet
/// cleaned, as the noise is gone there and plain loudness finds the talk;
/// the one the edit uses if it is such, else `DEFAULT`, else any. None
/// without one (`eq` keeps the wind: it would read as talk).
pub fn speech_source<'a>(made: &'a [String], chosen: Option<&'a str>) -> Option<&'a str> {
    let cleaned = |n: &&str| recipe(n).is_some_and(|r| r.kind == Kind::Dnn) && made.iter().any(|m| m == n);
    chosen.filter(cleaned).or_else(|| Some(DEFAULT).filter(cleaned)).or_else(|| made.iter().map(String::as_str).find(cleaned))
}

/// The record beside a clip's variants.
pub const MADE: &str = "_recipes.json";

/// Clip `key`'s (`<day>/<file>`) voices' folder: `voice/<day>/<stem>`;
/// none for a key reaching outside it.
pub fn dir(layout: &Layout, key: &str) -> Option<PathBuf> {
    let p = Path::new(key);
    if !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let stem = p.file_stem()?;
    Some(layout.voice().join(p.parent().unwrap_or(Path::new(""))).join(stem))
}

/// Variant `name`'s file in `dir`.
pub fn track(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.flac"))
}

/// The variants made in `dir`, by name.
pub fn variants(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "flac"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

/// What `dir`'s `_recipes.json` says; empty when it's missing or unreadable
/// (the variants are made again, nothing is lost).
pub fn made(dir: &Path) -> BTreeMap<String, Made> {
    std::fs::read_to_string(dir.join(MADE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// `made` written whole, or not at all.
pub fn write_made(dir: &Path, made: &BTreeMap<String, Made>) -> std::io::Result<()> {
    crate::store::replace(&dir.join(MADE), serde_json::to_string_pretty(made).map_err(std::io::Error::other)?.as_bytes())
}

/// The clips whose marking names a voice that isn't made (deleted, or
/// never made here): a line each, as `sheet check` says it (Omnix audit
/// C12). The edit plays the clip's own sound for such a one.
pub fn gone(layout: &Layout, sheet: &crate::Sheet) -> Vec<String> {
    sheet
        .clips
        .iter()
        .filter_map(|(key, c)| {
            let name = c.audio.as_deref().filter(|a| !a.is_empty())?;
            let made = dir(layout, key).is_some_and(|d| track(&d, name).is_file());
            (!made).then(|| format!("clip {key}: uses voice {name}, which isn't made (its own sound plays)"))
        })
        .collect()
}

/// Held while `_recipes.json` is read and written again.
static RECORDING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Variant `name`'s entry in `dir`'s `_recipes.json`, the file read again
/// just before it's written and under a lock: two voices made at once
/// don't lose each other's entry (audit A9).
pub fn record(dir: &Path, name: &str, entry: Made) -> std::io::Result<()> {
    let _held = RECORDING.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = made(dir);
    all.insert(name.to_owned(), entry);
    write_made(dir, &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clips_voices_live_by_its_day_and_name() {
        let layout = Layout::new("/p");
        assert_eq!(dir(&layout, "2030-05-01/A.MP4"), Some(PathBuf::from("/p/dailix/voice/2030-05-01/A")));
        assert_eq!(dir(&layout, "../x/A.MP4"), None, "never outside");
        assert_eq!(track(Path::new("/v"), "dnn6"), PathBuf::from("/v/dnn6.flac"));
        assert_eq!(recipe(DEFAULT).map(|r| r.args), Some(&["-D", "-a", "6"][..]));
        let made = ["dnn24".to_owned(), "dnn6".to_owned(), "eq".to_owned()];
        assert_eq!((speech_source(&made, Some("eq")), speech_source(&made, Some("dnn24")), speech_source(&made[2..], None)), (Some("dnn6"), Some("dnn24"), None));
        // Make makes the one picked, else the first not made.
        assert_eq!((to_make(Some("eq"), ["eq"]).map(|r| r.name), to_make(None, ["dnn6"]).map(|r| r.name), to_make(None, RECIPES.map(|r| r.name))), (Some("eq"), Some("dnn12"), None));
        assert_eq!((compare_gain(Some(-26.0)), compare_gain(Some(-50.0)), compare_gain(Some(-14.0)), compare_gain(None)), (6.0, 15.0, -6.0, 0.0));
    }

    #[test]
    fn a_variants_record_is_written_and_read_back() {
        let tmp = std::env::temp_dir().join(format!("dailix-voice-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let src = tmp.join("a.mp4");
        std::fs::write(&src, b"twelve bytes").unwrap();
        let dnn6 = recipe("dnn6").unwrap();
        let mut m = Made::stamp(dnn6, &src).unwrap();
        assert_eq!((m.source_size, m.args.join(" ")), (12, "-D -a 6".to_owned()));
        m.lufs = Some(-21.5);
        write_made(&tmp, &BTreeMap::from([("dnn6".to_owned(), m.clone())])).unwrap();
        std::fs::write(tmp.join("dnn6.flac"), b"").unwrap();
        assert_eq!((made(&tmp).get("dnn6"), variants(&tmp)), (Some(&m), vec!["dnn6".to_owned()]));
        // As footage-agent wrote it, without a loudness: read.
        let old = r#"{"dnn6": {"recipe": "dnn6", "kind": "dnn", "args": ["-D", "-a", "6"], "note": "n", "source": "/x/a.mp4", "source_size": 12, "source_mtime": 1}}"#;
        std::fs::write(tmp.join(MADE), old).unwrap();
        assert_eq!((made(&tmp)["dnn6"].source_size, made(&tmp)["dnn6"].lufs), (12, None));
        // A marking naming a voice that's there passes; one gone is said.
        let layout = Layout::new(&tmp);
        let clip_dir = dir(&layout, "2030-05-01/A.MP4").unwrap();
        std::fs::create_dir_all(&clip_dir).unwrap();
        std::fs::write(track(&clip_dir, "dnn6"), b"").unwrap();
        let sheet: crate::Sheet = serde_json::from_str(r#"{"clips": {"2030-05-01/A.MP4": {"audio": "dnn6"}, "2030-05-01/B.MP4": {"audio": "eq"}}}"#).unwrap();
        assert_eq!(gone(&layout, &sheet), ["clip 2030-05-01/B.MP4: uses voice eq, which isn't made (its own sound plays)"]);
        // Two recorded one after another: both kept.
        record(&tmp, "eq", Made::stamp(recipe("eq").unwrap(), &src).unwrap()).unwrap();
        assert_eq!(made(&tmp).keys().collect::<Vec<_>>(), ["dnn6", "eq"]);
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
