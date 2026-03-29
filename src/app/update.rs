use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

use iced::widget::{pane_grid, svg};
use notify::event::EventKind;

use super::score_cursor;
use super::*;
use crate::error_prompt::{ErrorFatality, ErrorPrompt, PromptButtons};
use crate::midi;

pub(super) fn update(app: &mut LilyView, message: Message) -> Task<Message> {
    match message {
        Message::StartupChecked(result) => app.handle_startup_checked(result),
        Message::Pane(message) => app.handle_pane_message(message),
        Message::File(message) => app.handle_file_message(message),
        Message::Viewer(message) => app.handle_viewer_message(message),
        Message::PianoRoll(message) => app.handle_piano_roll_message(message),
        Message::Logger(message) => app.handle_logger_message(message),
        Message::Prompt(message) => app.handle_prompt_message(message),
        Message::ModifiersChanged(modifiers) => app.handle_modifiers_changed(modifiers),
        Message::Tick => app.handle_tick(),
        Message::Frame(_now) => app.handle_frame(),
        Message::WindowResized(size) => app.handle_window_resized(size),
    }
}

impl LilyView {
    fn handle_viewer_message(&mut self, message: ViewerMessage) -> Task<Message> {
        match message {
            ViewerMessage::ScrollUp => {
                return iced::widget::operation::scroll_by(
                    SCORE_SCROLLABLE_ID,
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: -KEYBOARD_SCROLL_STEP,
                    },
                );
            }
            ViewerMessage::ScrollDown => {
                return iced::widget::operation::scroll_by(
                    SCORE_SCROLLABLE_ID,
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: KEYBOARD_SCROLL_STEP,
                    },
                );
            }
            ViewerMessage::ScrollPositionChanged { x, y } => {
                self.svg_scroll_x = x.max(0.0);
                self.svg_scroll_y = y.max(0.0);
            }
            ViewerMessage::ViewportCursorMoved(position) => {
                self.score_viewport_cursor = Some(position);
            }
            ViewerMessage::ViewportCursorLeft => {
                self.score_viewport_cursor = None;
            }
            ViewerMessage::PrevPage => {
                if let Some(rendered_score) = self.rendered_score.as_mut()
                    && rendered_score.current_page > 0
                {
                    rendered_score.current_page -= 1;
                }
            }
            ViewerMessage::NextPage => {
                if let Some(rendered_score) = self.rendered_score.as_mut()
                    && rendered_score.current_page + 1 < rendered_score.pages.len()
                {
                    rendered_score.current_page += 1;
                }
            }
            ViewerMessage::ZoomIn => {
                self.svg_zoom = next_zoom_step_up(self.svg_zoom, SVG_ZOOM_STEP, MAX_SVG_ZOOM);
            }
            ViewerMessage::ZoomOut => {
                self.svg_zoom = next_zoom_step_down(self.svg_zoom, SVG_ZOOM_STEP, MIN_SVG_ZOOM);
            }
            ViewerMessage::SmoothZoom(delta) => {
                let previous_zoom = self.svg_zoom;
                let next_zoom = smooth_zoom(self.svg_zoom, delta, MIN_SVG_ZOOM, MAX_SVG_ZOOM);

                if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
                    return Task::none();
                }

                self.svg_zoom = next_zoom;

                if let Some(cursor) = self.score_viewport_cursor {
                    let scale = next_zoom / previous_zoom.max(f32::EPSILON);
                    self.svg_scroll_x = anchored_scroll(self.svg_scroll_x, cursor.x, scale);
                    self.svg_scroll_y = anchored_scroll(self.svg_scroll_y, cursor.y, scale);

                    return self.restore_score_scroll();
                }
            }
            ViewerMessage::DecreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_sub(SVG_PAGE_BRIGHTNESS_STEP);
            }
            ViewerMessage::IncreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_add(SVG_PAGE_BRIGHTNESS_STEP)
                    .min(MAX_SVG_PAGE_BRIGHTNESS);
            }
            ViewerMessage::ResetZoom => {
                self.svg_zoom = DEFAULT_SVG_ZOOM;
            }
            ViewerMessage::ResetPageBrightness => {
                self.svg_page_brightness = DEFAULT_SVG_PAGE_BRIGHTNESS;
            }
        }

        Task::none()
    }

    fn handle_piano_roll_message(&mut self, message: PianoRollMessage) -> Task<Message> {
        let mut task = Task::none();

        match message {
            PianoRollMessage::Resized(event) => {
                if !self.piano_roll.visible {
                    self.piano_roll.visible = true;
                    task = self.restore_piano_roll_scroll();
                }
                let ratio = constrained_piano_ratio(self.score_area_height(), event.ratio);
                self.piano_ratio = ratio;
                self.piano_expanded_ratio = ratio;
                self.score_panes.resize(event.split, ratio);
            }
            PianoRollMessage::ToggleVisible => {
                if self.piano_roll.visible {
                    self.piano_roll.visible = false;
                    self.piano_expanded_ratio = self.piano_ratio;
                    self.piano_ratio = collapsed_piano_ratio(self.score_area_height());
                } else {
                    self.piano_roll.visible = true;
                    let ratio = constrained_piano_ratio(
                        self.score_area_height(),
                        self.piano_expanded_ratio,
                    );
                    self.piano_ratio = ratio;
                    self.piano_expanded_ratio = ratio;
                    task = self.restore_piano_roll_scroll();
                }

                self.score_panes.resize(self.score_split, self.piano_ratio);
            }
            PianoRollMessage::ViewportCursorMoved(position) => {
                self.piano_roll_viewport_cursor = Some(position);
            }
            PianoRollMessage::ViewportCursorLeft => {
                self.piano_roll_viewport_cursor = None;
            }
            PianoRollMessage::RollScrolled { x, y } => {
                self.piano_roll.set_horizontal_scroll(x);
                self.piano_roll.set_vertical_scroll(y);
            }
            PianoRollMessage::ZoomIn => {
                self.piano_roll.zoom_in();
            }
            PianoRollMessage::ZoomOut => {
                self.piano_roll.zoom_out();
            }
            PianoRollMessage::SmoothZoom(delta) => {
                let previous_zoom = self.piano_roll.zoom_x;
                let next_zoom = self.piano_roll.zoom_for_delta(delta);

                if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
                    return Task::none();
                }

                self.piano_roll.zoom_x = next_zoom;

                if let Some(cursor) = self.piano_roll_viewport_cursor {
                    let scale = next_zoom / previous_zoom.max(f32::EPSILON);
                    let anchored =
                        anchored_scroll(self.piano_roll.horizontal_scroll(), cursor.x, scale);
                    self.piano_roll.set_horizontal_scroll(anchored);
                    return self.restore_piano_roll_scroll();
                }
            }
            PianoRollMessage::ResetZoom => {
                self.piano_roll.reset_zoom();
            }
            PianoRollMessage::BeatSubdivisionSliderChanged(subdivision) => {
                self.piano_roll.set_beat_subdivision(subdivision);
            }
            PianoRollMessage::BeatSubdivisionInputChanged(input) => {
                self.piano_roll.set_beat_subdivision_input(input);
            }
            PianoRollMessage::FilePrevious => {
                self.piano_roll.select_previous_file();
                self.sync_playback_file();
            }
            PianoRollMessage::FileNext => {
                self.piano_roll.select_next_file();
                self.sync_playback_file();
            }
            PianoRollMessage::TrackPanelToggle => {
                self.piano_roll.toggle_track_panel();
            }
            PianoRollMessage::TrackPanelResizedBy(delta) => {
                self.piano_roll.resize_track_panel_by(delta);
            }
            PianoRollMessage::TrackMuteToggled(track_index) => {
                if let Some(muted) = self.piano_roll.toggle_track_mute(track_index)
                    && let Some(playback) = self.playback.as_mut()
                {
                    let _ = playback.set_track_muted(track_index, muted);
                }
            }
            PianoRollMessage::TrackSoloToggled(track_index) => {
                if let Some(soloed) = self.piano_roll.toggle_track_solo(track_index)
                    && let Some(playback) = self.playback.as_mut()
                {
                    let _ = playback.set_track_solo(track_index, soloed);
                }
            }
            PianoRollMessage::SetCursorTicks(tick) => {
                self.seek_playback_ticks(tick);
            }
            PianoRollMessage::TransportSeekNormalized(position) => {
                self.seek_playback_normalized(position);
            }
            PianoRollMessage::TransportPlayPause => {
                if let Some(playback) = self.playback.as_mut() {
                    if playback.is_playing() {
                        playback.pause();
                    } else {
                        let _ = playback.play();
                    }

                    self.refresh_playback_position();
                } else {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "Playback Error",
                            "No soundfont selected. Choose a SoundFont file first",
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                }
            }
            PianoRollMessage::TransportRewind => {
                if let Some(playback) = self.playback.as_mut() {
                    let was_playing = playback.is_playing();
                    playback.jump_to_tick(0);
                    if was_playing {
                        let _ = playback.play();
                    }
                    self.refresh_playback_position();
                } else {
                    self.seek_playback_ticks(0);
                }
            }
        }

        task
    }

    fn handle_startup_checked(
        &mut self,
        result: Result<lilypond::VersionCheck, String>,
    ) -> Task<Message> {
        match result {
            Ok(version_check) => {
                self.lilypond_status = LilypondStatus::Ready {
                    detected: version_check.detected,
                    min_required: version_check.min_required,
                };
                self.logger.push(format!(
                    "LilyPond ready: installed {}, minimum required {}",
                    version_check.detected, version_check.min_required
                ));
            }
            Err(error) => {
                self.lilypond_status = LilypondStatus::Unavailable;
                self.logger
                    .push(format!("LilyPond startup check failed: {error}"));
                self.show_prompt(
                    ErrorPrompt::new(
                        "LilyPond Startup Error",
                        error,
                        ErrorFatality::Critical,
                        PromptButtons::Ok,
                    ),
                    Some(PromptOkAction::ExitApp),
                );
            }
        }

        Task::none()
    }

    fn handle_pane_message(&mut self, message: PaneMessage) -> Task<Message> {
        match message {
            PaneMessage::Resized(event) => {
                let ratio = constrained_logger_ratio(self.window_height, event.ratio);
                self.logger_ratio = ratio;
                self.panes.resize(event.split, ratio);
                self.apply_piano_layout_constraints();
            }
            PaneMessage::ToggleLogger => {
                if let Some(logger_pane) = self.logger_pane.take() {
                    if let Some((_state, sibling)) = self.panes.close(logger_pane) {
                        self.main_pane = sibling;
                    }
                    self.logger_split = None;
                } else if let Some((pane, split)) = self.panes.split(
                    pane_grid::Axis::Horizontal,
                    self.main_pane,
                    PaneKind::Logger,
                ) {
                    let ratio = constrained_logger_ratio(self.window_height, self.logger_ratio);
                    self.panes.resize(split, ratio);
                    self.logger_pane = Some(pane);
                    self.logger_split = Some(split);
                    self.logger_ratio = ratio;
                } else {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "Logger Panel Error",
                            "Failed to open the logger panel",
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                }

                self.apply_piano_layout_constraints();
            }
        }

        Task::none()
    }

    fn handle_file_message(&mut self, message: FileMessage) -> Task<Message> {
        match message {
            FileMessage::RequestOpen => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("LilyPond score", &["ly", "ily"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |picked| Message::File(FileMessage::Picked(picked)),
            ),
            FileMessage::Picked(Some(path)) => {
                match selected_score_from_path(path) {
                    Ok(selected_score) => self.activate_score(selected_score),
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Open File Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                }
                Task::none()
            }
            FileMessage::Picked(None) => Task::none(),
            FileMessage::RequestSoundfont => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("SoundFont", &["sf2", "sf3"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |picked| Message::File(FileMessage::SoundfontPicked(picked)),
            ),
            FileMessage::SoundfontPicked(Some(path)) => {
                self.logger
                    .push(format!("Selected soundfont {}", path.display()));
                self.soundfont_status = SoundfontStatus::Ready(path.clone());
                self.initialize_playback(path);
                Task::none()
            }
            FileMessage::SoundfontPicked(None) => Task::none(),
        }
    }

    fn handle_logger_message(&mut self, message: LoggerMessage) -> Task<Message> {
        match message {
            LoggerMessage::RequestClear => {
                if !self.logger.is_empty() {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "Clear Logger",
                            "Do you want to clear all log messages?",
                            ErrorFatality::Recoverable,
                            PromptButtons::OkCancel,
                        ),
                        Some(PromptOkAction::ClearLogs),
                    );
                }
                Task::none()
            }
            LoggerMessage::TextAction(action) => {
                self.logger.handle_editor_action(action);
                Task::none()
            }
        }
    }

    fn handle_prompt_message(&mut self, message: PromptMessage) -> Task<Message> {
        match message {
            PromptMessage::Acknowledge => {
                if self.error_prompt.take().is_some() {
                    match self.prompt_ok_action.take() {
                        Some(PromptOkAction::ExitApp) => iced::exit(),
                        Some(PromptOkAction::ClearLogs) => {
                            self.logger.clear();
                            Task::none()
                        }
                        None => Task::none(),
                    }
                } else {
                    Task::none()
                }
            }
            PromptMessage::Cancel => {
                self.error_prompt = None;
                self.prompt_ok_action = None;
                Task::none()
            }
        }
    }

    fn handle_tick(&mut self) -> Task<Message> {
        self.spinner_step = self.spinner_step.wrapping_add(1);
        self.poll_score_watcher();
        self.poll_compile_logs();
        self.start_compile_if_queued();
        self.apply_initial_piano_roll_center_if_needed()
    }

    fn handle_frame(&mut self) -> Task<Message> {
        self.refresh_playback_position();
        Task::none()
    }

    fn activate_score(&mut self, selected_score: SelectedScore) {
        let watched_path = selected_score.path.clone();
        self.logger.push(format!(
            "Opened score file {}",
            selected_score.path.display()
        ));
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
        self.svg_zoom = DEFAULT_SVG_ZOOM;
        self.svg_page_brightness = DEFAULT_SVG_PAGE_BRIGHTNESS;
        self.current_score = Some(selected_score);
        self.restart_score_watcher(&watched_path);
        self.queue_compile("Score loaded, compiling SVG and MIDI");
        self.start_compile_if_queued();
    }

    fn restart_score_watcher(&mut self, path: &Path) {
        match crate::score_watcher::ScoreWatcher::start(path) {
            Ok(watcher) => {
                self.logger.push(format!("Watching {}", path.display()));
                self.score_watcher = Some(watcher);
            }
            Err(error) => {
                self.score_watcher = None;
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Watcher Error",
                        format!("Failed to watch score file changes: {error}"),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    fn poll_score_watcher(&mut self) {
        let Some(score_watcher) = &self.score_watcher else {
            return;
        };

        let watched_path = score_watcher.watched_path().to_path_buf();
        let mut should_recompile = false;
        let mut disconnected = false;

        loop {
            match score_watcher.try_recv() {
                Ok(Ok(event)) => {
                    if is_relevant_score_change(&event, &watched_path) {
                        should_recompile = true;
                    }
                }
                Ok(Err(error)) => {
                    self.logger.push(format!("[watcher:error] {error}"));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if disconnected {
            self.score_watcher = None;
            self.show_prompt(
                ErrorPrompt::new(
                    "File Watcher Error",
                    "Score file watcher disconnected",
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
        }

        if should_recompile {
            self.queue_compile("Score changed, recompiling");
        }
    }

    fn queue_compile(&mut self, message: &str) {
        if !self.compile_requested {
            self.logger.push(message.to_string());
        }
        self.compile_requested = true;
    }

    fn start_compile_if_queued(&mut self) {
        if !self.compile_requested || self.compile_session.is_some() {
            return;
        }

        let Some(selected_score) = self
            .current_score
            .as_ref()
            .map(|score| (score.path.clone(), score.file_name.clone()))
        else {
            self.compile_requested = false;
            return;
        };

        let output_prefix = match self.compile_output_prefix(&selected_score.1) {
            Ok(path) => path,
            Err(error) => {
                self.compile_requested = false;
                self.show_prompt(
                    ErrorPrompt::new(
                        "Build Directory Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                return;
            }
        };

        let mut request = lilypond::CompileRequest::new(selected_score.0.clone());
        request.args = vec![
            "--svg".to_string(),
            "-dmidi-extension=midi".to_string(),
            "-dinclude-settings=event-listener.ly".to_string(),
            "-dpoint-and-click=note-event".to_string(),
            "-o".to_string(),
            output_prefix.to_string_lossy().to_string(),
        ];
        request.working_dir = selected_score.0.parent().map(std::path::Path::to_path_buf);

        self.logger.push("Starting LilyPond compile".to_string());

        match lilypond::spawn_compile(request) {
            Ok(session) => {
                self.compile_session = Some(session);
                self.compile_requested = false;
            }
            Err(error) => {
                self.compile_requested = false;
                self.show_prompt(
                    ErrorPrompt::new(
                        "LilyPond Compile Error",
                        error.to_string(),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    fn compile_output_prefix(
        &mut self,
        selected_file_name: &str,
    ) -> Result<std::path::PathBuf, String> {
        if self.build_dir.is_none() {
            let build_dir = tempfile::Builder::new()
                .prefix("lily-view-build-")
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

    fn reload_rendered_score(&mut self) {
        let previous_page = self
            .rendered_score
            .as_ref()
            .map(|rendered_score| rendered_score.current_page)
            .unwrap_or(0);

        let Some(selected_score) = &self.current_score else {
            self.rendered_score = None;
            return;
        };

        match self.collect_rendered_pages(&selected_score.file_name) {
            Ok(pages) => {
                if pages.is_empty() {
                    self.rendered_score = None;
                    self.show_prompt(
                        ErrorPrompt::new(
                            "SVG Output Error",
                            "LilyPond finished without SVG output",
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                    return;
                }

                let page_count = pages.len();
                let current_page = previous_page.min(page_count.saturating_sub(1));
                self.rendered_score = Some(RenderedScore {
                    pages,
                    current_page,
                });
                self.logger.push(format!("Loaded {page_count} SVG page(s)"));
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "SVG Output Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    fn collect_rendered_pages(
        &self,
        selected_file_name: &str,
    ) -> Result<Vec<RenderedPage>, String> {
        let build_dir = self
            .build_dir
            .as_ref()
            .ok_or_else(|| "Temporary build directory is not available".to_string())?;
        let score_stem = selected_score_stem(selected_file_name)?;
        let entries = fs::read_dir(build_dir.path()).map_err(|error| {
            format!(
                "Failed to read build directory {}: {error}",
                build_dir.path().display()
            )
        })?;

        let mut pages = Vec::new();

        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read build artifact entry: {error}"))?;
            let path = entry.path();

            if !is_svg_file(&path) {
                continue;
            }

            let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(page_index) = svg_page_index(file_stem, score_stem) else {
                continue;
            };

            let page_size = read_svg_size(&path).unwrap_or(SvgSize {
                width: 1200.0,
                height: 1700.0,
            });

            pages.push((page_index, path, page_size));
        }

        pages.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut rendered_pages = Vec::with_capacity(pages.len());

        for (index, path, size) in pages {
            let bytes = fs::read(&path)
                .map_err(|error| format!("Failed to read SVG {}: {error}", path.display()))?;
            let source = String::from_utf8_lossy(&bytes);
            let page_index = index.saturating_sub(1) as usize;
            let note_anchors = score_cursor::parse_svg_note_anchors(&source, page_index);
            let system_bands = score_cursor::parse_svg_system_bands(&source);

            rendered_pages.push(RenderedPage {
                handle: svg::Handle::from_memory(bytes),
                size,
                note_anchors,
                system_bands,
            });
        }

        Ok(rendered_pages)
    }

    fn handle_window_resized(&mut self, size: Size) -> Task<Message> {
        self.window_height = size.height.max(1.0);

        if let Some(split) = self.logger_split {
            let ratio = constrained_logger_ratio(self.window_height, self.logger_ratio);
            self.logger_ratio = ratio;
            self.panes.resize(split, ratio);
        }

        self.apply_piano_layout_constraints();

        Task::none()
    }

    fn handle_modifiers_changed(&mut self, modifiers: iced::keyboard::Modifiers) -> Task<Message> {
        self.keyboard_modifiers = modifiers;
        Task::none()
    }

    fn show_prompt(&mut self, prompt: ErrorPrompt, ok_action: Option<PromptOkAction>) {
        self.error_prompt = Some(prompt);
        self.prompt_ok_action = ok_action;
    }

    fn score_area_height(&self) -> f32 {
        estimated_score_area_height(
            self.window_height,
            self.logger_pane.is_some(),
            self.logger_ratio,
        )
    }

    fn apply_piano_layout_constraints(&mut self) {
        if self.piano_roll.visible {
            let ratio = constrained_piano_ratio(self.score_area_height(), self.piano_ratio);
            self.piano_ratio = ratio;
            self.piano_expanded_ratio = ratio;
        } else {
            self.piano_ratio = collapsed_piano_ratio(self.score_area_height());
        }

        self.score_panes.resize(self.score_split, self.piano_ratio);
    }

    fn restore_piano_roll_scroll(&self) -> Task<Message> {
        iced::widget::operation::scroll_to(
            super::piano_roll::roll_scroll_id(),
            iced::widget::operation::AbsoluteOffset {
                x: Some(self.piano_roll.horizontal_scroll()),
                y: Some(self.piano_roll.vertical_scroll()),
            },
        )
    }

    fn restore_score_scroll(&self) -> Task<Message> {
        iced::widget::operation::scroll_to(
            super::SCORE_SCROLLABLE_ID,
            iced::widget::operation::AbsoluteOffset {
                x: Some(self.svg_scroll_x),
                y: Some(self.svg_scroll_y),
            },
        )
    }

    fn apply_initial_piano_roll_center_if_needed(&mut self) -> Task<Message> {
        if !self.piano_roll.visible
            || !self.piano_roll.pending_initial_center()
            || self.piano_roll.current_file().is_none()
        {
            return Task::none();
        }

        self.piano_roll.mark_initial_center_applied();
        iced::widget::operation::snap_to(
            super::piano_roll::roll_scroll_id(),
            iced::widget::operation::RelativeOffset {
                x: None,
                y: Some(0.5),
            },
        )
    }

    fn poll_compile_logs(&mut self) {
        let Some(session) = self.compile_session.take() else {
            return;
        };

        let mut keep_session = true;

        loop {
            match session.try_recv() {
                Ok(event) => match event {
                    lilypond::CompileEvent::Log { stream, line } => {
                        let prefix = match stream {
                            lilypond::LogStream::Stdout => "lilypond:stdout",
                            lilypond::LogStream::Stderr => "lilypond:stderr",
                        };
                        self.logger.push(format!("[{prefix}] {line}"));
                    }
                    lilypond::CompileEvent::ProcessError(message) => {
                        self.logger
                            .push(format!("[lilypond:process-error] {message}"));
                    }
                    lilypond::CompileEvent::Finished { success, exit_code } => {
                        if success {
                            self.logger.push(format!(
                                "LilyPond compile finished successfully (exit code {})",
                                exit_code.unwrap_or(0)
                            ));
                            self.reload_rendered_score();
                            self.reload_piano_roll();
                            self.reload_score_cursor_maps();
                            self.refresh_playback_position();
                        } else {
                            self.logger.push(format!(
                                "LilyPond compile failed (exit code {:?})",
                                exit_code
                            ));
                        }
                        keep_session = false;
                        break;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    keep_session = false;
                    break;
                }
            }
        }

        if keep_session {
            self.compile_session = Some(session);
        }
    }

    fn reload_piano_roll(&mut self) {
        let Some(selected_score) = &self.current_score else {
            self.piano_roll.clear_files();
            self.unload_playback_file();
            self.score_cursor_maps = None;
            self.score_cursor_overlay = None;
            return;
        };
        let Some(build_dir) = &self.build_dir else {
            self.piano_roll.clear_files();
            self.unload_playback_file();
            self.score_cursor_maps = None;
            self.score_cursor_overlay = None;
            return;
        };

        let score_stem = match selected_score_stem(&selected_score.file_name) {
            Ok(score_stem) => score_stem,
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "MIDI Output Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                self.piano_roll.clear_files();
                self.unload_playback_file();
                self.score_cursor_maps = None;
                self.score_cursor_overlay = None;
                return;
            }
        };

        match midi::collect_midi_roll_files(build_dir.path(), score_stem) {
            Ok(files) => {
                if files.is_empty() {
                    self.logger.push("No MIDI output found");
                } else {
                    self.logger
                        .push(format!("Loaded {} MIDI file(s)", files.len()));
                }
                self.piano_roll.replace_files(files);
                self.sync_playback_file();
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "MIDI Output Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                self.piano_roll.clear_files();
                self.unload_playback_file();
                self.score_cursor_maps = None;
                self.score_cursor_overlay = None;
            }
        }
    }

    fn reload_score_cursor_maps(&mut self) {
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;

        let Some(rendered_score) = &self.rendered_score else {
            return;
        };
        let Some(build_dir) = &self.build_dir else {
            return;
        };
        let Some(selected_score) = &self.current_score else {
            return;
        };

        let score_stem = match selected_score_stem(&selected_score.file_name) {
            Ok(score_stem) => score_stem,
            Err(_error) => return,
        };

        let note_anchors: Vec<_> = rendered_score
            .pages
            .iter()
            .flat_map(|page| page.note_anchors.iter().copied())
            .collect();
        if note_anchors.is_empty() {
            let point_and_click_disabled =
                score_cursor::score_disables_point_and_click(&selected_score.path)
                    .unwrap_or_default();

            if point_and_click_disabled {
                self.logger.push(
                    "Score cursor unavailable because point-and-click is disabled in the score",
                );
            }
            return;
        }

        match score_cursor::build_score_cursor_maps(
            build_dir.path(),
            score_stem,
            &note_anchors,
            &self.piano_roll.files,
        ) {
            Ok(maps) => {
                if maps.is_empty() {
                    return;
                }

                if let Some(current_file) = self.piano_roll.current_file() {
                    let score_has_repeats =
                        score_cursor::score_contains_repeats(&selected_score.path)
                            .unwrap_or_default();
                    if score_has_repeats {
                        let tick_tolerance = u64::from(current_file.data.ppq);
                        if let Some(map_max_tick) = maps.max_tick_for_midi(&current_file.path)
                            && current_file.data.total_ticks <= map_max_tick + tick_tolerance
                        {
                            self.logger.push(
                                "Score has repeats but MIDI appears non-unfolded, cursor follows MIDI timeline only",
                            );
                        }
                    }
                }

                self.score_cursor_maps = Some(maps);
            }
            Err(_error) => {}
        }
    }

    fn refresh_score_cursor_overlay(&mut self) {
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

    fn initialize_playback(&mut self, soundfont_path: PathBuf) {
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

    fn unload_playback_file(&mut self) {
        if let Some(playback) = self.playback.as_mut() {
            if playback.is_playing() {
                playback.pause();
            }
            playback.jump_to_tick(0);
        }

        self.refresh_playback_position();
    }

    fn sync_playback_file(&mut self) {
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

    fn sync_playback_track_mix(&mut self) {
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

    fn seek_playback_normalized(&mut self, position: f32) {
        let total_ticks = self
            .playback
            .as_ref()
            .map(crate::playback::MidiPlayback::total_ticks)
            .unwrap_or_else(|| self.current_midi_total_ticks());
        let normalized = position.clamp(0.0, 1.0);
        let tick = (total_ticks as f32 * normalized).round() as u64;

        self.seek_playback_ticks(tick);
    }

    fn seek_playback_ticks(&mut self, tick: u64) {
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

    fn refresh_playback_position(&mut self) {
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

    fn current_midi_file_path(&self) -> Option<PathBuf> {
        self.piano_roll.current_file().map(|file| file.path.clone())
    }

    fn current_midi_total_ticks(&self) -> u64 {
        self.piano_roll
            .current_file()
            .map(|file| file.data.total_ticks)
            .unwrap_or(0)
    }
}

fn snap_zoom_to_step(value: f32, step: f32) -> f32 {
    if step <= f32::EPSILON {
        return value;
    }

    (value / step).round() * step
}

fn next_zoom_step_up(current: f32, step: f32, max_zoom: f32) -> f32 {
    let snapped = snap_zoom_to_step(current, step);

    if (current - snapped).abs() <= 1e-4 {
        (snapped + step).clamp(MIN_SVG_ZOOM, max_zoom)
    } else if current < snapped {
        snapped.clamp(MIN_SVG_ZOOM, max_zoom)
    } else {
        (snapped + step).clamp(MIN_SVG_ZOOM, max_zoom)
    }
}

fn next_zoom_step_down(current: f32, step: f32, min_zoom: f32) -> f32 {
    let snapped = snap_zoom_to_step(current, step);

    if (current - snapped).abs() <= 1e-4 {
        (snapped - step).clamp(min_zoom, MAX_SVG_ZOOM)
    } else if current > snapped {
        snapped.clamp(min_zoom, MAX_SVG_ZOOM)
    } else {
        (snapped - step).clamp(min_zoom, MAX_SVG_ZOOM)
    }
}

fn smooth_zoom(
    current_zoom: f32,
    delta: iced::mouse::ScrollDelta,
    min_zoom: f32,
    max_zoom: f32,
) -> f32 {
    let intensity = match delta {
        iced::mouse::ScrollDelta::Lines { y, .. } => y * 0.14,
        iced::mouse::ScrollDelta::Pixels { y, .. } => y * 0.0035,
    };

    (current_zoom * intensity.exp()).clamp(min_zoom, max_zoom)
}

fn anchored_scroll(current_scroll: f32, cursor_in_viewport: f32, scale: f32) -> f32 {
    ((current_scroll + cursor_in_viewport) * scale - cursor_in_viewport).max(0.0)
}

fn is_relevant_score_change(event: &notify::Event, watched_path: &Path) -> bool {
    let kind_matches = matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );

    if !kind_matches {
        return false;
    }

    event.paths.is_empty() || event.paths.iter().any(|path| path == watched_path)
}

fn is_svg_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn svg_page_index(file_stem: &str, score_stem: &str) -> Option<u32> {
    if file_stem == score_stem {
        return Some(1);
    }

    let suffix = file_stem.strip_prefix(score_stem)?.strip_prefix('-')?;

    if let Some(page_suffix) = suffix.strip_prefix("page") {
        return page_suffix.parse::<u32>().ok();
    }

    suffix.parse::<u32>().ok()
}

fn read_svg_size(path: &Path) -> Option<SvgSize> {
    let source = fs::read_to_string(path).ok()?;

    if let Some(view_box) = svg_attribute_value(&source, "viewBox") {
        let numbers: Vec<f32> = view_box
            .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
            .filter(|value| !value.is_empty())
            .filter_map(|value| value.parse::<f32>().ok())
            .collect();

        if numbers.len() >= 4 {
            let width = numbers[2].abs();
            let height = numbers[3].abs();

            if width > 0.0 && height > 0.0 {
                return Some(SvgSize { width, height });
            }
        }
    }

    let width = svg_attribute_value(&source, "width").and_then(parse_svg_dimension);
    let height = svg_attribute_value(&source, "height").and_then(parse_svg_dimension);

    match (width, height) {
        (Some(width), Some(height)) if width > 0.0 && height > 0.0 => {
            Some(SvgSize { width, height })
        }
        _ => None,
    }
}

fn svg_attribute_value<'a>(source: &'a str, attribute_name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{attribute_name}={quote}");
        let Some(start) = source.find(&needle) else {
            continue;
        };
        let value_start = start + needle.len();
        let tail = &source[value_start..];
        let Some(value_end) = tail.find(quote) else {
            continue;
        };

        return Some(&tail[..value_end]);
    }

    None
}

fn parse_svg_dimension(raw_value: &str) -> Option<f32> {
    let numeric_prefix: String = raw_value
        .trim()
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
        .collect();

    if numeric_prefix.is_empty() {
        return None;
    }

    numeric_prefix.parse::<f32>().ok()
}

fn closest_system_band(
    bands: &[score_cursor::SystemBand],
    x: f32,
    min_y: f32,
    max_y: f32,
) -> Option<score_cursor::SystemBand> {
    let target_y = (min_y + max_y) * 0.5;
    bands
        .iter()
        .copied()
        .filter(|band| x >= band.x_start && x <= band.x_end)
        .min_by(|left, right| {
            let left_center = (left.min_y + left.max_y) * 0.5;
            let right_center = (right.min_y + right.max_y) * 0.5;
            let left_distance = (target_y - left_center).abs();
            let right_distance = (target_y - right_center).abs();
            left_distance.total_cmp(&right_distance)
        })
}
