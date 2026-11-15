//! Playing the edit: the clock and moving on from cut to cut (`tick`, the
//! next cut primed `PREROLL` ahead), the crossfade partner playing alongside,
//! the music's and the clips' levels as it plays, going to a time, and the
//! timeline's window following the playhead.

use super::*;

/// The next cut is primed this many seconds before the playing one ends:
/// enough for its decoder to open and seek. From a network share that took
/// 0.2-1.6 s; at 1 s ahead the slower ones still stalled the change.
pub(super) const PREROLL: f64 = 3.0;

/// The timeline zooms in until this many seconds fill it.
pub(super) const TIMELINE_SPAN: f64 = 2.0;

/// A range marked on the ruler from `a` to `b`, either way, within an
/// edit `length` long (gap 92); none if next to nothing is left.
pub(super) fn marked((a, b): (f64, f64), length: f64) -> Option<(f64, f64)> {
    let (from, to) = (a.min(b).max(0.0), a.max(b).min(length));
    (to - from > 1e-3).then_some((from, to))
}

/// Where a stretch playing (`play_stretch`) stands.
#[derive(Debug, PartialEq)]
enum Stretch {
    Playing,
    /// Stopped short of its end (paused, a seek): let go.
    LetGo,
    /// At its end: back to its start, paused.
    Back,
}

/// A stretch ending at `stop` with the playhead at `at`, playing or not;
/// `frame` long frames. Stopped within a frame of its end counts as there:
/// a stretch to the edit's end is paused by the edit first (audit A15).
fn stretch_step(playing: bool, at: f64, stop: f64, frame: f64) -> Stretch {
    match (playing, at >= stop - if playing { 0.0 } else { frame }) {
        (_, true) => Stretch::Back,
        (true, false) => Stretch::Playing,
        (false, false) => Stretch::LetGo,
    }
}

impl App {
    /// Plays `start..stop` once (a join, a marked range): past its end it's
    /// paused back at its start (`follow_stretch`).
    pub(super) fn play_stretch(&mut self, stretch: Option<(f64, f64)>) -> Task<Message> {
        let Some((start, stop)) = stretch else { return Task::none() };
        // One jump that plays: a seek and then Play started from the frame
        // still on screen, before the seek's had decoded.
        let Some((i, t)) = self.edit.locate(start) else { return Task::none() };
        self.pause_source();
        self.follow = true;
        self.goto(i, t, true);
        self.stretch_play = Some((stop, start));
        Task::none()
    }

    /// While a stretch plays (`play_stretch`): past its end, paused and
    /// back at its start; stopped another way (paused, a seek), let go.
    pub(super) fn follow_stretch(&mut self) -> Task<Message> {
        let Some((stop, start)) = self.stretch_play else { return Task::none() };
        let playing = self.playing();
        match stretch_step(playing, self.position(), stop, self.frame_len()) {
            Stretch::Playing => return Task::none(),
            Stretch::LetGo => self.stretch_play = None,
            Stretch::Back => {
                self.stretch_play = None;
                // The edit's own end has paused it already.
                let pause = if playing { self.update(Message::Toggle) } else { Task::none() };
                return Task::batch([pause, self.update(Message::Scrub(start)), self.update(Message::ScrubDone)]);
            }
        }
        Task::none()
    }

    /// Keeps the music in step with the edit: playing where playback is, at
    /// its level, or stopped while playback is.
    pub(super) fn follow_music(&mut self) {
        let (playing, t) = match &self.clip {
            // Only while the clock runs: waiting for its first frame it stands
            // still, and music started then took it as drift and restarted
            // over and over until the picture came.
            Some(c) if self.current < self.edit.cuts.len() => (c.playback.running(), self.edit.edit_time(self.current, c.playback.now())),
            _ => (false, 0.0),
        };
        for (player, which) in self.music.iter_mut().zip(SoundTrack::ALL) {
            player.follow(self.edit.track(which), t, self.edit.duration(), playing, self.listen.heard(listen::lane_of(which)));
        }
        // The cuts' sound past their pictures, played as sound clips are, so
        // a J-cut's head is heard under the cut before and an L-cut's tail
        // under the cut after.
        let beyond = self.edit.sound_beyond();
        self.beyond.resize_with(beyond.len(), Default::default);
        let heard = self.listen.heard(listen::CLIP_SOUND);
        for (player, track) in self.beyond.iter_mut().zip(&beyond) {
            player.follow(track, t, self.edit.duration(), playing, heard);
        }
        self.meter.follow(playing);
        // The cut's own volume points move its level as it plays, and
        // through a crossfade the two cuts trade places, equal power.
        let shares = self.crossfade_shares();
        let v = self.clip_volume() * shares.map_or(1.0, |(mine, _)| crate::mixdown::equal_power(mine) as f32);
        if let Some(c) = &mut self.clip {
            c.playback.set_volume(v);
        }
        if let (Some((_, theirs)), Some((i, inc))) = (shares, &self.partner) {
            let v = self.cut_level(*i, inc.playback.now()) * crate::mixdown::equal_power(theirs) as f32;
            if let Some((_, inc)) = &mut self.partner {
                inc.playback.set_volume(v);
            }
        }
    }

    /// Keeps the cut crossfading in alongside the playing one wherever
    /// playback is in a crossfade (played into, sought, scrubbed or paused
    /// there): at the matching time in its own clip, playing or paused with
    /// it. Elsewhere there's none.
    pub(super) fn follow_crossfade(&mut self) {
        // Played, the outgoing cut leads again, as `tick` expects.
        if self.playing() && self.partner.as_ref().is_some_and(|(j, _)| j + 1 == self.current) {
            self.swap_partner();
            if let Some(c) = &mut self.clip {
                c.playback.scrub_end();
                c.playback.play();
            }
        }
        let want = (|| {
            let (_, t) = self.clip_now()?;
            let playing = self.clip.as_ref()?.playback.playing;
            // The outgoing cut playing or paused in its crossfade: the next
            // cut alongside; the incoming cut paused in it (taken over to
            // edit): the one before, shown when it's not selected.
            let (i, t) = self.edit.crossfade_partner(self.current, t, playing)?;
            Some((i, self.edit.cuts[i].input.clone(), t, playing && i > self.current))
        })();
        let Some((i, input, t, playing)) = want else {
            self.partner = None;
            return;
        };
        match &mut self.partner {
            Some((j, inc)) if *j == i && inc.playback.media.plays(&self.edit.cuts[i]) => {
                let p = &mut inc.playback;
                // Primed to catch up: started as the edit gets there.
                if let Some(at) = p.go_at {
                    if !playing {
                        p.go_at = None;
                        p.pause();
                    } else if t >= at - 0.5 / p.media.fps {
                        p.go();
                    }
                    return;
                }
                // Played from paused, primed ahead as a resync is: played
                // from its paused frame, it came up behind and froze for the
                // 1-2 s of a resync.
                if p.playing != playing {
                    if !playing {
                        p.pause();
                    } else {
                        inc.playback = p.primed_again(p.media.speed.clip_after(t, Self::catch_up(p.startup())));
                        return;
                    }
                }
                // Playing, the two clocks run side by side, once it's running:
                // waiting for its first frame (a second or more for the OSV)
                // its clock stands still, and taking that for drift restarted
                // it over and over. Drifted, it's primed again further on, by
                // its decoder's start-up and some: jumped to where the edit
                // was, it came up as far behind as its start took and jumped
                // again, showing nothing (OSV 1920 px, 1.1-1.4 s). Paused, it
                // follows the playhead as the playing cut does when scrubbed:
                // a frame at a time, skipping those overtaken (a jump each
                // time started decoders faster than they ended).
                if playing {
                    if p.running() && (p.now() - t).abs() > 0.25 {
                        inc.playback = p.primed_again(p.media.speed.clip_after(t, Self::catch_up(p.startup())));
                    }
                } else if (p.target() - t).abs() > 1.0 / p.media.fps {
                    p.scrub(t);
                }
            }
            // Played into: the cut readied ahead, if it was.
            _ if playing && t - self.edit.cuts[i].start < 0.25 && self.preroll.as_ref().is_some_and(|(j, inp, at, _)| (*j, inp, *at) == (i, &input, self.edit.cuts[i].start)) => {
                let (_, _, _, mut primed) = self.preroll.take().unwrap();
                primed.go();
                self.partner = Some((i, Clip { playback: primed, shown: None }));
            }
            _ => {
                let Some(media) = cut_media(&self.library, &self.opts, &self.edit.cuts[i]) else { return };
                // Playing, primed ahead to start as the edit gets there, as
                // a resync is.
                let playback = if playing {
                    let at = media.speed.clip_after(t, Self::catch_up(0.0));
                    let mut p = Playback::primed(media, at, 0.0, self.full_on_pause);
                    p.go_at = Some(p.target());
                    p
                } else {
                    let mut p = Playback::new(media, 0.0, self.full_on_pause);
                    p.jump(t, false);
                    p
                };
                self.partner = Some((i, Clip { playback, shown: None }));
            }
        }
    }

    /// How far ahead of the edit a crossfade's partner is primed to catch
    /// up, edit seconds, for a decoder that took `startup` seconds to start
    /// last time:
    /// with room for it to take longer (it varied 1.1-1.4 s), at least half
    /// a second.
    pub(super) fn catch_up(startup: f64) -> f64 {
        // Not measured yet: about an OSV's at 1920 px.
        let startup = if startup > 0.0 { startup } else { 1.0 };
        (startup * 1.5).max(0.5)
    }

    /// The clips' sound in the mix: its level, the playing cut's own level
    /// (normalized, lowered or muted), its fades and volume points where it is
    /// (the listening volume is `audio::set_listening`'s).
    pub(super) fn clip_volume(&self) -> f32 {
        self.cut_level(self.current, self.clip.as_ref().map_or(0.0, |c| c.playback.now()))
    }

    /// Cut `i`'s sound at clip time `t`: the clips' level, its own (normalized,
    /// lowered or muted), its volume points and fades; heard as the lane is.
    /// None there: the clips' level alone.
    fn cut_level(&self, i: usize, t: f64) -> f32 {
        let (own, points, fade) = self.edit.cuts.get(i).map_or((0.0, 0.0, 1.0), |c| (c.sound_db(), crate::project::volume_at(&c.volume, t), c.sound_fade(t)));
        crate::mixdown::cut_gain(self.edit.sound.volume, own, points, fade) as f32 * self.listen.heard(listen::CLIP_SOUND)
    }


    pub(super) fn with_playback(&mut self, f: impl FnOnce(&mut Playback)) {
        if let Some(c) = &mut self.clip {
            f(&mut c.playback);
            self.error = c.playback.error.clone();
        }
    }

    pub(super) fn tick(&mut self) {
        let volume = self.clip_volume();
        self.show_frames();
        let Some(c) = &self.clip else { return };
        let Some(cut) = self.edit.cuts.get(self.current) else { return };
        let p = &c.playback;
        // A cut running to its clip's end has paused there (`ended`), and
        // still moves on.
        if !p.playing && !p.ended {
            self.preroll = None;
            return;
        }
        let (now, ended, out) = (p.now(), p.ended, cut.end);
        // A next cut that starts where this one ends in the same clip (after
        // a split) and plays alike just plays on: no seek, nothing to prime.
        let carries_on = self.edit.cuts.get(self.current + 1).is_some_and(|n| p.media.plays(n) && (n.start - cut.end).abs() < 1.5 / p.media.fps && n.crossfade == 0.0);
        self.ready_next(now, volume, carries_on);
        // At a cut's out point (or the clip's end), play on into the next cut.
        if now >= out || ended {
            self.move_on(carries_on, ended);
        }
    }

    /// The frames due from the playing cut and its crossfade partner.
    fn show_frames(&mut self) {
        if let Some((_, inc)) = &mut self.partner
            && inc.playback.go_at.is_none()
            && let Some((frame, fix)) = inc.playback.tick()
        {
            inc.shown = Some(Arc::new(Shown::new(frame, fix)));
        }
        let Some(c) = &mut self.clip else { return };
        if let Some((frame, fix)) = c.playback.tick() {
            c.shown = Some(Arc::new(Shown::new(frame, fix)));
            self.held = None;
        }
        self.error = c.playback.error.clone();
    }

    /// Towards the change to the next cut, with the playing one's clock at
    /// clip time `now`: the next cut readied, its sound started a device
    /// latency early, and at a crossfade played alongside.
    fn ready_next(&mut self, now: f64, volume: f32, carries_on: bool) {
        let cut = &self.edit.cuts[self.current];
        let next = self.current + 1;
        let next_cut = self.edit.cuts.get(next);
        // Where the next cut comes in: this one's end, or before it by the
        // crossfade; and edit seconds before it, read at this cut's speed.
        let change = self.edit.crossfade_from(self.current).unwrap_or(cut.end);
        let before_change = |secs: f64| cut.speed.clip_after(change, -secs);
        // In the last `PREROLL` seconds, ready the next cut so the change is
        // instant.
        if now >= before_change(PREROLL)
            && !carries_on
            && self.partner.as_ref().is_none_or(|(i, _)| *i != next)
            && let Some(n) = next_cut
            && self.preroll.as_ref().is_none_or(|(i, input, start, _)| (*i, input, *start) != (next, &n.input, n.start))
            && let Some(media) = cut_media(&self.library, &self.opts, n)
        {
            let primed = Playback::primed(media, n.start, volume, self.full_on_pause);
            self.preroll = Some((next, n.input.clone(), n.start, primed));
        }
        // Its sound starts a device latency early, so it's heard from the
        // moment this cut's ends (that latency is otherwise a gap, and the
        // picture waits through it to stay in sync).
        if let Some((i, input, at, primed)) = &mut self.preroll
            && next_cut.is_some_and(|n| (*i, &*input, *at) == (next, &n.input, n.start))
            && primed.sound_lead().is_some_and(|lead| now >= before_change(lead))
        {
            primed.start_sound();
        }
        // A crossfade starts: the next cut plays on alongside, drawn over
        // this one, until this one ends.
        if change < cut.end
            && now >= change
            && self.partner.as_ref().is_none_or(|(i, _)| *i != next)
            && let Some((i, input, at, mut primed)) = self.preroll.take()
            && next_cut.is_some_and(|n| (i, &input, at) == (next, &n.input, n.start))
        {
            primed.go();
            self.partner = Some((next, Clip { playback: primed, shown: None }));
        }
    }

    /// At the playing cut's out point: on into the next cut (on in the
    /// same playback, the crossfade's partner, the one readied, or a fresh
    /// one), or stopped at the edit's end or a clip still loading.
    fn move_on(&mut self, carries_on: bool, ended: bool) {
        let next = self.current + 1;
        let playing = self.clip.as_ref().is_some_and(|c| c.playback.playing);
        match self.edit.cuts.get(next) {
            None if playing => self.with_playback(Playback::pause),
            None => {}
            // Its clip is still loading: wait at the out point.
            Some(n) if !self.library.iter().any(|m| m.input == n.input) => {
                if playing {
                    self.with_playback(Playback::pause);
                    self.save_status = Some(format!("Cut {} is still loading", next + 1));
                }
            }
            Some(_) if carries_on && !ended => {
                self.current = next;
                let mode = self.edit.cuts[next].stabilize.unwrap_or(self.opts.stabilize);
                self.with_playback(|p| p.media.stab.set_mode(mode));
            }
            // Out of a crossfade: the cut that came in plays on.
            Some(_) if self.partner.as_ref().is_some_and(|(i, _)| *i == next) => {
                let (_, mut incoming) = self.partner.take().unwrap();
                if incoming.playback.go_at.is_some() {
                    incoming.playback.go();
                }
                self.clip = Some(incoming);
                self.current = next;
                self.preroll = None;
            }
            Some(n) => {
                let (start, input) = (n.start, n.input.clone());
                match self.preroll.take() {
                    Some((i, at_input, at, mut primed)) if (i, &at_input, at) == (next, &input, start) => {
                        // The last frame stays up until the primed one's first.
                        if self.held.is_none() {
                            self.held = self.sphere();
                        }
                        primed.go();
                        self.clip = Some(Clip { playback: primed, shown: None });
                        self.current = next;
                    }
                    _ => self.goto(next, start, true),
                }
            }
        }
    }

    /// Shows cut `i` at clip time `t`, playing or paused. A cut of another
    /// clip gets a fresh playback of it from the library; the last frame
    /// stays on screen until the new one is decoded.
    pub(super) fn goto(&mut self, i: usize, t: f64, play: bool) {
        let Some(cut) = self.edit.cuts.get(i) else { return };
        let t = t.clamp(cut.start, cut.end);
        // Another clip, or the same clip heard through another audio file.
        let other_clip = self.clip.as_ref().is_none_or(|c| !c.playback.media.plays(cut));
        if (i != self.current || other_clip) && self.held.is_none() {
            self.held = self.sphere();
        }
        match &mut self.clip {
            Some(c) if !other_clip => c.playback.jump(t, play),
            _ => {
                let Some(media) = cut_media(&self.library, &self.opts, cut) else { return };
                let mut playback = Playback::new(media, self.clip_volume(), self.full_on_pause);
                playback.jump(t, play);
                self.clip = Some(Clip { playback, shown: None });
            }
        }
        self.current = i;
        let mode = self.edit.cuts[i].stabilize.unwrap_or(self.opts.stabilize);
        self.with_playback(|p| p.media.stab.set_mode(mode));
        self.settle_look();
    }

    /// Follows a drag (the playhead, a trim, a keyframe) to clip time `t`
    /// of cut `i`, switching clips if the drag crossed into another one.
    pub(super) fn scrub_to(&mut self, i: usize, t: f64) {
        if i != self.current || self.clip.is_none() {
            self.goto(i, t, false);
        }
        self.with_playback(|p| p.scrub(t));
    }

    /// Where playback is in the playing cut, by the clock `tick` changes
    /// cuts by: playing, the clock (the frame shown lags it a little, and
    /// going by that dropped a crossfade `tick` had just begun); paused,
    /// where it's been put.
    pub(super) fn clip_now(&self) -> Option<(&Cut, f64)> {
        let cut = self.edit.cuts.get(self.current)?;
        let p = &self.clip.as_ref()?.playback;
        Some((cut, if p.playing { p.now() } else { p.target() }.clamp(cut.start, cut.end)))
    }

    /// Through a crossfade, the current cut's share of the blend and its
    /// partner's (linear, adding to 1): the outgoing cut's falls as the
    /// incoming one's rises.
    pub(super) fn crossfade_shares(&self) -> Option<(f64, f64)> {
        let (_, t) = self.clip_now()?;
        let (j, _) = self.partner.as_ref()?;
        let incoming = self.edit.crossfade_share(self.current, *j, t)?;
        Some(if *j == self.current + 1 { (1.0 - incoming, incoming) } else { (incoming, 1.0 - incoming) })
    }

    /// Makes the partner the current cut and the current one its partner:
    /// to edit the other cut of a crossfade, or to play it as it plays.
    pub(super) fn swap_partner(&mut self) {
        let Some((j, other)) = self.partner.take() else { return };
        if let Some(mine) = self.clip.replace(other) {
            self.partner = Some((self.current, mine));
        }
        self.current = j;
    }

    /// The other cut of a crossfade, as the view draws it over the current one.
    pub(super) fn partner_layer(&self) -> Option<gpu::Incoming> {
        // Paused with the playing cut selected, it's shown on its own, to
        // frame it.
        if self.selection.cut(&self.edit) == Some(self.current) && self.clip.as_ref().is_some_and(|c| !c.playback.playing) {
            return None;
        }
        let (i, inc) = self.partner.as_ref()?;
        let shown = inc.shown.clone()?;
        let p = &inc.playback;
        let t = p.target();
        // As `camera` frames the playing cut: by its keyframes (or those it
        // holds from before), else the view as it is.
        let keyed = || crate::project::keyframed(&self.edit.cuts, *i, t).and_then(|(k, at)| k.camera_at(at, &crate::project::held(&|f, t| self.aim_in(&p.media, f, t), at == t, t)));
        // The free look of the partner's own kind: a 360 view would push an
        // ordinary video's picture off screen, and the other way round.
        let look = if self.edit.cuts.get(*i).is_some_and(|c| c.flat) == self.look_flat { self.look } else { self.other_look };
        let camera = if self.follow { keyed().unwrap_or_else(|| look.camera()) } else { look.camera() };
        let camera = Camera { grade: self.grade(*i, t), lut: self.edit.cuts.get(*i).and_then(|c| c.lut_applied), ..camera };
        // The join's transition is the incoming cut's (`Cut::transition`).
        let transition = self.edit.cuts.get(*i).map(|c| c.transition_spec()).unwrap_or_default();
        let style = self.edit.cuts.get(*i).and_then(|c| Some((c.style.as_ref()?.spec(), t as f32)));
        Some(gpu::Incoming { rig: p.media.rig.clone(), shown, camera, rig_from_view: p.rig_from_view(self.rolling_shutter), mix: self.crossfade_shares()?.1 as f32, transition, style })
    }

    /// The playing cut, and the playhead's clip time. That is where a seek
    /// went even before its frame has decoded, and within the cut: in points
    /// fall between frames, so the frame shown can be a hair before it, and a
    /// keyframe there would be outside the cut, never drawn.
    pub(super) fn playing_cut(&self) -> Option<(&Cut, f64)> {
        let cut = self.edit.cuts.get(self.current)?;
        Some((cut, self.clip.as_ref()?.playback.target().clamp(cut.start, cut.end)))
    }

    pub(super) fn fps(&self) -> f64 {
        self.clip.as_ref().map_or(30.0, |c| c.playback.media.fps)
    }

    /// One frame's length, seconds, at the playing clip's rate: the step
    /// of a nudge, a key's least gap, the last frame before a cut's end.
    pub(super) fn frame_len(&self) -> f64 {
        1.0 / self.fps()
    }

    /// The playhead in edit time.
    pub(super) fn position(&self) -> f64 {
        match &self.clip {
            Some(c) if self.current < self.edit.cuts.len() => {
                // In points fall between frames, so the frame shown can be a
                // hair before the cut.
                let cut = &self.edit.cuts[self.current];
                self.edit.edit_time(self.current, c.playback.position().clamp(cut.start, cut.end))
            }
            _ => 0.0,
        }
    }

    /// After a cut's sound or speed changed (its level is followed every
    /// frame; its evening out and its tempo are in the decode), restarts
    /// the playing cut where it is if either changed.
    pub(super) fn reload_cut_sound(&mut self) {
        let Some(cut) = self.edit.cuts.get(self.current) else { return };
        let Some(c) = &self.clip else { return };
        if c.playback.media.plays(cut) {
            return;
        }
        let playing = c.playback.playing;
        if let Some((_, t)) = self.playing_cut() {
            self.goto(self.current, t, playing);
        }
    }

    /// After cuts were removed or restored: back to edit time `at`, or the
    /// start screen if no cut is left.
    pub(super) fn after_edit(&mut self, at: f64) {
        self.selection = self.selection.still_valid(&self.edit);
        match self.edit.locate(at.min(self.edit.duration())) {
            Some((i, t)) => {
                // The playback may be of a clip no longer at this index.
                if self.edit.cuts[i].input != self.clip.as_ref().map_or_else(PathBuf::new, |c| c.playback.media.input.clone()) {
                    self.clip = None;
                }
                self.goto(i, t, false);
                // A clip that can't be opened leaves `goto` early: still not
                // an index past the edit (the audit, 2026-11-11).
                self.current = i;
            }
            None => {
                self.clip = None;
                self.current = 0;
            }
        }
    }

    /// The stretch of edit time the timeline shows: (start, span).
    pub(super) fn timeline_window(&self) -> (f64, f64) {
        let d = self.edit.frame();
        // An empty edit (an empty day): a minute of ruler to drop into.
        if d <= 0.0 {
            return (0.0, 60.0);
        }
        let span = (d / self.timeline_zoom).max(TIMELINE_SPAN.min(d));
        (self.timeline_start.clamp(0.0, (d - span).max(0.0)), span)
    }

    /// The closest zoom: `TIMELINE_SPAN` seconds across.
    pub(super) fn timeline_most_zoom(&self) -> f64 {
        (self.edit.frame() / TIMELINE_SPAN).max(1.0)
    }

    /// Zooms the timeline keeping `anchor` in place: by default the playhead
    /// if it is in sight, else the middle.
    pub(super) fn zoom_timeline(&mut self, zoom: f64, anchor: Option<f64>) {
        let (start, span) = self.timeline_window();
        let position = self.position();
        let anchor = anchor.unwrap_or(if position >= start && position <= start + span { position } else { start + span / 2.0 });
        self.timeline_zoom = zoom.clamp(1.0, self.timeline_most_zoom());
        let (_, new_span) = self.timeline_window();
        self.timeline_start = anchor - (anchor - start) / span.max(1e-9) * new_span;
        self.timeline_start = self.timeline_window().0;
    }

    /// Scrolls the timeline to keep the playhead in sight, a page at a time.
    pub(super) fn reveal_playhead(&mut self) {
        let (start, span) = self.timeline_window();
        let position = self.position();
        if position < start || position > start + span {
            self.timeline_start = (position - span * 0.05).max(0.0);
        }
        self.timeline_start = self.timeline_window().0;
    }
}


impl App {
    /// Playing and moving through the edit: this area's messages, as `handle` passes them on.
    pub(super) fn on_transport(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.tick();
                self.tick_source();
                if self.playing() {
                    self.reveal_playhead();
                }
            }
            Message::TimelineZoom(factor, anchor) => self.zoom_timeline(self.timeline_zoom * factor, anchor),
            Message::TimelineWheel(dx, dy, at) => {
                let (pan, zoom) = if self.modifiers.shift() { (dx + dy, 0.0) } else { (dx, dy) };
                if zoom != 0.0 {
                    self.zoom_timeline(self.timeline_zoom * 1.25f64.powf(zoom as f64), Some(at));
                } else if pan != 0.0 {
                    let (start, span) = self.timeline_window();
                    self.timeline_start = start - pan as f64 * span * 0.1;
                    self.timeline_start = self.timeline_window().0;
                }
            }
            Message::TimelineZoomTo(v) => self.zoom_timeline(self.timeline_most_zoom().powf(v), None),
            Message::TimelinePan(start) => self.timeline_start = start,
            // Space while the bin shows a clip plays that clip.
            Message::Toggle if self.bin.view && self.bin.source.is_some() => return self.on_source(bin_player::SourceMsg::Toggle),
            Message::Toggle => {
                self.pause_source();
                self.follow = true;
                let playing = self.playing();
                if !playing && self.position() >= self.edit.duration() - 1.5 / self.fps() {
                    // From the end of the edit, play again from the start.
                    let start = self.edit.cuts.first().map_or(0.0, |c| c.start);
                    self.goto(0, start, true);
                } else {
                    self.with_playback(Playback::toggle);
                }
            }
            Message::Scrub(t) => {
                self.follow = true;
                // In an overlap, the selected cut's time, if it's one of the two.
                let selected = self.selection.cut(&self.edit).and_then(|i| Some((i, self.edit.on_cut(i, t)?)));
                if let Some((i, t)) = selected.or_else(|| self.edit.locate(t)) {
                    self.scrub_to(i, t);
                }
            }
            Message::ScrubDone => {
                self.snap_line = None;
                self.edit.commit();
                self.with_playback(Playback::scrub_end);
                if let Some((_, inc)) = &mut self.partner {
                    inc.playback.scrub_end();
                }
            }
            _ => {}
        }
        Task::none()
    }
}

/// A cut's clip as the player opens it from the loaded `library`: its own
/// sound and filters, the play size, its stabilisation. (Free of `App`, so
/// it can be called while the playing clip is borrowed.)
fn cut_media(library: &[Media], opts: &Options, cut: &Cut) -> Option<Media> {
    let mut media = library.iter().find(|m| m.input == cut.input)?.for_cut(cut);
    media.osv_size = opts.osv_size;
    media.stab.set_mode(cut.stabilize.unwrap_or(opts.stabilize));
    Some(media)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_is_marked_either_way_within_the_edit() {
        assert_eq!(marked((5.0, 2.0), 10.0), Some((2.0, 5.0)), "dragged back");
        assert_eq!(marked((-1.0, 12.0), 10.0), Some((0.0, 10.0)), "held to the edit");
        assert_eq!((marked((3.0, 3.0), 10.0), marked((11.0, 12.0), 10.0)), (None, None), "nothing left");
    }

    #[test]
    fn a_stretch_to_the_edits_end_comes_back_to_its_start() {
        let frame = 1.0 / 25.0;
        assert_eq!(stretch_step(true, 4.0, 5.0, frame), Stretch::Playing);
        assert_eq!(stretch_step(true, 5.0, 5.0, frame), Stretch::Back, "past its end");
        // The edit's end paused it on its last frame, short of the stop.
        assert_eq!(stretch_step(false, 5.0 - frame / 2.0, 5.0, frame), Stretch::Back);
        assert_eq!(stretch_step(false, 4.0, 5.0, frame), Stretch::LetGo, "paused by hand");
    }
}
