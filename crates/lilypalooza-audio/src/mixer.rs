//! Fixed mixer configuration.

use crate::instrument::InstrumentConfig;
use crate::track::{TrackId, TrackState};

/// Fixed mixer configuration.
#[derive(Debug, Clone)]
pub struct MixerConfig {
    /// Preallocated mixer tracks.
    pub tracks: Vec<MixerTrackConfig>,
}

impl MixerConfig {
    /// Creates a fixed-size mixer.
    #[must_use]
    pub fn with_track_count(track_count: usize) -> Self {
        let tracks = (0..track_count)
            .map(|index| MixerTrackConfig {
                id: TrackId(index as u16),
                state: TrackState::default(),
                instrument: InstrumentConfig::default(),
            })
            .collect();

        Self { tracks }
    }
}

/// Mixer track configuration.
#[derive(Debug, Clone)]
pub struct MixerTrackConfig {
    /// Stable mixer track identifier.
    pub id: TrackId,
    /// Track state.
    pub state: TrackState,
    /// Track instrument.
    pub instrument: InstrumentConfig,
}
