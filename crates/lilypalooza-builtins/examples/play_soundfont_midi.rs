#![allow(missing_docs)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BUILTIN_SOUNDFONT_ID, MixerState, SlotState,
    SoundfontResource, TrackId,
};
use lilypalooza_builtins::soundfont_synth;

const DEFAULT_SOUNDFONT_ID: &str = "default";
const DEFAULT_PIANO_PROGRAMS: [u8; 4] = [0, 1, 2, 3];

fn soundfont_slot(soundfont_id: &str, program: u8) -> SlotState {
    SlotState::built_in(
        BUILTIN_SOUNDFONT_ID,
        soundfont_synth::state(soundfont_id, 0, program),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    lilypalooza_builtins::register_all();

    let mut args = env::args_os();
    let program = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("play_soundfont_midi"));
    let Some(soundfont_path) = args.next().map(PathBuf::from) else {
        print_usage(&program);
        return Ok(());
    };
    let Some(midi_path) = args.next().map(PathBuf::from) else {
        print_usage(&program);
        return Ok(());
    };

    let midi_bytes = fs::read(&midi_path)?;
    let mut engine = AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())?;

    {
        let mut mixer = engine.mixer();
        mixer.set_soundfont(SoundfontResource {
            id: DEFAULT_SOUNDFONT_ID.to_string(),
            name: soundfont_name(&soundfont_path),
            path: soundfont_path.clone(),
        })?;
        for (track_index, program) in DEFAULT_PIANO_PROGRAMS.iter().copied().enumerate() {
            mixer.set_track_instrument(
                TrackId(track_index as u16),
                soundfont_slot(DEFAULT_SOUNDFONT_ID, program),
            )?;
        }
    }

    engine.flush();
    engine.sequencer().replace_from_midi_bytes(&midi_bytes)?;

    let total_ticks = engine.sequencer().total_ticks();
    if total_ticks == 0 {
        return Err("loaded MIDI has no musical events".into());
    }

    eprintln!("Playing {}", midi_path.display());
    eprintln!("SoundFont: {}", soundfont_path.display());
    eprintln!("Total ticks: {total_ticks}");

    engine.transport().play();

    loop {
        thread::sleep(Duration::from_millis(50));
        let playback_tick = engine.sequencer().playback_tick()?;
        if playback_tick >= total_ticks {
            break;
        }
    }

    thread::sleep(Duration::from_millis(500));
    engine.transport().pause();
    Ok(())
}

fn print_usage(program: &Path) {
    eprintln!("Usage: {} <soundfont.sf2> <file.mid>", program.display());
}

fn soundfont_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Soundfont".to_string())
}
