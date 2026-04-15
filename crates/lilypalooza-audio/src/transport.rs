//! Playback transport control and state snapshots.

use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands, Seconds, TransportState};

use crate::sequencer::Sequencer;

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportSnapshot {
    /// Current playback state.
    pub playback_state: PlaybackState,
    /// Current absolute transport position in seconds.
    pub seconds_position: Seconds,
    /// Current absolute transport position in beats.
    pub beats_position: Beats,
}

impl TransportSnapshot {
    /// Creates a transport snapshot value.
    #[must_use]
    pub fn new(
        playback_state: PlaybackState,
        seconds_position: Seconds,
        beats_position: Beats,
    ) -> Self {
        Self {
            playback_state,
            seconds_position,
            beats_position,
        }
    }

    /// Returns the current absolute transport position in seconds as `f64`.
    #[must_use]
    pub fn seconds_position_f64(self) -> f64 {
        self.seconds_position.to_seconds_f64()
    }

    /// Returns the current absolute transport position in beats as `f64`.
    #[must_use]
    pub fn beats_position_f64(self) -> f64 {
        self.beats_position.as_beats_f64()
    }
}

impl Default for TransportSnapshot {
    fn default() -> Self {
        Self::new(PlaybackState::Paused, Seconds::ZERO, Beats::ZERO)
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
    sequencer: Option<&'a Sequencer>,
}

impl<'a> Transport<'a> {
    pub(crate) fn new(
        commands: &'a mut MultiThreadedKnystCommands,
        sequencer: Option<&'a Sequencer>,
    ) -> Self {
        Self {
            commands,
            sequencer,
        }
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
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty();
        }
    }

    /// Seeks transport to an absolute seconds position.
    pub fn seek_seconds(&mut self, position: f64) {
        self.commands
            .transport_seek_to_seconds(Seconds::from_seconds_f64(position.max(0.0)));
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty();
        }
    }

    /// Seeks transport to an absolute beats position.
    pub fn seek_beats(&mut self, position: f64) {
        self.commands
            .transport_seek_to_beats(Beats::from_beats_f64(position.max(0.0)));
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty();
        }
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
            snapshot.seconds,
            snapshot.beats.unwrap_or(Beats::ZERO),
        ))
    }
}
