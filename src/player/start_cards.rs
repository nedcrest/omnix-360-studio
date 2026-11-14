//! The start screen's recent edits as cards (gap 110, option C;
//! `docs/omnix/PROJECT-CARDS.md`): each a 2×2 mosaic of its film's
//! frames, made in the background the first time and whenever the edit
//! is newer than them, one edit at a time (each is ~30 s of stitching).

use super::*;
use crate::cards::view::{Event, Frames, Motion};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime};

/// A card's width.
const CARD: f32 = 320.0;

/// A card whose frames aren't made yet.
const NONE: &Frames = &Frames(Vec::new());

/// Cards in a row.
const ACROSS: usize = 3;

/// How long the pointer counts as moving (scrubbing) after it last moved;
/// still that long, the tiles change by themselves.
const MOVING: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct StartCards {
    /// Each edit's frames, and its file's time when they were read.
    frames: HashMap<PathBuf, (Frames, Option<SystemTime>)>,
    /// The edit whose frames are being made.
    making: Option<PathBuf>,
    /// Edits whose frames couldn't be made (said once, not tried again).
    failed: HashSet<PathBuf>,
    hover: Option<Hover>,
}

/// The pointer over a card: since when, and where and when it last moved.
struct Hover {
    edit: PathBuf,
    since: Instant,
    moved: Option<(Instant, f32)>,
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

/// The card's motion at `now` for a pointer over it since `since`, last
/// moved at `moved`: scrubbing while it moves, then (with `animate`) the
/// tiles' own changes, counted from when it came to rest.
fn motion(now: Instant, since: Instant, moved: Option<(Instant, f32)>, animate: bool) -> Motion {
    let scrub = moved.filter(|(t, _)| now.saturating_duration_since(*t) < MOVING).map(|(_, x)| x);
    let still = moved.map_or(since, |(t, _)| (t + MOVING).max(since));
    Motion { hovered: (animate && scrub.is_none()).then(|| now.saturating_duration_since(still)), scrub }
}

impl App {
    /// The start screen's recent edits that are still there.
    fn recent_edits(&self) -> impl Iterator<Item = &PathBuf> {
        self.recent.iter().filter(|p| p.is_file())
    }

    /// On the start screen: each recent edit's frames read when they're
    /// there and current, else made, one edit at a time.
    pub(super) fn follow_start_cards(&mut self) -> Task<Message> {
        if !self.on_start() {
            return Task::none();
        }
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let edits: Vec<PathBuf> = self.recent_edits().cloned().collect();
        let mut task = Task::none();
        for edit in edits {
            let changed = modified(&edit);
            if self.start_cards.frames.get(&edit).is_some_and(|(_, at)| *at == changed) || self.start_cards.failed.contains(&edit) {
                continue;
            }
            if !crate::cards::needs_making(&edit) {
                let frames = crate::cards::strip_path(&edit).and_then(|s| Frames::from_strip(&s).ok()).unwrap_or_default();
                self.start_cards.frames.insert(edit, (frames, changed));
            } else if self.start_cards.making.is_none() {
                self.start_cards.making = Some(edit.clone());
                task = Task::perform(blocking({
                    let edit = edit.clone();
                    move || crate::cards::make(&edit).map(|_| ()).map_err(|e| format!("{e:#}"))
                }), move |done| Message::StartCard(CardMsg::Made(edit.clone(), done)));
            }
        }
        task
    }

    pub(super) fn on_start_card(&mut self, message: CardMsg) -> Task<Message> {
        let now = Instant::now();
        let cards = &mut self.start_cards;
        match message {
            CardMsg::Event(edit, Event::Open) => return self.update(Message::ProjectPicked(Some(edit))),
            CardMsg::Event(edit, Event::Enter) => cards.hover = Some(Hover { edit, since: now, moved: None }),
            CardMsg::Event(edit, Event::Move(x)) => match &mut cards.hover {
                Some(h) if h.edit == edit => h.moved = Some((now, x)),
                _ => cards.hover = Some(Hover { edit, since: now, moved: Some((now, x)) }),
            },
            CardMsg::Event(edit, Event::Exit) => {
                if cards.hover.as_ref().is_some_and(|h| h.edit == edit) {
                    cards.hover = None;
                }
            }
            CardMsg::Made(edit, done) => {
                cards.making = None;
                if let Err(e) = done {
                    eprintln!("{}: its card's frames: {e}", edit.display());
                    cards.failed.insert(edit);
                }
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

    /// The recent edits as cards, `ACROSS` a row.
    pub(super) fn recent_cards(&self) -> Option<Element<'_, Message>> {
        let edits: Vec<&PathBuf> = self.recent_edits().collect();
        if edits.is_empty() {
            return None;
        }
        let now = Instant::now();
        let card = |edit: &PathBuf| -> Element<'_, Message> {
            let frames = self.start_cards.frames.get(edit).map_or(NONE, |(f, _)| f);
            let hover = self.start_cards.hover.as_ref().filter(|h| &h.edit == edit);
            let motion = hover.map(|h| motion(now, h.since, h.moved, self.card_motion));
            let place = dailix_sheet::project::of_edit(edit).map_or_else(|| edit.parent().map(|p| p.display().to_string()).unwrap_or_default(), |root| dailix_sheet::project::title(&root));
            let note = if self.start_cards.making.as_ref() == Some(edit) { "making its frames…".to_owned() } else { place };
            let info = iced::widget::column![text(dailix_sheet::project::edit_name(edit)).size(15), text(note).size(12).style(text::secondary)].spacing(2).into();
            let to = { let edit = edit.clone(); move |e| Message::StartCard(CardMsg::Event(edit.clone(), e)) };
            crate::cards::view::card(frames, motion, CARD, false, info, to)
        };
        let rows = edits.chunks(ACROSS).map(|chunk| iced::widget::Row::with_children(chunk.iter().map(|e| card(e))).spacing(20).into());
        Some(iced::widget::column![text("Recent").size(16), iced::widget::Column::with_children(rows).spacing(20)].spacing(10).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moving_pointer_scrubs_and_a_still_one_lets_the_tiles_change() {
        let t0 = Instant::now();
        let s = |secs: f64| t0 + Duration::from_secs_f64(secs);
        assert_eq!(motion(s(2.0), t0, None, true), Motion { hovered: Some(Duration::from_secs(2)), scrub: None });
        assert_eq!(motion(s(2.5), t0, Some((s(2.0), 0.3)), true), Motion { hovered: None, scrub: Some(0.3) }, "moving");
        assert_eq!(motion(s(4.0), t0, Some((s(2.0), 0.3)), true).hovered, Some(Duration::from_secs(1)), "still since 3 s");
        assert_eq!(motion(s(9.0), t0, None, false).hovered, None, "the setting off");
    }
}
