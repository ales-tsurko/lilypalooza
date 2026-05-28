use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
};

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    instrument::registry,
    soundfont::{LoadedSoundfont, SoundfontResource, SoundfontSynthSettings},
};

/// Built-in empty instrument id.
pub const BUILTIN_NONE_ID: &str = "org.lilypalooza.none";
/// Default smoothing time for host-owned audio controls.
pub const DEFAULT_CONTROL_SMOOTHING_MS: f32 = 10.0;
static NEXT_SLOT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

fn next_slot_instance_id() -> u64 {
    NEXT_SLOT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed)
}

fn reserve_slot_instance_id(instance_id: u64) {
    let next = instance_id.saturating_add(1);
    let mut current = NEXT_SLOT_INSTANCE_ID.load(Ordering::Relaxed);
    while current < next {
        match NEXT_SLOT_INSTANCE_ID.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

/// One-pole low-pass smoother for audio-rate parameter targets.
#[derive(Debug, Clone)]
pub struct SmoothedAudioValue {
    current: f32,
    target: f32,
    a: f32,
    b: f32,
}

impl SmoothedAudioValue {
    /// Creates a smoother initialized at `initial`.
    #[must_use]
    pub fn new(initial: f32, sample_rate: usize) -> Self {
        let samples = (DEFAULT_CONTROL_SMOOTHING_MS * 0.001 * sample_rate.max(1) as f32).max(1.0);
        let a = (-std::f32::consts::TAU / samples).exp();
        Self {
            current: initial,
            target: initial,
            a,
            b: 1.0 - a,
        }
    }

    /// Sets a new target value without jumping from the current value.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Returns the next smoothed sample value.
    pub fn next_sample(&mut self) -> f32 {
        self.current = self.target * self.b + self.current * self.a;
        self.current
    }

    /// Returns the current unsmoothed target.
    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }
}

/// Thread-safe audio parameter target read by processor nodes.
#[derive(Debug, Clone)]
pub struct SharedAudioValue {
    inner: Arc<AtomicU32>,
}

impl SharedAudioValue {
    /// Creates a shared target initialized to `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            inner: Arc::new(AtomicU32::new(value.to_bits())),
        }
    }

    /// Updates the target value.
    pub fn set(&self, value: f32) {
        self.inner.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Reads the current target value.
    #[must_use]
    pub fn get(&self) -> f32 {
        f32::from_bits(self.inner.load(Ordering::Relaxed))
    }
}

/// Built-in SoundFont instrument id.
pub const BUILTIN_SOUNDFONT_ID: &str = "org.lilypalooza.soundfont";
/// Built-in gain effect id.
pub const BUILTIN_GAIN_ID: &str = "org.lilypalooza.gain";
/// Built-in metronome instrument id.
pub const BUILTIN_METRONOME_ID: &str = "org.lilypalooza.metronome";

/// Opaque persisted processor state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessorState(pub Vec<u8>);

/// Static processor parameter description.
#[derive(Debug, Clone, Copy)]
pub struct ParameterDescriptor {
    /// Stable parameter identifier.
    pub id: &'static str,
    /// User-visible parameter name.
    pub name: &'static str,
    /// Default parameter value in normalized `[0, 1]`.
    pub default: f32,
}

/// Default editor size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorSize {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}

/// Static processor editor description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorDescriptor {
    /// Preferred initial editor size.
    pub default_size: EditorSize,
    /// Minimum editor size, when constrained.
    pub min_size: Option<EditorSize>,
    /// Whether the editor should be resizable.
    pub resizable: bool,
}

/// Native host window handles passed to processor editors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorParent {
    /// Parent content/window handle.
    pub window: RawWindowHandle,
    /// Parent display handle when required by the backend.
    pub display: Option<RawDisplayHandle>,
}

/// Processor editor session lifecycle errors.
#[derive(thiserror::Error, Debug)]
pub enum EditorError {
    /// Processor has no editor.
    #[error("processor has no editor")]
    Unsupported,
    /// Host could not provide a valid window.
    #[error("editor host is unavailable: {0}")]
    HostUnavailable(String),
    /// Backend-specific editor failure.
    #[error("editor backend failed: {0}")]
    Backend(String),
}

/// Live host-side editor resize callback.
pub trait EditorResizeHandler: Send + Sync {
    /// Resizes the containing editor frame and returns the accepted content size.
    fn resize_editor(&self, size: EditorSize) -> Result<EditorSize, EditorError>;
}

/// Live processor controller errors.
#[derive(thiserror::Error, Debug)]
pub enum ControllerError {
    /// Requested parameter id is unknown.
    #[error("unknown parameter `{0}`")]
    UnknownParameter(String),
    /// Backend-specific controller failure.
    #[error("controller backend failed: {0}")]
    Backend(String),
}

/// Live controller API for a running processor instance.
pub trait Controller: Send {
    /// Returns the static processor descriptor.
    fn descriptor(&self) -> &'static ProcessorDescriptor;
    /// Reads one parameter as normalized `[0, 1]`.
    fn get_param(&self, id: &str) -> Result<f32, ControllerError>;
    /// Sets one parameter from normalized `[0, 1]`.
    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError>;
    /// Saves the current processor state.
    fn save_state(&self) -> Result<ProcessorState, ControllerError>;
    /// Loads a full processor state.
    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError>;
    /// Notifies the start of an edit gesture.
    fn begin_edit(&self, _id: &str) -> Result<(), ControllerError> {
        Ok(())
    }
    /// Notifies the end of an edit gesture.
    fn end_edit(&self, _id: &str) -> Result<(), ControllerError> {
        Ok(())
    }
    /// Creates a live editor session for the processor, when supported.
    fn create_editor_session(&self) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        Ok(None)
    }
}

/// Live runtime binding that exposes a host controller.
pub trait RuntimeBinding: Send {
    /// Creates a controller for this runtime binding.
    fn controller(&self) -> Box<dyn Controller>;

    /// Returns the currently reported processing latency in samples.
    fn latency_samples(&self) -> u32 {
        0
    }

    /// Updates the runtime binding in place from a new slot state.
    fn update_in_place(&self, _slot: &SlotState) -> Result<bool, ProcessorStateError> {
        Ok(false)
    }

    /// Prepares the backend for destruction before its audio node is freed.
    fn prepare_destroy(&self) {}
}

/// Instrument runtime instance created by a processor factory.
pub struct InstrumentRuntimeSpec {
    /// Audio processor.
    pub processor: Box<dyn InstrumentProcessor>,
    /// Host controller binding.
    pub binding: Box<dyn RuntimeBinding>,
}

impl std::fmt::Debug for InstrumentRuntimeSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstrumentRuntimeSpec")
            .field("processor", &"<instrument processor>")
            .field("binding", &"<runtime binding>")
            .finish()
    }
}

/// Effect runtime instance created by a processor factory.
pub struct EffectRuntimeSpec {
    /// Audio processor.
    pub processor: Box<dyn EffectProcessor>,
    /// Optional host controller binding.
    pub binding: Option<Box<dyn RuntimeBinding>>,
}

impl std::fmt::Debug for EffectRuntimeSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EffectRuntimeSpec")
            .field("processor", &"<effect processor>")
            .field("has_binding", &self.binding.is_some())
            .finish()
    }
}

/// Runtime resources supplied to effect factories.
#[derive(Debug, Clone, Copy)]
pub struct EffectRuntimeContext {
    /// Active audio device sample rate.
    pub sample_rate: usize,
    /// Active audio device block size.
    pub block_size: usize,
}

/// Runtime resources supplied by the host to instrument factories.
pub struct InstrumentRuntimeContext<'a> {
    /// Loaded SoundFont resources keyed by id.
    pub soundfonts: &'a HashMap<String, LoadedSoundfont>,
    /// User-visible SoundFont resource metadata.
    pub soundfont_resources: &'a [SoundfontResource],
    /// SoundFont synthesizer runtime settings.
    pub soundfont_settings: SoundfontSynthSettings,
}

impl std::fmt::Debug for InstrumentRuntimeContext<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstrumentRuntimeContext")
            .field("soundfont_count", &self.soundfonts.len())
            .field("soundfont_resource_count", &self.soundfont_resources.len())
            .field("soundfont_settings", &self.soundfont_settings)
            .finish()
    }
}

#[derive(thiserror::Error, Debug)]
/// Processor runtime factory error.
pub enum RuntimeFactoryError {
    /// Persisted processor state is invalid.
    #[error(transparent)]
    State(#[from] ProcessorStateError),
    /// Backend-specific factory failure.
    #[error("{0}")]
    Backend(String),
}

/// Live processor editor session.
pub trait EditorSession {
    /// Returns whether the live editor can be resized, when the backend can report it.
    fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
        Ok(None)
    }
    /// Returns a backend-reported initial content size, when available.
    fn initial_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        Ok(None)
    }
    /// Returns and clears a backend-requested content resize, when available.
    fn requested_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        Ok(None)
    }
    /// Sets a live host resize callback used by plugin-owned resize requests.
    fn set_resize_handler(
        &mut self,
        _handler: Option<Arc<dyn EditorResizeHandler>>,
    ) -> Result<(), EditorError> {
        Ok(())
    }
    /// Returns whether the host should observe native embedded-view size changes.
    fn tracks_native_content_resize(&self) -> bool {
        true
    }
    /// Attaches the editor view to the host parent.
    fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError>;
    /// Detaches the editor view from the host parent.
    fn detach(&mut self) -> Result<(), EditorError>;
    /// Updates editor visibility.
    fn set_visible(&mut self, visible: bool) -> Result<(), EditorError>;
    /// Resizes the editor content area and returns the accepted content size.
    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError>;
}

/// Static processor description.
#[derive(Debug, Clone, Copy)]
pub struct ProcessorDescriptor {
    /// User-visible processor name.
    pub name: &'static str,
    /// Processor parameters.
    pub params: &'static [ParameterDescriptor],
    /// Optional processor editor support.
    pub editor: Option<EditorDescriptor>,
}

impl ProcessorDescriptor {
    /// Returns one parameter descriptor by id.
    #[must_use]
    pub fn param(&self, id: &str) -> Option<&'static ParameterDescriptor> {
        self.params.iter().find(|param| param.id == id)
    }
}

/// Full MIDI event stream passed to instruments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiEvent {
    /// Channel note-on.
    NoteOn {
        /// MIDI channel.
        channel: u8,
        /// MIDI note number.
        note: u8,
        /// MIDI velocity.
        velocity: u8,
    },
    /// Channel note-off.
    NoteOff {
        /// MIDI channel.
        channel: u8,
        /// MIDI note number.
        note: u8,
        /// MIDI release velocity.
        velocity: u8,
    },
    /// Channel control change.
    ControlChange {
        /// MIDI channel.
        channel: u8,
        /// Controller number.
        controller: u8,
        /// Controller value.
        value: u8,
    },
    /// Channel program change.
    ProgramChange {
        /// MIDI channel.
        channel: u8,
        /// Program number.
        program: u8,
    },
    /// Channel pressure.
    ChannelPressure {
        /// MIDI channel.
        channel: u8,
        /// Pressure value.
        pressure: u8,
    },
    /// Polyphonic key pressure.
    PolyPressure {
        /// MIDI channel.
        channel: u8,
        /// MIDI note number.
        note: u8,
        /// Pressure value.
        pressure: u8,
    },
    /// Pitch bend in raw MIDI 14-bit range centered at 0.
    PitchBend {
        /// MIDI channel.
        channel: u8,
        /// Raw signed pitch-bend value.
        value: i16,
    },
    /// All notes off.
    AllNotesOff {
        /// MIDI channel.
        channel: u8,
    },
    /// All sound off.
    AllSoundOff {
        /// MIDI channel.
        channel: u8,
    },
    /// Reset all controllers.
    ResetAllControllers {
        /// MIDI channel.
        channel: u8,
    },
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
    /// Sets one parameter from normalized `[0, 1]`.
    fn set_param(&mut self, id: &str, normalized: f32) -> bool;
    /// Reads one parameter as normalized `[0, 1]`.
    fn get_param(&self, id: &str) -> Option<f32>;
    /// Saves the full processor state.
    fn save_state(&self) -> ProcessorState;
    /// Loads the full processor state.
    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError>;
    /// Resets transient runtime state and silences active voices.
    fn reset(&mut self);
    /// Returns the currently reported processing latency in samples.
    fn latency_samples(&self) -> u32 {
        0
    }
    /// Creates a live editor session for the processor, when supported.
    fn create_editor_session(&self) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        Ok(None)
    }
}

/// Processor role for instruments.
pub trait InstrumentProcessor: Processor {
    /// Handles one MIDI event.
    fn handle_midi(&mut self, event: MidiEvent);
    /// Renders one stereo block.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
    /// Returns whether the processor can sleep until a new event or parameter change arrives.
    fn is_sleeping(&self) -> bool {
        false
    }
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

/// Supported processor backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessorKind {
    /// Built-in effect processor.
    BuiltIn {
        /// Engine-defined processor identifier.
        processor_id: String,
    },
    /// Hosted external plugin processor.
    Plugin {
        /// Engine-defined plugin instance identifier.
        plugin_id: String,
    },
}

/// Persisted processor slot state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlotState {
    /// Stable persisted identity for this slot instance.
    pub instance_id: u64,
    /// Stable user-facing instance number used when identical processors share a rack.
    pub instance_label_index: u32,
    /// Which backend this slot uses.
    pub kind: ProcessorKind,
    /// Opaque persisted processor state.
    pub state: ProcessorState,
    /// Whether the slot stays instantiated but is bypassed in the signal path.
    pub bypassed: bool,
}

impl<'de> Deserialize<'de> for SlotState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PersistedSlotState {
            instance_id: u64,
            instance_label_index: u32,
            kind: ProcessorKind,
            state: ProcessorState,
            bypassed: bool,
        }

        let slot = PersistedSlotState::deserialize(deserializer)?;
        reserve_slot_instance_id(slot.instance_id);
        Ok(Self {
            instance_id: slot.instance_id,
            instance_label_index: slot.instance_label_index,
            kind: slot.kind,
            state: slot.state,
            bypassed: slot.bypassed,
        })
    }
}

impl Default for SlotState {
    fn default() -> Self {
        Self {
            instance_id: next_slot_instance_id(),
            instance_label_index: 1,
            kind: ProcessorKind::BuiltIn {
                processor_id: BUILTIN_NONE_ID.to_string(),
            },
            state: ProcessorState::default(),
            bypassed: false,
        }
    }
}

impl SlotState {
    /// Creates one slot state from an explicit backend kind and opaque state.
    #[must_use]
    pub fn new(kind: ProcessorKind, state: ProcessorState) -> Self {
        Self {
            instance_id: next_slot_instance_id(),
            instance_label_index: 1,
            kind,
            state,
            bypassed: false,
        }
    }

    /// Creates one built-in slot state from a processor id and opaque state.
    #[must_use]
    pub fn built_in(processor_id: impl Into<String>, state: ProcessorState) -> Self {
        Self::new(
            ProcessorKind::BuiltIn {
                processor_id: processor_id.into(),
            },
            state,
        )
    }

    /// Decodes one built-in state payload when the slot matches the requested processor id.
    pub fn decode_built_in<T>(
        &self,
        processor_id: &str,
        decode: fn(&ProcessorState) -> Result<T, ProcessorStateError>,
    ) -> Result<Option<T>, ProcessorStateError> {
        let ProcessorKind::BuiltIn {
            processor_id: slot_id,
        } = &self.kind
        else {
            return Ok(None);
        };
        if slot_id != processor_id {
            return Ok(None);
        }
        decode(&self.state).map(Some)
    }

    /// Returns whether this slot contains no processor.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        registry::is_empty(&self.kind)
    }

    /// Returns the static processor descriptor for this slot, when known.
    #[must_use]
    pub fn descriptor(&self) -> Option<&'static ProcessorDescriptor> {
        registry::resolve(&self.kind).map(|entry| entry.descriptor)
    }

    /// Returns a display title for this slot.
    #[must_use]
    pub fn title(&self, strip_name: &str, slot_index: usize) -> String {
        if slot_index == 0 {
            format!("{strip_name} Instrument")
        } else {
            format!("{strip_name} Effect {slot_index}")
        }
    }
}
