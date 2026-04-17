use iced::widget::canvas::{self as canvas_widget, Path, Stroke};
use iced::widget::{canvas, container};
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme, alignment, mouse};

const PAN_KNOB_SIZE: f32 = 42.0;
const PAN_ANGLE_START: f32 = 135.0;
const PAN_ANGLE_END: f32 = 405.0;
const PAN_CENTER_ANGLE: f32 = (PAN_ANGLE_START + PAN_ANGLE_END) * 0.5;
const PAN_DRAG_SCALAR: f32 = 0.008;
const PAN_WHEEL_SCALAR: f32 = 0.05;
const PAN_ZERO_EPSILON: f32 = 0.025;

pub(super) fn pan_knob<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    container(
        canvas(PanKnob {
            value: clamp_pan(value),
            on_change: Box::new(on_change),
        })
        .width(Length::Fixed(PAN_KNOB_SIZE))
        .height(Length::Fixed(PAN_KNOB_SIZE)),
    )
    .width(Length::Fixed(PAN_KNOB_SIZE))
    .align_x(alignment::Horizontal::Center)
    .into()
}

struct PanKnob<'a, Message> {
    value: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Default)]
struct PanKnobState {
    dragging: bool,
    last_cursor_y: Option<f32>,
}

impl<Message: Clone> canvas_widget::Program<Message> for PanKnob<'_, Message> {
    type State = PanKnobState;

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
                    let next = apply_drag_delta(self.value, position.y - last_y);
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_some() {
                    let next = apply_scroll_delta(self.value, *delta);
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
                start_angle: Radians(PAN_ANGLE_START.to_radians()),
                end_angle: Radians(PAN_ANGLE_END.to_radians()),
            });
        });
        frame.stroke(
            &background_arc,
            Stroke::default()
                .with_width(2.0)
                .with_color(palette.background.strong.color)
                .with_line_cap(canvas_widget::LineCap::Round),
        );

        if self.value != 0.0 {
            let arc = Path::new(|builder| {
                builder.arc(canvas_widget::path::Arc {
                    center,
                    radius: radius - 4.0,
                    start_angle: Radians(if self.value < 0.0 {
                        pan_to_angle(self.value)
                    } else {
                        PAN_CENTER_ANGLE.to_radians()
                    }),
                    end_angle: Radians(if self.value < 0.0 {
                        PAN_CENTER_ANGLE.to_radians()
                    } else {
                        pan_to_angle(self.value)
                    }),
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
            center.x + (radius - 9.0) * pan_to_angle(self.value).cos(),
            center.y + (radius - 9.0) * pan_to_angle(self.value).sin(),
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

fn apply_drag_delta(value: f32, delta_y: f32) -> f32 {
    normalize_pan(value - delta_y * PAN_DRAG_SCALAR)
}

fn apply_scroll_delta(value: f32, delta: mouse::ScrollDelta) -> f32 {
    let amount = match delta {
        mouse::ScrollDelta::Lines { y, .. } => y * PAN_WHEEL_SCALAR,
        mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * PAN_WHEEL_SCALAR,
    };
    normalize_pan(value + amount)
}

fn clamp_pan(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

fn normalize_pan(value: f32) -> f32 {
    let clamped = clamp_pan(value);
    if clamped.abs() <= PAN_ZERO_EPSILON {
        0.0
    } else {
        clamped
    }
}

fn pan_to_angle(value: f32) -> f32 {
    let normalized = (normalize_pan(value) + 1.0) * 0.5;
    let start = PAN_ANGLE_START.to_radians();
    let end = PAN_ANGLE_END.to_radians();
    start + (end - start) * normalized
}

#[cfg(test)]
mod tests {
    use iced::mouse;

    use super::{apply_drag_delta, apply_scroll_delta, pan_to_angle};

    #[test]
    fn drag_delta_moves_pan_and_clamps() {
        assert!((apply_drag_delta(0.0, -10.0) - 0.08).abs() < 1.0e-6);
        assert_eq!(apply_drag_delta(0.95, -10.0), 1.0);
        assert_eq!(apply_drag_delta(-0.95, 10.0), -1.0);
        assert_eq!(apply_drag_delta(0.01, 0.0), 0.0);
    }

    #[test]
    fn wheel_delta_moves_pan_and_clamps() {
        assert_eq!(
            apply_scroll_delta(0.0, mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            0.05
        );
        assert_eq!(
            apply_scroll_delta(0.98, mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            1.0
        );
    }

    #[test]
    fn pan_angle_maps_range_monotonically() {
        let left = pan_to_angle(-1.0);
        let center = pan_to_angle(0.0);
        let right = pan_to_angle(1.0);

        assert!(left < center);
        assert!(center < right);
    }
}
