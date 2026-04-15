//! Instrument abstractions for mixer tracks.

/// Track instrument configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentConfig {
    /// Which instrument backend this track uses.
    pub kind: InstrumentKind,
}

impl Default for InstrumentConfig {
    fn default() -> Self {
        Self {
            kind: InstrumentKind::SoundFont {
                soundfont_id: "default".to_string(),
                bank: 0,
                program: 0,
            },
        }
    }
}

/// Supported instrument backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentKind {
    /// Shared SoundFont resource with per-track bank/program selection.
    SoundFont {
        /// Shared soundfont resource identifier.
        soundfont_id: String,
        /// MIDI bank.
        bank: u16,
        /// MIDI program.
        program: u8,
    },
    /// Built-in sampler or synth instrument.
    BuiltIn {
        /// Engine-defined instrument identifier.
        instrument_id: String,
    },
    /// Hosted external plugin instrument.
    Plugin {
        /// Engine-defined plugin instance identifier.
        plugin_id: String,
    },
}
