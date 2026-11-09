//! The Effects panel's look (gap 90, agreed with the user 2026-11-09):
//! folding groups, each with its own reset — Place or Bars for an overlay,
//! then Light, Colour (with the white-balance picker), Look (LUT and
//! style), Finish (vignette, opacity), Whole edit (the film's fades,
//! folded at first). A complex control only lights up in the list; its
//! knobs slide out in a second panel beside it (`effects_extra_panel`),
//! as the Transitions panel's settings do.

use super::effects::{EffectMsg, LUT, LutChoice, PARAMS, PLACE, Target, WIDTH, label};
use super::{App, Message, icon_button, tip};
use crate::grade::{Grade, grade_at};
use crate::overlay::{Place, place_at};
use iced::widget::{button, column, container, pick_list, row, scrollable, slider, space, text, tooltip};
use iced::{Alignment, Element, Length};

/// The side panel's width.
const EXTRA_WIDTH: f32 = 220.0;

/// The panel's folding groups, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Place,
    Bars,
    Light,
    Colour,
    Look,
    Finish,
    WholeEdit,
}

impl Group {
    /// Its name, shown and kept in the settings while folded.
    pub fn name(self) -> &'static str {
        match self {
            Group::Place => "Place",
            Group::Bars => "Bars",
            Group::Light => "Light",
            Group::Colour => "Colour",
            Group::Look => "Look",
            Group::Finish => "Finish",
            Group::WholeEdit => "Whole edit",
        }
    }

    /// The effects its ↺ sets back (`Grade::values`).
    pub fn params(self) -> &'static [usize] {
        match self {
            Group::Light => &[0, 1, 2, 3],
            Group::Colour => &[4, 5, 6, 7],
            Group::Look => &[LUT],
            Group::Finish => &[8, 9, 11, 12, 13],
            Group::Place | Group::Bars | Group::WholeEdit => &[],
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
        let grade = target.map(|(tg, t)| self.shown_grade(tg, t)).unwrap_or_default();
        let enabled = target.is_some();
        let sliders = |ps: &[usize]| column(ps.iter().map(|&p| self.param_row(p, grade.values()[p], enabled))).spacing(6).into();
        let mut body = column![self.effects_header(target)].spacing(12).padding(12);
        match target {
            Some((Target::Overlay(i), t)) => match self.edit.item(i).and_then(|it| it.bars) {
                Some(b) => body = body.push(self.group(Group::Bars, bars_controls(b))),
                None => body = body.push(self.group(Group::Place, self.place_sliders(i, t))),
            },
            Some((Target::Cut(_), _)) | None => {}
        }
        let on_cut = matches!(target, Some((Target::Cut(_), _)));
        body = body.push(self.group(Group::Light, sliders(&[0, 1, 2, 3])));
        let picker = tip(icon_button(super::PICK, on_cut.then(|| Message::Effect(EffectMsg::PickWhite))), text("White balance: draw a box over something that should be grey or white (snow, a cloud)"), tooltip::Position::Left);
        body = body.push(self.group(Group::Colour, column![row![text("White balance").size(13).width(Length::Fill), picker].align_y(Alignment::Center), sliders(&[4, 5, 6, 7])].spacing(6).into()));
        if let Some((Target::Cut(i), _)) = target {
            body = body.push(self.group(Group::Look, column![self.lut_controls(i, &grade), self.style_tiles(i)].spacing(10).into()));
        }
        // An overlay's opacity is what's set most on it.
        let finish = if on_cut { [8, 9] } else { [9, 8] };
        let finish = column(finish.map(|p| if p == 8 { self.vignette_row(&grade, enabled) } else { self.param_row(p, grade.values()[p], enabled) })).spacing(6);
        body = body.push(self.group(Group::Finish, finish.into()));
        body = body.push(self.group(Group::WholeEdit, self.whole_edit()));
        container(scrollable(body)).width(WIDTH).height(Length::Fill).into()
    }

    /// The title, what the panel works on, and its tools: to all cuts,
    /// reset all.
    fn effects_header(&self, target: Option<(Target, f64)>) -> Element<'_, Message> {
        let m = |e: EffectMsg| Message::Effect(e);
        let on_cut = matches!(target, Some((Target::Cut(_), _)));
        let name = match target {
            Some((Target::Cut(i), _)) => format!("Cut {} · {}", i + 1, super::clip_name(&self.edit.cuts[i].input)),
            Some((Target::Overlay(i), _)) => match self.edit.item(i) {
                Some(it) if it.text.is_some() => format!("Text · {}", it.name()),
                Some(it) if it.bars.is_some() => it.name(),
                Some(it) => format!("Overlay · {}", super::clip_name(&it.input)),
                None => String::new(),
            },
            None => "No cut".to_owned(),
        };
        let close = tip(button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(m(EffectMsg::Toggle)), text("Close (G)"), tooltip::Position::Bottom);
        let to_all = tip(icon_button(super::TO_ALL_CUTS, on_cut.then(|| m(EffectMsg::ToAll))), text("To all cuts: give every other cut this look, replacing their effects"), tooltip::Position::Bottom);
        let any = target.filter(|&(tg, _)| !self.effects_of(tg).is_empty() || matches!(tg, Target::Overlay(i) if self.edit.item(i).is_some_and(|it| !it.motion.is_empty())));
        let reset = tip(icon_button(super::RESET_VIEW, any.map(|_| m(EffectMsg::Reset(None)))), text(if on_cut { "Reset: remove this cut's effects" } else { "Reset: remove this item's effects and place" }), tooltip::Position::Bottom);
        column![row![text("Effects").size(18).width(Length::Fill), close].align_y(Alignment::Center), row![text(name).size(13).width(Length::Fill), to_all, reset].spacing(6).align_y(Alignment::Center)].spacing(6).into()
    }

    /// A group: its name folding it open or shut, its ↺, and `content`
    /// while it's open.
    fn group<'a>(&self, g: Group, content: Element<'a, Message>) -> Element<'a, Message> {
        let folded = self.effects_folded.iter().any(|n| n == g.name());
        let fold = button(text(format!("{} {}", if folded { "▸" } else { "▾" }, g.name())).size(15)).padding([2, 0]).style(button::text).on_press(Message::Effect(EffectMsg::Fold(g)));
        let resets = !g.params().is_empty() || g == Group::Place;
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
    fn style_tiles(&self, i: usize) -> Element<'static, Message> {
        use crate::transition::Look;
        let style = self.edit.cuts[i].style.as_ref().map(|s| s.look);
        let m = |e: EffectMsg| Message::Effect(e);
        let off = tip(button(text("✕").size(12)).padding([1, 6]).style(button::text).on_press_maybe(style.map(|_| m(EffectMsg::Style(None)))), text("No style"), tooltip::Position::Left);
        let looks = Look::STYLES.into_iter().map(|l| (l, m(EffectMsg::Style(Some(l)))));
        column![row![text("Style").width(Length::Fill), off].align_y(Alignment::Center), super::transitions_panel::tiles(looks, style)].spacing(6).into()
    }

    /// The film's fades.
    fn whole_edit(&self) -> Element<'static, Message> {
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
    /// the cut's style's settings, or the vignette's shape, reach and
    /// softness.
    pub(super) fn effects_extra_panel(&self) -> Element<'_, Message> {
        let m = |e: EffectMsg| Message::Effect(e);
        let target = self.effect_target();
        let close = button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(m(EffectMsg::Extra(None)));
        let body = match (self.effect_extra, target) {
            (Some(Extra::Style), Some((Target::Cut(i), _))) => {
                let Some(s) = self.edit.cuts[i].style.as_ref() else { return space().into() };
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
        column![iced::widget::row(shapes).spacing(4).wrap().vertical_spacing(4), slider_of("Reach", crate::grade::SHAPE + 1, grade.vignette_reach), slider_of("Softness", crate::grade::SHAPE + 2, grade.vignette_soft)].spacing(8),
        text("Round, or from the top (a graduated filter on a bright sky), the bottom or the sides; how far in it reaches, how soft its edge"),
        tooltip::Position::Left,
    )
}
