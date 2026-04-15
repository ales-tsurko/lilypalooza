//! Fixed instrument tracks plus dynamic buses.

use crate::track::{
    BusId, BusSend, BusTrack, INSTRUMENT_TRACK_COUNT, MasterTrack, MixerTrack, TrackId, TrackRoute,
    TrackRouting,
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

/// Mixer model with fixed instrument tracks, dynamic buses, and a dedicated master.
#[derive(Debug, Clone, PartialEq)]
pub struct Mixer {
    /// Fixed instrument tracks.
    pub tracks: Vec<MixerTrack>,
    /// Dynamic bus tracks.
    pub buses: Vec<BusTrack>,
    /// Dedicated master track.
    pub master: MasterTrack,
    next_bus_id: u16,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
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

    /// Returns mutable access to one instrument track.
    pub fn track_mut(&mut self, id: TrackId) -> Result<&mut MixerTrack, MixerError> {
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

    /// Returns mutable access to one bus.
    pub fn bus_mut(&mut self, id: BusId) -> Result<&mut BusTrack, MixerError> {
        self.buses
            .iter_mut()
            .find(|bus| bus.id == id)
            .ok_or(MixerError::InvalidBusId(id))
    }

    /// Adds one dynamic bus.
    pub fn add_bus(&mut self, name: impl Into<String>) -> BusId {
        let id = BusId(self.next_bus_id);
        self.next_bus_id = self.next_bus_id.saturating_add(1);
        self.buses.push(BusTrack::new(id, name));
        id
    }

    /// Removes one dynamic bus and reroutes everything targeting it back to master.
    pub fn remove_bus(&mut self, id: BusId) -> Result<BusTrack, MixerError> {
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
    pub fn set_track_route(&mut self, id: TrackId, route: TrackRoute) -> Result<(), MixerError> {
        self.validate_track_route(route)?;
        self.track_mut(id)?.routing.main = route;
        Ok(())
    }

    /// Sets the main route for one bus track.
    pub fn set_bus_route(&mut self, id: BusId, route: TrackRoute) -> Result<(), MixerError> {
        self.validate_bus_route(id, route)?;
        self.bus_mut(id)?.routing.main = route;
        Ok(())
    }

    /// Replaces one instrument-track routing state.
    pub fn set_track_routing(
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
    pub fn set_bus_routing(&mut self, id: BusId, routing: TrackRouting) -> Result<(), MixerError> {
        self.validate_bus_route(id, routing.main)?;
        self.validate_bus_sends(id, routing.sends.iter().copied())?;
        self.bus_mut(id)?.routing = routing;
        Ok(())
    }

    /// Adds one parallel bus send to an instrument track.
    pub fn add_track_bus_send(&mut self, id: TrackId, send: BusSend) -> Result<(), MixerError> {
        self.validate_send(send)?;
        self.track_mut(id)?.routing.sends.push(send);
        Ok(())
    }

    /// Adds one parallel bus send to a bus.
    pub fn add_bus_send(&mut self, id: BusId, send: BusSend) -> Result<(), MixerError> {
        self.validate_bus_send(id, send)?;
        self.bus_mut(id)?.routing.sends.push(send);
        Ok(())
    }

    /// Replaces one parallel bus send on an instrument track.
    pub fn set_track_bus_send(
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
    pub fn set_bus_send(
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
    pub fn remove_track_bus_send(
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
    pub fn remove_bus_send(&mut self, id: BusId, index: usize) -> Result<BusSend, MixerError> {
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

#[cfg(test)]
mod tests {
    use super::{Mixer, MixerError};
    use crate::track::{BusId, BusSend, INSTRUMENT_TRACK_COUNT, TrackId, TrackRoute};

    #[test]
    fn mixer_preallocates_instrument_tracks_and_master() {
        let mixer = Mixer::new();
        assert_eq!(mixer.track_count(), INSTRUMENT_TRACK_COUNT);
        assert_eq!(mixer.bus_count(), 0);
        assert_eq!(mixer.master.name, "Master");
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
        let mut mixer = Mixer::new();
        let error = mixer
            .set_track_route(TrackId(0), TrackRoute::Bus(BusId(1)))
            .expect_err("missing bus should be rejected");
        assert_eq!(error, MixerError::InvalidBusId(BusId(1)));
    }

    #[test]
    fn dynamic_buses_accept_routes_and_sends() {
        let mut mixer = Mixer::new();
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
        let mut mixer = Mixer::new();
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
}
