use std::{thread, time::Duration};

use knyst::{
    controller::KnystCommands,
    prelude::{Beats, graph_output, handle},
};

use super::{
    BUILTIN_SOUNDFONT_ID, Controller, ControllerError, InstrumentProcessor,
    InstrumentProcessorNode, InstrumentRuntimeHandle, MidiEvent, ParameterDescriptor, Processor,
    ProcessorDescriptor, ProcessorState, ProcessorStateError, SharedInstrumentResetState,
    SlotState, SmoothedAudioValue,
};
use crate::{
    instrument::registry::{self, Entry, RuntimeFactory},
    test_utils::OfflineHarness,
};

fn soundfont_slot(soundfont_id: &str, program: u8) -> SlotState {
    registry::register([Entry::built_in_processor(
        BUILTIN_SOUNDFONT_ID,
        "SoundFont",
        &ProcessorDescriptor {
            name: "SoundFont",
            params: &[],
            editor: None,
        },
        RuntimeFactory::InstrumentDescriptor,
    )]);
    let mut state = vec![program];
    state.extend_from_slice(soundfont_id.as_bytes());
    SlotState::built_in(BUILTIN_SOUNDFONT_ID, ProcessorState(state))
}

#[test]
fn smoothed_audio_value_reaches_target_without_jump() {
    let mut value = SmoothedAudioValue::new(0.0, 1_000);

    value.set_target(1.0);

    let first = value.next_sample();
    assert!(first > 0.0);
    assert!(first < 1.0);
    for _ in 0..20 {
        value.next_sample();
    }
    assert!(value.next_sample() > 0.99);
}

#[test]
fn smoothed_audio_value_continues_from_current_value_when_retargeted() {
    let mut value = SmoothedAudioValue::new(0.0, 1_000);
    value.set_target(1.0);
    let before = value.next_sample();

    value.set_target(0.5);
    let after = value.next_sample();

    assert!((after - before).abs() < 0.1);
    assert!(after > before);
}

#[test]
fn controller_parameters_default_to_owned_descriptor_parameters() {
    struct TestController;

    impl Controller for TestController {
        fn descriptor(&self) -> &'static ProcessorDescriptor {
            static PARAMS: &[ParameterDescriptor] = &[ParameterDescriptor {
                id: "gain",
                name: "Gain",
                default: 0.25,
            }];
            static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
                name: "Controller",
                params: PARAMS,
                editor: None,
            };
            &DESCRIPTOR
        }

        fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
            Err(ControllerError::UnknownParameter(id.to_string()))
        }

        fn set_param(&self, id: &str, _normalized: f32) -> Result<(), ControllerError> {
            Err(ControllerError::UnknownParameter(id.to_string()))
        }

        fn save_state(&self) -> Result<ProcessorState, ControllerError> {
            Ok(ProcessorState::default())
        }

        fn load_state(&self, _state: &ProcessorState) -> Result<(), ControllerError> {
            Ok(())
        }
    }

    let parameters = TestController.parameters();

    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].id, "gain");
    assert_eq!(parameters[0].name, "Gain");
    assert!((parameters[0].default - 0.25).abs() <= f32::EPSILON);
    assert!(parameters[0].automatable);
    assert!(!parameters[0].readonly);
}

struct GateProcessor {
    active: bool,
}

impl Processor for GateProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
            name: "Gate",
            params: &[],
            editor: None,
        };
        &DESCRIPTOR
    }

    fn set_param(&mut self, _id: &str, _normalized: f32) -> bool {
        false
    }

    fn get_param(&self, _id: &str) -> Option<f32> {
        None
    }

    fn save_state(&self) -> ProcessorState {
        ProcessorState::default()
    }

    fn load_state(&mut self, _state: &ProcessorState) -> Result<(), ProcessorStateError> {
        Ok(())
    }

    fn reset(&mut self) {
        self.active = false;
    }
}

impl InstrumentProcessor for GateProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn { .. } => self.active = true,
            MidiEvent::NoteOff { .. } | MidiEvent::AllNotesOff { .. } => self.active = false,
            _ => {}
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let value = if self.active { 1.0 } else { 0.0 };
        left.fill(value);
        right.fill(value);
    }

    fn is_sleeping(&self) -> bool {
        !self.active
    }
}

#[test]
fn reset_then_note_on_in_same_block_produces_signal() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let handle = harness.context().with_activation(|| {
        let reset_state = SharedInstrumentResetState::default();
        let node = handle(InstrumentProcessorNode::new(
            Box::new(GateProcessor { active: true }),
            reset_state.clone(),
        ));
        graph_output(0, node.channels(2));
        InstrumentRuntimeHandle::new(node, reset_state)
    });

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats(1));
    thread::sleep(Duration::from_millis(10));

    handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.01), 1);
    handle.schedule_midi_at_with_offset(
        harness.commands(),
        Beats::from_beats_f64(1.02),
        1,
        MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        },
    );
    harness.commands().transport_play();

    for _ in 0..512 {
        harness.process_block();
        if harness.output_has_signal() {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }

    panic!("reset then note-on in same block should produce signal");
}

#[test]
fn immediate_reset_and_panic_silence_active_node() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let handle = harness.context().with_activation(|| {
        let reset_state = SharedInstrumentResetState::default();
        let node = handle(InstrumentProcessorNode::new(
            Box::new(GateProcessor { active: false }),
            reset_state.clone(),
        ));
        graph_output(0, node.channels(2));
        InstrumentRuntimeHandle::new(node, reset_state)
    });

    harness.commands().transport_play();
    handle.send_midi(
        harness.commands(),
        MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        },
    );

    for _ in 0..256 {
        harness.process_block();
        if harness.output_has_signal() {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(harness.output_has_signal(), "note on should produce signal");

    handle.send_reset(harness.commands(), 1);
    handle.send_midi_immediate(
        harness.commands(),
        1,
        MidiEvent::AllSoundOff { channel: 0 },
        InstrumentRuntimeHandle::immediate_event_delay(1),
    );

    for _ in 0..256 {
        harness.process_block();
        if !harness.output_has_signal() {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }

    panic!("immediate reset and panic should silence active node");
}

#[test]
fn stale_reset_generation_does_not_silence_newer_note() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let handle = harness.context().with_activation(|| {
        let reset_state = SharedInstrumentResetState::default();
        let node = handle(InstrumentProcessorNode::new(
            Box::new(GateProcessor { active: false }),
            reset_state.clone(),
        ));
        graph_output(0, node.channels(2));
        InstrumentRuntimeHandle::new(node, reset_state)
    });

    harness.commands().transport_pause();
    harness
        .commands()
        .transport_seek_to_beats(Beats::from_beats(1));
    thread::sleep(Duration::from_millis(10));

    handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.01), 1);
    handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.02), 2);
    handle.schedule_midi_at_with_offset(
        harness.commands(),
        Beats::from_beats_f64(1.03),
        2,
        MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        },
    );
    handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.04), 1);
    harness.commands().transport_play();

    let mut heard_signal = false;
    let mut signal_after_stale_reset = false;
    for _ in 0..512 {
        harness.process_block();
        let has_signal = harness.output_has_signal();
        if has_signal {
            heard_signal = true;
        }
        let beat = harness
            .commands()
            .current_transport_snapshot()
            .and_then(|snapshot| snapshot.beats)
            .unwrap_or(Beats::ZERO);
        if beat >= Beats::from_beats_f64(1.045) && has_signal {
            signal_after_stale_reset = true;
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    assert!(heard_signal, "newer generation note should produce signal");
    assert!(
        signal_after_stale_reset,
        "stale reset from an older generation must not silence newer playback"
    );
}

#[test]
fn generation_ordering_rejects_stale_reset_after_increment() {
    assert!(super::generation_is_current_or_newer(2, 1));
    assert!(super::generation_is_current_or_newer(2, 2));
    assert!(!super::generation_is_current_or_newer(1, 2));
}

#[test]
fn soundfont_slot_reports_processor_descriptor_and_no_editor_yet() {
    let slot = soundfont_slot("test", 0);

    let descriptor = slot
        .descriptor()
        .expect("soundfont slot should expose processor descriptor");

    assert_eq!(descriptor.name, "SoundFont");
    assert!(descriptor.editor.is_none());
}

#[test]
fn gain_effect_slot_reports_processor_descriptor_and_no_editor_yet() {
    registry::register([Entry::built_in_processor(
        crate::instrument::BUILTIN_GAIN_ID,
        "Gain",
        &ProcessorDescriptor {
            name: "Gain",
            params: &[],
            editor: None,
        },
        RuntimeFactory::Effect(|_, _| Ok(None)),
    )]);
    let slot = crate::instrument::SlotState::built_in(
        crate::instrument::BUILTIN_GAIN_ID,
        crate::instrument::ProcessorState::default(),
    );

    let descriptor = slot
        .descriptor()
        .expect("gain slot should expose processor descriptor");

    assert_eq!(descriptor.name, "Gain");
    assert!(descriptor.editor.is_none());
}

#[test]
fn slot_state_roundtrip_preserves_instance_id() {
    let mut slot = SlotState::built_in(
        crate::instrument::BUILTIN_GAIN_ID,
        crate::instrument::ProcessorState::default(),
    );
    slot.instance_label_index = 7;

    let ron = ron::to_string(&slot).expect("slot should serialize");
    let restored: SlotState = ron::from_str(&ron).expect("slot should deserialize");

    assert_eq!(restored.instance_id, slot.instance_id);
    assert_eq!(restored.instance_label_index, slot.instance_label_index);
}
