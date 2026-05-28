use super::{processor_and_editor::*, retro_ui::*, *};

/// Maximum MIDI 14-bit controller value.
pub const MIDI_14BIT_MAX: u16 = 16_383;
pub(crate) const MIDI_PROGRAM_MAX: u8 = 127;
pub(crate) const MIDI_CONTROL_MAX: f32 = 127.0;
pub(crate) const MIDI_CC_REVERB_WET: i32 = 91;
pub(crate) const MIDI_CC_CHORUS_WET: i32 = 93;
pub(crate) const MINIMUM_POLYPHONY: u16 = 8;
pub(crate) const MAXIMUM_POLYPHONY: u16 = 256;
pub(crate) const MIN_OUTPUT_GAIN_DB: f32 = -24.0;
pub(crate) const MAX_OUTPUT_GAIN_DB: f32 = 12.0;
pub(crate) const DEFAULT_OUTPUT_GAIN: f32 = 1.0;
pub(crate) const DEFAULT_OUTPUT_GAIN_NORMALIZED: f32 = 2.0 / 3.0;
pub(crate) const DEFAULT_MAXIMUM_POLYPHONY: u16 = 64;
pub(crate) const DEFAULT_REVERB_WET: f32 = 40.0 / 127.0;
pub(crate) const DEFAULT_CHORUS_WET: f32 = 0.0;
pub(crate) const PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW: f32 = 48.0;
pub(crate) const EDITOR_WIDTH: u32 = 820;
pub(crate) const EDITOR_HEIGHT: u32 = 456;
pub(crate) const RETRO_UI_FONT: &str = "retro-ui";
pub(crate) const RETRO_DISPLAY_FONT: &str = "retro-display";

pub(crate) fn normalized_to_u16(normalized: f32, max: u16) -> u16 {
    (normalized.clamp(0.0, 1.0) * f32::from(max))
        .round()
        .to_u16()
        .unwrap_or(max)
}

pub(crate) fn normalized_to_u8(normalized: f32, max: u8) -> u8 {
    (normalized.clamp(0.0, 1.0) * f32::from(max))
        .round()
        .to_u8()
        .unwrap_or(max)
}

pub(crate) fn positive_rows(value: f32) -> usize {
    value.floor().max(1.0).to_usize().unwrap_or(1)
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
    /// Linear output gain.
    #[serde(default = "default_output_gain")]
    pub output_gain: f32,
    /// Reverb wet mix amount.
    #[serde(default = "default_reverb_wet")]
    pub reverb_wet: f32,
    /// Chorus wet mix amount.
    #[serde(default = "default_chorus_wet")]
    pub chorus_wet: f32,
}

impl Default for SoundfontProcessorState {
    fn default() -> Self {
        Self {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 0,
            follow_midi: false,
            maximum_polyphony: DEFAULT_MAXIMUM_POLYPHONY,
            output_gain: DEFAULT_OUTPUT_GAIN,
            reverb_wet: DEFAULT_REVERB_WET,
            chorus_wet: DEFAULT_CHORUS_WET,
        }
    }
}

pub(crate) const fn default_maximum_polyphony() -> u16 {
    DEFAULT_MAXIMUM_POLYPHONY
}

pub(crate) const fn default_output_gain() -> f32 {
    DEFAULT_OUTPUT_GAIN
}

pub(crate) const fn default_reverb_wet() -> f32 {
    DEFAULT_REVERB_WET
}

pub(crate) const fn default_chorus_wet() -> f32 {
    DEFAULT_CHORUS_WET
}

/// Decodes typed SoundFont state from the processor state blob stored in slots.
pub fn decode_state(
    state: &ProcessorState,
) -> Result<SoundfontProcessorState, ProcessorStateError> {
    bincode::deserialize(&state.0).map_err(|error| ProcessorStateError::Decode(error.to_string()))
}

/// Encodes typed SoundFont state into the processor state blob stored in slots.
#[must_use]
pub fn encode_state(state: &SoundfontProcessorState) -> ProcessorState {
    match bincode::serialize(state) {
        Ok(bytes) => ProcessorState(bytes),
        Err(error) => {
            eprintln!("failed to encode SoundFont state: {error}");
            ProcessorState::default()
        }
    }
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

#[derive(thiserror::Error, Debug)]
pub(crate) enum SoundfontSynthError {
    #[error("failed to create synthesizer for soundfont `{id}`: {source}")]
    CreateSynth {
        id: String,
        #[source]
        source: rustysynth::SynthesizerError,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SoundfontCatalogEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) presets: Arc<Vec<SoundfontPreset>>,
}

#[derive(Debug)]
pub(crate) struct SoundfontProcessor {
    pub(crate) settings: SoundfontSynthSettings,
    pub(crate) synthesizer: Synthesizer,
    pub(crate) state: SoundfontProcessorState,
    pub(crate) shared_state: Option<SharedSoundfontState>,
    pub(crate) applied_shared_revision: u32,
    pub(crate) needs_render: bool,
    pub(crate) silent_blocks: u32,
    pub(crate) output_gain: SmoothedAudioValue,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SoundfontSharedChange {
    pub(crate) rebuild_needed: bool,
    pub(crate) program_or_follow_changed: bool,
    pub(crate) gain_changed: bool,
    pub(crate) mix_changed: bool,
}

impl SoundfontSharedChange {
    pub(crate) fn between(
        next: &SoundfontProcessorState,
        current: &SoundfontProcessorState,
    ) -> Self {
        Self {
            rebuild_needed: next.soundfont_id != current.soundfont_id
                || next.maximum_polyphony != current.maximum_polyphony,
            program_or_follow_changed: next.bank != current.bank
                || next.program != current.program
                || next.follow_midi != current.follow_midi,
            gain_changed: (next.output_gain - current.output_gain).abs() > f32::EPSILON,
            mix_changed: (next.reverb_wet - current.reverb_wet).abs() > f32::EPSILON
                || (next.chorus_wet - current.chorus_wet).abs() > f32::EPSILON,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SharedSoundfontState {
    pub(crate) inner: Arc<SharedSoundfontStateInner>,
}

#[derive(Debug)]
pub(crate) struct SharedSoundfontStateInner {
    pub(crate) soundfont: ArcSwap<SoundFont>,
    pub(crate) soundfont_id: ArcSwap<String>,
    pub(crate) bank: AtomicU16,
    pub(crate) program: AtomicU32,
    pub(crate) follow_midi: AtomicBool,
    pub(crate) maximum_polyphony: AtomicU32,
    pub(crate) output_gain_bits: AtomicU32,
    pub(crate) reverb_wet_bits: AtomicU32,
    pub(crate) chorus_wet_bits: AtomicU32,
    pub(crate) midi_activity: AtomicU32,
    pub(crate) revision: AtomicU32,
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
                output_gain_bits: AtomicU32::new(state.output_gain.to_bits()),
                reverb_wet_bits: AtomicU32::new(state.reverb_wet.to_bits()),
                chorus_wet_bits: AtomicU32::new(state.chorus_wet.to_bits()),
                midi_activity: AtomicU32::new(0),
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
            .output_gain_bits
            .store(state.output_gain.to_bits(), Ordering::Relaxed);
        self.inner
            .reverb_wet_bits
            .store(state.reverb_wet.to_bits(), Ordering::Relaxed);
        self.inner
            .chorus_wet_bits
            .store(state.chorus_wet.to_bits(), Ordering::Relaxed);
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
                output_gain: f32::from_bits(self.inner.output_gain_bits.load(Ordering::Relaxed)),
                reverb_wet: f32::from_bits(self.inner.reverb_wet_bits.load(Ordering::Relaxed)),
                chorus_wet: f32::from_bits(self.inner.chorus_wet_bits.load(Ordering::Relaxed)),
            },
            self.inner.revision.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn soundfont(&self) -> Arc<SoundFont> {
        self.inner.soundfont.load_full()
    }

    pub(crate) fn mark_midi_activity(&self) {
        self.inner.midi_activity.fetch_add(1, Ordering::Relaxed);
    }

    fn midi_activity(&self) -> u32 {
        self.inner.midi_activity.load(Ordering::Relaxed)
    }
}

pub(crate) const SOUNDFONT_PARAMS: &[ParameterDescriptor] = &[
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
        id: "output_gain",
        name: "Output Gain",
        default: DEFAULT_OUTPUT_GAIN_NORMALIZED,
    },
    ParameterDescriptor {
        id: "reverb_wet",
        name: "Reverb Dry/Wet",
        default: DEFAULT_REVERB_WET,
    },
    ParameterDescriptor {
        id: "chorus_wet",
        name: "Chorus Dry/Wet",
        default: DEFAULT_CHORUS_WET,
    },
];

pub(crate) const DESCRIPTOR: &ProcessorDescriptor = &ProcessorDescriptor {
    name: "SF-01",
    params: SOUNDFONT_PARAMS,
    editor: Some(EditorDescriptor {
        default_size: EditorSize {
            width: EDITOR_WIDTH,
            height: EDITOR_HEIGHT,
        },
        min_size: Some(EditorSize {
            width: EDITOR_WIDTH,
            height: EDITOR_HEIGHT,
        }),
        resizable: false,
    }),
};

pub(crate) fn descriptor() -> &'static ProcessorDescriptor {
    DESCRIPTOR
}

#[derive(Debug, Clone)]
pub(crate) struct SharedSoundfontBinding {
    pub(crate) catalog: Arc<Vec<SoundfontCatalogEntry>>,
    pub(crate) available_soundfonts: Arc<std::collections::HashMap<String, Arc<SoundFont>>>,
    pub(crate) state: SharedSoundfontState,
}

impl SharedSoundfontBinding {
    pub(crate) fn apply_state(
        &self,
        state: SoundfontProcessorState,
    ) -> Result<(), ControllerError> {
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
pub(crate) struct SoundfontBinding {
    pub(crate) shared: Arc<SharedSoundfontBinding>,
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

pub(crate) struct SoundfontController {
    pub(crate) shared: Arc<SharedSoundfontBinding>,
}

impl Controller for SoundfontController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        descriptor()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        let (state, _) = self.shared.state.snapshot();
        let param = SoundfontParam::parse(id)?;
        Ok(param.normalized_value(&state))
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        let (mut state, _) = self.shared.state.snapshot();
        SoundfontParam::parse(id)?.apply_normalized(&mut state, normalized);
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

#[derive(Clone, Copy)]
pub(crate) enum SoundfontParam {
    Midi(SoundfontMidiParam),
    Voice(SoundfontVoiceParam),
    Mix(SoundfontMixParam),
}

#[derive(Clone, Copy)]
pub(crate) enum SoundfontMidiParam {
    Bank,
    Program,
}

#[derive(Clone, Copy)]
pub(crate) enum SoundfontVoiceParam {
    FollowMidi,
    MaximumPolyphony,
}

#[derive(Clone, Copy)]
pub(crate) enum SoundfontMixParam {
    OutputGain,
    ReverbWet,
    ChorusWet,
}

impl SoundfontParam {
    fn parse(id: &str) -> Result<Self, ControllerError> {
        SOUNDFONT_PARAM_MAP
            .iter()
            .find_map(|(candidate, param)| (*candidate == id).then_some(*param))
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))
    }

    fn normalized_value(self, state: &SoundfontProcessorState) -> f32 {
        match self {
            Self::Midi(param) => param.normalized_value(state),
            Self::Voice(param) => param.normalized_value(state),
            Self::Mix(param) => param.normalized_value(state),
        }
    }

    fn apply_normalized(self, state: &mut SoundfontProcessorState, normalized: f32) {
        match self {
            Self::Midi(param) => param.apply_normalized(state, normalized),
            Self::Voice(param) => param.apply_normalized(state, normalized),
            Self::Mix(param) => param.apply_normalized(state, normalized),
        }
    }
}

impl SoundfontMidiParam {
    fn normalized_value(self, state: &SoundfontProcessorState) -> f32 {
        match self {
            Self::Bank => f32::from(state.bank) / f32::from(MIDI_14BIT_MAX),
            Self::Program => f32::from(state.program) / f32::from(MIDI_PROGRAM_MAX),
        }
    }

    fn apply_normalized(self, state: &mut SoundfontProcessorState, normalized: f32) {
        match self {
            Self::Bank => state.bank = normalized_to_u16(normalized, MIDI_14BIT_MAX),
            Self::Program => state.program = normalized_to_u8(normalized, MIDI_PROGRAM_MAX),
        }
    }
}

impl SoundfontVoiceParam {
    fn normalized_value(self, state: &SoundfontProcessorState) -> f32 {
        match self {
            Self::FollowMidi => {
                if state.follow_midi {
                    1.0
                } else {
                    0.0
                }
            }
            Self::MaximumPolyphony => normalize_polyphony(state.maximum_polyphony),
        }
    }

    fn apply_normalized(self, state: &mut SoundfontProcessorState, normalized: f32) {
        match self {
            Self::FollowMidi => state.follow_midi = normalized >= 0.5,
            Self::MaximumPolyphony => state.maximum_polyphony = denormalize_polyphony(normalized),
        }
    }
}

impl SoundfontMixParam {
    fn normalized_value(self, state: &SoundfontProcessorState) -> f32 {
        match self {
            Self::OutputGain => normalize_output_gain(state.output_gain),
            Self::ReverbWet => state.reverb_wet.clamp(0.0, 1.0),
            Self::ChorusWet => state.chorus_wet.clamp(0.0, 1.0),
        }
    }

    fn apply_normalized(self, state: &mut SoundfontProcessorState, normalized: f32) {
        match self {
            Self::OutputGain => state.output_gain = denormalize_output_gain(normalized),
            Self::ReverbWet => state.reverb_wet = normalized.clamp(0.0, 1.0),
            Self::ChorusWet => state.chorus_wet = normalized.clamp(0.0, 1.0),
        }
    }
}

pub(crate) const SOUNDFONT_PARAM_MAP: &[(&str, SoundfontParam)] = &[
    ("bank", SoundfontParam::Midi(SoundfontMidiParam::Bank)),
    ("program", SoundfontParam::Midi(SoundfontMidiParam::Program)),
    (
        "follow_midi",
        SoundfontParam::Voice(SoundfontVoiceParam::FollowMidi),
    ),
    (
        "maximum_polyphony",
        SoundfontParam::Voice(SoundfontVoiceParam::MaximumPolyphony),
    ),
    (
        "output_gain",
        SoundfontParam::Mix(SoundfontMixParam::OutputGain),
    ),
    (
        "reverb_wet",
        SoundfontParam::Mix(SoundfontMixParam::ReverbWet),
    ),
    (
        "chorus_wet",
        SoundfontParam::Mix(SoundfontMixParam::ChorusWet),
    ),
];

pub(crate) struct SoundfontEditorSession {
    pub(crate) shared: Arc<SharedSoundfontBinding>,
    pub(crate) window: Option<EguiWindowHandle>,
}

impl SoundfontEditorSession {
    fn new(shared: Arc<SharedSoundfontBinding>) -> Self {
        Self {
            shared,
            window: None,
        }
    }

    fn close_window(&mut self) {
        if let Some(mut window) = self.window.take() {
            window.close();
        }
    }
}

impl EditorSession for SoundfontEditorSession {
    fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError> {
        if self.window.is_some() {
            self.detach()?;
        }

        let shared = Arc::clone(&self.shared);
        let window = open_parented(
            parent.window,
            EguiWindowOptions {
                title: "SF-01".to_string(),
                width: f64::from(EDITOR_WIDTH),
                height: f64::from(EDITOR_HEIGHT),
            },
            move || SoundfontEditorApp {
                shared,
                retro_style_installed: false,
                bank_text: String::new(),
                bank_text_focused: false,
                polyphony_text: String::new(),
                polyphony_text_focused: false,
                program_scroll_first: 0,
                program_scroll_remainder: 0.0,
                soundfont_dropdown_open: false,
                seen_midi_activity: 0,
                midi_flash_frames: 0,
            },
        )
        .map_err(|error| EditorError::Backend(error.to_string()))?;

        self.window = Some(window);
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        self.close_window();
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), EditorError> {
        if visible && self.window.is_none() {
            return Err(EditorError::HostUnavailable(
                "SoundFont editor is not attached".to_string(),
            ));
        }
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        if size.width == 0 || size.height == 0 {
            return Err(EditorError::HostUnavailable(
                "SoundFont editor size must be non-zero".to_string(),
            ));
        }
        Ok(size)
    }
}

impl Drop for SoundfontEditorSession {
    fn drop(&mut self) {
        self.close_window();
    }
}

pub(crate) struct SoundfontEditorApp {
    pub(crate) shared: Arc<SharedSoundfontBinding>,
    pub(crate) retro_style_installed: bool,
    pub(crate) bank_text: String,
    pub(crate) bank_text_focused: bool,
    pub(crate) polyphony_text: String,
    pub(crate) polyphony_text_focused: bool,
    pub(crate) program_scroll_first: usize,
    pub(crate) program_scroll_remainder: f32,
    pub(crate) soundfont_dropdown_open: bool,
    pub(crate) seen_midi_activity: u32,
    pub(crate) midi_flash_frames: u8,
}

impl EguiApp for SoundfontEditorApp {
    fn update(&mut self, ui: &mut egui::Ui) {
        if !self.retro_style_installed {
            install_retro_style(ui.ctx());
            self.retro_style_installed = true;
            ui.ctx().request_repaint();
            return;
        }

        ui.painter().rect_filled(ui.max_rect(), 0.0, retro::FACE);

        let (snapshot, _) = self.shared.state.snapshot();
        let soundfont_names = self
            .shared
            .catalog
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<Vec<_>>();
        let selected_soundfont =
            selected_soundfont_index(&self.shared.catalog, &snapshot.soundfont_id);
        let banks = bank_numbers(&self.shared.catalog, selected_soundfont);
        let bank_index = selected_bank_index(&banks, snapshot.bank);
        let programs = program_choices(&self.shared.catalog, selected_soundfont, snapshot.bank);
        let program_index = selected_program_index(&programs, snapshot.program);
        if self.bank_text.is_empty() && !self.bank_text_focused {
            self.bank_text = snapshot.bank.to_string();
        }
        if self.polyphony_text.is_empty() && !self.polyphony_text_focused {
            self.polyphony_text = snapshot.maximum_polyphony.to_string();
        }

        let canvas = ui.max_rect();
        draw_window_shell(ui, canvas);
        let mut state = snapshot.clone();
        let mut changed = false;

        retro_group(ui, rect(18.0, 74.0, 390.0, 74.0), "SOUNDFONT", |ui| {
            let selected = soundfont_names
                .get(selected_soundfont)
                .map_or("No SoundFonts", String::as_str);
            if retro_select_box(ui, rect(0.0, 6.0, 350.0, 30.0), "soundfont", selected).clicked()
                && !soundfont_names.is_empty()
            {
                self.soundfont_dropdown_open = !self.soundfont_dropdown_open;
            }
        });

        retro_group(ui, rect(424.0, 74.0, 154.0, 74.0), "BANK", |ui| {
            let bank_response = retro_number_field(
                ui,
                rect(0.0, 6.0, 92.0, 30.0),
                "bank-number",
                &mut self.bank_text,
                snapshot.bank,
                0,
                MIDI_14BIT_MAX,
            );
            self.bank_text_focused = bank_response.focused;
            if let Some(bank) = bank_response.value {
                state.bank = bank;
                changed = true;
            }
            if retro_step_button(ui, rect(98.0, 6.0, 28.0, 14.0), true).clicked()
                && !banks.is_empty()
            {
                let Some(next) = banks.get((bank_index + 1) % banks.len()).copied() else {
                    return;
                };
                self.bank_text = next.to_string();
                self.bank_text_focused = false;
                self.select_bank(next);
            }
            if retro_step_button(ui, rect(98.0, 22.0, 28.0, 14.0), false).clicked()
                && !banks.is_empty()
            {
                let Some(next) = banks
                    .get((bank_index + banks.len() - 1) % banks.len())
                    .copied()
                else {
                    return;
                };
                self.bank_text = next.to_string();
                self.bank_text_focused = false;
                self.select_bank(next);
            }
        });

        let midi_activity = self.shared.state.midi_activity();
        if midi_activity != self.seen_midi_activity {
            self.seen_midi_activity = midi_activity;
            self.midi_flash_frames = 12;
        }
        let midi_active = self.midi_flash_frames > 0;
        self.midi_flash_frames = self.midi_flash_frames.saturating_sub(1);

        retro_group(ui, rect(604.0, 74.0, 196.0, 166.0), "MIDI", |ui| {
            if retro_checkbox(
                ui,
                rect(2.0, 20.0, 136.0, 24.0),
                snapshot.follow_midi,
                "Follow program",
            )
            .clicked()
            {
                state.follow_midi = !snapshot.follow_midi;
                changed = true;
            }
            draw_led(
                ui,
                pos(10.0, 72.0),
                true,
                if midi_active {
                    retro::GREEN
                } else {
                    retro::LED_IDLE
                },
            );
            retro_text(ui, pos(32.0, 64.0), "MIDI IN", 18.0, retro::TEXT, false);
        });

        retro_group(ui, rect(18.0, 166.0, 390.0, 280.0), "PROGRAM", |ui| {
            draw_display_box(
                ui,
                rect(0.0, 16.0, 316.0, 30.0),
                programs
                    .get(program_index)
                    .map_or("No programs", |program| program.label.as_str()),
            );
            if retro_step_button(ui, rect(322.0, 16.0, 28.0, 14.0), true).clicked()
                && !programs.is_empty()
                && let Some(program) =
                    programs.get((program_index + programs.len() - 1) % programs.len())
            {
                self.select_program(program.program);
            }
            if retro_step_button(ui, rect(322.0, 32.0, 28.0, 14.0), false).clicked()
                && !programs.is_empty()
                && let Some(program) = programs.get((program_index + 1) % programs.len())
            {
                self.select_program(program.program);
            }
            if let Some(program) = program_list(
                ui,
                rect(0.0, 54.0, 350.0, 192.0),
                &programs,
                program_index,
                &mut self.program_scroll_first,
                &mut self.program_scroll_remainder,
            ) {
                self.select_program(program);
            }
        });

        retro_group(ui, rect(424.0, 166.0, 154.0, 74.0), "POLYPHONY", |ui| {
            let polyphony_response = retro_number_field(
                ui,
                rect(0.0, 6.0, 92.0, 30.0),
                "polyphony-number",
                &mut self.polyphony_text,
                snapshot.maximum_polyphony,
                MINIMUM_POLYPHONY,
                MAXIMUM_POLYPHONY,
            );
            self.polyphony_text_focused = polyphony_response.focused;
            if let Some(polyphony) = polyphony_response.value {
                state.maximum_polyphony = polyphony;
                changed = true;
            }
            if retro_step_button(ui, rect(98.0, 6.0, 28.0, 14.0), true).clicked() {
                state.maximum_polyphony = snapshot
                    .maximum_polyphony
                    .saturating_add(1)
                    .min(MAXIMUM_POLYPHONY);
                self.polyphony_text = state.maximum_polyphony.to_string();
                self.polyphony_text_focused = false;
                changed = true;
            }
            if retro_step_button(ui, rect(98.0, 22.0, 28.0, 14.0), false).clicked() {
                state.maximum_polyphony = snapshot
                    .maximum_polyphony
                    .saturating_sub(1)
                    .max(MINIMUM_POLYPHONY);
                self.polyphony_text = state.maximum_polyphony.to_string();
                self.polyphony_text_focused = false;
                changed = true;
            }
        });

        retro_group(
            ui,
            rect(424.0, 258.0, 376.0, 56.0),
            "REVERB DRY / WET",
            |ui| {
                if let Some(next) = retro_slider(
                    ui,
                    "reverb-wet",
                    rect(54.0, 9.0, 208.0, 20.0),
                    snapshot.reverb_wet,
                ) {
                    state.reverb_wet = next.unwrap_or(0.0);
                    changed = true;
                }
                retro_text(ui, pos(0.0, 10.0), "DRY", 14.0, retro::TEXT, true);
                retro_text(ui, pos(270.0, 10.0), "WET", 14.0, retro::TEXT, true);
                retro_text(
                    ui,
                    pos(306.0, 7.0),
                    &format!("{:.0}%", snapshot.reverb_wet * 100.0),
                    18.0,
                    retro::DISPLAY,
                    true,
                );
            },
        );

        retro_group(
            ui,
            rect(424.0, 324.0, 376.0, 56.0),
            "CHORUS DRY / WET",
            |ui| {
                if let Some(next) = retro_slider(
                    ui,
                    "chorus-wet",
                    rect(54.0, 9.0, 208.0, 20.0),
                    snapshot.chorus_wet,
                ) {
                    state.chorus_wet = next.unwrap_or(0.0);
                    changed = true;
                }
                retro_text(ui, pos(0.0, 10.0), "DRY", 14.0, retro::TEXT, true);
                retro_text(ui, pos(270.0, 10.0), "WET", 14.0, retro::TEXT, true);
                retro_text(
                    ui,
                    pos(306.0, 7.0),
                    &format!("{:.0}%", snapshot.chorus_wet * 100.0),
                    18.0,
                    retro::DISPLAY,
                    true,
                );
            },
        );

        retro_group(
            ui,
            rect(424.0, 390.0, 376.0, 56.0),
            "OUTPUT GAIN (DB)",
            |ui| {
                let value = normalize_output_gain(snapshot.output_gain);
                if let Some(next) =
                    retro_slider(ui, "output-gain", rect(54.0, 9.0, 208.0, 20.0), value)
                {
                    state.output_gain = next
                        .map(denormalize_output_gain)
                        .unwrap_or_else(|| output_gain_from_db(0.0));
                    changed = true;
                }
                retro_text(ui, pos(0.0, 10.0), "-24", 14.0, retro::TEXT, true);
                retro_text(ui, pos(270.0, 10.0), "+12", 14.0, retro::TEXT, true);
                retro_text(
                    ui,
                    pos(306.0, 7.0),
                    &format_output_gain_number(output_gain_to_db(snapshot.output_gain)),
                    18.0,
                    retro::DISPLAY,
                    true,
                );
            },
        );

        if self.soundfont_dropdown_open
            && let Some(index) = retro_choice_list(
                ui,
                rect(38.0, 128.0, 350.0, 96.0),
                &soundfont_names,
                selected_soundfont,
                "soundfont-dropdown",
            )
        {
            self.select_soundfont(index);
            self.soundfont_dropdown_open = false;
        }

        if changed {
            self.apply_state(state);
        }

        ui.ctx().request_repaint();
    }
}
