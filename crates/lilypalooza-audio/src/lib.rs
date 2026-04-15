//! Audio engine crate for Lilypalooza.
//!
//! This crate is the future home of the playback engine, fixed mixer,
//! instruments, and transport.

pub mod engine;
pub mod instrument;
pub mod mixer;
pub mod transport;

pub use engine::{AudioEngine, AudioEngineError, AudioEngineOptions};
pub use instrument::{InstrumentConfig, InstrumentKind};
pub use mixer::{
    BusId, BusSend, BusTrack, INSTRUMENT_TRACK_COUNT, MasterTrack, MixerTrack, TrackId, TrackRoute,
    TrackRouting, TrackState,
};
pub use mixer::{MixerError, MixerHandle, MixerState};
pub use transport::{PlaybackState, Transport, TransportError, TransportSnapshot};
