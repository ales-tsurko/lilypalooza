use std::collections::HashMap;

use knyst::{
    graph::{GenOrGraph, connection::InputBundle},
    inputs,
    modal_interface::{KnystContext, knyst_commands},
    prelude::{
        Connection, GenericHandle, Handle, HandleData, KnystCommands, MultiThreadedKnystCommands,
        NodeId, bus, graph_output, handle,
    },
};

use crate::{
    engine::AudioEngineSettings,
    instrument::{
        Controller, EffectProcessorNode, EffectRuntimeContext, EffectRuntimeHandle,
        InstrumentProcessorNode, InstrumentRuntimeContext, InstrumentRuntimeHandle,
        ProcessorStateError, RuntimeBinding, RuntimeFactoryError, SharedAudioValue,
        SharedInstrumentResetState, SlotState, create_effect_runtime as build_effect_runtime_spec,
        create_instrument_runtime as build_instrument_runtime_spec,
        metronome_synth::{MetronomeProcessor, SharedMetronomeState},
        registry,
    },
    mixer::{
        BusId, BusSend, MixerError, MixerMeterSnapshot, MixerMeterSnapshotWindow, MixerState,
        SlotAddress, StripMeterSnapshot, Track, TrackId, TrackRoute,
    },
    soundfont::{LoadedSoundfont, SoundfontResource, SoundfontSynthError, SoundfontSynthSettings},
};

#[derive(thiserror::Error, Debug)]
pub(crate) enum MixerRuntimeError {
    #[error(transparent)]
    Mixer(#[from] MixerError),
    #[error(transparent)]
    Soundfont(#[from] SoundfontSynthError),
    #[error(transparent)]
    ProcessorState(#[from] ProcessorStateError),
    #[error(transparent)]
    RuntimeFactory(#[from] RuntimeFactoryError),
}

pub(crate) enum TrackInstrumentSync {
    GraphChanged,
    UpdatedInPlace,
}

mod meter;
mod pdc;
mod strip_nodes;

#[cfg(test)]
use knyst::prelude::{BlockSize, GenState, Sample, impl_gen};
#[cfg(test)]
use meter::normalize_meter_level;
use meter::{SharedStripLevel, SharedStripMeter};
use pdc::{PdcPlan, StripLatency, compute_pdc_plan_from_latencies};
#[cfg(test)]
use strip_nodes::StereoBalanceGain;
use strip_nodes::{StereoBalanceMeter, StereoDelay, StereoGain, TrackInstrumentStripNode};
pub(crate) use strip_nodes::{
    process_stereo_balance_meter_scalar, process_stereo_balance_meter_simd,
};

mod bus_runtime;
mod graph_helpers;
mod master_runtime;
mod mix_helpers;
mod mixer_runtime;
mod track_runtime;
mod track_signal_path;

use bus_runtime::*;
use graph_helpers::*;
use master_runtime::*;
use mix_helpers::*;
pub(crate) use mixer_runtime::*;
use track_runtime::*;
use track_signal_path::*;

#[cfg(test)]
mod runtime_tests;
