use iced::widget::{column, container, stack, text};
use iced::{Element, Fill};

use super::score_view;
use super::{LilyView, Message, PromptMessage};
use crate::error_prompt::PromptButtons;
use crate::status_bar;

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let tail_message = app.logger.last_line().unwrap_or("No log messages");
    let spinner = if app.compile_session.is_some() {
        super::SPINNER_FRAMES[app.spinner_step % super::SPINNER_FRAMES.len()]
    } else {
        " "
    };

    let base: Element<'_, Message> = column![
        main_content(app),
        status_bar::view(status_bar::Data {
            spinner,
            tail_message,
        }),
    ]
    .height(Fill)
    .into();

    if let Some(prompt) = &app.error_prompt {
        let overlay = match prompt.buttons() {
            PromptButtons::Ok => prompt.overlay_ok(Message::Prompt(PromptMessage::Acknowledge)),
            PromptButtons::OkCancel => prompt.overlay_ok_cancel(
                Message::Prompt(PromptMessage::Acknowledge),
                Message::Prompt(PromptMessage::Cancel),
            ),
        };

        stack([base, overlay]).into()
    } else {
        base
    }
}

fn main_content(app: &LilyView) -> Element<'_, Message> {
    match &app.lilypond_status {
        super::LilypondStatus::Checking => container(
            text("Checking LilyPond availability...").size(crate::ui_style::FONT_SIZE_BODY_MD),
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
            score_view::view(app)
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
