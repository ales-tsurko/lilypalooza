use super::*;

pub(super) enum TrackSignalPath {
    Separated {
        source_bus: Handle<GenericHandle>,
        strip: Handle<GenericHandle>,
    },
    Combined,
}

impl TrackSignalPath {
    pub(super) fn free(self) {
        match self {
            Self::Separated { source_bus, strip } => {
                knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(strip)));
                knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(strip)));
                knyst_commands().disconnect(Connection::clear_from_nodes(node_id_of(source_bus)));
                knyst_commands().disconnect(Connection::clear_to_nodes(node_id_of(source_bus)));
                knyst_commands().free_node(node_id_of(strip));
                knyst_commands().free_node(node_id_of(source_bus));
            }
            Self::Combined => {}
        }
    }
}

pub(super) struct TrackInstrumentRuntime {
    pub(super) handle: InstrumentRuntimeHandle,
    pub(super) binding: Box<dyn RuntimeBinding>,
    pub(super) processor_latency_samples: u32,
}

impl TrackInstrumentRuntime {
    pub(super) fn update_in_place(&mut self, track: &Track) -> Result<bool, MixerRuntimeError> {
        let Some(slot) = track.instrument_slot() else {
            return Ok(false);
        };
        Ok(self.binding.update_in_place(slot)?)
    }

    pub(super) fn connect(&self, destination: NodeId) {
        connect_stereo(self.handle.node_id(), destination);
    }

    pub(super) fn free(self) {
        let node = self.handle.node_id();
        self.binding.prepare_destroy();
        knyst_commands().disconnect(Connection::clear_from_nodes(node));
        knyst_commands().disconnect(Connection::clear_to_nodes(node));
        knyst_commands().free_node(node);
    }

    pub(super) fn controller(&self) -> Box<dyn Controller> {
        self.binding.controller()
    }

    pub(super) fn latency_samples(&self) -> u32 {
        self.binding
            .latency_samples()
            .max(self.processor_latency_samples)
    }
}

pub(super) struct EffectRuntime {
    pub(super) slot: SlotState,
    pub(super) handle: EffectRuntimeHandle,
    pub(super) binding: Option<Box<dyn RuntimeBinding>>,
    pub(super) wet: SharedAudioValue,
    pub(super) processor_latency_samples: u32,
}

impl EffectRuntime {
    pub(super) fn can_reuse_for(&self, slot: &SlotState) -> bool {
        self.slot.instance_id == slot.instance_id
            && self.slot.kind == slot.kind
            && self.slot.state == slot.state
    }

    pub(super) fn node_id(&self) -> NodeId {
        self.handle.node_id()
    }

    pub(super) fn controller(&self) -> Option<Box<dyn Controller>> {
        self.binding.as_ref().map(|binding| binding.controller())
    }

    pub(super) fn latency_samples(&self) -> u32 {
        self.binding
            .as_ref()
            .map_or(self.processor_latency_samples, |binding| {
                binding
                    .latency_samples()
                    .max(self.processor_latency_samples)
            })
    }

    pub(super) fn sync_bypass(&self, bypassed: bool) {
        self.wet.set(if bypassed { 0.0 } else { 1.0 });
    }
}

pub(super) fn create_effect_runtime(
    effect: &SlotState,
    settings: &AudioEngineSettings,
) -> Option<EffectRuntime> {
    let context = EffectRuntimeContext {
        sample_rate: settings.sample_rate,
        block_size: settings.block_size,
    };
    let spec = build_effect_runtime_spec(effect, &context).ok()??;
    let processor_latency_samples = spec.processor.latency_samples();
    let wet = SharedAudioValue::new(if effect.bypassed { 0.0 } else { 1.0 });
    let node = handle(EffectProcessorNode::new(
        spec.processor,
        wet.clone(),
        settings.sample_rate,
    ));
    Some(EffectRuntime {
        slot: effect.clone(),
        handle: EffectRuntimeHandle::new(node),
        binding: spec.binding,
        wet,
        processor_latency_samples,
    })
}

pub(super) fn sync_effect_runtimes(
    effects: &mut Vec<Option<EffectRuntime>>,
    slots: &[SlotState],
    settings: &AudioEngineSettings,
) {
    let mut old_effects = std::mem::take(effects);
    for slot in slots {
        let reused = old_effects
            .iter()
            .position(|effect| {
                effect
                    .as_ref()
                    .is_some_and(|effect| effect.can_reuse_for(slot))
            })
            .and_then(|index| old_effects.get_mut(index).and_then(Option::take))
            .map(|mut effect| {
                effect.sync_bypass(slot.bypassed);
                effect.slot = slot.clone();
                effect
            });
        effects.push(reused.or_else(|| create_effect_runtime(slot, settings)));
    }
    for effect in old_effects.into_iter().flatten() {
        free_effect(effect);
    }
}

pub(super) fn disconnect_effect_chain(source: NodeId, effects: &[Option<EffectRuntime>]) {
    knyst_commands().disconnect(Connection::clear_to_nodes(source));
    for effect in effects.iter().flatten() {
        let node = effect.node_id();
        knyst_commands().disconnect(Connection::clear_from_nodes(node));
        knyst_commands().disconnect(Connection::clear_to_nodes(node));
    }
}

pub(super) fn free_effect(effect: EffectRuntime) {
    let node = effect.node_id();
    if let Some(binding) = effect.binding.as_ref() {
        binding.prepare_destroy();
    }
    knyst_commands().disconnect(Connection::clear_from_nodes(node));
    knyst_commands().disconnect(Connection::clear_to_nodes(node));
    knyst_commands().free_node(node);
}
