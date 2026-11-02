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

impl App {
    pub(super) fn key(&mut self, key: Key, mods: keyboard::Modifiers) -> Task<Message> {
        let step = 5.0;
        // A dialog or a menu open takes the keys: Esc closes it (or stops a
        // run), the rest wait.
        let escape = matches!(key.as_ref(), Key::Named(Named::Escape));
        if self.normalize.is_some() {
            return if escape { self.update(Message::Sound(sound_ui::SoundMsg::Cancel)) } else { Task::none() };
        }
        if self.sound_menu.is_some() && escape {
            return self.update(Message::Sound(sound_ui::SoundMsg::Menu(None)));
        }
        // Editing: the keys that turn, frame or key the view do nothing. K
        // still adds a volume point to a selected sound.
        let sound = self.selection.sound(&self.edit).is_some();
        // Editing: the arrows, Home and End move the playhead (gap 55).
        if !self.framing && !mods.alt() && !mods.command() {
            match key.as_ref() {
                Key::Named(Named::ArrowLeft) if mods.shift() => return self.seek_by(-1.0, "−1 s"),
                Key::Named(Named::ArrowRight) if mods.shift() => return self.seek_by(1.0, "+1 s"),
                // A field of the angles strip being typed in keeps them.
                Key::Named(arrow @ (Named::ArrowUp | Named::ArrowDown)) if self.angles_shown => return angles::arrow(if arrow == Named::ArrowUp { 1.0 } else { -1.0 }),
                Key::Named(Named::ArrowUp) => return self.to_join(-1.0),
                Key::Named(Named::ArrowDown) => return self.to_join(1.0),
                Key::Named(Named::Home) => return self.seek_to(0.0, "start"),
                Key::Named(Named::End) => return self.seek_to(self.edit.frame(), "end"),
                _ => {}
            }
        }
        if !self.framing && framing_key(&key, mods, sound) {
            return Task::none();
        }
        match key.as_ref() {
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
                }
                self.drag_epoch += 1;
                if self.fullscreen {
                    return self.update(Message::Fullscreen);
                }
            }
            // As the Play button: from the end of the edit, from the start.
            Key::Named(Named::Space) => return self.update(Message::Toggle),
            // The selected cut one place left or right (a drag's key route).
            Key::Named(arrow @ (Named::ArrowLeft | Named::ArrowRight)) if mods.alt() => {
                let Some(i) = self.selection.cut(&self.edit) else { return Task::none() };
                let to = if arrow == Named::ArrowLeft { i.checked_sub(1) } else { Some(i + 2).filter(|&to| to <= self.edit.cuts.len()) };
                return to.map_or_else(Task::none, |to| self.update(Message::MoveCut(i, to)));
            }
            Key::Named(Named::ArrowLeft) if mods.shift() => self.free_look().yaw -= 2.0 * step,
            Key::Named(Named::ArrowRight) if mods.shift() => self.free_look().yaw += 2.0 * step,
            // A frame, as `,` and `.` (it was 5 s, thrown about unclearly:
            // gap 55).
            Key::Named(arrow @ (Named::ArrowLeft | Named::ArrowRight)) => {
                self.follow = true;
                let back = arrow == Named::ArrowLeft;
                self.with_playback(|p| p.step(if back { -1.0 } else { 1.0 }));
                return self.step_note(if back { "−1 f" } else { "+1 f" });
            }
            // With the strip showing, a field typed in may want them.
            Key::Named(arrow @ (Named::ArrowUp | Named::ArrowDown)) if self.angles_shown => {
                return angles::arrow(if arrow == Named::ArrowUp { 1.0 } else { -1.0 });
            }
            Key::Named(Named::ArrowUp) => self.free_look().pitch += step,
            Key::Named(Named::ArrowDown) => self.free_look().pitch -= step,
            Key::Named(Named::Home) => self.free_look().reset(),
            Key::Named(Named::Delete | Named::Backspace) if mods.shift() || !matches!(self.selection, Selection::None | Selection::Cut(_)) => {
                return self.update(Message::DeleteCut);
            }
            // Editing: Del takes the selected cut (framing, the keyframe).
            Key::Named(Named::Delete | Named::Backspace) if !self.framing => return self.update(Message::DeleteCut),
            Key::Named(Named::Delete | Named::Backspace) => return self.update(Message::KeyDelete),
            Key::Character(c) => match c {
                "o" | "O" if mods.command() && mods.shift() => return self.update(Message::OpenProject),
                "n" | "N" if mods.command() => return self.update(Message::NewProject),
                "o" if mods.command() => return self.update(Message::Open),
                "s" | "S" if mods.command() => return self.update(Message::SaveProject(mods.shift())),
                "z" | "Z" if mods.command() && mods.shift() => return self.update(Message::Redo),
                "z" if mods.command() => return self.update(Message::Undo),
                "y" if mods.command() => return self.update(Message::Redo),
                "e" | "E" if mods.command() => return self.update(Message::ExportPanel),
                "," if mods.command() => return self.update(Message::SettingsPanel),
                "c" | "C" => return self.update(Message::Split),
                "b" | "B" if !mods.command() => return self.update(Message::BinPanel),
                "q" | "Q" => self.free_look().roll -= step,
                "e" | "E" => self.free_look().roll += step,
                "=" | "+" if mods.command() => return self.update(Message::TimelineZoom(1.5, None)),
                "-" if mods.command() => return self.update(Message::TimelineZoom(1.0 / 1.5, None)),
                "0" if mods.command() => return self.update(Message::TimelineZoomTo(0.0)),
                "0" => self.free_look().reset(),
                // A sound clip or a point selected: a volume point at the
                // playhead on that clip's own line (within it), or the
                // playing cut's.
                "k" | "K" if self.selection.sound(&self.edit).is_some() => {
                    let own = self.selection.sound(&self.edit);
                    // The playhead in that sound's file time: the selected
                    // cut's, not the playing one's (K used to key that).
                    let at = match own {
                        Some(own @ Own::Cut(n)) => self.edit.cuts.get(n).map(|c| (own, c.start + self.position() - self.edit.offset(n))),
                        Some(own @ Own::Clip(which, c)) => self.edit.track(which).clips.get(c).map(|m| (own, m.start + self.position() - m.at)),
                        None => None,
                    };
                    let Some((own, t)) = at.filter(|&(own, t)| self.edit.own_line(own).is_some_and(|(_, (s, e))| t >= s && t <= e)) else { return Task::none() };
                    // No undo step when one is there already: just selected.
                    self.sound_op(own, |sound| SoundOp::PointAt { sound, t });
                    self.select_point_at(own, t);
                }
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
                "," | "." => {
                    self.follow = true;
                    let frames = if c == "," { -1.0 } else { 1.0 };
                    self.with_playback(|p| p.step(frames));
                }
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
                "g" | "G" => self.effect_message(effects::EffectMsg::Toggle),
                "r" | "R" => return self.update(Message::RollingShutter(!self.rolling_shutter)),
                "m" | "M" => return self.update(Message::Mute),
                "p" | "P" => {
                    let i = Projection::ALL.iter().position(|&p| p == self.shown_view().projection).unwrap_or(0);
                    return self.update(Message::Projection(Projection::ALL[(i + 1) % Projection::ALL.len()]));
                }
                "f" | "F" => return self.update(Message::Fullscreen),
                _ => {}
            },
            _ => {}
        }
        Task::none()
    }
}

impl App {
    /// The playhead `dt` seconds on, said by it as `note` for a moment.
    fn seek_by(&mut self, dt: f64, note: &'static str) -> Task<Message> {
        self.seek_to(self.position() + dt, note)
    }

    /// The playhead to edit time `t` (held within the edit), said by it.
    fn seek_to(&mut self, t: f64, note: &'static str) -> Task<Message> {
        self.follow = true;
        // The edit's end shows its last frame.
        let t = t.clamp(0.0, (self.edit.duration() - 1.0 / self.fps()).max(0.0));
        if let Some((i, t)) = self.edit.locate(t) {
            let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
            self.goto(i, t, playing);
        }
        self.step_note(note)
    }

    /// The playhead to the previous (`dir` < 0) or next join: where any
    /// cut, sound clip or overlay starts or ends, and the edit's ends.
    pub(super) fn to_join(&mut self, dir: f64) -> Task<Message> {
        let now = self.position();
        let t = joins(&self.edit).into_iter().filter(|&t| if dir < 0.0 { t < now - 1e-3 } else { t > now + 1e-3 }).min_by(|a, b| (a - now).abs().total_cmp(&(b - now).abs()));
        match t {
            Some(t) => self.seek_to(t, if dir < 0.0 { "← join" } else { "join →" }),
            None => Task::none(),
        }
    }

    /// Says `note` by the playhead for a moment (gap 55).
    fn step_note(&mut self, note: &'static str) -> Task<Message> {
        let n = self.step_note.map_or(0, |(_, n)| n + 1);
        self.step_note = Some((note, n));
        Task::perform(super::blocking(|| std::thread::sleep(std::time::Duration::from_millis(900))), move |_| Message::StepNoteGone(n))
    }
}

/// Every edit time where something starts or ends: the cuts' joins, the
/// sound clips' and overlay items' edges, the edit's start and end.
fn joins(edit: &crate::model::edit::Edit) -> Vec<f64> {
    let offsets = crate::project::offsets(&edit.cuts);
    let cuts = edit.cuts.iter().zip(&offsets).flat_map(|(c, &at)| [at, at + c.len()]);
    let sounds = crate::project::SoundTrack::ALL.into_iter().flat_map(|w| edit.track(w).clips.iter().flat_map(|c| [c.at, c.ends()]).collect::<Vec<_>>());
    let items = edit.lanes().flat_map(|l| &l.items).flat_map(|it| [it.at, it.ends()]);
    std::iter::once(0.0).chain(cuts).chain(sounds).chain(items).chain(std::iter::once(edit.duration())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{Cut, MusicClip};

    #[test]
    fn joins_are_every_edge_on_the_edit() {
        let mut e = crate::model::edit::Edit::default();
        e.cuts = vec![Cut::new("a".into(), 0.0, 4.0), Cut { crossfade: 1.0, ..Cut::new("b".into(), 0.0, 3.0) }];
        e.music.clips.push(MusicClip { input: "s".into(), at: 2.5, start: 0.0, end: 1.0, ..Default::default() });
        let mut t = joins(&e);
        t.sort_by(f64::total_cmp);
        t.dedup();
        // a 0..4, b overlapping it from 3 to 6, the song 2.5..3.5.
        assert_eq!(t, [0.0, 2.5, 3.0, 3.5, 4.0, 6.0]);
    }
}
