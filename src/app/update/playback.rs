use std::fs;
use std::path::{Path, PathBuf};

use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, INSTRUMENT_TRACK_COUNT, InstrumentSlotState, MixerState,
    PlaybackState, SoundfontResource, TrackId,
};

use super::*;

const DEFAULT_SOUNDFONT_ID: &str = "default";
const DEFAULT_PIANO_PROGRAMS: [u8; 4] = [0, 1, 2, 3];

impl Lilypalooza {
    pub(in crate::app) fn refresh_score_cursor_overlay(&mut self) {
        self.score_cursor_overlay = None;

        let Some(cursor_maps) = &self.score_cursor_maps else {
            return;
        };
        let Some(current_file) = self.piano_roll.current_file() else {
            return;
        };

        let tick = self.piano_roll.playback_tick();
        let Some(mut placement) = cursor_maps.for_midi_tick(&current_file.path, tick) else {
            return;
        };

        if let Some(rendered_score) = self.rendered_score.as_mut()
            && placement.page_index < rendered_score.pages.len()
        {
            if let Some(page) = rendered_score.pages.get(placement.page_index)
                && let Some(system_band) = closest_system_band(
                    &page.system_bands,
                    placement.x,
                    placement.min_y,
                    placement.max_y,
                )
            {
                placement.min_y = system_band.min_y - 1.0;
                placement.max_y = system_band.max_y + 1.0;
            }

            rendered_score.current_page = placement.page_index;
        }

        self.score_cursor_overlay = Some(placement);
    }

    pub(in crate::app) fn initialize_playback(&mut self, soundfont_path: PathBuf) {
        let previous_playback = self.playback.take();

        match previous_playback {
            Some(mut playback) => {
                match configure_soundfont(&mut playback, &soundfont_path, false) {
                    Ok(()) => {
                        self.playback = Some(playback);
                        self.soundfont_status = SoundfontStatus::Ready(soundfont_path.clone());
                        self.logger.push(format!(
                            "Updated playback soundfont to {}",
                            soundfont_path.display()
                        ));
                        self.sync_playback_file();
                        self.refresh_playback_position();
                    }
                    Err(error) => {
                        self.playback = Some(playback);
                        self.soundfont_status = SoundfontStatus::Error(error.clone());
                        self.logger
                            .push(format!("Failed to update playback soundfont: {error}"));
                        self.show_prompt(
                            ErrorPrompt::new(
                                "MIDI Playback Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                }
            }
            None => match AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
            {
                Ok(mut playback) => match configure_soundfont(&mut playback, &soundfont_path, true)
                {
                    Ok(()) => {
                        self.playback = Some(playback);
                        self.soundfont_status = SoundfontStatus::Ready(soundfont_path.clone());
                        self.logger.push(format!(
                            "Playback engine ready with soundfont {}",
                            soundfont_path.display()
                        ));
                        self.sync_playback_file();
                        self.refresh_playback_position();
                    }
                    Err(error) => {
                        self.soundfont_status = SoundfontStatus::Error(error.clone());
                        self.logger
                            .push(format!("Failed to initialize playback engine: {error}"));
                        self.show_prompt(
                            ErrorPrompt::new(
                                "MIDI Playback Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                },
                Err(error) => {
                    let error = error.to_string();
                    self.soundfont_status = SoundfontStatus::Error(error.clone());
                    self.logger
                        .push(format!("Failed to initialize playback engine: {error}"));
                    self.show_prompt(
                        ErrorPrompt::new(
                            "MIDI Playback Error",
                            error,
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                }
            },
        }
    }

    pub(in crate::app) fn unload_playback_file(&mut self) {
        if let Some(playback) = self.playback.as_mut() {
            playback.transport().pause();
            playback.transport().rewind();
            playback.sequencer().clear();
        }

        self.refresh_playback_position();
    }

    pub(in crate::app) fn sync_playback_file(&mut self) {
        let selected_file = self.current_midi_file_path();

        let Some(playback) = self.playback.as_mut() else {
            self.refresh_playback_position();
            return;
        };

        let Some(selected_file) = selected_file else {
            playback.transport().pause();
            playback.transport().rewind();
            playback.sequencer().clear();
            self.refresh_playback_position();
            return;
        };

        match fs::read(&selected_file) {
            Ok(bytes) => {
                let result = playback.sequencer().replace_from_midi_bytes(&bytes);
                match result {
                    Ok(()) => {
                        self.logger.push(format!(
                            "Loaded MIDI for playback {}",
                            selected_file.display()
                        ));
                        self.sync_playback_track_mix();
                    }
                    Err(error) => {
                        let error = error.to_string();
                        self.logger
                            .push(format!("Failed to load MIDI for playback: {error}"));
                        self.show_prompt(
                            ErrorPrompt::new(
                                "MIDI Playback Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                }
            }
            Err(error) => {
                let error = format!(
                    "Failed to read MIDI file {}: {error}",
                    selected_file.display()
                );
                self.logger.push(error.clone());
                self.show_prompt(
                    ErrorPrompt::new(
                        "MIDI Playback Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }

        self.refresh_playback_position();
    }

    pub(in crate::app) fn sync_playback_track_mix(&mut self) {
        let Some(playback) = self.playback.as_mut() else {
            return;
        };

        let track_mix = self.piano_roll.current_track_mix().to_vec();
        if track_mix.is_empty() {
            return;
        }

        let mut mixer = playback.mixer();
        for (track_index, state) in track_mix.into_iter().enumerate() {
            if track_index >= INSTRUMENT_TRACK_COUNT {
                continue;
            }

            let track_id = TrackId(track_index as u16);
            let _ = mixer.set_track_muted(track_id, state.muted);
            let _ = mixer.set_track_soloed(track_id, state.soloed);
        }
    }

    pub(in crate::app) fn seek_playback_normalized(&mut self, position: f32) {
        let total_ticks = self.current_midi_total_ticks();
        let normalized = position.clamp(0.0, 1.0);
        let tick = (total_ticks as f32 * normalized).round() as u64;

        self.seek_playback_ticks(tick);
    }

    pub(in crate::app) fn rewind_target_tick(&self) -> u64 {
        let current_tick = self.piano_roll.playback_tick();
        let rewind_flag_tick = self.piano_roll.rewind_flag_tick();

        if current_tick > rewind_flag_tick {
            rewind_flag_tick
        } else {
            0
        }
    }

    pub(in crate::app) fn seek_playback_ticks(&mut self, tick: u64) {
        self.transport_seek_preview = None;

        if let Some(playback) = self.playback.as_mut()
            && let Some(current_file) = self.piano_roll.current_file()
        {
            let ppq = f64::from(current_file.data.ppq.max(1));
            let beats = tick as f64 / ppq;
            playback.transport().seek_beats(beats);
            self.refresh_playback_position();
            return;
        }

        let total_ticks = self.current_midi_total_ticks();
        self.piano_roll
            .set_playback_position(tick.min(total_ticks), total_ticks, false);
        self.refresh_score_cursor_overlay();
    }

    pub(in crate::app) fn refresh_playback_position(&mut self) {
        let total_ticks = self.current_midi_total_ticks();

        if let Some(playback) = self.playback.as_mut() {
            let tick = playback
                .sequencer()
                .playback_tick()
                .unwrap_or_else(|_| self.piano_roll.playback_tick().min(total_ticks));
            let is_playing = playback
                .transport()
                .snapshot()
                .map(|snapshot| snapshot.playback_state == PlaybackState::Playing)
                .unwrap_or_else(|_| self.piano_roll.playback_is_playing());

            self.piano_roll
                .set_playback_position(tick.min(total_ticks), total_ticks, is_playing);
            self.refresh_score_cursor_overlay();
            return;
        }

        let current_tick = self.piano_roll.playback_tick().min(total_ticks);
        self.piano_roll
            .set_playback_position(current_tick, total_ticks, false);
        self.refresh_score_cursor_overlay();
    }

    pub(in crate::app) fn current_midi_file_path(&self) -> Option<PathBuf> {
        self.piano_roll.current_file().map(|file| file.path.clone())
    }

    pub(in crate::app) fn current_midi_total_ticks(&self) -> u64 {
        self.piano_roll
            .current_file()
            .map(|file| file.data.total_ticks)
            .unwrap_or(0)
    }
}

fn configure_soundfont(
    playback: &mut AudioEngine,
    soundfont_path: &Path,
    configure_default_tracks: bool,
) -> Result<(), String> {
    let soundfont = SoundfontResource {
        id: DEFAULT_SOUNDFONT_ID.to_string(),
        name: soundfont_name(soundfont_path),
        path: soundfont_path.to_path_buf(),
    };

    {
        let mut mixer = playback.mixer();
        mixer
            .set_soundfont(soundfont)
            .map_err(|error| error.to_string())?;

        if configure_default_tracks {
            for (track_index, program) in DEFAULT_PIANO_PROGRAMS.into_iter().enumerate() {
                mixer
                    .set_track_instrument(
                        TrackId(track_index as u16),
                        InstrumentSlotState::soundfont(DEFAULT_SOUNDFONT_ID, 0, program),
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    Ok(())
}

fn soundfont_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Soundfont".to_string())
}
