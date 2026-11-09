//! The Effects panel: the playing cut's colour effects at the playhead as
//! sliders (or, with an overlay selected, that overlay's place and effects),
//! and the edit's fades.
//!
//! Moving a slider sets that effect for the whole cut or item
//! (`KeyOp::EffectValue`): it makes no keyframe (the user, 2026-11-09).
//! Animating an effect is the automation lanes' (gap 39); keys from
//! before still play, and a slider moved sets its effect in all of them.

use super::{App, Message, Selection, tip};
use crate::grade::EffectKey;
use crate::overlay::{Place, place_at};
use crate::model::ops::keys::{EXACT, Graded, KeyOp, Line};
use crate::model::ops::sound::SoundOp;
use crate::model::ops::{End, Op, items::ItemOp};
use crate::text::{Colour, Outline, Text, TextAlign};
use iced::widget::{button, column, container, pick_list, row, scrollable, slider, text, tooltip};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub enum EffectMsg {
    /// Open or close the panel.
    Toggle,
    /// Effect `i` (as in `PARAMS`) moved to this value.
    Set(usize, f64),
    /// A slider let go: the next move is a change of its own for undo.
    Done,
    /// Effect `i` back to none here, or the cut's effects all gone.
    Reset(Option<usize>),
    /// The selected overlay's place value `i` (as in `PLACE`) moved to this.
    Place(usize, f64),
    /// Its place value `i` back to the default here.
    ResetPlace(usize),
    /// Its place dragged on the view (`handles`).
    Drag(Place),
    /// The selected text overlay's words edited, and its look.
    TextEdit(iced::widget::text_editor::Action),
    Text(TextChange),
    /// The look at the playhead on every cut.
    ToAll,
    /// Draw a box over something grey or white to set the white balance.
    PickWhite,
    FadeIn(f64),
    FadeOut(f64),
    /// The cut's style look (none: off), one of its settings moved (a
    /// slider's drag one undo step, closed by `Done`), all of them back.
    Style(Option<crate::transition::Look>),
    StyleKnob(&'static str, f32),
    StyleReset,
    /// The selected bars' ratio, slide and colour (a press made at once, a
    /// slider's drag one undo step closed by `Done`).
    Bars(crate::overlay::Bars, bool),
    /// Jump to cut `cut`'s effect keyframe at clip time `t`.
    Jump(usize, f64),
    /// Cut `cut`'s effect keyframe `key` dragged to clip time `t`.
    KeyMove(usize, usize, f64),
    /// The cut's LUT (how much of it is effect `LUT`).
    Lut(LutChoice),
    /// A group of the panel folded or opened; set back to none.
    Fold(super::effects_view::Group),
    ResetGroup(super::effects_view::Group),
    /// A complex control's knobs out beside the panel, or put away.
    Extra(Option<super::effects_view::Extra>),
    /// A value's automation lane shown (its Effects row opened) or, if
    /// nothing moves it, taken away (gap 39), for the panel's target.
    AutoLane(super::automation_ui::Param),
}

/// A LUT in a list: none, or a library name (or a file a project names).
#[derive(Clone, Debug, PartialEq)]
pub enum LutChoice {
    None,
    Named(String),
}

impl LutChoice {
    pub fn of(name: Option<&String>) -> Self {
        name.map_or(LutChoice::None, |n| LutChoice::Named(n.clone()))
    }

    pub fn name(&self) -> Option<String> {
        match self {
            LutChoice::None => None,
            LutChoice::Named(n) => Some(n.clone()),
        }
    }
}

impl std::fmt::Display for LutChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LutChoice::None => f.write_str("None"),
            // A file a project names: its file name.
            LutChoice::Named(n) => f.write_str(std::path::Path::new(n).file_name().map_or(n.as_str(), |s| s.to_str().unwrap_or(n))),
        }
    }
}

/// Each effect as the panel shows it: its name and what it does (the first
/// eleven of `Grade::values`; their ranges `grade::RANGES`).
pub(super) const PARAMS: [(&str, &str); 11] = [
    ("Exposure", "Brighter or darker, in stops, like the camera's EV"),
    ("Contrast", "More or less contrast, around mid-grey"),
    ("Highlights", "Pull back (−) or lift the bright parts, such as the sky"),
    ("Shadows", "Lift (+) or deepen the dark parts"),
    ("Temperature", "Cooler (−) or warmer"),
    ("Tint", "Greener (−) or more magenta"),
    ("Saturation", "Less or more colour; −1 is black and white"),
    ("Vibrance", "More colour mostly where there is little, sparing skin and sky"),
    ("Vignette", "Darker edges: round, or from the top, the bottom or the sides"),
    ("Opacity", "How much of the picture shows; on the main track it fades towards black"),
    ("Strength", "How much of the LUT: 100 % is all of it, less mixes in the picture as filmed"),
];

/// The LUT's strength among `Grade::values` (after `PARAMS`), drawn
/// beside the LUT picker.
pub(super) const LUT: usize = 10;

/// The panel's width.
pub(super) const WIDTH: f32 = 300.0;

/// A slider's value as shown: stops for exposure, else a signed amount.
pub(super) fn label(i: usize, v: f64) -> String {
    match i {
        0 => format!("{v:+.2} EV"),
        8 => format!("{v:.2}"),
        9 | 10 => format!("{:.0} %", v * 100.0),
        _ => format!("{v:+.2}"),
    }
}

/// A change to a text overlay's look.
#[derive(Debug, Clone)]
pub enum TextChange {
    Font(String),
    Size(f64),
    Colour(Colour),
    Bold(bool),
    Align(TextAlign),
    Outline(bool),
    OutlineWidth(f64),
    OutlineColour(Colour),
    Shadow(bool),
}

/// The colours a text's swatches offer.
const SWATCHES: [[u8; 3]; 9] = [[255, 255, 255], [0, 0, 0], [255, 214, 10], [255, 140, 0], [230, 40, 40], [255, 90, 170], [40, 120, 255], [30, 200, 110], [130, 130, 130]];

/// What the panel edits: the playing cut, or the overlay selected (in the
/// lane shown).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Target {
    Cut(usize),
    Overlay(crate::overlay::ItemRef),
}

/// Where one drag of a control writes: the target and its time, taken
/// when the drag starts and kept until it's let go. Read afresh at each
/// step, a drag during playback made a key per frame (gap 38).
#[derive(Default)]
pub(super) struct Pin(Option<(Target, f64)>);

impl Pin {
    /// The drag's target: `now` if it is `starting` (or nothing was
    /// pinned yet), else the one it started with.
    fn at(&mut self, starting: bool, now: Option<(Target, f64)>) -> Option<(Target, f64)> {
        if starting || self.0.is_none() {
            self.0 = now;
        }
        self.0
    }
}

/// Where an overlay is, as sliders: its name, range and what it does.
pub(super) const PLACE: [(&str, f64, f64, &str); 4] = [
    ("Across", 0.0, 1.0, "Where its centre is, from the left edge to the right"),
    ("Down", 0.0, 1.0, "Where its centre is, from the top to the bottom"),
    ("Size", 0.05, 2.0, "1 fills the frame as far as its shape allows"),
    ("Rotation", -180.0, 180.0, "Degrees, clockwise"),
];

impl App {
    /// What the panel edits and the time there in its own clock: the
    /// overlay selected, if it's showing at the playhead, else the playing
    /// cut.
    pub(super) fn effect_target(&self) -> Option<(Target, f64)> {
        let t = self.position();
        if let Some(i) = self.selection.item(&self.edit)
            && let Some(item) = self.edit.item(i)
            && t >= item.at
            && t <= item.ends()
        {
            return Some((Target::Overlay(i), item.local(t)));
        }
        self.playing_cut().map(|(_, t)| (Target::Cut(self.current), t))
    }

    pub(super) fn effects_of(&self, target: Target) -> &[EffectKey] {
        match target {
            Target::Cut(i) => &self.edit.cuts[i].effects,
            Target::Overlay(i) => &self.edit.item(i).expect("the selected overlay").effects,
        }
    }

    /// `target`'s automation clips (gap 39).
    pub(super) fn automation_of(&self, target: Target) -> &[crate::automation::AutoClip] {
        match target {
            Target::Cut(i) => &self.edit.cuts[i].automation,
            Target::Overlay(i) => &self.edit.item(i).expect("the selected overlay").automation,
        }
    }

    /// What `target` looks like at its time `t`, its automation over its
    /// values: what the panel's sliders show (the user, 2026-11-10).
    pub(super) fn shown_grade(&self, target: Target, t: f64) -> crate::grade::Grade {
        crate::automation::graded(crate::grade::grade_at(self.effects_of(target), t), self.automation_of(target), t)
    }

    /// The value named `param` moved by `target`'s automation at `t`, if a
    /// clip covers it there: a slider then sets that clip's point there.
    fn automated(&self, target: Target, param: &str, t: f64) -> bool {
        crate::automation::value(self.automation_of(target), param, t).is_some()
    }


    /// The target and time a drag writes to (`Pin`), if it's still there:
    /// an undo mid-drag can take the overlay or cut away.
    fn pinned(&mut self, starting: bool) -> Option<(Target, f64)> {
        let now = self.effect_target();
        let (target, t) = self.effect_pin.at(starting, now)?;
        let there = match target {
            Target::Cut(i) => i < self.edit.cuts.len(),
            Target::Overlay(i) => self.edit.item(i).is_some(),
        };
        there.then_some((target, t))
    }

    /// Effect `p` (as in `Grade::values`) at `v` on the target the drag
    /// started on, for the whole of it (`KeyOp::EffectValue`): an edit sets
    /// the value, it doesn't make a key (user, 2026-11-09).
    /// Where an automation clip moves it at the playhead, it sets the
    /// clip's point there instead (made if there's none).
    fn set_effect(&mut self, starting: bool, p: usize, v: f64) {
        let Some((target, t)) = self.pinned(starting) else { return };
        let (on, param) = (self.graded(target), crate::grade::NAMES[p].to_owned());
        if self.automated(target, &param, t) {
            self.op(Op::Auto(crate::model::ops::auto::AutoOp::Point { on, param, t, v, from: None, tolerance: self.tolerance() }));
            return;
        }
        self.op(Op::Key(KeyOp::EffectValue { on, param, value: v }));
    }

    /// The selected overlay's place value `p` (as in `Place::values`) at
    /// `v` for the whole item (`KeyOp::PlaceValue`), on the item the drag
    /// started on: no motion key (user, 2026-11-09).
    fn set_place(&mut self, starting: bool, p: usize, v: f64) {
        let Some((target @ Target::Overlay(i), t)) = self.pinned(starting) else { return };
        let Some(item) = self.edit.item(i).map(|it| it.id) else { return };
        let param = Place::NAMES[p].to_owned();
        if self.automated(target, &param, t) {
            let on = crate::model::ops::keys::Graded::Item(item);
            self.op(Op::Auto(crate::model::ops::auto::AutoOp::Point { on, param, t, v, from: None, tolerance: self.tolerance() }));
            return;
        }
        self.op(Op::Key(KeyOp::PlaceValue { item, param, value: v }));
    }

    /// Applies `ops` as one undo step, showing a refusal.
    pub(super) fn ops(&mut self, ops: &[Op]) {
        if let Err((_, e)) = self.edit.batch(ops) {
            self.error = Some(e.to_string());
        }
    }

    /// The id of what `target` names, for a key op.
    fn graded(&self, target: Target) -> Graded {
        match target {
            Target::Cut(i) => Graded::Cut(self.edit.cuts[i].id),
            Target::Overlay(r) => Graded::Item(self.edit.item(r).map_or(0, |it| it.id)),
        }
    }

    pub(super) fn effect_message(&mut self, m: EffectMsg) {
        let target = self.effect_target();
        match m {
            EffectMsg::Toggle => self.toggle_side(super::Side::Effects),
            EffectMsg::Set(p, v) => {
                let starting = !self.edit.changing();
                self.edit.begin();
                self.set_effect(starting, p, v);
                // A shape is a press, not a drag: its own undo step at once.
                if p == crate::grade::SHAPE {
                    self.edit.commit();
                }
            }
            EffectMsg::Place(p, v) => {
                let starting = !self.edit.changing();
                self.edit.begin();
                self.set_place(starting, p, v);
            }
            EffectMsg::Drag(to) => {
                let starting = !self.edit.changing();
                self.edit.begin();
                // Each value it changed, for the whole item.
                let Some((Target::Overlay(i), t)) = self.pinned(starting) else { return };
                let Some(it) = self.edit.item(i) else { return };
                let (was, now) = (crate::automation::placed(place_at(&it.motion, t), &it.automation, t).values(), to.values());
                for p in (0..4).filter(|&p| was[p] != now[p]) {
                    self.set_place(false, p, now[p]);
                }
            }
            EffectMsg::Done => self.edit.commit(),
            EffectMsg::Fold(g) => {
                let name = g.name().to_owned();
                if self.effects_folded.contains(&name) {
                    self.effects_folded.retain(|n| *n != name);
                } else {
                    self.effects_folded.push(name);
                }
            }
            EffectMsg::ResetGroup(g) => self.reset_group(g),
            EffectMsg::Extra(extra) => self.effect_extra = extra,
            EffectMsg::AutoLane(p) => {
                use super::automation_ui::LaneOf;
                let Some(of) = self.lane_owner() else { return };
                if self.extra_lanes.contains(&(of, p)) {
                    self.extra_lanes.retain(|&q| q != (of, p));
                } else {
                    self.extra_lanes.push((of, p));
                    *match of {
                        LaneOf::Cuts => &mut self.folds.effects,
                        LaneOf::Track(crate::overlay::Track::Overlay) => &mut self.folds.overlay_effects,
                        LaneOf::Track(crate::overlay::Track::Text) => &mut self.folds.text_effects,
                    } = false;
                }
            }
            EffectMsg::Bars(bars, press) => {
                let Some(item) = self.selection.item(&self.edit).and_then(|i| self.edit.item(i)).map(|it| it.id) else { return };
                self.edit.begin();
                self.op(Op::Item(crate::model::ops::items::ItemOp::SetBars { item, bars }));
                if press {
                    self.edit.commit();
                }
            }
            m @ (EffectMsg::Style(_) | EffectMsg::StyleKnob(..) | EffectMsg::StyleReset) => {
                let Some((Target::Cut(i), _)) = target else { return };
                self.on_style(i, m);
            }
            EffectMsg::Lut(choice) => {
                let Some((Target::Cut(i), _)) = target else { return };
                let cut = self.edit.cuts[i].id;
                self.op(crate::model::ops::Op::Cut(crate::model::ops::cuts::CutOp::Lut { cut, lut: choice.name() }));
                // Its file read here, in the player: the core only names it.
                if let Err(e) = self.edit.cuts[i].resolve_lut() {
                    self.error = Some(format!("{e:#}"));
                }
            }
            EffectMsg::TextEdit(action) => {
                let edits = action.is_edit();
                self.text_content.perform(action);
                let words = self.text_content.text().trim_end_matches('\n').to_owned();
                if edits && let Some((item, text)) = self.selected_text().filter(|(_, t)| t.content != words) {
                    // A run of typing is one undo step, until something else is done.
                    self.edit.begin();
                    self.op(Op::Item(ItemOp::SetText { item, text: Text { content: words, ..text } }));
                }
            }
            EffectMsg::Text(change) => {
                let Some((item, mut text)) = self.selected_text() else { return };
                // Sliders are one undo step a drag, the rest one a click.
                if matches!(change, TextChange::Size(_) | TextChange::OutlineWidth(_)) {
                    self.edit.begin();
                }
                match change {
                    TextChange::Font(f) => text.font = f,
                    TextChange::Size(v) => text.size = v,
                    TextChange::Colour(c) => text.colour = c,
                    TextChange::Bold(b) => text.bold = b,
                    TextChange::Align(a) => text.align = a,
                    TextChange::Outline(on) => text.outline = on.then_some(Outline { colour: Colour([0, 0, 0]), width: 0.08 }),
                    TextChange::OutlineWidth(w) => {
                        if let Some(o) = &mut text.outline {
                            o.width = w;
                        }
                    }
                    TextChange::OutlineColour(c) => {
                        if let Some(o) = &mut text.outline {
                            o.colour = c;
                        }
                    }
                    TextChange::Shadow(on) => text.shadow = on,
                }
                self.op(Op::Item(ItemOp::SetText { item, text }));
            }
            EffectMsg::Reset(Some(p)) => self.set_effect(true, p, crate::grade::NEUTRAL[p]),
            EffectMsg::ResetPlace(p) => self.set_place(true, p, Place::default().values()[p]),
            EffectMsg::Reset(None) => {
                let Some((target, _)) = target else { return };
                let on = self.graded(target);
                self.op(Op::Key(KeyOp::Clear { on }));
            }
            EffectMsg::ToAll => {
                let Some((_, t)) = self.playing_cut() else { return };
                let cut = self.edit.cuts[self.current].id;
                self.op(Op::Key(KeyOp::LookToAll { cut, t }));
            }
            EffectMsg::PickWhite => {
                if self.picking.take().is_none() && self.clip.is_some() {
                    self.with_playback(super::Playback::pause);
                    self.picking = Some(None);
                    self.picking_white = true;
                    self.save_status = Some("Draw a box over something that should be grey or white (Esc cancels)".to_owned());
                } else {
                    self.picking_white = false;
                    self.save_status = None;
                }
            }
            EffectMsg::FadeIn(seconds) => {
                self.edit.begin();
                self.op(Op::Sound(SoundOp::EditFade { end: End::In, seconds }));
            }
            EffectMsg::FadeOut(seconds) => {
                self.edit.begin();
                self.op(Op::Sound(SoundOp::EditFade { end: End::Out, seconds }));
            }
            EffectMsg::Jump(cut, t) => {
                self.follow = true;
                self.selection = Selection::cut_at(&self.edit, cut);
                self.goto(cut, t, false);
            }
            EffectMsg::KeyMove(cut, key, t) => {
                self.follow = true;
                let Some((id, from)) = self.edit.cuts.get(cut).and_then(|c| Some((c.id, c.effects.get(key)?.t))) else { return };
                self.edit.begin();
                let gap = 1.0 / self.fps();
                self.op(Op::Key(KeyOp::Retime { line: Line::Effects(Graded::Cut(id)), from, t, gap, tolerance: EXACT }));
                if let Some(t) = self.edit.cuts[cut].effects.get(key).map(|k| k.t) {
                    self.scrub_to(cut, t);
                }
            }
        }
    }

    /// The selected item's id and text, if it's a text item.
    fn selected_text(&self) -> Option<(crate::project::Id, Text)> {
        let it = self.edit.item(self.selection.item(&self.edit)?)?;
        Some((it.id, it.text.clone()?))
    }

    /// The text panel, at the player's left while a text item is selected:
    /// its words and look, wherever the playhead is (where it is and its
    /// effects stay in the Effects panel).
    pub(super) fn text_panel(&self) -> Option<Element<'_, Message>> {
        let i = self.selection.item(&self.edit)?;
        let words = self.edit.item(i)?.text.as_ref()?;
        let t = |c: TextChange| Message::Effect(EffectMsg::Text(c));
        let swatches = |chosen: Colour, f: fn(Colour) -> TextChange| {
            let mut r = row![].spacing(4);
            for c in SWATCHES {
                let colour = iced::Color::from_rgb8(c[0], c[1], c[2]);
                let on = chosen.0 == c;
                r = r.push(
                    button(iced::widget::space().width(16).height(16))
                        .padding(0)
                        .style(move |theme: &iced::Theme, _| button::Style {
                            background: Some(colour.into()),
                            border: iced::Border { color: if on { theme.extended_palette().primary.strong.color } else { iced::Color::from_rgba(0.5, 0.5, 0.5, 0.6) }, width: if on { 2.5 } else { 1.0 }, radius: 3.0.into() },
                            ..button::Style::default()
                        })
                        .on_press(Message::Effect(EffectMsg::Text(f(Colour(c))))),
                );
            }
            r
        };
        let families: Vec<FontChoice> = crate::text::families().iter().map(|f| FontChoice(f.clone())).collect();
        let close = tip(
            button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(Message::OverlaySelect(None)),
            text("Close (deselects the text)"),
            tooltip::Position::Bottom,
        );
        let mut controls = column![
            row![text("Text").size(18).width(Length::Fill), close].align_y(Alignment::Center),
            iced::widget::text_editor(&self.text_content).placeholder("Type the text here").height(80).on_action(|a| Message::Effect(EffectMsg::TextEdit(a))),
            row![
                tip(
                    pick_list(families, Some(FontChoice(words.font.clone())), |f| Message::Effect(EffectMsg::Text(TextChange::Font(f.0)))).width(Length::Fill),
                    text("Built-in fonts come with Omnix, so the text looks the same on any machine; the others are this system's, and where a project's font is missing the text falls back to Inter."),
                    tooltip::Position::Bottom,
                ),
                iced::widget::checkbox(words.bold).label("Bold").on_toggle(move |b| Message::Effect(EffectMsg::Text(TextChange::Bold(b)))),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![text("Size").width(Length::Fill), text(format!("{:.0} %", words.size * 100.0)).size(13)],
            slider(0.02..=0.3, words.size, move |v| Message::Effect(EffectMsg::Text(TextChange::Size(v)))).step(0.005).on_release(Message::Effect(EffectMsg::Done)),
            row![text("Lines").width(Length::Fill), pick_list(&TextAlign::ALL[..], Some(words.align), move |a| t(TextChange::Align(a))).width(110)].align_y(Alignment::Center),
            text("Colour"),
            swatches(words.colour, TextChange::Colour),
            iced::widget::checkbox(words.shadow).label("Shadow").on_toggle(move |b| Message::Effect(EffectMsg::Text(TextChange::Shadow(b)))),
            iced::widget::checkbox(words.outline.is_some()).label("Outline").on_toggle(move |b| Message::Effect(EffectMsg::Text(TextChange::Outline(b)))),
        ]
        .spacing(6);
        if let Some(o) = words.outline {
            controls = controls.push(row![text("Outline width").width(Length::Fill), text(format!("{:.0} %", o.width * 100.0)).size(13)]);
            controls = controls.push(slider(0.01..=0.3, o.width, move |v| Message::Effect(EffectMsg::Text(TextChange::OutlineWidth(v)))).step(0.005).on_release(Message::Effect(EffectMsg::Done)));
            controls = controls.push(swatches(o.colour, TextChange::OutlineColour));
        }
        Some(container(scrollable(controls.spacing(8).padding(12))).width(WIDTH).height(Length::Fill).into())
    }

    /// Neutralises the playing cut's colour cast from `rgb`, the median of
    /// something meant to be grey or white.
    pub(super) fn white_balance(&mut self, rgb: [f64; 3]) {
        let (temperature, tint) = crate::grade::white_balance(rgb);
        // The picked colour is the cut's, whatever the panel shows.
        if self.selection.item(&self.edit).is_some() {
            self.selection = Selection::None;
        }
        self.edit.begin();
        self.set_effect(true, 4, temperature);
        self.set_effect(false, 5, tint);
        self.edit.commit();
    }

    /// Cut `i`'s LUT, and its strength in `grade` (the effects at the
    /// playhead): the LUT acts before the effects, which act on what it gives.
    /// Whose lanes the panel's target has: the cuts', or its track's.
    pub(super) fn lane_owner(&self) -> Option<super::automation_ui::LaneOf> {
        use super::automation_ui::LaneOf;
        match self.effect_target()? {
            (Target::Cut(_), _) => Some(LaneOf::Cuts),
            (Target::Overlay((track, _)), _) => Some(LaneOf::Track(track)),
        }
    }

    /// Group `g`'s effects back to none (an overlay's place, the Look's
    /// style off too): one undo step.
    fn reset_group(&mut self, g: super::effects_view::Group) {
        use super::effects_view::Group;
        let Some((target, _)) = self.effect_target() else { return };
        self.edit.begin();
        for &p in g.params() {
            self.set_effect(true, p, crate::grade::NEUTRAL[p]);
        }
        if g == Group::Place {
            for p in 0..4 {
                self.set_place(true, p, Place::default().values()[p]);
            }
        }
        if let (Group::Look, Target::Cut(i)) = (g, target) {
            let cut = self.edit.cuts[i].id;
            self.op(Op::Cut(crate::model::ops::cuts::CutOp::Style { cut, look: None }));
        }
        self.edit.commit();
    }

    /// Cut `i`'s style look: set, a setting moved, all set back.
    fn on_style(&mut self, i: usize, message: EffectMsg) {
        use crate::model::ops::cuts::CutOp;
        let c = &self.edit.cuts[i];
        let (cut, style) = (c.id, c.style.clone());
        match message {
            // A press on the chosen one again brings its knobs out or puts
            // them away; another is chosen with its knobs out.
            EffectMsg::Style(look) => {
                use super::effects_view::Extra;
                if look.is_some() && look == style.as_ref().map(|s| s.look) {
                    self.effect_extra = (self.effect_extra != Some(Extra::Style)).then_some(Extra::Style);
                    return;
                }
                self.op(Op::Cut(CutOp::Style { cut, look }));
                self.effect_extra = look.map(|_| Extra::Style);
            }
            EffectMsg::StyleKnob(key, value) => {
                let choice = style.and_then(|s| s.look.knob(key)).is_some_and(|k| !k.choices.is_empty());
                self.edit.begin();
                self.op(Op::Cut(CutOp::StyleKnob { cut, key: key.to_owned(), value: Some(value) }));
                if choice {
                    self.edit.commit();
                }
            }
            EffectMsg::StyleReset => {
                self.edit.begin();
                for key in style.iter().flat_map(|s| s.params.keys()) {
                    self.op(Op::Cut(CutOp::StyleKnob { cut, key: key.clone(), value: None }));
                }
                self.edit.commit();
            }
            _ => {}
        }
    }
}

/// A font family in the text panel's list, the built-in ones marked.
#[derive(Clone, PartialEq)]
struct FontChoice(String);

impl std::fmt::Display for FontChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if crate::text::BUILT_IN.contains(&self.0.as_str()) { write!(f, "{} · built in", self.0) } else { f.write_str(&self.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_value_the_panel_draws_has_a_row_and_a_range() {
        // The LUT's strength too: drawn by the same row, past the ten.
        assert_eq!(PARAMS.len(), LUT + 1);
        let neutral = crate::grade::NEUTRAL;
        assert!(crate::grade::RANGES.iter().zip(neutral).all(|(&(lo, hi), v)| lo <= v && v <= hi));
    }

    #[test]
    fn a_drag_during_playback_writes_one_key() {
        let mut pin = Pin::default();
        // The playhead moves a frame on between the slider's steps.
        let step = |pin: &mut Pin, starting: bool, t: f64| pin.at(starting, Some((Target::Cut(0), t)));
        assert!(step(&mut pin, true, 4.90) == Some((Target::Cut(0), 4.90)));
        for t in [4.93, 4.97, 5.00, 5.03] {
            assert!(step(&mut pin, false, t) == Some((Target::Cut(0), 4.90)), "still the drag's start at {t}");
        }
        // The next drag starts where the playhead is then.
        assert!(step(&mut pin, true, 8.50) == Some((Target::Cut(0), 8.50)));
    }

    #[test]
    fn a_drag_begun_off_any_cut_pins_once_one_is_there() {
        let mut pin = Pin::default();
        assert!(pin.at(true, None).is_none());
        assert!(pin.at(false, Some((Target::Cut(1), 2.0))) == Some((Target::Cut(1), 2.0)));
        assert!(pin.at(false, Some((Target::Cut(1), 2.1))) == Some((Target::Cut(1), 2.0)));
    }
}
