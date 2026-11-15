//! The time seam's safety net (`docs/omnix/TIME-SEAM.md`, guard 1): every
//! time the code derives from one rich project, written out and compared
//! with `tests/golden/`. Moving the edit-time ↔ clip-time maths behind the
//! seam must leave these files as they are: at speed 1 the seam multiplies
//! by 1.0, which is exact, so a difference is a mistake, not noise.
//!
//! `OMNIX_BLESS=1 cargo test` writes them afresh, for a change meant to
//! show (named in its commit).

use crate::model::edit::Edit;
use crate::model::ops::{End, Op, cuts::{CutOp, Place}};
use crate::project::{Cut, Edge, Mark, MusicClip, Project, VolumeKey, offsets};
use crate::transition::Look;
use std::fmt::Write;

/// Four cuts with everything that carries a time: crossfades, a J and an
/// L cut, a sound that stops short, edge looks at both ends, sound fades,
/// marks, volume points, effect keys, an automation clip, Frame keys; a
/// sync-locked song over them and a sync-locked picture, an unlocked
/// title, the edit's fades.
pub(crate) fn film() -> Project {
    let edge = |look, seconds| Some(Edge { look, seconds, params: Default::default() });
    let mut a = Cut::new("a.mp4".into(), 10.0, 16.0);
    a.marks = vec![Mark { t: 11.0, note: "wave".into() }, Mark { t: 15.0, note: String::new() }];
    a.volume = vec![VolumeKey { t: 12.0, volume: 0.0 }, VolumeKey { t: 14.0, volume: -6.0 }];
    a.effects = vec![crate::grade::EffectKey { t: 13.0, grade: Default::default(), ease: Default::default() }];
    a.automation = vec![crate::automation::AutoClip {
        param: "exposure".into(),
        from: 11.0,
        to: 15.0,
        points: vec![crate::automation::AutoPoint { t: 11.5, v: 0.0, ease: Default::default() }, crate::automation::AutoPoint { t: 14.5, v: 1.0, ease: Default::default() }],
        follows: None,
        animation: None,
    }];
    (a.edge_in, a.sound_fade_in, a.sound_after) = (edge(Look::EdgeFade, 1.0), 0.5, 1.0);
    let mut b = Cut::new("b.mp4".into(), 2.0, 7.0);
    (b.crossfade, b.sound_before, b.edge_out, b.flat) = (1.0, 0.5, edge(Look::EdgeBlur, 0.8), true);
    let mut c = Cut::new("c.mp4".into(), 0.0, 4.0);
    (c.crossfade, c.sound_fade_out, c.sound_after) = (0.5, 0.3, -0.5);
    let mut d = Cut::new("d.mp4".into(), 20.0, 23.0);
    (d.edge_out, d.sound_before) = (edge(Look::EdgeFade, 1.5), 0.7);
    let mut p = Project { cuts: vec![a, b, c, d], ..Default::default() };
    p.music.clips.push(MusicClip { input: "song.wav".into(), at: 0.0, start: 0.0, end: 20.0, fade_in: 1.0, fade_out: 2.0, volume: vec![VolumeKey { t: 8.0, volume: -3.0 }], ..Default::default() });
    p.music.sync = true;
    p.voice.clips.push(MusicClip { input: "take.wav".into(), at: 4.0, start: 1.0, end: 3.0, ..Default::default() });
    p.overlays = vec![serde_json::from_value(serde_json::json!({ "sync": true, "items": [{ "input": "logo.png", "at": 3.0, "in": 0.0, "out": 6.0 }] })).unwrap()];
    p.text = vec![serde_json::from_value(serde_json::json!({ "items": [{ "input": "title.png", "at": 1.0, "in": 0.0, "out": 4.0 }] })).unwrap()];
    p.fades = crate::project::Fades { fade_in: 0.5, fade_out: 1.0 };
    p.settle_ids();
    // Frame keys on B, through the op that makes them.
    let mut e = Edit::new(p);
    let cut = e.cuts[1].id;
    for (t, x) in [(3.0, -10.0), (6.0, 20.0)] {
        let frame = crate::frame::Frame { x, zoom: 150.0, ..Default::default() };
        e.apply(&Op::Auto(crate::model::ops::auto::AutoOp::FrameKey { cut, t, frame, tolerance: 0.01 })).unwrap();
    }
    crate::model::ops::check(&e).unwrap();
    (*e).clone()
}

/// A number as the snapshots write it: to the microsecond, no `-0`.
pub(crate) fn n(x: f64) -> String {
    let s = format!("{x:.6}");
    if s.trim_start_matches('-').bytes().all(|b| b == b'0' || b == b'.') { "0.000000".into() } else { s }
}

/// Numbers in a row.
pub(crate) fn ns(xs: impl IntoIterator<Item = f64>) -> String {
    xs.into_iter().map(n).collect::<Vec<_>>().join(" ")
}

/// `text` against `tests/golden/<name>.txt`, or written there with
/// `OMNIX_BLESS` set. Says the first line that differs.
pub(crate) fn golden(name: &str, text: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.txt"));
    if std::env::var_os("OMNIX_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{} is missing: OMNIX_BLESS=1 cargo test writes it", path.display()));
    if want != text {
        let (k, (w, g)) = want.lines().chain(std::iter::repeat("<end>")).zip(text.lines().chain(std::iter::repeat("<end>"))).enumerate().find(|(_, (w, g))| w != g).unwrap();
        panic!("{name}: line {} changed\n  was: {w}\n  now: {g}", k + 1);
    }
}

/// Every time the core works out from `p`.
fn core(p: &Project) -> String {
    let mut s = String::new();
    let o = offsets(&p.cuts);
    let e = p;
    writeln!(s, "offsets {}", ns(o.iter().copied())).unwrap();
    writeln!(s, "edit_length {} duration {} film_end {:?}", n(crate::project::edit_length(&p.cuts)), n(p.duration()), p.film_end().map(n)).unwrap();
    for (i, c) in p.cuts.iter().enumerate() {
        writeln!(s, "cut {i}: len {} at {} crossfade_from {:?} max_crossfade {}", n(c.edit_len()), n(e.offset(i)), e.crossfade_from(i).map(n), n(crate::project::max_crossfade(&p.cuts, i))).unwrap();
        let (from, to) = c.sound_span();
        let (fi, fo) = c.sound_fades();
        writeln!(s, "  sound_span {} {} sound_fades {} {}", n(from), n(to), n(fi), n(fo)).unwrap();
        for m in c.sound_beyond(o[i]) {
            writeln!(s, "  beyond at {} {}..{} fades {} {}", n(m.at), n(m.start), n(m.end), n(m.fade_in), n(m.fade_out)).unwrap();
        }
        let edit = |t: f64| n(e.edit_time(i, t));
        writeln!(s, "  marks {}", c.marks.iter().map(|m| edit(m.t)).collect::<Vec<_>>().join(" ")).unwrap();
        writeln!(s, "  volume {}", c.volume.iter().map(|k| edit(k.t)).collect::<Vec<_>>().join(" ")).unwrap();
        writeln!(s, "  effects {}", c.effects.iter().map(|k| edit(k.t)).collect::<Vec<_>>().join(" ")).unwrap();
        for a in &c.automation {
            writeln!(s, "  auto {} {}..{} points {}", a.param, edit(a.from), edit(a.to), a.points.iter().map(|k| edit(k.t)).collect::<Vec<_>>().join(" ")).unwrap();
        }
        // Sampled over its clip, a little past either end.
        let mut t = c.start - 1.0;
        while t <= c.end + 1.0 + 1e-9 {
            let edge = c.edge_at(t).map_or_else(|| "-".to_owned(), |(spec, q)| format!("{:?} {:.6}", spec.look, q));
            let frame = c.frame_at(t).map_or_else(|| "-".to_owned(), |f| ns(f.values()));
            writeln!(s, "  t {} fade {} black {} edge {edge} frame {frame}", n(t), n(c.sound_fade(t)), n(p.black_at(i, t))).unwrap();
            t += 0.25;
        }
    }
    for m in p.sound_beyond() {
        writeln!(s, "project beyond {}", m.clips.iter().map(|c| format!("{}@{}", n(c.start), n(c.at))).collect::<Vec<_>>().join(" ")).unwrap();
    }
    // Tenths counted, not added up: the joins land on the grid exactly.
    for k in -5..=((p.duration() + 0.5) * 10.0).round() as i32 {
        let t = f64::from(k) / 10.0;
        let at = e.locate(t).map_or_else(|| "-".to_owned(), |(i, ct)| format!("{i} {}", n(ct)));
        writeln!(s, "locate {} {at}", n(t)).unwrap();
    }
    s
}

/// The cuts and what follows them, after an op.
fn after(p: &Project) -> String {
    let mut s = String::new();
    for c in &p.cuts {
        let edges = [&c.edge_in, &c.edge_out].map(|e| e.as_ref().map_or_else(|| "-".to_owned(), |e| n(e.seconds)));
        writeln!(s, "  cut {} {}..{} crossfade {} sound {} {} edges {} {} auto {}", c.input.display(), n(c.start), n(c.end), n(c.crossfade), n(c.sound_before), n(c.sound_after), edges[0], edges[1], c.automation.iter().map(|a| format!("{}:{}..{}", a.param, n(a.from), n(a.to))).collect::<Vec<_>>().join(" ")).unwrap();
    }
    writeln!(s, "  offsets {} duration {}", ns(offsets(&p.cuts)), n(p.duration())).unwrap();
    for c in &p.music.clips {
        writeln!(s, "  song at {} {}..{} fades {} {}", n(c.at), n(c.start), n(c.end), n(c.fade_in), n(c.fade_out)).unwrap();
    }
    for (lane, l) in p.overlays.iter().chain(&p.text).enumerate() {
        for i in &l.items {
            writeln!(s, "  item {lane} at {} {}..{}", n(i.at), n(i.start), n(i.end)).unwrap();
        }
    }
    s
}

/// Each op on the film afresh, and what it left; the sync lock follows.
fn ops(p: &Project) -> String {
    let id = |k: usize| p.cuts[k].id;
    let cut = |op| Op::Cut(op);
    let cases: Vec<(&str, Vec<Op>)> = vec![
        ("trim B's out to 6", vec![cut(CutOp::Trim { cut: id(1), end: End::Out, t: 6.0, most: 10.0 })]),
        ("trim A's in to 10.5", vec![cut(CutOp::Trim { cut: id(0), end: End::In, t: 10.5, most: 30.0 })]),
        ("delete C", vec![cut(CutOp::Delete { cut: id(2) })]),
        ("split A at 13, its second piece before D", vec![cut(CutOp::Split { cut: id(0), t: 13.0 })]),
        ("a cut put in at 7", vec![cut(CutOp::Add { input: "x.mp4".into(), start: 0.0, end: 2.0, place: Place::At(7.0), stabilize: None, lut: None })]),
        ("C's crossfade 0.8", vec![cut(CutOp::Crossfade { cut: id(2), seconds: 0.8 })]),
        ("D's out edge 0.5 s", vec![cut(CutOp::Edge { cut: id(3), end: End::Out, look: Some(Look::EdgeFade), seconds: 0.5 })]),
        ("A's sound out to 16.5", vec![cut(CutOp::SoundEdge { cut: id(0), end: End::Out, t: 16.5, most: 30.0 })]),
        ("C trimmed to nothing", vec![cut(CutOp::Trim { cut: id(2), end: End::Out, t: 0.01, most: 4.0 })]),
    ];
    let mut s = String::new();
    for (name, ops) in cases {
        let mut e = Edit::new(p.clone());
        writeln!(s, "{name}").unwrap();
        for op in &ops {
            if let Err(err) = e.apply(op) {
                writeln!(s, "  refused: {err:#}").unwrap();
            }
        }
        if name.starts_with("split") {
            let piece = e.cuts[1].id;
            e.apply(&cut(CutOp::Move { cut: piece, before: Some(e.cuts[4].id) })).unwrap();
        }
        s += &after(&e);
        if let Err(err) = crate::model::ops::check(&e) {
            writeln!(s, "  check: {err}").unwrap();
        }
    }
    s
}

#[test]
fn the_cores_times_are_as_they_were() {
    let p = film();
    golden("time-core", &core(&p));
    golden("time-ops", &ops(&p));
}

/// Each place in `src` that works a cut's clip span or turns its clip time
/// into edit time by hand (`cut.end - cut.start`, `+ t - cut.start`): the
/// seam's `clip_len`, `edit_len`, `edit_at` and `clip_at` say which clock,
/// and at speed only they are right (guard 3, step 7). A cut is told by
/// its name: a receiver ending in `cut` or `cuts[..]`.
fn around_the_seam(file: &str, text: &str) -> Vec<String> {
    let receiver = |s: &str| -> Option<String> {
        let r: String = s.chars().rev().take_while(|c| c.is_alphanumeric() || "_.[]".contains(*c)).collect::<String>().chars().rev().collect();
        let base = r.trim_end_matches(|c: char| c != ']').rsplit_once('[').map_or(r.as_str(), |(b, _)| b);
        (r.ends_with("cut") || base.ends_with("cuts")).then_some(r)
    };
    let mut found = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for (i, _) in code.match_indices(".end - ") {
            if let Some(r) = receiver(&code[..i])
                && code[i + 7..].starts_with(&format!("{r}.start"))
            {
                found.push(format!("{file}:{}: {}", n + 1, line.trim()));
            }
        }
        for (i, _) in code.match_indices(".start") {
            let before = &code[..i];
            let words: Vec<&str> = before.trim_end_matches(|c: char| c.is_alphanumeric() || "_.[]".contains(c)).split_whitespace().rev().take(3).collect();
            // `+ t - cut.start`: an edit offset added to clip time by hand.
            if receiver(before).is_some() && words.len() == 3 && words[0] == "-" && words[2] == "+" {
                found.push(format!("{file}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn around_the_seam_finds_a_cut_s_time_by_hand() {
    let hand = "let a = cut.end - cut.start;\nlet b = from + t - self.edit.cuts[i].start;\n";
    assert_eq!(around_the_seam("x.rs", hand).len(), 2);
    // A music clip or a title is always at 1×; a comment says what it likes.
    let fine = "let a = c.end - c.start; // cut.end - cut.start\nlet b = it.at + t - it.start;\nlet c = cut.clip_len();\n";
    assert!(around_the_seam("x.rs", fine).is_empty());
}

/// No code goes around the time seam (step 7): see `around_the_seam`.
#[test]
fn no_cut_time_is_worked_out_around_the_seam() {
    fn rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rs(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs(&src, &mut files);
    // This file's own test text is the one place that writes them.
    let found: Vec<String> = files.iter().filter(|p| !p.ends_with("time_snapshot.rs")).flat_map(|p| around_the_seam(&p.strip_prefix(&src).unwrap().display().to_string(), &std::fs::read_to_string(p).unwrap())).collect();
    assert!(found.is_empty(), "through the seam (Cut::clip_len, edit_len, edit_at, clip_at), not by hand:\n{}", found.join("\n"));
}
