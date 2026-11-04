//! A sheet read and written back unchanged is the same file, byte for
//! byte: other programs write it too, and nothing of theirs may move or go.

use rushix_sheet::{Doc, MarkKind, Rating};

/// A made-up sheet in the form the sheet's server writes (Python's
/// `json.dumps(indent=2, ensure_ascii=False)`), with what's easy to lose:
/// keys in varying order, `4` beside `9.0`, other scripts, emoji, escapes
/// and control characters, and fields this crate doesn't know at every
/// level.
const SAMPLE: &str = include_str!("fixtures/sample.json");

#[test]
fn a_sheet_read_and_written_back_is_the_same_file() {
    let doc = Doc::parse(SAMPLE).unwrap();
    assert_eq!(doc.text(), SAMPLE);
}

#[test]
fn the_typed_view_reads_the_sample() {
    let sheet = Doc::parse(SAMPLE).unwrap().sheet().unwrap();
    assert_eq!(sheet.days.keys().collect::<Vec<_>>(), ["2030-05-01", "2030-05-02"]);
    let first = &sheet.clips["2030-05-01/CAM_0001.MP4"];
    assert_eq!(first.rating, Some(Rating::Take));
    let kinds: Vec<_> = first.marks.iter().map(|m| m.kind).collect();
    assert_eq!(kinds, [MarkKind::Segment, MarkKind::Point, MarkKind::Segment, MarkKind::Flag, MarkKind::Segment]);
    assert_eq!(first.marks[2].end, Some(12.25));
    assert_eq!(sheet.clips["2030-05-01/CAM_0002.MP4"].rating, None);
    assert_eq!(sheet.clips["2030-05-02/PHONE 3.mov"].audio.as_deref(), Some("dnn6"));
    assert_eq!(sheet.roles[0].key, "x_мем");
}

/// A real sheet, named by `RUSHIX_SHEET`, read and written back: run by
/// hand on a copy (`RUSHIX_SHEET=… cargo test -- --ignored`), so no one's
/// marking is in the repo.
#[test]
#[ignore]
fn a_real_sheet_read_and_written_back_is_the_same_file() {
    let path = std::env::var("RUSHIX_SHEET").expect("RUSHIX_SHEET names a copy of a notes.json");
    let text = std::fs::read_to_string(path).unwrap();
    assert!(Doc::parse(&text).unwrap().text() == text, "the round trip changed the file");
}

/// Ids given to a real sheet (`RUSHIX_SHEET`, a copy): only `id` keys are
/// added; taken out again, the file is as it was, byte for byte.
#[test]
#[ignore]
fn ids_given_to_a_real_sheet_add_only_ids() {
    let path = std::env::var("RUSHIX_SHEET").expect("RUSHIX_SHEET names a copy of a notes.json");
    let text = std::fs::read_to_string(path).unwrap();
    let mut doc = Doc::parse(&text).unwrap();
    let given = doc.settle_ids();
    let sheet = doc.sheet().unwrap();
    let marks: Vec<_> = sheet.clips.values().flat_map(|c| &c.marks).collect();
    assert!(marks.iter().all(|m| m.id.is_some()));
    println!("ids given: {given} of {} marks", marks.len());
    // The ids taken out again: the file as it was.
    let mut value: serde_json::Value = serde_json::from_str(&doc.text()).unwrap();
    for clip in value["clips"].as_object_mut().unwrap().values_mut() {
        for m in clip.get_mut("marks").and_then(|m| m.as_array_mut()).into_iter().flatten() {
            m.as_object_mut().unwrap().shift_remove("id");
        }
    }
    assert!(serde_json::to_string_pretty(&value).unwrap() == text, "more than ids changed");
}

/// What `check` finds in a real sheet (`RUSHIX_SHEET`, a copy), printed:
/// problems already there block no op, but they're worth seeing.
#[test]
#[ignore]
fn check_a_real_sheet() {
    let path = std::env::var("RUSHIX_SHEET").expect("RUSHIX_SHEET names a copy of a notes.json");
    let doc = Doc::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    for p in doc.check() {
        println!("problem: {p}");
    }
}
