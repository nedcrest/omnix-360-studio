//! The Audio panel: what you hear (mute and the listening volume) and the
//! mix the export makes (the clips' sound and the music), docked at the
//! right of the view like the Effects panel, opened by the speaker button.

use super::{App, Message, SPEAKER, SPEAKER_MUTED, db_label, icon_button, tip};
use iced::widget::{column, container, row, rule, scrollable, slider, text, tooltip};
use crate::project::SoundTrack;
use iced::{Alignment, Element, Length};

const WIDTH: f32 = 300.0;

/// Small dimmed text under a control, saying what it does.
fn note<'a>(s: &'a str) -> Element<'a, Message> {
    text(s).size(13).style(text::secondary).into()
}

impl App {
    pub(super) fn audio_panel(&self) -> Element<'_, Message> {
        let level = |name: &'static str, db: f64, set: fn(f64) -> Message, about: &'static str| {
            column![
                row![text(name).width(Length::Fill), text(db_label(db)).size(13)],
                slider(crate::project::OFF_DB..=6.0, db, set).step(0.5).on_release(Message::LevelDone),
                note(about),
            ]
            .spacing(4)
        };
        let body = column![
            text("Audio").size(18),
            column![
                text("Listening"),
                row![
                    tip(icon_button(if self.muted { SPEAKER_MUTED } else { SPEAKER }, Some(Message::Mute)), text(if self.muted { "Unmute (Shift+M)" } else { "Mute (Shift+M)" }), tooltip::Position::Bottom),
                    slider(0.0..=1.0, self.volume(), Message::Volume).step(0.01f32),
                    text(format!("{:.0}%", self.volume() * 100.0)).size(13).width(40),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                note("Only what you hear: the export and the meter don't change with it."),
            ]
            .spacing(6),
            rule::horizontal(1),
            text("Mix").size(16),
            level("Clip sound", self.edit.sound.volume, Message::SoundLevel, "The clips' own sound, in playback and the export."),
            level("Music", self.edit.music.volume, |db| Message::TrackLevel(SoundTrack::Music, db), "The music track, in playback and the export."),
            tip(
                // As the levels above: name and value, then the slider across
                // the panel. Beside the button it was squeezed to nothing.
                column![
                    row![text("Music under speech").width(Length::Fill), text(format!("−{:.0} dB", self.duck_db)).size(13)],
                    slider(3.0..=30.0, self.duck_db, Message::DuckDepth).step(1.0),
                    iced::widget::button(text(if self.ducking { "Lowering…" } else { "Lower the music under speech" }).size(13)).on_press_maybe((!self.ducking && !self.edit.music.clips.is_empty()).then_some(Message::Duck)),
                ]
                .spacing(4),
                text("Auto-duck: volume points on the songs' own lines, this far down wherever a cleaned voice or a Voice take speaks. Ctrl+Z undoes it"),
                tooltip::Position::Bottom,
            ),
            level("SFX", self.edit.sfx.volume, |db| Message::TrackLevel(SoundTrack::Sfx, db), "The sound effects track, in playback and the export."),
            level("SFX 2", self.edit.sfx2.volume, |db| Message::TrackLevel(SoundTrack::Sfx2, db), "The second sound effects track, for effects over effects."),
            level("Voice", self.edit.voice.volume, |db| Message::TrackLevel(SoundTrack::Voice, db), "The voice-over track, in playback and the export."),
            iced::widget::button(text("Normalize sound…"))
                .on_press_maybe((!self.edit.cuts.is_empty()).then(|| {
                    let scope = self.selection.cut(&self.edit).filter(|&i| i < self.edit.cuts.len()).map_or(super::sound_ui::Scope::All, |i| super::sound_ui::Scope::Own(crate::project::Own::Cut(i)));
                    Message::Sound(super::sound_ui::SoundMsg::Open(scope))
                })),
            note("To change the level within a cut or a track, Ctrl+click its lane on the timeline for a volume point."),
        ]
        .spacing(14)
        .padding(12);
        container(scrollable(body)).width(WIDTH).height(Length::Fill).into()
    }
}
