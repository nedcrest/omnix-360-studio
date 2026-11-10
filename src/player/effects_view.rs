//! The Effects panel's look (gap 90, agreed with the user 2026-11-09;
//! tidied in gap 94): folding groups, each with its own reset. A cut's:
//! Light, Colour (with the white-balance picker), Look (LUT and style),
//! Finish (vignette, opacity). A text's or picture's: Text (its words and
//! look), Kind, Place, Show (opacity, reveal, blur), Animate, Look, then
//! Light and Colour folded at first, Finish (vignette); bars have their
//! own. The film's fades are in the Transitions panel. A complex control
//! only lights up in the list; its knobs slide out in a second panel
//! beside it (`effects_extra_panel`), as the Transitions panel's settings
//! do.

use super::effects::{EffectMsg, Focus, LUT, LutChoice, PARAMS, PLACE, Target, WIDTH, label};
use super::{App, Message, icon_button, tip};
use crate::grade::{Grade, grade_at};
use crate::overlay::{Place, place_at};
use iced::widget::{button, column, container, pick_list, row, scrollable, slider, space, text, tooltip};
use iced::{Alignment, Element, Length};

/// An in or an out in its list, by name.
#[derive(Clone, Copy, PartialEq, Eq)]
struct AnimChoice(crate::animate::Anim);

impl std::fmt::Display for AnimChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.name())
    }
}

/// A group as a framed block of the extended view (gap 98).
fn framed(g: Element<'_, Message>) -> Element<'_, Message> {
    container(g)
        .padding(10)
        .width(WIDTH - 24.0)
        .style(|theme: &iced::Theme| container::Style { border: iced::Border { color: theme.extended_palette().background.strong.color, width: 1.0, radius: 6.0.into() }, ..container::Style::default() })
        .into()
}

/// The panel's width in its extended view: three blocks to a row.
const WIDE: f32 = 3.0 * (WIDTH - 24.0) + 2.0 * 10.0 + 24.0;

/// The side panel's width.
const EXTRA_WIDTH: f32 = 220.0;

/// The panel's folding groups, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Scopes,
    Text,
    Kind,
    Place,
    Show,
    Animate,
    Bars,
    Light,
    Colour,
    Look,
    Finish,
}

impl Group {
    /// Its name, shown and kept in the settings while folded.
    pub fn name(self) -> &'static str {
        match self {
            Group::Scopes => "Scopes",
            Group::Text => "Text",
            Group::Kind => "Kind",
            Group::Place => "Place",
            Group::Show => "Show",
            Group::Animate => "Animate",
            Group::Bars => "Bars",
            Group::Light => "Light",
            Group::Colour => "Colour",
            Group::Look => "Look",
            Group::Finish => "Finish",
        }
    }

    /// The effects its ↺ sets back (`Grade::values`), on a text or
    /// picture if `item`: there opacity is in Show, not Finish.
    pub fn params(self, item: bool) -> &'static [usize] {
        use crate::grade::{BLUR, REVEAL};
        match self {
            Group::Light => &[16, 17, 0, 1, 2, 3],
            Group::Colour => &[4, 5, 6, 7],
            Group::Look => &[LUT],
            Group::Finish if item => &[8, 11, 12, 13],
            Group::Finish => &[8, 9, 11, 12, 13],
            Group::Show => &[9, REVEAL, BLUR],
            Group::Scopes | Group::Text | Group::Kind | Group::Place | Group::Animate | Group::Bars => &[],
        }
    }

    /// Where its folding is kept, and whether it starts folded: on a text
    /// or picture, Light and Colour do (rarely set there).
    pub fn fold_key(self, item: bool) -> (String, bool) {
        match self {
            Group::Light | Group::Colour if item => (format!("{} on items", self.name()), true),
            // Folded at first: open, they read the picture back a few
            // times a second.
            Group::Scopes => (self.name().to_owned(), true),
            _ => (self.name().to_owned(), false),
        }
    }
}

/// What the side panel beside the Effects panel holds: a complex
/// control's knobs, lit in the list while they're out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
    Style,
    Vignette,
}

impl App {
    pub(super) fn effects_panel(&self) -> Element<'_, Message> {
        let target = self.effect_target();
        let mut body = column![self.effects_header(target)].spacing(12).padding(12);
        if let Focus::Apart(selected) = self.effect_focus() {
            body = body.push(self.apart_notice(selected));
        }
        let mut groups = vec![self.group(Group::Scopes, self.scopes_view())];
        groups.extend(match target {
            Some((Target::Overlay(i), t)) => self.item_groups(i, t),
            Some((Target::Cut(i), t)) => self.cut_groups(Some((i, t))),
            None => self.cut_groups(None),
        });
        if !self.effects_wide {
            return container(scrollable(body.extend(groups))).width(WIDTH).height(Length::Fill).into();
        }
        // Extended (gap 98): each group a framed block, side by side, as
        // many to a row as fit, for a wide monitor.
        let blocks = iced::widget::row(groups.into_iter().map(framed)).spacing(10).wrap().vertical_spacing(10);
        container(scrollable(body.push(blocks))).width(WIDE).height(Length::Fill).into()
    }

    /// A cut's groups (none: greyed, nothing to edit): Light, Colour (with
    /// the white-balance picker), Look (LUT and style), Finish (vignette,
    /// opacity).
    fn cut_groups(&self, cut: Option<(usize, f64)>) -> Vec<Element<'_, Message>> {
        let grade = cut.map(|(i, t)| self.shown_grade(Target::Cut(i), t)).unwrap_or_default();
        let enabled = cut.is_some();
        let picker = tip(icon_button(super::PICK, enabled.then_some(Message::Effect(EffectMsg::PickWhite))), text("White balance: draw a box over something that should be grey or white (snow, a cloud)"), tooltip::Position::Left);
        let measuring = self.auto_measuring.is_some();
        let auto = tip(
            button(text(if measuring { "…" } else { "Auto" }).size(12)).padding([2, 8]).style(button::secondary).on_press_maybe((enabled && !measuring).then_some(Message::Effect(EffectMsg::AutoColour(true)))),
            text("Black and White from a dozen frames across the cut: its darkest to black, its brightest to white"),
            tooltip::Position::Left,
        );
        let levels = column![row![text("Levels").size(13).width(Length::Fill), auto].align_y(Alignment::Center), self.sliders(&grade, &[16, 17], enabled)].spacing(6);
        let mut groups = vec![
            self.group(Group::Light, column![levels, self.sliders(&grade, &[0, 1, 2, 3], enabled)].spacing(10).into()),
            self.group(Group::Colour, column![row![text("White balance").size(13).width(Length::Fill), picker].align_y(Alignment::Center), self.sliders(&grade, &[4, 5, 6, 7], enabled)].spacing(6).into()),
        ];
        if let Some((i, _)) = cut {
            groups.push(self.group(Group::Look, column![self.lut_controls(i, &grade), self.style_tiles(Target::Cut(i))].spacing(10).into()));
        }
        let finish = column![self.vignette_row(&grade, enabled), self.param_row(9, grade.opacity, enabled)].spacing(6);
        groups.push(self.group(Group::Finish, finish.into()));
        groups
    }

    /// A text's or picture's groups (gap 94): Text (its words and look,
    /// once a panel of their own at the left), Kind (a picture's), Place,
    /// Show (opacity, reveal, blur), Animate (ins and outs), Look (style),
    /// Light and Colour, Finish (the vignette); bars have their own.
    fn item_groups(&self, i: crate::overlay::ItemRef, t: f64) -> Vec<Element<'_, Message>> {
        let Some(it) = self.edit.item(i) else { return Vec::new() };
        let grade = self.shown_grade(Target::Overlay(i), t);
        if let Some(b) = it.bars {
            return vec![self.group(Group::Bars, bars_controls(b)), self.group(Group::Show, self.sliders(&grade, &[9], true))];
        }
        let mut groups = Vec::new();
        if let Some(words) = &it.text {
            groups.push(self.group(Group::Text, self.text_controls(words)));
        }
        if it.file().is_some() {
            groups.push(self.group(Group::Kind, self.kind_tiles(i)));
        }
        groups.extend([
            self.group(Group::Place, self.place_sliders(i, t)),
            self.group(Group::Show, self.sliders(&grade, &[9, crate::grade::REVEAL, crate::grade::BLUR], true)),
            self.group(Group::Animate, self.animate_controls(i)),
            self.group(Group::Look, self.style_tiles(Target::Overlay(i))),
            self.group(Group::Light, self.sliders(&grade, &[16, 17, 0, 1, 2, 3], true)),
            self.group(Group::Colour, self.sliders(&grade, &[4, 5, 6, 7], true)),
            self.group(Group::Finish, self.vignette_row(&grade, true)),
        ]);
        groups
    }

    /// Effects `ps`' rows at their values in `grade`.
    fn sliders(&self, grade: &Grade, ps: &[usize], enabled: bool) -> Element<'static, Message> {
        column(ps.iter().map(|&p| self.param_row(p, grade.values()[p], enabled))).spacing(6).into()
    }

    /// The title, what the panel works on, and its tools: to all cuts,
    /// reset all.
    fn effects_header(&self, target: Option<(Target, f64)>) -> Element<'_, Message> {
        let m = |e: EffectMsg| Message::Effect(e);
        let on_cut = matches!(target, Some((Target::Cut(_), _)));
        let name = match self.effect_focus() {
            Focus::On(tg, _) if self.selection == super::Selection::None => format!("{} (under the playhead)", self.target_name(tg)),
            Focus::On(tg, _) | Focus::Apart(tg) => self.target_name(tg),
            Focus::Nothing => "No cut".to_owned(),
        };
        let close = tip(button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(m(EffectMsg::Toggle)), text("Close (G)"), tooltip::Position::Bottom);
        let to_all = on_cut.then(|| tip(icon_button(super::TO_ALL_CUTS, Some(m(EffectMsg::ToAll))), text("To all cuts: give every other cut this look, replacing their effects"), tooltip::Position::Bottom));
        let any = target.filter(|&(tg, _)| !self.effects_of(tg).is_empty() || !self.automation_of(tg).is_empty() || matches!(tg, Target::Overlay(i) if self.edit.item(i).is_some_and(|it| !it.motion.is_empty())));
        let measuring = self.auto_measuring.is_some();
        let auto = on_cut.then(|| {
            let press = (!measuring).then_some(m(EffectMsg::AutoColour(false)));
            tip(button(text(if measuring { "Measuring…" } else { "Auto" }).size(13)).padding([2, 8]).style(button::secondary).on_press_maybe(press), text("Auto colour: levels, exposure and white balance from a dozen frames across the cut, careful and one undo step"), tooltip::Position::Bottom)
        });
        let reset = tip(button(text("↺ All").size(13)).padding([2, 8]).style(button::secondary).on_press_maybe(any.map(|_| m(EffectMsg::Reset(None)))), text(if on_cut { "Reset: remove this cut's effects and automation" } else { "Reset: remove this item's effects, place, automation and ins and outs" }), tooltip::Position::Bottom);
        let wide = tip(iced::widget::toggler(self.effects_wide).label("Extended view").text_size(12).on_toggle(|on| Message::Effect(EffectMsg::Wide(on))), text("The groups as framed blocks side by side, for a wide monitor"), tooltip::Position::Bottom);
        column![row![text("Effects").size(18).width(Length::Fill), wide, close].spacing(10).align_y(Alignment::Center), row![text(name).size(13).width(Length::Fill)].push(auto).push(to_all).push(reset).spacing(6).align_y(Alignment::Center)].spacing(6).into()
    }

    /// What `target` is called in the panel: «Cut 3 · name», «Text · words».
    fn target_name(&self, target: Target) -> String {
        match target {
            Target::Cut(i) => format!("Cut {} · {}", i + 1, super::clip_name(&self.edit.cuts[i].input)),
            Target::Overlay(i) => match self.edit.item(i) {
                Some(it) if it.text.is_some() => format!("Text · {}", it.name()),
                Some(it) if it.bars.is_some() => it.name(),
                Some(it) => format!("Overlay · {}", super::clip_name(&it.input)),
                None => String::new(),
            },
        }
    }

    /// The notice while the playhead is off the selected clip (gap 93):
    /// nothing is edited until it's there.
    fn apart_notice(&self, selected: Target) -> Element<'static, Message> {
        let what = match selected {
            Target::Cut(_) => "cut",
            Target::Overlay(_) => "item",
        };
        let go = button(text("Go to it").size(13)).padding([3, 10]).style(button::secondary).on_press(Message::Effect(EffectMsg::GoToSelected));
        let words = text(format!("The playhead isn't on the selected {what}. Nothing here is edited until it is: make sure which clip you're editing.")).size(13).color(iced::Color::from_rgb(0.12, 0.08, 0.0));
        container(column![words, go].spacing(6))
            .padding(10)
            .width(Length::Fill)
            .style(|_| container::Style { background: Some(iced::Color::from_rgb(0.98, 0.74, 0.25).into()), border: iced::Border { radius: 4.0.into(), ..Default::default() }, ..Default::default() })
            .into()
    }

    /// A group: its name folding it open or shut, its ↺, and `content`
    /// while it's open.
    fn group<'a>(&self, g: Group, content: Element<'a, Message>) -> Element<'a, Message> {
        let item = matches!(self.effect_target(), Some((Target::Overlay(_), _)));
        let (key, at_first) = g.fold_key(item);
        let folded = self.effects_folded.contains(&key) != at_first;
        let fold = button(text(format!("{} {}", if folded { "▸" } else { "▾" }, g.name())).size(15)).padding([2, 0]).style(button::text).on_press(Message::Effect(EffectMsg::Fold(key.clone())));
        let resets = !g.params(item).is_empty() || matches!(g, Group::Kind | Group::Place | Group::Animate | Group::Look);
        let reset = tip(button(text("↺").size(13)).padding([0, 5]).style(button::text).on_press_maybe(resets.then_some(Message::Effect(EffectMsg::ResetGroup(g)))), text(format!("{} back to none", g.name())), tooltip::Position::Left);
        let head = row![fold, space().width(Length::Fill), reset].align_y(Alignment::Center);
        if folded { head.into() } else { column![head, content].spacing(8).into() }
    }

    /// Effect `p`'s slider with its value and ↺.
    fn param_row(&self, p: usize, v: f64, enabled: bool) -> Element<'static, Message> {
        let ((name, hint), (lo, hi)) = (PARAMS[p], crate::grade::RANGES[p]);
        let reset = button(text("↺").size(13)).padding([0, 5]).style(button::text).on_press_maybe((enabled && v != crate::grade::NEUTRAL[p]).then_some(Message::Effect(EffectMsg::Reset(Some(p)))));
        let control = slider(lo..=hi, v, move |v| Message::Effect(EffectMsg::Set(p, v))).step(0.01).on_release(Message::Effect(EffectMsg::Done));
        let lane = self.lane_button(super::automation_ui::Param::Grade(p));
        tip(column![row![text(name).width(Length::Fill), text(label(p, v)).size(13), reset, lane].spacing(4).align_y(Alignment::Center), control].spacing(2), text(hint), tooltip::Position::Left)
    }

    /// The ◆ beside a value: its automation lane, lit while it shows (gap 39).
    fn lane_button(&self, p: super::automation_ui::Param) -> Element<'static, Message> {
        let laned = self.lane_owner().is_some_and(|of| self.lanes_of(of).contains(&p));
        tip(
            button(text("◆").size(11)).padding([0, 4]).style(if laned { button::primary } else { button::text }).on_press(Message::Effect(EffectMsg::AutoLane(p))),
            text(if laned { "Its automation lane shows under the Effects row" } else { "Automate it: a lane under the Effects row, to draw clips on" }),
            tooltip::Position::Left,
        )
    }

    /// The vignette's slider, and its shape's button lit while its knobs
    /// are out beside.
    fn vignette_row(&self, grade: &Grade, enabled: bool) -> Element<'static, Message> {
        let out = self.effect_extra == Some(Extra::Vignette);
        let shape = crate::grade::VIGNETTES[(grade.vignette_shape.round() as usize).min(crate::grade::VIGNETTES.len() - 1)];
        let more = button(text(format!("{shape} ▸")).size(12)).padding([3, 8]).style(if out { button::primary } else { button::secondary });
        let more = more.on_press_maybe((enabled && grade.vignette > 0.0).then_some(Message::Effect(EffectMsg::Extra((!out).then_some(Extra::Vignette)))));
        column![self.param_row(crate::grade::VIGNETTE, grade.vignette, enabled), tip(more, text("Its shape, reach and softness, beside"), tooltip::Position::Left)].spacing(4).into()
    }

    /// The overlay's place: across, down, size, rotation.
    fn place_sliders(&self, i: crate::overlay::ItemRef, t: f64) -> Element<'static, Message> {
        let place = self.edit.item(i).map_or_else(Place::default, |it| crate::automation::placed(place_at(&it.motion, t), &it.automation, t));
        let rows = PLACE.iter().enumerate().map(|(p, (name, lo, hi, hint))| {
            let v = place.values()[p];
            let shown = if p == 3 { format!("{v:+.0}°") } else { format!("{v:.2}") };
            let reset = button(text("↺").size(13)).padding([0, 5]).style(button::text).on_press_maybe((v != Place::default().values()[p]).then_some(Message::Effect(EffectMsg::ResetPlace(p))));
            let control = slider(*lo..=*hi, v, move |v| Message::Effect(EffectMsg::Place(p, v))).step(if p == 3 { 1.0 } else { 0.01 }).on_release(Message::Effect(EffectMsg::Done));
            let lane = self.lane_button(super::automation_ui::Param::Place(p));
            tip(column![row![text(*name).width(Length::Fill), text(shown).size(13), reset, lane].spacing(4).align_y(Alignment::Center), control].spacing(2), text(*hint), tooltip::Position::Left)
        });
        column(rows).spacing(6).into()
    }

    /// A picture's kind (gap 87, step 2c): plain, or one of `dress::Kind`.
    fn kind_tiles(&self, i: crate::overlay::ItemRef) -> Element<'static, Message> {
        use crate::dress::Kind;
        let now = self.edit.item(i).and_then(|it| it.kind);
        let tile = |kind: Option<Kind>| {
            let style = if now == kind { button::primary } else { button::secondary };
            button(text(kind.map_or("Plain", Kind::name)).size(12)).padding([3, 7]).style(style).on_press(Message::Effect(EffectMsg::Kind(kind))).into()
        };
        let tiles = std::iter::once(tile(None)).chain(Kind::ALL.map(|k| tile(Some(k))));
        tip(
            iced::widget::row(tiles).spacing(4).wrap().vertical_spacing(4),
            text("How it's dressed: a sticker's white cut-out edge, a polaroid's card, a picture in picture's rounded border; each with a shadow"),
            tooltip::Position::Left,
        )
    }

    /// The item's in and out (gap 87, step 2a): a list each (gap 94), ✕, how long.
    fn animate_controls(&self, i: crate::overlay::ItemRef) -> Element<'static, Message> {
        use crate::animate::Anim;
        use crate::model::ops::End;
        let m = |e: EffectMsg| Message::Effect(e);
        let made = |edge: End| self.edit.item(i).and_then(|it| it.automation.iter().find_map(|c| c.animation.filter(|a| a.edge == edge)));
        let side = |edge: End, title: &'static str| {
            let now = made(edge);
            let off = button(text("✕").size(12)).padding([1, 6]).style(button::text).on_press_maybe(now.map(|_| m(EffectMsg::Animate(edge, None))));
            let list = pick_list(Anim::ALL.map(AnimChoice).to_vec(), now.map(|a| AnimChoice(a.anim)), move |c: AnimChoice| m(EffectMsg::Animate(edge, Some(c.0)))).placeholder("None").text_size(13).width(Length::Fill);
            let mut col = column![row![text(title).width(36), list, off].spacing(6).align_y(Alignment::Center)].spacing(4);
            if let Some(a) = now {
                col = col.push(column![
                    row![text("Length").size(13).width(Length::Fill), text(format!("{:.1} s", a.seconds)).size(13)],
                    slider(0.2..=3.0, a.seconds, move |s| Message::Effect(EffectMsg::AnimateLength(edge, s))).step(0.1).on_release(m(EffectMsg::Done)),
                ]
                .spacing(2));
            }
            col
        };
        tip(
            column![side(End::In, "In"), side(End::Out, "Out")].spacing(12),
            text("How it comes on and goes off: made into automation clips on its Effects and Motion lanes, kept on its ends as it's trimmed"),
            tooltip::Position::Left,
        )
    }

    /// Cut `i`'s LUT and its strength.
    fn lut_controls(&self, i: usize, grade: &Grade) -> Element<'static, Message> {
        let cut = &self.edit.cuts[i];
        let mut choices = vec![LutChoice::None];
        choices.extend(self.luts.iter().map(|n| LutChoice::Named(n.clone())));
        let current = LutChoice::of(cut.lut.as_ref());
        if !choices.contains(&current) {
            choices.push(current.clone());
        }
        let d_log_m = self.library.iter().find(|m| m.input == cut.input).is_some_and(|m| m.color_mode == crate::dji::ColorMode::DLogM);
        let hint = if self.luts.is_empty() {
            "Add LUTs (.cube) in Settings: DJI's D-Log M to Rec.709 for D-Log M footage, or any look"
        } else {
            "A LUT turns the picture's colours into others before the effects: DJI's D-Log M to Rec.709 makes D-Log M footage look normal"
        };
        let mut col = column![
            row![text("LUT").width(Length::Fill), text(if d_log_m { "filmed in D-Log M" } else { "" }).size(13)].align_y(Alignment::Center),
            tip(pick_list(choices, Some(current), |c| Message::Effect(EffectMsg::Lut(c))).text_size(13).width(Length::Fill), text(hint), tooltip::Position::Left),
        ]
        .spacing(4);
        if cut.lut.is_some() {
            col = col.push(self.param_row(LUT, grade.lut, true));
        }
        col.into()
    }

    /// Cut `i`'s style tiles: the one chosen lit, a press on it again
    /// bringing its knobs out beside.
    fn style_tiles(&self, target: Target) -> Element<'static, Message> {
        use crate::transition::Look;
        let style = self.style_of(target).map(|s| s.look);
        let m = |e: EffectMsg| Message::Effect(e);
        let off = tip(button(text("✕").size(12)).padding([1, 6]).style(button::text).on_press_maybe(style.map(|_| m(EffectMsg::Style(None)))), text("No style"), tooltip::Position::Left);
        let looks = Look::STYLES.into_iter().map(|l| (l, m(EffectMsg::Style(Some(l)))));
        let mut col = column![row![text("Style").width(Length::Fill), off].align_y(Alignment::Center), super::transitions_panel::tiles(looks, style)].spacing(6);
        // Its settings beside, as the vignette's shape opens its own.
        if let Some(look) = style {
            let out = self.effect_extra == Some(Extra::Style);
            let more = button(text(format!("{} ▸", look.name())).size(12)).padding([3, 8]).style(if out { button::primary } else { button::secondary });
            col = col.push(tip(more.on_press(m(EffectMsg::Extra((!out).then_some(Extra::Style)))), text("Its settings, beside"), tooltip::Position::Left));
        }
        col.into()
    }

    /// The film's fades (in the Transitions panel).
    pub(super) fn whole_edit(&self) -> Element<'static, Message> {
        let fades = &self.edit.fades;
        let fade = |name: &'static str, v: f64, f: fn(f64) -> EffectMsg, hint: &'static str| {
            tip(
                column![
                    row![text(name).width(Length::Fill), text(if v > 0.0 { format!("{v:.1} s") } else { "none".to_owned() }).size(13)],
                    slider(0.0..=5.0, v, move |v| Message::Effect(f(v))).step(0.1).on_release(Message::Effect(EffectMsg::Done)),
                ]
                .spacing(2),
                text(hint),
                tooltip::Position::Left,
            )
        };
        column![
            fade("Fade in", fades.fade_in, EffectMsg::FadeIn, "From black at the start of the film, the sound with it"),
            fade("Fade out", fades.fade_out, EffectMsg::FadeOut, "To black at the end of the film, the sound with it"),
        ]
        .spacing(8)
        .into()
    }

    /// Beside the Effects panel while a complex control's knobs are out:
    /// the cut's or item's style's settings, or the vignette's shape, reach and
    /// softness.
    pub(super) fn effects_extra_panel(&self) -> Element<'_, Message> {
        let m = |e: EffectMsg| Message::Effect(e);
        let target = self.effect_target();
        let close = button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(m(EffectMsg::Extra(None)));
        let body = match (self.effect_extra, target) {
            (Some(Extra::Style), Some((tg, _))) => {
                let Some(s) = self.style_of(tg) else { return space().into() };
                let mut col = column![row![text(s.look.name()).size(16).width(Length::Fill), close].align_y(Alignment::Center)].spacing(14);
                for (k, &v) in s.look.knobs().iter().zip(&s.spec().values) {
                    col = col.push(super::transitions_panel::knob(k, v, |key, v| Message::Effect(EffectMsg::StyleKnob(key, v)), m(EffectMsg::Done)));
                }
                let reset = button(text("Reset").size(13)).style(button::secondary).on_press_maybe((!s.params.is_empty()).then(|| m(EffectMsg::StyleReset)));
                col.push(tip(reset, text("Every setting back to the style's own"), tooltip::Position::Top))
            }
            (Some(Extra::Vignette), Some((tg, t))) => {
                let grade = grade_at(self.effects_of(tg), t);
                column![row![text("Vignette").size(16).width(Length::Fill), close].align_y(Alignment::Center), vignette_controls(&grade)].spacing(14)
            }
            _ => return space().into(),
        };
        container(scrollable(body.padding(12))).width(EXTRA_WIDTH).height(Length::Fill).into()
    }
}

/// Bars' ratio (scope, 2:1, flat), how long they slide, their colour.
fn bars_controls(b: crate::overlay::Bars) -> Element<'static, Message> {
    let set = |bars, press| Message::Effect(EffectMsg::Bars(bars, press));
    let pick = |label: String, on: bool, bars| button(text(label).size(12)).padding([3, 8]).style(if on { button::primary } else { button::secondary }).on_press(set(bars, true)).into();
    let ratios = crate::overlay::BAR_RATIOS.map(|r| pick(format!("{r:.2}:1"), (b.ratio - r).abs() < 1e-6, crate::overlay::Bars { ratio: r, ..b }));
    let colours = [("Black", [0.0; 3]), ("White", [1.0; 3])].map(|(name, c)| pick(name.to_owned(), b.colour == c, crate::overlay::Bars { colour: c, ..b }));
    let slide = column![
        row![text("Slide").size(13).width(Length::Fill), text(format!("{:.1} s", b.slide)).size(13)],
        slider(0.0..=3.0, b.slide, move |s| set(crate::overlay::Bars { slide: s, ..b }, false)).step(0.1).on_release(Message::Effect(EffectMsg::Done)),
    ]
    .spacing(2);
    tip(
        column![iced::widget::row(ratios).spacing(4), iced::widget::row(colours).spacing(4), slide].spacing(8),
        text("The picture's shape between the bars, their colour, and how long they slide in at the item's start and out at its end; Opacity in Finish"),
        tooltip::Position::Left,
    )
}

/// The vignette's shape (gap 6) and how far in and how softly it reaches.
fn vignette_controls(grade: &Grade) -> Element<'static, Message> {
    let m = |e: EffectMsg| Message::Effect(e);
    let shapes = crate::grade::VIGNETTES.iter().enumerate().map(|(i, &name)| {
        let on = grade.vignette_shape.round() as usize == i;
        button(text(name).size(12)).padding([3, 8]).style(if on { button::primary } else { button::secondary }).on_press(m(EffectMsg::Set(crate::grade::SHAPE, i as f64))).into()
    });
    let slider_of = |name: &'static str, p: usize, v: f64| {
        column![row![text(name).size(13).width(Length::Fill), text(format!("{v:.2}")).size(13)], slider(0.0..=1.0, v, move |v| Message::Effect(EffectMsg::Set(p, v))).step(0.01).on_release(m(EffectMsg::Done))].spacing(2)
    };
    tip(
        column![iced::widget::row(shapes).spacing(4).wrap().vertical_spacing(4), slider_of(PARAMS[crate::grade::SHAPE + 1].0, crate::grade::SHAPE + 1, grade.vignette_reach), slider_of(PARAMS[crate::grade::SHAPE + 2].0, crate::grade::SHAPE + 2, grade.vignette_soft)].spacing(8),
        text("Round, or from the top (a graduated filter on a bright sky), the bottom or the sides; how far in it reaches, how soft its edge"),
        tooltip::Position::Left,
    )
}

impl App {
    /// Whether the scopes show (gap 69): the Effects panel open, their
    /// group unfolded.
    pub(super) fn scopes_on(&self) -> bool {
        let (key, at_first) = Group::Scopes.fold_key(false);
        self.side == Some(super::Side::Effects) && self.effects_folded.contains(&key) == at_first
    }

    /// The scopes: a histogram (red, green, blue, and luma in white) over a
    /// luma waveform, and the picture's darkest and brightest.
    fn scopes_view(&self) -> Element<'static, Message> {
        let Ok(mut feed) = self.scope.lock() else { return text("…").into() };
        let Some(s) = feed.latest.clone() else {
            return text(if feed.unreadable { "The scopes read 8-bit screens only" } else { "…" }).size(12).into();
        };
        let image = feed.waveform.get_or_insert_with(|| waveform_image(&s)).clone();
        drop(feed);
        let wave = iced::widget::image(image).width(Length::Fill).height(110).content_fit(iced::ContentFit::Fill);
        let pct = |v: f64| format!("{:.1} %", v * 100.0);
        let said = text(format!("Black {} · White {} · crushed {} · blown {}", s.black, s.white, pct(s.crushed), pct(s.blown))).size(12);
        let hint = "The cut's picture as graded (its LUT and style on, overlays apart). Black and White: its darkest and brightest of 255, specks aside; crushed at 2 and under, blown at 253 and over";
        tip(column![iced::widget::canvas(Histogram(s.clone())).width(Length::Fill).height(80), wave, said].spacing(4), text(hint), tooltip::Position::Left)
    }
}

/// The waveform as a picture: a column per stretch across the frame, luma
/// up, brighter where more pixels are.
fn waveform_image(s: &crate::scopes::Scopes) -> iced::widget::image::Handle {
    use crate::scopes::{COLUMNS, LEVELS};
    let most = s.waveform.iter().copied().max().unwrap_or(1).max(1) as f32;
    let mut rgba = vec![0u8; COLUMNS * LEVELS * 4];
    for c in 0..COLUMNS {
        for l in 0..LEVELS {
            let n = s.waveform[c * LEVELS + l];
            // A square root: a few pixels show, a crowd doesn't drown them.
            let a = if n == 0 { 0.0 } else { (n as f32 / most).sqrt().max(0.15) };
            let i = ((LEVELS - 1 - l) * COLUMNS + c) * 4;
            rgba[i..i + 4].copy_from_slice(&[(120.0 + 135.0 * a) as u8, 255, (120.0 + 135.0 * a) as u8, (a * 255.0) as u8]);
        }
    }
    iced::widget::image::Handle::from_rgba(COLUMNS as u32, LEVELS as u32, rgba)
}

/// The histogram's drawing.
struct Histogram(std::sync::Arc<crate::scopes::Scopes>);

impl iced::widget::canvas::Program<Message> for Histogram {
    type State = ();

    fn draw(&self, _: &(), renderer: &iced::Renderer, _: &iced::Theme, bounds: iced::Rectangle, _: iced::mouse::Cursor) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path, Stroke};
        use iced::{Color, Point};
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        frame.fill(&Path::rectangle(Point::ORIGIN, bounds.size()), Color::from_rgba(0.0, 0.0, 0.0, 0.35));
        let s = &self.0;
        // Each scaled to the busiest level of any, by square root.
        let most = s.histogram.iter().flat_map(|c| c.iter()).copied().max().unwrap_or(1).max(1) as f32;
        let colours = [Color::from_rgb(1.0, 0.3, 0.3), Color::from_rgb(0.3, 1.0, 0.3), Color::from_rgb(0.4, 0.6, 1.0), Color::WHITE];
        for (channel, colour) in s.histogram.iter().zip(colours) {
            let line = Path::new(|p| {
                for (l, &n) in channel.iter().enumerate() {
                    let pt = Point::new(l as f32 / 255.0 * w, h - (n as f32 / most).sqrt() * (h - 2.0));
                    if l == 0 { p.move_to(pt) } else { p.line_to(pt) }
                }
            });
            frame.stroke(&line, Stroke::default().with_width(1.0).with_color(Color { a: 0.8, ..colour }));
        }
        vec![frame.into_geometry()]
    }
}
