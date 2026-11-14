//! The keyboard shortcuts and mouse gestures, grouped, in a window over
//! the editor (gap 56): opened with F1 or the keyboard button, closed
//! with Esc or a click outside. Hidden until asked for, so the interface
//! stays without help text. Keep it in step with `keys.rs` and the
//! timeline's presses.

use super::{App, Message};
use iced::widget::{button, column, container, row, rule, scrollable, space, text};
use iced::{Alignment, Element, Font, Length};

/// What each group's keys do, as «keys», «what».
use crate::keys::GROUPS;

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
