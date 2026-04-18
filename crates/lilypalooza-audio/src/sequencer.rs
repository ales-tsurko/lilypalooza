//! MIDI-track sequencer and score scheduler.

use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands};
use knyst::scheduling::{MusicalTimeMap, TempoChange};
use knyst::time::SUBBEAT_TESIMALS_PER_BEAT;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::instrument::{InstrumentRuntimeHandle, MidiEvent as EngineMidiEvent};
use crate::mixer::{INSTRUMENT_TRACK_COUNT, TrackId};
use crate::transport::{Transport, TransportError};

const CONTROLLER_BARRIER_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(250);

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
#[derive(Clone)]
pub struct Sequencer {
    inner: Arc<Mutex<SequencerState>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct SequencerDebugState {
    pub reset_count: usize,
    pub schedule_count: usize,
    #[allow(dead_code)]
    pub scheduled_event_count: usize,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Sequencer {
    /// Creates an empty sequencer.
    #[must_use]
    pub fn new(chase_notes_on_seek: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SequencerState::new(chase_notes_on_seek))),
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

    pub(crate) fn configure_schedule_lead(&self, block_size: usize, sample_rate: usize) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .configure_schedule_lead(block_size, sample_rate);
    }

    pub(crate) fn prepare_for_play(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        _start_beat: Beats,
    ) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .prepare_for_play(commands, _start_beat);
    }

    pub(crate) fn prepare_for_pause(&self, commands: &mut MultiThreadedKnystCommands, at: Beats) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .schedule_pause_reset_at(at, commands);
    }

    pub(crate) fn prepare_for_pause_immediate(&self, commands: &mut MultiThreadedKnystCommands) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .dispatch_immediate_pause_reset(commands);
    }

    pub(crate) fn mark_dirty_for_seek(&self, position: Beats, needs_reset_on_play: bool) {
        let mut inner = self.inner.lock().expect("sequencer mutex poisoned");
        inner.dirty = true;
        inner.pending_position = Some(position);
        inner.needs_reset_on_play = needs_reset_on_play;
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.inner.lock().expect("sequencer mutex poisoned").playing = playing;
    }

    pub(crate) fn pending_position(&self) -> Option<Beats> {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .pending_position
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.inner.lock().expect("sequencer mutex poisoned").playing
    }

    pub(crate) fn has_loaded_score(&self) -> bool {
        !self
            .inner
            .lock()
            .expect("sequencer mutex poisoned")
            .sequences
            .is_empty()
    }

    pub(crate) fn process_tick(&self, commands: &mut MultiThreadedKnystCommands) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .process_tick(commands);
    }

    pub(crate) fn process_tick_at(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        current_beat: Beats,
    ) {
        self.inner
            .lock()
            .expect("sequencer mutex poisoned")
            .process_tick_at(commands, current_beat);
    }

    #[cfg(test)]
    pub(crate) fn debug_state(&self) -> SequencerDebugState {
        let inner = self.inner.lock().expect("sequencer mutex poisoned");
        SequencerDebugState {
            reset_count: inner.reset_count,
            schedule_count: inner.schedule_count,
            scheduled_event_count: inner.scheduled_event_count,
        }
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
        wait_for_controller_barrier(self.commands);

        let mut inner = self
            .sequencer
            .inner
            .lock()
            .expect("sequencer mutex poisoned");
        inner.tempo_map = build_tempo_map(&tempo_points);
        inner.sequences = sequences;
        inner.next_indices = vec![0; inner.sequences.len()];
        inner.ppq = ppq;
        inner.total_ticks = total_ticks;
        inner.scheduled_until = Beats::ZERO;
        inner.pending_position = None;
        inner.dirty = true;
        drop(inner);
        Ok(())
    }

    /// Returns the current playback position in MIDI ticks.
    pub fn playback_tick(&mut self) -> Result<u64, SequencerError> {
        let (ppq, playing, pending_position) = {
            let inner = self
                .sequencer
                .inner
                .lock()
                .expect("sequencer mutex poisoned");
            (inner.ppq, inner.playing, inner.pending_position)
        };
        if ppq == 0 {
            return Ok(0);
        }
        if !playing && let Some(position) = pending_position {
            return Ok(beats_to_ticks(position, ppq));
        }

        let beats = Transport::new(self.commands, None, None)
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
        inner.tempo_map = MusicalTimeMap::new();
        inner.ppq = 0;
        inner.total_ticks = 0;
        inner.scheduled_until = Beats::ZERO;
        inner.pending_position = None;
        inner.dirty = true;
        drop(inner);
    }

    fn replace_tempo_map(&mut self, tempos: &[TempoPoint]) {
        let tempo_map = build_tempo_map(tempos);
        self.commands
            .change_musical_time_map(move |map: &mut MusicalTimeMap| {
                *map = tempo_map.clone();
            });
    }
}

fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    let _ = receiver.recv_timeout(CONTROLLER_BARRIER_TIMEOUT);
}

struct SequencerState {
    generations: Vec<u32>,
    sequences: Vec<Sequence>,
    next_indices: Vec<usize>,
    tempo_map: MusicalTimeMap,
    ppq: u16,
    total_ticks: u64,
    sample_rate: f64,
    scheduled_until: Beats,
    lookahead: Beats,
    refill_margin: Beats,
    reset_frame_offset: i32,
    chase_frame_offset: i32,
    initial_frame_offset: i32,
    instrument_handles: Vec<Option<InstrumentRuntimeHandle>>,
    chase_notes_on_seek: bool,
    playing: bool,
    dirty: bool,
    needs_reset_on_play: bool,
    pending_position: Option<Beats>,
    #[cfg(test)]
    reset_count: usize,
    #[cfg(test)]
    schedule_count: usize,
    #[cfg(test)]
    scheduled_event_count: usize,
}

impl SequencerState {
    fn new(chase_notes_on_seek: bool) -> Self {
        Self {
            generations: vec![0; INSTRUMENT_TRACK_COUNT],
            sequences: Vec::new(),
            next_indices: Vec::new(),
            tempo_map: MusicalTimeMap::new(),
            ppq: 0,
            total_ticks: 0,
            sample_rate: 44_100.0,
            scheduled_until: Beats::ZERO,
            lookahead: Beats::from_beats(8),
            refill_margin: Beats::from_beats(2),
            reset_frame_offset: 256,
            chase_frame_offset: 384,
            initial_frame_offset: 512,
            instrument_handles: vec![None; INSTRUMENT_TRACK_COUNT],
            chase_notes_on_seek,
            playing: false,
            dirty: false,
            needs_reset_on_play: false,
            pending_position: None,
            #[cfg(test)]
            reset_count: 0,
            #[cfg(test)]
            schedule_count: 0,
            #[cfg(test)]
            scheduled_event_count: 0,
        }
    }

    fn configure_schedule_lead(&mut self, block_size: usize, sample_rate: usize) {
        let block = block_size.max(64) as i32;
        self.sample_rate = sample_rate.max(1) as f64;
        self.reset_frame_offset = block * 64;
        self.chase_frame_offset = block * 72;
        self.initial_frame_offset = block * 80;
    }

    fn prepare_for_play(&mut self, commands: &mut MultiThreadedKnystCommands, start_beat: Beats) {
        self.playing = true;
        let position = self.pending_position.unwrap_or(start_beat);

        self.reset_schedule_state_at(position);
        if self.needs_reset_on_play {
            self.schedule_reset_and_chase_at(position, commands);
        } else if self.chase_notes_on_seek {
            self.dispatch_chase_events_at(position, commands);
        }
        self.schedule_window(
            position,
            position + self.lookahead,
            commands,
            self.initial_frame_offset,
        );
        self.needs_reset_on_play = false;
        self.dirty = false;
    }

    fn process_tick(&mut self, commands: &mut MultiThreadedKnystCommands) {
        if !self.playing {
            return;
        }

        let Some(snapshot) = commands.current_transport_snapshot() else {
            return;
        };
        let current_beat = snapshot.beats.unwrap_or(Beats::ZERO);
        self.process_tick_at(commands, current_beat);
    }

    fn process_tick_at(&mut self, commands: &mut MultiThreadedKnystCommands, current_beat: Beats) {
        if self.dirty {
            let position = self.pending_position.unwrap_or(current_beat);
            self.reset_schedule_state_at(position);
            self.schedule_reset_and_chase_at(position, commands);
            self.schedule_window(
                position,
                position + self.lookahead,
                commands,
                self.initial_frame_offset,
            );
            self.dirty = false;
            return;
        }

        if self.scheduled_until > current_beat + self.refill_margin {
            return;
        }

        let window_start = self.scheduled_until.max(current_beat);
        self.schedule_window(window_start, window_start + self.lookahead, commands, 0);
    }

    fn schedule_window(
        &mut self,
        window_start: Beats,
        window_end: Beats,
        commands: &mut MultiThreadedKnystCommands,
        initial_frame_offset: i32,
    ) {
        #[cfg(test)]
        {
            self.schedule_count += 1;
        }

        let sample_rate = self.sample_rate;
        let tempo_map = &self.tempo_map;
        for (sequence, next_index) in self.sequences.iter().zip(self.next_indices.iter_mut()) {
            let Some(handle) = self
                .instrument_handles
                .get(sequence.target_track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            while let Some(first_event) = sequence.events.get(*next_index) {
                if first_event.at > window_end {
                    break;
                }

                let event_time = first_event.at;
                let group_start = *next_index;
                while let Some(next_event) = sequence.events.get(*next_index) {
                    if next_event.at != event_time {
                        break;
                    }
                    *next_index += 1;
                }

                if event_time < window_start {
                    continue;
                }

                let mut frame_offset = if event_time == window_start {
                    initial_frame_offset
                } else {
                    0
                };
                for timed_event in
                    ordered_events_at_same_time(&sequence.events[group_start..*next_index])
                {
                    #[cfg(test)]
                    {
                        self.scheduled_event_count += 1;
                    }
                    handle.schedule_midi_at_with_offset(
                        commands,
                        offset_beats(tempo_map, sample_rate, timed_event.at, frame_offset),
                        self.generations[sequence.target_track.index()],
                        timed_event.event,
                    );
                    frame_offset += 2;
                }
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
        self.pending_position = None;
    }

    fn schedule_pause_reset_at(&mut self, at: Beats, commands: &mut MultiThreadedKnystCommands) {
        #[cfg(test)]
        {
            self.reset_count += 1;
        }
        let mut tracks = BTreeSet::new();
        for sequence in &self.sequences {
            tracks.insert(sequence.target_track);
        }
        for track in tracks {
            self.generations[track.index()] = self.generations[track.index()].wrapping_add(1);
            let Some(handle) = self
                .instrument_handles
                .get(track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            let generation = self.generations[track.index()];
            handle.schedule_reset_at(
                commands,
                offset_beats(
                    &self.tempo_map,
                    self.sample_rate,
                    at,
                    self.reset_frame_offset,
                ),
                generation,
            );
            dispatch_scheduled_panic_events(
                handle,
                generation,
                commands,
                &self.tempo_map,
                self.sample_rate,
                at,
                self.reset_frame_offset + 2,
            );
        }
    }

    fn dispatch_chase_events_at(&self, at: Beats, commands: &mut MultiThreadedKnystCommands) {
        for sequence in &self.sequences {
            let Some(handle) = self
                .instrument_handles
                .get(sequence.target_track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            let generation = self.generations[sequence.target_track.index()];
            let mut frame_offset = self.chase_frame_offset;
            for event in chase_events_at(&sequence.events, at) {
                handle.schedule_midi_at_with_offset(
                    commands,
                    offset_beats(&self.tempo_map, self.sample_rate, at, frame_offset),
                    generation,
                    event,
                );
                frame_offset += 2;
            }
        }
    }

    fn schedule_reset_and_chase_at(
        &mut self,
        at: Beats,
        commands: &mut MultiThreadedKnystCommands,
    ) {
        let mut tracks = BTreeSet::new();
        for sequence in &self.sequences {
            tracks.insert(sequence.target_track);
        }
        for track in tracks {
            self.generations[track.index()] = self.generations[track.index()].wrapping_add(1);
            let Some(handle) = self
                .instrument_handles
                .get(track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            handle.schedule_reset_at(
                commands,
                offset_beats(
                    &self.tempo_map,
                    self.sample_rate,
                    at,
                    self.reset_frame_offset,
                ),
                self.generations[track.index()],
            );
            dispatch_scheduled_panic_events(
                handle,
                self.generations[track.index()],
                commands,
                &self.tempo_map,
                self.sample_rate,
                at,
                self.reset_frame_offset + 2,
            );
        }
        if self.chase_notes_on_seek {
            self.dispatch_chase_events_at(at, commands);
        }
    }

    fn dispatch_immediate_pause_reset(&mut self, commands: &mut MultiThreadedKnystCommands) {
        let mut tracks = BTreeSet::new();
        for sequence in &self.sequences {
            tracks.insert(sequence.target_track);
        }
        for track in tracks {
            self.generations[track.index()] = self.generations[track.index()].wrapping_add(1);
            let Some(handle) = self
                .instrument_handles
                .get(track.index())
                .copied()
                .flatten()
            else {
                continue;
            };
            let generation = self.generations[track.index()];
            handle.send_reset_live(commands, generation);
            dispatch_immediate_panic_events(handle, generation, commands);
        }
    }
}

fn offset_beats(
    tempo_map: &MusicalTimeMap,
    sample_rate: f64,
    at: Beats,
    frame_offset: i32,
) -> Beats {
    if frame_offset <= 0 {
        return at;
    }
    let seconds = tempo_map.musical_time_to_secs_f64(at);
    let offset_seconds = seconds + (frame_offset as f64 / sample_rate);
    tempo_map.seconds_to_beats(knyst::prelude::Seconds::from_seconds_f64(offset_seconds))
}

fn dispatch_scheduled_panic_events(
    handle: InstrumentRuntimeHandle,
    generation: u32,
    commands: &mut MultiThreadedKnystCommands,
    tempo_map: &MusicalTimeMap,
    sample_rate: f64,
    at: Beats,
    start_frame_offset: i32,
) {
    let mut frame_offset = start_frame_offset;
    for channel in 0..16_u8 {
        for event in [
            EngineMidiEvent::AllSoundOff { channel },
            EngineMidiEvent::AllNotesOff { channel },
            EngineMidiEvent::ResetAllControllers { channel },
        ] {
            handle.schedule_midi_at_with_offset(
                commands,
                offset_beats(tempo_map, sample_rate, at, frame_offset),
                generation,
                event,
            );
            frame_offset += 2;
        }
    }
}

fn dispatch_immediate_panic_events(
    handle: InstrumentRuntimeHandle,
    generation: u32,
    commands: &mut MultiThreadedKnystCommands,
) {
    let mut step = 1;
    for channel in 0..16_u8 {
        for event in [
            EngineMidiEvent::AllSoundOff { channel },
            EngineMidiEvent::AllNotesOff { channel },
            EngineMidiEvent::ResetAllControllers { channel },
        ] {
            handle.send_midi_immediate(
                commands,
                generation,
                event,
                InstrumentRuntimeHandle::immediate_event_delay(step),
            );
            step += 1;
        }
    }
}

fn ordered_events_at_same_time(events: &[TimedMidiEvent]) -> Vec<TimedMidiEvent> {
    let mut ordered = Vec::with_capacity(events.len());
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 0),
    );
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 1),
    );
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 2),
    );
    ordered
}

fn event_sort_group(event: EngineMidiEvent) -> u8 {
    match event {
        EngineMidiEvent::NoteOff { .. }
        | EngineMidiEvent::AllNotesOff { .. }
        | EngineMidiEvent::AllSoundOff { .. } => 0,
        EngineMidiEvent::ProgramChange { .. }
        | EngineMidiEvent::ControlChange { .. }
        | EngineMidiEvent::ChannelPressure { .. }
        | EngineMidiEvent::PolyPressure { .. }
        | EngineMidiEvent::PitchBend { .. }
        | EngineMidiEvent::ResetAllControllers { .. } => 1,
        EngineMidiEvent::NoteOn { .. } => 2,
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

fn build_tempo_map(tempos: &[TempoPoint]) -> MusicalTimeMap {
    let mut tempos = tempos.to_vec();
    tempos.sort_by_key(|tempo| tempo.at);

    let mut map = MusicalTimeMap::new();
    let initial_bpm = tempos
        .iter()
        .find(|tempo| tempo.at == Beats::ZERO)
        .map_or(120.0, |tempo| tempo.bpm);
    map.replace(0, TempoChange::NewTempo { bpm: initial_bpm });
    for tempo in tempos {
        map.insert(TempoChange::NewTempo { bpm: tempo.bpm }, tempo.at);
    }
    map
}

fn first_event_at_or_after(events: &[TimedMidiEvent], at: Beats) -> usize {
    events.partition_point(|event| event.at < at)
}

fn chase_events_at(events: &[TimedMidiEvent], at: Beats) -> Vec<EngineMidiEvent> {
    let mut active_notes: HashMap<(u8, u8), u8> = HashMap::new();

    for timed_event in events {
        if timed_event.at >= at {
            break;
        }
        match timed_event.event {
            EngineMidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => {
                active_notes.insert((channel, note), velocity);
            }
            EngineMidiEvent::NoteOff { channel, note, .. } => {
                active_notes.remove(&(channel, note));
            }
            EngineMidiEvent::AllNotesOff { channel } | EngineMidiEvent::AllSoundOff { channel } => {
                active_notes.retain(|(note_channel, _), _| *note_channel != channel);
            }
            _ => {}
        }
    }

    for timed_event in events {
        if timed_event.at != at {
            continue;
        }
        match timed_event.event {
            EngineMidiEvent::NoteOn { channel, note, .. }
            | EngineMidiEvent::NoteOff { channel, note, .. } => {
                active_notes.remove(&(channel, note));
            }
            EngineMidiEvent::AllNotesOff { channel } | EngineMidiEvent::AllSoundOff { channel } => {
                active_notes.retain(|(note_channel, _), _| *note_channel != channel);
            }
            _ => {}
        }
    }

    let mut chased: Vec<_> = active_notes
        .into_iter()
        .map(|((channel, note), velocity)| EngineMidiEvent::NoteOn {
            channel,
            note,
            velocity,
        })
        .collect();
    chased.sort_by_key(|event| match event {
        EngineMidiEvent::NoteOn { channel, note, .. } => (*channel, *note),
        _ => (0, 0),
    });
    chased
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
    use knyst::controller::KnystCommands;
    use knyst::prelude::Beats;

    use super::{
        Sequencer, TimedMidiEvent, beats_to_ticks, ordered_events_at_same_time, ticks_to_beats,
    };
    use crate::instrument::{InstrumentSlotState, MidiEvent};
    use crate::mixer::{Mixer, MixerState, TrackId};
    use crate::test_utils::{
        OfflineHarness, delayed_note_midi_bytes, four_track_midi_bytes, simple_midi_bytes,
        test_soundfont_resource,
    };

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
        const BLOCK_SIZE: usize = 64;
        const SAMPLE_RATE: f64 = 44_100.0;
        const BPM: f64 = 120.0;

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
        let sequencer = Sequencer::new(false);
        sequencer.sync_track_handle(TrackId(0), mixer.instrument_handle(TrackId(0)));

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
                .expect("test midi should load");
        }

        {
            sequencer.prepare_for_play(harness.commands(), Beats::ZERO);
            sequencer.set_playing(true);
            harness.commands().transport_play();
        }

        let beats_per_second = BPM / 60.0;
        for block in 0..512 {
            let block_start_seconds = (block * BLOCK_SIZE) as f64 / SAMPLE_RATE;
            let block_start_beat = Beats::from_beats_f64(block_start_seconds * beats_per_second);
            sequencer.process_tick_at(harness.commands(), block_start_beat);
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
        }

        panic!("sequencer scheduled playback produced silence");
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
        let sequencer = Sequencer::new(false);
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

    #[test]
    fn transport_play_primes_sequencer_and_produces_audio() {
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
        let sequencer = Sequencer::new(false);
        sequencer.sync_track_handle(TrackId(0), mixer.instrument_handle(TrackId(0)));

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&simple_midi_bytes(480))
                .expect("test midi should load");
        }

        {
            sequencer.prepare_for_play(harness.commands(), Beats::ZERO);
            sequencer.set_playing(true);
            harness.commands().transport_play();
        }

        for _ in 0..128 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
        }

        panic!("transport play path produced silence");
    }

    #[test]
    fn four_track_midi_with_tempo_track_produces_audio_from_start() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        for track_index in 0..4 {
            state
                .track_mut(TrackId(track_index as u16))
                .expect("track should exist")
                .instrument = InstrumentSlotState::soundfont("default", 0, track_index as u8);
        }
        let context = harness.context().clone();
        let settings = harness.settings();
        let mixer = Mixer::new(&context, harness.commands(), &settings, state)
            .expect("mixer should initialize");
        let sequencer = Sequencer::new(false);
        for track_index in 0..4 {
            let track_id = TrackId(track_index as u16);
            sequencer.sync_track_handle(track_id, mixer.instrument_handle(track_id));
        }

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&four_track_midi_bytes(480))
                .expect("test midi should load");
        }

        {
            sequencer.prepare_for_play(harness.commands(), Beats::ZERO);
            sequencer.set_playing(true);
            harness.commands().transport_play();
        }

        for _ in 0..128 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
        }

        panic!("four-track play path produced silence");
    }

    #[test]
    fn paused_midi_load_does_not_consume_events_before_play() {
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
        let sequencer = Sequencer::new(false);
        sequencer.sync_track_handle(TrackId(0), mixer.instrument_handle(TrackId(0)));

        {
            let mut handle = crate::sequencer::SequencerHandle::new(&sequencer, harness.commands());
            handle
                .replace_from_midi_bytes(&simple_midi_bytes(480))
                .expect("test midi should load");
        }

        for _ in 0..64 {
            harness.process_block();
        }

        assert!(
            !harness.output_has_signal(),
            "paused sequencer should not produce audio before play"
        );

        {
            sequencer.prepare_for_play(harness.commands(), Beats::ZERO);
            sequencer.set_playing(true);
            harness.commands().transport_play();
        }

        for _ in 0..128 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
        }

        panic!("play after paused load produced silence");
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

        let inner = sequencer.inner.lock().expect("sequencer mutex poisoned");
        assert_eq!(inner.sequences.len(), 1);
        let events = &inner.sequences[0].events;
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
}
