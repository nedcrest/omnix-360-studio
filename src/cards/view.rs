//! The card on the start pages (gap 110, option C): a 2×2 mosaic of a
//! project's frames. At rest nothing moves: each tile shows a frame from
//! its quarter of the film (or the trip). Under a still pointer the tiles
//! change one at a time, clockwise, each crossfading to the next frame of
//! its quarter; a moving pointer scrubs instead, the frame under it shown
//! in its quarter's tile. What shows when is `tiles`, a pure function of
//! the time hovered and the pointer; the widget draws what it says.

use iced::widget::{column, container, image, mouse_area, row, space, stack};
use iced::{Border, Color, Element, Length, Padding, Shadow, Theme, Vector};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Between one tile's change and the next's.
pub const STEP: Duration = Duration::from_millis(1500);

/// A tile's crossfade.
pub const FADE: Duration = Duration::from_millis(700);

/// The order the tiles change in: clockwise from the top left.
const ORDER: [usize; 4] = [0, 1, 3, 2];

/// Between the tiles, and the card's edge to them.
const GAP: f32 = 2.0;
const INSET: f32 = 6.0;

/// A project's frames, ready to draw.
#[derive(Clone, Debug, Default)]
pub struct Frames(pub Vec<image::Handle>);

impl Frames {
    /// An edit's strip (`cards::make`), cut into its frames.
    pub fn from_strip(path: &Path) -> anyhow::Result<Frames> {
        let strip = ::image::load_from_memory(&std::fs::read(path)?)?.to_rgba8();
        let w = strip.width() / super::FRAMES as u32;
        anyhow::ensure!(w > 0, "{} is too narrow for its frames", path.display());
        let frames = (0..super::FRAMES as u32).map(|k| image::Handle::from_rgba(w, strip.height(), ::image::imageops::crop_imm(&strip, k * w, 0, w, strip.height()).to_image().into_raw())).collect();
        Ok(Frames(frames))
    }

    /// Pictures, one a frame (a trip's clips' posters).
    pub fn from_files(paths: &[PathBuf]) -> Frames {
        Frames(paths.iter().map(image::Handle::from_path).collect())
    }
}

/// What the pointer is doing over the card.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// How long it has been over the card, when the tiles may change by
    /// themselves (none: the motion setting is off).
    pub hovered: Option<Duration>,
    /// Where it moved last, across the card 0..1, while it's moving.
    pub scrub: Option<f32>,
}

/// A tile: the frame it's leaving, the one it's showing, and how far it
/// has crossfaded (1: only `to`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub from: usize,
    pub to: usize,
    pub mix: f32,
}

/// Tile `q`'s frames among `n`: its quarter of them, or one of them when
/// there are fewer than four.
fn quarter(n: usize, q: usize) -> std::ops::Range<usize> {
    if n < 4 {
        let k = q % n.max(1);
        return k..k + 1;
    }
    q * n / 4..(q + 1) * n / 4
}

/// What each of the four tiles shows, for `n` frames: at rest the middle
/// of its quarter (the 2nd, 6th, 10th and 14th of 16); hovered, one tile
/// a `STEP` moving on to its quarter's next frame over `FADE`; scrubbing,
/// the frame under the pointer in its quarter's tile.
pub fn tiles(n: usize, motion: Motion) -> [Tile; 4] {
    let changes = motion.hovered.map_or(0, |e| (e.as_secs_f64() / STEP.as_secs_f64()) as usize);
    let mut tiles = [0, 1, 2, 3].map(|q| {
        let r = quarter(n, q);
        // The changes this tile has made: every fourth, from its turn.
        let turn = ORDER.iter().position(|&o| o == q).unwrap_or(0);
        let made = (changes + 3 - turn) / 4;
        let at = |made: usize| r.start + ((r.len() - 1) / 2 + made) % r.len();
        Tile { from: at(made.saturating_sub(1)), to: at(made), mix: 1.0 }
    });
    if let Some(e) = motion.hovered.filter(|_| changes > 0) {
        let q = ORDER[(changes - 1) % 4];
        let since = e.as_secs_f64() - changes as f64 * STEP.as_secs_f64();
        tiles[q].mix = (since / FADE.as_secs_f64()).clamp(0.0, 1.0) as f32;
    }
    if let Some(x) = motion.scrub.filter(|_| n > 0) {
        let f = ((x.clamp(0.0, 1.0) * n as f32) as usize).min(n - 1);
        if let Some(q) = (0..4).find(|&q| quarter(n, q).contains(&f)) {
            tiles[q] = Tile { from: f, to: f, mix: 1.0 };
        }
    }
    tiles
}

/// What a card tells its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Enter,
    /// The pointer across the card, 0..1.
    Move(f32),
    Exit,
    Open,
}

/// A card `width` wide: `frames` as a 2×2 mosaic over `info`. `motion`
/// is the pointer's when it's over this card; `stack`, sheets behind it
/// (a collection, such as a trip).
pub fn card<'a, M: Clone + 'a>(frames: &'a Frames, motion: Option<Motion>, width: f32, stack_of: bool, info: Element<'a, M>, on: impl Fn(Event) -> M + Clone + 'a) -> Element<'a, M> {
    let lit = motion.is_some();
    let tile_w = ((width - 2.0 * INSET - GAP) / 2.0).floor();
    let tile_h = (tile_w * 9.0 / 16.0).round();
    let shown = tiles(frames.0.len(), motion.unwrap_or_default());
    let tile = |t: Tile| -> Element<'a, M> {
        let picture = |k: usize, opacity: f32| -> Element<'a, M> {
            match frames.0.get(k) {
                Some(h) => image(h.clone()).width(tile_w).height(tile_h).content_fit(iced::ContentFit::Cover).opacity(opacity).into(),
                None => space().width(tile_w).height(tile_h).into(),
            }
        };
        let layers: Element<'a, M> = if t.mix >= 1.0 || t.from == t.to { picture(t.to, 1.0) } else { stack![picture(t.from, 1.0), picture(t.to, t.mix)].into() };
        container(layers).width(tile_w).height(tile_h).clip(true).into()
    };
    let mosaic = column![row![tile(shown[0]), tile(shown[1])].spacing(GAP), row![tile(shown[2]), tile(shown[3])].spacing(GAP)].spacing(GAP);
    // Where a scrub is, as a thin line under the tiles.
    let line: Element<'a, M> = match motion.and_then(|m| m.scrub) {
        Some(x) => row![space().width(Length::FillPortion((x.clamp(0.0, 1.0) * 1000.0) as u16 + 1)), container(space().width(3).height(2)).style(|t: &Theme| container::Style { background: Some(t.extended_palette().primary.base.color.into()), ..Default::default() }), space().width(Length::FillPortion(((1.0 - x.clamp(0.0, 1.0)) * 1000.0) as u16 + 1))].into(),
        None => space().height(2).into(),
    };
    let body = column![mosaic, line, info].spacing(6).width(width - 2.0 * INSET);
    let face = container(body).padding(INSET).width(width).style(move |t: &Theme| face_style(t, lit));
    let (o1, o2) = (Event::Enter, Event::Exit);
    let (on_move, on_open) = (on.clone(), on.clone());
    let face = mouse_area(face).on_enter(on(o1)).on_exit(on(o2)).on_move(move |p| on_move(Event::Move(p.x / width))).on_press(on_open(Event::Open)).interaction(iced::mouse::Interaction::Pointer);
    // Lifted when hovered: the room under it moves above it.
    let lift = if lit { 3.0 } else { 0.0 };
    let face = container(face).padding(Padding { top: 3.0 - lift, bottom: lift, ..Padding::ZERO });
    if !stack_of {
        return face.into();
    }
    // Two sheets behind, down and to the right: a collection.
    let sheet = |d: f32| container(space().width(width).height(Length::Fill)).padding(Padding { top: 3.0 + d, left: d, ..Padding::ZERO }).style(move |t: &Theme| sheet_style(t, d));
    let room = container(face).padding(Padding { right: 8.0, bottom: 8.0, ..Padding::ZERO });
    stack![sheet(8.0), sheet(4.0), room].into()
}

/// The card's face: a little lighter than the page, a faint highlight
/// round it, a large soft shadow; stronger, and the accent, when hovered.
fn face_style(t: &Theme, lit: bool) -> container::Style {
    let p = t.extended_palette();
    let edge = if lit { Color { a: 0.7, ..p.primary.base.color } } else { Color { a: 0.09, ..Color::WHITE } };
    container::Style {
        background: Some(p.background.weak.color.into()),
        border: Border { color: edge, width: 1.0, radius: 10.0.into() },
        shadow: Shadow { color: Color { a: if lit { 0.6 } else { 0.45 }, ..Color::BLACK }, offset: Vector::new(0.0, if lit { 11.0 } else { 8.0 }), blur_radius: if lit { 32.0 } else { 24.0 } },
        ..Default::default()
    }
}

/// A sheet behind a collection's card, darker the further back.
fn sheet_style(t: &Theme, d: f32) -> container::Style {
    let p = t.extended_palette();
    let base = p.background.weak.color;
    let k = 1.0 - d / 40.0;
    container::Style {
        background: Some(Color { r: base.r * k, g: base.g * k, b: base.b * k, a: 1.0 }.into()),
        border: Border { color: Color { a: 0.06, ..Color::WHITE }, width: 1.0, radius: 10.0.into() },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: f64) -> Motion {
        Motion { hovered: Some(Duration::from_secs_f64(secs)), scrub: None }
    }

    #[test]
    fn at_rest_each_tile_shows_the_middle_of_its_quarter() {
        let rest = tiles(16, Motion::default()).map(|t| (t.to, t.mix));
        assert_eq!(rest, super::super::AT_REST.map(|k| (k, 1.0)));
        assert_eq!(tiles(16, at(1.0)), tiles(16, Motion::default()), "nothing changes before the first step");
        // Fewer frames than tiles: round them.
        assert_eq!(tiles(3, Motion::default()).map(|t| t.to), [0, 1, 2, 0]);
    }

    #[test]
    fn hovered_the_tiles_change_one_at_a_time_clockwise_crossfading() {
        // The first change: the top left, half way through its fade.
        let t = tiles(16, at(1.5 + 0.35));
        assert_eq!((t[0].from, t[0].to), (1, 2));
        assert!((t[0].mix - 0.5).abs() < 1e-3, "{}", t[0].mix);
        assert!(t[1..].iter().all(|t| t.mix == 1.0), "one at a time");
        // Then the top right, then the bottom right, the bottom left.
        assert_eq!(tiles(16, at(3.0 + 0.7)).map(|t| t.to), [2, 6, 9, 13]);
        assert_eq!(tiles(16, at(6.0 + 0.7)).map(|t| t.to), [2, 6, 10, 14]);
        // Round its quarter and back to where it started.
        assert_eq!(tiles(16, at(4.0 * 6.0 + 0.7)).map(|t| t.to), super::super::AT_REST);
    }

    #[test]
    fn a_moving_pointer_shows_the_frame_under_it_in_its_quarters_tile() {
        let scrub = |x| tiles(16, Motion { hovered: Some(Duration::ZERO), scrub: Some(x) });
        assert_eq!(scrub(0.0)[0].to, 0);
        assert_eq!(scrub(0.99)[3], Tile { from: 15, to: 15, mix: 1.0 });
        let mid = scrub(0.4);
        assert_eq!((mid[1].to, mid[0].to), (6, 1), "frame 6 in the second tile, the first at rest");
    }
}
