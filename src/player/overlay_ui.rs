//! Overlays and text in the player: their pictures read or drawn for the
//! view, the text panel's words kept in step, what draws over the picture,
//! the on-screen handles, and their blocks on the timeline.

use super::*;
use crate::model::ops::items::ItemOp;
use crate::model::ops::{End, Op};

/// A small picture of an overlay for its timeline block: 48 px high,
/// over a dark checker so transparency shows.
pub(super) fn overlay_thumb(picture: &crate::overlay::Picture) -> iced::widget::image::Handle {
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
    iced::widget::image::Handle::from_rgba(w as u32, h as u32, rgba)
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
        let words = self.selection.item().and_then(|r| self.edit.item(r)?.text.as_ref().map(|t| (r, t.content.clone())));
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
            .lanes()
            .filter_map(|lane| lane.at(t).map(|(_, it)| it))
            .filter_map(|item| {
                let (id, picture) = self.overlay_pictures.get(&item.key())?.clone()?;
                let (place, grade) = crate::overlay::look(item, t);
                Some(gpu::OverlayDraw { id, picture, place, grade, black })
            })
            .collect()
    }

    /// The overlays showing at the playhead for the view's handles, while
    /// paused (playing, a drag would fight the motion keyframes).
    pub(super) fn overlay_handles(&self) -> handles::Handles {
        let t = self.position();
        let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
        let items = [crate::overlay::Track::Overlay, crate::overlay::Track::Text]
            .into_iter()
            .filter(|_| !playing)
            .filter_map(|track| {
                let (i, item) = self.edit.lane(track)?.at(t)?;
                let (_, picture) = self.overlay_pictures.get(&item.key())?.clone()?;
                Some(((track, i), crate::overlay::look(item, t).0, picture))
            })
            .collect();
        handles::Handles { items, selected: self.selection.item() }
    }

    /// The timeline's blocks for `track`'s lane.
    pub(super) fn overlay_blocks(&self, track: crate::overlay::Track) -> Vec<timeline::OverlayBlock> {
        self.edit.lane(track).map_or_else(Vec::new, |lane| {
            lane.items
                .iter()
                .map(|it| timeline::OverlayBlock {
                    label: it.name(),
                    from: it.at,
                    start: it.start,
                    len: it.end - it.start,
                    effects: it.effects.iter().map(|k| k.t).collect(),
                    motion: it.motion.iter().map(|k| (k.t, k.ease)).collect(),
                    thumb: self.overlay_thumbs.get(&it.key()).cloned(),
                })
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
                let added = self.op(Op::Item(ItemOp::AddPictures { inputs: paths, at: self.position(), len: ffmpeg::PICTURE_SECONDS }));
                if let Some(r) = added.and_then(|o| o.made).and_then(|id| self.item_ref(id)) {
                    self.selection = Selection::Item(r);
                }
            }
            Message::OverlayPicked(_) => {}
            Message::OverlaySelect(i) => {
                // Typing in another overlay is a change of its own for undo.
                self.edit.commit();
                match i {
                    Some(r) => self.selection = Selection::Item(r),
                    // An empty spot on its lane: only an item lets go.
                    None if self.selection.item().is_some() => self.selection = Selection::None,
                    None => {}
                }
            }
            Message::OverlayMove((track, i), at) => {
                if let Some(item) = self.edit.item((track, i)).map(|it| it.id) {
                    self.edit.begin();
                    self.op(Op::Item(ItemOp::Move { item, at }));
                }
            }
            Message::OverlayTrim((track, i), start, t) => {
                if let Some(item) = self.edit.item((track, i)).map(|it| it.id) {
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
                use crate::model::ops::keys::{Graded, KeyOp, Line};
                let keys = |it: &crate::overlay::Item| if effects { it.effects.get(key).map(|k| k.t) } else { it.motion.get(key).map(|k| k.t) };
                let Some((id, from)) = self.edit.item(r).and_then(|it| Some((it.id, keys(it)?))) else { return Task::none() };
                self.edit.begin();
                let line = if effects { Line::Effects(Graded::Item(id)) } else { Line::Motion(id) };
                self.op(Op::Key(KeyOp::Retime { line, from, t, gap: 1.0 / self.fps(), tolerance: 1e-6 }));
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
                let added = self.op(Op::Item(ItemOp::AddText { text: Default::default(), at: self.position(), len: ffmpeg::PICTURE_SECONDS }));
                if let Some(r) = added.and_then(|o| o.made).and_then(|id| self.item_ref(id)) {
                    self.selection = Selection::Item(r);
                }
                // Its words and look are set in the text panel, open now.
                self.text_of = None;
            }
            _ => {}
        }
        Task::none()
    }
}
