use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering},
};

use arc_swap::ArcSwap;
use lilypalooza_audio::{
    BUILTIN_SOUNDFONT_ID,
    instrument::{
        Controller, ControllerError, EditorDescriptor, EditorError, EditorParent, EditorSession,
        EditorSize, InstrumentProcessor, InstrumentRuntimeContext, InstrumentRuntimeSpec,
        MidiEvent, ParameterDescriptor, Processor, ProcessorDescriptor, ProcessorState,
        ProcessorStateError, RuntimeBinding, RuntimeFactoryError, SlotState, SmoothedAudioValue,
    },
    soundfont::{SoundfontPreset, SoundfontSynthSettings},
};
use lilypalooza_egui_baseview::{
    EguiApp, EguiWindowHandle, EguiWindowOptions, egui, open_parented,
};
use num_traits::ToPrimitive;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use serde::{Deserialize, Serialize};

pub(crate) mod model_and_runtime;
pub(crate) mod processor_and_editor;
mod retro_ui;

#[cfg(test)]
mod soundfont_tests;

pub(crate) use model_and_runtime::*;
pub use model_and_runtime::{
    MIDI_14BIT_MAX, SoundfontProcessorState, decode_state, encode_state, state,
};
pub(crate) use processor_and_editor::*;
