//! Mixer track primitives.

use serde::{Deserialize, Serialize};

use crate::instrument::SlotState;

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

/// Shared strip model used for instrument tracks, buses, and the master.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Stable bus identifier on bus strips.
    pub bus_id: Option<BusId>,
    /// User-visible name.
    pub name: String,
    /// Strip state.
    pub state: TrackState,
    /// Routing state.
    pub routing: TrackRouting,
    /// Processor slots in processing order.
    ///
    /// Convention:
    /// - `0` is always the instrument slot
    /// - `1..` are effect slots
    pub slots: Vec<SlotState>,
}

impl Track {
    fn new(name: impl Into<String>) -> Self {
        Self {
            bus_id: None,
            name: name.into(),
            state: TrackState::default(),
            routing: TrackRouting::default(),
            slots: vec![SlotState::default()],
        }
    }

    /// Creates a fixed instrument track from its stable id.
    #[must_use]
    pub fn instrument(id: TrackId) -> Self {
        Self::new(format!("Track {}", id.index() + 1))
    }

    /// Creates one bus track.
    #[must_use]
    pub fn bus(id: BusId, name: impl Into<String>) -> Self {
        let mut track = Self::new(name);
        track.bus_id = Some(id);
        track
    }

    /// Creates the dedicated master track.
    #[must_use]
    pub fn master() -> Self {
        Self::new("Master")
    }

    /// Returns one slot by unified index.
    #[must_use]
    pub fn slot(&self, slot_index: usize) -> Option<&SlotState> {
        self.slots.get(slot_index)
    }

    /// Returns mutable access to one slot by unified index.
    #[must_use]
    pub fn slot_mut(&mut self, slot_index: usize) -> Option<&mut SlotState> {
        self.slots.get_mut(slot_index)
    }

    /// Returns the instrument slot.
    #[must_use]
    pub fn instrument_slot(&self) -> Option<&SlotState> {
        self.slots.first()
    }

    /// Returns mutable access to the instrument slot.
    #[must_use]
    pub fn instrument_slot_mut(&mut self) -> Option<&mut SlotState> {
        self.slots.first_mut()
    }

    /// Replaces the instrument slot.
    pub fn set_instrument_slot(&mut self, instrument: SlotState) {
        if let Some(slot) = self.slots.first_mut() {
            *slot = instrument;
        } else {
            self.slots.push(instrument);
        }
    }

    /// Returns one effect slot by zero-based effect index.
    #[must_use]
    pub fn effect(&self, effect_index: usize) -> Option<&SlotState> {
        self.slots.get(effect_index + 1)
    }

    /// Returns mutable access to one effect slot by zero-based effect index.
    #[must_use]
    pub fn effect_mut(&mut self, effect_index: usize) -> Option<&mut SlotState> {
        self.slots.get_mut(effect_index + 1)
    }

    /// Returns all effect slots.
    #[must_use]
    pub fn effects(&self) -> &[SlotState] {
        &self.slots[1..]
    }

    /// Returns all effect slots as typed states.
    pub fn effect_states(&self) -> impl Iterator<Item = &SlotState> + '_ {
        self.slots.iter().skip(1)
    }

    /// Returns the number of effect slots.
    #[must_use]
    pub fn effect_count(&self) -> usize {
        self.slots.len().saturating_sub(1)
    }

    /// Replaces all effect slots.
    pub fn set_effects(&mut self, effects: Vec<SlotState>) {
        self.slots.truncate(1);
        self.slots.extend(effects);
    }

    /// Appends one effect slot.
    pub fn push_effect(&mut self, effect: SlotState) {
        self.slots.push(effect);
    }
}
