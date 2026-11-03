//! The window's bars, as the view draws them: the start screen, the top bar
//! (file, add, export; for a 360 cut while framing, how it's steadied and
//! drawn), the picture with the panels beside it, the timeline's model, and
//! the transport row. `App::view` puts them together.

use super::*;
use iced::widget::{column, row};

impl App {
    /// Before any clip: open clips, a project, or start from a song.
    pub(super) fn start_screen(&self) -> Element<'_, Message> {
        let error = self.error.as_ref().map(|e| text(e.clone()).style(text::danger));
        // Music first: the song is chosen and sets the length to fill.
        let song = self.edit.music.clips.first().map(|c| {
            let name = c.input.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            format!("Music: {name} ({}). Add clips to fill it.", clock(self.edit.music.end()))
        });
        let status: Element<'_, Message> = match (&self.loading, song) {
            (Some(_), _) => self.loading_bar(280.0),
            (None, Some(song)) => text(song).into(),
            (None, None) => text("Open DJI Osmo 360 .OSV clips, other videos or pictures, or a saved project, to start. Or start from a song.").into(),
        };
        let idle = self.loading.is_none();
        let buttons = row![
            labelled_button(ADD_CLIP, "Open clips…", idle.then_some(Message::Open)),
            labelled_button(OPEN_PROJECT, "Open project…", idle.then_some(Message::OpenProject)),
            labelled_button(ADD_MUSIC, "Add music…", idle.then_some(Message::AddSound(SoundTrack::Music))),
        ]
        .spacing(12);
        let start = center(column![self.logo(), status, buttons].push(error).spacing(16).align_x(Alignment::Center));
        column![start, credit()].into()
    }

    /// The top bar. Groups: settings | the project file | what to add |
    /// the export; then, while framing a 360 cut, how it is steadied and
    /// drawn; at the right the meter, the interface's size, fullscreen and
    /// the status.
    pub(super) fn top_bar<'a>(&'a self, clip: &'a Clip) -> Element<'a, Message> {
        let open_hint = if self.edit.cuts.is_empty() { "Open clips (Ctrl+O)" } else { "Add clips to the edit as cuts (Ctrl+O)" };
        let p = &clip.playback;
        let media = &p.media;

        let quality = p.quality();
        let showing = if quality.starts_with("loading") {
            status_icon(SHOWING_LOADING, format!("Showing the playback picture, {quality}"))
        } else if quality == "proxy" {
            status_icon(SHOWING_PROXY, "Showing the proxy (LRF): quick to decode, less detail".to_owned())
        } else {
            status_icon(SHOWING_FULL, format!("Showing the {}", if quality == "full res" { "OSV at full resolution".to_owned() } else { quality.clone() }))
        };
        let decode = match media.hw {
            Hwaccel::Software => status_icon(DECODE_CPU, "Decoding in software: no GPU decoder worked here".to_owned()),
            hw => status_icon(DECODE_GPU, format!("Decoding on the GPU with {}", match hw {
                Hwaccel::Vulkan => "Vulkan Video",
                Hwaccel::Vaapi => "VA-API",
                Hwaccel::VideoToolbox => "VideoToolbox",
                Hwaccel::D3d11va => "Direct3D 11",
                _ => "CUDA",
            })),
        };
        // One line: wrapped, a long one stood the whole top bar on end.
        let message = self.save_status.as_ref().map(|s| text(s.clone()).style(text::secondary).wrapping(text::Wrapping::None));
        // After a delete: what went, and Undo one click away.
        let toast = self.toast.map(|(what, _)| row![text(what).wrapping(text::Wrapping::None), iced::widget::button(text("Undo").size(13)).padding([2, 8]).on_press(Message::Undo)].spacing(8).align_y(Alignment::Center));
        let status: Element<'_, Message> = match &self.loading {
            Some(_) => self.loading_bar(140.0),
            // What decodes and shows matters for 360 footage: shown while framing.
            None => row![].push(toast).push(message).push(self.framing.then_some(showing)).push(self.framing.then_some(decode)).spacing(10).align_y(Alignment::Center).into(),
        };
        let shown_view = self.shown_view();
        let mut sources = Vec::new();
        if media.proxy.is_some() {
            sources.push(Source::Proxy);
        }
        sources.push(Source::Osv(1920.min(media.osv_full)));
        sources.push(Source::Osv(media.osv_full));
        sources.dedup();
        let source = match (media.osv_size, &media.proxy) {
            (None, Some(_)) => Source::Proxy,
            (size, _) => Source::Osv(size.unwrap_or(1920)),
        };
        // Groups: settings | the project file | what to add | the export;
        // then, while framing a 360 cut, how it is steadied and drawn.
        let top = row![
            tip(
                icon_button(SETTINGS, Some(Message::SettingsPanel)),
                text(if self.settings_open { "Close the Settings panel (Ctrl+,)" } else { "Settings (Ctrl+,)" }),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(KEYBOARD, Some(Message::Shortcuts)), text("Keyboard and mouse shortcuts (? or F1)"), tooltip::Position::Bottom),
            separator(),
            tip(icon_button(NEW_PROJECT, Some(Message::NewProject)), text("Start a new, empty project (Ctrl+N)"), tooltip::Position::Bottom),
            tip(icon_button(OPEN_PROJECT, Some(Message::OpenProject)), text("Open a saved edit (Ctrl+Shift+O)"), tooltip::Position::Bottom),
            tip(
                icon_button(SAVE, self.unsaved().then_some(Message::SaveProject(false))),
                text("Save the edit: cuts, keyframes and shape (Ctrl+S)"),
                tooltip::Position::Bottom,
            ),
            tip(
                icon_button(SAVE_AS, (!self.edit.cuts.is_empty()).then_some(Message::SaveProject(true))),
                text("Save the edit as a new project file (Ctrl+Shift+S)"),
                tooltip::Position::Bottom,
            ),
            separator(),
            tip(icon_button(ADD_CLIP, Some(Message::Open)), text(open_hint), tooltip::Position::Bottom),
            tip(icon_button(ADD_BLANK, Some(Message::AddBlank)), text("Add a blank (black) cut at the playhead: between cuts at a join, else it splits the cut"), tooltip::Position::Bottom),
            tip(
                icon_button(ADD_MUSIC, Some(Message::AddSound(SoundTrack::Music))),
                text(if self.edit.music.clips.is_empty() { "Add music under the edit" } else { "Add more music, after the last song" }),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(ADD_SFX, Some(Message::AddSound(SoundTrack::Sfx))), text("Add a sound effect at the playhead, on the SFX lane"), tooltip::Position::Bottom),
            tip(
                icon_button(ADD_VOICE, Some(Message::Record(super::record_ui::RecordMsg::Toggle))),
                text(if self.side == Some(Side::Record) { "Close the voice-over panel" } else { "Voice-over: record one at the playhead, or add one from disk" }),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(ADD_OVERLAY, Some(Message::AddOverlay)), text("Add pictures over the picture, at the playhead (PNG transparency shows through)"), tooltip::Position::Bottom),
            tip(icon_button(ADD_TEXT, Some(Message::AddText)), text("Add text over the picture, at the playhead (typed in the Text panel at the left)"), tooltip::Position::Bottom),
            separator(),
            self.export_controls(),
        ]
        // How a 360 clip is steadied and drawn: only while framing one; an
        // ordinary video or a picture has neither.
        .push((self.framing && media.flat.is_none()).then(|| labelled("Stabilisation", pick_list(&Mode::ALL[..], Some(media.stab.mode()), Message::Stab))))
        .push((self.framing && media.flat.is_none()).then(|| labelled("Projection", pick_list(&Projection::ALL[..], Some(shown_view.projection), Message::Projection))))
        .push((self.framing && media.flat.is_none()).then(|| {
            row![
                tip(labelled("Source", pick_list(sources, Some(source), Message::Source)), text("Playback source: the proxy is quick, the OSV sharper"), tooltip::Position::Bottom),
                tip(
                    toggler(self.full_on_pause).label("Full res").on_toggle(Message::FullOnPause),
                    text("Full res when paused: a paused frame is decoded from the OSV at full size; off, it stays at the playback source's quality"),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        }))
        .push(row![
            space().width(Length::Fill),
            // Beside what's playing: the transport row is full at the
            // window's default width, and cut it off.
            tip(
                row![
                    canvas(self.meter.view()).width(meter::WIDTH).height(meter::HEIGHT),
                    // The loudest since play started, red past 0 dBFS.
                    text(self.meter.loudest.map_or("–".to_string(), |l| if l > -48.0 { format!("{l:+.1}") } else { "–".to_string() }))
                        .size(12)
                        .width(34)
                        .color_maybe(self.meter.loudest.filter(|&l| l >= 0.0).map(|_| iced::Color::from_rgb8(0xe5, 0x3e, 0x3e))),
                    // Loudness as the platforms measure it: the last 3 s,
                    // and since play started in the tip.
                    text(self.meter.lufs.short_term().map_or("– LUFS".to_string(), |s| format!("{s:.0} LUFS"))).size(12).width(58),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                text(format!(
                    "The mix as it plays, left and right: peak (faint) and RMS, red past 0 dBFS; the number is the loudest peak since play started, dBFS. LUFS: its loudness over the last 3 s, as YouTube and others measure it (they aim at about −14); since play started: {}. It sums the clip sound and music at their levels, as the export mixes them, whatever the speakers are set to.",
                    self.meter.lufs.integrated().map_or("–".to_string(), |i| format!("{i:.1} LUFS"))
                )),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(TEXT_SMALLER, (self.ui_scale > *settings::UI_SCALE.start() + 1e-3).then_some(Message::UiScale(-1))), text("Make the interface smaller"), tooltip::Position::Bottom),
            tip(
                button(text(format!("{:.0}%", self.ui_scale * 100.0)).size(12)).style(button::text).padding([2, 2]).on_press(Message::UiScale(0)),
                text(format!("Interface size; click for {:.0}%", settings::UI_DEFAULT * 100.0)),
                tooltip::Position::Bottom,
            ),
            tip(icon_button(TEXT_BIGGER, (self.ui_scale < *settings::UI_SCALE.end() - 1e-3).then_some(Message::UiScale(1))), text("Make the interface bigger"), tooltip::Position::Bottom),
            tip(
                icon_button(if self.fullscreen { FULLSCREEN_EXIT } else { FULLSCREEN }, Some(Message::Fullscreen)),
                text(if self.fullscreen { "Leave fullscreen (F)" } else { "Fullscreen (F)" }),
                tooltip::Position::Bottom,
            ),
            status
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill))
        .spacing(12)
        .padding([6, 10])
        .align_y(Alignment::Center);
        top.into()
    }

    /// The picture (or the logo while it decodes) in the aspect lock's
    /// shape, with the panels beside it: a side panel at the right, Info,
    /// Settings or Text at the left, the bin at the far left.
    pub(super) fn picture(&self) -> Element<'_, Message> {
        let view: Element<'_, Message> = match self.held.clone().or_else(|| self.sphere()) {
            Some(sphere) => {
                let (aspect, picking) = (self.aspect, self.picking);
                let handles = self.overlay_handles();
                // A locked aspect is the largest box of that shape that fits.
                responsive(move |room| {
                    let (w, h) = match aspect.ratio() {
                        None => (room.width, room.height),
                        Some(r) => if room.width / room.height > r { (room.height * r, room.height) } else { (room.width, room.width / r) },
                    };
                    let sphere = shader(sphere.clone()).width(w).height(h);
                    // The box's layer is there from the start of picking: adding it
                    // mid-drag would rebuild the view's state and lose the drag.
                    // The handles' layer is always there for the same reason.
                    match picking {
                        Some(area) => center(stack![sphere, canvas(PickBox(area)).width(w).height(h)]).into(),
                        None => center(stack![sphere, canvas(handles.clone()).width(w).height(h)]).into(),
                    }
                })
                .into()
            }
            None => center(column![self.logo(), text("Decoding…")].spacing(16).align_x(Alignment::Center)).into(),
        };
        let view = container(view).width(Length::Fill).height(Length::Fill).style(|_| container::background(iced::Color::BLACK));
        // The mix check stands in the picture's place while it's on.
        let view: Element<'_, Message> = if self.check.on { container(self.mix_check_view()).width(Length::Fill).height(Length::Fill).into() } else { view.into() };
        let view: Element<'_, Message> = match self.side {
            Some(Side::Effects) => row![view, self.effects_panel()].into(),
            Some(Side::Export) => row![view, self.export_panel()].into(),
            Some(Side::Audio) => row![view, self.audio_panel()].into(),
            Some(Side::Record) => row![view, self.record_panel()].into(),
            None => view.into(),
        };
        // A text item's words and look, at the left while it's selected.
        // The Info and Settings panels win the left side over the Text panel
        // while open.
        let view: Element<'_, Message> = match self.info_panel().or_else(|| self.settings_panel()).or_else(|| self.text_panel()) {
            Some(panel) => row![panel, view].into(),
            None => view,
        };
        // The bin, a panel of its own at the far left, beside whichever.
        let view: Element<'_, Message> = row![self.bin_panel(), view].into();
        view
    }

    /// The timeline, built from the edit as it stands.
    pub(super) fn timeline_view(&self) -> Element<'_, Message> {
        let current = self.current_key();
        let tolerance = self.tolerance();
        let offsets = crate::project::offsets(&self.edit.cuts);
        let blocks = self
            .edit
            .cuts
            .iter()
            .zip(offsets)
            .map(|(c, from)| {
                let keys = c.keyframes.iter().enumerate().filter(|(_, k)| k.t >= c.start - tolerance && k.t <= c.end + tolerance).map(|(i, k)| (i, k.t, k.ease, k.follow.is_some())).collect();
                // Only those in the cut, and one either side for its ends.
                let thumbs = self.thumbs.get(&c.input).map_or_else(Vec::new, |all| {
                    let from = all.partition_point(|(t, _)| *t < c.start).saturating_sub(1);
                    let to = (all.partition_point(|(t, _)| *t <= c.end) + 1).min(all.len());
                    all[from..to].to_vec()
                });
                let wave = self.waveforms.get(&cut_wave_file(c)).cloned();
                let even_wave = if c.even { self.even_waves.get(&cut_wave_file(c)).cloned() } else { None };
                let level = level_tag(c.gain, c.mute, filter_tags(c.low_cut, c.clean, c.even));
                // A picture lasts as long as it's trimmed to.
                let most = self.library.iter().find(|m| m.input == c.input).map_or(f64::INFINITY, |m| if m.flat.is_some_and(|f| f.picture) { f64::INFINITY } else { m.duration });
                timeline::Block { labels: cut_labels(c, self.opts.stabilize), from, len: c.end - c.start, start: c.start, most, marks: c.marks.iter().map(|m| (m.t, m.note.clone())).collect(), keys, sound: c.volume.clone(), wave, even_wave, own_db: c.sound_db(), gain: c.gain, fade_in: c.sound_fade_in, fade_out: c.sound_fade_out, level, thumbs, crossfade: c.crossfade, effects: c.effects.iter().map(|k| k.t).collect() }
            })
            .collect();
        let (timeline_start, timeline_span) = self.timeline_window();
        let sounds = SoundTrack::ALL
            .map(|which| {
                let track = self.edit.track(which);
                let blocks = track
                    .clips
                    .iter()
                    .map(|c| timeline::MusicBlock {
                        label: c.input.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
                        from: c.at,
                        start: c.start,
                        len: c.end - c.start,
                        most: self.waveforms.get(&c.input).map_or(f64::INFINITY, |w| w.duration),
                        wave: self.waveforms.get(&c.input).cloned(),
                        fade_in: c.fade_in,
                        fade_out: c.fade_out,
                        volume: c.volume.clone(),
                        even_wave: if c.even { self.even_waves.get(&c.input).cloned() } else { None },
                        level: level_tag(c.gain, c.mute, filter_tags(c.low_cut, c.clean, c.even)),
                    })
                    .collect();
                timeline::SoundLane { which, track: track.clone(), blocks }
            })
            .into_iter()
            .collect();
        let timeline = timeline::Timeline {
            blocks,
            sounds,
            selected_music: self.selection.clip(&self.edit),
            selected_point: self.selection.point(&self.edit),
            listen: self.listen,
            check_marks: self.check_marks(),
            ruler: if self.ruler_large { timeline::RULER_LARGE } else { timeline::RULER_COMPACT },
            folds: self.folds,
            video: self.edit.duration(),
            clips_db: self.edit.sound.volume,
            duration: self.edit.frame(),
            start: timeline_start,
            span: timeline_span,
            position: self.position(),
            selected: self.selection.cut(&self.edit),
            current: current.map(|k| (self.current, k)),
            overlays: self.overlay_blocks(crate::overlay::Track::Overlay),
            texts: self.overlay_blocks(crate::overlay::Track::Text),
            selected_overlay: self.selection.item(&self.edit),
            drag_epoch: self.drag_epoch,
            selected_mark: self.selection.mark(&self.edit),
            step_note: self.step_note.map(|(n, _)| n),
            rows: timeline::Rows::new(self.split, self.framing, self.folds),
        }
        .view();
        timeline
    }

    /// The transport row. Groups: play | cutting | undo | effects | the
    /// Framing switch, and while it's on the view's keyframes and angles;
    /// then the timeline's zoom and the mix.
    pub(super) fn transport_bar<'a>(&'a self, clip: &'a Clip) -> Element<'a, Message> {
        let p = &clip.playback;
        let current = self.current_key();
        let aiming = current.map(|k| Aiming::of(&self.edit.cuts[self.current].keyframes[k].follow));
        let shown_view = self.shown_view();
        let (lo, hi) = shown_view.fov_range();
        // With a song selected, C splits that instead.
        let music = self.selection.clip(&self.edit).and_then(|(which, i)| self.edit.track(which).clips.get(i));
        let point = self.selection.point(&self.edit).is_some();
        // Any cut but the first can crossfade in from the one before.
        let crossfade_cut = Some(self.selection.cut(&self.edit).unwrap_or(self.current)).filter(|&i| i > 0).and_then(|i| self.edit.cuts.get(i));
        let can_split = match music {
            Some(c) => self.position() > c.at + edit::MIN_CUT && self.position() < c.ends() - edit::MIN_CUT,
            None => self.edit.cuts.get(self.current).is_some_and(|c| p.target() > c.start + edit::MIN_CUT && p.target() < c.end - edit::MIN_CUT),
        };
        // Groups: play | cutting | undo | effects | the framing switch, and
        // while it's on the view's keyframes and angles (for a 360 cut also
        // following and colour tracking); then the timeline's zoom, the mix.
        let flat = self.on_flat();
        let framing_tools = self.framing.then(|| {
            row![
                tip(
                    icon_button(KEY_ADD, Some(Message::KeyAdd)),
                    text(if current.is_some() { "Update this keyframe with the view (K)" } else { "Add a keyframe with this view (K)" }),
                    tooltip::Position::Top,
                ),
                tip(icon_button(KEY_DELETE, current.map(|_| Message::KeyDelete)), text("Delete this keyframe (Del)"), tooltip::Position::Top),
                tip(
                    pick_list(&Ease::ALL[..], current.map(|k| self.edit.cuts[self.current].keyframes[k].ease), Message::KeyEase).placeholder("Ease").width(96),
                    text("How this keyframe moves on to the next (J cycles)"),
                    // Below: the list opens upwards and the hint would cover it.
                    tooltip::Position::Bottom,
                ),
                tip(labelled("FOV", slider(lo..=hi, shown_view.fov, Message::Fov).width(110)), text("Field of view: how wide the view is (scroll on the view)"), tooltip::Position::Top),
                tip(icon_button(RESET_VIEW, Some(Message::ResetView)), text("Reset the view: straight ahead, level (0)"), tooltip::Position::Top),
            ]
            .push((!flat).then(|| {
                tip(
                    pick_list(&Aiming::ALL[..], aiming, Message::KeyFollow).placeholder("Follow").width(140),
                    text("What this keyframe looks at: fixed angles, the selfie stick (whoever holds it), or a colour track (pick a new one by drawing a box over the subject)"),
                    tooltip::Position::Bottom,
                )
            }))
            .push((!flat).then(|| {
                tip(
                    icon_button(PICK, (self.clip.is_some() && self.tracking.is_none()).then_some(Message::PickColour)),
                    text("Track a colour: draw a box over a subject (a jacket, a tent); the keyframe here then follows it"),
                    tooltip::Position::Top,
                )
            }))
            .push((!flat).then(|| tip(icon_button(SAVE_SPHERE, (!p.playing).then_some(Message::Save(Saving::Sphere))), text("Save the whole sphere as an equirect PNG (pause first)"), tooltip::Position::Top)))
            .spacing(12)
            .align_y(Alignment::Center)
        });
        let transport = row![
            tip(
                icon_button(if p.playing { PAUSE } else { PLAY }, Some(Message::Toggle)),
                text(if p.playing { "Pause (Space)" } else { "Play (Space)" }),
                tooltip::Position::Top,
            ),
            // A fixed width: the digits' widths differ in most fonts (macOS's
            // system font especially), and the row after it jittered as it ran.
            text(format!("{} / {}", clock(self.position()), clock(self.edit.duration()))).width(170).center(),
            tip(
                icon_button(SAVE_VIEW, (!p.playing).then_some(Message::Save(Saving::View))),
                text("Save this frame as a PNG (pause first)"),
                tooltip::Position::Top,
            ),
            separator(),
            tip(icon_button(SPLIT, can_split.then_some(Message::Split)), text(if music.is_some() { "Split the selected sound at the playhead (C)" } else { "Split the cut at the playhead (C)" }), tooltip::Position::Top),
            tip(
                icon_button(DELETE_CUT, (self.selection != Selection::None).then_some(Message::DeleteCut)),
                text(if point {
                    "Delete the selected volume point (Del)"
                } else if music.is_some() {
                    "Delete the selected sound (Del)"
                } else if self.framing {
                    "Delete the selected cut (Shift+Del)"
                } else {
                    "Delete the selected cut (Del)"
                }),
                tooltip::Position::Top,
            ),
            tip(
                icon_button(CROSSFADE, crossfade_cut.map(|_| Message::ToggleCrossfade)),
                text(match crossfade_cut {
                    Some(c) if c.crossfade > 0.0 => "Take away this cut's crossfade (Shift-drag its start edge to change its length)",
                    _ => "Crossfade into this cut from the one before, 1 s (Shift-drag its start edge to change its length)",
                }),
                tooltip::Position::Top,
            ),
            separator(),
            tip(icon_button(UNDO, Some(Message::Undo)), text("Undo (Ctrl+Z)"), tooltip::Position::Top),
            tip(icon_button(REDO, Some(Message::Redo)), text("Redo (Ctrl+Shift+Z)"), tooltip::Position::Top),
            separator(),
            tip(
                icon_button(EFFECTS, Some(Message::Effect(effects::EffectMsg::Toggle))),
                text(if self.side == Some(Side::Effects) { "Close the Effects panel (G)" } else { "Effects: exposure, colour and tone of the cut, keyframed; fades for the edit (G)" }),
                tooltip::Position::Top,
            ),
            separator(),
            tip(
                toggler(self.framing).label("Framing").on_toggle(Message::Framing),
                text("Framing: the view's keyframes, field of view and angles; for a 360 clip also stabilisation, projection and tracking. Off: editing only, and the view can't be turned by accident."),
                tooltip::Position::Top,
            ),
            tip(
                toggler(self.check.on).label("Mix check").on_toggle(|on| Message::Check(super::mixcheck_ui::CheckMsg::Toggle(on))),
                text("The sound against the export's loudness target and the mixing rules, in the picture's place: each clip as loud as it is, what to fix outlined; a click takes you to it"),
                tooltip::Position::Top,
            ),
        ]
        .push(framing_tools)
        .push(space().width(Length::Fill))
        .push(tip(icon_button(ZOOM_OUT, Some(Message::TimelineZoom(1.0 / 1.5, None))), text("Zoom the timeline out (Ctrl+−, or scroll on it)"), tooltip::Position::Top))
        .push(tip(
            slider(0.0..=1.0, self.timeline_zoom.ln() / self.timeline_most_zoom().ln().max(1e-9), Message::TimelineZoomTo).step(0.001).width(90),
            text("Timeline zoom"),
            tooltip::Position::Top,
        ))
        .push(tip(icon_button(ZOOM_IN, Some(Message::TimelineZoom(1.5, None))), text("Zoom the timeline in (Ctrl+=, or scroll on it)"), tooltip::Position::Top))
        .push(tip(icon_button(ZOOM_FIT, Some(Message::TimelineZoomTo(0.0))), text("Show the whole edit (Ctrl+0)"), tooltip::Position::Top))
        .push(separator())
        .push(tip(
            icon_button(if self.muted { SPEAKER_MUTED } else { SPEAKER }, Some(Message::AudioPanel)),
            text(if self.side == Some(Side::Audio) { "Close the Audio panel" } else { "Audio: listening volume and the mix levels (M mutes)" }),
            tooltip::Position::Top,
        ))
        .spacing(12)
        .padding([4, 10])
        .align_y(Alignment::Center);
        transport.into()
    }
}
