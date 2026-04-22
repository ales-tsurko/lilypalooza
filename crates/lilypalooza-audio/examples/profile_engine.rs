//! Headless audio-engine profiling harness.
//!
//! This example starts the engine without the desktop UI and periodically prints
//! callback-load observability plus master meter activity.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use lilypalooza_audio::instrument::soundfont_synth;
use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BUILTIN_SOUNDFONT_ID, MixerState, SlotState,
    SoundfontResource, TrackId,
};
use midly::num::{u4, u7, u15, u24, u28};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, Track, TrackEvent, TrackEventKind,
};

fn soundfont_slot(soundfont_id: &str, program: u8) -> SlotState {
    SlotState::built_in(
        BUILTIN_SOUNDFONT_ID,
        soundfont_synth::state(soundfont_id, 0, program),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options_from_iter(env::args_os().skip(1));
    let mut engine = AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())?;

    if let Some(soundfont) = &options.soundfont {
        let resource = SoundfontResource {
            id: "default".to_string(),
            name: soundfont
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("default")
                .to_string(),
            path: soundfont.clone(),
        };
        let mut mixer = engine.mixer();
        mixer.set_soundfont(resource)?;
        for track_index in 0..options.assign_tracks {
            mixer.set_track_instrument(
                TrackId(track_index as u16),
                soundfont_slot("default", options.base_program + track_index as u8),
            )?;
        }
    }

    if let Some(bytes) = options
        .midi
        .as_ref()
        .map(fs::read)
        .transpose()?
        .or_else(|| options.synthetic_midi.then(|| simple_midi_bytes(480)))
    {
        engine.replace_score_from_midi_bytes(&bytes)?;
    }

    if options.play {
        engine.transport().play();
    }

    eprintln!("pid={}", std::process::id());
    eprintln!(
        "soundfont={:?} midi={:?} synthetic_midi={} assign_tracks={} base_program={} play={} duration={}s report={}ms",
        options.soundfont,
        options.midi,
        options.synthetic_midi,
        options.assign_tracks,
        options.base_program,
        options.play,
        options.duration.as_secs(),
        options.report_interval.as_millis()
    );

    let started = Instant::now();
    while started.elapsed() < options.duration {
        thread::sleep(options.report_interval);
        let observability = engine.observability_snapshot();
        let meters = engine.meter_snapshot();
        let master_peak = meters.main.left.level.max(meters.main.right.level);
        if let Some(snapshot) = observability {
            eprintln!(
                "t={:>5.1}s load={:.3} peak={:.3} dropouts={} master={:.3}",
                started.elapsed().as_secs_f32(),
                snapshot.load_ratio,
                snapshot.peak_load_ratio,
                snapshot.dropout_count,
                master_peak
            );
        } else {
            eprintln!(
                "t={:>5.1}s load=unavailable master={:.3}",
                started.elapsed().as_secs_f32(),
                master_peak
            );
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Options {
    soundfont: Option<PathBuf>,
    midi: Option<PathBuf>,
    synthetic_midi: bool,
    assign_tracks: usize,
    base_program: u8,
    play: bool,
    duration: Duration,
    report_interval: Duration,
}

fn options_from_iter<I>(arguments: I) -> Options
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = arguments.into_iter().peekable();
    let mut options = Options {
        soundfont: None,
        midi: None,
        synthetic_midi: false,
        assign_tracks: 0,
        base_program: 0,
        play: false,
        duration: Duration::from_secs(20),
        report_interval: Duration::from_millis(1000),
    };

    while let Some(argument) = args.next() {
        match argument.to_string_lossy().as_ref() {
            "--soundfont" => options.soundfont = args.next().map(PathBuf::from),
            "--midi" => options.midi = args.next().map(PathBuf::from),
            "--synthetic-midi" => options.synthetic_midi = true,
            "--assign-tracks" => {
                if let Some(value) = args.next() {
                    options.assign_tracks = value.to_string_lossy().parse().unwrap_or(0);
                }
            }
            "--base-program" => {
                if let Some(value) = args.next() {
                    options.base_program = value.to_string_lossy().parse().unwrap_or(0);
                }
            }
            "--play" => options.play = true,
            "--duration" => {
                if let Some(value) = args.next() {
                    options.duration =
                        Duration::from_secs_f32(value.to_string_lossy().parse().unwrap_or(20.0));
                }
            }
            "--report-ms" => {
                if let Some(value) = args.next() {
                    options.report_interval =
                        Duration::from_millis(value.to_string_lossy().parse().unwrap_or(1000));
                }
            }
            _ => {}
        }
    }

    options
}

fn simple_midi_bytes(ppq: u16) -> Vec<u8> {
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(ppq)));
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
        .expect("example midi should serialize");
    bytes
}

#[cfg(test)]
mod tests {
    use super::options_from_iter;
    use std::ffi::OsString;

    #[test]
    fn parses_profile_engine_options() {
        let options = options_from_iter([
            OsString::from("--soundfont"),
            OsString::from("a.sf2"),
            OsString::from("--midi"),
            OsString::from("b.mid"),
            OsString::from("--synthetic-midi"),
            OsString::from("--assign-tracks"),
            OsString::from("4"),
            OsString::from("--base-program"),
            OsString::from("8"),
            OsString::from("--play"),
            OsString::from("--duration"),
            OsString::from("5"),
            OsString::from("--report-ms"),
            OsString::from("250"),
        ]);
        assert_eq!(
            options.soundfont.as_deref(),
            Some(PathBuf::from("a.sf2").as_path())
        );
        assert_eq!(
            options.midi.as_deref(),
            Some(PathBuf::from("b.mid").as_path())
        );
        assert!(options.synthetic_midi);
        assert_eq!(options.assign_tracks, 4);
        assert_eq!(options.base_program, 8);
        assert!(options.play);
        assert_eq!(options.duration.as_secs(), 5);
        assert_eq!(options.report_interval.as_millis(), 250);
    }
}
