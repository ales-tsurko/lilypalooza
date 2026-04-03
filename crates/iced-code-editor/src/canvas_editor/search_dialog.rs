//! Search and replace dialog UI.
//!
//! This module provides the visual interface for the search/replace functionality.

use iced::widget::{
    Space, Tooltip, button, checkbox, column, container, row, text, text_input, tooltip,
};
use iced::{Element, Length};
use iced_font_awesome::fa_icon_solid;
use std::time::Duration;

use super::Message;
use super::search::{MAX_MATCHES, SearchState};
use crate::i18n::Translations;

const POPUP_TEXT_SIZE: f32 = 12.0;
const POPUP_META_TEXT_SIZE: f32 = 11.0;
const POPUP_WIDTH: f32 = 236.0;
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

fn popup_symbol_button<'a>(
    symbol: &'a str,
    message: Message,
    tooltip_text: String,
) -> Tooltip<'a, Message> {
    Tooltip::new(
        button(text(symbol).size(16.0).line_height(1.0))
            .on_press(message)
            .padding(2)
            .style(crate::theme::popup_icon_button),
        text(tooltip_text).size(POPUP_META_TEXT_SIZE),
        tooltip::Position::Bottom,
    )
    .delay(TOOLTIP_DELAY)
    .style(crate::theme::popup_tooltip)
}

/// Creates the search/replace dialog UI element.
///
/// # Arguments
///
/// * `search_state` - Current search state
/// * `translations` - Translations for UI text
///
/// # Returns
///
/// An Iced element representing the search dialog, or empty space if closed
pub fn view<'a>(
    search_state: &SearchState,
    translations: &'a Translations,
) -> Element<'a, Message> {
    if !search_state.is_open {
        // Return empty Space when closed
        return Space::new().into();
    }

    // Search input field - compact, minimum practical width with placeholder
    let search_input = text_input(&translations.search_placeholder(), &search_state.query)
        .id(search_state.search_input_id.clone())
        .on_input(Message::SearchQueryChanged)
        .size(POPUP_TEXT_SIZE)
        .style(crate::theme::popup_text_input)
        .padding(4)
        .width(Length::Fill);

    let title_label = if search_state.is_replace_mode {
        "Replace".to_string()
    } else if search_state.query.is_empty() {
        "Find".to_string()
    } else if search_state.match_count() == 0 {
        "Find (0)".to_string()
    } else {
        let count_display = if search_state.match_count() >= MAX_MATCHES {
            format!("{}+", MAX_MATCHES)
        } else {
            format!("{}", search_state.match_count())
        };
        let counter = if let Some(idx) = search_state.current_match_index {
            format!("{}/{}", idx + 1, count_display)
        } else {
            count_display
        };
        format!("Find ({counter})")
    };

    // Navigation buttons - compact with Font Awesome icons and tooltips
    let prev_button = popup_symbol_button(
        "‹",
        Message::FindPrevious,
        translations.previous_match_tooltip(),
    );

    let next_button =
        popup_symbol_button("›", Message::FindNext, translations.next_match_tooltip());

    // Case sensitivity checkbox
    let case_checkbox = checkbox(search_state.case_sensitive)
        .on_toggle(|_| Message::ToggleCaseSensitive)
        .style(crate::theme::popup_checkbox);

    let case_icon = fa_icon_solid("font").size(11.0);
    let case_label_text = text(translations.case_sensitive_label()).size(POPUP_META_TEXT_SIZE);

    // Combined navigation + counter + case sensitivity row (all on one line)
    let search_row = row![search_input, prev_button, next_button]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    let options_row = row![
        case_checkbox,
        case_icon,
        Space::new().width(Length::Fixed(2.0)),
        case_label_text,
        Space::new().width(Length::Fill),
    ]
    .spacing(3)
    .align_y(iced::Alignment::Center);

    // Build the main content
    let mut content = column![search_row, options_row].spacing(6);

    // Add replace fields if in replace mode
    if search_state.is_replace_mode {
        let replace_input = text_input(
            &translations.replace_placeholder(),
            &search_state.replace_with,
        )
        .id(search_state.replace_input_id.clone())
        .on_input(Message::ReplaceQueryChanged)
        .size(POPUP_TEXT_SIZE)
        .style(crate::theme::popup_text_input)
        .padding(4)
        .width(Length::Fill);

        let replace_btn = Tooltip::new(
            button(fa_icon_solid("arrow-right-arrow-left").size(11.0))
                .on_press(Message::ReplaceNext)
                .padding(2)
                .style(crate::theme::popup_icon_button),
            text(translations.replace_current_tooltip()).size(POPUP_META_TEXT_SIZE),
            tooltip::Position::Bottom,
        )
        .delay(TOOLTIP_DELAY)
        .style(crate::theme::popup_tooltip);

        let replace_all_btn = Tooltip::new(
            button(fa_icon_solid("arrows-rotate").size(11.0))
                .on_press(Message::ReplaceAll)
                .padding(2)
                .style(crate::theme::popup_icon_button),
            text(translations.replace_all_tooltip()).size(POPUP_META_TEXT_SIZE),
            tooltip::Position::Bottom,
        )
        .delay(TOOLTIP_DELAY)
        .style(crate::theme::popup_tooltip);

        let replace_row = row![replace_input, replace_btn, replace_all_btn]
            .spacing(4)
            .align_y(iced::Alignment::Center);

        content = content.push(replace_row);
    }

    // Close button - small with Font Awesome icon and tooltip
    let close_button = Tooltip::new(
        button(text("×").size(16.0).line_height(1.0))
            .on_press(Message::CloseSearch)
            .padding(2)
            .style(crate::theme::popup_icon_button),
        text(translations.close_search_tooltip()).size(POPUP_META_TEXT_SIZE),
        tooltip::Position::Left,
    )
    .delay(TOOLTIP_DELAY)
    .style(crate::theme::popup_tooltip);

    // Title bar with close button - compact with magnifying glass icon
    let title_row = row![
        text("⌕")
            .size(15.0)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: crate::theme::popup_surface(theme).text_color
            }),
        Space::new().width(Length::Fixed(8.0)),
        text(title_label).size(POPUP_TEXT_SIZE),
        Space::new().width(Length::Fill),
        close_button
    ]
    .width(Length::Fixed(POPUP_WIDTH))
    .align_y(iced::Alignment::Center);

    // Final dialog container - minimal padding with semi-transparency
    let dialog = column![title_row, content].spacing(6).padding(8);

    // Custom style with 90% opacity for semi-transparency
    container(dialog)
        .padding(6)
        .width(Length::Fixed(POPUP_WIDTH + 12.0))
        .style(crate::theme::popup_surface)
        .into()
}
