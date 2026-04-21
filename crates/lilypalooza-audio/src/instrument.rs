//! Instrument and effect processor abstractions.

mod gain_effect;
pub(crate) mod metronome_synth;
pub(crate) mod soundfont_synth;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use knyst::r#gen::{Gen, GenContext};
use knyst::graph::{EventChange, EventPayload, ResolvedNodeEventInput, SchedulerChange};
use knyst::handles::{GenericHandle, Handle, HandleData};
use knyst::modal_interface::knyst_commands;
use knyst::prelude::{
    BlockSize, GenState, KnystCommands, MultiThreadedKnystCommands, Resources, Sample, impl_gen,
};
use serde::{Deserialize, Serialize};
pub use soundfont_synth::{SoundfontProcessorState, SoundfontResource};

/// Opaque persisted processor state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessorState(pub Vec<u8>);

/// Shared processor parameter value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// Boolean parameter.
    Bool(bool),
    /// Integer parameter.
    Int(i64),
    /// Floating-point parameter.
    Float(f32),
    /// Enumerated parameter stored as index.
    Enum(u32),
    /// Text parameter.
    Text(String),
}

/// Static processor parameter description.
#[derive(Debug, Clone, Copy)]
pub struct ParameterDescriptor {
    /// Stable parameter identifier.
    pub id: &'static str,
    /// User-visible parameter name.
    pub name: &'static str,
}

/// Static processor description.
#[derive(Debug, Clone, Copy)]
pub struct ProcessorDescriptor {
    /// User-visible processor name.
    pub name: &'static str,
    /// Processor parameters.
    pub params: &'static [ParameterDescriptor],
}

/// Full MIDI event stream passed to instruments.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiEvent {
    /// Channel note-on.
    NoteOn { channel: u8, note: u8, velocity: u8 },
    /// Channel note-off.
    NoteOff { channel: u8, note: u8, velocity: u8 },
    /// Channel control change.
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// Channel program change.
    ProgramChange { channel: u8, program: u8 },
    /// Channel pressure.
    ChannelPressure { channel: u8, pressure: u8 },
    /// Polyphonic key pressure.
    PolyPressure { channel: u8, note: u8, pressure: u8 },
    /// Pitch bend in raw MIDI 14-bit range centered at 0.
    PitchBend { channel: u8, value: i16 },
    /// All notes off.
    AllNotesOff { channel: u8 },
    /// All sound off.
    AllSoundOff { channel: u8 },
    /// Reset all controllers.
    ResetAllControllers { channel: u8 },
}

/// Processor state errors.
#[derive(thiserror::Error, Debug)]
pub enum ProcessorStateError {
    /// Failed to deserialize processor state.
    #[error("failed to decode processor state: {0}")]
    Decode(String),
}

/// Shared processor API for instruments and effects.
pub trait Processor: Send {
    /// Returns the static processor descriptor.
    fn descriptor(&self) -> &'static ProcessorDescriptor;
    /// Sets one parameter.
    fn set_param(&mut self, id: &str, value: ParamValue);
    /// Saves the full processor state.
    fn save_state(&self) -> ProcessorState;
    /// Loads the full processor state.
    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError>;
    /// Resets transient runtime state.
    fn reset(&mut self);
}

/// Processor role for instruments.
pub trait InstrumentProcessor: Processor {
    /// Handles one MIDI event.
    fn handle_midi(&mut self, event: MidiEvent);
    /// Renders one stereo block.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
    /// Returns whether the processor can sleep until a new event or parameter change arrives.
    fn is_sleeping(&self) -> bool {
        false
    }
}

/// Processor role for effects.
pub trait EffectProcessor: Processor {
    /// Processes one stereo block.
    fn process(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    );
}

/// Supported instrument backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentKind {
    /// Built-in sampler or synth instrument.
    BuiltIn {
        /// Engine-defined instrument identifier.
        instrument_id: String,
    },
    /// Hosted external plugin instrument.
    Plugin {
        /// Engine-defined plugin instance identifier.
        plugin_id: String,
    },
}

/// Persisted instrument slot state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSlotState {
    /// Which instrument backend this track uses.
    pub kind: InstrumentKind,
    /// Opaque persisted processor state.
    pub state: ProcessorState,
}

impl Default for InstrumentSlotState {
    fn default() -> Self {
        Self {
            kind: InstrumentKind::BuiltIn {
                instrument_id: "none".to_string(),
            },
            state: ProcessorState::default(),
        }
    }
}

impl InstrumentSlotState {
    /// Creates an empty instrument slot state.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a built-in SoundFont instrument slot state.
    #[must_use]
    pub fn soundfont(soundfont_id: impl Into<String>, bank: u16, program: u8) -> Self {
        Self {
            kind: InstrumentKind::BuiltIn {
                instrument_id: "soundfont".to_string(),
            },
            state: soundfont_synth::encode_soundfont_state(&SoundfontProcessorState {
                soundfont_id: soundfont_id.into(),
                bank,
                program,
            }),
        }
    }

    /// Returns whether this slot contains no instrument.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(
            self.kind,
            InstrumentKind::BuiltIn {
                ref instrument_id
            } if instrument_id == "none"
        )
    }

    /// Decodes the typed SoundFont state when this slot contains a SoundFont instrument.
    pub fn soundfont_state(&self) -> Result<Option<SoundfontProcessorState>, ProcessorStateError> {
        match self.kind {
            InstrumentKind::BuiltIn { ref instrument_id } if instrument_id == "soundfont" => {
                soundfont_synth::SoundfontProcessor::decode_state(&self.state).map(Some)
            }
            _ => Ok(None),
        }
    }
}

/// Supported effect backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    /// Built-in effect processor.
    BuiltIn {
        /// Engine-defined effect identifier.
        effect_id: String,
    },
    /// Hosted external plugin effect.
    Plugin {
        /// Engine-defined plugin instance identifier.
        plugin_id: String,
    },
}

/// Persisted effect slot state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectSlotState {
    /// Which effect backend this slot uses.
    pub kind: EffectKind,
    /// Opaque persisted processor state.
    pub state: ProcessorState,
}

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
        self.processor.render(
            &mut self.scratch_left[..frames],
            &mut self.scratch_right[..frames],
        );
        let mut outputs = ctx.outputs.iter_mut();
        let left_out = outputs
            .next()
            .expect("instrument node must expose left output");
        let right_out = outputs
            .next()
            .expect("instrument node must expose right output");
        left_out[..frames].copy_from_slice(&self.scratch_left[..frames]);
        right_out[..frames].copy_from_slice(&self.scratch_right[..frames]);
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
    scratch_left: Vec<Sample>,
    scratch_right: Vec<Sample>,
}

#[impl_gen]
impl EffectProcessorNode {
    #[new]
    pub(crate) fn new(processor: Box<dyn EffectProcessor>) -> Self {
        Self {
            processor,
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
        self.processor.process(
            &left_in[..frames],
            &right_in[..frames],
            &mut self.scratch_left[..frames],
            &mut self.scratch_right[..frames],
        );
        left_out[..frames].copy_from_slice(&self.scratch_left[..frames]);
        right_out[..frames].copy_from_slice(&self.scratch_right[..frames]);
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
            .expect("instrument handle should always own one node")
    }

    pub(crate) fn request_reset_now(&self, generation: u32) {
        self.reset_state.request(generation);
    }

    pub(crate) fn resolve_scheduler_event_target(
        &mut self,
        commands: &mut MultiThreadedKnystCommands,
    ) {
        let _ = commands
            .request_graph_settled()
            .recv_timeout(Self::SCHEDULER_TARGET_TIMEOUT);
        self.scheduler_event_target = commands
            .resolve_scheduler_event_input(self.node_id().event_input("event"))
            .recv_timeout(Self::SCHEDULER_TARGET_TIMEOUT)
            .ok()
            .flatten();
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
        let change = EventChange::beats(
            self.node_id().event_input("event"),
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
        commands.schedule_event(EventChange::beats(
            self.node_id().event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Reset { generation }),
            scheduled_at,
        ));
    }

    pub(crate) fn send_midi_immediate(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        generation: u32,
        event: MidiEvent,
        delay: Duration,
    ) {
        commands.schedule_event(EventChange::duration_from_now(
            self.node_id().event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Midi { generation, event }),
            delay,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn send_midi(&self, commands: &mut MultiThreadedKnystCommands, event: MidiEvent) {
        self.send_midi_immediate(commands, 0, event, Self::IMMEDIATE_EVENT_LEAD);
    }

    #[allow(dead_code)]
    pub(crate) fn send_reset(&self, commands: &mut MultiThreadedKnystCommands, generation: u32) {
        commands.schedule_event(EventChange::duration_from_now(
            self.node_id().event_input("event"),
            encode_instrument_event(ScheduledInstrumentEvent::Reset { generation }),
            Self::IMMEDIATE_EVENT_LEAD,
        ));
    }

    #[cfg(test)]
    pub(crate) fn immediate_event_delay(step: u32) -> Duration {
        Self::IMMEDIATE_EVENT_LEAD + Self::IMMEDIATE_EVENT_STEP.saturating_mul(step)
    }

    fn send_live_midi(&self, event: MidiEvent) {
        knyst_commands().schedule_event(EventChange::duration_from_now(
            self.node_id().event_input("event"),
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
            .expect("effect handle should always own one node")
    }
}

pub(crate) fn create_effect_processor(
    effect: &EffectSlotState,
) -> Result<Option<Box<dyn EffectProcessor>>, ProcessorStateError> {
    match &effect.kind {
        EffectKind::BuiltIn { effect_id } if effect_id == "gain" => Ok(Some(Box::new(
            gain_effect::GainEffectProcessor::from_state(&effect.state)?,
        ))),
        EffectKind::BuiltIn { .. } | EffectKind::Plugin { .. } => Ok(None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledInstrumentEvent {
    Midi { generation: u32, event: MidiEvent },
    Reset { generation: u32 },
}

fn encode_instrument_event(event: ScheduledInstrumentEvent) -> EventPayload {
    let mut bytes = [0_u8; 10];
    match event {
        ScheduledInstrumentEvent::Midi { generation, event } => {
            bytes[0] = 0;
            bytes[1..5].copy_from_slice(&generation.to_le_bytes());
            encode_midi_event(event, &mut bytes[5..10]);
        }
        ScheduledInstrumentEvent::Reset { generation } => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&generation.to_le_bytes());
        }
    }
    EventPayload::Bytes(Box::new(bytes))
}

pub(crate) fn decode_instrument_event(bytes: &[u8]) -> Option<ScheduledInstrumentEvent> {
    if bytes.len() != 10 {
        return None;
    }
    let generation = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
    match bytes[0] {
        0 => Some(ScheduledInstrumentEvent::Midi {
            generation,
            event: decode_midi_event(&bytes[5..10])?,
        }),
        1 => Some(ScheduledInstrumentEvent::Reset { generation }),
        _ => None,
    }
}

pub(crate) fn generation_is_current_or_newer(candidate: u32, current: u32) -> bool {
    let delta = candidate.wrapping_sub(current);
    delta == 0 || delta < (u32::MAX / 2)
}

fn encode_midi_event(event: MidiEvent, bytes: &mut [u8]) {
    match event {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => {
            bytes.copy_from_slice(&[0, channel, note, velocity, 0]);
        }
        MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        } => {
            bytes.copy_from_slice(&[1, channel, note, velocity, 0]);
        }
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => {
            bytes.copy_from_slice(&[2, channel, controller, value, 0]);
        }
        MidiEvent::ProgramChange { channel, program } => {
            bytes.copy_from_slice(&[3, channel, program, 0, 0]);
        }
        MidiEvent::ChannelPressure { channel, pressure } => {
            bytes.copy_from_slice(&[4, channel, pressure, 0, 0]);
        }
        MidiEvent::PolyPressure {
            channel,
            note,
            pressure,
        } => {
            bytes.copy_from_slice(&[5, channel, note, pressure, 0]);
        }
        MidiEvent::PitchBend { channel, value } => {
            let [lo, hi] = value.to_le_bytes();
            bytes.copy_from_slice(&[6, channel, lo, hi, 0]);
        }
        MidiEvent::AllNotesOff { channel } => {
            bytes.copy_from_slice(&[7, channel, 0, 0, 0]);
        }
        MidiEvent::AllSoundOff { channel } => {
            bytes.copy_from_slice(&[8, channel, 0, 0, 0]);
        }
        MidiEvent::ResetAllControllers { channel } => {
            bytes.copy_from_slice(&[9, channel, 0, 0, 0]);
        }
    }
}

fn decode_midi_event(bytes: &[u8]) -> Option<MidiEvent> {
    if bytes.len() != 5 {
        return None;
    }
    Some(match bytes[0] {
        0 => MidiEvent::NoteOn {
            channel: bytes[1],
            note: bytes[2],
            velocity: bytes[3],
        },
        1 => MidiEvent::NoteOff {
            channel: bytes[1],
            note: bytes[2],
            velocity: bytes[3],
        },
        2 => MidiEvent::ControlChange {
            channel: bytes[1],
            controller: bytes[2],
            value: bytes[3],
        },
        3 => MidiEvent::ProgramChange {
            channel: bytes[1],
            program: bytes[2],
        },
        4 => MidiEvent::ChannelPressure {
            channel: bytes[1],
            pressure: bytes[2],
        },
        5 => MidiEvent::PolyPressure {
            channel: bytes[1],
            note: bytes[2],
            pressure: bytes[3],
        },
        6 => MidiEvent::PitchBend {
            channel: bytes[1],
            value: i16::from_le_bytes([bytes[2], bytes[3]]),
        },
        7 => MidiEvent::AllNotesOff { channel: bytes[1] },
        8 => MidiEvent::AllSoundOff { channel: bytes[1] },
        9 => MidiEvent::ResetAllControllers { channel: bytes[1] },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use knyst::controller::KnystCommands;
    use knyst::prelude::{Beats, graph_output, handle};

    use super::{
        InstrumentProcessor, InstrumentProcessorNode, InstrumentRuntimeHandle, MidiEvent,
        ParamValue, Processor, ProcessorDescriptor, ProcessorState, ProcessorStateError,
        SharedInstrumentResetState,
        soundfont_synth::{
            LoadedSoundfont, SoundfontProcessor, SoundfontProcessorState, SoundfontSynthSettings,
        },
    };
    use crate::test_utils::{OfflineHarness, test_soundfont_resource};

    struct GateProcessor {
        active: bool,
    }

    impl Processor for GateProcessor {
        fn descriptor(&self) -> &'static ProcessorDescriptor {
            static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
                name: "Gate",
                params: &[],
            };
            &DESCRIPTOR
        }

        fn set_param(&mut self, _id: &str, _value: ParamValue) {}

        fn save_state(&self) -> ProcessorState {
            ProcessorState::default()
        }

        fn load_state(&mut self, _state: &ProcessorState) -> Result<(), ProcessorStateError> {
            Ok(())
        }

        fn reset(&mut self) {
            self.active = false;
        }
    }

    impl InstrumentProcessor for GateProcessor {
        fn handle_midi(&mut self, event: MidiEvent) {
            match event {
                MidiEvent::NoteOn { .. } => self.active = true,
                MidiEvent::NoteOff { .. } | MidiEvent::AllNotesOff { .. } => self.active = false,
                _ => {}
            }
        }

        fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
            let value = if self.active { 1.0 } else { 0.0 };
            left.fill(value);
            right.fill(value);
        }

        fn is_sleeping(&self) -> bool {
            !self.active
        }
    }

    #[test]
    fn reset_then_note_on_in_same_block_produces_signal() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let handle = harness.context().with_activation(|| {
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(GateProcessor { active: true }),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_pause();
        harness
            .commands()
            .transport_seek_to_beats(Beats::from_beats(1));
        thread::sleep(Duration::from_millis(10));

        handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.01), 1);
        handle.schedule_midi_at_with_offset(
            harness.commands(),
            Beats::from_beats_f64(1.02),
            1,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );
        harness.commands().transport_play();

        for _ in 0..512 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("reset then note-on in same block should produce signal");
    }

    #[test]
    fn immediate_reset_and_panic_silence_active_node() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let handle = harness.context().with_activation(|| {
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(GateProcessor { active: false }),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_play();
        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..256 {
            harness.process_block();
            if harness.output_has_signal() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(harness.output_has_signal(), "note on should produce signal");

        handle.send_reset(harness.commands(), 1);
        handle.send_midi_immediate(
            harness.commands(),
            1,
            MidiEvent::AllSoundOff { channel: 0 },
            InstrumentRuntimeHandle::immediate_event_delay(1),
        );

        for _ in 0..256 {
            harness.process_block();
            if !harness.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("immediate reset and panic should silence active node");
    }

    #[test]
    fn immediate_reset_and_panic_silence_active_soundfont_node() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let handle = harness.context().with_activation(|| {
            let loaded = LoadedSoundfont::load(&test_soundfont_resource())
                .expect("test soundfont should load");
            let processor = SoundfontProcessor::new(
                &loaded.soundfont,
                SoundfontSynthSettings::new(44_100, 64),
                SoundfontProcessorState::default(),
            )
            .expect("soundfont processor should initialize");
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(processor),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_play();
        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..256 {
            harness.process_block();
            if harness.output_has_signal() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(harness.output_has_signal(), "note on should produce signal");

        handle.send_reset(harness.commands(), 1);
        handle.send_midi_immediate(
            harness.commands(),
            1,
            MidiEvent::AllSoundOff { channel: 0 },
            InstrumentRuntimeHandle::immediate_event_delay(1),
        );

        for _ in 0..256 {
            harness.process_block();
            if !harness.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("immediate reset and panic should silence active soundfont node");
    }

    #[test]
    fn scheduled_midi_reaches_late_added_soundfont_node() {
        let mut harness = OfflineHarness::new(44_100, 64);
        harness.process_blocks(8);

        let handle = harness.context().with_activation(|| {
            let loaded = LoadedSoundfont::load(&test_soundfont_resource())
                .expect("test soundfont should load");
            let processor = SoundfontProcessor::new(
                &loaded.soundfont,
                SoundfontSynthSettings::new(44_100, 64),
                SoundfontProcessorState::default(),
            )
            .expect("soundfont processor should initialize");
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(processor),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_play();
        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..256 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("scheduled MIDI did not reach late-added soundfont node");
    }

    #[test]
    fn direct_midi_reaches_soundfont_node_added_after_transport_reset() {
        let mut harness = OfflineHarness::new(44_100, 64);
        harness.commands().transport_pause();
        harness
            .commands()
            .transport_seek_to_beats(Beats::from_beats_f64(0.0));
        harness.process_blocks(8);

        let handle = harness.context().with_activation(|| {
            let loaded = LoadedSoundfont::load(&test_soundfont_resource())
                .expect("test soundfont should load");
            let processor = SoundfontProcessor::new(
                &loaded.soundfont,
                SoundfontSynthSettings::new(44_100, 64),
                SoundfontProcessorState::default(),
            )
            .expect("soundfont processor should initialize");
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(processor),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_play();
        harness.process_blocks(8);
        handle.send_midi(
            harness.commands(),
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );

        for _ in 0..256 {
            harness.process_block();
            if harness.output_has_signal() {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }

        panic!("direct MIDI did not reach soundfont node added after transport reset");
    }

    #[test]
    fn stale_reset_generation_does_not_silence_newer_note() {
        let mut harness = OfflineHarness::new(44_100, 64);
        let handle = harness.context().with_activation(|| {
            let reset_state = SharedInstrumentResetState::default();
            let node = handle(InstrumentProcessorNode::new(
                Box::new(GateProcessor { active: false }),
                reset_state.clone(),
            ));
            graph_output(0, node.channels(2));
            InstrumentRuntimeHandle::new(node, reset_state)
        });

        harness.commands().transport_pause();
        harness
            .commands()
            .transport_seek_to_beats(Beats::from_beats(1));
        thread::sleep(Duration::from_millis(10));

        handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.01), 1);
        handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.02), 2);
        handle.schedule_midi_at_with_offset(
            harness.commands(),
            Beats::from_beats_f64(1.03),
            2,
            MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
            },
        );
        handle.schedule_reset_at(harness.commands(), Beats::from_beats_f64(1.04), 1);
        harness.commands().transport_play();

        let mut heard_signal = false;
        let mut signal_after_stale_reset = false;
        for _ in 0..512 {
            harness.process_block();
            let has_signal = harness.output_has_signal();
            if has_signal {
                heard_signal = true;
            }
            let beat = harness
                .commands()
                .current_transport_snapshot()
                .and_then(|snapshot| snapshot.beats)
                .unwrap_or(Beats::ZERO);
            if beat >= Beats::from_beats_f64(1.045) && has_signal {
                signal_after_stale_reset = true;
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }

        assert!(heard_signal, "newer generation note should produce signal");
        assert!(
            signal_after_stale_reset,
            "stale reset from an older generation must not silence newer playback"
        );
    }

    #[test]
    fn generation_ordering_rejects_stale_reset_after_increment() {
        assert!(super::generation_is_current_or_newer(2, 1));
        assert!(super::generation_is_current_or_newer(2, 2));
        assert!(!super::generation_is_current_or_newer(1, 2));
    }
}
