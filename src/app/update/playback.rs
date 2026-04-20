use std::fs;
use std::path::{Path, PathBuf};

use lilypalooza_audio::{AudioEngine, INSTRUMENT_TRACK_COUNT, SoundfontResource, TrackId};

use super::*;

const DEFAULT_SOUNDFONT_ID: &str = "default";
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
        self.soundfont_status = SoundfontStatus::Ready(soundfont_path.clone());
        self.logger.push(format!(
            "Selected playback soundfont {}",
            soundfont_path.display()
        ));

        if self.playback.is_none() {
            self.logger
                .push("Skipping soundfont load because audio engine is disabled");
            return;
        }

        if let Some(playback) = self.playback.as_mut()
            && let Err(error) = load_soundfont_resource(playback, &soundfont_path)
        {
            self.soundfont_status = SoundfontStatus::Error(error.clone());
            self.logger.push(error.clone());
        }

        self.sync_playback_file();
    }

    pub(in crate::app) fn unload_playback_file(&mut self) {
        if let Some(playback) = self.playback.as_mut() {
            playback.clear_score();
        }
    }

    pub(in crate::app) fn sync_playback_file(&mut self) {
        let Some(current_file) = self.piano_roll.current_file().cloned() else {
            self.unload_playback_file();
            return;
        };
        let selected_file = current_file.path.clone();
        let SoundfontStatus::Ready(soundfont_path) = &self.soundfont_status else {
            if self.playback.is_none() {
                return;
            }
            return;
        };
        let Some(playback) = self.playback.as_mut() else {
            self.logger
                .push("Skipping playback sync because audio engine is disabled");
            return;
        };

        let mut load_error = None;

        match fs::read(&selected_file) {
            Ok(bytes) => {
                let track_labels = current_file
                    .data
                    .tracks
                    .iter()
                    .map(|track| track.label.clone())
                    .collect::<Vec<_>>();
                if let Err(error) = sync_playback_engine(playback, &bytes, &track_labels) {
                    load_error = Some(error);
                } else {
                    self.soundfont_status = SoundfontStatus::Ready(soundfont_path.clone());
                    self.logger.push(format!(
                        "Loaded MIDI for playback {}",
                        selected_file.display()
                    ));
                    let total_ticks = self.current_midi_total_ticks();
                    self.piano_roll.set_playback_position(0, total_ticks, false);
                    self.refresh_score_cursor_overlay();
                    self.sync_playback_track_mix();
                }
            }
            Err(error) => {
                load_error = Some(format!(
                    "Failed to read MIDI file {}: {error}",
                    selected_file.display()
                ));
            }
        }

        if let Some(error) = load_error {
            self.soundfont_status = SoundfontStatus::Error(error.clone());
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
        let total_ticks = self.current_midi_total_ticks();
        let tick = tick.min(total_ticks);
        let is_playing = self.piano_roll.playback_is_playing();

        if let (Some(playback), Some(current_file)) =
            (self.playback.as_mut(), self.piano_roll.current_file())
        {
            let ppq = f64::from(current_file.data.ppq.max(1));
            playback.transport().seek_beats_immediate(tick as f64 / ppq);
        }

        self.piano_roll
            .set_playback_position(tick, total_ticks, is_playing);
        self.refresh_score_cursor_overlay();
    }

    pub(in crate::app) fn refresh_playback_position(&mut self) {
        let total_ticks = self.current_midi_total_ticks();
        let Some(playback) = self.playback.as_mut() else {
            let current_tick = self.piano_roll.playback_tick().min(total_ticks);
            self.piano_roll
                .set_playback_position(current_tick, total_ticks, false);
            if self.score_pane_visible() {
                self.refresh_score_cursor_overlay();
            } else {
                self.score_cursor_overlay = None;
            }
            return;
        };

        let is_playing = playback.sequencer().playback_is_playing();
        let actual_tick = playback
            .sequencer()
            .playback_tick()
            .map(|tick| tick.min(total_ticks))
            .unwrap_or_else(|_| self.piano_roll.playback_tick().min(total_ticks));

        self.piano_roll
            .set_playback_position(actual_tick, total_ticks, is_playing);
        if self.score_pane_visible() {
            self.refresh_score_cursor_overlay();
        } else {
            self.score_cursor_overlay = None;
        }
    }
    pub(in crate::app) fn current_midi_total_ticks(&self) -> u64 {
        self.piano_roll
            .current_file()
            .map(|file| file.data.total_ticks)
            .unwrap_or(0)
    }
}

fn load_soundfont_resource(
    playback: &mut AudioEngine,
    soundfont_path: &Path,
) -> Result<(), String> {
    let soundfont = SoundfontResource {
        id: DEFAULT_SOUNDFONT_ID.to_string(),
        name: soundfont_name(soundfont_path),
        path: soundfont_path.to_path_buf(),
    };

    playback
        .mixer()
        .set_soundfont(soundfont)
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn sync_playback_engine(
    playback: &mut AudioEngine,
    midi_bytes: &[u8],
    track_labels: &[String],
) -> Result<(), String> {
    playback
        .replace_score_from_midi_bytes(midi_bytes)
        .map_err(|error| error.to_string())?;
    {
        let mut mixer = playback.mixer();
        for track_index in 0..INSTRUMENT_TRACK_COUNT {
            let label = track_labels
                .get(track_index)
                .cloned()
                .unwrap_or_else(|| format!("Track {}", track_index + 1));
            let _ = mixer.set_track_name(TrackId(track_index as u16), label.clone());
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
