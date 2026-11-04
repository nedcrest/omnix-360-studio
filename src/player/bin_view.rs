//! The bin as a view of its own (gap 60): the project folder's media in
//! tiles, in the picture's place, as the mix check stands there. The
//! timeline stays below, so what a tile adds lands where it can be seen.
//!
//! The narrow bin panel (`bin.rs`) is for adding quickly while cutting;
//! this view is for looking over and sorting the material. Both read the
//! same scan and the same pictures, and preview the same way.

use super::bin::{Kind, Place};
use super::{App, Message, tip};
use iced::widget::{button, column, container, image, mouse_area, opaque, pin, row, rule, scrollable, slider, space, stack, text, tooltip};
use iced::{Alignment, Element, Length, Point, Task};
use std::path::{Path, PathBuf};

/// A tile's width: the slider's range and where it starts.
pub const TILE: std::ops::RangeInclusive<f32> = 140.0..=420.0;
pub const TILE_DEFAULT: f32 = 220.0;

/// The bin view's messages.
#[derive(Debug, Clone)]
pub enum BinMsg {
    /// Show the bin in the picture's place, or the picture again.
    Toggle,
    /// The tiles' width.
    Tile(f32),
    /// The pointer over the view, where a menu opens.
    Pointer(Point),
    /// A tile's menu opened at the pointer, or closed.
    Menu(Option<(PathBuf, Kind)>),
    /// The Info panel on a file of the bin.
    Info(PathBuf, Kind),
    /// A menu item chosen: the menu closes, then the item's message.
    Pick(Box<Message>),
}

/// Where a file is used in the edit, as a line: "cuts 2, 5 · Music · Overlay";
/// `None` when nothing uses it.
pub fn uses(edit: &crate::project::Project, path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    let cuts: Vec<String> = edit.cuts.iter().enumerate().filter(|(_, c)| c.input == path || c.audio.as_deref() == Some(path)).map(|(i, _)| (i + 1).to_string()).collect();
    match cuts.len() {
        0 => {}
        1 => parts.push(format!("cut {}", cuts[0])),
        _ => parts.push(format!("cuts {}", cuts.join(", "))),
    }
    for which in crate::project::SoundTrack::ALL {
        let n = edit.track(which).clips.iter().filter(|m| m.input == path).count();
        if n > 0 {
            parts.push(if n == 1 { which.name().to_owned() } else { format!("{} ×{n}", which.name()) });
        }
    }
    for (name, lanes) in [("Overlay", &edit.overlays), ("Text", &edit.text)] {
        let n = lanes.iter().flat_map(|l| &l.items).filter(|it| it.input == path).count();
        if n > 0 {
            parts.push(if n == 1 { name.to_owned() } else { format!("{name} ×{n}") });
        }
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

impl App {
    pub(super) fn on_bin_view(&mut self, msg: BinMsg) -> Task<Message> {
        match msg {
            BinMsg::Toggle => {
                self.bin.view = !self.bin.view;
                self.bin.menu = None;
                // One view in the picture's place at a time.
                if self.bin.view {
                    self.check.on = false;
                }
            }
            BinMsg::Tile(w) => self.bin.tile = w.clamp(*TILE.start(), *TILE.end()),
            BinMsg::Pointer(p) => self.bin.pointer = p,
            BinMsg::Menu(m) => self.bin.menu = m.map(|(path, kind)| (path, kind, self.bin.pointer)),
            BinMsg::Pick(m) => {
                self.bin.menu = None;
                return self.update(*m);
            }
            BinMsg::Info(path, kind) => {
                self.bin.menu = None;
                return self.open_info(super::info::Of::File(matches!(kind, Kind::Sound(_))), path);
            }
        }
        Task::none()
    }

    /// The bin in tiles, in the picture's place.
    pub(super) fn bin_view(&self) -> Element<'_, Message> {
        let to = |m: BinMsg| Message::BinView(m);
        let size = tip(
            slider(TILE, self.bin.tile, move |w| to(BinMsg::Tile(w))).width(140),
            text("The tiles' size"),
            tooltip::Position::Bottom,
        );
        let refresh = tip(button(text("↻").size(14)).padding([0, 6]).style(button::text).on_press(Message::BinRefresh), text("Read the folder again"), tooltip::Position::Bottom);
        let close = tip(button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(to(BinMsg::Toggle)), text("Back to the picture (Shift+B)"), tooltip::Position::Bottom);
        let mut head = row![text("Bin").size(20)].spacing(12).align_y(Alignment::Center);
        if let Some((_, root)) = &self.bin.read {
            head = head.push(text(root.display().to_string()).size(12).style(text::secondary));
        }
        let head = head.push(space().width(Length::Fill)).push(size).push(refresh).push(close);
        let mut body = column![head].spacing(10);

        if let Some(p) = &self.bin.preview {
            body = body.push(container(self.preview_box(p)).max_width(640));
        }
        match &self.bin.read {
            None => body = body.push(text("Save the project to see its folder here.").size(13).style(text::secondary)),
            Some((sections, _)) => {
                for section in sections {
                    body = body.push(rule::horizontal(1)).push(self.section_tiles(section.title, section.kind, &section.files));
                }
            }
        }
        let loaded = self.loaded_outside();
        if !loaded.is_empty() {
            body = body.push(rule::horizontal(1)).push(self.section_tiles("Loaded clips", Kind::Clip, &loaded));
        }
        let view = mouse_area(container(scrollable(body.padding(14)).width(Length::Fill).height(Length::Fill)).width(Length::Fill).height(Length::Fill)).on_move(move |p| to(BinMsg::Pointer(p)));
        self.bin_menu_layer(view.into())
    }

    /// The clips loaded that the folder's scan doesn't list: a project's
    /// footage is often read in place from an archive outside its folder.
    pub(super) fn loaded_outside(&self) -> Vec<PathBuf> {
        let listed = |p: &Path| self.bin.read.as_ref().is_some_and(|(s, _)| s.iter().any(|s| s.files.iter().any(|f| f == p)));
        let mut loaded: Vec<PathBuf> = self.library.iter().map(|m| m.input.clone()).filter(|p| !listed(p)).collect();
        loaded.sort();
        loaded.dedup();
        loaded
    }

    /// A heading and its files as tiles, wrapping to the view's width.
    fn section_tiles<'a>(&'a self, title: &'static str, kind: Kind, files: &[PathBuf]) -> Element<'a, Message> {
        let head = text(format!("{title}  ·  {}", files.len())).size(15);
        let tiles = row(files.iter().map(|f| self.tile(f, kind))).spacing(10).wrap().vertical_spacing(10);
        column![head, tiles].spacing(8).into()
    }

    /// A file as a tile: its picture, name and length, and where the edit
    /// uses it. A click previews it, a right-click opens its menu.
    fn tile<'a>(&'a self, path: &Path, kind: Kind) -> Element<'a, Message> {
        let w = self.bin.tile;
        let h = (w * 9.0 / 16.0).round();
        let meta = self.bin.meta.get(path);
        let picture: Element<'a, Message> = match meta.and_then(|m| m.thumb.clone()) {
            Some(handle) => image(handle).width(w).height(h).into(),
            None => container(text(if matches!(kind, Kind::Sound(_)) { "♪" } else { "…" }).size(h * 0.3))
                .width(w)
                .height(h)
                .center_x(w)
                .center_y(h)
                .style(|t: &iced::Theme| container::background(t.extended_palette().background.weak.color))
                .into(),
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let length = meta.and_then(|m| m.duration).map(super::bin::clock).unwrap_or_default();
        let mut info = column![text(name).size(13).wrapping(text::Wrapping::None), text(length).size(11).style(text::secondary)].spacing(1).width(w).clip(true);
        if let Some(used) = uses(&self.edit, path) {
            info = info.push(text(format!("In the edit: {used}")).size(11).style(text::success));
        }
        let selected = self.bin.preview.as_ref().is_some_and(|p| p.path == path);
        let tile = button(column![picture, info].spacing(4))
            .padding(4)
            .style(if selected { button::secondary } else { button::text })
            .on_press(Message::BinPreview(path.to_owned(), kind));
        mouse_area(tile).on_right_press(Message::BinView(BinMsg::Menu(Some((path.to_owned(), kind))))).into()
    }

    /// A tile's menu at the pointer, if open: a click outside closes it.
    fn bin_menu_layer<'a>(&'a self, below: Element<'a, Message>) -> Element<'a, Message> {
        let Some((path, kind, at)) = &self.bin.menu else { return below };
        let (kind, at) = (*kind, *at);
        let item = |label: &str, msg: Message| -> Element<'a, Message> { button(text(label.to_owned()).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).on_press(Message::BinView(BinMsg::Pick(Box::new(msg)))).into() };
        let add = |place: Place| Message::BinAdd(path.clone(), kind, place);
        let mut items: Vec<Element<'a, Message>> = Vec::new();
        match kind {
            Kind::Clip => {
                items.push(item("Add at the end of the edit", add(Place::End)));
                items.push(item("Add at the playhead", add(Place::Playhead)));
                items.push(item("Add at the join nearest the playhead", add(Place::Join)));
            }
            Kind::Sound(which) => {
                items.push(item(&format!("Add on {} after the last", which.name()), add(Place::End)));
                items.push(item(&format!("Add on {} at the playhead", which.name()), add(Place::Playhead)));
                items.push(item(&format!("Add on {} at the nearest join", which.name()), add(Place::Join)));
            }
            Kind::Picture => items.push(item("Add over the picture at the playhead", add(Place::Playhead))),
        }
        items.push(rule::horizontal(1).into());
        items.push(item("Info", Message::BinView(BinMsg::Info(path.clone(), kind))));
        items.push(item("Show in folder", Message::Reveal(path.clone())));
        items.push(item("Copy the path", Message::CopyPath(path.clone())));
        let menu = container(column(items).width(280)).padding(4).style(container::rounded_box);
        let close = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::BinView(BinMsg::Menu(None))).on_right_press(Message::BinView(BinMsg::Menu(None)));
        stack![below, close, pin(opaque(menu)).position(at)].into()
    }
}

#[cfg(test)]
mod tests {
    use super::uses;
    use crate::project::{Cut, Project};
    use std::path::{Path, PathBuf};

    #[test]
    fn uses_names_the_cuts_and_lanes_a_file_is_in() {
        let mut p = Project::default();
        p.cuts = vec![Cut::new(PathBuf::from("a.mp4"), 0.0, 4.0), Cut::new(PathBuf::from("b.mp4"), 0.0, 3.0), Cut::new(PathBuf::from("a.mp4"), 5.0, 9.0)];
        p.music.clips = vec![serde_json::from_value(serde_json::json!({"input": "song.mp3", "at": 0.0, "in": 0.0, "out": 5.0})).unwrap()];
        assert_eq!(uses(&p, Path::new("a.mp4")).as_deref(), Some("cuts 1, 3"));
        assert_eq!(uses(&p, Path::new("b.mp4")).as_deref(), Some("cut 2"));
        assert_eq!(uses(&p, Path::new("song.mp3")).as_deref(), Some("Music"));
        assert_eq!(uses(&p, Path::new("other.mp4")), None);
    }
}
