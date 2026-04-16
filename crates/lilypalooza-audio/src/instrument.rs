//! Instrument and effect processor abstractions.

mod gain_effect;
pub(crate) mod soundfont_synth;

use std::time::Duration;

use knyst::graph::{SimultaneousChanges, TimeOffset};
use knyst::handles::{GenericHandle, Handle, HandleData};
use knyst::prelude::{
    BlockSize, GenState, KnystCommands, MultiThreadedKnystCommands, Sample, impl_gen,
};
use knyst::trig::is_trigger;
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
    processor: Box<dyn InstrumentProcessor>,
    scratch_left: Vec<Sample>,
    scratch_right: Vec<Sample>,
}

#[impl_gen]
impl InstrumentProcessorNode {
    #[new]
    pub(crate) fn new(processor: Box<dyn InstrumentProcessor>) -> Self {
        Self {
            active_generation: 0,
            processor,
            scratch_left: Vec::new(),
            scratch_right: Vec::new(),
        }
    }

    #[process]
    #[allow(clippy::too_many_arguments)]
    fn process(
        &mut self,
        generation: &[Sample],
        activate_generation: &[Sample],
        channel: &[Sample],
        note: &[Sample],
        velocity: &[Sample],
        controller: &[Sample],
        value: &[Sample],
        note_on: &[Sample],
        note_off: &[Sample],
        control_change: &[Sample],
        program: &[Sample],
        program_change: &[Sample],
        pressure: &[Sample],
        channel_pressure: &[Sample],
        poly_pressure: &[Sample],
        pitch_bend: &[Sample],
        pitch_bend_set: &[Sample],
        all_notes_off: &[Sample],
        all_sound_off: &[Sample],
        reset_all_controllers: &[Sample],
        reset: &[Sample],
        left: &mut [Sample],
        right: &mut [Sample],
        block_size: BlockSize,
    ) -> GenState {
        let frames = block_size.0;
        for frame in 0..frames {
            let generation = generation[frame].max(0.0) as u32;
            if is_trigger(activate_generation[frame]) {
                self.active_generation = generation;
            }
            if is_trigger(reset[frame]) {
                self.processor.reset();
            }

            let channel = channel[frame].clamp(0.0, 15.0) as u8;
            let note = note[frame].clamp(0.0, 127.0) as u8;
            let velocity = velocity[frame].clamp(0.0, 127.0) as u8;
            let controller = controller[frame].clamp(0.0, 127.0) as u8;
            let value = value[frame].clamp(0.0, 127.0) as u8;
            let program = program[frame].clamp(0.0, 127.0) as u8;
            let pressure = pressure[frame].clamp(0.0, 127.0) as u8;
            let pitch_bend = pitch_bend[frame]
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            let generation_matches = generation == self.active_generation;

            if generation_matches && is_trigger(note_on[frame]) {
                self.processor.handle_midi(MidiEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                });
            }
            if generation_matches && is_trigger(note_off[frame]) {
                self.processor.handle_midi(MidiEvent::NoteOff {
                    channel,
                    note,
                    velocity,
                });
            }
            if generation_matches && is_trigger(control_change[frame]) {
                self.processor.handle_midi(MidiEvent::ControlChange {
                    channel,
                    controller,
                    value,
                });
            }
            if generation_matches && is_trigger(program_change[frame]) {
                self.processor
                    .handle_midi(MidiEvent::ProgramChange { channel, program });
            }
            if generation_matches && is_trigger(channel_pressure[frame]) {
                self.processor
                    .handle_midi(MidiEvent::ChannelPressure { channel, pressure });
            }
            if generation_matches && is_trigger(poly_pressure[frame]) {
                self.processor.handle_midi(MidiEvent::PolyPressure {
                    channel,
                    note,
                    pressure,
                });
            }
            if generation_matches && is_trigger(pitch_bend_set[frame]) {
                self.processor.handle_midi(MidiEvent::PitchBend {
                    channel,
                    value: pitch_bend,
                });
            }
            if generation_matches && is_trigger(all_notes_off[frame]) {
                self.processor
                    .handle_midi(MidiEvent::AllNotesOff { channel });
            }
            if generation_matches && is_trigger(all_sound_off[frame]) {
                self.processor
                    .handle_midi(MidiEvent::AllSoundOff { channel });
            }
            if generation_matches && is_trigger(reset_all_controllers[frame]) {
                self.processor
                    .handle_midi(MidiEvent::ResetAllControllers { channel });
            }
        }

        self.scratch_left.resize(frames, 0.0);
        self.scratch_right.resize(frames, 0.0);
        self.processor.render(
            &mut self.scratch_left[..frames],
            &mut self.scratch_right[..frames],
        );
        left[..frames].copy_from_slice(&self.scratch_left[..frames]);
        right[..frames].copy_from_slice(&self.scratch_right[..frames]);
        GenState::Continue
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
#[derive(Clone, Copy, Debug)]
pub struct InstrumentRuntimeHandle(Handle<GenericHandle>);

impl InstrumentRuntimeHandle {
    pub(crate) fn new(handle: Handle<GenericHandle>) -> Self {
        Self(handle)
    }

    #[cfg(test)]
    pub(crate) fn raw_handle(self) -> Handle<GenericHandle> {
        self.0
    }

    pub(crate) fn node_id(self) -> knyst::prelude::NodeId {
        self.0
            .node_ids()
            .next()
            .expect("instrument handle should always own one node")
    }

    fn schedule_midi_inner(
        self,
        commands: &mut MultiThreadedKnystCommands,
        mut changes: SimultaneousChanges,
        generation: Option<u32>,
        frame_offset: i32,
        event: MidiEvent,
    ) {
        let mut parameter_changes = self.node_id().change();
        if let Some(generation) = generation {
            parameter_changes = parameter_changes.set("generation", generation as f32);
        }
        let trigger = match event {
            MidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("note", f32::from(note))
                    .set("velocity", f32::from(velocity));
                "note_on"
            }
            MidiEvent::NoteOff {
                channel,
                note,
                velocity,
            } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("note", f32::from(note))
                    .set("velocity", f32::from(velocity));
                "note_off"
            }
            MidiEvent::ControlChange {
                channel,
                controller,
                value,
            } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("controller", f32::from(controller))
                    .set("value", f32::from(value));
                "control_change"
            }
            MidiEvent::ProgramChange { channel, program } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("program", f32::from(program));
                "program_change"
            }
            MidiEvent::ChannelPressure { channel, pressure } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("pressure", f32::from(pressure));
                "channel_pressure"
            }
            MidiEvent::PolyPressure {
                channel,
                note,
                pressure,
            } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("note", f32::from(note))
                    .set("pressure", f32::from(pressure));
                "poly_pressure"
            }
            MidiEvent::PitchBend { channel, value } => {
                parameter_changes = parameter_changes
                    .set("channel", f32::from(channel))
                    .set("pitch_bend", f32::from(value));
                "pitch_bend_set"
            }
            MidiEvent::AllNotesOff { channel } => {
                parameter_changes = parameter_changes.set("channel", f32::from(channel));
                "all_notes_off"
            }
            MidiEvent::AllSoundOff { channel } => {
                parameter_changes = parameter_changes.set("channel", f32::from(channel));
                "all_sound_off"
            }
            MidiEvent::ResetAllControllers { channel } => {
                parameter_changes = parameter_changes.set("channel", f32::from(channel));
                "reset_all_controllers"
            }
        };

        let parameter_changes = if frame_offset == 0 {
            parameter_changes
        } else {
            parameter_changes.time_offset(TimeOffset::Frames(i64::from(frame_offset)))
        };

        changes.push(parameter_changes);
        changes.push(
            self.node_id()
                .change()
                .trigger(trigger)
                .time_offset(TimeOffset::Frames(i64::from(frame_offset + 1))),
        );
        commands.schedule_changes(changes);
    }

    pub(crate) fn schedule_midi_at_with_offset(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        at: knyst::prelude::Beats,
        generation: u32,
        frame_offset: i32,
        event: MidiEvent,
    ) {
        self.schedule_midi_inner(
            commands,
            SimultaneousChanges::beats(at),
            Some(generation),
            frame_offset,
            event,
        );
    }

    pub(crate) fn send_midi_now_with_offset(
        &self,
        commands: &mut MultiThreadedKnystCommands,
        generation: u32,
        frame_offset: i32,
        event: MidiEvent,
    ) {
        self.schedule_midi_inner(
            commands,
            SimultaneousChanges::duration_from_now(Duration::ZERO),
            Some(generation),
            frame_offset,
            event,
        );
    }

    #[cfg(test)]
    pub(crate) fn send_midi(&self, commands: &mut MultiThreadedKnystCommands, event: MidiEvent) {
        self.schedule_midi_inner(
            commands,
            SimultaneousChanges::duration_from_now(Duration::ZERO),
            None,
            0,
            event,
        );
    }

    pub(crate) fn send_reset(&self, commands: &mut MultiThreadedKnystCommands, generation: u32) {
        let mut changes = SimultaneousChanges::now();
        changes.push(
            self.node_id()
                .change()
                .set("generation", generation as f32)
                .trigger("activate_generation"),
        );
        changes.push(self.node_id().change().trigger("reset"));
        commands.schedule_changes(changes);
    }

    /// Sends one note-on.
    pub fn note_on(&self, channel: u8, note: u8, velocity: u8) {
        self.0
            .set("channel", f32::from(channel))
            .set("note", f32::from(note))
            .set("velocity", f32::from(velocity))
            .trig("note_on");
    }

    /// Sends one note-off.
    pub fn note_off(&self, channel: u8, note: u8, velocity: u8) {
        self.0
            .set("channel", f32::from(channel))
            .set("note", f32::from(note))
            .set("velocity", f32::from(velocity))
            .trig("note_off");
    }

    /// Sends one generic MIDI event.
    pub fn midi(&self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => self.note_on(channel, note, velocity),
            MidiEvent::NoteOff {
                channel,
                note,
                velocity,
            } => self.note_off(channel, note, velocity),
            MidiEvent::ControlChange {
                channel,
                controller,
                value,
            } => {
                self.0
                    .set("channel", f32::from(channel))
                    .set("controller", f32::from(controller))
                    .set("value", f32::from(value))
                    .trig("control_change");
            }
            MidiEvent::ProgramChange { channel, program } => {
                self.0
                    .set("channel", f32::from(channel))
                    .set("program", f32::from(program))
                    .trig("program_change");
            }
            MidiEvent::ChannelPressure { channel, pressure } => {
                self.0
                    .set("channel", f32::from(channel))
                    .set("pressure", f32::from(pressure))
                    .trig("channel_pressure");
            }
            MidiEvent::PolyPressure {
                channel,
                note,
                pressure,
            } => {
                self.0
                    .set("channel", f32::from(channel))
                    .set("note", f32::from(note))
                    .set("pressure", f32::from(pressure))
                    .trig("poly_pressure");
            }
            MidiEvent::PitchBend { channel, value } => {
                self.0
                    .set("channel", f32::from(channel))
                    .set("pitch_bend", f32::from(value))
                    .trig("pitch_bend_set");
            }
            MidiEvent::AllNotesOff { channel } => {
                self.0
                    .set("channel", f32::from(channel))
                    .trig("all_notes_off");
            }
            MidiEvent::AllSoundOff { channel } => {
                self.0
                    .set("channel", f32::from(channel))
                    .trig("all_sound_off");
            }
            MidiEvent::ResetAllControllers { channel } => {
                self.0
                    .set("channel", f32::from(channel))
                    .trig("reset_all_controllers");
            }
        }
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
