use super::*;

pub(in crate::app) fn rewind_flag_hitbox(
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    bounds: Rectangle,
) -> Rectangle {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;

    Rectangle {
        x: x - REWIND_FLAG_HITBOX_WIDTH * 0.5,
        y: 0.0,
        width: REWIND_FLAG_HITBOX_WIDTH,
        height: bounds.height,
    }
}

pub(in crate::app) fn tick_from_tempo_lane_x(
    local_x: f32,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    total_ticks: u64,
) -> u64 {
    let absolute_x = local_x + horizontal_scroll;

    clamped_tick_from_f32(absolute_x / pixels_per_tick, total_ticks)
}

pub(in crate::app) fn clamped_tick_from_f32(value: f32, total_ticks: u64) -> u64 {
    let value = value.round().max(0.0);
    crate::number::f32_to_u64(value).min(total_ticks)
}

pub(in crate::app) fn snap_tick_to_subdivision_grid(
    data: &MidiRollData,
    beat_subdivision: u8,
    tick: u64,
) -> u64 {
    let clamped_tick = tick.min(data.total_ticks);
    let mut best_tick = 0;
    let mut best_distance = u64::MAX;

    for span in subdivision_grid_spans(data, beat_subdivision) {
        let center_index = span.nearest_index(clamped_tick);

        for candidate_index in [center_index - 1, center_index, center_index + 1] {
            if let Some((candidate_tick, distance)) = better_snap_candidate(
                span,
                candidate_index,
                clamped_tick,
                best_tick,
                best_distance,
            ) {
                best_tick = candidate_tick;
                best_distance = distance;
            }
        }
    }

    if best_distance == u64::MAX {
        clamped_tick
    } else {
        best_tick
    }
}

pub(in crate::app) fn better_snap_candidate(
    span: SubdivisionGridSpan,
    candidate_index: i64,
    clamped_tick: u64,
    best_tick: u64,
    best_distance: u64,
) -> Option<(u64, u64)> {
    let candidate_tick = span.candidate_tick(candidate_index)?;
    let distance = candidate_tick.abs_diff(clamped_tick);
    snap_candidate_is_better(candidate_tick, distance, best_tick, best_distance)
        .then_some((candidate_tick, distance))
}

pub(in crate::app) fn snap_candidate_is_better(
    candidate_tick: u64,
    distance: u64,
    best_tick: u64,
    best_distance: u64,
) -> bool {
    distance < best_distance || (distance == best_distance && candidate_tick < best_tick)
}

pub(in crate::app) fn adjacent_subdivision_tick(
    data: &MidiRollData,
    beat_subdivision: u8,
    tick: u64,
    forward: bool,
) -> u64 {
    let clamped_tick = tick.min(data.total_ticks);
    let mut best = None;

    for span in subdivision_grid_spans(data, beat_subdivision) {
        let base_index = span.adjacent_index(clamped_tick, forward);

        for candidate_index in [base_index - 1, base_index, base_index + 1] {
            let Some(candidate_tick) = span.candidate_tick(candidate_index) else {
                continue;
            };

            if adjacent_subdivision_candidate_is_better(best, candidate_tick, clamped_tick, forward)
            {
                best = Some(candidate_tick);
            }
        }
    }

    best.unwrap_or(clamped_tick)
}

pub(in crate::app) fn adjacent_subdivision_candidate_is_better(
    best: Option<u64>,
    candidate_tick: u64,
    current_tick: u64,
    forward: bool,
) -> bool {
    if !adjacent_subdivision_candidate_is_ahead(candidate_tick, current_tick, forward) {
        return false;
    }
    best.is_none_or(|best_tick| better_adjacent_subdivision(candidate_tick, best_tick, forward))
}

pub(in crate::app) fn adjacent_subdivision_candidate_is_ahead(
    candidate_tick: u64,
    current_tick: u64,
    forward: bool,
) -> bool {
    if forward {
        candidate_tick > current_tick
    } else {
        candidate_tick < current_tick
    }
}

pub(in crate::app) fn better_adjacent_subdivision(
    candidate_tick: u64,
    best_tick: u64,
    forward: bool,
) -> bool {
    if forward {
        candidate_tick < best_tick
    } else {
        candidate_tick > best_tick
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct SubdivisionGridSpan {
    pub(in crate::app) start_tick: u64,
    pub(in crate::app) end_tick: u64,
    pub(in crate::app) total_ticks: u64,
    pub(in crate::app) step: f64,
}

impl SubdivisionGridSpan {
    fn nearest_index(self, tick: u64) -> i64 {
        (self.relative_position(tick) / self.step).round() as i64
    }

    fn adjacent_index(self, tick: u64, forward: bool) -> i64 {
        let position = self.relative_position(tick) / self.step;
        if forward {
            position.floor() as i64 + 1
        } else {
            position.ceil() as i64 - 1
        }
    }

    fn candidate_tick(self, index: i64) -> Option<u64> {
        if index < 0 {
            return None;
        }

        let candidate_tick =
            crate::number::f64_to_u64((self.start_tick as f64 + index as f64 * self.step).round());

        self.contains_tick(candidate_tick).then_some(candidate_tick)
    }

    fn contains_tick(self, tick: u64) -> bool {
        tick >= self.start_tick && tick <= self.total_ticks && tick < self.end_tick
    }

    fn relative_position(self, tick: u64) -> f64 {
        tick.saturating_sub(self.start_tick) as f64
    }
}

pub(in crate::app) fn subdivision_grid_spans(
    data: &MidiRollData,
    beat_subdivision: u8,
) -> Vec<SubdivisionGridSpan> {
    let beat_subdivision = beat_subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);
    let default_signature = TimeSignatureChange {
        tick: 0,
        numerator: 4,
        denominator: 4,
    };
    let signatures = if data.time_signatures.is_empty() {
        std::slice::from_ref(&default_signature)
    } else {
        data.time_signatures.as_slice()
    };

    signatures
        .iter()
        .enumerate()
        .map(|(index, signature)| {
            let next_tick = signatures
                .get(index + 1)
                .map(|next| next.tick)
                .unwrap_or(data.total_ticks.saturating_add(1));
            subdivision_grid_span(data, beat_subdivision, *signature, next_tick)
        })
        .collect()
}

pub(in crate::app) fn subdivision_grid_span(
    data: &MidiRollData,
    beat_subdivision: u8,
    signature: TimeSignatureChange,
    next_signature_tick: u64,
) -> SubdivisionGridSpan {
    let beat_step = beat_step_ticks(data.ppq, signature).max(1) as f64;

    SubdivisionGridSpan {
        start_tick: signature.tick.min(data.total_ticks),
        end_tick: next_signature_tick.min(data.total_ticks.saturating_add(1)),
        total_ticks: data.total_ticks,
        step: (beat_step / f64::from(beat_subdivision)).max(f64::EPSILON),
    }
}

pub(in crate::app) fn draw_bar_numbers(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    for (bar_index, bar_tick) in data.bar_lines.iter().enumerate() {
        let x = *bar_tick as f32 * pixels_per_tick - horizontal_scroll;
        let label = format!("{}", bar_index + 1);
        let mut label_x = (x + 4.0).max(4.0);

        if let Some(next_bar_tick) = data.bar_lines.get(bar_index + 1) {
            let next_x = *next_bar_tick as f32 * pixels_per_tick - horizontal_scroll;
            let max_x = next_x - estimate_monospace_text_width(&label) - 6.0;
            if max_x <= x + 4.0 {
                continue;
            }
            label_x = label_x.min(max_x);
        }

        frame.fill_text(canvas::Text {
            content: label,
            position: Point::new(label_x, height - BAR_LABEL_BOTTOM_PADDING),
            color: Color::from_rgba(
                palette.background.weak.text.r,
                palette.background.weak.text.g,
                palette.background.weak.text.b,
                0.82,
            ),
            size: Pixels(ui_style::FONT_SIZE_UI_XS.saturating_sub(2) as f32),
            font: fonts::MONO,
            align_y: alignment::Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}

pub(in crate::app) fn draw_playback_cursor(
    frame: &mut canvas::Frame,
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    y: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;

    frame.stroke_rectangle(
        Point::new(x, y),
        Size::new(1.0, height.max(1.0)),
        canvas::Stroke {
            width: 1.4,
            style: canvas::Style::Solid(Color::from_rgba(
                palette.primary.base.color.r,
                palette.primary.base.color.g,
                palette.primary.base.color.b,
                0.92,
            )),
            ..canvas::Stroke::default()
        },
    );
}

pub(in crate::app) fn estimate_monospace_text_width(text: &str) -> f32 {
    text.chars().count() as f32 * ui_style::FONT_SIZE_UI_XS as f32 * 0.60
}

pub(in crate::app) fn max_track_label_chars(track_panel_width: f32) -> usize {
    let horizontal_padding = f32::from(ui_style::PADDING_XS) * 2.0;
    let reserved_width = TRACK_COLOR_BUTTON_SIZE
        + TRACK_COLOR_BUTTON_GAP
        + TRACK_LABEL_BUTTON_GAP
        + TRACK_BUTTON_WIDTH
        + TRACK_BUTTONS_GAP
        + TRACK_BUTTON_WIDTH;
    let available_width = (track_panel_width - horizontal_padding - reserved_width).max(0.0);
    let approx_char_width = ui_style::FONT_SIZE_UI_XS as f32 * 0.60;
    let estimated = crate::number::f32_to_usize((available_width / approx_char_width).floor());

    estimated.clamp(4, 18)
}

pub(in crate::app) fn pitch_to_y(
    max_pitch: u8,
    pitch: u8,
    row_height: f32,
    top_offset: f32,
) -> f32 {
    let row = f32::from(max_pitch.saturating_sub(pitch));
    top_offset + row * row_height
}

pub(in crate::app) fn pitch_count(min_pitch: u8, max_pitch: u8) -> u16 {
    u16::from(max_pitch.saturating_sub(min_pitch)) + 1
}

pub(in crate::app) fn beat_step_ticks(ppq: u16, signature: TimeSignatureChange) -> u64 {
    let quarter = u64::from(ppq.max(1));
    let denominator = u64::from(signature.denominator.max(1));

    quarter.saturating_mul(4) / denominator
}

pub(in crate::app) fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

pub(in crate::app) fn pitch_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let note_name = NAMES.get(usize::from(pitch % 12)).copied().unwrap_or("?");
    let octave = i16::from(pitch) / 12 - 1;

    format!("{note_name}{octave}")
}

pub(in crate::app) fn track_visibility_alpha(
    track_mix: &[TrackMixState],
    track_index: usize,
    global_solo_active: bool,
) -> f32 {
    let Some(current_state) = track_mix.get(track_index) else {
        return 1.0;
    };

    if current_state.muted {
        return 0.10;
    }

    let has_any_solo = global_solo_active || track_mix.iter().any(|state| state.soloed);
    if has_any_solo && !current_state.soloed {
        return 0.18;
    }

    1.0
}

pub(in crate::app) fn shorten_label(label: &str, max_len: usize) -> String {
    crate::track_names::ellipsize_middle(label, max_len)
}
