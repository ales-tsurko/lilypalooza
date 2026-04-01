use iced::widget::{container, row, text};
use iced::{Color, Element, Fill, Font, Theme};

use crate::ui_style;

pub(crate) const HEIGHT: f32 = 26.0;

pub(crate) struct Data<'a> {
    pub(crate) spinner: &'a str,
    pub(crate) tail_message: &'a str,
}

pub(crate) fn view<'a, Message>(data: Data<'a>) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let log_block = container(
        text(data.tail_message)
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(Font::MONOSPACE)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                iced::widget::text::Style {
                    color: Some(Color {
                        a: 0.74,
                        ..palette.background.weak.text
                    }),
                }
            }),
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
        row![spinner_block, log_block]
            .spacing(0)
            .width(Fill)
            .height(Fill)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Fill)
    .height(HEIGHT)
    .padding(0)
    .style(ui_style::status_bar_surface)
    .into()
}
