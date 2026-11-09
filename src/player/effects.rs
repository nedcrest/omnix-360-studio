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
use iced::widget::{button, column, pick_list, row, slider, text, tooltip};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub enum EffectMsg {
    /// Open or close the panel.
    Toggle,
    /// Levels set from the scopes' measure (gap 14): the picture's darkest
    /// to black, its brightest to white.
    AutoLevels,
    /// The playhead onto the selected clip, the panel's to edit (gap 93).
    GoToSelected,
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
    /// A group of the panel folded or opened, by its key (`Group::fold_key`).
    Fold(String),
    ResetGroup(super::effects_view::Group),
    /// A complex control's knobs out beside the panel, or put away.
    Extra(Option<super::effects_view::Extra>),
    /// The selected picture's kind (`dress::Kind`), or plain.
    Kind(Option<crate::dress::Kind>),
    /// The selected item's in or out (`End`): an animation or none; its
    /// length (a slider's drag, one undo step).
    Animate(crate::model::ops::End, Option<crate::animate::Anim>),
    AnimateLength(crate::model::ops::End, f64),
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
pub(super) const PARAMS: [(&str, &str); 18] = [
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
    ("Shape", "The vignette's shape: round, from the top, the bottom or the sides"),
    ("Reach", "How far in the vignette reaches"),
    ("Softness", "How soft the vignette's inner edge is"),
    ("Blur", "How soft it's drawn; at 100 % past reading"),
    ("Reveal", "How much has come on: a picture wiped in from the left, text typed letter by letter"),
    ("Black", "The level that becomes black, of 255: the picture's darkest (Scopes' Black) makes its black black"),
    ("White", "The level that becomes white, of 255: the picture's brightest makes its white white"),
];

/// The LUT's strength among `Grade::values` (a row of `PARAMS`), drawn
/// beside the LUT picker.
pub(super) const LUT: usize = 10;

/// The panel's width.
pub(super) const WIDTH: f32 = 300.0;

/// A slider's value as shown: stops for exposure, else a signed amount.
pub(super) fn label(i: usize, v: f64) -> String {
    match i {
        0 => format!("{v:+.2} EV"),
        8 => format!("{v:.2}"),
        9 | 10 | 14 | 15 => format!("{:.0} %", v * 100.0),
        16 | 17 => format!("{:.0}", v * 255.0),
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

/// What the panel edits: a cut, or a text or picture (in the lane shown).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Target {
    Cut(usize),
    Overlay(crate::overlay::ItemRef),
}

/// Whether the panel may edit (gap 93, the user, 2026-11-10): the selected
/// cut or item with the playhead on it (its ends counting), and the time
/// there in its own clock; the selection with the playhead off it, edited
/// only once it's there; with nothing selected, the cut under the
/// playhead.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Focus {
    On(Target, f64),
    Apart(Target),
    Nothing,
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

/// A selected cut or item: where it starts on the edit, how long, and its
/// own clock's start.
#[derive(Clone, Copy)]
struct Selected {
    target: Target,
    at: f64,
    len: f64,
    start: f64,
}

/// The panel's focus at edit time `t` (`Focus`): on `selected` while `t`
/// is on it, its ends counting; else apart; with nothing selected, the
/// `playing` cut (its index and clip time).
fn focus(t: f64, selected: Option<Selected>, playing: Option<(usize, f64)>) -> Focus {
    match selected {
        Some(s) if (s.at..=s.at + s.len).contains(&t) => Focus::On(s.target, s.start + t - s.at),
        Some(s) => Focus::Apart(s.target),
        None => playing.map_or(Focus::Nothing, |(i, t)| Focus::On(Target::Cut(i), t)),
    }
}

impl App {
    /// What the panel works on (`Focus`).
    pub(super) fn effect_focus(&self) -> Focus {
        let selected = match (self.selection.item(&self.edit), self.selection.cut(&self.edit)) {
            (Some(i), _) => self.edit.item(i).map(|it| Selected { target: Target::Overlay(i), at: it.at, len: it.end - it.start, start: it.start }),
            (None, Some(i)) => {
                let c = &self.edit.cuts[i];
                Some(Selected { target: Target::Cut(i), at: self.edit.edit_time(i, c.start), len: c.len(), start: c.start })
            }
            (None, None) => None,
        };
        focus(self.position(), selected, self.playing_cut().map(|(_, t)| (self.current, t)))
    }

    /// What the panel edits and the time there in its own clock, while it
    /// may (`effect_focus`).
    pub(super) fn effect_target(&self) -> Option<(Target, f64)> {
        match self.effect_focus() {
            Focus::On(target, t) => Some((target, t)),
            Focus::Apart(_) | Focus::Nothing => None,
        }
    }

    /// The playhead onto `target`: its nearer end, a frame inside.
    fn go_to(&mut self, target: Target) {
        let t = self.position();
        let frame = 1.0 / self.fps();
        let span = match target {
            Target::Cut(i) => {
                let c = &self.edit.cuts[i];
                (self.edit.edit_time(i, c.start), self.edit.edit_time(i, c.end))
            }
            Target::Overlay(i) => match self.edit.item(i) {
                Some(it) => (it.at, it.ends()),
                None => return,
            },
        };
        let to = if t < span.0 { span.0 } else { (span.1 - frame).max(span.0) };
        if let Some((i, clip_t)) = self.edit.locate(to) {
            let playing = self.playing();
            self.goto(i, clip_t, playing);
        }
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
            EffectMsg::AutoLevels => self.auto_levels(),
            EffectMsg::GoToSelected => {
                if let Focus::Apart(target) = self.effect_focus() {
                    self.go_to(target);
                }
            }
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
            EffectMsg::Fold(name) => {
                if self.effects_folded.contains(&name) {
                    self.effects_folded.retain(|n| *n != name);
                } else {
                    self.effects_folded.push(name);
                }
            }
            EffectMsg::ResetGroup(g) => self.reset_group(g),
            EffectMsg::Animate(edge, anim) => self.animate(edge, anim, None),
            EffectMsg::Kind(kind) => self.dress(kind),
            EffectMsg::AnimateLength(edge, seconds) => {
                self.edit.begin();
                self.animate(edge, None, Some(seconds));
            }
            EffectMsg::Extra(extra) => self.effect_extra = extra,
            EffectMsg::AutoLane(p) => {
                use super::automation_ui::LaneOf;
                let Some(of) = self.lane_owner() else { return };
                if self.extra_lanes.contains(&(of, p)) {
                    self.extra_lanes.retain(|&q| q != (of, p));
                } else {
                    self.extra_lanes.push((of, p));
                    // Its row opened: a place value's is the Motion row.
                    let motion = matches!(p, super::automation_ui::Param::Place(_));
                    *match (of, motion) {
                        (LaneOf::Cuts, _) => &mut self.folds.effects,
                        (LaneOf::Track(crate::overlay::Track::Overlay, _), false) => &mut self.folds.overlay_effects,
                        (LaneOf::Track(crate::overlay::Track::Overlay, _), true) => &mut self.folds.overlay_motion,
                        (LaneOf::Track(crate::overlay::Track::Text, _), false) => &mut self.folds.text_effects,
                        (LaneOf::Track(crate::overlay::Track::Text, _), true) => &mut self.folds.text_motion,
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
                let Some((target, _)) = target else { return };
                self.on_style(target, m);
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
            EffectMsg::Reset(Some(p)) => {
                self.edit.begin();
                self.unautomate(vec![crate::grade::NAMES[p].to_owned()]);
                self.set_effect(true, p, crate::grade::NEUTRAL[p]);
                self.edit.commit();
            }
            EffectMsg::ResetPlace(p) => {
                self.edit.begin();
                self.unautomate(vec![Place::NAMES[p].to_owned()]);
                self.set_place(true, p, Place::default().values()[p]);
                self.edit.commit();
            }
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
    pub(super) fn text_controls(&self, words: &crate::text::Text) -> Element<'_, Message> {
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
        let mut controls = column![
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
            row![text("Align").width(Length::Fill), pick_list(&TextAlign::ALL[..], Some(words.align), move |a| t(TextChange::Align(a))).width(110)].align_y(Alignment::Center),
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
        controls.spacing(8).into()
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
            (Target::Overlay((track, l, _)), _) => Some(LaneOf::Track(track, l)),
        }
    }

    /// The selected item's `edge` animated by `anim` (none with a length:
    /// the one there, that long), its length kept or half a second.
    fn animate(&mut self, edge: crate::model::ops::End, anim: Option<crate::animate::Anim>, seconds: Option<f64>) {
        let Some(it) = self.selection.item(&self.edit).and_then(|i| self.edit.item(i)) else { return };
        let now = it.automation.iter().find_map(|c| c.animation.filter(|a| a.edge == edge));
        let anim = if seconds.is_some() { now.map(|a| a.anim) } else { anim };
        let seconds = seconds.or(now.map(|a| a.seconds)).unwrap_or(0.5);
        let item = it.id;
        self.op(Op::Auto(crate::model::ops::auto::AutoOp::Animate { item, edge, anim, seconds }));
    }

    /// `names`' automation clips on the panel's target taken away, so a
    /// reset sets the value itself (ins and outs among them).
    fn unautomate(&mut self, names: Vec<String>) {
        let Some((target, _)) = self.effect_target() else { return };
        let on = self.graded(target);
        if self.automation_of(target).iter().any(|c| names.contains(&c.param)) {
            self.op(Op::Auto(crate::model::ops::auto::AutoOp::Clear { on, params: names }));
        }
    }

    /// The cut's levels from what the scopes measured of it: its darkest
    /// to black, its brightest to white. The measure is of the picture as
    /// graded, so the levels it already has are undone first; pressed
    /// again, it settles.
    fn auto_levels(&mut self) {
        use crate::grade::{BLACK_IN, WHITE_IN};
        let Some((target @ Target::Cut(_), t)) = self.effect_target() else { return };
        let Some(s) = self.scope.lock().ok().and_then(|f| f.latest.clone()) else { return };
        let g = self.shown_grade(target, t);
        let (b, span) = (g.black_in, g.white_in - g.black_in);
        let at = |level: u8| b + level as f64 / 255.0 * span;
        self.edit.begin();
        self.set_effect(true, BLACK_IN, at(s.black).clamp(0.0, 0.5));
        self.set_effect(false, WHITE_IN, at(s.white).clamp(0.5, 1.0));
        self.edit.commit();
    }

    /// The selected picture dressed as `kind`, or plain.
    fn dress(&mut self, kind: Option<crate::dress::Kind>) {
        let Some(it) = self.selection.item(&self.edit).and_then(|i| self.edit.item(i)) else { return };
        if it.kind != kind {
            let item = it.id;
            self.op(Op::Item(crate::model::ops::items::ItemOp::Kind { item, kind }));
        }
    }

    /// Group `g`'s effects back to none, their automation gone (an
    /// overlay's place, the Look's style off too): one undo step.
    fn reset_group(&mut self, g: super::effects_view::Group) {
        use super::effects_view::Group;
        let Some((target, _)) = self.effect_target() else { return };
        let params = g.params(matches!(target, Target::Overlay(_)));
        self.edit.begin();
        self.unautomate(params.iter().map(|&p| crate::grade::NAMES[p].to_owned()).collect());
        for &p in params {
            self.set_effect(true, p, crate::grade::NEUTRAL[p]);
        }
        if g == Group::Place {
            self.unautomate(Place::NAMES.iter().map(|n| n.to_string()).collect());
            for p in 0..4 {
                self.set_place(true, p, Place::default().values()[p]);
            }
        }
        if g == Group::Kind {
            self.dress(None);
        }
        if g == Group::Animate {
            for edge in [crate::model::ops::End::In, crate::model::ops::End::Out] {
                self.animate(edge, None, None);
            }
        }
        if g == Group::Look && self.style_of(target).is_some() {
            self.op(self.style_op(target, None));
        }
        self.edit.commit();
    }

    /// Cut `i`'s style look: set, a setting moved, all set back.
    /// The style on `target`, a cut's or a text's or picture's.
    pub(super) fn style_of(&self, target: Target) -> Option<&crate::project::Style> {
        match target {
            Target::Cut(i) => self.edit.cuts.get(i)?.style.as_ref(),
            Target::Overlay(i) => self.edit.item(i)?.style.as_ref(),
        }
    }

    /// `target`'s style to `look` (none: off).
    fn style_op(&self, target: Target, look: Option<crate::transition::Look>) -> Op {
        use crate::model::ops::{cuts::CutOp, items::ItemOp};
        match target {
            Target::Cut(i) => Op::Cut(CutOp::Style { cut: self.edit.cuts[i].id, look }),
            Target::Overlay(i) => Op::Item(ItemOp::Style { item: self.edit.item(i).map_or(0, |it| it.id), look }),
        }
    }

    /// One of `target`'s style's settings to `value` (none: its own).
    fn style_knob_op(&self, target: Target, key: String, value: Option<f32>) -> Op {
        use crate::model::ops::{cuts::CutOp, items::ItemOp};
        match target {
            Target::Cut(i) => Op::Cut(CutOp::StyleKnob { cut: self.edit.cuts[i].id, key, value }),
            Target::Overlay(i) => Op::Item(ItemOp::StyleKnob { item: self.edit.item(i).map_or(0, |it| it.id), key, value }),
        }
    }

    fn on_style(&mut self, target: Target, message: EffectMsg) {
        let style = self.style_of(target).cloned();
        match message {
            // A style chosen comes with its knobs out (its «Name ▸» button
            // puts them away and back); the chosen one again changes nothing.
            EffectMsg::Style(look) => {
                use super::effects_view::Extra;
                if look.is_some() && look == style.as_ref().map(|s| s.look) {
                    return;
                }
                self.op(self.style_op(target, look));
                self.effect_extra = look.map(|_| Extra::Style);
            }
            EffectMsg::StyleKnob(key, value) => {
                let choice = style.and_then(|s| s.look.knob(key)).is_some_and(|k| !k.choices.is_empty());
                self.edit.begin();
                self.op(self.style_knob_op(target, key.to_owned(), Some(value)));
                if choice {
                    self.edit.commit();
                }
            }
            EffectMsg::StyleReset => {
                self.edit.begin();
                for key in style.iter().flat_map(|s| s.params.keys()) {
                    self.op(self.style_knob_op(target, key.clone(), None));
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
    fn the_panel_edits_the_selection_only_with_the_playhead_on_it() {
        let item = Selected { target: Target::Overlay((crate::overlay::Track::Text, 0, 0)), at: 10.0, len: 4.0, start: 1.0 };
        assert!(focus(12.0, Some(item), Some((3, 5.0))) == Focus::On(item.target, 3.0), "on it, its own clock");
        assert!(focus(14.0, Some(item), None) == Focus::On(item.target, 5.0), "its end counts");
        assert!(focus(15.0, Some(item), Some((3, 5.0))) == Focus::Apart(item.target), "off it: not the cut under the playhead");
        assert!(focus(15.0, None, Some((3, 5.0))) == Focus::On(Target::Cut(3), 5.0), "nothing selected: the playing cut");
    }

    #[test]
    fn every_value_the_panel_draws_has_a_row_and_a_range() {
        assert_eq!(PARAMS.len(), crate::grade::NAMES.len());
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
