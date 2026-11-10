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
    /// Pictures on lane `lane` (0 the lowest) of the overlay track, `len`
    /// seconds each, one after another from edit time `at`, each where it
    /// fits. Gives the last one.
    AddPictures {
        inputs: Vec<PathBuf>,
        at: f64,
        len: f64,
        #[serde(default)]
        lane: usize,
    },
    /// Text on lane `lane` of the text track at edit time `at` for `len`
    /// seconds, or where it fits nearest.
    AddText {
        text: Text,
        at: f64,
        len: f64,
        #[serde(default)]
        lane: usize,
    },
    /// Cinematic bars on lane `lane` of the overlay track at edit time
    /// `at` for `len` seconds, or where they fit nearest (gap 15).
    AddBars {
        bars: crate::overlay::Bars,
        at: f64,
        len: f64,
        #[serde(default)]
        lane: usize,
    },
    /// An empty lane on top of `track`'s (gap 70): what's on it is drawn
    /// over the lanes below.
    AddLane { track: Track },
    /// Lane `lane` of `track` taken away: an empty one only, and never the
    /// last.
    RemoveLane { track: Track, lane: usize },
    /// A lane's name (gap 97); none or empty: its track's and number.
    LaneName { track: Track, lane: usize, name: Option<String> },
    /// A lane left out of the picture, or shown again.
    LaneHidden { track: Track, lane: usize, hidden: bool },
    /// A bars item's ratio, slide and colour, all of it.
    SetBars { item: Id, bars: crate::overlay::Bars },
    /// A picture dressed as a sticker, a polaroid or a picture in
    /// picture, or plain (`None`).
    Kind { item: Id, kind: Option<crate::dress::Kind> },
    /// A style look over a text's or a picture's own picture (old film,
    /// VHS, glow…), as `CutOp::Style`; none: off.
    Style { item: Id, look: Option<crate::transition::Look> },
    /// One of its style's settings, as `CutOp::StyleKnob`.
    StyleKnob { item: Id, key: String, value: Option<f32> },
    /// To start at edit time `at`, or where it fits nearest; onto lane
    /// `lane` of its track if given.
    Move {
        item: Id,
        at: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lane: Option<usize>,
    },
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
        ItemOp::AddPictures { inputs, at, len, lane } => {
            positive(*len)?;
            reachable(p, Track::Overlay, *lane)?;
            let mut at = *at;
            let mut last = None;
            for input in inputs {
                let id = p.next_id();
                at = add(p, Track::Overlay, *lane, Item { id, input: input.clone(), text: None, at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None });
                last = Some(id);
            }
            let Some(id) = last else { return Err(OpError::Invalid("no pictures to add".into())) };
            Ok(Outcome { summary: format!("{} picture(s) added, the last item #{id}", inputs.len()), made: Some(id) })
        }
        ItemOp::AddText { text, at, len, lane } => {
            positive(*len)?;
            reachable(p, Track::Text, *lane)?;
            let id = p.next_id();
            add(p, Track::Text, *lane, Item { id, input: PathBuf::new(), text: Some(text.clone()), at: *at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: None, automation: Vec::new(), kind: None, style: None });
            Ok(Outcome { summary: format!("text item #{id} added"), made: Some(id) })
        }
        ItemOp::AddBars { bars, at, len, lane } => {
            positive(*len)?;
            reachable(p, Track::Overlay, *lane)?;
            check_bars(bars)?;
            let id = p.next_id();
            add(p, Track::Overlay, *lane, Item { id, input: PathBuf::new(), text: None, at: *at, start: 0.0, end: *len, motion: Vec::new(), effects: Vec::new(), bars: Some(*bars), automation: Vec::new(), kind: None, style: None });
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
        &ItemOp::Style { item, look } => {
            let it = styleable(p, item)?;
            super::cuts::set_style(&mut it.style, look)?;
            said(format!("item #{item}: style {}", look.map_or("none", |l| l.name())))
        }
        ItemOp::StyleKnob { item, key, value } => {
            let style = styleable(p, *item)?.style.as_mut().ok_or_else(|| OpError::Invalid(format!("item #{item} has no style to set")))?;
            said(format!("item #{item}: {}", super::cuts::set_knob(style.look, &mut style.params, key, *value)?))
        }
        &ItemOp::Move { item, at, lane: onto } => {
            let (track, l, i) = find(p, item)?;
            match onto.filter(|&to| to != l) {
                Some(to) => {
                    if to >= lanes(p, track).len() {
                        return Err(OpError::NotFound(format!("lane {to} of the {track:?} track")));
                    }
                    let moved = lanes(p, track)[l].items.remove(i);
                    let lane = &mut lanes(p, track)[to];
                    lane.items.push(moved);
                    let i = lane.items.len() - 1;
                    lane.items[i].at = lane.place(i, at);
                    lane.items.sort_by(|a, b| a.at.total_cmp(&b.at));
                    let now = lane.items.iter().find(|it| it.id == item).map_or(at, |it| it.at);
                    said(format!("item #{item} on lane {to} at {now:.2} s"))
                }
                None => {
                    let lane = &mut lanes(p, track)[l];
                    lane.move_to(i, at);
                    said(format!("item #{item} at {:.2} s", lane.items[i].at))
                }
            }
        }
        &ItemOp::AddLane { track } => {
            shown(p, track).push(crate::overlay::Lane::default());
            said(format!("{track:?} track: lane {} added on top", lanes(p, track).len() - 1))
        }
        ItemOp::LaneName { track, lane, name } => {
            let l = shown(p, *track).get_mut(*lane).ok_or_else(|| OpError::NotFound(format!("lane {lane} of the {track:?} track")))?;
            l.name = name.as_ref().map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
            said(format!("{track:?} lane {lane}: {}", l.name.as_deref().unwrap_or("no name")))
        }
        &ItemOp::LaneHidden { track, lane, hidden } => {
            let l = shown(p, track).get_mut(lane).ok_or_else(|| OpError::NotFound(format!("lane {lane} of the {track:?} track")))?;
            l.hidden = hidden;
            said(format!("{track:?} lane {lane}: {}", if hidden { "hidden" } else { "shown" }))
        }
        &ItemOp::RemoveLane { track, lane } => {
            let all = lanes(p, track);
            match all.get(lane) {
                None => return Err(OpError::NotFound(format!("lane {lane} of the {track:?} track"))),
                Some(_) if all.len() == 1 => return Err(OpError::Invalid("a track keeps one lane".into())),
                Some(l) if !l.items.is_empty() => return Err(OpError::Invalid(format!("lane {lane} isn't empty"))),
                Some(_) => {}
            }
            all.remove(lane);
            said(format!("{track:?} track: lane {lane} taken away"))
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
fn add(p: &mut Project, track: Track, l: usize, item: Item) -> f64 {
    let lane = p.lane_mut(track, l);
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
/// Whether something can be added on lane `l` of `track`: one there, or
/// the next on top. A far one would make every lane below it (audit A6:
/// `lane: 1e9` ran out of memory).
fn reachable(p: &Project, track: Track, l: usize) -> Result<(), OpError> {
    let n = p.track_lanes(track).len().max(1);
    if l > n {
        return Err(OpError::Invalid(format!("lane {l} of the {track:?} track: it has {n}, a new one goes at {n}")));
    }
    Ok(())
}

/// `track`'s lanes as the timeline shows them: one at least, made if the
/// project has none yet (a new project's lane is shown before anything is
/// on it; naming, hiding or adding over it failed, audit A3).
fn shown(p: &mut Project, track: Track) -> &mut Vec<crate::overlay::Lane> {
    p.lane_mut(track, 0);
    lanes(p, track)
}

/// Item `id`, a text or a picture (bars take no style).
fn styleable(p: &mut Project, id: Id) -> Result<&mut Item, OpError> {
    let (track, lane, i) = find(p, id)?;
    let it = &mut lanes(p, track)[lane].items[i];
    if it.bars.is_some() {
        return Err(OpError::Invalid(format!("item #{id} is bars: no style")));
    }
    Ok(it)
}

fn find(p: &Project, id: Id) -> Result<(Track, usize, usize), OpError> {
    p.item_index(id).ok_or_else(|| OpError::NotFound(format!("item #{id}")))
}

#[cfg(test)]
mod tests {
    use super::super::{Op, check};
    use super::*;
    use crate::model::edit::Edit;

    #[test]
    fn a_new_projects_shown_lane_can_be_named_hidden_and_added_over() {
        let mut e = Edit::default();
        e.apply(&Op::Item(ItemOp::LaneName { track: Track::Text, lane: 0, name: Some("Titles".into()) })).unwrap();
        e.apply(&Op::Item(ItemOp::LaneHidden { track: Track::Overlay, lane: 0, hidden: true })).unwrap();
        e.apply(&Op::Item(ItemOp::AddLane { track: Track::Overlay })).unwrap();
        assert_eq!((e.text[0].name.as_deref(), e.overlays.len(), e.overlays[0].hidden), (Some("Titles"), 2, true), "the one shown, and one over it");
    }

    #[test]
    fn a_track_holds_lanes_one_over_another() {
        let mut e = Edit::default();
        let add = |lane| Op::Item(ItemOp::AddText { text: Text::default(), at: 1.0, len: 3.0, lane });
        let low = e.apply(&add(0)).unwrap().made.unwrap();
        e.apply(&Op::Item(ItemOp::AddLane { track: Track::Text })).unwrap();
        let high = e.apply(&add(1)).unwrap().made.unwrap();
        assert_eq!((e.text.len(), e.text[1].items[0].at), (2, 1.0), "the same time, a lane above");
        // Moved down onto the low lane: where it fits there.
        e.apply(&Op::Item(ItemOp::Move { item: high, at: 1.0, lane: Some(0) })).unwrap();
        assert_eq!((e.text[0].items.len(), e.text[1].items.len(), e.item_index(high)), (2, 0, Some((Track::Text, 0, 1))));
        assert_eq!(e.text[0].items[1].at, 4.0, "after the one already there");
        e.apply(&Op::Item(ItemOp::LaneName { track: Track::Text, lane: 1, name: Some(" Subtitles ".into()) })).unwrap();
        e.apply(&Op::Item(ItemOp::LaneHidden { track: Track::Text, lane: 0, hidden: true })).unwrap();
        assert_eq!((e.text[1].name.as_deref(), e.shown_lanes().count()), (Some("Subtitles"), 1), "named, the hidden lane not drawn");
        assert_eq!(e.shown_lane_refs().map(|(t, l, _)| (t, l)).collect::<Vec<_>>(), [(Track::Text, 1)], "nor reached by the handles");
        e.apply(&Op::Item(ItemOp::LaneHidden { track: Track::Text, lane: 0, hidden: false })).unwrap();
        e.apply(&Op::Item(ItemOp::RemoveLane { track: Track::Text, lane: 1 })).unwrap();
        assert!(e.apply(&Op::Item(ItemOp::RemoveLane { track: Track::Text, lane: 0 })).is_err(), "the last stays");
        assert!(e.apply(&Op::Item(ItemOp::Move { item: low, at: 0.0, lane: Some(3) })).is_err(), "no such lane");
        assert!(e.apply(&add(5)).is_err(), "not past the next on top");
        check(&e).unwrap();
    }

    #[test]
    fn pictures_follow_one_another_and_text_is_set_whole() {
        let mut e = Edit::default();
        let last = e.apply(&Op::Item(ItemOp::AddPictures { inputs: vec!["a.png".into(), "b.png".into()], at: 1.0, len: 5.0, lane: 0 })).unwrap().made.unwrap();
        let lane = &e.overlays[0].items;
        assert_eq!((lane[0].at, lane[1].at, lane[1].id), (1.0, 6.0, last));
        e.apply(&Op::Item(ItemOp::Kind { item: last, kind: Some(crate::dress::Kind::Polaroid) })).unwrap();
        assert_eq!(e.overlays[0].items[1].kind, Some(crate::dress::Kind::Polaroid));
        use crate::transition::Look;
        e.apply(&Op::Item(ItemOp::Style { item: last, look: Some(Look::Vhs) })).unwrap();
        e.apply(&Op::Item(ItemOp::StyleKnob { item: last, key: "amount".into(), value: Some(0.5) })).unwrap();
        assert_eq!(e.overlays[0].items[1].style.as_ref().map(|s| (s.look, s.params.len())), Some((Look::Vhs, 1)));
        assert!(e.apply(&Op::Item(ItemOp::Style { item: last, look: Some(Look::WhipPan) })).is_err(), "a transition isn't a style");
        let words = e.apply(&Op::Item(ItemOp::AddText { text: Text::default(), at: 2.0, len: 3.0, lane: 0 })).unwrap().made.unwrap();
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
        e.apply(&Op::Item(ItemOp::Move { item: first, at: 3.0, lane: None })).unwrap();
        assert!(e.overlays[0].items[0].ends() <= e.overlays[0].items[1].at + 1e-9);
        e.apply(&Op::Item(ItemOp::Trim { item: last, end: End::Out, t: 99.0, most: 3600.0 })).unwrap();
        e.apply(&Op::Item(ItemOp::Delete { item: first })).unwrap();
        assert!(matches!(e.apply(&Op::Item(ItemOp::Delete { item: first })), Err(OpError::NotFound(_))));
        check(&e).unwrap();
    }

    #[test]
    fn bars_are_added_set_and_kept_in_range() {
        let mut e = Edit::default();
        let id = e.apply(&Op::Item(ItemOp::AddBars { bars: Default::default(), at: 2.0, len: 5.0, lane: 0 })).unwrap().made.unwrap();
        let bars = |ratio| crate::overlay::Bars { ratio, colour: [1.0, 1.0, 1.0], ..Default::default() };
        e.apply(&Op::Item(ItemOp::SetBars { item: id, bars: bars(2.0) })).unwrap();
        assert_eq!(e.overlays[0].items[0].bars, Some(bars(2.0)));
        assert!(e.apply(&Op::Item(ItemOp::Kind { item: id, kind: Some(crate::dress::Kind::Sticker) })).is_err(), "bars aren't a picture to dress");
        assert!(e.apply(&Op::Item(ItemOp::SetBars { item: id, bars: bars(9.0) })).is_err(), "out of range");
        let text = e.apply(&Op::Item(ItemOp::AddText { text: Default::default(), at: 0.0, len: 2.0, lane: 0 })).unwrap().made.unwrap();
        assert!(e.apply(&Op::Item(ItemOp::SetBars { item: text, bars: bars(2.0) })).is_err(), "text isn't bars");
    }

}
