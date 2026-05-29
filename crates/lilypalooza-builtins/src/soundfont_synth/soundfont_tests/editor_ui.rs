use super::*;

#[test]
fn soundfont_controller_exposes_editor_session() {
    let resource = test_soundfont_resource();
    let loaded = LoadedSoundfont::load(&resource).expect("test SoundFont should load");
    let mut soundfonts = std::collections::HashMap::new();
    soundfonts.insert(resource.id.clone(), loaded);
    let resources = vec![resource];
    let slot = SlotState::built_in(
        BUILTIN_SOUNDFONT_ID,
        encode_state(&SoundfontProcessorState::default()),
    );
    let context = InstrumentRuntimeContext {
        soundfonts: &soundfonts,
        soundfont_resources: &resources,
        soundfont_settings: SoundfontSynthSettings::new(44_100, 64),
    };

    let runtime = create_runtime(&slot, &context)
        .expect("runtime should build")
        .expect("soundfont runtime should exist");
    let controller = runtime.binding.controller();

    assert!(controller.descriptor().editor.is_some());
    assert!(
        controller
            .create_editor_session()
            .expect("editor creation should succeed")
            .is_some()
    );
}

#[test]
fn soundfont_editor_is_not_resizable() {
    assert_eq!(
        super::descriptor().editor.map(|editor| editor.resizable),
        Some(false)
    );
}

#[test]
fn soundfont_editor_uses_retro_fixed_size() {
    let editor = super::descriptor()
        .editor
        .expect("soundfont editor descriptor should exist");

    assert_eq!(
        editor.default_size,
        EditorSize {
            width: super::EDITOR_WIDTH,
            height: super::EDITOR_HEIGHT,
        }
    );
    assert_eq!(editor.min_size, Some(editor.default_size));
}

#[test]
fn soundfont_descriptor_is_named_sf_01() {
    assert_eq!(super::descriptor().name, "SF-01");
}

#[test]
fn soundfont_editor_bundles_retro_fonts() {
    assert!(!include_bytes!("../../../assets/fonts/W95FA.otf").is_empty());
    assert!(!include_bytes!("../../../assets/fonts/CozetteVector.ttf").is_empty());
}

#[test]
fn soundfont_program_list_uses_same_background_as_select_box() {
    let mut first = 0;
    let mut scroll_remainder = 0.0;
    let shapes = render_test_ui(|ui| {
        let programs = vec![ProgramChoice {
            program: 0,
            label: "000 Piano".to_string(),
        }];
        super::program_list(
            ui,
            super::rect(0.0, 0.0, 160.0, 96.0),
            &programs,
            0,
            &mut first,
            &mut scroll_remainder,
        );
    });

    assert!(
        shapes.iter().any(|shape| {
            matches!(
                shape,
                super::egui::Shape::Rect(rect)
                    if rect.rect == super::rect(0.0, 0.0, 160.0, 96.0)
                        && rect.fill == super::retro::FIELD
            )
        }),
        "program list background should match select-box background"
    );
}

#[test]
fn soundfont_program_list_item_text_is_vertically_centered() {
    let mut first = 0;
    let mut scroll_remainder = 0.0;
    let shapes = render_test_ui(|ui| {
        let programs = vec![ProgramChoice {
            program: 0,
            label: "000 Piano".to_string(),
        }];
        super::program_list(
            ui,
            super::rect(0.0, 0.0, 160.0, 96.0),
            &programs,
            0,
            &mut first,
            &mut scroll_remainder,
        );
    });
    let row = super::egui::Rect::from_min_size(
        super::egui::pos2(4.0, 6.0),
        super::egui::vec2(128.0, 24.0),
    );

    let text = shapes
        .iter()
        .find_map(|shape| match shape {
            super::egui::Shape::Text(text) if text.galley.text().contains("000 Piano") => {
                Some(text)
            }
            _ => None,
        })
        .expect("program row text should be painted");
    let text_center = text.pos.y + text.galley.size().y / 2.0;

    assert!(
        (text_center - row.center().y).abs() <= 1.0,
        "program row text center {text_center} should match row center {}",
        row.center().y
    );
}

#[test]
fn soundfont_select_box_text_is_vertically_centered() {
    let shapes = render_test_ui(|ui| {
        super::retro_select_box(
            ui,
            super::rect(0.0, 0.0, 180.0, 30.0),
            "soundfont-test",
            "FluidR3",
        );
    });
    let text = text_shape(&shapes, "FluidR3");
    let center = text.pos.y + text.galley.size().y / 2.0;

    assert!((center - 15.0).abs() <= 1.0);
}

#[test]
fn soundfont_dropdown_rows_match_select_box_and_center_text() {
    let shapes = render_test_ui(|ui| {
        super::retro_choice_list(
            ui,
            super::rect(0.0, 0.0, 180.0, 60.0),
            &["FluidR3".to_string()],
            0,
            "dropdown-test",
        );
    });

    assert!(
        shapes.iter().any(|shape| {
            matches!(
                shape,
                super::egui::Shape::Rect(rect)
                    if rect.rect == super::rect(0.0, 0.0, 180.0, 60.0)
                        && rect.fill == super::retro::FIELD
            )
        }),
        "dropdown background should match select-box background"
    );

    let text = text_shape(&shapes, "FluidR3");
    let center = text.pos.y + text.galley.size().y / 2.0;
    assert!((center - 17.0).abs() <= 1.0);
}

#[test]
fn soundfont_program_list_scrolls_with_mouse_wheel() {
    let ctx = super::egui::Context::default();
    super::install_retro_style(&ctx);
    let programs = (0..8)
        .map(|program| ProgramChoice {
            program,
            label: format!("{program:03} Program"),
        })
        .collect::<Vec<_>>();
    let mut first = 0;

    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![super::egui::Event::PointerMoved(super::egui::pos2(
            20.0, 20.0,
        ))],
    );
    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![
            super::egui::Event::PointerMoved(super::egui::pos2(20.0, 20.0)),
            super::egui::Event::MouseWheel {
                unit: super::egui::MouseWheelUnit::Point,
                delta: super::egui::vec2(0.0, -48.0),
                modifiers: super::egui::Modifiers::default(),
                phase: super::egui::TouchPhase::Move,
            },
        ],
    );

    assert!(
        first > 0,
        "mouse wheel should advance the visible program window"
    );
}

#[test]
fn soundfont_program_list_ignores_tiny_wheel_deltas() {
    let ctx = super::egui::Context::default();
    super::install_retro_style(&ctx);
    let programs = (0..8)
        .map(|program| ProgramChoice {
            program,
            label: format!("{program:03} Program"),
        })
        .collect::<Vec<_>>();
    let mut first = 0;

    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![super::egui::Event::PointerMoved(super::egui::pos2(
            20.0, 20.0,
        ))],
    );
    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![
            super::egui::Event::PointerMoved(super::egui::pos2(20.0, 20.0)),
            super::egui::Event::MouseWheel {
                unit: super::egui::MouseWheelUnit::Point,
                delta: super::egui::vec2(0.0, -4.0),
                modifiers: super::egui::Modifiers::default(),
                phase: super::egui::TouchPhase::Move,
            },
        ],
    );

    assert_eq!(first, 0, "tiny wheel deltas should not skip a row");
}

#[test]
fn soundfont_program_list_thumb_drag_scrolls() {
    let ctx = super::egui::Context::default();
    super::install_retro_style(&ctx);
    let programs = (0..12)
        .map(|program| ProgramChoice {
            program,
            label: format!("{program:03} Program"),
        })
        .collect::<Vec<_>>();
    let mut first = 0;

    render_program_list_frame(&ctx, &programs, &mut first, vec![]);
    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![
            super::egui::Event::PointerMoved(super::egui::pos2(147.0, 37.0)),
            super::egui::Event::PointerButton {
                pos: super::egui::pos2(147.0, 37.0),
                button: super::egui::PointerButton::Primary,
                pressed: true,
                modifiers: super::egui::Modifiers::default(),
            },
            super::egui::Event::PointerMoved(super::egui::pos2(147.0, 74.0)),
        ],
    );
    render_program_list_frame(
        &ctx,
        &programs,
        &mut first,
        vec![super::egui::Event::PointerButton {
            pos: super::egui::pos2(147.0, 74.0),
            button: super::egui::PointerButton::Primary,
            pressed: false,
            modifiers: super::egui::Modifiers::default(),
        }],
    );

    assert!(first > 0, "dragging the scrollbar thumb should scroll");
}

#[test]
fn soundfont_editor_program_list_shows_at_least_three_items() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_presets(&loaded, &["first"], 6);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let visible_programs = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .filter_map(|shape| match shape {
            super::egui::Shape::Text(text)
                if text.galley.text().contains("Program")
                    && text.visual_bounding_rect().top() >= 235.0 =>
            {
                Some(text.galley.text().to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        visible_programs.len() >= 3,
        "program list should show at least three rows, got {visible_programs:?}"
    );
}

#[test]
fn soundfont_midi_indicator_is_round_led() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let has_led_circle = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .any(|shape| match shape {
            super::egui::Shape::Circle(circle) => {
                circle.center.distance(super::egui::pos2(634.0, 162.0)) < 2.0
            }
            _ => false,
        });

    assert!(has_led_circle, "MIDI IN indicator should be a circle");
}

#[test]
fn soundfont_midi_indicator_has_no_white_circle_highlight() {
    let shapes = render_test_ui(|ui| {
        super::draw_led(ui, super::pos(24.0, 24.0), true, super::retro::GREEN);
    });

    let has_white_led_stroke = shapes.iter().any(|shape| match shape {
        super::egui::Shape::Circle(circle) => {
            circle.center.distance(super::egui::pos2(24.0, 24.0)) < 4.0
                && circle.stroke.color == super::retro::HILITE
        }
        _ => false,
    });

    assert!(
        !has_white_led_stroke,
        "MIDI IN LED should not draw a white circular highlight"
    );
}

#[test]
fn soundfont_group_label_background_tracks_label_width() {
    let shapes = render_test_ui(|ui| {
        super::retro_group(ui, super::rect(0.0, 20.0, 196.0, 80.0), "MIDI", |_| {});
    });
    let label_cover = shapes
        .iter()
        .find_map(|shape| match shape {
            super::egui::Shape::Rect(rect)
                if rect.fill == super::retro::FACE
                    && rect.rect.top() < 24.0
                    && rect.rect.left() > 0.0 =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .expect("group label should paint an opaque caption background");

    assert!(
        label_cover.width() < 80.0,
        "caption background should be sized to the label, got {}",
        label_cover.width()
    );
}

#[test]
fn soundfont_slider_value_labels_stay_inside_group_bounds() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let shapes = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .collect::<Vec<_>>();
    let slider_value_bounds = shapes
        .iter()
        .filter_map(|shape| match shape {
            super::egui::Shape::Text(text)
                if text.galley.text().ends_with('%') || text.galley.text().ends_with("dB") =>
            {
                Some(text.visual_bounding_rect())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(!slider_value_bounds.is_empty());
    for bounds in slider_value_bounds {
        assert!(
            bounds.left() >= 424.0 && bounds.right() <= 800.0 && bounds.bottom() <= 438.0,
            "slider value label should fit the editor groups: {bounds:?}"
        );
    }
}

#[test]
fn soundfont_slider_groups_leave_bottom_padding_for_thumbs() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let shapes = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .collect::<Vec<_>>();
    let slider_groups = shapes
        .iter()
        .filter_map(|shape| match shape {
            super::egui::Shape::Rect(rect)
                if rect.fill == super::retro::FACE
                    && (rect.rect.width() - 376.0).abs() < 0.1
                    && near_f32(rect.rect.left(), 424.0)
                    && rect.rect.top() >= 258.0 =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(slider_groups.len(), 3);
    for group in slider_groups {
        let thumb = shapes
            .iter()
            .find_map(|shape| match shape {
                super::egui::Shape::Rect(rect)
                    if rect.fill == super::retro::FACE
                        && (rect.rect.width() - 18.0).abs() < 0.1
                        && (rect.rect.height() - 26.0).abs() < 0.1
                        && group.contains_rect(rect.rect) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("slider group should contain a thumb");
        let bottom_padding = group.bottom() - thumb.bottom();
        assert!(
            bottom_padding >= 8.0,
            "slider thumb needs bottom padding, got {bottom_padding} in {group:?}"
        );
    }
}

#[test]
fn soundfont_slider_groups_have_vertical_spacing() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let mut slider_groups = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .filter_map(|shape| match shape {
            super::egui::Shape::Rect(rect)
                if rect.fill == super::retro::FACE
                    && (rect.rect.width() - 376.0).abs() < 0.1
                    && near_f32(rect.rect.left(), 424.0)
                    && rect.rect.top() >= 258.0 =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    slider_groups.sort_by(|a, b| a.top().total_cmp(&b.top()));
    assert_eq!(slider_groups.len(), 3);
    for pair in slider_groups.windows(2) {
        let gap = pair[1].top() - pair[0].bottom();
        assert!(
            gap >= 10.0,
            "slider groups need more vertical gap, got {gap}"
        );
    }
}

#[test]
fn soundfont_output_gain_db_is_in_group_label_not_value() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let texts = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .filter_map(|shape| match shape {
            super::egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(texts.iter().any(|text| text == "OUTPUT GAIN (DB)"));
    assert!(
        texts.iter().all(|text| text != "0.0 dB"),
        "output gain value should omit dB suffix"
    );
}

#[test]
fn soundfont_header_says_soundfont_rompler_once() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let texts = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .filter_map(|shape| match shape {
            super::egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(texts.iter().any(|text| text == "SF-01  SOUNDFONT ROMPLER"));
    assert!(
        texts.iter().all(|text| text != "ROMPLER"),
        "header should not paint a separate ROMPLER label"
    );
}

#[test]
fn soundfont_header_text_is_vertically_centered() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let header = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .find_map(|shape| match shape {
            super::egui::Shape::Text(text) if text.galley.text() == "SF-01  SOUNDFONT ROMPLER" => {
                Some(text.visual_bounding_rect())
            }
            _ => None,
        })
        .expect("header text should be painted");

    assert!(
        (header.center().y - 25.0).abs() <= 1.0,
        "header text center should match title bar center: {header:?}"
    );
}

#[test]
fn soundfont_slider_double_click_resets_values() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();
    let mut state = app.shared.state.snapshot().0;
    state.reverb_wet = 0.7;
    state.chorus_wet = 0.6;
    state.output_gain = super::output_gain_from_db(12.0);
    app.shared
        .apply_state(state)
        .expect("test state should apply");

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    double_click_editor(&ctx, &mut app, super::egui::pos2(600.0, 293.0));
    double_click_editor(&ctx, &mut app, super::egui::pos2(600.0, 359.0));
    double_click_editor(&ctx, &mut app, super::egui::pos2(600.0, 425.0));

    let (state, _) = app.shared.state.snapshot();
    assert_f32_close(state.reverb_wet, 0.0);
    assert_f32_close(state.chorus_wet, 0.0);
    assert!((super::output_gain_to_db(state.output_gain) - 0.0).abs() < 1.0e-6);
}

#[test]
fn soundfont_number_field_can_remain_empty_while_editing() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();
    let bank_field = super::egui::pos2(488.0, 111.0);

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    click_editor(&ctx, &mut app, bank_field);
    let _ = ctx.run_ui(
        editor_input(vec![super::egui::Event::Key {
            key: super::egui::Key::Backspace,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: super::egui::Modifiers::default(),
        }]),
        |ui| app.update(ui),
    );
    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));

    assert_eq!(
        app.bank_text, "",
        "empty number field text should not be repopulated while focused"
    );
}

#[test]
fn soundfont_top_row_controls_are_centered_in_groups() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let shapes = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .collect::<Vec<_>>();
    let select_box = shapes
        .iter()
        .find_map(|shape| match shape {
            super::egui::Shape::Rect(rect)
                if near_f32(rect.rect.width(), 350.0) && near_f32(rect.rect.height(), 30.0) =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .expect("soundfont select box should be painted");

    assert_f32_close(select_box.center().y, 111.0);
}

#[test]
fn soundfont_chooser_click_opens_choices_without_changing_selection() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first", "second"]);
    let ctx = super::egui::Context::default();
    let chooser_pos = super::egui::pos2(210.0, 121.0);

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let _ = ctx.run_ui(
        editor_input(vec![
            super::egui::Event::PointerMoved(chooser_pos),
            super::egui::Event::PointerButton {
                pos: chooser_pos,
                button: super::egui::PointerButton::Primary,
                pressed: true,
                modifiers: super::egui::Modifiers::default(),
            },
        ]),
        |ui| app.update(ui),
    );
    let _ = ctx.run_ui(
        editor_input(vec![
            super::egui::Event::PointerMoved(chooser_pos),
            super::egui::Event::PointerButton {
                pos: chooser_pos,
                button: super::egui::PointerButton::Primary,
                pressed: false,
                modifiers: super::egui::Modifiers::default(),
            },
        ]),
        |ui| app.update(ui),
    );

    let (state, _) = app.shared.state.snapshot();
    assert_eq!(
        state.soundfont_id, "first",
        "clicking the chooser should open choices, not cycle selection"
    );
}

#[test]
fn soundfont_chooser_item_click_selects_choice() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first", "second"]);
    let ctx = super::egui::Context::default();
    let chooser_pos = super::egui::pos2(210.0, 121.0);
    let second_choice_pos = super::egui::pos2(210.0, 179.0);

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    click_editor(&ctx, &mut app, chooser_pos);
    assert!(
        app.soundfont_dropdown_open,
        "clicking the chooser should open the dropdown"
    );
    click_editor(&ctx, &mut app, second_choice_pos);

    let (state, _) = app.shared.state.snapshot();
    assert_eq!(state.soundfont_id, "second");
}

#[test]
fn soundfont_midi_panel_labels_input_without_idle_state_text() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut app = editor_app_with_soundfonts(&loaded, &["first"]);
    let ctx = super::egui::Context::default();

    let _ = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let output = ctx.run_ui(editor_input(vec![]), |ui| app.update(ui));
    let text = output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .filter_map(|shape| match shape {
            super::egui::Shape::Text(text) => Some(text.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        text.iter().any(|text| text == "MIDI IN"),
        "MIDI panel should have a stable input label"
    );
    assert!(
        text.iter().all(|text| text != "IDLE"),
        "MIDI panel should not label no activity as IDLE"
    );
}
