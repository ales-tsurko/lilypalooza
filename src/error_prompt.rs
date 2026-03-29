use iced::widget::{button, column, container, opaque, row, text};
use iced::{Element, Fill, Theme, alignment};

use crate::ui_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorFatality {
    Critical,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptButtons {
    Ok,
    OkCancel,
}

#[derive(Debug, Clone)]
pub(crate) struct ErrorPrompt {
    title: String,
    message: String,
    fatality: ErrorFatality,
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

    fn overlay<'a, Message>(
        &'a self,
        on_ok: Message,
        on_cancel: Option<Message>,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let is_critical = matches!(self.fatality, ErrorFatality::Critical);
        let critical_info = "The app cannot continue and will close after you press OK";

        let error_details = container(text(&self.message).size(ui_style::FONT_SIZE_BODY_SM))
            .width(Fill)
            .padding(ui_style::PADDING_SM)
            .style(move |theme: &Theme| ui_style::prompt_message(theme, is_critical));

        let ok_button = button(text("OK").size(ui_style::FONT_SIZE_BODY_SM))
            .style(if is_critical {
                ui_style::button_danger
            } else {
                ui_style::button_active
            })
            .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
            .on_press(on_ok.clone());

        let action_row = match self.buttons {
            PromptButtons::Ok => row![ok_button].spacing(ui_style::SPACE_SM),
            PromptButtons::OkCancel => {
                let cancel_message = on_cancel.unwrap_or_else(|| on_ok.clone());

                row![
                    button(text("Cancel").size(ui_style::FONT_SIZE_BODY_SM))
                        .style(ui_style::button_neutral)
                        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
                        .on_press(cancel_message),
                    ok_button
                ]
                .spacing(ui_style::SPACE_SM)
            }
        };

        let actions = container(action_row)
            .width(Fill)
            .align_x(alignment::Horizontal::Right);

        let mut content = column![
            text(&self.title).size(ui_style::FONT_SIZE_HEADING_LG),
            error_details
        ];

        if is_critical {
            content = content.push(text(critical_info).size(ui_style::FONT_SIZE_BODY_MD));
        }

        let dialog = container(content.push(actions).spacing(ui_style::SPACE_MD))
            .width(ui_style::SIZE_SURFACE_LG)
            .padding(ui_style::PADDING_MD)
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
