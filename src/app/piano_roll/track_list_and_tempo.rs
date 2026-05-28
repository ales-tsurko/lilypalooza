use iced::widget::column;

use super::*;

pub(in crate::app) fn track_list<'a>(
    app: &'a Lilypalooza,
    file: &'a MidiRollFile,
    track_mix: &'a [TrackMixState],
    track_panel_width: f32,
) -> Element<'a, Message> {
    let mut tracks_column = column![]
        .width(Fill)
        .spacing(ui_style::SPACE_XS)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_XS]);
    let label_max_chars = max_track_label_chars(track_panel_width);

    for track in &file.data.tracks {
        tracks_column = tracks_column.push(track_list_row(app, track, track_mix, label_max_chars));
    }

    tracks_column = append_empty_track_notice(tracks_column, file);

    scrollable(row![
        container(tracks_column).width(Fill),
        container(text("")).width(TRACK_LIST_SCROLLBAR_GUTTER as f32)
    ])
    .id(track_list_scroll_id())
    .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new()))
    .style(ui_style::workspace_scrollable)
    .into()
}

pub(in crate::app) fn track_list_row<'a>(
    app: &'a Lilypalooza,
    track: &'a MidiTrack,
    track_mix: &'a [TrackMixState],
    label_max_chars: usize,
) -> Element<'a, Message> {
    let state = track_mix.get(track.index).copied().unwrap_or_default();
    let track_color = app.effective_track_color(track.index);
    container(track_list_row_content(
        app,
        track,
        track_mix,
        state,
        label_max_chars,
    ))
    .padding([4, 6])
    .style(move |theme| {
        ui_style::piano_roll_track_surface(
            theme,
            track_color,
            app.selected_track_index == Some(track.index),
        )
    })
    .into()
}

fn track_list_row_content<'a>(
    app: &'a Lilypalooza,
    track: &'a MidiTrack,
    track_mix: &'a [TrackMixState],
    state: TrackMixState,
    label_max_chars: usize,
) -> Element<'a, Message> {
    row![
        container(track_title(app, track, track_mix, label_max_chars))
            .width(Fill)
            .clip(true),
        container(text("")).width(Length::Fixed(TRACK_LABEL_BUTTON_GAP)),
        track_mix_buttons(track.index, state),
    ]
    .align_y(alignment::Vertical::Center)
    .spacing(0)
    .width(Fill)
    .into()
}

pub(in crate::app) fn track_title<'a>(
    app: &'a Lilypalooza,
    track: &'a MidiTrack,
    track_mix: &'a [TrackMixState],
    label_max_chars: usize,
) -> Element<'a, Message> {
    if app.is_renaming_track_in(track.index, crate::app::WorkspacePaneKind::PianoRoll) {
        return renaming_track_title(app);
    }
    normal_track_title(app, track, track_mix, label_max_chars)
}

pub(in crate::app) fn renaming_track_title(app: &Lilypalooza) -> Element<'_, Message> {
    let swatch = button(container(text("")).width(18).height(18))
        .padding(0)
        .width(18)
        .height(18)
        .style(move |theme, status| {
            ui_style::track_color_swatch_button(theme, status, app.track_rename_color_value)
        })
        .on_press(Message::PianoRoll(PianoRollMessage::OpenTrackColorPicker));
    let input = text_input::<Message, Theme, Renderer>("", &app.track_rename_value)
        .id(iced::widget::Id::new(crate::app::TRACK_RENAME_INPUT_ID))
        .on_input(|value| Message::PianoRoll(PianoRollMessage::TrackRenameInputChanged(value)))
        .on_submit(Message::PianoRoll(PianoRollMessage::CommitTrackRename))
        .style(ui_style::track_name_input)
        .size(ui_style::FONT_SIZE_UI_XS)
        .padding([2, 4])
        .width(Fill);
    let focused = app.track_rename_color_picker_open || app.track_rename_was_focused;
    let editor_row = container(
        row![
            swatch,
            container(text(""))
                .width(1)
                .height(18)
                .style(move |theme| { ui_style::track_name_editor_divider(theme, focused) }),
            input
        ]
        .spacing(0)
        .align_y(alignment::Vertical::Center)
        .width(Fill),
    )
    .padding(0)
    .style(move |theme| ui_style::track_name_editor_shell(theme, focused))
    .width(Fill);
    color_picker_with_change(
        app.track_rename_color_picker_open,
        app.track_rename_color_value,
        editor_row,
        Message::PianoRoll(PianoRollMessage::CancelTrackRename),
        |color| Message::PianoRoll(PianoRollMessage::SubmitTrackColor(color)),
        |color| Message::PianoRoll(PianoRollMessage::PreviewTrackColor(color)),
    )
    .style(ui_style::color_picker_widget_style)
    .into()
}

pub(in crate::app) fn normal_track_title<'a>(
    app: &'a Lilypalooza,
    track: &'a MidiTrack,
    track_mix: &'a [TrackMixState],
    label_max_chars: usize,
) -> Element<'a, Message> {
    let track_label = app.effective_track_name(track.index);
    let track_color = app.effective_track_color(track.index);
    let visibility_alpha =
        track_visibility_alpha(track_mix, track.index, app.piano_roll.global_solo_active);
    let swatch_color = Color {
        a: track_color.a * visibility_alpha,
        ..track_color
    };
    let label = track_selectable_label(track.index, shorten_label(&track_label, label_max_chars));
    row![
        track_color_picker(app, track.index, swatch_color),
        container(text("")).width(Length::Fixed(TRACK_COLOR_BUTTON_GAP)),
        label
    ]
    .align_y(alignment::Vertical::Center)
    .width(Fill)
    .into()
}

fn track_selectable_label(track_index: usize, label: String) -> Element<'static, Message> {
    mouse_area(
        container(
            text(label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Fill),
        )
        .width(Fill),
    )
    .on_press(Message::PianoRoll(PianoRollMessage::SelectTrack(
        track_index,
    )))
    .on_double_click(Message::PianoRoll(PianoRollMessage::StartTrackRename(
        track_index,
    )))
    .into()
}

pub(in crate::app) fn track_color_picker(
    app: &Lilypalooza,
    track_index: usize,
    swatch_color: Color,
) -> Element<'_, Message> {
    let swatch_button = button(
        container(text(""))
            .width(TRACK_COLOR_BUTTON_SIZE)
            .height(TRACK_COLOR_BUTTON_SIZE),
    )
    .padding(0)
    .width(TRACK_COLOR_BUTTON_SIZE)
    .height(TRACK_COLOR_BUTTON_SIZE)
    .style(move |theme, status| ui_style::track_color_swatch_button(theme, status, swatch_color))
    .on_press(Message::PianoRoll(
        PianoRollMessage::OpenTrackColorPickerForTrack(track_index),
    ));
    color_picker_with_change(
        app.is_picking_track_color_in(track_index, crate::app::WorkspacePaneKind::PianoRoll),
        app.track_rename_color_value,
        swatch_button,
        Message::PianoRoll(PianoRollMessage::CancelTrackRename),
        |color| Message::PianoRoll(PianoRollMessage::SubmitTrackColor(color)),
        |color| Message::PianoRoll(PianoRollMessage::PreviewTrackColor(color)),
    )
    .style(ui_style::color_picker_widget_style)
    .into()
}

pub(in crate::app) fn track_mix_buttons(
    track_index: usize,
    state: TrackMixState,
) -> Element<'static, Message> {
    row![
        track_mix_button(
            "M",
            state.muted,
            PianoRollMessage::TrackMuteToggled(track_index)
        ),
        container(text("")).width(Length::Fixed(TRACK_BUTTONS_GAP)),
        track_mix_button(
            "S",
            state.soloed,
            PianoRollMessage::TrackSoloToggled(track_index)
        ),
    ]
    .width(Length::Fixed(TRACK_BUTTONS_GROUP_WIDTH))
    .align_y(alignment::Vertical::Center)
    .into()
}

pub(in crate::app) fn track_mix_button(
    label: &'static str,
    active: bool,
    message: PianoRollMessage,
) -> iced::widget::Button<'static, Message> {
    button(
        container(
            text(label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(fonts::MONO)
                .center(),
        )
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill),
    )
    .padding(0)
    .width(Length::Fixed(TRACK_BUTTON_WIDTH))
    .height(Length::Fixed(TRACK_BUTTON_HEIGHT))
    .style(if active {
        ui_style::button_compact_active
    } else {
        ui_style::button_compact_solid
    })
    .on_press(Message::PianoRoll(message))
}

pub(in crate::app) fn append_empty_track_notice<'a>(
    tracks_column: iced::widget::Column<'a, Message>,
    file: &MidiRollFile,
) -> iced::widget::Column<'a, Message> {
    if file.data.tracks.len() > 1 {
        return tracks_column;
    }
    tracks_column.push(
        text("No parts")
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(fonts::MONO),
    )
}

pub(in crate::app) struct TempoStubCanvas;

impl<Message> canvas::Program<Message> for TempoStubCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        solid_canvas_geometry(
            renderer,
            bounds,
            theme.extended_palette().background.weak.color,
        )
    }
}

fn solid_canvas_geometry(
    renderer: &Renderer,
    bounds: Rectangle,
    color: Color,
) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    frame.fill_rectangle(Point::new(0.0, 0.0), bounds.size(), color);
    vec![frame.into_geometry()]
}

#[derive(Debug, Default)]
pub(in crate::app) struct TrackResizeState {
    pub(in crate::app) dragging: bool,
    pub(in crate::app) last_cursor_x: Option<f32>,
}

pub(in crate::app) struct TrackResizeHandle;

impl canvas::Program<Message> for TrackResizeHandle {
    type State = TrackResizeState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if track_resize_drag_started(event, bounds, cursor) {
            return Some(begin_track_resize_drag(state, cursor));
        }
        if track_resize_drag_moved(event, state.dragging) {
            return update_track_resize_drag(state, cursor);
        }
        if track_resize_drag_ended(event) {
            end_track_resize_drag(state);
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        track_resize_handle_geometry(renderer, theme, bounds)
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        track_resize_mouse_interaction(state, bounds, cursor)
    }
}

fn track_resize_handle_geometry(
    renderer: &Renderer,
    theme: &Theme,
    bounds: Rectangle,
) -> Vec<canvas::Geometry> {
    let palette = theme.extended_palette();
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    frame.fill_rectangle(
        Point::new(0.0, 0.0),
        bounds.size(),
        Color::from_rgba(
            palette.background.base.color.r,
            palette.background.base.color.g,
            palette.background.base.color.b,
            0.90,
        ),
    );
    frame.fill_rectangle(
        Point::new((bounds.width * 0.5).floor(), 0.0),
        Size::new(1.0, bounds.height),
        Color::from_rgba(
            palette.background.strong.color.r,
            palette.background.strong.color.g,
            palette.background.strong.color.b,
            0.55,
        ),
    );
    vec![frame.into_geometry()]
}

fn track_resize_mouse_interaction(
    state: &TrackResizeState,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> mouse::Interaction {
    if state.dragging || cursor.position_in(bounds).is_some() {
        mouse::Interaction::ResizingHorizontally
    } else {
        mouse::Interaction::None
    }
}

pub(in crate::app) fn begin_track_resize_drag(
    state: &mut TrackResizeState,
    cursor: mouse::Cursor,
) -> canvas::Action<Message> {
    state.dragging = true;
    state.last_cursor_x = cursor.position().map(|position| position.x);
    canvas::Action::capture()
}

pub(in crate::app) fn track_resize_drag_started(
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> bool {
    matches!(
        event,
        canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    ) && cursor.position_in(bounds).is_some()
}

pub(in crate::app) fn track_resize_drag_moved(event: &canvas::Event, dragging: bool) -> bool {
    dragging
        && matches!(
            event,
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. })
        )
}

pub(in crate::app) fn track_resize_drag_ended(event: &canvas::Event) -> bool {
    matches!(
        event,
        canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | canvas::Event::Mouse(mouse::Event::CursorLeft)
    )
}

pub(in crate::app) fn update_track_resize_drag(
    state: &mut TrackResizeState,
    cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
    let cursor_x = cursor.position().map(|position| position.x);
    let (Some(last_x), Some(cursor_x)) = (state.last_cursor_x, cursor_x) else {
        return None;
    };
    let delta = cursor_x - last_x;
    state.last_cursor_x = Some(cursor_x);
    Some(
        canvas::Action::publish(Message::PianoRoll(PianoRollMessage::TrackPanelResizedBy(
            delta,
        )))
        .and_capture(),
    )
}

pub(in crate::app) fn end_track_resize_drag(state: &mut TrackResizeState) {
    state.dragging = false;
    state.last_cursor_x = None;
}

pub(in crate::app) struct TempoCanvas<'a> {
    pub(in crate::app) data: &'a MidiRollData,
    pub(in crate::app) zoom_x: f32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) horizontal_scroll: f32,
    pub(in crate::app) playback_tick: u64,
    pub(in crate::app) rewind_flag_tick: u64,
}

#[derive(Debug, Default)]
pub(in crate::app) struct TempoCanvasState {
    pub(in crate::app) cache: Cache,
    pub(in crate::app) cache_key: Option<TempoCanvasCacheKey>,
    pub(in crate::app) rewind_flag_press_origin: Option<Point>,
    pub(in crate::app) dragging_rewind_flag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct TempoCanvasCacheKey {
    pub(in crate::app) total_ticks: u64,
    pub(in crate::app) ppq: u16,
    pub(in crate::app) zoom_x_bits: u32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) horizontal_scroll_bits: u32,
    pub(in crate::app) tempo_changes_len: usize,
    pub(in crate::app) bar_lines_len: usize,
}

impl TempoCanvas<'_> {
    fn cache_key(&self) -> TempoCanvasCacheKey {
        TempoCanvasCacheKey {
            total_ticks: self.data.total_ticks,
            ppq: self.data.ppq,
            zoom_x_bits: self.zoom_x.to_bits(),
            beat_subdivision: self.beat_subdivision,
            horizontal_scroll_bits: self.horizontal_scroll.to_bits(),
            tempo_changes_len: self.data.tempo_changes.len(),
            bar_lines_len: self.data.bar_lines.len(),
        }
    }

    fn pixels_per_tick(&self) -> f32 {
        BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1))
    }

    fn tick_at_cursor_x(&self, local_x: f32, pixels_per_tick: f32) -> u64 {
        tick_from_tempo_lane_x(
            local_x,
            pixels_per_tick,
            self.horizontal_scroll,
            self.data.total_ticks,
        )
    }

    fn snapped_tick_at_cursor_x(&self, local_x: f32, pixels_per_tick: f32) -> u64 {
        snap_tick_to_subdivision_grid(
            self.data,
            self.beat_subdivision,
            self.tick_at_cursor_x(local_x, pixels_per_tick),
        )
    }

    fn rewind_flag_contains(
        &self,
        cursor_position: Point,
        pixels_per_tick: f32,
        bounds: Rectangle,
    ) -> bool {
        rewind_flag_hitbox(
            self.rewind_flag_tick,
            pixels_per_tick,
            self.horizontal_scroll,
            bounds,
        )
        .contains(cursor_position)
    }

    fn refresh_cache(&self, state: &mut TempoCanvasState) {
        let cache_key = self.cache_key();
        if state.cache_key == Some(cache_key) {
            return;
        }

        state.cache.clear();
        state.cache_key = Some(cache_key);
    }

    pub(in crate::app) fn right_press_action(
        &self,
        state: &mut TempoCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        pixels_per_tick: f32,
    ) -> Option<canvas::Action<Message>> {
        let cursor_position = cursor.position_in(bounds)?;
        let tick = self.snapped_tick_at_cursor_x(cursor_position.x, pixels_per_tick);
        state.dragging_rewind_flag = false;
        Some(publish_rewind_flag_tick(tick))
    }

    pub(in crate::app) fn left_press_action(
        &self,
        state: &mut TempoCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        pixels_per_tick: f32,
    ) -> Option<canvas::Action<Message>> {
        let cursor_position = cursor.position_in(bounds)?;
        if self.rewind_flag_contains(cursor_position, pixels_per_tick, bounds) {
            state.rewind_flag_press_origin = Some(cursor_position);
            state.dragging_rewind_flag = false;
            return Some(canvas::Action::capture());
        }

        let tick = self.tick_at_cursor_x(cursor_position.x, pixels_per_tick);
        Some(publish_cursor_tick(tick))
    }

    pub(in crate::app) fn cursor_move_action(
        &self,
        state: &mut TempoCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        pixels_per_tick: f32,
    ) -> Option<canvas::Action<Message>> {
        let Some(cursor_position) = cursor.position_in(bounds) else {
            reset_rewind_flag_drag(state);
            return None;
        };

        if let Some(action) = continue_pending_rewind_flag_drag(state, cursor_position) {
            return Some(action);
        }

        let tick = self.snapped_tick_at_cursor_x(cursor_position.x, pixels_per_tick);
        Some(publish_rewind_flag_tick(tick))
    }
}

pub(in crate::app) fn publish_rewind_flag_tick(tick: u64) -> canvas::Action<Message> {
    canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetRewindFlagTicks(
        tick,
    )))
    .and_capture()
}

pub(in crate::app) fn publish_cursor_tick(tick: u64) -> canvas::Action<Message> {
    canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)))
        .and_capture()
}

pub(in crate::app) fn reset_rewind_flag_drag(state: &mut TempoCanvasState) {
    state.rewind_flag_press_origin = None;
    state.dragging_rewind_flag = false;
}

pub(in crate::app) fn continue_pending_rewind_flag_drag(
    state: &mut TempoCanvasState,
    cursor_position: Point,
) -> Option<canvas::Action<Message>> {
    if state.dragging_rewind_flag {
        return None;
    }

    let origin = state.rewind_flag_press_origin?;
    if drag_distance(origin, cursor_position) < DRAG_START_THRESHOLD {
        return Some(canvas::Action::capture());
    }

    state.dragging_rewind_flag = true;
    None
}

impl canvas::Program<Message> for TempoCanvas<'_> {
    type State = TempoCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        self.refresh_cache(state);
        let pixels_per_tick = self.pixels_per_tick();
        self.tempo_canvas_mouse_action(state, event, bounds, cursor, pixels_per_tick)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        self.draw_tempo_canvas(state, renderer, theme, bounds)
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        self.tempo_mouse_interaction(state, bounds, cursor)
    }
}

impl TempoCanvas<'_> {
    fn draw_tempo_canvas(
        &self,
        state: &TempoCanvasState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let pixels_per_tick = self.pixels_per_tick();
        let static_geometry =
            self.draw_static_tempo_geometry(state, renderer, bounds, palette, pixels_per_tick);
        let dynamic_geometry =
            self.draw_dynamic_tempo_geometry(renderer, bounds, palette, pixels_per_tick);
        vec![static_geometry, dynamic_geometry]
    }

    fn draw_static_tempo_geometry(
        &self,
        state: &TempoCanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
        pixels_per_tick: f32,
    ) -> canvas::Geometry {
        state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(
                Point::new(0.0, 0.0),
                bounds.size(),
                palette.background.weak.color,
            );
            draw_bar_lines(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                0.0,
                bounds.height,
                palette,
            );
            draw_bar_numbers(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                bounds.height,
                palette,
            );
            draw_tempo_markers(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                bounds.height,
                palette,
            );
        })
    }

    fn draw_dynamic_tempo_geometry(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
        pixels_per_tick: f32,
    ) -> canvas::Geometry {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        draw_rewind_flag(
            &mut frame,
            self.rewind_flag_tick,
            pixels_per_tick,
            self.horizontal_scroll,
            bounds.height,
            palette,
        );
        draw_playback_cursor(
            &mut frame,
            self.playback_tick,
            pixels_per_tick,
            self.horizontal_scroll,
            0.0,
            bounds.height,
            palette,
        );
        frame.into_geometry()
    }

    fn tempo_mouse_interaction(
        &self,
        state: &TempoCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging_rewind_flag {
            return mouse::Interaction::Grabbing;
        }
        if self.cursor_over_rewind_flag(bounds, cursor) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }

    fn cursor_over_rewind_flag(&self, bounds: Rectangle, cursor: mouse::Cursor) -> bool {
        cursor.position_in(bounds).is_some_and(|position| {
            rewind_flag_hitbox(
                self.rewind_flag_tick,
                self.pixels_per_tick(),
                self.horizontal_scroll,
                bounds,
            )
            .contains(position)
        })
    }
}

pub(in crate::app) struct KeyCanvas {
    pub(in crate::app) min_pitch: u8,
    pub(in crate::app) max_pitch: u8,
    pub(in crate::app) vertical_offset: f32,
}

impl<Message> canvas::Program<Message> for KeyCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        draw_key_canvas(self, renderer, bounds)
    }
}

fn draw_key_canvas(
    canvas: &KeyCanvas,
    renderer: &Renderer,
    bounds: Rectangle,
) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let colors = KeyColors {
        white_key: Color::from_rgb8(238, 238, 238),
        black_key: Color::from_rgb8(44, 44, 44),
        white_text: Color::from_rgb8(242, 242, 242),
        black_text: Color::from_rgb8(32, 32, 32),
    };
    for pitch in canvas.min_pitch..=canvas.max_pitch {
        draw_key_row(&mut frame, canvas, bounds, pitch, colors);
    }
    vec![frame.into_geometry()]
}

#[derive(Clone, Copy)]
pub(in crate::app) struct KeyColors {
    pub(in crate::app) white_key: Color,
    pub(in crate::app) black_key: Color,
    pub(in crate::app) white_text: Color,
    pub(in crate::app) black_text: Color,
}

pub(in crate::app) fn draw_key_row(
    frame: &mut canvas::Frame,
    canvas: &KeyCanvas,
    bounds: Rectangle,
    pitch: u8,
    colors: KeyColors,
) {
    let y = pitch_to_y(
        canvas.max_pitch,
        pitch,
        NOTE_ROW_HEIGHT,
        -canvas.vertical_offset,
    );
    let palette = key_row_palette(pitch, colors);
    frame.fill_rectangle(
        Point::new(0.0, y),
        Size::new(bounds.width, NOTE_ROW_HEIGHT),
        palette.row,
    );
    stroke_key_row(frame, bounds.width, y);
    draw_key_row_label(frame, canvas, pitch, y, palette.text);
}
