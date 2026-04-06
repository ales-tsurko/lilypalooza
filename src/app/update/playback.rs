use super::*;

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

        match crate::playback::MidiPlayback::new(soundfont_path.clone()) {
            Ok(playback) => {
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
                self.playback = previous_playback;
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
        }
    }

    pub(in crate::app) fn unload_playback_file(&mut self) {
        if let Some(playback) = self.playback.as_mut() {
            if playback.is_playing() {
                playback.pause();
            }
            playback.jump_to_tick(0);
        }

        self.refresh_playback_position();
    }

    pub(in crate::app) fn sync_playback_file(&mut self) {
        let selected_file = self.current_midi_file_path();

        if selected_file.is_none() {
            self.refresh_playback_position();
            return;
        }

        if self.playback.is_none() {
            self.refresh_playback_position();
            return;
        }

        if self
            .playback
            .as_ref()
            .and_then(crate::playback::MidiPlayback::current_file)
            == selected_file.as_deref()
        {
            self.sync_playback_track_mix();
            self.refresh_playback_position();
            return;
        }

        let load_result = {
            let playback = self
                .playback
                .as_mut()
                .expect("playback presence already checked");
            playback.load_file(selected_file.as_deref())
        };

        match load_result {
            Ok(()) => {
                if let Some(path) = selected_file.as_ref() {
                    self.logger
                        .push(format!("Loaded MIDI for playback {}", path.display()));
                }
                self.sync_playback_track_mix();
            }
            Err(error) => {
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

        for (track_index, state) in track_mix.into_iter().enumerate() {
            if track_index >= playback.track_count() {
                continue;
            }

            let _ = playback.set_track_muted(track_index, state.muted);
            let _ = playback.set_track_solo(track_index, state.soloed);
        }
    }

    pub(in crate::app) fn seek_playback_normalized(&mut self, position: f32) {
        let total_ticks = self
            .playback
            .as_ref()
            .map(crate::playback::MidiPlayback::total_ticks)
            .unwrap_or_else(|| self.current_midi_total_ticks());
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

        if let Some(playback) = self.playback.as_mut() {
            playback.jump_to_tick(tick);
            self.refresh_playback_position();
            return;
        }

        let total_ticks = self.current_midi_total_ticks();
        self.piano_roll
            .set_playback_position(tick.min(total_ticks), total_ticks, false);
        self.refresh_score_cursor_overlay();
    }

    pub(in crate::app) fn refresh_playback_position(&mut self) {
        if let Some(playback) = self.playback.as_ref() {
            self.piano_roll.set_playback_position(
                playback.position_ticks(),
                playback.total_ticks(),
                playback.is_playing(),
            );
            self.refresh_score_cursor_overlay();
            return;
        }

        let total_ticks = self.current_midi_total_ticks();
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
