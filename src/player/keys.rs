//! The keyboard: what each key does (`App::key`), and which keys and
//! messages only framing uses, so editing can't turn the view by accident.

use super::*;
use crate::model::ops::sound::SoundOp;

/// Messages that only framing does: turning, zooming or keying the view,
/// its angles, stabilisation, projection, colour tracking.
pub(super) fn framing_only(message: &Message) -> bool {
    matches!(
        message,
        Message::Look { .. }
            | Message::Zoom(_)
            | Message::Fov(_)
            | Message::ResetView
            | Message::KeyAdd
            | Message::KeyDelete
            | Message::KeyEase(_)
            | Message::KeyFollow(_)
            | Message::PickColour
            | Message::Stab(_)
            | Message::Projection(_)
            | Message::AngleInput(..)
            | Message::AngleNudge(..)
            | Message::AngleArrow(..)
            | Message::AngleScrubStart(_)
            | Message::AngleScrub(..)
    )
}

/// The key as the shortcuts know it, whatever the keyboard layout: a
/// letter, digit or sign that isn't Latin (Ukrainian «я» for Z, «с» for
/// C) is taken by the key it is on, as on a US keyboard. Ctrl+Z did
/// nothing with the Ukrainian layout on, nor did C, K, M… (the user,
/// 2026-11-03). Named keys (arrows, Del) never depended on it.
pub(super) fn latin(key: Key, physical: iced::keyboard::key::Physical, shift: bool) -> Key {
    use iced::keyboard::key::{Code, Physical};
    let Key::Character(c) = &key else { return key };
    if c.is_ascii() {
        return key;
    }
    let Physical::Code(code) = physical else { return key };
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let codes = [Code::KeyA, Code::KeyB, Code::KeyC, Code::KeyD, Code::KeyE, Code::KeyF, Code::KeyG, Code::KeyH, Code::KeyI, Code::KeyJ, Code::KeyK, Code::KeyL, Code::KeyM, Code::KeyN, Code::KeyO, Code::KeyP, Code::KeyQ, Code::KeyR, Code::KeyS, Code::KeyT, Code::KeyU, Code::KeyV, Code::KeyW, Code::KeyX, Code::KeyY, Code::KeyZ];
    let us = if let Some(i) = codes.iter().position(|&k| k == code) {
        let l = &letters[i..i + 1];
        if shift { l.to_uppercase() } else { l.to_owned() }
    } else {
        match (code, shift) {
            (Code::BracketLeft, false) => "[",
            (Code::BracketRight, false) => "]",
            (Code::Comma, false) => ",",
            (Code::Period, false) => ".",
            (Code::Equal, false) => "=",
            (Code::Equal, true) => "+",
            (Code::Minus, false) => "-",
            _ => return key,
        }
        .to_owned()
    };
    Key::Character(us.into())
}

/// Keys that only framing uses: looking around (Shift+←/→, ↑/↓, Q/E,
/// Home, 0, + and −), keyframes (K, J, [ and ]), stabilisation (S),
/// projection (P), rolling shutter (R). K with a sound selected (`sound`)
/// adds a volume point, which editing keeps.
pub(super) fn framing_key(key: &Key, mods: keyboard::Modifiers, sound: bool) -> bool {
    match key.as_ref() {
        // Alt+← / Alt+→ move the selected cut, an edit.
        Key::Named(Named::ArrowLeft | Named::ArrowRight) => mods.shift() && !mods.alt(),
        Key::Named(Named::ArrowUp | Named::ArrowDown | Named::Home) => true,
        // Alt+[ / Alt+] trim to the playhead, an edit.
        Key::Character("[" | "]") if mods.alt() => false,
        Key::Character(c) if !mods.command() => match c {
            "k" | "K" => !sound,
            "q" | "Q" | "e" | "E" | "0" | "+" | "=" | "-" | "j" | "J" | "[" | "]" | "s" | "S" | "p" | "P" | "r" | "R" => true,
            _ => false,
        },
        _ => false,
    }
}

/// How far a key turns or tilts the view, degrees.
const LOOK_STEP: f64 = 5.0;

impl App {
    /// A key pressed: a window open takes it first, then the editing
    /// keys, then the rest by name or letter.
    pub(super) fn key(&mut self, key: Key, mods: keyboard::Modifiers) -> Task<Message> {
        if let Some(task) = self.key_in_window(&key).or_else(|| self.key_editing(&key, mods)) {
            return task;
        }
        match key.as_ref() {
            Key::Character(c) => self.character_key(c, mods),
            named => self.named_key(named, mods),
        }
    }

    /// A dialog, window or menu open takes the keys: Esc closes it (or
    /// stops a run), the rest wait; F1 the shortcuts.
    fn key_in_window(&mut self, key: &Key) -> Option<Task<Message>> {
        let escape = matches!(key.as_ref(), Key::Named(Named::Escape));
        if self.asking.is_some() {
            return Some(if escape { self.cancel_ask() } else { Task::none() });
        }
        if self.normalize.is_some() {
            return Some(if escape { self.update(Message::Sound(sound_ui::SoundMsg::Cancel)) } else { Task::none() });
        }
        // The «New» window open: Esc closes it; its field takes the rest.
        if self.new_menu.is_some() {
            return Some(if escape { self.update(Message::NewMenuClose) } else { Task::none() });
        }
        // The Settings window open: Esc closes it, the rest wait.
        if self.settings_open.is_some() {
            return Some(if escape { self.update(Message::SettingsPanel) } else { Task::none() });
        }
        // A marker's window open: Esc closes it; its field takes the rest.
        if self.mark_edit.is_some() {
            return Some(if escape { self.update(Message::MarkClose) } else { Task::none() });
        }
        // The shortcuts' list open: Esc or F1 close it, the rest wait. Not
        // «?»: it never reached here on the user's keyboard (2026-11-04),
        // and F1 is help everywhere.
        let help = matches!(key.as_ref(), Key::Named(Named::F1));
        if self.shortcuts {
            return Some(if escape || help { self.update(Message::Shortcuts) } else { Task::none() });
        }
        if help {
            return Some(self.update(Message::Shortcuts));
        }
        if self.sound_menu.is_some() && escape {
            return Some(self.update(Message::Sound(sound_ui::SoundMsg::Menu(None))));
        }
        None
    }

    /// Editing (not framing): the arrows, Home and End move the playhead
    /// (gap 55), Alt+↑↓ an item a lane, and the framing keys do nothing.
    fn key_editing(&mut self, key: &Key, mods: keyboard::Modifiers) -> Option<Task<Message>> {
        // Editing: the keys that turn, frame or key the view do nothing. K
        // still adds a volume point to a selected sound.
        let sound = self.selection.sound(&self.edit).is_some();
        // Editing: the arrows, Home and End move the playhead (gap 55).
        if !self.framing && !mods.alt() && !mods.command() {
            match key.as_ref() {
                Key::Named(Named::ArrowLeft) if mods.shift() => return Some(self.seek_by(-1.0, "−1 s")),
                Key::Named(Named::ArrowRight) if mods.shift() => return Some(self.seek_by(1.0, "+1 s")),
                Key::Named(Named::ArrowUp) => return Some(self.go_to_join(-1.0)),
                Key::Named(Named::ArrowDown) => return Some(self.go_to_join(1.0)),
                Key::Named(Named::Home) => return Some(self.seek_to(0.0, "start")),
                Key::Named(Named::End) => return Some(self.seek_to(self.edit.frame(), "end")),
                _ => {}
            }
        }
        // The selected text or picture a lane up or down (gap 70).
        if let Key::Named(arrow @ (Named::ArrowUp | Named::ArrowDown)) = key.as_ref()
            && mods.alt()
            && self.selection.item(&self.edit).is_some()
        {
            self.item_to_lane(arrow == Named::ArrowUp);
            return Some(Task::none());
        }
        // Editing: the framing keys do nothing.
        (!self.framing && framing_key(key, mods, sound)).then(Task::none)
    }

    /// The named keys: Esc, Space, the arrows, Home, Delete.
    fn named_key(&mut self, key: Key<&str>, mods: keyboard::Modifiers) -> Task<Message> {
        match key {
        Key::Named(Named::Escape) if self.picking.is_some() => return self.update(Message::PickColour),
        // Esc cancels a drag: a change under way (a trim, a fade, a run
        // of typing, a look-around keyed by Auto keyframe) goes back to
        // before it, and the timeline drops its drag, a cut's move too
        // (it lands only on release). Without a drag it changes nothing
        // there. Then, as before, it leaves full screen.
        Key::Named(Named::Escape) => {
            if self.edit.changing() {
                self.edit.cancel();
                self.selection = self.selection.still_valid(&self.edit);
            } else {
                // A range marked on the ruler let go of.
                self.range = None;
            }
            self.drag_epoch += 1;
            self.snap_line = None;
            if self.fullscreen {
                return self.update(Message::Fullscreen);
            }
        }
        // As the Play button: from the end of the edit, from the start.
        // Shift+Space: the range marked on the ruler, once (gap 92).
        Key::Named(Named::Space) if mods.shift() && self.range.is_some() => return self.update(Message::PlayRange),
        Key::Named(Named::Space) => return self.update(Message::Toggle),
        // The selected cut one place left or right (a drag's key route).
        Key::Named(arrow @ (Named::ArrowLeft | Named::ArrowRight)) if mods.alt() => {
            let Some(i) = self.selection.cut(&self.edit) else { return Task::none() };
            let to = if arrow == Named::ArrowLeft { i.checked_sub(1) } else { Some(i + 2).filter(|&to| to <= self.edit.cuts.len()) };
            return to.map_or_else(Task::none, |to| self.update(Message::MoveCut(i, to)));
        }
        Key::Named(Named::ArrowLeft) if mods.shift() => self.free_look().yaw -= 2.0 * LOOK_STEP,
        Key::Named(Named::ArrowRight) if mods.shift() => self.free_look().yaw += 2.0 * LOOK_STEP,
        // A frame, as `,` and `.` (it was 5 s, thrown about unclearly:
        // gap 55).
        Key::Named(arrow @ (Named::ArrowLeft | Named::ArrowRight)) => {
            let back = arrow == Named::ArrowLeft;
            return self.step_frame(if back { -1.0 } else { 1.0 });
        }
        // With the strip showing, a field typed in may want them.
        Key::Named(arrow @ (Named::ArrowUp | Named::ArrowDown)) if self.angles_shown => {
            return angles::arrow(if arrow == Named::ArrowUp { 1.0 } else { -1.0 });
        }
        Key::Named(Named::ArrowUp) => self.free_look().pitch += LOOK_STEP,
        Key::Named(Named::ArrowDown) => self.free_look().pitch -= LOOK_STEP,
        Key::Named(Named::Home) => self.free_look().reset(),
        // A segment picked in the bin's player: Delete removes it.
        Key::Named(Named::Delete | Named::Backspace) if self.bin.view && self.bin.source.as_ref().is_some_and(|s| s.picked.is_some()) => {
            return self.update(Message::BinPlayer(super::bin_player::SourceMsg::Remove));
        }
        Key::Named(Named::Delete | Named::Backspace) if mods.shift() || !matches!(self.selection, Selection::None | Selection::Cut(_)) => {
            return self.update(Message::DeleteCut);
        }
        // Editing: Del takes the selected cut (framing, the keyframe).
        Key::Named(Named::Delete | Named::Backspace) if !self.framing => return self.update(Message::DeleteCut),
        Key::Named(Named::Delete | Named::Backspace) => return self.update(Message::KeyDelete),
            _ => {}
        }
        Task::none()
    }

    /// The letter and sign keys.
    fn character_key(&mut self, c: &str, mods: keyboard::Modifiers) -> Task<Message> {
        match c {
            // In and out points in the bin's player (Alt clears them).
            "i" | "I" if self.bin.view && self.bin.source.is_some() && !mods.command() => return self.update(Message::BinPlayer(super::bin_player::SourceMsg::In(!mods.alt()))),
            "o" | "O" if self.bin.view && self.bin.source.is_some() && !mods.command() => return self.update(Message::BinPlayer(super::bin_player::SourceMsg::Out(!mods.alt()))),
            "o" | "O" if mods.command() && mods.shift() => return self.update(Message::OpenProject),
            "n" | "N" if mods.command() => return self.update(Message::NewMenu),
            "o" if mods.command() => return self.update(Message::Open),
            "s" | "S" if mods.command() => return self.update(Message::SaveProject(mods.shift())),
            "z" | "Z" if mods.command() && mods.shift() => return self.update(Message::Redo),
            "z" if mods.command() => return self.update(Message::Undo),
            "y" if mods.command() => return self.update(Message::Redo),
            "e" | "E" if mods.command() => return self.update(Message::ExportPanel),
            "," if mods.command() => return self.update(Message::SettingsPanel),
            "c" | "C" => return self.update(Message::Split),
            "t" | "T" => return self.update(Message::TransitionsPanel),
            "b" | "B" if !mods.command() && mods.shift() => return self.update(Message::BinView(super::bin_view::BinMsg::Toggle)),
            "b" | "B" if !mods.command() => return self.update(Message::BinPanel),
            "q" | "Q" => self.free_look().roll -= LOOK_STEP,
            "e" | "E" => self.free_look().roll += LOOK_STEP,
            "=" | "+" if mods.command() => return self.update(Message::TimelineZoom(1.5, None)),
            "-" if mods.command() => return self.update(Message::TimelineZoom(1.0 / 1.5, None)),
            "0" if mods.command() => return self.update(Message::TimelineZoomTo(0.0)),
            "0" => self.free_look().reset(),
            // A sound clip or a point selected: a volume point at the
            // playhead on that clip's own line (within it), or the
            // playing cut's.
            "k" | "K" if self.selection.sound(&self.edit).is_some() => self.key_volume_point(),
            "k" | "K" => return self.update(Message::KeyAdd),
            "j" | "J" => {
                if let Some(k) = self.current_key() {
                    let ease = self.edit.cuts[self.current].keyframes[k].ease;
                    let i = Ease::ALL.iter().position(|&e| e == ease).unwrap_or(0);
                    return self.update(Message::KeyEase(Ease::ALL[(i + 1) % Ease::ALL.len()]));
                }
            }
            "[" if mods.alt() => return self.update(Message::TrimToPlayhead(true)),
            "]" if mods.alt() => return self.update(Message::TrimToPlayhead(false)),
            "[" => return self.key_step(-1.0),
            "]" => return self.key_step(1.0),
            "," | "." => return self.step_frame(if c == "," { -1.0 } else { 1.0 }),
            "+" | "=" => self.free_look().zoom_by(-1.0),
            "-" => self.free_look().zoom_by(1.0),
            "s" | "S" => {
                let mode = self.clip.as_ref().map_or(self.opts.stabilize, |c| c.playback.media.stab.mode());
                let next = match mode {
                    Mode::Off => Mode::Horizon,
                    Mode::Horizon => Mode::Level,
                    Mode::Level => Mode::Lock,
                    Mode::Lock => Mode::Off,
                };
                return self.update(Message::Stab(next));
            }
            "g" | "G" => return self.effect_message(effects::EffectMsg::Toggle),
            "r" | "R" => return self.update(Message::RollingShutter(!self.rolling_shutter)),
            // D selects the cut under the playhead (Premiere's key): ↓ to a
            // cut, D, then Alt+← moves it (gap 58).
            "d" | "D" if !mods.command() => return self.update(Message::SelectAtPlayhead),
            // N turns snapping on and off, as in Final Cut.
            "n" | "N" if !mods.command() => return self.update(Message::Snapping),
            // M marks, as in FCP, Premiere and Resolve (gap 5); mute
            // moved to Shift+M, out of the way of a slip.
            "m" | "M" if mods.shift() => return self.update(Message::Mute),
            "m" | "M" => return self.update(Message::AddMark),
            "p" | "P" => {
                let i = Projection::ALL.iter().position(|&p| p == self.shown_view().projection).unwrap_or(0);
                return self.update(Message::Projection(Projection::ALL[(i + 1) % Projection::ALL.len()]));
            }
            "f" | "F" => return self.update(Message::Fullscreen),
            _ => {}
        }
        Task::none()
    }

    /// K on a sound clip or point selected: a volume point at the playhead
    /// on that sound's own line (within it), the selected cut's rather
    /// than the playing one's.
    fn key_volume_point(&mut self) {
            let own = self.selection.sound(&self.edit);
            // The playhead in that sound's file time: the selected
            // cut's, not the playing one's (K used to key that).
            let at = match own {
                Some(own @ Own::Cut(n)) => self.edit.cuts.get(n).map(|c| (own, c.start + self.position() - self.edit.offset(n))),
                Some(own @ Own::Clip(which, c)) => self.edit.track(which).clips.get(c).map(|m| (own, m.start + self.position() - m.at)),
                None => None,
            };
            let Some((own, t)) = at.filter(|&(own, t)| self.edit.own_line(own).is_some_and(|(_, (s, e))| t >= s && t <= e)) else { return };
            // No undo step when one is there already. A clip selected
            // stays selected (K again keys it again); a point
            // selected moves to the one keyed.
            self.sound_op(own, |sound| SoundOp::PointAt { sound, t });
            if self.selection.point(&self.edit).is_some() {
                self.select_point_at(own, t);
            }
    }
}

impl App {
    /// A frame back (`frames` −1) or on, along the edit: within the cut, a
    /// step of its playback; past its in or out point, the cut before or
    /// after (a playback step went on into the trimmed-off footage).
    fn step_frame(&mut self, frames: f64) -> Task<Message> {
        self.follow = true;
        let note = if frames < 0.0 { "−1 f" } else { "+1 f" };
        let t = self.aimed() + frames / self.fps();
        if self.edit.locate(t.max(0.0)).is_some_and(|(i, _)| i == self.current) {
            self.with_playback(|p| p.step(frames));
            // The full frame once the steps rest (only the quick one meanwhile).
            self.steps += 1;
            let n = self.steps;
            let rested = Task::perform(super::blocking(|| std::thread::sleep(std::time::Duration::from_millis(250))), move |_| Message::StepsRested(n));
            return Task::batch([self.step_note(note), rested]);
        }
        self.seek_to(t, note)
    }

    /// The playhead `dt` seconds on, said by it as `note` for a moment.
    fn seek_by(&mut self, dt: f64, note: &'static str) -> Task<Message> {
        self.seek_to(self.aimed() + dt, note)
    }

    /// Where the playhead is headed on the edit: the frame asked for last,
    /// which may still be decoding, so quick presses add up instead of each
    /// starting again from the frame on screen.
    fn aimed(&self) -> f64 {
        self.playing_cut().map_or_else(|| self.position(), |(_, t)| self.edit.edit_time(self.current, t))
    }

    /// The playhead to edit time `t` (held within the edit), said by it.
    fn seek_to(&mut self, t: f64, note: &'static str) -> Task<Message> {
        self.follow = true;
        // The edit's end shows its last frame.
        let t = t.clamp(0.0, (self.edit.duration() - self.frame_len()).max(0.0));
        if let Some((i, t)) = self.edit.locate(t) {
            let playing = self.playing();
            self.goto(i, t, playing);
        }
        self.step_note(note)
    }

    /// The playhead to the previous (`dir` < 0) or next stop: a marker, a
    /// cut's edge, the edit's ends. Said by the playhead as what it is
    /// («← marker»).
    pub(super) fn go_to_join(&mut self, dir: f64) -> Task<Message> {
        match next_stop(&stops(&self.edit), self.aimed(), dir, self.frame_len()) {
            Some((t, stop)) => {
                let note = match (stop, dir < 0.0) {
                    (Stop::Marker, true) => "← marker",
                    (Stop::Marker, false) => "marker →",
                    (Stop::Cut, true) => "← cut",
                    (Stop::Cut, false) => "cut →",
                    (Stop::Edge, true) => "start",
                    (Stop::Edge, false) => "end",
                };
                self.seek_to(t, note)
            }
            None => Task::none(),
        }
    }

    /// Says `note` by the playhead for a moment (gap 55).
    pub(super) fn step_note(&mut self, note: &'static str) -> Task<Message> {
        let n = self.step_note.map_or(0, |(_, n)| n + 1);
        self.step_note = Some((note, n));
        Task::perform(super::blocking(|| std::thread::sleep(std::time::Duration::from_millis(900))), move |_| Message::StepNoteGone(n))
    }
}

/// What the playhead stops at with ↑ / ↓; where several meet, the first
/// in this order names the stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Stop {
    Marker,
    Cut,
    Edge,
}

/// Where ↑ / ↓ stop: the markers (shown ones), the cuts' joins, the
/// edit's start and end. Not the sound clips' or overlays' edges: the
/// user found the playhead caught mid-cut by a sound effect's end, with
/// nothing there to see on the picture (2026-11-03).
fn stops(edit: &crate::model::edit::Edit) -> Vec<(f64, Stop)> {
    let offsets = crate::project::offsets(&edit.cuts);
    let mut all = vec![(0.0, Stop::Edge), (edit.duration(), Stop::Edge)];
    for (c, &at) in edit.cuts.iter().zip(&offsets) {
        all.extend([(at, Stop::Cut), (at + c.len(), Stop::Cut)]);
        all.extend(c.marks.iter().filter(|m| m.t >= c.start && m.t <= c.end).map(|m| (at + m.t - c.start, Stop::Marker)));
    }
    all
}

/// The stop before `now` (`dir` < 0) or after it, more than half a
/// `frame` away: the playhead lands on the frame nearest a stop, which can
/// be a hair past it, and that stop counted as the next again, so ↑ stood
/// still (the user, 2026-11-03). Several within half a frame are one, named
/// by the first kind.
fn next_stop(stops: &[(f64, Stop)], now: f64, dir: f64, frame: f64) -> Option<(f64, Stop)> {
    let gap = frame / 2.0 + 1e-6;
    let ahead = |t: f64| if dir < 0.0 { t < now - gap } else { t > now + gap };
    let nearest = stops.iter().map(|s| s.0).filter(|&t| ahead(t)).min_by(|a, b| (a - now).abs().total_cmp(&(b - now).abs()))?;
    stops.iter().filter(|s| (s.0 - nearest).abs() <= gap && ahead(s.0)).min_by_key(|s| s.1).map(|&(_, stop)| (nearest, stop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Cut, MusicClip};

    #[test]
    fn a_window_open_takes_the_keys_first() {
        let mut app = super::super::tests::app();
        let none = keyboard::Modifiers::empty();
        let snapping = app.snapping;
        app.shortcuts = true;
        let _ = app.key(Key::Character("n".into()), none);
        assert_eq!(app.snapping, snapping, "the list open: N waits");
        let _ = app.key(Key::Named(Named::F1), none);
        let _ = app.key(Key::Character("n".into()), none);
        assert!(!app.shortcuts && app.snapping != snapping, "closed by F1, then N snaps");
    }

    #[test]
    fn stops_are_every_edge_and_marker_on_the_edit() {
        let mut e = crate::model::edit::Edit::default();
        e.cuts = vec![Cut::new("a".into(), 0.0, 4.0), Cut { crossfade: 1.0, ..Cut::new("b".into(), 0.0, 3.0) }];
        e.music.clips.push(MusicClip { input: "s".into(), at: 2.5, start: 0.0, end: 1.0, ..Default::default() });
        // A marker in b at its clip time 1 s (edit 4 s), and one past its out
        // point, not shown, not a stop.
        e.cuts[1].marks = vec![crate::project::Mark { t: 1.5, note: String::new() }, crate::project::Mark { t: 9.0, note: String::new() }];
        let mut t: Vec<f64> = stops(&e).into_iter().map(|s| s.0).collect();
        t.sort_by(f64::total_cmp);
        t.dedup();
        // a 0..4, b overlapping it from 3 to 6, b's marker; not the song.
        assert_eq!(t, [0.0, 3.0, 4.0, 4.5, 6.0]);
    }

    #[test]
    fn a_ukrainian_layout_s_keys_are_taken_by_where_they_sit() {
        use iced::keyboard::key::{Code, Physical};
        let k = |c: &str, code: Code, shift: bool| latin(Key::Character(c.into()), Physical::Code(code), shift);
        assert_eq!(k("я", Code::KeyZ, false), Key::Character("z".into()));
        assert_eq!(k("С", Code::KeyC, true), Key::Character("C".into()));
        assert_eq!(k("х", Code::BracketLeft, false), Key::Character("[".into()));
        assert_eq!(k("ю", Code::Period, false), Key::Character(".".into()));
        assert_eq!(k("z", Code::KeyZ, false), Key::Character("z".into()), "Latin as it is");
    }

    /// ↑ from the end, the playhead landing on the nearest frame each time
    /// as the player's does, visits every stop once, back to the start, and
    /// ↓ every one on to the end: none stands still (the user found ↑ stuck
    /// at a marker and a sound's end that weren't on a frame).
    #[test]
    fn up_and_down_visit_every_stop_and_never_stand_still() {
        let mut e = crate::model::edit::Edit::default();
        e.cuts = vec![Cut::new("a".into(), 0.0, 5.7), Cut::new("b".into(), 2.4, 8.4), Cut::new("c".into(), 0.0, 15.0)];
        e.cuts[1].marks = vec![crate::project::Mark { t: 3.7779, note: String::new() }, crate::project::Mark { t: 7.0123, note: "the end of the talk".into() }];
        e.cuts[0].marks = vec![crate::project::Mark { t: 4.1, note: String::new() }];
        e.sfx.clips.push(MusicClip { input: "bell".into(), at: 5.7, start: 0.0, end: 1.594, ..Default::default() });
        let frame = 1.0 / 30.0;
        let snap = |t: f64| (t / frame).round() * frame;
        let all: Vec<(f64, Stop)> = stops(&e);
        let mut distinct: Vec<f64> = all.iter().map(|s| s.0).collect();
        distinct.sort_by(f64::total_cmp);
        distinct.dedup_by(|a, b| (*a - *b).abs() <= frame / 2.0);
        let walk = |dir: f64, from: f64| {
            let (mut now, mut seen) = (from, Vec::new());
            while let Some((t, _)) = next_stop(&all, now, dir, frame) {
                seen.push(t);
                now = snap(t);
                assert!(seen.len() <= distinct.len(), "stuck going {dir}: {seen:?}");
            }
            seen
        };
        let up = walk(-1.0, snap(e.duration()));
        let down = walk(1.0, 0.0);
        assert_eq!(up.len(), distinct.len() - 1, "every stop but the end, from the end: {up:?}");
        assert_eq!(down.len(), distinct.len() - 1, "every stop but the start: {down:?}");
        assert!(up.windows(2).all(|w| w[0] > w[1]) && down.windows(2).all(|w| w[0] < w[1]));
        // Named by what they are; a sound effect's end is no stop.
        assert_eq!(next_stop(&all, snap(11.0), -1.0, frame).map(|s| s.1), Some(Stop::Marker), "b's 7.0123 s is the edit's 10.31 s");
        assert_eq!(next_stop(&all, snap(7.5), -1.0, frame), Some((5.7 + 3.7779 - 2.4, Stop::Marker)), "past the bell's end at 7.294 s");
    }
}
