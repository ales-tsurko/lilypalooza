//! Playback transport control and state snapshots.

use std::time::Duration;

use knyst::prelude::{Beats, KnystCommands, MultiThreadedKnystCommands, Seconds, TransportState};

use crate::mixer::Mixer;
use crate::sequencer::Sequencer;

const CONTROLLER_BARRIER_TIMEOUT: Duration = Duration::from_millis(250);
const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
const TRANSPORT_POLL_INTERVAL: Duration = Duration::from_millis(2);

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
    mixer: Option<&'a mut Mixer>,
    sequencer: Option<&'a Sequencer>,
}

impl<'a> Transport<'a> {
    pub(crate) fn new(
        commands: &'a mut MultiThreadedKnystCommands,
        mixer: Option<&'a mut Mixer>,
        sequencer: Option<&'a Sequencer>,
    ) -> Self {
        Self {
            commands,
            mixer,
            sequencer,
        }
    }

    /// Starts playback.
    pub fn play(&mut self) {
        let pending_position = self.sequencer.and_then(Sequencer::pending_position);
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        let start_beat = pending_position.unwrap_or(current_beat);
        if start_beat != current_beat {
            self.commands.transport_seek_to_beats(start_beat);
            wait_for_transport_settled(self.commands);
            wait_for_transport_beats(self.commands, start_beat);
        }
        if let Some(sequencer) = self.sequencer {
            sequencer.prepare_for_play(self.commands, start_beat);
        }
        self.commands.transport_play();
        wait_for_transport_settled(self.commands);
        wait_for_transport_state(self.commands, TransportState::Playing);
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(true);
        }
    }

    /// Starts playback immediately without blocking for transport settlement.
    ///
    /// This is intended for interactive UI toggles where responsiveness matters
    /// more than synchronously waiting for the playing transport state to be
    /// fully observed on the calling thread.
    pub fn play_immediate(&mut self) {
        let pending_position = self.sequencer.and_then(Sequencer::pending_position);
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        let start_beat = pending_position.unwrap_or(current_beat);
        if start_beat != current_beat {
            self.commands.transport_seek_to_beats(start_beat);
        }
        if let Some(sequencer) = self.sequencer {
            sequencer.prepare_for_play(self.commands, start_beat);
        }
        self.commands.transport_play();
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(true);
        }
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(false);
        }
        if let Some(mixer) = self.mixer.as_deref() {
            mixer.reset_meters();
        }
        if !has_loaded_score {
            self.commands.transport_pause();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Paused);
            return;
        }
        self.commands.clear_scheduled_changes();
        wait_for_controller_barrier(self.commands);
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.prepare_for_pause_immediate(self.commands);
        }
        self.commands.transport_pause();
        wait_for_transport_settled(self.commands);
        wait_for_transport_state(self.commands, TransportState::Paused);
        self.commands.transport_seek_to_beats(current_beat);
        wait_for_transport_settled(self.commands);
        wait_for_transport_beats(self.commands, current_beat);
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_for_seek(current_beat, false);
        }
    }

    /// Pauses playback immediately without blocking for transport settlement.
    ///
    /// This is intended for interactive UI toggles where responsiveness matters
    /// more than synchronously waiting for the paused transport state to be
    /// fully observed on the calling thread.
    pub fn pause_immediate(&mut self) {
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        let current_beat = self
            .snapshot()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(false);
        }
        if let Some(mixer) = self.mixer.as_deref() {
            mixer.reset_meters();
        }
        self.commands.clear_scheduled_changes();
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.prepare_for_pause_immediate(self.commands);
        }
        self.commands.transport_pause();
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_for_seek(current_beat, false);
        }
    }

    /// Rewinds to the start and pauses playback.
    pub fn rewind(&mut self) {
        let was_playing = self.sequencer.is_some_and(Sequencer::is_playing);
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        if was_playing {
            self.seek_beats(0.0);
        } else {
            if !has_loaded_score {
                if let Some(sequencer) = self.sequencer {
                    sequencer.set_playing(false);
                }
                self.commands.transport_pause();
                wait_for_transport_settled(self.commands);
                wait_for_transport_state(self.commands, TransportState::Paused);
                self.commands.transport_seek_to_beats(Beats::ZERO);
                wait_for_transport_settled(self.commands);
                wait_for_transport_beats(self.commands, Beats::ZERO);
                return;
            }
            self.commands.transport_pause();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Paused);
            self.commands.clear_scheduled_changes();
            wait_for_controller_barrier(self.commands);
            if let Some(sequencer) = self.sequencer {
                sequencer.set_playing(false);
            }
            self.commands.transport_seek_to_beats(Beats::ZERO);
            wait_for_transport_settled(self.commands);
            wait_for_transport_beats(self.commands, Beats::ZERO);
            if has_loaded_score && let Some(sequencer) = self.sequencer {
                sequencer.mark_dirty_for_seek(Beats::ZERO, false);
            }
        }
    }

    /// Seeks transport to an absolute seconds position.
    pub fn seek_seconds(&mut self, position: f64) {
        let was_playing = self.sequencer.is_some_and(Sequencer::is_playing);
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        let position = Seconds::from_seconds_f64(position.max(0.0));
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(false);
        }
        if let Some(mixer) = self.mixer.as_deref() {
            mixer.reset_meters();
        }
        if !has_loaded_score {
            self.commands.transport_pause();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Paused);
            self.commands.transport_seek_to_seconds(position);
            wait_for_transport_settled(self.commands);
            wait_for_transport_seconds(self.commands, position);
            if was_playing {
                self.commands.transport_play();
                wait_for_transport_settled(self.commands);
                wait_for_transport_state(self.commands, TransportState::Playing);
            }
            return;
        }
        self.commands.clear_scheduled_changes();
        wait_for_controller_barrier(self.commands);
        if was_playing
            && has_loaded_score
            && let Some(sequencer) = self.sequencer
        {
            sequencer.prepare_for_pause_immediate(self.commands);
        }
        self.commands.transport_pause();
        wait_for_transport_settled(self.commands);
        wait_for_transport_state(self.commands, TransportState::Paused);
        self.commands.transport_seek_to_seconds(position);
        wait_for_transport_settled(self.commands);
        wait_for_transport_seconds(self.commands, position);
        let target_beat = self
            .snapshot()
            .ok()
            .map(|snapshot| snapshot.beats_position)
            .unwrap_or(Beats::ZERO);
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_for_seek(target_beat, was_playing);
            sequencer.set_playing(false);
        }
        if was_playing {
            self.commands.transport_play();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Playing);
            if has_loaded_score && let Some(sequencer) = self.sequencer {
                sequencer.set_playing(true);
            }
        }
    }

    /// Seeks transport to an absolute beats position.
    pub fn seek_beats(&mut self, position: f64) {
        let was_playing = self.sequencer.is_some_and(Sequencer::is_playing);
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        let position = Beats::from_beats_f64(position.max(0.0));
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(false);
        }
        if let Some(mixer) = self.mixer.as_deref() {
            mixer.reset_meters();
        }
        if !has_loaded_score {
            self.commands.transport_pause();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Paused);
            self.commands.transport_seek_to_beats(position);
            wait_for_transport_settled(self.commands);
            wait_for_transport_beats(self.commands, position);
            if was_playing {
                self.commands.transport_play();
                wait_for_transport_settled(self.commands);
                wait_for_transport_state(self.commands, TransportState::Playing);
            }
            return;
        }
        self.commands.clear_scheduled_changes();
        wait_for_controller_barrier(self.commands);
        if was_playing
            && has_loaded_score
            && let Some(sequencer) = self.sequencer
        {
            sequencer.prepare_for_pause_immediate(self.commands);
        }
        self.commands.transport_pause();
        wait_for_transport_settled(self.commands);
        wait_for_transport_state(self.commands, TransportState::Paused);
        self.commands.transport_seek_to_beats(position);
        wait_for_transport_settled(self.commands);
        wait_for_transport_beats(self.commands, position);
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_for_seek(position, was_playing);
            sequencer.set_playing(false);
        }
        if was_playing {
            self.commands.transport_play();
            wait_for_transport_settled(self.commands);
            wait_for_transport_state(self.commands, TransportState::Playing);
            if has_loaded_score && let Some(sequencer) = self.sequencer {
                sequencer.set_playing(true);
            }
        }
    }

    /// Seeks transport immediately without blocking for transport settlement.
    ///
    /// This is intended for interactive UI seeking. Exact settled seeking is
    /// still available through `seek_beats`.
    pub fn seek_beats_immediate(&mut self, position: f64) {
        let was_playing = self.sequencer.is_some_and(Sequencer::is_playing);
        let has_loaded_score = self.sequencer.is_some_and(Sequencer::has_loaded_score);
        let position = Beats::from_beats_f64(position.max(0.0));
        if let Some(sequencer) = self.sequencer {
            sequencer.set_playing(false);
        }
        if let Some(mixer) = self.mixer.as_deref() {
            mixer.reset_meters();
        }
        self.commands.clear_scheduled_changes();
        if was_playing
            && has_loaded_score
            && let Some(sequencer) = self.sequencer
        {
            sequencer.prepare_for_pause_immediate(self.commands);
        }
        self.commands.transport_pause();
        self.commands.transport_seek_to_beats(position);
        if has_loaded_score && let Some(sequencer) = self.sequencer {
            sequencer.mark_dirty_for_seek(position, was_playing);
            sequencer.set_playing(was_playing);
        }
        if was_playing {
            self.commands.transport_play();
        }
    }

    /// Reads current transport state from Knyst.
    pub fn snapshot(&mut self) -> Result<TransportSnapshot, TransportError> {
        let snapshot = self
            .commands
            .current_transport_snapshot()
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

fn wait_for_controller_barrier(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_snapshot();
    let _ = receiver.recv_timeout(CONTROLLER_BARRIER_TIMEOUT);
}

fn wait_for_transport_settled(commands: &mut MultiThreadedKnystCommands) {
    let receiver = commands.request_transport_settled();
    let _ = receiver.recv_timeout(SETTLE_TIMEOUT);
}

fn wait_for_transport_state(commands: &mut MultiThreadedKnystCommands, expected: TransportState) {
    let start = std::time::Instant::now();
    while start.elapsed() < SETTLE_TIMEOUT {
        if commands
            .current_transport_snapshot()
            .is_some_and(|snapshot| snapshot.state == expected)
        {
            return;
        }
        std::thread::sleep(TRANSPORT_POLL_INTERVAL);
    }
}

fn wait_for_transport_beats(commands: &mut MultiThreadedKnystCommands, expected: Beats) {
    let start = std::time::Instant::now();
    while start.elapsed() < SETTLE_TIMEOUT {
        if commands
            .current_transport_snapshot()
            .and_then(|snapshot| snapshot.beats)
            .is_some_and(|beats| beats == expected)
        {
            return;
        }
        std::thread::sleep(TRANSPORT_POLL_INTERVAL);
    }
}

fn wait_for_transport_seconds(commands: &mut MultiThreadedKnystCommands, expected: Seconds) {
    let start = std::time::Instant::now();
    while start.elapsed() < SETTLE_TIMEOUT {
        if commands
            .current_transport_snapshot()
            .is_some_and(|snapshot| snapshot.seconds == expected)
        {
            return;
        }
        std::thread::sleep(TRANSPORT_POLL_INTERVAL);
    }
}
