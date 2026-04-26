//! Built-in Lilypalooza processors.

mod gain_effect;
mod metronome_synth;
/// Built-in SoundFont synth state helpers and runtime.
pub mod soundfont_synth;

use lilypalooza_audio::instrument::registry::{self, Entry};
use lilypalooza_audio::{BUILTIN_GAIN_ID, BUILTIN_METRONOME_ID, BUILTIN_SOUNDFONT_ID};

/// Registers all built-in processors with the host registry.
pub fn register_all() {
    registry::register([
        Entry::builtin_instrument(
            BUILTIN_SOUNDFONT_ID,
            "SF-01",
            soundfont_synth::DESCRIPTOR,
            soundfont_synth::create_runtime,
        ),
        Entry::builtin_effect(
            BUILTIN_GAIN_ID,
            "Gain",
            gain_effect::DESCRIPTOR,
            gain_effect::create_runtime,
        ),
        Entry::builtin_instrument_descriptor(
            BUILTIN_METRONOME_ID,
            "Metronome",
            metronome_synth::DESCRIPTOR,
        ),
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
