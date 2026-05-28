use super::*;

pub(super) fn smooth_zoom(
    current_zoom: f32,
    delta: iced::mouse::ScrollDelta,
    min_zoom: f32,
    max_zoom: f32,
) -> f32 {
    let intensity = match delta {
        iced::mouse::ScrollDelta::Lines { y, .. } => y * 0.14,
        iced::mouse::ScrollDelta::Pixels { y, .. } => y * 0.0035,
    };

    (current_zoom * intensity.exp()).clamp(min_zoom, max_zoom)
}

pub(super) fn anchored_scroll(current_scroll: f32, cursor_in_viewport: f32, scale: f32) -> f32 {
    ((current_scroll + cursor_in_viewport) * scale - cursor_in_viewport).max(0.0)
}

pub(super) fn score_preview_target_zoom(zoom: f32, tier: ScoreZoomPreviewTier) -> f32 {
    match tier {
        ScoreZoomPreviewTier::Fallback => zoom.max(SCORE_PREVIEW_FALLBACK_MIN_ZOOM),
        ScoreZoomPreviewTier::Primary => zoom.max(SCORE_PREVIEW_PRIMARY_MIN_ZOOM),
    }
}

pub(super) fn render_score_zoom_preview(
    svg_bytes: Bytes,
    page_size: SvgSize,
    request: ScoreZoomPreviewRequest,
) -> Result<super::messages::ScorePreviewReady, String> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes.as_ref(), &options)
        .map_err(|error| format!("Failed to parse score SVG: {error}"))?;

    let logical_width =
        (page_size.width * super::score_view::score_base_scale() * request.zoom).max(1.0);
    let logical_height =
        (page_size.height * super::score_view::score_base_scale() * request.zoom).max(1.0);
    let longest_edge = logical_width.max(logical_height).max(1.0);
    let max_dimension = match request.tier {
        ScoreZoomPreviewTier::Fallback => SCORE_PREVIEW_FALLBACK_MAX_DIMENSION,
        ScoreZoomPreviewTier::Primary => SCORE_PREVIEW_PRIMARY_MAX_DIMENSION,
    };
    let raster_scale = (max_dimension / longest_edge).min(1.0);
    let raster_width = crate::number::f32_to_u32((logical_width * raster_scale).round().max(1.0));
    let raster_height = crate::number::f32_to_u32((logical_height * raster_scale).round().max(1.0));

    let mut pixmap = tiny_skia::Pixmap::new(raster_width, raster_height)
        .ok_or_else(|| "Failed to allocate score preview pixmap".to_string())?;

    let tree_size = tree.size().to_int_size().to_size();
    let transform = tiny_skia::Transform::from_scale(
        raster_width as f32 / tree_size.width(),
        raster_height as f32 / tree_size.height(),
    );

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(super::messages::ScorePreviewReady {
        page_index: request.page_index,
        zoom: request.zoom,
        tier: request.tier,
        handle: image::Handle::from_rgba(raster_width, raster_height, pixmap.take()),
    })
}

pub(super) fn is_relevant_score_change(event: &notify::Event, watched_path: &Path) -> bool {
    watched_event_paths(event, |path| path == watched_path)
}

pub(super) fn is_relevant_editor_file_change(event: &notify::Event, watched_path: &Path) -> bool {
    if !watched_event_kind(event) {
        return false;
    }

    let Some(parent) = watched_path.parent() else {
        return false;
    };

    event.paths.iter().any(|path| {
        path == watched_path
            || path.parent().is_some_and(|candidate| candidate == parent)
            || path == parent
    })
}

pub(super) fn is_relevant_browser_file_change(event: &notify::Event, watched_root: &Path) -> bool {
    watched_event_paths(event, |path| {
        path == watched_root || path.starts_with(watched_root)
    })
}

fn watched_event_paths(event: &notify::Event, matches_path: impl FnMut(&PathBuf) -> bool) -> bool {
    watched_event_kind(event) && (event.paths.is_empty() || event.paths.iter().any(matches_path))
}

fn watched_event_kind(event: &notify::Event) -> bool {
    matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

pub(super) fn is_svg_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

pub(super) fn svg_page_index(file_stem: &str, score_stem: &str) -> Option<u32> {
    if file_stem == score_stem {
        return Some(1);
    }

    let suffix = file_stem.strip_prefix(score_stem)?.strip_prefix('-')?;

    if let Some(page_suffix) = suffix.strip_prefix("page") {
        return page_suffix.parse::<u32>().ok();
    }

    suffix.parse::<u32>().ok()
}

pub(super) fn read_svg_size(path: &Path) -> Option<SvgSize> {
    let source = fs::read_to_string(path).ok()?;
    parse_svg_size_from_source(&source)
}

pub(super) fn parse_svg_size_from_source(source: &str) -> Option<SvgSize> {
    parse_svg_direct_size(source).or_else(|| parse_svg_view_box_size(source))
}

pub(super) fn parse_svg_direct_size(source: &str) -> Option<SvgSize> {
    let width = svg_attribute_value(source, "width").and_then(parse_svg_dimension_points);
    let height = svg_attribute_value(source, "height").and_then(parse_svg_dimension_points);
    positive_svg_size(width?, height?)
}

pub(super) fn positive_svg_size(width: f32, height: f32) -> Option<SvgSize> {
    (width > 0.0 && height > 0.0).then_some(SvgSize { width, height })
}

pub(super) fn parse_svg_coordinate_size_from_source(source: &str) -> Option<SvgSize> {
    parse_svg_view_box_size(source).or_else(|| parse_svg_size_from_source(source))
}

pub(super) fn parse_svg_view_box_size(source: &str) -> Option<SvgSize> {
    let view_box = svg_attribute_value(source, "viewBox")?;
    let numbers: Vec<f32> = view_box
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse::<f32>().ok())
        .collect();
    let (&width, &height) = numbers.get(2).zip(numbers.get(3))?;
    positive_svg_size(width.abs(), height.abs())
}

pub(super) fn svg_attribute_value<'a>(source: &'a str, attribute_name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{attribute_name}={quote}");
        let Some(start) = source.find(&needle) else {
            continue;
        };
        let value_start = start + needle.len();
        let Some(tail) = source.get(value_start..) else {
            continue;
        };
        let Some(value_end) = tail.find(quote) else {
            continue;
        };

        return tail.get(..value_end);
    }

    None
}

pub(super) fn parse_svg_dimension_points(raw_value: &str) -> Option<f32> {
    let trimmed = raw_value.trim();
    let numeric_prefix: String = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
        .collect();

    if numeric_prefix.is_empty() {
        return None;
    }

    let value = numeric_prefix.parse::<f32>().ok()?;
    let unit = trimmed
        .get(numeric_prefix.len()..)?
        .trim()
        .to_ascii_lowercase();

    let points = match unit.as_str() {
        "" | "px" | "pt" => value,
        "mm" => value * 72.0 / 25.4,
        "cm" => value * 72.0 / 2.54,
        "in" => value * 72.0,
        "pc" => value * 12.0,
        _ => value,
    };

    Some(points)
}

pub(super) fn closest_system_band(
    bands: &[score_cursor::SystemBand],
    x: f32,
    min_y: f32,
    max_y: f32,
) -> Option<score_cursor::SystemBand> {
    let target_y = (min_y + max_y) * 0.5;
    bands
        .iter()
        .copied()
        .filter(|band| x >= band.x_start && x <= band.x_end)
        .min_by(|left, right| {
            let left_center = (left.min_y + left.max_y) * 0.5;
            let right_center = (right.min_y + right.max_y) * 0.5;
            let left_distance = (target_y - left_center).abs();
            let right_distance = (target_y - right_center).abs();
            left_distance.total_cmp(&right_distance)
        })
}
