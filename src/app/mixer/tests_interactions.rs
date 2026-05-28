use iced::{Color, Event, Point, Theme, mouse, widget::button};
use iced_test::{Simulator, simulator};
use lilypalooza_audio::{AudioEngine, AudioEngineOptions, MixerState, mixer::TrackRoute};

use super::{
    tests_layout::{assert_snapshots_differ, color_distance, test_effect_slot},
    *,
};
use crate::{icons, ui_style};

#[test]
fn effect_rack_only_first_empty_slot_opens_picker() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            super::EFFECT_RACK_SLOT_WIDTH,
            super::PROCESSOR_SLOT_BUTTON_HEIGHT * 3.0,
        ],
        super::effect_rack(1, Vec::new(), None, None, true, 3),
    );

    ui.point_at(iced::Point::new(
        super::EFFECT_RACK_SLOT_WIDTH / 2.0,
        super::PROCESSOR_SLOT_BUTTON_HEIGHT * 1.5,
    ));
    let _discarded = ui.simulate(simulator::click());

    assert!(!ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleProcessorBrowser(_))
        )
    }));
}

#[test]
fn panel_toggle_button_emits_panel_toggle_message() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [super::STRIP_TOGGLE_SIZE, super::STRIP_TOGGLE_SIZE],
        super::strip_panel_toggle_button(
            false,
            false,
            Message::Mixer(MixerMessage::ToggleMixerEffectRack(0)),
        ),
    );

    ui.point_at(iced::Point::new(
        super::STRIP_TOGGLE_SIZE / 2.0,
        super::STRIP_TOGGLE_SIZE / 2.0,
    ));
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::ToggleMixerEffectRack(0))
        )
    }));
}

#[test]
fn panel_toggle_button_style_differs_from_mute_solo_style() {
    let panel = super::strip_panel_toggle_style(&Theme::Dark, button::Status::Active, false);
    let mute_solo = ui_style::button_compact_solid(&Theme::Dark, button::Status::Active);

    assert_ne!(panel.background, mute_solo.background);
}

#[test]
fn open_panel_toggle_uses_hover_style() {
    let open = super::strip_panel_toggle_style(&Theme::Dark, button::Status::Active, true);
    let hovered = ui_style::button_flat_compact_control(&Theme::Dark, button::Status::Hovered);

    assert_eq!(open.background, hovered.background);
    assert_eq!(open.border, hovered.border);
}

#[test]
fn folded_panel_toggle_indicates_hidden_content() {
    let empty = super::strip_panel_toggle_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        false,
    );
    let has_content = super::strip_panel_toggle_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        true,
    );

    assert_ne!(empty.color, has_content.color);
}

#[test]
fn instrument_slot_hit_button_keeps_label_text_visible() {
    let style = super::transparent_hit_button(&Theme::Dark, button::Status::Active);

    assert_ne!(
        style.text_color,
        Color::TRANSPARENT,
        "slot label text must remain visible inside the transparent hit area"
    );
}

#[test]
fn instrument_slot_area_separator_is_visible() {
    let style = super::slot_area_separator_surface(&Theme::Dark);

    assert!(
        style.background.is_some(),
        "slot editor and picker areas should have a visible separator"
    );
}

#[test]
fn instrument_slot_editor_area_is_compact() {
    assert!(
        super::INSTRUMENT_SLOT_EDITOR_AREA_WIDTH
            <= super::INSTRUMENT_BROWSER_ICON_SIZE + ui_style::grid_f32(1),
        "slot editor area should hug the icon instead of taking a wide chunk"
    );
}

#[test]
fn effect_rack_uses_fixed_scrollable_height() {
    let rack_height = super::EFFECT_RACK_HEIGHT;
    let picker_height = super::INSTRUMENT_PICKER_HEIGHT;

    crate::test_assertions::assert_float_eq!(
        rack_height,
        super::EFFECT_RACK_ROW_HEIGHT * super::EFFECT_RACK_VISIBLE_SLOTS as f32
    );
    assert!(rack_height > picker_height);
}

#[test]
fn effect_rack_panel_is_narrower_than_channel_strip() {
    let panel_width = super::EFFECT_RACK_PANEL_WIDTH;
    let strip_width = super::STRIP_WIDTH;
    assert!(panel_width < strip_width);
}

#[test]
fn effect_rack_panel_reserves_even_space_for_rack_and_routing() {
    let strip_height = 480.0;
    let (rack_height, routing_height) = super::effect_rack_panel_heights(strip_height, true);
    let available_height = strip_height - super::EFFECT_RACK_SEPARATOR_HEIGHT;

    assert!((rack_height / available_height - 0.5).abs() < 0.001);
    assert!((routing_height / available_height - 0.5).abs() < 0.001);
    assert!(
        (rack_height + routing_height + super::EFFECT_RACK_SEPARATOR_HEIGHT - strip_height).abs()
            < 0.001
    );
}

#[test]
fn master_effect_rack_panel_uses_full_height_without_routing() {
    let strip_height = 480.0;
    let (rack_height, routing_height) = super::effect_rack_panel_heights(strip_height, false);

    crate::test_assertions::assert_float_eq!(rack_height, strip_height);
    crate::test_assertions::assert_float_eq!(routing_height, 0.0);
}

#[test]
fn effect_rack_row_height_includes_one_separator() {
    crate::test_assertions::assert_float_eq!(
        super::EFFECT_RACK_ROW_HEIGHT,
        super::PROCESSOR_SLOT_BUTTON_HEIGHT + super::EFFECT_RACK_SEPARATOR_HEIGHT
    );
}

#[test]
fn effect_rack_scrollbar_is_narrow_and_reserved() {
    let width = super::EFFECT_RACK_SCROLLBAR_WIDTH;
    let scroller_width = super::EFFECT_RACK_SCROLLBAR_SCROLLER_WIDTH;
    let spacing = super::EFFECT_RACK_SCROLLBAR_SPACING;

    assert!(width < ui_style::grid_f32(2));
    assert!(scroller_width < width);
    assert!(spacing > 0.0);
}

#[test]
fn effect_rack_slot_style_does_not_draw_stacked_cell_borders() {
    let style =
        super::processor_slot_button_style(&Theme::Dark, button::Status::Active, false, true);

    crate::test_assertions::assert_float_eq!(
        style.border.width,
        0.0,
        "rack rows should use explicit shared separators, not per-row top/bottom borders"
    );
}

#[test]
fn effect_rack_add_button_background_is_transparent() {
    let idle = super::effect_rack_add_button_style(&Theme::Dark, button::Status::Active);
    let hovered = super::effect_rack_add_button_style(&Theme::Dark, button::Status::Hovered);

    assert_eq!(idle.background, None);
    assert_eq!(hovered.background, None);
    assert_ne!(idle.text_color, hovered.text_color);
}

#[test]
fn effect_rack_slot_background_is_transparent() {
    let idle =
        super::processor_slot_button_style(&Theme::Dark, button::Status::Active, false, true);
    let hovered =
        super::processor_slot_button_style(&Theme::Dark, button::Status::Hovered, false, true);

    assert_eq!(idle.background, None);
    assert_eq!(hovered.background, None);
}

#[test]
fn effect_drag_state_only_applies_to_matching_strip() {
    assert_eq!(
        super::effect_rack_drag_state(Some((1, 0)), Some((1, 2)), 1),
        Some(super::EffectRackDragState {
            source_effect_index: 0,
            target_effect_index: 2,
        })
    );
    assert_eq!(
        super::effect_rack_drag_state(Some((1, 0)), Some((2, 2)), 1),
        None
    );
    assert_eq!(
        super::effect_rack_drag_state(Some((1, 0)), Some((1, 0)), 1),
        None
    );
}

#[test]
fn effect_rack_drop_indicator_tracks_insert_position() {
    assert_eq!(
        super::effect_rack_drop_indicator(Some(super::EffectRackDragState {
            source_effect_index: 0,
            target_effect_index: 2,
        })),
        Some(super::EffectRackDropIndicator::After(2))
    );
    assert_eq!(
        super::effect_rack_drop_indicator(Some(super::EffectRackDragState {
            source_effect_index: 2,
            target_effect_index: 1,
        })),
        Some(super::EffectRackDropIndicator::After(0))
    );
    assert_eq!(
        super::effect_rack_drop_indicator(Some(super::EffectRackDragState {
            source_effect_index: 2,
            target_effect_index: 0,
        })),
        Some(super::EffectRackDropIndicator::Top)
    );
}

#[test]
fn effect_rack_drop_indicator_is_visually_distinct_from_separator() {
    let separator = super::effect_rack_separator_surface(&Theme::Dark);
    let indicator = super::effect_rack_drop_indicator_surface(&Theme::Dark);

    assert_ne!(separator.background, indicator.background);
}

#[test]
fn effect_rack_drag_indicator_changes_rendered_rack() {
    let effects = vec![
        super::EffectSlotDependency {
            slot_index: 1,
            instance_id: 10,
            instance_label_index: 1,
            selected: None,
            editor_enabled: false,
            bypassed: false,
        },
        super::EffectSlotDependency {
            slot_index: 2,
            instance_id: 11,
            instance_label_index: 2,
            selected: None,
            editor_enabled: false,
            bypassed: false,
        },
        super::EffectSlotDependency {
            slot_index: 3,
            instance_id: 12,
            instance_label_index: 3,
            selected: None,
            editor_enabled: false,
            bypassed: false,
        },
    ];
    let size = [
        super::EFFECT_RACK_SLOT_WIDTH,
        super::EFFECT_RACK_ROW_HEIGHT * 4.0,
    ];
    let mut idle = Simulator::with_size(
        iced::Settings::default(),
        size,
        super::effect_rack(1, effects.clone(), None, None, true, 4),
    );
    let mut dragging = Simulator::with_size(
        iced::Settings::default(),
        size,
        super::effect_rack(
            1,
            effects,
            None,
            Some(super::EffectRackDragState {
                source_effect_index: 0,
                target_effect_index: 2,
            }),
            true,
            4,
        ),
    );

    assert_snapshots_differ(
        &mut idle,
        &mut dragging,
        "effect_rack_drag_indicator_changes_rendered_rack",
    );
}

#[test]
fn effect_slot_display_labels_enumerate_duplicate_names_only() {
    let effects = vec![
        test_effect_slot(1, 10, 1, "Gain"),
        test_effect_slot(2, 11, 1, "Reverb"),
        test_effect_slot(3, 12, 2, "Gain"),
    ];

    assert_eq!(
        super::effect_slot_display_labels(&effects),
        vec![
            Some("Gain [1]".to_string()),
            Some("Reverb".to_string()),
            Some("Gain [2]".to_string())
        ]
    );
}

#[test]
fn effect_slot_display_labels_follow_persistent_label_index_after_reorder() {
    let effects = vec![
        test_effect_slot(1, 12, 3, "Gain"),
        test_effect_slot(2, 10, 1, "Gain"),
        test_effect_slot(3, 11, 2, "Gain"),
    ];

    assert_eq!(
        super::effect_slot_display_labels(&effects),
        vec![
            Some("Gain [3]".to_string()),
            Some("Gain [1]".to_string()),
            Some("Gain [2]".to_string())
        ]
    );
}

#[test]
fn effect_slot_display_labels_do_not_compact_after_middle_instance_removed() {
    let effects = vec![
        test_effect_slot(1, 10, 1, "Gain"),
        test_effect_slot(2, 12, 3, "Gain"),
    ];

    assert_eq!(
        super::effect_slot_display_labels(&effects),
        vec![Some("Gain [1]".to_string()), Some("Gain [3]".to_string())]
    );
}

#[test]
fn effect_rack_renders_duplicate_instance_numbers() {
    let effects = vec![
        test_effect_slot(1, 10, 1, "Gain"),
        test_effect_slot(2, 11, 2, "Gain"),
        test_effect_slot(3, 12, 3, "Gain"),
    ];
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [
            super::EFFECT_RACK_SLOT_WIDTH,
            super::EFFECT_RACK_ROW_HEIGHT * 4.0,
        ],
        super::effect_rack(1, effects, None, None, true, 4),
    );

    ui.find("Gain [1]").expect("first gain label should render");
    ui.find("Gain [2]")
        .expect("second gain label should render");
    ui.find("Gain [3]").expect("third gain label should render");
}

#[test]
fn processor_slot_hover_highlights_title_and_icon_together() {
    let idle_text = super::processor_slot_label_button_style(
        &Theme::Dark,
        button::Status::Active,
        false,
        false,
    )
    .text_color;
    let hovered_text =
        super::processor_slot_label_button_style(&Theme::Dark, button::Status::Active, false, true)
            .text_color;
    let hovered_icon = super::processor_slot_label_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        true,
    )
    .color;

    assert_ne!(idle_text, hovered_text);
    assert_eq!(hovered_icon, Some(hovered_text));
}

#[test]
fn processor_slot_hover_highlights_icon_across_clickable_area() {
    let idle_button =
        super::processor_slot_icon_button_style(&Theme::Dark, button::Status::Active, false)
            .text_color;
    let hovered_button =
        super::processor_slot_icon_button_style(&Theme::Dark, button::Status::Active, true)
            .text_color;
    let hovered_icon = super::processor_slot_active_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        true,
    )
    .color;

    assert_ne!(idle_button, hovered_button);
    assert_eq!(hovered_icon, Some(hovered_button));
}

#[test]
fn processor_slot_editor_hover_does_not_highlight_picker_icon() {
    let editor_hover_text =
        super::processor_slot_label_button_style(&Theme::Dark, button::Status::Active, false, true)
            .text_color;
    let picker_idle_icon = super::processor_slot_active_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        false,
    )
    .color;

    assert_ne!(picker_idle_icon, Some(editor_hover_text));
}

#[test]
fn selected_instrument_slot_accents_icon_and_name() {
    let idle_text = super::processor_slot_label_button_style(
        &Theme::Dark,
        button::Status::Active,
        false,
        false,
    )
    .text_color;
    let active_text =
        super::processor_slot_label_button_style(&Theme::Dark, button::Status::Active, true, false)
            .text_color;
    let active_icon = super::processor_slot_label_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        true,
        false,
    )
    .color;

    assert_ne!(idle_text, active_text);
    assert_eq!(active_icon, Some(active_text));
}

#[test]
fn selected_instrument_slot_accent_stays_readable() {
    let active_text =
        super::processor_slot_label_button_style(&Theme::Dark, button::Status::Active, true, false)
            .text_color;
    let palette = Theme::Dark.extended_palette();
    let raw_primary = palette.primary.base.color;
    let readable_text = palette.background.weak.text;

    assert!(
        color_distance(active_text, readable_text) < color_distance(raw_primary, readable_text)
    );
}

#[test]
fn bypass_icon_toggles_shape_without_state_color() {
    assert_eq!(super::processor_slot_bypass_icon(false), icons::power());
    assert_eq!(super::processor_slot_bypass_icon(true), icons::power_off());
    assert_ne!(
        super::processor_slot_bypass_icon(false),
        super::processor_slot_bypass_icon(true)
    );

    let normal = super::processor_slot_active_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        false,
        false,
    )
    .color;
    let bypassed = super::processor_slot_active_icon_style(
        &Theme::Dark,
        iced::widget::svg::Status::Idle,
        true,
        false,
    )
    .color;

    assert_eq!(normal, bypassed);
}

#[test]
fn effect_rack_panel_uses_even_rack_and_routing_heights() {
    let (rack_height, routing_height) = super::effect_rack_panel_heights(301.0, true);

    crate::test_assertions::assert_float_eq!(rack_height, 150.0);
    crate::test_assertions::assert_float_eq!(routing_height, 150.0);
}

#[test]
fn route_menus_shrink_to_item_count_until_maximum() {
    crate::test_assertions::assert_float_eq!(
        super::route_menu_height_for_items(1),
        super::ROUTE_MENU_ITEM_HEIGHT
    );
    crate::test_assertions::assert_float_eq!(
        super::route_menu_height_for_items(3),
        super::ROUTE_MENU_ITEM_HEIGHT * 3.0
    );
    crate::test_assertions::assert_float_eq!(
        super::route_menu_height_for_items(super::ROUTE_MENU_MAX_ITEMS + 4),
        super::ROUTE_MENU_ITEM_HEIGHT * super::ROUTE_MENU_MAX_ITEMS as f32
    );
}

#[test]
fn send_rows_reserve_a_second_line_for_gain() {
    let row_height = super::SEND_ROW_HEIGHT;
    let compact_controls_width = [super::SEND_PICKER_WIDTH, super::SEND_MODE_WIDTH]
        .iter()
        .sum::<f32>();
    let extra_spacing = [
        super::SEND_ROW_CONTENT_BOTTOM_SPACING,
        super::SEND_PANEL_TOP_SPACING,
    ]
    .iter()
    .sum::<f32>();

    assert!(row_height >= ui_style::grid_f32(12));
    crate::test_assertions::assert_float_eq!(super::SEND_MODE_HEIGHT, super::SEND_CONTROL_HEIGHT);
    assert!(extra_spacing > 0.0);
    assert!(compact_controls_width <= super::EFFECT_RACK_PANEL_WIDTH);
}

#[test]
fn send_gain_slider_double_click_resets_to_zero_db() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [super::EFFECT_RACK_PANEL_WIDTH, super::SEND_ROW_HEIGHT],
        super::send_row(
            RoutingStrip::Track(0),
            0,
            SendDependency {
                bus_id: 1,
                gain_bits: (-6.0f32).to_bits(),
                enabled: true,
                pre_fader: false,
            },
            vec![SendDestinationChoice {
                action: SendDestinationAction::Route(1),
                label: "Verb".to_string(),
            }],
            true,
        ),
    );

    ui.point_at(iced::Point::new(
        super::EFFECT_RACK_PANEL_WIDTH * 0.5,
        super::SEND_ROW_HEIGHT - ui_style::grid_f32(4),
    ));
    let _discarded = ui.simulate(simulator::click());
    let _discarded = ui.simulate(simulator::click());

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::SetSendGain(RoutingStrip::Track(0), 0, gain))
                if gain.abs() <= 1.0e-4
        )
    }));
}

#[test]
fn send_gain_slider_drag_changes_value() {
    let mut ui = Simulator::with_size(
        iced::Settings::default(),
        [super::EFFECT_RACK_PANEL_WIDTH, super::SEND_ROW_HEIGHT],
        super::send_row(
            RoutingStrip::Track(0),
            0,
            SendDependency {
                bus_id: 1,
                gain_bits: (-6.0f32).to_bits(),
                enabled: true,
                pre_fader: false,
            },
            vec![SendDestinationChoice {
                action: SendDestinationAction::Route(1),
                label: "Verb".to_string(),
            }],
            true,
        ),
    );
    let start = Point::new(
        ui_style::grid_f32(8),
        super::SEND_ROW_HEIGHT - ui_style::grid_f32(4),
    );
    let end = Point::new(
        super::EFFECT_RACK_PANEL_WIDTH - ui_style::grid_f32(8),
        super::SEND_ROW_HEIGHT - ui_style::grid_f32(4),
    );

    ui.point_at(start);
    let _discarded = ui.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    ui.point_at(end);
    let _discarded = ui.simulate([Event::Mouse(mouse::Event::CursorMoved { position: end })]);
    let _discarded = ui.simulate([Event::Mouse(mouse::Event::ButtonReleased(
        mouse::Button::Left,
    ))]);

    assert!(ui.into_messages().any(|message| {
        matches!(
            message,
            Message::Mixer(MixerMessage::SetSendGain(RoutingStrip::Track(0), 0, gain))
                if (gain + 6.0).abs() > 1.0e-4 && gain.abs() > 1.0e-4
        )
    }));
}

#[test]
fn strip_minimum_height_covers_route_footer() {
    let fixed_controls = super::INSTRUMENT_PICKER_HEIGHT
        + super::STRIP_TOGGLE_SIZE
        + super::STRIP_FOOTER_HEIGHT
        + (ui_style::PADDING_SM as f32 * 2.0);

    assert!(super::STRIP_MIN_HEIGHT > fixed_controls + ui_style::grid_f32(16));
    assert!(
        super::MIXER_MIN_HEIGHT
            >= super::STRIP_MIN_HEIGHT
                + super::SECTION_HEADER_HEIGHT
                + (ui_style::PADDING_SM as f32 * 2.0)
    );
}

#[test]
fn strip_minimum_height_covers_fader_controls_and_route_footer() {
    let pan_stack_height = ui_style::SPACE_XS as f32
        + super::VALUE_LABEL_HEIGHT
        + 40.0
        + (super::LABEL_CONTROL_SPACING * 2.0);
    let gain_stack_height = super::control_stack_height(96.0);
    let content_height = super::INSTRUMENT_PICKER_HEIGHT
        + pan_stack_height
        + gain_stack_height
        + super::STRIP_TOGGLE_SIZE
        + (super::STRIP_STACK_SPACING * 3.0);
    let required_strip_height =
        content_height + super::STRIP_FOOTER_HEIGHT + (ui_style::PADDING_SM as f32 * 2.0);

    assert!(super::STRIP_MIN_HEIGHT >= required_strip_height);
}

#[test]
fn output_picker_bottom_inset_matches_instrument_top_inset() {
    crate::test_assertions::assert_float_eq!(super::ROUTE_PICKER_BOTTOM_INSET, 0.0);
}

#[test]
fn route_picker_closed_button_hides_native_left_label_for_centered_overlay() {
    let style = super::route_pick_list_centered_style(
        &Theme::Dark,
        iced::widget::pick_list::Status::Active,
    );

    assert_eq!(style.text_color, Color::TRANSPARENT);
    assert_eq!(style.placeholder_color, Color::TRANSPARENT);
}

#[test]
fn fader_height_accounts_for_route_footer() {
    let strip_height = 360.0;
    let expected = (strip_height
        - (ui_style::PADDING_SM as f32 * 2.0)
        - super::SECTION_HEADER_HEIGHT
        - super::INSTRUMENT_PICKER_HEIGHT
        - super::STRIP_TOGGLE_SIZE
        - super::STRIP_FOOTER_HEIGHT
        - 30.0
        - (super::VALUE_LABEL_HEIGHT * 3.0)
        - (ui_style::SPACE_XS as f32 * 6.0))
        .max(96.0);

    crate::test_assertions::assert_float_eq!(
        super::gain_control_height(strip_height, super::GainControlMode::Fader),
        expected
    );
}

#[test]
fn route_choices_follow_added_removed_buses_and_exclude_source_bus() {
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let (verb, delay) = {
        let mut mixer = playback.mixer();
        let verb = mixer.add_bus("Verb").expect("bus should be added");
        let delay = mixer.add_bus("Delay").expect("bus should be added");
        (verb, delay)
    };
    let mixer = playback.mixer_state().clone();

    let track_choices = super::route_choices(&mixer, RoutingStrip::Track(0));
    assert_eq!(
        track_choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Master", "Verb", "Delay"]
    );

    let bus_choices = super::route_choices(&mixer, RoutingStrip::Bus(verb.0));
    assert!(
        !bus_choices
            .iter()
            .any(|choice| choice.route == TrackRoute::Bus(verb))
    );
    assert!(
        bus_choices
            .iter()
            .any(|choice| choice.route == TrackRoute::Bus(delay))
    );

    playback
        .mixer()
        .remove_bus(delay)
        .expect("bus removal should succeed");
    let mixer = playback.mixer_state().clone();
    let track_choices = super::route_choices(&mixer, RoutingStrip::Track(0));
    assert!(
        !track_choices
            .iter()
            .any(|choice| choice.route == TrackRoute::Bus(delay))
    );
}

#[test]
fn send_destination_choices_exclude_source_bus() {
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let (verb, delay) = {
        let mut mixer = playback.mixer();
        let verb = mixer.add_bus("Verb").expect("bus should be added");
        let delay = mixer.add_bus("Delay").expect("bus should be added");
        (verb, delay)
    };
    let mixer = playback.mixer_state().clone();

    let choices = super::send_destination_choices(&mixer, RoutingStrip::Bus(verb.0));

    assert_eq!(
        choices
            .iter()
            .filter_map(|choice| match choice.action {
                super::SendDestinationAction::Route(bus_id) => Some(bus_id),
                super::SendDestinationAction::Remove => None,
            })
            .collect::<Vec<_>>(),
        vec![delay.0]
    );
}

#[test]
fn route_and_send_choices_exclude_feedback_destinations() {
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let (verb, delay) = {
        let mut mixer = playback.mixer();
        let verb = mixer.add_bus("Verb").expect("bus should be added");
        let delay = mixer.add_bus("Delay").expect("bus should be added");
        mixer
            .set_bus_route(verb, TrackRoute::Bus(delay))
            .expect("forward route should be allowed");
        (verb, delay)
    };
    let mixer = playback.mixer_state().clone();

    let delay_route_choices = super::route_choices(&mixer, RoutingStrip::Bus(delay.0));
    assert!(
        !delay_route_choices
            .iter()
            .any(|choice| choice.route == TrackRoute::Bus(verb))
    );

    let delay_send_choices = super::send_destination_choices(&mixer, RoutingStrip::Bus(delay.0));
    assert!(!delay_send_choices.iter().any(|choice| {
        matches!(choice.action, super::SendDestinationAction::Route(bus_id) if bus_id == verb.0)
    }));
}

#[test]
fn send_menu_adds_remove_action_before_bus_choices() {
    let choices = super::send_menu_choices(vec![super::SendDestinationChoice {
        action: super::SendDestinationAction::Route(7),
        label: "Verb".to_string(),
    }]);

    assert!(matches!(
        choices[0].action,
        super::SendDestinationAction::Remove
    ));
    assert!(matches!(
        choices[1].action,
        super::SendDestinationAction::Route(7)
    ));
}
