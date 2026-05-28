use std::ops::{Deref, Range};

use knyst::{
    modal_interface::KnystContext,
    prelude::{Beats, MultiThreadedKnystCommands},
};

use super::*;
use crate::{
    engine::{AudioEngineError, AudioEngineSettings},
    instrument::{Controller, InstrumentRuntimeHandle, SlotState},
    mixer::runtime::{MixerRuntime, MixerRuntimeError, TrackInstrumentSync},
    sequencer::Sequencer,
    soundfont::SoundfontResource,
};

#[derive(Debug, Clone, Copy)]
enum SendOwner {
    Track(TrackId),
    Bus(BusId),
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

    pub(crate) fn controller(
        &self,
        address: SlotAddress,
    ) -> Result<Option<Box<dyn Controller>>, AudioEngineError> {
        self.runtime
            .controller(&self.state, address)
            .map_err(Into::into)
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

impl std::fmt::Debug for MixerHandle<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MixerHandle")
            .field("state", &self.mixer.state)
            .finish_non_exhaustive()
    }
}

impl MixerHandle<'_> {
    /// Replaces the whole mixer state and rebuilds the attached audio graph.
    pub fn replace_state(&mut self, state: MixerState) -> Result<(), AudioEngineError> {
        let settings = self.mixer.runtime.meter_settings();
        let new_runtime = MixerRuntime::attach(self.context, self.commands, &settings, &state)?;
        let old_runtime = std::mem::replace(&mut self.mixer.runtime, new_runtime);
        self.mixer.state = state;
        old_runtime.free();
        settle_graph_mutation(self.commands);
        for (track_id, _) in self.mixer.state.tracks_with_ids() {
            self.sequencer.sync_track_handle(
                self.commands,
                track_id,
                self.mixer.instrument_handle(track_id),
            );
        }
        self.sequencer
            .sync_metronome_handle(self.commands, Some(self.mixer.metronome_handle()));
        Ok(())
    }

    /// Adds or replaces one loaded SoundFont resource.
    pub fn set_soundfont(&mut self, resource: SoundfontResource) -> Result<(), AudioEngineError> {
        self.mixer.state.set_soundfont(resource);
        self.mixer.runtime.sync_soundfonts(&self.mixer.state)?;
        self.mixer.runtime.sync_tracks_after_soundfonts_changed(
            self.context,
            self.commands,
            &self.mixer.state,
        )?;
        settle_graph_mutation(self.commands);
        for (track_id, _) in self.mixer.state.tracks_with_ids() {
            self.sequencer.sync_track_handle(
                self.commands,
                track_id,
                self.mixer.instrument_handle(track_id),
            );
        }
        Ok(())
    }

    /// Removes a loaded SoundFont resource.
    pub fn remove_soundfont(&mut self, id: &str) -> Result<SoundfontResource, AudioEngineError> {
        let removed = self.mixer.state.remove_soundfont(id)?;
        self.mixer.runtime.sync_soundfonts(&self.mixer.state)?;
        self.mixer.runtime.sync_tracks_after_soundfonts_changed(
            self.context,
            self.commands,
            &self.mixer.state,
        )?;
        settle_graph_mutation(self.commands);
        for (track_id, _) in self.mixer.state.tracks_with_ids() {
            self.sequencer.sync_track_handle(
                self.commands,
                track_id,
                self.mixer.instrument_handle(track_id),
            );
        }
        Ok(removed)
    }

    /// Adds a bus strip and attaches its runtime nodes.
    pub fn add_bus(&mut self, name: impl Into<String>) -> Result<BusId, AudioEngineError> {
        let bus_id = self.mixer.state.add_bus(name);
        self.mixer
            .runtime
            .add_bus(self.context, self.commands, &self.mixer.state, bus_id)?;
        settle_graph_mutation(self.commands);
        Ok(bus_id)
    }

    /// Removes a bus strip and detaches its runtime nodes.
    pub fn remove_bus(&mut self, id: BusId) -> Result<Track, AudioEngineError> {
        let removed = self.mixer.state.remove_bus(id)?;
        self.mixer
            .runtime
            .remove_bus(self.commands, &self.mixer.state, id)?;
        settle_graph_mutation(self.commands);
        Ok(removed)
    }

    /// Recomputes routing after processor latency changes.
    pub fn sync_processor_latencies(&mut self) -> Result<(), AudioEngineError> {
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        Ok(())
    }

    /// Replaces the instrument slot on a track.
    pub fn set_track_instrument(
        &mut self,
        id: TrackId,
        instrument: SlotState,
    ) -> Result<(), AudioEngineError> {
        self.mixer
            .state
            .track_mut(id)?
            .set_instrument_slot(instrument);
        let sync = self.mixer.runtime.sync_track_instrument(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        let graph_changed = matches!(sync, TrackInstrumentSync::GraphChanged);
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        if graph_changed {
            self.sequencer
                .sync_track_handle(self.commands, id, self.mixer.instrument_handle(id));
            if self.sequencer.is_playing() {
                let current_beat = current_playing_beat(self.commands).unwrap_or(Beats::ZERO);
                self.sequencer.mark_dirty_for_seek(current_beat, true);
            }
        }
        Ok(())
    }

    /// Replaces the effect slots on a track.
    pub fn set_track_effects(
        &mut self,
        id: TrackId,
        effects: Vec<SlotState>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.set_effects(effects);
        let graph_changed = self.mixer.runtime.sync_track_effects(
            self.context,
            self.commands,
            &self.mixer.state,
            id,
        )?;
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        if graph_changed {
            self.sequencer
                .sync_track_handle(self.commands, id, self.mixer.instrument_handle(id));
            if self.sequencer.is_playing() {
                let current_beat = current_playing_beat(self.commands).unwrap_or(Beats::ZERO);
                self.sequencer.mark_dirty_for_seek(current_beat, true);
            }
        }
        Ok(())
    }

    /// Renames a track strip.
    pub fn set_track_name(
        &mut self,
        id: TrackId,
        name: impl Into<String>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.name = name.into();
        Ok(())
    }

    /// Renames a bus strip.
    pub fn set_bus_name(
        &mut self,
        id: BusId,
        name: impl Into<String>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.name = name.into();
        Ok(())
    }

    /// Sets a track gain in decibels.
    pub fn set_track_gain_db(&mut self, id: TrackId, gain_db: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.gain_db = gain_db;
        self.mixer.runtime.sync_track_strip(&self.mixer.state, id)?;
        Ok(())
    }

    /// Sets a track pan value in the `[-1.0, 1.0]` range.
    pub fn set_track_pan(&mut self, id: TrackId, pan: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.pan = pan.clamp(-1.0, 1.0);
        self.mixer.runtime.sync_track_strip(&self.mixer.state, id)?;
        Ok(())
    }

    /// Mutes or unmutes a track.
    pub fn set_track_muted(&mut self, id: TrackId, muted: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.muted = muted;
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
        Ok(())
    }

    /// Solos or unsolos a track.
    pub fn set_track_soloed(&mut self, id: TrackId, soloed: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.track_mut(id)?.state.soloed = soloed;
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
        Ok(())
    }

    /// Sets a track output route.
    pub fn set_track_route(
        &mut self,
        id: TrackId,
        route: TrackRoute,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_track_route(id, route)?;
        self.sync_routing_after_mutation()
    }

    /// Sets full track routing state.
    pub fn set_track_routing(
        &mut self,
        id: TrackId,
        routing: TrackRouting,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_track_routing(id, routing)?;
        self.sync_routing_after_mutation()
    }

    /// Adds a bus send to a track.
    pub fn add_track_send(&mut self, id: TrackId, send: BusSend) -> Result<(), AudioEngineError> {
        self.mixer.state.add_track_bus_send(id, send)?;
        self.sync_routing_after_mutation()
    }

    /// Replaces one bus send on a track.
    pub fn set_track_send(
        &mut self,
        id: TrackId,
        index: usize,
        send: BusSend,
    ) -> Result<(), AudioEngineError> {
        self.set_send(SendOwner::Track(id), index, send)
    }

    /// Removes one bus send from a track.
    pub fn remove_track_send(
        &mut self,
        id: TrackId,
        index: usize,
    ) -> Result<BusSend, AudioEngineError> {
        let removed = self.mixer.state.remove_track_bus_send(id, index)?;
        self.sync_routing_after_mutation()?;
        Ok(removed)
    }

    /// Sets a bus gain in decibels.
    pub fn set_bus_gain_db(&mut self, id: BusId, gain_db: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.gain_db = gain_db;
        self.mixer.runtime.sync_bus_strip(&self.mixer.state, id)?;
        Ok(())
    }

    /// Replaces the effect slots on a bus.
    pub fn set_bus_effects(
        &mut self,
        id: BusId,
        effects: Vec<SlotState>,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.set_effects(effects);
        self.mixer
            .runtime
            .sync_bus_effects(self.context, self.commands, &self.mixer.state, id)?;
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        Ok(())
    }

    /// Sets a bus pan value in the `[-1.0, 1.0]` range.
    pub fn set_bus_pan(&mut self, id: BusId, pan: f32) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.pan = pan.clamp(-1.0, 1.0);
        self.mixer.runtime.sync_bus_strip(&self.mixer.state, id)?;
        Ok(())
    }

    /// Mutes or unmutes a bus.
    pub fn set_bus_muted(&mut self, id: BusId, muted: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.muted = muted;
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
        Ok(())
    }

    /// Solos or unsolos a bus.
    pub fn set_bus_soloed(&mut self, id: BusId, soloed: bool) -> Result<(), AudioEngineError> {
        self.mixer.state.bus_mut(id)?.state.soloed = soloed;
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
        Ok(())
    }

    /// Sets a bus output route.
    pub fn set_bus_route(&mut self, id: BusId, route: TrackRoute) -> Result<(), AudioEngineError> {
        self.mixer.state.set_bus_route(id, route)?;
        self.sync_routing_after_mutation()
    }

    /// Sets full bus routing state.
    pub fn set_bus_routing(
        &mut self,
        id: BusId,
        routing: TrackRouting,
    ) -> Result<(), AudioEngineError> {
        self.mixer.state.set_bus_routing(id, routing)?;
        self.sync_routing_after_mutation()
    }

    /// Adds a bus send to a bus.
    pub fn add_bus_send(&mut self, id: BusId, send: BusSend) -> Result<(), AudioEngineError> {
        self.mixer.state.add_bus_send(id, send)?;
        self.sync_routing_after_mutation()
    }

    /// Replaces one bus send on a bus.
    pub fn set_bus_send(
        &mut self,
        id: BusId,
        index: usize,
        send: BusSend,
    ) -> Result<(), AudioEngineError> {
        self.set_send(SendOwner::Bus(id), index, send)
    }

    fn set_send(
        &mut self,
        owner: SendOwner,
        index: usize,
        send: BusSend,
    ) -> Result<(), AudioEngineError> {
        let old_send = self.current_send(owner, index)?;
        match owner {
            SendOwner::Track(id) => self.mixer.state.set_track_bus_send(id, index, send)?,
            SendOwner::Bus(id) => self.mixer.state.set_bus_send(id, index, send)?,
        }
        if send_level_only_changed(old_send, send) {
            self.mixer.runtime.sync_all_send_levels(&self.mixer.state);
            return Ok(());
        }
        self.sync_routing_after_mutation()
    }

    fn current_send(&self, owner: SendOwner, index: usize) -> Result<BusSend, AudioEngineError> {
        match owner {
            SendOwner::Track(id) => {
                let track = self.mixer.state.track(id)?;
                track.routing.sends.get(index).copied().ok_or_else(|| {
                    MixerError::BusSendIndexOutOfBounds {
                        index,
                        len: track.routing.sends.len(),
                    }
                    .into()
                })
            }
            SendOwner::Bus(id) => Ok(current_bus_send(&self.mixer.state, id, index)?),
        }
    }

    /// Removes one bus send from a bus.
    pub fn remove_bus_send(
        &mut self,
        id: BusId,
        index: usize,
    ) -> Result<BusSend, AudioEngineError> {
        let removed = self.mixer.state.remove_bus_send(id, index)?;
        self.sync_routing_after_mutation()?;
        Ok(removed)
    }

    fn sync_routing_after_mutation(&mut self) -> Result<(), AudioEngineError> {
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        Ok(())
    }

    /// Sets the master gain in decibels.
    pub fn set_master_gain_db(&mut self, gain_db: f32) {
        self.mixer.state.master_mut().state.gain_db = gain_db;
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
    }

    /// Sets the master pan value in the `[-1.0, 1.0]` range.
    pub fn set_master_pan(&mut self, pan: f32) {
        self.mixer.state.master_mut().state.pan = pan.clamp(-1.0, 1.0);
        self.mixer.runtime.sync_all_levels(&self.mixer.state);
    }

    /// Resets the master meter hold state.
    pub fn reset_master_meter(&mut self) {
        self.mixer.reset_master_meter();
    }

    /// Resets one track meter hold state.
    pub fn reset_track_meter(&mut self, id: TrackId) -> Result<(), AudioEngineError> {
        self.mixer.reset_track_meter(id)
    }

    /// Resets one bus meter hold state.
    pub fn reset_bus_meter(&mut self, id: BusId) -> Result<(), AudioEngineError> {
        self.mixer.reset_bus_meter(id)
    }

    /// Replaces the master effect slots.
    pub fn set_master_effects(&mut self, effects: Vec<SlotState>) -> Result<(), AudioEngineError> {
        self.mixer.state.master_mut().set_effects(effects);
        self.mixer
            .runtime
            .sync_master_effects(self.context, self.commands, &self.mixer.state)?;
        self.mixer
            .runtime
            .sync_all_routing(self.context, self.commands, &self.mixer.state)?;
        settle_graph_mutation(self.commands);
        Ok(())
    }

    /// Bypasses or enables a processor slot.
    pub fn set_slot_bypassed(
        &mut self,
        address: SlotAddress,
        bypassed: bool,
    ) -> Result<(), AudioEngineError> {
        if address.slot_index == 0 {
            return Err(MixerError::InvalidSlotAddress {
                strip_index: address.strip_index,
                slot_index: address.slot_index,
            }
            .into());
        }
        self.mixer.state.slot_mut(address)?.bypassed = bypassed;
        self.mixer
            .runtime
            .sync_slot_bypass(&self.mixer.state, address)?;
        Ok(())
    }
}
