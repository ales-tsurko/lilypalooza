//! MIDI-track sequencer and score scheduler.

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use knyst::prelude::{
    Beats, KnystCommands, MultiThreadedKnystCommands, SchedulerChange, SchedulerExtension,
    SchedulerExtensionContext, TransportState,
};
use knyst::scheduling::{MusicalTimeMap, TempoChange};
use knyst::time::SUBBEAT_TESIMALS_PER_BEAT;
use knyst::time::Seconds;
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

    pub(crate) fn scheduler_extension(&self) -> SequencerSchedulerExtension {
        let updates_rx = self
            .scheduler_updates_rx
            .lock()
            .ok()
            .and_then(|mut receiver| receiver.take())
            .unwrap_or_else(|| {
                let (_tx, rx) = crossbeam_channel::unbounded();
                rx
            });
        SequencerSchedulerExtension {
            sequencer: self.clone(),
            config: self.config.load_full(),
            runtime: self.runtime.load_full(),
            updates_rx,
        }
    }

    pub(crate) fn sync_track_handle(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        track_id: TrackId,
        mut handle: Option<InstrumentRuntimeHandle>,
    ) {
        if let Some(handle_ref) = handle.as_mut() {
            handle_ref.resolve_scheduler_event_target(commands);
        }
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

    pub(crate) fn sync_metronome_handle(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        mut handle: Option<InstrumentRuntimeHandle>,
    ) {
        if let Some(handle_ref) = handle.as_mut() {
            handle_ref.resolve_scheduler_event_target(commands);
        }
        let current = self.config.load_full();
        let mut next = (*current).clone();
        next.metronome_handle = handle;
        let next = Arc::new(next);
        self.config.store(next.clone());
        let _ = self
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Config(next));
    }

    pub(crate) fn set_metronome_enabled(&self, enabled: bool) {
        let current = self.config.load_full();
        let mut next = (*current).clone();
        next.metronome_enabled = enabled;
        let next = Arc::new(next);
        self.config.store(next.clone());
        let _ = self
            .scheduler_updates_tx
            .send(SequencerSchedulerUpdate::Config(next));
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
        _commands: &mut MultiThreadedKnystCommands,
        start_beat: Beats,
    ) {
        let position = self.hot.load_pending_position().unwrap_or(start_beat);
        self.hot.store_current_position(position);
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
        let mut time_signatures = Vec::new();
        let mut total_ticks = 0_u64;

        for track in &smf.tracks {
            if let Some(sequence) = Sequence::from_midi_track(sequences.len(), track, ppq) {
                sequences.push(sequence);
            }

            let mut absolute_ticks = 0_u64;
            for event in track {
                absolute_ticks = absolute_ticks.saturating_add(u64::from(event.delta.as_int()));
                total_ticks = total_ticks.max(absolute_ticks);
                match event.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(micros_per_quarter)) => {
                        let micros_per_quarter = micros_per_quarter.as_int();
                        if micros_per_quarter == 0 {
                            continue;
                        }
                        tempo_points.push(TempoPoint {
                            at: ticks_to_beats(absolute_ticks, ppq),
                            bpm: 60_000_000.0 / f64::from(micros_per_quarter),
                        });
                    }
                    TrackEventKind::Meta(MetaMessage::TimeSignature(
                        numerator,
                        denominator_power,
                        _,
                        _,
                    )) => {
                        let denominator =
                            1_u16.checked_shl(u32::from(denominator_power)).unwrap_or(0);
                        if denominator != 0 {
                            time_signatures.push(TimeSignaturePoint {
                                at: ticks_to_beats(absolute_ticks, ppq),
                                numerator,
                                denominator: denominator.min(u16::from(u8::MAX)) as u8,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        self.replace_tempo_map(&tempo_points);
        wait_for_controller_barrier(self.commands);
        let current = self.sequencer.config.load_full();
        let mut next = (*current).clone();
        next.tempo_map = build_tempo_map(&tempo_points);
        next.time_signatures = normalized_time_signatures(&time_signatures);
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
        next.time_signatures = vec![TimeSignaturePoint::default()];
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

pub(crate) struct SequencerSchedulerExtension {
    sequencer: Sequencer,
    config: Arc<SequencerConfig>,
    runtime: Arc<SequencerRuntime>,
    updates_rx: Receiver<SequencerSchedulerUpdate>,
}

impl SequencerSchedulerExtension {
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
}

impl SchedulerExtension for SequencerSchedulerExtension {
    fn collect_block_changes(
        &mut self,
        ctx: &SchedulerExtensionContext,
        out: &mut Vec<SchedulerChange>,
    ) {
        self.drain_updates();
        if !self.sequencer.hot.playing.load(Ordering::Relaxed) {
            return;
        }
        let Some(transport) = ctx.transport else {
            return;
        };
        if transport.state != TransportState::Playing {
            return;
        }
        let current_beat = transport.beats.unwrap_or(Beats::ZERO);
        let mut block_start_beat = current_beat;
        let mut block_start_seconds = transport.seconds.to_seconds_f64();
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
            let position =
                pending_position.unwrap_or_else(|| self.sequencer.hot.load_current_position());
            reset_schedule_state_at(&self.config, &self.runtime, position);
            block_start_beat = position;
            block_start_seconds = self.config.tempo_map.musical_time_to_secs_f64(position);
            if needs_reset_on_play {
                collect_reset_and_chase_at(&self.config, &self.runtime, position, out);
            } else if self.config.chase_notes_on_seek {
                collect_chase_events_at(&self.config, &self.runtime, position, 0, out);
            }
            self.sequencer.hot.store_pending_position(None);
        }

        collect_block_window(
            &self.config,
            &self.runtime,
            block_start_beat,
            block_start_seconds,
            ctx.block_size,
            ctx.sample_rate as f64,
            out,
        );
        collect_metronome_block_window(
            &self.config,
            block_start_beat,
            block_start_seconds,
            ctx.block_size,
            ctx.sample_rate as f64,
            out,
        );
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
    time_signatures: Vec<TimeSignaturePoint>,
    sample_rate: f64,
    reset_frame_offset: i32,
    chase_frame_offset: i32,
    instrument_handles: Vec<Option<InstrumentRuntimeHandle>>,
    metronome_handle: Option<InstrumentRuntimeHandle>,
    metronome_enabled: bool,
    chase_notes_on_seek: bool,
}

struct SequencerRuntime {
    generations: Box<[AtomicU32]>,
    next_indices: Box<[AtomicUsize]>,
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
            time_signatures: vec![TimeSignaturePoint::default()],
            sample_rate: 44_100.0,
            reset_frame_offset: 256,
            chase_frame_offset: 384,
            instrument_handles: vec![None; INSTRUMENT_TRACK_COUNT],
            metronome_handle: None,
            metronome_enabled: false,
            chase_notes_on_seek,
        }
    }

    fn configure_schedule_lead(&mut self, block_size: usize, sample_rate: usize) {
        let block = block_size.max(64) as i32;
        self.sample_rate = sample_rate.max(1) as f64;
        self.reset_frame_offset = block * 64;
        self.chase_frame_offset = block * 72;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TimeSignaturePoint {
    at: Beats,
    numerator: u8,
    denominator: u8,
}

impl Default for TimeSignaturePoint {
    fn default() -> Self {
        Self {
            at: Beats::ZERO,
            numerator: 4,
            denominator: 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MetronomeClick {
    at: Beats,
    accented: bool,
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
            #[cfg(test)]
            reset_count: AtomicUsize::new(0),
            #[cfg(test)]
            schedule_count: AtomicUsize::new(0),
            #[cfg(test)]
            scheduled_event_count: AtomicUsize::new(0),
        }
    }
}

fn collect_block_window(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    _block_start_beat: Beats,
    block_start_seconds: f64,
    block_size: usize,
    sample_rate: f64,
    out: &mut Vec<SchedulerChange>,
) {
    #[cfg(test)]
    {
        runtime.schedule_count.fetch_add(1, Ordering::Relaxed);
    }

    let block_end_seconds = block_start_seconds + block_size as f64 / sample_rate.max(1.0);

    for (sequence_index, sequence) in config.sequences.iter().enumerate() {
        let Some(handle) = config
            .instrument_handles
            .get(sequence.target_track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        let generation = runtime.generations[sequence.target_track.index()].load(Ordering::Relaxed);
        let next_index = &runtime.next_indices[sequence_index];
        let mut next = next_index.load(Ordering::Relaxed);

        while let Some(event) = sequence.events.get(next) {
            let event_seconds = config.tempo_map.musical_time_to_secs_f64(event.at);
            if event_seconds >= block_end_seconds {
                break;
            }
            if event_seconds < block_start_seconds {
                next += 1;
                continue;
            }

            let group_start = next;
            while let Some(next_event) = sequence.events.get(next) {
                if next_event.at != event.at {
                    break;
                }
                next += 1;
            }

            let sample_offset =
                block_sample_offset(block_start_seconds, event_seconds, sample_rate, block_size);
            for timed_event in ordered_events_at_same_time(&sequence.events[group_start..next]) {
                #[cfg(test)]
                {
                    runtime
                        .scheduled_event_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                if let Some(change) =
                    handle.scheduler_midi_change(sample_offset, generation, timed_event.event)
                {
                    out.push(change);
                }
            }
        }

        next_index.store(next, Ordering::Relaxed);
    }
}

fn collect_metronome_block_window(
    config: &SequencerConfig,
    block_start_beat: Beats,
    block_start_seconds: f64,
    block_size: usize,
    sample_rate: f64,
    out: &mut Vec<SchedulerChange>,
) {
    if !config.metronome_enabled {
        return;
    }
    let Some(handle) = config.metronome_handle.as_ref() else {
        return;
    };
    let block_end_seconds = block_start_seconds + block_size as f64 / sample_rate.max(1.0);
    let block_end_beat = config
        .tempo_map
        .seconds_to_beats(Seconds::from_seconds_f64(block_end_seconds));

    for click in metronome_clicks_between(&config.time_signatures, block_start_beat, block_end_beat)
    {
        let click_seconds = config.tempo_map.musical_time_to_secs_f64(click.at);
        let sample_offset =
            block_sample_offset(block_start_seconds, click_seconds, sample_rate, block_size);
        let velocity = if click.accented { 127 } else { 100 };
        if let Some(change) = handle.scheduler_midi_change(
            sample_offset,
            0,
            EngineMidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity,
            },
        ) {
            out.push(change);
        }
    }
}

fn metronome_clicks_between(
    time_signatures: &[TimeSignaturePoint],
    start_beat: Beats,
    end_beat: Beats,
) -> Vec<MetronomeClick> {
    let start = start_beat.as_beats_f64();
    let end = end_beat.as_beats_f64();
    if end <= start {
        return Vec::new();
    }

    let mut clicks = Vec::new();
    for (index, signature) in time_signatures.iter().enumerate() {
        let segment_start = signature.at.as_beats_f64();
        let segment_end = time_signatures
            .get(index + 1)
            .map(|next| next.at.as_beats_f64())
            .unwrap_or(f64::INFINITY);
        let window_start = start.max(segment_start);
        let window_end = end.min(segment_end);
        if window_end <= window_start {
            continue;
        }

        let beat_unit = 4.0 / f64::from(signature.denominator.max(1));
        let beats_from_segment = (window_start - segment_start) / beat_unit;
        let mut step_index = beats_from_segment.ceil() as i64;
        if step_index < 0 {
            step_index = 0;
        }
        loop {
            let click_beat = segment_start + step_index as f64 * beat_unit;
            if click_beat >= window_end {
                break;
            }
            if click_beat + 1.0e-9 >= window_start {
                clicks.push(MetronomeClick {
                    at: Beats::from_beats_f64(click_beat),
                    accented: step_index % i64::from(signature.numerator.max(1)) == 0,
                });
            }
            step_index += 1;
        }
    }
    clicks
}

fn normalized_time_signatures(points: &[TimeSignaturePoint]) -> Vec<TimeSignaturePoint> {
    let mut points = points.to_vec();
    if !points.iter().any(|point| point.at == Beats::ZERO) {
        points.push(TimeSignaturePoint::default());
    }
    points.sort_by_key(|point| point.at);
    let mut normalized = Vec::with_capacity(points.len());
    for point in points {
        if normalized
            .last()
            .is_some_and(|last: &TimeSignaturePoint| last.at == point.at)
        {
            normalized.pop();
        }
        normalized.push(point);
    }
    normalized
}

fn reset_schedule_state_at(config: &SequencerConfig, runtime: &SequencerRuntime, at: Beats) {
    for (sequence_index, sequence) in config.sequences.iter().enumerate() {
        runtime.next_indices[sequence_index].store(
            first_event_at_or_after(&sequence.events, at),
            Ordering::Relaxed,
        );
    }
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
            .cloned()
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

fn collect_chase_events_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    start_sample_offset: usize,
    out: &mut Vec<SchedulerChange>,
) {
    for sequence in &config.sequences {
        let Some(handle) = config
            .instrument_handles
            .get(sequence.target_track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        let generation = runtime.generations[sequence.target_track.index()].load(Ordering::Relaxed);
        let mut sample_offset = start_sample_offset;
        for event in chase_events_at(&sequence.events, at) {
            if let Some(change) = handle.scheduler_midi_change(sample_offset, generation, event) {
                out.push(change);
            }
            sample_offset = sample_offset.saturating_add(2);
        }
    }
}

fn collect_reset_and_chase_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    out: &mut Vec<SchedulerChange>,
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
            .cloned()
            .flatten()
        else {
            continue;
        };
        if let Some(change) = handle.scheduler_reset_change(0, generation) {
            out.push(change);
        }
        collect_block_panic_events(handle, generation, 2, out);
    }
    if config.chase_notes_on_seek {
        collect_chase_events_at(config, runtime, at, 0, out);
    }
}

fn dispatch_immediate_pause_reset(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    _commands: &mut MultiThreadedKnystCommands,
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
            .cloned()
            .flatten()
        else {
            continue;
        };
        handle.request_reset_now(generation);
    }
}

fn collect_block_panic_events(
    handle: InstrumentRuntimeHandle,
    generation: u32,
    start_sample_offset: usize,
    out: &mut Vec<SchedulerChange>,
) {
    let mut sample_offset = start_sample_offset;
    for channel in 0..16_u8 {
        for event in [
            EngineMidiEvent::AllSoundOff { channel },
            EngineMidiEvent::AllNotesOff { channel },
            EngineMidiEvent::ResetAllControllers { channel },
        ] {
            if let Some(change) = handle.scheduler_midi_change(sample_offset, generation, event) {
                out.push(change);
            }
            sample_offset = sample_offset.saturating_add(2);
        }
    }
}

fn block_sample_offset(
    block_start_seconds: f64,
    event_seconds: f64,
    sample_rate: f64,
    block_size: usize,
) -> usize {
    (((event_seconds - block_start_seconds) * sample_rate).round() as isize)
        .clamp(0, block_size.saturating_sub(1) as isize) as usize
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
    use knyst::prelude::Beats;

    use super::{
        MetronomeClick, Sequencer, TimeSignaturePoint, TimedMidiEvent, beats_to_ticks,
        metronome_clicks_between, normalized_time_signatures, ordered_events_at_same_time,
        ticks_to_beats,
    };
    use crate::instrument::{BUILTIN_SOUNDFONT_ID, MidiEvent, SlotState, soundfont_synth};
    use crate::mixer::{Mixer, MixerState, TrackId};
    use crate::test_utils::{OfflineHarness, simple_midi_bytes, test_soundfont_resource};

    fn soundfont_slot(program: u8) -> SlotState {
        SlotState::built_in(
            BUILTIN_SOUNDFONT_ID,
            soundfont_synth::state("default", 0, program),
        )
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
}
