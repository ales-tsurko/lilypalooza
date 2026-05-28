use super::*;

pub(super) fn create_track_source_bus(context: &KnystContext) -> Handle<GenericHandle> {
    context.with_activation(|| bus(2))
}

pub(super) fn create_track_strip(
    commands: &mut MultiThreadedKnystCommands,
    level: SharedStripLevel,
    meter: SharedStripMeter,
    route_bus: Handle<GenericHandle>,
) -> Handle<GenericHandle> {
    let strip = handle_with_inputs(
        commands,
        StereoBalanceMeter::new(level, meter.clone(), meter.sample_rate()),
        inputs!(),
    );
    connect_stereo(node_id_of(strip), node_id_of(route_bus));
    strip
}

pub(super) fn create_track_instrument(
    context: &KnystContext,
    track: &Track,
    soundfont_resources: &[SoundfontResource],
    soundfonts: &HashMap<String, LoadedSoundfont>,
    soundfont_settings: SoundfontSynthSettings,
    inline_strip: Option<(SharedStripLevel, SharedStripMeter)>,
) -> Result<Option<TrackInstrumentRuntime>, MixerRuntimeError> {
    let Some(slot) = track.instrument_slot() else {
        return Ok(None);
    };
    let Some(spec) = build_instrument_runtime_spec(
        slot,
        &InstrumentRuntimeContext {
            soundfonts,
            soundfont_resources,
            soundfont_settings,
        },
    )?
    else {
        return Ok(None);
    };
    let crate::instrument::InstrumentRuntimeSpec { processor, binding } = spec;
    let processor_latency_samples = processor.latency_samples();
    let node = context.with_activation(|| {
        let reset_state = SharedInstrumentResetState::default();
        let node = if let Some((level, meter)) = inline_strip {
            handle(TrackInstrumentStripNode::new(
                processor,
                reset_state.clone(),
                level,
                meter,
                usize::try_from(soundfont_settings.sample_rate).unwrap_or(44_100),
            ))
        } else {
            handle(InstrumentProcessorNode::new(processor, reset_state.clone()))
        };
        InstrumentRuntimeHandle::new(node, reset_state)
    });
    Ok(Some(TrackInstrumentRuntime {
        handle: node,
        binding,
        processor_latency_samples,
    }))
}

pub(super) fn destination_node(
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

pub(super) fn connect_stereo(source: NodeId, destination: NodeId) {
    knyst_commands().connect(source.to(destination).from_index(0).to_index(0));
    knyst_commands().connect(source.to(destination).from_index(1).to_index(1));
}

pub(super) fn connect_stereo_with_delay(
    commands: &mut MultiThreadedKnystCommands,
    source: NodeId,
    destination: NodeId,
    delay_samples: u32,
) -> Option<NodeId> {
    if delay_samples == 0 {
        connect_stereo(source, destination);
        return None;
    }

    let delay = handle_with_inputs(
        commands,
        StereoDelay::new(delay_samples as usize),
        inputs!(),
    );
    let delay_node = node_id_of(delay);
    connect_stereo(source, delay_node);
    connect_stereo(delay_node, destination);
    Some(delay_node)
}

pub(super) fn free_optional_node(node: Option<NodeId>) {
    if let Some(node) = node {
        free_node(node);
    }
}

pub(super) fn free_node(node: NodeId) {
    knyst_commands().disconnect(Connection::clear_from_nodes(node));
    knyst_commands().disconnect(Connection::clear_to_nodes(node));
    knyst_commands().free_node(node);
}

pub(super) fn handle_with_inputs(
    commands: &mut MultiThreadedKnystCommands,
    processor: impl GenOrGraph,
    inputs: impl Into<InputBundle>,
) -> Handle<GenericHandle> {
    let num_inputs = processor.num_inputs();
    let num_outputs = processor.num_outputs();
    let node_id = commands.push(processor, inputs);
    Handle::new(GenericHandle::new(node_id, num_inputs, num_outputs))
}

pub(super) fn node_id_of<H: HandleData + Copy>(handle: Handle<H>) -> NodeId {
    handle
        .node_ids()
        .next()
        .unwrap_or_else(|| NodeId::new(u64::MAX))
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(super) struct MeterTap {
    meter: SharedStripMeter,
}

#[cfg(test)]
#[impl_gen]
impl MeterTap {
    #[new]
    pub(super) fn new(meter: SharedStripMeter) -> Self {
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

        for ((left, right), (left_out, right_out)) in left_in
            .iter()
            .copied()
            .zip(right_in.iter().copied())
            .zip(left_out.iter_mut().zip(right_out.iter_mut()))
            .take(block_size.0)
        {
            *left_out = left;
            *right_out = right;
            peak_left = peak_left.max(left.abs());
            peak_right = peak_right.max(right.abs());
        }

        self.meter.observe_stereo(peak_left, peak_right);
        GenState::Continue
    }
}
