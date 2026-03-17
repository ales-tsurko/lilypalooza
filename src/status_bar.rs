use iced::widget::{Tooltip, button, container, row, text, tooltip};
use iced::{Element, Fill, Font, alignment, font};

use crate::ui_style;

pub(crate) const HEIGHT: f32 = 26.0;

pub(crate) struct Data<'a> {
    pub(crate) file_name: &'a str,
    pub(crate) spinner: &'a str,
    pub(crate) tail_message: &'a str,
    pub(crate) logger_open: bool,
    pub(crate) can_clear_logs: bool,
}

pub(crate) struct Actions<Message> {
    pub(crate) open_file: Message,
    pub(crate) toggle_logger: Message,
    pub(crate) clear_logger: Message,
}

pub(crate) fn view<'a, Message>(data: Data<'a>, actions: Actions<Message>) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let toggle_icon = "≣";

    let file_button = button(
        row![
            text("🗎").size(ui_style::FONT_SIZE_UI_XS),
            text(data.file_name)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(Font {
                    family: font::Family::Monospace,
                    weight: font::Weight::Semibold,
                    ..Font::MONOSPACE
                }),
        ]
        .height(Fill)
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .style(ui_style::status_block_button)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .on_press(actions.open_file);

    let file_block = container(
        Tooltip::new(
            file_button,
            text("Open file").size(ui_style::FONT_SIZE_UI_XS),
            tooltip::Position::Top,
        )
        .gap(6),
    )
    .height(Fill)
    .center_y(Fill)
    .padding([0, ui_style::PADDING_STATUS_BAR_H])
    .style(ui_style::status_block_surface);

    let log_block = container(
        text(data.tail_message)
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(Font::MONOSPACE),
    )
    .width(Fill)
    .height(Fill)
    .center_y(Fill)
    .padding([0, ui_style::PADDING_STATUS_BAR_H])
    .style(ui_style::status_block_surface);

    let spinner_block = container(
        text(data.spinner)
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(Font::MONOSPACE),
    )
    .height(Fill)
    .center_y(Fill)
    .padding([0, ui_style::PADDING_STATUS_BAR_H])
    .style(ui_style::status_block_surface);

    let toggle_button = button(
        text(toggle_icon)
            .font(Font::MONOSPACE)
            .size(ui_style::FONT_SIZE_UI_XS),
    )
    .style(if data.logger_open {
        ui_style::button_active
    } else {
        ui_style::button_neutral
    })
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .on_press(actions.toggle_logger);

    let clear_button = button(
        text("⌫")
            .font(Font::MONOSPACE)
            .size(ui_style::FONT_SIZE_UI_XS),
    )
    .style(ui_style::button_neutral)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ]);

    let clear_button = if data.can_clear_logs {
        clear_button.on_press(actions.clear_logger)
    } else {
        clear_button
    };

    let buttons = row![
        Tooltip::new(
            toggle_button,
            text(if data.logger_open {
                "Hide logger"
            } else {
                "Show logger"
            })
            .size(ui_style::FONT_SIZE_UI_XS),
            tooltip::Position::Top,
        )
        .gap(6),
        Tooltip::new(
            clear_button,
            text("Clear logger").size(ui_style::FONT_SIZE_UI_XS),
            tooltip::Position::Top,
        )
        .gap(6),
    ]
    .spacing(ui_style::SPACE_XS)
    .align_y(alignment::Vertical::Center);

    let button_block = container(buttons)
        .height(Fill)
        .center_y(Fill)
        .padding([0, ui_style::PADDING_STATUS_BAR_H])
        .style(ui_style::status_block_surface);

    container(
        row![file_block, spinner_block, log_block, button_block]
            .spacing(0)
            .width(Fill)
            .height(Fill)
            .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .height(HEIGHT)
    .padding(0)
    .style(ui_style::status_bar_surface)
    .into()
}
