use super::*;

pub(super) struct MasterRuntime {
    pub(super) input: Handle<GenericHandle>,
    pub(super) effects: Vec<Option<EffectRuntime>>,
    pub(super) strip: Handle<GenericHandle>,
    pub(super) meter: SharedStripMeter,
    pub(super) level: SharedStripLevel,
}

pub(super) struct MetronomeRuntime {
    pub(super) handle: InstrumentRuntimeHandle,
    pub(super) shared: SharedMetronomeState,
}

impl MetronomeRuntime {
    pub(super) fn new(
        context: &KnystContext,
        master_input: NodeId,
        settings: &AudioEngineSettings,
    ) -> Self {
        context.with_activation(|| {
            let reset_state = SharedInstrumentResetState::default();
            let shared = SharedMetronomeState::default();
            let processor = MetronomeProcessor::new(settings.sample_rate, shared.clone());
            let handle = handle(InstrumentProcessorNode::new(
                Box::new(processor),
                reset_state.clone(),
            ));
            connect_stereo(node_id_of(handle), master_input);
            Self {
                handle: InstrumentRuntimeHandle::new(handle, reset_state),
                shared,
            }
        })
    }

    pub(super) fn free(self) {
        knyst_commands().disconnect(Connection::clear_from_nodes(self.handle.node_id()));
        knyst_commands().disconnect(Connection::clear_to_nodes(self.handle.node_id()));
        knyst_commands().free_node(self.handle.node_id());
    }
}

impl MasterRuntime {
    pub(super) fn new(
        context: &KnystContext,
        commands: &mut MultiThreadedKnystCommands,
        settings: &AudioEngineSettings,
        mixer: &MixerState,
    ) -> Self {
        let meter = SharedStripMeter::new(settings.sample_rate, settings.block_size);
        let level = SharedStripLevel::new(
            db_to_amplitude(mixer.master().state.gain_db),
            mixer.master().state.pan,
        );
        let strip = handle_with_inputs(
            commands,
            StereoBalanceMeter::new(level.clone(), meter.clone(), settings.sample_rate),
            inputs!(),
        );
        let input = context.with_activation(|| {
            let input = bus(2);
            graph_output(0, strip.channels(2));
            input
        });
        let mut runtime = Self {
            input,
            effects: Vec::new(),
            strip,
            meter,
            level,
        };
        runtime.rebuild_effects(context, commands, mixer.master(), settings);
        runtime
    }

    pub(super) fn input_node(&self) -> NodeId {
        node_id_of(self.input)
    }

    pub(super) fn set_level(&mut self, gain: f32, pan: f32) {
        self.level.set(gain, pan);
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
    ) {
        context.with_activation(|| {
            disconnect_effect_chain(node_id_of(self.input), &self.effects);
            sync_effect_runtimes(&mut self.effects, track.effects(), settings);
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

    pub(super) fn free(self) {
        for effect in self.effects.into_iter().flatten() {
            free_effect(effect);
        }
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.strip)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.strip)));
        knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(self.input)));
        knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(self.input)));
        knyst_commands().free_node(node_id_of(self.strip));
        knyst_commands().free_node(node_id_of(self.input));
    }
}
