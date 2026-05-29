use super::*;

#[test]
fn soundfont_processor_renders_after_note_on() {
    assert_processor_renders_after_note_on(
        0,
        false,
        "soundfont processor produced silence after note on",
    );
}

#[test]
fn soundfont_presets_are_sorted_by_bank_program_and_name() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let presets = super::soundfont_presets(&loaded.soundfont);

    assert!(presets.windows(2).all(|pair| {
        let left = &pair[0];
        let right = &pair[1];
        (left.bank, left.program, left.name.as_str())
            <= (right.bank, right.program, right.name.as_str())
    }));
}

#[test]
fn soundfont_processor_stays_silent_without_note_on() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    let mut left = vec![1.0; 64];
    let mut right = vec![1.0; 64];
    for _ in 0..8 {
        processor.render(&mut left, &mut right);
        assert!(
            left.iter()
                .chain(right.iter())
                .all(|sample| sample.abs() <= 1.0e-6),
            "soundfont processor should stay silent before any note on"
        );
        left.fill(1.0);
        right.fill(1.0);
    }
}

#[test]
fn soundfont_processor_renders_after_note_on_on_nonzero_channel() {
    assert_processor_renders_after_note_on(
        3,
        false,
        "soundfont processor produced silence after nonzero-channel note on",
    );
}

#[test]
fn soundfont_processor_reset_silences_active_note() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    processor.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    for _ in 0..8 {
        processor.render(&mut left, &mut right);
    }

    processor.reset();
    left.fill(0.0);
    right.fill(0.0);
    for _ in 0..8 {
        processor.render(&mut left, &mut right);
    }

    assert!(
        left.iter()
            .chain(right.iter())
            .all(|sample| sample.abs() <= 1.0e-6),
        "soundfont processor reset should silence active notes"
    );
}

#[test]
fn soundfont_processor_renders_after_reset_then_note_on() {
    assert_processor_renders_after_note_on(
        0,
        true,
        "soundfont processor produced silence after reset then note on",
    );
}

#[test]
fn soundfont_processor_ignores_midi_program_override() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let settings = SoundfontSynthSettings::new(44_100, 64);
    let mut selected_program = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");
    let mut overridden_program = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");

    overridden_program.handle_midi(MidiEvent::ProgramChange {
        channel: 0,
        program: 0,
    });
    overridden_program.handle_midi(MidiEvent::ControlChange {
        channel: 0,
        controller: 0,
        value: 0,
    });
    overridden_program.handle_midi(MidiEvent::ControlChange {
        channel: 0,
        controller: 32,
        value: 0,
    });

    selected_program.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    overridden_program.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut selected_left = vec![0.0; 512];
    let mut selected_right = vec![0.0; 512];
    let mut overridden_left = vec![0.0; 512];
    let mut overridden_right = vec![0.0; 512];

    for _ in 0..8 {
        selected_program.render(&mut selected_left, &mut selected_right);
        overridden_program.render(&mut overridden_left, &mut overridden_right);
    }

    assert_eq!(selected_left, overridden_left);
    assert_eq!(selected_right, overridden_right);
}

#[test]
fn soundfont_processor_follows_midi_program_and_bank_when_enabled() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let settings = SoundfontSynthSettings::new(44_100, 64);
    let mut selected_program = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            follow_midi: true,
            maximum_polyphony: 64,
            output_gain: 0.5,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");
    let mut overridden_program = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            follow_midi: true,
            maximum_polyphony: 64,
            output_gain: 0.5,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");

    overridden_program.handle_midi(MidiEvent::ControlChange {
        channel: 0,
        controller: 0,
        value: 0,
    });
    overridden_program.handle_midi(MidiEvent::ProgramChange {
        channel: 0,
        program: 0,
    });

    selected_program.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    overridden_program.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut selected_left = vec![0.0; 512];
    let mut selected_right = vec![0.0; 512];
    let mut overridden_left = vec![0.0; 512];
    let mut overridden_right = vec![0.0; 512];

    for _ in 0..8 {
        selected_program.render(&mut selected_left, &mut selected_right);
        overridden_program.render(&mut overridden_left, &mut overridden_right);
    }

    assert!(
        selected_left
            .iter()
            .zip(overridden_left.iter())
            .chain(selected_right.iter().zip(overridden_right.iter()))
            .any(|(a, b)| (a - b).abs() > 1.0e-6),
        "midi-selected program should change the rendered signal when follow_midi is enabled"
    );
}

#[test]
fn soundfont_wet_params_roundtrip_and_enable_internal_effects() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState {
            reverb_wet: 0.25,
            chorus_wet: 0.75,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");

    assert!(processor.synthesizer.get_enable_reverb_and_chorus());
    assert_eq!(processor.get_param("reverb_wet"), Some(0.25));
    assert_eq!(processor.get_param("chorus_wet"), Some(0.75));

    assert!(processor.set_param("reverb_wet", 0.5));
    assert!(processor.set_param("chorus_wet", 0.125));
    let decoded =
        SoundfontProcessor::decode_state(&processor.save_state()).expect("state should decode");

    assert_f32_close(decoded.reverb_wet, 0.5);
    assert_f32_close(decoded.chorus_wet, 0.125);
}

#[test]
fn soundfont_output_gain_param_uses_trim_db_scale() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    assert_f32_close(
        super::DESCRIPTOR
            .params
            .iter()
            .find(|param| param.id == "output_gain")
            .expect("output gain param should exist")
            .default,
        2.0 / 3.0,
    );
    assert_f32_close(
        processor
            .get_param("output_gain")
            .expect("output gain param should exist"),
        2.0 / 3.0,
    );

    assert!(processor.set_param("output_gain", 1.0));
    assert_f32_close(
        processor.state.output_gain,
        super::output_gain_from_db(12.0),
    );

    assert!(processor.set_param("output_gain", 0.0));
    assert!((processor.state.output_gain - super::output_gain_from_db(-24.0)).abs() < 1.0e-6);

    assert!(processor.set_param("output_gain", 0.5));
    let expected = super::output_gain_from_db(-6.0);
    assert!((processor.state.output_gain - expected).abs() < 1.0e-6);
}

#[test]
fn soundfont_output_gain_db_format_shows_signed_gain() {
    assert_eq!(super::format_output_gain_db(12.0), "+12.0 dB");
    assert_eq!(super::format_output_gain_db(-6.0), "-6.0 dB");
    assert_eq!(super::format_output_gain_db(0.0), "0.0 dB");
}

#[test]
fn soundfont_processor_selected_program_changes_rendered_signal() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let settings = SoundfontSynthSettings::new(44_100, 64);
    let mut piano = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 0,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");
    let mut violin = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");

    piano.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    violin.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut piano_left = vec![0.0; 512];
    let mut piano_right = vec![0.0; 512];
    let mut violin_left = vec![0.0; 512];
    let mut violin_right = vec![0.0; 512];

    for _ in 0..8 {
        piano.render(&mut piano_left, &mut piano_right);
        violin.render(&mut violin_left, &mut violin_right);
    }

    assert!(
        piano_left
            .iter()
            .zip(violin_left.iter())
            .chain(piano_right.iter().zip(violin_right.iter()))
            .any(|(a, b)| (a - b).abs() > 1.0e-6),
        "different selected SoundFont programs rendered the same signal"
    );
}

#[test]
fn soundfont_processor_reset_preserves_selected_program() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let settings = SoundfontSynthSettings::new(44_100, 64);
    let mut violin = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 40,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");
    let mut piano = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 0,
            ..SoundfontProcessorState::default()
        },
    )
    .expect("processor should initialize");

    violin.reset();
    violin.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    piano.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut violin_left = vec![0.0; 512];
    let mut violin_right = vec![0.0; 512];
    let mut piano_left = vec![0.0; 512];
    let mut piano_right = vec![0.0; 512];

    for _ in 0..8 {
        violin.render(&mut violin_left, &mut violin_right);
        piano.render(&mut piano_left, &mut piano_right);
    }

    assert!(
        violin_left
            .iter()
            .zip(piano_left.iter())
            .chain(violin_right.iter().zip(piano_right.iter()))
            .any(|(a, b)| (a - b).abs() > 1.0e-6),
        "reset restored the SoundFont processor to the default piano program"
    );
}

#[test]
fn soundfont_processor_reset_restores_silent_fast_path() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    processor.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    processor.render(&mut left, &mut right);

    processor.reset();
    left.fill(1.0);
    right.fill(1.0);
    processor.render(&mut left, &mut right);
    assert!(
        left.iter()
            .chain(right.iter())
            .all(|sample| sample.abs() <= 1.0e-6),
        "soundfont processor reset should restore the silent fast path"
    );
}

#[test]
fn soundfont_processor_returns_to_silent_fast_path_after_release_tail() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    processor.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    processor.handle_midi(MidiEvent::NoteOff {
        channel: 0,
        note: 60,
        velocity: 0,
    });

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    for _ in 0..1_024 {
        processor.render(&mut left, &mut right);
        if !processor.needs_render {
            return;
        }
    }

    panic!("soundfont processor never returned to the silent fast path after note release");
}

#[test]
fn soundfont_processor_reports_sleeping_when_dormant() {
    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let mut processor = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        SoundfontSynthSettings::new(44_100, 64),
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");

    assert!(
        processor.is_sleeping(),
        "fresh soundfont processor should start dormant"
    );

    processor.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    assert!(
        !processor.is_sleeping(),
        "note on should wake the processor"
    );

    processor.handle_midi(MidiEvent::AllSoundOff { channel: 0 });
    assert!(
        processor.is_sleeping(),
        "all sound off should return the processor to dormant state"
    );
}

#[test]
#[ignore = "manual perf report"]
fn perf_report_soundfont_processor_block_costs() {
    const BLOCKS: usize = 20_000;
    const BLOCK_SIZE: usize = 64;

    let loaded =
        LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
    let settings = SoundfontSynthSettings::new(44_100, BLOCK_SIZE);

    let mut idle = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");
    let mut armed = SoundfontProcessor::new(
        &Arc::clone(&loaded.soundfont),
        settings,
        SoundfontProcessorState::default(),
    )
    .expect("processor should initialize");
    armed.handle_midi(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });

    let mut idle_left = vec![0.0; BLOCK_SIZE];
    let mut idle_right = vec![0.0; BLOCK_SIZE];
    let idle_started = Instant::now();
    for _ in 0..BLOCKS {
        idle.render(&mut idle_left, &mut idle_right);
    }
    let idle_elapsed = idle_started.elapsed();

    let mut armed_left = vec![0.0; BLOCK_SIZE];
    let mut armed_right = vec![0.0; BLOCK_SIZE];
    let armed_started = Instant::now();
    for _ in 0..BLOCKS {
        armed.render(&mut armed_left, &mut armed_right);
    }
    let armed_elapsed = armed_started.elapsed();

    println!(
        "soundfont processor perf over {BLOCKS} blocks: idle={idle_elapsed:?} \
         armed={armed_elapsed:?}"
    );
}
