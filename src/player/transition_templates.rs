//! Transition templates in the Transitions panel (gap 25c): the panel's
//! join saved under a name into the program's library (`template`), and
//! any saved one put on the join, both tabs alike.

use super::transitions_panel::tiles_of;
use super::{App, Message, tip};
use crate::model::ops::{Op, cuts::CutOp};
use crate::template::{self, Template};
use iced::widget::{button, column, row, text, text_input, tooltip};
use iced::{Element, Length, Task};

#[derive(Clone, Debug)]
pub enum TemplateMsg {
    /// Put the named one on the panel's join.
    Apply(String),
    /// The name being typed to save the join under, and saving it.
    Name(String),
    Save,
    /// Take one out of the library: asked first, then answered.
    Remove(String),
    Removed(String, bool),
}

impl App {
    pub(super) fn on_template(&mut self, message: TemplateMsg) -> Task<Message> {
        match message {
            TemplateMsg::Apply(name) => {
                let Some(cut) = self.join_index().map(|i| self.edit.cuts[i].id) else { return Task::none() };
                match template::load(&name) {
                    Ok(template) => {
                        self.op(Op::Cut(CutOp::Template { cut, template }));
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
            TemplateMsg::Name(name) => self.template_name = name,
            TemplateMsg::Save => {
                let Some(i) = self.join_index() else { return Task::none() };
                match template::save(&self.template_name, &Template::of_join(&self.edit.cuts, i)) {
                    Ok(()) => {
                        self.save_status = Some(format!("Template «{}» saved", self.template_name.trim()));
                        self.template_name.clear();
                        self.templates = template::names();
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
            TemplateMsg::Remove(name) => {
                return self.ask(super::ask::Ask {
                    title: "Remove the template".to_owned(),
                    text: format!("Take «{name}» out of the templates? Joins it was put on keep their transitions."),
                    answers: vec![("Remove".to_owned(), Message::Template(TemplateMsg::Removed(name.clone(), true)))],
                    cancel: Message::Template(TemplateMsg::Removed(name, false)),
                });
            }
            TemplateMsg::Removed(name, true) => {
                if let Err(e) = template::remove(&name) {
                    self.error = Some(format!("{e:#}"));
                }
                self.templates = template::names();
            }
            TemplateMsg::Removed(_, false) => {}
        }
        Task::none()
    }

    /// The saved templates to put on the join, and its own to save.
    pub(super) fn templates_section(&self) -> Element<'_, Message> {
        let to = |m: TemplateMsg| Message::Template(m);
        let named = self.templates.iter().map(|name| {
            let put = tip(button(text(name.clone()).size(13)).padding([6, 10]).style(button::secondary).on_press(to(TemplateMsg::Apply(name.clone()))), text("Put on this join: its overlap, out and in"), tooltip::Position::Left);
            let off = tip(button(text("✕").size(11)).padding([1, 5]).style(button::text).on_press(to(TemplateMsg::Remove(name.clone()))), text("Take out of the templates"), tooltip::Position::Top);
            row![put, off].spacing(0).into()
        });
        let name = text_input("Name", &self.template_name).size(13).on_input(|s| Message::Template(TemplateMsg::Name(s))).on_submit(to(TemplateMsg::Save)).width(Length::Fill);
        let save = button(text("Save").size(13)).style(button::secondary).on_press_maybe((!self.template_name.trim().is_empty()).then_some(to(TemplateMsg::Save)));
        let save = tip(save, text("This join's overlap, out and in as a template for any edit (the same name is replaced)"), tooltip::Position::Top);
        column![text("Templates").size(14), tiles_of(named), row![name, save].spacing(6)].spacing(8).into()
    }
}
