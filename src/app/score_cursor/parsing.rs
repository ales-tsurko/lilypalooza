use super::*;

pub(super) const TICKS_PER_WHOLE_NOTE: f64 = 4.0;
pub(super) const STAFF_LINE_LENGTH_MIN: f32 = 30.0;
pub(super) const STAFF_LINE_SPACING_TARGET: f32 = 1.0;
pub(super) const STAFF_LINE_SPACING_TOLERANCE: f32 = 0.25;
pub(super) const SYSTEM_VERTICAL_GAP_THRESHOLD: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::app) struct SourceLocation {
    pub(in crate::app) line: u32,
    pub(in crate::app) column: u32,
}

#[derive(Debug, Clone)]
pub(in crate::app) struct SvgNoteAnchor {
    pub(in crate::app) source: SourceLocation,
    pub(in crate::app) source_path: Option<PathBuf>,
    pub(in crate::app) page_index: usize,
    pub(in crate::app) x: f32,
    pub(in crate::app) y: f32,
}

#[derive(Debug, Clone)]
pub(in crate::app) struct PointAndClickTarget {
    pub(in crate::app) path: Option<PathBuf>,
    pub(in crate::app) line: usize,
    pub(in crate::app) column: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NoteEvent {
    pub(super) moment_whole: f64,
    pub(super) duration_whole: f64,
    pub(super) pitch: u8,
    pub(super) source: SourceLocation,
}

#[derive(Debug, Clone)]
pub(super) struct NotesFile {
    segments: Vec<NoteSegment>,
}

#[derive(Debug, Clone)]
pub(super) struct NoteSegment {
    events: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CursorPoint {
    pub(super) tick: u64,
    pub(super) page_index: usize,
    pub(super) x: f32,
    pub(super) min_y: f32,
    pub(super) max_y: f32,
}

#[derive(Debug, Clone)]
pub(super) struct ScoreCursorMap {
    pub(super) points: Vec<CursorPoint>,
    pub(super) max_tick: u64,
}

#[derive(Debug, Clone)]
pub(in crate::app) struct ScoreCursorMaps {
    pub(super) maps: HashMap<PathBuf, ScoreCursorMap>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct ScoreCursorPlacement {
    pub(in crate::app) page_index: usize,
    pub(in crate::app) x: f32,
    pub(in crate::app) min_y: f32,
    pub(in crate::app) max_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct SystemBand {
    pub(in crate::app) x_start: f32,
    pub(in crate::app) x_end: f32,
    pub(in crate::app) min_y: f32,
    pub(in crate::app) max_y: f32,
}

impl ScoreCursorMaps {
    pub(in crate::app) fn for_midi_tick(
        &self,
        midi_path: &Path,
        tick: u64,
    ) -> Option<ScoreCursorPlacement> {
        let map = self.maps.get(midi_path)?;
        map.position_at(tick)
    }

    pub(in crate::app) fn max_tick_for_midi(&self, midi_path: &Path) -> Option<u64> {
        self.maps.get(midi_path).map(|map| map.max_tick)
    }

    pub(in crate::app) fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

impl ScoreCursorMap {
    fn position_at(&self, tick: u64) -> Option<ScoreCursorPlacement> {
        let first = self.points.first()?;
        if tick < first.tick || tick > self.max_tick {
            return None;
        }

        let next_index = self.points.partition_point(|point| point.tick <= tick);
        let current_index = next_index.saturating_sub(1);
        let current = *self.points.get(current_index)?;

        let mut x = current.x;
        let mut min_y = current.min_y;
        let mut max_y = current.max_y;
        if let Some(next) = self
            .points
            .get(current_index.saturating_add(1)..)
            .unwrap_or(&[])
            .iter()
            .copied()
            .find(|next| {
                next.page_index == current.page_index
                    && next.tick > current.tick
                    && next.x > current.x
            })
        {
            let distance = (next.tick - current.tick) as f32;
            let progress = ((tick - current.tick) as f32 / distance).clamp(0.0, 1.0);
            x = lerp(current.x, next.x, progress);
            min_y = lerp(current.min_y, next.min_y, progress);
            max_y = lerp(current.max_y, next.max_y, progress);
        }

        Some(ScoreCursorPlacement {
            page_index: current.page_index,
            x,
            min_y,
            max_y,
        })
    }
}

pub(in crate::app) fn parse_svg_note_anchors(
    svg_source: &str,
    page_index: usize,
) -> Vec<SvgNoteAnchor> {
    let mut anchors = Vec::new();
    let mut search_offset = 0;

    while let Some(search_tail) = svg_source.get(search_offset..) {
        let Some(anchor_start_relative) = search_tail.find("<a ") else {
            break;
        };
        let anchor_start = search_offset + anchor_start_relative;
        let Some(anchor_tail) = svg_source.get(anchor_start..) else {
            break;
        };
        let Some(anchor_end_relative) = anchor_tail.find("</a>") else {
            break;
        };
        let anchor_end = anchor_start + anchor_end_relative + "</a>".len();
        let Some(anchor) = svg_source.get(anchor_start..anchor_end) else {
            break;
        };
        search_offset = anchor_end;

        let Some(href) = attribute_value(anchor, "xlink:href") else {
            continue;
        };
        let Some((source, source_path)) = source_location_from_href(href) else {
            continue;
        };
        let Some((x, y)) = first_translate(anchor) else {
            continue;
        };

        anchors.push(SvgNoteAnchor {
            source,
            source_path,
            page_index,
            x,
            y,
        });
    }

    anchors
}

pub(in crate::app) fn point_and_click_target_at(
    anchors: &[SvgNoteAnchor],
    x: f32,
    y: f32,
) -> Option<PointAndClickTarget> {
    const CLICK_RADIUS: f32 = 4.5;
    const CLICK_RADIUS_SQUARED: f32 = CLICK_RADIUS * CLICK_RADIUS;

    let anchor = anchors
        .iter()
        .filter_map(|anchor| {
            let dx = anchor.x - x;
            let dy = anchor.y - y;
            let distance_squared = (dx * dx) + (dy * dy);
            (distance_squared <= CLICK_RADIUS_SQUARED).then_some((distance_squared, anchor))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))?
        .1;

    Some(PointAndClickTarget {
        path: anchor.source_path.clone(),
        line: anchor.source.line.saturating_sub(1) as usize,
        column: anchor.source.column as usize,
    })
}

pub(in crate::app) fn parse_svg_system_bands(svg_source: &str) -> Vec<SystemBand> {
    let staff_lines = parse_staff_lines(svg_source);
    if staff_lines.is_empty() {
        return Vec::new();
    }

    let mut by_span: Vec<((i32, i32), Vec<StaffLine>)> = Vec::new();
    for line in staff_lines {
        let key = (
            (line.x_start * 10.0).round() as i32,
            (line.x_end * 10.0).round() as i32,
        );
        if let Some((_, values)) = by_span.iter_mut().find(|(candidate, _)| *candidate == key) {
            values.push(line);
        } else {
            by_span.push((key, vec![line]));
        }
    }

    let mut bands = Vec::new();

    for (_, mut lines) in by_span {
        lines.sort_by(|left, right| left.y.total_cmp(&right.y));
        let mut index = 0usize;
        let mut staves = Vec::new();

        while index + 4 < lines.len() {
            let Some(window) = lines.get(index..index + 5) else {
                break;
            };
            let mut spacing_ok = true;
            for pair in window.windows(2) {
                let [previous, next] = pair else {
                    continue;
                };
                let spacing = next.y - previous.y;
                if (spacing - STAFF_LINE_SPACING_TARGET).abs() > STAFF_LINE_SPACING_TOLERANCE {
                    spacing_ok = false;
                    break;
                }
            }

            if spacing_ok {
                let Some(first) = window.first() else {
                    break;
                };
                let Some(last) = window.last() else {
                    break;
                };
                staves.push((first.x_start, first.x_end, first.y, last.y));
                index += 5;
            } else {
                index += 1;
            }
        }

        if staves.is_empty() {
            continue;
        }

        staves.sort_by(|left, right| left.2.total_cmp(&right.2));
        let mut current: Vec<(f32, f32, f32, f32)> = Vec::new();

        for staff in staves {
            if let Some(previous) = current.last().copied() {
                let gap = staff.2 - previous.3;
                if gap > SYSTEM_VERTICAL_GAP_THRESHOLD {
                    if let Some(system) = system_from_staves(&current) {
                        bands.push(system);
                    }
                    current.clear();
                }
            }
            current.push(staff);
        }

        if let Some(system) = system_from_staves(&current) {
            bands.push(system);
        }
    }

    bands.sort_by(|left, right| {
        left.min_y
            .total_cmp(&right.min_y)
            .then_with(|| left.x_start.total_cmp(&right.x_start))
    });
    bands
}

pub(in crate::app) fn score_contains_repeats(path: &Path) -> Result<bool, String> {
    for code in score_source_code_lines(path)? {
        if code.contains("\\repeat") || code.contains("\\alternative") || code.contains("\\volta") {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(in crate::app) fn score_disables_point_and_click(path: &Path) -> Result<bool, String> {
    for code in score_source_code_lines(path)? {
        if code.contains("\\pointAndClickOff")
            || (code.contains("point-and-click") && code.contains("#f"))
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(in crate::app) fn build_score_cursor_maps(
    build_dir: &Path,
    score_stem: &str,
    anchors: &[SvgNoteAnchor],
    midi_files: &[MidiRollFile],
) -> Result<ScoreCursorMaps, String> {
    if anchors.is_empty() || midi_files.is_empty() {
        return Ok(ScoreCursorMaps {
            maps: HashMap::new(),
        });
    }

    let note_files = collect_note_files(build_dir, score_stem)?;
    if note_files.is_empty() {
        return Ok(ScoreCursorMaps {
            maps: HashMap::new(),
        });
    }

    let anchor_index = index_anchors(anchors);
    let mut maps = HashMap::new();

    for midi_file in midi_files {
        let Some(map) = best_map_for_midi(&note_files, &anchor_index, &midi_file.data) else {
            continue;
        };
        maps.insert(midi_file.path.clone(), map);
    }

    Ok(ScoreCursorMaps { maps })
}

pub(super) fn collect_note_files(
    build_dir: &Path,
    score_stem: &str,
) -> Result<Vec<NotesFile>, String> {
    let entries = fs::read_dir(build_dir).map_err(|error| {
        format!(
            "Failed to read build directory {} for score cursor: {error}",
            build_dir.display()
        )
    })?;

    let mut note_paths = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!("Failed to inspect build artifact for score cursor mapping: {error}")
        })?;
        let path = entry.path();
        if !is_notes_artifact(&path, score_stem) {
            continue;
        }
        note_paths.push(path);
    }

    note_paths.sort();

    let mut note_files = Vec::new();
    for path in note_paths {
        let events = parse_note_events(&path)?;
        let segments = split_note_segments(events);
        if !segments.is_empty() {
            note_files.push(NotesFile { segments });
        }
    }

    Ok(note_files)
}

pub(super) fn is_notes_artifact(path: &Path, score_stem: &str) -> bool {
    let extension_matches = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("notes"));
    if !extension_matches {
        return false;
    }

    let stem = path.file_stem().and_then(|value| value.to_str());
    stem.is_some_and(|stem| stem == score_stem || stem.starts_with(&format!("{score_stem}-")))
}

pub(super) fn parse_note_events(path: &Path) -> Result<Vec<NoteEvent>, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "Failed to read score cursor note file {}: {error}",
            path.display()
        )
    })?;
    let mut events = Vec::new();

    for line in source.lines() {
        let Some(event) = parse_note_event_line(line) else {
            continue;
        };
        events.push(event);
    }

    Ok(events)
}

pub(super) fn parse_note_event_line(line: &str) -> Option<NoteEvent> {
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() < 6 || columns.get(1).copied()? != "note" {
        return None;
    }

    let moment_whole = columns.first()?.parse::<f64>().ok()?;
    let pitch = columns.get(2)?.parse::<u8>().ok()?;
    let duration_whole = columns.get(4)?.parse::<f64>().ok()?;
    let point_and_click = columns
        .iter()
        .copied()
        .find(|value| value.starts_with("point-and-click "))?;

    let mut parts = point_and_click
        .strip_prefix("point-and-click ")?
        .split_whitespace();
    let column = parts.next()?.parse::<u32>().ok()?;
    let line = parts.next()?.parse::<u32>().ok()?;
    let source = SourceLocation { line, column };

    Some(NoteEvent {
        moment_whole,
        duration_whole,
        pitch,
        source,
    })
}

pub(super) fn index_anchors(
    anchors: &[SvgNoteAnchor],
) -> HashMap<SourceLocation, Vec<SvgNoteAnchor>> {
    let mut index: HashMap<SourceLocation, Vec<SvgNoteAnchor>> = HashMap::new();

    for anchor in anchors {
        index.entry(anchor.source).or_default().push(anchor.clone());
    }

    for values in index.values_mut() {
        values.sort_by(|left, right| {
            left.page_index
                .cmp(&right.page_index)
                .then_with(|| left.y.total_cmp(&right.y))
                .then_with(|| left.x.total_cmp(&right.x))
        });
    }

    index
}

pub(super) fn best_map_for_midi(
    note_files: &[NotesFile],
    anchor_index: &HashMap<SourceLocation, Vec<SvgNoteAnchor>>,
    midi_data: &MidiRollData,
) -> Option<ScoreCursorMap> {
    let mut best: Option<CandidateMap> = None;

    for note_file in note_files {
        let Some(candidate) = best_segment_map(&note_file.segments, anchor_index, midi_data) else {
            continue;
        };
        if candidate.is_better_than(best.as_ref()) {
            best = Some(candidate);
        }
    }

    best.map(|candidate| candidate.map)
}

pub(super) fn map_note_file(
    events: &[NoteEvent],
    anchor_index: &HashMap<SourceLocation, Vec<SvgNoteAnchor>>,
    ppq: u16,
) -> Option<(ScoreCursorMap, usize)> {
    if events.is_empty() {
        return None;
    }

    let ticks_per_whole = f64::from(ppq) * TICKS_PER_WHOLE_NOTE;
    let mut usage: HashMap<SourceLocation, usize> = HashMap::new();
    let mut mapped = Vec::new();
    let mut matched_events = 0usize;
    let mut max_tick = 0u64;

    for event in events {
        let Some(candidates) = anchor_index.get(&event.source) else {
            continue;
        };

        let next_index = usage.entry(event.source).or_insert(0);
        let Some(anchor) = candidates.get(*next_index % candidates.len()).cloned() else {
            continue;
        };
        *next_index += 1;

        let start_tick = (event.moment_whole * ticks_per_whole).round();
        let end_tick =
            ((event.moment_whole + event.duration_whole.max(0.0)) * ticks_per_whole).round();
        let tick = if start_tick.is_sign_negative() {
            0
        } else {
            crate::number::f64_to_u64(start_tick)
        };
        let end_tick = if end_tick.is_sign_negative() {
            tick
        } else {
            crate::number::f64_to_u64(end_tick)
        };
        max_tick = max_tick.max(end_tick.max(tick));

        mapped.push((tick, anchor.page_index, anchor.x, anchor.y));
        matched_events += 1;
    }

    if mapped.is_empty() {
        return None;
    }

    mapped.sort_by_key(|left| left.0);

    let mut points = Vec::new();
    let mut index = 0usize;

    while index < mapped.len() {
        let Some(&(tick, page_index, first_x, first_y)) = mapped.get(index) else {
            break;
        };
        let mut min_y = first_y;
        let mut max_y = first_y;
        let mut x_values = vec![first_x];
        let mut group_end = index + 1;

        while group_end < mapped.len() {
            let Some(&(next_tick, next_page, next_x, next_y)) = mapped.get(group_end) else {
                break;
            };
            if next_tick != tick || next_page != page_index {
                break;
            }

            min_y = min_y.min(next_y);
            max_y = max_y.max(next_y);
            x_values.push(next_x);
            group_end += 1;
        }

        x_values.sort_by(f32::total_cmp);
        let mid = x_values.len() / 2;
        let x = x_values.get(mid).copied().unwrap_or(first_x);
        points.push(CursorPoint {
            tick,
            page_index,
            x,
            min_y,
            max_y,
        });

        index = group_end;
    }

    if points.is_empty() {
        return None;
    }

    max_tick = max_tick.max(points.last().map(|point| point.tick).unwrap_or(0));
    Some((ScoreCursorMap { points, max_tick }, matched_events))
}

pub(super) fn attribute_value<'a>(source: &'a str, attribute_name: &str) -> Option<&'a str> {
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

pub(super) fn source_location_from_href(href: &str) -> Option<(SourceLocation, Option<PathBuf>)> {
    let href = href.strip_prefix("textedit://")?;

    let mut parts = href.rsplitn(4, ':');
    let _column_end = parts.next()?.parse::<u32>().ok()?;
    let column = parts.next()?.parse::<u32>().ok()?;
    let line = parts.next()?.parse::<u32>().ok()?;
    let source_path = parts.next().and_then(decode_textedit_path);

    Some((SourceLocation { line, column }, source_path))
}

pub(super) fn decode_textedit_path(source: &str) -> Option<PathBuf> {
    let trimmed = source
        .strip_prefix("localhost/")
        .or_else(|| source.strip_prefix("localhost"))
        .unwrap_or(source);
    let decoded = percent_decode(trimmed);

    if decoded.is_empty() {
        return None;
    }

    Some(PathBuf::from(decoded))
}

pub(super) fn percent_decode(source: &str) -> String {
    let mut decoded = Vec::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes.get(index).copied() == Some(b'%')
            && index + 2 < bytes.len()
            && let Some(hex_bytes) = bytes.get(index + 1..index + 3)
            && let Ok(hex) = std::str::from_utf8(hex_bytes)
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            decoded.push(value);
            index += 3;
            continue;
        }

        if let Some(byte) = bytes.get(index).copied() {
            decoded.push(byte);
        }
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

pub(super) fn first_translate(source: &str) -> Option<(f32, f32)> {
    let translate_start = source.find("translate(")? + "translate(".len();
    let tail = source.get(translate_start..)?;
    let translate_end = tail.find(')')?;
    let values = tail.get(..translate_end)?;

    let mut numbers = values
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse::<f32>().ok());

    let x = numbers.next()?;
    let y = numbers.next()?;

    Some((x, y))
}

pub(super) fn lerp(left: f32, right: f32, amount: f32) -> f32 {
    left + (right - left) * amount
}

pub(super) fn score_source_code_lines(path: &Path) -> Result<Vec<String>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read score source {}: {error}", path.display()))?;

    Ok(source
        .lines()
        .map(|line| line.split('%').next().unwrap_or("").trim().to_string())
        .collect())
}

pub(super) fn split_note_segments(events: Vec<NoteEvent>) -> Vec<NoteSegment> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut current = Vec::new();
    let Some(first) = events.first() else {
        return Vec::new();
    };
    let mut previous_moment = first.moment_whole;

    for event in events {
        if !current.is_empty() && event.moment_whole + f64::EPSILON < previous_moment {
            segments.push(NoteSegment { events: current });
            current = Vec::new();
        }
        previous_moment = event.moment_whole;
        current.push(event);
    }

    if !current.is_empty() {
        segments.push(NoteSegment { events: current });
    }

    segments
}

#[derive(Debug, Clone)]
pub(super) struct CandidateMap {
    pub(super) map: ScoreCursorMap,
    pub(super) span_score: usize,
    pub(super) coverage_score: usize,
    pub(super) pitch_score: usize,
    pub(super) tick_delta: u64,
}

impl CandidateMap {
    fn rank(&self) -> (usize, usize, usize, Reverse<u64>) {
        (
            self.span_score,
            self.pitch_score,
            self.coverage_score,
            Reverse(self.tick_delta),
        )
    }

    pub(super) fn is_better_than(&self, best: Option<&Self>) -> bool {
        best.is_none_or(|best| self.rank() > best.rank())
    }
}

pub(super) fn best_segment_map(
    segments: &[NoteSegment],
    anchor_index: &HashMap<SourceLocation, Vec<SvgNoteAnchor>>,
    midi_data: &MidiRollData,
) -> Option<CandidateMap> {
    let mut best: Option<CandidateMap> = None;
    let ppq = midi_data.ppq;
    let midi_reference_end = midi_data
        .notes
        .iter()
        .map(|note| note.start_tick)
        .max()
        .unwrap_or(midi_data.total_ticks)
        .max(1);

    for segment in segments {
        let Some((map, matched)) = map_note_file(&segment.events, anchor_index, ppq) else {
            continue;
        };
        let total = segment.events.len();
        let coverage_ratio = coverage(matched, total);
        if coverage_ratio < 2500 {
            continue;
        }
        let pitch_score = pitch_match_score(&segment.events, midi_data, ppq);
        if pitch_score < 1800 {
            continue;
        }
        let span_score = span_similarity_score(map.max_tick, midi_reference_end);
        if span_score < 2200 {
            continue;
        }

        let tick_delta = map.max_tick.abs_diff(midi_reference_end);
        let candidate = CandidateMap {
            map,
            span_score,
            coverage_score: coverage_ratio,
            pitch_score,
            tick_delta,
        };

        if candidate.is_better_than(best.as_ref()) {
            best = Some(candidate);
        }
    }

    best
}
