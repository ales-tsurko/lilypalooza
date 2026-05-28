//! VST3 plugin adapter.

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    slice,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use lilypalooza_audio::instrument::{
    Controller,
    ControllerError,
    EditorError,
    EditorParent,
    EditorResizeHandler,
    EditorSession,
    EditorSize,
    EffectProcessor,
    EffectRuntimeContext,
    EffectRuntimeSpec,
    InstrumentProcessor,
    InstrumentRuntimeContext,
    InstrumentRuntimeSpec,
    MidiEvent,
    Processor,
    ProcessorDescriptor,
    ProcessorState,
    ProcessorStateError,
    RuntimeBinding,
    RuntimeFactoryError,
    SlotState,
    registry,
};
use raw_window_handle::{RawWindowHandle, XlibWindowHandle};
use serde::{Deserialize, Serialize};
use vst3::{
    Class,
    ComPtr,
    ComWrapper,
    Steinberg::{Vst::*, *},
};

mod editor;
mod host_com;
mod probe;
mod runtime;

pub use editor::register_plugins;
pub use probe::{
    FORMAT,
    ValidationReport,
    Vst3PluginMetadata,
    Vst3ProbeError,
    candidate_paths,
    is_vst3_candidate,
    probe,
    resolve_vst3_library_path,
    stable_processor_id,
};

#[cfg(test)]
mod tests;
