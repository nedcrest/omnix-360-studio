//! Operations on overlay and text items: add, move, trim, the text's words
//! and look, delete (`OPS-API.md` step 4). An item's place and effects
//! over time are keyframes (step 5).

use super::{End, OpError, Outcome, said};
use crate::model::edit::MIN_CUT;
use crate::overlay::{Item, Track};
use crate::project::{Id, Project};
use crate::text::Text;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemOp {
    /// Pictures on the overlay track, `len` seconds each, one after
    /// another from edit time `at`, each where it fits. Gives the last one.
    AddPictures { inputs: Vec<PathBuf>, at: f64, len: f64 },
    /// Text on the text track at edit time `at` for `len` seconds, or where
    /// it fits nearest.
    AddText { text: Text, at: f64, len: f64 },
    /// Cinematic bars on the overlay track at edit time `at` for `len`
    /// seconds, or where they fit nearest (gap 15).
    AddBars { bars: crate::overlay::Bars, at: f64, len: f64 },
    /// A bars item's ratio, slide and colour, all of it.
    SetBars { item: Id, bars: crate::overlay::Bars },
    /// A picture dressed as a sticker, a polaroid or a picture in
    /// picture, or plain (`None`).
    Kind { item: Id, kind: Option<crate::dress::Kind> },
    /// To start at edit time `at`, or where it fits nearest.
    Move { item: Id, at: f64 },
    /// Its in or out edge to `t` (its own time), staying where it is on the
    /// edit; no longer than `most` from its start.
    Trim { item: Id, end: End, t: f64, most: f64 },
    /// A text item's words and look, all of it.
    SetText { item: Id, text: Text },
    Delete { item: Id },
}

/// Bars wider than tall, a slide of none or more, a colour in 0..1.
fn check_bars(b: &crate::overlay::Bars) -> Result<(), OpError> {
    if !(1.0..=4.0).contains(&b.ratio) {
        return Err(OpError::OutOfRange { what: "bars' ratio", value: b.ratio, least: 1.0, most: 4.0 });
    }
    if !(0.0..=10.0).contains(&b.slide) {
        return Err(OpError::OutOfRange { what: "bars' slide", value: b.slide, least: 0.0, most: 10.0 });
    }
    if b.colour.iter().any(|c| !(0.0..=1.0).contains(c)) {
        return Err(OpError::Invalid("a colour runs 0..1".into()));
    }
    Ok(())
}

pub(super) fn apply(p: &mut Project, op: &ItemOp) -> Result<Outcome, OpError> {
    match op {
        ItemOp::AddPictures { inputs, at, len } => {
            positive(*len)?;
            let mut at = *at;
            let mut last = None;
            for input in inputs {
                let id = p.next_id();
                at = add(p, Track::Overlay, Item { id, input: input.clone(), text: None, at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None });
                last = Some(id);
            }
            let Some(id) = last else { return Err(OpError::Invalid("no pictures to add".into())) };
            Ok(Outcome { summary: format!("{} picture(s) added, the last item #{id}", inputs.len()), made: Some(id) })
        }
        ItemOp::AddText { text, at, len } => {
            positive(*len)?;
            let id = p.next_id();
            add(p, Track::Text, Item { id, input: PathBuf::new(), text: Some(text.clone()), at: *at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None });
            Ok(Outcome { summary: format!("text item #{id} added"), made: Some(id) })
        }
        ItemOp::AddBars { bars, at, len } => {
            positive(*len)?;
            check_bars(bars)?;
            let id = p.next_id();
            add(p, Track::Overlay, Item { id, input: PathBuf::new(), text: None, at: *at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: Some(*bars), automation: Vec::new(), kind: None });
            Ok(Outcome { summary: format!("bars {:.2}:1, item #{id}", bars.ratio), made: Some(id) })
        }
        ItemOp::SetBars { item, bars } => {
            check_bars(bars)?;
            let (track, lane, i) = find(p, *item)?;
            let it = &mut lanes(p, track)[lane].items[i];
            if it.bars.is_none() {
                return Err(OpError::Invalid(format!("item #{item} isn't bars")));
            }
            it.bars = Some(*bars);
            said(format!("item #{item}: bars {:.2}:1", bars.ratio))
        }
        &ItemOp::Kind { item, kind } => {
            let (track, lane, i) = find(p, item)?;
            let it = &mut lanes(p, track)[lane].items[i];
            if it.file().is_none() {
                return Err(OpError::Invalid(format!("item #{item} isn't a picture")));
            }
            it.kind = kind;
            said(format!("item #{item}: {}", kind.map_or("plain", |k| k.name())))
        }
        &ItemOp::Move { item, at } => {
            let (track, lane, i) = find(p, item)?;
            let lane = &mut lanes(p, track)[lane];
            lane.move_to(i, at);
            said(format!("item #{item} at {:.2} s", lane.items[i].at))
        }
        &ItemOp::Trim { item, end, t, most } => {
            let (track, lane, i) = find(p, item)?;
            let lane = &mut lanes(p, track)[lane];
            let was = (lane.items[i].start, lane.items[i].end);
            lane.trim(i, end == End::In, t, MIN_CUT, most);
            let it = &mut lane.items[i];
            // Its ins and outs stay on their edges.
            crate::animate::keep_on_edges(&mut it.automation, was, (it.start, it.end));
            said(format!("item #{item}: {:.2}-{:.2} s on the edit", it.at, it.ends()))
        }
        ItemOp::SetText { item, text } => {
            let (track, lane, i) = find(p, *item)?;
            let it = &mut lanes(p, track)[lane].items[i];
            if it.text.is_none() {
                return Err(OpError::Invalid(format!("item #{item} is a picture, not text")));
            }
            it.text = Some(text.clone());
            said(format!("text item #{item}: «{}»", text.content))
        }
        &ItemOp::Delete { item } => {
            let (track, lane, i) = find(p, item)?;
            lanes(p, track)[lane].items.remove(i);
            said(format!("item #{item} removed"))
        }
    }
}

fn positive(len: f64) -> Result<(), OpError> {
    if len >= MIN_CUT { Ok(()) } else { Err(OpError::OutOfRange { what: "length", value: len, least: MIN_CUT, most: f64::MAX }) }
}

/// Puts `item` on the lane of `track` the player shows, nearest its `at`
/// where it fits; returns where it ends.
fn add(p: &mut Project, track: Track, item: Item) -> f64 {
    let lane = p.lane_mut(track);
    lane.items.push(item);
    let i = lane.items.len() - 1;
    lane.items[i].at = lane.place(i, lane.items[i].at);
    lane.items[i].ends()
}

fn lanes(p: &mut Project, track: Track) -> &mut Vec<crate::overlay::Lane> {
    match track {
        Track::Overlay => &mut p.overlays,
        Track::Text => &mut p.text,
    }
}

/// Which track, lane and place item `id` is in.
fn find(p: &Project, id: Id) -> Result<(Track, usize, usize), OpError> {
    p.item_index(id).ok_or_else(|| OpError::NotFound(format!("item #{id}")))
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;

    #[test]
    fn pictures_follow_one_another_and_text_is_set_whole() {
        let mut e = Edit::default();
        let last = e.apply(&Op::Item(ItemOp::AddPictures { inputs: vec!["a.png".into(), "b.png".into()], at: 1.0, len: 5.0 })).unwrap().made.unwrap();
        let lane = &e.overlays[0].items;
        assert_eq!((lane[0].at, lane[1].at, lane[1].id), (1.0, 6.0, last));
        e.apply(&Op::Item(ItemOp::Kind { item: last, kind: Some(crate::dress::Kind::Polaroid) })).unwrap();
        assert_eq!(e.overlays[0].items[1].kind, Some(crate::dress::Kind::Polaroid));
        let words = e.apply(&Op::Item(ItemOp::AddText { text: Text::default(), at: 2.0, len: 3.0 })).unwrap().made.unwrap();
        let text = Text { content: "Kos".into(), bold: false, ..Text::default() };
        let set = Op::Item(ItemOp::SetText { item: words, text: text.clone() });
        e.apply(&set).unwrap();
        e.apply(&set).unwrap();
        assert_eq!(e.text[0].items[0].text.as_ref(), Some(&text));
        assert!(e.undo() && e.text[0].items[0].text == Some(Text::default()), "set twice is one step: the second changed nothing");
        e.redo();
        assert!(matches!(e.apply(&Op::Item(ItemOp::SetText { item: last, text })), Err(OpError::Invalid(_))));
        // A move into the next picture stops at its edge.
        let first = e.overlays[0].items[0].id;
        e.apply(&Op::Item(ItemOp::Move { item: first, at: 3.0 })).unwrap();
        assert!(e.overlays[0].items[0].ends() <= e.overlays[0].items[1].at + 1e-9);
        e.apply(&Op::Item(ItemOp::Trim { item: last, end: End::Out, t: 99.0, most: 3600.0 })).unwrap();
        e.apply(&Op::Item(ItemOp::Delete { item: first })).unwrap();
        assert!(matches!(e.apply(&Op::Item(ItemOp::Delete { item: first })), Err(OpError::NotFound(_))));
        check(&e).unwrap();
    }

    #[test]
    fn bars_are_added_set_and_kept_in_range() {
        let mut e = Edit::default();
        let id = e.apply(&Op::Item(ItemOp::AddBars { bars: Default::default(), at: 2.0, len: 5.0 })).unwrap().made.unwrap();
        let bars = |ratio| crate::overlay::Bars { ratio, colour: [1.0, 1.0, 1.0], ..Default::default() };
        e.apply(&Op::Item(ItemOp::SetBars { item: id, bars: bars(2.0) })).unwrap();
        assert_eq!(e.overlays[0].items[0].bars, Some(bars(2.0)));
        assert!(e.apply(&Op::Item(ItemOp::Kind { item: id, kind: Some(crate::dress::Kind::Sticker) })).is_err(), "bars aren't a picture to dress");
        assert!(e.apply(&Op::Item(ItemOp::SetBars { item: id, bars: bars(9.0) })).is_err(), "out of range");
        let text = e.apply(&Op::Item(ItemOp::AddText { text: Default::default(), at: 0.0, len: 2.0 })).unwrap().made.unwrap();
        assert!(e.apply(&Op::Item(ItemOp::SetBars { item: text, bars: bars(2.0) })).is_err(), "text isn't bars");
    }

}
