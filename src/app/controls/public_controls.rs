mod gain_fader_canvas;
mod horizontal_slider_canvas;

use super::*;

pub(in crate::app) const FADER_WIDTH: f32 = ui_style::grid_f32(8);
pub(in crate::app) const FADER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(5);
pub(in crate::app) const FADER_RAIL_WIDTH: f32 = ui_style::grid_f32(2);
pub(in crate::app) const GAIN_SCALE_WIDTH: f32 = ui_style::grid_f32(6);
pub(in crate::app) const GAIN_SCALE_TICK_WIDTH: f32 = 5.0;
pub(in crate::app) const GAIN_SCALE_LABEL_MIN_GAP: f32 = 12.0;
pub(in crate::app) const GAIN_SCALE_LABEL_SIZE: f32 = 8.0;
pub(in crate::app) const GAIN_SCALE_DB_MARKS: [f32; 8] =
    [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -36.0, -60.0];
pub(in crate::app) const HORIZONTAL_SLIDER_HEIGHT: f32 = 24.0;
pub(in crate::app) const HORIZONTAL_SLIDER_RAIL_HEIGHT: f32 = ui_style::grid_f32(2);
pub(in crate::app) const HORIZONTAL_SLIDER_HANDLE_WIDTH: f32 = ui_style::grid_f32(5);
pub(in crate::app) const HORIZONTAL_SLIDER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(5);
pub(in crate::app) const COMPACT_HORIZONTAL_SLIDER_HEIGHT: f32 = ui_style::grid_f32(5);
pub(in crate::app) const COMPACT_HORIZONTAL_SLIDER_RAIL_HEIGHT: f32 = ui_style::grid_f32(1);
pub(in crate::app) const COMPACT_HORIZONTAL_SLIDER_HANDLE_WIDTH: f32 = ui_style::grid_f32(3);
pub(in crate::app) const COMPACT_HORIZONTAL_SLIDER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(3);
pub(in crate::app) const GAIN_KNOB_SIZE: f32 = 48.0;
pub(in crate::app) const PAN_KNOB_SIZE: f32 = 40.0;
pub(in crate::app) const KNOB_ANGLE_START: f32 = 135.0;
pub(in crate::app) const KNOB_ANGLE_END: f32 = 405.0;
pub(in crate::app) const KNOB_CENTER_ANGLE: f32 = (KNOB_ANGLE_START + KNOB_ANGLE_END) * 0.5;
pub(in crate::app) const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(350);
pub(in crate::app) const GAIN_TAPER_MINUS_40_POSITION: f32 = 0.16;
pub(in crate::app) const GAIN_TAPER_MINUS_10_POSITION: f32 = 0.52;
pub(in crate::app) const GAIN_TAPER_ZERO_POSITION: f32 = 0.76;
pub(in crate::app) const PAN_ZERO_EPSILON: f32 = 0.025;
pub(in crate::app) const DEFAULT_DRAG_SCALAR: f32 = 0.008;
pub(in crate::app) const FINE_DRAG_MULTIPLIER: f32 = 0.2;
pub(in crate::app) const DEFAULT_WHEEL_SCALAR: f32 = 0.05;
pub(in crate::app) const GAIN_MIN_DB: f32 = -60.0;
pub(in crate::app) const GAIN_MAX_DB: f32 = 12.0;

pub(in crate::app) fn fader_rail_layout(total_height: f32) -> (f32, f32) {
    let rail_y = 8.0;
    let rail_height = (total_height - rail_y * 2.0).max(FADER_HANDLE_HEIGHT + 6.0);
    (rail_y, rail_height)
}

pub(in crate::app) fn gain_control_width(mode_is_knob: bool) -> f32 {
    if mode_is_knob {
        GAIN_KNOB_SIZE
    } else {
        FADER_WIDTH
    }
}

pub(in crate::app) fn pan_knob<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    public_control(PublicControlSpec::pan_knob(value), on_change)
}

pub(in crate::app) fn gain_knob<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    public_control(PublicControlSpec::gain_knob(value), on_change)
}

pub(in crate::app) fn gain_fader<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    public_control(PublicControlSpec::gain_fader(value), on_change)
}

#[derive(Clone, Copy)]
enum PublicControlSpec {
    Knob {
        value: f32,
        size: f32,
        mode: KnobMode,
        drag_scalar: f32,
        wheel_scalar: f32,
    },
    GainFader {
        value: f32,
    },
    Slider {
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        default_value: f32,
        metrics: HorizontalSliderMetrics,
        scale: HorizontalSliderScale,
    },
}

impl PublicControlSpec {
    fn pan_knob(value: f32) -> Self {
        Self::Knob {
            value: clamp_pan(value),
            size: PAN_KNOB_SIZE,
            mode: KnobMode::Bipolar {
                zero_epsilon: PAN_ZERO_EPSILON,
            },
            drag_scalar: DEFAULT_DRAG_SCALAR,
            wheel_scalar: DEFAULT_WHEEL_SCALAR,
        }
    }

    fn gain_knob(value: f32) -> Self {
        Self::Knob {
            value: clamp_gain(value),
            size: GAIN_KNOB_SIZE,
            mode: KnobMode::Gain,
            drag_scalar: 0.02,
            wheel_scalar: 0.04,
        }
    }

    fn gain_fader(value: f32) -> Self {
        Self::GainFader {
            value: clamp_gain(value),
        }
    }

    fn slider(
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        default_value: f32,
        metrics: HorizontalSliderMetrics,
        scale: HorizontalSliderScale,
    ) -> Self {
        Self::Slider {
            value,
            min,
            max,
            step,
            default_value,
            metrics,
            scale,
        }
    }
}

fn public_control<'a, Message: Clone + 'a>(
    spec: PublicControlSpec,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    match spec {
        PublicControlSpec::Knob {
            value,
            size,
            mode,
            drag_scalar,
            wheel_scalar,
        } => container(
            canvas(Knob {
                value,
                mode,
                default_value: 0.0,
                drag_scalar,
                wheel_scalar,
                on_change: Box::new(on_change),
            })
            .width(Length::Fixed(size))
            .height(Length::Fixed(size)),
        )
        .width(Length::Fixed(size))
        .align_x(alignment::Horizontal::Center)
        .into(),
        PublicControlSpec::GainFader { value } => container(
            canvas(GainFader {
                value,
                default_value: 0.0,
                wheel_scalar: 1.0,
                on_change: Box::new(on_change),
            })
            .width(Length::Fixed(FADER_WIDTH))
            .height(Length::Fill),
        )
        .width(Length::Fixed(FADER_WIDTH))
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .into(),
        PublicControlSpec::Slider {
            value,
            min,
            max,
            step,
            default_value,
            metrics,
            scale,
        } => canvas(HorizontalSlider {
            value,
            min,
            max,
            step,
            default_value,
            metrics,
            scale,
            on_change: Box::new(on_change),
        })
        .width(Length::Fill)
        .height(Length::Fixed(metrics.height))
        .into(),
    }
}

pub(in crate::app) fn gain_fader_scale<'a, Message: 'a>(height: f32) -> Element<'a, Message> {
    canvas(GainFaderScale)
        .width(Length::Fixed(GAIN_SCALE_WIDTH))
        .height(Length::Fixed(height.max(1.0)))
        .into()
}

pub(in crate::app) fn gain_fader_scale_width() -> f32 {
    GAIN_SCALE_WIDTH
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct HorizontalSliderSpec {
    pub(in crate::app) value: f32,
    pub(in crate::app) min: f32,
    pub(in crate::app) max: f32,
    pub(in crate::app) step: f32,
    pub(in crate::app) default_value: f32,
    pub(in crate::app) metrics: HorizontalSliderMetrics,
    pub(in crate::app) scale: HorizontalSliderScale,
}

pub(in crate::app) fn horizontal_slider<'a, Message: Clone + 'a>(
    spec: HorizontalSliderSpec,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    public_control(
        PublicControlSpec::slider(
            spec.value,
            spec.min,
            spec.max,
            spec.step,
            spec.default_value,
            spec.metrics,
            spec.scale,
        ),
        on_change,
    )
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct HorizontalSliderMetrics {
    pub(in crate::app) height: f32,
    pub(in crate::app) rail_height: f32,
    pub(in crate::app) handle_width: f32,
    pub(in crate::app) handle_height: f32,
}

pub(in crate::app) const HORIZONTAL_SLIDER_METRICS: HorizontalSliderMetrics =
    HorizontalSliderMetrics {
        height: HORIZONTAL_SLIDER_HEIGHT,
        rail_height: HORIZONTAL_SLIDER_RAIL_HEIGHT,
        handle_width: HORIZONTAL_SLIDER_HANDLE_WIDTH,
        handle_height: HORIZONTAL_SLIDER_HANDLE_HEIGHT,
    };

pub(in crate::app) const COMPACT_HORIZONTAL_SLIDER_METRICS: HorizontalSliderMetrics =
    HorizontalSliderMetrics {
        height: COMPACT_HORIZONTAL_SLIDER_HEIGHT,
        rail_height: COMPACT_HORIZONTAL_SLIDER_RAIL_HEIGHT,
        handle_width: COMPACT_HORIZONTAL_SLIDER_HANDLE_WIDTH,
        handle_height: COMPACT_HORIZONTAL_SLIDER_HANDLE_HEIGHT,
    };

pub(in crate::app) struct Knob<'a, Message> {
    pub(in crate::app) value: f32,
    pub(in crate::app) mode: KnobMode,
    pub(in crate::app) default_value: f32,
    pub(in crate::app) drag_scalar: f32,
    pub(in crate::app) wheel_scalar: f32,
    pub(in crate::app) on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

pub(in crate::app) struct GainFader<'a, Message> {
    pub(in crate::app) value: f32,
    pub(in crate::app) default_value: f32,
    pub(in crate::app) wheel_scalar: f32,
    pub(in crate::app) on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct GainFaderScale;

pub(in crate::app) struct HorizontalSlider<'a, Message> {
    pub(in crate::app) value: f32,
    pub(in crate::app) min: f32,
    pub(in crate::app) max: f32,
    pub(in crate::app) step: f32,
    pub(in crate::app) default_value: f32,
    pub(in crate::app) metrics: HorizontalSliderMetrics,
    pub(in crate::app) scale: HorizontalSliderScale,
    pub(in crate::app) on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) enum HorizontalSliderScale {
    Linear,
    GainDb { max: f32 },
}

#[derive(Clone, Copy)]
pub(in crate::app) enum KnobMode {
    Bipolar { zero_epsilon: f32 },
    Gain,
}

#[derive(Default)]
pub(in crate::app) struct KnobState {
    pub(in crate::app) dragging: bool,
    pub(in crate::app) shift_pressed: bool,
    pub(in crate::app) last_cursor_y: Option<f32>,
    pub(in crate::app) last_press_at: Option<Instant>,
}

#[derive(Default)]
pub(in crate::app) struct GainFaderState {
    pub(in crate::app) dragging: bool,
    pub(in crate::app) shift_pressed: bool,
    pub(in crate::app) last_cursor_y: Option<f32>,
    pub(in crate::app) drag_value: f32,
    pub(in crate::app) last_press_at: Option<Instant>,
}

#[derive(Default)]
pub(in crate::app) struct HorizontalSliderState {
    pub(in crate::app) dragging: bool,
    pub(in crate::app) shift_pressed: bool,
    pub(in crate::app) last_cursor_x: Option<f32>,
    pub(in crate::app) drag_normalized: f32,
    pub(in crate::app) last_press_at: Option<Instant>,
}

#[derive(Default)]
pub(in crate::app) struct CanvasGeometryCache {
    cache: canvas_widget::Cache<Renderer>,
    signature: Cell<Option<u64>>,
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
            canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.shift_pressed = modifiers.shift();
                None
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.handle_knob_press(state, bounds, cursor)
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                self.handle_knob_drag(state, position.y)
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.position_in(bounds).is_some() =>
            {
                self.handle_knob_scroll(*delta)
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.last_cursor_y = None;
                None
            }
            _ => None,
        }
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
