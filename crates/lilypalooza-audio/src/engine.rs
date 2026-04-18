//! Top-level audio engine state.
#![allow(missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use audio_thread_priority::promote_current_thread_to_real_time;
use knyst::audio_backend::{AudioBackend, AudioBackendError};
use knyst::audio_backend::{CpalBackend, CpalBackendOptions};
use knyst::modal_interface::{KnystContext, SphereError};
use knyst::prelude::{
    Beats, KnystCommands, KnystSphere, MultiThreadedKnystCommands, SphereSettings,
};

use crate::mixer::{Mixer, MixerError, MixerHandle, MixerMeterSnapshot, MixerState};
use crate::sequencer::{Sequencer, SequencerError, SequencerHandle};
use crate::transport::Transport;

/// Startup options for the audio engine.
#[derive(Debug, Clone, Default)]
pub struct AudioEngineOptions {
    /// Knyst sphere settings.
    pub sphere_settings: SphereSettings,
    /// Whether seeking should chase already-held notes into the new position.
    pub chase_notes_on_seek: bool,
}

/// Shared runtime settings for the audio engine and mixer.
#[derive(Debug, Clone, Copy)]
pub struct AudioEngineSettings {
    /// Backend sample rate.
    pub sample_rate: usize,
    /// Backend block size.
    pub block_size: usize,
}

/// Top-level audio engine container.
pub struct AudioEngine {
    mixer: Mixer,
    sequencer: Sequencer,
    runtime_dirty: Arc<AtomicBool>,
    scheduler_shutdown: Arc<AtomicBool>,
    scheduler_thread: Option<JoinHandle<()>>,
    backend: Box<dyn AudioBackend>,
    settings: AudioEngineSettings,
    // Keeps the Knyst runtime alive; context and commands do not own it.
    #[allow(dead_code)]
    sphere: KnystSphere,
    context: KnystContext,
    commands: MultiThreadedKnystCommands,
}

/// Errors returned by the audio engine facade.
#[derive(thiserror::Error, Debug)]
pub enum AudioEngineError {
    /// Failed to create or start the audio backend.
    #[error(transparent)]
    AudioBackend(#[from] AudioBackendError),
    /// Failed to initialize or run the Knyst sphere.
    #[error(transparent)]
    Sphere(#[from] SphereError),
    /// Failed to attach or update the runtime mixer.
    #[error("mixer runtime error: {0}")]
    MixerRuntime(String),
    /// Failed to access or update mixer state.
    #[error(transparent)]
    Mixer(#[from] MixerError),
    /// Failed to start the scheduler thread.
    #[error(transparent)]
    Thread(#[from] std::io::Error),
}

impl AudioEngine {
    /// Starts the audio engine on the default CPAL output backend.
    pub fn start_cpal(
        mixer: MixerState,
        options: AudioEngineOptions,
    ) -> Result<Self, AudioEngineError> {
        let backend = CpalBackend::new(CpalBackendOptions::default())?;
        Self::start(mixer, backend, options)
    }

    /// Starts the audio engine on the provided backend.
    pub fn start<B: AudioBackend + 'static>(
        mixer: MixerState,
        mut backend: B,
        options: AudioEngineOptions,
    ) -> Result<Self, AudioEngineError> {
        let settings = AudioEngineSettings {
            sample_rate: backend.sample_rate(),
            block_size: backend.block_size().unwrap_or(64),
        };
        let (sphere, controller) =
            KnystSphere::start_return_controller(&mut backend, options.sphere_settings, |_| {})?;
        let context = sphere.context();
        let mut commands = controller.start_on_new_thread();
        commands.transport_pause();
        commands.transport_seek_to_beats(Beats::ZERO);
        wait_for_transport_reset(&mut commands);
        let mixer = Mixer::new(&context, &mut commands, &settings, mixer)?;
        let sequencer = Sequencer::new(options.chase_notes_on_seek);
        sequencer.configure_schedule_lead(settings.block_size, settings.sample_rate);
        let runtime_dirty = Arc::new(AtomicBool::new(true));
        for track in mixer.state.tracks() {
            sequencer.sync_track_handle(track.id, mixer.instrument_handle(track.id));
        }
        let scheduler_shutdown = Arc::new(AtomicBool::new(false));
        let scheduler_thread = Some(start_scheduler_thread(
            context.commands(),
            sequencer.clone(),
            scheduler_shutdown.clone(),
            settings,
        )?);
        Ok(Self {
            mixer,
            sequencer,
            runtime_dirty,
            scheduler_shutdown,
            scheduler_thread,
            backend: Box::new(backend),
            settings,
            sphere,
            context,
            commands,
        })
    }

    /// Returns the shared runtime settings.
    pub fn settings(&self) -> AudioEngineSettings {
        self.settings
    }

    /// Returns the transport control handle.
    pub fn transport(&mut self) -> Transport<'_> {
        Transport::new(
            &mut self.commands,
            Some(&mut self.mixer),
            Some(&self.sequencer),
            Some(self.runtime_dirty.as_ref()),
        )
    }

    /// Returns the mixer control handle.
    pub fn mixer(&mut self) -> MixerHandle<'_> {
        MixerHandle::new(
            &mut self.mixer,
            &self.sequencer,
            self.runtime_dirty.as_ref(),
            &self.context,
            &mut self.commands,
        )
    }

    pub fn mixer_state(&self) -> &MixerState {
        &self.mixer.state
    }

    /// Returns the sequencer control handle.
    pub fn sequencer(&mut self) -> SequencerHandle<'_> {
        SequencerHandle::new(&self.sequencer, &mut self.commands)
    }

    pub fn meter_snapshot(&self) -> MixerMeterSnapshot {
        self.mixer.meter_snapshot()
    }

    pub fn clear_score(&mut self) {
        self.prepare_for_score_reload();
        self.sequencer().clear();
    }

    pub fn replace_score_from_midi_bytes(&mut self, bytes: &[u8]) -> Result<(), SequencerError> {
        self.prepare_for_score_reload();
        {
            let mut sequencer = self.sequencer();
            sequencer.clear();
            sequencer.replace_from_midi_bytes(bytes)?;
        }
        Ok(())
    }

    /// Flushes pending runtime configuration changes through the running graph.
    pub fn flush(&mut self) {
        self.runtime_dirty.store(false, Ordering::Release);
        self.commands.transport_play();
        thread::sleep(Duration::from_millis(25));
        self.commands.transport_pause();
        self.commands.transport_seek_to_beats(Beats::ZERO);
        wait_for_transport_reset(&mut self.commands);
    }

    fn prepare_for_score_reload(&mut self) {
        let has_loaded_score = self.sequencer.has_loaded_score();
        let current_beat = self
            .commands
            .current_transport_snapshot()
            .and_then(|snapshot| snapshot.beats)
            .unwrap_or(Beats::ZERO);

        self.sequencer.set_playing(false);
        self.mixer.reset_meters();

        if !has_loaded_score {
            self.commands.transport_pause();
            wait_for_transport_paused(&mut self.commands);
            self.commands.transport_seek_to_beats(Beats::ZERO);
            wait_for_transport_reset(&mut self.commands);
            return;
        }

        self.commands.clear_scheduled_changes();
        wait_for_controller_barrier(&mut self.commands);

        self.sequencer
            .prepare_for_pause(&mut self.commands, current_beat);
        wait_for_controller_barrier(&mut self.commands);
        wait_for_transport_advance(&mut self.commands, Duration::from_millis(80));

        self.commands.transport_pause();
        wait_for_transport_paused(&mut self.commands);
        self.commands.transport_seek_to_beats(Beats::ZERO);
        wait_for_transport_reset(&mut self.commands);
    }
}

fn wait_for_transport_reset(commands: &mut MultiThreadedKnystCommands) {
    for _ in 0..50 {
        let Some(snapshot) = commands.current_transport_snapshot() else {
            thread::sleep(Duration::from_millis(2));
            continue;
        };

        if snapshot.state == knyst::prelude::TransportState::Paused
            && snapshot.beats.unwrap_or(Beats::ZERO) == Beats::ZERO
        {
            return;
        }

        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
fn wait_for_transport_playing(commands: &mut MultiThreadedKnystCommands) {
    for _ in 0..50 {
        let Some(snapshot) = commands.current_transport_snapshot() else {
            thread::sleep(Duration::from_millis(2));
            continue;
        };

        if snapshot.state == knyst::prelude::TransportState::Playing {
            return;
        }

        thread::sleep(Duration::from_millis(2));
    }
}

fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    let _ = receiver.recv_timeout(Duration::from_millis(50));
}

fn wait_for_transport_paused(commands: &mut MultiThreadedKnystCommands) {
    for _ in 0..50 {
        let Some(snapshot) = commands.current_transport_snapshot() else {
            thread::sleep(Duration::from_millis(2));
            continue;
        };

        if snapshot.state == knyst::prelude::TransportState::Paused {
            return;
        }

        thread::sleep(Duration::from_millis(2));
    }
}

fn wait_for_transport_advance(commands: &mut MultiThreadedKnystCommands, timeout: Duration) {
    let start = std::time::Instant::now();
    let initial = commands
        .current_transport_snapshot()
        .and_then(|snapshot| snapshot.beats)
        .unwrap_or(Beats::ZERO);
    while start.elapsed() < timeout {
        if let Some(snapshot) = commands.current_transport_snapshot()
            && snapshot.beats.unwrap_or(Beats::ZERO) > initial
        {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
fn wait_for_transport_reset_to(commands: &mut MultiThreadedKnystCommands, target: Beats) {
    for _ in 0..50 {
        let Some(snapshot) = commands.current_transport_snapshot() else {
            thread::sleep(Duration::from_millis(2));
            continue;
        };

        if snapshot.state == knyst::prelude::TransportState::Paused
            && snapshot.beats.unwrap_or(Beats::ZERO) == target
        {
            return;
        }

        thread::sleep(Duration::from_millis(2));
    }
}

impl From<crate::mixer::runtime::MixerRuntimeError> for AudioEngineError {
    fn from(value: crate::mixer::runtime::MixerRuntimeError) -> Self {
        Self::MixerRuntime(value.to_string())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.scheduler_shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.scheduler_thread.take() {
            let _ = thread.join();
        }
        let _ = self.backend.stop();
    }
}

fn start_scheduler_thread(
    mut commands: MultiThreadedKnystCommands,
    sequencer: Sequencer,
    shutdown: Arc<AtomicBool>,
    settings: AudioEngineSettings,
) -> Result<JoinHandle<()>, std::io::Error> {
    thread::Builder::new()
        .name("lilypalooza-sequencer".to_string())
        .spawn(move || {
            set_scheduler_thread_priority(settings);
            while !shutdown.load(Ordering::Acquire) {
                sequencer.process_tick(&mut commands);
                thread::sleep(Duration::from_millis(2));
            }
        })
}

fn set_scheduler_thread_priority(settings: AudioEngineSettings) {
    let _ = promote_current_thread_to_real_time(
        settings.block_size.max(1) as u32,
        settings.sample_rate.max(1) as u32,
    );
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;

    use knyst::controller::KnystCommands;
    use knyst::graph::SimultaneousChanges;
    use knyst::handles::HandleData;
    use knyst::prelude::{Beats, BlockSize, GenState, Sample, graph_output, handle, impl_gen};

    use super::{AudioEngine, AudioEngineOptions, wait_for_transport_reset_to};
    use crate::instrument::InstrumentSlotState;
    use crate::instrument::{
        InstrumentProcessor, InstrumentProcessorNode, MidiEvent, ParamValue, Processor,
        ProcessorDescriptor, ProcessorState, ProcessorStateError,
    };
    use crate::mixer::{INSTRUMENT_TRACK_COUNT, MixerState, TrackId};
    use crate::test_utils::{
        SharedTestBackend, SharedTestBackendHandle, TestBackend, delayed_note_midi_bytes,
        four_track_midi_bytes, simple_midi_bytes, sustained_note_midi_bytes,
        test_soundfont_resource,
    };
    use crate::transport::Transport;

    struct ScheduledValueGen;
    struct TestNoteProcessor {
        active: bool,
    }

    fn settle_backend(backend: &SharedTestBackendHandle) {
        for _ in 0..50 {
            backend.process_block();
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn render_soundfont_program(program: u8) -> Vec<Sample> {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(
                    TrackId(0),
                    InstrumentSlotState::soundfont("default", 0, program),
                )
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);
        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return backend_handle.output_channel(0);
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("engine end-to-end path produced silence");
    }

    #[impl_gen]
    impl ScheduledValueGen {
        #[new]
        fn new() -> Self {
            Self
        }

        #[process]
        fn process(
            &mut self,
            value: &[Sample],
            out: &mut [Sample],
            block_size: BlockSize,
        ) -> GenState {
            out[..block_size.0].copy_from_slice(&value[..block_size.0]);
            GenState::Continue
        }
    }

    impl Processor for TestNoteProcessor {
        fn descriptor(&self) -> &'static ProcessorDescriptor {
            static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
                name: "Test Note Processor",
                params: &[],
            };
            &DESCRIPTOR
        }

        fn set_param(&mut self, _id: &str, _value: ParamValue) {}

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

    impl InstrumentProcessor for TestNoteProcessor {
        fn handle_midi(&mut self, event: MidiEvent) {
            match event {
                MidiEvent::NoteOn { .. } => self.active = true,
                MidiEvent::NoteOff { .. }
                | MidiEvent::AllNotesOff { .. }
                | MidiEvent::AllSoundOff { .. } => self.active = false,
                _ => {}
            }
        }

        fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
            let value = if self.active { 0.25 } else { 0.0 };
            left.fill(value);
            right.fill(value);
        }
    }

    #[test]
    fn engine_starts_with_transport_paused_at_zero() {
        let backend = TestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        let snapshot = engine
            .transport()
            .snapshot()
            .expect("transport snapshot should be available");

        assert_eq!(
            snapshot.playback_state,
            crate::transport::PlaybackState::Paused
        );
        assert_eq!(snapshot.beats_position, Beats::ZERO);
    }

    #[test]
    fn engine_renders_audio_after_soundfont_and_midi_load() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);
        engine
            .sequencer()
            .replace_from_midi_bytes(&sustained_note_midi_bytes(480, 1920))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                let debug = engine.sequencer.debug_state();
                assert!(
                    debug.schedule_count <= 4,
                    "sequencer should not reschedule excessively: {debug:?}"
                );
                assert!(
                    debug.reset_count <= 1,
                    "sequencer should not keep resetting notes: {debug:?}"
                );
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("engine end-to-end path produced silence");
    }

    #[test]
    fn engine_switches_soundfont_programs_at_track_level() {
        let piano = render_soundfont_program(0);
        let violin = render_soundfont_program(40);

        assert!(
            piano
                .iter()
                .zip(violin.iter())
                .any(|(a, b)| (a - b).abs() > 1.0e-6),
            "different track-level SoundFont programs rendered the same output"
        );
    }

    #[test]
    fn loading_soundfont_after_track_assignment_restores_master_output() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept pending soundfont instrument");
        }
        settle_backend(&backend_handle);

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("late soundfont load produced track signal without master output");
    }

    #[test]
    fn selecting_soundfont_program_before_playback_produces_master_output() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("pre-play program selection produced silence at master output");
    }

    #[test]
    fn selecting_soundfont_program_during_playback_produces_master_output() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");
        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }
        settle_backend(&backend_handle);
        engine.transport().play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("during-playback program selection produced silence at master output");
    }

    #[test]
    fn persistent_engine_reload_then_preplay_program_selection_produces_master_output() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine
            .replace_score_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        settle_backend(&backend_handle);
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("persistent-engine reload followed by pre-play program selection produced silence");
    }

    #[test]
    fn persistent_engine_reload_then_live_program_selection_reaches_master_output() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine
            .replace_score_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");

        settle_backend(&backend_handle);
        engine.transport().play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("persistent-engine reload followed by live program selection produced silence");
    }

    #[test]
    fn persistent_engine_live_track_assignment_allows_direct_midi_to_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine
            .replace_score_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        let handle = engine
            .mixer
            .instrument_handle(TrackId(0))
            .expect("track runtime should expose instrument handle");

        settle_backend(&backend_handle);
        engine.transport().play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        handle.send_midi(
            &mut engine.commands,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("direct MIDI into live-assigned persistent-engine track did not reach master");
    }

    #[test]
    fn app_lifecycle_preplay_program_selection_reaches_master_after_score_replace() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont load should work");
        }

        engine
            .replace_score_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            for track_index in 0..INSTRUMENT_TRACK_COUNT {
                mixer
                    .set_track_name(
                        TrackId(track_index as u16),
                        format!("Track {}", track_index + 1),
                    )
                    .expect("track rename should work");
            }
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        settle_backend(&backend_handle);
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("app-style preplay program selection produced no master output");
    }

    #[test]
    fn app_lifecycle_live_program_selection_reaches_master_after_score_replace() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont load should work");
        }

        engine
            .replace_score_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            for track_index in 0..INSTRUMENT_TRACK_COUNT {
                mixer
                    .set_track_name(
                        TrackId(track_index as u16),
                        format!("Track {}", track_index + 1),
                    )
                    .expect("track rename should work");
            }
        }

        settle_backend(&backend_handle);
        engine.transport().play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("app-style live program selection produced no master output");
    }

    #[test]
    fn app_lifecycle_preplay_program_selection_without_backend_settle_reaches_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont load should work");
        }

        engine
            .replace_score_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("preplay program selection without backend settle produced no master output");
    }

    #[test]
    fn app_lifecycle_live_program_selection_without_backend_settle_reaches_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont load should work");
        }

        engine
            .replace_score_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");

        engine.transport().play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("live program selection without backend settle produced no master output");
    }

    #[test]
    fn persistent_engine_reset_then_live_track_assignment_allows_direct_midi_to_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine.transport().pause();
        engine.transport().rewind();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
        }

        let handle = engine
            .mixer
            .instrument_handle(TrackId(0))
            .expect("track runtime should expose instrument handle");

        settle_backend(&backend_handle);
        engine.commands.transport_play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        handle.send_midi(
            &mut engine.commands,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("persistent-engine reset followed by live assignment stayed silent");
    }

    #[test]
    fn persistent_engine_reset_then_live_track_assignment_allows_direct_note_on_to_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine.transport().pause();
        engine.transport().rewind();

        let handle = {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
            engine
                .mixer
                .instrument_handle(TrackId(0))
                .expect("track runtime should expose instrument handle")
        };

        settle_backend(&backend_handle);
        engine.commands.transport_play();
        super::wait_for_transport_playing(&mut engine.commands);
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.context.with_activation(|| {
            handle.note_on(0, 60, 100);
        });

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("persistent-engine reset followed by direct note_on stayed silent");
    }

    #[test]
    fn raw_transport_reset_then_live_track_assignment_allows_direct_note_on_to_master() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }

        engine.commands.transport_pause();
        engine.commands.transport_seek_to_beats(Beats::ZERO);

        let handle = {
            let mut mixer = engine.mixer();
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
            engine
                .mixer
                .instrument_handle(TrackId(0))
                .expect("track runtime should expose instrument handle")
        };

        settle_backend(&backend_handle);
        engine.commands.transport_play();
        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.context.with_activation(|| {
            handle.note_on(0, 60, 100);
        });

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("raw transport reset followed by direct note_on stayed silent");
    }

    #[test]
    fn pre_play_track_mix_sync_does_not_silence_first_playback() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
                .expect("track should accept soundfont instrument");
            mixer
                .set_track_muted(TrackId(0), false)
                .expect("mute sync should succeed");
            mixer
                .set_track_soloed(TrackId(0), false)
                .expect("solo sync should succeed");
        }
        settle_backend(&backend_handle);
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("pre-play mute/solo sync silenced first playback");
    }

    #[test]
    fn engine_renders_audio_without_callback_installation() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut transport = Transport::new(
                &mut engine.commands,
                Some(&mut engine.mixer),
                Some(&engine.sequencer),
                Some(engine.runtime_dirty.as_ref()),
            );
            transport.play();
        }

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("engine without callback produced silence");
    }

    #[test]
    fn engine_renders_audio_for_four_track_midi_with_tempo_track() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            for track_index in 0..4 {
                mixer
                    .set_track_instrument(
                        TrackId(track_index as u16),
                        InstrumentSlotState::soundfont("default", 0, track_index as u8),
                    )
                    .expect("track should accept soundfont instrument");
            }
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&four_track_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("engine four-track end-to-end path produced silence");
    }

    #[test]
    fn track_rename_does_not_dirty_audio_runtime() {
        let backend = TestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        engine.flush();
        assert!(
            !engine.runtime_dirty.load(Ordering::Acquire),
            "flush should clear runtime dirty state"
        );

        engine
            .mixer()
            .set_track_name(TrackId(0), "Violin")
            .expect("track rename should succeed");

        assert!(
            !engine.runtime_dirty.load(Ordering::Acquire),
            "renaming a track must not dirty the audio runtime"
        );
    }

    #[test]
    fn soundfont_load_dirties_audio_runtime_for_next_play_flush() {
        let backend = TestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        engine.flush();
        assert!(
            !engine.runtime_dirty.load(Ordering::Acquire),
            "flush should clear runtime dirty state"
        );

        engine
            .mixer()
            .set_soundfont(test_soundfont_resource())
            .expect("soundfont should load");

        assert!(
            engine.runtime_dirty.load(Ordering::Acquire),
            "loading a soundfont must dirty the audio runtime so play can flush topology changes"
        );
    }

    #[test]
    fn track_instrument_assignment_dirties_audio_runtime_for_next_play_flush() {
        let backend = TestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        engine.flush();
        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
        }
        assert!(
            engine.runtime_dirty.load(Ordering::Acquire),
            "loading a soundfont must dirty the audio runtime"
        );
        engine.flush();

        engine
            .mixer()
            .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 40))
            .expect("track should accept soundfont instrument");

        assert!(
            engine.runtime_dirty.load(Ordering::Acquire),
            "assigning a track instrument must dirty the audio runtime so play can flush topology changes"
        );
    }

    #[test]
    fn paused_seek_then_play_starts_from_seek_position() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&sustained_note_midi_bytes(480, 1920))
            .expect("midi should load");
        engine.transport().seek_beats(1.0);
        let before_play = engine
            .transport()
            .snapshot()
            .expect("transport snapshot should be available");
        engine.transport().play();

        let mut max_peak = 0.0_f32;

        for _ in 0..128 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            let peak = backend_handle
                .output_channel(0)
                .into_iter()
                .chain(backend_handle.output_channel(1))
                .map(f32::abs)
                .fold(0.0_f32, f32::max);
            max_peak = max_peak.max(peak);
            thread::sleep(Duration::from_millis(2));
        }

        let after_play = engine
            .transport()
            .snapshot()
            .expect("transport snapshot should be available");
        let debug = engine.sequencer.debug_state();
        panic!(
            "paused seek followed by play produced silence; before_play={before_play:?}; after_play={after_play:?}; debug={debug:?}; max_peak={max_peak}"
        );
    }

    #[test]
    fn paused_seek_then_play_schedules_note_beyond_initial_jump() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 1920))
            .expect("midi should load");
        engine.transport().seek_beats(3.0);
        engine.transport().play();

        let mut max_peak = 0.0_f32;
        for _ in 0..4096 {
            backend_handle.process_block();
            max_peak = max_peak.max(
                backend_handle
                    .output_channel(0)
                    .into_iter()
                    .chain(backend_handle.output_channel(1))
                    .map(f32::abs)
                    .fold(0.0, f32::max),
            );
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        let debug = engine.sequencer.debug_state();
        panic!(
            "paused seek followed by play should reach delayed note after jump; debug={debug:?}; max_peak={max_peak}"
        );
    }

    #[test]
    fn paused_seek_into_sustained_note_chases_note_on() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&sustained_note_midi_bytes(480, 1920))
            .expect("midi should load");
        engine.transport().seek_beats(2.0);
        engine.transport().play();

        for _ in 0..512 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("seek into sustained note should chase active note on");
    }

    #[test]
    fn seek_while_playing_into_sustained_note_chases_note_on() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&sustained_note_midi_bytes(480, 1920))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..32 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.transport().seek_beats(2.0);

        for _ in 0..512 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("seek while playing into sustained note should chase active note on");
    }

    #[test]
    fn pause_resets_active_notes() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");
        engine.transport().seek_beats(1.5);
        engine.transport().play();

        let mut heard_signal = false;
        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                heard_signal = true;
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(heard_signal, "playback should produce signal before rewind");

        engine.transport().pause();

        let mut silent_blocks = 0_usize;
        for _ in 0..256 {
            backend_handle.process_block();
            if !backend_handle.output_has_signal() {
                silent_blocks += 1;
                if silent_blocks >= 16 {
                    return;
                }
            } else {
                silent_blocks = 0;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("pause should eventually clear active notes");
    }

    #[test]
    fn rewind_while_playing_keeps_playback_running() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");
        engine.transport().seek_beats(1.5);
        engine.transport().play();

        let mut heard_signal = false;
        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                heard_signal = true;
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(heard_signal, "playback should produce signal before rewind");

        engine.transport().rewind();
        let snapshot = engine
            .transport()
            .snapshot()
            .expect("transport snapshot should be available");
        assert_eq!(
            snapshot.playback_state,
            crate::transport::PlaybackState::Playing
        );

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("rewind while playing should resume audible playback");
    }

    #[test]
    fn rewind_while_playing_then_pause_clears_notes() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&sustained_note_midi_bytes(480, 3840))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..128 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.transport().seek_beats(4.0);
        for _ in 0..128 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.transport().rewind();
        engine.transport().pause();

        for _ in 0..128 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        let max_after_pause = backend_handle
            .output_channel(0)
            .into_iter()
            .chain(backend_handle.output_channel(1))
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(
            max_after_pause < 1.0e-4,
            "rewind then pause should leave no active notes, peak after pause was {max_after_pause}"
        );
    }

    #[test]
    fn rewind_while_playing_then_pause_then_play_resumes_cleanly() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            MixerState::new(),
            backend,
            AudioEngineOptions {
                chase_notes_on_seek: true,
                ..AudioEngineOptions::default()
            },
        )
        .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
            .expect("midi should load");
        engine.transport().seek_beats(1.5);
        engine.transport().play();

        let mut heard_signal = false;
        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                heard_signal = true;
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(heard_signal, "playback should produce signal before rewind");

        engine.transport().rewind();
        engine.transport().pause();
        engine.transport().play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("rewind then pause then play should resume audible playback");
    }

    #[test]
    fn paused_seek_then_direct_parameter_change_and_play_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        let node = engine.context.with_activation(|| {
            let node = handle(ScheduledValueGen::new());
            graph_output(0, node.channels(1));
            node
        });
        let node_id = node
            .node_ids()
            .next()
            .expect("scheduled value node should exist");

        engine.commands.transport_pause();
        engine
            .commands
            .transport_seek_to_beats(Beats::from_beats(1));
        wait_for_transport_reset_to(&mut engine.commands, Beats::from_beats(1));
        let mut changes = SimultaneousChanges::duration_from_now(Duration::ZERO);
        changes.push(node_id.change().set("value", 1.0));
        engine.commands.schedule_changes(changes);
        engine.commands.transport_play();

        for _ in 0..128 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("paused seek followed by direct parameter change produced silence");
    }

    #[test]
    fn paused_seek_then_direct_midi_into_instrument_node_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        let instrument = engine.context.with_activation(|| {
            let node = handle(InstrumentProcessorNode::new(Box::new(TestNoteProcessor {
                active: false,
            })));
            graph_output(0, node.channels(2));
            node
        });
        let handle = crate::instrument::InstrumentRuntimeHandle::new(instrument);

        engine.commands.transport_pause();
        engine
            .commands
            .transport_seek_to_beats(Beats::from_beats(1));
        wait_for_transport_reset_to(&mut engine.commands, Beats::from_beats(1));
        engine.commands.transport_play();

        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        handle.send_midi(
            &mut engine.commands,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..128 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("paused seek followed by direct MIDI into instrument node produced silence");
    }

    #[test]
    fn paused_seek_then_direct_scheduled_midi_into_instrument_node_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        let instrument = engine.context.with_activation(|| {
            let node = handle(InstrumentProcessorNode::new(Box::new(TestNoteProcessor {
                active: false,
            })));
            graph_output(0, node.channels(2));
            node
        });
        let handle = crate::instrument::InstrumentRuntimeHandle::new(instrument);

        engine.commands.transport_pause();
        engine
            .commands
            .transport_seek_to_beats(Beats::from_beats(1));
        wait_for_transport_reset_to(&mut engine.commands, Beats::from_beats(1));
        handle.schedule_reset_at(&mut engine.commands, Beats::from_beats_f64(1.01), 1);
        handle.schedule_midi_at_with_offset(
            &mut engine.commands,
            Beats::from_beats_f64(1.02),
            1,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );
        engine.commands.transport_play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!(
            "paused seek followed by direct scheduled MIDI into instrument node produced silence"
        );
    }

    #[test]
    fn paused_seek_then_direct_scheduled_midi_into_soundfont_track_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        let handle = engine
            .mixer
            .instrument_handle(TrackId(0))
            .expect("track runtime should expose instrument handle");

        engine.transport().seek_beats(1.0);
        handle.schedule_reset_at(&mut engine.commands, Beats::from_beats_f64(1.01), 1);
        handle.schedule_midi_at_with_offset(
            &mut engine.commands,
            Beats::from_beats_f64(1.02),
            1,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );
        engine.commands.transport_play();

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!(
            "paused seek followed by direct scheduled MIDI into soundfont track produced silence"
        );
    }

    #[test]
    fn paused_seek_then_immediate_midi_into_soundfont_track_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");
        let _audio = backend_handle.start_realtime();

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        settle_backend(&backend_handle);

        let handle = engine
            .mixer
            .instrument_handle(TrackId(0))
            .expect("track runtime should expose instrument handle");

        engine.transport().seek_beats(1.0);
        engine.commands.transport_play();

        for _ in 0..8 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        handle.send_midi(
            &mut engine.commands,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..1024 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("paused seek followed by immediate MIDI into soundfont track produced silence");
    }
}
