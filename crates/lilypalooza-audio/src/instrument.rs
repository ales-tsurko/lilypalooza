//! Instrument and effect processor abstractions.

pub(crate) mod metronome_synth;
/// Processor discovery and creation catalog.
pub mod registry;

mod definitions;
mod runtime_nodes;

pub use definitions::*;
pub use runtime_nodes::*;

#[cfg(test)]
mod instrument_tests;
