//! Playback transport control and state snapshots.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
    runtime_dirty: Option<&'a AtomicBool>,
}

impl<'a> Transport<'a> {
    pub(crate) fn new(
        commands: &'a mut MultiThreadedKnystCommands,
        sequencer: Option<&'a Sequencer>,
        runtime_dirty: Option<&'a AtomicBool>,
    ) -> Self {
        Self {
            commands,
            sequencer,
            runtime_dirty,
        }
    }

    /// Starts playback.
    pub fn play(&mut self) {
        self.flush_runtime_if_dirty();
        let pending_position = self.sequencer.and_then(Sequencer::pending_position);
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        let start_beat = pending_position.unwrap_or(current_beat);
        if start_beat != current_beat {
            self.commands.transport_seek_to_beats(start_beat);
            wait_for_transport_beats(self.commands, start_beat);
        }
        if let Some(sequencer) = self.sequencer {
            sequencer.prepare_for_play(self.commands, start_beat);
            sequencer.set_playing(true);
        }
        self.commands.transport_play();
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        if let Some(sequencer) = self.sequencer {
            sequencer.reset_notes(self.commands);
            dispatch_immediate_changes(self.commands);
            sequencer.set_playing(false);
            sequencer.mark_dirty_at(current_beat);
        }
        self.commands.transport_pause();
    }

    /// Rewinds to the start and pauses playback.
    pub fn rewind(&mut self) {
        let was_playing = self
            .snapshot()
            .map(|snapshot| snapshot.playback_state == PlaybackState::Playing)
            .unwrap_or(false);
        if let Some(sequencer) = self.sequencer {
            sequencer.reset_notes(self.commands);
            dispatch_immediate_changes(self.commands);
            sequencer.set_playing(false);
        }
        self.commands.transport_pause();
        flush_scheduled_events(self.commands);
        self.commands.transport_seek_to_beats(Beats::ZERO);
        wait_for_transport_beats(self.commands, Beats::ZERO);
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_at(Beats::ZERO);
            if was_playing {
                sequencer.prepare_for_play(self.commands, Beats::ZERO);
                sequencer.set_playing(true);
            }
        }
        if was_playing {
            self.commands.transport_play();
        }
    }

    /// Seeks transport to an absolute seconds position.
    pub fn seek_seconds(&mut self, position: f64) {
        let was_playing = self
            .snapshot()
            .map(|snapshot| snapshot.playback_state == PlaybackState::Playing)
            .unwrap_or(false);
        if !was_playing {
            self.flush_runtime_if_dirty();
        }
        let position = Seconds::from_seconds_f64(position.max(0.0));
        if was_playing && let Some(sequencer) = self.sequencer {
            sequencer.reset_notes(self.commands);
            dispatch_immediate_changes(self.commands);
            sequencer.set_playing(false);
        }
        self.commands.transport_pause();
        if was_playing {
            flush_scheduled_events(self.commands);
        }
        self.commands.transport_seek_to_seconds(position);
        let target_beat = wait_for_transport_seconds(self.commands, position)
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_at(target_beat);
            if was_playing {
                sequencer.prepare_for_play(self.commands, target_beat);
                sequencer.set_playing(true);
            } else {
                sequencer.set_playing(false);
            }
        }
        if was_playing {
            self.commands.transport_play();
        }
    }

    /// Seeks transport to an absolute beats position.
    pub fn seek_beats(&mut self, position: f64) {
        let was_playing = self
            .snapshot()
            .map(|snapshot| snapshot.playback_state == PlaybackState::Playing)
            .unwrap_or(false);
        if !was_playing {
            self.flush_runtime_if_dirty();
        }
        let position = Beats::from_beats_f64(position.max(0.0));
        if was_playing && let Some(sequencer) = self.sequencer {
            sequencer.reset_notes(self.commands);
            dispatch_immediate_changes(self.commands);
            sequencer.set_playing(false);
        }
        self.commands.transport_pause();
        if was_playing {
            flush_scheduled_events(self.commands);
        }
        self.commands.transport_seek_to_beats(position);
        wait_for_transport_beats(self.commands, position);
        if let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_at(position);
            if was_playing {
                sequencer.prepare_for_play(self.commands, position);
                sequencer.set_playing(true);
            } else {
                sequencer.set_playing(false);
            }
        }
        if was_playing {
            self.commands.transport_play();
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

    fn flush_runtime_if_dirty(&mut self) {
        if self
            .runtime_dirty
            .is_some_and(|dirty| dirty.swap(false, Ordering::AcqRel))
        {
            flush_pending_runtime_changes(self.commands, Beats::ZERO);
        }
    }
}

fn flush_scheduled_events(commands: &mut MultiThreadedKnystCommands) {
    let flush_beat = Beats::from_beats(1_000_000);
    commands.transport_seek_to_beats(flush_beat);
    wait_for_transport_beats(commands, flush_beat);
}

fn flush_pending_runtime_changes(commands: &mut MultiThreadedKnystCommands, target_beat: Beats) {
    commands.transport_play();
    wait_for_transport_playing(commands);
    std::thread::sleep(Duration::from_millis(25));
    commands.transport_pause();
    commands.transport_seek_to_beats(Beats::ZERO);
    wait_for_transport_beats(commands, Beats::ZERO);
    if target_beat != Beats::ZERO {
        commands.transport_seek_to_beats(target_beat);
        wait_for_transport_beats(commands, target_beat);
    }
}

fn dispatch_immediate_changes(commands: &mut MultiThreadedKnystCommands) {
    commands.transport_play();
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(2));
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
            continue;
        };
        if snapshot.state == TransportState::Playing {
            break;
        }
    }
}

fn wait_for_transport_beats(commands: &mut MultiThreadedKnystCommands, target_beats: Beats) {
    for _ in 0..50 {
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
            continue;
        };

        let beats_match = snapshot.beats.unwrap_or(Beats::ZERO) == target_beats;
        if beats_match {
            return;
        }
    }
}

fn wait_for_transport_playing(commands: &mut MultiThreadedKnystCommands) {
    for _ in 0..50 {
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
            continue;
        };

        if snapshot.state == TransportState::Playing {
            return;
        }
    }
}

fn wait_for_transport_seconds(
    commands: &mut MultiThreadedKnystCommands,
    target_seconds: Seconds,
) -> Option<TransportSnapshot> {
    for _ in 0..50 {
        let Ok(Some(snapshot)) = commands
            .request_transport_snapshot()
            .recv_timeout(Duration::from_millis(2))
        else {
            continue;
        };

        if (snapshot.seconds.to_seconds_f64() - target_seconds.to_seconds_f64()).abs() <= 1.0e-6 {
            return Some(TransportSnapshot::new(
                match snapshot.state {
                    TransportState::Playing => PlaybackState::Playing,
                    TransportState::Paused => PlaybackState::Paused,
                },
                snapshot.seconds,
                snapshot.beats.unwrap_or(Beats::ZERO),
            ));
        }
    }

    None
}
