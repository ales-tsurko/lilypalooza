use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use serde::{Deserialize, Serialize};

use crate::instrument::{
    InstrumentProcessor, MidiEvent, ParamValue, ParameterDescriptor, Processor,
    ProcessorDescriptor, ProcessorState, ProcessorStateError,
};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundfontProcessorState {
    /// Shared SoundFont resource identifier.
    pub soundfont_id: String,
    /// MIDI bank.
    pub bank: u16,
    /// MIDI program.
    pub program: u8,
}

impl Default for SoundfontProcessorState {
    fn default() -> Self {
        Self {
            soundfont_id: "default".to_string(),
            bank: 0,
            program: 0,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SoundfontSynthError {
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
pub struct SoundfontSynthSettings {
    pub sample_rate: i32,
    pub block_size: usize,
    pub maximum_polyphony: usize,
}

impl SoundfontSynthSettings {
    #[must_use]
    pub fn new(sample_rate: i32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            maximum_polyphony: 64,
        }
    }
}

#[derive(Debug)]
pub(crate) struct LoadedSoundfont {
    pub(crate) path: PathBuf,
    pub(crate) soundfont: Arc<SoundFont>,
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
        Ok(Self {
            path: resource.path.clone(),
            soundfont: Arc::new(soundfont),
        })
    }
}

#[derive(Debug)]
pub struct SoundfontProcessor {
    synthesizer: Synthesizer,
    state: SoundfontProcessorState,
    shared_program: Option<SharedSoundfontProgramState>,
    applied_shared_revision: u32,
    needs_render: bool,
    silent_blocks: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct SharedSoundfontProgramState {
    inner: Arc<SharedSoundfontProgramStateInner>,
}

#[derive(Debug)]
struct SharedSoundfontProgramStateInner {
    bank: AtomicU16,
    program: AtomicU32,
    revision: AtomicU32,
}

impl SharedSoundfontProgramState {
    pub(crate) fn new(bank: u16, program: u8) -> Self {
        Self {
            inner: Arc::new(SharedSoundfontProgramStateInner {
                bank: AtomicU16::new(bank),
                program: AtomicU32::new(u32::from(program)),
                revision: AtomicU32::new(1),
            }),
        }
    }

    pub(crate) fn update(&self, bank: u16, program: u8) {
        self.inner.bank.store(bank, Ordering::Relaxed);
        self.inner
            .program
            .store(u32::from(program), Ordering::Relaxed);
        self.inner.revision.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u16, u8, u32) {
        (
            self.inner.bank.load(Ordering::Relaxed),
            self.inner.program.load(Ordering::Relaxed) as u8,
            self.inner.revision.load(Ordering::Relaxed),
        )
    }
}

const SOUNDFONT_PARAMS: &[ParameterDescriptor] = &[
    ParameterDescriptor {
        id: "soundfont_id",
        name: "SoundFont",
    },
    ParameterDescriptor {
        id: "bank",
        name: "Bank",
    },
    ParameterDescriptor {
        id: "program",
        name: "Program",
    },
];

const SOUNDFONT_DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
    name: "SoundFont",
    params: SOUNDFONT_PARAMS,
};

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
        shared_program: Option<SharedSoundfontProgramState>,
    ) -> Result<Self, SoundfontSynthError> {
        let mut synth_settings = SynthesizerSettings::new(settings.sample_rate);
        synth_settings.block_size = settings.block_size;
        synth_settings.maximum_polyphony = settings.maximum_polyphony;
        synth_settings.enable_reverb_and_chorus = false;
        let synthesizer = Synthesizer::new(soundfont, &synth_settings).map_err(|source| {
            SoundfontSynthError::CreateSynth {
                id: state.soundfont_id.clone(),
                source,
            }
        })?;
        let applied_shared_revision = shared_program
            .as_ref()
            .map_or(0, |shared| shared.snapshot().2);
        let mut processor = Self {
            synthesizer,
            state,
            shared_program,
            applied_shared_revision,
            needs_render: false,
            silent_blocks: 0,
        };
        processor.apply_program();
        Ok(processor)
    }

    pub fn decode_state(
        state: &ProcessorState,
    ) -> Result<SoundfontProcessorState, ProcessorStateError> {
        bincode::deserialize(&state.0)
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
        self.needs_render = false;
        self.silent_blocks = 0;
    }

    fn sync_shared_program(&mut self) {
        let Some(shared) = &self.shared_program else {
            return;
        };
        let (bank, program, revision) = shared.snapshot();
        if revision == self.applied_shared_revision {
            return;
        }
        self.state.bank = bank;
        self.state.program = program;
        self.applied_shared_revision = revision;
        self.apply_program();
    }
}

impl Processor for SoundfontProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        &SOUNDFONT_DESCRIPTOR
    }

    fn set_param(&mut self, id: &str, value: ParamValue) {
        match (id, value) {
            ("soundfont_id", ParamValue::Text(soundfont_id)) => {
                self.state.soundfont_id = soundfont_id;
            }
            ("bank", ParamValue::Int(bank)) => {
                self.state.bank = bank.clamp(0, 16_383) as u16;
                self.apply_program();
            }
            ("program", ParamValue::Int(program)) => {
                self.state.program = program.clamp(0, 127) as u8;
                self.apply_program();
            }
            _ => {}
        }
    }

    fn save_state(&self) -> ProcessorState {
        encode_soundfont_state(&self.state)
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        self.state = Self::decode_state(state)?;
        self.apply_program();
        Ok(())
    }

    fn reset(&mut self) {
        self.apply_program();
    }
}

impl InstrumentProcessor for SoundfontProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        self.sync_shared_program();
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
                if !matches!(controller, 0 | 32) {
                    self.synthesizer.process_midi_message(
                        Self::TRACK_CHANNEL,
                        0xB0,
                        i32::from(controller),
                        i32::from(value),
                    );
                }
            }
            MidiEvent::ProgramChange { .. } => {}
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
                let midi_value = (i32::from(value) + 8192).clamp(0, 16_383);
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
        self.sync_shared_program();
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
    ProcessorState(bincode::serialize(state).expect("soundfont state serialization should succeed"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use super::{
        LoadedSoundfont, SoundfontProcessor, SoundfontProcessorState, SoundfontSynthSettings,
    };
    use crate::instrument::{InstrumentProcessor, MidiEvent, Processor};
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
