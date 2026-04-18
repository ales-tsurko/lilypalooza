//! Audio engine crate for Lilypalooza.
//!
//! This crate is the future home of the playback engine, fixed mixer,
//! instruments, and transport.

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
    EffectKind, EffectProcessor, EffectRuntimeHandle, EffectSlotState, InstrumentKind,
    InstrumentProcessor, InstrumentRuntimeHandle, InstrumentSlotState, MidiEvent, ParamValue,
    ParameterDescriptor, Processor, ProcessorDescriptor, ProcessorState, ProcessorStateError,
    SoundfontProcessorState, SoundfontResource,
};
pub use mixer::{
    BusId, BusSend, BusTrack, INSTRUMENT_TRACK_COUNT, MasterTrack, MixerTrack, TrackId, TrackRoute,
    TrackRouting, TrackState,
};
pub use mixer::{MixerError, MixerHandle, MixerState};
pub use sequencer::{Sequencer, SequencerError, SequencerHandle};
pub use transport::{PlaybackState, Transport, TransportError, TransportSnapshot};
