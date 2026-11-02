//! The view: what is shown (following the cut's keyframes, or looked at by
//! hand), keying it, the free look per kind of clip, following a stick or
//! a colour, and the LUTs a cut is shown through.

use super::*;
use crate::model::ops::Op;
use crate::model::ops::keys::{EXACT, KeyOp, Line};

/// The Follow picker's choices for a keyframe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aiming {
    Fixed,
    Stick,
    /// Any colour track: choosing it on a keyframe that follows none draws
    /// a box to pick one.
    Colour,
}

impl Aiming {
    pub(super) const ALL: [Aiming; 3] = [Aiming::Fixed, Aiming::Stick, Aiming::Colour];

    pub(super) fn of(follow: &Option<Follow>) -> Aiming {
        match follow {
            None => Aiming::Fixed,
            Some(Follow::Stick) => Aiming::Stick,
            Some(Follow::Track(_)) => Aiming::Colour,
        }
    }
}

impl std::fmt::Display for Aiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Aiming::Fixed => f.write_str("Fixed"),
            Aiming::Stick => f.write_str("Stick"),
            Aiming::Colour => f.write_str("Colour track"),
        }
    }
}

impl App {
    /// Half a frame: how near the playhead must be to sit on a keyframe.
    pub(super) fn tolerance(&self) -> f64 {
        self.clip.as_ref().map_or(0.0, |c| 0.5 / c.playback.media.fps)
    }

    /// The keyframe at the shown frame, if there is one.
    /// The playing cut's id and the time of its view key under the
    /// playhead: what a key op names.
    fn current_key_time(&self) -> Option<(crate::project::Id, f64)> {
        let k = self.current_key()?;
        let cut = &self.edit.cuts[self.current];
        Some((cut.id, cut.keyframes[k].t))
    }

    pub(super) fn current_key(&self) -> Option<usize> {
        let (cut, t) = self.playing_cut()?;
        cut.keyframes.near(t, self.tolerance())
    }

    /// The keyframes framing the playing cut at its clip time `t`, as
    /// `export` frames it: a cut without any holds the edit's last view.
    pub(super) fn keyframed(&self, t: f64) -> Option<(&Keyframes, f64)> {
        crate::project::keyframed(&self.edit.cuts, self.current, t)
    }

    /// Where `follow` points in the playing clip's view at its time `t`.
    pub(super) fn aim(&self, follow: &Follow, t: f64) -> Option<(f64, f64)> {
        self.aim_in(&self.clip.as_ref()?.playback.media, follow, t)
    }

    /// Where `follow` points in `media`'s clip at `t`.
    pub(super) fn aim_in(&self, media: &Media, follow: &Follow, t: f64) -> Option<(f64, f64)> {
        match follow {
            Follow::Stick => media.stab.stick(t * 1000.0, media.readout_ms),
            Follow::Track(path) => {
                let track = self
                    .followed
                    .borrow_mut()
                    .entry(path.clone())
                    .or_insert_with(|| crate::track::Track::load(path).inspect_err(|e| eprintln!("{e:#}")).ok().map(Arc::new))
                    .clone()?;
                track.aim(&media.stab, t, media.readout_ms)
            }
        }
    }

    /// Where everything a keyframe could follow points at clip time `t`
    /// (the stick and the edit's tracks), for keyframe edits, which can't
    /// look while the edit is borrowed.
    pub(super) fn aims(&self, t: f64) -> Vec<(Follow, Option<(f64, f64)>)> {
        let mut follows = vec![Follow::Stick];
        for f in self.edit.cuts.iter().flat_map(|c| c.keyframes.iter()).filter_map(|k| k.follow.clone()) {
            if !follows.contains(&f) {
                follows.push(f);
            }
        }
        follows.into_iter().map(|f| (f.clone(), self.aim(&f, t))).collect()
    }

    /// The view on screen, as angles.
    pub(super) fn shown_view(&self) -> View {
        match self.playing_cut() {
            Some((_, t)) if self.follow => self.keyframed(t).and_then(|(k, at)| k.view_at(at, &crate::project::held(&|f, t| self.aim(f, t), at == t, t))).unwrap_or_else(|| self.unkeyed_view()),
            _ => self.look,
        }
    }

    /// What a cut without keyframes to follow shows: an ordinary video or a
    /// picture its whole picture, as the export frames it
    /// (`project::keyframed`); a 360 cut the free look. The free look is
    /// shared by every cut of a kind, so showing it on ordinary videos
    /// shifted all of them when one was framed by hand (the user,
    /// 2026-10-24), while the export showed them whole.
    pub(super) fn unkeyed_view(&self) -> View {
        if self.edit.cuts.get(self.current).is_some_and(|c| c.flat) { View::home(Projection::Perspective) } else { self.look }
    }

    /// The view on screen, ready to draw: mid-morph between perspective and
    /// little planet when following keyframes.
    pub(super) fn camera(&self) -> Camera {
        let camera = match self.playing_cut() {
            Some((_, t)) if self.follow => self.keyframed(t).and_then(|(k, at)| k.camera_at(at, &crate::project::held(&|f, t| self.aim(f, t), at == t, t))).unwrap_or_else(|| self.unkeyed_view().camera()),
            _ => self.look.camera(),
        };
        let grade = self.playing_cut().map(|(_, t)| self.grade(self.current, t)).unwrap_or_default();
        Camera { grade, lut: self.edit.cuts.get(self.current).and_then(|c| c.lut_applied), ..camera }
    }

    /// Cut `i`'s colour effects at clip time `t`, faded to black where the
    /// edit's fades are.
    pub(super) fn grade(&self, i: usize, t: f64) -> crate::grade::Grade {
        let Some(cut) = self.edit.cuts.get(i) else { return Default::default() };
        let mut grade = crate::grade::grade_at(&cut.effects, t);
        grade.black = self.edit.fades.black(self.edit.edit_time(i, t), self.edit.duration());
        grade
    }

    /// Makes `look` the free look of the playing cut's kind. Run after every
    /// message, as the current cut changes in many places (playing on at an
    /// out point, a crossfade, a split, undo): swapped only in `goto`, a 360
    /// view made after playing into a 360 cut became the ordinary video's.
    pub(super) fn settle_look(&mut self) {
        let flat = self.edit.cuts.get(self.current).is_some_and(|c| c.flat);
        if flat != self.look_flat {
            std::mem::swap(&mut self.look, &mut self.other_look);
            self.look_flat = flat;
        }
    }

    /// Whether the playing clip is an ordinary video: no stabilisation,
    /// projection, stick or colour track there.
    pub(super) fn on_flat(&self) -> bool {
        self.clip.as_ref().is_some_and(|c| c.playback.media.flat.is_some())
    }

    /// Marks each cut whose clip has loaded as an ordinary video or not;
    /// the views of the two kinds are kept apart (`project::keyframed`).
    pub(super) fn mark_flat(&mut self) {
        let mut missing = Vec::new();
        for cut in &mut self.edit.cuts {
            if let Some(m) = self.library.iter().find(|m| m.input == cut.input) {
                cut.flat = m.flat.is_some();
            }
            // Its LUT loaded too; a missing one leaves the cut as filmed.
            if cut.resolve_lut().is_err() {
                missing.extend(cut.lut.clone().filter(|n| !missing.contains(n)));
            }
        }
        if !missing.is_empty() {
            self.error = Some(format!("Missing LUT, shown as filmed (add it in Settings): {}", missing.join(", ")));
        }
    }

    /// The LUT a newly added clip gets: the D-Log M one for D-Log M.
    pub(super) fn auto_lut(&self, input: &Path) -> Option<String> {
        let media = self.library.iter().find(|m| m.input == input)?;
        (media.color_mode == crate::dji::ColorMode::DLogM).then(|| self.dlog_m_lut.clone()).flatten()
    }

    /// Gives every D-Log M cut without a LUT the D-Log M one, one undo step.
    pub(super) fn lut_dlog_m_cuts(&mut self) {
        use crate::model::ops::{Op, cuts::CutOp};
        // One undo step for them all.
        let ops: Vec<Op> = self.edit.cuts.iter().filter(|c| c.lut.is_none()).filter_map(|c| Some(Op::Cut(CutOp::Lut { cut: c.id, lut: Some(self.auto_lut(&c.input)?) }))).collect();
        if let Err((_, e)) = self.edit.batch(&ops) {
            self.error = Some(e.to_string());
        }
    }

    /// The view to change by hand: stops following keyframes, carrying on
    /// from the view they show now.
    pub(super) fn free_look(&mut self) -> &mut View {
        if self.follow {
            self.look = self.shown_view();
            self.follow = false;
            self.hand_cut = self.edit.cuts.get(self.current).map(|c| (self.current, c.input.clone()));
        }
        &mut self.look
    }

    /// Once the view follows keyframes again (play, a seek, another cut),
    /// a cut looked around in by hand that has no keyframes of its own keeps
    /// what was shown as a keyframe at its in point (one undo step), so it
    /// doesn't share the free look with other cuts or lose it at the next
    /// play. Made then rather than at the first move, which would keep only
    /// the start of a drag. `before` is the free look of each kind (360,
    /// flat) as it was.
    ///
    /// A cut with a single keyframe shows one view throughout, so it is
    /// framed the same way: looking around updates that keyframe. Without
    /// this only the first framing stuck, and every later one snapped back
    /// at the next play or seek. A following keyframe, or two and more (a
    /// camera move), are left alone.
    pub(super) fn keep_hand_view(&mut self, before: (View, View)) {
        if !self.follow {
            return;
        }
        let Some((i, input)) = self.hand_cut.take() else { return };
        let Some(cut) = self.edit.cuts.get(i).filter(|c| c.input == input) else { return };
        let look = if cut.flat { before.1 } else { before.0 };
        let keys: Vec<_> = cut.keyframes.iter().collect();
        let at = match keys.as_slice() {
            [] if look == View::home(Projection::Perspective) && cut.flat => return,
            [] => cut.start,
            [k] if k.follow.is_none() && k.view != look => k.t,
            _ => return,
        };
        let cut = self.edit.cuts[i].id;
        self.op(Op::Key(KeyOp::View { cut, t: at, view: look, tolerance: 0.0, aims: Vec::new() }));
    }

    /// Keys the view shown at the playhead on the playing cut, or updates
    /// the keyframe there; the view then follows the keyframes again.
    pub(super) fn key_view(&mut self) {
        let (view, tolerance) = (self.shown_view(), self.tolerance());
        let Some((_, t)) = self.playing_cut() else { return };
        let aims = self.aims(t);
        if let Some(cut) = self.edit.cuts.get(self.current).map(|c| c.id) {
            self.op(Op::Key(KeyOp::View { cut, t, view, tolerance, aims }));
            self.follow = true;
        }
    }

    /// With Auto keyframe on: a look around while paused (`before` is each
    /// kind's free look as it was) keys the view at the playhead, as K does.
    /// While playing a drag would leave a keyframe each frame, so looking
    /// around stays free there.
    pub(super) fn auto_key(&mut self, before: (View, View)) {
        let now = if self.look_flat { (self.other_look, self.look) } else { (self.look, self.other_look) };
        if !self.auto_key || self.follow || now == before || self.clip.as_ref().is_none_or(|c| c.playback.playing) {
            return;
        }
        let Some((_, t)) = self.playing_cut() else { return };
        // One undo step for a look-around at one time, closed in `update`
        // when something else is done.
        if self.auto_key_at != Some((self.current, t)) {
            self.edit.commit();
            self.edit.begin();
            self.auto_key_at = Some((self.current, t));
        }
        self.key_view();
    }

    /// Jumps to the nearest keyframe before (`-1`) or after (`1`) the playhead.
    pub(super) fn key_step(&mut self, dir: f64) -> Task<Message> {
        let tolerance = self.tolerance();
        let Some((cut, now)) = self.playing_cut() else { return Task::none() };
        let within = |k: &&crate::view::Keyframe| k.t >= cut.start - tolerance && k.t <= cut.end + tolerance;
        let keys: Vec<_> = cut.keyframes.iter().enumerate().filter(|(_, k)| within(k)).collect();
        let next = if dir < 0.0 { keys.iter().rev().find(|(_, k)| k.t < now - tolerance) } else { keys.iter().find(|(_, k)| k.t > now + tolerance) };
        let current = self.current;
        next.map(|(i, _)| *i).map_or(Task::none(), |i| self.update(Message::KeyJump(current, i)))
    }

    /// The 360 view as it should be drawn now, once a frame is decoded.
    pub(super) fn sphere(&self) -> Option<Sphere> {
        let c = self.clip.as_ref()?;
        Some(Sphere {
            rig: c.playback.media.rig.clone(),
            shown: c.shown.clone()?,
            camera: self.camera(),
            rig_from_view: c.playback.rig_from_view(self.rolling_shutter),
            fov_deg: self.shown_view().fov,
            picking: self.picking.is_some(),
            incoming: self.partner_layer(),
            overlays: self.overlay_draws(),
        })
    }
}

impl App {
    /// The view and its keyframes: this area's messages, as `handle` passes them on.
    pub(super) fn on_framing(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LookDone => {}
            Message::Framing(on) => {
                self.framing = on;
                if !on {
                    self.picking = None;
                }
            }
            Message::KeyMove(i, k, _) if self.edit.cuts.get(i).is_none_or(|c| k >= c.keyframes.len()) => {}
            Message::KeyAdd => self.key_view(),
            Message::KeyDelete => {
                if let Some((cut, t)) = self.current_key_time() {
                    self.op(Op::Key(KeyOp::Delete { line: Line::View(cut), t, tolerance: EXACT }));
                }
            }
            Message::KeyEase(ease) => {
                if let Some((cut, t)) = self.current_key_time() {
                    self.op(Op::Key(KeyOp::Ease { line: Line::View(cut), t, ease, tolerance: EXACT }));
                    self.follow = true;
                }
            }
            Message::KeyFollow(_) | Message::Stab(_) | Message::Projection(_) if self.on_flat() => {}
            Message::KeyFollow(aiming) => {
                let (Some(k), Some((_, t))) = (self.current_key(), self.playing_cut()) else { return Task::none() };
                if Aiming::of(&self.edit.cuts[self.current].keyframes[k].follow) == aiming {
                    return Task::none();
                }
                let follow = match aiming {
                    Aiming::Fixed => None,
                    Aiming::Stick => Some(Follow::Stick),
                    // A track comes from picking its colour; the keyframe
                    // here follows it once tracked.
                    Aiming::Colour => return self.update(Message::PickColour),
                };
                let (aims, cut, at) = (self.aims(t), self.edit.cuts[self.current].id, self.edit.cuts[self.current].keyframes[k].t);
                self.op(Op::Key(KeyOp::Follow { cut, t: at, follow, tolerance: EXACT, aims }));
                self.follow = true;
            }
            Message::KeyJump(i, k) => {
                self.follow = true;
                self.selection = Selection::cut_at(&self.edit, i);
                if let Some(t) = self.edit.cuts.get(i).and_then(|c| c.keyframes.get(k)).map(|k| k.t) {
                    let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
                    self.goto(i, t, playing);
                }
            }
            Message::KeyMove(i, k, t) => {
                self.follow = true;
                let Some((cut, from)) = self.edit.cuts.get(i).and_then(|c| Some((c.id, c.keyframes.get(k)?.t))) else { return Task::none() };
                self.edit.begin();
                self.op(Op::Key(KeyOp::Retime { line: Line::View(cut), from, t, gap: 1.0 / self.fps(), tolerance: EXACT }));
                let t = self.edit.cuts[i].keyframes[k].t;
                self.scrub_to(i, t);
            }
            Message::Look { yaw, pitch, roll } => {
                let look = self.free_look();
                look.yaw += yaw.to_degrees();
                look.pitch += pitch.to_degrees();
                look.roll += roll.to_degrees();
            }
            Message::Zoom(deg) => self.free_look().zoom_by(deg),
            Message::Fov(deg) => self.free_look().fov = deg,
            Message::AngleInput(angle, typed) => self.type_angle(angle, typed),
            Message::AngleDone => {}
            Message::AngleScrubStart(angle) => self.scrubbing = Some(angle),
            Message::AngleScrubDone => self.scrubbing = None,
            Message::AngleScrub(angle, dx) => self.scrub_angle(angle, dx),
            Message::AngleNudge(angle, dir) => self.nudge_angle(angle, dir),
            Message::AngleArrow(Some(angle), dir) => self.nudge_angle(angle, dir),
            Message::AngleArrow(None, dir) => self.free_look().pitch += 5.0 * dir,
            Message::Stab(mode) => {
                // The playing cut's, and the one new cuts get.
                self.opts.stabilize = mode;
                if let Some(cut) = self.edit.cuts.get(self.current).map(|c| c.id) {
                    self.op(crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Stabilize { cut, mode: Some(mode) }));
                }
                self.with_playback(|p| p.media.stab.set_mode(mode));
            }
            Message::Projection(p) => {
                let look = self.free_look();
                look.projection = p;
                look.reset();
            }
            Message::Source(s) => {
                let size = match s {
                    Source::Proxy => None,
                    Source::Osv(size) => Some(size),
                };
                self.opts.osv_size = size;
                self.with_playback(|p| p.set_osv_size(size));
            }
            Message::RollingShutter(on) => self.rolling_shutter = on,
            Message::FullOnPause(on) => {
                self.full_on_pause = on;
                self.with_playback(|p| p.set_full_on_pause(on));
            }
            Message::ResetView => self.free_look().reset(),
            _ => {}
        }
        Task::none()
    }
}
