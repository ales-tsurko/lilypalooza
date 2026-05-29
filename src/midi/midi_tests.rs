use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, Track, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
};
use tempfile::NamedTempFile;

use super::{midi_file_index, parse_midi_roll_file};

#[test]
fn midi_index_matches_primary_and_suffix() {
    assert_eq!(midi_file_index("score", "score"), Some(1));
    assert_eq!(midi_file_index("score-2", "score"), Some(2));
}

#[test]
fn midi_index_ignores_non_matching_stem() {
    assert_eq!(midi_file_index("other", "score"), None);
    assert_eq!(midi_file_index("score-final", "score"), None);
}

#[test]
fn parse_midi_roll_compacts_non_musical_tracks() {
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(480)));
    let tempo_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Tempo".as_slice())),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let note_track: Track<'static> = vec![
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Violin".as_slice())),
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(100),
                },
            },
        },
        TrackEvent {
            delta: u28::from(480),
            kind: TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(60),
                    vel: u7::from(0),
                },
            },
        },
        TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    let smf = Smf {
        header,
        tracks: vec![tempo_track, note_track],
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .expect("test midi should serialize");

    let file = NamedTempFile::new().expect("temp midi file should exist");
    std::fs::write(file.path(), bytes).expect("temp midi bytes should write");

    let data = parse_midi_roll_file(file.path()).expect("midi should parse");

    assert_eq!(data.tracks.len(), 1);
    assert_eq!(data.tracks[0].index, 0);
    assert_eq!(data.tracks[0].label, "Violin");
    assert_eq!(data.notes.len(), 1);
    assert_eq!(data.notes[0].track_index, 0);
}
