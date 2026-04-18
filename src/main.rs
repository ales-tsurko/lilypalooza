//! Lilypalooza desktop application entry point.
//!
//! This binary wires the UI modules and starts the Iced runtime.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

mod app;
mod browser_file_watcher;
mod editor_file_watcher;
mod error_prompt;
mod fonts;
mod icons;
mod lilypond;
mod logger;
mod midi;
mod score_watcher;
mod settings;
mod shortcuts;
mod state;
mod status_bar;
mod ui_style;

fn main() -> iced::Result {
    let startup = startup_options();
    if startup.no_gui_playback {
        return run_headless_playback(&startup).map_err(|error| {
            iced::Error::WindowCreationFailed(Box::new(std::io::Error::other(error)))
        });
    }
    app::run(
        startup.soundfont,
        startup.score,
        startup.audio_enabled,
        startup.audio_isolation,
    )
}

struct StartupOptions {
    audio_isolation: bool,
    audio_enabled: bool,
    assign_tracks: usize,
    base_program: u8,
    no_gui_playback: bool,
    soundfont: Option<PathBuf>,
    score: Option<PathBuf>,
}

fn startup_options() -> StartupOptions {
    startup_options_from_iter(env::args_os().skip(1))
}

fn startup_options_from_iter<I>(arguments: I) -> StartupOptions
where
    I: IntoIterator<Item = OsString>,
{
    const SOUND_FONT_ENV: &str = "LILYPALOOZA_SOUNDFONT";
    const AUDIO_ISOLATION_FLAG: &str = "--audio-isolation";
    const ASSIGN_TRACKS_FLAG: &str = "--assign-tracks";
    const BASE_PROGRAM_FLAG: &str = "--base-program";
    const NO_GUI_PLAYBACK_FLAG: &str = "--no-gui-playback";
    const SOUND_FONT_FLAG: &str = "--soundfont";
    const NO_AUDIO_FLAG: &str = "--no-audio";
    const SCORE_ENV: &str = "LILYPALOOZA_SCORE";
    const SCORE_FLAG: &str = "--score";
    const SCORE_ALIAS_FLAG: &str = "--file";

    let mut args = arguments.into_iter().peekable();
    let mut audio_isolation = false;
    let mut audio_enabled = true;
    let mut assign_tracks = 4usize;
    let mut base_program = 0u8;
    let mut no_gui_playback = false;
    let mut cli_soundfont: Option<PathBuf> = None;
    let mut cli_score: Option<PathBuf> = None;

    while let Some(argument) = args.next() {
        if argument == AUDIO_ISOLATION_FLAG {
            audio_isolation = true;
            continue;
        }

        if argument == NO_GUI_PLAYBACK_FLAG {
            no_gui_playback = true;
            continue;
        }

        if argument == NO_AUDIO_FLAG {
            audio_enabled = false;
            continue;
        }

        if argument == ASSIGN_TRACKS_FLAG {
            let Some(value) = args.next() else {
                eprintln!("Ignoring {ASSIGN_TRACKS_FLAG}: no value was provided");
                continue;
            };
            assign_tracks = value.to_string_lossy().parse().unwrap_or(assign_tracks);
            continue;
        }

        if argument == BASE_PROGRAM_FLAG {
            let Some(value) = args.next() else {
                eprintln!("Ignoring {BASE_PROGRAM_FLAG}: no value was provided");
                continue;
            };
            base_program = value.to_string_lossy().parse().unwrap_or(base_program);
            continue;
        }

        if argument == SOUND_FONT_FLAG {
            let Some(value) = args.next() else {
                eprintln!("Ignoring {SOUND_FONT_FLAG}: no path was provided");
                continue;
            };
            cli_soundfont = Some(PathBuf::from(value));
            continue;
        }

        if argument == SCORE_FLAG || argument == SCORE_ALIAS_FLAG {
            let Some(value) = args.next() else {
                eprintln!("Ignoring {argument:?}: no path was provided");
                continue;
            };
            cli_score = Some(PathBuf::from(value));
            continue;
        }

        let Some(argument_str) = argument.to_str() else {
            continue;
        };

        if let Some(value) = argument_str.strip_prefix("--soundfont=") {
            if value.is_empty() {
                eprintln!("Ignoring --soundfont=: empty path");
                continue;
            }
            cli_soundfont = Some(PathBuf::from(value));
            continue;
        }

        let score_value = argument_str
            .strip_prefix("--score=")
            .or_else(|| argument_str.strip_prefix("--file="));
        let Some(value) = score_value else {
            continue;
        };

        if value.is_empty() {
            eprintln!("Ignoring score startup flag: empty path");
            continue;
        }

        cli_score = Some(PathBuf::from(value));
    }

    let soundfont = cli_soundfont.or_else(|| {
        env::var_os(SOUND_FONT_ENV)
            .filter(|value| !is_empty_os_string(value))
            .map(PathBuf::from)
    });
    let score = cli_score.or_else(|| {
        env::var_os(SCORE_ENV)
            .filter(|value| !is_empty_os_string(value))
            .map(PathBuf::from)
    });

    StartupOptions {
        audio_isolation,
        audio_enabled,
        assign_tracks,
        base_program,
        no_gui_playback,
        soundfont,
        score,
    }
}

const DEFAULT_SOUNDFONT_ID: &str = "default";

fn run_headless_playback(startup: &StartupOptions) -> Result<(), String> {
    use lilypalooza_audio::{
        AudioEngine, AudioEngineOptions, InstrumentSlotState, MixerState, SoundfontResource,
        TrackId,
    };

    let soundfont_path = startup
        .soundfont
        .as_ref()
        .ok_or_else(|| "--no-gui-playback requires --soundfont".to_string())?;
    let score_path = startup
        .score
        .as_ref()
        .ok_or_else(|| "--no-gui-playback requires --score".to_string())?;

    let _ = lilypond::check_lilypond().map_err(|error| error.to_string())?;

    let build_dir = tempfile::Builder::new()
        .prefix("lilypalooza-headless-build-")
        .tempdir()
        .map_err(|error| error.to_string())?;
    let score_stem = score_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "score path has no valid stem".to_string())?;
    let output_prefix = build_dir.path().join(score_stem);

    let mut request = lilypond::CompileRequest::new(score_path.clone());
    request.args = vec![
        "--svg".to_string(),
        "-dmidi-extension=midi".to_string(),
        "-dinclude-settings=event-listener.ly".to_string(),
        "-dpoint-and-click=note-event".to_string(),
        "-o".to_string(),
        output_prefix.to_string_lossy().to_string(),
    ];
    request.working_dir = score_path.parent().map(std::path::Path::to_path_buf);

    let session = lilypond::spawn_compile(request).map_err(|error| error.to_string())?;
    let compile_success = wait_for_compile(session)?;
    if !compile_success {
        return Err("LilyPond compile failed".to_string());
    }

    let midi_files = midi::collect_midi_roll_files(build_dir.path(), score_stem)?;
    let midi_file = midi_files
        .first()
        .ok_or_else(|| "LilyPond finished without MIDI output".to_string())?;
    let midi_bytes = fs::read(&midi_file.path).map_err(|error| error.to_string())?;

    let mut engine = AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
        .map_err(|error| error.to_string())?;
    {
        let mut mixer = engine.mixer();
        mixer
            .set_soundfont(SoundfontResource {
                id: DEFAULT_SOUNDFONT_ID.to_string(),
                name: soundfont_name(soundfont_path),
                path: soundfont_path.clone(),
            })
            .map_err(|error| error.to_string())?;
        for track_index in 0..startup.assign_tracks {
            mixer
                .set_track_instrument(
                    TrackId(track_index as u16),
                    InstrumentSlotState::soundfont(
                        DEFAULT_SOUNDFONT_ID,
                        0,
                        startup.base_program.saturating_add(track_index as u8),
                    ),
                )
                .map_err(|error| error.to_string())?;
        }
    }

    engine.flush();
    engine
        .replace_score_from_midi_bytes(&midi_bytes)
        .map_err(|error| error.to_string())?;

    let total_ticks = engine.sequencer().total_ticks();
    if total_ticks == 0 {
        return Err("loaded MIDI has no musical events".to_string());
    }

    eprintln!("Headless playback");
    eprintln!("Score: {}", score_path.display());
    eprintln!("MIDI: {}", midi_file.path.display());
    eprintln!("SoundFont: {}", soundfont_path.display());
    eprintln!(
        "Assigned tracks: {} (base program {})",
        startup.assign_tracks, startup.base_program
    );
    eprintln!("Total ticks: {total_ticks}");

    engine.transport().play();

    loop {
        thread::sleep(Duration::from_millis(50));
        let playback_tick = engine
            .sequencer()
            .playback_tick()
            .map_err(|error| error.to_string())?;
        if playback_tick >= total_ticks {
            break;
        }
    }

    thread::sleep(Duration::from_millis(250));
    engine.transport().pause();
    Ok(())
}

fn wait_for_compile(session: lilypond::CompileSession) -> Result<bool, String> {
    loop {
        match session.try_recv() {
            Ok(lilypond::CompileEvent::Log { stream, line }) => {
                let prefix = match stream {
                    lilypond::LogStream::Stdout => "lilypond:stdout",
                    lilypond::LogStream::Stderr => "lilypond:stderr",
                };
                eprintln!("[{prefix}] {line}");
            }
            Ok(lilypond::CompileEvent::ProcessError(message)) => {
                return Err(message);
            }
            Ok(lilypond::CompileEvent::Finished { success, .. }) => {
                return Ok(success);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(10)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("LilyPond compile session disconnected".to_string());
            }
        }
    }
}

fn soundfont_name(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Soundfont".to_string())
}

fn is_empty_os_string(value: &OsString) -> bool {
    value.to_str().is_none_or(str::is_empty)
}

#[cfg(test)]
mod tests {
    use super::startup_options_from_iter;
    use std::ffi::OsString;

    #[test]
    fn parses_no_audio_flag() {
        let startup = startup_options_from_iter([OsString::from("--no-audio")]);
        assert!(!startup.audio_enabled);
    }

    #[test]
    fn parses_audio_isolation_flag() {
        let startup = startup_options_from_iter([OsString::from("--audio-isolation")]);
        assert!(startup.audio_isolation);
    }

    #[test]
    fn parses_no_gui_playback_flag() {
        let startup = startup_options_from_iter([OsString::from("--no-gui-playback")]);
        assert!(startup.no_gui_playback);
    }

    #[test]
    fn parses_headless_playback_assignment_flags() {
        let startup = startup_options_from_iter([
            OsString::from("--no-gui-playback"),
            OsString::from("--assign-tracks"),
            OsString::from("1"),
            OsString::from("--base-program"),
            OsString::from("12"),
        ]);
        assert!(startup.no_gui_playback);
        assert_eq!(startup.assign_tracks, 1);
        assert_eq!(startup.base_program, 12);
    }
}
