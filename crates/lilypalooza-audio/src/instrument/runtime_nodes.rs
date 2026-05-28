use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use knyst::{
    r#gen::{Gen, GenContext},
    graph::{EventChange, EventPayload, ResolvedNodeEventInput, SchedulerChange},
    handles::{GenericHandle, Handle, HandleData},
    modal_interface::knyst_commands,
    prelude::{
        BlockSize,
        GenState,
        KnystCommands,
        MultiThreadedKnystCommands,
        Resources,
        Sample,
        impl_gen,
    },
};

use super::definitions::*;
use crate::instrument::registry;

/// Knyst node wrapper for any instrument processor.
pub(crate) struct InstrumentProcessorNode {
    active_generation: u32,
    reset_state: SharedInstrumentResetState,
    processor: Box<dyn InstrumentProcessor>,
    scratch_left: Vec<Sample>,
    scratch_right: Vec<Sample>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SharedInstrumentResetState {
    generation: Arc<AtomicU32>,
}

impl SharedInstrumentResetState {
    pub(crate) fn request(&self, generation: u32) {
        self.generation.store(generation, Ordering::Relaxed);
    }

    pub(crate) fn load(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }
}

impl InstrumentProcessorNode {
    pub(crate) fn new(
        processor: Box<dyn InstrumentProcessor>,
        reset_state: SharedInstrumentResetState,
    ) -> Self {
        Self {
            active_generation: 0,
            reset_state,
            processor,
            scratch_left: Vec::new(),
            scratch_right: Vec::new(),
        }
    }
}

impl Gen for InstrumentProcessorNode {
    fn process(&mut self, ctx: GenContext<'_, '_, '_>, _resources: &mut Resources) -> GenState {
        let frames = ctx.outputs.block_size();
        self.scratch_left.resize(frames, 0.0);
        self.scratch_right.resize(frames, 0.0);

        let requested_reset = self.reset_state.load();
        if requested_reset != self.active_generation
            && generation_is_current_or_newer(requested_reset, self.active_generation)
        {
            self.active_generation = requested_reset;
            self.processor.reset();
        }

        for event in ctx.events {
            if event.input != 0 {
                continue;
            }
            let EventPayload::Bytes(bytes) = &event.payload else {
                continue;
            };
            let Some(event) = decode_instrument_event(bytes) else {
                continue;
            };
            match event {
                ScheduledInstrumentEvent::Reset { generation } => {
                    if generation_is_current_or_newer(generation, self.active_generation) {
                        self.active_generation = generation;
                        self.processor.reset();
                    }
                }
                ScheduledInstrumentEvent::Midi { generation, event } => {
                    if generation == self.active_generation {
                        self.processor.handle_midi(event);
                    }
                }
            }
        }
        let Some(scratch_left) = self.scratch_left.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(scratch_right) = self.scratch_right.get_mut(..frames) else {
            return GenState::Continue;
        };
        self.processor.render(scratch_left, scratch_right);
        let mut outputs = ctx.outputs.iter_mut();
        let Some(left_out) = outputs.next() else {
            return GenState::Continue;
        };
        let Some(right_out) = outputs.next() else {
            return GenState::Continue;
        };
        let Some(left_out) = left_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(right_out) = right_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        left_out.copy_from_slice(scratch_left);
        right_out.copy_from_slice(scratch_right);
        if self.processor.is_sleeping() {
            GenState::Sleep
        } else {
            GenState::Continue
        }
    }

    fn num_inputs(&self) -> usize {
        0
    }

    fn num_outputs(&self) -> usize {
        2
    }

    fn num_event_inputs(&self) -> usize {
        1
    }

    fn event_input_desc(&self, input: usize) -> &'static str {
        match input {
            0 => "event",
            _ => "",
        }
    }

    fn name(&self) -> &'static str {
        "InstrumentProcessorNode"
    }
}

/// Knyst node wrapper for any effect processor.
pub(crate) struct EffectProcessorNode {
    processor: Box<dyn EffectProcessor>,
    wet_target: SharedAudioValue,
    wet: SmoothedAudioValue,
    scratch_left: Vec<Sample>,
    scratch_right: Vec<Sample>,
}

#[impl_gen]
impl EffectProcessorNode {
    #[new]
    pub(crate) fn new(
        processor: Box<dyn EffectProcessor>,
        wet_target: SharedAudioValue,
        sample_rate: usize,
    ) -> Self {
        let wet = SmoothedAudioValue::new(wet_target.get(), sample_rate);
        Self {
            processor,
            wet_target,
            wet,
            scratch_left: Vec::new(),
            scratch_right: Vec::new(),
        }
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
        let frames = block_size.0;
        self.scratch_left.resize(frames, 0.0);
        self.scratch_right.resize(frames, 0.0);
        let Some(left_in) = left_in.get(..frames) else {
            return GenState::Continue;
        };
        let Some(right_in) = right_in.get(..frames) else {
            return GenState::Continue;
        };
        let Some(left_out) = left_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(right_out) = right_out.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(scratch_left) = self.scratch_left.get_mut(..frames) else {
            return GenState::Continue;
        };
        let Some(scratch_right) = self.scratch_right.get_mut(..frames) else {
            return GenState::Continue;
        };
        self.processor
            .process(left_in, right_in, scratch_left, scratch_right);
        self.wet.set_target(self.wet_target.get());
        for (((left_in, right_in), (scratch_left, scratch_right)), (left_out, right_out)) in left_in
            .iter()
            .zip(right_in)
            .zip(scratch_left.iter().zip(scratch_right))
            .zip(left_out.iter_mut().zip(right_out))
        {
            let wet = self.wet.next_sample();
            let dry = 1.0 - wet;
            *left_out = *left_in * dry + *scratch_left * wet;
            *right_out = *right_in * dry + *scratch_right * wet;
        }
        GenState::Continue
    }
}

/// Typed Knyst handle for instrument runtime nodes.
#[derive(Clone, Debug)]
pub struct InstrumentRuntimeHandle {
    handle: Handle<GenericHandle>,
    reset_state: SharedInstrumentResetState,
    scheduler_event_target: Option<ResolvedNodeEventInput>,
}

impl InstrumentRuntimeHandle {
    #[cfg(test)]
    const IMMEDIATE_EVENT_LEAD: Duration = Duration::from_millis(30);
    #[cfg(test)]
    const IMMEDIATE_EVENT_STEP: Duration = Duration::from_millis(2);
    const LIVE_EVENT_DELAY: Duration = Duration::from_millis(2);
    const SCHEDULER_TARGET_TIMEOUT: Duration = Duration::from_millis(250);

    pub(crate) fn new(
        handle: Handle<GenericHandle>,
        reset_state: SharedInstrumentResetState,
    ) -> Self {
        Self {
            handle,
            reset_state,
            scheduler_event_target: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn raw_handle(&self) -> Handle<GenericHandle> {
        self.handle
    }

    pub(crate) fn node_id(&self) -> knyst::prelude::NodeId {
        self.handle
            .node_ids()
            .next()
            .unwrap_or_else(|| knyst::prelude::NodeId::new(u64::MAX))
    }

    pub(crate) fn request_reset_now(&self, generation: u32) {
        self.reset_state.request(generation);
    }

    pub(crate) fn resolve_scheduler_event_target(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
    ) {
        let node_id = self.node_id();
        self.scheduler_event_target = None;
        let deadline = Instant::now() + Self::SCHEDULER_TARGET_TIMEOUT;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let timeout = remaining.min(Duration::from_millis(10));
            if let Ok(Some(target)) = commands
                .resolve_scheduler_event_input(node_id.event_input("event"))
                .recv_timeout(timeout)
            {
                self.scheduler_event_target = Some(target);
                return;
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    pub(crate) fn scheduler_midi_change(
        &self,
        sample_offset: usize,
        generation: u32,
        event: MidiEvent,
    ) -> Option<SchedulerChange> {
        Some(SchedulerChange::Event {
            target: self.scheduler_event_target?,
            sample_offset,
            payload: encode_instrument_event(ScheduledInstrumentEvent::Midi { generation, event }),
        })
    }

    pub(crate) fn scheduler_reset_change(
        &self,
        sample_offset: usize,
        generation: u32,
    ) -> Option<SchedulerChange> {
        Some(SchedulerChange::Event {
            target: self.scheduler_event_target?,
            sample_offset,
            payload: encode_instrument_event(ScheduledInstrumentEvent::Reset { generation }),
        })
    }

    pub(crate) fn schedule_midi_at_with_offset(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        scheduled_at: knyst::prelude::Beats,
        generation: u32,
        event: MidiEvent,
    ) {
        let node_id = self.node_id();
        let change = EventChange::beats(
            node_id.event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Midi { generation, event }),
            scheduled_at,
        );
        commands.schedule_event(change);
    }

    pub(crate) fn schedule_reset_at(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        scheduled_at: knyst::prelude::Beats,
        generation: u32,
    ) {
        let node_id = self.node_id();
        commands.schedule_event(EventChange::beats(
            node_id.event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Reset { generation }),
            scheduled_at,
        ));
    }

    #[cfg(test)]
    pub(crate) fn send_midi_immediate(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        generation: u32,
        event: MidiEvent,
        delay: Duration,
    ) {
        let node_id = self.node_id();
        commands.schedule_event(EventChange::duration_from_now(
            node_id.event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Midi { generation, event }),
            delay,
        ));
    }

    #[cfg(test)]
    pub(crate) fn send_midi(&self, commands: &mut MultiThreadedKnystCommands, event: MidiEvent) {
        self.send_midi_immediate(commands, 0, event, Self::IMMEDIATE_EVENT_LEAD);
    }

    #[cfg(test)]
    pub(crate) fn send_reset(&self, commands: &mut MultiThreadedKnystCommands, generation: u32) {
        let node_id = self.node_id();
        commands.schedule_event(EventChange::duration_from_now(
            node_id.event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Reset { generation }),
            Self::IMMEDIATE_EVENT_LEAD,
        ));
    }

    #[cfg(test)]
    pub(crate) fn immediate_event_delay(step: u32) -> Duration {
        Self::IMMEDIATE_EVENT_LEAD + Self::IMMEDIATE_EVENT_STEP.saturating_mul(step)
    }

    fn send_live_midi(&self, event: MidiEvent) {
        let node_id = self.node_id();
        knyst_commands().schedule_event(EventChange::duration_from_now(
            node_id.event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Midi {
                generation: 0,
                event,
            }),
            Self::LIVE_EVENT_DELAY,
        ));
    }

    /// Sends one note-on.
    pub fn note_on(&self, channel: u8, note: u8, velocity: u8) {
        self.send_live_midi(MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        });
    }

    /// Sends one note-off.
    pub fn note_off(&self, channel: u8, note: u8, velocity: u8) {
        self.send_live_midi(MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        });
    }

    /// Sends one generic MIDI event.
    pub fn midi(&self, event: MidiEvent) {
        self.send_live_midi(event);
    }
}

/// Typed Knyst handle for effect runtime nodes.
#[derive(Clone, Copy, Debug)]
pub struct EffectRuntimeHandle(Handle<GenericHandle>);

impl EffectRuntimeHandle {
    pub(crate) fn new(handle: Handle<GenericHandle>) -> Self {
        Self(handle)
    }

    pub(crate) fn node_id(self) -> knyst::prelude::NodeId {
        self.0
            .node_ids()
            .next()
            .unwrap_or_else(|| knyst::prelude::NodeId::new(u64::MAX))
    }
}

pub(crate) fn create_instrument_runtime(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    registry::create_instrument_runtime(slot, context)
}

pub(crate) fn create_effect_runtime(
    effect: &SlotState,
    context: &EffectRuntimeContext,
) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError> {
    registry::create_effect_runtime(effect, context)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledInstrumentEvent {
    Midi { generation: u32, event: MidiEvent },
    Reset { generation: u32 },
}

fn encode_instrument_event(event: ScheduledInstrumentEvent) -> EventPayload {
    let bytes = match event {
        ScheduledInstrumentEvent::Midi { generation, event } => {
            let [g0, g1, g2, g3] = generation.to_le_bytes();
            let [m0, m1, m2, m3, m4] = encode_midi_event(event);
            [0, g0, g1, g2, g3, m0, m1, m2, m3, m4]
        }
        ScheduledInstrumentEvent::Reset { generation } => {
            let [g0, g1, g2, g3] = generation.to_le_bytes();
            [1, g0, g1, g2, g3, 0, 0, 0, 0, 0]
        }
    };
    EventPayload::Bytes(Box::new(bytes))
}

pub(crate) fn decode_instrument_event(bytes: &[u8]) -> Option<ScheduledInstrumentEvent> {
    let bytes: &[u8; 10] = bytes.try_into().ok()?;
    let [kind, g0, g1, g2, g3, m0, m1, m2, m3, m4] = *bytes;
    let generation = u32::from_le_bytes([g0, g1, g2, g3]);
    match kind {
        0 => Some(ScheduledInstrumentEvent::Midi {
            generation,
            event: decode_midi_event([m0, m1, m2, m3, m4])?,
        }),
        1 => Some(ScheduledInstrumentEvent::Reset { generation }),
        _ => None,
    }
}

pub(crate) fn generation_is_current_or_newer(candidate: u32, current: u32) -> bool {
    let delta = candidate.wrapping_sub(current);
    delta == 0 || delta < (u32::MAX / 2)
}

fn encode_midi_event(event: MidiEvent) -> [u8; 5] {
    match event {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => [0, channel, note, velocity, 0],
        MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        } => [1, channel, note, velocity, 0],
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => [2, channel, controller, value, 0],
        MidiEvent::ProgramChange { channel, program } => [3, channel, program, 0, 0],
        MidiEvent::ChannelPressure { channel, pressure } => [4, channel, pressure, 0, 0],
        MidiEvent::PolyPressure {
            channel,
            note,
            pressure,
        } => [5, channel, note, pressure, 0],
        MidiEvent::PitchBend { channel, value } => {
            let [lo, hi] = value.to_le_bytes();
            [6, channel, lo, hi, 0]
        }
        MidiEvent::AllNotesOff { channel } => [7, channel, 0, 0, 0],
        MidiEvent::AllSoundOff { channel } => [8, channel, 0, 0, 0],
        MidiEvent::ResetAllControllers { channel } => [9, channel, 0, 0, 0],
    }
}

fn decode_midi_event(bytes: [u8; 5]) -> Option<MidiEvent> {
    let [kind, a, b, c, _] = bytes;
    match kind {
        0 | 1 => decode_note_midi_event(kind, a, b, c),
        2..=6 => decode_channel_midi_event(kind, a, b, c),
        7..=9 => decode_channel_command_midi_event(kind, a),
        _ => None,
    }
}

fn decode_note_midi_event(kind: u8, channel: u8, note: u8, velocity: u8) -> Option<MidiEvent> {
    match kind {
        0 => Some(MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        }),
        1 => Some(MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        }),
        _ => None,
    }
}

fn decode_channel_midi_event(kind: u8, channel: u8, b: u8, c: u8) -> Option<MidiEvent> {
    match kind {
        2 | 5 | 6 => decode_two_value_channel_event(kind, channel, b, c),
        3 | 4 => decode_single_value_channel_event(kind, channel, b),
        _ => None,
    }
}

fn decode_single_value_channel_event(kind: u8, channel: u8, value: u8) -> Option<MidiEvent> {
    match kind {
        3 => Some(MidiEvent::ProgramChange {
            channel,
            program: value,
        }),
        4 => Some(MidiEvent::ChannelPressure {
            channel,
            pressure: value,
        }),
        _ => None,
    }
}

fn decode_two_value_channel_event(kind: u8, channel: u8, b: u8, c: u8) -> Option<MidiEvent> {
    match kind {
        2 => Some(MidiEvent::ControlChange {
            channel,
            controller: b,
            value: c,
        }),
        5 => Some(MidiEvent::PolyPressure {
            channel,
            note: b,
            pressure: c,
        }),
        6 => Some(MidiEvent::PitchBend {
            channel,
            value: i16::from_le_bytes([b, c]),
        }),
        _ => None,
    }
}

fn decode_channel_command_midi_event(kind: u8, channel: u8) -> Option<MidiEvent> {
    match kind {
        7 => Some(MidiEvent::AllNotesOff { channel }),
        8 => Some(MidiEvent::AllSoundOff { channel }),
        9 => Some(MidiEvent::ResetAllControllers { channel }),
        _ => None,
    }
}
