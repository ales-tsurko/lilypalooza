//! Audio engine crate for Lilypalooza.
//!
//! This crate is the future home of the playback engine, fixed mixer,
//! instruments, and transport.

#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod engine;
pub mod instrument;
pub mod mixer;
pub mod sequencer;
pub mod transport;

#[cfg(test)]
mod test_utils;

pub use engine::{
    AudioEngine, AudioEngineError, AudioEngineOptions, AudioEngineSettings,
    EngineObservabilitySnapshot,
};
pub use instrument::{
    BUILTIN_GAIN_ID, BUILTIN_METRONOME_ID, BUILTIN_NONE_ID, BUILTIN_SOUNDFONT_ID, Controller,
    ControllerError, EditorDescriptor, EditorError, EditorParent, EditorSession, EditorSize,
    EffectProcessor, EffectRuntimeHandle, InstrumentProcessor, InstrumentRuntimeHandle, MidiEvent,
    ParameterDescriptor, Processor, ProcessorDescriptor, ProcessorKind, ProcessorState,
    ProcessorStateError, SlotState, SoundfontProcessorState, SoundfontResource,
};
pub use mixer::{
    BusId, BusSend, INSTRUMENT_TRACK_COUNT, Track, TrackId, TrackRoute, TrackRouting, TrackState,
};
pub use mixer::{MixerError, MixerHandle, MixerState};
pub use sequencer::{Sequencer, SequencerError, SequencerHandle};
pub use transport::{PlaybackState, Transport, TransportError, TransportSnapshot};
