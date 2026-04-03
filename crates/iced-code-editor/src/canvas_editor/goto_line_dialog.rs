use iced::widget::{Tooltip, button, column, container, row, text, text_input, tooltip};
use iced::{Element, Length};
use iced_font_awesome::fa_icon_solid;

use super::Message;
use super::goto_line::GotoLineState;

pub fn view<'a>(goto_line_state: &GotoLineState) -> Element<'a, Message> {
    let input = text_input("Line[:Column]...", &goto_line_state.query)
        .id(goto_line_state.input_id.clone())
        .on_input(Message::GotoLineQueryChanged)
        .on_submit(Message::SubmitGotoLine)
        .padding(4)
        .width(Length::Fixed(180.0));

    let close_button = Tooltip::new(
        button(fa_icon_solid("xmark").size(10.0))
            .on_press(Message::CloseGotoLine)
            .padding(2),
        text("Close go to line (Esc)"),
        tooltip::Position::Left,
    )
    .style(container::rounded_box);

    let title_row = row![
        fa_icon_solid("hashtag").size(12.0),
        text("Go to line").size(12),
        iced::widget::Space::new().width(Length::Fill),
        close_button
    ]
    .width(Length::Fixed(180.0))
    .align_y(iced::Alignment::Center);

    let dialog = column![title_row, input].spacing(5).padding(8);

    container(dialog)
        .padding(6)
        .style(|theme| {
            let base = container::rounded_box(theme);
            container::Style {
                background: base.background.map(|bg| match bg {
                    iced::Background::Color(color) => {
                        iced::Background::Color(iced::Color { a: 0.85, ..color })
                    }
                    _ => bg,
                }),
                ..base
            }
        })
        .into()
}
