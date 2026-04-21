use serde::{Deserialize, Serialize};

use crate::instrument::{
    EffectProcessor, ParameterDescriptor, Processor, ProcessorDescriptor, ProcessorState,
    ProcessorStateError,
};

const MIN_GAIN_DB: f32 = -60.0;
const MAX_GAIN_DB: f32 = 12.0;
const GAIN_RANGE_DB: f32 = MAX_GAIN_DB - MIN_GAIN_DB;
const DEFAULT_GAIN_DB: f32 = 0.0;
const DEFAULT_GAIN_NORMALIZED: f32 = (DEFAULT_GAIN_DB - MIN_GAIN_DB) / GAIN_RANGE_DB;

const GAIN_EFFECT_PARAMS: &[ParameterDescriptor] = &[ParameterDescriptor {
    id: "gain_db",
    name: "Gain",
    default: DEFAULT_GAIN_NORMALIZED,
}];

const GAIN_EFFECT_DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
    name: "Gain",
    params: GAIN_EFFECT_PARAMS,
    editor: None,
};

pub(crate) fn descriptor() -> &'static ProcessorDescriptor {
    &GAIN_EFFECT_DESCRIPTOR
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct GainEffectState {
    gain_db: f32,
}

impl Default for GainEffectState {
    fn default() -> Self {
        Self {
            gain_db: DEFAULT_GAIN_DB,
        }
    }
}

pub(crate) struct GainEffectProcessor {
    state: GainEffectState,
}

impl GainEffectProcessor {
    pub(crate) fn from_state(state: &ProcessorState) -> Result<Self, ProcessorStateError> {
        let state = if state.0.is_empty() {
            GainEffectState::default()
        } else {
            bincode::deserialize(&state.0)
                .map_err(|err| ProcessorStateError::Decode(err.to_string()))?
        };
        Ok(Self { state })
    }
}

impl Processor for GainEffectProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        descriptor()
    }

    fn set_param(&mut self, id: &str, normalized: f32) -> bool {
        if id != "gain_db" {
            return false;
        }
        self.state.gain_db = MIN_GAIN_DB + normalized.clamp(0.0, 1.0) * GAIN_RANGE_DB;
        true
    }

    fn get_param(&self, id: &str) -> Option<f32> {
        (id == "gain_db")
            .then_some(((self.state.gain_db - MIN_GAIN_DB) / GAIN_RANGE_DB).clamp(0.0, 1.0))
    }

    fn save_state(&self) -> ProcessorState {
        ProcessorState(bincode::serialize(&self.state).unwrap_or_default())
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        *self = Self::from_state(state)?;
        Ok(())
    }

    fn reset(&mut self) {}
}

impl EffectProcessor for GainEffectProcessor {
    fn process(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        let gain = knyst::db_to_amplitude(self.state.gain_db);
        for frame in 0..out_left.len() {
            out_left[frame] = in_left[frame] * gain;
            out_right[frame] = in_right[frame] * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GAIN_RANGE_DB, GainEffectProcessor, MIN_GAIN_DB};
    use crate::instrument::{EffectProcessor, Processor, ProcessorState};

    #[test]
    fn gain_effect_scales_expected_signal() {
        let mut processor =
            GainEffectProcessor::from_state(&ProcessorState::default()).expect("processor");
        processor.set_param("gain_db", (-6.0 - MIN_GAIN_DB) / GAIN_RANGE_DB);

        let left_in = [0.0, 0.25, -0.5, 1.0];
        let right_in = [1.0, -0.5, 0.25, 0.0];
        let mut left_out = [0.0; 4];
        let mut right_out = [0.0; 4];
        processor.process(&left_in, &right_in, &mut left_out, &mut right_out);

        let gain = knyst::db_to_amplitude(-6.0);
        for index in 0..left_in.len() {
            assert!((left_out[index] - left_in[index] * gain).abs() < 1.0e-6);
            assert!((right_out[index] - right_in[index] * gain).abs() < 1.0e-6);
        }
    }
}
