use iced::widget::{column, container, pane_grid, stack, text};
use iced::{Element, Fill};

use super::score_view;
use super::{FileMessage, LilyView, LoggerMessage, Message, PaneKind, PaneMessage, PromptMessage};
use crate::error_prompt::PromptButtons;
use crate::{status_bar, ui_style};

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let panes = pane_grid::PaneGrid::new(&app.panes, |_pane, kind, _is_maximized| match kind {
        PaneKind::Main => {
            pane_grid::Content::new(main_content(app)).style(ui_style::pane_main_surface)
        }
        PaneKind::Logger => pane_grid::Content::new(logger_content(app))
            .title_bar(logger_title_bar())
            .style(ui_style::pane_logger_surface),
    })
    .width(Fill)
    .height(Fill)
    .on_resize(8, |event| Message::Pane(PaneMessage::LoggerResized(event)));

    let file_name = app
        .current_score
        .as_ref()
        .map(|selected_score| selected_score.file_name.as_str())
        .unwrap_or("No file");

    let tail_message = app.logger.last_line().unwrap_or("No log messages");
    let spinner = if app.compile_session.is_some() {
        super::SPINNER_FRAMES[app.spinner_step % super::SPINNER_FRAMES.len()]
    } else {
        " "
    };

    let base: Element<'_, Message> = column![
        container(panes).width(Fill).height(Fill),
        status_bar::view(
            status_bar::Data {
                file_name,
                spinner,
                tail_message,
                logger_open: app.logger_pane.is_some(),
                can_clear_logs: !app.logger.is_empty(),
            },
            status_bar::Actions {
                open_file: Message::File(FileMessage::RequestOpen),
                toggle_logger: Message::Pane(PaneMessage::ToggleLogger),
                clear_logger: Message::Logger(LoggerMessage::RequestClear),
            },
        ),
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

fn logger_title_bar<'a>() -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(text("Logger").size(ui_style::FONT_SIZE_UI_SM))
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .always_show_controls()
        .style(ui_style::pane_title_bar_surface)
}

fn main_content(app: &LilyView) -> Element<'_, Message> {
    match &app.lilypond_status {
        super::LilypondStatus::Checking => {
            container(text("Checking LilyPond availability...").size(ui_style::FONT_SIZE_BODY_MD))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill)
                .into()
        }
        super::LilypondStatus::Ready {
            detected,
            min_required,
        } => {
            let _ = (detected, min_required);
            score_view::view(app)
        }
        super::LilypondStatus::Unavailable => {
            container(text("LilyPond unavailable.").size(ui_style::FONT_SIZE_BODY_MD))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill)
                .into()
        }
    }
}

fn logger_content(app: &LilyView) -> Element<'_, Message> {
    app.logger
        .view(|action| Message::Logger(LoggerMessage::TextAction(action)))
}
