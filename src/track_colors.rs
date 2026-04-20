use iced::Color;

use crate::state::TrackColorOverride;

pub(crate) fn default_track_color(track_index: usize) -> Color {
    const COLORS: [Color; 12] = [
        Color::from_rgb(0.90, 0.35, 0.35),
        Color::from_rgb(0.90, 0.62, 0.31),
        Color::from_rgb(0.88, 0.82, 0.30),
        Color::from_rgb(0.50, 0.82, 0.33),
        Color::from_rgb(0.29, 0.76, 0.49),
        Color::from_rgb(0.28, 0.75, 0.70),
        Color::from_rgb(0.29, 0.63, 0.90),
        Color::from_rgb(0.44, 0.53, 0.92),
        Color::from_rgb(0.65, 0.47, 0.92),
        Color::from_rgb(0.83, 0.41, 0.82),
        Color::from_rgb(0.86, 0.38, 0.63),
        Color::from_rgb(0.77, 0.43, 0.48),
    ];

    COLORS[track_index % COLORS.len()]
}

pub(crate) fn effective_track_color(track_index: usize, user_override: Option<Color>) -> Color {
    user_override.unwrap_or_else(|| default_track_color(track_index))
}

pub(crate) fn to_override(color: Color) -> TrackColorOverride {
    TrackColorOverride {
        r: color.r,
        g: color.g,
        b: color.b,
        a: color.a,
    }
}

pub(crate) fn from_override(color: TrackColorOverride) -> Color {
    Color::from_rgba(color.r, color.g, color.b, color.a)
}

pub(crate) fn color_hash(colors: &[Color]) -> u64 {
    let mut hash = 0u64;
    for (index, color) in colors.iter().enumerate() {
        let mix = u64::from(color.r.to_bits())
            ^ u64::from(color.g.to_bits()).rotate_left(13)
            ^ u64::from(color.b.to_bits()).rotate_left(29)
            ^ u64::from(color.a.to_bits()).rotate_left(47);
        hash ^= mix.rotate_left((index % 63) as u32);
    }
    hash
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::{default_track_color, effective_track_color, from_override, to_override};

    #[test]
    fn effective_track_color_prefers_override() {
        let override_color = Color::from_rgb(0.1, 0.2, 0.3);
        assert_eq!(
            effective_track_color(3, Some(override_color)),
            override_color
        );
        assert_eq!(effective_track_color(3, None), default_track_color(3));
    }

    #[test]
    fn color_override_roundtrip_preserves_rgba() {
        let color = Color::from_rgba(0.1, 0.2, 0.3, 0.4);
        assert_eq!(from_override(to_override(color)), color);
    }
}
