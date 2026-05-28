use iced::widget::column;

use super::*;

pub(super) fn strip_processor_header(
    _strip_index: usize,
    instrument: Option<(
        crate::app::processor_editor_windows::EditorTarget,
        Option<&ProcessorChoice>,
        bool,
        Option<ProcessorSlotSegment>,
    )>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let mut content = row![]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center);
    if let Some((target, selected, editor_enabled, hovered_segment)) = instrument {
        content = content.push(processor_slot_controls(
            target,
            ProcessorSlotRole::Instrument,
            selected,
            editor_enabled,
            false,
            hovered_segment,
            controls_enabled,
        ));
    } else {
        content = content.push(container(text("")).width(Length::Fixed(PROCESSOR_SLOT_WIDTH)));
    }
    content.into()
}

pub(super) fn visible_strip_window(
    total: usize,
    scroll_x: f32,
    viewport_width: f32,
) -> std::ops::Range<usize> {
    if total == 0 {
        return 0..0;
    }

    let stride = STRIP_WIDTH + STRIP_SPACING;
    let first_visible = crate::number::f32_to_usize((scroll_x.max(0.0) / stride.max(1.0)).floor());
    let visible_count =
        crate::number::f32_to_usize((viewport_width.max(stride) / stride.max(1.0)).ceil())
            .saturating_add(STRIP_VIRTUALIZATION_OVERSCAN * 2);
    let start = first_visible
        .saturating_sub(STRIP_VIRTUALIZATION_OVERSCAN)
        .min(total);
    let end = start.saturating_add(visible_count).min(total);
    start..end
}

pub(super) fn strip_span_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * STRIP_WIDTH + count.saturating_sub(1) as f32 * STRIP_SPACING
    }
}

pub(super) fn horizontal_spacer(width: f32) -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(width.max(0.0)))
        .height(Fill)
        .into()
}

pub(super) fn section_header_bar<'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(row![
        container(text("")).width(Length::Fixed(HEADER_SIDE_INSET)),
        container(content.into())
            .height(Length::Fixed(SECTION_HEADER_HEIGHT))
            .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    ])
    .align_y(alignment::Vertical::Center)
    .height(Length::Fixed(SECTION_HEADER_HEIGHT))
    .width(Fill)
    .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    .into()
}

pub(super) fn value_label_slot<'a>(
    width: f32,
    label: impl Into<String>,
    color: Option<iced::Color>,
) -> Element<'a, Message> {
    let text = text(label.into()).size(ui_style::FONT_SIZE_UI_XS.saturating_sub(1));
    let text = if let Some(color) = color {
        text.color(color)
    } else {
        text
    };

    container(text)
        .width(Length::Fixed(width))
        .height(Length::Fixed(VALUE_LABEL_HEIGHT))
        .center_x(Length::Fixed(width))
        .align_y(alignment::Vertical::Bottom)
        .into()
}

pub(super) fn gain_label(gain_db: f32) -> String {
    if gain_db <= GAIN_MIN_DB {
        "-inf".to_string()
    } else {
        format!("{gain_db:.1}")
    }
}

pub(super) struct StripShellArgs<'a> {
    pub(super) title: Element<'a, Message>,
    pub(super) instrument_picker: Option<Element<'a, Message>>,
    pub(super) route_picker: Option<Element<'a, Message>>,
    pub(super) gain_db: f32,
    pub(super) pan: f32,
    pub(super) meter_stack: Element<'a, Message>,
    pub(super) actions: StripActions<'a>,
    pub(super) strip_height: f32,
    pub(super) gain_mode: GainControlMode,
    pub(super) show_gain_scale: bool,
}

pub(super) fn strip_shell<'a>(args: StripShellArgs<'a>) -> Element<'a, Message> {
    let StripShellArgs {
        title,
        instrument_picker,
        route_picker,
        gain_db,
        pan,
        meter_stack,
        actions,
        strip_height,
        gain_mode,
        show_gain_scale,
    } = args;
    let mut content = column![]
        .spacing(STRIP_STACK_SPACING)
        .align_x(alignment::Horizontal::Center)
        .width(Fill);

    content = content.push(
        container(instrument_picker.unwrap_or_else(|| container(text("")).into()))
            .width(Fill)
            .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT)),
    );

    if let Some(on_pan) = actions.on_pan {
        content = content.push(
            column![
                container(text("")).height(Length::Fixed(ui_style::SPACE_XS as f32)),
                value_label_slot(INSTRUMENT_PICKER_HEIGHT, format!("{:+.2}", pan), None),
                pan_knob(pan, on_pan),
            ]
            .spacing(LABEL_CONTROL_SPACING)
            .align_x(alignment::Horizontal::Center),
        );
    }

    if let Some(on_gain) = actions.on_gain {
        let control_height = gain_control_height(strip_height, gain_mode);
        let gain_width = gain_control_width(matches!(gain_mode, GainControlMode::Knob));
        let stack_height = control_stack_height(control_height);

        let gain_control = match gain_mode {
            GainControlMode::Fader => container(gain_fader(gain_db, on_gain))
                .width(Fill)
                .height(Length::Fixed(control_height))
                .center_x(Fill)
                .into(),
            GainControlMode::Knob => gain_knob(gain_db, on_gain),
        };

        let gain_column = column![
            value_label_slot(gain_width, gain_label(gain_db), None),
            container(gain_control)
                .width(Length::Fixed(gain_width))
                .height(Length::Fixed(control_height))
                .center_x(Length::Fixed(gain_width))
                .align_y(alignment::Vertical::Bottom),
        ]
        .spacing(LABEL_CONTROL_SPACING)
        .height(Length::Fixed(stack_height))
        .align_x(alignment::Horizontal::Center)
        .width(Length::Shrink);

        let gain_controls: Element<'a, Message> =
            if matches!(gain_mode, GainControlMode::Fader) && show_gain_scale {
                row![
                    column![
                        container(text("")).height(Length::Fixed(VALUE_LABEL_HEIGHT)),
                        gain_fader_scale(control_height),
                    ]
                    .spacing(LABEL_CONTROL_SPACING)
                    .height(Length::Fixed(stack_height))
                    .width(Length::Fixed(gain_fader_scale_width())),
                    gain_column,
                ]
                .spacing(GAIN_SCALE_SPACING)
                .height(Length::Fixed(stack_height))
                .width(Length::Shrink)
                .into()
            } else {
                gain_column.into()
            };

        content = content.push(
            row![gain_controls, meter_stack]
                .spacing(METER_STACK_SPACING)
                .height(Length::Fixed(stack_height))
                .width(Length::Shrink),
        );
    }

    content = content.push(
        container(
            row![
                actions
                    .mute
                    .map_or_else(strip_toggle_placeholder, |(active, message)| {
                        strip_toggle_button("M", active, message)
                    },),
                actions
                    .solo
                    .map_or_else(strip_toggle_placeholder, |(active, message)| {
                        strip_toggle_button("S", active, message)
                    },),
                actions.panel.map_or_else(
                    strip_toggle_placeholder,
                    |(active, has_content, message)| {
                        strip_panel_toggle_button(active, has_content, message)
                    },
                ),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .center_x(Fill),
    );

    let title = container(title)
        .width(Fill)
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .align_y(alignment::Vertical::Center);

    container(
        column![
            container(content).width(Fill),
            container(text("")).height(Length::Fixed(TITLE_TOP_SPACING)),
            title,
            container(text("")).height(Length::Fixed(ROUTE_PICKER_TOP_SPACING)),
            container(route_picker.unwrap_or_else(|| container(text("")).into()))
                .width(Fill)
                .height(Length::Fixed(ROUTE_PICKER_HEIGHT))
                .center_y(Length::Fixed(ROUTE_PICKER_HEIGHT)),
            container(text("")).height(Length::Fixed(ROUTE_PICKER_BOTTOM_INSET)),
        ]
        .spacing(0)
        .width(Fill),
    )
    .padding(ui_style::PADDING_SM)
    .width(Fill)
    .height(Length::Fixed(strip_height))
    .style(ui_style::transparent_surface)
    .into()
}

pub(super) fn track_title_content<'a>(
    track_index: usize,
    title: &str,
    renaming: bool,
    rename_value: &str,
    color: Color,
    color_picker_open: bool,
) -> Element<'a, Message> {
    if renaming {
        let swatch = button(
            container(text(""))
                .width(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE))
                .height(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE)),
        )
        .padding(0)
        .width(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE))
        .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
        .style(move |theme, status| ui_style::track_color_swatch_button(theme, status, color))
        .on_press(Message::Mixer(MixerMessage::OpenTrackColorPicker));
        let input = text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
            .id(Id::new(crate::app::TRACK_RENAME_INPUT_ID))
            .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
            .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
            .style(ui_style::track_name_input)
            .size(ui_style::FONT_SIZE_UI_SM)
            .padding([
                TRACK_TITLE_EDITOR_INPUT_PADDING_V,
                TRACK_TITLE_EDITOR_INPUT_PADDING_H,
            ])
            .width(Fill);
        let focused = color_picker_open;
        let editor_row = container(
            row![
                swatch,
                container(text(""))
                    .width(1)
                    .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
                    .style(move |theme| { ui_style::track_name_editor_divider(theme, focused) }),
                container(input)
                    .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
                    .center_y(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
            ]
            .spacing(0)
            .align_y(alignment::Vertical::Center)
            .width(Fill),
        )
        .padding(0)
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .style(move |theme| ui_style::track_name_editor_shell(theme, focused))
        .width(Fill);
        return color_picker_with_change(
            color_picker_open,
            color,
            editor_row,
            Message::Mixer(MixerMessage::CancelTrackRename),
            |color| Message::Mixer(MixerMessage::SubmitTrackColor(color)),
            |color| Message::Mixer(MixerMessage::PreviewTrackColor(color)),
        )
        .style(ui_style::color_picker_widget_style)
        .into();
    }

    mouse_area(
        container(
            text(crate::track_names::ellipsize_middle(title, 18))
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                })
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .center_x(Fill),
    )
    .on_press(Message::Mixer(MixerMessage::StartTrackRename(track_index)))
    .into()
}

pub(super) fn bus_title_content<'a>(
    bus_id: u16,
    title: &str,
    renaming: bool,
    rename_value: &str,
) -> Element<'a, Message> {
    if renaming {
        return container(
            text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
                .id(Id::new(crate::app::TRACK_RENAME_INPUT_ID))
                .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
                .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
                .size(ui_style::FONT_SIZE_UI_SM)
                .padding([
                    TRACK_TITLE_EDITOR_INPUT_PADDING_V,
                    TRACK_TITLE_EDITOR_INPUT_PADDING_H,
                ])
                .width(Fill),
        )
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .center_y(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .into();
    }

    mouse_area(
        container(
            text(crate::track_names::ellipsize_middle(title, 18))
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                })
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .center_x(Fill),
    )
    .on_press(Message::Mixer(MixerMessage::StartBusRename(bus_id)))
    .into()
}

pub(super) fn meter_stack<'a>(
    dependency: MeterStackDependency,
    meter_reset: Option<Message>,
) -> Element<'a, Message> {
    lazy(dependency, move |dependency| -> Element<'static, Message> {
        let gain_mode = if dependency.compact_gain {
            GainControlMode::Knob
        } else {
            GainControlMode::Fader
        };
        let strip_height = f32::from_bits(dependency.strip_height_bits);
        let meter_snapshot = dependency.meter.snapshot();
        let meter_colors = dependency.colors.colors();
        let meter_height = meter_control_height(strip_height, gain_mode);
        let meter_width = stereo_meter_width(meter_scale_visible(gain_mode));
        let meter_bar_width = stereo_meter_bar_width();
        let meter_scale_width = (meter_width - meter_bar_width).max(0.0);
        let meter_label = meter_peak_label(meter_snapshot);
        let meter_label_color = if meter_snapshot.clip_latched {
            meter_colors.clip
        } else {
            meter_colors.scale_text
        };
        let meter = if meter_scale_visible(gain_mode) {
            stereo_meter_with_scale(meter_snapshot, meter_colors, meter_height)
        } else {
            stereo_meter(meter_snapshot, meter_colors, meter_height)
        };
        let meter = if let Some(message) = meter_reset.clone() {
            mouse_area(meter).on_press(message).into()
        } else {
            meter
        };

        column![
            row![
                value_label_slot(meter_bar_width, meter_label, Some(meter_label_color)),
                container(text("")).width(Length::Fixed(meter_scale_width)),
            ]
            .width(Length::Fixed(meter_width))
            .height(Length::Fixed(VALUE_LABEL_HEIGHT))
            .align_y(alignment::Vertical::Bottom),
            container(meter)
                .width(Length::Fixed(meter_width))
                .height(Length::Fixed(meter_height))
                .center_x(Length::Fixed(meter_width))
                .align_y(alignment::Vertical::Bottom),
        ]
        .spacing(LABEL_CONTROL_SPACING)
        .height(Length::Fixed(control_stack_height(meter_height)))
        .align_x(alignment::Horizontal::Center)
        .width(Length::Shrink)
        .into()
    })
    .into()
}

pub(super) fn gain_control_height(strip_height: f32, gain_mode: GainControlMode) -> f32 {
    match gain_mode {
        GainControlMode::Knob => 48.0,
        GainControlMode::Fader => (strip_height
            - (ui_style::PADDING_SM as f32 * 2.0)
            - SECTION_HEADER_HEIGHT
            - INSTRUMENT_PICKER_HEIGHT
            - STRIP_TOGGLE_SIZE
            - STRIP_FOOTER_HEIGHT
            - 30.0
            - (VALUE_LABEL_HEIGHT * 3.0)
            - (ui_style::SPACE_XS as f32 * 6.0))
            .max(96.0),
    }
}

pub(super) fn meter_control_height(strip_height: f32, gain_mode: GainControlMode) -> f32 {
    gain_control_height(strip_height, gain_mode)
}

pub(super) fn control_stack_height(control_height: f32) -> f32 {
    control_height + VALUE_LABEL_HEIGHT + ui_style::SPACE_XS as f32
}

pub(super) fn meter_peak_label(snapshot: StripMeterSnapshot) -> String {
    let hold_db = snapshot.left.hold_db.max(snapshot.right.hold_db);
    if hold_db <= STRIP_METER_MIN_DB {
        "-inf".to_string()
    } else {
        format!("{hold_db:.1}")
    }
}

pub(super) fn gain_control_mode(pane_height: f32) -> GainControlMode {
    if pane_height <= MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET {
        GainControlMode::Knob
    } else {
        GainControlMode::Fader
    }
}

pub(super) fn meter_scale_visible(gain_mode: GainControlMode) -> bool {
    matches!(gain_mode, GainControlMode::Fader)
}

pub(super) fn strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    selected: bool,
    on_select: Option<Message>,
) -> Element<'a, Message> {
    track_strip_panel(content, width, height, None, selected, on_select)
}

pub(super) fn tinted_track_strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    track_color: Color,
    selected: bool,
    on_select: Option<Message>,
) -> Element<'a, Message> {
    track_strip_panel(
        content,
        width,
        height,
        Some(track_color),
        selected,
        on_select,
    )
}

fn track_strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    track_color: Option<Color>,
    selected: bool,
    on_select: Option<Message>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if let Some(message) = on_select {
        stack![
            mouse_area(container(text("")).width(Fill).height(Fill)).on_press(message),
            content
        ]
        .into()
    } else {
        content
    };

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |theme| ui_style::mixer_track_strip_surface(theme, track_color, selected))
        .into()
}

pub(super) fn track_should_use_roll_tint(track_index: usize, existing_track_count: usize) -> bool {
    track_index < existing_track_count
}

pub(super) fn strip_toggle_button(
    label: &'static str,
    active: bool,
    message: Message,
) -> Element<'static, Message> {
    button(
        container(text(label).size(ui_style::FONT_SIZE_UI_XS))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    )
    .style(if active {
        ui_style::button_compact_active
    } else {
        ui_style::button_compact_solid
    })
    .padding(0)
    .width(Length::Fixed(STRIP_TOGGLE_SIZE))
    .height(Length::Fixed(STRIP_TOGGLE_SIZE))
    .on_press(message)
    .into()
}

pub(super) fn strip_panel_toggle_button(
    active: bool,
    has_content: bool,
    message: Message,
) -> Element<'static, Message> {
    button(
        container(ui_style::icon(
            icons::cable(),
            ui_style::grid_f32(3),
            move |theme, status| strip_panel_toggle_icon_style(theme, status, active, has_content),
        ))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill),
    )
    .style(move |theme, status| strip_panel_toggle_style(theme, status, active))
    .padding(0)
    .width(Length::Fixed(STRIP_TOGGLE_SIZE))
    .height(Length::Fixed(STRIP_TOGGLE_SIZE))
    .on_press(message)
    .into()
}

pub(super) fn strip_panel_toggle_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
    active: bool,
    has_content: bool,
) -> iced::widget::svg::Style {
    if active || has_content {
        return processor_slot_icon_style(theme, status, true);
    }

    processor_slot_icon_style(theme, status, false)
}

pub(super) fn strip_panel_toggle_style(
    theme: &iced::Theme,
    status: button::Status,
    active: bool,
) -> button::Style {
    if active {
        return ui_style::button_flat_compact_control(
            theme,
            if matches!(status, button::Status::Disabled) {
                status
            } else {
                button::Status::Hovered
            },
        );
    }
    ui_style::button_flat_compact_control(theme, status)
}

pub(super) fn strip_toggle_placeholder() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(STRIP_TOGGLE_SIZE))
        .height(Length::Fixed(STRIP_TOGGLE_SIZE))
        .into()
}

#[cfg(test)]
pub(super) fn slot_selector_controls(
    label: String,
    primary_action: Option<Message>,
    secondary_action: Option<Message>,
) -> Element<'static, Message> {
    let editor_segment = button(
        container(ui_style::icon(
            icons::keyboard_music(),
            INSTRUMENT_BROWSER_ICON_SIZE,
            ui_style::svg_muted_control,
        ))
        .width(Length::Fixed(INSTRUMENT_SLOT_EDITOR_AREA_WIDTH))
        .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
        .center_x(Length::Fixed(INSTRUMENT_SLOT_EDITOR_AREA_WIDTH))
        .center_y(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT)),
    )
    .style(transparent_hit_button)
    .padding(0)
    .width(Length::Fixed(INSTRUMENT_SLOT_EDITOR_AREA_WIDTH))
    .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
    .on_press_maybe(primary_action);

    let picker_segment = button(
        container(
            text(label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
        .clip(true)
        .center_x(Fill)
        .align_y(alignment::Vertical::Center),
    )
    .style(transparent_hit_button)
    .padding(0)
    .width(Fill)
    .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
    .on_press_maybe(secondary_action.clone());

    let surface: Element<'static, Message> = button(
        container(
            row![
                editor_segment,
                slot_area_separator(),
                picker_segment,
                container(text(""))
                    .width(Length::Fixed(INSTRUMENT_BROWSER_ICON_SIZE))
                    .height(Fill),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT)),
    )
    .style(|theme, status| ui_style::button_selector_field(theme, status, false))
    .padding([0, ui_style::grid(2)])
    .width(Length::Fixed(INSTRUMENT_SLOT_WIDTH))
    .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
    .on_press(Message::Noop)
    .into();

    let button = if let Some(message) = secondary_action {
        mouse_area(surface).on_right_press(message).into()
    } else {
        surface
    };

    container(button)
        .width(Fill)
        .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(INSTRUMENT_PICKER_HEIGHT))
        .into()
}
