//! Fixed instrument tracks plus dynamic buses.

pub(crate) mod runtime;
mod track;

use std::ops::Deref;
use std::ops::Range;
use std::time::Duration;

use knyst::controller::KnystCommands;
use knyst::modal_interface::KnystContext;
use knyst::prelude::{Beats, MultiThreadedKnystCommands, TransportState};
use serde::{Deserialize, Serialize};

use crate::engine::{AudioEngineError, AudioEngineSettings};
use crate::instrument::{
    EffectSlotState, InstrumentRuntimeHandle, InstrumentSlotState, SoundfontResource,
};
use crate::sequencer::Sequencer;
use runtime::{MixerRuntime, MixerRuntimeError, TrackInstrumentSync};
pub use track::{
    BusId, BusSend, BusTrack, INSTRUMENT_TRACK_COUNT, MasterTrack, MixerTrack, TrackId, TrackRoute,
    TrackRouting, TrackState,
};

const GRAPH_SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
const PLAYHEAD_SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(2);

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
}

/// One channel meter snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChannelMeterSnapshot {
    /// Normalized level in `0..=1`.
    pub level: f32,
    /// Normalized hold marker in `0..=1`.
    pub hold: f32,
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

/// Serializable mixer state with fixed instrument tracks, dynamic buses, and a dedicated master.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerState {
    tracks: Vec<MixerTrack>,
    buses: Vec<BusTrack>,
    master: MasterTrack,
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
        let tracks = (0..INSTRUMENT_TRACK_COUNT)
            .map(|index| MixerTrack::new(TrackId(index as u16)))
            .collect();
        Self {
            tracks,
            buses: Vec::new(),
            master: MasterTrack::default(),
            soundfonts: Vec::new(),
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

    /// Returns all configured shared SoundFonts.
    #[must_use]
    pub fn soundfonts(&self) -> &[SoundfontResource] {
        &self.soundfonts
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

pub(crate) struct Mixer {
    pub(crate) state: MixerState,
    pub(crate) runtime: MixerRuntime,
}

impl Mixer {
    pub(crate) fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        state: MixerState,
    ) -> Result<Self, MixerRuntimeError> {
        let runtime = MixerRuntime::attach(context, commands, settings, &state)?;
        Ok(Self { state, runtime })
    }

    pub(crate) fn instrument_handle(&self, track_id: TrackId) -> Option<InstrumentRuntimeHandle> {
        self.runtime.instrument_handle(track_id)
    }

    pub(crate) fn metronome_handle(&self) -> InstrumentRuntimeHandle {
        self.runtime.metronome_handle()
    }

    pub(crate) fn meter_snapshot(&self) -> MixerMeterSnapshot {
        self.runtime.meter_snapshot(&self.state)
    }

    pub(crate) fn meter_snapshot_window(
        &self,
        track_range: Range<usize>,
        bus_range: Range<usize>,
    ) -> MixerMeterSnapshotWindow {
        self.runtime
            .meter_snapshot_window(&self.state, track_range, bus_range)
    }

    pub(crate) fn reset_meters(&self) {
        self.runtime.reset_meters();
    }

    pub(crate) fn reset_master_meter(&self) {
        self.runtime.reset_master_meter();
    }

    pub(crate) fn reset_track_meter(&self, id: TrackId) -> Result<(), AudioEngineError> {
        self.runtime.reset_track_meter(id)?;
        Ok(())
    }

    pub(crate) fn reset_bus_meter(&self, id: BusId) -> Result<(), AudioEngineError> {
        self.runtime.reset_bus_meter(id)?;
        Ok(())
    }
}

/// Mutable mixer control handle.
#[allow(missing_docs)]
pub struct MixerHandle<'a> {
    mixer: &'a mut Mixer,
    sequencer: &'a Sequencer,
    context: &'a KnystContext,
    commands: &'a mut MultiThreadedKnystCommands,
}

impl<'a> MixerHandle<'a> {
    pub(crate) fn new(
        mixer: &'a mut Mixer,
        sequencer: &'a Sequencer,
        context: &'a KnystContext,
        commands: &'a mut MultiThreadedKnystCommands,
    ) -> MixerHandle<'a> {
        Self {
            mixer,
            sequencer,
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
    pub fn replace_state(&mut self, state: MixerState) -> Result<(), AudioEngineError> {
        let settings = self.mixer.runtime.meter_settings();
        let new_runtime = MixerRuntime::attach(self.context, self.commands, &settings, &state)?;
        let old_runtime = std::mem::replace(&mut self.mixer.runtime, new_runtime);
        self.mixer.state = state;
        old_runtime.free();
        settle_graph_mutation(self.commands);
        for track in self.mixer.state.tracks() {
            self.sequencer.sync_track_handle(
                self.commands,
                track.id,
                self.mixer.instrument_handle(track.id),
            );
        }
        self.sequencer
            .sync_metronome_handle(self.commands, Some(self.mixer.metronome_handle()));
        Ok(())
    }

    pub fn set_soundfont(&mut self, resource: SoundfontResource) -> Result<(), AudioEngineError> {
        let soundfont_id = resource.id.clone();
        self.mixer.state.set_soundfont(resource);
        self.mixer.runtime.sync_soundfonts(&self.mixer.state)?;
        self.mixer.runtime.sync_tracks_for_soundfont(
            self.context,
            self.commands,
            &self.mixer.state,
            &soundfont_id,
        )?;
        settle_graph_mutation(self.commands);
        for track in self.mixer.state.tracks() {
            self.sequencer.sync_track_handle(
                self.commands,
                track.id,
                self.mixer.instrument_handle(track.id),
            );
        }
        Ok(())
    }

    pub fn remove_soundfont(&mut self, id: &str) -> Result<SoundfontResource, AudioEngineError> {
        let removed = self.mixer.state.remove_soundfont(id)?;
        self.mixer.runtime.sync_soundfonts(&self.mixer.state)?;
        self.mixer.runtime.sync_tracks_for_soundfont(
            self.context,
            self.commands,
            &self.mixer.state,
            &removed.id,
        )?;
        settle_graph_mutation(self.commands);
        for track in self.mixer.state.tracks() {
            self.sequencer.sync_track_handle(
                self.commands,
                track.id,
                self.mixer.instrument_handle(track.id),
            );
        }
        Ok(removed)
    }

    pub fn add_bus(&mut self, name: impl Into<String>) -> Result<BusId, AudioEngineError> {
        let bus_id = self.mixer.state.add_bus(name);
        self.mixer
            .runtime
            .add_bus(self.context, self.commands, &self.mixer.state, bus_id)?;
        settle_graph_mutation(self.commands);
        Ok(bus_id)
    }

    pub fn remove_bus(&mut self, id: BusId) -> Result<BusTrack, AudioEngineError> {
        let removed = self.mixer.state.remove_bus(id)?;
        self.mixer
            .runtime
            .remove_bus(self.commands, &self.mixer.state, id)?;
        settle_graph_mutation(self.commands);
        Ok(removed)
    }

    pub fn set_track_instrument(
        &mut self,
        id: TrackId,
        instrument: InstrumentSlotState,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.instrument = instrument;
        let sync = self.mixer.runtime.sync_track_instrument(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        if matches!(sync, TrackInstrumentSync::GraphChanged) {
            self.mixer.runtime.sync_track_routing(
                self.context,
                self.commands,
                &self.mixer.state,
                id,
            )?;
            settle_graph_mutation(self.commands);
            self.sequencer
                .sync_track_handle(self.commands, id, self.mixer.instrument_handle(id));
            if self.sequencer.is_playing() {
                let current_beat = current_playing_beat(self.commands).unwrap_or(Beats::ZERO);
                self.sequencer.mark_dirty_for_seek(current_beat, true);
            }
        }
        Ok(())
    }

    pub fn set_track_effects(
        &mut self,
        id: TrackId,
        effects: Vec<EffectSlotState>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.effects = effects;
        self.mixer.runtime.sync_track_effects(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        self.mixer.runtime.sync_track_routing(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        settle_graph_mutation(self.commands);
        Ok(())
    }

    pub fn set_track_name(
        &mut self,
        id: TrackId,
        name: impl Into<String>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.name = name.into();
        Ok(())
    }

    pub fn set_bus_name(
        &mut self,
        id: BusId,
        name: impl Into<String>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.name = name.into();
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
        Ok(removed)
    }

    pub fn set_bus_gain_db(&mut self, id: BusId, gain_db: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.gain_db = gain_db;
        self.mixer
            .runtime
            .sync_bus_strip(self.commands, &self.mixer.state, id)?;
        Ok(())
    }

    pub fn set_bus_effects(
        &mut self,
        id: BusId,
        effects: Vec<EffectSlotState>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.effects = effects;
        self.mixer
            .runtime
            .sync_bus_effects(self.context, self.commands, &self.mixer.state, id)?;
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
        Ok(())
    }

    pub fn add_bus_send(&mut self, id: BusId, send: BusSend) -> Result<(), AudioEngineError> {
        self.mixer.state.add_bus_send(id, send)?;
        self.mixer
            .runtime
            .sync_bus_routing(self.context, self.commands, &self.mixer.state, id)?;
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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
        settle_graph_mutation(self.commands);
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

    pub fn reset_master_meter(&mut self) {
        self.mixer.reset_master_meter();
    }

    pub fn reset_track_meter(&mut self, id: TrackId) -> Result<(), AudioEngineError> {
        self.mixer.reset_track_meter(id)
    }

    pub fn reset_bus_meter(&mut self, id: BusId) -> Result<(), AudioEngineError> {
        self.mixer.reset_bus_meter(id)
    }

    pub fn set_master_effects(
        &mut self,
        effects: Vec<EffectSlotState>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.master.effects = effects;
        self.mixer
            .runtime
            .sync_master_effects(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        Ok(())
    }
}

fn settle_graph_mutation(commands: &mut MultiThreadedKnystCommands) {
    let Ok(receiver) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        commands.request_graph_settled()
    })) else {
        return;
    };
    let _ = receiver.recv_timeout(GRAPH_SETTLE_TIMEOUT);
}

fn current_playing_beat(commands: &mut MultiThreadedKnystCommands) -> Option<Beats> {
    let start = std::time::Instant::now();
    while start.elapsed() < GRAPH_SETTLE_TIMEOUT {
        if let Some(snapshot) = commands.current_transport_snapshot()
            && snapshot.state == TransportState::Playing
        {
            return snapshot.beats;
        }
        std::thread::sleep(PLAYHEAD_SNAPSHOT_POLL_INTERVAL);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{MixerError, MixerState};
    use crate::instrument::{EffectKind, EffectSlotState, ProcessorState, SoundfontResource};
    use crate::mixer::{BusId, BusSend, INSTRUMENT_TRACK_COUNT, TrackId, TrackRoute};
    use std::path::PathBuf;

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
        mixer.set_soundfont(SoundfontResource {
            id: "fluid".to_string(),
            name: "FluidR3".to_string(),
            path: PathBuf::from("/tmp/FluidR3.sf2"),
        });
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

    #[test]
    fn replacing_soundfont_keeps_one_entry_per_id() {
        let mut mixer = MixerState::new();
        mixer.set_soundfont(SoundfontResource {
            id: "fluid".to_string(),
            name: "FluidR3".to_string(),
            path: PathBuf::from("/tmp/FluidR3.sf2"),
        });
        mixer.set_soundfont(SoundfontResource {
            id: "fluid".to_string(),
            name: "GeneralUser".to_string(),
            path: PathBuf::from("/tmp/GeneralUser.sf2"),
        });

        assert_eq!(mixer.soundfonts().len(), 1);
        assert_eq!(mixer.soundfonts()[0].name, "GeneralUser");
    }

    #[test]
    fn mixer_roundtrips_effect_slots() {
        let mut mixer = MixerState::new();
        mixer
            .track_mut(TrackId(0))
            .expect("track should exist")
            .effects
            .push(EffectSlotState {
                kind: EffectKind::BuiltIn {
                    effect_id: "gain".to_string(),
                },
                state: ProcessorState::default(),
            });

        let ron = ron::to_string(&mixer).expect("mixer should serialize");
        let restored: MixerState = ron::from_str(&ron).expect("mixer should deserialize");
        assert_eq!(restored, mixer);
    }
}
