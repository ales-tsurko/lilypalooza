use std::time::{Duration, Instant};

use iced::widget::canvas::{self as canvas_widget, Path, Stroke};
use iced::widget::{canvas, container};
use iced::{Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme, alignment, mouse};

const FADER_WIDTH: f32 = 30.0;
const FADER_HANDLE_HEIGHT: f32 = 18.0;
const FADER_RAIL_WIDTH: f32 = 6.0;
const HORIZONTAL_SLIDER_HEIGHT: f32 = 24.0;
const HORIZONTAL_SLIDER_RAIL_HEIGHT: f32 = 6.0;
const HORIZONTAL_SLIDER_HANDLE_WIDTH: f32 = 18.0;
const HORIZONTAL_SLIDER_HANDLE_HEIGHT: f32 = 18.0;
const GAIN_KNOB_SIZE: f32 = 48.0;
const PAN_KNOB_SIZE: f32 = 42.0;
const KNOB_ANGLE_START: f32 = 135.0;
const KNOB_ANGLE_END: f32 = 405.0;
const KNOB_CENTER_ANGLE: f32 = (KNOB_ANGLE_START + KNOB_ANGLE_END) * 0.5;
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(350);
const PAN_ZERO_EPSILON: f32 = 0.025;
const DEFAULT_DRAG_SCALAR: f32 = 0.008;
const DEFAULT_WHEEL_SCALAR: f32 = 0.05;
const GAIN_MIN_DB: f32 = -60.0;
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
            mode: KnobMode::Unipolar {
                min: GAIN_MIN_DB,
                max: GAIN_MAX_DB,
            },
            default_value: 0.0,
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

pub(super) fn gain_fader<'a, Message: Clone + 'a>(
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    container(
        canvas(GainFader {
            value: clamp_gain(value),
            default_value: 0.0,
            drag_scalar: 0.35,
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
        on_change: Box::new(on_change),
    })
    .width(Length::Fill)
    .height(Length::Fixed(HORIZONTAL_SLIDER_HEIGHT))
    .into()
}

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
    drag_scalar: f32,
    wheel_scalar: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
}

struct HorizontalSlider<'a, Message> {
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    default_value: f32,
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
    last_press_at: Option<Instant>,
}

#[derive(Default)]
struct GainFaderState {
    dragging: bool,
    last_cursor_y: Option<f32>,
    last_press_at: Option<Instant>,
}

#[derive(Default)]
struct HorizontalSliderState {
    dragging: bool,
    last_press_at: Option<Instant>,
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
        let hover = state.dragging || cursor.position_in(bounds).is_some();
        let accent = if hover {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };

        let rail_width = FADER_RAIL_WIDTH;
        let rail_x = (bounds.width - rail_width) * 0.5;
        let (rail_y, rail_height) = fader_rail_layout(bounds.height);
        let rail_bounds = Rectangle {
            x: rail_x,
            y: rail_y,
            width: rail_width,
            height: rail_height,
        };
        let handle_center_y = gain_value_to_y(self.value, rail_bounds);
        let fill_bounds = Rectangle {
            x: rail_bounds.x,
            y: handle_center_y,
            width: rail_bounds.width,
            height: (rail_bounds.y + rail_bounds.height - handle_center_y).max(0.0),
        };
        let handle_bounds = Rectangle {
            x: 4.0,
            y: (handle_center_y - FADER_HANDLE_HEIGHT * 0.5)
                .clamp(2.0, bounds.height - FADER_HANDLE_HEIGHT - 2.0),
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
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(
                            self.value_for_cursor_x(position.x, bounds),
                        ))
                        .and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.dragging {
                    return Some(
                        canvas_widget::Action::publish((self.on_change)(
                            self.value_for_cursor_x(position.x - bounds.x, bounds),
                        ))
                        .and_capture(),
                    );
                }
            }
            canvas_widget::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_some() {
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
            }
            canvas_widget::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
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
        let rail_x = HORIZONTAL_SLIDER_HANDLE_WIDTH * 0.5;
        let rail_width = (bounds.width - HORIZONTAL_SLIDER_HANDLE_WIDTH).max(1.0);
        let rail_y = (bounds.height - HORIZONTAL_SLIDER_RAIL_HEIGHT) * 0.5;
        let handle_center_x = self.handle_center_x(bounds.width, self.normalize(self.value));
        let fill_width = (handle_center_x - rail_x).clamp(0.0, rail_width);
        let handle_bounds = Rectangle {
            x: (handle_center_x - HORIZONTAL_SLIDER_HANDLE_WIDTH * 0.5)
                .clamp(0.0, bounds.width - HORIZONTAL_SLIDER_HANDLE_WIDTH),
            y: (bounds.height - HORIZONTAL_SLIDER_HANDLE_HEIGHT) * 0.5,
            width: HORIZONTAL_SLIDER_HANDLE_WIDTH,
            height: HORIZONTAL_SLIDER_HANDLE_HEIGHT,
        };

        frame.fill(
            &Path::rounded_rectangle(
                Point::new(rail_x, rail_y),
                iced::Size::new(rail_width, HORIZONTAL_SLIDER_RAIL_HEIGHT),
                2.0.into(),
            ),
            palette.background.strong.color,
        );
        frame.fill(
            &Path::rounded_rectangle(
                Point::new(rail_x, rail_y),
                iced::Size::new(fill_width, HORIZONTAL_SLIDER_RAIL_HEIGHT),
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
        frame.stroke(
            &Path::line(
                Point::new(notch_x, handle_bounds.y + 4.0),
                Point::new(notch_x, handle_bounds.y + handle_bounds.height - 4.0),
            ),
            Stroke::default().with_width(2.0).with_color(accent),
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

impl<Message> GainFader<'_, Message> {
    fn apply_drag_delta(&self, delta_y: f32) -> f32 {
        clamp_gain(self.value - delta_y * self.drag_scalar)
    }

    fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let amount = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * self.wheel_scalar,
            mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.wheel_scalar,
        };
        clamp_gain(self.value + amount)
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
        let usable_width = (bounds.width - HORIZONTAL_SLIDER_HANDLE_WIDTH).max(1.0);
        let normalized =
            ((x - HORIZONTAL_SLIDER_HANDLE_WIDTH * 0.5) / usable_width).clamp(0.0, 1.0);
        self.normalize(self.min + normalized * (self.max - self.min))
    }

    fn handle_center_x(&self, width: f32, value: f32) -> f32 {
        let usable_width = (width - HORIZONTAL_SLIDER_HANDLE_WIDTH).max(1.0);
        let normalized = ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0);
        HORIZONTAL_SLIDER_HANDLE_WIDTH * 0.5 + usable_width * normalized
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

fn gain_value_to_y(value: f32, rail_bounds: Rectangle) -> f32 {
    let normalized =
        ((clamp_gain(value) - GAIN_MIN_DB) / (GAIN_MAX_DB - GAIN_MIN_DB)).clamp(0.0, 1.0);
    rail_bounds.y + rail_bounds.height * (1.0 - normalized)
}

#[cfg(test)]
mod tests {
    use iced::mouse;
    use iced::widget::canvas::{self as canvas_widget, Program};
    use iced::{Point, Rectangle};

    use std::time::{Duration, Instant};

    use super::{
        GainFader, HorizontalSlider, HorizontalSliderState, Knob, KnobMode, gain_value_to_y,
        is_double_click,
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
        assert!((knob.apply_drag_delta(-10.0) - 0.08).abs() < 1.0e-6);
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
            mode: KnobMode::Unipolar {
                min: -60.0,
                max: 12.0,
            },
            default_value: 0.0,
            drag_scalar: 0.35,
            wheel_scalar: 1.0,
            on_change: Box::new(|_| ()),
        };

        assert_eq!(knob.apply_drag_delta(500.0), -60.0);
        assert_eq!(knob.apply_drag_delta(-500.0), 12.0);
    }

    #[test]
    fn gain_fader_drag_delta_clamps_to_gain_range() {
        let fader = GainFader {
            value: -12.0,
            default_value: 0.0,
            drag_scalar: 0.35,
            wheel_scalar: 1.0,
            on_change: Box::new(|_| ()),
        };

        assert_eq!(fader.apply_drag_delta(500.0), -60.0);
        assert_eq!(fader.apply_drag_delta(-500.0), 12.0);
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
    fn horizontal_slider_double_click_publishes_default_value() {
        let slider = HorizontalSlider {
            value: -3.0,
            min: -36.0,
            max: 6.0,
            step: 0.5,
            default_value: -12.0,
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
}
