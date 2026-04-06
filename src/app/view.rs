use iced::widget::{column, container, row, stack, text};
use iced::{Element, Fill};

use super::dock_view;
use super::{Lilypalooza, Message, PromptMessage};
use crate::error_prompt::PromptButtons;
use crate::fonts;
use crate::status_bar;

pub(super) fn view(app: &Lilypalooza) -> Element<'_, Message> {
    let tail_message = app.logger.last_line().unwrap_or("No log messages");
    let spinner = app.spinner_frame();

    let base: Element<'_, Message> = column![
        main_content(app),
        status_bar::view(status_bar::Data {
            spinner,
            tail_message,
        }),
    ]
    .height(Fill)
    .into();

    let overlay: Element<'_, Message> = if let Some(prompt) = &app.error_prompt {
        match prompt.buttons() {
            PromptButtons::Ok => prompt.overlay_ok(Message::Prompt(PromptMessage::Acknowledge)),
            PromptButtons::OkCancel => prompt.overlay_ok_cancel(
                Message::Prompt(PromptMessage::Acknowledge),
                Message::Prompt(PromptMessage::Cancel),
            ),
            PromptButtons::SaveDiscardCancel => prompt.overlay_save_discard_cancel(
                Message::Prompt(PromptMessage::Acknowledge),
                Message::Prompt(PromptMessage::Discard),
                Message::Prompt(PromptMessage::Cancel),
            ),
        }
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };

    stack([base, overlay]).into()
}

fn main_content(app: &Lilypalooza) -> Element<'_, Message> {
    match &app.lilypond_status {
        super::LilypondStatus::Checking => container(
            row![
                text(app.spinner_frame())
                    .size(crate::ui_style::FONT_SIZE_BODY_MD)
                    .font(fonts::MONO),
                text("Checking LilyPond availability...").size(crate::ui_style::FONT_SIZE_BODY_MD),
            ]
            .spacing(crate::ui_style::SPACE_SM)
            .align_y(iced::alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .into(),
        super::LilypondStatus::Ready {
            detected,
            min_required,
        } => {
            let _ = (detected, min_required);
            dock_view::view(app)
        }
        super::LilypondStatus::Unavailable => {
            container(text("LilyPond unavailable.").size(crate::ui_style::FONT_SIZE_BODY_MD))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill)
                .into()
        }
    }
}
