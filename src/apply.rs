//! `omnix apply` and `describe`: operations from a file, and what a
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
        for line in look_lines(c) {
            println!("    {line}");
        }
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

/// A cut's looks as `describe` says them: the join before it (its overlap
/// and look), its edge looks and its style, each with the settings moved,
/// and its effect keys.
fn look_lines(c: &crate::project::Cut) -> Vec<String> {
    use std::collections::BTreeMap;
    let set = |params: &BTreeMap<String, f32>| if params.is_empty() { String::new() } else { format!("  {}", params.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")) };
    let mut lines = Vec::new();
    if c.crossfade > 0.0 || c.transition.is_some() {
        let look = c.transition.map_or("dissolve".to_owned(), key_of);
        lines.push(format!("join before: {look} over {:.2} s{}", c.crossfade, set(&c.transition_params)));
    }
    for (side, edge) in [("in", &c.edge_in), ("out", &c.edge_out)] {
        if let Some(e) = edge {
            lines.push(format!("{side}: {} {:.2} s{}", key_of(e.look), e.seconds, set(&e.params)));
        }
    }
    if let Some(s) = &c.style {
        lines.push(format!("style: {}{}", key_of(s.look), set(&s.params)));
    }
    // Its effect keys, each with what it sets (as the project writes it).
    for k in &c.effects {
        let mut values = serde_json::to_value(k).unwrap_or_default();
        if let Some(map) = values.as_object_mut() {
            map.remove("t");
        }
        lines.push(format!("effects at {:.2} s (clip): {values}", k.t));
    }
    lines
}

/// A look's name in the project and the ops (`whip_pan`).
fn key_of(look: crate::transition::Look) -> String {
    serde_json::to_string(&look).expect("a look is JSON").trim_matches('"').to_owned()
}

/// The looks an agent can set, for `omnix looks`: every one by its key,
/// overlap, edge and style, with its settings (key, range, default,
/// choices), then the transition templates' library and its names; or,
/// given one, that template as the JSON a `template` op takes.
pub fn looks(template: Option<&str>) -> Result<String> {
    use crate::transition::Look;
    if let Some(name) = template {
        return Ok(serde_json::to_string_pretty(&crate::template::load(name)?)?);
    }
    let mut out = String::new();
    for (kind, looks) in [
        ("overlap (op `transition`, settings `transition_knob`)", &Look::OVERLAP[..]),
        ("edge (op `edge`, settings `edge_knob`)", &Look::EDGES[..]),
        ("style (op `style`, settings `style_knob`)", &Look::STYLES[..]),
    ] {
        out += &format!("{kind}:\n");
        for &l in looks {
            let built = if Look::code(Some(l)) == 0 { "  (not built yet: plays as a dissolve)" } else { "" };
            out += &format!("  {}  {}: {}{built}\n", key_of(l), l.name(), l.about());
            for k in l.knobs() {
                let range = if k.choices.is_empty() { format!("{}..{} by {}", k.least, k.most, k.step) } else { k.choices.iter().enumerate().map(|(i, c)| format!("{i} {c}")).collect::<Vec<_>>().join(", ") };
                out += &format!("    {} = {}  ({range})\n", k.key, k.default);
            }
        }
    }
    let names = crate::template::names();
    let dir = crate::template::library().map_or_else(|| "none".to_owned(), |d| d.display().to_string());
    out += &format!("templates in {dir}: {}\n", if names.is_empty() { "none".to_owned() } else { names.join(", ") });
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn transitions_are_said_with_their_settings() {
        use crate::project::{Cut, Edge};
        use crate::transition::Look;
        let mut c = Cut::new("a.mp4".into(), 0.0, 4.0);
        (c.crossfade, c.transition) = (0.5, Some(Look::WhipPan));
        c.transition_params.insert("streak".into(), 1.5);
        c.edge_out = Some(Edge { look: Look::EdgeFade, seconds: 1.0, params: Default::default() });
        assert_eq!(super::look_lines(&c), ["join before: whip_pan over 0.50 s  streak 1.5", "out: edge_fade 1.00 s"]);
        c.style = Some(crate::project::Style { look: Look::Vhs, params: Default::default() });
        assert_eq!(super::look_lines(&c).last().unwrap(), "style: vhs");
        c.effects.push(crate::grade::EffectKey { t: 1.0, grade: crate::grade::Grade { vignette: 0.5, vignette_shape: 1.0, ..Default::default() }, ease: Default::default() });
        assert_eq!(super::look_lines(&c).last().unwrap(), r#"effects at 1.00 s (clip): {"vignette_shape":1.0,"vignette":0.5}"#);
        let list = super::looks(None).unwrap();
        for word in ["whip_pan", "streak = 1", "edge_zoom", "0 In, 1 Out", "sphere_whip", "not built yet", "old_film", "grain = 0.5", "templates in"] {
            assert!(list.contains(word), "{word} missing:\n{list}");
        }
    }

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
