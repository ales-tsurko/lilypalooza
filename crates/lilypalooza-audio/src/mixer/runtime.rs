use std::collections::HashMap;

use knyst::modal_interface::{KnystContext, knyst_commands};
use knyst::prelude::{
    BlockSize, Connection, GenState, GenericHandle, Handle, HandleData, KnystCommands,
    MultiThreadedKnystCommands, NodeId, Sample, bus, graph_output, handle, impl_gen,
};

use crate::engine::AudioEngineSettings;
use crate::instrument::soundfont_synth::{
    LoadedSoundfont, SoundfontProcessor, SoundfontSynthError, SoundfontSynthSettings,
};
use crate::instrument::{
    EffectProcessorNode, EffectRuntimeHandle, InstrumentKind, InstrumentProcessorNode,
    InstrumentRuntimeHandle, ProcessorStateError, create_effect_processor,
};
use crate::mixer::{
    BusId, BusSend, BusTrack, MixerError, MixerState, MixerTrack, TrackId, TrackRoute,
};

#[derive(thiserror::Error, Debug)]
pub(crate) enum MixerRuntimeError {
    #[error(transparent)]
    Mixer(#[from] MixerError),
    #[error(transparent)]
    Soundfont(#[from] SoundfontSynthError),
    #[error(transparent)]
    ProcessorState(#[from] ProcessorStateError),
}

pub(crate) struct MixerRuntime {
    master: MasterRuntime,
    tracks: Vec<TrackRuntime>,
    buses: HashMap<BusId, BusRuntime>,
    soundfonts: HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
}

impl MixerRuntime {
    pub(crate) fn instrument_handle(&self, track_id: TrackId) -> Option<InstrumentRuntimeHandle> {
        Some(
            self.tracks
                .get(track_id.index())?
                .instrument
                .as_ref()?
                .handle,
        )
    }

    pub(crate) fn attach(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        mixer: &MixerState,
    ) -> Result<Self, MixerRuntimeError> {
        context.with_activation(|| {
            let master = MasterRuntime::new(context, commands, mixer);
            let soundfont_settings =
                SoundfontSynthSettings::new(settings.sample_rate as i32, settings.block_size);

            let mut buses = HashMap::with_capacity(mixer.buses.len());
            for bus_track in &mixer.buses {
                buses.insert(
                    bus_track.id,
                    BusRuntime::new(context, commands, bus_track, mixer),
                );
            }

            let mut runtime = Self {
                master,
                tracks: Vec::with_capacity(mixer.tracks.len()),
                buses,
                soundfonts: HashMap::new(),
                soundfont_settings,
            };
            runtime.sync_soundfonts(mixer)?;

            let mut tracks = Vec::with_capacity(mixer.tracks.len());
            for track in &mixer.tracks {
                tracks.push(TrackRuntime::new(
                    context,
                    commands,
                    track,
                    mixer,
                    &runtime.soundfonts,
                    runtime.soundfont_settings,
                )?);
            }
            runtime.tracks = tracks;
            runtime.sync_all_routing(context, commands, mixer)?;
            runtime.sync_all_levels(commands, mixer);
            Ok(runtime)
        })
    }

    pub(crate) fn sync_soundfonts(&mut self, mixer: &MixerState) -> Result<(), MixerRuntimeError> {
        let resources: HashMap<_, _> = mixer
            .soundfonts()
            .iter()
            .map(|resource| (resource.id.clone(), resource))
            .collect();

        self.soundfonts.retain(|id, _| resources.contains_key(id));

        for resource in mixer.soundfonts() {
            let should_reload = self
                .soundfonts
                .get(&resource.id)
                .is_none_or(|loaded| loaded.path != resource.path);
            if should_reload {
                let loaded = LoadedSoundfont::load(resource)?;
                self.soundfonts.insert(resource.id.clone(), loaded);
            }
        }

        Ok(())
    }

    pub(crate) fn sync_tracks_for_soundfont(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        soundfont_id: &str,
    ) -> Result<(), MixerRuntimeError> {
        for track in &mixer.tracks {
            let InstrumentKind::BuiltIn { instrument_id } = &track.instrument.kind else {
                continue;
            };
            if instrument_id != "soundfont" {
                continue;
            }
            let Ok(state) = SoundfontProcessor::decode_state(&track.instrument.state) else {
                continue;
            };
            let track_soundfont_id = state.soundfont_id;
            if track_soundfont_id == soundfont_id {
                self.sync_track_instrument(context, commands, mixer, track.id)?;
            }
        }
        Ok(())
    }

    pub(crate) fn add_bus(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_track = mixer.bus(bus_id)?;
        context.with_activation(|| {
            self.buses
                .insert(bus_id, BusRuntime::new(context, commands, bus_track, mixer));
        });
        self.sync_bus_routing(context, commands, mixer, bus_id)?;
        self.sync_all_levels(commands, mixer);
        Ok(())
    }

    pub(crate) fn remove_bus(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        if let Some(runtime) = self.buses.remove(&bus_id) {
            runtime.free();
        }
        self.sync_all_routing_no_create(commands, mixer)?;
        self.sync_all_levels(commands, mixer);
        Ok(())
    }

    pub(crate) fn sync_track_instrument(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let track = mixer.track(track_id)?;
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        runtime.sync_source(
            context,
            commands,
            track,
            &self.soundfonts,
            self.soundfont_settings,
        )?;
        Ok(())
    }

    pub(crate) fn sync_track_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let track = mixer.track(track_id)?;
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        runtime.rebuild_effects(context, commands, &track.effects);
        Ok(())
    }

    pub(crate) fn sync_track_strip(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let amplitude = track_effective_amplitude(mixer, mixer.track(track_id)?);
        let track = mixer.track(track_id)?;
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        runtime.apply_strip(commands, track, amplitude);
        Ok(())
    }

    pub(crate) fn sync_bus_strip(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let amplitude = bus_effective_amplitude(mixer.bus(bus_id)?);
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        runtime.apply_strip(commands, bus, amplitude);
        Ok(())
    }

    pub(crate) fn sync_bus_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        runtime.rebuild_effects(context, commands, &bus.effects);
        Ok(())
    }

    pub(crate) fn sync_master_effects(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        self.master
            .rebuild_effects(context, commands, &mixer.master.effects);
        Ok(())
    }

    pub(crate) fn sync_track_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        track_id: TrackId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let track = mixer.track(track_id)?;
        let runtime = self
            .tracks
            .get_mut(track_id.index())
            .ok_or(MixerError::InvalidTrackId(track_id))?;
        runtime.sync_routing(
            context,
            commands,
            mixer,
            self.master.input_node(),
            &bus_inputs,
            track,
        )?;
        self.sync_all_levels(commands, mixer);
        Ok(())
    }

    pub(crate) fn sync_bus_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        bus_id: BusId,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        let bus = mixer.bus(bus_id)?;
        let runtime = self
            .buses
            .get_mut(&bus_id)
            .ok_or(MixerError::InvalidBusId(bus_id))?;
        runtime.sync_routing(
            context,
            commands,
            mixer,
            self.master.input_node(),
            &bus_inputs,
            bus,
        )?;
        self.sync_all_levels(commands, mixer);
        Ok(())
    }

    pub(crate) fn sync_all_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        for track in &mixer.tracks {
            self.sync_track_routing(context, commands, mixer, track.id)?;
        }
        let bus_ids: Vec<_> = mixer.buses.iter().map(|bus| bus.id).collect();
        for bus_id in bus_ids {
            self.sync_bus_routing(context, commands, mixer, bus_id)?;
        }
        Ok(())
    }

    fn sync_all_routing_no_create(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Result<(), MixerRuntimeError> {
        let bus_inputs = self.bus_input_nodes();
        for track in &mixer.tracks {
            let runtime = self
                .tracks
                .get_mut(track.id.index())
                .ok_or(MixerError::InvalidTrackId(track.id))?;
            runtime.sync_routing_existing(
                commands,
                mixer,
                self.master.input_node(),
                &bus_inputs,
                track,
            )?;
        }
        for bus in &mixer.buses {
            if let Some(runtime) = self.buses.get_mut(&bus.id) {
                runtime.sync_routing_existing(
                    commands,
                    mixer,
                    self.master.input_node(),
                    &bus_inputs,
                    bus,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn sync_all_levels(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) {
        self.master.set_level(
            db_to_amplitude(mixer.master.state.gain_db),
            mixer.master.state.pan,
        );
        let track_amplitudes: Vec<_> = mixer
            .tracks
            .iter()
            .map(|track| track_effective_amplitude(mixer, track))
            .collect();
        for (runtime, (track, amplitude)) in self
            .tracks
            .iter_mut()
            .zip(mixer.tracks.iter().zip(track_amplitudes.into_iter()))
        {
            runtime.apply_strip(commands, track, amplitude);
        }
        for bus in &mixer.buses {
            if let Some(runtime) = self.buses.get_mut(&bus.id) {
                runtime.apply_strip(commands, bus, bus_effective_amplitude(bus));
            }
        }
    }

    fn bus_input_nodes(&self) -> HashMap<BusId, NodeId> {
        self.buses
            .iter()
            .map(|(bus_id, runtime)| (*bus_id, runtime.input_node()))
            .collect()
    }
}

struct MasterRuntime {
    input: Handle<GenericHandle>,
    effects: Vec<EffectRuntimeHandle>,
    strip: Handle<GenericHandle>,
}

impl MasterRuntime {
    fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Self {
        let input = bus(2);
        let strip = handle(StereoBalanceGain::new());
        set_scalar(strip, 2, db_to_amplitude(mixer.master.state.gain_db));
        set_scalar(strip, 3, mixer.master.state.pan);
        graph_output(0, strip.channels(2));
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
        };
        runtime.rebuild_effects(context, commands, &mixer.master.effects);
        runtime
    }

    fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    fn set_level(&mut self, gain: f32, pan: f32) {
        set_scalar(self.strip, 2, gain);
        set_scalar(self.strip, 3, pan);
    }

    fn rebuild_effects(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        effects: &[crate::instrument::EffectSlotState],
    ) {
        context.with_activation(|| {
            knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
            for effect in self.effects.drain(..) {
                free_effect(effect);
            }
            let mut previous = node_id_of(self.input);
            for effect in effects {
                let Some(effect) = create_effect_runtime(effect) else {
                    continue;
                };
                let node = effect.node_id();
                connect_stereo(previous, node);
                previous = node;
                self.effects.push(effect);
            }
            connect_stereo(previous, node_id_of(self.strip));
        });
    }
}

struct TrackRuntime {
    source_bus: Handle<GenericHandle>,
    effects: Vec<EffectRuntimeHandle>,
    strip: Handle<GenericHandle>,
    route_bus: Handle<GenericHandle>,
    instrument: Option<TrackInstrumentRuntime>,
    send_nodes: Vec<NodeId>,
}

impl TrackRuntime {
    fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        track: &MixerTrack,
        mixer: &MixerState,
        soundfonts: &HashMap<String, LoadedSoundfont>,
        soundfont_settings: SoundfontSynthSettings,
    ) -> Result<Self, MixerRuntimeError> {
        let source_bus = bus(2);
        let strip = handle(StereoBalanceGain::new());
        let route_bus = bus(2);

        connect_stereo(node_id_of(strip), node_id_of(route_bus));
        set_scalar(strip, 2, db_to_amplitude(track.state.gain_db));
        set_scalar(strip, 3, track.state.pan);

        let mut runtime = Self {
            source_bus,
            effects: Vec::new(),
            strip,
            route_bus,
            instrument: None,
            send_nodes: Vec::new(),
        };
        runtime.rebuild_effects(context, commands, &track.effects);
        runtime.sync_source(context, commands, track, soundfonts, soundfont_settings)?;
        runtime.apply_strip(commands, track, track_effective_amplitude(mixer, track));
        Ok(runtime)
    }

    fn sync_source(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        track: &MixerTrack,
        soundfonts: &HashMap<String, LoadedSoundfont>,
        soundfont_settings: SoundfontSynthSettings,
    ) -> Result<(), MixerRuntimeError> {
        context.with_activation(|| {
            if let Some(instrument) = self.instrument.take() {
                instrument.free();
            }
            knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.source_bus)));
        });

        let Some(instrument) =
            create_track_instrument(context, track, soundfonts, soundfont_settings)?
        else {
            return Ok(());
        };
        context.with_activation(|| {
            instrument.connect(node_id_of(self.source_bus));
        });
        self.instrument = Some(instrument);
        Ok(())
    }

    fn rebuild_effects(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        effects: &[crate::instrument::EffectSlotState],
    ) {
        context.with_activation(|| {
            knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.source_bus)));
            for effect in self.effects.drain(..) {
                free_effect(effect);
            }
            let mut previous = node_id_of(self.source_bus);
            for effect in effects {
                let Some(effect) = create_effect_runtime(effect) else {
                    continue;
                };
                let node = effect.node_id();
                connect_stereo(previous, node);
                previous = node;
                self.effects.push(effect);
            }
            connect_stereo(previous, node_id_of(self.strip));
        });
    }

    fn apply_strip(
        &mut self,
        _commands: &mut MultiThreadedKnystCommands,
        track: &MixerTrack,
        gain: f32,
    ) {
        set_scalar(self.strip, 2, gain);
        set_scalar(self.strip, 3, track.state.pan);
    }

    fn sync_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        track: &MixerTrack,
    ) -> Result<(), MixerRuntimeError> {
        self.sync_routing_existing(commands, mixer, master_input, bus_inputs, track)?;
        self.rebuild_sends(
            context,
            commands,
            bus_inputs,
            &track.routing.sends,
            node_id_of(self.source_bus),
            node_id_of(self.strip),
        );
        Ok(())
    }

    fn sync_routing_existing(
        &mut self,
        _commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        track: &MixerTrack,
    ) -> Result<(), MixerRuntimeError> {
        let destination = destination_node(track.routing.main, master_input, bus_inputs, mixer)?;
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        connect_stereo(node_id_of(self.route_bus), destination);
        Ok(())
    }

    fn rebuild_sends(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        bus_inputs: &HashMap<BusId, NodeId>,
        sends: &[BusSend],
        pre_source: NodeId,
        post_source: NodeId,
    ) {
        for node in self.send_nodes.drain(..) {
            knyst_commands().disconnect(Connection::clear_from_nodes(node));
            knyst_commands().disconnect(Connection::clear_to_nodes(node));
            knyst_commands().free_node(node);
        }

        context.with_activation(|| {
            for send in sends {
                let Some(destination) = bus_inputs.get(&send.bus_id).copied() else {
                    continue;
                };
                let gain = handle(StereoGain::new());
                set_scalar(gain, 2, db_to_amplitude(send.gain_db));
                let gain_node = node_id_of(gain);
                connect_stereo(
                    if send.pre_fader {
                        pre_source
                    } else {
                        post_source
                    },
                    gain_node,
                );
                connect_stereo(gain_node, destination);
                self.send_nodes.push(gain_node);
            }
        });
    }
}

struct TrackInstrumentRuntime {
    handle: InstrumentRuntimeHandle,
}

impl TrackInstrumentRuntime {
    fn connect(&self, destination: NodeId) {
        connect_stereo(self.handle.node_id(), destination);
    }

    fn free(self) {
        let node = self.handle.node_id();
        knyst_commands().disconnect(Connection::clear_from_nodes(node));
        knyst_commands().disconnect(Connection::clear_to_nodes(node));
        knyst_commands().free_node(node);
    }
}

fn create_effect_runtime(
    effect: &crate::instrument::EffectSlotState,
) -> Option<EffectRuntimeHandle> {
    let processor = create_effect_processor(effect).ok()??;
    let node = handle(EffectProcessorNode::new(processor));
    Some(EffectRuntimeHandle::new(node))
}

fn free_effect(effect: EffectRuntimeHandle) {
    let node = effect.node_id();
    knyst_commands().disconnect(Connection::clear_from_nodes(node));
    knyst_commands().disconnect(Connection::clear_to_nodes(node));
    knyst_commands().free_node(node);
}

struct BusRuntime {
    input: Handle<GenericHandle>,
    effects: Vec<EffectRuntimeHandle>,
    strip: Handle<GenericHandle>,
    route_bus: Handle<GenericHandle>,
    send_nodes: Vec<NodeId>,
}

impl BusRuntime {
    fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        bus_track: &BusTrack,
        _mixer: &MixerState,
    ) -> Self {
        let input = bus(2);
        let strip = handle(StereoBalanceGain::new());
        let route_bus = bus(2);
        connect_stereo(node_id_of(strip), node_id_of(route_bus));
        set_scalar(strip, 2, db_to_amplitude(bus_track.state.gain_db));
        set_scalar(strip, 3, bus_track.state.pan);
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
            route_bus,
            send_nodes: Vec::new(),
        };
        runtime.rebuild_effects(context, commands, &bus_track.effects);
        runtime
    }

    fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    fn apply_strip(
        &mut self,
        _commands: &mut MultiThreadedKnystCommands,
        bus_track: &BusTrack,
        gain: f32,
    ) {
        set_scalar(self.strip, 2, gain);
        set_scalar(self.strip, 3, bus_track.state.pan);
    }

    fn rebuild_effects(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        effects: &[crate::instrument::EffectSlotState],
    ) {
        context.with_activation(|| {
            knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
            for effect in self.effects.drain(..) {
                free_effect(effect);
            }
            let mut previous = node_id_of(self.input);
            for effect in effects {
                let Some(effect) = create_effect_runtime(effect) else {
                    continue;
                };
                let node = effect.node_id();
                connect_stereo(previous, node);
                previous = node;
                self.effects.push(effect);
            }
            connect_stereo(previous, node_id_of(self.strip));
        });
    }

    fn sync_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        bus_track: &BusTrack,
    ) -> Result<(), MixerRuntimeError> {
        self.sync_routing_existing(commands, mixer, master_input, bus_inputs, bus_track)?;
        self.rebuild_sends(
            context,
            commands,
            bus_inputs,
            &bus_track.routing.sends,
            node_id_of(self.input),
            node_id_of(self.strip),
        );
        Ok(())
    }

    fn sync_routing_existing(
        &mut self,
        _commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        bus_track: &BusTrack,
    ) -> Result<(), MixerRuntimeError> {
        let destination =
            destination_node(bus_track.routing.main, master_input, bus_inputs, mixer)?;
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        connect_stereo(node_id_of(self.route_bus), destination);
        Ok(())
    }

    fn rebuild_sends(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        bus_inputs: &HashMap<BusId, NodeId>,
        sends: &[BusSend],
        pre_source: NodeId,
        post_source: NodeId,
    ) {
        for node in self.send_nodes.drain(..) {
            knyst_commands().disconnect(Connection::clear_from_nodes(node));
            knyst_commands().disconnect(Connection::clear_to_nodes(node));
            knyst_commands().free_node(node);
        }

        context.with_activation(|| {
            for send in sends {
                let Some(destination) = bus_inputs.get(&send.bus_id).copied() else {
                    continue;
                };
                let gain = handle(StereoGain::new());
                set_scalar(gain, 2, db_to_amplitude(send.gain_db));
                let gain_node = node_id_of(gain);
                connect_stereo(
                    if send.pre_fader {
                        pre_source
                    } else {
                        post_source
                    },
                    gain_node,
                );
                connect_stereo(gain_node, destination);
                self.send_nodes.push(gain_node);
            }
        });
    }

    fn free(self) {
        for effect in self.effects {
            free_effect(effect);
        }
        for node in self.send_nodes {
            knyst_commands().disconnect(Connection::clear_from_nodes(node));
            knyst_commands().disconnect(Connection::clear_to_nodes(node));
            knyst_commands().free_node(node);
        }
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.strip)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.strip)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.input)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
        knyst_commands().free_node(node_id_of(self.strip));
        knyst_commands().free_node(node_id_of(self.route_bus));
        knyst_commands().free_node(node_id_of(self.input));
    }
}

fn create_track_instrument(
    context: &KnystContext,
    track: &MixerTrack,
    soundfonts: &HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
) -> Result<Option<TrackInstrumentRuntime>, MixerRuntimeError> {
    match &track.instrument.kind {
        InstrumentKind::BuiltIn { instrument_id } => {
            if instrument_id != "soundfont" {
                return Ok(None);
            }
            let state = SoundfontProcessor::decode_state(&track.instrument.state)?;
            let Some(loaded) = soundfonts.get(&state.soundfont_id) else {
                return Ok(None);
            };
            let processor = SoundfontProcessor::new(&loaded.soundfont, soundfont_settings, state)?;
            let instrument = context.with_activation(|| {
                let node = handle(InstrumentProcessorNode::new(Box::new(processor)));
                TrackInstrumentRuntime {
                    handle: InstrumentRuntimeHandle::new(node),
                }
            });
            Ok(Some(instrument))
        }
        InstrumentKind::Plugin { .. } => Ok(None),
    }
}

fn destination_node(
    route: TrackRoute,
    master_input: NodeId,
    bus_inputs: &HashMap<BusId, NodeId>,
    mixer: &MixerState,
) -> Result<NodeId, MixerRuntimeError> {
    match route {
        TrackRoute::Master => Ok(master_input),
        TrackRoute::Bus(bus_id) => {
            mixer.bus(bus_id)?;
            bus_inputs
                .get(&bus_id)
                .copied()
                .ok_or(MixerError::InvalidBusId(bus_id).into())
        }
    }
}

fn connect_stereo(source: NodeId, destination: NodeId) {
    knyst_commands().connect(source.to(destination).from_index(0).to_index(0));
    knyst_commands().connect(source.to(destination).from_index(1).to_index(1));
}

fn set_scalar(handle: Handle<GenericHandle>, channel: usize, value: f32) {
    handle.set(channel, value);
}

fn node_id_of<H: HandleData + Copy>(handle: Handle<H>) -> NodeId {
    handle
        .node_ids()
        .next()
        .expect("runtime handles should always own one node")
}

fn track_effective_amplitude(mixer: &MixerState, track: &MixerTrack) -> f32 {
    let any_solo = mixer.tracks.iter().any(|track| track.state.soloed)
        || mixer.buses.iter().any(|bus| bus.state.soloed);
    let routed_to_soloed_bus = route_bus_id(track.routing.main)
        .is_some_and(|bus_id| mixer.bus(bus_id).is_ok_and(|bus| bus.state.soloed))
        || track
            .routing
            .sends
            .iter()
            .any(|send| mixer.bus(send.bus_id).is_ok_and(|bus| bus.state.soloed));
    if track.state.muted || (any_solo && !track.state.soloed && !routed_to_soloed_bus) {
        0.0
    } else {
        db_to_amplitude(track.state.gain_db)
    }
}

fn bus_effective_amplitude(bus: &BusTrack) -> f32 {
    if bus.state.muted {
        0.0
    } else {
        db_to_amplitude(bus.state.gain_db)
    }
}

fn route_bus_id(route: TrackRoute) -> Option<BusId> {
    match route {
        TrackRoute::Master => None,
        TrackRoute::Bus(bus_id) => Some(bus_id),
    }
}

fn db_to_amplitude(db: f32) -> f32 {
    knyst::db_to_amplitude(db)
}

pub(super) struct StereoGain;

#[impl_gen]
impl StereoGain {
    #[new]
    fn new() -> Self {
        Self
    }

    #[process]
    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        left_in: &[Sample],
        right_in: &[Sample],
        gain: &[Sample],
        left_out: &mut [Sample],
        right_out: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        for frame in 0..block_size.0 {
            let gain = gain[frame];
            left_out[frame] = left_in[frame] * gain;
            right_out[frame] = right_in[frame] * gain;
        }
        GenState::Continue
    }
}

pub(super) struct StereoBalanceGain;

#[impl_gen]
impl StereoBalanceGain {
    #[new]
    fn new() -> Self {
        Self
    }

    #[process]
    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        left_in: &[Sample],
        right_in: &[Sample],
        gain: &[Sample],
        pan: &[Sample],
        left_out: &mut [Sample],
        right_out: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        for frame in 0..block_size.0 {
            let pan = pan[frame].clamp(-1.0, 1.0);
            let left_gain = if pan > 0.0 { 1.0 - pan } else { 1.0 };
            let right_gain = if pan < 0.0 { 1.0 + pan } else { 1.0 };
            let gain = gain[frame];
            left_out[frame] = left_in[frame] * gain * left_gain;
            right_out[frame] = right_in[frame] * gain * right_gain;
        }
        GenState::Continue
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use knyst::controller::KnystCommands;
    use knyst::modal_interface::knyst_commands;
    use knyst::prelude::{bus, graph_output, handle};

    use super::{
        MixerRuntimeError, TrackRuntime, connect_stereo, create_track_instrument, node_id_of,
    };
    use crate::instrument::soundfont_synth::{
        LoadedSoundfont, SoundfontProcessor, SoundfontSynthSettings,
    };
    use crate::instrument::{
        InstrumentKind, InstrumentProcessorNode, InstrumentRuntimeHandle, InstrumentSlotState,
        MidiEvent, ProcessorState, SoundfontProcessorState,
    };
    use crate::mixer::{Mixer, MixerState, TrackId, TrackRoute};
    use crate::test_utils::{OfflineHarness, test_soundfont_resource};

    fn build_soundfont_mixer(harness: &mut OfflineHarness) -> Result<Mixer, MixerRuntimeError> {
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        for track_id in 1..crate::mixer::INSTRUMENT_TRACK_COUNT {
            state
                .track_mut(TrackId(track_id as u16))
                .expect("track should exist")
                .instrument = InstrumentSlotState {
                kind: InstrumentKind::Plugin {
                    plugin_id: "none".to_string(),
                },
                state: ProcessorState::default(),
            };
        }
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let context = harness.context().clone();
        let settings = harness.settings();
        Mixer::new(&context, harness.commands(), &settings, state)
    }

    #[test]
    fn raw_commands_and_active_context_use_same_graph() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let raw_graph = harness.commands().current_graph();
        let active_graph = harness
            .context()
            .with_activation(|| knyst_commands().current_graph());
        assert_eq!(raw_graph, active_graph);

        let _mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let raw_graph = harness.commands().current_graph();
        let active_graph = harness
            .context()
            .with_activation(|| knyst_commands().current_graph());
        assert_eq!(raw_graph, active_graph);
    }

    #[test]
    fn inspect_mixer_graph() {
        let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.context().with_activation(|| {
            graph_output(2, handle.raw_handle().channels(2));
        });
        let inspection = harness.inspection();
        eprintln!("outputs: {}", inspection.num_outputs);
        eprintln!(
            "graph output edges: {:?}",
            inspection.graph_output_input_edges
        );
        for (index, node) in inspection.nodes.iter().enumerate() {
            eprintln!(
                "{index}: {} {:?} inputs={:?} outputs={:?}",
                node.name, node.address, node.input_edges, node.output_channels
            );
        }
    }

    #[test]
    fn create_track_instrument_returns_audible_handle() {
        let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
        let resource = test_soundfont_resource();
        let loaded = LoadedSoundfont::load(&resource).expect("test SoundFont should load");
        let mut soundfonts = HashMap::new();
        soundfonts.insert(resource.id.clone(), loaded);
        let mut state = MixerState::new();
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let track = state.track(TrackId(0)).expect("track 0 should exist");
        let instrument = create_track_instrument(
            harness.context(),
            track,
            &soundfonts,
            SoundfontSynthSettings::new(44_100, 64),
        )
        .expect("instrument should build")
        .expect("soundfont track should produce an instrument");
        harness.context().with_activation(|| {
            graph_output(2, instrument.handle.raw_handle().channels(2));
        });

        instrument.handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(8);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(
            harness
                .output_channel(2)
                .iter()
                .any(|sample| sample.abs() > 1.0e-6)
        );
    }

    #[test]
    fn track_runtime_keeps_instrument_audible() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let resource = test_soundfont_resource();
        let loaded = LoadedSoundfont::load(&resource).expect("test SoundFont should load");
        let mut soundfonts = HashMap::new();
        soundfonts.insert(resource.id.clone(), loaded);
        let mut state = MixerState::new();
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let track = state.track(TrackId(0)).expect("track 0 should exist");
        let context = harness.context().clone();
        let runtime = TrackRuntime::new(
            &context,
            harness.commands(),
            track,
            &state,
            &soundfonts,
            SoundfontSynthSettings::new(44_100, 64),
        )
        .expect("track runtime should build");
        let handle = runtime
            .instrument
            .as_ref()
            .expect("track runtime should keep instrument")
            .handle;
        harness.context().with_activation(|| {
            graph_output(0, handle.raw_handle().channels(2));
        });

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(8);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(harness.output_has_signal());
    }

    #[test]
    fn fresh_instrument_after_mixer_init_is_audible() {
        let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let resource = test_soundfont_resource();
        let loaded = LoadedSoundfont::load(&resource).expect("test SoundFont should load");
        let mut soundfonts = HashMap::new();
        soundfonts.insert(resource.id.clone(), loaded);
        let track = mixer.state.track(TrackId(0)).expect("track 0 should exist");
        let instrument = create_track_instrument(
            harness.context(),
            track,
            &soundfonts,
            SoundfontSynthSettings::new(44_100, 64),
        )
        .expect("instrument should build")
        .expect("soundfont track should produce an instrument");
        harness.context().with_activation(|| {
            graph_output(2, instrument.handle.raw_handle().channels(2));
        });
        harness.process_blocks(50);

        instrument.handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(
            harness
                .output_channel(2)
                .iter()
                .any(|sample| sample.abs() > 1.0e-6)
        );
    }

    #[test]
    fn track_soundfont_reaches_master_output() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.process_blocks(50);

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(harness.output_has_signal());
    }

    #[test]
    fn track_soundfont_reaches_master_output_with_thread_local_note_on() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.process_blocks(50);

        harness.context().with_activation(|| {
            handle.note_on(0, 60, 100);
        });

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(harness.output_has_signal());
    }

    #[test]
    fn track_source_bus_receives_instrument_audio() {
        let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.context().with_activation(|| {
            graph_output(2, mixer.runtime.tracks[0].source_bus.channels(2));
        });
        harness.process_blocks(50);

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(
            harness
                .output_channel(2)
                .iter()
                .any(|sample| sample.abs() > 1.0e-6)
        );
    }

    #[test]
    fn track_instrument_node_itself_renders_audio() {
        let mut harness = OfflineHarness::new_with_outputs(44_100, 64, 4);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.context().with_activation(|| {
            graph_output(2, handle.raw_handle().channels(2));
        });
        harness.process_blocks(50);

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(
            harness
                .output_channel(2)
                .iter()
                .any(|sample| sample.abs() > 1.0e-6)
        );
    }

    #[test]
    fn direct_node_to_bus_connection_carries_audio() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let processor = SoundfontProcessor::new(
            &loaded.soundfont,
            SoundfontSynthSettings::new(
                harness.settings().sample_rate as i32,
                harness.settings().block_size,
            ),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");
        let (handle, bus_handle) = harness.context().with_activation(|| {
            let node = handle(InstrumentProcessorNode::new(Box::new(processor)));
            let bus_handle = bus(2);
            graph_output(0, bus_handle.channels(2));
            (InstrumentRuntimeHandle::new(node), bus_handle)
        });
        connect_stereo(handle.node_id(), node_id_of(bus_handle));

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(8);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(harness.output_has_signal());
    }

    #[test]
    fn muted_track_stays_silent() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        let track = state.track_mut(TrackId(0)).expect("track 0 should exist");
        track.instrument = InstrumentSlotState::soundfont("default", 0, 0);
        track.state.muted = true;
        let context = harness.context().clone();
        let settings = harness.settings();
        let mixer = Mixer::new(&context, harness.commands(), &settings, state)
            .expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(8);

        assert!(!harness.output_has_signal());
    }

    #[test]
    fn routed_bus_still_reaches_master_output() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mut state = MixerState::new();
        state.set_soundfont(test_soundfont_resource());
        state
            .track_mut(TrackId(0))
            .expect("track 0 should exist")
            .instrument = InstrumentSlotState::soundfont("default", 0, 0);
        let bus_id = state.add_bus("Bus");
        state
            .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
            .expect("track should route to bus");
        let context = harness.context().clone();
        let settings = harness.settings();
        let mixer = Mixer::new(&context, harness.commands(), &settings, state)
            .expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.process_blocks(50);

        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        harness.process_blocks(50);

        assert!(harness.output_has_signal());
    }
}
