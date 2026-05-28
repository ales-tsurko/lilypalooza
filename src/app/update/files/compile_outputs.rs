use super::*;

impl Lilypalooza {
    pub(in crate::app) fn queue_compile(&mut self, message: &str) {
        if !self.compile_requested {
            self.logger.push(message.to_string());
        }
        self.compile_requested = true;
    }

    pub(in crate::app) fn start_compile_if_queued(&mut self) {
        if !self.can_start_queued_compile() {
            return;
        }

        let Some(selected_score) = self.selected_score_for_compile() else {
            return;
        };

        let output_prefix = match self.compile_output_prefix(&selected_score.1) {
            Ok(path) => path,
            Err(error) => {
                self.show_compile_start_error("Build Directory Error", error);
                return;
            }
        };

        let request = compile_request_for_score(&selected_score.0, &output_prefix);
        self.logger.push("Starting LilyPond compile".to_string());
        self.compile_generation = self.compile_generation.wrapping_add(1);
        self.start_compile_request(request);
    }

    pub(super) fn can_start_queued_compile(&self) -> bool {
        self.compile_requested && self.compile_session.is_none() && !self.compile_outputs_loading
    }

    pub(super) fn selected_score_for_compile(&mut self) -> Option<(PathBuf, String)> {
        let selected_score = self
            .current_score
            .as_ref()
            .map(|score| (score.path.clone(), score.file_name.clone()));
        if selected_score.is_none() {
            self.compile_requested = false;
        }
        selected_score
    }

    pub(super) fn show_compile_start_error(&mut self, title: &'static str, error: String) {
        self.compile_requested = false;
        self.show_prompt(
            ErrorPrompt::new(title, error, ErrorFatality::Recoverable, PromptButtons::Ok),
            None,
        );
    }

    pub(super) fn start_compile_request(&mut self, request: lilypond::CompileRequest) {
        match lilypond::spawn_compile(request) {
            Ok(session) => {
                self.compile_session = Some(session);
                self.compile_requested = false;
            }
            Err(error) => {
                self.show_compile_start_error("LilyPond Compile Error", error.to_string());
            }
        }
    }

    pub(in crate::app) fn compile_output_prefix(
        &mut self,
        selected_file_name: &str,
    ) -> Result<std::path::PathBuf, String> {
        if self.build_dir.is_none() {
            let build_dir = tempfile::Builder::new()
                .prefix("lilypalooza-build-")
                .tempdir()
                .map_err(|error| format!("Failed to create temporary build directory: {error}"))?;
            self.logger.push(format!(
                "Using temporary build dir {}",
                build_dir.path().display()
            ));
            self.build_dir = Some(build_dir);
        }

        let file_stem = selected_score_stem(selected_file_name)?;
        let build_dir = self
            .build_dir
            .as_ref()
            .ok_or_else(|| "Temporary build directory is not available".to_string())?;

        Ok(build_dir.path().join(file_stem))
    }

    pub(in crate::app) fn poll_compile_logs(&mut self) -> Task<Message> {
        let Some(session) = self.compile_session.take() else {
            return Task::none();
        };

        let drain = drain_compile_logs(&session);
        self.logger.extend(drain.lines);

        if drain.keep_session {
            self.compile_session = Some(session);
            return Task::none();
        }

        if !drain.finished_successfully {
            return Task::none();
        }

        self.load_finished_compile_outputs()
    }

    pub(super) fn load_finished_compile_outputs(&mut self) -> Task<Message> {
        let Some(selected_score) = self.current_score.as_ref() else {
            return Task::none();
        };
        let Some(build_dir) = self.build_dir.as_ref() else {
            return Task::none();
        };

        self.compile_outputs_loading = true;
        let generation = self.compile_generation;
        let build_dir = build_dir.path().to_path_buf();
        let score_path = selected_score.path.clone();
        let file_name = selected_score.file_name.clone();

        Task::perform(
            async move {
                super::messages::CompileOutputsReady {
                    generation,
                    result: load_compile_outputs(build_dir, score_path, file_name),
                }
            },
            Message::CompileOutputsReady,
        )
    }

    pub(in crate::app) fn handle_compile_outputs_ready(
        &mut self,
        ready: super::messages::CompileOutputsReady,
    ) -> Task<Message> {
        self.compile_outputs_loading = false;

        if !self.compile_outputs_ready_matches_current(&ready) {
            self.start_compile_if_queued();
            return Task::none();
        }

        match ready.result {
            Ok(outputs) => self.apply_loaded_compile_outputs(outputs),
            Err(error) => self.handle_compile_outputs_error(error),
        }

        self.start_compile_if_queued();
        Task::none()
    }

    pub(super) fn compile_outputs_ready_matches_current(
        &self,
        ready: &super::messages::CompileOutputsReady,
    ) -> bool {
        if ready.generation != self.compile_generation {
            return false;
        }

        let current_score_path = self
            .current_score
            .as_ref()
            .map(|score| score.path.as_path());
        ready
            .result
            .as_ref()
            .ok()
            .is_none_or(|outputs| current_score_path == Some(outputs.score_path.as_path()))
    }

    pub(super) fn apply_loaded_compile_outputs(&mut self, outputs: super::LoadedCompileOutputs) {
        let page_count = self.replace_rendered_pages(outputs.rendered_pages);
        self.reset_score_zoom_preview();
        self.logger.push(format!("Loaded {page_count} SVG page(s)"));

        self.replace_midi_files(outputs.midi_files);
        self.score_cursor_maps = outputs.score_cursor_maps;
        self.log_score_cursor_diagnostics(
            outputs.point_and_click_disabled,
            outputs.score_has_repeats,
        );
        self.refresh_playback_position();
    }

    pub(super) fn replace_rendered_pages(
        &mut self,
        pages: Vec<super::LoadedRenderedPage>,
    ) -> usize {
        let previous_page = self
            .rendered_score
            .as_ref()
            .map(|rendered_score| rendered_score.current_page)
            .unwrap_or(0);
        let page_count = pages.len();
        self.rendered_score = Some(RenderedScore {
            pages: pages.into_iter().map(rendered_page_from_loaded).collect(),
            current_page: previous_page.min(page_count.saturating_sub(1)),
        });
        page_count
    }

    pub(super) fn reset_score_zoom_preview(&mut self) {
        self.score_zoom_preview = None;
        self.score_zoom_preview_pending = None;
        self.score_zoom_last_interaction = None;
        self.score_zoom_persist_pending = false;
    }

    pub(super) fn replace_midi_files(&mut self, midi_files: Vec<midi::MidiRollFile>) {
        if midi_files.is_empty() {
            self.logger.push("No MIDI output found");
        } else {
            self.logger
                .push(format!("Loaded {} MIDI file(s)", midi_files.len()));
        }
        self.piano_roll.replace_files(midi_files);
        self.sync_playback_file();
    }

    pub(super) fn log_score_cursor_diagnostics(
        &mut self,
        point_and_click_disabled: bool,
        score_has_repeats: bool,
    ) {
        self.log_disabled_point_and_click(point_and_click_disabled);
        self.log_non_unfolded_repeat_cursor(score_has_repeats);
    }

    pub(super) fn log_disabled_point_and_click(&mut self, point_and_click_disabled: bool) {
        if self
            .score_cursor_maps
            .as_ref()
            .is_some_and(ScoreCursorMaps::is_empty)
            && point_and_click_disabled
        {
            self.logger
                .push("Score cursor unavailable because point-and-click is disabled in the score");
        }
    }

    pub(super) fn log_non_unfolded_repeat_cursor(&mut self, score_has_repeats: bool) {
        let Some(maps) = &self.score_cursor_maps else {
            return;
        };
        let Some(current_file) = self.piano_roll.current_file() else {
            return;
        };
        if !score_has_repeats {
            return;
        }
        let tick_tolerance = u64::from(current_file.data.ppq);
        if let Some(map_max_tick) = maps.max_tick_for_midi(&current_file.path)
            && current_file.data.total_ticks <= map_max_tick + tick_tolerance
        {
            self.logger.push(
                "Score has repeats but MIDI appears non-unfolded, cursor follows MIDI timeline \
                 only",
            );
        }
    }

    pub(super) fn handle_compile_outputs_error(&mut self, error: String) {
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
        self.reset_score_zoom_preview();
        self.show_prompt(
            ErrorPrompt::new(
                "Compile Output Error",
                error,
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(in crate::app) fn unload_current_score(&mut self) {
        self.current_score = None;
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.compile_outputs_loading = false;
        self.score_watcher = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
    }
}
