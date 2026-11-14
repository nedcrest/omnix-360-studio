//! The card on the start pages (gap 110, option C): a 2×2 mosaic of a
//! project's frames. At rest nothing moves: each tile shows a frame from
//! its quarter of the film (or the trip). Hovered, the tiles change one
//! at a time, 1-2-3-4, each crossfading over a second to its frame in the
//! next of four sets (the user, 2026-11-14: coded, not scrubbed by the
//! pointer; no frame twice). What shows when is `tiles`, a pure function
//! of the time hovered; the widget draws what it says.

use iced::widget::{column, container, image, mouse_area, row, space, stack};
use iced::{Border, Color, Element, Length, Padding, Shadow, Theme, Vector};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Between one tile's change and the next's.
pub const STEP: Duration = Duration::from_secs(1);

/// A tile's crossfade.
pub const FADE: Duration = Duration::from_secs(1);

/// The order the tiles change in: as read, top left to bottom right.
const ORDER: [usize; 4] = [0, 1, 2, 3];

/// Between the tiles, and the card's edge to them.
const GAP: f32 = 2.0;
const INSET: f32 = 6.0;

/// A project's frames, ready to draw.
#[derive(Clone, Debug, Default, PartialEq)]
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
/// of its quarter (the 2nd, 6th, 10th and 14th of 16); hovered for
/// `hovered`, one tile a `STEP`, from the first at once, moving on to its
/// quarter's next frame over `FADE`.
pub fn tiles(n: usize, hovered: Option<Duration>) -> [Tile; 4] {
    let changes = hovered.map_or(0, |e| (e.as_secs_f64() / STEP.as_secs_f64()) as usize + 1);
    let mut tiles = [0, 1, 2, 3].map(|q| {
        let r = quarter(n, q);
        // The changes this tile has made: every fourth, from its turn.
        let turn = ORDER.iter().position(|&o| o == q).unwrap_or(0);
        let made = (changes + 3 - turn) / 4;
        let at = |made: usize| r.start + ((r.len() - 1) / 2 + made) % r.len();
        Tile { from: at(made.saturating_sub(1)), to: at(made), mix: 1.0 }
    });
    if let Some(e) = hovered {
        let q = ORDER[(changes - 1) % 4];
        let since = e.as_secs_f64() - (changes - 1) as f64 * STEP.as_secs_f64();
        tiles[q].mix = (since / FADE.as_secs_f64()).clamp(0.0, 1.0) as f32;
    }
    tiles
}

/// What a card tells its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Enter,
    Exit,
    Open,
}

/// A card `width` wide: `frames` as a 2×2 mosaic over `info`. `hovered`:
/// the pointer is over it (lit and lifted), and for how long the tiles
/// have been changing (none: they don't); `stack_of`, sheets behind it (a
/// collection, such as a trip).
pub fn card<'a, M: Clone + 'a>(frames: &'a Frames, hovered: Option<Option<Duration>>, width: f32, stack_of: bool, info: Element<'a, M>, on: impl Fn(Event) -> M + Clone + 'a) -> Element<'a, M> {
    let lit = hovered.is_some();
    let tile_w = ((width - 2.0 * INSET - GAP) / 2.0).floor();
    let tile_h = (tile_w * 9.0 / 16.0).round();
    let shown = tiles(frames.0.len(), hovered.flatten());
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
    let body = column![mosaic, info].spacing(8).width(width - 2.0 * INSET);
    let face = container(body).padding(INSET).width(width).style(move |t: &Theme| face_style(t, lit));
    let face = mouse_area(face).on_enter(on(Event::Enter)).on_exit(on(Event::Exit)).on_press(on(Event::Open)).interaction(iced::mouse::Interaction::Pointer);
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
/// round it, a large soft shadow; hovered, lighter still (the user,
/// 2026-11-14), the shadow stronger and the edge the accent.
fn face_style(t: &Theme, lit: bool) -> container::Style {
    let p = t.extended_palette();
    let edge = if lit { Color { a: 0.7, ..p.primary.base.color } } else { Color { a: 0.09, ..Color::WHITE } };
    let base = p.background.weak.color;
    let face = if lit { Color { r: base.r + (1.0 - base.r) * 0.08, g: base.g + (1.0 - base.g) * 0.08, b: base.b + (1.0 - base.b) * 0.08, a: base.a } } else { base };
    container::Style {
        background: Some(face.into()),
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

    fn at(secs: f64) -> Option<Duration> {
        Some(Duration::from_secs_f64(secs))
    }

    #[test]
    fn at_rest_each_tile_shows_the_middle_of_its_quarter() {
        let rest = tiles(16, None).map(|t| (t.to, t.mix));
        assert_eq!(rest, super::super::AT_REST.map(|k| (k, 1.0)));
        // Fewer frames than tiles: round them.
        assert_eq!(tiles(3, None).map(|t| t.to), [0, 1, 2, 0]);
    }

    #[test]
    fn hovered_the_tiles_change_one_a_second_in_reading_order() {
        // At once the first tile starts, half way through its fade at 0.5 s.
        let t = tiles(16, at(0.5));
        assert_eq!((t[0].from, t[0].to), (1, 2));
        assert!((t[0].mix - 0.5).abs() < 1e-3, "{}", t[0].mix);
        assert!(t[1..].iter().all(|t| t.mix == 1.0 && t.from == t.to), "one at a time");
        // Then the second, the third, the fourth: the next set whole.
        assert_eq!(tiles(16, at(1.99)).map(|t| t.to), [2, 6, 9, 13]);
        assert_eq!(tiles(16, at(3.99)).map(|t| t.to), [2, 6, 10, 14]);
        // Four sets, no frame twice, and round to the first.
        let set = |hovered| tiles(16, hovered).map(|t| t.to);
        let sets = [set(None), set(at(3.99)), set(at(7.99)), set(at(11.99))];
        let mut all: Vec<usize> = sets.iter().flatten().copied().collect();
        all.sort_unstable();
        assert_eq!(all, (0..16).collect::<Vec<_>>(), "{sets:?}");
        assert_eq!(tiles(16, at(16.0 - 0.01)).map(|t| t.to), super::super::AT_REST);
    }
}
