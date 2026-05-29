//! CLAP adapter and probe helpers for Lilypalooza.

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use clap_sys::{
    audio_buffer::clap_audio_buffer,
    events::{
        CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_IS_LIVE, CLAP_EVENT_MIDI, clap_event_header,
        clap_event_midi, clap_input_events, clap_output_events,
    },
    ext::{
        gui::{
            CLAP_EXT_GUI, CLAP_WINDOW_API_COCOA, CLAP_WINDOW_API_WAYLAND, CLAP_WINDOW_API_WIN32,
            CLAP_WINDOW_API_X11, clap_host_gui, clap_plugin_gui, clap_window, clap_window_handle,
        },
        latency::{CLAP_EXT_LATENCY, clap_host_latency, clap_plugin_latency},
        params::{CLAP_EXT_PARAMS, clap_host_params, clap_plugin_params},
        state::{CLAP_EXT_STATE, clap_host_state, clap_plugin_state},
    },
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    host::clap_host,
    plugin::{clap_plugin, clap_plugin_descriptor},
    plugin_features::{
        CLAP_PLUGIN_FEATURE_AUDIO_EFFECT, CLAP_PLUGIN_FEATURE_DRUM,
        CLAP_PLUGIN_FEATURE_DRUM_MACHINE, CLAP_PLUGIN_FEATURE_INSTRUMENT,
        CLAP_PLUGIN_FEATURE_SAMPLER, CLAP_PLUGIN_FEATURE_SYNTHESIZER,
    },
    process::{CLAP_PROCESS_ERROR, clap_process},
    stream::{clap_istream, clap_ostream},
    version::{CLAP_VERSION, clap_version_is_compatible},
};
use lilypalooza_audio::{
    ProcessorDescriptor, SlotState,
    instrument::{
        Controller, ControllerError, EditorDescriptor, EditorError, EditorParent,
        EditorResizeHandler, EditorSession, EditorSize, EffectProcessor, EffectRuntimeContext,
        EffectRuntimeSpec, InstrumentProcessor, InstrumentRuntimeContext, InstrumentRuntimeSpec,
        MidiEvent, Processor, ProcessorState, ProcessorStateError, RuntimeBinding,
        RuntimeFactoryError, registry,
    },
};
use raw_window_handle::RawWindowHandle;
use serde::{Deserialize, Serialize};

mod probe;
mod processor_editor;
mod runtime;

pub use probe::{
    ClapPluginMetadata, ClapProbeError, FORMAT, RuntimeKey, ValidationReport, candidate_paths,
    is_clap_candidate, probe, resolve_clap_library_path, stable_processor_id,
};
pub use processor_editor::register_plugins;

#[cfg(test)]
mod clap_tests;
