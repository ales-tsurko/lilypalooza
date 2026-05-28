//! Built-in Lilypalooza processors.

mod gain_effect;
mod metronome_synth;
/// Built-in SoundFont synth state helpers and runtime.
pub mod soundfont_synth;

use lilypalooza_audio::{
    BUILTIN_GAIN_ID,
    BUILTIN_METRONOME_ID,
    BUILTIN_SOUNDFONT_ID,
    instrument::registry::{self, Entry, RuntimeFactory},
};

/// Registers all built-in processors with the host registry.
pub fn register_all() {
    registry::register([
        Entry::built_in_processor(
            BUILTIN_SOUNDFONT_ID,
            "SF-01",
            soundfont_synth::DESCRIPTOR,
            RuntimeFactory::Instrument(soundfont_synth::create_runtime),
        )
        .with_category("Sampler"),
        Entry::built_in_processor(
            BUILTIN_GAIN_ID,
            "Gain",
            gain_effect::DESCRIPTOR,
            RuntimeFactory::Effect(gain_effect::create_runtime),
        )
        .with_category("Utility"),
        Entry::built_in_processor(
            BUILTIN_METRONOME_ID,
            "Metronome",
            metronome_synth::DESCRIPTOR,
            RuntimeFactory::InstrumentDescriptor,
        )
        .with_category("Utility"),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundfont_builtin_registry_name_is_sf_01() {
        register_all();
        let entry = registry::entry(BUILTIN_SOUNDFONT_ID).expect("soundfont entry should exist");

        assert_eq!(entry.name, "SF-01");
        assert_eq!(entry.descriptor.name, "SF-01");
    }
}
