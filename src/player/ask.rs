//! Questions asked inside the window: save before closing, stop a running
//! export, move a file to the trash. They were the system's own dialogs
//! (rfd), which came up in a corner of the screen rather than over the
//! window, and every press of the window's close button opened another
//! (user, 2026-11-04). Here one question shows at a time, centred on the
//! window, and asking again while it's up does nothing.

use super::{App, Message};
use iced::widget::{button, center, column, container, mouse_area, opaque, row, space, text};
use iced::{Alignment, Element, Length, Task};

/// A question and its answers: each a button and what it does; `cancel`
/// is what Esc and the Cancel button do.
pub struct Ask {
    pub title: String,
    pub text: String,
    pub answers: Vec<(String, Message)>,
    pub cancel: Message,
    /// A line to type in, what it holds now (a lane's name, gap 97); the
    /// answer given it is `Message::LaneName`'s.
    pub field: Option<String>,
}

impl App {
    /// Shows `ask`, unless a question is up already.
    pub(super) fn ask(&mut self, ask: Ask) -> Task<Message> {
        if self.asking.is_none() {
            self.asking = Some(ask);
        }
        Task::none()
    }

    /// Answers the question up: it closes, then the answer is done, with
    /// what was typed in its field.
    pub(super) fn answer(&mut self, message: Message) -> Task<Message> {
        let typed = self.asking.take().and_then(|a| a.field);
        let message = match (message, typed) {
            (Message::LaneName(track, lane, _), Some(name)) => Message::LaneName(track, lane, name),
            (Message::GroupName(group, _), Some(name)) => Message::GroupName(group, name),
            (m, _) => m,
        };
        self.update(message)
    }

    /// The question's field typed in.
    pub(super) fn ask_typed(&mut self, typed: String) {
        if let Some(field) = self.asking.as_mut().and_then(|a| a.field.as_mut()) {
            *field = typed;
        }
    }

    /// Esc while a question is up.
    pub(super) fn cancel_ask(&mut self) -> Task<Message> {
        match self.asking.as_ref().map(|a| a.cancel.clone()) {
            Some(cancel) => self.answer(cancel),
            None => Task::none(),
        }
    }

    /// The question over the window, or nothing. Always a layer of its own,
    /// so the window under it keeps its state when one comes up.
    pub(super) fn ask_layer(&self) -> Element<'_, Message> {
        let Some(ask) = &self.asking else { return space().into() };
        let answer = |label: &str, msg: &Message, primary: bool| -> Element<'_, Message> {
            button(text(label.to_owned()))
                .padding([6, 14])
                .style(if primary { button::primary } else { button::secondary })
                .on_press(Message::Answer(Box::new(msg.clone())))
                .into()
        };
        let mut buttons = row![space().width(Length::Fill)].spacing(8).align_y(Alignment::Center);
        for (i, (label, msg)) in ask.answers.iter().enumerate() {
            buttons = buttons.push(answer(label, msg, i == 0));
        }
        buttons = buttons.push(answer("Cancel", &ask.cancel, false));
        let mut body = column![text(ask.title.clone()).size(18), text(ask.text.clone()).size(14)].spacing(14);
        if let Some(field) = &ask.field {
            let first = ask.answers.first().map(|(_, m)| Message::Answer(Box::new(m.clone())));
            body = body.push(iced::widget::text_input("", field).on_input(Message::AskTyped).on_submit_maybe(first).padding(6));
        }
        let card = container(body.push(buttons).width(460)).padding(18).style(container::rounded_box);
        modal(card, None)
    }
}

/// A card over the window, the rest dimmed: nothing below takes a click;
/// one outside the card sends `outside`, if there is one. Every dialog's.
pub(super) fn modal<'a>(card: impl Into<Element<'a, Message>>, outside: Option<Message>) -> Element<'a, Message> {
    let shade = container(center(opaque(card))).width(Length::Fill).height(Length::Fill).style(|_| container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55)));
    let area = mouse_area(shade);
    opaque(match outside {
        Some(m) => area.on_press(m),
        None => area,
    })
}
