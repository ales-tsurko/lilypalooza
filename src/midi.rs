use std::{
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
};

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
struct ParsedMidiTrack {
    raw_index: usize,
    explicit_label: Option<String>,
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
    collect_midi_roll_paths(build_dir, score_stem)?
        .into_iter()
        .map(load_midi_roll_file)
        .collect()
}

fn collect_midi_roll_paths(build_dir: &Path, score_stem: &str) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(build_dir).map_err(|error| {
        format!(
            "Failed to read build directory {}: {error}",
            build_dir.display()
        )
    })?;

    let mut midi_paths = Vec::new();

    for entry in entries {
        push_midi_roll_path(entry, score_stem, &mut midi_paths)?;
    }

    midi_paths.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    Ok(midi_paths
        .into_iter()
        .map(|(_sort_index, path)| path)
        .collect())
}

fn push_midi_roll_path(
    entry: io::Result<fs::DirEntry>,
    score_stem: &str,
    midi_paths: &mut Vec<(u32, PathBuf)>,
) -> Result<(), String> {
    let entry = entry.map_err(|error| format!("Failed to read build artifact entry: {error}"))?;
    let path = entry.path();
    let Some(file_stem) = matching_midi_file_stem(&path, score_stem) else {
        return Ok(());
    };

    let sort_index = midi_file_index(file_stem, score_stem).unwrap_or(u32::MAX);
    midi_paths.push((sort_index, path));
    Ok(())
}

fn matching_midi_file_stem<'a>(path: &'a Path, score_stem: &str) -> Option<&'a str> {
    let file_stem = path.file_stem()?.to_str()?;
    (is_midi_file(path) && is_score_midi_stem(file_stem, score_stem)).then_some(file_stem)
}

fn load_midi_roll_file(path: PathBuf) -> Result<MidiRollFile, String> {
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
    Ok(MidiRollFile {
        path,
        file_name,
        data,
    })
}

pub(crate) fn parse_midi_roll_file(path: &Path) -> Result<MidiRollData, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read MIDI file {}: {error}", path.display()))?;
    let mut reader = ByteReader::new(&bytes);

    let header = parse_midi_header(&mut reader, path)?;
    let mut notes = Vec::new();
    let mut parsed_tracks = Vec::new();
    let mut tempo_changes = Vec::new();
    let mut time_signatures = Vec::new();
    let mut total_ticks = 0_u64;

    for track_index in 0..header.track_count {
        let track_bytes = read_midi_track_chunk(&mut reader, path)?;
        let track = parse_midi_track(track_index, track_bytes, path)?;
        total_ticks = total_ticks.max(track.total_ticks);
        notes.extend(track.notes);
        parsed_tracks.push(track.track);
        tempo_changes.extend(track.tempo_changes);
        time_signatures.extend(track.time_signatures);
    }

    let tracks = compact_tracks_and_notes(&parsed_tracks, &mut notes);

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

    let bar_lines = build_bar_lines(total_ticks, header.ppq, &time_signatures);

    let (min_pitch, max_pitch) = note_pitch_range(&notes);

    Ok(MidiRollData {
        ppq: header.ppq,
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
struct MidiHeader {
    track_count: u16,
    ppq: u16,
}

#[derive(Debug, Clone)]
struct ParsedMidiTrackEvents {
    track: ParsedMidiTrack,
    notes: Vec<MidiNote>,
    tempo_changes: Vec<TempoChange>,
    time_signatures: Vec<TimeSignatureChange>,
    total_ticks: u64,
}

#[derive(Debug, Clone, Copy)]
struct ActiveNote {
    start_tick: u64,
    track_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct MidiEventStatus {
    status: u8,
    first_data: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MidiTrackFlow {
    Continue,
    End,
}

fn parse_midi_header(reader: &mut ByteReader<'_>, path: &Path) -> Result<MidiHeader, String> {
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

    reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing format", path.display()))?;
    let track_count = reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing track count", path.display()))?;
    let division = reader
        .read_u16_be()
        .ok_or_else(|| format!("MIDI file {} is missing division", path.display()))?;

    skip_extended_header(reader, path, header_length)?;
    validate_midi_division(path, division)?;

    Ok(MidiHeader {
        track_count,
        ppq: division,
    })
}

fn skip_extended_header(
    reader: &mut ByteReader<'_>,
    path: &Path,
    header_length: u32,
) -> Result<(), String> {
    let Some(extra_len) = header_length.checked_sub(6) else {
        return Ok(());
    };

    let extra_len = usize::try_from(extra_len).map_err(|error| {
        format!(
            "MIDI file {} has unsupported header length: {error}",
            path.display()
        )
    })?;
    reader
        .read_exact(extra_len)
        .ok_or_else(|| format!("MIDI file {} has truncated extended header", path.display()))?;
    Ok(())
}

fn validate_midi_division(path: &Path, division: u16) -> Result<(), String> {
    if (division & 0x8000) == 0 {
        return Ok(());
    }

    Err(format!(
        "MIDI file {} uses SMPTE timing which is not supported",
        path.display()
    ))
}

fn read_midi_track_chunk<'a>(reader: &mut ByteReader<'a>, path: &Path) -> Result<&'a [u8], String> {
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
    let track_len = usize::try_from(track_len).map_err(|error| {
        format!(
            "MIDI file {} has oversized track length: {error}",
            path.display()
        )
    })?;

    reader
        .read_exact(track_len)
        .ok_or_else(|| format!("MIDI file {} has truncated track data", path.display()))
}

fn parse_midi_track(
    track_index: u16,
    track_bytes: &[u8],
    path: &Path,
) -> Result<ParsedMidiTrackEvents, String> {
    let mut reader = ByteReader::new(track_bytes);
    let mut state = MidiTrackState::new(track_index);

    while !reader.is_eof() {
        if state.read_event(&mut reader, path)? == MidiTrackFlow::End {
            break;
        }
    }

    state.finish();
    Ok(state.into_events())
}

struct MidiTrackState {
    track_index: u16,
    absolute_tick: u64,
    running_status: Option<u8>,
    track_name: Option<String>,
    instrument_name: Option<String>,
    active_notes: HashMap<(u8, u8), Vec<ActiveNote>>,
    notes: Vec<MidiNote>,
    tempo_changes: Vec<TempoChange>,
    time_signatures: Vec<TimeSignatureChange>,
}

impl MidiTrackState {
    fn new(track_index: u16) -> Self {
        Self {
            track_index,
            absolute_tick: 0,
            running_status: None,
            track_name: None,
            instrument_name: None,
            active_notes: HashMap::new(),
            notes: Vec::new(),
            tempo_changes: Vec::new(),
            time_signatures: Vec::new(),
        }
    }

    fn read_event(
        &mut self,
        reader: &mut ByteReader<'_>,
        path: &Path,
    ) -> Result<MidiTrackFlow, String> {
        let delta = reader
            .read_vlq()
            .ok_or_else(|| format!("MIDI file {} has invalid delta time", path.display()))?;
        self.absolute_tick = self.absolute_tick.saturating_add(u64::from(delta));

        let event = read_midi_event_status(reader, path, &mut self.running_status)?;
        self.dispatch_event(reader, path, event)
    }

    fn dispatch_event(
        &mut self,
        reader: &mut ByteReader<'_>,
        path: &Path,
        event: MidiEventStatus,
    ) -> Result<MidiTrackFlow, String> {
        match event.status {
            0xFF => self.read_meta_event(reader, path),
            0xF0 | 0xF7 => {
                skip_sysex_event(reader, path)?;
                Ok(MidiTrackFlow::Continue)
            }
            _ => {
                self.read_channel_event(reader, path, event)?;
                Ok(MidiTrackFlow::Continue)
            }
        }
    }

    fn read_meta_event(
        &mut self,
        reader: &mut ByteReader<'_>,
        path: &Path,
    ) -> Result<MidiTrackFlow, String> {
        let meta_type = reader
            .read_u8()
            .ok_or_else(|| format!("MIDI file {} has truncated meta event", path.display()))?;
        let data = read_length_prefixed_event_data(reader, path, "meta event")?;

        Ok(self.apply_meta_event(meta_type, data))
    }

    fn apply_meta_event(&mut self, meta_type: u8, data: &[u8]) -> MidiTrackFlow {
        match meta_type {
            0x2F => MidiTrackFlow::End,
            0x03 => {
                self.track_name = non_empty_meta_text(data);
                MidiTrackFlow::Continue
            }
            0x04 => {
                if self.instrument_name.is_none() {
                    self.instrument_name = non_empty_meta_text(data);
                }
                MidiTrackFlow::Continue
            }
            0x51 => {
                self.tempo_changes
                    .extend(meta_tempo(self.absolute_tick, data));
                MidiTrackFlow::Continue
            }
            0x58 => {
                self.time_signatures
                    .extend(meta_time_signature(self.absolute_tick, data));
                MidiTrackFlow::Continue
            }
            _ => MidiTrackFlow::Continue,
        }
    }

    fn read_channel_event(
        &mut self,
        reader: &mut ByteReader<'_>,
        path: &Path,
        event: MidiEventStatus,
    ) -> Result<(), String> {
        let payload = read_channel_event_payload(reader, path, event)?;
        self.apply_channel_event(event.status, payload);
        Ok(())
    }

    fn apply_channel_event(&mut self, status: u8, payload: ChannelEventPayload) {
        let event_type = status & 0xF0;
        let channel = status & 0x0F;

        match event_type {
            0x80 => self.finish_active_note(channel, payload.data1),
            0x90 if payload.data2 == Some(0) => self.finish_active_note(channel, payload.data1),
            0x90 => self.start_active_note(channel, payload.data1),
            _ => {}
        }
    }

    fn start_active_note(&mut self, channel: u8, pitch: u8) {
        self.active_notes
            .entry((channel, pitch))
            .or_default()
            .push(ActiveNote {
                start_tick: self.absolute_tick,
                track_index: usize::from(self.track_index),
            });
    }

    fn finish_active_note(&mut self, channel: u8, pitch: u8) {
        finish_active_note(
            &mut self.active_notes,
            &mut self.notes,
            channel,
            pitch,
            self.absolute_tick,
        );
    }

    fn finish(&mut self) {
        for ((_, pitch), mut stack) in self.active_notes.drain() {
            while let Some(active) = stack.pop() {
                self.notes.push(MidiNote {
                    start_tick: active.start_tick,
                    end_tick: self.absolute_tick.max(active.start_tick.saturating_add(1)),
                    pitch,
                    track_index: active.track_index,
                });
            }
        }
    }

    fn into_events(self) -> ParsedMidiTrackEvents {
        ParsedMidiTrackEvents {
            track: ParsedMidiTrack {
                raw_index: usize::from(self.track_index),
                explicit_label: explicit_track_label(
                    self.track_name.as_deref(),
                    self.instrument_name.as_deref(),
                ),
            },
            notes: self.notes,
            tempo_changes: self.tempo_changes,
            time_signatures: self.time_signatures,
            total_ticks: self.absolute_tick,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChannelEventPayload {
    data1: u8,
    data2: Option<u8>,
}

fn read_midi_event_status(
    reader: &mut ByteReader<'_>,
    path: &Path,
    running_status: &mut Option<u8>,
) -> Result<MidiEventStatus, String> {
    let status_or_data = reader
        .read_u8()
        .ok_or_else(|| format!("MIDI file {} has truncated event", path.display()))?;

    if status_or_data < 0x80 {
        return running_status_event(path, *running_status, status_or_data);
    }

    update_running_status(status_or_data, running_status);
    Ok(MidiEventStatus {
        status: status_or_data,
        first_data: None,
    })
}

fn running_status_event(
    path: &Path,
    running_status: Option<u8>,
    first_data: u8,
) -> Result<MidiEventStatus, String> {
    let Some(status) = running_status else {
        return Err(format!(
            "MIDI file {} uses running status before status byte",
            path.display()
        ));
    };

    Ok(MidiEventStatus {
        status,
        first_data: Some(first_data),
    })
}

fn update_running_status(status: u8, running_status: &mut Option<u8>) {
    *running_status = if status < 0xF0 { Some(status) } else { None };
}

fn skip_sysex_event(reader: &mut ByteReader<'_>, path: &Path) -> Result<(), String> {
    read_length_prefixed_event_data(reader, path, "sysex event").map(|_| ())
}

fn read_length_prefixed_event_data<'a>(
    reader: &mut ByteReader<'a>,
    path: &Path,
    event_name: &str,
) -> Result<&'a [u8], String> {
    let data_len = reader.read_vlq().ok_or_else(|| {
        format!(
            "MIDI file {} has invalid {event_name} length",
            path.display()
        )
    })?;
    let data_len = usize::try_from(data_len).map_err(|error| {
        format!(
            "MIDI file {} has oversized {event_name}: {error}",
            path.display()
        )
    })?;

    reader.read_exact(data_len).ok_or_else(|| {
        format!(
            "MIDI file {} has truncated {event_name} data",
            path.display()
        )
    })
}

fn non_empty_meta_text(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn meta_tempo(tick: u64, data: &[u8]) -> Option<TempoChange> {
    let [hi, mid, lo] = data else {
        return None;
    };

    Some(TempoChange {
        tick,
        micros_per_quarter: (u32::from(*hi) << 16) | (u32::from(*mid) << 8) | u32::from(*lo),
    })
}

fn meta_time_signature(tick: u64, data: &[u8]) -> Option<TimeSignatureChange> {
    let (&numerator, &denominator_power) = data.first().zip(data.get(1))?;

    Some(TimeSignatureChange {
        tick,
        numerator: numerator.max(1),
        denominator: 2_u8.pow(u32::from(denominator_power)).max(1),
    })
}

fn read_channel_event_payload(
    reader: &mut ByteReader<'_>,
    path: &Path,
    event: MidiEventStatus,
) -> Result<ChannelEventPayload, String> {
    let data_len = channel_event_data_len(event.status);
    let data1 = read_channel_data1(reader, path, event)?;
    let data2 = read_channel_data2(reader, path, data_len)?;

    Ok(ChannelEventPayload { data1, data2 })
}

fn channel_event_data_len(status: u8) -> usize {
    if matches!(status & 0xF0, 0xC0 | 0xD0) {
        1
    } else {
        2
    }
}

fn read_channel_data1(
    reader: &mut ByteReader<'_>,
    path: &Path,
    event: MidiEventStatus,
) -> Result<u8, String> {
    if let Some(first_data) = event.first_data {
        return Ok(first_data);
    }

    reader
        .read_u8()
        .ok_or_else(|| format!("MIDI file {} has truncated channel event", path.display()))
}

fn read_channel_data2(
    reader: &mut ByteReader<'_>,
    path: &Path,
    data_len: usize,
) -> Result<Option<u8>, String> {
    if data_len != 2 {
        return Ok(None);
    }

    reader
        .read_u8()
        .map(Some)
        .ok_or_else(|| format!("MIDI file {} has truncated channel event", path.display()))
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
        Some(u16::from_be_bytes(bytes.try_into().ok()?))
    }

    fn read_u32_be(&mut self) -> Option<u32> {
        let bytes = self.read_exact(4)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
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

fn explicit_track_label(track_name: Option<&str>, instrument_name: Option<&str>) -> Option<String> {
    if let Some(name) = track_name.map(str::trim).filter(|name| !name.is_empty()) {
        return Some(name.to_string());
    }

    if let Some(name) = instrument_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return Some(name.to_string());
    }

    None
}

fn compact_tracks_and_notes(tracks: &[ParsedMidiTrack], notes: &mut [MidiNote]) -> Vec<MidiTrack> {
    let mut used_raw_indices = Vec::new();
    for note in notes.iter() {
        if !used_raw_indices.contains(&note.track_index) {
            used_raw_indices.push(note.track_index);
        }
    }

    let mut compacted_tracks = Vec::with_capacity(used_raw_indices.len());
    let mut raw_to_compact = HashMap::with_capacity(used_raw_indices.len());
    for (compact_index, raw_index) in used_raw_indices.into_iter().enumerate() {
        let explicit_label = tracks
            .iter()
            .find(|track| track.raw_index == raw_index)
            .and_then(|track| track.explicit_label.clone());
        compacted_tracks.push(MidiTrack {
            index: compact_index,
            label: explicit_label.unwrap_or_else(|| format!("Track {}", compact_index + 1)),
        });
        raw_to_compact.insert(raw_index, compact_index);
    }
    for note in notes.iter_mut() {
        if let Some(compact_index) = raw_to_compact.get(&note.track_index) {
            note.track_index = *compact_index;
        }
    }

    compacted_tracks
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

    if normalized.first().is_none_or(|tempo| tempo.tick != 0) {
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

    if normalized
        .first()
        .is_none_or(|signature| signature.tick != 0)
    {
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
mod tests;
