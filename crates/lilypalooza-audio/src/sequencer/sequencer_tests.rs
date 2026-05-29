use knyst::prelude::Beats;

use super::{
    midi_events::{
        TimedMidiEvent, beats_to_ticks, metronome_clicks_between, normalized_time_signatures,
        ordered_events_at_same_time, ticks_to_beats,
    },
    scheduler::{MetronomeClick, Sequencer, TimeSignaturePoint},
};
use crate::{
    instrument::{BUILTIN_SOUNDFONT_ID, MidiEvent, ProcessorState, SlotState},
    mixer::{Mixer, MixerState, TrackId},
    test_utils::{OfflineHarness, simple_midi_bytes, test_soundfont_resource},
};

fn soundfont_slot(program: u8) -> SlotState {
    SlotState::built_in(BUILTIN_SOUNDFONT_ID, ProcessorState(vec![program]))
}

#[test]
fn ticks_to_beats_maps_whole_and_fractional_beats() {
    assert_eq!(ticks_to_beats(0, 480), Beats::ZERO);
    assert_eq!(ticks_to_beats(480, 480), Beats::from_beats(1));
    assert_eq!(
        ticks_to_beats(240, 480),
        Beats::from_fractional_beats::<2>(0, 1)
    );
}

#[test]
fn beats_to_ticks_maps_whole_and_fractional_beats() {
    assert_eq!(beats_to_ticks(Beats::ZERO, 480), 0);
    assert_eq!(beats_to_ticks(Beats::from_beats(1), 480), 480);
    assert_eq!(
        beats_to_ticks(Beats::from_fractional_beats::<2>(0, 1), 480),
        240
    );
}

#[test]
fn sequencer_maps_first_musical_track_to_first_instrument_track() {
    let mut harness = OfflineHarness::new(44_100, 64);
    let mut state = MixerState::new();
    state.set_soundfont(test_soundfont_resource());
    state
        .track_mut(TrackId(0))
        .expect("track 0 should exist")
        .set_instrument_slot(soundfont_slot(0));
    let context = harness.context().clone();
    let settings = harness.settings();
    let mixer = Mixer::new(&context, harness.commands(), &settings, state)
        .expect("mixer should initialize");
    let sequencer = Sequencer::new(false);
    sequencer.sync_track_handle(
        harness.commands(),
        TrackId(0),
        mixer.instrument_handle(TrackId(0)),
    );

    {
        let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
        handle
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("test midi should load");
    }

    let config = sequencer.config.load_full();
    assert_eq!(config.sequences.len(), 1);
    assert_eq!(config.sequences[0].target_track, TrackId(0));
}

#[test]
fn sequencer_delivers_each_midi_event_once() {
    let sequencer = Sequencer::new(false);
    {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut sequencer_handle =
            crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
        sequencer_handle
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("test midi should load");
    }

    let config = sequencer.config.load_full();
    assert_eq!(config.sequences.len(), 1);
    let events = &config.sequences[0].events;
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        TimedMidiEvent {
            at: Beats::ZERO,
            event: MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        }
    );
    assert_eq!(
        events[1],
        TimedMidiEvent {
            at: Beats::from_beats(1),
            event: MidiEvent::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0,
            },
        }
    );
}

#[test]
fn same_time_events_order_note_off_before_note_on() {
    let at = Beats::from_beats(1);
    let events = [
        TimedMidiEvent {
            at,
            event: MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        },
        TimedMidiEvent {
            at,
            event: MidiEvent::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0,
            },
        },
        TimedMidiEvent {
            at,
            event: MidiEvent::ProgramChange {
                channel: 0,
                program: 10,
            },
        },
    ];

    let ordered = ordered_events_at_same_time(&events);
    assert_eq!(
        ordered.iter().map(|event| event.event).collect::<Vec<_>>(),
        vec![
            MidiEvent::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0,
            },
            MidiEvent::ProgramChange {
                channel: 0,
                program: 10,
            },
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        ]
    );
}

#[test]
fn metronome_clicks_accent_first_beat_of_bar() {
    let clicks = metronome_clicks_between(
        &[TimeSignaturePoint::default()],
        Beats::ZERO,
        Beats::from_beats(5),
    );
    assert_eq!(
        clicks,
        vec![
            MetronomeClick {
                at: Beats::ZERO,
                accented: true,
            },
            MetronomeClick {
                at: Beats::from_beats(1),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats(2),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats(3),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats(4),
                accented: true,
            },
        ]
    );
}

#[test]
fn metronome_clicks_respect_time_signature_changes() {
    let clicks = metronome_clicks_between(
        &normalized_time_signatures(&[
            TimeSignaturePoint::default(),
            TimeSignaturePoint {
                at: Beats::from_beats(4),
                numerator: 3,
                denominator: 8,
            },
        ]),
        Beats::from_beats(3),
        Beats::from_beats_f64(6.5),
    );
    assert_eq!(
        clicks,
        vec![
            MetronomeClick {
                at: Beats::from_beats(3),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats(4),
                accented: true,
            },
            MetronomeClick {
                at: Beats::from_beats_f64(4.5),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats(5),
                accented: false,
            },
            MetronomeClick {
                at: Beats::from_beats_f64(5.5),
                accented: true,
            },
            MetronomeClick {
                at: Beats::from_beats(6),
                accented: false,
            },
        ]
    );
}
