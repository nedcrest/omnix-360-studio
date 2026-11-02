//! `rosmo360 apply` and `describe`: operations from a file, and what a
//! project holds by id to write them against (`OPS-API.md`). The tests
//! and an agent change a project the way the player does.

use crate::model::edit::Edit;
use crate::model::ops::{Op, check};
use crate::project::{Project, SoundTrack};
use anyhow::{Context, Result, bail};
use std::path::Path;

/// Applies the ops in `ops` (a JSON list) to `project` as one step, all or
/// none, printing what each did; writes the result to `output` if given.
pub fn run(project: &Path, ops: &Path, output: Option<&Path>) -> Result<()> {
    let p = Project::load(project)?;
    let text = std::fs::read_to_string(ops).with_context(|| format!("reading {}", ops.display()))?;
    let ops: Vec<Op> = serde_json::from_str(&text).with_context(|| format!("reading {}", ops.display()))?;
    let mut edit = Edit::default();
    edit.replace(p);
    match edit.batch(&ops) {
        Ok(done) => done.iter().for_each(|o| println!("{}", o.summary)),
        Err((n, e)) => bail!("op {} of {}: {e}; nothing changed", n + 1, ops.len()),
    }
    check(&edit).map_err(|e| anyhow::anyhow!("the ops left the project broken: {e}"))?;
    if let Some(out) = output {
        edit.save(out)?;
        eprintln!("wrote {}", out.display());
    }
    Ok(())
}

/// The JSON Schema of an ops file: a list of `Op`, each field described by
/// its doc comment, so an agent's tools come from the same types the
/// player applies.
pub fn schema() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(Vec<Op>)).expect("a schema is JSON")
}

/// What `project` holds, a line each, by id: what ops name.
pub fn describe(project: &Path) -> Result<()> {
    let p = Project::load(project)?;
    let name = |path: &Path| path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let offsets = crate::project::offsets(&p.cuts);
    println!("cuts:");
    for (c, at) in p.cuts.iter().zip(offsets) {
        println!("  cut #{:<3} {:7.2}-{:7.2} s  {} {:.2}-{:.2}  level {:+.1} dB{}", c.id, at, at + c.len(), name(&c.input), c.start, c.end, c.gain, if c.mute { " muted" } else { "" });
        for m in c.marks.iter().filter(|m| m.t >= c.start && m.t <= c.end) {
            println!("    marker {:.2} s (clip) at {:.2} s{}", m.t, at + m.t - c.start, if m.note.is_empty() { String::new() } else { format!("  «{}»", m.note) });
        }
    }
    for w in SoundTrack::ALL {
        let clips = &p.track(w).clips;
        if clips.is_empty() {
            continue;
        }
        println!("{}:", w.name());
        for c in clips {
            println!("  clip #{:<2} {:7.2}-{:7.2} s  {} {:.2}-{:.2}  level {:+.1} dB{}{}", c.id, c.at, c.ends(), name(&c.input), c.start, c.end, c.gain, if c.mute { " muted" } else { "" }, c.ducked.as_ref().map_or(String::new(), |d| format!(" ducked {:.0} dB", d.depth)));
        }
    }
    for (what, lanes) in [("overlays", &p.overlays), ("text", &p.text)] {
        let items: Vec<_> = lanes.iter().flat_map(|l| &l.items).collect();
        if items.is_empty() {
            continue;
        }
        println!("{what}:");
        for i in items {
            let label = i.text.as_ref().map_or_else(|| name(&i.input), |t| format!("«{}»", t.content));
            println!("  item #{:<2} {:7.2}-{:7.2} s  {label}", i.id, i.at, i.ends());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_schema_names_every_area_and_op_with_what_it_does() {
        let schema = super::schema();
        for word in ["\"cut\"", "\"clip\"", "\"sound\"", "\"item\"", "\"key\"", "\"split\"", "\"duck\"", "\"look_to_all\"", "\"set_text\""] {
            assert!(schema.contains(word), "{word} missing");
        }
        assert!(schema.contains("replacing a duck before"), "doc comments become descriptions");
        // And it is a schema an agent's tool list can take.
        let v: serde_json::Value = serde_json::from_str(&schema).unwrap();
        assert_eq!(v["type"], "array");
    }
}
