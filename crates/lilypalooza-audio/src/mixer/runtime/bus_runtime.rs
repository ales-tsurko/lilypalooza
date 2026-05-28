use super::*;

pub(super) struct BusRuntime {
    pub(super) input: Handle<GenericHandle>,
    pub(super) effects: Vec<Option<EffectRuntime>>,
    pub(super) strip: Handle<GenericHandle>,
    pub(super) meter: SharedStripMeter,
    pub(super) level: SharedStripLevel,
    pub(super) route_bus: Handle<GenericHandle>,
    pub(super) route_delay_node: Option<NodeId>,
    pub(super) sends: Vec<SendRuntime>,
    pub(super) sample_rate: usize,
}

impl BusRuntime {
    pub(super) fn latencies(&self, _bus_track: &Track, input_latency: u32) -> StripLatency {
        let effects_latency = self
            .effects
            .iter()
            .filter_map(|runtime| runtime.as_ref())
            .map(EffectRuntime::latency_samples)
            .sum::<u32>();
        let post_fader = input_latency.saturating_add(effects_latency);
        StripLatency {
            pre_fader: input_latency,
            post_fader,
            output: post_fader,
        }
    }

    pub(super) fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        bus_track: &Track,
        _mixer: &MixerState,
    ) -> Self {
        let initial_gain = bus_effective_amplitude(bus_track);
        let meter = SharedStripMeter::new(settings.sample_rate, settings.block_size);
        let level = SharedStripLevel::new(initial_gain, bus_track.state.pan);
        let strip = handle_with_inputs(
            commands,
            StereoBalanceMeter::new(level.clone(), meter.clone(), settings.sample_rate),
            inputs!(),
        );
        let (input, route_bus) = context.with_activation(|| {
            let input = bus(2);
            let route_bus = bus(2);
            connect_stereo(node_id_of(strip), node_id_of(route_bus));
            (input, route_bus)
        });
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
            meter,
            level,
            route_bus,
            route_delay_node: None,
            sends: Vec::new(),
            sample_rate: settings.sample_rate,
        };
        runtime.rebuild_effects(context, commands, bus_track, settings);
        runtime
    }

    pub(super) fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    pub(super) fn apply_strip(&mut self, bus_track: &Track, gain: f32) {
        self.level.set(gain, bus_track.state.pan);
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
        bus_track: &Track,
        settings: &AudioEngineSettings,
    ) {
        context.with_activation(|| {
            disconnect_effect_chain(node_id_of(self.input), &self.effects);
            sync_effect_runtimes(&mut self.effects, bus_track.effects(), settings);
            let mut previous = node_id_of(self.input);
            for effect in &self.effects {
                if let Some(effect) = effect.as_ref() {
                    let node = effect.node_id();
                    connect_stereo(previous, node);
                    previous = node;
                }
            }
            connect_stereo(previous, node_id_of(self.strip));
        });
    }

    pub(super) fn sync_routing(
        &mut self,
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        mixer: &MixerState,
        targets: RoutingTargets<'_>,
        bus_track: &Track,
    ) -> Result<(), MixerRuntimeError> {
        context
            .with_activation(|| self.sync_routing_existing(commands, mixer, targets, bus_track))?;
        let Some(bus_id) = bus_track.bus_id else {
            return Ok(());
        };
        let strip_latency = self.latencies(bus_track, targets.pdc_plan.bus_input_latency(bus_id));
        self.rebuild_sends(
            context,
            commands,
            SendRouting {
                bus_inputs: targets.bus_inputs,
                pdc_plan: targets.pdc_plan,
                sends: &bus_track.routing.sends,
                pre_source: LatentNode {
                    node: node_id_of(self.input),
                    latency: strip_latency.pre_fader,
                },
                post_source: LatentNode {
                    node: node_id_of(self.strip),
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
        bus_track: &Track,
    ) -> Result<(), MixerRuntimeError> {
        let destination = destination_node(
            bus_track.routing.main,
            targets.master_input,
            targets.bus_inputs,
            mixer,
        )?;
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.route_bus)));
        free_optional_node(self.route_delay_node.take());
        let Some(bus_id) = bus_track.bus_id else {
            return Ok(());
        };
        let delay = targets.pdc_plan.route_delay(
            bus_track.routing.main,
            self.latencies(bus_track, targets.pdc_plan.bus_input_latency(bus_id))
                .output,
        );
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
        for effect in self.effects.into_iter().flatten() {
            free_effect(effect);
        }
        for send in self.sends {
            send.free();
        }
        free_optional_node(self.route_delay_node);
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
