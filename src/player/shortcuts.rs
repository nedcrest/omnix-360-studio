//! The keyboard shortcuts and mouse gestures, grouped, in a window over
//! the editor (gap 56): opened with F1 or the keyboard button, closed
//! with Esc or a click outside. Hidden until asked for, so the interface
//! stays without help text. Keep it in step with `keys.rs` and the
//! timeline's presses.

use super::{App, Message};
use iced::widget::{button, column, container, row, rule, scrollable, space, text};
use iced::{Alignment, Element, Font, Length};

/// What each group's keys do, as «keys», «what».
const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Play and the playhead",
        &[
            ("Space", "play / pause"),
            ("Shift+Space", "play the range marked on the ruler once (Shift and a drag marks it, Esc lets go)"),
            ("← / →  or  , / .", "a frame back / on"),
            ("Shift+← / →", "a second back / on (editing)"),
            ("↑ / ↓", "the previous / next cut or marker (editing)"),
            ("Home / End", "the edit's start / end (editing)"),
            ("Shift+M", "mute"),
            ("F", "full screen"),
        ],
    ),
    (
        "Cuts",
        &[
            ("C", "split at the playhead (a sound clip if one is selected)"),
            ("T", "transitions: the join nearest the playhead"),
            ("M", "a marker at the playhead"),
            ("Alt+[ / Alt+]", "trim the start / end to the playhead"),
            ("D", "select the cut under the playhead"),
            ("Alt+← / Alt+→", "move the selected cut one place"),
            ("Alt+↑ / Alt+↓", "the selected text or picture a lane up / down (a new lane on top)"),
            ("Del", "delete what's selected (Shift+Del: the cut)"),
            ("Ctrl+G / Ctrl+Shift+G", "group the clips selected / undo the selected clip's group"),
            ("Esc", "cancel a drag under way"),
            ("Ctrl+Z / Ctrl+Shift+Z", "undo / redo"),
        ],
    ),
    (
        "On the timeline (mouse)",
        &[
            ("drag an edge", "trim the selected clip (yellow: more footage, red: its end)"),
            ("drag a clip", "move it"),
            ("Shift+click a clip", "add it to the selection (again: take it away)"),
            ("click a grouped clip", "selects its group (Alt+click: the clip alone)"),
            ("Shift+drag a cut's start", "crossfade: out, onto the cut before"),
            ("Shift+drag a sound clip", "let it overlap its neighbour (a crossfade)"),
            ("drag a cut's sound edge", "its sound alone past the picture or short of it: J / L cut (the cut selected; it snaps to joins and the sounds beside, Shift: free)"),
            ("drag a fade knob", "fade in / out"),
            ("Ctrl+click a sound", "a volume point there"),
            ("Ctrl+click a point", "take it away"),
            ("drag a level tag", "the clip's own level; double-click: back to normal"),
            ("right-click", "the sound or overlay menu"),
            ("drag a diamond or mark", "move the keyframe in time"),
            ("click / drag a marker's flag", "select it, go to it / move it (the row over the ruler)"),
            ("double- or right-click a flag", "name the marker, or delete it"),
            ("N", "snapping on / off (a yellow line where a drag lands)"),
            ("Shift while dragging", "this drag free, no snapping"),
            ("scroll  /  Shift+scroll", "zoom the timeline / pan it"),
            ("Ctrl+= / Ctrl+− / Ctrl+0", "zoom in / out / the whole edit"),
        ],
    ),
    (
        "Sound",
        &[
            ("K", "a volume point at the playhead on the selected sound"),
            ("S / M on a lane", "solo / mute it"),
        ],
    ),
    (
        "Framing (the Framing switch on)",
        &[
            ("drag the picture", "look around"),
            ("Shift+← / →, ↑ / ↓", "turn, tilt"),
            ("Q / E or right-drag", "roll"),
            ("scroll, + / −", "zoom"),
            ("0 or Home", "reset the view"),
            ("K", "keyframe the view here"),
            ("[ / ]", "the previous / next keyframe"),
            ("J", "the keyframe's ease"),
            ("Del", "delete the keyframe"),
            ("S", "stabilisation"),
            ("P", "projection"),
            ("R", "rolling shutter"),
        ],
    ),
    (
        "Panels and files",
        &[
            ("G", "effects"),
            ("B", "bin"),
            ("Shift+B", "bin in tiles, in the picture's place"),
            ("I / O", "bin player: in and out of a segment (Alt clears)"),
            ("Delete", "bin player: delete the picked segment"),
            ("Ctrl+,", "settings"),
            ("Ctrl+E", "export"),
            ("Ctrl+N", "new: a video in a trip, a trip, an edit"),
            ("Ctrl+O", "add clips"),
            ("Ctrl+Shift+O", "open a project"),
            ("Ctrl+S / Ctrl+Shift+S", "save / save as"),
            ("F1", "this list"),
        ],
    ),
];

/// The bars' tint of each group here (`Tint`).
fn tint_of(group: &str) -> super::Tint {
    use super::Tint;
    match group {
        "Play and the playhead" | "On the timeline (mouse)" => Tint::Move,
        "Cuts" => Tint::Cut,
        "Sound" => Tint::Sound,
        "Framing (the Framing switch on)" => Tint::Frame,
        _ => Tint::Global,
    }
}

impl App {
    /// The shortcuts over `below`, while open.
    pub(super) fn shortcuts_dialog<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        if !self.shortcuts {
            return below;
        }
        let group = |(name, keys): &(&'static str, &'static [(&'static str, &'static str)])| {
            let rows = keys.iter().map(|(k, what)| row![text(*k).font(Font::MONOSPACE).size(13).width(210), text(*what).size(13)].spacing(10).into());
            // The group's tint, as its buttons wear it in the bars (gap 66).
            let colour = tint_of(name).colour();
            let swatch = container(space()).width(10).height(10).style(move |_| container::Style::default().background(colour).border(iced::Border { radius: 2.0.into(), ..Default::default() }));
            column![row![swatch, text(*name).size(15)].spacing(8).align_y(Alignment::Center), column(rows).spacing(4)].spacing(8)
        };
        // Two columns, the groups in order down each.
        let half = GROUPS.len().div_ceil(2);
        let col = |gs: &'static [(&'static str, &'static [(&'static str, &'static str)])]| column(gs.iter().map(|g| group(g).into())).spacing(18).width(Length::Fill);
        let body = row![col(&GROUPS[..half]), col(&GROUPS[half..])].spacing(28);
        let card = container(
            column![
                row![text("Keyboard and mouse").size(18), space().width(Length::Fill), button(text("Close")).on_press(Message::Shortcuts)].align_y(Alignment::Center),
                rule::horizontal(1),
                scrollable(body).height(Length::Shrink),
            ]
            .spacing(12)
            .width(960),
        )
        .padding(18)
        .max_height(760)
        .style(container::rounded_box);
        // A click outside closes it.
        iced::widget::stack![below, super::ask::modal(card, Some(Message::Shortcuts))].into()
    }
}
