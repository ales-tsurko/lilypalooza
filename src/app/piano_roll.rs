use std::collections::HashMap;

use iced::{
    Color,
    Element,
    Fill,
    Length,
    Pixels,
    Point,
    Rectangle,
    Renderer,
    Size,
    Theme,
    alignment,
    mouse,
    widget::{
        button,
        canvas,
        canvas::Cache,
        container,
        mouse_area,
        row,
        scrollable,
        slider,
        stack,
        svg,
        text,
        text_input,
        tooltip,
    },
};
use iced_aw::helpers::color_picker_with_change;

use super::{Lilypalooza, Message, PianoRollMessage, dock_view::HeaderControlGroup};
use crate::{
    fonts,
    icons,
    midi::{MidiNote, MidiRollData, MidiRollFile, MidiTrack, TimeSignatureChange},
    settings::PianoRollViewSettings,
    ui_style,
};

mod grid_and_labels;
mod roll_canvas;
mod state_and_controls;
mod track_list_and_tempo;

pub(in crate::app) use grid_and_labels::adjacent_subdivision_tick;
use grid_and_labels::*;
use roll_canvas::*;
use state_and_controls::*;
pub(in crate::app) use state_and_controls::{
    PianoRollState,
    TrackMixState,
    content,
    controls,
    roll_scroll_id,
    track_list_scroll_id,
    track_list_scroll_y,
};
use track_list_and_tempo::*;
#[cfg(test)]
mod tests {
    use super::{
        BAR_LABEL_BOTTOM_PADDING,
        HEADER_CONTROL_HEIGHT,
        KEYBOARD_WIDTH,
        PianoRollState,
        REWIND_FLAG_BANNER_HEIGHT,
        REWIND_FLAG_HITBOX_WIDTH,
        REWIND_FLAG_WIDTH,
        TEMPO_LABEL_TOP_PADDING,
        TEMPO_LANE_HEIGHT,
        TRACK_BUTTON_HEIGHT,
        TRACK_BUTTON_WIDTH,
        TRACK_COLOR_BUTTON_GAP,
        TRACK_COLOR_BUTTON_SIZE,
        TRACK_LABEL_BUTTON_GAP,
        TRACK_LIST_SCROLLBAR_GUTTER,
        TRACK_PANEL_DEFAULT_WIDTH,
        TRACK_PANEL_MAX_WIDTH,
        TRACK_PANEL_MIN_WIDTH,
        TRACK_RESIZE_HANDLE_WIDTH,
        TRACK_ROW_HEIGHT,
        TrackMixState,
        adjacent_subdivision_tick,
        snap_tick_to_subdivision_grid,
        track_visibility_alpha,
    };
    use crate::{
        midi::{MidiRollData, TimeSignatureChange},
        settings::PianoRollViewSettings,
    };

    fn is_grid_multiple(value: f32) -> bool {
        ((value / 4.0).round() - (value / 4.0)).abs() < 1.0e-4
    }

    #[test]
    fn fixed_piano_roll_sizes_follow_four_px_grid() {
        for value in [
            TRACK_PANEL_DEFAULT_WIDTH,
            TRACK_PANEL_MIN_WIDTH,
            TRACK_PANEL_MAX_WIDTH,
            TRACK_RESIZE_HANDLE_WIDTH,
            TRACK_COLOR_BUTTON_SIZE,
            TRACK_COLOR_BUTTON_GAP,
            TRACK_BUTTON_WIDTH,
            TRACK_BUTTON_HEIGHT,
            TRACK_ROW_HEIGHT,
            f32::from(TRACK_LIST_SCROLLBAR_GUTTER),
            TRACK_LABEL_BUTTON_GAP,
            KEYBOARD_WIDTH,
            TEMPO_LANE_HEIGHT,
            REWIND_FLAG_HITBOX_WIDTH,
            REWIND_FLAG_WIDTH,
            REWIND_FLAG_BANNER_HEIGHT,
            TEMPO_LABEL_TOP_PADDING,
            BAR_LABEL_BOTTOM_PADDING,
        ] {
            assert!(is_grid_multiple(value), "{value} should use the 4px grid");
        }
    }

    #[test]
    fn track_list_min_width_increases_by_two_grid_units() {
        crate::test_assertions::assert_float_eq!(TRACK_PANEL_MIN_WIDTH, 124.0);
    }

    #[test]
    fn track_row_controls_are_smaller_than_track_height() {
        let track_row_height = std::hint::black_box(TRACK_ROW_HEIGHT);
        assert!(std::hint::black_box(TRACK_COLOR_BUTTON_SIZE) < track_row_height);
        assert!(std::hint::black_box(TRACK_BUTTON_HEIGHT) < track_row_height);
        assert!(std::hint::black_box(TRACK_BUTTON_WIDTH) < track_row_height);
    }

    #[test]
    fn external_solo_dims_visible_tracks() {
        let track_mix = vec![TrackMixState::default(), TrackMixState::default()];

        assert!(track_visibility_alpha(&track_mix, 0, true) < 0.20);
        assert!(track_visibility_alpha(&track_mix, 1, true) < 0.20);
    }

    #[test]
    fn local_solo_keeps_soloed_track_visible() {
        let track_mix = vec![
            TrackMixState {
                muted: false,
                soloed: false,
            },
            TrackMixState {
                muted: false,
                soloed: true,
            },
        ];

        assert!(track_visibility_alpha(&track_mix, 0, false) < 0.20);
        crate::test_assertions::assert_float_eq!(track_visibility_alpha(&track_mix, 1, false), 1.0);
    }

    #[test]
    fn piano_roll_default_track_panel_width_uses_grid() {
        let state = PianoRollState::new(PianoRollViewSettings::default());
        assert!(is_grid_multiple(state.track_panel_width));
    }

    #[test]
    fn header_controls_use_shared_pane_header_height() {
        crate::test_assertions::assert_float_eq!(
            HEADER_CONTROL_HEIGHT,
            crate::app::dock_view::HEADER_CONTROL_HEIGHT
        );
    }

    #[test]
    fn snap_tick_to_subdivision_grid_uses_nearest_subdivision() {
        let data = midi_roll_data(960, vec![]);

        assert_eq!(snap_tick_to_subdivision_grid(&data, 4, 118), 120);
        assert_eq!(snap_tick_to_subdivision_grid(&data, 8, 61), 60);
    }

    #[test]
    fn adjacent_subdivision_tick_moves_across_signature_spans() {
        let data = midi_roll_data(
            1200,
            vec![TimeSignatureChange {
                tick: 480,
                numerator: 6,
                denominator: 8,
            }],
        );

        assert_eq!(adjacent_subdivision_tick(&data, 2, 470, true), 480);
        assert_eq!(adjacent_subdivision_tick(&data, 2, 490, false), 480);
    }

    #[test]
    fn adjacent_subdivision_tick_stays_at_edge_without_candidate() {
        let data = midi_roll_data(960, vec![]);

        assert_eq!(adjacent_subdivision_tick(&data, 4, 0, false), 0);
        assert_eq!(adjacent_subdivision_tick(&data, 4, 960, true), 960);
    }

    fn midi_roll_data(total_ticks: u64, time_signatures: Vec<TimeSignatureChange>) -> MidiRollData {
        MidiRollData {
            ppq: 480,
            total_ticks,
            notes: Vec::new(),
            tracks: Vec::new(),
            time_signatures,
            tempo_changes: Vec::new(),
            bar_lines: vec![0],
            min_pitch: 21,
            max_pitch: 108,
        }
    }
}
