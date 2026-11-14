//! The start screen's edits as cards (gap 110, option C;
//! `docs/omnix/PROJECT-CARDS.md`): the recent ones, and each project's
//! while the projects are browsed; each a 2×2 mosaic of its film's
//! frames, made in the background the first time and whenever the edit
//! is newer than them, one edit at a time (each is ~30 s of stitching).

use super::*;
use crate::cards::view::Event;
use std::time::Instant;

/// A card's width.
const CARD: f32 = 320.0;

/// Cards in a row.
const ACROSS: usize = 3;

#[derive(Default)]
pub(super) struct StartCards {
    frames: crate::cards::view::Cache,
    hover: Option<Hover>,
}

/// The pointer over a card, since when.
struct Hover {
    edit: PathBuf,
    since: Instant,
}

#[derive(Debug, Clone)]
pub enum CardMsg {
    Event(PathBuf, Event),
    Made(PathBuf, Result<(), String>),
    /// A screen frame while a card is hovered: drawn again.
    Tick,
    /// The setting: the hovered card's tiles change by themselves.
    Motion(bool),
}

impl App {
    /// The start screen's recent edits that are still there.
    fn recent_edits(&self) -> impl Iterator<Item = &PathBuf> {
        self.recent.iter().filter(|p| p.is_file())
    }

    /// The edits the start screen shows as cards: the recent ones, and the
    /// browsed projects' while they're browsed.
    fn shown_edits(&self) -> Vec<PathBuf> {
        let browsed = self.browse.then(|| self.browsed.iter().flat_map(|(_, edits)| edits.iter())).into_iter().flatten();
        let mut edits: Vec<PathBuf> = self.recent_edits().chain(browsed).cloned().collect();
        edits.dedup();
        edits
    }

    /// On the start screen: each shown edit's frames read when they're
    /// there and current, else made, one edit at a time.
    pub(super) fn follow_start_cards(&mut self) -> Task<Message> {
        if !self.on_start() {
            return Task::none();
        }
        let edits = self.shown_edits();
        match self.start_cards.frames.want(&edits) {
            Some(edit) => Task::perform(blocking({
                let edit = edit.clone();
                move || crate::cards::make(&edit).map(|_| ()).map_err(|e| format!("{e:#}"))
            }), move |done| Message::StartCard(CardMsg::Made(edit.clone(), done))),
            None => Task::none(),
        }
    }

    pub(super) fn on_start_card(&mut self, message: CardMsg) -> Task<Message> {
        let now = Instant::now();
        let cards = &mut self.start_cards;
        match message {
            CardMsg::Event(edit, Event::Open) => return self.update(Message::ProjectPicked(Some(edit))),
            CardMsg::Event(edit, Event::Enter) => cards.hover = Some(Hover { edit, since: now }),
            CardMsg::Event(edit, Event::Exit) => {
                if cards.hover.as_ref().is_some_and(|h| h.edit == edit) {
                    cards.hover = None;
                }
            }
            CardMsg::Made(edit, done) => {
                cards.frames.made(&edit, done);
                return self.follow_start_cards();
            }
            CardMsg::Motion(on) => self.card_motion = on,
            CardMsg::Tick => {}
        }
        Task::none()
    }

    /// Screen frames while a card on the start screen is hovered.
    pub(super) fn start_cards_ticking(&self) -> bool {
        self.start_cards.hover.is_some() && self.on_start()
    }

    /// Edit `edit` as a card: its frames, its name, and `note` under it
    /// (where it is) unless its frames are being made.
    fn edit_card<'a>(&'a self, edit: &'a PathBuf, note: String, now: Instant) -> Element<'a, Message> {
        let frames = self.start_cards.frames.get(edit);
        let hovered = self.start_cards.hover.as_ref().filter(|h| &h.edit == edit).map(|h| self.card_motion.then(|| now.saturating_duration_since(h.since)));
        let note = if self.start_cards.frames.making(edit) { "making its frames…".to_owned() } else { note };
        let info = iced::widget::column![text(dailix_sheet::project::edit_name(edit)).size(15)].push((!note.is_empty()).then(|| text(note).size(12).style(text::secondary))).spacing(2).into();
        let to = {
            let edit = edit.clone();
            move |e| Message::StartCard(CardMsg::Event(edit.clone(), e))
        };
        crate::cards::view::card(frames, hovered, CARD, false, info, to)
    }

    /// `cards`, `ACROSS` a row.
    fn card_rows<'a>(cards: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let mut cards = cards.into_iter().peekable();
        let mut rows = iced::widget::Column::new().spacing(20);
        while cards.peek().is_some() {
            rows = rows.push(iced::widget::Row::with_children(cards.by_ref().take(ACROSS)).spacing(20));
        }
        rows.into()
    }

    /// The recent edits as cards, under where each is.
    pub(super) fn recent_cards(&self) -> Option<Element<'_, Message>> {
        let now = Instant::now();
        let cards: Vec<Element<'_, Message>> = self
            .recent_edits()
            .map(|edit| {
                let place = dailix_sheet::project::of_edit(edit).map_or_else(|| edit.parent().map(|p| p.display().to_string()).unwrap_or_default(), |root| dailix_sheet::project::title(&root));
                self.edit_card(edit, place, now)
            })
            .collect();
        (!cards.is_empty()).then(|| iced::widget::column![text("Recent").size(16), Self::card_rows(cards)].spacing(10).into())
    }

    /// A browsed project's edits as cards.
    pub(super) fn project_cards<'a>(&'a self, edits: &'a [PathBuf]) -> Element<'a, Message> {
        let now = Instant::now();
        Self::card_rows(edits.iter().map(|edit| self.edit_card(edit, String::new(), now)).collect())
    }
}

