//! Mixer track primitives.

/// Stable mixer track identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub u16);

/// Basic track state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackState {
    /// Linear gain.
    pub gain: f32,
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
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }
    }
}
