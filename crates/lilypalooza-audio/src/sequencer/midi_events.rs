use super::{
    scheduler::{MetronomeClick, SequencerConfig, SequencerRuntime, TimeSignaturePoint},
    *,
};

pub(super) fn metronome_clicks_between(
    time_signatures: &[TimeSignaturePoint],
    start_beat: Beats,
    end_beat: Beats,
) -> Vec<MetronomeClick> {
    let start = start_beat.as_beats_f64();
    let end = end_beat.as_beats_f64();
    if end <= start {
        return Vec::new();
    }

    let mut clicks = Vec::new();
    for (index, signature) in time_signatures.iter().enumerate() {
        let segment_start = signature.at.as_beats_f64();
        let segment_end = time_signatures
            .get(index + 1)
            .map(|next| next.at.as_beats_f64())
            .unwrap_or(f64::INFINITY);
        let window_start = start.max(segment_start);
        let window_end = end.min(segment_end);
        if window_end <= window_start {
            continue;
        }

        let beat_unit = 4.0 / f64::from(signature.denominator.max(1));
        let beats_from_segment = (window_start - segment_start) / beat_unit;
        let mut step_index = beats_from_segment.ceil() as i64;
        if step_index < 0 {
            step_index = 0;
        }
        loop {
            let click_beat = segment_start + step_index as f64 * beat_unit;
            if click_beat >= window_end {
                break;
            }
            if click_beat + 1.0e-9 >= window_start {
                clicks.push(MetronomeClick {
                    at: Beats::from_beats_f64(click_beat),
                    accented: step_index % i64::from(signature.numerator.max(1)) == 0,
                });
            }
            step_index += 1;
        }
    }
    clicks
}

pub(super) fn normalized_time_signatures(points: &[TimeSignaturePoint]) -> Vec<TimeSignaturePoint> {
    let mut points = points.to_vec();
    if !points.iter().any(|point| point.at == Beats::ZERO) {
        points.push(TimeSignaturePoint::default());
    }
    points.sort_by_key(|point| point.at);
    let mut normalized = Vec::with_capacity(points.len());
    for point in points {
        if normalized
            .last()
            .is_some_and(|last: &TimeSignaturePoint| last.at == point.at)
        {
            normalized.pop();
        }
        normalized.push(point);
    }
    normalized
}

pub(super) fn reset_schedule_state_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
) {
    for (sequence_index, sequence) in config.sequences.iter().enumerate() {
        if let Some(next_index) = runtime.next_indices.get(sequence_index) {
            next_index.store(
                first_event_at_or_after(&sequence.events, at),
                Ordering::Relaxed,
            );
        }
    }
}

pub(super) fn schedule_pause_reset_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    commands: &mut MultiThreadedKnystCommands,
) {
    #[cfg(test)]
    {
        runtime.reset_count.fetch_add(1, Ordering::Relaxed);
    }
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let Some(generation) = runtime.generations.get(track.index()) else {
            continue;
        };
        let generation = generation.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let Some(handle) = config
            .instrument_handles
            .get(track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        handle.schedule_reset_at(
            commands,
            offset_beats(
                &config.tempo_map,
                config.sample_rate,
                at,
                config.reset_frame_offset,
            ),
            generation,
        );
        dispatch_scheduled_panic_events(
            handle,
            generation,
            commands,
            &config.tempo_map,
            config.sample_rate,
            at,
            config.reset_frame_offset + 2,
        );
    }
}

pub(super) fn collect_chase_events_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    start_sample_offset: usize,
    out: &mut Vec<SchedulerChange>,
) {
    for sequence in &config.sequences {
        let Some(handle) = config
            .instrument_handles
            .get(sequence.target_track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        let Some(generation) = runtime.generations.get(sequence.target_track.index()) else {
            continue;
        };
        let generation = generation.load(Ordering::Relaxed);
        let mut sample_offset = start_sample_offset;
        for event in chase_events_at(&sequence.events, at) {
            if let Some(change) = handle.scheduler_midi_change(sample_offset, generation, event) {
                out.push(change);
            }
            sample_offset = sample_offset.saturating_add(2);
        }
    }
}

pub(super) fn collect_reset_and_chase_at(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    at: Beats,
    out: &mut Vec<SchedulerChange>,
) {
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let Some(generation) = runtime.generations.get(track.index()) else {
            continue;
        };
        let generation = generation.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let Some(handle) = config
            .instrument_handles
            .get(track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        if let Some(change) = handle.scheduler_reset_change(0, generation) {
            out.push(change);
        }
        collect_block_panic_events(handle, generation, 2, out);
    }
    if config.chase_notes_on_seek {
        collect_chase_events_at(config, runtime, at, 0, out);
    }
}

pub(super) fn dispatch_immediate_pause_reset(
    config: &SequencerConfig,
    runtime: &SequencerRuntime,
    _commands: &mut MultiThreadedKnystCommands,
) {
    let mut tracks = BTreeSet::new();
    for sequence in &config.sequences {
        tracks.insert(sequence.target_track);
    }
    for track in tracks {
        let Some(generation) = runtime.generations.get(track.index()) else {
            continue;
        };
        let generation = generation.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let Some(handle) = config
            .instrument_handles
            .get(track.index())
            .cloned()
            .flatten()
        else {
            continue;
        };
        handle.request_reset_now(generation);
    }
}

pub(super) fn collect_block_panic_events(
    handle: InstrumentRuntimeHandle,
    generation: u32,
    start_sample_offset: usize,
    out: &mut Vec<SchedulerChange>,
) {
    let mut sample_offset = start_sample_offset;
    for channel in 0..16_u8 {
        for event in [
            EngineMidiEvent::AllSoundOff { channel },
            EngineMidiEvent::AllNotesOff { channel },
            EngineMidiEvent::ResetAllControllers { channel },
        ] {
            if let Some(change) = handle.scheduler_midi_change(sample_offset, generation, event) {
                out.push(change);
            }
            sample_offset = sample_offset.saturating_add(2);
        }
    }
}

pub(super) fn block_sample_offset(
    block_start_seconds: f64,
    event_seconds: f64,
    sample_rate: f64,
    block_size: usize,
) -> usize {
    let max_offset = block_size.saturating_sub(1);
    let offset = ((event_seconds - block_start_seconds) * sample_rate).round();
    if !offset.is_finite() || offset <= 0.0 {
        0
    } else if offset >= max_offset as f64 {
        max_offset
    } else {
        offset.to_usize().unwrap_or(max_offset)
    }
}

pub(super) fn offset_beats(
    tempo_map: &MusicalTimeMap,
    sample_rate: f64,
    at: Beats,
    frame_offset: i32,
) -> Beats {
    if frame_offset <= 0 {
        return at;
    }
    let seconds = tempo_map.musical_time_to_secs_f64(at);
    let offset_seconds = seconds + (frame_offset as f64 / sample_rate);
    tempo_map.seconds_to_beats(knyst::prelude::Seconds::from_seconds_f64(offset_seconds))
}

pub(super) fn dispatch_scheduled_panic_events(
    handle: InstrumentRuntimeHandle,
    generation: u32,
    commands: &mut MultiThreadedKnystCommands,
    tempo_map: &MusicalTimeMap,
    sample_rate: f64,
    at: Beats,
    start_frame_offset: i32,
) {
    let mut frame_offset = start_frame_offset;
    for channel in 0..16_u8 {
        for event in [
            EngineMidiEvent::AllSoundOff { channel },
            EngineMidiEvent::AllNotesOff { channel },
            EngineMidiEvent::ResetAllControllers { channel },
        ] {
            handle.schedule_midi_at_with_offset(
                commands,
                offset_beats(tempo_map, sample_rate, at, frame_offset),
                generation,
                event,
            );
            frame_offset += 2;
        }
    }
}

pub(super) fn ordered_events_at_same_time(events: &[TimedMidiEvent]) -> Vec<TimedMidiEvent> {
    let mut ordered = Vec::with_capacity(events.len());
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 0),
    );
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 1),
    );
    ordered.extend(
        events
            .iter()
            .copied()
            .filter(|event| event_sort_group(event.event) == 2),
    );
    ordered
}

pub(super) fn event_sort_group(event: EngineMidiEvent) -> u8 {
    match event {
        EngineMidiEvent::NoteOff { .. }
        | EngineMidiEvent::AllNotesOff { .. }
        | EngineMidiEvent::AllSoundOff { .. } => 0,
        EngineMidiEvent::ProgramChange { .. }
        | EngineMidiEvent::ControlChange { .. }
        | EngineMidiEvent::ChannelPressure { .. }
        | EngineMidiEvent::PolyPressure { .. }
        | EngineMidiEvent::PitchBend { .. }
        | EngineMidiEvent::ResetAllControllers { .. } => 1,
        EngineMidiEvent::NoteOn { .. } => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sequence {
    pub(super) target_track: TrackId,
    pub(super) events: Vec<TimedMidiEvent>,
}

impl Sequence {
    pub(super) fn from_midi_track(
        track_index: usize,
        track: &[TrackEvent<'_>],
        ppq: u16,
    ) -> Option<Self> {
        if track_index >= INSTRUMENT_TRACK_COUNT {
            return None;
        }

        let mut absolute_ticks = 0_u64;
        let mut events = Vec::new();

        for event in track {
            absolute_ticks = absolute_ticks.saturating_add(u64::from(event.delta.as_int()));
            let TrackEventKind::Midi { channel, message } = event.kind else {
                continue;
            };
            let channel = channel.as_int();
            let Some(event) = midi_message(channel, message) else {
                continue;
            };
            events.push(TimedMidiEvent {
                at: ticks_to_beats(absolute_ticks, ppq),
                event,
            });
        }

        if events.is_empty() {
            return None;
        }

        Some(Self {
            target_track: TrackId(track_index as u16),
            events,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TimedMidiEvent {
    pub(super) at: Beats,
    pub(super) event: EngineMidiEvent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TempoPoint {
    pub(super) at: Beats,
    pub(super) bpm: f64,
}

pub(super) fn build_tempo_map(tempos: &[TempoPoint]) -> MusicalTimeMap {
    let mut tempos = tempos.to_vec();
    tempos.sort_by_key(|tempo| tempo.at);

    let mut map = MusicalTimeMap::new();
    let initial_bpm = tempos
        .iter()
        .find(|tempo| tempo.at == Beats::ZERO)
        .map_or(120.0, |tempo| tempo.bpm);
    map.replace(0, TempoChange::NewTempo { bpm: initial_bpm });
    for tempo in tempos {
        map.insert(TempoChange::NewTempo { bpm: tempo.bpm }, tempo.at);
    }
    map
}

pub(super) fn first_event_at_or_after(events: &[TimedMidiEvent], at: Beats) -> usize {
    events.partition_point(|event| event.at < at)
}

pub(super) fn chase_events_at(events: &[TimedMidiEvent], at: Beats) -> Vec<EngineMidiEvent> {
    let mut active_notes: HashMap<(u8, u8), u8> = HashMap::new();

    for timed_event in events {
        if timed_event.at >= at {
            break;
        }
        match timed_event.event {
            EngineMidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => {
                active_notes.insert((channel, note), velocity);
            }
            EngineMidiEvent::NoteOff { channel, note, .. } => {
                active_notes.remove(&(channel, note));
            }
            EngineMidiEvent::AllNotesOff { channel } | EngineMidiEvent::AllSoundOff { channel } => {
                active_notes.retain(|(note_channel, _), _| *note_channel != channel);
            }
            _ => {}
        }
    }

    for timed_event in events {
        if timed_event.at != at {
            continue;
        }
        match timed_event.event {
            EngineMidiEvent::NoteOn { channel, note, .. }
            | EngineMidiEvent::NoteOff { channel, note, .. } => {
                active_notes.remove(&(channel, note));
            }
            EngineMidiEvent::AllNotesOff { channel } | EngineMidiEvent::AllSoundOff { channel } => {
                active_notes.retain(|(note_channel, _), _| *note_channel != channel);
            }
            _ => {}
        }
    }

    let mut chased: Vec<_> = active_notes
        .into_iter()
        .map(|((channel, note), velocity)| EngineMidiEvent::NoteOn {
            channel,
            note,
            velocity,
        })
        .collect();
    chased.sort_by_key(|event| match event {
        EngineMidiEvent::NoteOn { channel, note, .. } => (*channel, *note),
        _ => (0, 0),
    });
    chased
}

pub(super) fn midi_message(channel: u8, message: MidiMessage) -> Option<EngineMidiEvent> {
    match message {
        MidiMessage::NoteOn { key, vel } => {
            Some(note_on_message(channel, key.as_int(), vel.as_int()))
        }
        MidiMessage::NoteOff { key, vel } => {
            Some(note_off_message(channel, key.as_int(), vel.as_int()))
        }
        MidiMessage::Aftertouch { key, vel } => Some(EngineMidiEvent::PolyPressure {
            channel,
            note: key.as_int(),
            pressure: vel.as_int(),
        }),
        MidiMessage::ChannelAftertouch { vel } => Some(EngineMidiEvent::ChannelPressure {
            channel,
            pressure: vel.as_int(),
        }),
        MidiMessage::Controller { controller, value } => Some(controller_message(
            channel,
            controller.as_int(),
            value.as_int(),
        )),
        MidiMessage::ProgramChange { program } => Some(EngineMidiEvent::ProgramChange {
            channel,
            program: program.as_int(),
        }),
        MidiMessage::PitchBend { bend } => Some(EngineMidiEvent::PitchBend {
            channel,
            value: bend.as_int(),
        }),
    }
}

fn note_on_message(channel: u8, note: u8, velocity: u8) -> EngineMidiEvent {
    if velocity == 0 {
        note_off_message(channel, note, 0)
    } else {
        EngineMidiEvent::NoteOn {
            channel,
            note,
            velocity,
        }
    }
}

fn note_off_message(channel: u8, note: u8, velocity: u8) -> EngineMidiEvent {
    EngineMidiEvent::NoteOff {
        channel,
        note,
        velocity,
    }
}

fn controller_message(channel: u8, controller: u8, value: u8) -> EngineMidiEvent {
    match controller {
        120 => EngineMidiEvent::AllSoundOff { channel },
        121 => EngineMidiEvent::ResetAllControllers { channel },
        123 => EngineMidiEvent::AllNotesOff { channel },
        controller => EngineMidiEvent::ControlChange {
            channel,
            controller,
            value,
        },
    }
}

pub(super) fn ticks_to_beats(ticks: u64, ppq: u16) -> Beats {
    let ppq = u64::from(ppq.max(1));
    let beats = (ticks / ppq) as u32;
    let beat_tesimals = ((ticks % ppq) * u64::from(SUBBEAT_TESIMALS_PER_BEAT) / ppq) as u32;
    Beats::new(beats, beat_tesimals)
}

pub(super) fn beats_to_ticks(beats: Beats, ppq: u16) -> u64 {
    let ticks = (beats.as_beats_f64() * f64::from(ppq.max(1))).round();
    if !ticks.is_finite() || ticks <= 0.0 {
        0
    } else {
        ticks.to_u64().unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use midly::{PitchBend, num::u7};

    use super::*;

    #[test]
    fn midi_message_maps_channel_voice_events() {
        let channel = 3;
        let cases = [
            (
                MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(100),
                },
                EngineMidiEvent::NoteOn {
                    channel,
                    note: 60,
                    velocity: 100,
                },
            ),
            (
                MidiMessage::NoteOn {
                    key: u7::from(60),
                    vel: u7::from(0),
                },
                EngineMidiEvent::NoteOff {
                    channel,
                    note: 60,
                    velocity: 0,
                },
            ),
            (
                MidiMessage::NoteOff {
                    key: u7::from(61),
                    vel: u7::from(64),
                },
                EngineMidiEvent::NoteOff {
                    channel,
                    note: 61,
                    velocity: 64,
                },
            ),
            (
                MidiMessage::Aftertouch {
                    key: u7::from(62),
                    vel: u7::from(32),
                },
                EngineMidiEvent::PolyPressure {
                    channel,
                    note: 62,
                    pressure: 32,
                },
            ),
            (
                MidiMessage::ChannelAftertouch { vel: u7::from(48) },
                EngineMidiEvent::ChannelPressure {
                    channel,
                    pressure: 48,
                },
            ),
            (
                MidiMessage::ProgramChange {
                    program: u7::from(12),
                },
                EngineMidiEvent::ProgramChange {
                    channel,
                    program: 12,
                },
            ),
            (
                MidiMessage::PitchBend {
                    bend: PitchBend::from_int(123),
                },
                EngineMidiEvent::PitchBend {
                    channel,
                    value: 123,
                },
            ),
        ];

        for (message, expected) in cases {
            assert_eq!(midi_message(channel, message), Some(expected));
        }
    }

    #[test]
    fn midi_message_maps_controller_modes() {
        let channel = 1;
        let cases = [
            (
                7,
                EngineMidiEvent::ControlChange {
                    channel,
                    controller: 7,
                    value: 99,
                },
            ),
            (120, EngineMidiEvent::AllSoundOff { channel }),
            (121, EngineMidiEvent::ResetAllControllers { channel }),
            (123, EngineMidiEvent::AllNotesOff { channel }),
        ];

        for (controller, expected) in cases {
            let message = MidiMessage::Controller {
                controller: u7::from(controller),
                value: u7::from(99),
            };
            assert_eq!(midi_message(channel, message), Some(expected));
        }
    }
}
