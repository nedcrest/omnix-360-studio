//! The keyboard: what each key does (`App::key`), and which keys and
//! messages only framing uses, so editing can't turn the view by accident.

use super::*;

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
        Key::Named(Named::ArrowLeft | Named::ArrowRight) => mods.shift(),
        Key::Named(Named::ArrowUp | Named::ArrowDown | Named::Home) => true,
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
        let sound = self.selected_music.is_some() || self.selected_volume.is_some() || self.selected_sound.is_some();
        if !self.framing && framing_key(&key, mods, sound) {
            return Task::none();
        }
        match key.as_ref() {
            Key::Named(Named::Escape) if self.picking.is_some() => return self.update(Message::PickColour),
            Key::Named(Named::Escape) if self.fullscreen => return self.update(Message::Fullscreen),
            // As the Play button: from the end of the edit, from the start.
            Key::Named(Named::Space) => return self.update(Message::Toggle),
            Key::Named(Named::ArrowLeft) if mods.shift() => self.free_look().yaw -= 2.0 * step,
            Key::Named(Named::ArrowRight) if mods.shift() => self.free_look().yaw += 2.0 * step,
            Key::Named(arrow @ (Named::ArrowLeft | Named::ArrowRight)) => {
                self.follow = true;
                let t = self.position() + if arrow == Named::ArrowLeft { -5.0 } else { 5.0 };
                if let Some((i, t)) = self.edit.locate(t.max(0.0)) {
                    let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
                    self.goto(i, t, playing);
                }
            }
            // With the strip showing, a field typed in may want them.
            Key::Named(arrow @ (Named::ArrowUp | Named::ArrowDown)) if self.angles_shown => {
                return angles::arrow(if arrow == Named::ArrowUp { 1.0 } else { -1.0 });
            }
            Key::Named(Named::ArrowUp) => self.free_look().pitch += step,
            Key::Named(Named::ArrowDown) => self.free_look().pitch -= step,
            Key::Named(Named::Home) => self.free_look().reset(),
            Key::Named(Named::Delete | Named::Backspace) if mods.shift() || self.selected_music.is_some() || self.selected_volume.is_some() || self.selected_sound.is_some() || self.selected_overlay.is_some() => {
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
                // On a sound track: a volume point at the playhead, on its line.
                "k" | "K" if self.selected_music.is_some() || self.selected_volume.is_some() => {
                    let which = self.selected_music.or(self.selected_volume).unwrap().0;
                    let t = self.position();
                    // An undo step only if a point is added, not when one is
                    // just selected.
                    let mut i = 0;
                    self.edit.attempt(|e| {
                        let keys = &mut e.track_mut(which).keyframes;
                        let n = keys.len();
                        i = crate::project::volume_key_at(keys, t);
                        keys.len() > n
                    });
                    self.select_volume(which, i);
                }
                // A cut's sound point selected: another at the playhead.
                "k" | "K" if self.selected_sound.is_some() => {
                    let Some((_, t)) = self.playing_cut() else { return Task::none() };
                    let cut = self.current;
                    let mut i = 0;
                    self.edit.attempt(|e| {
                        let keys = &mut e.cuts[cut].volume;
                        let n = keys.len();
                        i = crate::project::volume_key_at(keys, t);
                        keys.len() > n
                    });
                    self.select_sound(cut, i);
                }
                "k" | "K" => return self.update(Message::KeyAdd),
                "j" | "J" => {
                    if let Some(k) = self.current_key() {
                        let ease = self.edit.cuts[self.current].keyframes[k].ease;
                        let i = Ease::ALL.iter().position(|&e| e == ease).unwrap_or(0);
                        return self.update(Message::KeyEase(Ease::ALL[(i + 1) % Ease::ALL.len()]));
                    }
                }
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
