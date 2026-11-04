//! Random runs of ops on a sheet: after each, the sheet has no problem
//! `check` finds, reads back from its own text as it is, and undo walks
//! back through every state it was in. Deeper runs:
//! `PROPTEST_CASES=20000 cargo test --test props`.

use proptest::prelude::*;
use dailix_sheet::{ClipOp, DayOp, Doc, Edit, MarkOp, Op, RoleOp};

const CLIPS: [&str; 3] = ["2030-05-01/a.mp4", "2030-05-01/b.mp4", "2030-05-02/c.mov"];

fn time() -> impl Strategy<Value = f64> {
    (0u32..4000).prop_map(|n| n as f64 / 100.0)
}

fn mark() -> impl Strategy<Value = String> {
    (1u32..8).prop_map(|n| format!("m{n}"))
}

fn role() -> impl Strategy<Value = Option<String>> {
    prop_oneof![Just(None), Just(Some("broll".to_owned())), Just(Some("talk".to_owned())), Just(Some("x_мем".to_owned())), Just(Some("nonsense".to_owned()))]
}

fn op() -> impl Strategy<Value = Op> {
    let clip = proptest::sample::select(CLIPS.to_vec()).prop_map(str::to_owned);
    prop_oneof![
        (clip.clone(), time(), proptest::option::of(time()), any::<bool>(), role(), ".{0,6}").prop_map(|(clip, t, end, flag, role, note)| Op::Mark(MarkOp::Add { clip, t, end, flag, role, note })),
        (mark(), time(), proptest::option::of(time())).prop_map(|(mark, t, end)| Op::Mark(MarkOp::Move { mark, t, end })),
        (mark(), role()).prop_map(|(mark, role)| Op::Mark(MarkOp::Role { mark, role })),
        (mark(), ".{0,6}").prop_map(|(mark, note)| Op::Mark(MarkOp::Note { mark, note })),
        mark().prop_map(|mark| Op::Mark(MarkOp::Remove { mark })),
        (clip.clone(), proptest::option::of(0u8..4)).prop_map(|(clip, rating)| Op::Clip(ClipOp::Rate { clip, rating })),
        (clip.clone(), ".{0,6}").prop_map(|(clip, note)| Op::Clip(ClipOp::Note { clip, note })),
        (clip, proptest::option::of("dnn6|dnn12")).prop_map(|(clip, audio)| Op::Clip(ClipOp::Voice { clip, audio })),
        ("2030-05-0[1-3]", ".{0,6}").prop_map(|(day, note)| Op::Day(DayOp::Note { day, note })),
        "мем|жарт|Meme".prop_map(|title| Op::Role(RoleOp::Add { title, en: String::new() })),
    ]
}

proptest! {
    #[test]
    fn runs_of_ops_keep_the_sheet_sound_and_undo_walks_back(ops in proptest::collection::vec(op(), 1..40)) {
        let mut edit = Edit::new(Doc::parse("{}").unwrap());
        let mut states = vec![edit.doc().text()];
        for op in &ops {
            if edit.apply(op).is_ok() && edit.doc().text() != *states.last().unwrap() {
                states.push(edit.doc().text());
            }
            let doc = edit.doc();
            prop_assert!(doc.check().is_empty(), "{:?} after {:?}", doc.check(), op);
            prop_assert_eq!(Doc::parse(&doc.text()).unwrap().text(), doc.text());
        }
        // Undo back through every state, then redo forward again.
        for state in states.iter().rev().skip(1) {
            prop_assert!(edit.undo());
            prop_assert_eq!(&edit.doc().text(), state);
        }
        prop_assert!(!edit.undo());
        for state in states.iter().skip(1) {
            prop_assert!(edit.redo());
            prop_assert_eq!(&edit.doc().text(), state);
        }
    }
}
