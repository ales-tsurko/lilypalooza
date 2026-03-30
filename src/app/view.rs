use iced::widget::{Tooltip, button, column, container, pane_grid, row, stack, svg, text, tooltip};
use iced::{ContentFit, Element, Fill, Length, alignment};

use super::score_view;
use super::{FileMessage, LilyView, LoggerMessage, Message, PaneKind, PaneMessage, PromptMessage};
use crate::error_prompt::PromptButtons;
use crate::{icons, status_bar, ui_style};

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let panes = pane_grid::PaneGrid::new(&app.panes, |_pane, kind, _is_maximized| match kind {
        PaneKind::Main => {
            pane_grid::Content::new(main_content(app)).style(ui_style::pane_main_surface)
        }
        PaneKind::Logger => pane_grid::Content::new(logger_content(app))
            .title_bar(logger_title_bar(!app.logger.is_empty()))
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
            },
            status_bar::Actions {
                open_file: Message::File(FileMessage::RequestOpen),
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

fn logger_title_bar<'a>(can_clear_logs: bool) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(
        row![
            title_icon(icons::scroll_text()),
            text("Logger").size(ui_style::FONT_SIZE_UI_SM),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .controls(logger_clear_button(can_clear_logs))
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

fn title_icon(icon: iced::widget::svg::Handle) -> Element<'static, Message> {
    container(
        svg(icon)
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .content_fit(ContentFit::Contain)
            .style(ui_style::svg_window_control),
    )
    .width(Length::Fixed(14.0))
    .height(Length::Fixed(14.0))
    .center_x(Length::Fixed(14.0))
    .center_y(Length::Fixed(14.0))
    .into()
}

fn logger_clear_button(can_clear_logs: bool) -> Element<'static, Message> {
    let button = button(title_icon(icons::brush_cleaning()))
        .style(ui_style::button_window_control)
        .padding([4, 7]);
    let button = if can_clear_logs {
        button.on_press(Message::Logger(LoggerMessage::RequestClear))
    } else {
        button
    };

    Tooltip::new(
        button,
        text("Clear logger").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}
