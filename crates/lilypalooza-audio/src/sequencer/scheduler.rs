use super::{midi_events::*, *};

pub(super) const CONTROLLER_BARRIER_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(250);

pub(super) fn send_scheduler_update(
    sender: &Sender<SequencerSchedulerUpdate>,
    update: SequencerSchedulerUpdate,
) {
    match sender.send(update) {
        Ok(()) | Err(_) => {}
    }
}

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
    pub(super) config: Arc<ArcSwap<SequencerConfig>>,
    runtime: Arc<ArcSwap<SequencerRuntime>>,
    scheduler_updates_tx: Sender<SequencerSchedulerUpdate>,
    scheduler_updates_rx: Arc<Mutex<Option<Receiver<SequencerSchedulerUpdate>>>>,
}

impl std::fmt::Debug for Sequencer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sequencer")
            .field("playing", &self.is_playing())
            .field("has_loaded_score", &self.has_loaded_score())
            .finish_non_exhaustive()
    }
}

pub(super) enum SequencerSchedulerUpdate {
    Config(Arc<SequencerConfig>),
    Both(Arc<SequencerConfig>, Arc<SequencerRuntime>),
}

pub(super) struct SequencerHotState {
    pub(super) playing: AtomicBool,
    pub(super) dirty: AtomicBool,
    pub(super) needs_reset_on_play: AtomicBool,
    pub(super) has_loaded_score: AtomicBool,
    pub(super) ppq: AtomicU16,
    pub(super) total_ticks: AtomicU64,
    pub(super) current_beats_bits: AtomicU64,
    pub(super) pending_position_present: AtomicBool,
    pub(super) pending_position_bits: AtomicU64,
}

impl SequencerHotState {
    pub(super) fn new() -> Self {
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

    pub(super) fn load_pending_position(&self) -> Option<Beats> {
        self.pending_position_present
            .load(Ordering::Acquire)
            .then(|| {
                Beats::from_beats_f64(f64::from_bits(
                    self.pending_position_bits.load(Ordering::Acquire),
                ))
            })
    }

    pub(super) fn store_pending_position(&self, position: Option<Beats>) {
        if let Some(position) = position {
            self.pending_position_bits
                .store(position.as_beats_f64().to_bits(), Ordering::Release);
            self.pending_position_present.store(true, Ordering::Release);
        } else {
            self.pending_position_present
                .store(false, Ordering::Release);
        }
    }

    pub(super) fn load_current_position(&self) -> Beats {
        Beats::from_beats_f64(f64::from_bits(
            self.current_beats_bits.load(Ordering::Relaxed),
        ))
    }

    pub(super) fn store_current_position(&self, position: Beats) {
        self.current_beats_bits
            .store(position.as_beats_f64().to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct SequencerDebugState {
    pub reset_count: usize,
    pub schedule_count: usize,
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
        send_scheduler_update(
            &self.scheduler_updates_tx,
            SequencerSchedulerUpdate::Config(next),
        );
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
        send_scheduler_update(
            &self.scheduler_updates_tx,
            SequencerSchedulerUpdate::Config(next),
        );
    }

    pub(crate) fn set_metronome_enabled(&self, enabled: bool) {
        let current = self.config.load_full();
        let mut next = (*current).clone();
        next.metronome_enabled = enabled;
        let next = Arc::new(next);
        self.config.store(next.clone());
        send_scheduler_update(
            &self.scheduler_updates_tx,
            SequencerSchedulerUpdate::Config(next),
        );
    }

    pub(crate) fn configure_schedule_lead(&self, block_size: usize, sample_rate: usize) {
        let current = self.config.load_full();
        let mut next = (*current).clone();
        next.configure_schedule_lead(block_size, sample_rate);
        let next = Arc::new(next);
        self.config.store(next.clone());
        send_scheduler_update(
            &self.scheduler_updates_tx,
            SequencerSchedulerUpdate::Config(next),
        );
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
        }
    }
}

/// Mutable sequencer control handle.
pub struct SequencerHandle<'a> {
    sequencer: &'a Sequencer,
    commands: &'a mut MultiThreadedKnystCommands,
}

impl std::fmt::Debug for SequencerHandle<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SequencerHandle")
            .field("sequencer", &self.sequencer)
            .finish_non_exhaustive()
    }
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
        send_scheduler_update(
            &self.sequencer.scheduler_updates_tx,
            SequencerSchedulerUpdate::Both(next.clone(), runtime),
        );
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
        send_scheduler_update(
            &self.sequencer.scheduler_updates_tx,
            SequencerSchedulerUpdate::Both(next, runtime),
        );
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

    pub(super) fn replace_tempo_map(&mut self, tempos: &[TempoPoint]) {
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
    pub(super) fn drain_updates(&mut self) {
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

pub(super) fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    match receiver.recv_timeout(CONTROLLER_BARRIER_TIMEOUT) {
        Ok(_) | Err(_) => {}
    }
}

#[derive(Clone)]
pub(super) struct SequencerConfig {
    pub(super) sequences: Vec<Sequence>,
    pub(super) tempo_map: MusicalTimeMap,
    pub(super) ppq: u16,
    pub(super) total_ticks: u64,
    pub(super) time_signatures: Vec<TimeSignaturePoint>,
    pub(super) sample_rate: f64,
    pub(super) reset_frame_offset: i32,
    pub(super) chase_frame_offset: i32,
    pub(super) instrument_handles: Vec<Option<InstrumentRuntimeHandle>>,
    pub(super) metronome_handle: Option<InstrumentRuntimeHandle>,
    pub(super) metronome_enabled: bool,
    pub(super) chase_notes_on_seek: bool,
}

pub(super) struct SequencerRuntime {
    pub(super) generations: Box<[AtomicU32]>,
    pub(super) next_indices: Box<[AtomicUsize]>,
    #[cfg(test)]
    pub(super) reset_count: AtomicUsize,
    #[cfg(test)]
    pub(super) schedule_count: AtomicUsize,
    #[cfg(test)]
    pub(super) scheduled_event_count: AtomicUsize,
}

impl SequencerConfig {
    pub(super) fn new(chase_notes_on_seek: bool) -> Self {
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

    pub(super) fn configure_schedule_lead(&mut self, block_size: usize, sample_rate: usize) {
        let block = block_size.max(64) as i32;
        self.sample_rate = sample_rate.max(1) as f64;
        self.reset_frame_offset = block * 64;
        self.chase_frame_offset = block * 72;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimeSignaturePoint {
    pub(super) at: Beats,
    pub(super) numerator: u8,
    pub(super) denominator: u8,
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
pub(super) struct MetronomeClick {
    pub(super) at: Beats,
    pub(super) accented: bool,
}

impl SequencerRuntime {
    pub(super) fn new(sequence_count: usize) -> Self {
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

pub(super) fn collect_block_window(
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
        let Some(generation) = runtime.generations.get(sequence.target_track.index()) else {
            continue;
        };
        let generation = generation.load(Ordering::Relaxed);
        let Some(next_index) = runtime.next_indices.get(sequence_index) else {
            continue;
        };
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
            let Some(group) = sequence.events.get(group_start..next) else {
                continue;
            };
            for timed_event in ordered_events_at_same_time(group) {
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

pub(super) fn collect_metronome_block_window(
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
