use super::*;

pub(in crate::app) fn is_double_click(previous: Option<Instant>, now: Instant) -> bool {
    previous
        .map(|last| now.saturating_duration_since(last) <= DOUBLE_CLICK_THRESHOLD)
        .unwrap_or(false)
}

pub(in crate::app) fn clamp_pan(value: f32) -> f32 {
    value.clamp(-1.0, 1.0)
}

pub(in crate::app) fn clamp_gain(value: f32) -> f32 {
    value.clamp(GAIN_MIN_DB, GAIN_MAX_DB)
}

pub(in crate::app) fn gain_fader_handle_y(bounds_height: f32, handle_center_y: f32) -> f32 {
    let min_y = 2.0;
    let max_y = (bounds_height - FADER_HANDLE_HEIGHT - 2.0).max(min_y);
    (handle_center_y - FADER_HANDLE_HEIGHT * 0.5).clamp(min_y, max_y)
}

#[cfg(test)]
pub(in crate::app) fn horizontal_slider_handle_x(bounds_width: f32, handle_center_x: f32) -> f32 {
    horizontal_slider_handle_x_with_width(
        bounds_width,
        handle_center_x,
        HORIZONTAL_SLIDER_HANDLE_WIDTH,
    )
}

pub(in crate::app) fn horizontal_slider_handle_x_with_width(
    bounds_width: f32,
    handle_center_x: f32,
    handle_width: f32,
) -> f32 {
    let min_x = 0.0;
    let max_x = (bounds_width - handle_width).max(min_x);
    (handle_center_x - handle_width * 0.5).clamp(min_x, max_x)
}

pub(in crate::app) fn gain_fader_rail_bounds(bounds: Rectangle) -> Rectangle {
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

pub(in crate::app) fn visible_gain_scale_marks(height: f32) -> Vec<f32> {
    let (_, rail_height) = fader_rail_layout(height);
    let base_gap = rail_height * 0.1;
    let stride = if base_gap <= 0.0 {
        GAIN_SCALE_DB_MARKS.len()
    } else {
        crate::number::f32_to_usize((GAIN_SCALE_LABEL_MIN_GAP / base_gap).ceil().max(1.0))
    };

    let mut marks = Vec::with_capacity(GAIN_SCALE_DB_MARKS.len());
    for (index, db) in GAIN_SCALE_DB_MARKS.iter().copied().enumerate() {
        if (index == 0 || index == GAIN_SCALE_DB_MARKS.len() - 1 || index % stride == 0)
            && marks.last().copied() != Some(db)
        {
            marks.push(db);
        }
    }

    if let Some(first_mark) = GAIN_SCALE_DB_MARKS.first().copied()
        && marks.first().copied() != Some(first_mark)
    {
        marks.insert(0, first_mark);
    }
    if let Some(last_mark) = GAIN_SCALE_DB_MARKS.last().copied()
        && marks.last().copied() != Some(last_mark)
    {
        marks.push(last_mark);
    }

    marks
}

pub(in crate::app) fn gain_scale_label(db: f32) -> String {
    if db > 0.0 {
        format!("+{db:.0}")
    } else {
        format!("{db:.0}")
    }
}

pub(in crate::app) fn color_signature(color: Color) -> u64 {
    u64::from(color.r.to_bits())
        ^ u64::from(color.g.to_bits()).rotate_left(13)
        ^ u64::from(color.b.to_bits()).rotate_left(26)
        ^ u64::from(color.a.to_bits()).rotate_left(39)
}

pub(in crate::app) fn gain_normalized_to_db(normalized: f32) -> f32 {
    gain_normalized_to_db_with_max(normalized, GAIN_MAX_DB)
}

pub(in crate::app) fn gain_db_to_normalized(value: f32) -> f32 {
    gain_db_to_normalized_with_max(value, GAIN_MAX_DB)
}

pub(in crate::app) fn gain_normalized_to_db_with_max(normalized: f32, max: f32) -> f32 {
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

pub(in crate::app) fn gain_db_to_normalized_with_max(value: f32, max: f32) -> f32 {
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

pub(in crate::app) fn interpolate(
    value: f32,
    in_min: f32,
    in_max: f32,
    out_min: f32,
    out_max: f32,
) -> f32 {
    if (in_max - in_min).abs() <= f32::EPSILON {
        out_min
    } else {
        let ratio = ((value - in_min) / (in_max - in_min)).clamp(0.0, 1.0);
        out_min + ratio * (out_max - out_min)
    }
}

pub(in crate::app) fn gain_value_to_y(value: f32, rail_bounds: Rectangle) -> f32 {
    rail_bounds.y + rail_bounds.height * (1.0 - gain_db_to_normalized(value))
}

pub(in crate::app) fn y_to_gain_value(y: f32, rail_bounds: Rectangle) -> f32 {
    let normalized = (1.0 - ((y - rail_bounds.y) / rail_bounds.height.max(1.0))).clamp(0.0, 1.0);
    gain_normalized_to_db(normalized)
}
