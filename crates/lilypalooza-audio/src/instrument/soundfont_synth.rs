use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

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

    pub fn new(
        soundfont: &Arc<SoundFont>,
        settings: SoundfontSynthSettings,
        state: SoundfontProcessorState,
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
        let mut processor = Self { synthesizer, state };
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
        self.synthesizer.note_off_all(true);
        self.synthesizer.reset();
    }
}

impl InstrumentProcessor for SoundfontProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn { note, velocity, .. } => {
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
            } => self.synthesizer.process_midi_message(
                Self::TRACK_CHANNEL,
                0xB0,
                i32::from(controller),
                i32::from(value),
            ),
            MidiEvent::ProgramChange { program, .. } => self.synthesizer.process_midi_message(
                Self::TRACK_CHANNEL,
                0xC0,
                i32::from(program),
                0,
            ),
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
            MidiEvent::AllSoundOff { .. } => self
                .synthesizer
                .note_off_all_channel(Self::TRACK_CHANNEL, true),
            MidiEvent::ResetAllControllers { .. } => {
                self.synthesizer
                    .process_midi_message(Self::TRACK_CHANNEL, 0xB0, 121, 0)
            }
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.synthesizer.render(left, right);
    }
}

pub(crate) fn encode_soundfont_state(state: &SoundfontProcessorState) -> ProcessorState {
    ProcessorState(bincode::serialize(state).expect("soundfont state serialization should succeed"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
}
