use super::*;

pub(super) struct TrackRuntime {
    pub(super) effects: Vec<Option<EffectRuntime>>,
    pub(super) meter: SharedStripMeter,
    pub(super) level: SharedStripLevel,
    pub(super) route_bus: Handle<GenericHandle>,
    pub(super) route_delay_node: Option<NodeId>,
    pub(super) instrument: Option<TrackInstrumentRuntime>,
    pub(super) sends: Vec<SendRuntime>,
    pub(super) signal_path: TrackSignalPath,
    pub(super) sample_rate: usize,
}

pub(super) struct TrackRuntimeBuildContext<'a> {
    pub(super) settings: &'a AudioEngineSettings,
    pub(super) mixer: &'a MixerState,
    pub(super) master_input: NodeId,
    pub(super) bus_inputs: &'a HashMap<BusId, NodeId>,
    pub(super) soundfont_resources: &'a [SoundfontResource],
    pub(super) soundfonts: &'a HashMap<String, LoadedSoundfont>,
    pub(super) soundfont_settings: SoundfontSynthSettings,
}

#[derive(Clone, Copy)]
pub(super) struct RoutingTargets<'a> {
    pub(super) master_input: NodeId,
    pub(super) bus_inputs: &'a HashMap<BusId, NodeId>,
    pub(super) pdc_plan: &'a PdcPlan,
}

#[derive(Clone, Copy)]
pub(super) struct LatentNode {
    pub(super) node: NodeId,
    pub(super) latency: u32,
}

pub(super) struct SendRouting<'a> {
    pub(super) bus_inputs: &'a HashMap<BusId, NodeId>,
    pub(super) pdc_plan: &'a PdcPlan,
    pub(super) sends: &'a [BusSend],
    pub(super) pre_source: LatentNode,
    pub(super) post_source: LatentNode,
    pub(super) sample_rate: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SendTopology {
    pub(super) bus_id: BusId,
    pub(super) pre_fader: bool,
}

impl From<BusSend> for SendTopology {
    fn from(send: BusSend) -> Self {
        Self {
            bus_id: send.bus_id,
            pre_fader: send.pre_fader,
        }
    }
}

pub(super) struct SendRuntime {
    pub(super) topology: SendTopology,
    pub(super) level: SharedAudioValue,
    pub(super) nodes: Vec<NodeId>,
}

impl SendRuntime {
    pub(super) fn set_send(&self, send: BusSend) {
        let target = if send.enabled {
            db_to_amplitude(send.gain_db)
        } else {
            0.0
        };
        self.level.set(target);
    }

    pub(super) fn free(self) {
        for node in self.nodes {
            free_node(node);
        }
    }
}

impl TrackRuntime {
    pub(super) fn latencies(&self, _track: &Track) -> StripLatency {
        let instrument_latency = self
            .instrument
            .as_ref()
            .map_or(0, TrackInstrumentRuntime::latency_samples);
        let effects_latency = self
            .effects
            .iter()
            .filter_map(|runtime| runtime.as_ref())
            .map(EffectRuntime::latency_samples)
            .sum::<u32>();
        let post_fader = instrument_latency.saturating_add(effects_latency);
        StripLatency {
            pre_fader: instrument_latency,
            post_fader,
            output: post_fader,
        }
    }

    pub(super) fn pre_send_source_node(&self) -> NodeId {
        match &self.signal_path {
            TrackSignalPath::Separated { source_bus, .. } => node_id_of(*source_bus),
            TrackSignalPath::Combined => self
                .instrument
                .as_ref()
                .map(|instrument| instrument.handle.node_id())
                .unwrap_or_else(|| node_id_of(self.route_bus)),
        }
    }

    pub(super) fn post_send_source_node(&self) -> NodeId {
        match &self.signal_path {
            TrackSignalPath::Separated { strip, .. } => node_id_of(*strip),
            TrackSignalPath::Combined => self
                .instrument
                .as_ref()
                .map(|instrument| instrument.handle.node_id())
                .unwrap_or_else(|| node_id_of(self.route_bus)),
        }
    }

    pub(super) fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        track: &Track,
        build: TrackRuntimeBuildContext<'_>,
    ) -> Result<Self, MixerRuntimeError> {
        let initial_gain = track_effective_amplitude(build.mixer, track);
        let meter = SharedStripMeter::new(build.settings.sample_rate, build.settings.block_size);
        let level = SharedStripLevel::new(initial_gain, track.state.pan);
        let route_bus = context.with_activation(|| bus(2));

        let mut instrument = None;
        let signal_path = if track_prefers_combined_signal_path(track) {
            if let Some(created_instrument) = create_track_instrument(
                context,
                track,
                build.soundfont_resources,
                build.soundfonts,
                build.soundfont_settings,
                Some((level.clone(), meter.clone())),
            )? {
                context.with_activation(|| {
                    created_instrument.connect(node_id_of(route_bus));
                });
                instrument = Some(created_instrument);
                TrackSignalPath::Combined
            } else {
                TrackSignalPath::Separated {
                    source_bus: create_track_source_bus(context),
                    strip: create_track_strip(commands, level.clone(), meter.clone(), route_bus),
                }
            }
        } else {
            TrackSignalPath::Separated {
                source_bus: create_track_source_bus(context),
                strip: create_track_strip(commands, level.clone(), meter.clone(), route_bus),
            }
        };

        let mut runtime = Self {
            effects: Vec::new(),
            meter,
            level,
            route_bus,
            route_delay_node: None,
            instrument,
            sends: Vec::new(),
            signal_path,
            sample_rate: build.settings.sample_rate,
        };
        if !matches!(runtime.signal_path, TrackSignalPath::Combined) {
            runtime.rebuild_effects(context, commands, track, build.settings);
            runtime.sync_source(
                context,
                commands,
                track,
                build.soundfont_resources,
                build.soundfonts,
                build.soundfont_settings,
            )?;
        }
        let pdc_plan = PdcPlan::default();
        let targets = RoutingTargets {
            master_input: build.master_input,
            bus_inputs: build.bus_inputs,
            pdc_plan: &pdc_plan,
        };
        runtime.sync_routing(context, commands, build.mixer, targets, track)?;
        Ok(runtime)
    }

    pub(super) fn sync_source(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        track: &Track,
        soundfont_resources: &[SoundfontResource],
        soundfonts: &HashMap<String, LoadedSoundfont>,
        soundfont_settings: SoundfontSynthSettings,
    ) -> Result<bool, MixerRuntimeError> {
        if matches!(self.signal_path, TrackSignalPath::Combined) {
            if let Some(instrument) = self.instrument.as_mut()
                && instrument.update_in_place(track)?
            {
                return Ok(true);
            }
            context.with_activation(|| {
                if let Some(instrument) = self.instrument.take() {
                    instrument.free();
                }
            });
            let Some(instrument) = create_track_instrument(
                context,
                track,
                soundfont_resources,
                soundfonts,
                soundfont_settings,
                Some((self.level.clone(), self.meter.clone())),
            )?
            else {
                return Ok(false);
            };
            context.with_activation(|| {
                instrument.connect(node_id_of(self.route_bus));
            });
            self.instrument = Some(instrument);
            return Ok(false);
        }

        if let Some(instrument) = self.instrument.as_mut()
            && instrument.update_in_place(track)?
        {
            return Ok(true);
        }
        context.with_activation(|| {
            if let Some(instrument) = self.instrument.take() {
                instrument.free();
            }
            knyst_commands().disconnect(Connection::clear_from_nodes(self.pre_send_source_node()));
        });

        let Some(instrument) = create_track_instrument(
            context,
            track,
            soundfont_resources,
            soundfonts,
            soundfont_settings,
            None,
        )?
        else {
            return Ok(false);
        };
        context.with_activation(|| {
            instrument.connect(self.pre_send_source_node());
        });
        self.instrument = Some(instrument);
        Ok(false)
    }

    pub(super) fn matches_signal_path(&self, track: &Track) -> bool {
        matches!(self.signal_path, TrackSignalPath::Combined)
            == track_prefers_combined_signal_path(track)
    }

    pub(super) fn sync_effect_bypass(&self, effect_index: usize, bypassed: bool) {
        if let Some(Some(effect)) = self.effects.get(effect_index) {
            effect.sync_bypass(bypassed);
        }
    }

    pub(super) fn rebuild_effects(
        &mut self,
        context: &KnystContext,
        _commands: &mut MultiThreadedKnystCommands,
        track: &Track,
        settings: &AudioEngineSettings,
    ) -> bool {
        if matches!(self.signal_path, TrackSignalPath::Combined) {
            return track_prefers_combined_signal_path(track);
        }
        context.with_activation(|| {
            disconnect_effect_chain(self.pre_send_source_node(), &self.effects);
            sync_effect_runtimes(&mut self.effects, track.effects(), settings);
            let mut previous = self.pre_send_source_node();
            for effect in &self.effects {
                if let Some(effect) = effect.as_ref() {
                    let node = effect.node_id();
                    connect_stereo(previous, node);
                    previous = node;
                }
            }
            connect_stereo(previous, self.post_send_source_node());
        });
        true
    }

    pub(super) fn apply_strip(&mut self, track: &Track, gain: f32) {
        self.level.set(gain, track.state.pan);
    }

    pub(super) fn sync_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        targets: RoutingTargets<'_>,
        track: &Track,
    ) -> Result<(), MixerRuntimeError> {
        context.with_activation(|| self.sync_routing_existing(commands, mixer, targets, track))?;
        let strip_latency = self.latencies(track);
        self.rebuild_sends(
            context,
            commands,
            SendRouting {
                bus_inputs: targets.bus_inputs,
                pdc_plan: targets.pdc_plan,
                sends: &track.routing.sends,
                pre_source: LatentNode {
                    node: self.pre_send_source_node(),
                    latency: strip_latency.pre_fader,
                },
                post_source: LatentNode {
                    node: self.post_send_source_node(),
                    latency: strip_latency.post_fader,
                },
                sample_rate: self.sample_rate,
            },
        );
        Ok(())
    }

    pub(super) fn sync_routing_existing(
        &mut self,
        _commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        targets: RoutingTargets<'_>,
        track: &Track,
    ) -> Result<(), MixerRuntimeError> {
        let destination = destination_node(
            track.routing.main,
            targets.master_input,
            targets.bus_inputs,
            mixer,
        )?;
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        free_optional_node(self.route_delay_node.take());
        let delay = targets
            .pdc_plan
            .route_delay(track.routing.main, self.latencies(track).output);
        self.route_delay_node =
            connect_stereo_with_delay(_commands, node_id_of(self.route_bus), destination, delay);
        Ok(())
    }

    pub(super) fn rebuild_sends(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        routing: SendRouting<'_>,
    ) {
        for send in self.sends.drain(..) {
            send.free();
        }

        context.with_activation(|| {
            for send in routing.sends {
                let topology = SendTopology::from(*send);
                let Some(destination) = routing.bus_inputs.get(&topology.bus_id).copied() else {
                    continue;
                };
                let level = SharedAudioValue::new(if send.enabled {
                    db_to_amplitude(send.gain_db)
                } else {
                    0.0
                });
                let gain = handle_with_inputs(
                    commands,
                    StereoGain::new(level.clone(), routing.sample_rate),
                    inputs!(),
                );
                let gain_node = node_id_of(gain);
                let source = if topology.pre_fader {
                    routing.pre_source
                } else {
                    routing.post_source
                };
                connect_stereo(source.node, gain_node);
                let delay = routing
                    .pdc_plan
                    .bus_send_delay(topology.bus_id, source.latency);
                let mut nodes = Vec::with_capacity(2);
                if let Some(delay_node) =
                    connect_stereo_with_delay(commands, gain_node, destination, delay)
                {
                    nodes.push(delay_node);
                }
                nodes.push(gain_node);
                self.sends.push(SendRuntime {
                    topology,
                    level,
                    nodes,
                });
            }
        });
    }

    pub(super) fn sync_send_levels(&self, sends: &[BusSend]) {
        for (runtime, send) in self.sends.iter().zip(sends) {
            if runtime.topology == SendTopology::from(*send) {
                runtime.set_send(*send);
            }
        }
    }

    pub(super) fn free(self) {
        if let Some(instrument) = self.instrument {
            instrument.free();
        }
        for effect in self.effects.into_iter().flatten() {
            free_effect(effect);
        }
        for send in self.sends {
            send.free();
        }
        free_optional_node(self.route_delay_node);
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.route_bus)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        self.signal_path.free();
        knyst_commands().free_node(node_id_of(self.route_bus));
    }
}
