//! Top-level audio engine state.

use std::{ops::Range, thread, time::Duration};

use knyst::{
    audio_backend::{AudioBackend, AudioBackendError, CpalBackend, CpalBackendOptions},
    modal_interface::{KnystContext, SphereError},
    prelude::{Beats, KnystCommands, KnystSphere, MultiThreadedKnystCommands, SphereSettings},
};

#[cfg(feature = "test-support")]
use crate::test_support::TestSupportBackend;
#[cfg(all(test, not(feature = "test-support")))]
use crate::test_utils::TestBackend as TestSupportBackend;
use crate::{
    instrument::Controller,
    mixer::{
        Mixer, MixerError, MixerHandle, MixerMeterSnapshot, MixerMeterSnapshotWindow, MixerState,
        SlotAddress,
    },
    sequencer::{Sequencer, SequencerError, SequencerHandle},
    transport::Transport,
};

const CONTROLLER_BARRIER_TIMEOUT: Duration = Duration::from_millis(250);
const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// Startup options for the audio engine.
#[derive(Debug, Clone, Default)]
pub struct AudioEngineOptions {
    /// Knyst sphere settings.
    pub sphere_settings: SphereSettings,
    /// Preferred output device name.
    pub device: Option<String>,
    /// Whether seeking should chase already-held notes into the new position.
    pub chase_notes_on_seek: bool,
    /// Preferred backend sample rate in Hz.
    pub sample_rate: Option<usize>,
    /// Preferred backend block size in frames.
    pub block_size: Option<usize>,
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
    backend: Box<dyn AudioBackend>,
    settings: AudioEngineSettings,
    // Keeps the Knyst runtime alive; context and commands do not own it.
    _sphere: KnystSphere,
    context: KnystContext,
    commands: MultiThreadedKnystCommands,
}

impl std::fmt::Debug for AudioEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioEngine")
            .field("sequencer", &self.sequencer)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
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
}

/// Runtime observability snapshot for the engine callback path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineObservabilitySnapshot {
    /// Current callback load ratio (`1.0` means exactly one block budget).
    pub load_ratio: f32,
    /// Peak callback load ratio observed since engine start.
    pub peak_load_ratio: f32,
    /// Number of detected callback-overrun/dropout-like issues.
    pub dropout_count: u64,
}

impl AudioEngine {
    /// Starts the audio engine on the deterministic in-process test backend.
    #[cfg(any(test, feature = "test-support"))]
    pub fn start_test(
        mixer: MixerState,
        options: AudioEngineOptions,
    ) -> Result<Self, AudioEngineError> {
        let sample_rate = options.sample_rate.unwrap_or(44_100);
        let block_size = options.block_size.unwrap_or(64);
        Self::start(
            mixer,
            TestSupportBackend::new(sample_rate, block_size, 2),
            options,
        )
    }

    /// Starts the audio engine on the default CPAL output backend.
    pub fn start_cpal(
        mixer: MixerState,
        options: AudioEngineOptions,
    ) -> Result<Self, AudioEngineError> {
        let backend = CpalBackend::new(CpalBackendOptions {
            device: options.device.clone().unwrap_or_else(|| "default".into()),
            ..CpalBackendOptions::default()
        })?;
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
        for (track_id, _) in mixer.state.tracks_with_ids() {
            sequencer.sync_track_handle(&mut commands, track_id, mixer.instrument_handle(track_id));
        }
        sequencer.sync_metronome_handle(&mut commands, Some(mixer.metronome_handle()));
        commands.set_scheduler_extension(Box::new(sequencer.scheduler_extension()));
        Ok(Self {
            mixer,
            sequencer,
            backend: Box::new(backend),
            settings,
            _sphere: sphere,
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
        )
    }

    /// Returns the mixer control handle.
    pub fn mixer(&mut self) -> MixerHandle<'_> {
        MixerHandle::new(
            &mut self.mixer,
            &self.sequencer,
            &self.context,
            &mut self.commands,
        )
    }

    /// Returns the current persistent mixer state.
    pub fn mixer_state(&self) -> &MixerState {
        &self.mixer.state
    }

    /// Creates a controller for the processor at `address`, when available.
    pub fn controller(
        &self,
        address: SlotAddress,
    ) -> Result<Option<Box<dyn Controller>>, AudioEngineError> {
        self.mixer.controller(address)
    }

    /// Refreshes processor latency compensation from live plugin bindings.
    pub fn sync_processor_latencies(&mut self) -> Result<(), AudioEngineError> {
        self.mixer().sync_processor_latencies()
    }

    /// Returns the sequencer control handle.
    pub fn sequencer(&mut self) -> SequencerHandle<'_> {
        SequencerHandle::new(&self.sequencer, &mut self.commands)
    }

    /// Returns the latest full mixer meter snapshot.
    pub fn meter_snapshot(&self) -> MixerMeterSnapshot {
        self.mixer.meter_snapshot()
    }

    /// Returns the latest meter snapshot for visible track and bus ranges.
    pub fn meter_snapshot_window(
        &self,
        track_range: Range<usize>,
        bus_range: Range<usize>,
    ) -> MixerMeterSnapshotWindow {
        self.mixer.meter_snapshot_window(track_range, bus_range)
    }

    /// Requests an audio callback observability snapshot from the audio thread.
    pub fn observability_snapshot(&mut self) -> Option<EngineObservabilitySnapshot> {
        let receiver = self.commands.request_observability_snapshot();
        receiver
            .recv_timeout(SETTLE_TIMEOUT)
            .ok()
            .flatten()
            .map(|snapshot| EngineObservabilitySnapshot {
                load_ratio: snapshot.load_ratio,
                peak_load_ratio: snapshot.peak_load_ratio,
                dropout_count: snapshot.dropout_count,
            })
    }

    /// Clears the currently loaded score and resets transport state.
    pub fn clear_score(&mut self) {
        self.prepare_for_score_reload();
        self.sequencer().clear();
    }

    /// Enables or disables metronome playback.
    pub fn set_metronome_enabled(&mut self, enabled: bool) {
        self.sequencer.set_metronome_enabled(enabled);
    }

    /// Sets the metronome gain in decibels.
    pub fn set_metronome_gain_db(&mut self, gain_db: f32) {
        self.mixer.runtime.set_metronome_gain_db(gain_db);
    }

    /// Sets the metronome pitch multiplier.
    pub fn set_metronome_pitch(&mut self, pitch: f32) {
        self.mixer.runtime.set_metronome_pitch(pitch);
    }

    /// Replaces the score with MIDI bytes and resets sequencer state.
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
        let receiver = self.commands.request_graph_settled();
        match receiver.recv_timeout(SETTLE_TIMEOUT) {
            Ok(()) | Err(_) => {}
        }
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
            wait_for_transport_settled(&mut self.commands);
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
        wait_for_transport_settled(&mut self.commands);
        self.commands.transport_seek_to_beats(Beats::ZERO);
        wait_for_transport_reset(&mut self.commands);
    }
}

fn wait_for_transport_reset(commands: &mut MultiThreadedKnystCommands) {
    wait_for_transport_reset_to(commands, Beats::ZERO);
}

fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    match receiver.recv_timeout(CONTROLLER_BARRIER_TIMEOUT) {
        Ok(_) | Err(_) => {}
    }
}

fn wait_for_transport_settled(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_settled();
    match receiver.recv_timeout(SETTLE_TIMEOUT) {
        Ok(()) | Err(_) => {}
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
        self.commands.clear_scheduler_extension();
        match self.backend.stop() {
            Ok(()) | Err(_) => {}
        }
    }
}

#[cfg(test)]
mod engine_tests;
