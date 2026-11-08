//! Random sequences of ops on a small project (`OPS-API.md` step 6): what
//! must hold whatever an agent or a script sends, in any order, naming
//! things that are there or not. proptest shrinks a failing sequence to
//! the fewest ops that still fail; the test prints it as a script.

use super::clips::ClipOp;
use super::cuts::{CutOp, Place};
use super::items::ItemOp;
use super::keys::{Graded, KeyOp, Line};
use super::sound::SoundOp;
use super::{End, Op, SoundRef, check};
use crate::grade::Grade;
use crate::model::edit::Edit;
use crate::overlay::{Item, Lane, Place as Spot};
use crate::project::{Cut, MusicClip, Project, SoundTrack};
use crate::view::{Ease, Follow, Projection, View};
use proptest::prelude::*;

/// Three cuts, a song, a sound effect, a picture and a title: ids 1 to 7.
fn project() -> Project {
    let mut p = Project { cuts: vec![Cut::new("a.mp4".into(), 0.0, 4.0), Cut::new("b.mp4".into(), 1.0, 4.0), Cut::new("c.mp4".into(), 0.0, 2.0)], ..Default::default() };
    p.music.clips.push(MusicClip { input: "song.wav".into(), at: 0.0, start: 0.0, end: 9.0, ..Default::default() });
    p.sfx.clips.push(MusicClip { input: "bell.wav".into(), at: 2.0, start: 0.0, end: 1.0, ..Default::default() });
    let item = |text: bool, at: f64| Item { id: 0, input: if text { "".into() } else { "logo.png".into() }, text: text.then(Default::default), at, start: 0.0, end: 3.0, motion: Vec::new(), effects: Vec::new() };
    p.overlays.push(Lane { items: vec![item(false, 0.0)] });
    p.text.push(Lane { items: vec![item(true, 1.0)] });
    p.settle_ids();
    p
}

/// An id that's there, or past them (the ops add a few), or none.
fn id() -> impl Strategy<Value = u32> {
    0u32..12
}

fn time() -> impl Strategy<Value = f64> {
    -1.0..12.0f64
}

fn end() -> impl Strategy<Value = End> {
    prop_oneof![Just(End::In), Just(End::Out)]
}

fn ease() -> impl Strategy<Value = Ease> {
    prop_oneof![Just(Ease::Smooth), Just(Ease::Linear), Just(Ease::Hold)]
}

fn sound() -> impl Strategy<Value = SoundRef> {
    prop_oneof![id().prop_map(SoundRef::Cut), id().prop_map(SoundRef::Clip)]
}

fn track() -> impl Strategy<Value = SoundTrack> {
    prop::sample::select(SoundTrack::ALL.to_vec())
}

fn graded() -> impl Strategy<Value = Graded> {
    prop_oneof![id().prop_map(Graded::Cut), id().prop_map(Graded::Item)]
}

fn line() -> impl Strategy<Value = Line> {
    prop_oneof![id().prop_map(Line::View), graded().prop_map(Line::Effects), id().prop_map(Line::Motion)]
}

fn cut_op() -> impl Strategy<Value = CutOp> {
    let place = prop_oneof![Just(Place::End), time().prop_map(Place::At), time().prop_map(Place::Join)];
    prop_oneof![
        (0.0..1.0f64, 0.0..5.0f64, place).prop_map(|(start, len, place)| CutOp::Add { input: "x.mp4".into(), start, end: start + len, place, stabilize: None, lut: None }),
        (id(), end(), time()).prop_map(|(cut, end, t)| CutOp::Trim { cut, end, t, most: 6.0 }),
        (id(), prop::option::of(id())).prop_map(|(cut, before)| CutOp::Move { cut, before }),
        (id(), time()).prop_map(|(cut, t)| CutOp::Split { cut, t }),
        (id(), 0.0..4.0f64).prop_map(|(cut, seconds)| CutOp::Crossfade { cut, seconds }),
        (id(), prop::option::of(prop::sample::select(crate::transition::Look::OVERLAP.to_vec()))).prop_map(|(cut, look)| CutOp::Transition { cut, look }),
        (id(), end(), prop::option::of(prop::sample::select(crate::transition::Look::EDGES.to_vec())), 0.0..4.0f64).prop_map(|(cut, end, look, seconds)| CutOp::Edge { cut, end, look, seconds }),
        id().prop_map(|cut| CutOp::Delete { cut }),
        (id(), time(), "[a-z]{0,6}").prop_map(|(cut, t, note)| CutOp::Mark { cut, t, note }),
        (id(), time(), time()).prop_map(|(cut, from, t)| CutOp::MoveMark { cut, from, t }),
        (id(), time()).prop_map(|(cut, t)| CutOp::Unmark { cut, t }),
        (id(), end(), time()).prop_map(|(cut, end, t)| CutOp::SoundEdge { cut, end, t, most: 6.0 }),
        id().prop_map(|cut| CutOp::SoundToPicture { cut }),
    ]
}

fn clip_op() -> impl Strategy<Value = ClipOp> {
    prop_oneof![
        (track(), time(), 0.0..4.0f64).prop_map(|(track, at, len)| ClipOp::Add { track, input: "s.wav".into(), at, start: 0.0, end: len }),
        (id(), time(), any::<bool>()).prop_map(|(clip, at, overlap)| ClipOp::Move { clip, at, overlap }),
        (id(), end(), time(), any::<bool>()).prop_map(|(clip, end, t, overlap)| ClipOp::Trim { clip, end, t, most: 9.0, overlap }),
        (id(), time()).prop_map(|(clip, at)| ClipOp::Split { clip, at }),
        id().prop_map(|clip| ClipOp::OtherSfx { clip }),
        id().prop_map(|clip| ClipOp::Delete { clip }),
    ]
}

fn sound_op() -> impl Strategy<Value = SoundOp> {
    prop_oneof![
        (sound(), -50.0..20.0f64).prop_map(|(sound, db)| SoundOp::Level { sound, db }),
        (sound(), end(), -1.0..5.0f64).prop_map(|(sound, end, seconds)| SoundOp::Fade { sound, end, seconds }),
        (sound(), any::<bool>()).prop_map(|(sound, on)| SoundOp::Mute { sound, on }),
        sound().prop_map(|sound| SoundOp::ShowLine { sound }),
        sound().prop_map(|sound| SoundOp::DeleteLine { sound }),
        sound().prop_map(|sound| SoundOp::Original { sound }),
        sound().prop_map(|sound| SoundOp::LowerForVoice { sound }),
        (sound(), time(), -40.0..6.0f64).prop_map(|(sound, t, db)| SoundOp::AddPoint { sound, t, db }),
        (sound(), time()).prop_map(|(sound, t)| SoundOp::PointAt { sound, t }),
        (sound(), time(), time(), -40.0..6.0f64).prop_map(|(sound, from, t, db)| SoundOp::MovePoint { sound, from, t, db }),
        (sound(), time()).prop_map(|(sound, t)| SoundOp::DeletePoint { sound, t }),
        sound().prop_map(|sound| SoundOp::ContinueLevel { sound }),
        (sound(), prop::option::of(-40.0..-5.0f64), any::<bool>()).prop_map(|(sound, lufs, even)| SoundOp::Normalized { sound, lufs, even }),
        (0.0..30.0f64, prop::collection::vec((time(), 0.0..3.0f64).prop_map(|(a, l)| (a, a + l)), 0..3)).prop_map(|(depth, speech)| SoundOp::Duck { depth, speech }),
    ]
}

fn item_op() -> impl Strategy<Value = ItemOp> {
    prop_oneof![
        (time(), 0.0..5.0f64).prop_map(|(at, len)| ItemOp::AddPictures { inputs: vec!["p.png".into(), "q.png".into()], at, len }),
        (time(), 0.0..5.0f64).prop_map(|(at, len)| ItemOp::AddText { text: Default::default(), at, len }),
        (id(), time()).prop_map(|(item, at)| ItemOp::Move { item, at }),
        (id(), end(), time()).prop_map(|(item, end, t)| ItemOp::Trim { item, end, t, most: 3600.0 }),
        (id(), "[a-z ]{0,8}").prop_map(|(item, content)| ItemOp::SetText { item, text: crate::text::Text { content, ..Default::default() } }),
        id().prop_map(|item| ItemOp::Delete { item }),
    ]
}

fn key_op() -> impl Strategy<Value = KeyOp> {
    let tolerance = 0.0..0.1f64;
    prop_oneof![
        (id(), time(), -90.0..90.0f64, tolerance.clone()).prop_map(|(cut, t, yaw, tolerance)| KeyOp::View { cut, t, view: View { yaw, ..View::home(Projection::Perspective) }, tolerance, aims: vec![(Follow::Stick, Some((10.0, -5.0)))] }),
        (id(), time(), any::<bool>(), tolerance.clone()).prop_map(|(cut, t, on, tolerance)| KeyOp::Follow { cut, t, follow: on.then_some(Follow::Stick), tolerance, aims: vec![(Follow::Stick, None)] }),
        (graded(), time(), -2.0..2.0f64, tolerance.clone()).prop_map(|(on, t, exposure, tolerance)| KeyOp::Effects { on, t, grade: Grade { exposure, ..Grade::default() }, tolerance }),
        (id(), time(), 0.0..1.0f64, tolerance.clone()).prop_map(|(item, t, x, tolerance)| KeyOp::Motion { item, t, place: Spot { x, ..Spot::default() }, tolerance }),
        (line(), time(), ease(), tolerance.clone()).prop_map(|(line, t, ease, tolerance)| KeyOp::Ease { line, t, ease, tolerance }),
        (line(), time(), time(), 0.0..0.5f64, tolerance.clone()).prop_map(|(line, from, t, gap, tolerance)| KeyOp::Retime { line, from, t, gap, tolerance }),
        (line(), time(), tolerance).prop_map(|(line, t, tolerance)| KeyOp::Delete { line, t, tolerance }),
        graded().prop_map(|on| KeyOp::Clear { on }),
        (id(), time()).prop_map(|(cut, t)| KeyOp::LookToAll { cut, t }),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![cut_op().prop_map(Op::Cut), clip_op().prop_map(Op::Clip), sound_op().prop_map(Op::Sound), item_op().prop_map(Op::Item), key_op().prop_map(Op::Key)]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, ..ProptestConfig::default() })]

    /// Whatever is sent: the project stays whole, a refused op changes
    /// nothing, undo goes all the way back and redo all the way forward,
    /// and every op reads back from JSON as it was.
    #[test]
    fn any_sequence_of_ops_keeps_the_project_whole_and_undoable(ops in prop::collection::vec(op(), 1..25)) {
        let mut e = Edit::default();
        e.replace(project());
        let start = Project::clone(&e);
        for op in &ops {
            let json = serde_json::to_string(op).unwrap();
            prop_assert_eq!(&serde_json::from_str::<Op>(&json).unwrap(), op, "{}", json);
            let before = Project::clone(&e);
            let result = e.apply(op);
            if let Err(why) = &result {
                prop_assert_eq!(&Project::clone(&e), &before, "{} refused ({}) but changed the project", json, why);
            }
            if let Err(broken) = check(&e) {
                prop_assert!(false, "{} left it broken: {}", json, broken);
            }
        }
        let end = Project::clone(&e);
        while e.undo() {}
        prop_assert_eq!(&Project::clone(&e), &start, "undo all the way");
        while e.redo() {}
        prop_assert_eq!(&Project::clone(&e), &end, "redo all the way");
    }
}

/// Found by the property test (shrunk as it printed it): a picture squeezed
/// between neighbours with less room than the shortest item, its out edge
/// trimmed in, panicked (`clamp` with its bounds the wrong way round).
#[test]
fn a_squeezed_item_trims_without_a_panic() {
    let mut e = Edit::default();
    e.replace(project());
    let pictures = |at: f64, len: f64| Op::Item(ItemOp::AddPictures { inputs: vec!["p.png".into(), "q.png".into()], at, len });
    e.apply(&pictures(11.758063601041425, 1.6549611253934415)).unwrap();
    e.apply(&pictures(6.476533619232805, 3.4309897559092923)).unwrap();
    e.apply(&Op::Item(ItemOp::Trim { item: 9, end: End::In, t: 11.801837021077244, most: 3600.0 })).unwrap();
    e.apply(&Op::Item(ItemOp::Trim { item: 9, end: End::Out, t: 0.0, most: 3600.0 })).unwrap();
    check(&e).unwrap();
}
