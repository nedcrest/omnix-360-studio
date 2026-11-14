//! The card on the start pages (gap 110, option C): a 2×2 mosaic of a
//! project's frames. At rest nothing moves: each tile shows a frame from
//! its quarter of the film (or the trip). Hovered, the tiles change one
//! at a time, 1-2-3-4, each crossfading over a second to its frame in the
//! next of four sets (the user, 2026-11-14: coded, not scrubbed by the
//! pointer; no frame twice). What shows when is `tiles`, a pure function
//! of the time hovered; the widget draws what it says.

use iced::widget::{column, container, image, mouse_area, row, space, stack};
use iced::{Border, Color, Element, Padding, Shadow, Theme, Vector};
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

/// From this many frames each tile has a quarter of them, two or more,
/// to go round; fewer, the tiles go round all of them together.
const QUARTERS: usize = 8;

/// The frame tile `q` shows among `n` after it has changed `made` times.
/// With `QUARTERS` or more, it goes round its quarter of them from the
/// quarter's middle (the 2nd, 6th, 10th and 14th of 16 at rest). Fewer
/// (a trip with a few posters), the tiles take every frame in turn,
/// `q + 4 × made`, so four in a row never show one twice when there are
/// four or more and every frame comes round (the user, 2026-11-14: a
/// trip of five posters hardly moved).
fn frame(n: usize, q: usize, made: usize) -> usize {
    if n < QUARTERS {
        return (q + 4 * made) % n.max(1);
    }
    let (start, len) = (q * n / 4, (q + 1) * n / 4 - q * n / 4);
    start + ((len - 1) / 2 + made) % len
}

/// What each of the four tiles shows, for `n` frames: at rest the middle
/// of its quarter (the 2nd, 6th, 10th and 14th of 16); hovered for
/// `hovered`, one tile a `STEP`, from the first at once, moving on to its
/// quarter's next frame over `FADE`.
pub fn tiles(n: usize, hovered: Option<Duration>) -> [Tile; 4] {
    let changes = hovered.map_or(0, |e| (e.as_secs_f64() / STEP.as_secs_f64()) as usize + 1);
    let mut tiles = [0, 1, 2, 3].map(|q| {
        // The changes this tile has made: every fourth, from its turn.
        let turn = ORDER.iter().position(|&o| o == q).unwrap_or(0);
        let made = (changes + 3 - turn) / 4;
        Tile { from: frame(n, q, made.saturating_sub(1)), to: frame(n, q, made), mix: 1.0 }
    });
    if let Some(e) = hovered {
        let q = ORDER[(changes - 1) % 4];
        let since = e.as_secs_f64() - (changes - 1) as f64 * STEP.as_secs_f64();
        tiles[q].mix = (since / FADE.as_secs_f64()).clamp(0.0, 1.0) as f32;
    }
    tiles
}

/// Edits' frames for a page of cards: read when their strip is current,
/// else made, one edit at a time (each is ~30 s of stitching). The page
/// runs the making (`cards::make`) off its thread and says when it's done.
#[derive(Debug, Default)]
pub struct Cache {
    /// Each edit's frames, and its file's time when they were read.
    frames: std::collections::HashMap<PathBuf, (Frames, Option<std::time::SystemTime>)>,
    making: Option<PathBuf>,
    /// Edits whose frames couldn't be made, and their file's time then:
    /// said once, tried again when the edit changes (an empty edit filled,
    /// a disk back).
    failed: std::collections::HashMap<PathBuf, Option<std::time::SystemTime>>,
}

/// A card whose frames aren't made yet.
const NONE: &Frames = &Frames(Vec::new());

impl Cache {
    /// `edits`' frames read where they're current; an edit to make them
    /// for, if none is being made and one needs them.
    pub fn want<'p>(&mut self, edits: impl IntoIterator<Item = &'p PathBuf>) -> Option<PathBuf> {
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let mut next = None;
        for edit in edits {
            let changed = modified(edit);
            if self.frames.get(edit).is_some_and(|(_, at)| *at == changed) || self.failed.get(edit).is_some_and(|at| *at == changed) || self.making.as_ref() == Some(edit) {
                continue;
            }
            // A current strip that won't read (cut short, spoiled) is made
            // again, as a missing one.
            let read = (!super::needs_making(edit)).then(|| super::strip_path(edit).and_then(|s| Frames::from_strip(&s).ok())).flatten();
            match read {
                Some(frames) => {
                    self.frames.insert(edit.clone(), (frames, changed));
                }
                None if self.making.is_none() && next.is_none() => next = Some(edit.clone()),
                None => {}
            }
        }
        self.making = next.clone().or(self.making.take());
        next
    }

    /// `edit`'s frames made, or why not.
    pub fn made(&mut self, edit: &Path, done: Result<(), String>) {
        self.making = None;
        if let Err(e) = done {
            eprintln!("{}: its card's frames: {e}", edit.display());
            let changed = std::fs::metadata(edit).and_then(|m| m.modified()).ok();
            self.failed.insert(edit.to_owned(), changed);
        }
    }

    /// `edit`'s frames, none yet if they aren't read.
    pub fn get(&self, edit: &Path) -> &Frames {
        self.frames.get(edit).map_or(NONE, |(f, _)| f)
    }

    /// Whether `edit`'s frames are being made.
    pub fn making(&self, edit: &Path) -> bool {
        self.making.as_deref() == Some(edit)
    }
}

/// Whether a card of `n` frames changes at all when hovered: not with 1,
/// 2 or 4 (each tile keeps its one frame, none shown twice), so its page
/// needn't ask for screen frames.
pub fn moves(n: usize) -> bool {
    let frames = |hovered| tiles(n, hovered).map(|t| (t.from, t.to));
    (1..=4).any(|k| frames(Some(STEP * k)) != frames(None))
}

/// What a card tells its page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Enter,
    Exit,
    Open,
}

/// A card `width` wide: `frames` as a 2×2 mosaic over `info`, given
/// `info_height` so every card of a row is as tall and the sheets behind
/// know their size (a stack takes its first layer's). `hovered`: the
/// pointer is over it (lit and lifted), and for how long the tiles have
/// been changing (none: they don't); `stack_of`, sheets behind it (a
/// collection, such as a trip).
pub fn card<'a, M: Clone + 'a>(frames: &'a Frames, hovered: Option<Option<Duration>>, width: f32, stack_of: bool, info: Element<'a, M>, info_height: f32, on: impl Fn(Event) -> M + Clone + 'a) -> Element<'a, M> {
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
    // None yet (a trip with no footage, an edit's being made): said, in
    // the mosaic's room.
    let mosaic: Element<'a, M> = if frames.0.is_empty() {
        let (w, h) = (2.0 * tile_w + GAP, 2.0 * tile_h + GAP);
        container(iced::widget::text("No pictures yet").size(13).style(iced::widget::text::secondary)).width(w).height(h).center_x(w).center_y(h).into()
    } else {
        column![row![tile(shown[0]), tile(shown[1])].spacing(GAP), row![tile(shown[2]), tile(shown[3])].spacing(GAP)].spacing(GAP).into()
    };
    let body = column![mosaic, container(info).height(info_height).clip(true)].spacing(8).width(width - 2.0 * INSET);
    let face_h = 2.0 * INSET + 2.0 * tile_h + GAP + 8.0 + info_height;
    let face = container(body).padding(INSET).width(width).style(move |t: &Theme| face_style(t, lit));
    let face = mouse_area(face).on_enter(on(Event::Enter)).on_exit(on(Event::Exit)).on_press(on(Event::Open)).interaction(iced::mouse::Interaction::Pointer);
    // Lifted when hovered: the room under it moves above it.
    let lift = if lit { 3.0 } else { 0.0 };
    let face = container(face).padding(Padding { top: 3.0 - lift, bottom: lift, ..Padding::ZERO });
    if !stack_of {
        return face.into();
    }
    // Two sheets behind, down and to the right: a collection.
    let sheet = |d: f32| container(container(space()).width(width).height(face_h).style(move |t: &Theme| sheet_style(t, d))).padding(Padding { top: 3.0 + d, left: d, ..Padding::ZERO });
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
    fn the_cache_reads_current_frames_and_makes_the_rest_one_at_a_time() {
        let root = std::env::temp_dir().join(format!("omnix-cache-{}", std::process::id()));
        let l = dailix_sheet::layout::Layout::new(&root);
        l.create().unwrap();
        std::fs::write(l.project(), "{}").unwrap();
        let edits: Vec<PathBuf> = ["a", "b", "c"].map(|v| l.video_edit(v)).to_vec();
        for e in &edits {
            std::fs::create_dir_all(e.parent().unwrap()).unwrap();
            std::fs::write(e, "{}").unwrap();
        }
        // a's frames there and newer than it: read, not made.
        let tiny: Vec<crate::stitch::Frame> = (0..super::super::FRAMES).map(|_| crate::stitch::Frame { width: 2, height: 2, rgb: vec![99; 12], deep: false }).collect();
        let strip = super::super::strip_path(&edits[0]).unwrap();
        std::fs::create_dir_all(strip.parent().unwrap()).unwrap();
        std::fs::write(&strip, super::super::strip(&tiny).unwrap()).unwrap();
        std::fs::File::options().write(true).open(&edits[0]).unwrap().set_modified(std::time::SystemTime::UNIX_EPOCH).unwrap();
        let mut cache = Cache::default();
        assert_eq!(cache.want(&edits), Some(edits[1].clone()), "b made first, one at a time");
        assert_eq!((cache.get(&edits[0]).0.len(), cache.making(&edits[1])), (super::super::FRAMES, true));
        assert_eq!(cache.want(&edits), None, "b still being made");
        cache.made(&edits[1], Err("no footage".into()));
        assert_eq!(cache.want(&edits), Some(edits[2].clone()), "b not tried again");
        // b changed (filled, its disk back): tried again once c is done.
        cache.made(&edits[2], Ok(()));
        std::fs::File::options().write(true).open(&edits[1]).unwrap().set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(cache.want(&edits), Some(edits[1].clone()), "b, changed, tried again");
        // A strip that won't read is made again.
        cache.made(&edits[1], Ok(()));
        std::fs::write(&strip, b"not a picture").unwrap();
        std::fs::File::options().write(true).open(&edits[0]).unwrap().set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1)).unwrap();
        assert_eq!(cache.want(&edits), Some(edits[0].clone()), "a's strip spoiled");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn at_rest_each_tile_shows_the_middle_of_its_quarter() {
        let rest = tiles(16, None).map(|t| (t.to, t.mix));
        assert_eq!(rest, super::super::AT_REST.map(|k| (k, 1.0)));
        // Fewer frames than tiles: round them.
        assert_eq!(tiles(3, None).map(|t| t.to), [0, 1, 2, 0]);
    }

    #[test]
    fn a_card_of_one_two_or_four_frames_stays_still() {
        assert_eq!((0..=8).filter(|&n| moves(n)).collect::<Vec<_>>(), [3, 5, 6, 7, 8]);
        assert!(moves(16));
    }

    #[test]
    fn a_few_frames_all_come_round_never_two_alike_at_once() {
        // Five posters: the tiles take them in turn, every one shows, and
        // any moment's four are different.
        let mut seen = std::collections::BTreeSet::new();
        for k in 0..40 {
            let shown = tiles(5, at(k as f64 * 0.5)).map(|t| t.to);
            seen.extend(shown);
            let mut four = shown.to_vec();
            four.sort_unstable();
            four.dedup();
            assert_eq!(four.len(), 4, "at {} s: {shown:?}", k as f64 * 0.5);
        }
        assert_eq!(seen.len(), 5);
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
