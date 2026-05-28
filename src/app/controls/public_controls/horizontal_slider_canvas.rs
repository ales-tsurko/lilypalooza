use super::*;

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

impl<Message: Clone> Knob<'_, Message> {
    pub(super) fn handle_knob_press(
        &self,
        state: &mut KnobState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas_widget::Action<Message>> {
        cursor.position_over(bounds)?;
        let position = cursor.position()?;
        let now = Instant::now();
        if is_double_click(state.last_press_at, now) {
            return Some(self.reset_knob_after_double_click(state));
        }
        state.last_press_at = Some(now);
        state.dragging = true;
        state.last_cursor_y = Some(position.y);
        Some(canvas_widget::Action::capture())
    }

    fn reset_knob_after_double_click(
        &self,
        state: &mut KnobState,
    ) -> canvas_widget::Action<Message> {
        state.dragging = false;
        state.last_cursor_y = None;
        state.last_press_at = None;
        canvas_widget::Action::publish((self.on_change)(self.default_value)).and_capture()
    }

    pub(super) fn handle_knob_drag(
        &self,
        state: &mut KnobState,
        cursor_y: f32,
    ) -> Option<canvas_widget::Action<Message>> {
        if !state.dragging {
            return None;
        }
        let last_y = state.last_cursor_y?;
        state.last_cursor_y = Some(cursor_y);
        let next = self.apply_drag_delta(cursor_y - last_y, state.shift_pressed);
        Some(canvas_widget::Action::publish((self.on_change)(next)).and_capture())
    }

    pub(super) fn handle_knob_scroll(
        &self,
        delta: mouse::ScrollDelta,
    ) -> Option<canvas_widget::Action<Message>> {
        let next = self.apply_scroll_delta(delta);
        Some(canvas_widget::Action::publish((self.on_change)(next)).and_capture())
    }
}

impl<Message> Knob<'_, Message> {
    pub(in crate::app::controls) fn apply_drag_delta(&self, delta_y: f32, fine: bool) -> f32 {
        let drag_scalar = self.drag_scalar * if fine { FINE_DRAG_MULTIPLIER } else { 1.0 };
        match self.mode {
            KnobMode::Bipolar { .. } => self.normalize(self.value - delta_y * drag_scalar),
            KnobMode::Gain => {
                let normalized = (self.normalized_value() - delta_y * drag_scalar).clamp(0.0, 1.0);
                self.value_from_normalized(normalized)
            }
        }
    }

    pub(in crate::app::controls) fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
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

    pub(super) fn normalize(&self, value: f32) -> f32 {
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

    pub(super) fn normalized_value(&self) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => (self.value + 1.0) * 0.5,
            KnobMode::Gain => gain_db_to_normalized(self.value),
        }
    }

    pub(super) fn value_from_normalized(&self, normalized: f32) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => self.normalize(normalized * 2.0 - 1.0),
            KnobMode::Gain => gain_normalized_to_db(normalized),
        }
    }

    pub(in crate::app::controls) fn value_angle(&self) -> f32 {
        let start = KNOB_ANGLE_START.to_radians();
        let end = KNOB_ANGLE_END.to_radians();
        start + (end - start) * self.normalized_value()
    }

    pub(super) fn is_neutral(&self) -> bool {
        match self.mode {
            KnobMode::Bipolar { .. } => self.value == 0.0,
            KnobMode::Gain => self.value <= GAIN_MIN_DB,
        }
    }

    pub(super) fn active_start_angle(&self) -> f32 {
        self.active_angle(ActiveAngleEdge::Start)
    }

    pub(super) fn active_end_angle(&self) -> f32 {
        self.active_angle(ActiveAngleEdge::End)
    }

    fn active_angle(&self, edge: ActiveAngleEdge) -> f32 {
        match self.mode {
            KnobMode::Bipolar { .. } => match (edge, self.value < 0.0) {
                (ActiveAngleEdge::Start, true) | (ActiveAngleEdge::End, false) => {
                    self.value_angle()
                }
                _ => KNOB_CENTER_ANGLE.to_radians(),
            },
            KnobMode::Gain => match edge {
                ActiveAngleEdge::Start => KNOB_ANGLE_START.to_radians(),
                ActiveAngleEdge::End => self.value_angle(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ActiveAngleEdge {
    Start,
    End,
}

impl<Message> GainFader<'_, Message> {
    pub(in crate::app::controls) fn apply_scroll_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let amount = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * self.wheel_scalar,
            mouse::ScrollDelta::Pixels { y, .. } => y / 120.0 * self.wheel_scalar,
        };
        let normalized = (gain_db_to_normalized(self.value) + amount).clamp(0.0, 1.0);
        gain_normalized_to_db(normalized)
    }
}

impl<Message> HorizontalSlider<'_, Message> {
    pub(in crate::app::controls) fn normalize(&self, value: f32) -> f32 {
        let clamped = value.clamp(self.min, self.max);
        if self.step <= 0.0 {
            clamped
        } else {
            let steps = ((clamped - self.min) / self.step).round();
            (self.min + steps * self.step).clamp(self.min, self.max)
        }
    }

    pub(in crate::app::controls) fn value_for_cursor_x(&self, x: f32, bounds: Rectangle) -> f32 {
        let usable_width = (bounds.width - self.metrics.handle_width).max(1.0);
        let normalized = ((x - self.metrics.handle_width * 0.5) / usable_width).clamp(0.0, 1.0);
        self.normalize(self.value_from_normalized(normalized))
    }

    pub(in crate::app::controls) fn drag_normalized_delta(
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

    pub(in crate::app::controls) fn normalized_value(&self, value: f32) -> f32 {
        match self.scale {
            HorizontalSliderScale::Linear => {
                ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
            }
            HorizontalSliderScale::GainDb { max } => gain_db_to_normalized_with_max(value, max),
        }
    }

    pub(in crate::app::controls) fn value_from_normalized(&self, normalized: f32) -> f32 {
        match self.scale {
            HorizontalSliderScale::Linear => self.min + normalized * (self.max - self.min),
            HorizontalSliderScale::GainDb { max } => {
                gain_normalized_to_db_with_max(normalized, max)
            }
        }
    }
}
