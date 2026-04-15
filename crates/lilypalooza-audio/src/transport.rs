//! Playback transport control and state snapshots.

use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands, Seconds, TransportState};

/// Playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// Playback is running.
    Playing,
    /// Playback is paused.
    #[default]
    Paused,
}

/// Read-only transport state snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransportSnapshot {
    /// Current playback state.
    pub playback_state: PlaybackState,
    /// Current absolute transport position in seconds.
    pub seconds_position: f64,
    /// Current absolute transport position in beats.
    pub beats_position: f64,
}

impl TransportSnapshot {
    /// Creates a transport snapshot value.
    #[must_use]
    pub fn new(playback_state: PlaybackState, seconds_position: f64, beats_position: f64) -> Self {
        Self {
            playback_state,
            seconds_position,
            beats_position,
        }
    }
}

/// Errors returned by transport operations.
#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    /// Failed to receive a transport snapshot from Knyst.
    #[error(transparent)]
    SnapshotChannelClosed(#[from] std::sync::mpsc::RecvError),
    /// Knyst reported no transport snapshot.
    #[error("transport snapshot is unavailable")]
    SnapshotUnavailable,
}

/// Mutable transport control handle.
pub struct Transport<'a> {
    commands: &'a mut MultiThreadedKnystCommands,
}

impl<'a> Transport<'a> {
    pub(crate) fn new(commands: &'a mut MultiThreadedKnystCommands) -> Self {
        Self { commands }
    }

    /// Starts playback.
    pub fn play(&mut self) {
        self.commands.transport_play();
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        self.commands.transport_pause();
    }

    /// Rewinds to the start and pauses playback.
    pub fn rewind(&mut self) {
        self.commands.transport_pause();
        self.commands.transport_seek_to_seconds(Seconds::ZERO);
    }

    /// Seeks transport to an absolute seconds position.
    pub fn seek_seconds(&mut self, position: f64) {
        self.commands
            .transport_seek_to_seconds(Seconds::from_seconds_f64(position.max(0.0)));
    }

    /// Seeks transport to an absolute beats position.
    pub fn seek_beats(&mut self, position: f64) {
        self.commands
            .transport_seek_to_beats(Beats::from_beats_f64(position.max(0.0)));
    }

    /// Reads current transport state from Knyst.
    pub fn snapshot(&mut self) -> Result<TransportSnapshot, TransportError> {
        let snapshot = self
            .commands
            .request_transport_snapshot()
            .recv()?
            .ok_or(TransportError::SnapshotUnavailable)?;
        Ok(TransportSnapshot::new(
            match snapshot.state {
                TransportState::Playing => PlaybackState::Playing,
                TransportState::Paused => PlaybackState::Paused,
            },
            snapshot.seconds.to_seconds_f64(),
            snapshot.beats.unwrap_or(Beats::ZERO).as_beats_f64(),
        ))
    }
}
