//! Fixed instrument tracks plus dynamic buses.

pub(crate) mod runtime;
mod track;

use std::ops::Deref;

use crate::engine::AudioEngineError;
use crate::instrument::InstrumentConfig;
use knyst::modal_interface::KnystContext;
use knyst::prelude::MultiThreadedKnystCommands;
use runtime::{MixerRuntime, MixerRuntimeError};
use serde::{Deserialize, Serialize};
pub use track::{
    BusId, BusSend, BusTrack, INSTRUMENT_TRACK_COUNT, MasterTrack, MixerTrack, TrackId, TrackRoute,
    TrackRouting, TrackState,
};

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
    /// Bus send index is outside the current send list.
    #[error("bus send index {index} is out of bounds (len: {len})")]
    BusSendIndexOutOfBounds {
        /// Requested send index.
        index: usize,
        /// Current send count.
        len: usize,
    },
}

/// Serializable mixer state with fixed instrument tracks, dynamic buses, and a dedicated master.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerState {
    tracks: Vec<MixerTrack>,
    buses: Vec<BusTrack>,
    master: MasterTrack,
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
        let tracks = (0..INSTRUMENT_TRACK_COUNT)
            .map(|index| MixerTrack::new(TrackId(index as u16)))
            .collect();
        Self {
            tracks,
            buses: Vec::new(),
            master: MasterTrack::default(),
            next_bus_id: 1,
        }
    }

    /// Returns the number of fixed instrument tracks.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Returns the number of dynamic bus tracks.
    #[must_use]
    pub fn bus_count(&self) -> usize {
        self.buses.len()
    }

    /// Returns immutable access to one instrument track.
    pub fn track(&self, id: TrackId) -> Result<&MixerTrack, MixerError> {
        self.tracks
            .get(id.index())
            .ok_or(MixerError::InvalidTrackId(id))
    }

    /// Returns all fixed instrument tracks.
    #[must_use]
    pub fn tracks(&self) -> &[MixerTrack] {
        &self.tracks
    }

    /// Returns mutable access to one instrument track.
    pub(crate) fn track_mut(&mut self, id: TrackId) -> Result<&mut MixerTrack, MixerError> {
        self.tracks
            .get_mut(id.index())
            .ok_or(MixerError::InvalidTrackId(id))
    }

    /// Returns immutable access to one bus.
    pub fn bus(&self, id: BusId) -> Result<&BusTrack, MixerError> {
        self.buses
            .iter()
            .find(|bus| bus.id == id)
            .ok_or(MixerError::InvalidBusId(id))
    }

    /// Returns all dynamic buses.
    #[must_use]
    pub fn buses(&self) -> &[BusTrack] {
        &self.buses
    }

    /// Returns the dedicated master track.
    #[must_use]
    pub fn master(&self) -> &MasterTrack {
        &self.master
    }

    /// Returns mutable access to one bus.
    pub(crate) fn bus_mut(&mut self, id: BusId) -> Result<&mut BusTrack, MixerError> {
        self.buses
            .iter_mut()
            .find(|bus| bus.id == id)
            .ok_or(MixerError::InvalidBusId(id))
    }

    /// Adds one dynamic bus.
    pub(crate) fn add_bus(&mut self, name: impl Into<String>) -> BusId {
        let id = BusId(self.next_bus_id);
        self.next_bus_id = self.next_bus_id.saturating_add(1);
        self.buses.push(BusTrack::new(id, name));
        id
    }

    /// Removes one dynamic bus and reroutes everything targeting it back to master.
    pub(crate) fn remove_bus(&mut self, id: BusId) -> Result<BusTrack, MixerError> {
        let index = self
            .buses
            .iter()
            .position(|bus| bus.id == id)
            .ok_or(MixerError::InvalidBusId(id))?;
        let removed = self.buses.remove(index);

        for track in &mut self.tracks {
            if track.routing.main == TrackRoute::Bus(id) {
                track.routing.main = TrackRoute::Master;
            }
            track.routing.sends.retain(|send| send.bus_id != id);
        }
        for bus in &mut self.buses {
            if bus.routing.main == TrackRoute::Bus(id) {
                bus.routing.main = TrackRoute::Master;
            }
            bus.routing.sends.retain(|send| send.bus_id != id);
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
        let track = self.track_mut(id)?;
        let len = track.routing.sends.len();
        if index >= len {
            return Err(MixerError::BusSendIndexOutOfBounds { index, len });
        }
        track.routing.sends[index] = send;
        Ok(())
    }

    /// Replaces one parallel bus send on a bus.
    pub(crate) fn set_bus_send(
        &mut self,
        id: BusId,
        index: usize,
        send: BusSend,
    ) -> Result<(), MixerError> {
        self.validate_bus_send(id, send)?;
        let bus = self.bus_mut(id)?;
        let len = bus.routing.sends.len();
        if index >= len {
            return Err(MixerError::BusSendIndexOutOfBounds { index, len });
        }
        bus.routing.sends[index] = send;
        Ok(())
    }

    /// Removes one parallel bus send from an instrument track.
    pub(crate) fn remove_track_bus_send(
        &mut self,
        id: TrackId,
        index: usize,
    ) -> Result<BusSend, MixerError> {
        let track = self.track_mut(id)?;
        let len = track.routing.sends.len();
        if index >= len {
            return Err(MixerError::BusSendIndexOutOfBounds { index, len });
        }
        Ok(track.routing.sends.remove(index))
    }

    /// Removes one parallel bus send from a bus.
    pub(crate) fn remove_bus_send(
        &mut self,
        id: BusId,
        index: usize,
    ) -> Result<BusSend, MixerError> {
        let bus = self.bus_mut(id)?;
        let len = bus.routing.sends.len();
        if index >= len {
            return Err(MixerError::BusSendIndexOutOfBounds { index, len });
        }
        Ok(bus.routing.sends.remove(index))
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
        Ok(())
    }
}

pub(crate) struct Mixer {
    pub(crate) state: MixerState,
    pub(crate) runtime: MixerRuntime,
}

impl Mixer {
    pub(crate) fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        state: MixerState,
    ) -> Result<Self, MixerRuntimeError> {
        let runtime = MixerRuntime::attach(context, commands, &state)?;
        Ok(Self { state, runtime })
    }
}

/// Mutable mixer control handle.
#[allow(missing_docs)]
pub struct MixerHandle<'a> {
    mixer: &'a mut Mixer,
    context: &'a KnystContext,
    commands: &'a mut MultiThreadedKnystCommands,
}

impl<'a> MixerHandle<'a> {
    pub(crate) fn new(
        mixer: &'a mut Mixer,
        context: &'a KnystContext,
        commands: &'a mut MultiThreadedKnystCommands,
    ) -> MixerHandle<'a> {
        Self {
            mixer,
            context,
            commands,
        }
    }
}

impl Deref for MixerHandle<'_> {
    type Target = MixerState;

    fn deref(&self) -> &Self::Target {
        &self.mixer.state
    }
}

#[allow(missing_docs)]
impl MixerHandle<'_> {
    pub fn add_bus(&mut self, name: impl Into<String>) -> Result<BusId, AudioEngineError> {
        let bus_id = self.mixer.state.add_bus(name);
        self.mixer
            .runtime
            .add_bus(self.context, self.commands, &self.mixer.state, bus_id)?;
        Ok(bus_id)
    }

    pub fn remove_bus(&mut self, id: BusId) -> Result<BusTrack, AudioEngineError> {
        let removed = self.mixer.state.remove_bus(id)?;
        self.mixer
            .runtime
            .remove_bus(self.commands, &self.mixer.state, id)?;
        Ok(removed)
    }

    pub fn set_track_instrument(
        &mut self,
        id: TrackId,
        instrument: InstrumentConfig,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.instrument = instrument;
        self.mixer.runtime.sync_track_instrument(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(())
    }

    pub fn set_track_gain_db(&mut self, id: TrackId, gain_db: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.gain_db = gain_db;
        self.mixer
            .runtime
            .sync_track_strip(self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_track_pan(&mut self, id: TrackId, pan: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.pan = pan.clamp(-1.0, 1.0);
        self.mixer
            .runtime
            .sync_track_strip(self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_track_muted(&mut self, id: TrackId, muted: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.muted = muted;
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
        Ok(())
    }

    pub fn set_track_soloed(&mut self, id: TrackId, soloed: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.soloed = soloed;
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
        Ok(())
    }

    pub fn set_track_route(
        &mut self,
        id: TrackId,
        route: TrackRoute,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_track_route(id, route)?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(())
    }

    pub fn set_track_routing(
        &mut self,
        id: TrackId,
        routing: TrackRouting,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_track_routing(id, routing)?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(())
    }

    pub fn add_track_send(&mut self, id: TrackId, send: BusSend) -> Result<(), AudioEngineError> {
        self.mixer.state.add_track_bus_send(id, send)?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(())
    }

    pub fn set_track_send(
        &mut self,
        id: TrackId,
        index: usize,
        send: BusSend,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_track_bus_send(id, index, send)?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(())
    }

    pub fn remove_track_send(
        &mut self,
        id: TrackId,
        index: usize,
    ) -> Result<BusSend, AudioEngineError> {
        let removed = self.mixer.state.remove_track_bus_send(id, index)?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        Ok(removed)
    }

    pub fn set_bus_gain_db(&mut self, id: BusId, gain_db: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.gain_db = gain_db;
        self.mixer
            .runtime
            .sync_bus_strip(self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_bus_pan(&mut self, id: BusId, pan: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.pan = pan.clamp(-1.0, 1.0);
        self.mixer
            .runtime
            .sync_bus_strip(self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_bus_muted(&mut self, id: BusId, muted: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.muted = muted;
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
        Ok(())
    }

    pub fn set_bus_soloed(&mut self, id: BusId, soloed: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.soloed = soloed;
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
        Ok(())
    }

    pub fn set_bus_route(&mut self, id: BusId, route: TrackRoute) -> Result<(), AudioEngineError> {
        self.mixer.state.set_bus_route(id, route)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_bus_routing(
        &mut self,
        id: BusId,
        routing: TrackRouting,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_bus_routing(id, routing)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn add_bus_send(&mut self, id: BusId, send: BusSend) -> Result<(), AudioEngineError> {
        self.mixer.state.add_bus_send(id, send)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_bus_send(
        &mut self,
        id: BusId,
        index: usize,
        send: BusSend,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_bus_send(id, index, send)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn remove_bus_send(
        &mut self,
        id: BusId,
        index: usize,
    ) -> Result<BusSend, AudioEngineError> {
        let removed = self.mixer.state.remove_bus_send(id, index)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        Ok(removed)
    }

    pub fn set_master_gain_db(&mut self, gain_db: f32) {
        self.mixer.state.master.state.gain_db = gain_db;
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
    }

    pub fn set_master_pan(&mut self, pan: f32) {
        self.mixer.state.master.state.pan = pan.clamp(-1.0, 1.0);
        self.mixer
            .runtime
            .sync_all_levels(self.commands, &self.mixer.state);
    }
}

#[cfg(test)]
mod tests {
    use super::{MixerError, MixerState};
    use crate::mixer::{BusId, BusSend, INSTRUMENT_TRACK_COUNT, TrackId, TrackRoute};

    #[test]
    fn mixer_preallocates_instrument_tracks_and_master() {
        let mixer = MixerState::new();
        assert_eq!(mixer.track_count(), INSTRUMENT_TRACK_COUNT);
        assert_eq!(mixer.bus_count(), 0);
        assert_eq!(mixer.master().name, "Master");
        assert!(
            mixer
                .track(TrackId(0))
                .expect("track should exist")
                .id
                .is_instrument()
        );
        assert!(
            mixer
                .track(TrackId((INSTRUMENT_TRACK_COUNT - 1) as u16))
                .expect("track should exist")
                .id
                .is_instrument()
        );
    }

    #[test]
    fn track_routing_rejects_missing_bus_targets() {
        let mut mixer = MixerState::new();
        let error = mixer
            .set_track_route(TrackId(0), TrackRoute::Bus(BusId(1)))
            .expect_err("missing bus should be rejected");
        assert_eq!(error, MixerError::InvalidBusId(BusId(1)));
    }

    #[test]
    fn dynamic_buses_accept_routes_and_sends() {
        let mut mixer = MixerState::new();
        let bus_id = mixer.add_bus("Verb");
        mixer
            .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
            .expect("bus route should succeed");
        mixer
            .add_track_bus_send(TrackId(0), BusSend::new(bus_id, -6.0, false))
            .expect("bus send should succeed");
        assert_eq!(mixer.bus_count(), 1);
        assert_eq!(
            mixer
                .track(TrackId(0))
                .expect("track should exist")
                .routing
                .sends
                .len(),
            1
        );
    }

    #[test]
    fn removing_bus_reroutes_tracks_and_clears_sends() {
        let mut mixer = MixerState::new();
        let bus_id = mixer.add_bus("Verb");
        mixer
            .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
            .expect("bus route should succeed");
        mixer
            .add_track_bus_send(TrackId(0), BusSend::new(bus_id, -6.0, false))
            .expect("bus send should succeed");

        mixer
            .remove_bus(bus_id)
            .expect("bus removal should succeed");

        let track = mixer.track(TrackId(0)).expect("track should exist");
        assert_eq!(track.routing.main, TrackRoute::Master);
        assert!(track.routing.sends.is_empty());
    }

    #[test]
    fn mixer_roundtrips_through_ron() {
        let mut mixer = MixerState::new();
        let bus_id = mixer.add_bus("Verb");
        mixer
            .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
            .expect("bus route should succeed");
        mixer
            .add_track_bus_send(TrackId(1), BusSend::new(bus_id, -3.0, true))
            .expect("bus send should succeed");

        let ron = ron::to_string(&mixer).expect("mixer should serialize");
        let restored: MixerState = ron::from_str(&ron).expect("mixer should deserialize");

        assert_eq!(restored, mixer);
    }
}
