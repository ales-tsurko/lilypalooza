use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use rustysynth::SoundFont;
use serde::{Deserialize, Serialize};

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

/// Runtime SoundFont settings supplied by the audio engine.
#[derive(Debug, Clone, Copy)]
pub struct SoundfontSynthSettings {
    /// Audio sample rate.
    pub sample_rate: i32,
    /// Audio callback block size.
    pub block_size: usize,
}

impl SoundfontSynthSettings {
    /// Creates SoundFont runtime settings.
    #[must_use]
    pub fn new(sample_rate: i32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
        }
    }
}

/// Loaded SoundFont resource owned by the audio engine.
#[derive(Debug)]
pub struct LoadedSoundfont {
    /// Source path.
    pub path: PathBuf,
    /// Parsed SoundFont data.
    pub soundfont: Arc<SoundFont>,
    /// Sorted preset metadata.
    pub presets: Arc<Vec<SoundfontPreset>>,
}

impl LoadedSoundfont {
    /// Loads a SoundFont resource from disk.
    pub fn load(resource: &SoundfontResource) -> Result<Self, SoundfontSynthError> {
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
        let presets = soundfont_presets(&soundfont);
        Ok(Self {
            path: resource.path.clone(),
            soundfont: Arc::new(soundfont),
            presets: Arc::new(presets),
        })
    }
}

/// SoundFont loading errors.
#[derive(thiserror::Error, Debug)]
pub enum SoundfontSynthError {
    /// Failed to read the SoundFont file.
    #[error("failed to read soundfont `{path}`: {source}")]
    ReadFile {
        /// SoundFont path.
        path: PathBuf,
        /// Source IO error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to parse the SoundFont file.
    #[error("failed to parse soundfont `{path}`: {source}")]
    ParseFile {
        /// SoundFont path.
        path: PathBuf,
        /// Source parser error.
        #[source]
        source: rustysynth::SoundFontError,
    },
}

/// One sorted SoundFont preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundfontPreset {
    /// MIDI bank.
    pub bank: u16,
    /// MIDI program.
    pub program: u8,
    /// Preset name.
    pub name: String,
}

fn soundfont_presets(soundfont: &SoundFont) -> Vec<SoundfontPreset> {
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
            .then(left.name.cmp(&right.name))
    });
    presets
}
