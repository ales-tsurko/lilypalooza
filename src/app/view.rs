use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, svg, text, text_input,
};
use iced::{Element, Fill, Length, alignment};

use super::dock_view;
use super::messages::ShortcutsMessage;
use super::{Lilypalooza, Message, PromptMessage};
use crate::error_prompt::PromptButtons;
use crate::fonts;
use crate::icons;
use crate::shortcuts;
use crate::status_bar;
use crate::ui_style;

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

    let shortcuts_overlay: Element<'_, Message> = if app.open_shortcuts_dialog {
        shortcuts_overlay(app)
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };

    let prompt_overlay: Element<'_, Message> = if let Some(prompt) = &app.error_prompt {
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

    stack([base, shortcuts_overlay, prompt_overlay]).into()
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

fn shortcuts_overlay(app: &Lilypalooza) -> Element<'_, Message> {
    let backdrop: Element<'_, Message> = container(
        mouse_area(container(text("")).width(Fill).height(Fill))
            .on_press(Message::Shortcuts(ShortcutsMessage::CloseDialog)),
    )
    .width(Fill)
    .height(Fill)
    .style(ui_style::prompt_backdrop)
    .into();

    let actions = shortcuts::filtered_action_metadata(&app.shortcuts_search_query);

    let header = container(
        row![
            text("Actions")
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                }),
            container(text("")).width(Fill),
            button(
                svg(icons::x())
                    .width(Length::Fixed(12.0))
                    .height(Length::Fixed(12.0))
            )
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H
            ])
            .on_press(Message::Shortcuts(ShortcutsMessage::CloseDialog)),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
    .style(ui_style::prompt_header);

    let search = text_input("Search actions", &app.shortcuts_search_query)
        .on_input(|value| Message::Shortcuts(ShortcutsMessage::SearchChanged(value)))
        .id(app.shortcuts_search_input_id.clone())
        .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
        .size(ui_style::FONT_SIZE_UI_SM);

    let rows = if actions.is_empty() {
        column![
            container(text("No actions match the current search.").size(ui_style::FONT_SIZE_UI_SM))
                .padding(ui_style::PADDING_SM)
        ]
        .spacing(0)
    } else {
        actions
            .into_iter()
            .fold(column![].spacing(0), |column, metadata| {
                let shortcut = shortcuts::label_for_action_id(&app.shortcut_settings, metadata.id)
                    .unwrap_or_else(|| "Unassigned".to_string());
                let selected = app.shortcuts_selected_action == Some(metadata.id);
                column.push(
                    button(
                        row![
                            column![
                                row![
                                    text(metadata.name).size(ui_style::FONT_SIZE_UI_SM),
                                    container(
                                        text(crate::settings::shortcut_action_id_key(metadata.id))
                                            .size(ui_style::FONT_SIZE_UI_XS)
                                            .font(fonts::MONO),
                                    )
                                    .padding([1, ui_style::PADDING_XS])
                                    .style(ui_style::shortcut_action_id_label),
                                ]
                                .spacing(ui_style::SPACE_XS),
                                text(metadata.description).size(ui_style::FONT_SIZE_UI_SM)
                            ]
                            .spacing(2)
                            .width(Fill),
                            text(shortcut)
                                .size(ui_style::FONT_SIZE_UI_SM)
                                .font(fonts::MONO),
                        ]
                        .spacing(ui_style::SPACE_SM)
                        .align_y(alignment::Vertical::Center)
                        .width(Fill),
                    )
                    .width(Fill)
                    .height(Length::Fixed(super::SHORTCUTS_ACTION_ROW_HEIGHT))
                    .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
                    .style(move |theme, status| {
                        ui_style::button_shortcut_palette_item(theme, status, selected)
                    })
                    .on_press(Message::Shortcuts(
                        ShortcutsMessage::ActivateAction(metadata.id),
                    )),
                )
            })
    };

    let dialog = container(
        column![
            header,
            container(
                column![
                    search,
                    scrollable(rows)
                        .id(super::SHORTCUTS_SCROLLABLE_ID)
                        .height(Length::Fixed(420.0))
                        .style(ui_style::workspace_scrollable),
                ]
                .spacing(ui_style::SPACE_SM)
            )
            .padding(ui_style::PADDING_SM),
        ]
        .spacing(0),
    )
    .width(Length::Fixed(920.0))
    .style(ui_style::prompt_dialog);

    stack([
        backdrop,
        container(dialog)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into(),
    ])
    .into()
}
