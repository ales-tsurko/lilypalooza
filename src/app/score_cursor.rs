use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::midi::{MidiRollData, MidiRollFile};

const TICKS_PER_WHOLE_NOTE: f64 = 4.0;
const STAFF_LINE_LENGTH_MIN: f32 = 30.0;
const STAFF_LINE_SPACING_TARGET: f32 = 1.0;
const STAFF_LINE_SPACING_TOLERANCE: f32 = 0.25;
const SYSTEM_VERTICAL_GAP_THRESHOLD: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SourceLocation {
    line: u32,
    column: u32,
}

#[derive(Debug, Clone)]
pub(super) struct SvgNoteAnchor {
    source: SourceLocation,
    source_path: Option<PathBuf>,
    page_index: usize,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
pub(super) struct PointAndClickTarget {
    pub(super) path: Option<PathBuf>,
    pub(super) line: usize,
    pub(super) column: usize,
}

#[derive(Debug, Clone, Copy)]
struct NoteEvent {
    moment_whole: f64,
    duration_whole: f64,
    pitch: u8,
    source: SourceLocation,
}

#[derive(Debug, Clone)]
struct NotesFile {
    segments: Vec<NoteSegment>,
}

#[derive(Debug, Clone)]
struct NoteSegment {
    events: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Copy)]
struct CursorPoint {
    tick: u64,
    page_index: usize,
    x: f32,
    min_y: f32,
    max_y: f32,
}

#[derive(Debug, Clone)]
struct ScoreCursorMap {
    points: Vec<CursorPoint>,
    max_tick: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ScoreCursorMaps {
    maps: HashMap<PathBuf, ScoreCursorMap>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScoreCursorPlacement {
    pub(super) page_index: usize,
    pub(super) x: f32,
    pub(super) min_y: f32,
    pub(super) max_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SystemBand {
    pub(super) x_start: f32,
    pub(super) x_end: f32,
    pub(super) min_y: f32,
    pub(super) max_y: f32,
}

impl ScoreCursorMaps {
    pub(super) fn for_midi_tick(
        &self,
        midi_path: &Path,
        tick: u64,
    ) -> Option<ScoreCursorPlacement> {
        let map = self.maps.get(midi_path)?;
        map.position_at(tick)
    }

    pub(super) fn max_tick_for_midi(&self, midi_path: &Path) -> Option<u64> {
        self.maps.get(midi_path).map(|map| map.max_tick)
    }

    pub(super) fn is_empty(&self) -> bool {
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
        if let Some(next) = self.points[current_index + 1..]
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

pub(super) fn parse_svg_note_anchors(svg_source: &str, page_index: usize) -> Vec<SvgNoteAnchor> {
    let mut anchors = Vec::new();
    let mut search_offset = 0;

    while let Some(anchor_start_relative) = svg_source[search_offset..].find("<a ") {
        let anchor_start = search_offset + anchor_start_relative;
        let Some(anchor_end_relative) = svg_source[anchor_start..].find("</a>") else {
            break;
        };
        let anchor_end = anchor_start + anchor_end_relative + "</a>".len();
        let anchor = &svg_source[anchor_start..anchor_end];
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

pub(super) fn point_and_click_target_at(
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

pub(super) fn parse_svg_system_bands(svg_source: &str) -> Vec<SystemBand> {
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
            let window = &lines[index..index + 5];
            let mut spacing_ok = true;
            for pair in window.windows(2) {
                let spacing = pair[1].y - pair[0].y;
                if (spacing - STAFF_LINE_SPACING_TARGET).abs() > STAFF_LINE_SPACING_TOLERANCE {
                    spacing_ok = false;
                    break;
                }
            }

            if spacing_ok {
                let top = window[0].y;
                let bottom = window[4].y;
                staves.push((window[0].x_start, window[0].x_end, top, bottom));
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

pub(super) fn score_contains_repeats(path: &Path) -> Result<bool, String> {
    for code in score_source_code_lines(path)? {
        if code.contains("\\repeat") || code.contains("\\alternative") || code.contains("\\volta") {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(super) fn score_disables_point_and_click(path: &Path) -> Result<bool, String> {
    for code in score_source_code_lines(path)? {
        if code.contains("\\pointAndClickOff")
            || (code.contains("point-and-click") && code.contains("#f"))
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(super) fn build_score_cursor_maps(
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

fn collect_note_files(build_dir: &Path, score_stem: &str) -> Result<Vec<NotesFile>, String> {
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

fn is_notes_artifact(path: &Path, score_stem: &str) -> bool {
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

fn parse_note_events(path: &Path) -> Result<Vec<NoteEvent>, String> {
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

fn parse_note_event_line(line: &str) -> Option<NoteEvent> {
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

    let mut parts = point_and_click["point-and-click ".len()..].split_whitespace();
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

fn index_anchors(anchors: &[SvgNoteAnchor]) -> HashMap<SourceLocation, Vec<SvgNoteAnchor>> {
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

fn best_map_for_midi(
    note_files: &[NotesFile],
    anchor_index: &HashMap<SourceLocation, Vec<SvgNoteAnchor>>,
    midi_data: &MidiRollData,
) -> Option<ScoreCursorMap> {
    let mut best: Option<CandidateMap> = None;

    for note_file in note_files {
        let Some(candidate) = best_segment_map(&note_file.segments, anchor_index, midi_data) else {
            continue;
        };
        let should_replace = match &best {
            None => true,
            Some(best_candidate) => {
                candidate.span_score > best_candidate.span_score
                    || (candidate.span_score == best_candidate.span_score
                        && candidate.pitch_score > best_candidate.pitch_score)
                    || (candidate.span_score == best_candidate.span_score
                        && candidate.pitch_score == best_candidate.pitch_score
                        && candidate.coverage_score > best_candidate.coverage_score)
                    || (candidate.span_score == best_candidate.span_score
                        && candidate.pitch_score == best_candidate.pitch_score
                        && candidate.coverage_score == best_candidate.coverage_score
                        && candidate.tick_delta < best_candidate.tick_delta)
            }
        };
        if should_replace {
            best = Some(candidate);
        }
    }

    best.map(|candidate| candidate.map)
}

fn map_note_file(
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
        let anchor = candidates[*next_index % candidates.len()].clone();
        *next_index += 1;

        let start_tick = (event.moment_whole * ticks_per_whole).round();
        let end_tick =
            ((event.moment_whole + event.duration_whole.max(0.0)) * ticks_per_whole).round();
        let tick = if start_tick.is_sign_negative() {
            0
        } else {
            start_tick as u64
        };
        let end_tick = if end_tick.is_sign_negative() {
            tick
        } else {
            end_tick as u64
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
        let (tick, page_index, first_x, first_y) = mapped[index];
        let mut min_y = first_y;
        let mut max_y = first_y;
        let mut x_values = vec![first_x];
        let mut group_end = index + 1;

        while group_end < mapped.len() {
            let (next_tick, next_page, next_x, next_y) = mapped[group_end];
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
        let x = x_values[mid];
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

fn attribute_value<'a>(source: &'a str, attribute_name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{attribute_name}={quote}");
        let Some(start) = source.find(&needle) else {
            continue;
        };
        let value_start = start + needle.len();
        let tail = &source[value_start..];
        let Some(value_end) = tail.find(quote) else {
            continue;
        };
        return Some(&tail[..value_end]);
    }

    None
}

fn source_location_from_href(href: &str) -> Option<(SourceLocation, Option<PathBuf>)> {
    let href = href.strip_prefix("textedit://")?;

    let mut parts = href.rsplitn(4, ':');
    let _column_end = parts.next()?.parse::<u32>().ok()?;
    let column = parts.next()?.parse::<u32>().ok()?;
    let line = parts.next()?.parse::<u32>().ok()?;
    let source_path = parts.next().and_then(decode_textedit_path);

    Some((SourceLocation { line, column }, source_path))
}

fn decode_textedit_path(source: &str) -> Option<PathBuf> {
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

fn percent_decode(source: &str) -> String {
    let mut decoded = Vec::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            decoded.push(value);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn first_translate(source: &str) -> Option<(f32, f32)> {
    let translate_start = source.find("translate(")? + "translate(".len();
    let tail = &source[translate_start..];
    let translate_end = tail.find(')')?;
    let values = &tail[..translate_end];

    let mut numbers = values
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse::<f32>().ok());

    let x = numbers.next()?;
    let y = numbers.next()?;

    Some((x, y))
}

fn lerp(left: f32, right: f32, amount: f32) -> f32 {
    left + (right - left) * amount
}

fn score_source_code_lines(path: &Path) -> Result<Vec<String>, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read score source {}: {error}", path.display()))?;

    Ok(source
        .lines()
        .map(|line| line.split('%').next().unwrap_or("").trim().to_string())
        .collect())
}

fn split_note_segments(events: Vec<NoteEvent>) -> Vec<NoteSegment> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut current = Vec::new();
    let mut previous_moment = events[0].moment_whole;

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
struct CandidateMap {
    map: ScoreCursorMap,
    span_score: usize,
    coverage_score: usize,
    pitch_score: usize,
    tick_delta: u64,
}

fn best_segment_map(
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

        let should_replace = match &best {
            None => true,
            Some(best) => {
                candidate.span_score > best.span_score
                    || (candidate.span_score == best.span_score
                        && candidate.pitch_score > best.pitch_score)
                    || (candidate.span_score == best.span_score
                        && candidate.pitch_score == best.pitch_score
                        && candidate.coverage_score > best.coverage_score)
                    || (candidate.span_score == best.span_score
                        && candidate.pitch_score == best.pitch_score
                        && candidate.coverage_score == best.coverage_score
                        && candidate.tick_delta < best.tick_delta)
            }
        };
        if should_replace {
            best = Some(candidate);
        }
    }

    best
}

fn pitch_match_score(segment: &[NoteEvent], midi_data: &MidiRollData, ppq: u16) -> usize {
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
        let quant_tick = (tick as u64) / quant;
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

fn coverage(matched: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }

    matched.saturating_mul(10_000) / total
}

fn span_similarity_score(a: u64, b: u64) -> usize {
    let min = a.min(b) as f64;
    let max = a.max(b) as f64;
    if max <= f64::EPSILON {
        return 0;
    }

    ((min / max) * 10_000.0).round() as usize
}

#[derive(Clone, Copy)]
struct StaffLine {
    x_start: f32,
    x_end: f32,
    y: f32,
}

fn parse_staff_lines(source: &str) -> Vec<StaffLine> {
    let mut lines = Vec::new();
    let mut search_offset = 0usize;

    while let Some(group_start_rel) = source[search_offset..].find("<g ") {
        let group_start = search_offset + group_start_rel;
        let Some(group_end_rel) = source[group_start..].find("</g>") else {
            break;
        };
        let group_end = group_start + group_end_rel + "</g>".len();
        let group = &source[group_start..group_end];
        search_offset = group_end;

        let Some((tx, ty)) = first_translate(group) else {
            continue;
        };
        let Some(line_start_rel) = group.find("<line ") else {
            continue;
        };
        let line = &group[line_start_rel..];

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

fn system_from_staves(staves: &[(f32, f32, f32, f32)]) -> Option<SystemBand> {
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
