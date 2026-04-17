use iced::widget::canvas::{self as canvas_widget, Path, Stroke};
use iced::widget::{canvas, container};
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme, alignment, mouse};

const GAIN_KNOB_SIZE: f32 = 48.0;
const PAN_KNOB_SIZE: f32 = 42.0;
const KNOB_ANGLE_START: f32 = 135.0;
const KNOB_ANGLE_END: f32 = 405.0;
const KNOB_CENTER_ANGLE: f32 = (KNOB_ANGLE_START + KNOB_ANGLE_END) * 0.5;
const PAN_ZERO_EPSILON: f32 = 0.025;
const DEFAULT_DRAG_SCALAR: f32 = 0.008;
const DEFAULT_WHEEL_SCALAR: f32 = 0.05;
const GAIN_MIN_DB: f32 = -60.0;
const GAIN_MAX_DB: f32 = 12.0;

pub(super) fn pan_knob<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    container(
        canvas(Knob {
            value: clamp_pan(value),
            mode: KnobMode::Bipolar {
                zero_epsilon: PAN_ZERO_EPSILON,
            },
            drag_scalar: DEFAULT_DRAG_SCALAR,
            wheel_scalar: DEFAULT_WHEEL_SCALAR,
            on_change: Box::new(on_change),
        })
        .width(Length::Fixed(PAN_KNOB_SIZE))
        .height(Length::Fixed(PAN_KNOB_SIZE)),
    )
    .width(Length::Fixed(PAN_KNOB_SIZE))
    .align_x(alignment::Horizontal::Center)
    .into()
}

pub(super) fn gain_knob<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    container(
        canvas(Knob {
            value: clamp_gain(value),
            mode: KnobMode::Unipolar {
                min: GAIN_MIN_DB,
                max: GAIN_MAX_DB,
            },
            drag_scalar: 0.35,
            wheel_scalar: 1.0,
            on_change: Box::new(on_change),
        })
        .width(Length::Fixed(GAIN_KNOB_SIZE))
        .height(Length::Fixed(GAIN_KNOB_SIZE)),
    )
    .width(Length::Fixed(GAIN_KNOB_SIZE))
    .align_x(alignment::Horizontal::Center)
    .into()
}

struct Knob<'a, Message> {
    value: f32,
    mode: KnobMode,
    drag_scalar: f32,
    wheel_scalar: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Clone, Copy)]
enum KnobMode {
    Bipolar { zero_epsilon: f32 },
    Unipolar { min: f32, max: f32 },
}

#[derive(Default)]
struct KnobState {
    dragging: bool,
    last_cursor_y: Option<f32>,
}

impl<Message: Clone> canvas_widget::Program<Message> for Knob<'_, Message> {
    type State = KnobState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas_widget::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas_widget::Action<Message>> {
        match event {
            canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_over(bounds) {
                    state.dragging = true;
                    state.last_cursor_y = Some(position.y);
                    return Some(canvas_widget::Action::capture());
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let (Some(last_y), Some(position)) = (state.last_cursor_y, cursor.position())
                {
                    state.last_cursor_y = Some(position.y);
                    let next = self.apply_drag_delta(position.y - last_y);
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_some() {
                    let next = self.apply_scroll_delta(*delta);
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.last_cursor_y = None;
            }
            _ => {}
        }

        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas_widget::Geometry> {
        let palette = theme.extended_palette();
        let mut frame = canvas_widget::Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width * 0.5, bounds.height * 0.5);
        let radius = bounds.width.min(bounds.height) * 0.5 - 6.0;

        let base = Path::circle(center, radius);
        frame.fill(&base, palette.background.weak.color);
        frame.stroke(
            &base,
            Stroke::default()
                .with_width(1.0)
                .with_color(palette.background.strong.color),
        );

        let hover = state.dragging || cursor.position_in(bounds).is_some();
        let accent = if hover {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };

        let background_arc = Path::new(|builder| {
            builder.arc(canvas_widget::path::Arc {
                center,
                radius: radius - 4.0,
                start_angle: Radians(KNOB_ANGLE_START.to_radians()),
                end_angle: Radians(KNOB_ANGLE_END.to_radians()),
            });
        });
        frame.stroke(
            &background_arc,
            Stroke::default()
                .with_width(2.0)
                .with_color(palette.background.strong.color)
                .with_line_cap(canvas_widget::LineCap::Round),
        );

        if !self.is_neutral() {
            let arc = Path::new(|builder| {
                builder.arc(canvas_widget::path::Arc {
                    center,
                    radius: radius - 4.0,
                    start_angle: Radians(self.active_start_angle()),
                    end_angle: Radians(self.active_end_angle()),
                });
            });
            frame.stroke(
                &arc,
                Stroke::default()
                    .with_width(3.0)
                    .with_color(accent)
                    .with_line_cap(canvas_widget::LineCap::Round),
            );
        }

        let indicator = Point::new(
            center.x + (radius - 9.0) * self.value_angle().cos(),
            center.y + (radius - 9.0) * self.value_angle().sin(),
        );
        frame.stroke(
            &Path::line(center, indicator),
            Stroke::default()
                .with_width(2.0)
                .with_color(accent)
                .with_line_cap(canvas_widget::LineCap::Round),
        );
        frame.fill(
            &Path::circle(center, 3.0),
            Color {
                a: 0.9,
                ..palette.background.base.text
            },
        );

        vec![frame.into_geometry()]
    }
}

impl<Message> Knob<'_, Message> {
    fn apply_drag_delta(&self, delta_y: f32) -> f32 {
        self.normalize(self.value - delta_y * self.drag_scalar)
    }

    fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let amount = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * self.wheel_scalar,
            mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.wheel_scalar,
        };
        self.normalize(self.value + amount)
    }

    fn normalize(&self, value: f32) -> f32 {
        match self.mode {
            KnobMode::Bipolar { zero_epsilon } => {
                let clamped = value.clamp(-1.0, 1.0);
                if clamped.abs() <= zero_epsilon {
                    0.0
                } else {
                    clamped
                }
            }
            KnobMode::Unipolar { min, max } => value.clamp(min, max),
        }
    }

    fn normalized_value(&self) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => (self.value + 1.0) * 0.5,
            KnobMode::Unipolar { min, max } => ((self.value - min) / (max - min)).clamp(0.0, 1.0),
        }
    }

    fn value_angle(&self) -> f32 {
        let start = KNOB_ANGLE_START.to_radians();
        let end = KNOB_ANGLE_END.to_radians();
        start + (end - start) * self.normalized_value()
    }

    fn is_neutral(&self) -> bool {
        match self.mode {
            KnobMode::Bipolar { .. } => self.value == 0.0,
            KnobMode::Unipolar { min, .. } => self.value == min,
        }
    }

    fn active_start_angle(&self) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => {
                if self.value < 0.0 {
                    self.value_angle()
                } else {
                    KNOB_CENTER_ANGLE.to_radians()
                }
            }
            KnobMode::Unipolar { .. } => KNOB_ANGLE_START.to_radians(),
        }
    }

    fn active_end_angle(&self) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => {
                if self.value < 0.0 {
                    KNOB_CENTER_ANGLE.to_radians()
                } else {
                    self.value_angle()
                }
            }
            KnobMode::Unipolar { .. } => self.value_angle(),
        }
    }
}

fn clamp_pan(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

fn clamp_gain(value: f32) -> f32 {
    value.clamp(GAIN_MIN_DB, GAIN_MAX_DB)
}

#[cfg(test)]
mod tests {
    use iced::mouse;

    use super::{Knob, KnobMode};

    #[test]
    fn drag_delta_moves_pan_and_clamps() {
        let knob = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        };
        assert!((knob.apply_drag_delta(-10.0) - 0.08).abs() < 1.0e-6);
    }

    #[test]
    fn wheel_delta_moves_pan_and_clamps() {
        let knob = Knob {
            value: 0.0,
            mode: KnobMode::Bipolar {
                zero_epsilon: 0.025,
            },
            drag_scalar: 0.008,
            wheel_scalar: 0.05,
            on_change: Box::new(|_| ()),
        };
        assert_eq!(
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
            mode: KnobMode::Unipolar {
                min: -60.0,
                max: 12.0,
            },
            drag_scalar: 0.35,
            wheel_scalar: 1.0,
            on_change: Box::new(|_| ()),
        };

        assert_eq!(knob.apply_drag_delta(500.0), -60.0);
        assert_eq!(knob.apply_drag_delta(-500.0), 12.0);
    }
}
