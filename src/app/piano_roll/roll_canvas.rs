use super::*;

pub(in crate::app) struct KeyRowPalette {
    pub(in crate::app) row: Color,
    pub(in crate::app) text: Color,
}

pub(in crate::app) fn key_row_palette(pitch: u8, colors: KeyColors) -> KeyRowPalette {
    if is_black_key(pitch) {
        KeyRowPalette {
            row: colors.black_key,
            text: colors.white_text,
        }
    } else {
        KeyRowPalette {
            row: colors.white_key,
            text: colors.black_text,
        }
    }
}

pub(in crate::app) fn stroke_key_row(frame: &mut canvas::Frame, width: f32, y: f32) {
    frame.stroke_rectangle(
        Point::new(0.0, y),
        Size::new(width, NOTE_ROW_HEIGHT),
        canvas::Stroke {
            width: 1.0,
            style: canvas::Style::Solid(Color::from_rgba(0.0, 0.0, 0.0, 0.18)),
            ..canvas::Stroke::default()
        },
    );
}

pub(in crate::app) fn draw_key_row_label(
    frame: &mut canvas::Frame,
    canvas: &KeyCanvas,
    pitch: u8,
    y: f32,
    color: Color,
) {
    if !key_row_has_label(canvas, pitch) {
        return;
    }
    frame.fill_text(canvas::Text {
        content: pitch_name(pitch),
        position: Point::new(4.0, y + NOTE_ROW_HEIGHT * 0.5),
        color,
        size: Pixels(ui_style::FONT_SIZE_UI_XS as f32),
        font: fonts::MONO,
        align_y: alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

pub(in crate::app) fn key_row_has_label(canvas: &KeyCanvas, pitch: u8) -> bool {
    pitch.is_multiple_of(12) || pitch == canvas.min_pitch || pitch == canvas.max_pitch
}

pub(in crate::app) struct RollNotesCanvas<'a> {
    pub(in crate::app) data: &'a MidiRollData,
    pub(in crate::app) zoom_x: f32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) playback_tick: u64,
    pub(in crate::app) track_mix: &'a [TrackMixState],
    pub(in crate::app) track_colors: Vec<Color>,
    pub(in crate::app) global_solo_active: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct RollNotesState {
    pub(in crate::app) cache: Cache,
    pub(in crate::app) cache_key: Option<RollNotesCacheKey>,
    pub(in crate::app) stack_offsets: HashMap<NoteStackKey, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct RollNotesCacheKey {
    pub(in crate::app) total_ticks: u64,
    pub(in crate::app) ppq: u16,
    pub(in crate::app) min_pitch: u8,
    pub(in crate::app) max_pitch: u8,
    pub(in crate::app) notes_len: usize,
    pub(in crate::app) tracks_len: usize,
    pub(in crate::app) time_signatures_len: usize,
    pub(in crate::app) zoom_x_bits: u32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) track_mix_hash: u64,
    pub(in crate::app) track_colors_hash: u64,
    pub(in crate::app) global_solo_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::app) struct NoteStackKey {
    pub(in crate::app) start_tick: u64,
    pub(in crate::app) pitch: u8,
}

impl TempoCanvas<'_> {
    pub(in crate::app) fn tempo_canvas_mouse_action(
        &self,
        state: &mut TempoCanvasState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        pixels_per_tick: f32,
    ) -> Option<canvas::Action<Message>> {
        if tempo_canvas_released(event) {
            reset_rewind_flag_drag(state);
            return None;
        }
        let canvas::Event::Mouse(mouse_event) = event else {
            return None;
        };
        self.tempo_canvas_mouse_event_action(state, mouse_event, bounds, cursor, pixels_per_tick)
    }

    fn tempo_canvas_mouse_event_action(
        &self,
        state: &mut TempoCanvasState,
        event: &mouse::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        pixels_per_tick: f32,
    ) -> Option<canvas::Action<Message>> {
        match event {
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                self.right_press_action(state, bounds, cursor, pixels_per_tick)
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                self.left_press_action(state, bounds, cursor, pixels_per_tick)
            }
            mouse::Event::CursorMoved { .. } => {
                self.cursor_move_action(state, bounds, cursor, pixels_per_tick)
            }
            _ => None,
        }
    }
}

pub(in crate::app) fn tempo_canvas_released(event: &canvas::Event) -> bool {
    matches!(
        event,
        canvas::Event::Mouse(mouse::Event::ButtonReleased(_))
            | canvas::Event::Mouse(mouse::Event::CursorLeft)
    )
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct NoteGeometry {
    pub(in crate::app) index: usize,
    pub(in crate::app) key: NoteStackKey,
    pub(in crate::app) x: f32,
    pub(in crate::app) y: f32,
    pub(in crate::app) width: f32,
    pub(in crate::app) height: f32,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct VisibilityState<'a> {
    pub(in crate::app) track_mix: &'a [TrackMixState],
    pub(in crate::app) track_colors: &'a [Color],
    pub(in crate::app) global_solo_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct StackTop {
    pub(in crate::app) note_index: usize,
}

impl canvas::Program<Message> for RollNotesCanvas<'_> {
    type State = RollNotesState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cache_key = RollNotesCacheKey {
            total_ticks: self.data.total_ticks,
            ppq: self.data.ppq,
            min_pitch: self.data.min_pitch,
            max_pitch: self.data.max_pitch,
            notes_len: self.data.notes.len(),
            tracks_len: self.data.tracks.len(),
            time_signatures_len: self.data.time_signatures.len(),
            zoom_x_bits: self.zoom_x.to_bits(),
            beat_subdivision: self.beat_subdivision,
            track_mix_hash: track_mix_hash(self.track_mix),
            track_colors_hash: crate::track_colors::color_hash(&self.track_colors),
            global_solo_active: self.global_solo_active,
        };
        if self.state_needs_cache_clear(state, cache_key) {
            state.cache.clear();
            state.cache_key = Some(cache_key);
        }

        let cursor_position = roll_notes_press_position(event, bounds, cursor)?;

        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));
        let tick =
            clamped_tick_from_f32(cursor_position.x / pixels_per_tick, self.data.total_ticks);
        let notes = build_note_geometries(self.data, pixels_per_tick);
        let stacks = build_note_stacks(&notes);
        let draw_order = compute_note_draw_order(&notes, &stacks, &state.stack_offsets);

        if cycle_clicked_note_stack(state, cursor_position, &notes, &stacks, draw_order) {
            return Some(publish_roll_cursor_tick(tick));
        }

        Some(publish_roll_cursor_tick(tick))
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        self.draw_roll_notes(state, renderer, theme, bounds)
    }
}

impl RollNotesCanvas<'_> {
    fn pixels_per_tick(&self) -> f32 {
        BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1))
    }

    fn draw_roll_notes(
        &self,
        state: &RollNotesState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let pixels_per_tick = self.pixels_per_tick();
        let static_geometry =
            self.draw_static_roll_notes(state, renderer, bounds, palette, pixels_per_tick);
        let cursor_geometry =
            self.draw_roll_playback_cursor(renderer, bounds, palette, pixels_per_tick);
        vec![static_geometry, cursor_geometry]
    }

    fn draw_static_roll_notes(
        &self,
        state: &RollNotesState,
        renderer: &Renderer,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
        pixels_per_tick: f32,
    ) -> canvas::Geometry {
        state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(
                Point::new(0.0, 0.0),
                bounds.size(),
                palette.background.base.color,
            );

            for pitch in self.data.min_pitch..=self.data.max_pitch {
                let y = pitch_to_y(self.data.max_pitch, pitch, NOTE_ROW_HEIGHT, 0.0);
                let row_color = if is_black_key(pitch) {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.11)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                };

                frame.fill_rectangle(
                    Point::new(0.0, y),
                    Size::new(bounds.width, NOTE_ROW_HEIGHT),
                    row_color,
                );
            }

            draw_grid(
                frame,
                self.data,
                self.beat_subdivision,
                pixels_per_tick,
                bounds.height,
                palette,
            );

            let notes = build_note_geometries(self.data, pixels_per_tick);
            let stacks = build_note_stacks(&notes);
            let draw_order = compute_note_draw_order(&notes, &stacks, &state.stack_offsets);

            for note_index in draw_order {
                let Some(&geometry) = notes.get(note_index) else {
                    continue;
                };
                let Some(note) = self.data.notes.get(note_index) else {
                    continue;
                };
                draw_note(
                    frame,
                    VisibilityState {
                        track_mix: self.track_mix,
                        track_colors: &self.track_colors,
                        global_solo_active: self.global_solo_active,
                    },
                    note,
                    geometry,
                );
            }
        })
    }

    fn draw_roll_playback_cursor(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
        palette: &iced::theme::palette::Extended,
        pixels_per_tick: f32,
    ) -> canvas::Geometry {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        draw_playback_cursor(
            &mut frame,
            self.playback_tick,
            pixels_per_tick,
            0.0,
            0.0,
            bounds.height,
            palette,
        );
        frame.into_geometry()
    }

    fn state_needs_cache_clear(
        &self,
        state: &RollNotesState,
        cache_key: RollNotesCacheKey,
    ) -> bool {
        state.cache_key != Some(cache_key)
    }
}

pub(in crate::app) fn roll_notes_press_position(
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<Point> {
    if !matches!(
        event,
        canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    ) {
        return None;
    }
    cursor.position_in(bounds)
}

pub(in crate::app) fn cycle_clicked_note_stack(
    state: &mut RollNotesState,
    cursor_position: Point,
    notes: &[NoteGeometry],
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    draw_order: Vec<usize>,
) -> bool {
    let Some(geometry) = clicked_note_geometry(cursor_position, notes, draw_order) else {
        return false;
    };
    let Some(stack) = stacks.get(&geometry.key) else {
        return false;
    };
    if stack.len() <= 1 {
        return false;
    }
    let offset = state.stack_offsets.entry(geometry.key).or_insert(0);
    *offset = (*offset + 1) % stack.len();
    state.cache.clear();
    true
}

pub(in crate::app) fn clicked_note_geometry(
    cursor_position: Point,
    notes: &[NoteGeometry],
    draw_order: Vec<usize>,
) -> Option<NoteGeometry> {
    draw_order
        .into_iter()
        .rev()
        .filter_map(|note_index| notes.get(note_index).copied())
        .find(|geometry| note_contains_point(*geometry, cursor_position))
}

pub(in crate::app) fn publish_roll_cursor_tick(tick: u64) -> canvas::Action<Message> {
    canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)))
        .and_capture()
}

pub(in crate::app) fn track_mix_hash(track_mix: &[TrackMixState]) -> u64 {
    let mut hash = 0u64;
    for (index, state) in track_mix.iter().enumerate() {
        let bits = ((state.muted as u64) << 1) | state.soloed as u64;
        hash ^= bits.rotate_left((index % 63) as u32);
    }
    hash
}

pub(in crate::app) fn draw_note(
    frame: &mut canvas::Frame,
    visibility: VisibilityState<'_>,
    note: &MidiNote,
    geometry: NoteGeometry,
) {
    let mut color = visibility
        .track_colors
        .get(note.track_index)
        .copied()
        .unwrap_or_else(|| crate::track_colors::default_track_color(note.track_index));
    let visibility_alpha = track_visibility_alpha(
        visibility.track_mix,
        note.track_index,
        visibility.global_solo_active,
    );
    color.a *= visibility_alpha;

    frame.fill_rectangle(
        Point::new(geometry.x, geometry.y),
        Size::new(geometry.width, geometry.height),
        color,
    );

    frame.stroke_rectangle(
        Point::new(geometry.x, geometry.y),
        Size::new(geometry.width, geometry.height),
        canvas::Stroke {
            width: 1.0,
            style: canvas::Style::Solid(Color::from_rgba(
                0.0,
                0.0,
                0.0,
                0.38 * visibility_alpha.max(0.30),
            )),
            ..canvas::Stroke::default()
        },
    );
}

pub(in crate::app) fn build_note_geometries(
    data: &MidiRollData,
    pixels_per_tick: f32,
) -> Vec<NoteGeometry> {
    data.notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let x = note.start_tick as f32 * pixels_per_tick;
            let width =
                ((note.end_tick.saturating_sub(note.start_tick)) as f32 * pixels_per_tick).max(2.0);
            let y = pitch_to_y(data.max_pitch, note.pitch, NOTE_ROW_HEIGHT, 0.0) + 0.5;
            let height = (NOTE_ROW_HEIGHT - 1.0).max(1.0);

            NoteGeometry {
                index,
                key: NoteStackKey {
                    start_tick: note.start_tick,
                    pitch: note.pitch,
                },
                x,
                y,
                width,
                height,
            }
        })
        .collect()
}

pub(in crate::app) fn build_note_stacks(
    notes: &[NoteGeometry],
) -> HashMap<NoteStackKey, Vec<usize>> {
    let mut stacks: HashMap<NoteStackKey, Vec<usize>> = HashMap::new();

    for note in notes {
        stacks.entry(note.key).or_default().push(note.index);
    }

    stacks.retain(|_key, members| members.len() > 1);
    stacks
}

pub(in crate::app) fn compute_note_draw_order(
    notes: &[NoteGeometry],
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    stack_offsets: &HashMap<NoteStackKey, usize>,
) -> Vec<usize> {
    let top_notes = compute_stack_top_notes(stacks, stack_offsets);

    let mut order = Vec::with_capacity(notes.len());
    let mut deferred = Vec::new();

    for note in notes {
        if let Some(top_index) = top_notes.get(&note.key) {
            if note.index == top_index.note_index {
                deferred.push(note.index);
            } else {
                order.push(note.index);
            }
        } else {
            order.push(note.index);
        }
    }

    order.extend(deferred);
    order
}

pub(in crate::app) fn compute_stack_top_notes(
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    stack_offsets: &HashMap<NoteStackKey, usize>,
) -> HashMap<NoteStackKey, StackTop> {
    let mut top_notes = HashMap::new();

    for (key, members) in stacks {
        let len = members.len();
        let offset = stack_offsets.get(key).copied().unwrap_or(0) % len;
        let top_pos = (len - 1 + len - offset) % len;
        top_notes.insert(
            *key,
            StackTop {
                note_index: members.get(top_pos).copied().unwrap_or(0),
            },
        );
    }

    top_notes
}

pub(in crate::app) fn note_contains_point(note: NoteGeometry, point: Point) -> bool {
    point.x >= note.x
        && point.x <= note.x + note.width
        && point.y >= note.y
        && point.y <= note.y + note.height
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct GridDrawContext {
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) pixels_per_tick: f32,
    pub(in crate::app) height: f32,
    pub(in crate::app) subdivision_color: Color,
    pub(in crate::app) beat_color: Color,
    pub(in crate::app) total_ticks: u64,
}

pub(in crate::app) fn draw_grid(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    beat_subdivision: u8,
    pixels_per_tick: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let context = GridDrawContext {
        beat_subdivision: beat_subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX),
        pixels_per_tick,
        height,
        subdivision_color: grid_line_color(palette, 0.18),
        beat_color: grid_line_color(palette, 0.38),
        total_ticks: data.total_ticks,
    };

    for (index, signature) in data.time_signatures.iter().enumerate() {
        draw_signature_grid(frame, data, index, *signature, context);
    }
    draw_bar_lines(frame, data, pixels_per_tick, 0.0, 0.0, height, palette);
}

pub(in crate::app) fn draw_signature_grid(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    signature_index: usize,
    signature: TimeSignatureChange,
    context: GridDrawContext,
) {
    let end_tick = signature_grid_end_tick(data, signature_index);
    let beat_step = beat_step_ticks(data.ppq, signature).max(1);
    let mut beat_tick = signature.tick;

    while beat_tick <= context.total_ticks && beat_tick < end_tick {
        draw_beat_subdivision_grid(frame, beat_tick, end_tick, beat_step, context);
        draw_grid_line(
            frame,
            beat_tick as f32 * context.pixels_per_tick,
            context.height,
            1.0,
            context.beat_color,
        );
        beat_tick = beat_tick.saturating_add(beat_step);
    }
}

pub(in crate::app) fn signature_grid_end_tick(data: &MidiRollData, signature_index: usize) -> u64 {
    data.time_signatures
        .get(signature_index + 1)
        .map(|next| next.tick)
        .unwrap_or(data.total_ticks.saturating_add(1))
}

pub(in crate::app) fn draw_beat_subdivision_grid(
    frame: &mut canvas::Frame,
    beat_tick: u64,
    end_tick: u64,
    beat_step: u64,
    context: GridDrawContext,
) {
    if context.beat_subdivision <= 1 {
        return;
    }
    for division in 1..context.beat_subdivision {
        let subdivision_tick = beat_tick as f32
            + (f32::from(division) * beat_step as f32 / f32::from(context.beat_subdivision));
        if subdivision_tick >= end_tick as f32 || subdivision_tick > context.total_ticks as f32 {
            break;
        }
        draw_grid_line(
            frame,
            subdivision_tick * context.pixels_per_tick,
            context.height,
            0.8,
            context.subdivision_color,
        );
    }
}

pub(in crate::app) fn grid_line_color(
    palette: &iced::theme::palette::Extended,
    alpha: f32,
) -> Color {
    Color::from_rgba(
        palette.background.strong.color.r,
        palette.background.strong.color.g,
        palette.background.strong.color.b,
        alpha,
    )
}

pub(in crate::app) fn draw_grid_line(
    frame: &mut canvas::Frame,
    x: f32,
    height: f32,
    width: f32,
    color: Color,
) {
    frame.stroke_rectangle(
        Point::new(x, 0.0),
        Size::new(1.0, height.max(1.0)),
        canvas::Stroke {
            width,
            style: canvas::Style::Solid(color),
            ..canvas::Stroke::default()
        },
    );
}

pub(in crate::app) fn draw_bar_lines(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    y: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let bar_color = Color::from_rgba(
        palette.background.strong.color.r,
        palette.background.strong.color.g,
        palette.background.strong.color.b,
        0.56,
    );

    for bar_tick in &data.bar_lines {
        let x = *bar_tick as f32 * pixels_per_tick - horizontal_scroll;
        frame.stroke_rectangle(
            Point::new(x, y),
            Size::new(1.0, height.max(1.0)),
            canvas::Stroke {
                width: 1.8,
                style: canvas::Style::Solid(bar_color),
                ..canvas::Stroke::default()
            },
        );
    }
}

pub(in crate::app) fn draw_tempo_markers(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    for (index, tempo) in data.tempo_changes.iter().enumerate() {
        let x = tempo.tick as f32 * pixels_per_tick - horizontal_scroll;
        let next_x = data
            .tempo_changes
            .get(index + 1)
            .map(|next| next.tick as f32 * pixels_per_tick - horizontal_scroll);
        let bpm = if tempo.micros_per_quarter == 0 {
            0.0
        } else {
            60_000_000.0 / tempo.micros_per_quarter as f32
        };
        let label = format!("♩={bpm:.1}");

        frame.stroke_rectangle(
            Point::new(x, 0.0),
            Size::new(1.0, height.max(1.0)),
            canvas::Stroke {
                width: 1.2,
                style: canvas::Style::Solid(Color::from_rgba(
                    palette.secondary.base.color.r,
                    palette.secondary.base.color.g,
                    palette.secondary.base.color.b,
                    0.98,
                )),
                ..canvas::Stroke::default()
            },
        );

        let mut label_x = (x + 4.0).max(4.0);
        if let Some(next_x) = next_x {
            let max_x = next_x - estimate_monospace_text_width(&label) - 6.0;
            if max_x > x + 4.0 {
                label_x = label_x.min(max_x);
            }
        }

        frame.fill_text(canvas::Text {
            content: label,
            position: Point::new(label_x, TEMPO_LABEL_TOP_PADDING),
            color: Color::from_rgba(0.96, 0.97, 0.99, 0.98),
            size: Pixels(ui_style::FONT_SIZE_UI_XS.saturating_sub(1) as f32),
            font: fonts::MONO,
            align_y: alignment::Vertical::Top,
            ..canvas::Text::default()
        });
    }
}

pub(in crate::app) fn draw_rewind_flag(
    frame: &mut canvas::Frame,
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;
    let stem_left = (x - 1.0).round();
    let fill_color = palette.primary.strong.color;

    frame.fill_rectangle(
        Point::new(stem_left, 0.0),
        Size::new(2.0, height.max(1.0)),
        fill_color,
    );

    frame.fill_rectangle(
        Point::new(stem_left + 1.0, 0.0),
        Size::new(REWIND_FLAG_WIDTH, REWIND_FLAG_BANNER_HEIGHT),
        fill_color,
    );
}

pub(in crate::app) fn piano_roll_scroll_position_marker(
    playback_tick: u64,
    total_ticks: u64,
) -> Element<'static, Message> {
    let normalized = if total_ticks == 0 {
        0.0
    } else {
        (playback_tick as f32 / total_ticks as f32).clamp(0.0, 1.0)
    };

    canvas(HorizontalScrollMarkerCanvas { normalized })
        .width(Fill)
        .height(Fill)
        .into()
}

pub(in crate::app) struct HorizontalScrollMarkerCanvas {
    pub(in crate::app) normalized: f32,
}

impl canvas::Program<Message> for HorizontalScrollMarkerCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        draw_horizontal_scroll_marker(self.normalized, renderer, theme, bounds)
    }
}

fn draw_horizontal_scroll_marker(
    normalized: f32,
    renderer: &Renderer,
    theme: &Theme,
    bounds: Rectangle,
) -> Vec<canvas::Geometry> {
    let palette = theme.extended_palette();
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let marker_width = SCROLL_MARKER_LENGTH.min(bounds.width.max(1.0));
    let marker_center_x = normalized * bounds.width.max(1.0);
    let marker_x =
        (marker_center_x - marker_width * 0.5).clamp(0.0, (bounds.width - marker_width).max(0.0));
    let marker_y = (bounds.height - SCROLL_MARKER_THICKNESS - SCROLL_MARKER_EDGE_INSET).max(0.0);
    frame.fill_rectangle(
        Point::new(marker_x, marker_y),
        Size::new(marker_width, SCROLL_MARKER_THICKNESS),
        Color::from_rgba(
            palette.secondary.base.color.r,
            palette.secondary.base.color.g,
            palette.secondary.base.color.b,
            0.72,
        ),
    );
    vec![frame.into_geometry()]
}
