use iced::widget::column;

use super::*;

pub(in crate::app) fn content(app: &Lilypalooza) -> Element<'_, Message> {
    let colors = meter_colors(&app.theme);
    let existing_track_count = app
        .piano_roll
        .current_file()
        .map(|file| file.data.tracks.len())
        .unwrap_or(0);
    let track_colors: Vec<_> = (0..existing_track_count)
        .map(|track_index| app.effective_track_color(track_index))
        .collect();
    let renaming_target = app.renaming_target;
    let renaming_origin = app.renaming_origin;
    let track_rename_value = app.track_rename_value.clone();
    let track_rename_color_value = app.track_rename_color_value;
    let track_rename_color_picker_open = app.track_rename_color_picker_open;

    if let Some(playback) = app.playback.as_ref() {
        let mixer = playback.mixer_state();

        return responsive(move |size| {
            let gain_mode = gain_control_mode(size.height);
            let strip_height =
                (size.height - (ui_style::PADDING_SM as f32 * 2.0) - SECTION_HEADER_HEIGHT)
                    .max(STRIP_MIN_HEIGHT);
            let instrument_visible = visible_strip_window(
                mixer.tracks().len(),
                app.mixer_instrument_scroll_x,
                app.mixer_instrument_viewport_width.max(size.width * 0.5),
            );
            let bus_visible = visible_strip_window(
                mixer.buses().len(),
                app.mixer_bus_scroll_x,
                app.mixer_bus_viewport_width.max(size.width * 0.2),
            );
            let meter_window =
                playback.meter_snapshot_window(instrument_visible.clone(), bus_visible.clone());
            let master_width = if app.open_mixer_effect_rack_tracks.contains(&0) {
                MAIN_SECTION_WIDTH + EFFECT_RACK_PANEL_WIDTH
            } else {
                MAIN_SECTION_WIDTH
            };
            let mixer_row = row![
                container(master_track_area(MasterTrackAreaArgs {
                    mixer,
                    meter_snapshot: meter_window.main,
                    colors,
                    strip_height,
                    gain_mode,
                    open_effect_rack_strips: &app.open_mixer_effect_rack_tracks,
                    hovered_processor_slot: app.hovered_processor_slot,
                    effect_drag: effect_rack_drag_state(
                        app.effect_drag_source,
                        app.effect_drag_target,
                        0,
                    ),
                    controls_enabled: true,
                }))
                .width(Length::Fixed(master_width))
                .height(Fill)
                .style(ui_style::mixer_side_group_surface),
                container(instrument_track_area(InstrumentTrackAreaArgs {
                    mixer,
                    meters: &meter_window,
                    colors,
                    strip_height,
                    gain_mode,
                    visible: instrument_visible,
                    existing_track_count,
                    track_colors: &track_colors,
                    renaming_target,
                    renaming_origin,
                    track_rename_value: &track_rename_value,
                    track_rename_color_value,
                    track_rename_color_picker_open,
                    selected_track_index: app.selected_track_index,
                    hovered_processor_slot: app.hovered_processor_slot,
                    effect_drag_source: app.effect_drag_source,
                    effect_drag_target: app.effect_drag_target,
                    open_effect_rack_strips: &app.open_mixer_effect_rack_tracks,
                    controls_enabled: true,
                }))
                .width(FillPortion(5))
                .height(Fill)
                .style(ui_style::mixer_instrument_group_surface),
                container(bus_track_area(BusTrackAreaArgs {
                    mixer,
                    meters: &meter_window,
                    colors,
                    strip_height,
                    gain_mode,
                    visible: bus_visible,
                    open_effect_rack_strips: &app.open_mixer_effect_rack_tracks,
                    hovered_processor_slot: app.hovered_processor_slot,
                    effect_drag_source: app.effect_drag_source,
                    effect_drag_target: app.effect_drag_target,
                    renaming_target,
                    renaming_origin,
                    track_rename_value: &track_rename_value,
                    controls_enabled: true,
                }))
                .width(FillPortion(2))
                .height(Fill)
                .style(ui_style::mixer_side_group_surface)
            ];
            mixer_row
                .spacing(ui_style::SPACE_SM)
                .padding(ui_style::PADDING_SM)
                .width(Fill)
                .height(Fill)
                .into()
        })
        .into();
    }

    content_without_audio()
}

pub(in crate::app) fn content_without_audio() -> Element<'static, Message> {
    container(
        text("Audio engine disabled")
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill)
    .style(ui_style::mixer_instrument_group_surface)
    .into()
}

pub(super) struct MasterTrackAreaArgs<'a> {
    pub(super) mixer: &'a MixerState,
    pub(super) meter_snapshot: StripMeterSnapshot,
    pub(super) colors: MeterColors,
    pub(super) strip_height: f32,
    pub(super) gain_mode: GainControlMode,
    pub(super) open_effect_rack_strips: &'a [usize],
    pub(super) hovered_processor_slot: Option<(
        crate::app::processor_editor_windows::EditorTarget,
        ProcessorSlotSegment,
    )>,
    pub(super) effect_drag: Option<EffectRackDragState>,
    pub(super) controls_enabled: bool,
}

pub(super) fn master_track_area(args: MasterTrackAreaArgs<'_>) -> Element<'static, Message> {
    let MasterTrackAreaArgs {
        mixer,
        meter_snapshot,
        colors,
        strip_height,
        gain_mode,
        open_effect_rack_strips,
        hovered_processor_slot,
        effect_drag,
        controls_enabled,
    } = args;
    let mut master_row = row![sticky_master_strip(
        mixer,
        meter_snapshot,
        colors,
        strip_height,
        gain_mode,
        open_effect_rack_strips.contains(&0),
        controls_enabled,
    )]
    .align_y(alignment::Vertical::Top)
    .height(Length::Fixed(strip_height));
    if open_effect_rack_strips.contains(&0) {
        master_row = master_row.push(track_effect_rack_panel(
            0,
            effect_slot_dependencies(mixer.master()),
            None,
            hovered_processor_slot,
            effect_drag,
            controls_enabled,
            strip_height,
        ));
    }

    column![
        container(section_header_bar(row![section_title("Main")]))
            .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            container(master_row).width(Fill).height(Fill),
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

pub(super) fn sticky_master_strip(
    mixer: &MixerState,
    meter_snapshot: StripMeterSnapshot,
    colors: MeterColors,
    strip_height: f32,
    gain_mode: GainControlMode,
    effect_rack_open: bool,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let master = mixer.master();
    lazy(
        MainStripDependency {
            gain_bits: master.state.gain_db.to_bits(),
            pan_bits: master.state.pan.to_bits(),
            meter: MeterDependency::from_snapshot(meter_snapshot),
            compact_gain: matches!(gain_mode, GainControlMode::Knob),
            effect_rack_open,
            panel_has_content: !master.effects().is_empty(),
            strip_height_bits: strip_height.to_bits(),
        },
        move |dependency| {
            let gain_mode = if dependency.compact_gain {
                GainControlMode::Knob
            } else {
                GainControlMode::Fader
            };
            strip_panel(
                strip_shell(StripShellArgs {
                    title: container(section_title("Main"))
                        .width(Fill)
                        .center_x(Fill)
                        .into(),
                    instrument_picker: None,
                    route_picker: None,
                    gain_db: f32::from_bits(dependency.gain_bits),
                    pan: f32::from_bits(dependency.pan_bits),
                    meter_stack: meter_stack(
                        MeterStackDependency {
                            meter: dependency.meter,
                            colors: MeterColorsDependency::from_colors(colors),
                            compact_gain: dependency.compact_gain,
                            strip_height_bits: dependency.strip_height_bits,
                        },
                        Some(if controls_enabled {
                            Message::Mixer(MixerMessage::ResetMasterMeter)
                        } else {
                            noop_message()
                        }),
                    ),
                    actions: StripActions {
                        panel: Some((
                            dependency.effect_rack_open,
                            dependency.panel_has_content,
                            if controls_enabled {
                                Message::Mixer(MixerMessage::ToggleMixerEffectRack(0))
                            } else {
                                noop_message()
                            },
                        )),
                        solo: None,
                        mute: None,
                        on_gain: Some(Box::new(move |value| {
                            if controls_enabled {
                                Message::Mixer(MixerMessage::SetMasterGain(value))
                            } else {
                                noop_message()
                            }
                        })),
                        on_pan: Some(Box::new(move |value| {
                            if controls_enabled {
                                Message::Mixer(MixerMessage::SetMasterPan(value))
                            } else {
                                noop_message()
                            }
                        })),
                    },
                    strip_height: f32::from_bits(dependency.strip_height_bits),
                    gain_mode,
                    show_gain_scale: true,
                }),
                MAIN_STRIP_WIDTH,
                f32::from_bits(dependency.strip_height_bits),
                false,
                None,
            )
        },
    )
    .into()
}

pub(super) struct InstrumentTrackAreaArgs<'a> {
    pub(super) mixer: &'a MixerState,
    pub(super) meters: &'a MixerMeterSnapshotWindow,
    pub(super) colors: MeterColors,
    pub(super) strip_height: f32,
    pub(super) gain_mode: GainControlMode,
    pub(super) visible: std::ops::Range<usize>,
    pub(super) existing_track_count: usize,
    pub(super) track_colors: &'a [Color],
    pub(super) renaming_target: Option<crate::app::RenameTarget>,
    pub(super) renaming_origin: Option<crate::app::WorkspacePaneKind>,
    pub(super) track_rename_value: &'a str,
    pub(super) track_rename_color_value: Color,
    pub(super) track_rename_color_picker_open: bool,
    pub(super) selected_track_index: Option<usize>,
    pub(super) hovered_processor_slot: Option<(
        crate::app::processor_editor_windows::EditorTarget,
        ProcessorSlotSegment,
    )>,
    pub(super) effect_drag_source: Option<(usize, usize)>,
    pub(super) effect_drag_target: Option<(usize, usize)>,
    pub(super) open_effect_rack_strips: &'a [usize],
    pub(super) controls_enabled: bool,
}

pub(super) fn instrument_track_area(
    args: InstrumentTrackAreaArgs<'_>,
) -> Element<'static, Message> {
    let InstrumentTrackAreaArgs {
        mixer,
        meters,
        colors,
        strip_height,
        gain_mode,
        visible,
        existing_track_count,
        track_colors,
        renaming_target,
        renaming_origin,
        track_rename_value,
        track_rename_color_value,
        track_rename_color_picker_open,
        selected_track_index,
        hovered_processor_slot,
        effect_drag_source,
        effect_drag_target,
        open_effect_rack_strips,
        controls_enabled,
    } = args;
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = strip_span_width(mixer.tracks().len().saturating_sub(visible.end));
    let track_row = mixer
        .tracks()
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
            move |row, (local_index, track)| {
                let track_index = visible.start + local_index;
                let strip_index = track_index + 1;
                let effect_rack_open = open_effect_rack_strips.contains(&strip_index);
                let selected_choice = selected_instrument_choice(track.instrument_slot(), mixer);
                let effects = effect_slot_dependencies(track);
                let route_choices = route_choices(mixer, RoutingStrip::Track(track_index));
                let selected_route = selected_route_choice(track.routing.main, &route_choices);
                let strip_hovered_processor_slot =
                    hovered_processor_slot.filter(|(target, _)| target.strip_index == strip_index);
                let hovered_processor_slot = strip_hovered_processor_slot
                    .filter(|(target, _)| target.slot_index == 0)
                    .map(|(target, segment)| (target.slot_index, segment));
                let track_color = track_colors
                    .get(track_index)
                    .copied()
                    .unwrap_or_else(|| crate::track_colors::default_track_color(track_index));
                let meter_dependency = MeterStackDependency {
                    meter: MeterDependency::from_snapshot(
                        meters.tracks.get(local_index).copied().unwrap_or_default(),
                    ),
                    colors: MeterColorsDependency::from_colors(colors),
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                };
                let row = row.push(lazy(
                    TrackStripDependency {
                        index: track_index,
                        name: track.name.clone(),
                        selected: selected_choice.clone(),
                        editor_enabled: track
                            .instrument_slot()
                            .filter(|slot| !slot.is_empty())
                            .and_then(|slot| slot.descriptor())
                            .and_then(|descriptor| descriptor.editor)
                            .is_some(),
                        effects: effects.clone(),
                        hovered_processor_slot,
                        color_bits: color_bits(track_color),
                        gain_bits: track.state.gain_db.to_bits(),
                        pan_bits: track.state.pan.to_bits(),
                        route: selected_route,
                        route_choices,
                        meter: meter_dependency.meter,
                        compact_gain: matches!(gain_mode, GainControlMode::Knob),
                        effect_rack_open,
                        panel_has_content: !effects.is_empty()
                            || !track.routing.sends.is_empty()
                            || track.routing.main != TrackRoute::Master,
                        strip_height_bits: strip_height.to_bits(),
                        soloed: track.state.soloed,
                        muted: track.state.muted,
                        tint_enabled: track_should_use_roll_tint(track_index, existing_track_count),
                        highlighted: selected_track_index == Some(track_index),
                        renaming: renaming_target
                            == Some(crate::app::RenameTarget::Track(track_index))
                            && renaming_origin == Some(crate::app::WorkspacePaneKind::Mixer),
                        rename_value: track_rename_value.to_string(),
                        color_picker_open: renaming_target
                            == Some(crate::app::RenameTarget::Track(track_index))
                            && renaming_origin == Some(crate::app::WorkspacePaneKind::Mixer)
                            && track_rename_color_picker_open,
                    },
                    move |dependency| {
                        let name = dependency.name.clone();
                        let is_selected = dependency.highlighted;
                        let track_color = if dependency.renaming {
                            track_rename_color_value
                        } else {
                            color_from_bits(dependency.color_bits)
                        };
                        let strip_height = f32::from_bits(dependency.strip_height_bits);
                        let gain_mode = if dependency.compact_gain {
                            GainControlMode::Knob
                        } else {
                            GainControlMode::Fader
                        };
                        let shell = strip_shell(StripShellArgs {
                            title: track_title_content(
                                track_index,
                                &name,
                                dependency.renaming,
                                &dependency.rename_value,
                                track_color,
                                dependency.color_picker_open,
                            ),
                            instrument_picker: Some({
                                let target = crate::app::processor_editor_windows::EditorTarget {
                                    strip_index,
                                    slot_index: 0,
                                };
                                strip_processor_header(
                                    strip_index,
                                    Some((
                                        target,
                                        dependency.selected.as_ref(),
                                        dependency.editor_enabled,
                                        dependency
                                            .hovered_processor_slot
                                            .map(|(_, segment)| segment),
                                    )),
                                    controls_enabled,
                                )
                            }),
                            route_picker: Some(route_picker(
                                RoutingStrip::Track(track_index),
                                dependency.route.clone(),
                                dependency.route_choices.clone(),
                                controls_enabled,
                            )),
                            gain_db: f32::from_bits(dependency.gain_bits),
                            pan: f32::from_bits(dependency.pan_bits),
                            meter_stack: meter_stack(
                                meter_dependency,
                                Some(if controls_enabled {
                                    Message::Mixer(MixerMessage::ResetTrackMeter(track_index))
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
                                    dependency.soloed,
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::ToggleTrackSolo(track_index))
                                    } else {
                                        noop_message()
                                    },
                                )),
                                mute: Some((
                                    dependency.muted,
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::ToggleTrackMute(track_index))
                                    } else {
                                        noop_message()
                                    },
                                )),
                                on_gain: Some(Box::new(move |value| {
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::SetTrackGain(
                                            track_index,
                                            value,
                                        ))
                                    } else {
                                        noop_message()
                                    }
                                })),
                                on_pan: Some(Box::new(move |value| {
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::SetTrackPan(
                                            track_index,
                                            value,
                                        ))
                                    } else {
                                        noop_message()
                                    }
                                })),
                            },
                            strip_height,
                            gain_mode,
                            show_gain_scale: true,
                        });

                        if dependency.tint_enabled {
                            tinted_track_strip_panel(
                                shell,
                                STRIP_WIDTH,
                                strip_height,
                                track_color,
                                is_selected,
                                Some(Message::Mixer(MixerMessage::SelectTrack(track_index))),
                            )
                        } else {
                            strip_panel(
                                shell,
                                STRIP_WIDTH,
                                strip_height,
                                is_selected,
                                Some(Message::Mixer(MixerMessage::SelectTrack(track_index))),
                            )
                        }
                    },
                ));
                if effect_rack_open {
                    row.push(track_effect_rack_panel(
                        strip_index,
                        effects.clone(),
                        Some(EffectRackPanelRouting {
                            source: RoutingStrip::Track(track_index),
                            sends: send_dependencies(&track.routing),
                            send_choices: send_destination_choices(
                                mixer,
                                RoutingStrip::Track(track_index),
                            ),
                        }),
                        strip_hovered_processor_slot,
                        effect_rack_drag_state(effect_drag_source, effect_drag_target, strip_index),
                        controls_enabled,
                        strip_height,
                    ))
                } else {
                    row
                }
            },
        );
    let track_row = track_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(
            row![
                section_title("Instrument Tracks"),
                container(text("")).width(Fill),
            ]
            .align_y(alignment::Vertical::Center),
        ))
        .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(track_row)
                .id(instrument_scroll_id())
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .on_scroll(
                    |viewport| Message::Mixer(MixerMessage::InstrumentViewportScrolled(viewport))
                )
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
