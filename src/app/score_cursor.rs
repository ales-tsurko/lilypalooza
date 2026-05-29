use std::{
    cmp::Reverse,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::midi::{MidiRollData, MidiRollFile};

mod matching;
mod parsing;

use matching::*;
use parsing::*;
pub(in crate::app) use parsing::{
    ScoreCursorMaps, ScoreCursorPlacement, SvgNoteAnchor, SystemBand, build_score_cursor_maps,
    parse_svg_note_anchors, parse_svg_system_bands, point_and_click_target_at,
    score_contains_repeats, score_disables_point_and_click,
};
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::*;
    use crate::midi::{MidiNote, MidiTrack, TempoChange, TimeSignatureChange};

    fn anchor(line: u32, x: f32, y: f32) -> SvgNoteAnchor {
        SvgNoteAnchor {
            source: SourceLocation { line, column: 1 },
            source_path: Some(PathBuf::from("score.ly")),
            page_index: 0,
            x,
            y,
        }
    }

    fn midi_data(notes: Vec<MidiNote>) -> MidiRollData {
        MidiRollData {
            ppq: 480,
            total_ticks: 960,
            notes,
            tracks: vec![MidiTrack {
                index: 0,
                label: "Piano".to_string(),
            }],
            time_signatures: vec![TimeSignatureChange {
                tick: 0,
                numerator: 4,
                denominator: 4,
            }],
            tempo_changes: vec![TempoChange {
                tick: 0,
                micros_per_quarter: 500_000,
            }],
            bar_lines: vec![0],
            min_pitch: 60,
            max_pitch: 64,
        }
    }

    fn midi_note(start_tick: u64, end_tick: u64, pitch: u8) -> MidiNote {
        MidiNote {
            start_tick,
            end_tick,
            pitch,
            track_index: 0,
        }
    }

    #[test]
    fn parses_lilypond_note_events_with_point_and_click() {
        let event = parse_note_event_line("0.25\tnote\t64\tname\t0.125\tpoint-and-click 9 42 file")
            .unwrap();

        crate::test_assertions::assert_float_eq!(event.moment_whole, 0.25);
        crate::test_assertions::assert_float_eq!(event.duration_whole, 0.125);
        assert_eq!(event.pitch, 64);
        assert_eq!(
            event.source,
            SourceLocation {
                line: 42,
                column: 9
            }
        );
        assert!(parse_note_event_line("0.25\trest\t64\tname\t0.125").is_none());
        assert!(parse_note_event_line("0.25\tnote\t64").is_none());
    }

    #[test]
    fn parses_score_system_bands_from_staff_lines() {
        fn staff_group(y: f32) -> String {
            format!(r#"<g transform="translate(10 {y})"><line x1="0" y1="0" x2="120" y2="0"/></g>"#)
        }

        let mut svg = String::new();
        for y in [10.0, 11.0, 12.0, 13.0, 14.0, 22.0, 23.0, 24.0, 25.0, 26.0] {
            svg.push_str(&staff_group(y));
        }
        for y in [50.0, 51.0, 52.0, 53.0, 54.0] {
            svg.push_str(&staff_group(y));
        }
        svg.push_str(r#"<g transform="translate(10 90)"><line x1="0" y1="0" x2="2" y2="0"/></g>"#);

        let bands = parse_svg_system_bands(&svg);

        assert_eq!(bands.len(), 2);
        crate::test_assertions::assert_float_eq!(bands[0].x_start, 10.0);
        crate::test_assertions::assert_float_eq!(bands[0].x_end, 130.0);
        crate::test_assertions::assert_float_eq!(bands[0].min_y, 10.0);
        crate::test_assertions::assert_float_eq!(bands[0].max_y, 26.0);
        crate::test_assertions::assert_float_eq!(bands[1].min_y, 50.0);
        crate::test_assertions::assert_float_eq!(bands[1].max_y, 54.0);
    }

    #[test]
    fn maps_score_notes_to_midi_cursor_positions() {
        let build_dir = tempdir().unwrap();
        std::fs::write(
            build_dir.path().join("score.notes"),
            "0\tnote\t60\tname\t0.25\tpoint-and-click 1 1 \
             file\n0.25\tnote\t62\tname\t0.25\tpoint-and-click 1 2 file\n",
        )
        .unwrap();
        let midi_path = build_dir.path().join("score.midi");
        let maps = build_score_cursor_maps(
            build_dir.path(),
            "score",
            &[anchor(1, 10.0, 20.0), anchor(2, 30.0, 24.0)],
            &[MidiRollFile {
                path: midi_path.clone(),
                file_name: "score.midi".to_string(),
                data: midi_data(vec![midi_note(0, 240, 60), midi_note(480, 720, 62)]),
            }],
        )
        .unwrap();

        let placement = maps.for_midi_tick(&midi_path, 240).unwrap();

        assert_eq!(maps.max_tick_for_midi(&midi_path), Some(960));
        assert_eq!(placement.page_index, 0);
        crate::test_assertions::assert_float_eq!(placement.x, 20.0);
        crate::test_assertions::assert_float_eq!(placement.min_y, 22.0);
        crate::test_assertions::assert_float_eq!(placement.max_y, 22.0);
        assert!(maps.for_midi_tick(&midi_path, 961).is_none());
    }

    #[test]
    fn candidate_ranking_prefers_span_pitch_coverage_then_shorter_delta() {
        let map = ScoreCursorMap {
            points: vec![CursorPoint {
                tick: 0,
                page_index: 0,
                x: 0.0,
                min_y: 0.0,
                max_y: 0.0,
            }],
            max_tick: 0,
        };
        let baseline = CandidateMap {
            map: map.clone(),
            span_score: 10,
            coverage_score: 10,
            pitch_score: 10,
            tick_delta: 10,
        };
        let closer = CandidateMap {
            map,
            span_score: 10,
            coverage_score: 10,
            pitch_score: 10,
            tick_delta: 9,
        };

        assert!(baseline.is_better_than(None));
        assert!(closer.is_better_than(Some(&baseline)));
        assert!(!baseline.is_better_than(Some(&closer)));
    }

    #[test]
    fn detects_score_cursor_source_flags_ignoring_comments() {
        let dir = tempdir().unwrap();
        let score = dir.path().join("score.ly");
        std::fs::write(
            &score,
            "% \\repeat ignored\n\\pointAndClickOff\nmusic = { c' \\alternative { c' } }\n",
        )
        .unwrap();

        assert!(score_disables_point_and_click(&score).unwrap());
        assert!(score_contains_repeats(&score).unwrap());
    }

    #[test]
    fn parses_svg_note_anchors_and_point_and_click_targets() {
        let svg = r#"
            <a xlink:href="textedit://localhost/tmp/score.ly:12:8:9">
                <g transform="translate(21.5, 34.0)"><path d=""/></g>
            </a>
        "#;

        let anchors = parse_svg_note_anchors(svg, 3);
        let target = point_and_click_target_at(&anchors, 22.0, 34.0).unwrap();

        assert_eq!(anchors.len(), 1);
        assert_eq!(target.path, Some(PathBuf::from("tmp/score.ly")));
        assert_eq!(target.line, 11);
        assert_eq!(target.column, 8);
        assert!(point_and_click_target_at(&anchors, 100.0, 100.0).is_none());
    }
}
