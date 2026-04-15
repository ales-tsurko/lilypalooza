//! Top-level audio engine state.
#![allow(missing_docs)]

use crate::mixer::{Mixer, MixerError, MixerHandle, MixerState};
use crate::transport::Transport;
use knyst::audio_backend::{AudioBackend, AudioBackendError};
use knyst::modal_interface::{KnystContext, SphereError};
use knyst::prelude::{KnystSphere, MultiThreadedKnystCommands, SphereSettings};

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
}

impl AudioEngine {
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
        let mixer = Mixer::new(&context, &mut commands, &settings, mixer)?;
        Ok(Self {
            mixer,
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
        Transport::new(&mut self.commands)
    }

    /// Returns the mixer control handle.
    pub fn mixer(&mut self) -> MixerHandle<'_> {
        MixerHandle::new(&mut self.mixer, &self.context, &mut self.commands)
    }
}

impl From<crate::mixer::runtime::MixerRuntimeError> for AudioEngineError {
    fn from(value: crate::mixer::runtime::MixerRuntimeError) -> Self {
        Self::MixerRuntime(value.to_string())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.backend.stop();
    }
}
