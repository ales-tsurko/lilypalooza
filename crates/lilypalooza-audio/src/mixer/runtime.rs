use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use knyst::graph::GenOrGraph;
use knyst::graph::SimultaneousChanges;
use knyst::graph::connection::InputBundle;
use knyst::inputs;
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
    BusId, BusSend, BusTrack, ChannelMeterSnapshot, MixerError, MixerMeterSnapshot, MixerState,
    MixerTrack, STRIP_METER_MAX_DB, STRIP_METER_MIN_DB, StripMeterSnapshot, TrackId, TrackRoute,
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

const METER_FLOOR: f32 = 0.00003162278;

#[derive(Debug, Default, Clone)]
struct SharedStripMeter(Arc<SharedStripMeterInner>);

#[derive(Debug, Default)]
struct SharedStripMeterInner {
    peak_l: AtomicU32,
    peak_r: AtomicU32,
    hold_l: AtomicU32,
    hold_r: AtomicU32,
    clip_latched: AtomicBool,
}

impl SharedStripMeter {
    fn observe_stereo(&self, left: f32, right: f32) {
        let left = left.abs();
        let right = right.abs();

        self.0.peak_l.store(left.to_bits(), Ordering::Relaxed);
        self.0.peak_r.store(right.to_bits(), Ordering::Relaxed);

        let hold_l = f32::from_bits(self.0.hold_l.load(Ordering::Relaxed));
        if left > hold_l {
            self.0.hold_l.store(left.to_bits(), Ordering::Relaxed);
        }

        let hold_r = f32::from_bits(self.0.hold_r.load(Ordering::Relaxed));
        if right > hold_r {
            self.0.hold_r.store(right.to_bits(), Ordering::Relaxed);
        }

        if left >= 1.0 || right >= 1.0 {
            self.0.clip_latched.store(true, Ordering::Relaxed);
        }
    }

    fn reset(&self) {
        self.0.peak_l.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.0.peak_r.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.0.hold_l.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.0.hold_r.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.0.clip_latched.store(false, Ordering::Relaxed);
    }

    fn snapshot(&self) -> StripMeterSnapshot {
        let peak_l = f32::from_bits(self.0.peak_l.load(Ordering::Relaxed));
        let peak_r = f32::from_bits(self.0.peak_r.load(Ordering::Relaxed));
        let hold_l = f32::from_bits(self.0.hold_l.load(Ordering::Relaxed));
        let hold_r = f32::from_bits(self.0.hold_r.load(Ordering::Relaxed));

        StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: normalize_meter_level(peak_l),
                hold: normalize_meter_level(hold_l),
            },
            right: ChannelMeterSnapshot {
                level: normalize_meter_level(peak_r),
                hold: normalize_meter_level(hold_r),
            },
            clip_latched: self.0.clip_latched.load(Ordering::Relaxed),
        }
    }
}

fn normalize_meter_level(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.abs().max(METER_FLOOR).log10();
    ((db - STRIP_METER_MIN_DB) / (STRIP_METER_MAX_DB - STRIP_METER_MIN_DB)).clamp(0.0, 1.0)
}

pub(crate) struct MixerRuntime {
    master: MasterRuntime,
    tracks: Vec<Option<TrackRuntime>>,
    buses: HashMap<BusId, BusRuntime>,
    soundfonts: HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
}

impl MixerRuntime {
    pub(crate) fn instrument_handle(&self, track_id: TrackId) -> Option<InstrumentRuntimeHandle> {
        Some(
            self.tracks
                .get(track_id.index())?
                .as_ref()?
                .instrument
                .as_ref()?
                .handle,
        )
    }

    pub(crate) fn meter_snapshot(&self, mixer: &MixerState) -> MixerMeterSnapshot {
        MixerMeterSnapshot {
            main: self.master.meter.snapshot(),
            tracks: mixer
                .tracks()
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    self.tracks
                        .get(index)
                        .and_then(|runtime| runtime.as_ref())
                        .map_or_else(StripMeterSnapshot::default, |runtime| {
                            runtime.meter.snapshot()
                        })
                })
                .collect(),
            buses: mixer
                .buses()
                .iter()
                .map(|bus| {
                    (
                        bus.id,
                        self.buses
                            .get(&bus.id)
                            .map_or_else(StripMeterSnapshot::default, |runtime| {
                                runtime.meter.snapshot()
                            }),
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn reset_meters(&self) {
        self.master.meter.reset();
        for runtime in self.tracks.iter().flatten() {
            runtime.meter.reset();
        }
        for runtime in self.buses.values() {
            runtime.meter.reset();
        }
    }

    pub(crate) fn reset_master_meter(&self) {
        self.master.meter.reset();
    }

    pub(crate) fn reset_track_meter(&self, id: TrackId) -> Result<(), MixerRuntimeError> {
        let runtime = self
            .tracks
            .get(id.index())
            .ok_or(MixerError::InvalidTrackId(id))?
            .as_ref()
            .ok_or(MixerError::InvalidTrackId(id))?;
        runtime.meter.reset();
        Ok(())
    }

    pub(crate) fn reset_bus_meter(&self, id: BusId) -> Result<(), MixerRuntimeError> {
        let runtime = self.buses.get(&id).ok_or(MixerError::InvalidBusId(id))?;
        runtime.meter.reset();
        Ok(())
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
                tracks.push(if track_needs_runtime(track) {
                    Some(TrackRuntime::new(
                        context,
                        commands,
                        track,
                        mixer,
                        &runtime.soundfonts,
                        runtime.soundfont_settings,
                    )?)
                } else {
                    None
                });
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
        if !track_needs_runtime(track) {
            if let Some(runtime) = runtime.take() {
                runtime.free();
            }
            return Ok(());
        }
        if let Some(runtime) = runtime.as_mut() {
            runtime.sync_source(
                context,
                commands,
                track,
                &self.soundfonts,
                self.soundfont_settings,
            )?;
        } else {
            *runtime = Some(TrackRuntime::new(
                context,
                commands,
                track,
                mixer,
                &self.soundfonts,
                self.soundfont_settings,
            )?);
        }
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
        if !track_needs_runtime(track) {
            if let Some(runtime) = runtime.take() {
                runtime.free();
            }
            return Ok(());
        }
        if let Some(runtime) = runtime.as_mut() {
            runtime.rebuild_effects(context, commands, &track.effects);
        } else {
            *runtime = Some(TrackRuntime::new(
                context,
                commands,
                track,
                mixer,
                &self.soundfonts,
                self.soundfont_settings,
            )?);
        }
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
        if let Some(runtime) = runtime.as_mut() {
            runtime.apply_strip(commands, track, amplitude);
        }
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
        if let Some(runtime) = runtime.as_mut() {
            runtime.sync_routing(
                context,
                commands,
                mixer,
                self.master.input_node(),
                &bus_inputs,
                track,
            )?;
        }
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
        self.sync_all_levels(commands, mixer);
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
            if let Some(runtime) = runtime.as_mut() {
                runtime.sync_routing_existing(
                    commands,
                    mixer,
                    self.master.input_node(),
                    &bus_inputs,
                    track,
                )?;
            }
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
            commands,
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
            if let Some(runtime) = runtime.as_mut() {
                runtime.apply_strip(commands, track, amplitude);
            }
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
    _meter_node: Handle<GenericHandle>,
    meter: SharedStripMeter,
}

impl MasterRuntime {
    fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
    ) -> Self {
        let input = bus(2);
        let meter = SharedStripMeter::default();
        let strip = handle_with_inputs(
            commands,
            StereoBalanceGain::new(),
            inputs!(
                (2 : db_to_amplitude(mixer.master.state.gain_db)),
                (3 : mixer.master.state.pan)
            ),
        );
        let meter_node = handle(MeterTap::new(meter.clone()));
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        graph_output(0, meter_node.channels(2));
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
            _meter_node: meter_node,
            meter,
        };
        runtime.rebuild_effects(context, commands, &mixer.master.effects);
        runtime
    }

    fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    fn set_level(&mut self, commands: &mut MultiThreadedKnystCommands, gain: f32, pan: f32) {
        set_scalar(commands, self.strip, 2, gain);
        set_scalar(commands, self.strip, 3, pan);
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
    meter_node: Handle<GenericHandle>,
    meter: SharedStripMeter,
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
        let initial_gain = track_effective_amplitude(mixer, track);
        let source_bus = bus(2);
        let meter = SharedStripMeter::default();
        let strip = handle_with_inputs(
            commands,
            StereoBalanceGain::new(),
            inputs!((2 : initial_gain), (3 : track.state.pan)),
        );
        let meter_node = handle(MeterTap::new(meter.clone()));
        let route_bus = bus(2);

        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));

        let mut runtime = Self {
            source_bus,
            effects: Vec::new(),
            strip,
            meter_node,
            meter,
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
        commands: &mut MultiThreadedKnystCommands,
        track: &MixerTrack,
        gain: f32,
    ) {
        set_scalar(commands, self.strip, 2, gain);
        set_scalar(commands, self.strip, 3, track.state.pan);
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
        commands: &mut MultiThreadedKnystCommands,
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
                let gain = handle_with_inputs(
                    commands,
                    StereoGain::new(),
                    inputs!((2 : db_to_amplitude(send.gain_db))),
                );
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
        if let Some(instrument) = self.instrument {
            instrument.free();
        }
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
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.meter_node)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.meter_node)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.source_bus)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.source_bus)));
        knyst_commands().free_node(node_id_of(self.strip));
        knyst_commands().free_node(node_id_of(self.meter_node));
        knyst_commands().free_node(node_id_of(self.route_bus));
        knyst_commands().free_node(node_id_of(self.source_bus));
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
    meter_node: Handle<GenericHandle>,
    meter: SharedStripMeter,
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
        let initial_gain = bus_effective_amplitude(bus_track);
        let input = bus(2);
        let meter = SharedStripMeter::default();
        let strip = handle_with_inputs(
            commands,
            StereoBalanceGain::new(),
            inputs!((2 : initial_gain), (3 : bus_track.state.pan)),
        );
        let meter_node = handle(MeterTap::new(meter.clone()));
        let route_bus = bus(2);
        connect_stereo(node_id_of(strip), node_id_of(meter_node));
        connect_stereo(node_id_of(meter_node), node_id_of(route_bus));
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
            meter_node,
            meter,
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
        commands: &mut MultiThreadedKnystCommands,
        bus_track: &BusTrack,
        gain: f32,
    ) {
        set_scalar(commands, self.strip, 2, gain);
        set_scalar(commands, self.strip, 3, bus_track.state.pan);
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
        commands: &mut MultiThreadedKnystCommands,
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
                let gain = handle_with_inputs(
                    commands,
                    StereoGain::new(),
                    inputs!((2 : db_to_amplitude(send.gain_db))),
                );
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
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.meter_node)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.meter_node)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.input)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
        knyst_commands().free_node(node_id_of(self.strip));
        knyst_commands().free_node(node_id_of(self.meter_node));
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

fn handle_with_inputs(
    commands: &mut MultiThreadedKnystCommands,
    processor: impl GenOrGraph,
    inputs: impl Into<InputBundle>,
) -> Handle<GenericHandle> {
    let num_inputs = processor.num_inputs();
    let num_outputs = processor.num_outputs();
    let node_id = commands.push(processor, inputs);
    Handle::new(GenericHandle::new(node_id, num_inputs, num_outputs))
}

fn set_scalar(
    commands: &mut MultiThreadedKnystCommands,
    handle: Handle<GenericHandle>,
    channel: usize,
    value: f32,
) {
    let mut changes = SimultaneousChanges::duration_from_now(Duration::ZERO);
    changes.push(node_id_of(handle).change().set(channel, value));
    commands.schedule_changes(changes);
}

fn node_id_of<H: HandleData + Copy>(handle: Handle<H>) -> NodeId {
    handle
        .node_ids()
        .next()
        .expect("runtime handles should always own one node")
}

#[derive(Debug, Clone)]
struct MeterTap {
    meter: SharedStripMeter,
}

#[impl_gen]
impl MeterTap {
    #[new]
    fn new(meter: SharedStripMeter) -> Self {
        Self { meter }
    }

    #[process]
    fn process(
        &mut self,
        left_in: &[Sample],
        right_in: &[Sample],
        left_out: &mut [Sample],
        right_out: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        let mut peak_left = 0.0_f32;
        let mut peak_right = 0.0_f32;

        for frame in 0..block_size.0 {
            let left = left_in[frame];
            let right = right_in[frame];
            left_out[frame] = left;
            right_out[frame] = right;
            peak_left = peak_left.max(left.abs());
            peak_right = peak_right.max(right.abs());
        }

        self.meter.observe_stereo(peak_left, peak_right);
        GenState::Continue
    }
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

fn track_needs_runtime(track: &MixerTrack) -> bool {
    !matches!(
        track.instrument.kind,
        InstrumentKind::BuiltIn { ref instrument_id } if instrument_id == "none"
    ) || !track.effects.is_empty()
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
    use knyst::controller::KnystCommands;
    use knyst::modal_interface::knyst_commands;
    use knyst::prelude::{BlockSize, GenState, Sample, bus, graph_output, handle, impl_gen};

    use super::{
        MixerRuntimeError, SharedStripMeter, connect_stereo, node_id_of, normalize_meter_level,
    };
    use crate::instrument::{InstrumentKind, InstrumentSlotState, MidiEvent, ProcessorState};
    use crate::mixer::{Mixer, MixerState, TrackId};
    use crate::test_utils::{OfflineHarness, test_soundfont_resource};

    #[test]
    fn strip_meter_captures_stereo_peak_and_hold() {
        let meter = SharedStripMeter::default();

        meter.observe_stereo(0.25, 0.5);
        let snapshot = meter.snapshot();

        assert!(snapshot.left.level > 0.0);
        assert!(snapshot.right.level > snapshot.left.level);
        assert_eq!(snapshot.left.hold, snapshot.left.level);
        assert_eq!(snapshot.right.hold, snapshot.right.level);
        assert!(!snapshot.clip_latched);
    }

    #[test]
    fn strip_meter_hold_and_clip_stick_until_reset() {
        let meter = SharedStripMeter::default();

        meter.observe_stereo(1.1, 0.2);
        let hot = meter.snapshot();
        meter.observe_stereo(0.1, 0.05);
        let cooled = meter.snapshot();

        assert!(hot.clip_latched);
        assert!(cooled.clip_latched);
        assert_eq!(cooled.left.hold, hot.left.hold);

        meter.reset();
        let reset = meter.snapshot();
        assert!(!reset.clip_latched);
        assert_eq!(reset.left.hold, 0.0);
        assert_eq!(reset.right.hold, 0.0);
    }

    #[test]
    fn meter_snapshot_normalizes_db_monotonically() {
        assert!(normalize_meter_level(0.05) < normalize_meter_level(0.5));
        assert!(normalize_meter_level(0.5) < normalize_meter_level(1.0));
    }

    #[test]
    fn reset_one_strip_meter_does_not_touch_another() {
        let left = SharedStripMeter::default();
        let right = SharedStripMeter::default();

        left.observe_stereo(1.1, 0.7);
        right.observe_stereo(0.9, 0.8);

        left.reset();

        let left_snapshot = left.snapshot();
        let right_snapshot = right.snapshot();

        assert!(!left_snapshot.clip_latched);
        assert_eq!(left_snapshot.left.hold, 0.0);
        assert_eq!(left_snapshot.right.hold, 0.0);

        assert!(right_snapshot.clip_latched);
        assert!(right_snapshot.left.hold > 0.0);
        assert!(right_snapshot.right.hold > 0.0);
    }

    struct TestSineGen {
        phase: f32,
        phase_increment: f32,
    }

    #[impl_gen]
    impl TestSineGen {
        #[new]
        fn new(sample_rate: f32, frequency: f32) -> Self {
            Self {
                phase: 0.0,
                phase_increment: std::f32::consts::TAU * frequency / sample_rate,
            }
        }

        #[process]
        fn process(
            &mut self,
            left_out: &mut [Sample],
            right_out: &mut [Sample],
            block_size: BlockSize,
        ) -> GenState {
            for frame in 0..block_size.0 {
                let sample = self.phase.sin();
                left_out[frame] = sample;
                right_out[frame] = sample * 0.5;
                self.phase += self.phase_increment;
            }
            GenState::Continue
        }
    }

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
    fn track_soundfont_reaches_master_output_with_thread_local_note_on() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let mixer = build_soundfont_mixer(&mut harness).expect("mixer should initialize");
        let handle = mixer
            .instrument_handle(TrackId(0))
            .expect("track instrument should exist");
        harness.process_blocks(50);
        harness.commands().transport_play();
        harness.process_block();

        harness.context().with_activation(|| {
            handle.note_on(0, 60, 100);
        });

        harness.process_blocks(50);

        assert!(harness.errors().is_empty(), "{:?}", harness.errors());
        assert!(harness.output_has_signal());
    }

    #[test]
    fn direct_sine_node_to_bus_preserves_expected_samples() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let bus_handle = harness.context().with_activation(|| {
            let signal = handle(TestSineGen::new(44_100.0, 440.0));
            let bus_handle = bus(2);
            graph_output(0, bus_handle.channels(2));
            connect_stereo(node_id_of(signal), node_id_of(bus_handle));
            bus_handle
        });

        let _ = bus_handle;
        harness.process_block();

        let phase_increment = std::f32::consts::TAU * 440.0 / 44_100.0;
        for frame in 0..8 {
            let expected_left = (phase_increment * frame as f32).sin();
            let expected_right = expected_left * 0.5;
            assert!((harness.output_channel(0)[frame] - expected_left).abs() < 1.0e-5);
            assert!((harness.output_channel(1)[frame] - expected_right).abs() < 1.0e-5);
        }
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
        harness.commands().transport_play();
        harness.process_blocks(4);

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
}
