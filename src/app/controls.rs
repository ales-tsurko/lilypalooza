use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use iced::{
    Color,
    Element,
    Length,
    Pixels,
    Point,
    Radians,
    Rectangle,
    Renderer,
    Theme,
    alignment,
    keyboard,
    mouse,
    widget::{
        canvas,
        canvas::{self as canvas_widget, Path, Stroke, Text},
        container,
    },
};

use crate::ui_style;

mod geometry;
mod public_controls;

use geometry::*;
pub(super) use public_controls::*;
#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use iced::{
        Point,
        Rectangle,
        keyboard,
        mouse,
        widget::canvas::{self as canvas_widget, Program},
    };

    use super::{
        COMPACT_HORIZONTAL_SLIDER_METRICS,
        FINE_DRAG_MULTIPLIER,
        GAIN_MIN_DB,
        GAIN_SCALE_DB_MARKS,
        GAIN_SCALE_LABEL_MIN_GAP,
        GainFader,
        GainFaderState,
        HORIZONTAL_SLIDER_METRICS,
        HorizontalSlider,
        HorizontalSliderScale,
        HorizontalSliderState,
        Knob,
        KnobMode,
        gain_db_to_normalized,
        gain_db_to_normalized_with_max,
        gain_fader_handle_y,
        gain_fader_rail_bounds,
        gain_normalized_to_db,
        gain_normalized_to_db_with_max,
        gain_scale_label,
        gain_value_to_y,
        horizontal_slider_handle_x,
        is_double_click,
        visible_gain_scale_marks,
        y_to_gain_value,
    };

    fn shift_toggle_drag_values<P>(
        program: &P,
        state: &mut P::State,
        bounds: Rectangle,
        start: Point,
        first: Point,
        second: Point,
        second_value_label: &str,
    ) -> (f32, f32)
    where
        P: Program<f32>,
    {
        let _discarded = Program::update(
            program,
            state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _discarded = Program::update(
            program,
            state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(start),
        );
        let first_value = drag_value(program, state, bounds, first, "first value");
        let no_message = Program::update(
            program,
            state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::empty(),
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(no_message.is_none());
        let second_value = drag_value(program, state, bounds, second, second_value_label);
        (first_value, second_value)
    }

    fn drag_value<P>(
        program: &P,
        state: &mut P::State,
        bounds: Rectangle,
        position: Point,
        label: &str,
    ) -> f32
    where
        P: Program<f32>,
    {
        let action = Program::update(
            program,
            state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position }),
            bounds,
            mouse::Cursor::Available(position),
        )
        .unwrap_or_else(|| panic!("{label} drag action"));
        let (value, _, _) = action.into_inner();
        value.unwrap_or_else(|| panic!("{label}"))
    }

    #[test]
    fn drag_delta_moves_pan_and_clamps() {
        let knob = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        };
        assert!((knob.apply_drag_delta(-10.0, false) - 0.08).abs() < 1.0e-6);
    }

    #[test]
    fn shift_drag_delta_microtunes_pan() {
        let knob = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar { zero_epsilon: 0.0 },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        };

        let normal = knob.apply_drag_delta(-10.0, false);
        let fine = knob.apply_drag_delta(-10.0, true);

        assert!((fine - normal * FINE_DRAG_MULTIPLIER).abs() < 1.0e-6);
    }

    #[test]
    fn wheel_delta_moves_pan_and_clamps() {
        let knob = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        };
        crate::test_assertions::assert_float_eq!(
            knob.apply_scroll_delta(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            0.05
        );
    }

    #[test]
    fn pan_angle_maps_range_monotonically() {
        let left = Knob {
            value: -1.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        }
        .value_angle();
        let center = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        }
        .value_angle();
        let right = Knob {
            value: 1.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            default_value: 0.0,
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        }
        .value_angle();

        assert!(left < center);
        assert!(center < right);
    }

    #[test]
    fn gain_knob_clamps_to_gain_range() {
        let knob = Knob {
            value: -12.0,
            mode: KnobMode::Gain,
            default_value: 0.0,
            drag_scalar: 0.02,
            wheel_scalar: 0.04,
            on_change: Box::new(|_| ()),
        };

        crate::test_assertions::assert_float_eq!(knob.apply_drag_delta(500.0, false), -60.0);
        crate::test_assertions::assert_float_eq!(knob.apply_drag_delta(-500.0, false), 12.0);
    }

    #[test]
    fn gain_fader_drag_delta_clamps_to_gain_range() {
        let fader = GainFader {
            value: -12.0,
            default_value: 0.0,
            wheel_scalar: 1.0,
            on_change: Box::new(|_| ()),
        };

        assert!(fader.apply_scroll_delta(mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 }) < -12.0);
        assert!(fader.apply_scroll_delta(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }) > -12.0);
    }

    #[test]
    fn gain_fader_y_is_monotonic() {
        let rail = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 200.0,
        };

        let low = gain_value_to_y(-60.0, rail);
        let mid = gain_value_to_y(-24.0, rail);
        let high = gain_value_to_y(12.0, rail);

        assert!(high < mid);
        assert!(mid < low);
    }

    #[test]
    fn gain_fader_bottom_maps_smoothly_without_dead_zone() {
        let rail = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 200.0,
        };

        crate::test_assertions::assert_float_eq!(
            y_to_gain_value(rail.y + rail.height, rail),
            GAIN_MIN_DB
        );
        assert!(y_to_gain_value(rail.y + rail.height - 4.0, rail) > GAIN_MIN_DB);
    }

    #[test]
    fn gain_fader_shift_drag_is_relative_and_finer_than_regular_drag() {
        let fader = GainFader {
            value: -12.0,
            default_value: 0.0,
            wheel_scalar: 1.0,
            on_change: Box::new(|value| value),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 32.0,
            height: 220.0,
        };
        let start = Point::new(16.0, 100.0);
        let end = Point::new(16.0, 60.0);
        let mut state = GainFaderState::default();

        let _discarded = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _discarded = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(start),
        );
        let action = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position: end }),
            bounds,
            mouse::Cursor::Available(end),
        )
        .expect("shift drag action");

        let (fine_message, _, _) = action.into_inner();
        let fine_value = fine_message.expect("value");
        let regular_value = y_to_gain_value(end.y, gain_fader_rail_bounds(bounds));

        assert!(fine_value > -12.0);
        assert!(fine_value < regular_value);
    }

    #[test]
    fn gain_fader_shift_toggle_during_drag_does_not_jump() {
        let fader = GainFader {
            value: -12.0,
            default_value: 0.0,
            wheel_scalar: 1.0,
            on_change: Box::new(|value| value),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 32.0,
            height: 220.0,
        };
        let start = Point::new(16.0, 100.0);
        let first = Point::new(16.0, 90.0);
        let second = Point::new(16.0, 89.0);
        let mut state = GainFaderState::default();
        let (first_value, second_value) = shift_toggle_drag_values(
            &fader,
            &mut state,
            bounds,
            start,
            first,
            second,
            "second value",
        );

        assert!(second_value > first_value);
        assert!(second_value - first_value < 2.0);
    }

    #[test]
    fn gain_fader_handle_y_stays_valid_for_short_bounds() {
        crate::test_assertions::assert_float_eq!(gain_fader_handle_y(23.0, 12.0), 2.0);
    }

    #[test]
    fn gain_mapping_uses_console_style_taper_points() {
        let rail = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 200.0,
        };

        let zero_y = gain_value_to_y(0.0, rail);
        let normalized = 1.0 - ((zero_y - rail.y) / rail.height);
        assert!((normalized - 0.76).abs() < 1.0e-6);
        assert!((gain_db_to_normalized_with_max(-40.0, 12.0) - 0.16).abs() < 1.0e-6);
        assert!((gain_db_to_normalized_with_max(-10.0, 12.0) - 0.52).abs() < 1.0e-6);
        assert!((gain_db_to_normalized_with_max(0.0, 6.0) - 0.76).abs() < 1.0e-6);
        assert!((gain_normalized_to_db_with_max(0.76, 6.0) - 0.0).abs() < 1.0e-6);
    }

    #[test]
    fn gain_mapping_roundtrips_between_normalized_and_db() {
        for value in [0.0, 0.1, 0.35, 0.5, 0.8, 1.0] {
            let db = gain_normalized_to_db(value);
            let back = gain_db_to_normalized(db);
            assert!((back - value).abs() < 1.0e-5 || (value == 0.0 && back == 0.0));
        }
    }

    #[test]
    fn double_click_detection_respects_threshold() {
        let now = Instant::now();
        assert!(is_double_click(
            Some(
                now.checked_sub(Duration::from_millis(100))
                    .expect("test instant should allow 100 ms subtraction")
            ),
            now
        ));
        assert!(!is_double_click(
            Some(
                now.checked_sub(Duration::from_millis(500))
                    .expect("test instant should allow 500 ms subtraction")
            ),
            now
        ));
        assert!(!is_double_click(None, now));
    }

    #[test]
    fn horizontal_slider_value_for_cursor_snaps_and_clamps() {
        let slider = HorizontalSlider {
            value: -12.0,
            min: -36.0,
            max: 6.0,
            step: 0.5,
            default_value: -12.0,
            metrics: HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::Linear,
            on_change: Box::new(|_| ()),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 24.0,
        };

        crate::test_assertions::assert_float_eq!(slider.value_for_cursor_x(-10.0, bounds), -36.0);
        crate::test_assertions::assert_float_eq!(slider.value_for_cursor_x(400.0, bounds), 6.0);
        crate::test_assertions::assert_float_eq!(slider.normalize(-12.24), -12.0);
        crate::test_assertions::assert_float_eq!(slider.normalize(-12.26), -12.5);
    }

    #[test]
    fn horizontal_slider_shift_drag_microtunes_from_start_value() {
        let slider = HorizontalSlider {
            value: -12.0,
            min: -36.0,
            max: 6.0,
            step: 0.5,
            default_value: -12.0,
            metrics: HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::Linear,
            on_change: Box::new(|_| ()),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 24.0,
        };

        let normal = slider.value_for_cursor_x(160.0, bounds);
        let fine_normalized =
            slider.drag_normalized_delta(slider.normalized_value(slider.value), 80.0, bounds, true);
        let fine = slider.normalize(slider.value_from_normalized(fine_normalized));

        assert!(fine > -12.0);
        assert!(fine < normal);
    }

    #[test]
    fn horizontal_slider_shift_toggle_during_drag_does_not_jump() {
        let slider = HorizontalSlider {
            value: -12.0,
            min: -36.0,
            max: 6.0,
            step: 0.01,
            default_value: -12.0,
            metrics: HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::Linear,
            on_change: Box::new(|value| value),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 24.0,
        };
        let start = Point::new(80.0, 12.0);
        let first = Point::new(100.0, 12.0);
        let second = Point::new(101.0, 12.0);
        let mut state = HorizontalSliderState::default();
        let (first_value, second_value) = shift_toggle_drag_values(
            &slider,
            &mut state,
            bounds,
            start,
            first,
            second,
            "second value",
        );

        assert!(second_value > first_value);
        assert!(second_value - first_value < 0.5);
    }

    #[test]
    fn horizontal_slider_shift_drag_accumulates_sub_step_motion() {
        let slider = HorizontalSlider {
            value: -12.0,
            min: -36.0,
            max: 6.0,
            step: 0.5,
            default_value: -12.0,
            metrics: HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::Linear,
            on_change: Box::new(|value| value),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 24.0,
        };
        let mut state = HorizontalSliderState::default();

        let _discarded = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _discarded = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(80.0, 12.0)),
        );

        let mut last_value = -12.0;
        for x in 81..=100 {
            let position = Point::new(x as f32, 12.0);
            let action = Program::update(
                &slider,
                &mut state,
                &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position }),
                bounds,
                mouse::Cursor::Available(position),
            )
            .expect("drag action");
            let (value, _, _) = action.into_inner();
            last_value = value.expect("value");
        }

        assert!(
            last_value > -12.0,
            "fine drag should accumulate tiny movements instead of dropping them"
        );
    }

    #[test]
    fn horizontal_slider_double_click_publishes_default_value() {
        let slider = HorizontalSlider {
            value: -3.0,
            min: -36.0,
            max: 6.0,
            step: 0.5,
            default_value: -12.0,
            metrics: HORIZONTAL_SLIDER_METRICS,
            scale: HorizontalSliderScale::Linear,
            on_change: Box::new(|value| value),
        };
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 24.0,
        };
        let cursor = mouse::Cursor::Available(Point::new(80.0, 12.0));
        let mut state = HorizontalSliderState::default();

        let _discarded = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            cursor,
        );
        let action = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            cursor,
        )
        .expect("double click action");

        let (message, _, status) = action.into_inner();
        assert_eq!(message, Some(-12.0));
        assert_eq!(status, iced::event::Status::Captured);
    }

    #[test]
    fn horizontal_slider_handle_x_stays_valid_for_narrow_bounds() {
        crate::test_assertions::assert_float_eq!(horizontal_slider_handle_x(19.0, 10.0), 0.0);
    }

    #[test]
    fn compact_horizontal_slider_uses_smaller_handle() {
        let metrics = [COMPACT_HORIZONTAL_SLIDER_METRICS, HORIZONTAL_SLIDER_METRICS];

        assert!(metrics[0].handle_width < metrics[1].handle_width);
        assert!(metrics[0].handle_height < metrics[1].handle_height);
        assert!(metrics[0].height < metrics[1].height);
    }

    #[test]
    fn gain_scale_marks_compact_with_height_and_keep_endpoints() {
        let tall = visible_gain_scale_marks(240.0);
        let short = visible_gain_scale_marks(88.0);

        assert!(short.len() < tall.len());
        assert_eq!(short.first().copied(), Some(GAIN_SCALE_DB_MARKS[0]));
        assert_eq!(short.last().copied(), Some(GAIN_MIN_DB));
    }

    #[test]
    fn gain_scale_marks_keep_minimum_vertical_gap() {
        let height = 88.0;
        let marks = visible_gain_scale_marks(height);
        let rail = gain_fader_rail_bounds(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 32.0,
            height,
        });
        let positions: Vec<f32> = marks
            .into_iter()
            .map(|db| gain_value_to_y(db, rail))
            .collect();

        for pair in positions.windows(2) {
            assert!(pair[1] - pair[0] >= GAIN_SCALE_LABEL_MIN_GAP - 0.001);
        }
    }

    #[test]
    fn gain_scale_labels_include_positive_sign() {
        assert_eq!(gain_scale_label(12.0), "+12");
        assert_eq!(gain_scale_label(0.0), "0");
        assert_eq!(gain_scale_label(-60.0), "-60");
    }
}
