//! Top-level audio engine state.

use crate::mixer::MixerConfig;
use crate::transport::Transport;
use knyst::audio_backend::{AudioBackend, AudioBackendError};
use knyst::modal_interface::SphereError;
use knyst::prelude::{KnystSphere, MultiThreadedKnystCommands, SphereSettings};

/// Startup options for the audio engine.
#[derive(Debug, Clone, Default)]
pub struct AudioEngineOptions {
    /// Knyst sphere settings.
    pub sphere_settings: SphereSettings,
}

/// Top-level audio engine container.
pub struct AudioEngine {
    mixer: MixerConfig,
    backend: Box<dyn AudioBackend>,
    _sphere: KnystSphere,
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
}

impl AudioEngine {
    /// Starts the audio engine on the provided backend.
    pub fn start<B: AudioBackend + 'static>(
        mixer: MixerConfig,
        mut backend: B,
        options: AudioEngineOptions,
    ) -> Result<Self, AudioEngineError> {
        let sphere = KnystSphere::start(&mut backend, options.sphere_settings, |_| {})?;
        let commands = sphere.commands();
        Ok(Self {
            mixer,
            backend: Box::new(backend),
            _sphere: sphere,
            commands,
        })
    }

    /// Returns the mixer configuration.
    #[must_use]
    pub fn mixer(&self) -> &MixerConfig {
        &self.mixer
    }

    /// Returns the transport control handle.
    pub fn transport(&mut self) -> Transport<'_> {
        Transport::new(&mut self.commands)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.backend.stop();
    }
}
