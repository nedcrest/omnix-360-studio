//! «New» (user, 2026-11-08): a project is a Dailix trip, made there, so
//! Omnix offers a new video in a trip (an empty edit in its own folder,
//! opened), a new trip (Dailix, on its form), or an edit without a trip
//! (the empty edit as before).

use super::{App, Message};
use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Alignment, Element, Length, Task};
use std::path::PathBuf;

/// The «New» window as chosen so far.
#[derive(Debug, Clone, Default)]
pub struct NewMenu {
    /// The trips in the projects place: folder and name.
    pub trips: Vec<(PathBuf, String)>,
    pub trip: Option<PathBuf>,
    pub name: String,
}

/// The video's name field, focused on opening.
const NAME: &str = "new-video-name";

impl App {
    /// The window's messages.
    pub(super) fn on_new_menu(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewMenu => {
                let trips = self.projects_place().map(|p| dailix_sheet::project::all(&p)).unwrap_or_default().into_iter().map(|r| {
                    let t = dailix_sheet::project::title(&r);
                    (r, t)
                });
                self.new_menu = Some(NewMenu { trips: trips.collect(), ..NewMenu::default() });
                return iced::widget::operation::focus(NAME);
            }
            Message::NewMenuClose => self.new_menu = None,
            Message::NewVideoTrip(root) => {
                if let Some(m) = &mut self.new_menu {
                    m.trip = Some(root);
                }
            }
            Message::NewVideoName(n) => {
                if let Some(m) = &mut self.new_menu {
                    m.name = n;
                }
            }
            Message::NewVideoMake => {
                let Some(NewMenu { trip: Some(root), name, .. }) = self.new_menu.clone() else { return Task::none() };
                match crate::from_sheet::write_named(&root, &name) {
                    Ok(edit) => {
                        self.new_menu = None;
                        return self.update(Message::ProjectPicked(Some(edit)));
                    }
                    Err(e) => self.error = Some(format!("The video couldn't be made: {e:#}")),
                }
            }
            Message::NewTripInDailix => {
                self.new_menu = None;
                self.error = crate::sibling::start(crate::sibling::DAILIX, &["new-trip".as_ref()]).err();
            }
            _ => {}
        }
        Task::none()
    }

    /// The window over the program while it's open.
    pub(super) fn new_menu_layer(&self) -> Element<'_, Message> {
        let Some(m) = &self.new_menu else { return space().into() };
        let close = button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(Message::NewMenuClose);
        let note = |s: &'static str| text(s).size(13).style(text::secondary);
        let mut trips = column![].spacing(2);
        for (root, title) in &m.trips {
            let picked = m.trip.as_ref() == Some(root);
            trips = trips.push(button(text(title.clone()).size(14)).width(Length::Fill).padding([4, 10]).style(if picked { button::primary } else { button::text }).on_press(Message::NewVideoTrip(root.clone())));
        }
        if m.trips.is_empty() {
            trips = trips.push(note("No trips in the projects place yet: make one in Dailix."));
        }
        let ready = m.trip.is_some();
        let video = column![
            text("A video in a trip").size(16),
            note("An empty edit in the trip's own folder for it, its clips in the bin."),
            scrollable(trips).height(Length::Shrink),
            row![
                text_input("Its name, e.g. Shorts: peacocks", &m.name).id(NAME).size(14).on_input(Message::NewVideoName).on_submit_maybe(ready.then_some(Message::NewVideoMake)),
                button(text("Make the video")).style(button::primary).on_press_maybe(ready.then_some(Message::NewVideoMake)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8);
        let trip = column![text("A trip").size(16), note("Made in Dailix, in the fixed layout; it opens on its form."), button(text("New trip in Dailix…")).style(button::secondary).on_press(Message::NewTripInDailix)].spacing(8);
        let loose = column![text("Without a trip").size(16), note("An empty edit of clips from anywhere, saved where you choose."), button(text("Empty edit")).style(button::secondary).on_press(Message::NewProject)].spacing(8);
        let card = container(column![row![text("New").size(20).width(Length::Fill), close].align_y(Alignment::Center), video, rule::horizontal(1), trip, rule::horizontal(1), loose].spacing(14))
            .padding(18)
            .width(560)
            .max_height(640)
            .style(container::rounded_box);
        super::ask::modal(card, Some(Message::NewMenuClose))
    }
}
