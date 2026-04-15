//! Top-level audio engine state.

use crate::mixer::MixerConfig;
use crate::transport::Transport;

/// Top-level audio engine container.
#[derive(Debug, Clone)]
pub struct AudioEngine {
    mixer: MixerConfig,
    transport: Transport,
}

impl AudioEngine {
    /// Creates a new audio engine with the provided mixer configuration.
    #[must_use]
    pub fn new(mixer: MixerConfig) -> Self {
        Self {
            mixer,
            transport: Transport::default(),
        }
    }

    /// Returns the mixer configuration.
    #[must_use]
    pub fn mixer(&self) -> &MixerConfig {
        &self.mixer
    }

    /// Returns the transport state.
    #[must_use]
    pub fn transport(&self) -> &Transport {
        &self.transport
    }
}
