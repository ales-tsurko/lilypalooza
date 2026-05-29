use std::path::{Path, PathBuf};

use iced::{Color, Element, Length, Theme, widget::container};
use iced_test::{Simulator, simulator};
use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BUILTIN_GAIN_ID, MixerState, mixer::MixerMeterSnapshotWindow,
};

use super::{
    COMPACT_GAIN_SWITCH_OFFSET, GainControlMode, INSTRUMENT_PICKER_HEIGHT, INSTRUMENT_SLOT_WIDTH,
    InstrumentChoice, MAIN_STRIP_WIDTH, MIXER_MIN_HEIGHT, ProcessorBrowserBackend,
    ROUTE_PICKER_BOTTOM_INSET, ROUTE_PICKER_TOP_SPACING, SECTION_BODY_GAP, SEND_CONTROL_HEIGHT,
    SEND_MODE_HEIGHT, SEND_PANEL_TOP_SPACING, SEND_ROW_CONTENT_BOTTOM_SPACING, SEND_ROW_HEIGHT,
    STRIP_MIN_HEIGHT, STRIP_WIDTH, StripMeterSnapshot, TITLE_TOP_SPACING,
    TRACK_TITLE_EDITOR_CONTROL_HEIGHT, TRACK_TITLE_EDITOR_HEIGHT,
    TRACK_TITLE_EDITOR_INPUT_PADDING_H, TRACK_TITLE_EDITOR_INPUT_PADDING_V,
    TRACK_TITLE_EDITOR_SWATCH_SIZE, VALUE_LABEL_HEIGHT, meter_colors,
};
use crate::{
    app::{Message, messages::MixerMessage},
    ui_style,
};

pub(super) fn assert_snapshot_matches(
    ui: &mut iced_test::Simulator<'_, crate::app::Message>,
    baseline_name: &str,
) {
    let _snapshot_guard = super::super::ICED_SNAPSHOT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = ui.snapshot(&Theme::Dark).expect("snapshot should render");
    let baseline_path = Path::new(baseline_name);

    assert!(
        snapshot
            .matches_hash(baseline_name)
            .expect("snapshot hash should be readable"),
        "snapshot hash mismatch for: {baseline_name}"
    );
    assert!(
        snapshot
            .matches_image(baseline_path)
            .expect("snapshot image should be readable"),
        "snapshot image mismatch for: {baseline_name}"
    );
}

pub(super) fn assert_snapshots_equal(
    first: &mut iced_test::Simulator<'_, crate::app::Message>,
    second: &mut iced_test::Simulator<'_, crate::app::Message>,
    baseline_name: &str,
) {
    let (_snapshot_guard, baseline_path) = write_temp_snapshot_baseline(first, baseline_name);
    let second_snapshot = second
        .snapshot(&Theme::Dark)
        .expect("second snapshot should render");
    assert!(
        second_snapshot
            .matches_hash(&baseline_path)
            .expect("second snapshot hash should be readable"),
        "snapshot hash mismatch for: {baseline_name}"
    );
    assert!(
        second_snapshot
            .matches_image(&baseline_path)
            .expect("second snapshot image should be readable"),
        "snapshot image mismatch for: {baseline_name}"
    );
}

pub(super) fn color_distance(first: Color, second: Color) -> f32 {
    (first.r - second.r).abs() + (first.g - second.g).abs() + (first.b - second.b).abs()
}

pub(super) fn test_effect_slot(
    slot_index: usize,
    instance_id: u64,
    instance_label_index: u32,
    name: &str,
) -> super::EffectSlotDependency {
    super::EffectSlotDependency {
        slot_index,
        instance_id,
        instance_label_index,
        selected: Some(super::ProcessorChoice::Processor {
            processor_id: name.to_lowercase(),
            name: name.to_string(),
            backend: super::ProcessorBrowserBackend::BuiltIn,
        }),
        editor_enabled: false,
        bypassed: false,
    }
}

pub(super) fn assert_snapshots_differ(
    first: &mut iced_test::Simulator<'_, crate::app::Message>,
    second: &mut iced_test::Simulator<'_, crate::app::Message>,
    baseline_name: &str,
) {
    let (_snapshot_guard, baseline_path) = write_temp_snapshot_baseline(first, baseline_name);
    assert!(
        !second
            .snapshot(&Theme::Dark)
            .expect("second snapshot should render")
            .matches_hash(&baseline_path)
            .expect("second snapshot hash should be readable"),
        "snapshot hash unexpectedly matched for: {baseline_name}"
    );
}

fn write_temp_snapshot_baseline(
    ui: &mut iced_test::Simulator<'_, crate::app::Message>,
    baseline_name: &str,
) -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
    let snapshot_guard = super::super::ICED_SNAPSHOT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut baseline_path = PathBuf::from("/tmp");
    baseline_path.push(baseline_name);
    let png = baseline_path.with_file_name(format!("{baseline_name}-wgpu.png"));
    let sha = baseline_path.with_file_name(format!("{baseline_name}-wgpu.sha256"));
    remove_snapshot_file_if_exists(&png);
    remove_snapshot_file_if_exists(&sha);

    let snapshot = ui
        .snapshot(&Theme::Dark)
        .expect("first snapshot should render");
    assert!(
        snapshot
            .matches_hash(&baseline_path)
            .expect("first snapshot hash should be readable")
    );
    assert!(
        snapshot
            .matches_image(&baseline_path)
            .expect("first snapshot image should be readable")
    );
    (snapshot_guard, baseline_path)
}

fn remove_snapshot_file_if_exists(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to remove {}: {error}", path.display()),
    }
}

pub(super) fn is_grid_multiple(value: f32) -> bool {
    ((value / 4.0).round() - (value / 4.0)).abs() < 1.0e-4
}

#[test]
fn fixed_strip_sizes_follow_four_px_grid() {
    for value in [
        MAIN_STRIP_WIDTH,
        STRIP_WIDTH,
        STRIP_MIN_HEIGHT,
        MIXER_MIN_HEIGHT,
        INSTRUMENT_PICKER_HEIGHT,
        TRACK_TITLE_EDITOR_HEIGHT,
        VALUE_LABEL_HEIGHT,
        ROUTE_PICKER_TOP_SPACING,
        ROUTE_PICKER_BOTTOM_INSET,
        SEND_ROW_HEIGHT,
        SEND_ROW_CONTENT_BOTTOM_SPACING,
        SEND_PANEL_TOP_SPACING,
        SEND_CONTROL_HEIGHT,
        SEND_MODE_HEIGHT,
        COMPACT_GAIN_SWITCH_OFFSET,
    ] {
        assert!(is_grid_multiple(value), "{value} should use the 4px grid");
    }
}

#[test]
fn mixer_track_title_editor_height_increases_by_one_grid_unit() {
    crate::test_assertions::assert_float_eq!(TRACK_TITLE_EDITOR_HEIGHT, ui_style::grid_f32(5));
}

#[test]
fn mixer_track_title_editor_input_padding_matches_taller_height() {
    assert_eq!(TRACK_TITLE_EDITOR_INPUT_PADDING_V, 2);
    assert_eq!(TRACK_TITLE_EDITOR_INPUT_PADDING_H, ui_style::grid(1));
}

#[test]
fn mixer_track_title_editor_controls_scale_with_taller_shell() {
    crate::test_assertions::assert_float_eq!(
        TRACK_TITLE_EDITOR_CONTROL_HEIGHT,
        TRACK_TITLE_EDITOR_HEIGHT
    );
}

#[test]
fn mixer_track_title_editor_swatch_is_square() {
    crate::test_assertions::assert_float_eq!(
        TRACK_TITLE_EDITOR_SWATCH_SIZE,
        TRACK_TITLE_EDITOR_HEIGHT
    );
}

#[test]
fn mixer_strip_title_gap_uses_three_grid_units() {
    crate::test_assertions::assert_float_eq!(TITLE_TOP_SPACING, ui_style::grid_f32(3));
}

#[test]
fn mixer_section_header_has_no_body_gap() {
    crate::test_assertions::assert_float_eq!(SECTION_BODY_GAP, 0.0);
}

#[test]
fn mixer_track_title_editor_widget_matches_snapshot() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            STRIP_WIDTH + ui_style::grid_f32(4),
            TRACK_TITLE_EDITOR_HEIGHT + ui_style::grid_f32(4),
        ],
        container(editable_test_track_title())
            .width(Length::Fixed(STRIP_WIDTH))
            .padding(ui_style::PADDING_SM),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_track_title_editor_widget_five_grid",
    );
}

fn editable_test_track_title() -> Element<'static, Message> {
    super::track_title_content(
        0,
        "Track 1",
        true,
        "Track 1",
        Color::from_rgb(0.42, 0.58, 0.86),
        false,
    )
}

#[test]
fn mixer_strip_title_editor_matches_snapshot() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            STRIP_WIDTH + ui_style::grid_f32(8),
            STRIP_MIN_HEIGHT + ui_style::grid_f32(8),
        ],
        container(super::strip_shell(super::StripShellArgs {
            title: editable_test_track_title(),
            instrument_picker: None,
            route_picker: None,
            gain_db: 0.0,
            pan: 0.0,
            meter_stack: container(iced::widget::text("")).into(),
            actions: super::StripActions {
                panel: None,
                solo: None,
                mute: None,
                on_gain: None,
                on_pan: None,
            },
            strip_height: STRIP_MIN_HEIGHT,
            gain_mode: GainControlMode::Knob,
            show_gain_scale: false,
        }))
        .padding(ui_style::PADDING_SM),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_strip_title_editor_integration_five_grid",
    );
}

#[test]
fn mixer_strip_controls_leave_gap_above_title_matches_snapshot() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [STRIP_WIDTH + ui_style::grid_f32(8), 520.0],
        container(super::strip_shell(super::StripShellArgs {
            title: super::track_title_content(
                0,
                "Violin",
                true,
                "Violin",
                Color::from_rgb(0.92, 0.34, 0.34),
                false,
            ),
            instrument_picker: Some(super::processor_slot_controls(
                crate::app::processor_editor_windows::EditorTarget {
                    strip_index: 1,
                    slot_index: 0,
                },
                super::ProcessorSlotRole::Instrument,
                Some(&InstrumentChoice::None),
                false,
                false,
                None,
                true,
            )),
            route_picker: None,
            gain_db: 0.0,
            pan: 0.0,
            meter_stack: container(iced::widget::text(""))
                .width(Length::Fixed(72.0))
                .height(Length::Fixed(220.0))
                .into(),
            actions: super::StripActions {
                panel: Some((false, false, super::noop_message())),
                solo: Some((false, super::noop_message())),
                mute: Some((false, super::noop_message())),
                on_gain: Some(Box::new(|_| super::noop_message())),
                on_pan: Some(Box::new(|_| super::noop_message())),
            },
            strip_height: 480.0,
            gain_mode: GainControlMode::Knob,
            show_gain_scale: false,
        }))
        .padding(ui_style::PADDING_SM),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_strip_controls_title_gap_fixed",
    );
}

#[test]
fn mixer_full_track_strip_rename_matches_snapshot() {
    let mixer = MixerState::new();
    let meters = MixerMeterSnapshotWindow::default();
    let colors = meter_colors(&Theme::Dark);

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [STRIP_WIDTH + ui_style::grid_f32(8), 520.0],
        super::instrument_track_area(super::InstrumentTrackAreaArgs {
            mixer: &mixer,
            meters: &meters,
            colors,
            strip_height: STRIP_MIN_HEIGHT,
            gain_mode: GainControlMode::Knob,
            visible: 0..1,
            existing_track_count: 1,
            track_colors: &[crate::track_colors::default_track_color(0)],
            renaming_target: Some(crate::app::RenameTarget::Track(0)),
            renaming_origin: Some(crate::app::WorkspacePaneKind::Mixer),
            track_rename_value: "Violin",
            track_rename_color_value: Color::from_rgb(0.92, 0.34, 0.34),
            track_rename_color_picker_open: false,
            selected_track_index: None,
            hovered_processor_slot: None,
            effect_drag_source: None,
            effect_drag_target: None,
            open_effect_rack_strips: &[],
            controls_enabled: true,
        }),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_full_track_strip_rename_no_section_gap",
    );
}

#[test]
fn mixer_master_strip_main_title_matches_snapshot() {
    let mixer = MixerState::new();
    let colors = meter_colors(&Theme::Dark);

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            MAIN_STRIP_WIDTH + ui_style::grid_f32(8),
            STRIP_MIN_HEIGHT + ui_style::grid_f32(8),
        ],
        super::sticky_master_strip(
            &mixer,
            StripMeterSnapshot::default(),
            colors,
            STRIP_MIN_HEIGHT,
            GainControlMode::Knob,
            false,
            true,
        ),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_master_strip_main_title_no_section_gap",
    );
}

#[test]
fn mixer_bus_area_empty_centers_add_bus_matches_snapshot() {
    let mixer = MixerState::new();
    let meters = MixerMeterSnapshotWindow::default();
    let colors = meter_colors(&Theme::Dark);

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [STRIP_WIDTH + ui_style::grid_f32(8), 320.0],
        super::bus_track_area(super::BusTrackAreaArgs {
            mixer: &mixer,
            meters: &meters,
            colors,
            strip_height: STRIP_MIN_HEIGHT,
            gain_mode: GainControlMode::Knob,
            visible: 0..0,
            open_effect_rack_strips: &[],
            hovered_processor_slot: None,
            effect_drag_source: None,
            effect_drag_target: None,
            renaming_target: None,
            renaming_origin: None,
            track_rename_value: "",
            controls_enabled: true,
        }),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_bus_area_empty_centered_add_icon_no_section_gap",
    );
}

#[test]
fn mixer_bus_area_nonempty_appends_add_bus_lane_matches_snapshot() {
    let (mut app, _task) = crate::app::new_with_default_test_state();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);

    let mixer = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .clone();
    let meters = MixerMeterSnapshotWindow::default();
    let colors = meter_colors(&Theme::Dark);

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [STRIP_WIDTH * 2.0 + ui_style::grid_f32(8), 320.0],
        super::bus_track_area(super::BusTrackAreaArgs {
            mixer: &mixer,
            meters: &meters,
            colors,
            strip_height: STRIP_MIN_HEIGHT,
            gain_mode: GainControlMode::Knob,
            visible: 0..mixer.bus_count(),
            open_effect_rack_strips: &[],
            hovered_processor_slot: None,
            effect_drag_source: None,
            effect_drag_target: None,
            renaming_target: None,
            renaming_origin: None,
            track_rename_value: "",
            controls_enabled: true,
        }),
    );
    assert_snapshot_matches(
        &mut ui,
        "tests/snapshots/mixer_bus_area_nonempty_add_icon_lane_flat_close",
    );
}

#[test]
fn add_bus_button_hover_is_consistent_across_whole_button() {
    let view = || -> Element<'static, Message> {
        container(super::add_bus_button(true))
            .width(Length::Fixed(ui_style::grid_f32(12)))
            .height(Length::Fixed(ui_style::grid_f32(12)))
            .center_x(Length::Fixed(ui_style::grid_f32(12)))
            .center_y(Length::Fixed(ui_style::grid_f32(12)))
            .into()
    };

    let mut button_hover = Simulator::with_size(
        iced::Settings::default(),
        [ui_style::grid_f32(12), ui_style::grid_f32(12)],
        view(),
    );
    button_hover.point_at(iced::Point::new(
        ui_style::grid_f32(4),
        ui_style::grid_f32(6),
    ));

    let mut icon_hover = Simulator::with_size(
        iced::Settings::default(),
        [ui_style::grid_f32(12), ui_style::grid_f32(12)],
        view(),
    );
    icon_hover.point_at(iced::Point::new(
        ui_style::grid_f32(6),
        ui_style::grid_f32(6),
    ));

    assert_snapshots_equal(
        &mut button_hover,
        &mut icon_hover,
        "add_bus_button_hover_consistency",
    );
}

#[test]
fn instrument_slot_icon_area_opens_editor() {
    let target = crate::app::processor_editor_windows::EditorTarget {
        strip_index: 3,
        slot_index: 0,
    };
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [INSTRUMENT_SLOT_WIDTH, INSTRUMENT_PICKER_HEIGHT],
        super::slot_selector_controls(
            "Violin".to_string(),
            Some(Message::Mixer(MixerMessage::OpenEditor(target))),
            Some(Message::Mixer(MixerMessage::ToggleProcessorBrowser(target))),
        ),
    );

    ui.point_at(iced::Point::new(20.0, INSTRUMENT_PICKER_HEIGHT / 2.0));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::OpenEditor(clicked)) if clicked == target
        )
    }));
}

#[test]
fn instrument_slot_name_area_opens_picker() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [INSTRUMENT_SLOT_WIDTH, INSTRUMENT_PICKER_HEIGHT],
        {
            let target = crate::app::processor_editor_windows::EditorTarget {
                strip_index: 3,
                slot_index: 0,
            };
            super::slot_selector_controls(
                "Violin".to_string(),
                Some(Message::Mixer(MixerMessage::OpenEditor(target))),
                Some(Message::Mixer(MixerMessage::ToggleProcessorBrowser(target))),
            )
        },
    );

    ui.point_at(iced::Point::new(
        INSTRUMENT_SLOT_WIDTH / 2.0,
        INSTRUMENT_PICKER_HEIGHT / 2.0,
    ));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleProcessorBrowser(
                crate::app::processor_editor_windows::EditorTarget {
                    strip_index: 3,
                    slot_index: 0,
                }
            ))
        )
    }));
}

#[test]
fn hovered_effect_slot_power_area_toggles_bypass() {
    let target = crate::app::processor_editor_windows::EditorTarget {
        strip_index: 3,
        slot_index: 1,
    };
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [INSTRUMENT_SLOT_WIDTH, INSTRUMENT_PICKER_HEIGHT],
        super::processor_slot_controls(
            target,
            super::ProcessorSlotRole::Effect,
            Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_GAIN_ID.to_string(),
                name: "Gain".to_string(),
                backend: ProcessorBrowserBackend::BuiltIn,
            }),
            true,
            false,
            Some(super::ProcessorSlotSegment::Bypass),
            true,
        ),
    );

    ui.point_at(iced::Point::new(16.0, INSTRUMENT_PICKER_HEIGHT / 2.0));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleSlotBypass(clicked)) if clicked == target
        )
    }));
}

#[test]
fn hovered_effect_slot_list_area_opens_processor_picker() {
    let target = crate::app::processor_editor_windows::EditorTarget {
        strip_index: 3,
        slot_index: 1,
    };
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [INSTRUMENT_SLOT_WIDTH, INSTRUMENT_PICKER_HEIGHT],
        super::processor_slot_controls(
            target,
            super::ProcessorSlotRole::Effect,
            Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_GAIN_ID.to_string(),
                name: "Gain".to_string(),
                backend: ProcessorBrowserBackend::BuiltIn,
            }),
            true,
            false,
            Some(super::ProcessorSlotSegment::Picker),
            true,
        ),
    );

    ui.point_at(iced::Point::new(96.0, INSTRUMENT_PICKER_HEIGHT / 2.0));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleProcessorBrowser(clicked)) if clicked == target
        )
    }));
}

fn clicked_effect_slot_messages(strip_index: usize, slot_index: usize) -> Vec<Message> {
    let target = crate::app::processor_editor_windows::EditorTarget {
        strip_index,
        slot_index,
    };
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [INSTRUMENT_SLOT_WIDTH, INSTRUMENT_PICKER_HEIGHT],
        super::processor_slot_controls(
            target,
            super::ProcessorSlotRole::Effect,
            Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_GAIN_ID.to_string(),
                name: "Gain".to_string(),
                backend: ProcessorBrowserBackend::BuiltIn,
            }),
            true,
            false,
            None,
            true,
        ),
    );
    ui.point_at(iced::Point::new(24.0, INSTRUMENT_PICKER_HEIGHT / 2.0));
    let _discarded = ui.simulate(simulator::click());
    ui.into_messages().collect()
}

#[test]
fn effect_slot_click_emits_drag_start_and_drop_messages() {
    assert_effect_slot_click_emits_drag_messages(3, 2, 1);
}

#[test]
fn master_effect_slot_click_emits_drag_start_and_drop_messages() {
    assert_effect_slot_click_emits_drag_messages(0, 1, 0);
}

fn assert_effect_slot_click_emits_drag_messages(
    strip_index: usize,
    slot_index: usize,
    effect_index: usize,
) {
    let messages = clicked_effect_slot_messages(strip_index, slot_index);
    assert!(messages.iter().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::StartTrackEffectDrag {
                strip_index: actual_strip,
                effect_index: actual_effect,
            })
            if *actual_strip == strip_index && *actual_effect == effect_index
        )
    }));
    assert!(messages.iter().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::DropTrackEffect {
                strip_index: actual_strip,
                effect_index: actual_effect,
            })
            if *actual_strip == strip_index && *actual_effect == effect_index
        )
    }));
}

#[test]
fn master_effect_rack_empty_slot_opens_master_effect_picker() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            super::EFFECT_RACK_SLOT_WIDTH,
            super::PROCESSOR_SLOT_BUTTON_HEIGHT,
        ],
        super::effect_rack(0, Vec::new(), None, None, true, 1),
    );

    ui.point_at(iced::Point::new(
        super::EFFECT_RACK_SLOT_WIDTH / 2.0,
        super::PROCESSOR_SLOT_BUTTON_HEIGHT / 2.0,
    ));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleProcessorBrowser(target))
                if target.strip_index == 0 && target.slot_index == 1
        )
    }));
}

#[test]
fn master_effect_browser_overlay_shows_effect_picker() {
    lilypalooza_builtins::register_all();
    let (mut app, _task) = crate::app::new_with_default_test_state();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.open_processor_browser_target = Some(crate::app::processor_editor_windows::EditorTarget {
        strip_index: 0,
        slot_index: 1,
    });

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [800.0, 600.0],
        super::instrument_browser_overlay(&app),
    );

    ui.find("Choose Effect")
        .expect("effect picker title should render");
    ui.find("Master").expect("master section should render");
    ui.find("Built-in")
        .expect("built-in section title should render");
    ui.find("Utility")
        .expect_err("collapsed utility section should not expose its children");
    ui.find("Gain")
        .expect_err("collapsed gain entry should not render");
}
