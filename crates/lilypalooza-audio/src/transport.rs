//! Playback transport state.

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// Playback is stopped.
    #[default]
    Stopped,
    /// Playback is running.
    Playing,
    /// Playback is paused.
    Paused,
}

/// Transport state for the audio engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Transport {
    /// Current playback state.
    pub playback_state: PlaybackState,
    /// Current absolute tick position.
    pub tick_position: u64,
}
