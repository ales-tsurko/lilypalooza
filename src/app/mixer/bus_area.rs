use iced::widget::column;

use super::*;

pub(super) struct BusTrackAreaArgs<'a> {
    pub(super) mixer: &'a MixerState,
    pub(super) meters: &'a MixerMeterSnapshotWindow,
    pub(super) colors: MeterColors,
    pub(super) strip_height: f32,
    pub(super) gain_mode: GainControlMode,
    pub(super) visible: std::ops::Range<usize>,
    pub(super) open_effect_rack_strips: &'a [usize],
    pub(super) hovered_processor_slot: Option<(
        crate::app::processor_editor_windows::EditorTarget,
        ProcessorSlotSegment,
    )>,
    pub(super) effect_drag_source: Option<(usize, usize)>,
    pub(super) effect_drag_target: Option<(usize, usize)>,
    pub(super) renaming_target: Option<crate::app::RenameTarget>,
    pub(super) renaming_origin: Option<crate::app::WorkspacePaneKind>,
    pub(super) track_rename_value: &'a str,
    pub(super) controls_enabled: bool,
}

pub(super) fn bus_track_area(args: BusTrackAreaArgs<'_>) -> Element<'static, Message> {
    let BusTrackAreaArgs {
        mixer,
        meters,
        colors,
        strip_height,
        gain_mode,
        visible,
        open_effect_rack_strips,
        hovered_processor_slot,
        effect_drag_source,
        effect_drag_target,
        renaming_target,
        renaming_origin,
        track_rename_value,
        controls_enabled,
    } = args;
    if mixer.buses().is_empty() {
        return column![
            container(section_header_bar(row![section_title("Buses")]))
                .style(ui_style::workspace_toolbar_surface),
            container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
            row![
                container(text(""))
                    .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                    .height(Fill)
                    .style(ui_style::chrome_separator),
                container(add_bus_button(controls_enabled))
                    .width(Fill)
                    .height(Fill)
                    .center_x(Fill)
                    .center_y(Fill),
                container(text(""))
                    .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                    .height(Fill)
                    .style(ui_style::chrome_separator),
            ]
            .height(Fill),
            container(text(""))
                .width(Fill)
                .height(Length::Fixed(1.0))
                .style(ui_style::chrome_separator)
        ]
        .spacing(0)
        .height(Fill)
        .into();
    }

    let total_buses = mixer.buses().len();
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = if visible.end < total_buses {
        strip_span_width(total_buses.saturating_sub(visible.end) + 1)
    } else {
        0.0
    };
    let bus_row = mixer
        .buses()
        .get(visible.clone())
        .unwrap_or(&[])
        .iter()
        .enumerate()
        .fold(
            row![]
                .spacing(STRIP_SPACING)
                .align_y(alignment::Vertical::Top)
                .height(Length::Fixed(strip_height))
                .push(horizontal_spacer(left_spacer)),
            |row, (local_index, bus)| {
                let Some(bus_id) = bus.bus_id else {
                    return row;
                };
                let strip_index = 1 + mixer.track_count() + visible.start + local_index;
                let effect_rack_open = open_effect_rack_strips.contains(&strip_index);
                let effects = effect_slot_dependencies(bus);
                let routing_strip = RoutingStrip::Bus(bus_id.0);
                let route_choices = route_choices(mixer, routing_strip);
                let selected_route = selected_route_choice(bus.routing.main, &route_choices);
                let meter_dependency = MeterStackDependency {
                    meter: MeterDependency::from_snapshot(
                        meters.buses.get(local_index).copied().unwrap_or_default(),
                    ),
                    colors: MeterColorsDependency::from_colors(colors),
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                };
                let row = row.push(lazy(
                    BusStripDependency {
                        strip_index,
                        id: bus_id.0,
                        name: bus.name.clone(),
                        effects: effects.clone(),
                        effect_rack_open,
                        gain_bits: bus.state.gain_db.to_bits(),
                        pan_bits: bus.state.pan.to_bits(),
                        route: selected_route,
                        route_choices,
                        meter: meter_dependency.meter,
                        compact_gain: matches!(gain_mode, GainControlMode::Knob),
                        strip_height_bits: strip_height.to_bits(),
                        panel_has_content: !effects.is_empty()
                            || !bus.routing.sends.is_empty()
                            || bus.routing.main != TrackRoute::Master,
                        soloed: bus.state.soloed,
                        muted: bus.state.muted,
                        renaming: renaming_target == Some(crate::app::RenameTarget::Bus(bus_id.0))
                            && renaming_origin == Some(crate::app::WorkspacePaneKind::Mixer),
                        rename_value: track_rename_value.to_string(),
                    },
                    move |dependency| {
                        let name = dependency.name.clone();
                        let strip_index = dependency.strip_index;
                        let bus_id = dependency.id;
                        let gain_db = f32::from_bits(dependency.gain_bits);
                        let pan = f32::from_bits(dependency.pan_bits);
                        let strip_height = f32::from_bits(dependency.strip_height_bits);
                        let soloed = dependency.soloed;
                        let muted = dependency.muted;
                        let gain_mode = if dependency.compact_gain {
                            GainControlMode::Knob
                        } else {
                            GainControlMode::Fader
                        };
                        let base_strip = strip_panel(
                            strip_shell(StripShellArgs {
                                title: bus_title_content(
                                    bus_id,
                                    &name,
                                    dependency.renaming,
                                    &dependency.rename_value,
                                ),
                                instrument_picker: None,
                                route_picker: Some(route_picker(
                                    RoutingStrip::Bus(bus_id),
                                    dependency.route.clone(),
                                    dependency.route_choices.clone(),
                                    controls_enabled,
                                )),
                                gain_db,
                                pan,
                                meter_stack: meter_stack(
                                    meter_dependency,
                                    Some(if controls_enabled {
                                        Message::Mixer(MixerMessage::ResetBusMeter(bus_id))
                                    } else {
                                        noop_message()
                                    }),
                                ),
                                actions: StripActions {
                                    panel: Some((
                                        dependency.effect_rack_open,
                                        dependency.panel_has_content,
                                        if controls_enabled {
                                            Message::Mixer(MixerMessage::ToggleMixerEffectRack(
                                                strip_index,
                                            ))
                                        } else {
                                            noop_message()
                                        },
                                    )),
                                    solo: Some((
                                        soloed,
                                        if controls_enabled {
                                            Message::Mixer(MixerMessage::ToggleBusSolo(bus_id))
                                        } else {
                                            noop_message()
                                        },
                                    )),
                                    mute: Some((
                                        muted,
                                        if controls_enabled {
                                            Message::Mixer(MixerMessage::ToggleBusMute(bus_id))
                                        } else {
                                            noop_message()
                                        },
                                    )),
                                    on_gain: Some(Box::new(move |value| {
                                        if controls_enabled {
                                            Message::Mixer(MixerMessage::SetBusGain(bus_id, value))
                                        } else {
                                            noop_message()
                                        }
                                    })),
                                    on_pan: Some(Box::new(move |value| {
                                        if controls_enabled {
                                            Message::Mixer(MixerMessage::SetBusPan(bus_id, value))
                                        } else {
                                            noop_message()
                                        }
                                    })),
                                },
                                strip_height,
                                gain_mode,
                                show_gain_scale: true,
                            }),
                            STRIP_WIDTH,
                            strip_height,
                            false,
                            None,
                        );
                        let remove_button: Element<'static, Message> = container(
                            ui_style::flat_icon_button(
                                icons::x(),
                                ui_style::grid_f32(4),
                                ui_style::grid_f32(3),
                                ui_style::button_flat_compact_control,
                                ui_style::svg_dimmed_control,
                            )
                            .on_press(Message::Mixer(MixerMessage::RemoveBus(bus_id))),
                        )
                        .width(Fill)
                        .height(Fill)
                        .align_x(alignment::Horizontal::Right)
                        .align_y(alignment::Vertical::Top)
                        .padding([ui_style::grid(2), ui_style::grid(2)])
                        .into();
                        let layered: Element<'static, Message> =
                            stack([base_strip, remove_button]).into();
                        layered
                    },
                ));
                if effect_rack_open {
                    row.push(track_effect_rack_panel(
                        strip_index,
                        effects.clone(),
                        Some(EffectRackPanelRouting {
                            source: RoutingStrip::Bus(bus_id.0),
                            sends: send_dependencies(&bus.routing),
                            send_choices: send_destination_choices(
                                mixer,
                                RoutingStrip::Bus(bus_id.0),
                            ),
                        }),
                        hovered_processor_slot,
                        effect_rack_drag_state(effect_drag_source, effect_drag_target, strip_index),
                        controls_enabled,
                        strip_height,
                    ))
                } else {
                    row
                }
            },
        );
    let bus_row = if visible.end >= total_buses {
        bus_row.push(add_bus_lane(strip_height, controls_enabled))
    } else {
        bus_row
    };
    let bus_row = bus_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(row![section_title("Buses")]))
            .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(bus_row)
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .on_scroll(|viewport| Message::Mixer(MixerMessage::BusViewportScrolled(viewport)))
                .style(ui_style::workspace_scrollable)
                .width(Fill)
                .height(Fill),
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
        ]
        .height(Fill),
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(1.0))
            .style(ui_style::chrome_separator)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

pub(super) fn add_bus_button(controls_enabled: bool) -> Element<'static, Message> {
    ui_style::flat_icon_button(
        icons::plus(),
        ui_style::grid_f32(7),
        ui_style::grid_f32(4),
        ui_style::button_flat_compact_control,
        ui_style::svg_muted_control,
    )
    .on_press_maybe(Some(if controls_enabled {
        Message::Mixer(MixerMessage::AddBus)
    } else {
        noop_message()
    }))
    .into()
}

pub(super) fn add_bus_lane(strip_height: f32, controls_enabled: bool) -> Element<'static, Message> {
    container(add_bus_button(controls_enabled))
        .width(Length::Fixed(STRIP_WIDTH))
        .height(Length::Fixed(strip_height))
        .center_x(Length::Fixed(STRIP_WIDTH))
        .center_y(Length::Fixed(strip_height))
        .into()
}

pub(super) fn section_title<'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..fonts::UI
            }),
    )
    .height(Length::Fixed(SECTION_HEADER_HEIGHT))
    .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    .into()
}

pub(super) fn route_picker(
    source: RoutingStrip,
    selected: RouteChoice,
    choices: Vec<RouteChoice>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    if choices.len() <= 1 {
        return route_picker_placeholder(selected.label);
    }
    let menu_height = route_menu_height_for_items(choices.len());

    let label = route_label(&selected.label);
    let pick_list = pick_list(choices, Some(selected), move |choice| {
        if controls_enabled {
            Message::Mixer(MixerMessage::SetMainRoute(source, choice.route))
        } else {
            noop_message()
        }
    })
    .placeholder("Master")
    .width(Fill)
    .menu_height(Length::Fixed(menu_height))
    .padding([ui_style::grid(1), ui_style::grid(2)])
    .text_size(ui_style::FONT_SIZE_UI_XS)
    .font(fonts::UI)
    .style(route_pick_list_centered_style)
    .menu_style(route_pick_list_menu_style);

    stack![
        pick_list,
        route_picker_centered_label(label, button::Status::Active)
    ]
    .into()
}

pub(super) fn route_picker_placeholder(label: String) -> Element<'static, Message> {
    container(route_picker_centered_label(
        route_label(&label),
        button::Status::Disabled,
    ))
    .width(Fill)
    .height(Length::Fixed(ROUTE_PICKER_HEIGHT))
    .style(route_picker_placeholder_surface)
    .into()
}

pub(super) fn route_picker_centered_label(
    label: String,
    status: button::Status,
) -> Element<'static, Message> {
    container(
        text(label)
            .size(ui_style::FONT_SIZE_UI_XS)
            .align_x(alignment::Horizontal::Center)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(Fill)
    .height(Length::Fixed(ROUTE_PICKER_HEIGHT))
    .padding([0, ui_style::grid(4)])
    .center_x(Fill)
    .center_y(Length::Fixed(ROUTE_PICKER_HEIGHT))
    .style(move |theme| {
        let button = ui_style::button_selector_field(theme, status, false);
        container::Style {
            text_color: Some(button.text_color),
            ..container::Style::default()
        }
    })
    .into()
}

pub(super) fn route_label(label: &str) -> String {
    crate::track_names::ellipsize_middle(label, ROUTE_PICKER_MAX_LEN)
}

pub(super) fn route_pick_list_style(
    theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let open = matches!(status, iced::widget::pick_list::Status::Opened { .. });
    let button_status = match status {
        iced::widget::pick_list::Status::Active => button::Status::Active,
        iced::widget::pick_list::Status::Hovered => button::Status::Hovered,
        iced::widget::pick_list::Status::Opened { is_hovered } => {
            if is_hovered {
                button::Status::Hovered
            } else {
                button::Status::Active
            }
        }
    };
    let button = ui_style::button_selector_field(theme, button_status, open);
    let palette = theme.extended_palette();
    iced::widget::pick_list::Style {
        text_color: button.text_color,
        placeholder_color: button.text_color,
        handle_color: palette.background.weak.text,
        background: button
            .background
            .unwrap_or(palette.background.weak.color.into()),
        border: button.border,
    }
}

pub(super) fn route_pick_list_centered_style(
    theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let mut style = route_pick_list_style(theme, status);
    style.text_color = Color::TRANSPARENT;
    style.placeholder_color = Color::TRANSPARENT;
    style
}

pub(super) fn route_pick_list_menu_style(
    theme: &iced::Theme,
) -> iced::widget::overlay::menu::Style {
    let palette = theme.extended_palette();
    iced::widget::overlay::menu::Style {
        background: palette.background.weak.color.into(),
        border: border::rounded(ui_style::RADIUS_UI)
            .width(1)
            .color(palette.background.strong.color),
        text_color: palette.background.weak.text,
        selected_text_color: palette.background.strong.text,
        selected_background: palette.background.strong.color.into(),
        shadow: iced::Shadow::default(),
    }
}

pub(super) fn route_picker_placeholder_surface(theme: &iced::Theme) -> container::Style {
    let button = ui_style::button_selector_field(theme, button::Status::Disabled, false);
    container::Style {
        text_color: Some(button.text_color),
        background: button.background,
        border: button.border,
        shadow: button.shadow,
        ..container::Style::default()
    }
}
