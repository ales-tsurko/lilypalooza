//! Audio engine crate for Lilypalooza.
//!
//! This crate is the future home of the playback engine, fixed mixer,
//! instruments, and transport.

pub mod engine;
pub mod instrument;
pub mod mixer;
pub mod track;
pub mod transport;

pub use engine::AudioEngine;
pub use instrument::{InstrumentConfig, InstrumentKind};
pub use mixer::{MixerConfig, MixerTrackConfig};
pub use track::{TrackId, TrackState};
pub use transport::{PlaybackState, Transport};
