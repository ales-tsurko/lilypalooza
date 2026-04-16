//! Top-level audio engine state.
#![allow(missing_docs)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use knyst::audio_backend::{AudioBackend, AudioBackendError};
use knyst::audio_backend::{CpalBackend, CpalBackendOptions};
use knyst::modal_interface::{KnystContext, SphereError};
use knyst::prelude::{
    Beats, KnystCommands, KnystSphere, MultiThreadedKnystCommands, SphereSettings,
};

use crate::mixer::{Mixer, MixerError, MixerHandle, MixerState};
use crate::sequencer::{Sequencer, SequencerHandle};
use crate::transport::Transport;

/// Startup options for the audio engine.
#[derive(Debug, Clone, Default)]
pub struct AudioEngineOptions {
    /// Knyst sphere settings.
    pub sphere_settings: SphereSettings,
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
        let sequencer = Sequencer::new();
        let runtime_dirty = Arc::new(AtomicBool::new(true));
        for track in mixer.state.tracks() {
            sequencer.sync_track_handle(track.id, mixer.instrument_handle(track.id));
        }
        let scheduler_shutdown = Arc::new(AtomicBool::new(false));
        let scheduler_thread = Some(start_scheduler_thread(
            context.commands(),
            sequencer.clone(),
            scheduler_shutdown.clone(),
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

    /// Returns the sequencer control handle.
    pub fn sequencer(&mut self) -> SequencerHandle<'_> {
        SequencerHandle::new(&self.sequencer, &mut self.commands)
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
}

fn wait_for_transport_reset(commands: &mut MultiThreadedKnystCommands) {
    for _ in 0..50 {
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
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
fn wait_for_transport_reset_to(commands: &mut MultiThreadedKnystCommands, target: Beats) {
    for _ in 0..50 {
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
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
) -> Result<JoinHandle<()>, std::io::Error> {
    thread::Builder::new()
        .name("lilypalooza-sequencer".to_string())
        .spawn(move || {
            set_scheduler_thread_priority();
            while !shutdown.load(Ordering::Acquire) {
                sequencer.process_tick(&mut commands);
                thread::sleep(Duration::from_millis(2));
            }
        })
}

fn set_scheduler_thread_priority() {}

#[cfg(test)]
mod tests {
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
    use crate::mixer::{MixerState, TrackId};
    use crate::test_utils::{
        SharedTestBackend, TestBackend, delayed_note_midi_bytes, four_track_midi_bytes,
        simple_midi_bytes, test_soundfont_resource,
    };
    use crate::transport::Transport;

    struct ScheduledValueGen;
    struct TestNoteProcessor {
        active: bool,
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

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }
        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..256 {
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
    fn engine_renders_audio_without_callback_installation() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");

        {
            let mut transport = Transport::new(
                &mut engine.commands,
                Some(&engine.sequencer),
                Some(engine.runtime_dirty.as_ref()),
            );
            transport.play();
        }

        for _ in 0..256 {
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

        engine
            .sequencer()
            .replace_from_midi_bytes(&four_track_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..256 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("engine four-track end-to-end path produced silence");
    }

    #[test]
    fn scheduler_thread_refills_future_events() {
        let (backend, backend_handle) = SharedTestBackend::new(16_000, 1_024, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 960))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..40 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }

        panic!("scheduler thread did not refill future delayed events");
    }

    #[test]
    fn paused_seek_then_play_starts_from_seek_position() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }

        engine
            .sequencer()
            .replace_from_midi_bytes(&delayed_note_midi_bytes(480, 480))
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
    fn rewind_resets_active_notes() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        let mut heard_signal = false;
        for _ in 0..256 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                heard_signal = true;
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert!(heard_signal, "playback should produce signal before rewind");

        engine.transport().rewind();

        for _ in 0..64 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        let max_after_rewind = backend_handle
            .output_channel(0)
            .into_iter()
            .chain(backend_handle.output_channel(1))
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(
            max_after_rewind < 1.0e-4,
            "rewind should clear active notes, peak after rewind was {max_after_rewind}"
        );
    }

    #[test]
    fn rewind_while_playing_keeps_playback_running() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

        {
            let mut mixer = engine.mixer();
            mixer
                .set_soundfont(test_soundfont_resource())
                .expect("soundfont should load");
            mixer
                .set_track_instrument(TrackId(0), InstrumentSlotState::soundfont("default", 0, 0))
                .expect("track should accept soundfont instrument");
        }

        engine
            .sequencer()
            .replace_from_midi_bytes(&simple_midi_bytes(480))
            .expect("midi should load");
        engine.transport().play();

        for _ in 0..64 {
            backend_handle.process_block();
            thread::sleep(Duration::from_millis(2));
        }

        engine.transport().rewind();
        let snapshot = engine
            .transport()
            .snapshot()
            .expect("transport snapshot should be available");
        assert_eq!(
            snapshot.playback_state,
            crate::transport::PlaybackState::Playing
        );

        for _ in 0..256 {
            backend_handle.process_block();
            if backend_handle.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }

        panic!("rewind while playing should resume audible playback");
    }

    #[test]
    fn paused_seek_then_direct_parameter_change_and_play_produces_signal() {
        let (backend, backend_handle) = SharedTestBackend::new(44_100, 64, 2);
        let mut engine =
            AudioEngine::start(MixerState::new(), backend, AudioEngineOptions::default())
                .expect("engine should start");

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
}
