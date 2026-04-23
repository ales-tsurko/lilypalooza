use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use arc_swap::ArcSwap;
use raw_window_handle::RawWindowHandle;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use serde::{Deserialize, Serialize};
use vizia::prelude::*;
use vizia::{
    Application as ViziaApplication, ParentWindow, WindowHandle as ViziaWindowHandle,
    WindowScalePolicy,
};

use crate::instrument::{
    BUILTIN_SOUNDFONT_ID, Controller, ControllerError, EditorDescriptor, EditorError, EditorParent,
    EditorSession, EditorSize, InstrumentProcessor, InstrumentRuntimeContext,
    InstrumentRuntimeSpec, MidiEvent, ParameterDescriptor, Processor, ProcessorDescriptor,
    ProcessorState, ProcessorStateError, RuntimeBinding, RuntimeFactoryError, SlotState,
};

pub(crate) const MIDI_14BIT_MAX: u16 = 16_383;
const MIDI_PROGRAM_MAX: u8 = 127;
const DEFAULT_MAXIMUM_POLYPHONY: u16 = 64;
const MINIMUM_POLYPHONY: u16 = 8;
const MAXIMUM_POLYPHONY: u16 = 256;
const DEFAULT_MASTER_VOLUME: f32 = 0.5;

/// Shared SoundFont resource configured in the mixer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundfontResource {
    /// Stable SoundFont identifier.
    pub id: String,
    /// User-visible SoundFont name.
    pub name: String,
    /// Absolute path to the `.sf2` file.
    pub path: PathBuf,
}

/// Persisted state for the built-in SoundFont instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundfontProcessorState {
    /// Shared SoundFont resource identifier.
    pub soundfont_id: String,
    /// MIDI bank.
    pub bank: u16,
    /// MIDI program.
    pub program: u8,
    /// Whether MIDI bank/program messages should override the selected preset.
    #[serde(default)]
    pub follow_midi: bool,
    /// Maximum voice count for the synthesizer instance.
    #[serde(default = "default_maximum_polyphony")]
    pub maximum_polyphony: u16,
    /// Linear master output volume.
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
}

impl Default for SoundfontProcessorState {
    fn default() -> Self {
        Self {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 0,
            follow_midi: false,
            maximum_polyphony: DEFAULT_MAXIMUM_POLYPHONY,
            master_volume: DEFAULT_MASTER_VOLUME,
        }
    }
}

const fn default_maximum_polyphony() -> u16 {
    DEFAULT_MAXIMUM_POLYPHONY
}

const fn default_master_volume() -> f32 {
    DEFAULT_MASTER_VOLUME
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum SoundfontSynthError {
    #[error("failed to read soundfont `{path}`: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse soundfont `{path}`: {source}")]
    ParseFile {
        path: PathBuf,
        #[source]
        source: rustysynth::SoundFontError,
    },
    #[error("failed to create synthesizer for soundfont `{id}`: {source}")]
    CreateSynth {
        id: String,
        #[source]
        source: rustysynth::SynthesizerError,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SoundfontSynthSettings {
    pub sample_rate: i32,
    pub block_size: usize,
}

impl SoundfontSynthSettings {
    #[must_use]
    pub fn new(sample_rate: i32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
        }
    }
}

#[derive(Debug)]
pub(crate) struct LoadedSoundfont {
    pub(crate) path: PathBuf,
    pub(crate) soundfont: Arc<SoundFont>,
    pub(crate) presets: Arc<Vec<SoundfontPreset>>,
}

impl LoadedSoundfont {
    pub(crate) fn load(resource: &SoundfontResource) -> Result<Self, SoundfontSynthError> {
        let file = fs::read(&resource.path).map_err(|source| SoundfontSynthError::ReadFile {
            path: resource.path.clone(),
            source,
        })?;
        let soundfont = SoundFont::new(&mut file.as_slice()).map_err(|source| {
            SoundfontSynthError::ParseFile {
                path: resource.path.clone(),
                source,
            }
        })?;
        let presets = soundfont_presets(&soundfont);
        Ok(Self {
            path: resource.path.clone(),
            soundfont: Arc::new(soundfont),
            presets: Arc::new(presets),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SoundfontPreset {
    bank: u16,
    program: u8,
    name: String,
}

#[derive(Debug, Clone)]
struct SoundfontCatalogEntry {
    id: String,
    name: String,
    presets: Arc<Vec<SoundfontPreset>>,
}

#[derive(Debug)]
pub(crate) struct SoundfontProcessor {
    settings: SoundfontSynthSettings,
    synthesizer: Synthesizer,
    state: SoundfontProcessorState,
    shared_state: Option<SharedSoundfontState>,
    applied_shared_revision: u32,
    needs_render: bool,
    silent_blocks: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct SharedSoundfontState {
    inner: Arc<SharedSoundfontStateInner>,
}

#[derive(Debug)]
struct SharedSoundfontStateInner {
    soundfont: ArcSwap<SoundFont>,
    soundfont_id: ArcSwap<String>,
    bank: AtomicU16,
    program: AtomicU32,
    follow_midi: AtomicBool,
    maximum_polyphony: AtomicU32,
    master_volume_bits: AtomicU32,
    revision: AtomicU32,
}

impl SharedSoundfontState {
    pub(crate) fn new(state: &SoundfontProcessorState, soundfont: Arc<SoundFont>) -> Self {
        Self {
            inner: Arc::new(SharedSoundfontStateInner {
                soundfont: ArcSwap::from(soundfont),
                soundfont_id: ArcSwap::from_pointee(state.soundfont_id.clone()),
                bank: AtomicU16::new(state.bank),
                program: AtomicU32::new(u32::from(state.program)),
                follow_midi: AtomicBool::new(state.follow_midi),
                maximum_polyphony: AtomicU32::new(u32::from(state.maximum_polyphony)),
                master_volume_bits: AtomicU32::new(state.master_volume.to_bits()),
                revision: AtomicU32::new(1),
            }),
        }
    }

    pub(crate) fn update(&self, state: &SoundfontProcessorState, soundfont: Arc<SoundFont>) {
        self.inner.soundfont.store(soundfont);
        self.inner
            .soundfont_id
            .store(Arc::new(state.soundfont_id.clone()));
        self.inner.bank.store(state.bank, Ordering::Relaxed);
        self.inner
            .program
            .store(u32::from(state.program), Ordering::Relaxed);
        self.inner
            .follow_midi
            .store(state.follow_midi, Ordering::Relaxed);
        self.inner
            .maximum_polyphony
            .store(u32::from(state.maximum_polyphony), Ordering::Relaxed);
        self.inner
            .master_volume_bits
            .store(state.master_volume.to_bits(), Ordering::Relaxed);
        self.inner.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn update_bank_program(&self, bank: u16, program: u8) {
        self.inner.bank.store(bank, Ordering::Relaxed);
        self.inner
            .program
            .store(u32::from(program), Ordering::Relaxed);
        self.inner.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> (SoundfontProcessorState, u32) {
        (
            SoundfontProcessorState {
                soundfont_id: self.inner.soundfont_id.load().as_ref().clone(),
                bank: self.inner.bank.load(Ordering::Relaxed),
                program: self.inner.program.load(Ordering::Relaxed) as u8,
                follow_midi: self.inner.follow_midi.load(Ordering::Relaxed),
                maximum_polyphony: self.inner.maximum_polyphony.load(Ordering::Relaxed) as u16,
                master_volume: f32::from_bits(
                    self.inner.master_volume_bits.load(Ordering::Relaxed),
                ),
            },
            self.inner.revision.load(Ordering::Relaxed),
        )
    }

    fn soundfont(&self) -> Arc<SoundFont> {
        self.inner.soundfont.load_full()
    }
}

const SOUNDFONT_PARAMS: &[ParameterDescriptor] = &[
    ParameterDescriptor {
        id: "bank",
        name: "Bank",
        default: 0.0,
    },
    ParameterDescriptor {
        id: "program",
        name: "Program",
        default: 0.0,
    },
    ParameterDescriptor {
        id: "follow_midi",
        name: "Follow MIDI",
        default: 0.0,
    },
    ParameterDescriptor {
        id: "maximum_polyphony",
        name: "Polyphony",
        default: 0.225_806_44,
    },
    ParameterDescriptor {
        id: "master_volume",
        name: "Master Volume",
        default: DEFAULT_MASTER_VOLUME,
    },
];

pub(crate) const DESCRIPTOR: &ProcessorDescriptor = &ProcessorDescriptor {
    name: "SoundFont",
    params: SOUNDFONT_PARAMS,
    editor: Some(EditorDescriptor {
        default_size: EditorSize {
            width: 440,
            height: 360,
        },
        min_size: Some(EditorSize {
            width: 360,
            height: 320,
        }),
        resizable: false,
    }),
};

pub(crate) fn descriptor() -> &'static ProcessorDescriptor {
    DESCRIPTOR
}

#[derive(Debug, Clone)]
struct SharedSoundfontBinding {
    catalog: Arc<Vec<SoundfontCatalogEntry>>,
    available_soundfonts: Arc<std::collections::HashMap<String, Arc<SoundFont>>>,
    state: SharedSoundfontState,
}

impl SharedSoundfontBinding {
    fn apply_state(&self, state: SoundfontProcessorState) -> Result<(), ControllerError> {
        let Some(soundfont) = self.available_soundfonts.get(&state.soundfont_id) else {
            return Err(ControllerError::Backend(format!(
                "soundfont resource `{}` is unavailable",
                state.soundfont_id
            )));
        };
        self.state.update(&state, Arc::clone(soundfont));
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SoundfontBinding {
    shared: Arc<SharedSoundfontBinding>,
}

impl RuntimeBinding for SoundfontBinding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(SoundfontController {
            shared: Arc::clone(&self.shared),
        })
    }

    fn update_in_place(&self, slot: &SlotState) -> Result<bool, ProcessorStateError> {
        let Some(state) = slot.decode_built_in(BUILTIN_SOUNDFONT_ID, decode_state)? else {
            return Ok(false);
        };
        Ok(self.shared.apply_state(state).is_ok())
    }
}

struct SoundfontController {
    shared: Arc<SharedSoundfontBinding>,
}

impl Controller for SoundfontController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        descriptor()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        let (state, _) = self.shared.state.snapshot();
        match id {
            "bank" => Ok(f32::from(state.bank) / f32::from(MIDI_14BIT_MAX)),
            "program" => Ok(f32::from(state.program) / f32::from(MIDI_PROGRAM_MAX)),
            "follow_midi" => Ok(if state.follow_midi { 1.0 } else { 0.0 }),
            "maximum_polyphony" => Ok(normalize_polyphony(state.maximum_polyphony)),
            "master_volume" => Ok(state.master_volume.clamp(0.0, 1.0)),
            _ => Err(ControllerError::UnknownParameter(id.to_string())),
        }
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        let (mut state, _) = self.shared.state.snapshot();
        match id {
            "bank" => {
                state.bank =
                    (normalized.clamp(0.0, 1.0) * f32::from(MIDI_14BIT_MAX)).round() as u16;
            }
            "program" => {
                state.program =
                    (normalized.clamp(0.0, 1.0) * f32::from(MIDI_PROGRAM_MAX)).round() as u8;
            }
            "follow_midi" => state.follow_midi = normalized >= 0.5,
            "maximum_polyphony" => {
                state.maximum_polyphony = denormalize_polyphony(normalized);
            }
            "master_volume" => state.master_volume = normalized.clamp(0.0, 1.0),
            _ => return Err(ControllerError::UnknownParameter(id.to_string())),
        }
        self.shared.apply_state(state)
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        let (state, _) = self.shared.state.snapshot();
        Ok(encode_state(&state))
    }

    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError> {
        let state =
            decode_state(state).map_err(|error| ControllerError::Backend(error.to_string()))?;
        self.shared.apply_state(state)
    }

    fn create_editor_session(&self) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        Ok(Some(Box::new(SoundfontEditorSession::new(Arc::clone(
            &self.shared,
        )))))
    }
}

struct SoundfontEditorSession {
    shared: Arc<SharedSoundfontBinding>,
    window: Option<ViziaWindowHandle>,
}

impl SoundfontEditorSession {
    fn new(shared: Arc<SharedSoundfontBinding>) -> Self {
        Self {
            shared,
            window: None,
        }
    }
}

impl EditorSession for SoundfontEditorSession {
    fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError> {
        if self.window.is_some() {
            self.detach()?;
        }

        let parent = parent_window(parent.window)?;
        let shared = Arc::clone(&self.shared);
        let window = ViziaApplication::new(move |cx| {
            SoundfontEditorModel::build_editor(cx, Arc::clone(&shared));
        })
        .title("SoundFont")
        .inner_size((440, 360))
        .with_scale_policy(WindowScalePolicy::SystemScaleFactor)
        .open_parented(&parent);

        self.window = Some(window);
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        if let Some(mut window) = self.window.take() {
            window.close();
        }
        Ok(())
    }

    fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
        Ok(())
    }

    fn resize(&mut self, _size: EditorSize) -> Result<(), EditorError> {
        Ok(())
    }
}

impl Drop for SoundfontEditorSession {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

#[derive(Clone)]
enum SoundfontEditorEvent {
    SelectSoundfont(usize),
    SelectPreset(usize),
    ToggleFollowMidi,
    SetPolyphony(f64),
    SetMasterVolume(f32),
}

#[derive(Clone, Copy)]
struct SoundfontEditorView {
    soundfont_names: Signal<Vec<String>>,
    selected_soundfont: Signal<usize>,
    preset_names: Signal<Vec<String>>,
    selected_preset: Signal<usize>,
    follow_midi: Signal<bool>,
    maximum_polyphony: Signal<f64>,
    master_volume: Signal<f32>,
}

struct SoundfontEditorModel {
    shared: Arc<SharedSoundfontBinding>,
    selected_soundfont: Signal<usize>,
    preset_names: Signal<Vec<String>>,
    selected_preset: Signal<usize>,
    follow_midi: Signal<bool>,
    maximum_polyphony: Signal<f64>,
    master_volume: Signal<f32>,
}

impl SoundfontEditorModel {
    fn build_editor(cx: &mut Context, shared: Arc<SharedSoundfontBinding>) {
        let (snapshot, _) = shared.state.snapshot();
        let soundfont_names = Signal::new(
            shared
                .catalog
                .iter()
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>(),
        );
        let selected_soundfont = selected_soundfont_index(&shared.catalog, &snapshot.soundfont_id);
        let preset_names = Signal::new(preset_names(&shared.catalog, selected_soundfont));
        let selected_preset = selected_preset_index(
            &shared.catalog,
            selected_soundfont,
            snapshot.bank,
            snapshot.program,
        );
        let selected_soundfont_signal = Signal::new(selected_soundfont);
        let selected_preset_signal = Signal::new(selected_preset);
        let follow_midi = Signal::new(snapshot.follow_midi);
        let maximum_polyphony = Signal::new(f64::from(snapshot.maximum_polyphony));
        let master_volume = Signal::new(snapshot.master_volume.clamp(0.0, 1.0));
        let view = SoundfontEditorView {
            soundfont_names,
            selected_soundfont: selected_soundfont_signal,
            preset_names,
            selected_preset: selected_preset_signal,
            follow_midi,
            maximum_polyphony,
            master_volume,
        };

        Self {
            shared,
            selected_soundfont: view.selected_soundfont,
            preset_names: view.preset_names,
            selected_preset: view.selected_preset,
            follow_midi: view.follow_midi,
            maximum_polyphony: view.maximum_polyphony,
            master_volume: view.master_volume,
        }
        .build(cx);

        build_soundfont_editor(cx, view);
    }

    fn refresh_presets(&mut self, soundfont_index: usize, bank: u16, program: u8) {
        self.preset_names
            .set(preset_names(&self.shared.catalog, soundfont_index));
        self.selected_preset.set(selected_preset_index(
            &self.shared.catalog,
            soundfont_index,
            bank,
            program,
        ));
    }
}

impl Model for SoundfontEditorModel {
    fn event(&mut self, _cx: &mut EventContext<'_>, event: &mut Event) {
        event.map(|app_event, _| match app_event {
            SoundfontEditorEvent::SelectSoundfont(index) => {
                let Some(entry) = self.shared.catalog.get(*index) else {
                    return;
                };
                self.selected_soundfont.set(*index);

                let (mut state, _) = self.shared.state.snapshot();
                state.soundfont_id = entry.id.clone();
                if let Some(preset) = entry
                    .presets
                    .iter()
                    .find(|preset| preset.bank == state.bank && preset.program == state.program)
                    .or_else(|| entry.presets.first())
                {
                    state.bank = preset.bank;
                    state.program = preset.program;
                }

                self.refresh_presets(*index, state.bank, state.program);
                let _ = self.shared.apply_state(state);
            }
            SoundfontEditorEvent::SelectPreset(index) => {
                let soundfont_index = self.selected_soundfont.get();
                let Some(entry) = self.shared.catalog.get(soundfont_index) else {
                    return;
                };
                let Some(preset) = entry.presets.get(*index) else {
                    return;
                };

                self.selected_preset.set(*index);
                let (mut state, _) = self.shared.state.snapshot();
                state.bank = preset.bank;
                state.program = preset.program;
                let _ = self.shared.apply_state(state);
            }
            SoundfontEditorEvent::ToggleFollowMidi => {
                let next = !self.follow_midi.get();
                self.follow_midi.set(next);
                let (mut state, _) = self.shared.state.snapshot();
                state.follow_midi = next;
                let _ = self.shared.apply_state(state);
            }
            SoundfontEditorEvent::SetPolyphony(value) => {
                let polyphony = value
                    .round()
                    .clamp(f64::from(MINIMUM_POLYPHONY), f64::from(MAXIMUM_POLYPHONY))
                    as u16;
                self.maximum_polyphony.set(f64::from(polyphony));
                let (mut state, _) = self.shared.state.snapshot();
                state.maximum_polyphony = polyphony;
                let _ = self.shared.apply_state(state);
            }
            SoundfontEditorEvent::SetMasterVolume(value) => {
                let volume = value.clamp(0.0, 1.0);
                self.master_volume.set(volume);
                let (mut state, _) = self.shared.state.snapshot();
                state.master_volume = volume;
                let _ = self.shared.apply_state(state);
            }
        });
    }
}

fn build_soundfont_editor(cx: &mut Context, view: SoundfontEditorView) {
    let selected_preset_label = Memo::new(move |_| {
        view.preset_names
            .get()
            .get(view.selected_preset.get())
            .cloned()
            .unwrap_or_else(|| "No programs".to_string())
    });

    VStack::new(cx, |cx| {
        editor_row(cx, "SoundFont", |cx| {
            ComboBox::new(cx, view.soundfont_names, view.selected_soundfont)
                .on_select(|cx, index| cx.emit(SoundfontEditorEvent::SelectSoundfont(index)))
                .width(Stretch(1.0));
        });

        editor_row(cx, "Program", |cx| {
            Dropdown::new(
                cx,
                move |cx| {
                    Button::new(cx, move |cx| {
                        Label::new(cx, selected_preset_label).hoverable(false)
                    })
                    .on_press(|cx| cx.emit(PopupEvent::Switch))
                    .width(Stretch(1.0));
                },
                move |cx| {
                    VirtualList::new(cx, view.preset_names, 28.0, move |cx, _index, item| {
                        Label::new(cx, item).hoverable(false)
                    })
                    .width(Stretch(1.0))
                    .height(Pixels(224.0))
                    .show_horizontal_scrollbar(false)
                    .selectable(Selectable::Single)
                    .selection(view.selected_preset.map(|index| vec![*index]))
                    .on_select(|cx, index| {
                        cx.emit(SoundfontEditorEvent::SelectPreset(index));
                        cx.emit(PopupEvent::Close);
                    });
                },
            )
            .show_arrow(false)
            .width(Stretch(1.0));
        });

        HStack::new(cx, |cx| {
            Checkbox::new(cx, view.follow_midi)
                .on_toggle(|cx| cx.emit(SoundfontEditorEvent::ToggleFollowMidi))
                .id("soundfont-follow-midi");
            Label::new(cx, "Follow MIDI bank/program").describing("soundfont-follow-midi");
        })
        .horizontal_gap(Pixels(8.0))
        .alignment(Alignment::Left)
        .size(Auto);

        editor_row(cx, "Polyphony", |cx| {
            Spinbox::new(cx, view.maximum_polyphony)
                .min(f64::from(MINIMUM_POLYPHONY))
                .max(f64::from(MAXIMUM_POLYPHONY))
                .on_change(|cx, value| cx.emit(SoundfontEditorEvent::SetPolyphony(value)))
                .width(Pixels(120.0));
        });

        editor_row(cx, "Master Volume", |cx| {
            HStack::new(cx, |cx| {
                Slider::new(cx, view.master_volume)
                    .range(0.0..1.0)
                    .on_change(|cx, value| cx.emit(SoundfontEditorEvent::SetMasterVolume(value)))
                    .width(Stretch(1.0));
                Label::new(
                    cx,
                    Memo::new(move |_| format!("{:.0}%", view.master_volume.get() * 100.0)),
                )
                .width(Pixels(52.0));
            })
            .horizontal_gap(Pixels(12.0))
            .alignment(Alignment::Center);
        });
    })
    .vertical_gap(Pixels(12.0))
    .padding(Pixels(16.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn editor_row<F>(cx: &mut Context, label: &'static str, content: F)
where
    F: FnOnce(&mut Context),
{
    HStack::new(cx, |cx| {
        Label::new(cx, label).width(Pixels(104.0));
        content(cx);
    })
    .alignment(Alignment::Center)
    .horizontal_gap(Pixels(12.0))
    .width(Stretch(1.0))
    .height(Auto);
}

fn preset_names(catalog: &[SoundfontCatalogEntry], soundfont_index: usize) -> Vec<String> {
    catalog
        .get(soundfont_index)
        .map(|entry| {
            entry
                .presets
                .iter()
                .map(|preset| format!("{:03}:{:03} {}", preset.bank, preset.program, preset.name))
                .collect()
        })
        .unwrap_or_default()
}

fn selected_soundfont_index(catalog: &[SoundfontCatalogEntry], soundfont_id: &str) -> usize {
    catalog
        .iter()
        .position(|entry| entry.id == soundfont_id)
        .unwrap_or(0)
}

fn selected_preset_index(
    catalog: &[SoundfontCatalogEntry],
    soundfont_index: usize,
    bank: u16,
    program: u8,
) -> usize {
    catalog
        .get(soundfont_index)
        .and_then(|entry| {
            entry
                .presets
                .iter()
                .position(|preset| preset.bank == bank && preset.program == program)
        })
        .unwrap_or(0)
}

fn parent_window(raw: RawWindowHandle) -> Result<ParentWindow, EditorError> {
    match raw {
        RawWindowHandle::AppKit(handle) => Ok(ParentWindow(handle.ns_view.as_ptr())),
        RawWindowHandle::Win32(handle) => Ok(ParentWindow(handle.hwnd.get() as *mut _)),
        RawWindowHandle::Xcb(handle) => Ok(ParentWindow(handle.window.get() as usize as *mut _)),
        RawWindowHandle::Xlib(handle) => Ok(ParentWindow(handle.window as usize as *mut _)),
        RawWindowHandle::Wayland(handle) => Ok(ParentWindow(handle.surface.as_ptr())),
        other => Err(EditorError::HostUnavailable(format!(
            "unsupported editor parent handle: {other:?}"
        ))),
    }
}

pub(crate) fn create_runtime(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    let Some(state) = slot.decode_built_in(BUILTIN_SOUNDFONT_ID, decode_state)? else {
        return Ok(None);
    };
    let Some(loaded) = context.soundfonts.get(&state.soundfont_id) else {
        return Ok(None);
    };
    let shared_state = SharedSoundfontState::new(&state, Arc::clone(&loaded.soundfont));
    let available_soundfonts = Arc::new(
        context
            .soundfonts
            .iter()
            .map(|(id, loaded)| (id.clone(), Arc::clone(&loaded.soundfont)))
            .collect(),
    );
    let catalog = Arc::new(
        context
            .soundfont_resources
            .iter()
            .filter_map(|resource| {
                context
                    .soundfonts
                    .get(&resource.id)
                    .map(|loaded| SoundfontCatalogEntry {
                        id: resource.id.clone(),
                        name: resource.name.clone(),
                        presets: Arc::clone(&loaded.presets),
                    })
            })
            .collect(),
    );
    let processor = SoundfontProcessor::new_with_shared_program(
        &loaded.soundfont,
        context.soundfont_settings,
        state,
        Some(shared_state.clone()),
    )
    .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    Ok(Some(InstrumentRuntimeSpec {
        processor: Box::new(processor),
        binding: Box::new(SoundfontBinding {
            shared: Arc::new(SharedSoundfontBinding {
                catalog,
                available_soundfonts,
                state: shared_state,
            }),
        }),
    }))
}

/// Encodes typed SoundFont state into the processor state blob stored in slots.
#[must_use]
pub fn encode_state(state: &SoundfontProcessorState) -> ProcessorState {
    encode_soundfont_state(state)
}

/// Encodes a simple SoundFont program selection into the processor state blob.
#[must_use]
pub fn state(soundfont_id: impl Into<String>, bank: u16, program: u8) -> ProcessorState {
    encode_state(&SoundfontProcessorState {
        soundfont_id: soundfont_id.into(),
        bank,
        program,
        ..SoundfontProcessorState::default()
    })
}

/// Decodes typed SoundFont state from the processor state blob stored in slots.
pub fn decode_state(
    state: &ProcessorState,
) -> Result<SoundfontProcessorState, ProcessorStateError> {
    SoundfontProcessor::decode_state(state)
}

impl SoundfontProcessor {
    const TRACK_CHANNEL: i32 = 0;

    #[cfg(test)]
    pub fn new(
        soundfont: &Arc<SoundFont>,
        settings: SoundfontSynthSettings,
        state: SoundfontProcessorState,
    ) -> Result<Self, SoundfontSynthError> {
        Self::new_with_shared_program(soundfont, settings, state, None)
    }

    pub(crate) fn new_with_shared_program(
        soundfont: &Arc<SoundFont>,
        settings: SoundfontSynthSettings,
        state: SoundfontProcessorState,
        shared_state: Option<SharedSoundfontState>,
    ) -> Result<Self, SoundfontSynthError> {
        let mut synthesizer = build_synthesizer(soundfont, settings, &state)?;
        synthesizer.set_master_volume(state.master_volume.clamp(0.0, 1.0));
        let applied_shared_revision = shared_state
            .as_ref()
            .map_or(0, |shared| shared.snapshot().1);
        let mut processor = Self {
            settings,
            synthesizer,
            state,
            shared_state,
            applied_shared_revision,
            needs_render: false,
            silent_blocks: 0,
        };
        processor.apply_program();
        Ok(processor)
    }

    /// Decodes typed SoundFont state from the processor state blob stored in slots.
    pub fn decode_state(
        state: &ProcessorState,
    ) -> Result<SoundfontProcessorState, ProcessorStateError> {
        bincode::deserialize(&state.0)
            .or_else(|_| {
                #[derive(Deserialize)]
                struct LegacySoundfontProcessorState {
                    soundfont_id: String,
                    bank: u16,
                    program: u8,
                }

                bincode::deserialize::<LegacySoundfontProcessorState>(&state.0).map(|legacy| {
                    SoundfontProcessorState {
                        soundfont_id: legacy.soundfont_id,
                        bank: legacy.bank,
                        program: legacy.program,
                        ..SoundfontProcessorState::default()
                    }
                })
            })
            .map_err(|error| ProcessorStateError::Decode(error.to_string()))
    }

    fn apply_program(&mut self) {
        self.synthesizer.note_off_all(true);
        self.synthesizer.reset();
        self.synthesizer.process_midi_message(
            Self::TRACK_CHANNEL,
            0xB0,
            0x00,
            i32::from(self.state.bank.min(127)),
        );
        self.synthesizer.process_midi_message(
            Self::TRACK_CHANNEL,
            0xC0,
            i32::from(self.state.program),
            0,
        );
        self.synthesizer
            .set_master_volume(self.state.master_volume.clamp(0.0, 1.0));
        self.needs_render = false;
        self.silent_blocks = 0;
    }

    fn rebuild_synth(&mut self) {
        let Some(shared) = &self.shared_state else {
            return;
        };
        let soundfont = shared.soundfont();
        if let Ok(synthesizer) = build_synthesizer(&soundfont, self.settings, &self.state) {
            self.synthesizer = synthesizer;
        }
        self.apply_program();
    }

    fn sync_shared_state(&mut self) {
        let Some(shared) = &self.shared_state else {
            return;
        };
        let (state, revision) = shared.snapshot();
        if revision == self.applied_shared_revision {
            return;
        }
        let rebuild_needed = state.soundfont_id != self.state.soundfont_id
            || state.maximum_polyphony != self.state.maximum_polyphony;
        let program_changed = state.bank != self.state.bank || state.program != self.state.program;
        let follow_changed = state.follow_midi != self.state.follow_midi;
        let volume_changed = (state.master_volume - self.state.master_volume).abs() > f32::EPSILON;
        self.state = state;
        self.applied_shared_revision = revision;
        if rebuild_needed {
            self.rebuild_synth();
            return;
        }
        if program_changed || follow_changed {
            self.apply_program();
            return;
        }
        if volume_changed {
            self.synthesizer
                .set_master_volume(self.state.master_volume.clamp(0.0, 1.0));
        }
    }
}

impl Processor for SoundfontProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        DESCRIPTOR
    }

    fn set_param(&mut self, id: &str, normalized: f32) -> bool {
        let normalized = normalized.clamp(0.0, 1.0);
        match id {
            "bank" => {
                self.state.bank = (normalized * f32::from(MIDI_14BIT_MAX)).round() as u16;
                self.apply_program();
                true
            }
            "program" => {
                self.state.program = (normalized * f32::from(MIDI_PROGRAM_MAX)).round() as u8;
                self.apply_program();
                true
            }
            "follow_midi" => {
                self.state.follow_midi = normalized >= 0.5;
                true
            }
            "maximum_polyphony" => {
                self.state.maximum_polyphony = denormalize_polyphony(normalized);
                self.rebuild_synth();
                true
            }
            "master_volume" => {
                self.state.master_volume = normalized.clamp(0.0, 1.0);
                self.synthesizer.set_master_volume(self.state.master_volume);
                true
            }
            _ => false,
        }
    }

    fn get_param(&self, id: &str) -> Option<f32> {
        match id {
            "bank" => Some(f32::from(self.state.bank) / f32::from(MIDI_14BIT_MAX)),
            "program" => Some(f32::from(self.state.program) / f32::from(MIDI_PROGRAM_MAX)),
            "follow_midi" => Some(if self.state.follow_midi { 1.0 } else { 0.0 }),
            "maximum_polyphony" => Some(normalize_polyphony(self.state.maximum_polyphony)),
            "master_volume" => Some(self.state.master_volume.clamp(0.0, 1.0)),
            _ => None,
        }
    }

    fn save_state(&self) -> ProcessorState {
        encode_soundfont_state(&self.state)
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        self.state = Self::decode_state(state)?;
        self.rebuild_synth();
        Ok(())
    }

    fn reset(&mut self) {
        self.apply_program();
    }
}

impl InstrumentProcessor for SoundfontProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        self.sync_shared_state();
        match event {
            MidiEvent::NoteOn { note, velocity, .. } => {
                if velocity > 0 {
                    self.needs_render = true;
                    self.silent_blocks = 0;
                }
                self.synthesizer
                    .note_on(Self::TRACK_CHANNEL, i32::from(note), i32::from(velocity))
            }
            MidiEvent::NoteOff { note, velocity, .. } => self.synthesizer.process_midi_message(
                Self::TRACK_CHANNEL,
                0x80,
                i32::from(note),
                i32::from(velocity),
            ),
            MidiEvent::ControlChange {
                controller, value, ..
            } => {
                if self.state.follow_midi && controller == 0 {
                    self.state.bank = u16::from(value);
                    if let Some(shared) = &self.shared_state {
                        shared.update_bank_program(self.state.bank, self.state.program);
                        self.applied_shared_revision = shared.snapshot().1;
                    }
                    self.apply_program();
                }
                if !matches!(controller, 32) {
                    self.synthesizer.process_midi_message(
                        Self::TRACK_CHANNEL,
                        0xB0,
                        i32::from(controller),
                        i32::from(value),
                    );
                }
            }
            MidiEvent::ProgramChange { program, .. } => {
                if self.state.follow_midi {
                    self.state.program = program;
                    if let Some(shared) = &self.shared_state {
                        shared.update_bank_program(self.state.bank, self.state.program);
                        self.applied_shared_revision = shared.snapshot().1;
                    }
                    self.apply_program();
                }
            }
            MidiEvent::ChannelPressure { pressure, .. } => self.synthesizer.process_midi_message(
                Self::TRACK_CHANNEL,
                0xD0,
                i32::from(pressure),
                0,
            ),
            MidiEvent::PolyPressure { note, pressure, .. } => {
                self.synthesizer.process_midi_message(
                    Self::TRACK_CHANNEL,
                    0xA0,
                    i32::from(note),
                    i32::from(pressure),
                )
            }
            MidiEvent::PitchBend { value, .. } => {
                let midi_value = (i32::from(value) + 8192).clamp(0, i32::from(MIDI_14BIT_MAX));
                self.synthesizer.process_midi_message(
                    Self::TRACK_CHANNEL,
                    0xE0,
                    midi_value & 0x7F,
                    (midi_value >> 7) & 0x7F,
                );
            }
            MidiEvent::AllNotesOff { .. } => self
                .synthesizer
                .note_off_all_channel(Self::TRACK_CHANNEL, false),
            MidiEvent::AllSoundOff { .. } => {
                self.needs_render = false;
                self.silent_blocks = 0;
                self.synthesizer
                    .note_off_all_channel(Self::TRACK_CHANNEL, true)
            }
            MidiEvent::ResetAllControllers { .. } => {
                self.synthesizer
                    .process_midi_message(Self::TRACK_CHANNEL, 0xB0, 121, 0)
            }
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.sync_shared_state();
        if !self.needs_render {
            left.fill(0.0);
            right.fill(0.0);
            return;
        }
        self.synthesizer.render(left, right);
        let peak = left
            .iter()
            .chain(right.iter())
            .map(|sample| sample.abs())
            .fold(0.0_f32, f32::max);
        if peak <= 1.0e-6 {
            self.silent_blocks = self.silent_blocks.saturating_add(1);
            if self.silent_blocks >= 8 {
                self.needs_render = false;
            }
        } else {
            self.silent_blocks = 0;
        }
    }

    fn is_sleeping(&self) -> bool {
        !self.needs_render
    }
}

pub(crate) fn encode_soundfont_state(state: &SoundfontProcessorState) -> ProcessorState {
    ProcessorState(bincode::serialize(state).unwrap_or_default())
}

fn normalize_polyphony(value: u16) -> f32 {
    let span = f32::from(MAXIMUM_POLYPHONY - MINIMUM_POLYPHONY);
    f32::from(value.clamp(MINIMUM_POLYPHONY, MAXIMUM_POLYPHONY) - MINIMUM_POLYPHONY) / span
}

fn denormalize_polyphony(normalized: f32) -> u16 {
    let span = f32::from(MAXIMUM_POLYPHONY - MINIMUM_POLYPHONY);
    MINIMUM_POLYPHONY + (normalized.clamp(0.0, 1.0) * span).round() as u16
}

fn build_synthesizer(
    soundfont: &Arc<SoundFont>,
    settings: SoundfontSynthSettings,
    state: &SoundfontProcessorState,
) -> Result<Synthesizer, SoundfontSynthError> {
    let mut synth_settings = SynthesizerSettings::new(settings.sample_rate);
    synth_settings.block_size = settings.block_size;
    synth_settings.maximum_polyphony = usize::from(
        state
            .maximum_polyphony
            .clamp(MINIMUM_POLYPHONY, MAXIMUM_POLYPHONY),
    );
    synth_settings.enable_reverb_and_chorus = false;
    Synthesizer::new(soundfont, &synth_settings).map_err(|source| {
        SoundfontSynthError::CreateSynth {
            id: state.soundfont_id.clone(),
            source,
        }
    })
}

fn soundfont_presets(soundfont: &SoundFont) -> Vec<SoundfontPreset> {
    let mut presets = soundfont
        .get_presets()
        .iter()
        .filter_map(|preset| {
            let bank = u16::try_from(preset.get_bank_number()).ok()?;
            let program = u8::try_from(preset.get_patch_number()).ok()?;
            Some(SoundfontPreset {
                bank,
                program,
                name: preset.get_name().trim().to_string(),
            })
        })
        .collect::<Vec<_>>();
    presets.sort_by(|left, right| {
        left.bank
            .cmp(&right.bank)
            .then(left.program.cmp(&right.program))
            .then_with(|| left.name.cmp(&right.name))
    });
    presets
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use super::{
        LoadedSoundfont, SoundfontProcessor, SoundfontProcessorState, SoundfontSynthSettings,
        create_runtime, decode_state, encode_state,
    };
    use crate::instrument::{InstrumentProcessor, MidiEvent, Processor};
    use crate::instrument::{InstrumentRuntimeContext, SlotState};
    use crate::test_utils::test_soundfont_resource;

    #[test]
    fn soundfont_processor_renders_after_note_on() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
            if left
                .iter()
                .chain(right.iter())
                .any(|sample| sample.abs() > 1.0e-6)
            {
                return;
            }
        }

        panic!("soundfont processor produced silence after note on");
    }

    #[test]
    fn soundfont_presets_are_sorted_by_bank_program_and_name() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let presets = super::soundfont_presets(&loaded.soundfont);

        assert!(presets.windows(2).all(|pair| {
            let left = &pair[0];
            let right = &pair[1];
            (left.bank, left.program, left.name.as_str())
                <= (right.bank, right.program, right.name.as_str())
        }));
    }

    #[test]
    fn soundfont_processor_stays_silent_without_note_on() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        let mut left = vec![1.0; 64];
        let mut right = vec![1.0; 64];
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
            assert!(
                left.iter()
                    .chain(right.iter())
                    .all(|sample| sample.abs() <= 1.0e-6),
                "soundfont processor should stay silent before any note on"
            );
            left.fill(1.0);
            right.fill(1.0);
        }
    }

    #[test]
    fn soundfont_processor_renders_after_note_on_on_nonzero_channel() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 3,
            note: 60,
            velocity: 100,
        });

        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
            if left
                .iter()
                .chain(right.iter())
                .any(|sample| sample.abs() > 1.0e-6)
            {
                return;
            }
        }

        panic!("soundfont processor produced silence after nonzero-channel note on");
    }

    #[test]
    fn soundfont_processor_reset_silences_active_note() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
        }

        processor.reset();
        left.fill(0.0);
        right.fill(0.0);
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
        }

        assert!(
            left.iter()
                .chain(right.iter())
                .all(|sample| sample.abs() <= 1.0e-6),
            "soundfont processor reset should silence active notes"
        );
    }

    #[test]
    fn soundfont_processor_renders_after_reset_then_note_on() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.reset();
        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        for _ in 0..8 {
            processor.render(&mut left, &mut right);
            if left
                .iter()
                .chain(right.iter())
                .any(|sample| sample.abs() > 1.0e-6)
            {
                return;
            }
        }

        panic!("soundfont processor produced silence after reset then note on");
    }

    #[test]
    fn soundfont_processor_ignores_midi_program_override() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let settings = SoundfontSynthSettings::new(44_100, 64);
        let mut selected_program = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");
        let mut overridden_program = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");

        overridden_program.handle_midi(MidiEvent::ProgramChange {
            channel: 0,
            program: 0,
        });
        overridden_program.handle_midi(MidiEvent::ControlChange {
            channel: 0,
            controller: 0,
            value: 0,
        });
        overridden_program.handle_midi(MidiEvent::ControlChange {
            channel: 0,
            controller: 32,
            value: 0,
        });

        selected_program.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        overridden_program.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut selected_left = vec![0.0; 512];
        let mut selected_right = vec![0.0; 512];
        let mut overridden_left = vec![0.0; 512];
        let mut overridden_right = vec![0.0; 512];

        for _ in 0..8 {
            selected_program.render(&mut selected_left, &mut selected_right);
            overridden_program.render(&mut overridden_left, &mut overridden_right);
        }

        assert_eq!(selected_left, overridden_left);
        assert_eq!(selected_right, overridden_right);
    }

    #[test]
    fn soundfont_processor_follows_midi_program_and_bank_when_enabled() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let settings = SoundfontSynthSettings::new(44_100, 64);
        let mut selected_program = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                follow_midi: true,
                maximum_polyphony: 64,
                master_volume: 0.5,
            },
        )
        .expect("processor should initialize");
        let mut overridden_program = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                follow_midi: true,
                maximum_polyphony: 64,
                master_volume: 0.5,
            },
        )
        .expect("processor should initialize");

        overridden_program.handle_midi(MidiEvent::ControlChange {
            channel: 0,
            controller: 0,
            value: 0,
        });
        overridden_program.handle_midi(MidiEvent::ProgramChange {
            channel: 0,
            program: 0,
        });

        selected_program.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        overridden_program.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut selected_left = vec![0.0; 512];
        let mut selected_right = vec![0.0; 512];
        let mut overridden_left = vec![0.0; 512];
        let mut overridden_right = vec![0.0; 512];

        for _ in 0..8 {
            selected_program.render(&mut selected_left, &mut selected_right);
            overridden_program.render(&mut overridden_left, &mut overridden_right);
        }

        assert!(
            selected_left
                .iter()
                .zip(overridden_left.iter())
                .chain(selected_right.iter().zip(overridden_right.iter()))
                .any(|(a, b)| (a - b).abs() > 1.0e-6),
            "midi-selected program should change the rendered signal when follow_midi is enabled"
        );
    }

    #[test]
    fn soundfont_state_decodes_previous_project_format_with_defaults() {
        #[derive(serde::Serialize)]
        struct LegacySoundfontProcessorState {
            soundfont_id: String,
            bank: u16,
            program: u8,
        }

        let legacy = crate::instrument::ProcessorState(
            bincode::serialize(&LegacySoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 2,
                program: 40,
            })
            .expect("legacy state should encode"),
        );

        let decoded = decode_state(&legacy).expect("legacy state should decode");

        assert_eq!(decoded.soundfont_id, "default");
        assert_eq!(decoded.bank, 2);
        assert_eq!(decoded.program, 40);
        assert!(!decoded.follow_midi);
        assert_eq!(decoded.maximum_polyphony, 64);
        assert!((decoded.master_volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn soundfont_controller_exposes_editor_session() {
        let resource = test_soundfont_resource();
        let loaded = LoadedSoundfont::load(&resource).expect("test SoundFont should load");
        let mut soundfonts = std::collections::HashMap::new();
        soundfonts.insert(resource.id.clone(), loaded);
        let resources = vec![resource];
        let slot = SlotState::built_in(
            crate::instrument::BUILTIN_SOUNDFONT_ID,
            encode_state(&SoundfontProcessorState::default()),
        );
        let context = InstrumentRuntimeContext {
            soundfonts: &soundfonts,
            soundfont_resources: &resources,
            soundfont_settings: SoundfontSynthSettings::new(44_100, 64),
        };

        let runtime = create_runtime(&slot, &context)
            .expect("runtime should build")
            .expect("soundfont runtime should exist");
        let controller = runtime.binding.controller();

        assert!(controller.descriptor().editor.is_some());
        assert!(
            controller
                .create_editor_session()
                .expect("editor creation should succeed")
                .is_some()
        );
    }

    #[test]
    fn soundfont_editor_is_not_resizable() {
        assert_eq!(
            super::descriptor().editor.map(|editor| editor.resizable),
            Some(false)
        );
    }

    #[test]
    fn soundfont_processor_selected_program_changes_rendered_signal() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let settings = SoundfontSynthSettings::new(44_100, 64);
        let mut piano = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 0,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");
        let mut violin = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");

        piano.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        violin.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut piano_left = vec![0.0; 512];
        let mut piano_right = vec![0.0; 512];
        let mut violin_left = vec![0.0; 512];
        let mut violin_right = vec![0.0; 512];

        for _ in 0..8 {
            piano.render(&mut piano_left, &mut piano_right);
            violin.render(&mut violin_left, &mut violin_right);
        }

        assert!(
            piano_left
                .iter()
                .zip(violin_left.iter())
                .chain(piano_right.iter().zip(violin_right.iter()))
                .any(|(a, b)| (a - b).abs() > 1.0e-6),
            "different selected SoundFont programs rendered the same signal"
        );
    }

    #[test]
    fn soundfont_processor_reset_preserves_selected_program() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let settings = SoundfontSynthSettings::new(44_100, 64);
        let mut violin = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 40,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");
        let mut piano = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 0,
                ..SoundfontProcessorState::default()
            },
        )
        .expect("processor should initialize");

        violin.reset();
        violin.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        piano.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut violin_left = vec![0.0; 512];
        let mut violin_right = vec![0.0; 512];
        let mut piano_left = vec![0.0; 512];
        let mut piano_right = vec![0.0; 512];

        for _ in 0..8 {
            violin.render(&mut violin_left, &mut violin_right);
            piano.render(&mut piano_left, &mut piano_right);
        }

        assert!(
            violin_left
                .iter()
                .zip(piano_left.iter())
                .chain(violin_right.iter().zip(piano_right.iter()))
                .any(|(a, b)| (a - b).abs() > 1.0e-6),
            "reset restored the SoundFont processor to the default piano program"
        );
    }

    #[test]
    fn soundfont_processor_reset_restores_silent_fast_path() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        processor.render(&mut left, &mut right);

        processor.reset();
        left.fill(1.0);
        right.fill(1.0);
        processor.render(&mut left, &mut right);
        assert!(
            left.iter()
                .chain(right.iter())
                .all(|sample| sample.abs() <= 1.0e-6),
            "soundfont processor reset should restore the silent fast path"
        );
    }

    #[test]
    fn soundfont_processor_returns_to_silent_fast_path_after_release_tail() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        processor.handle_midi(MidiEvent::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0,
        });

        let mut left = vec![0.0; 64];
        let mut right = vec![0.0; 64];
        for _ in 0..1_024 {
            processor.render(&mut left, &mut right);
            if !processor.needs_render {
                return;
            }
        }

        panic!("soundfont processor never returned to the silent fast path after note release");
    }

    #[test]
    fn soundfont_processor_reports_sleeping_when_dormant() {
        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let mut processor = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            SoundfontSynthSettings::new(44_100, 64),
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");

        assert!(
            processor.is_sleeping(),
            "fresh soundfont processor should start dormant"
        );

        processor.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        assert!(
            !processor.is_sleeping(),
            "note on should wake the processor"
        );

        processor.handle_midi(MidiEvent::AllSoundOff { channel: 0 });
        assert!(
            processor.is_sleeping(),
            "all sound off should return the processor to dormant state"
        );
    }

    #[test]
    #[ignore = "manual perf report"]
    fn perf_report_soundfont_processor_block_costs() {
        const BLOCKS: usize = 20_000;
        const BLOCK_SIZE: usize = 64;

        let loaded =
            LoadedSoundfont::load(&test_soundfont_resource()).expect("test SoundFont should load");
        let settings = SoundfontSynthSettings::new(44_100, BLOCK_SIZE);

        let mut idle = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");
        let mut armed = SoundfontProcessor::new(
            &Arc::clone(&loaded.soundfont),
            settings,
            SoundfontProcessorState::default(),
        )
        .expect("processor should initialize");
        armed.handle_midi(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });

        let mut idle_left = vec![0.0; BLOCK_SIZE];
        let mut idle_right = vec![0.0; BLOCK_SIZE];
        let idle_started = Instant::now();
        for _ in 0..BLOCKS {
            idle.render(&mut idle_left, &mut idle_right);
        }
        let idle_elapsed = idle_started.elapsed();

        let mut armed_left = vec![0.0; BLOCK_SIZE];
        let mut armed_right = vec![0.0; BLOCK_SIZE];
        let armed_started = Instant::now();
        for _ in 0..BLOCKS {
            armed.render(&mut armed_left, &mut armed_right);
        }
        let armed_elapsed = armed_started.elapsed();

        println!(
            "soundfont processor perf over {BLOCKS} blocks: idle={idle_elapsed:?} armed={armed_elapsed:?}"
        );
    }
}
