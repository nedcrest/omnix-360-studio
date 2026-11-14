//! The Settings window: how the player behaves, kept between runs
//! (`settings.rs`), in groups, over the program.

use super::effects::LutChoice;
use super::{App, Message, tip};
use iced::widget::{button, column, container, pick_list, row, rule, scrollable, text, toggler, tooltip};
use iced::{Alignment, Element, Length, Theme};

/// The Settings window's size, and its groups' column.
const WIDTH: f32 = 760.0;
const HEIGHT: f32 = 520.0;
const GROUPS: f32 = 150.0;

/// The theme the settings file names, as iced names it; Kanagawa Dragon
/// (the user's choice, 2026-10-14) for none or one iced no longer has.
/// The desktop's own theme was followed until then; dropped as the user
/// liked a fixed one better, and its reading crashed the Mac at times.
pub fn theme_named(name: &str) -> Theme {
    Theme::ALL.iter().find(|t| t.to_string() == name).cloned().unwrap_or(Theme::KanagawaDragon)
}

impl App {
    /// The LUT library: each LUT with a button to take it out, Add, and
    /// which one D-Log M clips get.
    fn lut_library(&self) -> Element<'_, Message> {
        let note = |s: &'static str| text(s).size(13).style(text::secondary);
        let mut list = column![].spacing(4);
        for name in &self.luts {
            let remove = tip(button(text("✕").size(13)).padding([0, 6]).style(button::text).on_press(Message::RemoveLut(name.clone())), text("Take it out of the library"), tooltip::Position::Left);
            list = list.push(row![text(name.clone()).size(14).width(Length::Fill), remove].align_y(Alignment::Center));
        }
        if self.luts.is_empty() {
            list = list.push(note("None yet."));
        }
        let mut choices = vec![LutChoice::None];
        choices.extend(self.luts.iter().map(|n| LutChoice::Named(n.clone())));
        column![
            list,
            button(text("Add LUT…").size(14)).on_press(Message::AddLut),
            note("Copied into Omnix's settings folder, so projects keep working when the download moves."),
            text("For D-Log M clips").size(14),
            pick_list(choices, Some(LutChoice::of(self.dlog_m_lut.as_ref())), Message::DlogMLut).text_size(13).width(Length::Fill),
            note("Clips filmed in D-Log M get it when they're added; the Effects panel changes it per cut. DJI's is \"Osmo 360 D-Log M to Rec.709 LUT\" in the DJI Download Center."),
        ]
        .spacing(6)
        .into()
    }

    /// Where the projects are: Omnix's own folder, or Dailix's.
    fn projects_setting(&self) -> Element<'_, Message> {
        let note = |s: String| text(s).size(13).style(text::secondary);
        let shown = |p: &std::path::PathBuf| text(p.display().to_string()).size(14);
        let mut col = column![].spacing(6);
        col = match (&self.projects, &self.dailix_projects) {
            (Some(own), _) => col.push(shown(own)),
            (None, Some(dailix)) => col.push(shown(dailix)).push(note("Dailix's, until you choose one here.".into())),
            (None, None) => col.push(note("None yet: choose one, or set Dailix's.".into())),
        };
        let mut buttons = row![button(text("Choose…").size(14)).on_press(Message::ProjectsChoose)].spacing(8);
        if self.projects.is_some() && self.dailix_projects.is_some() {
            buttons = buttons.push(button(text("Use Dailix's").size(14)).style(button::secondary).on_press(Message::ProjectsFromDailix));
        }
        col.push(buttons).into()
    }

    /// The projects place Omnix uses: its own, else Dailix's.
    pub(super) fn projects_place(&self) -> Option<std::path::PathBuf> {
        self.projects.clone().or_else(|| self.dailix_projects.clone())
    }

    /// The Settings window over the program while it's open (user,
    /// 2026-11-07: a window of its own with groups, not a list down the
    /// side): the groups at the left, the chosen one's settings at the
    /// right. ✕, Esc or a click outside close it.
    pub(super) fn settings_layer(&self) -> Element<'_, Message> {
        let Some(open) = self.settings_open else { return iced::widget::space().into() };
        let close = tip(button(text("✕").size(14)).padding([0, 6]).style(button::text).on_press(Message::SettingsPanel), text("Close (Esc)"), tooltip::Position::Bottom);
        let groups = column(Group::ALL.iter().map(|&g| {
            button(text(g.name()).size(15)).width(Length::Fill).padding([6, 10]).style(if g == open { button::primary } else { button::text }).on_press(Message::SettingsGroup(g)).into()
        }))
        .spacing(4)
        .width(GROUPS);
        let page = match open {
            Group::General => self.general_page(),
            Group::Picture => self.picture_page(),
            Group::Editing => self.editing_page(),
        };
        let card = container(
            column![
                row![text("Settings").size(20).width(Length::Fill), close].align_y(Alignment::Center),
                row![groups, rule::vertical(1), scrollable(container(page).padding([0, 12])).width(Length::Fill).height(Length::Fill)].spacing(12),
            ]
            .spacing(12),
        )
        .padding(16)
        .width(WIDTH)
        .height(HEIGHT)
        .style(container::rounded_box);
        super::ask::modal(card, Some(Message::SettingsPanel))
    }

    /// Where projects are, and how the program looks.
    fn general_page(&self) -> Element<'_, Message> {
        column![
            section("Projects", self.projects_setting()),
            section("Look", column![pick_list(Theme::ALL, Some(self.theme.clone()), Message::Theme).width(Length::Fill), note("Kanagawa Dragon unless you choose another.")].spacing(6).into()),
        ]
        .spacing(18)
        .into()
    }

    /// How clips are decoded and graded.
    fn picture_page(&self) -> Element<'_, Message> {
        column![
            section(
                "Decoding",
                column![
                    toggler(self.ten_bit).label("10-bit").on_toggle(Message::TenBit),
                    note("Decodes the OSV and 10-bit videos (a phone's HDR) at all their 10 bits instead of 8: smoother skies and gradients, twice the data to move. Applies from the next seek or clip."),
                ]
                .spacing(6)
                .into()
            ),
            section(
                "Correction",
                column![
                    toggler(self.rolling_shutter).label("Rolling shutter").on_toggle(Message::RollingShutter),
                    note("Straightens what a 360 camera's row-by-row readout bends while it turns, in playback and the export."),
                ]
                .spacing(6)
                .into()
            ),
            section("LUTs", self.lut_library()),
        ]
        .spacing(18)
        .into()
    }

    /// The timeline, keyframes and the view's numbers.
    fn editing_page(&self) -> Element<'_, Message> {
        column![
            section("Timeline", column![toggler(self.ruler_large).label("Large time ruler").on_toggle(Message::RulerLarge), note("Off: a compact ruler, more room for the lanes.")].spacing(6).into()),
            section("Start screen", column![toggler(self.card_motion).label("Animate project cards").on_toggle(|on| Message::StartCard(super::start_cards::CardMsg::Motion(on))), note("Off: a card's frames move only as the pointer moves over it.")].spacing(6).into()),
            section(
                "Keyframes",
                column![
                    toggler(self.auto_key).label("Auto keyframe").on_toggle(Message::AutoKey),
                    note("On: looking around while paused sets the view keyframe at the playhead, or updates the one there, as K does. Each look-around is one undo step. While playing, looking around stays free."),
                    note("Off: looking around is free until you press K."),
                ]
                .spacing(6)
                .into()
            ),
            section(
                "View angles",
                column![
                    toggler(self.angles_shown).label("Show the view as numbers").on_toggle(Message::ShowAngles),
                    note("A strip under the transport row with the yaw, pitch, roll, FOV and correction: type one, or drag its name sideways, to set it as dragging the view would; scroll or ↑ ↓ on a field nudges it, Shift for finer steps. Handy for giving clips from a camera mounted the same way the same framing."),
                ]
                .spacing(6)
                .into()
            ),
        ]
        .spacing(18)
        .into()
    }
}

/// The Settings window's groups, in the order they're listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    General,
    Picture,
    Editing,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::General, Group::Picture, Group::Editing];

    fn name(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::Picture => "Picture",
            Group::Editing => "Editing",
        }
    }
}

/// A setting's heading and what's under it.
fn section<'a>(title: &'static str, body: Element<'a, Message>) -> Element<'a, Message> {
    column![text(title).size(16), body].spacing(8).into()
}

fn note<'a>(s: &'static str) -> iced::widget::Text<'a> {
    text(s).size(13).style(text::secondary)
}

#[cfg(test)]
mod tests {
    use super::theme_named;
    use iced::Theme;

    #[test]
    fn every_theme_is_read_back_as_saved() {
        for theme in Theme::ALL {
            assert_eq!(&theme_named(&theme.to_string()), theme);
        }
        // What the desktop-following choice was saved as.
        assert_eq!(theme_named("system"), Theme::KanagawaDragon);
    }
}
