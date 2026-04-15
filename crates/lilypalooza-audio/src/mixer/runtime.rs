use std::collections::HashMap;

use crate::engine::AudioEngineSettings;
use crate::instrument::soundfont_synth::{
    LoadedSoundfont, SoundfontProcessor, SoundfontSynthError, SoundfontSynthSettings,
};
use knyst::modal_interface::KnystContext;
use knyst::prelude::{
    BlockSize, Connection, GenState, GenericHandle, Handle, HandleData, KnystCommands,
    MultiThreadedKnystCommands, NodeId, Sample, WavetableId, bus, graph_output, handle, impl_gen,
    oscillator,
};

use crate::instrument::{
    InstrumentKind, InstrumentProcessorNode, InstrumentRuntimeHandle, ProcessorStateError,
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
    pub(crate) fn attach(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        mixer: &MixerState,
    ) -> Result<Self, MixerRuntimeError> {
        context.with_activation(|| {
            let master = MasterRuntime::new(commands, mixer);
            let soundfont_settings =
                SoundfontSynthSettings::new(settings.sample_rate as i32, settings.block_size);

            let mut buses = HashMap::with_capacity(mixer.buses.len());
            for bus_track in &mixer.buses {
                buses.insert(bus_track.id, BusRuntime::new(commands, bus_track, mixer));
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
                .insert(bus_id, BusRuntime::new(commands, bus_track, mixer));
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
            runtime.free(commands);
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
    strip: Handle<GenericHandle>,
}

impl MasterRuntime {
    fn new(commands: &mut MultiThreadedKnystCommands, mixer: &MixerState) -> Self {
        let input = bus(2);
        let strip = handle(StereoBalanceGain::new());
        connect_stereo(commands, node_id_of(input), node_id_of(strip));
        set_scalar(strip, 2, db_to_amplitude(mixer.master.state.gain_db));
        set_scalar(strip, 3, mixer.master.state.pan);
        graph_output(0, strip.channels(2));
        Self { input, strip }
    }

    fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    fn set_level(&mut self, gain: f32, pan: f32) {
        set_scalar(self.strip, 2, gain);
        set_scalar(self.strip, 3, pan);
    }
}

struct TrackRuntime {
    source_bus: Handle<GenericHandle>,
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

        connect_stereo(commands, node_id_of(source_bus), node_id_of(strip));
        connect_stereo(commands, node_id_of(strip), node_id_of(route_bus));
        set_scalar(strip, 2, db_to_amplitude(track.state.gain_db));
        set_scalar(strip, 3, track.state.pan);

        let mut runtime = Self {
            source_bus,
            strip,
            route_bus,
            instrument: None,
            send_nodes: Vec::new(),
        };
        runtime.sync_source(context, commands, track, soundfonts, soundfont_settings)?;
        runtime.apply_strip(commands, track, track_effective_amplitude(mixer, track));
        Ok(runtime)
    }

    fn sync_source(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        track: &MixerTrack,
        soundfonts: &HashMap<String, LoadedSoundfont>,
        soundfont_settings: SoundfontSynthSettings,
    ) -> Result<(), MixerRuntimeError> {
        if let Some(instrument) = self.instrument.take() {
            instrument.free(commands);
        }
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.source_bus)));

        let Some(instrument) =
            create_track_instrument(context, commands, track, soundfonts, soundfont_settings)?
        else {
            return Ok(());
        };
        instrument.connect(commands, node_id_of(self.source_bus));
        self.instrument = Some(instrument);
        Ok(())
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
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        track: &MixerTrack,
    ) -> Result<(), MixerRuntimeError> {
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        let destination = destination_node(track.routing.main, master_input, bus_inputs, mixer)?;
        connect_stereo(commands, node_id_of(self.route_bus), destination);
        Ok(())
    }

    fn rebuild_sends(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        bus_inputs: &HashMap<BusId, NodeId>,
        sends: &[BusSend],
        pre_source: NodeId,
        post_source: NodeId,
    ) {
        for node in self.send_nodes.drain(..) {
            commands.disconnect(Connection::clear_from_nodes(node));
            commands.disconnect(Connection::clear_to_nodes(node));
            commands.free_node(node);
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
                    commands,
                    if send.pre_fader {
                        pre_source
                    } else {
                        post_source
                    },
                    gain_node,
                );
                connect_stereo(commands, gain_node, destination);
                self.send_nodes.push(gain_node);
            }
        });
    }
}

enum TrackInstrumentRuntime {
    BuiltInMono { source: NodeId, stereoizer: NodeId },
    Processor { handle: InstrumentRuntimeHandle },
}

impl TrackInstrumentRuntime {
    fn connect(&self, commands: &mut MultiThreadedKnystCommands, destination: NodeId) {
        match self {
            Self::BuiltInMono { stereoizer, .. } => {
                connect_stereo(commands, *stereoizer, destination);
            }
            Self::Processor { handle } => {
                connect_stereo(commands, handle.node_id(), destination);
            }
        }
    }

    fn free(self, commands: &mut MultiThreadedKnystCommands) {
        match self {
            Self::BuiltInMono { source, stereoizer } => {
                commands.disconnect(Connection::clear_from_nodes(source));
                commands.disconnect(Connection::clear_to_nodes(source));
                commands.disconnect(Connection::clear_from_nodes(stereoizer));
                commands.disconnect(Connection::clear_to_nodes(stereoizer));
                commands.free_node(source);
                commands.free_node(stereoizer);
            }
            Self::Processor { handle } => {
                let node = handle.node_id();
                commands.disconnect(Connection::clear_from_nodes(node));
                commands.disconnect(Connection::clear_to_nodes(node));
                commands.free_node(node);
            }
        }
    }
}

struct BusRuntime {
    input: Handle<GenericHandle>,
    strip: Handle<GenericHandle>,
    route_bus: Handle<GenericHandle>,
    send_nodes: Vec<NodeId>,
}

impl BusRuntime {
    fn new(
        commands: &mut MultiThreadedKnystCommands,
        bus_track: &BusTrack,
        _mixer: &MixerState,
    ) -> Self {
        let input = bus(2);
        let strip = handle(StereoBalanceGain::new());
        let route_bus = bus(2);
        connect_stereo(commands, node_id_of(input), node_id_of(strip));
        connect_stereo(commands, node_id_of(strip), node_id_of(route_bus));
        set_scalar(strip, 2, db_to_amplitude(bus_track.state.gain_db));
        set_scalar(strip, 3, bus_track.state.pan);
        Self {
            input,
            strip,
            route_bus,
            send_nodes: Vec::new(),
        }
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
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        master_input: NodeId,
        bus_inputs: &HashMap<BusId, NodeId>,
        bus_track: &BusTrack,
    ) -> Result<(), MixerRuntimeError> {
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        let destination =
            destination_node(bus_track.routing.main, master_input, bus_inputs, mixer)?;
        connect_stereo(commands, node_id_of(self.route_bus), destination);
        Ok(())
    }

    fn rebuild_sends(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        bus_inputs: &HashMap<BusId, NodeId>,
        sends: &[BusSend],
        pre_source: NodeId,
        post_source: NodeId,
    ) {
        for node in self.send_nodes.drain(..) {
            commands.disconnect(Connection::clear_from_nodes(node));
            commands.disconnect(Connection::clear_to_nodes(node));
            commands.free_node(node);
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
                    commands,
                    if send.pre_fader {
                        pre_source
                    } else {
                        post_source
                    },
                    gain_node,
                );
                connect_stereo(commands, gain_node, destination);
                self.send_nodes.push(gain_node);
            }
        });
    }

    fn free(self, commands: &mut MultiThreadedKnystCommands) {
        for node in self.send_nodes {
            commands.disconnect(Connection::clear_from_nodes(node));
            commands.disconnect(Connection::clear_to_nodes(node));
            commands.free_node(node);
        }
        commands.disconnect(Connection::clear_from_nodes(node_id_of(self.strip)));
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.strip)));
        commands.disconnect(Connection::clear_from_nodes(node_id_of(self.route_bus)));
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        commands.disconnect(Connection::clear_from_nodes(node_id_of(self.input)));
        commands.disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
        commands.free_node(node_id_of(self.strip));
        commands.free_node(node_id_of(self.route_bus));
        commands.free_node(node_id_of(self.input));
    }
}

fn create_track_instrument(
    context: &KnystContext,
    commands: &mut MultiThreadedKnystCommands,
    track: &MixerTrack,
    soundfonts: &HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
) -> Result<Option<TrackInstrumentRuntime>, MixerRuntimeError> {
    match &track.instrument.kind {
        InstrumentKind::BuiltIn { instrument_id } => {
            if instrument_id == "soundfont" {
                let state = SoundfontProcessor::decode_state(&track.instrument.state)?;
                let Some(loaded) = soundfonts.get(&state.soundfont_id) else {
                    return Ok(None);
                };
                let processor =
                    SoundfontProcessor::new(&loaded.soundfont, soundfont_settings, state)?;
                let instrument = context.with_activation(|| {
                    let node = handle(InstrumentProcessorNode::new(Box::new(processor)));
                    TrackInstrumentRuntime::Processor {
                        handle: InstrumentRuntimeHandle::new(node),
                    }
                });
                return Ok(Some(instrument));
            }
            if instrument_id != "test-tone" && instrument_id != "test-sine" {
                return Ok(None);
            }
            let instrument = context.with_activation(|| {
                let frequency = test_tone_frequency(track.id);
                let source = oscillator(WavetableId::cos()).freq(frequency) * 0.12;
                let stereoizer = handle(MonoToStereo::new());
                commands.connect(node_id_of(source).to(node_id_of(stereoizer)).to_index(0));
                TrackInstrumentRuntime::BuiltInMono {
                    source: node_id_of(source),
                    stereoizer: node_id_of(stereoizer),
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

fn connect_stereo(commands: &mut MultiThreadedKnystCommands, source: NodeId, destination: NodeId) {
    commands.connect(source.to(destination).from_index(0).to_index(0));
    commands.connect(source.to(destination).from_index(1).to_index(1));
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

fn test_tone_frequency(track_id: TrackId) -> f32 {
    let semitones = (track_id.index() % 24) as f32;
    110.0 * 2.0f32.powf(semitones / 12.0)
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

pub(super) struct MonoToStereo;

#[impl_gen]
impl MonoToStereo {
    #[new]
    fn new() -> Self {
        Self
    }

    #[process]
    fn process(
        &mut self,
        signal: &[Sample],
        left: &mut [Sample],
        right: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        left[..block_size.0].copy_from_slice(&signal[..block_size.0]);
        right[..block_size.0].copy_from_slice(&signal[..block_size.0]);
        GenState::Continue
    }
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
