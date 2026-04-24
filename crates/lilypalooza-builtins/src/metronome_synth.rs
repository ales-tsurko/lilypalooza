use lilypalooza_audio::{ParameterDescriptor, ProcessorDescriptor};

const MIN_GAIN_DB: f32 = -36.0;
const MAX_GAIN_DB: f32 = 6.0;
const GAIN_RANGE_DB: f32 = MAX_GAIN_DB - MIN_GAIN_DB;
const DEFAULT_GAIN_DB: f32 = -12.0;
const DEFAULT_GAIN_NORMALIZED: f32 = (DEFAULT_GAIN_DB - MIN_GAIN_DB) / GAIN_RANGE_DB;
const DEFAULT_PITCH: f32 = 0.5;

const PARAMS: &[ParameterDescriptor] = &[
    ParameterDescriptor {
        id: "gain_db",
        name: "Gain",
        default: DEFAULT_GAIN_NORMALIZED,
    },
    ParameterDescriptor {
        id: "pitch",
        name: "Pitch",
        default: DEFAULT_PITCH,
    },
];

pub(crate) const DESCRIPTOR: &ProcessorDescriptor = &ProcessorDescriptor {
    name: "Metronome",
    params: PARAMS,
    editor: None,
};
