use serde::{Deserialize, Serialize};

use super::runtime;
#[cfg(test)]
use super::state_helpers::send_level_only_changed;
pub use super::track::{
    BusId, BusSend, INSTRUMENT_TRACK_COUNT, Track, TrackId, TrackRoute, TrackRouting, TrackState,
};
use crate::{instrument::SlotState, soundfont::SoundfontResource};

/// Strip balance/meter processing implementation selected for manual benchmarks.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceMeterBenchmarkPath {
    /// Scalar implementation.
    Scalar,
    /// SIMD implementation.
    Simd,
}

/// Buffer set passed to strip balance/meter benchmark implementations.
#[doc(hidden)]
#[derive(Debug)]
pub struct BalanceMeterBenchmarkBlock<'a> {
    pub left_in: &'a [f32],
    pub right_in: &'a [f32],
    pub left_out: &'a mut [f32],
    pub right_out: &'a mut [f32],
    pub left_mul: f32,
    pub right_mul: f32,
    pub frames: usize,
}

/// Runs one strip balance/meter processing path for manual benchmarks.
#[doc(hidden)]
pub fn benchmark_process_stereo_balance_meter(
    path: BalanceMeterBenchmarkPath,
    block: BalanceMeterBenchmarkBlock<'_>,
) -> (f32, f32) {
    match path {
        BalanceMeterBenchmarkPath::Scalar => runtime::process_stereo_balance_meter_scalar(
            block.left_in,
            block.right_in,
            block.left_out,
            block.right_out,
            block.left_mul,
            block.right_mul,
            block.frames,
        ),
        BalanceMeterBenchmarkPath::Simd => runtime::process_stereo_balance_meter_simd(
            block.left_in,
            block.right_in,
            block.left_out,
            block.right_out,
            block.left_mul,
            block.right_mul,
            block.frames,
        ),
    }
}

/// Strip meter minimum displayed level in dBFS.
pub const STRIP_METER_MIN_DB: f32 = -60.0;
/// Strip meter maximum displayed level in dBFS.
pub const STRIP_METER_MAX_DB: f32 = 0.0;

/// Mixer model error.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MixerError {
    /// Track id is outside the fixed track range.
    #[error("track id {0:?} is out of bounds")]
    InvalidTrackId(TrackId),
    /// Bus id does not exist.
    #[error("bus id {0:?} does not exist")]
    InvalidBusId(BusId),
    /// One bus cannot route to itself.
    #[error("bus {0:?} cannot route to itself")]
    SelfRouting(BusId),
    /// Routing would create a feedback cycle.
    #[error("routing from bus {source_id:?} to bus {destination_id:?} would create feedback")]
    FeedbackRouting {
        /// Source bus.
        source_id: BusId,
        /// Destination bus.
        destination_id: BusId,
    },
    /// Bus send index is outside the current send list.
    #[error("bus send index {index} is out of bounds (len: {len})")]
    BusSendIndexOutOfBounds {
        /// Requested send index.
        index: usize,
        /// Current send count.
        len: usize,
    },
    /// SoundFont id does not exist.
    #[error("soundfont id `{0}` does not exist")]
    InvalidSoundfontId(String),
    /// Slot address is outside the current strip layout.
    #[error("slot address strip {strip_index} slot {slot_index} is invalid")]
    InvalidSlotAddress {
        /// Visible strip index.
        strip_index: usize,
        /// Unified slot index inside that strip.
        slot_index: usize,
    },
}

/// One channel meter snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelMeterSnapshot {
    /// Normalized level in `0..=1`.
    pub level: f32,
    /// Normalized hold marker in `0..=1`.
    pub hold: f32,
    /// Unclamped held peak in dBFS.
    pub hold_db: f32,
}

impl Default for ChannelMeterSnapshot {
    fn default() -> Self {
        Self {
            level: 0.0,
            hold: 0.0,
            hold_db: STRIP_METER_MIN_DB,
        }
    }
}

/// One stereo strip meter snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StripMeterSnapshot {
    /// Left channel meter state.
    pub left: ChannelMeterSnapshot,
    /// Right channel meter state.
    pub right: ChannelMeterSnapshot,
    /// Latched clip state.
    pub clip_latched: bool,
}

/// Full mixer meter snapshot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MixerMeterSnapshot {
    /// Main strip meter state.
    pub main: StripMeterSnapshot,
    /// Instrument track meter states.
    pub tracks: Vec<StripMeterSnapshot>,
    /// Bus strip meter states.
    pub buses: Vec<(BusId, StripMeterSnapshot)>,
}

/// Visible mixer meter window snapshot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MixerMeterSnapshotWindow {
    /// Main strip meter state.
    pub main: StripMeterSnapshot,
    /// Visible instrument track meter states in window order.
    pub tracks: Vec<StripMeterSnapshot>,
    /// Visible bus meter states in window order.
    pub buses: Vec<StripMeterSnapshot>,
}

/// One processor slot address in visible mixer order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotAddress {
    /// Visible strip index.
    pub strip_index: usize,
    /// Unified slot index.
    ///
    /// Convention:
    /// - `0` is the instrument slot
    /// - `1..` are effect slots
    pub slot_index: usize,
}

/// Serializable mixer state with fixed instrument tracks, dynamic buses, and a dedicated master.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerState {
    strips: Vec<Track>,
    soundfonts: Vec<SoundfontResource>,
    next_bus_id: u16,
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

impl MixerState {
    /// Creates the mixer.
    #[must_use]
    pub fn new() -> Self {
        let mut strips = Vec::with_capacity(1 + INSTRUMENT_TRACK_COUNT);
        strips.push(Track::master());
        strips.extend(
            (0..INSTRUMENT_TRACK_COUNT).map(|index| Track::instrument(TrackId(index as u16))),
        );
        Self {
            strips,
            soundfonts: Vec::new(),
            next_bus_id: 1,
        }
    }

    /// Returns the number of fixed instrument tracks.
    #[must_use]
    pub fn track_count(&self) -> usize {
        INSTRUMENT_TRACK_COUNT
    }

    /// Returns the number of dynamic bus tracks.
    #[must_use]
    pub fn bus_count(&self) -> usize {
        self.strips.len().saturating_sub(1 + INSTRUMENT_TRACK_COUNT)
    }

    /// Returns the total visible strip count including the master strip.
    #[must_use]
    pub fn strip_count(&self) -> usize {
        self.strips.len()
    }

    /// Returns one strip by visible mixer index.
    #[must_use]
    pub fn strip_by_index(&self, strip_index: usize) -> Option<&Track> {
        self.strips.get(strip_index)
    }

    /// Returns one slot by visible strip and unified slot indices.
    #[must_use]
    pub fn slot(&self, address: SlotAddress) -> Option<&SlotState> {
        self.strip_by_index(address.strip_index)
            .and_then(|strip| strip.slot(address.slot_index))
    }

    /// Returns immutable access to one instrument track.
    pub fn track(&self, id: TrackId) -> Result<&Track, MixerError> {
        self.strips
            .get(1 + id.index())
            .ok_or(MixerError::InvalidTrackId(id))
    }

    /// Returns all fixed instrument tracks.
    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        self.strips
            .get(1..1 + INSTRUMENT_TRACK_COUNT)
            .unwrap_or(&[])
    }

    /// Returns all fixed instrument tracks with their stable ids.
    pub fn tracks_with_ids(&self) -> impl Iterator<Item = (TrackId, &Track)> {
        self.tracks()
            .iter()
            .enumerate()
            .map(|(index, track)| (TrackId(index as u16), track))
    }

    /// Returns mutable access to one instrument track.
    pub(crate) fn track_mut(&mut self, id: TrackId) -> Result<&mut Track, MixerError> {
        self.strips
            .get_mut(1 + id.index())
            .ok_or(MixerError::InvalidTrackId(id))
    }

    /// Returns immutable access to one bus.
    pub fn bus(&self, id: BusId) -> Result<&Track, MixerError> {
        self.buses()
            .iter()
            .find(|bus| bus.bus_id == Some(id))
            .ok_or(MixerError::InvalidBusId(id))
    }

    /// Returns all dynamic buses.
    #[must_use]
    pub fn buses(&self) -> &[Track] {
        self.strips.get(1 + INSTRUMENT_TRACK_COUNT..).unwrap_or(&[])
    }

    /// Returns all dynamic bus tracks with their stable ids.
    pub fn buses_with_ids(&self) -> impl Iterator<Item = (BusId, &Track)> {
        self.buses()
            .iter()
            .filter_map(|bus| bus.bus_id.map(|bus_id| (bus_id, bus)))
    }

    /// Returns the dedicated master track.
    #[must_use]
    pub fn master(&self) -> &Track {
        let Some((master, _)) = self.strips.split_first() else {
            std::process::abort();
        };
        master
    }

    /// Returns mutable access to the dedicated master track.
    #[must_use]
    pub(crate) fn master_mut(&mut self) -> &mut Track {
        let Some((master, _)) = self.strips.split_first_mut() else {
            std::process::abort();
        };
        master
    }

    /// Returns all configured shared SoundFonts.
    #[must_use]
    pub fn soundfonts(&self) -> &[SoundfontResource] {
        &self.soundfonts
    }

    /// Returns mutable access to one bus.
    pub(crate) fn bus_mut(&mut self, id: BusId) -> Result<&mut Track, MixerError> {
        self.strips
            .get_mut(1 + INSTRUMENT_TRACK_COUNT..)
            .unwrap_or(&mut [])
            .iter_mut()
            .find(|bus| bus.bus_id == Some(id))
            .ok_or(MixerError::InvalidBusId(id))
    }

    pub(crate) fn slot_mut(&mut self, address: SlotAddress) -> Result<&mut SlotState, MixerError> {
        self.strips
            .get_mut(address.strip_index)
            .and_then(|strip| strip.slot_mut(address.slot_index))
            .ok_or(MixerError::InvalidSlotAddress {
                strip_index: address.strip_index,
                slot_index: address.slot_index,
            })
    }

    /// Adds one dynamic bus.
    pub(crate) fn add_bus(&mut self, name: impl Into<String>) -> BusId {
        let id = BusId(self.next_bus_id);
        self.next_bus_id = self.next_bus_id.saturating_add(1);
        self.strips.push(Track::bus(id, name));
        id
    }

    /// Removes one dynamic bus and reroutes everything targeting it back to master.
    pub(crate) fn remove_bus(&mut self, id: BusId) -> Result<Track, MixerError> {
        let index = self
            .strips
            .iter()
            .position(|strip| strip.bus_id == Some(id))
            .ok_or(MixerError::InvalidBusId(id))?;
        let removed = self.strips.remove(index);

        for strip in self.strips.get_mut(1..).unwrap_or(&mut []) {
            if strip.routing.main == TrackRoute::Bus(id) {
                strip.routing.main = TrackRoute::Master;
            }
            strip.routing.sends.retain(|send| send.bus_id != id);
        }

        Ok(removed)
    }

    /// Sets the main route for one instrument track.
    pub(crate) fn set_track_route(
        &mut self,
        id: TrackId,
        route: TrackRoute,
    ) -> Result<(), MixerError> {
        self.validate_track_route(route)?;
        self.track_mut(id)?.routing.main = route;
        Ok(())
    }

    /// Sets the main route for one bus track.
    pub(crate) fn set_bus_route(&mut self, id: BusId, route: TrackRoute) -> Result<(), MixerError> {
        self.validate_bus_route(id, route)?;
        self.bus_mut(id)?.routing.main = route;
        Ok(())
    }

    /// Replaces one instrument-track routing state.
    pub(crate) fn set_track_routing(
        &mut self,
        id: TrackId,
        routing: TrackRouting,
    ) -> Result<(), MixerError> {
        self.validate_track_route(routing.main)?;
        self.validate_sends(routing.sends.iter().copied())?;
        self.track_mut(id)?.routing = routing;
        Ok(())
    }

    /// Replaces one bus-track routing state.
    pub(crate) fn set_bus_routing(
        &mut self,
        id: BusId,
        routing: TrackRouting,
    ) -> Result<(), MixerError> {
        self.validate_bus_route(id, routing.main)?;
        self.validate_bus_sends(id, routing.sends.iter().copied())?;
        self.bus_mut(id)?.routing = routing;
        Ok(())
    }

    /// Adds one parallel bus send to an instrument track.
    pub(crate) fn add_track_bus_send(
        &mut self,
        id: TrackId,
        send: BusSend,
    ) -> Result<(), MixerError> {
        self.validate_send(send)?;
        self.track_mut(id)?.routing.sends.push(send);
        Ok(())
    }

    /// Adds one parallel bus send to a bus.
    pub(crate) fn add_bus_send(&mut self, id: BusId, send: BusSend) -> Result<(), MixerError> {
        self.validate_bus_send(id, send)?;
        self.bus_mut(id)?.routing.sends.push(send);
        Ok(())
    }

    /// Replaces one parallel bus send on an instrument track.
    pub(crate) fn set_track_bus_send(
        &mut self,
        id: TrackId,
        index: usize,
        send: BusSend,
    ) -> Result<(), MixerError> {
        self.validate_send(send)?;
        replace_bus_send(&mut self.track_mut(id)?.routing.sends, index, send)
    }

    /// Replaces one parallel bus send on a bus.
    pub(crate) fn set_bus_send(
        &mut self,
        id: BusId,
        index: usize,
        send: BusSend,
    ) -> Result<(), MixerError> {
        self.validate_bus_send(id, send)?;
        replace_bus_send(&mut self.bus_mut(id)?.routing.sends, index, send)
    }

    /// Removes one parallel bus send from an instrument track.
    pub(crate) fn remove_track_bus_send(
        &mut self,
        id: TrackId,
        index: usize,
    ) -> Result<BusSend, MixerError> {
        remove_bus_send_at(&mut self.track_mut(id)?.routing.sends, index)
    }

    /// Removes one parallel bus send from a bus.
    pub(crate) fn remove_bus_send(
        &mut self,
        id: BusId,
        index: usize,
    ) -> Result<BusSend, MixerError> {
        remove_bus_send_at(&mut self.bus_mut(id)?.routing.sends, index)
    }

    fn validate_track_route(&self, route: TrackRoute) -> Result<(), MixerError> {
        match route {
            TrackRoute::Master => Ok(()),
            TrackRoute::Bus(bus_id) => {
                self.bus(bus_id)?;
                Ok(())
            }
        }
    }

    fn validate_bus_route(&self, source_id: BusId, route: TrackRoute) -> Result<(), MixerError> {
        match route {
            TrackRoute::Master => Ok(()),
            TrackRoute::Bus(bus_id) => {
                if source_id == bus_id {
                    return Err(MixerError::SelfRouting(source_id));
                }
                self.bus(bus_id)?;
                self.validate_no_feedback(source_id, bus_id)?;
                Ok(())
            }
        }
    }

    fn validate_sends(&self, sends: impl IntoIterator<Item = BusSend>) -> Result<(), MixerError> {
        for send in sends {
            self.validate_send(send)?;
        }
        Ok(())
    }

    fn validate_bus_sends(
        &self,
        source_id: BusId,
        sends: impl IntoIterator<Item = BusSend>,
    ) -> Result<(), MixerError> {
        for send in sends {
            self.validate_bus_send(source_id, send)?;
        }
        Ok(())
    }

    fn validate_send(&self, send: BusSend) -> Result<(), MixerError> {
        self.bus(send.bus_id)?;
        Ok(())
    }

    fn validate_bus_send(&self, source_id: BusId, send: BusSend) -> Result<(), MixerError> {
        if source_id == send.bus_id {
            return Err(MixerError::SelfRouting(source_id));
        }
        self.bus(send.bus_id)?;
        self.validate_no_feedback(source_id, send.bus_id)?;
        Ok(())
    }

    /// Returns `true` when adding `source_id -> destination_id` would keep the routing graph
    /// acyclic.
    pub fn can_route_bus_to_bus(&self, source_id: BusId, destination_id: BusId) -> bool {
        source_id != destination_id
            && self.bus(source_id).is_ok()
            && self.bus(destination_id).is_ok()
            && !self.bus_reaches_bus(destination_id, source_id)
    }

    fn validate_no_feedback(
        &self,
        source_id: BusId,
        destination_id: BusId,
    ) -> Result<(), MixerError> {
        if self.bus_reaches_bus(destination_id, source_id) {
            return Err(MixerError::FeedbackRouting {
                source_id,
                destination_id,
            });
        }
        Ok(())
    }

    fn bus_reaches_bus(&self, start: BusId, target: BusId) -> bool {
        let mut stack = vec![start];
        let mut visited = Vec::new();

        while let Some(bus_id) = stack.pop() {
            if bus_id == target {
                return true;
            }
            if visited.contains(&bus_id) {
                continue;
            }
            visited.push(bus_id);

            let Ok(bus) = self.bus(bus_id) else {
                continue;
            };
            if let TrackRoute::Bus(next) = bus.routing.main {
                stack.push(next);
            }
            stack.extend(bus.routing.sends.iter().map(|send| send.bus_id));
        }

        false
    }

    pub(crate) fn set_soundfont(&mut self, resource: SoundfontResource) {
        if let Some(existing) = self.soundfonts.iter_mut().find(|sf| sf.id == resource.id) {
            *existing = resource;
        } else {
            self.soundfonts.push(resource);
            self.soundfonts
                .sort_by(|left, right| left.id.cmp(&right.id));
        }
    }

    pub(crate) fn remove_soundfont(&mut self, id: &str) -> Result<SoundfontResource, MixerError> {
        let index = self
            .soundfonts
            .iter()
            .position(|sf| sf.id == id)
            .ok_or_else(|| MixerError::InvalidSoundfontId(id.to_string()))?;
        Ok(self.soundfonts.remove(index))
    }
}

fn replace_bus_send(sends: &mut [BusSend], index: usize, send: BusSend) -> Result<(), MixerError> {
    let len = sends.len();
    let existing = sends
        .get_mut(index)
        .ok_or(MixerError::BusSendIndexOutOfBounds { index, len })?;
    *existing = send;
    Ok(())
}

fn remove_bus_send_at(sends: &mut Vec<BusSend>, index: usize) -> Result<BusSend, MixerError> {
    let len = sends.len();
    if index >= len {
        return Err(MixerError::BusSendIndexOutOfBounds { index, len });
    }
    Ok(sends.remove(index))
}

#[cfg(test)]
mod state_tests;
