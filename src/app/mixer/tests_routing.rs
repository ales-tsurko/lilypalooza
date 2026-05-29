use iced::{
    Color, Element, Length, Point, Theme,
    widget::{button, container},
};
use iced_test::{Simulator, simulator};
use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BUILTIN_GAIN_ID, BUILTIN_SOUNDFONT_ID, MixerState, SlotState,
    mixer::{MixerMeterSnapshotWindow, StripMeterSnapshot, TrackRoute},
};

use super::{
    tests_layout::{assert_snapshot_matches, assert_snapshots_differ, is_grid_multiple},
    *,
};
use crate::{app::meters::meter_colors, icons, ui_style};

fn open_panel_snapshot_size(section_width: f32, extra_gap: f32, height: f32) -> [f32; 2] {
    [
        section_width + super::EFFECT_RACK_PANEL_WIDTH + extra_gap,
        height,
    ]
}

#[test]
fn route_choices_reflect_renamed_buses() {
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let bus_id = playback
        .mixer()
        .add_bus("Verb")
        .expect("bus should be added");
    playback
        .mixer()
        .set_bus_name(bus_id, "Long Hall")
        .expect("bus should be renamed");

    let choices = super::route_choices(playback.mixer_state(), RoutingStrip::Track(0));

    assert!(choices.iter().any(|choice| choice.label == "Long Hall"));
    assert!(!choices.iter().any(|choice| choice.label == "Verb"));
}

#[test]
fn track_effect_rack_panel_matches_snapshot() {
    lilypalooza_builtins::register_all();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_track_effects(
            lilypalooza_audio::TrackId(0),
            vec![SlotState::built_in(
                BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState::default(),
            )],
        )
        .expect("effect should be installed");
    let mixer = playback.mixer_state().clone();

    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [super::EFFECT_RACK_PANEL_WIDTH, 360.0],
        super::track_effect_rack_panel(
            1,
            super::effect_slot_dependencies(&mixer.tracks()[0]),
            Some(super::EffectRackPanelRouting {
                source: RoutingStrip::Track(0),
                sends: super::send_dependencies(&mixer.tracks()[0].routing),
                send_choices: super::send_destination_choices(&mixer, RoutingStrip::Track(0)),
            }),
            None,
            None,
            true,
            360.0,
        ),
    );

    assert_snapshot_matches(&mut ui, "tests/snapshots/track_effect_rack_panel");
}

#[test]
fn instrument_track_area_open_panel_matches_snapshot() {
    assert_open_effect_panel_snapshot(OpenEffectPanelArea::InstrumentTrack);
}

#[test]
fn master_track_area_open_panel_matches_snapshot() {
    assert_open_effect_panel_snapshot(OpenEffectPanelArea::Master);
}

#[derive(Clone, Copy)]
enum OpenEffectPanelArea {
    InstrumentTrack,
    Master,
}

fn assert_open_effect_panel_snapshot(area: OpenEffectPanelArea) {
    let mixer = MixerState::new();
    let meters = MixerMeterSnapshotWindow::default();
    let colors = meter_colors(&Theme::Dark);
    let track_colors = [crate::track_colors::default_track_color(0)];
    let (size, content, snapshot): ([f32; 2], Element<'_, Message>, &str) = match area {
        OpenEffectPanelArea::InstrumentTrack => (
            open_panel_snapshot_size(STRIP_WIDTH, ui_style::grid_f32(8), 520.0),
            super::instrument_track_area(super::InstrumentTrackAreaArgs {
                mixer: &mixer,
                meters: &meters,
                colors,
                strip_height: STRIP_MIN_HEIGHT,
                gain_mode: GainControlMode::Knob,
                visible: 0..1,
                existing_track_count: 1,
                track_colors: &track_colors,
                renaming_target: None,
                renaming_origin: None,
                track_rename_value: "",
                track_rename_color_value: Color::TRANSPARENT,
                track_rename_color_picker_open: false,
                selected_track_index: None,
                hovered_processor_slot: None,
                effect_drag_source: None,
                effect_drag_target: None,
                open_effect_rack_strips: &[1],
                controls_enabled: true,
            }),
            "tests/snapshots/instrument_track_area_open_panel",
        ),
        OpenEffectPanelArea::Master => (
            open_panel_snapshot_size(
                MAIN_SECTION_WIDTH,
                0.0,
                STRIP_MIN_HEIGHT + ui_style::grid_f32(8),
            ),
            super::master_track_area(super::MasterTrackAreaArgs {
                mixer: &mixer,
                meter_snapshot: StripMeterSnapshot::default(),
                colors,
                strip_height: STRIP_MIN_HEIGHT,
                gain_mode: GainControlMode::Knob,
                open_effect_rack_strips: &[0],
                hovered_processor_slot: None,
                effect_drag: None,
                controls_enabled: true,
            }),
            "tests/snapshots/master_track_area_open_panel",
        ),
    };
    let mut ui = Simulator::with_size(iced::Settings::default(), size, content);
    assert_snapshot_matches(&mut ui, snapshot);
}

#[test]
fn mixer_strip_min_height_no_longer_reserves_per_track_effect_rack() {
    assert!(
        super::gain_control_height(360.0, super::GainControlMode::Fader) >= 96.0,
        "effect rack lives in the side panel while the strip footer still needs routing space"
    );
}

#[test]
fn instrument_slot_hit_button_does_not_draw_segment_hover() {
    let idle = super::transparent_hit_button(&Theme::Dark, button::Status::Active);
    let hovered = super::transparent_hit_button(&Theme::Dark, button::Status::Hovered);

    assert_eq!(
        idle.background, hovered.background,
        "slot hit areas should not draw rectangular segment hover"
    );
}

#[test]
fn instrument_slot_hit_button_has_split_foreground_hover() {
    let idle = super::transparent_hit_button(&Theme::Dark, button::Status::Active);
    let hovered = super::transparent_hit_button(&Theme::Dark, button::Status::Hovered);

    assert_ne!(
        idle.text_color, hovered.text_color,
        "slot hit areas should highlight only their own foreground"
    );
}

#[test]
fn instrument_slot_text_foreground_matches_icon_foreground() {
    let idle_text = super::transparent_hit_button(&Theme::Dark, button::Status::Active).text_color;
    let idle_icon = ui_style::svg_muted_control(&Theme::Dark, iced::widget::svg::Status::Idle)
        .color
        .expect("muted icon idle color should exist");
    assert_eq!(idle_text, idle_icon);

    let hovered_text =
        super::transparent_hit_button(&Theme::Dark, button::Status::Hovered).text_color;
    let hovered_icon =
        ui_style::svg_muted_control(&Theme::Dark, iced::widget::svg::Status::Hovered)
            .color
            .expect("muted icon hover color should exist");
    assert_eq!(hovered_text, hovered_icon);
}

#[test]
fn hovered_processor_label_is_truncated() {
    let label = super::processor_hover_label("Very Long Effect Processor Name", false);

    assert!(label.chars().count() <= super::PROCESSOR_SLOT_LABEL_MAX_LEN);
    assert_ne!(label, "Very Long Effect Processor Name");
}

#[test]
fn effect_rack_processor_label_is_truncated_to_split_slot_width() {
    let label = super::processor_hover_label("Very Long Effect Processor Name", true);

    assert!(label.chars().count() <= super::PROCESSOR_SLOT_LABEL_MAX_LEN);
    assert_ne!(label, "Very Long Effect Processor Name");
}

#[test]
fn instrument_slot_surface_has_whole_button_hover_reaction() {
    let idle = ui_style::button_selector_field(&Theme::Dark, button::Status::Active, false);
    let hovered = ui_style::button_selector_field(&Theme::Dark, button::Status::Hovered, false);

    assert_ne!(
        idle.background, hovered.background,
        "slot surface should still have a whole-button hover reaction"
    );
}

#[test]
fn remove_bus_button_idle_and_hover_render_differ() {
    let view = || -> Element<'static, Message> {
        container(
            ui_style::flat_icon_button(
                icons::x(),
                ui_style::grid_f32(4),
                ui_style::grid_f32(3),
                ui_style::button_flat_compact_control,
                ui_style::svg_dimmed_control,
            )
            .on_press(Message::Noop),
        )
        .width(Length::Fixed(ui_style::grid_f32(8)))
        .height(Length::Fixed(ui_style::grid_f32(8)))
        .center_x(Length::Fixed(ui_style::grid_f32(8)))
        .center_y(Length::Fixed(ui_style::grid_f32(8)))
        .into()
    };

    let mut idle = Simulator::with_size(
        iced::Settings::default(),
        [ui_style::grid_f32(8), ui_style::grid_f32(8)],
        view(),
    );

    let mut hover = Simulator::with_size(
        iced::Settings::default(),
        [ui_style::grid_f32(8), ui_style::grid_f32(8)],
        view(),
    );
    hover.point_at(iced::Point::new(
        ui_style::grid_f32(4),
        ui_style::grid_f32(4),
    ));

    assert_snapshots_differ(
        &mut idle,
        &mut hover,
        "remove_bus_button_idle_hover_difference",
    );
}

#[test]
fn empty_slot_maps_to_none_choice() {
    let mixer = MixerState::new();
    assert_eq!(
        selected_instrument_choice(Some(&SlotState::default()), &mixer),
        Some(InstrumentChoice::None)
    );
}

#[test]
fn soundfont_slot_maps_to_soundfont_choice() {
    lilypalooza_builtins::register_all();
    let mixer = MixerState::new();
    assert_eq!(
        selected_instrument_choice(
            Some(&SlotState::built_in(
                BUILTIN_SOUNDFONT_ID,
                lilypalooza_builtins::soundfont_synth::state("default", 0, 2),
            )),
            &mixer
        ),
        Some(InstrumentChoice::Processor {
            processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
            name: "SF-01".to_string(),
            backend: ProcessorBrowserBackend::BuiltIn,
        })
    );
}

#[test]
fn processor_choices_filter_instruments_and_effects_by_role() {
    lilypalooza_builtins::register_all();

    let instruments = processor_choices(super::ProcessorSlotRole::Instrument);
    let effects = processor_choices(super::ProcessorSlotRole::Effect);

    assert!(instruments.iter().any(|choice| matches!(
        choice,
        InstrumentChoice::Processor { processor_id, .. } if processor_id == BUILTIN_SOUNDFONT_ID
    )));
    assert!(!instruments.iter().any(|choice| matches!(
        choice,
        InstrumentChoice::Processor { processor_id, .. } if processor_id == BUILTIN_GAIN_ID
    )));
    assert!(effects.iter().any(|choice| matches!(
        choice,
        InstrumentChoice::Processor { processor_id, .. } if processor_id == BUILTIN_GAIN_ID
    )));
    assert!(!effects.iter().any(|choice| matches!(
        choice,
        InstrumentChoice::Processor { processor_id, .. } if processor_id == BUILTIN_SOUNDFONT_ID
    )));
}

#[test]
fn gain_effect_slot_maps_to_effect_choice() {
    lilypalooza_builtins::register_all();

    assert_eq!(
        selected_processor_choice(
            Some(&SlotState::built_in(
                BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState::default(),
            )),
            super::ProcessorSlotRole::Effect,
        ),
        Some(InstrumentChoice::Processor {
            processor_id: BUILTIN_GAIN_ID.to_string(),
            name: "Gain".to_string(),
            backend: ProcessorBrowserBackend::BuiltIn,
        })
    );
}

#[test]
fn built_in_browser_entries_filter_by_instrument_name_and_group_by_type() {
    lilypalooza_builtins::register_all();
    let choices = vec![
        InstrumentChoice::None,
        InstrumentChoice::Processor {
            processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
            name: "SoundFont".to_string(),
            backend: ProcessorBrowserBackend::BuiltIn,
        },
    ];

    let browser = instrument_browser_entries(&choices, "sound");

    assert!(!browser.show_none);
    assert_eq!(
        browser.backends,
        vec![super::ProcessorBrowserBackendSection {
            key: super::ProcessorBrowserSectionKey::backend(
                super::ProcessorSlotRole::Instrument,
                ProcessorBrowserBackend::BuiltIn,
            ),
            title: "Built-in".to_string(),
            sections: vec![super::ProcessorBrowserSection {
                key: super::ProcessorBrowserSectionKey::new(
                    super::ProcessorSlotRole::Instrument,
                    ProcessorBrowserBackend::BuiltIn,
                    "Sampler".to_string(),
                ),
                title: "Sampler".to_string(),
                entries: vec![InstrumentChoice::Processor {
                    processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                    name: "SoundFont".to_string(),
                    backend: ProcessorBrowserBackend::BuiltIn,
                }],
            }],
        }]
    );
}

#[test]
fn processor_browser_empty_choice_is_available_on_single_page() {
    let choices = vec![
        InstrumentChoice::None,
        InstrumentChoice::Processor {
            processor_id: "clap:/tmp/vendor.clap#vendor.instrument".to_string(),
            name: "Vendor Instrument".to_string(),
            backend: ProcessorBrowserBackend::Clap,
        },
    ];

    let browser = instrument_browser_entries(&choices, "");

    assert!(browser.show_none);
}

#[test]
fn plugin_browser_entries_group_by_manufacturer() {
    const TEST_DESCRIPTOR: lilypalooza_audio::ProcessorDescriptor =
        lilypalooza_audio::ProcessorDescriptor {
            name: "Vendor Effect",
            params: &[],
            editor: None,
        };
    let processor_id = "clap:/tmp/vendor.clap#vendor.effect";
    lilypalooza_audio::instrument::registry::register([
        lilypalooza_audio::instrument::registry::Entry::plugin_processor(
            processor_id.to_string(),
            "Vendor Effect".to_string(),
            lilypalooza_audio::instrument::registry::Backend::Clap,
            Some("Acme Audio".to_string()),
            &TEST_DESCRIPTOR,
            lilypalooza_audio::instrument::registry::RuntimeFactory::Effect(|_, _| Ok(None)),
        ),
    ]);
    let choices = vec![InstrumentChoice::Processor {
        processor_id: processor_id.to_string(),
        name: "Vendor Effect".to_string(),
        backend: ProcessorBrowserBackend::Clap,
    }];

    let browser = instrument_browser_entries(&choices, "");

    assert_eq!(
        browser.backends,
        vec![super::ProcessorBrowserBackendSection {
            key: super::ProcessorBrowserSectionKey::backend(
                super::ProcessorSlotRole::Instrument,
                ProcessorBrowserBackend::Clap,
            ),
            title: "CLAP".to_string(),
            sections: vec![super::ProcessorBrowserSection {
                key: super::ProcessorBrowserSectionKey::new(
                    super::ProcessorSlotRole::Instrument,
                    ProcessorBrowserBackend::Clap,
                    "Acme Audio".to_string(),
                ),
                title: "Acme Audio".to_string(),
                entries: choices,
            }],
        }]
    );
}

#[test]
fn processor_browser_entries_show_multiple_backends_on_one_page() {
    const TEST_DESCRIPTOR: lilypalooza_audio::ProcessorDescriptor =
        lilypalooza_audio::ProcessorDescriptor {
            name: "CLAP Instrument",
            params: &[],
            editor: None,
        };
    let plugin_id = "clap:/tmp/vendor.clap#vendor.instrument";
    lilypalooza_builtins::register_all();
    lilypalooza_audio::instrument::registry::register([
        lilypalooza_audio::instrument::registry::Entry::plugin_processor(
            plugin_id.to_string(),
            "CLAP Instrument".to_string(),
            lilypalooza_audio::instrument::registry::Backend::Clap,
            Some("Acme Audio".to_string()),
            &TEST_DESCRIPTOR,
            lilypalooza_audio::instrument::registry::RuntimeFactory::Instrument(|_, _| Ok(None)),
        ),
    ]);
    let choices = vec![
        InstrumentChoice::None,
        InstrumentChoice::Processor {
            processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
            name: "SF-01".to_string(),
            backend: ProcessorBrowserBackend::BuiltIn,
        },
        InstrumentChoice::Processor {
            processor_id: plugin_id.to_string(),
            name: "CLAP Instrument".to_string(),
            backend: ProcessorBrowserBackend::Clap,
        },
    ];

    let browser = instrument_browser_entries(&choices, "");
    let titles = browser
        .backends
        .iter()
        .map(|backend| backend.title.as_str())
        .collect::<Vec<_>>();

    assert_eq!(titles, vec!["Built-in", "CLAP"]);
}

#[test]
fn processor_browser_sections_are_folded_by_default_and_expand_on_search() {
    let key = super::ProcessorBrowserSectionKey::new(
        super::ProcessorSlotRole::Effect,
        ProcessorBrowserBackend::BuiltIn,
        "Utility".to_string(),
    );

    assert!(!super::processor_browser_section_expanded(&key, &[], ""));
    assert!(super::processor_browser_section_expanded(
        &key,
        std::slice::from_ref(&key),
        ""
    ));
    assert!(super::processor_browser_section_expanded(&key, &[], "gain"));
}

#[test]
fn processor_browser_section_header_toggles_section() {
    let key = super::ProcessorBrowserSectionKey::new(
        super::ProcessorSlotRole::Effect,
        ProcessorBrowserBackend::BuiltIn,
        "Utility".to_string(),
    );
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [240.0, 48.0],
        super::processor_browser_section_header(
            key.clone(),
            "Utility".to_string(),
            false,
            super::ProcessorBrowserRowDepth::Group,
        ),
    );

    ui.point_at(Point::new(20.0, 16.0));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleProcessorBrowserSection(clicked)) if clicked == key
        )
    }));
}

#[test]
fn processor_browser_row_padding_tracks_hierarchy_depth() {
    let backend = super::processor_browser_choice_padding(super::ProcessorBrowserRowDepth::Root);
    let group = super::processor_browser_choice_padding(super::ProcessorBrowserRowDepth::Group);
    let leaf = super::processor_browser_choice_padding(super::ProcessorBrowserRowDepth::Leaf);

    assert_eq!(backend[1], ui_style::grid(3));
    assert_eq!(group[1], ui_style::grid(7));
    assert_eq!(leaf[1], ui_style::grid(11));
}

#[test]
fn processor_browser_section_headers_use_folder_icons() {
    assert_eq!(
        super::processor_browser_section_icon(false),
        icons::folder()
    );
    assert_eq!(
        super::processor_browser_section_icon(true),
        icons::folder_open()
    );
}

#[test]
fn processor_browser_leaf_icons_match_slot_role() {
    let choice = InstrumentChoice::Processor {
        processor_id: "example".to_string(),
        name: "Example".to_string(),
        backend: ProcessorBrowserBackend::BuiltIn,
    };

    assert_eq!(
        super::processor_browser_choice_icon(super::ProcessorSlotRole::Instrument, &choice),
        icons::keyboard_music()
    );
    assert_eq!(
        super::processor_browser_choice_icon(super::ProcessorSlotRole::Effect, &choice),
        icons::audio_waveform()
    );
}

#[test]
fn instrument_trigger_label_uses_none_and_truncates_long_names() {
    assert_eq!(instrument_trigger_label(None), "Empty");

    let label = instrument_trigger_label(Some(&InstrumentChoice::Processor {
        processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
        name: "Extremely Long SoundFont Synth Name".to_string(),
        backend: ProcessorBrowserBackend::BuiltIn,
    }));
    assert!(label.chars().count() <= super::PROCESSOR_SLOT_LABEL_MAX_LEN);
    assert_ne!(label, "Extremely Long SoundFont Synth Name");
}

#[test]
fn instrument_trigger_label_uses_none_for_empty_choice() {
    assert_eq!(
        instrument_trigger_label(Some(&InstrumentChoice::None)),
        "Empty"
    );
}

#[test]
fn instrument_slot_primary_action_opens_editor_when_available() {
    assert!(matches!(
        instrument_slot_primary_action(
            2,
            Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SoundFont".to_string(),
                backend: ProcessorBrowserBackend::BuiltIn,
            }),
            true,
            true,
        ),
        Some(crate::app::messages::Message::Mixer(
            crate::app::messages::MixerMessage::OpenEditor(_)
        ))
    ));
}

#[test]
fn instrument_slot_primary_action_opens_picker_only_when_empty() {
    assert!(matches!(
        instrument_slot_primary_action(2, Some(&InstrumentChoice::None), false, true),
        Some(crate::app::messages::Message::Mixer(
            crate::app::messages::MixerMessage::ToggleProcessorBrowser(target)
        )) if target.strip_index == 3 && target.slot_index == 0
    ));
    assert!(
        instrument_slot_primary_action(
            2,
            Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SoundFont".to_string(),
                backend: ProcessorBrowserBackend::BuiltIn,
            }),
            false,
            true,
        )
        .is_none()
    );
}

#[test]
fn short_mixer_height_uses_gain_knob_mode() {
    assert_eq!(gain_control_mode(MIXER_MIN_HEIGHT), GainControlMode::Knob);
    assert_eq!(
        gain_control_mode(MIXER_MIN_HEIGHT - 10.0),
        GainControlMode::Knob
    );
    assert_eq!(
        gain_control_mode(MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET - 1.0),
        GainControlMode::Knob
    );
    assert_eq!(
        gain_control_mode(MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET + 1.0),
        GainControlMode::Fader
    );
}

#[test]
fn empty_toggle_slots_use_track_toggle_size() {
    crate::test_assertions::assert_float_eq!(STRIP_TOGGLE_SIZE, INSTRUMENT_PICKER_HEIGHT - 4.0);
}

#[test]
fn meter_and_gain_controls_share_same_height() {
    let strip_height = 280.0;
    crate::test_assertions::assert_float_eq!(
        meter_control_height(strip_height, GainControlMode::Fader),
        gain_control_height(strip_height, GainControlMode::Fader)
    );
    crate::test_assertions::assert_float_eq!(
        meter_control_height(strip_height, GainControlMode::Knob),
        gain_control_height(strip_height, GainControlMode::Knob)
    );
    crate::test_assertions::assert_float_eq!(
        control_stack_height(meter_control_height(strip_height, GainControlMode::Fader)),
        control_stack_height(gain_control_height(strip_height, GainControlMode::Fader))
    );
}

#[test]
fn meter_peak_label_uses_hold_and_floor() {
    assert_eq!(meter_peak_label(StripMeterSnapshot::default()), "-inf");
    let snapshot = StripMeterSnapshot {
        left: ChannelMeterSnapshot {
            level: 0.2,
            hold: 1.0,
            hold_db: 3.2,
        },
        right: ChannelMeterSnapshot {
            level: 0.2,
            hold: 0.5,
            hold_db: -6.0,
        },
        clip_latched: false,
    };
    assert_eq!(meter_peak_label(snapshot), "3.2");
}

#[test]
fn gain_label_uses_negative_infinity_at_floor() {
    assert_eq!(gain_label(-60.0), "-inf");
    assert_eq!(gain_label(-24.0), "-24.0");
}

#[test]
fn main_section_width_includes_group_borders() {
    crate::test_assertions::assert_float_eq!(
        MAIN_SECTION_WIDTH,
        MAIN_STRIP_WIDTH + GROUP_SIDE_BORDER_WIDTH * 2.0
    );
}

#[test]
fn value_labels_use_shared_slot_height() {
    assert!(is_grid_multiple(VALUE_LABEL_HEIGHT));
}

#[test]
fn control_stack_height_adds_shared_label_slot() {
    let control: f32 = 100.0;
    crate::test_assertions::assert_float_eq!(
        control_stack_height(control),
        control + VALUE_LABEL_HEIGHT + ui_style::SPACE_XS as f32
    );
}

#[test]
fn compact_gain_mode_hides_meter_scale() {
    assert!(!meter_scale_visible(GainControlMode::Knob));
    assert!(meter_scale_visible(GainControlMode::Fader));
}

#[test]
fn visible_strip_window_limits_rendered_strip_count() {
    let visible = visible_strip_window(128, 0.0, STRIP_WIDTH * 4.0);
    assert!(visible.end - visible.start <= 4 + STRIP_VIRTUALIZATION_OVERSCAN * 2);

    let scrolled = visible_strip_window(128, STRIP_WIDTH * 40.0, STRIP_WIDTH * 4.0);
    assert!(scrolled.start >= 38);
    assert!(scrolled.end <= 46);
}

#[test]
fn only_existing_roll_tracks_use_tint() {
    assert!(track_should_use_roll_tint(0, 4));
    assert!(track_should_use_roll_tint(3, 4));
    assert!(!track_should_use_roll_tint(4, 4));
    assert!(!track_should_use_roll_tint(127, 4));
}

#[test]
fn track_strip_dependency_includes_tint_state() {
    let base = TrackStripDependency {
        index: 0,
        name: "Track".to_string(),
        selected: Some(InstrumentChoice::None),
        editor_enabled: false,
        effects: Vec::new(),
        hovered_processor_slot: None,
        color_bits: color_bits(iced::Color::from_rgb(0.1, 0.2, 0.3)),
        gain_bits: 0.0f32.to_bits(),
        pan_bits: 0.0f32.to_bits(),
        route: RouteChoice {
            route: TrackRoute::Master,
            label: "Master".to_string(),
        },
        route_choices: vec![RouteChoice {
            route: TrackRoute::Master,
            label: "Master".to_string(),
        }],
        meter: MeterDependency::from_snapshot(StripMeterSnapshot::default()),
        compact_gain: false,
        effect_rack_open: false,
        panel_has_content: false,
        strip_height_bits: 140.0f32.to_bits(),
        soloed: false,
        muted: false,
        tint_enabled: false,
        highlighted: false,
        renaming: false,
        rename_value: String::new(),
        color_picker_open: false,
    };
    let tinted = TrackStripDependency {
        tint_enabled: true,
        ..base.clone()
    };

    assert_ne!(base, tinted);
}

#[test]
fn strip_lazy_dependencies_include_panel_open_state() {
    let master_closed = super::MainStripDependency {
        gain_bits: 0.0f32.to_bits(),
        pan_bits: 0.0f32.to_bits(),
        meter: MeterDependency::from_snapshot(StripMeterSnapshot::default()),
        compact_gain: false,
        effect_rack_open: false,
        panel_has_content: false,
        strip_height_bits: 240.0f32.to_bits(),
    };
    let master_open = super::MainStripDependency {
        effect_rack_open: true,
        ..master_closed.clone()
    };

    let track_closed = TrackStripDependency {
        index: 0,
        name: "Track".to_string(),
        selected: Some(InstrumentChoice::None),
        editor_enabled: false,
        effects: Vec::new(),
        hovered_processor_slot: None,
        color_bits: color_bits(iced::Color::from_rgb(0.1, 0.2, 0.3)),
        gain_bits: 0.0f32.to_bits(),
        pan_bits: 0.0f32.to_bits(),
        route: RouteChoice {
            route: TrackRoute::Master,
            label: "Master".to_string(),
        },
        route_choices: vec![RouteChoice {
            route: TrackRoute::Master,
            label: "Master".to_string(),
        }],
        meter: MeterDependency::from_snapshot(StripMeterSnapshot::default()),
        compact_gain: false,
        effect_rack_open: false,
        panel_has_content: false,
        strip_height_bits: 240.0f32.to_bits(),
        soloed: false,
        muted: false,
        tint_enabled: false,
        highlighted: false,
        renaming: false,
        rename_value: String::new(),
        color_picker_open: false,
    };
    let track_open = TrackStripDependency {
        effect_rack_open: true,
        ..track_closed.clone()
    };

    assert_ne!(master_closed, master_open);
    assert_ne!(track_closed, track_open);
}
