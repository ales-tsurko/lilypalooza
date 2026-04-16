use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct MidiRollFile {
    pub(crate) path: std::path::PathBuf,
    pub(crate) file_name: String,
    pub(crate) data: MidiRollData,
}

#[derive(Debug, Clone)]
pub(crate) struct MidiRollData {
    pub(crate) ppq: u16,
    pub(crate) total_ticks: u64,
    pub(crate) notes: Vec<MidiNote>,
    pub(crate) tracks: Vec<MidiTrack>,
    pub(crate) time_signatures: Vec<TimeSignatureChange>,
    pub(crate) tempo_changes: Vec<TempoChange>,
    pub(crate) bar_lines: Vec<u64>,
    pub(crate) min_pitch: u8,
    pub(crate) max_pitch: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct MidiTrack {
    pub(crate) index: usize,
    pub(crate) label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct MidiNote {
    pub(crate) start_tick: u64,
    pub(crate) end_tick: u64,
    pub(crate) pitch: u8,
    pub(crate) track_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TimeSignatureChange {
    pub(crate) tick: u64,
    pub(crate) numerator: u8,
    pub(crate) denominator: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TempoChange {
    pub(crate) tick: u64,
    pub(crate) micros_per_quarter: u32,
}

pub(crate) fn collect_midi_roll_files(
    build_dir: &Path,
    score_stem: &str,
) -> Result<Vec<MidiRollFile>, String> {
    let entries = fs::read_dir(build_dir).map_err(|error| {
        format!(
            "Failed to read build directory {}: {error}",
            build_dir.display()
        )
    })?;

    let mut midi_paths = Vec::new();

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Failed to read build artifact entry: {error}"))?;
        let path = entry.path();

        if !is_midi_file(&path) {
            continue;
        }

        let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !is_score_midi_stem(file_stem, score_stem) {
            continue;
        }

        let sort_index = midi_file_index(file_stem, score_stem).unwrap_or(u32::MAX);
        midi_paths.push((sort_index, path));
    }

    midi_paths.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut result = Vec::new();

    for (_sort_index, path) in midi_paths {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "MIDI artifact has invalid UTF-8 file name: {}",
                    path.display()
                )
            })?
            .to_string();

        let data = parse_midi_roll_file(&path)?;
        result.push(MidiRollFile {
            path,
            file_name,
            data,
        });
    }

    Ok(result)
}

pub(crate) fn parse_midi_roll_file(path: &Path) -> Result<MidiRollData, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read MIDI file {}: {error}", path.display()))?;

    let mut reader = ByteReader::new(&bytes);

    let header_tag = reader
        .read_exact(4)
        .ok_or_else(|| format!("MIDI file {} is missing MThd header", path.display()))?;

    if header_tag != b"MThd" {
        return Err(format!(
            "MIDI file {} has invalid header chunk",
            path.display()
        ));
    }

    let header_length = reader
        .read_u32_be()
        .ok_or_else(|| format!("MIDI file {} has truncated header", path.display()))?;

    if header_length < 6 {
        return Err(format!(
            "MIDI file {} has invalid header length {header_length}",
            path.display()
        ));
    }

    let _format = reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing format", path.display()))?;
    let track_count = reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing track count", path.display()))?;
    let division = reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing division", path.display()))?;

    if header_length > 6 {
        let extra_len = usize::try_from(header_length - 6)
            .map_err(|_| format!("MIDI file {} has unsupported header length", path.display()))?;
        let _ = reader
            .read_exact(extra_len)
            .ok_or_else(|| format!("MIDI file {} has truncated extended header", path.display()))?;
    }

    if (division & 0x8000) != 0 {
        return Err(format!(
            "MIDI file {} uses SMPTE timing which is not supported",
            path.display()
        ));
    }

    let ppq = division;

    let mut notes = Vec::new();
    let mut tracks = Vec::new();
    let mut tempo_changes = Vec::new();
    let mut time_signatures = Vec::new();

    let mut total_ticks = 0_u64;

    for track_index in 0..track_count {
        let track_tag = reader
            .read_exact(4)
            .ok_or_else(|| format!("MIDI file {} has truncated track header", path.display()))?;

        if track_tag != b"MTrk" {
            return Err(format!(
                "MIDI file {} has invalid track chunk",
                path.display()
            ));
        }

        let track_len = reader
            .read_u32_be()
            .ok_or_else(|| format!("MIDI file {} has truncated track length", path.display()))?;
        let track_len = usize::try_from(track_len)
            .map_err(|_| format!("MIDI file {} has oversized track length", path.display()))?;

        let track_bytes = reader
            .read_exact(track_len)
            .ok_or_else(|| format!("MIDI file {} has truncated track data", path.display()))?;

        let mut track_reader = ByteReader::new(track_bytes);
        let mut absolute_tick = 0_u64;
        let mut running_status: Option<u8> = None;
        let mut track_name: Option<String> = None;
        let mut instrument_name: Option<String> = None;
        let mut active_notes: HashMap<(u8, u8), Vec<ActiveNote>> = HashMap::new();
        while !track_reader.is_eof() {
            let delta = track_reader
                .read_vlq()
                .ok_or_else(|| format!("MIDI file {} has invalid delta time", path.display()))?;
            absolute_tick = absolute_tick.saturating_add(u64::from(delta));

            let status_or_data = track_reader
                .read_u8()
                .ok_or_else(|| format!("MIDI file {} has truncated event", path.display()))?;

            let (status, first_data) = if status_or_data < 0x80 {
                let Some(status) = running_status else {
                    return Err(format!(
                        "MIDI file {} uses running status before status byte",
                        path.display()
                    ));
                };
                (status, Some(status_or_data))
            } else {
                if status_or_data < 0xF0 {
                    running_status = Some(status_or_data);
                } else {
                    running_status = None;
                }
                (status_or_data, None)
            };

            match status {
                0xFF => {
                    let meta_type = track_reader.read_u8().ok_or_else(|| {
                        format!("MIDI file {} has truncated meta event", path.display())
                    })?;
                    let data_len = track_reader.read_vlq().ok_or_else(|| {
                        format!("MIDI file {} has invalid meta event length", path.display())
                    })?;
                    let data_len = usize::try_from(data_len).map_err(|_| {
                        format!("MIDI file {} has oversized meta event", path.display())
                    })?;
                    let data = track_reader.read_exact(data_len).ok_or_else(|| {
                        format!("MIDI file {} has truncated meta event data", path.display())
                    })?;

                    match meta_type {
                        0x2F => break,
                        0x03 => {
                            let raw_name = String::from_utf8_lossy(data).trim().to_string();
                            if !raw_name.is_empty() {
                                track_name = Some(raw_name);
                            }
                        }
                        0x04 => {
                            let raw_name = String::from_utf8_lossy(data).trim().to_string();
                            if !raw_name.is_empty() && instrument_name.is_none() {
                                instrument_name = Some(raw_name);
                            }
                        }
                        0x51 => {
                            if data.len() == 3 {
                                let micros_per_quarter = (u32::from(data[0]) << 16)
                                    | (u32::from(data[1]) << 8)
                                    | u32::from(data[2]);
                                tempo_changes.push(TempoChange {
                                    tick: absolute_tick,
                                    micros_per_quarter,
                                });
                            }
                        }
                        0x58 => {
                            if data.len() >= 2 {
                                let numerator = data[0].max(1);
                                let denominator = 2_u8.pow(u32::from(data[1])).max(1);
                                time_signatures.push(TimeSignatureChange {
                                    tick: absolute_tick,
                                    numerator,
                                    denominator,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let data_len = track_reader.read_vlq().ok_or_else(|| {
                        format!("MIDI file {} has invalid sysex length", path.display())
                    })?;
                    let data_len = usize::try_from(data_len).map_err(|_| {
                        format!("MIDI file {} has oversized sysex event", path.display())
                    })?;
                    let _ = track_reader.read_exact(data_len).ok_or_else(|| {
                        format!("MIDI file {} has truncated sysex event", path.display())
                    })?;
                }
                _ => {
                    let event_type = status & 0xF0;
                    let channel = status & 0x0F;

                    let data_len = if matches!(event_type, 0xC0 | 0xD0) {
                        1
                    } else {
                        2
                    };
                    let data1 = if let Some(first_data) = first_data {
                        first_data
                    } else {
                        track_reader.read_u8().ok_or_else(|| {
                            format!("MIDI file {} has truncated channel event", path.display())
                        })?
                    };
                    let data2 = if data_len == 2 {
                        Some(track_reader.read_u8().ok_or_else(|| {
                            format!("MIDI file {} has truncated channel event", path.display())
                        })?)
                    } else {
                        None
                    };

                    match event_type {
                        0x80 => {
                            finish_active_note(
                                &mut active_notes,
                                &mut notes,
                                channel,
                                data1,
                                absolute_tick,
                            );
                        }
                        0x90 => {
                            let velocity = data2.unwrap_or(0);

                            if velocity == 0 {
                                finish_active_note(
                                    &mut active_notes,
                                    &mut notes,
                                    channel,
                                    data1,
                                    absolute_tick,
                                );
                            } else {
                                active_notes.entry((channel, data1)).or_default().push(
                                    ActiveNote {
                                        start_tick: absolute_tick,
                                        track_index: usize::from(track_index),
                                    },
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        for ((_, pitch), mut stack) in active_notes {
            while let Some(active) = stack.pop() {
                notes.push(MidiNote {
                    start_tick: active.start_tick,
                    end_tick: absolute_tick.max(active.start_tick.saturating_add(1)),
                    pitch,
                    track_index: active.track_index,
                });
            }
        }

        total_ticks = total_ticks.max(absolute_tick);

        tracks.push(MidiTrack {
            index: usize::from(track_index),
            label: format_track_label(
                usize::from(track_index),
                track_name.as_deref(),
                instrument_name.as_deref(),
            ),
        });
    }

    notes.sort_by(|left, right| {
        left.start_tick
            .cmp(&right.start_tick)
            .then_with(|| left.pitch.cmp(&right.pitch))
            .then_with(|| left.track_index.cmp(&right.track_index))
    });

    if let Some(last_note) = notes.last() {
        total_ticks = total_ticks.max(last_note.end_tick);
    }

    tempo_changes = normalize_tempos(tempo_changes);
    time_signatures = normalize_time_signatures(time_signatures);

    let bar_lines = build_bar_lines(total_ticks, ppq, &time_signatures);

    let (min_pitch, max_pitch) = note_pitch_range(&notes);

    Ok(MidiRollData {
        ppq,
        total_ticks,
        notes,
        tracks,
        time_signatures,
        tempo_changes,
        bar_lines,
        min_pitch,
        max_pitch,
    })
}

#[derive(Debug, Clone, Copy)]
struct ActiveNote {
    start_tick: u64,
    track_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct ByteReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.bytes.len()
    }

    fn read_u8(&mut self) -> Option<u8> {
        let value = *self.bytes.get(self.cursor)?;
        self.cursor += 1;
        Some(value)
    }

    fn read_u16_be(&mut self) -> Option<u16> {
        let bytes = self.read_exact(2)?;
        Some(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_be(&mut self) -> Option<u32> {
        let bytes = self.read_exact(4)?;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_exact(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.cursor.checked_add(len)?;
        let bytes = self.bytes.get(self.cursor..end)?;
        self.cursor = end;
        Some(bytes)
    }

    fn read_vlq(&mut self) -> Option<u32> {
        let mut value = 0_u32;

        for _ in 0..4 {
            let byte = self.read_u8()?;
            value = (value << 7) | u32::from(byte & 0x7F);

            if (byte & 0x80) == 0 {
                return Some(value);
            }
        }

        None
    }
}

fn is_midi_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mid") || extension.eq_ignore_ascii_case("midi")
        })
}

fn midi_file_index(file_stem: &str, score_stem: &str) -> Option<u32> {
    if file_stem == score_stem {
        return Some(1);
    }

    let suffix = file_stem.strip_prefix(score_stem)?.strip_prefix('-')?;
    suffix.parse::<u32>().ok()
}

fn is_score_midi_stem(file_stem: &str, score_stem: &str) -> bool {
    file_stem == score_stem || file_stem.starts_with(&format!("{score_stem}-"))
}

fn format_track_label(
    track_index: usize,
    track_name: Option<&str>,
    instrument_name: Option<&str>,
) -> String {
    if let Some(name) = track_name.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }

    if let Some(name) = instrument_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }

    format!("Track {}", track_index.saturating_add(1))
}

fn finish_active_note(
    active_notes: &mut HashMap<(u8, u8), Vec<ActiveNote>>,
    notes: &mut Vec<MidiNote>,
    channel: u8,
    pitch: u8,
    end_tick: u64,
) {
    let Some(stack) = active_notes.get_mut(&(channel, pitch)) else {
        return;
    };

    let Some(active) = stack.pop() else {
        return;
    };

    let end_tick = end_tick.max(active.start_tick.saturating_add(1));

    notes.push(MidiNote {
        start_tick: active.start_tick,
        end_tick,
        pitch,
        track_index: active.track_index,
    });
}

fn normalize_tempos(mut tempos: Vec<TempoChange>) -> Vec<TempoChange> {
    tempos.sort_by_key(|tempo| tempo.tick);

    let mut normalized: Vec<TempoChange> = Vec::new();

    for tempo in tempos {
        if let Some(last) = normalized.last_mut()
            && last.tick == tempo.tick
        {
            *last = tempo;
        } else {
            normalized.push(tempo);
        }
    }

    if normalized.is_empty() || normalized[0].tick != 0 {
        normalized.insert(
            0,
            TempoChange {
                tick: 0,
                micros_per_quarter: 500_000,
            },
        );
    }

    normalized
}

fn normalize_time_signatures(mut signatures: Vec<TimeSignatureChange>) -> Vec<TimeSignatureChange> {
    signatures.sort_by_key(|signature| signature.tick);

    let mut normalized: Vec<TimeSignatureChange> = Vec::new();

    for signature in signatures {
        if let Some(last) = normalized.last_mut()
            && last.tick == signature.tick
        {
            *last = signature;
        } else {
            normalized.push(signature);
        }
    }

    if normalized.is_empty() || normalized[0].tick != 0 {
        normalized.insert(
            0,
            TimeSignatureChange {
                tick: 0,
                numerator: 4,
                denominator: 4,
            },
        );
    }

    normalized
}

fn build_bar_lines(total_ticks: u64, ppq: u16, signatures: &[TimeSignatureChange]) -> Vec<u64> {
    if signatures.is_empty() {
        return vec![0];
    }

    let mut lines = Vec::new();

    for (index, signature) in signatures.iter().enumerate() {
        let start_tick = signature.tick;
        let end_tick = signatures
            .get(index + 1)
            .map(|next| next.tick)
            .unwrap_or(total_ticks.saturating_add(1));
        let bar_length = bar_length_ticks(ppq, *signature).max(1);

        let mut tick = start_tick;

        while tick <= total_ticks && tick < end_tick {
            if lines.last().copied() != Some(tick) {
                lines.push(tick);
            }

            tick = tick.saturating_add(bar_length);
        }
    }

    if lines.is_empty() {
        lines.push(0);
    }

    lines
}

fn bar_length_ticks(ppq: u16, signature: TimeSignatureChange) -> u64 {
    let quarter = u64::from(ppq);
    let numerator = u64::from(signature.numerator.max(1));
    let denominator = u64::from(signature.denominator.max(1));

    quarter.saturating_mul(4).saturating_mul(numerator) / denominator
}

fn note_pitch_range(notes: &[MidiNote]) -> (u8, u8) {
    let Some(first) = notes.first() else {
        return (21, 108);
    };

    let mut min_pitch = first.pitch;
    let mut max_pitch = first.pitch;

    for note in notes {
        min_pitch = min_pitch.min(note.pitch);
        max_pitch = max_pitch.max(note.pitch);
    }

    (min_pitch.min(21), max_pitch.max(108))
}

#[cfg(test)]
mod tests {
    use midly::num::{u4, u7, u15, u24, u28};
    use midly::{
        Format, Header, MetaMessage, MidiMessage, Smf, Timing, Track, TrackEvent, TrackEventKind,
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
    fn parse_midi_roll_preserves_track_order() {
        let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(480)));
        let tempo_track: Track<'static> = vec![
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

        assert_eq!(data.tracks.len(), 2);
        assert_eq!(data.tracks[0].index, 0);
        assert_eq!(data.tracks[1].index, 1);
        assert_eq!(data.notes.len(), 1);
        assert_eq!(data.notes[0].track_index, 1);
    }
}
