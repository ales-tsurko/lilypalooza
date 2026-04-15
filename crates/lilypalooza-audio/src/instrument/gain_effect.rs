use crate::instrument::{
    EffectProcessor, ParamValue, ParameterDescriptor, Processor, ProcessorDescriptor,
    ProcessorState, ProcessorStateError,
};
use serde::{Deserialize, Serialize};

const GAIN_EFFECT_PARAMS: &[ParameterDescriptor] = &[ParameterDescriptor {
    id: "gain_db",
    name: "Gain",
}];

const GAIN_EFFECT_DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
    name: "Gain",
    params: GAIN_EFFECT_PARAMS,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct GainEffectState {
    gain_db: f32,
}

impl Default for GainEffectState {
    fn default() -> Self {
        Self { gain_db: 0.0 }
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
        &GAIN_EFFECT_DESCRIPTOR
    }

    fn set_param(&mut self, id: &str, value: ParamValue) {
        if id == "gain_db" {
            match value {
                ParamValue::Float(value) => self.state.gain_db = value,
                ParamValue::Int(value) => self.state.gain_db = value as f32,
                ParamValue::Bool(value) => self.state.gain_db = if value { 0.0 } else { -96.0 },
                ParamValue::Enum(_) | ParamValue::Text(_) => {}
            }
        }
    }

    fn save_state(&self) -> ProcessorState {
        ProcessorState(
            bincode::serialize(&self.state)
                .expect("gain effect state serialization should never fail"),
        )
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
