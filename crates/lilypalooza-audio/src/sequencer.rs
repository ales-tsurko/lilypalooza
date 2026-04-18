//! MIDI-track sequencer and score scheduler.

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands};
use knyst::scheduling::{MusicalTimeMap, TempoChange};
use knyst::time::SUBBEAT_TESIMALS_PER_BEAT;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::instrument::{InstrumentRuntimeHandle, MidiEvent as EngineMidiEvent};
use crate::mixer::{INSTRUMENT_TRACK_COUNT, TrackId};
use crate::transport::TransportError;

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
    hot: Arc<SequencerHotState>,
    config: Arc<ArcSwap<SequencerConfig>>,
    runtime: Arc<ArcSwap<SequencerRuntime>>,
    scheduler_updates_tx: Sender<SequencerSchedulerUpdate>,
    scheduler_updates_rx: Arc<Mutex<Option<Receiver<SequencerSchedulerUpdate>>>>,
}

enum SequencerSchedulerUpdate {
    Config(Arc<SequencerConfig>),
    Both(Arc<SequencerConfig>, Arc<SequencerRuntime>),
}

struct SequencerHotState {
    playing: AtomicBool,
    dirty: AtomicBool,
    needs_reset_on_play: AtomicBool,
    has_loaded_score: AtomicBool,
    ppq: AtomicU16,
    total_ticks: AtomicU64,
    current_beats_bits: AtomicU64,
    pending_position_present: AtomicBool,
    pending_position_bits: AtomicU64,
}

impl SequencerHotState {
    fn new() -> Self {
        Self {
            playing: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            needs_reset_on_play: AtomicBool::new(false),
            has_loaded_score: AtomicBool::new(false),
            ppq: AtomicU16::new(0),
            total_ticks: AtomicU64::new(0),
            current_beats_bits: AtomicU64::new(Beats::ZERO.as_beats_f64().to_bits()),
            pending_position_present: AtomicBool::new(false),
            pending_position_bits: AtomicU64::new(0),
        }
    }

    fn load_pending_position(&self) -> Option<Beats> {
        self.pending_position_present
            .load(Ordering::Acquire)
            .then(|| {
                Beats::from_beats_f64(f64::from_bits(
                    self.pending_position_bits.load(Ordering::Acquire),
                ))
            })
    }

    fn store_pending_position(&self, position: Option<Beats>) {
        if let Some(position) = position {
            self.pending_position_bits
                .store(position.as_beats_f64().to_bits(), Ordering::Release);
            self.pending_position_present.store(true, Ordering::Release);
        } else {
            self.pending_position_present
                .store(false, Ordering::Release);
        }
    }

    fn load_current_position(&self) -> Beats {
        Beats::from_beats_f64(f64::from_bits(
            self.current_beats_bits.load(Ordering::Relaxed),
        ))
    }

    fn store_current_position(&self, position: Beats) {
        self.current_beats_bits
            .store(position.as_beats_f64().to_bits(), Ordering::Relaxed);
    }
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
        let config = Arc::new(SequencerConfig::new(chase_notes_on_seek));
        let (scheduler_updates_tx, scheduler_updates_rx) = unbounded();
        Self {
            hot: Arc::new(SequencerHotState::new()),
            config: Arc::new(ArcSwap::from(config.clone())),
            runtime: Arc::new(ArcSwap::from(Arc::new(SequencerRuntime::new(
                config.sequences.len(),
            )))),
            scheduler_updates_tx,
            scheduler_updates_rx: Arc::new(Mutex::new(Some(scheduler_updates_rx))),
        }
    }

    pub(crate) fn scheduler_runner(&self) -> SequencerRunner {
        let updates_rx = self
            .scheduler_updates_rx
            .lock()
            .expect("scheduler update receiver lock should not be poisoned")
            .take()
            .expect("sequencer scheduler runner can only be created once");
        SequencerRunner {
            sequencer: self.clone(),
            config: self.config.load_full(),
            runtime: self.runtime.load_full(),
            updates_rx,
        }
    }

    pub(crate) fn sync_track_handle(
        &self,
        track_id: TrackId,
        handle: Option<InstrumentRuntimeHandle>,
    ) {
        let current = self.config.load_full();
        let mut next = (*current).clone();
        if let Some(slot) = next.instrument_handles.get_mut(track_id.index()) {
            *slot = handle;
        }
        let next = Arc::new(next);
        self.config.store(next.clone());
        let _ = self
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Config(next));
        self.hot.dirty.store(true, Ordering::Release);
    }

    pub(crate) fn configure_schedule_lead(&self, block_size: usize, sample_rate: usize) {
        let current = self.config.load_full();
        let mut next = (*current).clone();
        next.configure_schedule_lead(block_size, sample_rate);
        let next = Arc::new(next);
        self.config.store(next.clone());
        let _ = self
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Config(next));
    }

    pub(crate) fn prepare_for_play(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        _start_beat: Beats,
    ) {
        let config = self.config.load();
        let runtime = self.runtime.load();
        let pending_position = self.hot.load_pending_position();
        let needs_reset_on_play = self.hot.needs_reset_on_play.swap(false, Ordering::AcqRel);
        let position = pending_position.unwrap_or(_start_beat);
        reset_schedule_state_at(&config, &runtime, position);
        if needs_reset_on_play {
            schedule_reset_and_chase_at(&config, &runtime, position, commands);
        } else if config.chase_notes_on_seek {
            dispatch_chase_events_at(&config, &runtime, position, commands);
        }
        schedule_window(
            &config,
            &runtime,
            position,
            position + config.lookahead,
            commands,
            config.initial_frame_offset,
        );
        self.hot.store_pending_position(None);
        self.hot.store_current_position(position);
        self.hot.dirty.store(false, Ordering::Release);
        self.hot.playing.store(true, Ordering::Relaxed);
    }

    pub(crate) fn prepare_for_pause(&self, commands: &mut MultiThreadedKnystCommands, at: Beats) {
        let config = self.config.load();
        let runtime = self.runtime.load();
        schedule_pause_reset_at(&config, &runtime, at, commands);
    }

    pub(crate) fn prepare_for_pause_immediate(&self, commands: &mut MultiThreadedKnystCommands) {
        let config = self.config.load();
        let runtime = self.runtime.load();
        dispatch_immediate_pause_reset(&config, &runtime, commands);
    }

    pub(crate) fn mark_dirty_for_seek(&self, position: Beats, needs_reset_on_play: bool) {
        self.hot.dirty.store(true, Ordering::Release);
        self.hot.store_pending_position(Some(position));
        self.hot.store_current_position(position);
        self.hot
            .needs_reset_on_play
            .store(needs_reset_on_play, Ordering::Release);
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.hot.playing.store(playing, Ordering::Relaxed);
    }

    pub(crate) fn pending_position(&self) -> Option<Beats> {
        self.hot.load_pending_position()
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.hot.playing.load(Ordering::Relaxed)
    }

    pub(crate) fn has_loaded_score(&self) -> bool {
        self.hot.has_loaded_score.load(Ordering::Relaxed)
    }

    pub(crate) fn process_tick(&self, commands: &mut MultiThreadedKnystCommands) {
        if !self.hot.playing.load(Ordering::Relaxed) {
            return;
        }

        let Some(snapshot) = commands.current_transport_snapshot() else {
            return;
        };
        let current_beat = snapshot.beats.unwrap_or(Beats::ZERO);
        self.hot.store_current_position(current_beat);
        self.process_tick_at(commands, current_beat);
    }

    pub(crate) fn process_tick_at(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        current_beat: Beats,
    ) {
        let config = self.config.load();
        let runtime = self.runtime.load();
        let dirty = self.hot.dirty.swap(false, Ordering::AcqRel);
        let pending_position = self.hot.load_pending_position();
        let needs_reset_on_play = if dirty {
            self.hot.needs_reset_on_play.swap(false, Ordering::AcqRel)
        } else {
            false
        };
        if dirty {
            let position = pending_position.unwrap_or(current_beat);
            reset_schedule_state_at(&config, &runtime, position);
            if needs_reset_on_play {
                schedule_reset_and_chase_at(&config, &runtime, position, commands);
            } else if config.chase_notes_on_seek {
                dispatch_chase_events_at(&config, &runtime, position, commands);
            }
            schedule_window(
                &config,
                &runtime,
                position,
                position + config.lookahead,
                commands,
                config.initial_frame_offset,
            );
            self.hot.store_pending_position(None);
            return;
        }

        process_tick_at(&config, &runtime, commands, current_beat);
    }

    #[cfg(test)]
    pub(crate) fn debug_state(&self) -> SequencerDebugState {
        SequencerDebugState {
            reset_count: self.runtime.load().reset_count.load(Ordering::Relaxed),
            schedule_count: self.runtime.load().schedule_count.load(Ordering::Relaxed),
            scheduled_event_count: self
                .runtime
                .load()
                .scheduled_event_count
                .load(Ordering::Relaxed),
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
        let current = self.sequencer.config.load_full();
        let mut next = (*current).clone();
        next.tempo_map = build_tempo_map(&tempo_points);
        next.sequences = sequences;
        next.ppq = ppq;
        next.total_ticks = total_ticks;
        let has_loaded_score = !next.sequences.is_empty();
        self.sequencer.config.store(Arc::new(next.clone()));
        let next = Arc::new(next);
        let runtime = Arc::new(SequencerRuntime::new(next.sequences.len()));
        self.sequencer.config.store(next.clone());
        self.sequencer.runtime.store(runtime.clone());
        let _ = self
            .sequencer
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Both(next.clone(), runtime));
        self.sequencer.hot.ppq.store(ppq, Ordering::Relaxed);
        self.sequencer
            .hot
            .total_ticks
            .store(total_ticks, Ordering::Relaxed);
        self.sequencer
            .hot
            .has_loaded_score
            .store(has_loaded_score, Ordering::Relaxed);
        self.sequencer.hot.store_pending_position(None);
        self.sequencer.hot.store_current_position(Beats::ZERO);
        self.sequencer.hot.dirty.store(true, Ordering::Release);
        Ok(())
    }

    /// Returns the current playback position in MIDI ticks.
    pub fn playback_tick(&mut self) -> Result<u64, SequencerError> {
        let ppq = self.sequencer.hot.ppq.load(Ordering::Relaxed);
        let playing = self.sequencer.hot.playing.load(Ordering::Relaxed);
        let pending_position = self.sequencer.hot.load_pending_position();
        if ppq == 0 {
            return Ok(0);
        }
        if !playing && let Some(position) = pending_position {
            return Ok(beats_to_ticks(position, ppq));
        }
        Ok(beats_to_ticks(
            self.sequencer.hot.load_current_position(),
            ppq,
        ))
    }

    /// Returns whether playback is currently running according to the sequencer hot state.
    #[must_use]
    pub fn playback_is_playing(&self) -> bool {
        self.sequencer.hot.playing.load(Ordering::Relaxed)
    }

    /// Returns the total loaded MIDI duration in ticks.
    #[must_use]
    pub fn total_ticks(&self) -> u64 {
        self.sequencer.hot.total_ticks.load(Ordering::Relaxed)
    }

    /// Clears the loaded score and scheduling state.
    pub fn clear(&mut self) {
        let current = self.sequencer.config.load_full();
        let mut next = (*current).clone();
        next.sequences.clear();
        next.tempo_map = MusicalTimeMap::new();
        next.ppq = 0;
        next.total_ticks = 0;
        let next = Arc::new(next);
        let runtime = Arc::new(SequencerRuntime::new(0));
        self.sequencer.config.store(next.clone());
        self.sequencer.runtime.store(runtime.clone());
        let _ = self
            .sequencer
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Both(next, runtime));
        self.sequencer.hot.ppq.store(0, Ordering::Relaxed);
        self.sequencer.hot.total_ticks.store(0, Ordering::Relaxed);
        self.sequencer
            .hot
            .has_loaded_score
            .store(false, Ordering::Relaxed);
        self.sequencer.hot.store_pending_position(None);
        self.sequencer.hot.store_current_position(Beats::ZERO);
        self.sequencer.hot.dirty.store(true, Ordering::Release);
    }

    fn replace_tempo_map(&mut self, tempos: &[TempoPoint]) {
        let tempo_map = build_tempo_map(tempos);
        self.commands
            .change_musical_time_map(move |map: &mut MusicalTimeMap| {
                *map = tempo_map.clone();
            });
    }
}

pub(crate) struct SequencerRunner {
    pub(crate) sequencer: Sequencer,
    config: Arc<SequencerConfig>,
    runtime: Arc<SequencerRuntime>,
    updates_rx: Receiver<SequencerSchedulerUpdate>,
}

impl SequencerRunner {
    fn drain_updates(&mut self) {
        loop {
            match self.updates_rx.try_recv() {
                Ok(SequencerSchedulerUpdate::Config(config)) => {
                    self.config = config;
                }
                Ok(SequencerSchedulerUpdate::Both(config, runtime)) => {
                    self.config = config;
                    self.runtime = runtime;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(crate) fn process_tick(&mut self, commands: &mut MultiThreadedKnystCommands) {
        self.drain_updates();
        if !self.sequencer.hot.playing.load(Ordering::Relaxed) {
            return;
        }

        let Some(snapshot) = commands.current_transport_snapshot() else {
            return;
        };
        let current_beat = snapshot.beats.unwrap_or(Beats::ZERO);
        self.sequencer.hot.store_current_position(current_beat);
        let dirty = self.sequencer.hot.dirty.swap(false, Ordering::AcqRel);
        let pending_position = self.sequencer.hot.load_pending_position();
        let needs_reset_on_play = if dirty {
            self.sequencer
                .hot
                .needs_reset_on_play
                .swap(false, Ordering::AcqRel)
        } else {
            false
        };
        if dirty {
            let position = pending_position.unwrap_or(current_beat);
            reset_schedule_state_at(&self.config, &self.runtime, position);
            if needs_reset_on_play {
                schedule_reset_and_chase_at(&self.config, &self.runtime, position, commands);
            } else if self.config.chase_notes_on_seek {
                dispatch_chase_events_at(&self.config, &self.runtime, position, commands);
            }
            schedule_window(
                &self.config,
                &self.runtime,
                position,
                position + self.config.lookahead,
                commands,
                self.config.initial_frame_offset,
            );
            self.sequencer.hot.store_pending_position(None);
            return;
        }

        process_tick_at(&self.config, &self.runtime, commands, current_beat);
    }
}

fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    let _ = receiver.recv_timeout(CONTROLLER_BARRIER_TIMEOUT);
}

#[derive(Clone)]
struct SequencerConfig {
    sequences: Vec<Sequence>,
    tempo_map: MusicalTimeMap,
    ppq: u16,
    total_ticks: u64,
    sample_rate: f64,
    lookahead: Beats,
    refill_margin: Beats,
    reset_frame_offset: i32,
    chase_frame_offset: i32,
    initial_frame_offset: i32,
    instrument_handles: Vec<Option<InstrumentRuntimeHandle>>,
    chase_notes_on_seek: bool,
}

struct SequencerRuntime {
    generations: Box<[AtomicU32]>,
    next_indices: Box<[AtomicUsize]>,
    scheduled_until_bits: AtomicU64,
    #[cfg(test)]
    reset_count: AtomicUsize,
    #[cfg(test)]
    schedule_count: AtomicUsize,
    #[cfg(test)]
    scheduled_event_count: AtomicUsize,
}

impl SequencerConfig {
    fn new(chase_notes_on_seek: bool) -> Self {
        Self {
            sequences: Vec::new(),
            tempo_map: MusicalTimeMap::new(),
            ppq: 0,
            total_ticks: 0,
            sample_rate: 44_100.0,
            lookahead: Beats::from_beats(8),
            refill_margin: Beats::from_beats(2),
            reset_frame_offset: 256,
            chase_frame_offset: 384,
            initial_frame_offset: 512,
            instrument_handles: vec![None; INSTRUMENT_TRACK_COUNT],
            chase_notes_on_seek,
        }
    }

    fn configure_schedule_lead(&mut self, block_size: usize, sample_rate: usize) {
        let block = block_size.max(64) as i32;
        self.sample_rate = sample_rate.max(1) as f64;
        self.reset_frame_offset = block * 64;
        self.chase_frame_offset = block * 72;
        self.initial_frame_offset = block * 80;
    }
}

impl SequencerRuntime {
    fn new(sequence_count: usize) -> Self {
        Self {
            generations: (0..INSTRUMENT_TRACK_COUNT)
                .map(|_| AtomicU32::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            next_indices: (0..sequence_count)
                .map(|_| AtomicUsize::new(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            scheduled_until_bits: AtomicU64::new(Beats::ZERO.as_beats_f64().to_bits()),
            #[cfg(test)]
            reset_count: AtomicUsize::new(0),
            #[cfg(test)]
            schedule_count: AtomicUsize::new(0),
            #[cfg(test)]
            scheduled_event_count: AtomicUsize::new(0),
        }
    }

    fn load_scheduled_until(&self) -> Beats {
        Beats::from_beats_f64(f64::from_bits(
            self.scheduled_until_bits.load(Ordering::Relaxed),
        ))
    }

    fn store_scheduled_until(&self, position: Beats) {
        self.scheduled_until_bits
            .store(position.as_beats_f64().to_bits(), Ordering::Relaxed);
    }
}

fn process_tick_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    commands: &mut MultiThreadedKnystCommands,
    current_beat: Beats,
) {
    if runtime.load_scheduled_until() > current_beat + config.refill_margin {
        return;
    }

    let window_start = runtime.load_scheduled_until().max(current_beat);
    schedule_window(
        config,
        runtime,
        window_start,
        window_start + config.lookahead,
        commands,
        0,
    );
}

fn schedule_window(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    window_start: Beats,
    window_end: Beats,
    commands: &mut MultiThreadedKnystCommands,
    initial_frame_offset: i32,
) {
    #[cfg(test)]
    {
        runtime.schedule_count.fetch_add(1, Ordering::Relaxed);
    }

    let sample_rate = config.sample_rate;
    let tempo_map = &config.tempo_map;
    for (sequence_index, sequence) in config.sequences.iter().enumerate() {
        let Some(handle) = config
            .instrument_handles
            .get(sequence.target_track.index())
            .copied()
            .flatten()
        else {
            continue;
        };
        let next_index = &runtime.next_indices[sequence_index];
        let mut next = next_index.load(Ordering::Relaxed);
        while let Some(first_event) = sequence.events.get(next) {
            if first_event.at > window_end {
                break;
            }

            let event_time = first_event.at;
            let group_start = next;
            while let Some(next_event) = sequence.events.get(next) {
                if next_event.at != event_time {
                    break;
                }
                next += 1;
            }

            if event_time < window_start {
                continue;
            }

            let mut frame_offset = if event_time == window_start {
                initial_frame_offset
            } else {
                0
            };
            for timed_event in ordered_events_at_same_time(&sequence.events[group_start..next]) {
                #[cfg(test)]
                {
                    runtime
                        .scheduled_event_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                handle.schedule_midi_at_with_offset(
                    commands,
                    offset_beats(tempo_map, sample_rate, timed_event.at, frame_offset),
                    runtime.generations[sequence.target_track.index()].load(Ordering::Relaxed),
                    timed_event.event,
                );
                frame_offset += 2;
            }
        }
        next_index.store(next, Ordering::Relaxed);
    }

    runtime.store_scheduled_until(window_end);
}

fn reset_schedule_state_at(config: &SequencerConfig, runtime: &SequencerRuntime, at: Beats) {
    for (sequence_index, sequence) in config.sequences.iter().enumerate() {
        runtime.next_indices[sequence_index].store(
            first_event_at_or_after(&sequence.events, at),
            Ordering::Relaxed,
        );
    }
    runtime.store_scheduled_until(at);
}

fn schedule_pause_reset_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    commands: &mut MultiThreadedKnystCommands,
) {
    #[cfg(test)]
    {
        runtime.reset_count.fetch_add(1, Ordering::Relaxed);
    }
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let generation = runtime.generations[track.index()]
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let Some(handle) = config
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
                &config.tempo_map,
                config.sample_rate,
                at,
                config.reset_frame_offset,
            ),
            generation,
        );
        dispatch_scheduled_panic_events(
            handle,
            generation,
            commands,
            &config.tempo_map,
            config.sample_rate,
            at,
            config.reset_frame_offset + 2,
        );
    }
}

fn dispatch_chase_events_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    commands: &mut MultiThreadedKnystCommands,
) {
    for sequence in &config.sequences {
        let Some(handle) = config
            .instrument_handles
            .get(sequence.target_track.index())
            .copied()
            .flatten()
        else {
            continue;
        };
        let generation = runtime.generations[sequence.target_track.index()].load(Ordering::Relaxed);
        let mut frame_offset = config.chase_frame_offset;
        for event in chase_events_at(&sequence.events, at) {
            handle.schedule_midi_at_with_offset(
                commands,
                offset_beats(&config.tempo_map, config.sample_rate, at, frame_offset),
                generation,
                event,
            );
            frame_offset += 2;
        }
    }
}

fn schedule_reset_and_chase_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    commands: &mut MultiThreadedKnystCommands,
) {
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let generation = runtime.generations[track.index()]
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let Some(handle) = config
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
                &config.tempo_map,
                config.sample_rate,
                at,
                config.reset_frame_offset,
            ),
            generation,
        );
        dispatch_scheduled_panic_events(
            handle,
            generation,
            commands,
            &config.tempo_map,
            config.sample_rate,
            at,
            config.reset_frame_offset + 2,
        );
    }
    if config.chase_notes_on_seek {
        dispatch_chase_events_at(config, runtime, at, commands);
    }
}

fn dispatch_immediate_pause_reset(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    commands: &mut MultiThreadedKnystCommands,
) {
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let generation = runtime.generations[track.index()]
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let Some(handle) = config
            .instrument_handles
            .get(track.index())
            .copied()
            .flatten()
        else {
            continue;
        };
        handle.send_reset_live(commands, generation);
        dispatch_immediate_panic_events(handle, generation, commands);
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

        let config = sequencer.config.load_full();
        assert_eq!(config.sequences.len(), 1);
        assert_eq!(config.sequences[0].target_track, TrackId(0));
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
}
