//! Mixer track primitives.

use serde::{Deserialize, Serialize};

use crate::instrument::{EffectSlotState, InstrumentSlotState};

/// Number of fixed instrument tracks.
pub const INSTRUMENT_TRACK_COUNT: usize = 128;

/// Stable fixed instrument-track identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrackId(pub u16);

impl TrackId {
    /// Returns the zero-based track index.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    /// Returns `true` when the id belongs to the fixed instrument range.
    #[must_use]
    pub fn is_instrument(self) -> bool {
        self.index() < INSTRUMENT_TRACK_COUNT
    }
}

/// Stable dynamic bus identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusId(pub u16);

/// Main output routing target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrackRoute {
    /// Route directly to master.
    #[default]
    Master,
    /// Route to one mixer bus.
    Bus(BusId),
}

/// One parallel send to a bus.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BusSend {
    /// Bus destination.
    pub bus_id: BusId,
    /// Send gain in dB.
    pub gain_db: f32,
    /// Whether the send taps pre-fader.
    pub pre_fader: bool,
}

impl BusSend {
    /// Creates one bus send.
    #[must_use]
    pub fn new(bus_id: BusId, gain_db: f32, pre_fader: bool) -> Self {
        Self {
            bus_id,
            gain_db,
            pre_fader,
        }
    }
}

/// Routing state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TrackRouting {
    /// Main output route.
    pub main: TrackRoute,
    /// Parallel sends to buses.
    pub sends: Vec<BusSend>,
}

/// Shared strip state used by tracks, buses, and the master.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackState {
    /// Fader gain in dB.
    pub gain_db: f32,
    /// Pan in the range `-1.0..=1.0`.
    pub pan: f32,
    /// Mute state.
    pub muted: bool,
    /// Solo state.
    pub soloed: bool,
}

impl Default for TrackState {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }
    }
}

/// One fixed instrument track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerTrack {
    /// Stable mixer track identifier.
    pub id: TrackId,
    /// User-visible name.
    pub name: String,
    /// Strip state.
    pub state: TrackState,
    /// Routing state.
    pub routing: TrackRouting,
    /// Instrument slot.
    pub instrument: InstrumentSlotState,
    /// Effect slots in processing order.
    pub effects: Vec<EffectSlotState>,
}

impl MixerTrack {
    /// Creates a fixed instrument track from its stable id.
    #[must_use]
    pub fn new(id: TrackId) -> Self {
        Self {
            id,
            name: format!("Track {}", id.index() + 1),
            state: TrackState::default(),
            routing: TrackRouting::default(),
            instrument: InstrumentSlotState::default(),
            effects: Vec::new(),
        }
    }
}

/// One dynamic bus track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusTrack {
    /// Stable bus identifier.
    pub id: BusId,
    /// User-visible name.
    pub name: String,
    /// Strip state.
    pub state: TrackState,
    /// Routing state.
    pub routing: TrackRouting,
    /// Effect slots in processing order.
    pub effects: Vec<EffectSlotState>,
}

impl BusTrack {
    /// Creates one bus track.
    #[must_use]
    pub fn new(id: BusId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            state: TrackState::default(),
            routing: TrackRouting::default(),
            effects: Vec::new(),
        }
    }
}

/// Dedicated master track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterTrack {
    /// User-visible name.
    pub name: String,
    /// Strip state.
    pub state: TrackState,
    /// Effect slots in processing order.
    pub effects: Vec<EffectSlotState>,
}

impl Default for MasterTrack {
    fn default() -> Self {
        Self {
            name: String::from("Master"),
            state: TrackState::default(),
            effects: Vec::new(),
        }
    }
}
