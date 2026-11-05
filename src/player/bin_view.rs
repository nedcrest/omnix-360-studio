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
/// How long the pointer rests on a clip's tile before its frames show.
const HOVER: std::time::Duration = std::time::Duration::from_secs(1);
/// A hovered clip's frames: each this wide, two by two.
const HOVER_W: f32 = 220.0;
/// The right side's default width, and the least it and the tiles keep.
pub const SIDE_DEFAULT: f32 = 460.0;
const MIN_SIDE: f32 = 260.0;
const MIN_TILES: f32 = 200.0;
/// The share of the view's height the pictures at the side take at most.
const PICTURES: f32 = 0.5;
/// The edge between them: wide enough to grab.
const GRIP: f32 = 9.0;

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
    /// The edge between the tiles and the file shown pressed (true) or let go.
    Grip(bool),
    /// What Info read of the file shown.
    About(PathBuf, Result<std::sync::Arc<super::info::File>, String>),
    /// A tile clicked: the file shown at the right, not played.
    Show(PathBuf, Kind),
    /// The pointer out of the bin view: no tile is hovered.
    Leave,
    /// The pointer onto a clip's tile, and off it.
    HoverIn(PathBuf),
    HoverOut(PathBuf),
    /// A screen frame while the pointer rests on a tile: past `HOVER`, its
    /// frames show.
    HoverTick,
    /// A hovered clip's frames read.
    HoverFrames(PathBuf, Vec<image::Handle>),
    /// A file dropped on the window from a file manager.
    Dropped(PathBuf),
    /// A section's «+»: pick files to copy into its folder.
    AddTo(&'static str),
    /// Files to copy into a section's folder (picked), or by their kind.
    CopyIn(Vec<PathBuf>, Option<&'static str>),
    /// The copies done: where each went, or why not.
    Copied(Vec<Result<PathBuf, String>>),
    /// Move a copy in the project's folder to the system's trash: asked
    /// first, then done if the answer is yes.
    Trash(PathBuf),
    TrashAnswered(PathBuf, bool),
    Trashed(PathBuf, Result<(), String>),
    /// A loaded clip from outside the folder taken off the bin's list.
    Hide(PathBuf),
}

/// The folder under `root` a file of `kind` goes in: the section's named
/// `title`, else the first section of its kind (Footage, Music, Graphics).
pub fn folder_for(root: &Path, kind: Kind, title: Option<&str>) -> Option<PathBuf> {
    super::bin::SECTIONS.iter().find(|(t, k, _)| k.alike(kind) && title.is_none_or(|title| *t == title)).map(|(_, _, dirs)| root.join(dirs[0]))
}

/// Copies `file` into `dir` and says where it went. Copied under a hidden
/// name first and renamed when whole, so the bin never lists half a file.
/// A file of the same name and size there is taken as this one already
/// copied; another of the same name gets a number. The original is only
/// read (an archive's files are never changed).
pub fn copy_in(file: &Path, dir: &Path) -> Result<PathBuf, String> {
    let name = file.file_name().ok_or("no file name")?.to_string_lossy().into_owned();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let size = std::fs::metadata(file).map_err(|e| format!("{name}: {e}"))?.len();
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    let mut target = dir.join(&name);
    let mut n = 2;
    while let Ok(there) = std::fs::metadata(&target) {
        if there.len() == size {
            return Ok(target);
        }
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    let part = dir.join(format!(".{}.part", target.file_name().unwrap_or_default().to_string_lossy()));
    std::fs::copy(file, &part).and_then(|_| std::fs::rename(&part, &target)).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("{name}: {e}")
    })?;
    Ok(target)
}

/// A time as minutes, seconds and tenths: the bin player's readout.
fn fine_clock(t: f64) -> String {
    let tenths = (t.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
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
                self.bin.copied = None;
                // Its player stops with it, and opens again with it on the
                // clip shown at the side (it showed only frames before).
                self.bin.source = None;
                if self.bin.view {
                    // One view in the picture's place at a time.
                    self.check.on = false;
                    let reopen = self.bin.preview.as_ref().filter(|p| p.kind == Kind::Clip).map(|p| p.path.clone());
                    let meta = self.read_bin_meta();
                    return Task::batch([meta, reopen.map_or_else(Task::none, |p| self.source_open(p))]);
                }
            }
            BinMsg::Tile(w) => self.bin.tile = w.clamp(*TILE.start(), *TILE.end()),
            BinMsg::Pointer(p) => {
                self.bin.pointer = p;
                // A move closes the frames and starts the wait again.
                if let Some((_, since, shown)) = &mut self.bin.hover {
                    *since = std::time::Instant::now();
                    *shown = false;
                }
                if self.bin.gripping {
                    let width = self.bin.size.get().width;
                    self.bin.side = width - p.x - GRIP / 2.0;
                    self.bin.side = self.side_width(width);
                }
            }
            BinMsg::Grip(on) => self.bin.gripping = on,
            BinMsg::Leave => self.bin.hover = None,
            BinMsg::HoverIn(path) => self.bin.hover = Some((path, std::time::Instant::now(), false)),
            BinMsg::HoverOut(path) => {
                if self.bin.hover.as_ref().is_some_and(|(p, _, _)| *p == path) {
                    self.bin.hover = None;
                }
            }
            BinMsg::HoverTick => {
                let Some((path, since, shown)) = &mut self.bin.hover else { return Task::none() };
                if *shown || since.elapsed() < HOVER || self.bin.menu.is_some() {
                    return Task::none();
                }
                *shown = true;
                let path = path.clone();
                if self.bin.hover_frames.contains_key(&path) {
                    return Task::none();
                }
                self.bin.hover_frames.insert(path.clone(), Vec::new());
                let duration = self.bin.meta.get(&path).and_then(|m| m.duration).unwrap_or(10.0);
                return Task::perform(super::blocking({
                    let path = path.clone();
                    move || super::bin::frames(&path, duration, 320)
                }), move |f| Message::BinView(BinMsg::HoverFrames(path.clone(), f)));
            }
            BinMsg::HoverFrames(path, frames) => {
                self.bin.hover_frames.insert(path, frames);
            }
            BinMsg::Show(path, kind) => {
                let shown = self.bin_preview(path.clone(), kind, false);
                // A clip plays at the side, in the bin's own player.
                if kind != Kind::Clip {
                    self.bin.source = None;
                    return shown;
                }
                return Task::batch([shown, self.source_open(path)]);
            }
            BinMsg::About(path, file) => {
                if let Some((p, f)) = &mut self.bin.about
                    && *p == path
                {
                    *f = Some(file);
                }
            }
            BinMsg::Menu(m) => self.bin.menu = m.map(|(path, kind)| (path, kind, self.bin.pointer)),
            BinMsg::Pick(m) => {
                self.bin.menu = None;
                return self.update(*m);
            }
            BinMsg::Dropped(path) => return self.copy_files(vec![path], None),
            BinMsg::AddTo(title) => {
                let Some(&(_, kind, _)) = super::bin::SECTIONS.iter().find(|(t, _, _)| *t == title) else { return Task::none() };
                let exts = match kind {
                    Kind::Clip => super::bin::CLIP_EXT,
                    Kind::Sound(_) => super::bin::SOUND_EXT,
                    Kind::Picture => super::bin::PICTURE_EXT,
                };
                // Upper case too: the file dialog's filter is exact on Linux.
                let exts: Vec<String> = exts.iter().flat_map(|e| [e.to_string(), e.to_uppercase()]).collect();
                return Task::perform(
                    async move { rfd::AsyncFileDialog::new().set_title(format!("Add to {title}")).add_filter(title, &exts).pick_files().await.map(|fs| fs.iter().map(|f| f.path().to_owned()).collect::<Vec<_>>()) },
                    move |files| Message::BinView(BinMsg::CopyIn(files.unwrap_or_default(), Some(title))),
                );
            }
            BinMsg::CopyIn(files, title) => return self.copy_files(files, title),
            BinMsg::Copied(done) => {
                self.bin.copying = self.bin.copying.saturating_sub(done.len());
                let (ok, failed): (Vec<_>, Vec<_>) = done.into_iter().partition(Result::is_ok);
                if let Some(Err(e)) = failed.into_iter().next() {
                    self.error = Some(format!("Couldn't copy into the project: {e}"));
                }
                if !ok.is_empty() {
                    self.bin.copied = Some(if ok.len() == 1 { "1 file added".to_owned() } else { format!("{} files added", ok.len()) });
                }
                return self.rescan_bin();
            }
            BinMsg::Trash(path) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                return self.ask(super::ask::Ask {
                    title: "Move to trash".to_owned(),
                    text: format!("Move {name} from the project's folder to the trash? It can be restored from there."),
                    answers: vec![("Move to trash".to_owned(), Message::BinView(BinMsg::TrashAnswered(path.clone(), true)))],
                    cancel: Message::BinView(BinMsg::TrashAnswered(path, false)),
                });
            }
            BinMsg::TrashAnswered(path, true) => {
                // Asked again now: the edit may have changed while the
                // question was open.
                if let Some(used) = uses(&self.edit, &path) {
                    self.error = Some(format!("Still in the edit ({used}): not moved to the trash"));
                    return Task::none();
                }
                return Task::perform(super::blocking(move || (trash::delete(&path).map_err(|e| e.to_string()), path)), |(r, path)| Message::BinView(BinMsg::Trashed(path, r)));
            }
            BinMsg::TrashAnswered(_, false) => {}
            BinMsg::Trashed(path, result) => {
                match result {
                    Ok(()) => {
                        self.bin.meta.remove(&path);
                        if self.bin.preview.as_ref().is_some_and(|p| p.path == path) {
                            self.bin.preview = None;
                        }
                    }
                    Err(e) => self.error = Some(format!("Couldn't move it to the trash: {e}")),
                }
                return self.rescan_bin();
            }
            BinMsg::Hide(path) => {
                self.bin.hidden.insert(path);
            }
            BinMsg::Info(path, kind) => {
                self.bin.menu = None;
                return self.open_info(super::info::Of::File(matches!(kind, Kind::Sound(_))), path);
            }
        }
        Task::none()
    }

    /// Copies `files` into the project's folder: into the section named
    /// `title`, else each into its kind's first section. A file already
    /// inside the project's folder stays where it is.
    fn copy_files(&mut self, files: Vec<PathBuf>, title: Option<&'static str>) -> Task<Message> {
        if files.is_empty() {
            return Task::none();
        }
        let Some(root) = self.bin.read.as_ref().map(|(_, r)| r.clone()) else {
            self.error = Some("Save the project first: files are copied into its folder".to_owned());
            return Task::none();
        };
        let mut jobs = Vec::new();
        for file in files {
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let Some(kind) = super::bin::kind_of(&file).filter(|_| file.is_file()) else {
                self.error = Some(format!("{name}: not a clip, sound or picture"));
                continue;
            };
            if file.starts_with(&root) {
                continue;
            }
            match folder_for(&root, kind, title) {
                Some(dir) => jobs.push((file, dir)),
                None => self.error = Some(format!("{name} doesn't belong in {}", title.unwrap_or("the bin"))),
            }
        }
        if jobs.is_empty() {
            return self.rescan_bin();
        }
        self.bin.copying += jobs.len();
        self.bin.copied = None;
        Task::perform(super::blocking(move || jobs.iter().map(|(f, d)| copy_in(f, d)).collect::<Vec<_>>()), |done| Message::BinView(BinMsg::Copied(done)))
    }

    /// The bin in tiles, in the picture's place.
    pub(super) fn bin_view(&self) -> Element<'_, Message> {
        let to = |m: BinMsg| Message::BinView(m);
        // The view's width, to turn the pointer's place into the side's width.
        let view = iced::widget::responsive(move |size| {
            self.bin.size.set(size);
            self.bin_body()
        });
        let view = mouse_area(view).on_move(move |p| to(BinMsg::Pointer(p))).on_release(to(BinMsg::Grip(false))).on_exit(to(BinMsg::Leave));
        // The layers are always there, empty when unused: wrapping the view
        // only while a card or menu shows rebuilt its state each time (the
        // tiles scrolled back to the top, a note lost its focus, the role
        // list closed, a tile forgot the pointer was on it).
        stack![view, self.hover_layer(), self.bin_menu_layer()].into()
    }

    /// The bin view's head, its tiles, and the file shown at the right.
    fn bin_body(&self) -> Element<'_, Message> {
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
        if self.bin.copying > 0 {
            head = head.push(text(format!("Copying {} into the project…", if self.bin.copying == 1 { "a file".to_owned() } else { format!("{} files", self.bin.copying) })).size(13));
        } else if let Some(note) = &self.bin.copied {
            head = head.push(text(note.clone()).size(13).style(text::success));
        }
        let head = head.push(space().width(Length::Fill)).push(size).push(refresh).push(close);
        let mut body = column![].spacing(10);
        match &self.bin.read {
            None => body = body.push(text("Save the project to see its folder here.").size(13).style(text::secondary)),
            // Every section, an empty one too: its «+» is where to add.
            Some((sections, _)) => {
                for &(title, kind, _) in &super::bin::SECTIONS {
                    // Its own files and each subfolder's (a day's), the «+» on
                    // the first; an empty section still shows, for its «+».
                    let mine: Vec<&super::bin::Section> = sections.iter().filter(|s| s.title == title).collect();
                    if mine.is_empty() {
                        body = body.push(rule::horizontal(1)).push(self.section_tiles(title, title.to_owned(), kind, &[], true));
                    }
                    for (i, s) in mine.into_iter().enumerate() {
                        body = body.push(rule::horizontal(1)).push(self.section_tiles(title, s.heading(), kind, &s.files, i == 0));
                    }
                }
            }
        }
        let loaded = self.loaded_outside();
        if !loaded.is_empty() {
            body = body.push(rule::horizontal(1)).push(self.section_tiles("Loaded clips", "Loaded clips".to_owned(), Kind::Clip, &loaded, false));
        }
        let tiles = scrollable(body.padding([0, 14])).width(Length::Fill).height(Length::Fill);
        // The file shown at the right, beside the tiles, once one is picked;
        // the edge between them dragged shares the width.
        let below: Element<'_, Message> = match &self.bin.preview {
            None => tiles.into(),
            Some(p) => {
                let side = self.side_width(self.bin.size.get().width);
                let grip = mouse_area(container(rule::vertical(1)).width(GRIP).height(Length::Fill).center_x(GRIP))
                    .on_press(to(BinMsg::Grip(true)))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally);
                row![tiles, grip, container(self.side_pane(p)).width(side).height(Length::Fill)].into()
            }
        };
        column![container(head).padding([10, 14]), below].into()
    }

    /// The right side's width in a view `width` wide: as dragged, but
    /// leaving room for a column of tiles and for the file shown.
    fn side_width(&self, width: f32) -> f32 {
        self.bin.side.min(width - GRIP - MIN_TILES).max(MIN_SIDE)
    }

    /// The file shown at the right: its pictures (a clip's frames, the
    /// picture, a sound's note), play and stop, the add buttons, and what
    /// Info reads of it.
    fn side_pane<'a>(&'a self, p: &'a super::bin::Preview) -> Element<'a, Message> {
        let name = p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let playing = p.player.is_some();
        let play: Element<'a, Message> = match p.kind {
            Kind::Picture => space().into(),
            Kind::Clip => {
                let playing = self.source_playing();
                let ready = self.bin.source.as_ref().is_some_and(|s| s.path == p.path && s.clip.is_some());
                tip(
                    button(text(if playing { "⏸" } else { "▶" }).size(14)).padding([2, 10]).on_press_maybe(ready.then_some(Message::BinPlayer(super::bin_player::SourceMsg::Toggle))),
                    text(if playing { "Pause (Space)" } else { "Play (Space)" }),
                    tooltip::Position::Bottom,
                )
            }
            _ => tip(
                button(text(if playing { "■" } else { "▶" }).size(14)).padding([2, 10]).on_press(if playing { Message::BinStop } else { Message::BinPreview(p.path.clone(), p.kind) }),
                text(if playing { "Stop" } else if p.kind == Kind::Clip { "Play (in a window of its own)" } else { "Play" }),
                tooltip::Position::Bottom,
            ),
        };
        let mut col = column![text(name).size(15), row![play, space().width(Length::Fill), super::bin::add_buttons(&p.path, p.kind)].align_y(Alignment::Center)].spacing(8);
        // The pictures take at most half the view's height, however wide
        // the side is dragged: past that they stop growing, and Info stays
        // where it is.
        let cap = (self.bin.size.get().height * PICTURES).max(120.0);
        let fit = |h: &image::Handle, most: f32| container(image(h.clone()).width(Length::Fill)).max_height(most).center_x(Length::Fill);
        match p.kind {
            // The clip in the player once its first frame is in; its frames
            // till then.
            Kind::Clip if let Some((picture, aspect, at, length)) = self.source_picture().filter(|_| self.bin.source.as_ref().is_some_and(|s| s.path == p.path)) => {
                let room = self.side_width(self.bin.size.get().width) - 24.0;
                let h = (room / aspect).min(cap);
                let w = h * aspect;
                col = col.push(container(iced::widget::shader(picture).width(w).height(h)).center_x(Length::Fill));
                let source = self.bin.source.as_ref();
                let marks: &[crate::notes::Segment] = source.map_or(&[], |s| &s.marks);
                let strip = super::bin_player::Strip {
                    at,
                    length,
                    marks: marks.iter().map(|m| (m.start, m.end, super::bin_player::role_colour(m.role.as_deref()))).collect(),
                    picked: source.and_then(|s| s.picked),
                    mark_in: source.and_then(|s| s.mark_in),
                };
                col = col.push(iced::widget::canvas(strip).width(Length::Fill).height(if marks.is_empty() { 18 } else { 30 }));
                col = col.push(text(format!("{} / {}", fine_clock(at), fine_clock(length))).size(12).style(text::secondary));
                // The picked segment: its role, span and the sheet's note.
                if let Some(m) = source.and_then(|s| s.picked.and_then(|i| s.marks.get(i))) {
                    let span = match m.end {
                        Some(e) => format!("{} – {}", fine_clock(m.start), fine_clock(e)),
                        None => fine_clock(m.start),
                    };
                    // Its role and note, as the contact sheet sets them.
                    let none = crate::notes::Role { key: String::new(), name: "—".to_owned() };
                    let choices: Vec<crate::notes::Role> = std::iter::once(none.clone()).chain(self.bin.roles.iter().cloned()).collect();
                    let current = m.role.as_ref().and_then(|r| choices.iter().find(|c| &c.key == r).cloned()).unwrap_or(none);
                    let role = iced::widget::pick_list(choices, Some(current), |r: crate::notes::Role| Message::BinPlayer(super::bin_player::SourceMsg::Role((!r.key.is_empty()).then_some(r.key))))
                        .text_size(13)
                        .placeholder("role…");
                    // It wraps, so a long note reads whole; Enter saves it.
                    let note: Element<'_, Message> = match source {
                        Some(s) => iced::widget::text_editor(&s.note)
                            .placeholder("what happens here")
                            .size(13)
                            .wrapping(text::Wrapping::WordOrGlyph)
                            .on_action(|a| Message::BinPlayer(super::bin_player::SourceMsg::Note(a)))
                            .key_binding(|press| match press.key.as_ref() {
                                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => Some(iced::widget::text_editor::Binding::Custom(Message::BinPlayer(super::bin_player::SourceMsg::NoteSave))),
                                _ => iced::widget::text_editor::Binding::from_key_press(press),
                            })
                            .into(),
                        None => space().into(),
                    };
                    let colour = super::bin_player::role_colour(m.role.as_deref());
                    let remove = tip(
                        button(text("✕").size(13)).padding([2, 8]).style(button::danger).on_press(Message::BinPlayer(super::bin_player::SourceMsg::Remove)),
                        text("Delete this segment (Delete)"),
                        tooltip::Position::Bottom,
                    );
                    col = col.push(row![
                        container(space()).width(4).height(28).style(move |_| container::background(colour)),
                        text(span).size(13),
                        role,
                        space().width(Length::Fill),
                        remove,
                    ].spacing(8).align_y(Alignment::Center));
                    col = col.push(note);
                }
            }
            // Till the player's first frame: the tile's picture.
            Kind::Clip => match self.bin.meta.get(&p.path).and_then(|m| m.thumb.clone()) {
                Some(h) => col = col.push(fit(&h, cap)),
                None => col = col.push(text("…").size(20)),
            },
            Kind::Picture => {
                if let Some(h) = self.bin.meta.get(&p.path).and_then(|m| m.thumb.clone()) {
                    col = col.push(fit(&h, cap));
                }
            }
            Kind::Sound(_) => col = col.push(container(text("♪").size(64)).width(Length::Fill).center_x(Length::Fill)),
        }
        col = col.push(rule::horizontal(1));
        let line = |label: &'static str, value: String| row![text(label).size(13).style(text::secondary).width(96), text(value).size(13).width(Length::Fill)].spacing(6);
        col = col.push(line("In the edit", uses(&self.edit, &p.path).unwrap_or_else(|| "not used".to_owned())));
        match self.bin.about.as_ref().filter(|(path, _)| *path == p.path).map(|(_, f)| f) {
            None | Some(None) => col = col.push(text("Reading…").size(13).style(text::secondary)),
            Some(Some(Err(e))) => col = col.push(text(e.clone()).size(13).style(text::danger)),
            Some(Some(Ok(file))) => {
                col = col.push(line("Kind", file.kind.to_owned()));
                for (label, value) in &file.rows {
                    col = col.push(line(label, value.clone()));
                }
            }
        }
        container(scrollable(col.padding(12))).height(Length::Fill).style(container::rounded_box).into()
    }

    /// The clips loaded that the folder's scan doesn't list: a project's
    /// footage is often read in place from an archive outside its folder.
    pub(super) fn loaded_outside(&self) -> Vec<PathBuf> {
        let listed = |p: &Path| self.bin.read.as_ref().is_some_and(|(s, _)| s.iter().any(|s| s.files.iter().any(|f| f == p)));
        let mut loaded: Vec<PathBuf> = self.library.iter().map(|m| m.input.clone()).filter(|p| !listed(p) && !self.bin.hidden.contains(p)).collect();
        loaded.sort();
        loaded.dedup();
        loaded
    }

    /// A heading and its files as tiles, wrapping to the view's width.
    fn section_tiles<'a>(&'a self, title: &'static str, heading: String, kind: Kind, files: &[PathBuf], add: bool) -> Element<'a, Message> {
        let mut head = row![text(format!("{heading}  ·  {}", files.len())).size(15)].spacing(8).align_y(Alignment::Center);
        if add {
            head = head.push(tip(
                button(text("+").size(14)).padding([0, 8]).on_press(Message::BinView(BinMsg::AddTo(title))),
                text(format!("Copy files into {title} (or drop them on the bin)")),
                tooltip::Position::Right,
            ));
        }
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
            .on_press(Message::BinView(BinMsg::Show(path.to_owned(), kind)));
        let area = mouse_area(tile).on_right_press(Message::BinView(BinMsg::Menu(Some((path.to_owned(), kind)))));
        // A clip's frames show when the pointer rests on its tile.
        match kind {
            Kind::Clip => area.on_enter(Message::BinView(BinMsg::HoverIn(path.to_owned()))).on_exit(Message::BinView(BinMsg::HoverOut(path.to_owned()))).into(),
            _ => area.into(),
        }
    }

    /// A hovered clip's frames beside the pointer, kept inside the view.
    fn hover_layer<'a>(&'a self) -> Element<'a, Message> {
        let Some((path, _, true)) = &self.bin.hover else { return space().into() };
        let Some(frames) = self.bin.hover_frames.get(path) else { return space().into() };
        let body: Element<'a, Message> = if frames.is_empty() {
            text("…").size(20).into()
        } else {
            column(frames.chunks(2).map(|pair| row(pair.iter().map(|h| image(h.clone()).width(HOVER_W).into())).spacing(4).into())).spacing(4).into()
        };
        let card = container(body).padding(6).style(container::rounded_box);
        let (w, h) = (HOVER_W * 2.0 + 16.0, HOVER_W * 9.0 / 16.0 * 2.0 + 16.0);
        let room = self.bin.size.get();
        let p = self.bin.pointer;
        let x = (p.x + 16.0).min(room.width - w).max(0.0);
        let y = if p.y + 16.0 + h > room.height { (p.y - 16.0 - h).max(0.0) } else { p.y + 16.0 };
        pin(card).position(Point::new(x, y)).into()
    }

    /// A tile's menu at the pointer, if open: a click outside closes it.
    fn bin_menu_layer<'a>(&'a self) -> Element<'a, Message> {
        let Some((path, kind, at)) = &self.bin.menu else { return space().into() };
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
        // Only a copy inside the project's folder goes to the trash; raw
        // material outside it (an archive, a card) is only taken off the list.
        // Neither while the edit uses it.
        items.push(rule::horizontal(1).into());
        let inside = self.bin.read.as_ref().is_some_and(|(_, root)| path.starts_with(root));
        let label = if inside { "Move to trash…" } else { "Take out of the bin" };
        match uses(&self.edit, path) {
            Some(used) => items.push(button(text(format!("{label} (in the edit: {used})")).size(13)).style(button::text).width(Length::Fill).padding([5, 10]).into()),
            None if inside => items.push(item(label, Message::BinView(BinMsg::Trash(path.clone())))),
            None => items.push(item(label, Message::BinView(BinMsg::Hide(path.clone())))),
        }
        let menu = container(column(items).width(280)).padding(4).style(container::rounded_box);
        let close = mouse_area(container(space()).width(Length::Fill).height(Length::Fill)).on_press(Message::BinView(BinMsg::Menu(None))).on_right_press(Message::BinView(BinMsg::Menu(None)));
        stack![close, pin(opaque(menu)).position(at)].into()
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, copy_in, folder_for, uses};
    use crate::project::{Cut, Project, SoundTrack};
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

    #[test]
    fn a_file_goes_in_its_sections_folder() {
        let root = Path::new("/p");
        assert_eq!(folder_for(root, Kind::Clip, None), Some(PathBuf::from("/p/01_footage")));
        assert_eq!(folder_for(root, Kind::Clip, Some("External video")), Some(PathBuf::from("/p/06_external")));
        assert_eq!(folder_for(root, Kind::Sound(SoundTrack::Music), Some("Sound effects")), Some(PathBuf::from("/p/07_sfx")));
        assert_eq!(folder_for(root, Kind::Picture, None), Some(PathBuf::from("/p/08_graphics")));
        // A sound picked for the Graphics section doesn't belong there.
        assert_eq!(folder_for(root, Kind::Sound(SoundTrack::Music), Some("Graphics")), None);
    }

    #[test]
    fn copying_in_keeps_the_original_and_numbers_a_clash() {
        let dir = std::env::temp_dir().join(format!("omnix-copy-in-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (src, other, into) = (dir.join("src/song.mp3"), dir.join("other/song.mp3"), dir.join("p/02_audio/music"));
        for (f, body) in [(&src, &b"abc"[..]), (&other, &b"abcdef"[..])] {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, body).unwrap();
        }
        assert_eq!(copy_in(&src, &into), Ok(into.join("song.mp3")));
        // The same file again: taken as already there, not copied twice.
        assert_eq!(copy_in(&src, &into), Ok(into.join("song.mp3")));
        // Another of the same name: a number, the first left alone.
        assert_eq!(copy_in(&other, &into), Ok(into.join("song (2).mp3")));
        assert_eq!(std::fs::read(into.join("song.mp3")).unwrap(), b"abc");
        assert_eq!(std::fs::read(&src).unwrap(), b"abc");
        // No half copy left behind.
        let mut names: Vec<_> = std::fs::read_dir(&into).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["song (2).mp3", "song.mp3"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
