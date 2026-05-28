use super::*;

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
