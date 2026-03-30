use iced::widget::{Tooltip, button, container, row, text, tooltip};
use iced::{Element, Fill, Font, alignment, font};

use crate::ui_style;

pub(crate) const HEIGHT: f32 = 26.0;

pub(crate) struct Data<'a> {
    pub(crate) file_name: &'a str,
    pub(crate) spinner: &'a str,
    pub(crate) tail_message: &'a str,
}

pub(crate) struct Actions<Message> {
    pub(crate) open_file: Message,
}

pub(crate) fn view<'a, Message>(data: Data<'a>, actions: Actions<Message>) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
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
        .gap(6)
        .padding(8)
        .style(ui_style::tooltip_popup),
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

    container(
        row![file_block, spinner_block, log_block]
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
