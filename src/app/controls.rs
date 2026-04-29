use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::ui_style;
use iced::widget::canvas::{self as canvas_widget, Path, Stroke, Text};
use iced::widget::{canvas, container};
use iced::{
    Color, Element, Length, Pixels, Point, Radians, Rectangle, Renderer, Theme, alignment,
    keyboard, mouse,
};

const FADER_WIDTH: f32 = ui_style::grid_f32(8);
const FADER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(5);
const FADER_RAIL_WIDTH: f32 = ui_style::grid_f32(2);
const GAIN_SCALE_WIDTH: f32 = ui_style::grid_f32(6);
const GAIN_SCALE_TICK_WIDTH: f32 = 5.0;
const GAIN_SCALE_LABEL_MIN_GAP: f32 = 12.0;
const GAIN_SCALE_LABEL_SIZE: f32 = 8.0;
const GAIN_SCALE_DB_MARKS: [f32; 8] = [12.0, 6.0, 0.0, -6.0, -12.0, -24.0, -36.0, -60.0];
const HORIZONTAL_SLIDER_HEIGHT: f32 = 24.0;
const HORIZONTAL_SLIDER_RAIL_HEIGHT: f32 = ui_style::grid_f32(2);
const HORIZONTAL_SLIDER_HANDLE_WIDTH: f32 = ui_style::grid_f32(5);
const HORIZONTAL_SLIDER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(5);
const COMPACT_HORIZONTAL_SLIDER_HEIGHT: f32 = ui_style::grid_f32(5);
const COMPACT_HORIZONTAL_SLIDER_RAIL_HEIGHT: f32 = ui_style::grid_f32(1);
const COMPACT_HORIZONTAL_SLIDER_HANDLE_WIDTH: f32 = ui_style::grid_f32(3);
const COMPACT_HORIZONTAL_SLIDER_HANDLE_HEIGHT: f32 = ui_style::grid_f32(3);
const GAIN_KNOB_SIZE: f32 = 48.0;
const PAN_KNOB_SIZE: f32 = 40.0;
const KNOB_ANGLE_START: f32 = 135.0;
const KNOB_ANGLE_END: f32 = 405.0;
const KNOB_CENTER_ANGLE: f32 = (KNOB_ANGLE_START + KNOB_ANGLE_END) * 0.5;
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(350);
const GAIN_TAPER_MINUS_40_POSITION: f32 = 0.16;
const GAIN_TAPER_MINUS_10_POSITION: f32 = 0.52;
const GAIN_TAPER_ZERO_POSITION: f32 = 0.76;
const PAN_ZERO_EPSILON: f32 = 0.025;
const DEFAULT_DRAG_SCALAR: f32 = 0.008;
const FINE_DRAG_MULTIPLIER: f32 = 0.2;
const DEFAULT_WHEEL_SCALAR: f32 = 0.05;
pub(super) const GAIN_MIN_DB: f32 = -60.0;
const GAIN_MAX_DB: f32 = 12.0;

pub(super) fn fader_rail_layout(total_height: f32) -> (f32, f32) {
    let rail_y = 8.0;
    let rail_height = (total_height - rail_y * 2.0).max(FADER_HANDLE_HEIGHT + 6.0);
    (rail_y, rail_height)
}

pub(super) fn gain_control_width(mode_is_knob: bool) -> f32 {
    if mode_is_knob {
        GAIN_KNOB_SIZE
    } else {
        FADER_WIDTH
    }
}

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
            default_value: 0.0,
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
            mode: KnobMode::Gain,
            default_value: 0.0,
            drag_scalar: 0.02,
            wheel_scalar: 0.04,
            on_change: Box::new(on_change),
        })
        .width(Length::Fixed(GAIN_KNOB_SIZE))
        .height(Length::Fixed(GAIN_KNOB_SIZE)),
    )
    .width(Length::Fixed(GAIN_KNOB_SIZE))
    .align_x(alignment::Horizontal::Center)
    .into()
}

pub(super) fn gain_fader<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    container(
        canvas(GainFader {
            value: clamp_gain(value),
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
    .into()
}

pub(super) fn gain_fader_scale<'a, Message: 'a>(height: f32) -> Element<'a, Message> {
    canvas(GainFaderScale)
        .width(Length::Fixed(GAIN_SCALE_WIDTH))
        .height(Length::Fixed(height.max(1.0)))
        .into()
}

pub(super) fn gain_fader_scale_width() -> f32 {
    GAIN_SCALE_WIDTH
}

pub(super) fn horizontal_slider<'a, Message: Clone + 'a>(
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    default_value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    canvas(HorizontalSlider {
        value,
        min,
        max,
        step,
        default_value,
        metrics: HORIZONTAL_SLIDER_METRICS,
        scale: HorizontalSliderScale::Linear,
        on_change: Box::new(on_change),
    })
    .width(Length::Fill)
    .height(Length::Fixed(HORIZONTAL_SLIDER_METRICS.height))
    .into()
}

pub(super) fn compact_gain_slider<'a, Message: Clone + 'a>(
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    default_value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    canvas(HorizontalSlider {
        value,
        min,
        max,
        step,
        default_value,
        metrics: COMPACT_HORIZONTAL_SLIDER_METRICS,
        scale: HorizontalSliderScale::GainDb { max },
        on_change: Box::new(on_change),
    })
    .width(Length::Fill)
    .height(Length::Fixed(COMPACT_HORIZONTAL_SLIDER_METRICS.height))
    .into()
}

#[derive(Clone, Copy)]
struct HorizontalSliderMetrics {
    height: f32,
    rail_height: f32,
    handle_width: f32,
    handle_height: f32,
}

const HORIZONTAL_SLIDER_METRICS: HorizontalSliderMetrics = HorizontalSliderMetrics {
    height: HORIZONTAL_SLIDER_HEIGHT,
    rail_height: HORIZONTAL_SLIDER_RAIL_HEIGHT,
    handle_width: HORIZONTAL_SLIDER_HANDLE_WIDTH,
    handle_height: HORIZONTAL_SLIDER_HANDLE_HEIGHT,
};

const COMPACT_HORIZONTAL_SLIDER_METRICS: HorizontalSliderMetrics = HorizontalSliderMetrics {
    height: COMPACT_HORIZONTAL_SLIDER_HEIGHT,
    rail_height: COMPACT_HORIZONTAL_SLIDER_RAIL_HEIGHT,
    handle_width: COMPACT_HORIZONTAL_SLIDER_HANDLE_WIDTH,
    handle_height: COMPACT_HORIZONTAL_SLIDER_HANDLE_HEIGHT,
};

struct Knob<'a, Message> {
    value: f32,
    mode: KnobMode,
    default_value: f32,
    drag_scalar: f32,
    wheel_scalar: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

struct GainFader<'a, Message> {
    value: f32,
    default_value: f32,
    wheel_scalar: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Clone, Copy)]
struct GainFaderScale;

struct HorizontalSlider<'a, Message> {
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    default_value: f32,
    metrics: HorizontalSliderMetrics,
    scale: HorizontalSliderScale,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

#[derive(Clone, Copy)]
enum HorizontalSliderScale {
    Linear,
    GainDb { max: f32 },
}

#[derive(Clone, Copy)]
enum KnobMode {
    Bipolar { zero_epsilon: f32 },
    Gain,
}

#[derive(Default)]
struct KnobState {
    dragging: bool,
    shift_pressed: bool,
    last_cursor_y: Option<f32>,
    last_press_at: Option<Instant>,
}

#[derive(Default)]
struct GainFaderState {
    dragging: bool,
    shift_pressed: bool,
    last_cursor_y: Option<f32>,
    drag_value: f32,
    last_press_at: Option<Instant>,
}

#[derive(Default)]
struct HorizontalSliderState {
    dragging: bool,
    shift_pressed: bool,
    last_cursor_x: Option<f32>,
    drag_normalized: f32,
    last_press_at: Option<Instant>,
}

#[derive(Default)]
struct CanvasGeometryCache {
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
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.position_over(bounds).is_some()
                    && let Some(position) = cursor.position()
                {
                    let now = Instant::now();
                    if is_double_click(state.last_press_at, now) {
                        state.dragging = false;
                        state.last_cursor_y = None;
                        state.last_press_at = None;
                        return Some(
                            canvas_widget::Action::publish((self.on_change)(self.default_value))
                                .and_capture(),
                        );
                    }
                    state.last_press_at = Some(now);
                    state.dragging = true;
                    state.last_cursor_y = Some(position.y);
                    return Some(canvas_widget::Action::capture());
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.dragging
                    && let Some(last_y) = state.last_cursor_y
                {
                    state.last_cursor_y = Some(position.y);
                    let next = self.apply_drag_delta(position.y - last_y, state.shift_pressed);
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.position_in(bounds).is_some() =>
            {
                let next = self.apply_scroll_delta(*delta);
                return Some(canvas_widget::Action::publish((self.on_change)(next)).and_capture());
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

impl<Message: Clone> canvas_widget::Program<Message> for GainFader<'_, Message> {
    type State = GainFaderState;

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
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.position_over(bounds).is_some()
                    && let Some(position) = cursor.position()
                {
                    let now = Instant::now();
                    if is_double_click(state.last_press_at, now) {
                        state.dragging = false;
                        state.last_press_at = None;
                        return Some(
                            canvas_widget::Action::publish((self.on_change)(self.default_value))
                                .and_capture(),
                        );
                    }
                    state.last_press_at = Some(now);
                    state.dragging = true;
                    state.last_cursor_y = Some(position.y);
                    state.drag_value = self.value;
                    if state.shift_pressed {
                        return Some(canvas_widget::Action::capture());
                    }
                    let next =
                        y_to_gain_value(position.y - bounds.y, gain_fader_rail_bounds(bounds));
                    state.drag_value = next;
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position })
                if state.dragging =>
            {
                let rail_bounds = gain_fader_rail_bounds(bounds);
                let next = state.last_cursor_y.map_or(self.value, |last_y| {
                    let delta_normalized = -((position.y - last_y) / rail_bounds.height.max(1.0))
                        * if state.shift_pressed {
                            FINE_DRAG_MULTIPLIER
                        } else {
                            1.0
                        };
                    let normalized = (gain_db_to_normalized(state.drag_value) + delta_normalized)
                        .clamp(0.0, 1.0);
                    gain_normalized_to_db(normalized)
                });
                state.last_cursor_y = Some(position.y);
                state.drag_value = next;
                return Some(canvas_widget::Action::publish((self.on_change)(next)).and_capture());
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.position_in(bounds).is_some() =>
            {
                let next = self.apply_scroll_delta(*delta);
                return Some(canvas_widget::Action::publish((self.on_change)(next)).and_capture());
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
        let hover = state.dragging || cursor.position_in(bounds).is_some();
        let accent = if hover {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };

        let rail_bounds = gain_fader_rail_bounds(bounds);
        let handle_center_y = gain_value_to_y(self.value, rail_bounds);
        let fill_bounds = Rectangle {
            x: rail_bounds.x,
            y: handle_center_y,
            width: rail_bounds.width,
            height: (rail_bounds.y + rail_bounds.height - handle_center_y).max(0.0),
        };
        let handle_bounds = Rectangle {
            x: 4.0,
            y: gain_fader_handle_y(bounds.height, handle_center_y),
            width: (bounds.width - 8.0).max(12.0),
            height: FADER_HANDLE_HEIGHT,
        };

        frame.fill(
            &Path::rectangle(
                Point::new(rail_bounds.x, rail_bounds.y),
                iced::Size::new(rail_bounds.width, rail_bounds.height),
            ),
            palette.background.weak.color,
        );
        frame.stroke(
            &Path::rectangle(
                Point::new(rail_bounds.x, rail_bounds.y),
                iced::Size::new(rail_bounds.width, rail_bounds.height),
            ),
            Stroke::default()
                .with_width(1.0)
                .with_color(palette.background.strong.color),
        );

        frame.fill(
            &Path::rectangle(
                Point::new(fill_bounds.x, fill_bounds.y),
                iced::Size::new(fill_bounds.width, fill_bounds.height),
            ),
            accent,
        );

        frame.fill(
            &Path::rectangle(
                Point::new(handle_bounds.x, handle_bounds.y),
                iced::Size::new(handle_bounds.width, handle_bounds.height),
            ),
            palette.background.base.color,
        );
        frame.stroke(
            &Path::rectangle(
                Point::new(handle_bounds.x, handle_bounds.y),
                iced::Size::new(handle_bounds.width, handle_bounds.height),
            ),
            Stroke::default()
                .with_width(1.0)
                .with_color(palette.background.strong.color),
        );

        let notch_y = handle_bounds.y + handle_bounds.height * 0.5;
        frame.stroke(
            &Path::line(
                Point::new(handle_bounds.x + 5.0, notch_y),
                Point::new(handle_bounds.x + handle_bounds.width - 5.0, notch_y),
            ),
            Stroke::default().with_width(2.0).with_color(accent),
        );

        vec![frame.into_geometry()]
    }
}

impl<Message> canvas_widget::Program<Message> for GainFaderScale {
    type State = CanvasGeometryCache;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas_widget::Geometry> {
        let palette = theme.extended_palette();
        let text_color = palette.background.strong.text;
        let tick_color = palette.background.strong.color;
        let signature = color_signature(text_color) ^ color_signature(tick_color).rotate_left(1);
        if state.signature.get() != Some(signature) {
            state.cache.clear();
            state.signature.set(Some(signature));
        }

        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let rail_bounds = gain_fader_rail_bounds(bounds);
            let marks = visible_gain_scale_marks(bounds.height);
            for (index, db) in marks.iter().copied().enumerate() {
                let y = gain_value_to_y(db, rail_bounds);
                frame.stroke(
                    &Path::line(
                        Point::new(bounds.width - GAIN_SCALE_TICK_WIDTH, y),
                        Point::new(bounds.width, y),
                    ),
                    Stroke::default().with_width(1.0).with_color(tick_color),
                );
                frame.fill_text(Text {
                    content: gain_scale_label(db),
                    position: Point::new(bounds.width - GAIN_SCALE_TICK_WIDTH - 3.0, y),
                    color: text_color,
                    size: Pixels(GAIN_SCALE_LABEL_SIZE),
                    align_x: alignment::Horizontal::Right.into(),
                    align_y: if index == 0 {
                        alignment::Vertical::Top
                    } else if index == marks.len() - 1 {
                        alignment::Vertical::Bottom
                    } else {
                        alignment::Vertical::Center
                    },
                    ..Text::default()
                });
            }
        });

        vec![geometry]
    }
}

impl<Message: Clone> canvas_widget::Program<Message> for HorizontalSlider<'_, Message> {
    type State = HorizontalSliderState;

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
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_in(bounds) {
                    let now = Instant::now();
                    if is_double_click(state.last_press_at, now) {
                        state.dragging = false;
                        state.last_press_at = None;
                        return Some(
                            canvas_widget::Action::publish((self.on_change)(
                                self.normalize(self.default_value),
                            ))
                            .and_capture(),
                        );
                    }
                    state.last_press_at = Some(now);
                    state.dragging = true;
                    state.last_cursor_x = Some(position.x);
                    state.drag_normalized = self.normalized_value(self.value);
                    if state.shift_pressed {
                        return Some(canvas_widget::Action::capture());
                    }
                    let next = self.value_for_cursor_x(position.x, bounds);
                    state.drag_normalized = self.normalized_value(next);
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(next)).and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position })
                if state.dragging =>
            {
                let local_x = position.x - bounds.x;
                let value = state.last_cursor_x.map_or(self.value, |last_x| {
                    state.drag_normalized = self.drag_normalized_delta(
                        state.drag_normalized,
                        local_x - last_x,
                        bounds,
                        state.shift_pressed,
                    );
                    self.normalize(self.value_from_normalized(state.drag_normalized))
                });
                state.last_cursor_x = Some(local_x);
                return Some(canvas_widget::Action::publish((self.on_change)(value)).and_capture());
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.position_in(bounds).is_some() =>
            {
                let amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y * self.step.max(0.01),
                    mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.step.max(0.01),
                };
                return Some(
                    canvas_widget::Action::publish((self.on_change)(
                        self.normalize(self.value + amount),
                    ))
                    .and_capture(),
                );
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.last_cursor_x = None;
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
        let hover = state.dragging || cursor.position_in(bounds).is_some();
        let accent = if hover {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };
        let metrics = self.metrics;
        let rail_x = metrics.handle_width * 0.5;
        let rail_width = (bounds.width - metrics.handle_width).max(1.0);
        let rail_y = (bounds.height - metrics.rail_height) * 0.5;
        let handle_center_x = self.handle_center_x(bounds.width, self.normalize(self.value));
        let fill_width = (handle_center_x - rail_x).clamp(0.0, rail_width);
        let handle_bounds = Rectangle {
            x: horizontal_slider_handle_x_with_width(
                bounds.width,
                handle_center_x,
                metrics.handle_width,
            ),
            y: (bounds.height - metrics.handle_height) * 0.5,
            width: metrics.handle_width,
            height: metrics.handle_height,
        };

        frame.fill(
            &Path::rounded_rectangle(
                Point::new(rail_x, rail_y),
                iced::Size::new(rail_width, metrics.rail_height),
                2.0.into(),
            ),
            palette.background.strong.color,
        );
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(rail_x, rail_y),
                iced::Size::new(fill_width, metrics.rail_height),
                2.0.into(),
            ),
            accent,
        );

        frame.fill(
            &Path::rectangle(
                Point::new(handle_bounds.x, handle_bounds.y),
                iced::Size::new(handle_bounds.width, handle_bounds.height),
            ),
            palette.background.base.color,
        );
        frame.stroke(
            &Path::rectangle(
                Point::new(handle_bounds.x, handle_bounds.y),
                iced::Size::new(handle_bounds.width, handle_bounds.height),
            ),
            Stroke::default()
                .with_width(1.0)
                .with_color(palette.background.strong.color),
        );
        let notch_x = handle_bounds.x + handle_bounds.width * 0.5;
        let notch_inset = (metrics.handle_height * 0.25).max(2.0);
        frame.stroke(
            &Path::line(
                Point::new(notch_x, handle_bounds.y + notch_inset),
                Point::new(
                    notch_x,
                    handle_bounds.y + handle_bounds.height - notch_inset,
                ),
            ),
            Stroke::default().with_width(2.0).with_color(accent),
        );

        vec![frame.into_geometry()]
    }
}

impl<Message> Knob<'_, Message> {
    fn apply_drag_delta(&self, delta_y: f32, fine: bool) -> f32 {
        let drag_scalar = self.drag_scalar * if fine { FINE_DRAG_MULTIPLIER } else { 1.0 };
        match self.mode {
            KnobMode::Bipolar { .. } => self.normalize(self.value - delta_y * drag_scalar),
            KnobMode::Gain => {
                let normalized = (self.normalized_value() - delta_y * drag_scalar).clamp(0.0, 1.0);
                self.value_from_normalized(normalized)
            }
        }
    }

    fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let amount = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * self.wheel_scalar,
            mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.wheel_scalar,
        };
        match self.mode {
            KnobMode::Bipolar { .. } => self.normalize(self.value + amount),
            KnobMode::Gain => {
                let normalized = (self.normalized_value() + amount).clamp(0.0, 1.0);
                self.value_from_normalized(normalized)
            }
        }
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
            KnobMode::Gain => clamp_gain(value),
        }
    }

    fn normalized_value(&self) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => (self.value + 1.0) * 0.5,
            KnobMode::Gain => gain_db_to_normalized(self.value),
        }
    }

    fn value_from_normalized(&self, normalized: f32) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => self.normalize(normalized * 2.0 - 1.0),
            KnobMode::Gain => gain_normalized_to_db(normalized),
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
            KnobMode::Gain => self.value <= GAIN_MIN_DB,
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
            KnobMode::Gain => KNOB_ANGLE_START.to_radians(),
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
            KnobMode::Gain => self.value_angle(),
        }
    }
}

impl<Message> GainFader<'_, Message> {
    fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let amount = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * self.wheel_scalar,
            mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.wheel_scalar,
        };
        let normalized = (gain_db_to_normalized(self.value) + amount).clamp(0.0, 1.0);
        gain_normalized_to_db(normalized)
    }
}

impl<Message> HorizontalSlider<'_, Message> {
    fn normalize(&self, value: f32) -> f32 {
        let clamped = value.clamp(self.min, self.max);
        if self.step <= 0.0 {
            clamped
        } else {
            let steps = ((clamped - self.min) / self.step).round();
            (self.min + steps * self.step).clamp(self.min, self.max)
        }
    }

    fn value_for_cursor_x(&self, x: f32, bounds: Rectangle) -> f32 {
        let usable_width = (bounds.width - self.metrics.handle_width).max(1.0);
        let normalized = ((x - self.metrics.handle_width * 0.5) / usable_width).clamp(0.0, 1.0);
        self.normalize(self.value_from_normalized(normalized))
    }

    fn drag_normalized_delta(
        &self,
        normalized: f32,
        delta_x: f32,
        bounds: Rectangle,
        fine: bool,
    ) -> f32 {
        let usable_width = (bounds.width - self.metrics.handle_width).max(1.0);
        let drag_scale = if fine { FINE_DRAG_MULTIPLIER } else { 1.0 };
        (normalized + delta_x / usable_width * drag_scale).clamp(0.0, 1.0)
    }

    fn handle_center_x(&self, width: f32, value: f32) -> f32 {
        let usable_width = (width - self.metrics.handle_width).max(1.0);
        let normalized = self.normalized_value(value);
        self.metrics.handle_width * 0.5 + usable_width * normalized
    }

    fn normalized_value(&self, value: f32) -> f32 {
        match self.scale {
            HorizontalSliderScale::Linear => {
                ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
            }
            HorizontalSliderScale::GainDb { max } => gain_db_to_normalized_with_max(value, max),
        }
    }

    fn value_from_normalized(&self, normalized: f32) -> f32 {
        match self.scale {
            HorizontalSliderScale::Linear => self.min + normalized * (self.max - self.min),
            HorizontalSliderScale::GainDb { max } => {
                gain_normalized_to_db_with_max(normalized, max)
            }
        }
    }
}

fn is_double_click(previous: Option<Instant>, now: Instant) -> bool {
    previous
        .map(|last| now.saturating_duration_since(last) <= DOUBLE_CLICK_THRESHOLD)
        .unwrap_or(false)
}

fn clamp_pan(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

fn clamp_gain(value: f32) -> f32 {
    value.clamp(GAIN_MIN_DB, GAIN_MAX_DB)
}

fn gain_fader_handle_y(bounds_height: f32, handle_center_y: f32) -> f32 {
    let min_y = 2.0;
    let max_y = (bounds_height - FADER_HANDLE_HEIGHT - 2.0).max(min_y);
    (handle_center_y - FADER_HANDLE_HEIGHT * 0.5).clamp(min_y, max_y)
}

#[cfg(test)]
fn horizontal_slider_handle_x(bounds_width: f32, handle_center_x: f32) -> f32 {
    horizontal_slider_handle_x_with_width(
        bounds_width,
        handle_center_x,
        HORIZONTAL_SLIDER_HANDLE_WIDTH,
    )
}

fn horizontal_slider_handle_x_with_width(
    bounds_width: f32,
    handle_center_x: f32,
    handle_width: f32,
) -> f32 {
    let min_x = 0.0;
    let max_x = (bounds_width - handle_width).max(min_x);
    (handle_center_x - handle_width * 0.5).clamp(min_x, max_x)
}

fn gain_fader_rail_bounds(bounds: Rectangle) -> Rectangle {
    let rail_width = FADER_RAIL_WIDTH;
    let rail_x = (bounds.width - rail_width) * 0.5;
    let (rail_y, rail_height) = fader_rail_layout(bounds.height);
    Rectangle {
        x: rail_x,
        y: rail_y,
        width: rail_width,
        height: rail_height,
    }
}

fn visible_gain_scale_marks(height: f32) -> Vec<f32> {
    let (_, rail_height) = fader_rail_layout(height);
    let base_gap = rail_height * 0.1;
    let stride = if base_gap <= 0.0 {
        GAIN_SCALE_DB_MARKS.len()
    } else {
        (GAIN_SCALE_LABEL_MIN_GAP / base_gap).ceil().max(1.0) as usize
    };

    let mut marks = Vec::with_capacity(GAIN_SCALE_DB_MARKS.len());
    for (index, db) in GAIN_SCALE_DB_MARKS.iter().copied().enumerate() {
        if (index == 0 || index == GAIN_SCALE_DB_MARKS.len() - 1 || index % stride == 0)
            && marks.last().copied() != Some(db)
        {
            marks.push(db);
        }
    }

    if marks.first().copied() != Some(GAIN_SCALE_DB_MARKS[0]) {
        marks.insert(0, GAIN_SCALE_DB_MARKS[0]);
    }
    let last_mark = GAIN_SCALE_DB_MARKS[GAIN_SCALE_DB_MARKS.len() - 1];
    if marks.last().copied() != Some(last_mark) {
        marks.push(last_mark);
    }

    marks
}

fn gain_scale_label(db: f32) -> String {
    if db > 0.0 {
        format!("+{db:.0}")
    } else {
        format!("{db:.0}")
    }
}

fn color_signature(color: Color) -> u64 {
    u64::from(color.r.to_bits())
        ^ u64::from(color.g.to_bits()).rotate_left(13)
        ^ u64::from(color.b.to_bits()).rotate_left(26)
        ^ u64::from(color.a.to_bits()).rotate_left(39)
}

fn gain_normalized_to_db(normalized: f32) -> f32 {
    gain_normalized_to_db_with_max(normalized, GAIN_MAX_DB)
}

fn gain_db_to_normalized(value: f32) -> f32 {
    gain_db_to_normalized_with_max(value, GAIN_MAX_DB)
}

fn gain_normalized_to_db_with_max(normalized: f32, max: f32) -> f32 {
    let normalized = normalized.clamp(0.0, 1.0);
    if normalized <= GAIN_TAPER_MINUS_40_POSITION {
        interpolate(
            normalized,
            0.0,
            GAIN_TAPER_MINUS_40_POSITION,
            GAIN_MIN_DB,
            -40.0,
        )
    } else if normalized <= GAIN_TAPER_MINUS_10_POSITION {
        interpolate(
            normalized,
            GAIN_TAPER_MINUS_40_POSITION,
            GAIN_TAPER_MINUS_10_POSITION,
            -40.0,
            -10.0,
        )
    } else if normalized <= GAIN_TAPER_ZERO_POSITION {
        interpolate(
            normalized,
            GAIN_TAPER_MINUS_10_POSITION,
            GAIN_TAPER_ZERO_POSITION,
            -10.0,
            0.0,
        )
    } else {
        interpolate(normalized, GAIN_TAPER_ZERO_POSITION, 1.0, 0.0, max.max(0.0))
    }
}

fn gain_db_to_normalized_with_max(value: f32, max: f32) -> f32 {
    let max = max.max(0.0);
    let value = value.clamp(GAIN_MIN_DB, max);
    if value <= -40.0 {
        interpolate(value, GAIN_MIN_DB, -40.0, 0.0, GAIN_TAPER_MINUS_40_POSITION)
    } else if value <= -10.0 {
        interpolate(
            value,
            -40.0,
            -10.0,
            GAIN_TAPER_MINUS_40_POSITION,
            GAIN_TAPER_MINUS_10_POSITION,
        )
    } else if value <= 0.0 {
        interpolate(
            value,
            -10.0,
            0.0,
            GAIN_TAPER_MINUS_10_POSITION,
            GAIN_TAPER_ZERO_POSITION,
        )
    } else if max == 0.0 {
        GAIN_TAPER_ZERO_POSITION
    } else {
        interpolate(value, 0.0, max, GAIN_TAPER_ZERO_POSITION, 1.0)
    }
}

fn interpolate(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    if (in_max - in_min).abs() <= f32::EPSILON {
        out_min
    } else {
        let ratio = ((value - in_min) / (in_max - in_min)).clamp(0.0, 1.0);
        out_min + ratio * (out_max - out_min)
    }
}

fn gain_value_to_y(value: f32, rail_bounds: Rectangle) -> f32 {
    rail_bounds.y + rail_bounds.height * (1.0 - gain_db_to_normalized(value))
}

fn y_to_gain_value(y: f32, rail_bounds: Rectangle) -> f32 {
    let normalized = (1.0 - ((y - rail_bounds.y) / rail_bounds.height.max(1.0))).clamp(0.0, 1.0);
    gain_normalized_to_db(normalized)
}

#[cfg(test)]
mod tests {
    use iced::widget::canvas::{self as canvas_widget, Program};
    use iced::{Point, Rectangle, keyboard, mouse};

    use std::time::{Duration, Instant};

    use super::{
        COMPACT_HORIZONTAL_SLIDER_METRICS, FINE_DRAG_MULTIPLIER, GAIN_MIN_DB, GAIN_SCALE_DB_MARKS,
        GAIN_SCALE_LABEL_MIN_GAP, GainFader, GainFaderState, HORIZONTAL_SLIDER_METRICS,
        HorizontalSlider, HorizontalSliderScale, HorizontalSliderState, Knob, KnobMode,
        gain_db_to_normalized, gain_db_to_normalized_with_max, gain_fader_handle_y,
        gain_fader_rail_bounds, gain_normalized_to_db, gain_normalized_to_db_with_max,
        gain_scale_label, gain_value_to_y, horizontal_slider_handle_x, is_double_click,
        visible_gain_scale_marks, y_to_gain_value,
    };

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

        assert_eq!(knob.apply_drag_delta(500.0, false), -60.0);
        assert_eq!(knob.apply_drag_delta(-500.0, false), 12.0);
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

        assert_eq!(y_to_gain_value(rail.y + rail.height, rail), GAIN_MIN_DB);
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

        let _ = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _ = Program::update(
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

        let _ = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _ = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(start),
        );
        let first_action = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position: first }),
            bounds,
            mouse::Cursor::Available(first),
        )
        .expect("first drag action");
        let (first_value, _, _) = first_action.into_inner();
        let first_value = first_value.expect("first value");

        let no_message = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::empty(),
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(no_message.is_none());

        let second_action = Program::update(
            &fader,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position: second }),
            bounds,
            mouse::Cursor::Available(second),
        )
        .expect("second drag action");
        let (second_value, _, _) = second_action.into_inner();
        let second_value = second_value.expect("second value");

        assert!(second_value > first_value);
        assert!(second_value - first_value < 2.0);
    }

    #[test]
    fn gain_fader_handle_y_stays_valid_for_short_bounds() {
        assert_eq!(gain_fader_handle_y(23.0, 12.0), 2.0);
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
        assert!(is_double_click(Some(now - Duration::from_millis(100)), now));
        assert!(!is_double_click(
            Some(now - Duration::from_millis(500)),
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

        assert_eq!(slider.value_for_cursor_x(-10.0, bounds), -36.0);
        assert_eq!(slider.value_for_cursor_x(400.0, bounds), 6.0);
        assert_eq!(slider.normalize(-12.24), -12.0);
        assert_eq!(slider.normalize(-12.26), -12.5);
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

        let _ = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _ = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(start),
        );
        let first_action = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position: first }),
            bounds,
            mouse::Cursor::Available(first),
        )
        .expect("first drag action");
        let (first_value, _, _) = first_action.into_inner();
        let first_value = first_value.expect("first value");

        let no_message = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::empty(),
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(no_message.is_none());

        let second_action = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position: second }),
            bounds,
            mouse::Cursor::Available(second),
        )
        .expect("second drag action");
        let (second_value, _, _) = second_action.into_inner();
        let second_value = second_value.expect("second value");

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

        let _ = Program::update(
            &slider,
            &mut state,
            &canvas_widget::Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let _ = Program::update(
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

        let _ = Program::update(
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
        assert_eq!(horizontal_slider_handle_x(19.0, 10.0), 0.0);
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
