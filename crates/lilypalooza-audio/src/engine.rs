//! Top-level audio engine state.
#![allow(missing_docs)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
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
    backend: Box<dyn AudioBackend>,
    settings: AudioEngineSettings,
    scheduler_stop: Arc<AtomicBool>,
    scheduler_thread: Option<JoinHandle<()>>,
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
        let sphere = KnystSphere::start(&mut backend, options.sphere_settings, |_| {})?;
        let context = sphere.context();
        let mut commands = sphere.commands();
        commands.transport_pause();
        commands.transport_seek_to_beats(Beats::ZERO);
        let mixer = Mixer::new(&context, &mut commands, &settings, mixer)?;
        let sequencer = Sequencer::new();
        for track in mixer.state.tracks() {
            sequencer.sync_track_handle(track.id, mixer.instrument_handle(track.id));
        }
        let scheduler_stop = Arc::new(AtomicBool::new(false));
        let scheduler_thread = Some(start_scheduler_thread(
            sphere.commands(),
            sequencer.clone(),
            Arc::clone(&scheduler_stop),
        ));
        Ok(Self {
            mixer,
            sequencer,
            backend: Box::new(backend),
            settings,
            scheduler_stop,
            scheduler_thread,
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
        Transport::new(&mut self.commands, Some(&self.sequencer))
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

    /// Returns the sequencer control handle.
    pub fn sequencer(&mut self) -> SequencerHandle<'_> {
        SequencerHandle::new(&self.sequencer, &mut self.commands)
    }
}

impl From<crate::mixer::runtime::MixerRuntimeError> for AudioEngineError {
    fn from(value: crate::mixer::runtime::MixerRuntimeError) -> Self {
        Self::MixerRuntime(value.to_string())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.scheduler_stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.scheduler_thread.take() {
            let _ = thread.join();
        }
        let _ = self.backend.stop();
    }
}

fn start_scheduler_thread(
    mut commands: MultiThreadedKnystCommands,
    sequencer: Sequencer,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let _ = sequencer.process_tick(&mut commands);
            thread::sleep(Duration::from_millis(10));
        }
    })
}

#[cfg(test)]
mod tests {
    use knyst::prelude::Beats;

    use super::{AudioEngine, AudioEngineOptions};
    use crate::test_utils::TestBackend;

    #[test]
    fn engine_starts_with_transport_paused_at_zero() {
        let backend = TestBackend::new(44_100, 64, 2);
        let mut engine = AudioEngine::start(
            crate::mixer::MixerState::new(),
            backend,
            AudioEngineOptions::default(),
        )
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
}
