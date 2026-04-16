//! MIDI-track sequencer and rolling lookahead scheduler.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands};
use knyst::scheduling::{MusicalTimeMap, TempoChange};
use knyst::time::SUBBEAT_TESIMALS_PER_BEAT;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::instrument::{InstrumentRuntimeHandle, MidiEvent as EngineMidiEvent};
use crate::mixer::{INSTRUMENT_TRACK_COUNT, TrackId};
use crate::transport::{PlaybackState, Transport, TransportError, TransportSnapshot};

/// Sequencer errors.
#[derive(thiserror::Error, Debug)]
pub enum SequencerError {
    /// MIDI parsing failed.
    #[error("failed to parse MIDI: {0}")]
    Parse(String),
    /// SMPTE timing is unsupported.
    #[error("SMPTE MIDI timing is unsupported")]
    UnsupportedTiming,
    /// Transport snapshot failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
}

/// Score-backed sequencer state.
#[derive(Debug, Clone)]
pub struct Sequencer {
    inner: Arc<Mutex<SequencerState>>,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequencer {
    /// Creates an empty sequencer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SequencerState::new())),
        }
    }

    pub(crate) fn sync_track_handle(
        &self,
        track_id: TrackId,
        handle: Option<InstrumentRuntimeHandle>,
    ) {
        let mut inner = self.inner.lock().expect("sequencer mutex poisoned");
        if let Some(slot) = inner.instrument_handles.get_mut(track_id.index()) {
            *slot = handle;
        }
        inner.dirty = true;
    }

    pub(crate) fn mark_dirty(&self) {
        self.inner.lock().expect("sequencer mutex poisoned").dirty = true;
    }

    pub(crate) fn process_tick(
        &self,
        commands: &mut MultiThreadedKnystCommands,
    ) -> Result<(), SequencerError> {
        let snapshot = Transport::new(commands, None).snapshot()?;
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .process_tick(commands, snapshot);
        Ok(())
    }
}

/// Mutable sequencer control handle.
pub struct SequencerHandle<'a> {
    sequencer: &'a Sequencer,
    commands: &'a mut MultiThreadedKnystCommands,
}

impl<'a> SequencerHandle<'a> {
    pub(crate) fn new(
        sequencer: &'a Sequencer,
        commands: &'a mut MultiThreadedKnystCommands,
    ) -> Self {
        Self {
            sequencer,
            commands,
        }
    }

    /// Replaces all loaded track sequences from MIDI bytes.
    pub fn replace_from_midi_bytes(&mut self, bytes: &[u8]) -> Result<(), SequencerError> {
        let smf = Smf::parse(bytes).map_err(|error| SequencerError::Parse(error.to_string()))?;
        let Timing::Metrical(ppq) = smf.header.timing else {
            return Err(SequencerError::UnsupportedTiming);
        };
        let ppq = ppq.as_int();

        let mut sequences = Vec::new();
        let mut tempo_points = Vec::new();
        let mut total_ticks = 0_u64;

        for track in &smf.tracks {
            if let Some(sequence) = Sequence::from_midi_track(sequences.len(), track, ppq) {
                sequences.push(sequence);
            }

            let mut absolute_ticks = 0_u64;
            for event in track {
                absolute_ticks = absolute_ticks.saturating_add(u64::from(event.delta.as_int()));
                total_ticks = total_ticks.max(absolute_ticks);
                let TrackEventKind::Meta(MetaMessage::Tempo(micros_per_quarter)) = event.kind
                else {
                    continue;
                };
                let micros_per_quarter = micros_per_quarter.as_int();
                if micros_per_quarter == 0 {
                    continue;
                }
                tempo_points.push(TempoPoint {
                    at: ticks_to_beats(absolute_ticks, ppq),
                    bpm: 60_000_000.0 / f64::from(micros_per_quarter),
                });
            }
        }

        self.replace_tempo_map(&tempo_points);

        let mut inner = self
            .sequencer
            .inner
            .lock()
            .expect("sequencer mutex poisoned");
        inner.sequences = sequences;
        inner.next_indices = vec![0; inner.sequences.len()];
        inner.ppq = ppq;
        inner.total_ticks = total_ticks;
        inner.scheduled_until = Beats::ZERO;
        inner.last_position = None;
        inner.dirty = true;
        Ok(())
    }

    /// Returns the current playback position in MIDI ticks.
    pub fn playback_tick(&mut self) -> Result<u64, SequencerError> {
        let ppq = self
            .sequencer
            .inner
            .lock()
            .expect("sequencer mutex poisoned")
            .ppq;
        if ppq == 0 {
            return Ok(0);
        }

        let beats = Transport::new(self.commands, None)
            .snapshot()?
            .beats_position;
        Ok(beats_to_ticks(beats, ppq))
    }

    /// Returns the total loaded MIDI duration in ticks.
    #[must_use]
    pub fn total_ticks(&self) -> u64 {
        self.sequencer
            .inner
            .lock()
            .expect("sequencer mutex poisoned")
            .total_ticks
    }

    /// Clears the loaded score and scheduling state.
    pub fn clear(&mut self) {
        let mut inner = self
            .sequencer
            .inner
            .lock()
            .expect("sequencer mutex poisoned");
        inner.sequences.clear();
        inner.next_indices.clear();
        inner.ppq = 0;
        inner.total_ticks = 0;
        inner.scheduled_until = Beats::ZERO;
        inner.last_position = None;
        inner.dirty = true;
    }

    fn replace_tempo_map(&mut self, tempos: &[TempoPoint]) {
        let mut tempos = tempos.to_vec();
        tempos.sort_by_key(|tempo| tempo.at);
        self.commands
            .change_musical_time_map(move |map: &mut MusicalTimeMap| {
                *map = MusicalTimeMap::new();
                let initial_bpm = tempos
                    .iter()
                    .find(|tempo| tempo.at == Beats::ZERO)
                    .map_or(120.0, |tempo| tempo.bpm);
                map.replace(0, TempoChange::NewTempo { bpm: initial_bpm });
                for tempo in tempos {
                    map.insert(TempoChange::NewTempo { bpm: tempo.bpm }, tempo.at);
                }
            });
    }
}

#[derive(Debug)]
struct SequencerState {
    sequences: Vec<Sequence>,
    next_indices: Vec<usize>,
    ppq: u16,
    total_ticks: u64,
    scheduled_until: Beats,
    lookahead: Beats,
    refill_margin: Beats,
    instrument_handles: Vec<Option<InstrumentRuntimeHandle>>,
    dirty: bool,
    last_position: Option<Beats>,
}

impl SequencerState {
    fn new() -> Self {
        Self {
            sequences: Vec::new(),
            next_indices: Vec::new(),
            ppq: 0,
            total_ticks: 0,
            scheduled_until: Beats::ZERO,
            lookahead: Beats::from_beats(8),
            refill_margin: Beats::from_beats(2),
            instrument_handles: vec![None; INSTRUMENT_TRACK_COUNT],
            dirty: false,
            last_position: None,
        }
    }

    fn process_tick(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        snapshot: TransportSnapshot,
    ) {
        let now = snapshot.beats_position;
        let transport_jumped_back = self.last_position.is_some_and(|last| now < last);
        if self.dirty || transport_jumped_back {
            self.sync_to_transport(commands, snapshot);
            return;
        }

        self.last_position = Some(now);

        if snapshot.playback_state != PlaybackState::Playing {
            return;
        }

        if self.scheduled_until > now + self.refill_margin {
            return;
        }

        let window_start = self.scheduled_until.max(now);
        self.schedule_window(commands, window_start, window_start + self.lookahead);
    }

    fn sync_to_transport(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        snapshot: TransportSnapshot,
    ) {
        let should_reset_notes =
            self.last_position.is_some() || snapshot.playback_state == PlaybackState::Playing;
        if should_reset_notes {
            self.reset_notes(commands);
        }
        let now = snapshot.beats_position;
        self.reset_schedule_state_at(now);
        self.schedule_window(commands, now, now + self.lookahead);
        self.dirty = false;
    }

    fn schedule_window(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        window_start: Beats,
        window_end: Beats,
    ) {
        for (sequence, next_index) in self.sequences.iter().zip(self.next_indices.iter_mut()) {
            let Some(handle) = self
                .instrument_handles
                .get(sequence.target_track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            while let Some(timed_event) = sequence.events.get(*next_index) {
                if timed_event.at > window_end {
                    break;
                }
                if timed_event.at >= window_start {
                    handle.schedule_midi(commands, timed_event.at, timed_event.event);
                }
                *next_index += 1;
            }
        }
        self.scheduled_until = window_end;
    }

    fn reset_schedule_state_at(&mut self, at: Beats) {
        self.next_indices = self
            .sequences
            .iter()
            .map(|sequence| first_event_at_or_after(&sequence.events, at))
            .collect();
        self.scheduled_until = at;
        self.last_position = Some(at);
    }

    fn reset_notes(&self, commands: &mut MultiThreadedKnystCommands) {
        let mut tracks = BTreeSet::new();
        for sequence in &self.sequences {
            tracks.insert(sequence.target_track);
        }
        for track in tracks {
            let Some(handle) = self
                .instrument_handles
                .get(track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            for channel in 0..16 {
                handle.send_midi(commands, EngineMidiEvent::AllNotesOff { channel });
                handle.send_midi(commands, EngineMidiEvent::AllSoundOff { channel });
                handle.send_midi(commands, EngineMidiEvent::ResetAllControllers { channel });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sequence {
    target_track: TrackId,
    events: Vec<TimedMidiEvent>,
}

impl Sequence {
    fn from_midi_track(track_index: usize, track: &[TrackEvent<'_>], ppq: u16) -> Option<Self> {
        if track_index >= INSTRUMENT_TRACK_COUNT {
            return None;
        }

        let mut absolute_ticks = 0_u64;
        let mut events = Vec::new();

        for event in track {
            absolute_ticks = absolute_ticks.saturating_add(u64::from(event.delta.as_int()));
            let TrackEventKind::Midi { channel, message } = event.kind else {
                continue;
            };
            let channel = channel.as_int();
            let Some(event) = midi_message(channel, message) else {
                continue;
            };
            events.push(TimedMidiEvent {
                at: ticks_to_beats(absolute_ticks, ppq),
                event,
            });
        }

        if events.is_empty() {
            return None;
        }

        Some(Self {
            target_track: TrackId(track_index as u16),
            events,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimedMidiEvent {
    at: Beats,
    event: EngineMidiEvent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TempoPoint {
    at: Beats,
    bpm: f64,
}

fn first_event_at_or_after(events: &[TimedMidiEvent], at: Beats) -> usize {
    events.partition_point(|event| event.at < at)
}

fn midi_message(channel: u8, message: MidiMessage) -> Option<EngineMidiEvent> {
    Some(match message {
        MidiMessage::NoteOn { key, vel } if vel.as_int() == 0 => EngineMidiEvent::NoteOff {
            channel,
            note: key.as_int(),
            velocity: 0,
        },
        MidiMessage::NoteOn { key, vel } => EngineMidiEvent::NoteOn {
            channel,
            note: key.as_int(),
            velocity: vel.as_int(),
        },
        MidiMessage::NoteOff { key, vel } => EngineMidiEvent::NoteOff {
            channel,
            note: key.as_int(),
            velocity: vel.as_int(),
        },
        MidiMessage::Aftertouch { key, vel } => EngineMidiEvent::PolyPressure {
            channel,
            note: key.as_int(),
            pressure: vel.as_int(),
        },
        MidiMessage::Controller { controller, value } => match controller.as_int() {
            120 => EngineMidiEvent::AllSoundOff { channel },
            121 => EngineMidiEvent::ResetAllControllers { channel },
            123 => EngineMidiEvent::AllNotesOff { channel },
            controller => EngineMidiEvent::ControlChange {
                channel,
                controller,
                value: value.as_int(),
            },
        },
        MidiMessage::ProgramChange { program } => EngineMidiEvent::ProgramChange {
            channel,
            program: program.as_int(),
        },
        MidiMessage::ChannelAftertouch { vel } => EngineMidiEvent::ChannelPressure {
            channel,
            pressure: vel.as_int(),
        },
        MidiMessage::PitchBend { bend } => EngineMidiEvent::PitchBend {
            channel,
            value: bend.as_int(),
        },
    })
}

fn ticks_to_beats(ticks: u64, ppq: u16) -> Beats {
    let ppq = u64::from(ppq.max(1));
    let beats = (ticks / ppq) as u32;
    let beat_tesimals = ((ticks % ppq) * u64::from(SUBBEAT_TESIMALS_PER_BEAT) / ppq) as u32;
    Beats::new(beats, beat_tesimals)
}

fn beats_to_ticks(beats: Beats, ppq: u16) -> u64 {
    (beats.as_beats_f64() * f64::from(ppq.max(1))).round() as u64
}

#[cfg(test)]
mod tests {
    use knyst::prelude::Beats;

    use super::{Sequencer, beats_to_ticks, ticks_to_beats};
    use crate::instrument::InstrumentSlotState;
    use crate::mixer::{Mixer, MixerState, TrackId};
    use crate::test_utils::{OfflineHarness, simple_midi_bytes, test_soundfont_resource};
    use crate::transport::{PlaybackState, TransportSnapshot};

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
    fn sequencer_schedules_midi_into_track_output() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let context = harness.context().clone();
        let settings = harness.settings();
        let mixer = Mixer::new(&context, harness.commands(), &settings, state)
            .expect("mixer should initialize");
        let sequencer = Sequencer::new();
        sequencer.sync_track_handle(TrackId(0), mixer.instrument_handle(TrackId(0)));

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&simple_midi_bytes(480))
                .expect("test midi should load");
        }

        {
            let mut inner = sequencer.inner.lock().expect("sequencer mutex poisoned");
            inner.process_tick(
                harness.commands(),
                TransportSnapshot::new(
                    PlaybackState::Playing,
                    knyst::time::Seconds::ZERO,
                    Beats::ZERO,
                ),
            );
        }

        for _ in 0..64 {
            harness.process_block();
            let snapshot = TransportSnapshot::new(
                PlaybackState::Playing,
                knyst::time::Seconds::ZERO,
                Beats::from_fractional_beats::<64>(0, 1),
            );
            let mut inner = sequencer.inner.lock().expect("sequencer mutex poisoned");
            inner.process_tick(harness.commands(), snapshot);
        }

        assert!(harness.output_has_signal());
    }

    #[test]
    fn sequencer_maps_first_musical_track_to_first_instrument_track() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let context = harness.context().clone();
        let settings = harness.settings();
        let mixer = Mixer::new(&context, harness.commands(), &settings, state)
            .expect("mixer should initialize");
        let sequencer = Sequencer::new();
        sequencer.sync_track_handle(TrackId(0), mixer.instrument_handle(TrackId(0)));

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&simple_midi_bytes(480))
                .expect("test midi should load");
        }

        let inner = sequencer.inner.lock().expect("sequencer mutex poisoned");
        assert_eq!(inner.sequences.len(), 1);
        assert_eq!(inner.sequences[0].target_track, TrackId(0));
    }
}
