use iced::widget::column;

use super::*;

pub(super) fn effect_rack(
    strip_index: usize,
    effects: Vec<EffectSlotDependency>,
    hovered_processor_slot: Option<(usize, ProcessorSlotSegment)>,
    effect_drag: Option<EffectRackDragState>,
    controls_enabled: bool,
    min_slots: usize,
) -> Element<'static, Message> {
    let indicator = effect_rack_drop_indicator(effect_drag);
    let labels = effect_slot_display_labels(&effects);
    let mut content = column![].spacing(0).width(Fill);
    if indicator == Some(EffectRackDropIndicator::Top) {
        content = content.push(effect_rack_drop_indicator_line());
    }
    for (effect, label) in effects.iter().zip(labels) {
        let effect_index = effect.slot_index - 1;
        let target = crate::app::processor_editor_windows::EditorTarget {
            strip_index,
            slot_index: effect.slot_index,
        };
        content = content.push(effect_rack_draggable_filled_slot(
            target,
            processor_slot_controls_sized(ProcessorSlotControlArgs {
                target,
                role: ProcessorSlotRole::Effect,
                selected: effect.selected.as_ref(),
                editor_enabled: effect.editor_enabled,
                bypassed: effect.bypassed,
                hovered_segment: hovered_processor_slot
                    .filter(|(slot_index, _)| *slot_index == effect.slot_index)
                    .map(|(_, segment)| segment),
                controls_enabled,
                slot_width: EFFECT_RACK_SLOT_WIDTH,
                list_item: true,
                label_override: label,
            }),
            indicator == Some(EffectRackDropIndicator::After(effect_index)),
            controls_enabled,
        ));
    }

    let slot_count = min_slots.max(effects.len() + 1);
    for slot_index in effects.len() + 1..=slot_count {
        let add_target = crate::app::processor_editor_windows::EditorTarget {
            strip_index,
            slot_index,
        };
        let first_empty = slot_index == effects.len() + 1;
        content = content.push(effect_rack_empty_slot(
            add_target,
            first_empty,
            controls_enabled,
        ));
    }

    let rack: Element<'static, Message> = container(
        scrollable(content)
            .id(effect_rack_scroll_id(strip_index))
            .width(Fill)
            .on_scroll(move |viewport| {
                Message::Mixer(MixerMessage::EffectRackViewportScrolled {
                    strip_index,
                    viewport,
                })
            })
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new()
                    .width(EFFECT_RACK_SCROLLBAR_WIDTH)
                    .scroller_width(EFFECT_RACK_SCROLLBAR_SCROLLER_WIDTH)
                    .spacing(EFFECT_RACK_SCROLLBAR_SPACING)
                    .margin(EFFECT_RACK_SCROLLBAR_MARGIN),
            ))
            .style(effect_rack_scrollable),
    )
    .width(Fill)
    .height(Fill)
    .style(effect_rack_surface)
    .into();

    container(
        mouse_area(rack)
            .on_move(move |position| {
                Message::Mixer(MixerMessage::TrackEffectDragMoved {
                    strip_index,
                    y: position.y,
                })
            })
            .on_exit(Message::Mixer(MixerMessage::EffectRackCursorLeft(
                strip_index,
            ))),
    )
    .width(Fill)
    .height(Fill)
    .into()
}

pub(super) fn send_panel(
    source: RoutingStrip,
    sends: Vec<SendDependency>,
    choices: Vec<SendDestinationChoice>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let mut content = column![
        container(text("")).height(Length::Fixed(SEND_PANEL_TOP_SPACING)),
        container(
            row![
                text("Sends")
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..fonts::UI
                    }),
                container(text("")).width(Fill),
                send_add_button(source, first_send_bus_id(&choices), controls_enabled)
            ]
            .align_y(alignment::Vertical::Center)
        )
        .height(Length::Fixed(SEND_PANEL_HEADER_HEIGHT))
        .padding([0, ui_style::grid(2)])
        .center_y(Length::Fixed(SEND_PANEL_HEADER_HEIGHT))
    ]
    .spacing(0)
    .width(Fill);

    if choices.is_empty() {
        content = content.push(
            container(text("No buses").size(ui_style::FONT_SIZE_UI_XS))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill),
        );
    } else {
        for (index, send) in sends.into_iter().enumerate() {
            content = content.push(send_row(
                source,
                index,
                send,
                choices.clone(),
                controls_enabled,
            ));
        }
    }

    container(scrollable(content).style(effect_rack_scrollable))
        .width(Fill)
        .height(Fill)
        .style(effect_rack_surface)
        .into()
}

pub(super) fn send_add_button(
    source: RoutingStrip,
    first_bus_id: Option<u16>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    ui_style::flat_icon_button(
        icons::plus(),
        ui_style::grid_f32(4),
        ui_style::grid_f32(3),
        ui_style::button_flat_compact_control,
        ui_style::svg_dimmed_control,
    )
    .width(Length::Fixed(ui_style::grid_f32(5)))
    .height(Length::Fixed(ui_style::grid_f32(5)))
    .on_press_maybe(
        (controls_enabled)
            .then_some(first_bus_id)
            .flatten()
            .map(|bus_id| Message::Mixer(MixerMessage::AddSend(source, bus_id))),
    )
    .into()
}

pub(super) fn send_row(
    source: RoutingStrip,
    index: usize,
    send: SendDependency,
    choices: Vec<SendDestinationChoice>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let choices = send_menu_choices(choices);
    let selected = selected_send_destination_choice(send.bus_id, &choices);
    let enabled = send.enabled;
    let pre_fader = send.pre_fader;
    let gain = f32::from_bits(send.gain_bits);
    let gain_text = gain_label(gain);

    let content_height = SEND_ROW_HEIGHT - EFFECT_RACK_SEPARATOR_HEIGHT;
    container(
        column![
            container(
                column![
                    row![
                        send_icon_button(
                            if enabled {
                                icons::power()
                            } else {
                                icons::power_off()
                            },
                            Message::Mixer(MixerMessage::ToggleSendEnabled(source, index)),
                            controls_enabled,
                        ),
                        container(
                            pick_list(choices.clone(), selected, move |choice| {
                                if !controls_enabled {
                                    return noop_message();
                                }
                                match choice.action {
                                    SendDestinationAction::Route(bus_id) => Message::Mixer(
                                        MixerMessage::SetSendDestination(source, index, bus_id),
                                    ),
                                    SendDestinationAction::Remove => {
                                        Message::Mixer(MixerMessage::RemoveSend(source, index))
                                    }
                                }
                            })
                            .placeholder("Bus")
                            .width(Length::Fixed(SEND_PICKER_WIDTH))
                            .menu_height(Length::Fixed(route_menu_height_for_items(choices.len())))
                            .padding([ui_style::grid(1), ui_style::grid(2)])
                            .text_size(ui_style::FONT_SIZE_UI_XS)
                            .font(fonts::UI)
                            .style(route_pick_list_style)
                            .menu_style(route_pick_list_menu_style),
                        )
                        .height(Length::Fixed(SEND_CONTROL_HEIGHT))
                        .center_y(Length::Fixed(SEND_CONTROL_HEIGHT)),
                        send_mode_button(source, index, pre_fader, controls_enabled),
                    ]
                    .spacing(ui_style::SPACE_XS)
                    .align_y(alignment::Vertical::Center),
                    row![
                        send_gain_slider(source, index, gain, controls_enabled),
                        container(text(gain_text).size(ui_style::FONT_SIZE_UI_XS))
                            .width(Length::Fixed(ui_style::grid_f32(7)))
                            .align_y(alignment::Vertical::Center),
                    ]
                    .spacing(ui_style::SPACE_XS)
                    .align_y(alignment::Vertical::Center),
                ]
                .spacing(ui_style::SPACE_XS)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Fill)
            .height(Length::Fixed(content_height))
            .padding(Padding {
                top: 0.0,
                right: ui_style::grid_f32(2),
                bottom: SEND_ROW_CONTENT_BOTTOM_SPACING,
                left: ui_style::grid_f32(2),
            })
            .center_y(Length::Fixed(content_height)),
            effect_rack_separator(),
        ]
        .spacing(0)
        .width(Fill),
    )
    .width(Fill)
    .height(Length::Fixed(SEND_ROW_HEIGHT))
    .into()
}

pub(super) fn send_gain_slider(
    source: RoutingStrip,
    index: usize,
    gain: f32,
    controls_enabled: bool,
) -> Element<'static, Message> {
    horizontal_slider(
        super::super::controls::HorizontalSliderSpec {
            value: gain,
            min: SEND_GAIN_MIN_DB,
            max: SEND_GAIN_MAX_DB,
            step: SEND_GAIN_STEP_DB,
            default_value: 0.0,
            metrics: COMPACT_HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::GainDb {
                max: SEND_GAIN_MAX_DB,
            },
        },
        move |value| {
            if controls_enabled {
                Message::Mixer(MixerMessage::SetSendGain(source, index, value))
            } else {
                noop_message()
            }
        },
    )
}

pub(super) fn send_icon_button(
    icon: iced::widget::svg::Handle,
    message: Message,
    controls_enabled: bool,
) -> Element<'static, Message> {
    ui_style::flat_icon_button(
        icon,
        ui_style::grid_f32(4),
        ui_style::grid_f32(3),
        ui_style::button_flat_compact_control,
        ui_style::svg_dimmed_control,
    )
    .width(Length::Fixed(ui_style::grid_f32(5)))
    .height(Length::Fixed(ui_style::grid_f32(5)))
    .on_press_maybe(controls_enabled.then_some(message))
    .into()
}

pub(super) fn send_mode_button(
    source: RoutingStrip,
    index: usize,
    pre_fader: bool,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let label = if pre_fader { "Pre" } else { "Post" };
    button(
        container(
            text(label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .line_height(1.0)
                .align_x(alignment::Horizontal::Center),
        )
        .width(Fill)
        .height(Length::Fixed(SEND_MODE_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(SEND_MODE_HEIGHT)),
    )
    .style(move |theme, status| ui_style::button_pane_tab(theme, status, pre_fader))
    .padding([0, 0])
    .width(Length::Fixed(SEND_MODE_WIDTH))
    .height(Length::Fixed(SEND_MODE_HEIGHT))
    .on_press_maybe(
        controls_enabled.then_some(Message::Mixer(MixerMessage::ToggleSendPreFader(
            source, index,
        ))),
    )
    .into()
}

pub(super) fn processor_slot_controls(
    target: crate::app::processor_editor_windows::EditorTarget,
    role: ProcessorSlotRole,
    selected: Option<&ProcessorChoice>,
    editor_enabled: bool,
    bypassed: bool,
    hovered_segment: Option<ProcessorSlotSegment>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    processor_slot_controls_sized(ProcessorSlotControlArgs {
        target,
        role,
        selected,
        editor_enabled,
        bypassed,
        hovered_segment,
        controls_enabled,
        slot_width: PROCESSOR_SLOT_WIDTH,
        list_item: false,
        label_override: None,
    })
}

pub(super) fn effect_rack_empty_slot(
    target: crate::app::processor_editor_windows::EditorTarget,
    add_slot: bool,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let picker_action = (controls_enabled && add_slot)
        .then_some(Message::Mixer(MixerMessage::ToggleProcessorBrowser(target)));

    if !add_slot {
        return container(effect_rack_separator())
            .width(Length::Fixed(EFFECT_RACK_SLOT_WIDTH))
            .height(Length::Fixed(EFFECT_RACK_ROW_HEIGHT))
            .align_y(alignment::Vertical::Bottom)
            .into();
    }

    let add_button: Element<'static, Message> = button(
        container(
            row![
                container(ui_style::icon(
                    icons::plus(),
                    PROCESSOR_BROWSER_ICON_SIZE,
                    effect_rack_add_icon_style,
                ))
                .width(Length::Fixed(PROCESSOR_SLOT_SEGMENT_WIDTH))
                .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
                .center_x(Length::Fixed(PROCESSOR_SLOT_SEGMENT_WIDTH))
                .center_y(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT)),
                text("Add effect")
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .wrapping(iced::widget::text::Wrapping::None)
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT)),
    )
    .padding([0, ui_style::grid(2)])
    .style(effect_rack_add_button_style)
    .width(Length::Fixed(EFFECT_RACK_SLOT_WIDTH))
    .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
    .on_press_maybe(picker_action)
    .into();

    effect_rack_filled_slot(add_button)
}

pub(super) fn effect_rack_filled_slot(
    content: Element<'static, Message>,
) -> Element<'static, Message> {
    effect_rack_filled_slot_with_separator(content, effect_rack_separator())
}

pub(super) fn effect_rack_filled_slot_with_separator(
    content: Element<'static, Message>,
    separator: Element<'static, Message>,
) -> Element<'static, Message> {
    column![content, separator]
        .spacing(0)
        .width(Length::Fixed(EFFECT_RACK_SLOT_WIDTH))
        .into()
}

pub(super) fn effect_rack_draggable_filled_slot(
    target: crate::app::processor_editor_windows::EditorTarget,
    content: Element<'static, Message>,
    drop_indicator_after: bool,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let effect_index = target.slot_index - 1;
    let separator = if drop_indicator_after {
        effect_rack_drop_indicator_line()
    } else {
        effect_rack_separator()
    };
    let mut row = mouse_area(effect_rack_filled_slot_with_separator(content, separator))
        .on_move(move |position| {
            Message::Mixer(MixerMessage::TrackEffectDragMoved {
                strip_index: target.strip_index,
                y: EFFECT_RACK_ROW_HEIGHT * effect_index as f32 + position.y,
            })
        })
        .interaction(mouse::Interaction::Pointer);
    if controls_enabled {
        row = row
            .on_enter(Message::Mixer(MixerMessage::TrackEffectDragMoved {
                strip_index: target.strip_index,
                y: EFFECT_RACK_ROW_HEIGHT * (effect_index as f32 + 0.5),
            }))
            .on_press(Message::Mixer(MixerMessage::StartTrackEffectDrag {
                strip_index: target.strip_index,
                effect_index,
            }))
            .on_release(Message::Mixer(MixerMessage::DropTrackEffect {
                strip_index: target.strip_index,
                effect_index,
            }));
    }

    container(row)
        .width(Length::Fixed(EFFECT_RACK_SLOT_WIDTH))
        .height(Length::Fixed(EFFECT_RACK_ROW_HEIGHT))
        .into()
}

pub(super) fn effect_rack_separator() -> Element<'static, Message> {
    effect_rack_rule(effect_rack_separator_surface)
}

pub(super) fn effect_rack_drop_indicator_line() -> Element<'static, Message> {
    effect_rack_rule(effect_rack_drop_indicator_surface)
}

fn effect_rack_rule(style: fn(&iced::Theme) -> container::Style) -> Element<'static, Message> {
    container(
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(EFFECT_RACK_SEPARATOR_HEIGHT))
            .style(style),
    )
    .padding([0, crate::number::f32_to_u16(EFFECT_RACK_SEPARATOR_INSET)])
    .width(Fill)
    .height(Length::Fixed(EFFECT_RACK_SEPARATOR_HEIGHT))
    .into()
}

pub(super) fn effect_rack_add_button_style(
    theme: &iced::Theme,
    status: button::Status,
) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: None,
        text_color: slot_segment_foreground(theme, hovered, false),
        border: border::rounded(0).width(0),
        shadow: iced::Shadow::default(),
        ..button::Style::default()
    }
}

pub(super) fn effect_rack_add_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
) -> iced::widget::svg::Style {
    let hovered = matches!(status, iced::widget::svg::Status::Hovered);
    iced::widget::svg::Style {
        color: Some(slot_segment_foreground(theme, hovered, false)),
    }
}

pub(super) fn processor_slot_segment_button_style(
    theme: &iced::Theme,
    status: button::Status,
    active: bool,
    slot_hovered: bool,
) -> button::Style {
    let hovered =
        slot_hovered || matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: None,
        text_color: slot_segment_foreground(theme, hovered, active),
        ..button::Style::default()
    }
}

pub(super) fn processor_slot_segment_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
    active: bool,
    slot_hovered: bool,
) -> iced::widget::svg::Style {
    let hovered = slot_hovered || matches!(status, iced::widget::svg::Status::Hovered);
    iced::widget::svg::Style {
        color: Some(slot_segment_foreground(theme, hovered, active)),
    }
}

pub(super) fn processor_slot_label_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
    active: bool,
    slot_hovered: bool,
) -> iced::widget::svg::Style {
    processor_slot_segment_icon_style(theme, status, active, slot_hovered)
}

pub(super) fn processor_slot_active_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
    _active: bool,
    slot_hovered: bool,
) -> iced::widget::svg::Style {
    processor_slot_segment_icon_style(theme, status, false, slot_hovered)
}

pub(super) fn processor_slot_label_button_style(
    theme: &iced::Theme,
    status: button::Status,
    active: bool,
    slot_hovered: bool,
) -> button::Style {
    processor_slot_segment_button_style(theme, status, active, slot_hovered)
}

pub(super) fn processor_slot_icon_button_style(
    theme: &iced::Theme,
    status: button::Status,
    slot_hovered: bool,
) -> button::Style {
    processor_slot_segment_button_style(theme, status, false, slot_hovered)
}

#[cfg(test)]
pub(super) fn transparent_hit_button(theme: &iced::Theme, status: button::Status) -> button::Style {
    processor_slot_segment_button_style(theme, status, false, false)
}

pub(super) fn slot_segment_foreground(theme: &iced::Theme, hovered: bool, active: bool) -> Color {
    let palette = theme.extended_palette();
    if active {
        let accent = if hovered {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };
        let text = palette.background.weak.text;
        let amount = 0.45;
        return Color {
            r: accent.r + (text.r - accent.r) * amount,
            g: accent.g + (text.g - accent.g) * amount,
            b: accent.b + (text.b - accent.b) * amount,
            a: accent.a + (text.a - accent.a) * amount,
        };
    }
    if hovered {
        return palette.background.weak.text;
    }

    let color = palette.background.weak.text;
    let background = palette.background.weak.color;
    let amount = 0.38;
    Color {
        r: color.r + (background.r - color.r) * amount,
        g: color.g + (background.g - color.g) * amount,
        b: color.b + (background.b - color.b) * amount,
        a: color.a + (background.a - color.a) * amount,
    }
}

pub(super) fn effect_rack_surface(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        ..container::Style::default()
    }
}

pub(super) fn effect_rack_scrollable(
    theme: &iced::Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    let palette = theme.extended_palette();
    let mut style = ui_style::workspace_scrollable(theme, status);
    style.container.background = Some(palette.background.base.color.into());
    style.container.text_color = Some(palette.background.base.text);
    style.vertical_rail.background = Some(palette.background.base.color.into());
    style.vertical_rail.scroller.background = palette.background.weak.color.into();
    style
}

pub(super) fn effect_rack_separator_surface(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        ..container::Style::default()
    }
}

pub(super) fn effect_rack_drop_indicator_surface(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.primary.strong.color.into()),
        ..container::Style::default()
    }
}
