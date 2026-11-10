//! Overlays and text in the player: their pictures read or drawn for the
//! view, the text panel's words kept in step, what draws over the picture,
//! the on-screen handles, and their blocks on the timeline.

use std::path::PathBuf;
use super::*;
use crate::model::ops::items::ItemOp;
use crate::model::ops::{End, Op};

/// A small picture of an overlay for its timeline block: 48 px high,
/// over a dark checker so transparency shows.
pub(super) fn overlay_thumb(picture: &crate::overlay::Picture) -> timeline::Thumb {
    let h = 48usize;
    let w = (picture.width * h / picture.height.max(1)).clamp(1, 4 * h);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x * picture.width / w, y * picture.height / h);
            let p = &picture.rgba[(sy * picture.width + sx) * 4..][..4];
            let under = if (x / 6 + y / 6) % 2 == 0 { 60.0 } else { 90.0 };
            let a = p[3] as f32 / 255.0;
            rgba.extend([0, 1, 2].map(|c| (p[c] as f32 + under * (1.0 - a)).min(255.0) as u8));
            rgba.push(255);
        }
    }
    timeline::Thumb { image: iced::widget::image::Handle::from_rgba(w as u32, h as u32, rgba), aspect: w as f32 / h as f32 }
}

impl App {
    /// Starts getting each overlay picture the edit uses that isn't ready
    /// yet, at the view's quality: a file read (4096 px on its long side at
    /// most), text drawn (for a 1440 px high frame). Those no longer used
    /// (text since retyped) are let go.
    pub(super) fn load_overlay_pictures(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        let items: Vec<_> = self.edit.lanes().flat_map(|l| &l.items).cloned().collect();
        let keys: std::collections::HashSet<String> = items.iter().map(crate::overlay::Item::key).collect();
        self.overlay_pictures.retain(|k, _| keys.contains(k));
        self.overlay_thumbs.retain(|k, _| keys.contains(k));
        for item in items {
            let key = item.key();
            if self.overlay_pictures.contains_key(&key) {
                continue;
            }
            self.overlay_pictures.insert(key.clone(), None);
            tasks.push(Task::perform(
                blocking(move || item.picture(4096, 1440).map(|(p, missing)| (Arc::new(p), missing)).map_err(|e| format!("{e:#}"))),
                move |r| Message::OverlayPicture(key.clone(), r),
            ));
        }
        Task::batch(tasks)
    }

    /// Fills the panel's text box with the selected text overlay's words
    /// when the selection moves to another one.
    pub(super) fn settle_text(&mut self) {
        let words = self.selection.item(&self.edit).and_then(|r| self.edit.item(r)?.text.as_ref().map(|t| (r, t.content.clone())));
        match words {
            Some((i, content)) if self.text_of != Some(i) || self.text_content.text().trim_end_matches('\n') != content => {
                self.text_content = iced::widget::text_editor::Content::with_text(&content);
                self.text_of = Some(i);
            }
            Some(_) => {}
            None => self.text_of = None,
        }
    }

    /// The overlays showing at the playhead, lowest first, as the export
    /// burns them in.
    pub(super) fn overlay_draws(&self) -> Vec<gpu::OverlayDraw> {
        let t = self.position();
        let black = self.edit.fades.black(t, self.edit.duration());
        self.edit
            .shown_lanes()
            .filter_map(|lane| lane.at(t).map(|(_, it)| it))
            .filter_map(|item| Some((item, self.overlay_pictures.get(&item.key())?.clone()?)))
            .flat_map(|(item, (id, picture))| {
                let style = item.style.as_ref().map(|s| (s.spec(), item.local(t) as f32));
                crate::overlay::spots(item, t).into_iter().map(move |(spot, grade)| gpu::OverlayDraw { id, picture: picture.clone(), spot, grade, black, style })
            })
            .collect()
    }

    /// The overlays showing at the playhead for the view's handles, while
    /// paused (playing, a drag would fight the motion keyframes).
    pub(super) fn overlay_handles(&self) -> handles::Handles {
        let t = self.position();
        let playing = self.playing();
        let lanes = [crate::overlay::Track::Overlay, crate::overlay::Track::Text].into_iter().flat_map(|track| (0..self.edit.track_lanes(track).len()).map(move |l| (track, l)));
        let items = lanes
            .filter(|_| !playing)
            .filter_map(|(track, l)| {
                // Bars have no place to drag.
                let (i, item) = self.edit.lane(track, l)?.at(t).filter(|(_, it)| it.bars.is_none())?;
                let (_, picture) = self.overlay_pictures.get(&item.key())?.clone()?;
                Some(((track, l, i), crate::overlay::look(item, t).0, picture))
            })
            .collect();
        handles::Handles { items, selected: self.selection.item(&self.edit) }
    }

    /// Which lane of `track` something added goes on (gap 70): the
    /// selected item's, if it's on that track, else the active one.
    pub(super) fn adding_lane(&self, track: crate::overlay::Track) -> usize {
        let active = self.active_lanes[track_index(track)].min(self.shown_lanes(track) - 1);
        self.selection.item(&self.edit).filter(|r| r.0 == track).map_or(active, |r| r.1)
    }

    /// The selected item onto the lane above (`up`; a new one on top from
    /// the top lane) or below, where it fits nearest its time: one step.
    pub(super) fn item_to_lane(&mut self, up: bool) {
        let Some((track, l, i)) = self.selection.item(&self.edit) else { return };
        let Some(it) = self.edit.lane(track, l).and_then(|lane| lane.items.get(i)) else { return };
        let (item, at) = (it.id, it.at);
        let Some(to) = (if up { Some(l + 1) } else { l.checked_sub(1) }) else { return };
        self.edit.begin();
        if to == self.edit.track_lanes(track).len() {
            self.op(Op::Item(ItemOp::AddLane { track }));
        }
        self.op(Op::Item(ItemOp::Move { item, at, lane: Some(to) }));
        self.edit.commit();
    }

    /// Picture `path` onto lane `lane` of the overlay track at edit time
    /// `at`, where it fits nearest (gap 91). From outside the project's
    /// folder it's copied into its pictures first, as the bin copies.
    pub(super) fn drop_picture(&mut self, path: PathBuf, lane: usize, at: f64) {
        let root = self.bin.read.as_ref().map(|(_, r)| r.clone());
        let input = match root.filter(|r| !path.starts_with(r)) {
            Some(r) => match super::bin_view::copy_in(&path, &r.join(dailix_sheet::layout::PICTURES)) {
                Ok(copied) => copied,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            },
            None => path,
        };
        self.folds.overlay = false;
        if let Some(id) = self.op(Op::Item(ItemOp::AddPictures { inputs: vec![input], at, len: ffmpeg::PICTURE_SECONDS, lane })).and_then(|o| o.made) {
            self.selection = Selection::Item(id);
            self.active_lanes[track_index(crate::overlay::Track::Overlay)] = lane;
        }
    }

    /// How many lanes `track` shows: its own, one at least.
    pub(super) fn shown_lanes(&self, track: crate::overlay::Track) -> usize {
        self.edit.track_lanes(track).len().max(1)
    }

    /// The timeline's blocks for each lane of `track`, the lowest first.
    pub(super) fn track_blocks(&self, track: crate::overlay::Track) -> Vec<Vec<timeline::OverlayBlock>> {
        (0..self.shown_lanes(track)).map(|l| self.overlay_blocks(track, l)).collect()
    }

    /// The automation lanes of each lane of `track`.
    pub(super) fn lanes_of_track(&self, track: crate::overlay::Track) -> Vec<Vec<super::automation_ui::Param>> {
        (0..self.shown_lanes(track)).map(|l| self.lanes_of(super::automation_ui::LaneOf::Track(track, l))).collect()
    }

    /// The timeline's blocks for lane `l` of `track`.
    pub(super) fn overlay_blocks(&self, track: crate::overlay::Track, l: usize) -> Vec<timeline::OverlayBlock> {
        self.edit.lane(track, l).map_or_else(Vec::new, |lane| {
            lane.items
                .iter()
                .map(|it| timeline::OverlayBlock { item: it.clone(), label: it.name(), thumb: self.overlay_thumbs.get(&it.key()).cloned() })
                .collect()
        })
    }
}

impl App {
    /// Overlays and text: this area's messages, as `handle` passes them on.
    pub(super) fn on_overlay_ui(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddOverlay => {
                let dir = self.project.as_ref().and_then(|p| p.parent().map(Path::to_owned));
                return Task::perform(
                    async move {
                        let mut dialog = rfd::AsyncFileDialog::new()
                            .set_title("Add overlay pictures")
                            .add_filter("Pictures", &["png", "PNG", "jpg", "JPG", "jpeg", "JPEG", "webp", "WEBP", "bmp", "BMP", "tif", "TIF", "tiff", "TIFF"]);
                        if let Some(dir) = dir {
                            dialog = dialog.set_directory(dir);
                        }
                        dialog.pick_files().await.unwrap_or_default().iter().map(|f| f.path().to_owned()).collect()
                    },
                    Message::OverlayPicked,
                );
            }
            Message::OverlayPicked(paths) if !paths.is_empty() => {
                // Folded, the lane opens to show what came.
                self.folds.overlay = false;
                // One after another from the playhead, each where it fits.
                let added = self.op(Op::Item(ItemOp::AddPictures { inputs: paths, at: self.position(), len: ffmpeg::PICTURE_SECONDS, lane: self.adding_lane(crate::overlay::Track::Overlay) }));
                if let Some(id) = added.and_then(|o| o.made) {
                    self.selection = Selection::Item(id);
                }
            }
            Message::OverlayPicked(_) => {}
            Message::OverlaySelect(i) => {
                // Typing in another overlay is a change of its own for undo.
                self.edit.commit();
                match i {
                    Some(r) => {
                        self.selection = Selection::item_at(&self.edit, r);
                        self.active_lanes[track_index(r.0)] = r.1;
                    }
                    // An empty spot on its lane: only an item lets go.
                    None if self.selection.item(&self.edit).is_some() => self.selection = Selection::None,
                    None => {}
                }
            }
            Message::LaneAdd(track) => {
                self.op(Op::Item(ItemOp::AddLane { track }));
                self.active_lanes[track_index(track)] = self.edit.track_lanes(track).len() - 1;
            }
            Message::LaneRename(track, lane) => {
                let now = self.edit.lane(track, lane).and_then(|l| l.name.clone()).unwrap_or_default();
                return self.ask(super::ask::Ask {
                    title: "Lane name".to_owned(),
                    text: "What's on it: «Subtitles», «Place names»… Empty gives back its number.".to_owned(),
                    answers: vec![("Name it".to_owned(), Message::LaneName(track, lane, String::new()))],
                    cancel: Message::AskClosed,
                    field: Some(now),
                });
            }
            Message::LaneName(track, lane, name) => {
                self.op(Op::Item(ItemOp::LaneName { track, lane, name: Some(name) }));
            }
            Message::LaneHide(track, lane, hidden) => {
                self.op(Op::Item(ItemOp::LaneHidden { track, lane, hidden }));
            }
            Message::LaneActive(track, lane) => {
                self.active_lanes[track_index(track)] = lane;
                if self.selection.item(&self.edit).is_some() {
                    self.selection = Selection::None;
                }
            }
            Message::LaneRemove(track, lane) => {
                self.op(Op::Item(ItemOp::RemoveLane { track, lane }));
            }
            Message::OverlayToLane(r, at, lane) => {
                if let Some(item) = self.edit.item(r).map(|it| it.id) {
                    self.op(Op::Item(ItemOp::Move { item, at, lane: Some(lane) }));
                    self.active_lanes[track_index(r.0)] = lane;
                }
                self.edit.commit();
            }
            Message::OverlayMove(r, at) => {
                if let Some(item) = self.edit.item(r).map(|it| it.id) {
                    self.edit.begin();
                    self.op(Op::Item(ItemOp::Move { item, at, lane: None }));
                }
            }
            Message::OverlayTrim(r, start, t) => {
                if let Some(item) = self.edit.item(r).map(|it| it.id) {
                    self.edit.begin();
                    self.op(Op::Item(ItemOp::Trim { item, end: if start { End::In } else { End::Out }, t, most: ffmpeg::PICTURE_MOST }));
                }
            }
            Message::OverlayJump(at) => {
                self.follow = true;
                if let Some((i, t)) = self.edit.locate(at) {
                    self.goto(i, t, false);
                }
            }
            Message::OverlayKeyMove(r, effects, key, t) => {
                self.follow = true;
                use crate::model::ops::keys::{EXACT, Graded, KeyOp, Line};
                let keys = |it: &crate::overlay::Item| if effects { it.effects.get(key).map(|k| k.t) } else { it.motion.get(key).map(|k| k.t) };
                let Some((id, from)) = self.edit.item(r).and_then(|it| Some((it.id, keys(it)?))) else { return Task::none() };
                self.edit.begin();
                let line = if effects { Line::Effects(Graded::Item(id)) } else { Line::Motion(id) };
                self.op(Op::Key(KeyOp::Retime { line, from, t, gap: 1.0 / self.fps(), tolerance: EXACT }));
                let Some(item) = self.edit.item(r) else { return Task::none() };
                let local = keys(item);
                // Follow the mark, so what it holds shows as it moves.
                if let Some(at) = local.map(|t| item.at + t - item.start)
                    && let Some((i, t)) = self.edit.locate(at)
                {
                    self.scrub_to(i, t);
                }
            }
            Message::OverlayPicture(key, Ok((picture, missing))) => {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                if let Some(font) = missing {
                    self.error = Some(format!("The font {font} isn't on this computer; its text shows in Inter"));
                }
                self.overlay_thumbs.insert(key.clone(), overlay_thumb(&picture));
                self.overlay_pictures.insert(key, Some((NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), picture)));
            }
            Message::OverlayPicture(key, Err(e)) => {
                self.error = Some(format!("Couldn't read the overlay {}: {e}", key.trim_start_matches("file:")));
            }
            Message::AddText => {
                self.folds.text = false;
                let added = self.op(Op::Item(ItemOp::AddText { text: Default::default(), at: self.position(), len: ffmpeg::PICTURE_SECONDS, lane: self.adding_lane(crate::overlay::Track::Text) }));
                if let Some(id) = added.and_then(|o| o.made) {
                    self.selection = Selection::Item(id);
                }
                // Its words and look are set in the Effects panel, open now.
                self.text_of = None;
                self.side = Some(super::Side::Effects);
            }
            // Ten seconds of them, selected: their ratio, slide and colour
            // in the Effects panel.
            Message::AddBars => {
                self.folds.overlay = false;
                let added = self.op(Op::Item(ItemOp::AddBars { bars: Default::default(), at: self.position(), len: 10.0, lane: self.adding_lane(crate::overlay::Track::Overlay) }));
                if let Some(id) = added.and_then(|o| o.made) {
                    self.selection = Selection::Item(id);
                }
            }
            _ => {}
        }
        Task::none()
    }
}

/// `track`'s place in `App::active_lanes`.
pub(super) fn track_index(track: crate::overlay::Track) -> usize {
    match track {
        crate::overlay::Track::Overlay => 0,
        crate::overlay::Track::Text => 1,
    }
}
