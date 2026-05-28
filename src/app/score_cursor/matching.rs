use super::*;

pub(super) fn pitch_match_score(
    segment: &[NoteEvent],
    midi_data: &MidiRollData,
    ppq: u16,
) -> usize {
    if segment.is_empty() || midi_data.notes.is_empty() {
        return 0;
    }

    let ticks_per_whole = f64::from(ppq) * TICKS_PER_WHOLE_NOTE;
    let quant = u64::from((ppq / 8).max(1));
    let mut midi_points: HashMap<(u64, u8), usize> = HashMap::new();
    for note in &midi_data.notes {
        let quant_tick = note.start_tick / quant;
        *midi_points.entry((quant_tick, note.pitch)).or_insert(0) += 1;
    }

    let mut hits = 0usize;
    for event in segment {
        let tick = (event.moment_whole * ticks_per_whole).round();
        if tick.is_sign_negative() {
            continue;
        }
        let quant_tick = crate::number::f64_to_u64(tick) / quant;
        let same_tick = midi_points.contains_key(&(quant_tick, event.pitch));
        let prev_tick = quant_tick
            .checked_sub(1)
            .is_some_and(|value| midi_points.contains_key(&(value, event.pitch)));
        let next_tick = midi_points.contains_key(&(quant_tick + 1, event.pitch));

        if same_tick || prev_tick || next_tick {
            hits += 1;
        }
    }

    hits.saturating_mul(10_000) / segment.len().max(1)
}

pub(super) fn coverage(matched: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }

    matched.saturating_mul(10_000) / total
}

pub(super) fn span_similarity_score(a: u64, b: u64) -> usize {
    let min = a.min(b) as f64;
    let max = a.max(b) as f64;
    if max <= f64::EPSILON {
        return 0;
    }

    crate::number::f64_to_usize(((min / max) * 10_000.0).round())
}

#[derive(Clone, Copy)]
pub(super) struct StaffLine {
    pub(super) x_start: f32,
    pub(super) x_end: f32,
    pub(super) y: f32,
}

pub(super) fn parse_staff_lines(source: &str) -> Vec<StaffLine> {
    let mut lines = Vec::new();
    let mut search_offset = 0usize;

    while let Some(search_tail) = source.get(search_offset..) {
        let Some(group_start_rel) = search_tail.find("<g ") else {
            break;
        };
        let group_start = search_offset + group_start_rel;
        let Some(group_tail) = source.get(group_start..) else {
            break;
        };
        let Some(group_end_rel) = group_tail.find("</g>") else {
            break;
        };
        let group_end = group_start + group_end_rel + "</g>".len();
        let Some(group) = source.get(group_start..group_end) else {
            break;
        };
        search_offset = group_end;

        let Some((tx, ty)) = first_translate(group) else {
            continue;
        };
        let Some(line_start_rel) = group.find("<line ") else {
            continue;
        };
        let Some(line) = group.get(line_start_rel..) else {
            continue;
        };

        let Some(x1) = attribute_value(line, "x1").and_then(|value| value.parse::<f32>().ok())
        else {
            continue;
        };
        let Some(x2) = attribute_value(line, "x2").and_then(|value| value.parse::<f32>().ok())
        else {
            continue;
        };
        let Some(y1) = attribute_value(line, "y1").and_then(|value| value.parse::<f32>().ok())
        else {
            continue;
        };
        let Some(y2) = attribute_value(line, "y2").and_then(|value| value.parse::<f32>().ok())
        else {
            continue;
        };
        if (y1 - y2).abs() > 0.001 {
            continue;
        }

        let x_start = tx + x1.min(x2);
        let x_end = tx + x1.max(x2);
        if (x_end - x_start) < STAFF_LINE_LENGTH_MIN {
            continue;
        }

        lines.push(StaffLine {
            x_start,
            x_end,
            y: ty + y1,
        });
    }

    lines
}

pub(super) fn system_from_staves(staves: &[(f32, f32, f32, f32)]) -> Option<SystemBand> {
    if staves.is_empty() {
        return None;
    }

    let mut x_start = f32::INFINITY;
    let mut x_end = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for staff in staves {
        x_start = x_start.min(staff.0);
        x_end = x_end.max(staff.1);
        min_y = min_y.min(staff.2);
        max_y = max_y.max(staff.3);
    }

    Some(SystemBand {
        x_start,
        x_end,
        min_y,
        max_y,
    })
}
