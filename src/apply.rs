//! `omnix apply` and `describe`: operations from a file, and what a
//! project holds by id to write them against (`OPS-API.md`). The tests
//! and an agent change a project the way the player does.

use crate::model::edit::Edit;
use crate::model::ops::{Op, check};
use crate::project::{Project, SoundTrack};
use anyhow::{Context, Result, bail};
use serde::Serialize;
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
    for w in warnings(&edit) {
        eprintln!("warning: {w}");
    }
    if let Some(out) = output {
        edit.save(out)?;
        eprintln!("wrote {}", out.display());
    }
    Ok(())
}

/// `omnix looks --save-template`: the join before cut `cut` of `project`
/// saved to the templates' library as `name` (audit C7: the player alone
/// could).
pub fn save_template(project: &Path, cut: crate::project::Id, name: &str) -> Result<()> {
    let p = Project::load(project)?;
    let t = join_template(&p, cut)?;
    crate::template::save(name, &t)?;
    eprintln!("saved the join before cut #{cut} as «{name}»");
    Ok(())
}

/// The join before cut `cut` as a template; none before the first cut, nor
/// a join with nothing on it.
fn join_template(p: &Project, cut: crate::project::Id) -> Result<crate::template::Template> {
    let i = p.cut_index(cut).with_context(|| format!("no cut #{cut}"))?;
    if i == 0 {
        bail!("cut #{cut} is the first: there's no join before it");
    }
    let t = crate::template::Template::of_join(&p.cuts, i);
    if t.overlap.is_none() && t.edge_out.is_none() && t.edge_in.is_none() {
        bail!("the join before cut #{cut} is a plain cut: nothing to save");
    }
    Ok(t)
}

/// What the project names that isn't there, or reaches past its file: an
/// op can't know (audit C4), so it's said after, not refused.
fn warnings(p: &Project) -> Vec<String> {
    let mut said: Vec<String> = p.clone().drop_missing(|f| f.exists()).said().into_iter().collect();
    for c in &p.cuts {
        if let Some(lut) = c.lut.as_deref().filter(|l| crate::lut::find(l).is_none()) {
            said.push(format!("cut #{}: no LUT «{lut}» (add it in Settings)", c.id));
        }
        if c.input.exists()
            && let Ok(long) = crate::ffmpeg::duration(&c.input)
            && c.end > long + 0.05
        {
            said.push(format!("cut #{} runs to {:.2} s of a clip {long:.2} s long", c.id, c.end));
        }
    }
    for (id, by) in p.past_end() {
        said.push(format!("#{id} runs {by:.2} s past the film's end (the trim_to_end op trims it)"));
    }
    said
}

/// How a measuring command (`normalize`, `duck`, `autocolour`) ends: its
/// ops applied to `project` as one step and written to `output` if given;
/// or, `print`, only checked and printed as an ops file `omnix apply`
/// takes (audit C11), the project left alone.
pub fn finish(project: Project, ops: &[Op], print: bool, output: Option<&Path>) -> Result<()> {
    let mut edit = Edit::default();
    edit.replace(project);
    edit.batch(ops).map_err(|(n, e)| anyhow::anyhow!("op {} of {}: {e}", n + 1, ops.len()))?;
    if print {
        println!("{}", serde_json::to_string_pretty(ops)?);
    } else if let Some(out) = output {
        edit.save(out)?;
        eprintln!("wrote {}", out.display());
    }
    Ok(())
}

/// What a measuring command says of each thing: on stdout, or on stderr
/// while stdout carries its ops (`--ops`).
pub fn say(print_ops: bool, line: &str) {
    if print_ops {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

/// The JSON Schema of an ops file: a list of `Op`, each field described by
/// its doc comment, so an agent's tools come from the same types the
/// player applies.
pub fn schema() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(Vec<Op>)).expect("a schema is JSON")
}

/// What `project` holds, a line each, by id: what ops name.
pub fn describe(project: &Path) -> Result<()> {
    for line in describe_lines(&Project::load(project)?) {
        println!("{line}");
    }
    Ok(())
}

/// `describe`'s lines for `p`.
fn describe_lines(p: &Project) -> Vec<String> {
    let mut out = Vec::new();
    let mut println = |line: String| out.push(line);
    let name = |path: &Path| path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let aspect = p.aspect.map_or("free".to_owned(), |a| format!("{}:{}", a.0, a.1));
    println(format!("edit: aspect {aspect}  clips' sound {:+.1} dB  fades in {:.2} s, out {:.2} s", p.sound.volume, p.fades.fade_in, p.fades.fade_out));
    let offsets = crate::project::offsets(&p.cuts);
    println("cuts:".to_owned());
    for (i, (c, at)) in p.cuts.iter().zip(offsets).enumerate() {
        println(format!("  cut #{:<3} {:7.2}-{:7.2} s  {} {:.2}-{:.2}  level {:+.1} dB{}", c.id, at, at + c.edit_len(), name(&c.input), c.start, c.end, c.gain, if c.mute { " muted" } else { "" }));
        if let (Some(origin), false) = (c.origin, p.pieces(i).is_empty()) {
            println(format!("    a piece of clip #{origin} (join_pieces joins them back)"));
        }
        for line in cut_lines(c).into_iter().chain(look_lines(c)) {
            println(format!("    {line}"));
        }
        for m in c.marks.iter().filter(|m| m.t >= c.start && m.t <= c.end) {
            println(format!("    marker {:.2} s (clip) at {:.2} s{}", m.t, at + c.edit_at(m.t), if m.note.is_empty() { String::new() } else { format!("  «{}»", m.note) }));
        }
    }
    for w in SoundTrack::ALL {
        let track = p.track(w);
        if track.clips.is_empty() {
            continue;
        }
        let sync = if track.sync { "  sync lock" } else { "" };
        println(format!("{}: level {:+.1} dB{sync}", w.name(), track.volume));
        for c in &track.clips {
            println(format!("  clip #{:<2} {:7.2}-{:7.2} s  {} {:.2}-{:.2}  level {:+.1} dB{}{}", c.id, c.at, c.ends(), name(&c.input), c.start, c.end, c.gain, if c.mute { " muted" } else { "" }, c.ducked.as_ref().map_or(String::new(), |d| format!(" ducked {:.0} dB", d.depth))));
            let sound = Sounding { even: c.even, low_cut: c.low_cut, clean: c.clean, fades: (c.fade_in, c.fade_out), beyond: (0.0, 0.0), loudness: c.loudness };
            for line in sound_lines(&sound, &c.volume) {
                println(format!("    {line}"));
            }
            if let Some(b) = c.beats {
                println(format!("    beats at {:.2}{}", b.threshold, if b.snap { ", snapping" } else { "" }));
            }
            if let Some(d) = c.ducked.as_ref().filter(|d| !d.speech.is_empty()) {
                let spans: Vec<String> = d.speech.iter().map(|(a, b)| format!("{a:.2}-{b:.2}")).collect();
                println(format!("    ducked under speech (file time): {}", spans.join(", ")));
            }
        }
    }
    for (what, lanes) in [("overlays", &p.overlays), ("text", &p.text)] {
        if lanes.iter().all(|l| l.items.is_empty()) {
            continue;
        }
        println(format!("{what}:"));
        // Lane by lane when there are several, 0 the lowest (gap 70).
        for (n, i) in lanes.iter().enumerate().flat_map(|(n, l)| l.items.iter().map(move |i| (n, i))) {
            if (lanes.len() > 1 || lanes[n].name.is_some() || lanes[n].hidden || lanes[n].sync) && lanes[n].items.first().is_some_and(|f| f.id == i.id) {
                let name = lanes[n].name.as_ref().map_or(String::new(), |s| format!(" «{s}»"));
                println(format!("  lane {n}{name}{}{}:", if lanes[n].hidden { " (hidden)" } else { "" }, if lanes[n].sync { " sync lock" } else { "" }));
            }
            let label = match (&i.text, i.bars) {
                (Some(t), _) => format!("«{}»", t.content),
                (None, Some(b)) => format!("bars {:.2}:1, sliding {:.1} s, colour {:?}", b.ratio, b.slide, b.colour),
                (None, None) => format!("{}{}", name(&i.input), i.kind.map_or(String::new(), |k| format!(" as a {}", k.name().to_lowercase()))),
            };
            println(format!("  item #{:<2} {:7.2}-{:7.2} s  {label}", i.id, i.at, i.ends()));
            if let Some(t) = &i.text {
                println(format!("    {}", text_line(t)));
            }
            println(format!("    own time {:.2}-{:.2} s{}", i.start, i.end, if i.file().is_some() { " (of its file)" } else { "" }));
            for line in key_lines("place", "own", &i.motion).into_iter().chain(key_lines("effects", "own", &i.effects)) {
                println(format!("    {line}"));
            }
            if let Some(s) = &i.style {
                println(format!("    {}", style_line(s)));
            }
            for line in auto_lines(&i.automation) {
                println(format!("    {line}"));
            }
        }
    }
    out.extend(whole_lines(p));
    out
}

/// `describe`'s last lines: the groups, and what runs past the film's end.
fn whole_lines(p: &Project) -> Vec<String> {
    let mut out = Vec::new();
    for g in &p.groups {
        let name = if g.name.is_empty() { String::new() } else { format!(" «{}»", g.name) };
        let members: Vec<String> = p.members(g.id).iter().map(|m| format!("#{m}")).collect();
        out.push(format!("group #{}{name} {}: {}", g.id, g.colour.hex(), members.join(", ")));
    }
    // Gap 101: what an agent should trim (the trim_to_end op).
    let past: Vec<String> = p.past_end().iter().map(|(id, by)| format!("#{id} by {by:.2} s")).collect();
    if let (false, Some(end)) = (past.is_empty(), p.film_end()) {
        out.push(format!("past the film's end at {end:.2} s: {}", past.join(", ")));
    }
    out
}

/// Keys as `describe` says them: each one's time (`clock`: clip or own)
/// and what it sets, as the project writes it.
fn key_lines<K: Serialize + crate::view::Timed>(what: &str, clock: &str, keys: &[K]) -> Vec<String> {
    keys.iter()
        .map(|k| {
            let mut values = serde_json::to_value(k).unwrap_or_default();
            if let Some(map) = values.as_object_mut() {
                map.remove("t");
            }
            format!("{what} at {:.2} s ({clock}): {values}", k.time())
        })
        .collect()
}

/// A text's look as `describe` says it: each field but its words, as the
/// project and `text_field` write them.
fn text_line(t: &crate::text::Text) -> String {
    let all = serde_json::to_value(t).unwrap_or_default();
    let fields: Vec<String> = crate::model::ops::items::TEXT_FIELDS.iter().skip(1).filter_map(|&k| all.get(k).map(|v| format!("{k} {v}"))).collect();
    format!("text: {}", fields.join(", "))
}

/// A cut's own settings as `describe` says them: its view keys,
/// stabilisation, LUT, note, sound file and sound (`flat` isn't saved: it's
/// found when the clip opens).
fn cut_lines(c: &crate::project::Cut) -> Vec<String> {
    let mut lines = Vec::new();
    for k in c.keyframes.iter() {
        let v = &k.view;
        let ease = serde_json::to_string(&k.ease).unwrap_or_default();
        let projection = serde_json::to_string(&v.projection).unwrap_or_default();
        let follow = k.follow.as_ref().map_or(String::new(), |f| format!(", following {f}"));
        lines.push(format!("view key {:.2} s (clip): yaw {:.1} pitch {:.1} roll {:.1} fov {:.1} {}, {}{follow}", k.t, v.yaw, v.pitch, v.roll, v.fov, projection.trim_matches('"'), ease.trim_matches('"')));
    }
    if let Some(m) = c.stabilize {
        lines.push(format!("stabilise: {}", serde_json::to_string(&m).unwrap_or_default().trim_matches('"')));
    }
    if let Some(lut) = &c.lut {
        lines.push(format!("lut: {lut}"));
    }
    if let Some(note) = &c.note {
        lines.push(format!("note: «{note}»"));
    }
    if let Some(audio) = &c.audio {
        lines.push(format!("sound from: {}", audio.display()));
    }
    if c.speech {
        lines.push("its own sound is speech".to_owned());
    }
    if let Some(f) = &c.frame {
        lines.push(format!("frame: {}", f.said()));
    }
    if let Some(v) = &c.sheet_voice {
        lines.push(format!("Dailix's voice seen: {v}"));
    }
    let sound = Sounding { even: c.even, low_cut: c.low_cut, clean: c.clean, fades: (c.sound_fade_in, c.sound_fade_out), beyond: (c.sound_before, c.sound_after), loudness: c.loudness };
    lines.extend(sound_lines(&sound, &c.volume));
    lines
}

/// What a cut's or a sound clip's sound goes through: evened, rumble cut,
/// cleaned; its fades; a cut's sound before and after its picture (J and
/// L); its measured loudness.
struct Sounding {
    even: bool,
    low_cut: bool,
    clean: bool,
    fades: (f64, f64),
    beyond: (f64, f64),
    loudness: Option<f64>,
}

/// A `sound:` line with what's set (none if nothing), and a `points:` line
/// for its volume points (dB at its clip's time).
fn sound_lines(s: &Sounding, volume: &[crate::project::VolumeKey]) -> Vec<String> {
    let mut said: Vec<String> = [(s.even, "even"), (s.low_cut, "low cut"), (s.clean, "clean")].into_iter().filter(|(on, _)| *on).map(|(_, w)| w.to_owned()).collect();
    for (v, what) in [(s.fades.0, "fade in"), (s.fades.1, "fade out"), (s.beyond.0, "before its picture"), (s.beyond.1, "after its picture")] {
        if v != 0.0 {
            said.push(format!("{what} {v:.2} s"));
        }
    }
    if let Some(l) = s.loudness {
        said.push(format!("measured {l:.1} LUFS"));
    }
    let mut lines = Vec::new();
    if !said.is_empty() {
        lines.push(format!("sound: {}", said.join(", ")));
    }
    if !volume.is_empty() {
        lines.push(format!("points: {}", volume.iter().map(|k| format!("{:.2} s {:+.1} dB", k.t, k.volume)).collect::<Vec<_>>().join(", ")));
    }
    lines
}

/// A cut's looks as `describe` says them: the join before it (its overlap
/// and look), its edge looks and its style, each with the settings moved,
/// and its effect keys.
fn look_lines(c: &crate::project::Cut) -> Vec<String> {
    let mut lines = Vec::new();
    if c.crossfade > 0.0 || c.transition.is_some() {
        let look = c.transition.map_or("dissolve".to_owned(), key_of);
        lines.push(format!("join before: {look} over {:.2} s{}", c.crossfade, settings(&c.transition_params)));
    }
    for (side, edge) in [("in", &c.edge_in), ("out", &c.edge_out)] {
        if let Some(e) = edge {
            lines.push(format!("{side}: {} {:.2} s{}", key_of(e.look), e.seconds, settings(&e.params)));
        }
    }
    if let Some(s) = &c.style {
        lines.push(style_line(s));
    }
    lines.extend(key_lines("effects", "clip", &c.effects));
    lines.extend(auto_lines(&c.automation));
    for t in &c.tracks {
        let span = t.samples.first().zip(t.samples.last()).map_or(String::new(), |(a, b)| format!(", {:.2}-{:.2} s (clip)", a.t, b.t));
        lines.push(format!("track «{}»: {} samples{span}", t.name, t.samples.len()));
    }
    lines
}

/// A look's settings the user moved, as `describe` says them.
fn settings(params: &std::collections::BTreeMap<String, f32>) -> String {
    if params.is_empty() { String::new() } else { format!("  {}", params.iter().map(|(k, v)| format!("{k} {}", (*v as f64 * 1000.0).round() / 1000.0)).collect::<Vec<_>>().join(", ")) }
}

/// A cut's or an item's style, as `describe` says it.
fn style_line(s: &crate::project::Style) -> String {
    format!("style: {}{}", key_of(s.look), settings(&s.params))
}

/// Automation clips as `describe` says them: the value, its stretch (clip
/// or own time) and its points.
fn auto_lines(clips: &[crate::automation::AutoClip]) -> Vec<String> {
    clips
        .iter()
        .map(|c| {
            let points: Vec<String> = c.points.iter().map(|q| format!("{:.2} s {}", q.t, (q.v * 1000.0).round() / 1000.0)).collect();
            let follows = c.follows.as_ref().map_or(String::new(), |f| format!(" (follows cut #{}'s «{}», {:+})", f.cut, f.track, f.offset));
            let follows = follows + &c.animation.map_or(String::new(), |a| format!(" ({} {})", if a.edge == crate::model::ops::End::In { "in" } else { "out" }, a.anim.name()));
            format!("automation {} {:.2}-{:.2} s{follows}: {}", c.param, c.from, c.to, points.join(", "))
        })
        .collect()
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
        ("style, on a cut or a text or picture (op cut or item `style`, settings `style_knob`)", &Look::STYLES[..]),
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
    let effects: Vec<String> = crate::grade::NAMES.iter().zip(crate::grade::RANGES).map(|(n, (lo, hi))| format!("{n} {lo}..{hi}")).collect();
    let only: Vec<&str> = crate::grade::ITEM_ONLY.iter().map(|&k| crate::grade::NAMES[k]).collect();
    out += &format!("effects (op key `effect_value`, auto `add`; {} on text and pictures only): {}\n", only.join(" and "), effects.join(", "));
    let frame: Vec<String> = crate::frame::NAMES.iter().zip(crate::frame::RANGES).map(|(n, (lo, hi))| format!("{n} {lo}..{hi}")).collect();
    out += &format!("an ordinary clip's frame (op cut `frame`; x, y, zoom in % of the picture's height, rotation in degrees): {}\n", frame.join(", "));
    let kinds: Vec<String> = crate::dress::Kind::ALL.iter().map(|k| format!("{} ({})", serde_json::to_string(k).expect("a kind is JSON").trim_matches('"'), k.about())).collect();
    out += &format!("kinds of picture (op item `kind`, or none for plain): {}\n", kinds.join("; "));
    let anims: Vec<String> = crate::animate::Anim::ALL.iter().map(|a| serde_json::to_string(a).expect("an animation is JSON").trim_matches('"').to_owned()).collect();
    out += &format!("ins and outs of text and pictures (op auto `animate`, edge in or out, seconds): {}\n", anims.join(", "));
    let luts = crate::lut::library_names();
    let dir = crate::lut::library().map_or_else(|| "none".to_owned(), |d| d.display().to_string());
    out += &format!("LUTs in {dir} (op cut `lut`, by name; or a .cube file): {}\n", if luts.is_empty() { "none".to_owned() } else { luts.join(", ") });
    out += &format!("fonts (op item `text_field`, field `font`): {}\n", crate::text::families().join(", "));
    let names = crate::template::names();
    let dir = crate::template::library().map_or_else(|| "none".to_owned(), |d| d.display().to_string());
    out += &format!("templates in {dir}: {}\n", if names.is_empty() { "none".to_owned() } else { names.join(", ") });
    Ok(out)
}

#[cfg(test)]
mod tests {
    /// What `describe` says of the time seam's film, line by line: an
    /// agent reads its times (`TIME-SEAM.md`, guard 1).
    #[test]
    fn describe_says_the_films_times_as_it_did() {
        let p = crate::time_snapshot::film();
        crate::time_snapshot::golden("time-describe", &(super::describe_lines(&p).join("\n") + "\n"));
    }

    #[test]
    fn a_texts_look_is_said_and_one_field_changed() {
        use crate::model::ops::{Op, items::ItemOp};
        let item = crate::overlay::Item { id: 0, input: "".into(), text: Some(Default::default()), at: 0.0, start: 0.0, end: 3.0, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None, group: None };
        let mut p = crate::project::Project::default();
        p.text.push(crate::overlay::Lane { items: vec![item], ..Default::default() });
        p.settle_ids();
        let mut e = crate::model::edit::Edit::new(p);
        let field = |field: &str, value: serde_json::Value| Op::Item(ItemOp::TextField { item: 1, field: field.into(), value });
        e.apply(&field("size", serde_json::json!(0.1))).unwrap();
        e.apply(&field("content", serde_json::json!("Sea"))).unwrap();
        assert!(e.apply(&field("size", serde_json::json!(5))).is_err() && e.apply(&field("weight", serde_json::json!(1))).is_err() && e.apply(&field("bold", serde_json::json!("yes"))).is_err());
        let said = super::describe_lines(&e).join("\n");
        assert!(said.contains("«Sea»") && said.contains("text: font \"Inter\", size 0.1, colour") && said.contains("bold true"), "{said}");
        // Its own time, and a place and an effect for the whole of it.
        e.apply(&Op::Key(crate::model::ops::keys::KeyOp::PlaceValue { item: 1, param: "x".into(), value: 0.25 })).unwrap();
        e.apply(&Op::Key(crate::model::ops::keys::KeyOp::EffectValue { on: crate::model::ops::keys::Graded::Item(1), param: "blur".into(), value: 0.5 })).unwrap();
        let said = super::describe_lines(&e).join("\n");
        assert!(said.contains("own time 0.00-3.00 s") && said.contains("place at 0.00 s (own): {") && said.contains("\"x\":0.25") && said.contains("effects at 0.00 s (own)") && said.contains("\"blur\":0.5"), "{said}");
    }

    #[test]
    fn a_joins_template_is_taken_from_its_cut() {
        use crate::project::{Cut, Project};
        let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut::new("b.mp4".into(), 0.0, 4.0)], ..Default::default() };
        p.settle_ids();
        assert!(super::join_template(&p, 1).is_err() && super::join_template(&p, 2).is_err() && super::join_template(&p, 9).is_err(), "the first, a plain cut, none");
        p.cuts[1].crossfade = 0.5;
        assert_eq!(super::join_template(&p, 2).unwrap().overlap.map(|o| o.seconds), Some(0.5));
    }

    #[test]
    fn apply_warns_of_what_isnt_there() {
        let mut c = crate::project::Cut::new("nowhere/a.mp4".into(), 0.0, 4.0);
        c.lut = Some("no such look".into());
        let w = super::warnings(&crate::project::Project { cuts: vec![c], ..Default::default() }).join("\n");
        assert!(w.contains("nowhere/a.mp4") && w.contains("no LUT «no such look»"), "{w}");
    }

    #[test]
    fn describe_says_a_cuts_view_and_sound_and_the_edits_own() {
        use crate::project::{Cut, MusicClip, Project, VolumeKey};
        let mut c = Cut::new("a.mp4".into(), 0.0, 4.0);
        let view = crate::view::View { yaw: 30.0, ..crate::view::View::home(crate::view::Projection::Perspective) };
        c.keyframes.set(1.0, view, 0.0, &|_, _| None);
        (c.stabilize, c.lut, c.audio) = (Some(crate::stab::Mode::Horizon), Some("dlogm".into()), Some("voice.flac".into()));
        (c.even, c.sound_before, c.loudness) = (true, 0.5, Some(-23.0));
        c.volume = vec![VolumeKey { t: 1.0, volume: -6.0 }];
        let mut p = Project { cuts: vec![c], aspect: Some(crate::view::Aspect(16, 9)), ..Project::default() };
        (p.music.volume, p.music.sync) = (-3.0, true);
        p.music.clips.push(MusicClip { input: "song.wav".into(), end: 10.0, fade_out: 2.0, beats: Some(crate::project::Beats { threshold: 0.4, snap: true }), ..MusicClip::default() });
        p.settle_ids();
        (p.cuts[0].group, p.music.clips[0].group) = (Some(1), Some(1));
        p.groups.push(crate::project::Group { id: 1, name: "Opening".into(), colour: crate::text::Colour([230, 159, 0]) });
        let lines = super::describe_lines(&p).join("\n");
        assert!(lines.contains("group #1 «Opening» #e69f00: #1, #2"), "{lines}");
        for said in [
            "edit: aspect 16:9  clips' sound +0.0 dB  fades in 0.00 s, out 0.00 s",
            "view key 1.00 s (clip): yaw 30.0",
            "stabilise: horizon",
            "lut: dlogm",
            "sound from: voice.flac",
            "sound: even, before its picture 0.50 s, measured -23.0 LUFS",
            "points: 1.00 s -6.0 dB",
            "Music: level -3.0 dB  sync lock",
            "beats at 0.40, snapping",
            "sound: fade out 2.00 s",
        ] {
            assert!(lines.contains(said), "{said:?} in\n{lines}");
        }
    }

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
        c.automation.push(crate::automation::AutoClip { param: "exposure".into(), from: 0.0, to: 2.0, points: vec![crate::automation::AutoPoint { t: 1.0, v: 0.5, ease: Default::default() }], follows: None, animation: None });
        assert_eq!(super::look_lines(&c).last().unwrap(), "automation exposure 0.00-2.00 s: 1.00 s 0.5");
        let list = super::looks(None).unwrap();
        for word in ["slide_left", "whip_pan", "streak = 1", "edge_zoom", "0 In, 1 Out", "sphere_whip", "not built yet", "old_film", "grain = 0.5", "templates in"] {
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
        // Every area and every op says what it does (audit C9).
        for area in v["$defs"]["Op"]["oneOf"].as_array().unwrap() {
            assert!(area.get("description").is_some(), "{}", area["properties"]["area"]);
            for op in area["oneOf"].as_array().into_iter().flatten() {
                assert!(op.get("description").is_some(), "{} {}", area["properties"]["area"], op["properties"]["op"]);
            }
        }
        // Values carry their ranges and defaults, opacity's 1 among them.
        let grade = &v["$defs"]["Grade"]["properties"];
        assert_eq!((grade["exposure"]["minimum"].as_f64(), grade["exposure"]["maximum"].as_f64(), grade["opacity"]["default"].as_f64()), (Some(-3.0), Some(3.0), Some(1.0)));
        assert_eq!(v["$defs"]["OverlayPlace"]["properties"]["size"]["default"].as_f64(), Some(1.0));
        let err = crate::model::ops::OpError::OutOfRange { what: "length", value: 0.01, least: 0.1, most: f64::MAX };
        assert_eq!(err.to_string(), "length 0.01 is outside 0.1..∞");
    }

    #[test]
    fn describe_names_what_runs_past_the_film() {
        let mut p = crate::project::Project { cuts: vec![crate::project::Cut::new("a.mp4".into(), 0.0, 4.0)], ..Default::default() };
        p.music.clips.push(crate::project::MusicClip { input: "song.wav".into(), at: 1.0, end: 5.0, ..Default::default() });
        p.settle_ids();
        let said = super::describe_lines(&p).join("\n");
        assert!(said.contains(&format!("past the film's end at 4.00 s: #{} by 2.00 s", p.music.clips[0].id)), "{said}");
        assert!(super::warnings(&p).iter().any(|w| w.contains("2.00 s past the film's end")));
    }

    #[test]
    fn describe_names_a_clips_pieces() {
        let mut e = crate::model::edit::Edit::new(crate::project::Project { cuts: vec![crate::project::Cut::new("a.mp4".into(), 0.0, 4.0)], ..Default::default() });
        let a = e.cuts[0].id;
        e.apply(&crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Split { cut: a, t: 2.0 })).unwrap();
        let said = super::describe_lines(&e).join("\n");
        assert_eq!(said.matches(&format!("a piece of clip #{a}")).count(), 2, "{said}");
    }
}
