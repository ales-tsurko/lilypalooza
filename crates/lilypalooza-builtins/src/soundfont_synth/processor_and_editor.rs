use super::{model_and_runtime::*, *};

impl SoundfontEditorApp {
    pub(crate) fn select_soundfont(&self, index: usize) {
        let Some(entry) = self.shared.catalog.get(index) else {
            return;
        };

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
        self.apply_state(state);
    }

    pub(crate) fn select_bank(&self, bank: u16) {
        let (snapshot, _) = self.shared.state.snapshot();
        let soundfont_index =
            selected_soundfont_index(&self.shared.catalog, &snapshot.soundfont_id);
        let Some(entry) = self.shared.catalog.get(soundfont_index) else {
            return;
        };
        let Some(preset) = entry
            .presets
            .iter()
            .find(|preset| preset.bank == bank && preset.program == snapshot.program)
            .or_else(|| entry.presets.iter().find(|preset| preset.bank == bank))
        else {
            return;
        };

        let mut state = snapshot;
        state.bank = preset.bank;
        state.program = preset.program;
        self.apply_state(state);
    }

    pub(crate) fn select_program(&self, program: u8) {
        let (snapshot, _) = self.shared.state.snapshot();
        let soundfont_index =
            selected_soundfont_index(&self.shared.catalog, &snapshot.soundfont_id);
        let Some(entry) = self.shared.catalog.get(soundfont_index) else {
            return;
        };
        let Some(preset) = entry
            .presets
            .iter()
            .find(|preset| preset.bank == snapshot.bank && preset.program == program)
        else {
            return;
        };

        let mut state = snapshot;
        state.bank = preset.bank;
        state.program = preset.program;
        self.apply_state(state);
    }

    pub(crate) fn apply_state(&self, state: SoundfontProcessorState) {
        if let Err(error) = self.shared.apply_state(state) {
            eprintln!("SoundFont editor state update failed: {error}");
        }
    }
}

pub(crate) struct ProgramChoice {
    pub(crate) program: u8,
    pub(crate) label: String,
}

pub(crate) fn bank_numbers(catalog: &[SoundfontCatalogEntry], soundfont_index: usize) -> Vec<u16> {
    catalog
        .get(soundfont_index)
        .map(|entry| {
            let mut banks = entry
                .presets
                .iter()
                .map(|preset| preset.bank)
                .collect::<Vec<_>>();
            banks.dedup();
            banks
        })
        .unwrap_or_default()
}

pub(crate) fn program_choices(
    catalog: &[SoundfontCatalogEntry],
    soundfont_index: usize,
    bank: u16,
) -> Vec<ProgramChoice> {
    catalog
        .get(soundfont_index)
        .map(|entry| {
            entry
                .presets
                .iter()
                .filter(|preset| preset.bank == bank)
                .map(|preset| ProgramChoice {
                    program: preset.program,
                    label: format!("{:03} {}", preset.program, preset.name),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn selected_soundfont_index(
    catalog: &[SoundfontCatalogEntry],
    soundfont_id: &str,
) -> usize {
    catalog
        .iter()
        .position(|entry| entry.id == soundfont_id)
        .unwrap_or(0)
}

pub(crate) fn selected_bank_index(banks: &[u16], bank: u16) -> usize {
    banks
        .iter()
        .position(|candidate| *candidate == bank)
        .unwrap_or(0)
}

pub(crate) fn selected_program_index(programs: &[ProgramChoice], program: u8) -> usize {
    programs
        .iter()
        .position(|candidate| candidate.program == program)
        .unwrap_or(0)
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

impl SoundfontProcessor {
    const TRACK_CHANNEL: i32 = 0;

    #[cfg(test)]
    pub(super) fn new(
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
        let initial_output_gain = state.output_gain;
        let mut synthesizer = build_synthesizer(soundfont, settings, &state)?;
        synthesizer.set_master_volume(1.0);
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
            output_gain: SmoothedAudioValue::new(
                initial_output_gain,
                usize::try_from(settings.sample_rate).unwrap_or(44_100),
            ),
        };
        processor.apply_program();
        Ok(processor)
    }

    /// Decodes typed SoundFont state from the processor state blob stored in slots.
    pub(super) fn decode_state(
        state: &ProcessorState,
    ) -> Result<SoundfontProcessorState, ProcessorStateError> {
        decode_state(state)
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
        self.synthesizer.set_master_volume(1.0);
        self.output_gain.set_target(self.state.output_gain);
        self.apply_effect_mix();
        self.needs_render = false;
        self.silent_blocks = 0;
    }

    fn apply_effect_mix(&mut self) {
        self.synthesizer.process_midi_message(
            Self::TRACK_CHANNEL,
            0xB0,
            MIDI_CC_REVERB_WET,
            midi_control_value(self.state.reverb_wet),
        );
        self.synthesizer.process_midi_message(
            Self::TRACK_CHANNEL,
            0xB0,
            MIDI_CC_CHORUS_WET,
            midi_control_value(self.state.chorus_wet),
        );
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
        let change = SoundfontSharedChange::between(&state, &self.state);
        self.state = state;
        self.applied_shared_revision = revision;
        self.apply_shared_change(change);
    }

    fn apply_shared_change(&mut self, change: SoundfontSharedChange) {
        if change.rebuild_needed {
            self.rebuild_synth();
            return;
        }
        if change.program_or_follow_changed {
            self.apply_program();
            return;
        }
        if change.gain_changed {
            self.output_gain.set_target(self.state.output_gain);
        }
        if change.mix_changed {
            self.apply_effect_mix();
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
                self.state.bank = normalized_to_u16(normalized, MIDI_14BIT_MAX);
                self.apply_program();
                true
            }
            "program" => {
                self.state.program = normalized_to_u8(normalized, MIDI_PROGRAM_MAX);
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
            "output_gain" => {
                self.state.output_gain = denormalize_output_gain(normalized);
                self.output_gain.set_target(self.state.output_gain);
                true
            }
            "reverb_wet" => {
                self.state.reverb_wet = normalized.clamp(0.0, 1.0);
                self.apply_effect_mix();
                true
            }
            "chorus_wet" => {
                self.state.chorus_wet = normalized.clamp(0.0, 1.0);
                self.apply_effect_mix();
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
            "output_gain" => Some(normalize_output_gain(self.state.output_gain)),
            "reverb_wet" => Some(self.state.reverb_wet.clamp(0.0, 1.0)),
            "chorus_wet" => Some(self.state.chorus_wet.clamp(0.0, 1.0)),
            _ => None,
        }
    }

    fn save_state(&self) -> ProcessorState {
        encode_state(&self.state)
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
        if let Some(shared) = &self.shared_state {
            shared.mark_midi_activity();
        }
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
                self.handle_control_change(controller, value);
            }
            MidiEvent::ProgramChange { program, .. } => self.handle_program_change(program),
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
        self.output_gain.set_target(self.state.output_gain);
        for (left_sample, right_sample) in left.iter_mut().zip(right.iter_mut()) {
            let gain = self.output_gain.next_sample();
            *left_sample *= gain;
            *right_sample *= gain;
        }
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

impl SoundfontProcessor {
    fn handle_control_change(&mut self, controller: u8, value: u8) {
        if self.state.follow_midi && controller == 0 {
            self.state.bank = u16::from(value);
            self.publish_followed_program();
            self.apply_program();
        }
        if controller != 32 {
            self.synthesizer.process_midi_message(
                Self::TRACK_CHANNEL,
                0xB0,
                i32::from(controller),
                i32::from(value),
            );
        }
    }

    fn handle_program_change(&mut self, program: u8) {
        if !self.state.follow_midi {
            return;
        }
        self.state.program = program;
        self.publish_followed_program();
        self.apply_program();
    }

    fn publish_followed_program(&mut self) {
        if let Some(shared) = &self.shared_state {
            shared.update_bank_program(self.state.bank, self.state.program);
            self.applied_shared_revision = shared.snapshot().1;
        }
    }
}

pub(crate) fn normalize_polyphony(value: u16) -> f32 {
    let span = f32::from(MAXIMUM_POLYPHONY - MINIMUM_POLYPHONY);
    f32::from(value.clamp(MINIMUM_POLYPHONY, MAXIMUM_POLYPHONY) - MINIMUM_POLYPHONY) / span
}

pub(crate) fn denormalize_polyphony(normalized: f32) -> u16 {
    let span = f32::from(MAXIMUM_POLYPHONY - MINIMUM_POLYPHONY);
    MINIMUM_POLYPHONY
        + (normalized.clamp(0.0, 1.0) * span)
            .round()
            .to_u16()
            .unwrap_or(MAXIMUM_POLYPHONY - MINIMUM_POLYPHONY)
}

pub(crate) fn midi_control_value(normalized: f32) -> i32 {
    (normalized.clamp(0.0, 1.0) * MIDI_CONTROL_MAX).round() as i32
}

pub(crate) fn output_gain_to_db(linear: f32) -> f32 {
    (20.0 * linear.max(0.0).log10()).clamp(MIN_OUTPUT_GAIN_DB, MAX_OUTPUT_GAIN_DB)
}

pub(crate) fn output_gain_from_db(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(MIN_OUTPUT_GAIN_DB, MAX_OUTPUT_GAIN_DB) / 20.0)
}

pub(crate) fn normalize_output_gain(linear: f32) -> f32 {
    (output_gain_to_db(linear) - MIN_OUTPUT_GAIN_DB) / (MAX_OUTPUT_GAIN_DB - MIN_OUTPUT_GAIN_DB)
}

pub(crate) fn denormalize_output_gain(normalized: f32) -> f32 {
    let db =
        MIN_OUTPUT_GAIN_DB + normalized.clamp(0.0, 1.0) * (MAX_OUTPUT_GAIN_DB - MIN_OUTPUT_GAIN_DB);
    output_gain_from_db(db)
}

#[cfg(test)]
pub(crate) fn format_output_gain_db(db: f32) -> String {
    format!("{} dB", format_output_gain_number(db))
}

pub(crate) fn format_output_gain_number(db: f32) -> String {
    if db.abs() < 0.05 {
        "0.0".to_string()
    } else if db > 0.0 {
        format!("+{db:.1}")
    } else {
        format!("{db:.1}")
    }
}

pub(crate) fn build_synthesizer(
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
    synth_settings.enable_reverb_and_chorus = true;
    Synthesizer::new(soundfont, &synth_settings).map_err(|source| {
        SoundfontSynthError::CreateSynth {
            id: state.soundfont_id.clone(),
            source,
        }
    })
}

#[cfg(test)]
pub(crate) fn soundfont_presets(soundfont: &SoundFont) -> Vec<SoundfontPreset> {
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
