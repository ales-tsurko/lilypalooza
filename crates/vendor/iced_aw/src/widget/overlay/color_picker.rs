//! Use a color picker as an input element for picking colors.
//!
//! *This API requires the following crate features to be activated: `color_picker`*

use crate::{
    color_picker,
    core::color::{HexString, Hsv},
    style::{self, Status, color_picker::Style, style_state::StyleState},
};

use iced_core::{
    Alignment, Border, Clipboard, Color, Element, Event, Layout, Length, Overlay, Padding, Pixels,
    Point, Rectangle, Renderer as _, Shell, Size, Text, Vector, Widget,
    alignment::{Horizontal, Vertical},
    event, keyboard,
    layout::{Limits, Node},
    mouse::{self, Cursor},
    overlay, renderer,
    text::Renderer as _,
    touch,
    widget::{self, tree::Tree},
};
use iced_widget::{
    Button, Column, Renderer, Row, button,
    canvas::{self, LineCap, Path, Stroke},
    graphics::geometry::Renderer as _,
    text::{self, Wrapping},
};
use std::collections::HashMap;

/// The padding around the elements.
const PADDING: Padding = Padding::new(8.0);
/// The spacing between the element.
const SPACING: Pixels = Pixels(5.0);
/// The spacing between the buttons.
const BUTTON_SPACING: Pixels = Pixels(4.0);
/// The text size used across the picker UI.
const TEXT_SIZE: Pixels = Pixels(10.0);

/// The step value of the keyboard change of the sat/value color values.
const SAT_VALUE_STEP: f32 = 0.005;
/// The step value of the keyboard change of the hue color value.
const HUE_STEP: i32 = 1;
/// The step value of the keyboard change of the RGBA color values.
const RGBA_STEP: i16 = 1;

/// The overlay of the [`ColorPicker`](crate::widget::ColorPicker).
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog,
    'b: 'a,
{
    /// The state of the [`ColorPickerOverlay`].
    state: &'a mut State,
    /// The cancel message of the [`ColorPickerOverlay`].
    on_cancel: Message,
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Button<'a, Message, Theme, Renderer>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Button<'a, Message, Theme, Renderer>,
    /// The function that produces a message when the submit button of the [`ColorPickerOverlay`].
    on_submit: &'a dyn Fn(Color) -> Message,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<&'a dyn Fn(Color) -> Message>,
    /// The style of the [`ColorPickerOverlay`].
    class: &'a <Theme as style::color_picker::Catalog>::Class<'b>,
    /// The bounds of the control that opened the picker.
    target_bounds: Rectangle,
    /// The reference to the tree holding the state of this overlay.
    tree: &'a mut Tree,
    viewport: Rectangle,
}

impl<'a, 'b, Message, Theme> ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    'b: 'a,
{
    /// Creates a new [`ColorPickerOverlay`] on the given position.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut color_picker::State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Color) -> Message,
        on_color_change: Option<&'a dyn Fn(Color) -> Message>,
        class: &'a <Theme as style::color_picker::Catalog>::Class<'b>,
        target_bounds: Rectangle,
        tree: &'a mut Tree,
        viewport: Rectangle,
    ) -> Self {
        let color_picker::State { overlay_state, .. } = state;

        ColorPickerOverlay {
            state: overlay_state,
            on_cancel: on_cancel.clone(),
            cancel_button: Button::<'a, Message, Theme, Renderer>::new(
                iced_widget::Text::new("Cancel")
                    .size(TEXT_SIZE)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .on_press(on_cancel.clone()),
            submit_button: Button::<'a, Message, Theme, Renderer>::new(
                iced_widget::Text::new("Apply")
                    .size(TEXT_SIZE)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .on_press(on_cancel), // Sending a fake message
            on_submit,
            on_color_change,
            class,
            target_bounds,
            tree,
            viewport,
        }
    }

    /// Turn this [`ColorPickerOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// Force redraw all components if the internal state was changed
    fn clear_cache(&self) {
        self.state.clear_cache();
    }

    /// The event handling for the HSV color area.
    fn on_event_hsv_color(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        shell: &mut Shell<Message>,
    ) -> event::Status {
        let mut hsv_color_children = layout.children();

        let hsv_color: Hsv = self.state.color.into();
        let mut color_changed = false;

        let sat_value_bounds = hsv_color_children
            .next()
            .expect("widget: Layout should have a sat/value layout")
            .bounds();
        let hue_bounds = hsv_color_children
            .next()
            .expect("widget: Layout should have a hue layout")
            .bounds();

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_value =
                        |value: u16, y: f32| ((i32::from(value) + y as i32).rem_euclid(360)) as u16;

                    if cursor.is_over(hue_bounds) {
                        self.state.color = Color {
                            a: self.state.color.a,
                            ..Hsv {
                                hue: move_value(hsv_color.hue, *y),
                                ..hsv_color
                            }
                            .into()
                        };
                        color_changed = true;
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(sat_value_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::SatValue;
                    self.state.focus = Focus::SatValue;
                }
                if cursor.is_over(hue_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Hue;
                    self.state.focus = Focus::Hue;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        let calc_percentage_sat =
            |cursor_position: Point| (cursor_position.x.max(0.0) / sat_value_bounds.width).min(1.0);

        let calc_percentage_value = |cursor_position: Point| {
            (cursor_position.y.max(0.0) / sat_value_bounds.height).min(1.0)
        };

        let calc_hue = |cursor_position: Point| {
            ((cursor_position.x.max(0.0) / hue_bounds.width).min(1.0) * 360.0) as u16
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::SatValue => {
                self.state.color = Color {
                    a: self.state.color.a,
                    ..Hsv {
                        saturation: cursor
                            .position_in(sat_value_bounds)
                            .map(calc_percentage_sat)
                            .unwrap_or_default(),
                        value: cursor
                            .position_in(sat_value_bounds)
                            .map(calc_percentage_value)
                            .unwrap_or_default(),
                        ..hsv_color
                    }
                    .into()
                };
                color_changed = true;
            }
            ColorBarDragged::Hue => {
                self.state.color = Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue: cursor
                            .position_in(hue_bounds)
                            .map(calc_hue)
                            .unwrap_or_default(),
                        ..hsv_color
                    }
                    .into()
                };
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The event handling for the RGBA color area.
    #[allow(clippy::too_many_lines)]
    fn on_event_rgba_color(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        shell: &mut Shell<Message>,
    ) -> event::Status {
        let mut rgba_color_children = layout.children();
        let mut color_changed = false;

        let mut red_row_children = rgba_color_children
            .next()
            .expect("widget: Layout should have a red row layout")
            .children();
        let _ = red_row_children.next();
        let red_bar_bounds = red_row_children
            .next()
            .expect("widget: Layout should have a red bar layout")
            .bounds();

        let mut green_row_children = rgba_color_children
            .next()
            .expect("widget: Layout should have a green row layout")
            .children();
        let _ = green_row_children.next();
        let green_bar_bounds = green_row_children
            .next()
            .expect("widget: Layout should have a green bar layout")
            .bounds();

        let mut blue_row_children = rgba_color_children
            .next()
            .expect("widget: Layout should have a blue row layout")
            .children();
        let _ = blue_row_children.next();
        let blue_bar_bounds = blue_row_children
            .next()
            .expect("widget: Layout should have a blue bar layout")
            .bounds();

        let mut alpha_row_children = rgba_color_children
            .next()
            .expect("widget: Layout should have an alpha row layout")
            .children();
        let _ = alpha_row_children.next();
        let alpha_bar_bounds = alpha_row_children
            .next()
            .expect("widget: Layout should have an alpha bar layout")
            .bounds();

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_value =
                        //|value: f32, y: f32| (value * 255.0 + y).clamp(0.0, 255.0) / 255.0;
                        |value: f32, y: f32| value.mul_add(255.0, y).clamp(0.0, 255.0) / 255.0;

                    if cursor.is_over(red_bar_bounds) {
                        self.state.color = Color {
                            r: move_value(self.state.color.r, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(green_bar_bounds) {
                        self.state.color = Color {
                            g: move_value(self.state.color.g, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(blue_bar_bounds) {
                        self.state.color = Color {
                            b: move_value(self.state.color.b, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(alpha_bar_bounds) {
                        self.state.color = Color {
                            a: move_value(self.state.color.a, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(red_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Red;
                    self.state.focus = Focus::Red;
                }
                if cursor.is_over(green_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Green;
                    self.state.focus = Focus::Green;
                }
                if cursor.is_over(blue_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Blue;
                    self.state.focus = Focus::Blue;
                }
                if cursor.is_over(alpha_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Alpha;
                    self.state.focus = Focus::Alpha;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        let calc_percentage = |bounds: Rectangle, cursor_position: Point| {
            (cursor_position.x.max(0.0) / bounds.width).min(1.0)
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::Red => {
                self.state.color = Color {
                    r: cursor
                        .position_in(red_bar_bounds)
                        .map(|position| calc_percentage(red_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Green => {
                self.state.color = Color {
                    g: cursor
                        .position_in(green_bar_bounds)
                        .map(|position| calc_percentage(green_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Blue => {
                self.state.color = Color {
                    b: cursor
                        .position_in(blue_bar_bounds)
                        .map(|position| calc_percentage(blue_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Alpha => {
                self.state.color = Color {
                    a: cursor
                        .position_in(alpha_bar_bounds)
                        .map(|position| calc_percentage(alpha_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The even handling for the keyboard input.
    fn on_event_keyboard(&mut self, event: &Event, shell: &mut Shell<Message>) -> event::Status {
        if self.state.focus == Focus::None {
            return event::Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                shell.publish(self.on_cancel.clone());
                return event::Status::Captured;
            }
            let mut status = event::Status::Ignored;

            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab)) {
                if self.state.keyboard_modifiers.shift() {
                    self.state.focus = self.state.focus.previous();
                } else {
                    self.state.focus = self.state.focus.next();
                }
                // TODO: maybe place this better
                self.clear_cache();
            } else {
                let sat_value_handle = |key_code: &keyboard::Key, color: &mut Color| {
                    let mut hsv_color: Hsv = (*color).into();
                    let mut status = event::Status::Ignored;

                    match key_code {
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            hsv_color.saturation -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            hsv_color.saturation += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            hsv_color.value -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            hsv_color.value += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.saturation = hsv_color.saturation.clamp(0.0, 1.0);
                    hsv_color.value = hsv_color.value.clamp(0.0, 1.0);

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };
                    status
                };

                let hue_handle = |key_code: &keyboard::Key, color: &mut Color| {
                    let mut hsv_color: Hsv = (*color).into();
                    let mut status = event::Status::Ignored;

                    let mut value = i32::from(hsv_color.hue);

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            value -= HUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            value += HUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.hue = value.rem_euclid(360) as u16;

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };

                    status
                };

                let rgba_bar_handle = |key_code: &keyboard::Key, value: &mut f32| {
                    let mut byte_value = (*value * 255.0) as i16;
                    let mut status = event::Status::Captured;

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            byte_value -= RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            byte_value += RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }
                    *value = f32::from(byte_value.clamp(0, 255)) / 255.0;

                    status
                };

                match self.state.focus {
                    Focus::SatValue => status = sat_value_handle(key, &mut self.state.color),
                    Focus::Hue => status = hue_handle(key, &mut self.state.color),
                    Focus::Red => status = rgba_bar_handle(key, &mut self.state.color.r),
                    Focus::Green => status = rgba_bar_handle(key, &mut self.state.color.g),
                    Focus::Blue => status = rgba_bar_handle(key, &mut self.state.color.b),
                    Focus::Alpha => status = rgba_bar_handle(key, &mut self.state.color.a),
                    _ => {}
                }

                // If color changed via keyboard, call on_color_change callback
                if status == event::Status::Captured
                    && let Some(on_color_change) = self.on_color_change
                {
                    shell.publish(on_color_change(self.state.color));
                }
            }

            status
        } else if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            self.state.keyboard_modifiers = *modifiers;
            event::Status::Ignored
        } else {
            event::Status::Ignored
        }
    }
}

impl<'a, Message, Theme> Overlay<Message, Theme, Renderer>
    for ColorPickerOverlay<'a, '_, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let (max_width, max_height) = if bounds.width > bounds.height {
            (480.0, 260.0)
        } else {
            (260.0, 480.0)
        };

        let limits = Limits::new(Size::ZERO, bounds)
            .shrink(PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .max_width(max_width)
            .max_height(max_height);

        let divider = if bounds.width > bounds.height {
            Row::<(), Theme, Renderer>::new()
                .spacing(SPACING)
                .push(Row::new().width(Length::Fill).height(Length::Fill))
                .push(Row::new().width(Length::Fill).height(Length::Fill))
                .layout(self.tree, renderer, &limits)
        } else {
            Column::<(), Theme, Renderer>::new()
                .spacing(SPACING)
                .push(Row::new().width(Length::Fill).height(Length::Fill))
                .push(Row::new().width(Length::Fill).height(Length::Fill))
                .layout(self.tree, renderer, &limits)
        };

        let mut divider_children = divider.children().iter();

        let block1_bounds = divider_children
            .next()
            .expect("Divider should have a first child")
            .bounds();
        let block2_bounds = divider_children
            .next()
            .expect("Divider should have a second child")
            .bounds();

        // ----------- Block 1 ----------------------
        let block1_node = block1_layout(self, renderer, block1_bounds);

        // ----------- Block 2 ----------------------
        let block2_node = block2_layout(self, renderer, block2_bounds);

        let (width, height) = if bounds.width > bounds.height {
            (
                block1_node.size().width + block2_node.size().width + SPACING.0, // + (2.0 * PADDING as f32),
                block2_node.size().height,
            )
        } else {
            (
                block2_node.size().width,
                block1_node.size().height + block2_node.size().height + SPACING.0,
            )
        };

        let node = Node::with_children(Size::new(width, height), vec![block1_node, block2_node]);
        let size = node.size();

        let x = self.target_bounds.x.clamp(
            self.viewport.x,
            (self.viewport.x + self.viewport.width - size.width).max(self.viewport.x),
        );
        let space_below = (self.viewport.y + self.viewport.height)
            - (self.target_bounds.y + self.target_bounds.height);
        let space_above = self.target_bounds.y - self.viewport.y;
        let y = if space_below >= size.height || space_below >= space_above {
            self.target_bounds.y + self.target_bounds.height
        } else {
            (self.target_bounds.y - size.height).max(self.viewport.y)
        };

        node.move_to(Point::new(x, y))
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<Message>,
    ) {
        if event::Status::Captured == self.on_event_keyboard(event, shell) {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && !cursor.is_over(layout.bounds())
        {
            shell.publish(self.on_cancel.clone());
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        if let Event::Touch(touch::Event::FingerPressed { position, .. }) = event
            && !layout.bounds().contains(*position)
        {
            shell.publish(self.on_cancel.clone());
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        let mut children = layout.children();
        // ----------- Block 1 ----------------------
        let block1_layout = children
            .next()
            .expect("widget: Layout should have a 1. block layout");
        let hsv_color_status = self.on_event_hsv_color(event, block1_layout, cursor, shell);
        // ----------- Block 1 end ------------------

        // ----------- Block 2 ----------------------
        let mut block2_children = children
            .next()
            .expect("widget: Layout should have a 2. block layout")
            .children();

        // ----------- RGB Color -----------------------
        let rgba_color_layout = block2_children
            .next()
            .expect("widget: Layout should have a RGBA color layout");
        let rgba_color_status = self.on_event_rgba_color(event, rgba_color_layout, cursor, shell);

        // ----------- Text input ----------------------
        let _text_input_layout = block2_children
            .next()
            .expect("widget: Layout should have a hex text layout");

        // ----------- Buttons -------------------------
        let cancel_button_layout = block2_children
            .next()
            .expect("widget: Layout should have a cancel button layout for a ColorPicker");
        let submit_button_layout = block2_children
            .next()
            .expect("widget: Layout should have a submit button layout for a ColorPicker");
        let cancel_bounds = cancel_button_layout.bounds();
        let submit_bounds = submit_button_layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(cancel_bounds) {
                    self.state.pressed_button = PickerButton::Cancel;
                    self.state.focus = Focus::Cancel;
                    shell.capture_event();
                    shell.request_redraw();
                } else if cursor.is_over(submit_bounds) {
                    self.state.pressed_button = PickerButton::Submit;
                    self.state.focus = Focus::Submit;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Touch(touch::Event::FingerPressed { position, .. }) => {
                if cancel_bounds.contains(*position) {
                    self.state.pressed_button = PickerButton::Cancel;
                    self.state.focus = Focus::Cancel;
                    shell.capture_event();
                    shell.request_redraw();
                } else if submit_bounds.contains(*position) {
                    self.state.pressed_button = PickerButton::Submit;
                    self.state.focus = Focus::Submit;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                match self.state.pressed_button {
                    PickerButton::Cancel if cursor.is_over(cancel_bounds) => {
                        shell.publish(self.on_cancel.clone());
                        shell.capture_event();
                    }
                    PickerButton::Submit if cursor.is_over(submit_bounds) => {
                        shell.publish((self.on_submit)(self.state.color));
                        shell.capture_event();
                    }
                    _ => {}
                }
                if self.state.pressed_button != PickerButton::None {
                    self.state.pressed_button = PickerButton::None;
                    shell.request_redraw();
                }
            }
            Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if self.state.pressed_button != PickerButton::None {
                    self.state.pressed_button = PickerButton::None;
                    shell.request_redraw();
                }
            }
            _ => {}
        }
        // ----------- Block 2 end ------------------

        if hsv_color_status == event::Status::Captured
            || rgba_color_status == event::Status::Captured
        {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut children = layout.children();

        let mouse_interaction = mouse::Interaction::default();

        // Block 1
        let block1_layout = children
            .next()
            .expect("Graphics: Layout should have a 1. block layout");
        let mut block1_mouse_interaction = mouse::Interaction::default();
        // HSV color
        let mut hsv_color_children = block1_layout.children();
        let sat_value_layout = hsv_color_children
            .next()
            .expect("Graphics: Layout should have a sat/value layout");
        if cursor.is_over(sat_value_layout.bounds()) {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }
        let hue_layout = hsv_color_children
            .next()
            .expect("Graphics: Layout should have a hue layout");
        if cursor.is_over(hue_layout.bounds()) {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }

        // Block 2
        let block2_layout = children
            .next()
            .expect("Graphics: Layout should have a 2. block layout");
        let mut block2_mouse_interaction = mouse::Interaction::default();
        let mut block2_children = block2_layout.children();
        // RGBA color
        let rgba_color_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a RGBA color layout");
        let mut rgba_color_children = rgba_color_layout.children();

        let f = |layout: Layout<'_>, cursor: Cursor| {
            let mut children = layout.children();

            let _label_layout = children.next();
            let bar_layout = children
                .next()
                .expect("Graphics: Layout should have a bar layout");

            if cursor.is_over(bar_layout.bounds()) {
                mouse::Interaction::ResizingHorizontally
            } else {
                mouse::Interaction::default()
            }
        };
        let red_row_layout = rgba_color_children
            .next()
            .expect("Graphics: Layout should have a red row layout");
        block2_mouse_interaction = block2_mouse_interaction.max(f(red_row_layout, cursor));
        let green_row_layout = rgba_color_children
            .next()
            .expect("Graphics: Layout should have a green row layout");
        block2_mouse_interaction = block2_mouse_interaction.max(f(green_row_layout, cursor));
        let blue_row_layout = rgba_color_children
            .next()
            .expect("Graphics: Layout should have a blue row layout");
        block2_mouse_interaction = block2_mouse_interaction.max(f(blue_row_layout, cursor));
        let alpha_row_layout = rgba_color_children
            .next()
            .expect("Graphics: Layout should have an alpha row layout");
        block2_mouse_interaction = block2_mouse_interaction.max(f(alpha_row_layout, cursor));

        let _hex_text_layout = block2_children.next();

        // Buttons
        let cancel_button_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a cancel button layout for a ColorPicker");
        let cancel_mouse_interaction = if cursor.is_over(cancel_button_layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        };

        let submit_button_layout = block2_children
            .next()
            .expect("Graphics: Layout should have a submit button layout for a ColorPicker");
        let submit_mouse_interaction = if cursor.is_over(submit_button_layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        };

        mouse_interaction
            .max(block1_mouse_interaction)
            .max(block2_mouse_interaction)
            .max(cancel_mouse_interaction)
            .max(submit_mouse_interaction)
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let mut children = layout.children();

        // Skip block 1 (HSV color area)
        let _block1_layout = children.next();

        // Block 2 contains the buttons
        if let Some(block2_layout) = children.next() {
            let mut block2_children = block2_layout.children();

            // Skip rgba_colors, hex_text
            let _rgba_layout = block2_children.next();
            let _hex_text_layout = block2_children.next();

            // Operate on cancel button
            if let Some(cancel_layout) = block2_children.next() {
                Widget::operate(
                    &mut self.cancel_button,
                    &mut self.tree.children[0],
                    cancel_layout,
                    renderer,
                    operation,
                );
            }

            // Operate on submit button
            if let Some(submit_layout) = block2_children.next() {
                Widget::operate(
                    &mut self.submit_button,
                    &mut self.tree.children[1],
                    submit_layout,
                    renderer,
                    operation,
                );
            }
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        let bounds = layout.bounds();
        let mut children = layout.children();

        let mut style_sheet: HashMap<StyleState, Style> = HashMap::new();
        let _ = style_sheet.insert(
            StyleState::Active,
            style::color_picker::Catalog::style(theme, self.class, Status::Active),
        );
        let _ = style_sheet.insert(
            StyleState::Selected,
            style::color_picker::Catalog::style(theme, self.class, Status::Selected),
        );
        let _ = style_sheet.insert(
            StyleState::Hovered,
            style::color_picker::Catalog::style(theme, self.class, Status::Hovered),
        );
        let _ = style_sheet.insert(
            StyleState::Focused,
            style::color_picker::Catalog::style(theme, self.class, Status::Focused),
        );

        let mut style_state = StyleState::Active;
        if self.state.focus == Focus::Overlay {
            style_state = style_state.max(StyleState::Focused);
        }
        if cursor.is_over(bounds) {
            style_state = style_state.max(StyleState::Hovered);
        }

        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet[&style_state].border_radius.into(),
                        width: style_sheet[&style_state].border_width,
                        color: style_sheet[&style_state].border_color,
                    },
                    ..renderer::Quad::default()
                },
                style_sheet[&style_state].background,
            );
        }

        // ----------- Block 1 ----------------------
        let block1_layout = children
            .next()
            .expect("Graphics: Layout should have a 1. block layout");
        block1(renderer, self, block1_layout, cursor, &style_sheet);

        // ----------- Block 2 ----------------------
        let block2_layout = children
            .next()
            .expect("Graphics: Layout should have a 2. block layout");
        block2(
            renderer,
            self,
            block2_layout,
            cursor,
            theme,
            style,
            &bounds,
            &style_sheet,
        );
    }
}

/// Defines the layout of the 1. block of the color picker containing the HSV part.
fn block1_layout<'a, Message, Theme>(
    color_picker: &mut ColorPickerOverlay<'_, '_, Message, Theme>,
    renderer: &Renderer,
    bounds: Rectangle,
) -> Node
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    let block1_limits = Limits::new(Size::ZERO, bounds.size())
        .width(Length::Fill)
        .height(Length::Fill);

    let block1_node = Column::<(), Theme, Renderer>::new()
        .spacing(PADDING.y() / 2.) // Average vertical padding
        .push(
            Row::new()
                .width(Length::Fill)
                .height(Length::FillPortion(7)),
        )
        .push(
            Row::new()
                .width(Length::Fill)
                .height(Length::FillPortion(1)),
        )
        .layout(color_picker.tree, renderer, &block1_limits);

    block1_node.move_to(Point::new(bounds.x + PADDING.left, bounds.y + PADDING.top))
}

/// Defines the layout of the 2. block of the color picker containing the RGBA part, Hex and buttons.
fn block2_layout<'a, Message, Theme>(
    color_picker: &mut ColorPickerOverlay<'_, '_, Message, Theme>,
    renderer: &Renderer,
    bounds: Rectangle,
) -> Node
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    let block2_limits = Limits::new(Size::ZERO, bounds.size())
        .width(Length::Fill)
        .height(Length::Fill);

    // Pre-Buttons TODO: get rid of it
    let cancel_limits = block2_limits;
    let cancel_button = color_picker.cancel_button.layout(
        &mut color_picker.tree.children[0],
        renderer,
        &cancel_limits,
    );

    let hex_text_limits = block2_limits;

    let mut hex_text_layout = Row::<Message, Theme, Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fixed(TEXT_SIZE.0 + PADDING.y()))
        .layout(color_picker.tree, renderer, &hex_text_limits);

    let block2_limits = block2_limits.shrink(Size::new(
        0.0,
        cancel_button.bounds().height + hex_text_layout.bounds().height + 2.0 * SPACING.0,
    ));

    // RGBA Colors
    let mut rgba_colors: Column<'_, Message, Theme, Renderer> =
        Column::<Message, Theme, Renderer>::new();

    for _ in 0..4 {
        rgba_colors = rgba_colors.push(
            Row::new()
                .align_y(Alignment::Center)
                .spacing(SPACING)
                .padding(PADDING)
                .height(Length::Fill)
                .push(
                    widget::Text::new("X:")
                        .size(TEXT_SIZE)
                        .align_x(Horizontal::Center)
                        .align_y(Vertical::Center),
                )
                .push(
                    Row::new()
                        .width(Length::FillPortion(5))
                        .height(Length::Fill),
                )
                .push(
                    widget::Text::new("XXX")
                        .size(TEXT_SIZE)
                        .align_x(Horizontal::Center)
                        .align_y(Vertical::Center),
                ),
        );
    }
    let mut element: Element<Message, Theme, Renderer> = Element::new(rgba_colors);
    let rgba_tree = if let Some(child_tree) = color_picker.tree.children.get_mut(2) {
        child_tree.diff(element.as_widget_mut());
        child_tree
    } else {
        let child_tree = Tree::new(element.as_widget());
        color_picker.tree.children.insert(2, child_tree);
        &mut color_picker.tree.children[2]
    };

    let mut rgba_colors = element
        .as_widget_mut()
        .layout(rgba_tree, renderer, &block2_limits);

    let rgba_bounds = rgba_colors.bounds();
    rgba_colors = rgba_colors.move_to(Point::new(
        rgba_bounds.x + PADDING.left,
        rgba_bounds.y + PADDING.top,
    ));
    let rgba_bounds = rgba_colors.bounds();

    // Hex text
    let hex_bounds = hex_text_layout.bounds();
    hex_text_layout = hex_text_layout.move_to(Point::new(
        hex_bounds.x + PADDING.left,
        hex_bounds.y + rgba_bounds.height + PADDING.top + SPACING.0,
    ));
    let hex_bounds = hex_text_layout.bounds();

    // Buttons
    let cancel_limits =
        block2_limits.max_width(((rgba_bounds.width / 2.0) - BUTTON_SPACING.0).max(0.0));

    let mut cancel_button = color_picker.cancel_button.layout(
        &mut color_picker.tree.children[0],
        renderer,
        &cancel_limits,
    );

    let submit_limits =
        block2_limits.max_width(((rgba_bounds.width / 2.0) - BUTTON_SPACING.0).max(0.0));

    let mut submit_button = color_picker.submit_button.layout(
        &mut color_picker.tree.children[1],
        renderer,
        &submit_limits,
    );

    let cancel_bounds = cancel_button.bounds();
    cancel_button = cancel_button.move_to(Point::new(
        cancel_bounds.x + PADDING.left,
        cancel_bounds.y + rgba_bounds.height + hex_bounds.height + PADDING.top + 2.0 * SPACING.0,
    ));
    let cancel_bounds = cancel_button.bounds();

    let submit_bounds = submit_button.bounds();
    submit_button = submit_button.move_to(Point::new(
        submit_bounds.x + rgba_colors.bounds().width - submit_bounds.width + PADDING.left,
        submit_bounds.y + rgba_bounds.height + hex_bounds.height + PADDING.top + 2.0 * SPACING.0,
    ));

    Node::with_children(
        Size::new(
            rgba_bounds.width + PADDING.x(),
            rgba_bounds.height
                + hex_bounds.height
                + cancel_bounds.height
                + PADDING.y()
                + (2.0 * SPACING.0),
        ),
        vec![rgba_colors, hex_text_layout, cancel_button, submit_button],
    )
    .move_to(Point::new(bounds.x, bounds.y))
}

/// Draws the 1. block of the color picker containing the HSV part.
fn block1<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    // ----------- Block 1 ----------------------
    let hsv_color_layout = layout;

    // ----------- HSV Color ----------------------
    hsv_color(
        renderer,
        color_picker,
        hsv_color_layout,
        cursor,
        style_sheet,
    );

    // ----------- Block 1 end ------------------
}

/// Draws the 2. block of the color picker containing the RGBA part, Hex and buttons.
#[allow(clippy::too_many_arguments)]
fn block2<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    theme: &Theme,
    style: &renderer::Style,
    _viewport: &Rectangle,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    // ----------- Block 2 ----------------------
    let mut block2_children = layout.children();

    // ----------- RGBA Color ----------------------
    let rgba_color_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a RGBA color layout");
    rgba_color(
        renderer,
        rgba_color_layout,
        &color_picker.state.color,
        cursor,
        style,
        style_sheet,
        color_picker.state.focus,
    );

    // ----------- Hex text ----------------------
    let hex_text_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a hex text layout");
    hex_text(
        renderer,
        hex_text_layout,
        &color_picker.state.color,
        cursor,
        style,
        style_sheet,
        color_picker.state.focus,
    );

    // ----------- Buttons -------------------------
    let cancel_button_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a cancel button layout for a ColorPicker");

    let submit_button_layout = block2_children
        .next()
        .expect("Graphics: Layout should have a submit button layout for a ColorPicker");
    draw_picker_button(
        renderer,
        cancel_button_layout.bounds(),
        "Cancel",
        &cancel_button_style(
            theme,
            picker_button_status(
                color_picker.state,
                PickerButton::Cancel,
                cancel_button_layout.bounds(),
                cursor,
            ),
        ),
    );
    draw_picker_button(
        renderer,
        submit_button_layout.bounds(),
        "Apply",
        &apply_button_style(
            theme,
            picker_button_status(
                color_picker.state,
                PickerButton::Submit,
                submit_button_layout.bounds(),
                cursor,
            ),
        ),
    );

    // Buttons are not focusable right now...
    if color_picker.state.focus == Focus::Cancel {
        let bounds = cancel_button_layout.bounds();
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet[&StyleState::Focused].border_radius.into(),
                        width: style_sheet[&StyleState::Focused].border_width,
                        color: style_sheet[&StyleState::Focused].border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }

    if color_picker.state.focus == Focus::Submit {
        let bounds = submit_button_layout.bounds();
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet[&StyleState::Focused].border_radius.into(),
                        width: style_sheet[&StyleState::Focused].border_width,
                        color: style_sheet[&StyleState::Focused].border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }
    // ----------- Block 2 end ------------------
}

/// Draws the HSV color area.
#[allow(clippy::too_many_lines)]
fn hsv_color<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    layout: Layout<'_>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    let mut hsv_color_children = layout.children();
    let hsv_color: Hsv = color_picker.state.color.into();

    let sat_value_layout = hsv_color_children
        .next()
        .expect("Graphics: Layout should have a sat/value layout");
    let mut sat_value_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::SatValue {
        sat_value_style_state = sat_value_style_state.max(StyleState::Focused);
    }
    if cursor.is_over(sat_value_layout.bounds()) {
        sat_value_style_state = sat_value_style_state.max(StyleState::Hovered);
    }

    let geometry = color_picker.state.sat_value_canvas_cache.draw(
        renderer,
        sat_value_layout.bounds().size(),
        |frame| {
            let column_count = frame.width() as u16;
            let row_count = frame.height() as u16;

            for column in 0..column_count {
                for row in 0..row_count {
                    let saturation = f32::from(column) / frame.width();
                    let value = f32::from(row) / frame.height();

                    frame.fill_rectangle(
                        Point::new(f32::from(column), f32::from(row)),
                        Size::new(1.0, 1.0),
                        Color::from(Hsv::from_hsv(hsv_color.hue, saturation, value)),
                    );
                }
            }

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    Hsv {
                        hue: 0,
                        saturation: 0.0,
                        value: 1.0 - hsv_color.value,
                    }
                    .into(),
                ),
                width: 3.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            let saturation = hsv_color.saturation * frame.width();
            let value = hsv_color.value * frame.height();

            frame.stroke(
                &Path::line(
                    Point::new(saturation, 0.0),
                    Point::new(saturation, frame.height()),
                ),
                stroke,
            );

            frame.stroke(
                &Path::line(Point::new(0.0, value), Point::new(frame.width(), value)),
                stroke,
            );

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    style_sheet
                        .get(&sat_value_style_state)
                        .expect("Style Sheet not found.")
                        .bar_border_color,
                ),
                width: 2.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            frame.stroke(
                &Path::rectangle(
                    Point::new(0.0, 0.0),
                    Size::new(frame.size().width - 0.0, frame.size().height - 0.0),
                ),
                stroke,
            );
        },
    );

    let translation = Vector::new(sat_value_layout.bounds().x, sat_value_layout.bounds().y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });

    let hue_layout = hsv_color_children
        .next()
        .expect("Graphics: Layout should have a hue layout");
    let mut hue_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::Hue {
        hue_style_state = hue_style_state.max(StyleState::Focused);
    }
    if cursor.is_over(hue_layout.bounds()) {
        hue_style_state = hue_style_state.max(StyleState::Hovered);
    }

    let geometry =
        color_picker
            .state
            .hue_canvas_cache
            .draw(renderer, hue_layout.bounds().size(), |frame| {
                let column_count = frame.width() as u16;

                for column in 0..column_count {
                    let hue = (f32::from(column) * 360.0 / frame.width()) as u16;

                    let hsv_color = Hsv::from_hsv(hue, 1.0, 1.0);
                    let stroke = Stroke {
                        style: canvas::Style::Solid(hsv_color.into()),
                        width: 1.0,
                        line_cap: LineCap::Round,
                        ..Stroke::default()
                    };

                    frame.stroke(
                        &Path::line(
                            Point::new(f32::from(column), 0.0),
                            Point::new(f32::from(column), frame.height()),
                        ),
                        stroke,
                    );
                }

                let stroke = Stroke {
                    style: canvas::Style::Solid(Color::BLACK),
                    width: 3.0,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                };

                let column = f32::from(hsv_color.hue) * frame.width() / 360.0;

                frame.stroke(
                    &Path::line(Point::new(column, 0.0), Point::new(column, frame.height())),
                    stroke,
                );

                let stroke = Stroke {
                    style: canvas::Style::Solid(
                        style_sheet
                            .get(&hue_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_color,
                    ),
                    width: 2.0,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                };

                frame.stroke(
                    &Path::rectangle(
                        Point::new(0.0, 0.0),
                        Size::new(frame.size().width, frame.size().height),
                    ),
                    stroke,
                );
            });

    let translation = Vector::new(hue_layout.bounds().x, hue_layout.bounds().y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });
}

/// Draws the RGBA color area.
#[allow(clippy::too_many_lines)]
fn rgba_color(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    color: &Color,
    cursor: Cursor,
    style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
    focus: Focus,
) {
    let mut rgba_color_children = layout.children();

    let f = |renderer: &mut Renderer,
             layout: Layout,
             label: &str,
             color: Color,
             value: f32,
             cursor: Cursor,
             target: Focus| {
        let mut children = layout.children();

        let label_layout = children
            .next()
            .expect("Graphics: Layout should have a label layout");
        let bar_layout = children
            .next()
            .expect("Graphics: Layout should have a bar layout");
        let value_layout = children
            .next()
            .expect("Graphics: Layout should have a value layout");

        // Label
        renderer.fill_text(
            Text {
                content: label.to_owned(),
                bounds: Size::new(label_layout.bounds().width, label_layout.bounds().height),
                size: TEXT_SIZE,
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::None,
            },
            Point::new(
                label_layout.bounds().center_x(),
                label_layout.bounds().center_y(),
            ),
            style.text_color,
            label_layout.bounds(),
        );

        let bar_bounds = bar_layout.bounds();

        let bar_style_state = if cursor.is_over(bar_bounds) {
            StyleState::Hovered
        } else {
            StyleState::Active
        };

        // Bar background
        let background_bounds = Rectangle {
            x: bar_bounds.x,
            y: bar_bounds.y,
            width: bar_bounds.width * value,
            height: bar_bounds.height,
        };
        if (background_bounds.width > 0.) && (background_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: background_bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_radius
                            .into(),
                        width: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_width,
                        color: Color::TRANSPARENT,
                    },
                    ..renderer::Quad::default()
                },
                color,
            );
        }

        // Bar
        if (bar_bounds.width > 0.) && (bar_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: bar_bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_radius
                            .into(),
                        width: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_width,
                        color: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }

        // Value
        renderer.fill_text(
            Text {
                content: format!("{}", (255.0 * value) as u8),
                bounds: Size::new(value_layout.bounds().width, value_layout.bounds().height),
                size: TEXT_SIZE,
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: iced_widget::text::LineHeight::Relative(1.3),
                shaping: iced_widget::text::Shaping::Basic,
                wrapping: Wrapping::None,
            },
            Point::new(
                value_layout.bounds().center_x(),
                value_layout.bounds().center_y(),
            ),
            style.text_color,
            value_layout.bounds(),
        );

        let bounds = layout.bounds();
        if (focus == target) && (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_radius
                            .into(),
                        width: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_width,
                        color: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_color,
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    };

    // Red
    let red_row_layout = rgba_color_children
        .next()
        .expect("Graphics: Layout should have a red row layout");

    f(
        renderer,
        red_row_layout,
        "R",
        Color::from_rgb(color.r, 0.0, 0.0),
        color.r,
        cursor,
        Focus::Red,
    );

    // Green
    let green_row_layout = rgba_color_children
        .next()
        .expect("Graphics: Layout should have a green row layout");

    f(
        renderer,
        green_row_layout,
        "G",
        Color::from_rgb(0.0, color.g, 0.0),
        color.g,
        cursor,
        Focus::Green,
    );

    // Blue
    let blue_row_layout = rgba_color_children
        .next()
        .expect("Graphics: Layout should have a blue row layout");

    f(
        renderer,
        blue_row_layout,
        "B",
        Color::from_rgb(0.0, 0.0, color.b),
        color.b,
        cursor,
        Focus::Blue,
    );

    // Alpha
    let alpha_row_layout = rgba_color_children
        .next()
        .expect("Graphics: Layout should have an alpha row layout");

    f(
        renderer,
        alpha_row_layout,
        "A",
        Color::from_rgba(0.0, 0.0, 0.0, color.a),
        color.a,
        cursor,
        Focus::Alpha,
    );
}

/// Draws the hex text representation of the color.
fn hex_text(
    renderer: &mut Renderer,
    layout: Layout<'_>,
    color: &Color,
    cursor: Cursor,
    _style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
    _focus: Focus,
) {
    let hsv: Hsv = (*color).into();

    let hex_text_style_state = if cursor.is_over(layout.bounds()) {
        StyleState::Hovered
    } else {
        StyleState::Active
    };

    let bounds = layout.bounds();
    if (bounds.width > 0.) && (bounds.height > 0.) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: style_sheet[&hex_text_style_state].bar_border_radius.into(),
                    width: style_sheet[&hex_text_style_state].bar_border_width,
                    color: style_sheet[&hex_text_style_state].bar_border_color,
                },
                ..renderer::Quad::default()
            },
            *color,
        );
    }

    renderer.fill_text(
        Text {
            content: color.as_hex_string(),
            bounds: Size::new(bounds.width, bounds.height),
            size: TEXT_SIZE,
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: Wrapping::default(),
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        Color {
            a: 1.0,
            ..Hsv {
                hue: 0,
                saturation: 0.0,
                value: if hsv.value < 0.5 { 1.0 } else { 0.0 },
            }
            .into()
        },
        bounds,
    );
}

/// The state of the [`ColorPickerOverlay`].
#[derive(Debug)]
pub struct State {
    /// The selected color of the [`ColorPickerOverlay`].
    pub(crate) color: Color,
    /// The color used to initialize [`ColorPickerOverlay`].
    pub(crate) initial_color: Color,
    /// The cache of the sat/value canvas of the [`ColorPickerOverlay`].
    pub(crate) sat_value_canvas_cache: canvas::Cache,
    /// The cache of the hue canvas of the [`ColorPickerOverlay`].
    pub(crate) hue_canvas_cache: canvas::Cache,
    /// The dragged color bar of the [`ColorPickerOverlay`].
    pub(crate) color_bar_dragged: ColorBarDragged,
    /// the focus of the [`ColorPickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
    /// The currently pressed action button.
    pub(crate) pressed_button: PickerButton,
}

impl State {
    /// Creates a new State with the given color.
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            color,
            initial_color: color,
            ..Self::default()
        }
    }

    /// Reset cached canvas when internal state is modified.
    ///
    /// If the color has changed, empty all canvas caches
    /// as they (unfortunately) do not depend on the picker state.
    fn clear_cache(&self) {
        self.sat_value_canvas_cache.clear();
        self.hue_canvas_cache.clear();
    }

    /// Synchronize the color with an externally provided value.
    pub(crate) fn force_synchronize(&mut self, color: Color) {
        self.initial_color = color;
        self.color = color;
        self.clear_cache();
    }
}

impl Default for State {
    fn default() -> Self {
        let default_color = Color::from_rgb(0.5, 0.25, 0.25);
        Self {
            color: default_color,
            initial_color: default_color,
            sat_value_canvas_cache: canvas::Cache::default(),
            hue_canvas_cache: canvas::Cache::default(),
            color_bar_dragged: ColorBarDragged::None,
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            pressed_button: PickerButton::None,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
/// The currently pressed popup action button.
pub enum PickerButton {
    /// No button is pressed.
    #[default]
    None,
    /// The cancel action button.
    Cancel,
    /// The apply action button.
    Submit,
}

/// Just a workaround to pass the button states from the tree to the overlay
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog,
{
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Element<'a, Message, Theme, Renderer>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> Default for ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn default() -> Self {
        Self {
            cancel_button: Button::new(widget::Text::new("Cancel")).into(),
            submit_button: Button::new(widget::Text::new("Apply")).into(),
        }
    }
}

fn cancel_button_style<Theme>(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Color::from_rgb(0.26, 0.28, 0.32).into()),
        text_color: Color::from_rgb(0.96, 0.96, 0.97),
        border: Border::default()
            .rounded(4)
            .width(1)
            .color(Color::from_rgb(0.55, 0.57, 0.61)),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgb(0.31, 0.33, 0.38).into()),
            text_color: Color::WHITE,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Color::from_rgb(0.22, 0.24, 0.28).into()),
            text_color: Color::from_rgb(0.90, 0.90, 0.92),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Color::from_rgb(0.23, 0.24, 0.27).into()),
            text_color: Color::from_rgb(0.55, 0.56, 0.60),
            ..base
        },
    }
}

fn apply_button_style<Theme>(_theme: &Theme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(Color::from_rgb(0.35, 0.43, 0.62).into()),
        text_color: Color::from_rgb(0.97, 0.98, 1.0),
        border: Border::default()
            .rounded(4)
            .width(1)
            .color(Color::from_rgb(0.47, 0.56, 0.76)),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgb(0.41, 0.50, 0.71).into()),
            text_color: Color::WHITE,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(Color::from_rgb(0.29, 0.36, 0.53).into()),
            text_color: Color::from_rgb(0.92, 0.94, 0.98),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(Color::from_rgb(0.27, 0.31, 0.39).into()),
            text_color: Color::from_rgb(0.58, 0.62, 0.70),
            border: Border::default()
                .rounded(4)
                .width(1)
                .color(Color::from_rgb(0.42, 0.46, 0.54)),
            ..button::Style::default()
        },
    }
}

fn picker_button_status(
    state: &State,
    button_role: PickerButton,
    bounds: Rectangle,
    cursor: Cursor,
) -> button::Status {
    if state.pressed_button == button_role {
        button::Status::Pressed
    } else if cursor.is_over(bounds) {
        button::Status::Hovered
    } else {
        button::Status::Active
    }
}

fn draw_picker_button(
    renderer: &mut Renderer,
    bounds: Rectangle,
    label: &str,
    style: &button::Style,
) {
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.border,
            shadow: style.shadow,
            snap: style.snap,
        },
        style
            .background
            .unwrap_or(iced_core::Background::Color(Color::TRANSPARENT)),
    );

    renderer.fill_text(
        Text {
            content: label.to_owned(),
            bounds: Size::new(bounds.width, bounds.height),
            size: TEXT_SIZE,
            font: renderer.default_font(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: text::LineHeight::Relative(1.3),
            shaping: text::Shaping::Basic,
            wrapping: Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        style.text_color,
        bounds,
    );
}

#[allow(clippy::unimplemented)]
impl<Message, Theme> Widget<Message, Theme, Renderer>
    for ColorPickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.cancel_button),
            Tree::new(&self.submit_button),
        ]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.cancel_button, &self.submit_button]);
    }

    fn size(&self) -> Size<Length> {
        unimplemented!("This should never be reached!")
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        unimplemented!("This should never be reached!")
    }

    fn draw(
        &self,
        _state: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        unimplemented!("This should never be reached!")
    }
}

impl<'a, Message, Theme> From<ColorPickerOverlayButtons<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn from(overlay: ColorPickerOverlayButtons<'a, Message, Theme>) -> Self {
        Self::new(overlay)
    }
}

/// The state of the currently dragged area.
#[derive(Copy, Clone, Debug, Default)]
pub enum ColorBarDragged {
    /// No area is focussed.
    #[default]
    None,

    /// The saturation/value area is focussed.
    SatValue,

    /// The hue area is focussed.
    Hue,

    /// The red area is focussed.
    Red,

    /// The green area is focussed.
    Green,

    /// The blue area is focussed.
    Blue,

    /// The alpha area is focussed.
    Alpha,
}

/// An enumeration of all focusable element of the [`ColorPickerOverlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Nothing is in focus.
    #[default]
    None,

    /// The overlay itself is in focus.
    Overlay,

    /// The saturation and value area is in focus.
    SatValue,

    /// The hue bar is in focus.
    Hue,

    /// The red bar is in focus.
    Red,

    /// The green bar is in focus.
    Green,

    /// The blue bar is in focus.
    Blue,

    /// The alpha bar is in focus.
    Alpha,

    /// The cancel button is in focus.
    Cancel,

    /// The submit button is in focus.
    Submit,
}

impl Focus {
    /// Gets the next focusable element.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Overlay => Self::SatValue,
            Self::SatValue => Self::Hue,
            Self::Hue => Self::Red,
            Self::Red => Self::Green,
            Self::Green => Self::Blue,
            Self::Blue => Self::Alpha,
            Self::Alpha => Self::Cancel,
            Self::Cancel => Self::Submit,
            Self::Submit | Self::None => Self::Overlay,
        }
    }

    /// Gets the previous focusable element.
    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Overlay => Self::Submit,
            Self::SatValue => Self::Overlay,
            Self::Hue => Self::SatValue,
            Self::Red => Self::Hue,
            Self::Green => Self::Red,
            Self::Blue => Self::Green,
            Self::Alpha => Self::Blue,
            Self::Cancel => Self::Alpha,
            Self::Submit => Self::Cancel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::Theme;

    #[test]
    fn cancel_button_text_is_bright_and_hover_changes_style() {
        let active = cancel_button_style(&Theme::Dark, button::Status::Active);
        let hovered = cancel_button_style(&Theme::Dark, button::Status::Hovered);

        assert!(active.text_color.r > 0.9);
        assert!(active.text_color.g > 0.9);
        assert!(active.text_color.b > 0.9);
        assert_ne!(active.background, hovered.background);
    }

    #[test]
    fn apply_button_text_is_bright_and_hover_changes_style() {
        let active = apply_button_style(&Theme::Dark, button::Status::Active);
        let hovered = apply_button_style(&Theme::Dark, button::Status::Hovered);

        assert!(active.text_color.r > 0.9);
        assert!(active.text_color.g > 0.9);
        assert!(active.text_color.b > 0.9);
        assert_ne!(active.background, hovered.background);
    }
}
