use iced::widget::{button, column, container, opaque, row, svg, text};
use iced::{Color, Element, Fill, Length, Theme, alignment};

use crate::fonts;
use crate::icons;
use crate::ui_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorFatality {
    Critical,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptTone {
    Error,
    Warning,
    #[expect(dead_code, reason = "Reserved for future informational prompts")]
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptButtons {
    Ok,
    OkCancel,
    SaveDiscardCancel,
}

#[derive(Debug, Clone)]
pub(crate) struct ErrorPrompt {
    title: String,
    message: String,
    fatality: ErrorFatality,
    tone: PromptTone,
    buttons: PromptButtons,
}

impl ErrorPrompt {
    pub(crate) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        fatality: ErrorFatality,
        buttons: PromptButtons,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            fatality,
            tone: match fatality {
                ErrorFatality::Critical => PromptTone::Error,
                ErrorFatality::Recoverable => PromptTone::Warning,
            },
            buttons,
        }
    }

    pub(crate) fn buttons(&self) -> PromptButtons {
        self.buttons
    }

    pub(crate) fn overlay_ok<'a, Message>(&'a self, on_ok: Message) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay(on_ok, None)
    }

    pub(crate) fn overlay_ok_cancel<'a, Message>(
        &'a self,
        on_ok: Message,
        on_cancel: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay(on_ok, Some(on_cancel))
    }

    pub(crate) fn overlay_save_discard_cancel<'a, Message>(
        &'a self,
        on_save: Message,
        on_discard: Message,
        on_cancel: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay_with_actions(PromptActions {
            ok: Some(("Save", on_save)),
            discard: Some(("Discard", on_discard)),
            cancel: Some(("Cancel", on_cancel)),
        })
    }

    fn overlay<'a, Message>(
        &'a self,
        on_ok: Message,
        on_cancel: Option<Message>,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay_with_actions(PromptActions {
            ok: Some(("OK", on_ok.clone())),
            discard: None,
            cancel: on_cancel.map(|message| ("Cancel", message)),
        })
    }

    fn overlay_with_actions<'a, Message>(
        &'a self,
        actions: PromptActions<Message>,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let is_critical = matches!(self.fatality, ErrorFatality::Critical);
        let critical_info = "The app cannot continue and will close after you press OK";

        let error_details = container(
            text(&self.message)
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(fonts::UI),
        )
        .width(Fill)
        .style(move |theme: &Theme| ui_style::prompt_message(theme, is_critical));

        let action_row = match self.buttons {
            PromptButtons::Ok => {
                let Some((label, message)) = actions.ok.clone() else {
                    unreachable!("OK prompt requires confirm action");
                };
                row![
                    button(text(label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(if is_critical {
                            ui_style::button_danger
                        } else {
                            ui_style::button_active
                        })
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(message)
                ]
                .spacing(ui_style::SPACE_SM)
            }
            PromptButtons::OkCancel => {
                let Some((ok_label, ok_message)) = actions.ok.clone() else {
                    unreachable!("OK/Cancel prompt requires confirm action");
                };
                let Some((cancel_label, cancel_message)) = actions.cancel.clone() else {
                    unreachable!("OK/Cancel prompt requires cancel action");
                };

                row![
                    button(text(cancel_label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(ui_style::button_neutral)
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(cancel_message),
                    button(text(ok_label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(if is_critical {
                            ui_style::button_danger
                        } else {
                            ui_style::button_active
                        })
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(ok_message)
                ]
                .spacing(ui_style::SPACE_SM)
            }
            PromptButtons::SaveDiscardCancel => {
                let Some((save_label, save_message)) = actions.ok.clone() else {
                    unreachable!("Save/Discard/Cancel prompt requires save action");
                };
                let Some((discard_label, discard_message)) = actions.discard.clone() else {
                    unreachable!("Save/Discard/Cancel prompt requires discard action");
                };
                let Some((cancel_label, cancel_message)) = actions.cancel.clone() else {
                    unreachable!("Save/Discard/Cancel prompt requires cancel action");
                };

                row![
                    button(text(cancel_label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(ui_style::button_neutral)
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(cancel_message),
                    button(text(discard_label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(ui_style::button_neutral)
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(discard_message),
                    button(text(save_label).size(ui_style::FONT_SIZE_UI_SM))
                        .style(ui_style::button_active)
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(save_message)
                ]
                .spacing(ui_style::SPACE_SM)
            }
        };

        let actions = container(action_row)
            .width(Fill)
            .align_x(alignment::Horizontal::Right);

        let (title_icon, title_color) = match self.tone {
            PromptTone::Error => (icons::circle_x(), Color::from_rgb(0.84, 0.36, 0.37)),
            PromptTone::Warning => (icons::circle_alert(), Color::from_rgb(0.86, 0.58, 0.23)),
            PromptTone::Info => (icons::info(), Color::from_rgb(0.33, 0.56, 0.86)),
        };

        let title = container(
            row![
                svg(title_icon)
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0))
                    .style(move |_: &Theme, _status| svg::Style {
                        color: Some(title_color)
                    }),
                text(&self.title)
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..fonts::UI
                    })
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
        .style(ui_style::prompt_header);

        let mut content = column![error_details];

        if is_critical {
            content = content.push(
                text(critical_info)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::UI),
            );
        }

        let body = container(content.push(actions).spacing(ui_style::SPACE_MD))
            .width(Fill)
            .padding(ui_style::PADDING_SM);

        let dialog = container(column![title, body].spacing(0))
            .width(ui_style::SIZE_SURFACE_LG)
            .style(ui_style::prompt_dialog);

        let centered_dialog = container(dialog)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill);

        let backdrop = container(centered_dialog)
            .width(Fill)
            .height(Fill)
            .style(ui_style::prompt_backdrop);

        opaque(backdrop)
    }
}

#[derive(Clone)]
struct PromptActions<Message> {
    ok: Option<(&'static str, Message)>,
    discard: Option<(&'static str, Message)>,
    cancel: Option<(&'static str, Message)>,
}
