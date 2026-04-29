use iced::widget::{Tooltip, button, column, container, row, text, text_input, tooltip};
use iced::{Element, Length};
use std::time::Duration;

use super::Message;
use super::goto_line::GotoLineState;

const POPUP_TEXT_SIZE: f32 = 12.0;
const POPUP_META_TEXT_SIZE: f32 = 11.0;
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

pub fn view<'a>(goto_line_state: &GotoLineState) -> Element<'a, Message> {
    let input = text_input("Line[:Column]...", &goto_line_state.query)
        .id(goto_line_state.input_id.clone())
        .on_input(Message::GotoLineQueryChanged)
        .on_submit(Message::SubmitGotoLine)
        .size(POPUP_TEXT_SIZE)
        .style(crate::theme::popup_text_input)
        .padding(4)
        .width(Length::Fixed(180.0));

    let close_button = Tooltip::new(
        button(text("×").size(16.0).line_height(1.0))
            .on_press(Message::CloseGotoLine)
            .padding(2)
            .style(crate::theme::popup_icon_button),
        text("Close go to line (Esc)").size(POPUP_META_TEXT_SIZE),
        tooltip::Position::Left,
    )
    .delay(TOOLTIP_DELAY)
    .style(crate::theme::popup_tooltip);

    let title_row = row![
        text("#")
            .size(13.0)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: crate::theme::popup_surface(theme).text_color
            }),
        iced::widget::Space::new().width(Length::Fixed(8.0)),
        text("Go to line").size(POPUP_TEXT_SIZE),
        iced::widget::Space::new().width(Length::Fill),
        close_button
    ]
    .width(Length::Fixed(180.0))
    .align_y(iced::Alignment::Center);

    let dialog = column![title_row, input].spacing(5).padding(8);

    container(dialog)
        .padding(6)
        .style(crate::theme::popup_surface)
        .into()
}
