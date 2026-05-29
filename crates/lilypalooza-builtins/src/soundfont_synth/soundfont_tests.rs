#![expect(
    clippy::let_underscore_must_use,
    reason = "egui editor tests intentionally discard FullOutput"
)]

use std::{path::PathBuf, sync::Arc, time::Instant};

use lilypalooza_audio::{
    BUILTIN_SOUNDFONT_ID, SlotState, SoundfontPreset,
    instrument::{EditorSize, InstrumentProcessor, InstrumentRuntimeContext, MidiEvent, Processor},
    soundfont::{LoadedSoundfont, SoundfontResource, SoundfontSynthSettings},
};
use lilypalooza_egui_baseview::EguiApp;

use super::{
    DESCRIPTOR, EDITOR_HEIGHT, EDITOR_WIDTH, ProgramChoice, SoundfontProcessor,
    SoundfontProcessorState, create_runtime, descriptor, egui, encode_state, format_output_gain_db,
    output_gain_from_db, output_gain_to_db,
    retro_ui::{
        draw_led, install_retro_style, pos, program_list, rect, retro, retro_choice_list,
        retro_group, retro_select_box,
    },
    soundfont_presets,
};

fn assert_f32_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

fn near_f32(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= 0.1
}

fn test_soundfont_resource() -> SoundfontResource {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/soundfonts/lilypalooza-test.sf2")
        .canonicalize()
        .expect("test SoundFont should exist");
    SoundfontResource {
        id: "default".to_string(),
        name: "FluidR3".to_string(),
        path,
    }
}

fn test_soundfont_processor() -> SoundfontProcessor {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize")
}

fn assert_processor_renders_after_note_on(channel: u8, reset_first: bool, failure: &str) {
    let mut processor = test_soundfont_processor();
    if reset_first {
        processor.reset();
    }
    processor.handle_midi(MidiEvent::NoteOn {
        channel,
        note: 60,
        velocity: 100,
    });

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    for _ in 0..8 {
        processor.render(&mut left, &mut right);
        if left
            .iter()
            .chain(right.iter())
            .any(|sample| sample.abs() > 1.0e-6)
        {
            return;
        }
    }

    panic!("{failure}");
}

mod editor_ui;
mod processor;

fn render_test_ui(mut add_contents: impl FnMut(&mut super::egui::Ui)) -> Vec<super::egui::Shape> {
    let ctx = super::egui::Context::default();
    install_retro_style(&ctx);
    let output = ctx.run_ui(
        super::egui::RawInput {
            screen_rect: Some(rect(0.0, 0.0, 240.0, 180.0)),
            ..super::egui::RawInput::default()
        },
        |ui| add_contents(ui),
    );

    output
        .shapes
        .into_iter()
        .flat_map(|shape| flatten_shape(shape.shape))
        .collect()
}

fn render_program_list_frame(
    ctx: &super::egui::Context,
    programs: &[ProgramChoice],
    first: &mut usize,
    events: Vec<super::egui::Event>,
) {
    let mut scroll_remainder = 0.0;
    let _ = ctx.run_ui(
        super::egui::RawInput {
            screen_rect: Some(rect(0.0, 0.0, 240.0, 180.0)),
            events,
            ..super::egui::RawInput::default()
        },
        |ui| {
            program_list(
                ui,
                rect(0.0, 0.0, 160.0, 96.0),
                programs,
                0,
                first,
                &mut scroll_remainder,
            );
        },
    );
}

fn editor_input(events: Vec<super::egui::Event>) -> super::egui::RawInput {
    super::egui::RawInput {
        screen_rect: Some(rect(
            0.0,
            0.0,
            super::EDITOR_WIDTH as f32,
            super::EDITOR_HEIGHT as f32,
        )),
        events,
        ..super::egui::RawInput::default()
    }
}

fn click_editor(
    ctx: &super::egui::Context,
    app: &mut super::SoundfontEditorApp,
    pos: super::egui::Pos2,
) {
    let _ = ctx.run_ui(
        editor_input(vec![
            super::egui::Event::PointerMoved(pos),
            super::egui::Event::PointerButton {
                pos,
                button: super::egui::PointerButton::Primary,
                pressed: true,
                modifiers: super::egui::Modifiers::default(),
            },
        ]),
        |ui| app.update(ui),
    );
    let _ = ctx.run_ui(
        editor_input(vec![
            super::egui::Event::PointerMoved(pos),
            super::egui::Event::PointerButton {
                pos,
                button: super::egui::PointerButton::Primary,
                pressed: false,
                modifiers: super::egui::Modifiers::default(),
            },
        ]),
        |ui| app.update(ui),
    );
}

fn double_click_editor(
    ctx: &super::egui::Context,
    app: &mut super::SoundfontEditorApp,
    pos: super::egui::Pos2,
) {
    for (time, pressed) in [(1.00, true), (1.01, false), (1.08, true), (1.09, false)] {
        let mut input = editor_input(vec![
            super::egui::Event::PointerMoved(pos),
            super::egui::Event::PointerButton {
                pos,
                button: super::egui::PointerButton::Primary,
                pressed,
                modifiers: super::egui::Modifiers::default(),
            },
        ]);
        input.time = Some(time);
        let _ = ctx.run_ui(input, |ui| app.update(ui));
    }
}

fn editor_app_with_soundfonts(loaded: &LoadedSoundfont, ids: &[&str]) -> super::SoundfontEditorApp {
    editor_app_with_presets(loaded, ids, 1)
}

fn editor_app_with_presets(
    loaded: &LoadedSoundfont,
    ids: &[&str],
    preset_count: u8,
) -> super::SoundfontEditorApp {
    let catalog = ids
        .iter()
        .map(|id| super::SoundfontCatalogEntry {
            id: (*id).to_string(),
            name: (*id).to_string(),
            presets: Arc::new(
                (0..preset_count)
                    .map(|program| SoundfontPreset {
                        bank: 0,
                        program,
                        name: format!("Program {program}"),
                    })
                    .collect(),
            ),
        })
        .collect::<Vec<_>>();
    let available = ids
        .iter()
        .map(|id| ((*id).to_string(), Arc::clone(&loaded.soundfont)))
        .collect::<std::collections::HashMap<_, _>>();
    let state = SoundfontProcessorState {
        soundfont_id: ids[0].to_string(),
        ..SoundfontProcessorState::default()
    };
    super::SoundfontEditorApp {
        shared: Arc::new(super::SharedSoundfontBinding {
            catalog: Arc::new(catalog),
            available_soundfonts: Arc::new(available),
            state: super::SharedSoundfontState::new(&state, Arc::clone(&loaded.soundfont)),
        }),
        retro_style_installed: false,
        bank_text: String::new(),
        bank_text_focused: false,
        polyphony_text: String::new(),
        polyphony_text_focused: false,
        program_scroll_first: 0,
        program_scroll_remainder: 0.0,
        soundfont_dropdown_open: false,
        seen_midi_activity: 0,
        midi_flash_frames: 0,
    }
}

fn flatten_shape(shape: super::egui::Shape) -> Vec<super::egui::Shape> {
    match shape {
        super::egui::Shape::Vec(shapes) => shapes.into_iter().flat_map(flatten_shape).collect(),
        shape => vec![shape],
    }
}

fn text_shape<'a>(
    shapes: &'a [super::egui::Shape],
    expected: &str,
) -> &'a super::egui::epaint::TextShape {
    shapes
        .iter()
        .find_map(|shape| match shape {
            super::egui::Shape::Text(text) if text.galley.text().contains(expected) => Some(text),
            _ => None,
        })
        .expect("expected text should be painted")
}
