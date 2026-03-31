use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

use iced::widget::{pane_grid, svg};
use iced_core::{Bytes, image};
use notify::event::EventKind;
use resvg::tiny_skia;
use resvg::usvg;

use super::score_cursor;
use super::*;
use crate::error_prompt::{ErrorFatality, ErrorPrompt, PromptButtons};
use crate::midi;
use crate::settings::{self, DockGroupSettings, DockNodeSettings, WorkspaceLayoutSettings};

const DRAG_START_THRESHOLD: f32 = 8.0;
const SCORE_PREVIEW_FALLBACK_MAX_DIMENSION: f32 = 2200.0;
const SCORE_PREVIEW_PRIMARY_MAX_DIMENSION: f32 = 3600.0;
const SCORE_PREVIEW_FALLBACK_MIN_ZOOM: f32 = 1.0;
const SCORE_PREVIEW_PRIMARY_MIN_ZOOM: f32 = 1.8;

pub(super) fn update(app: &mut LilyView, message: Message) -> Task<Message> {
    match message {
        Message::StartupChecked(result) => app.handle_startup_checked(result),
        Message::Pane(message) => app.handle_pane_message(message),
        Message::File(message) => app.handle_file_message(message),
        Message::Viewer(message) => app.handle_viewer_message(message),
        Message::ScorePreviewReady(result) => app.handle_score_preview_ready(result),
        Message::PianoRoll(message) => app.handle_piano_roll_message(message),
        Message::Editor(message) => app.handle_editor_message(message),
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
                    self.score_zoom_preview = None;
                    self.score_zoom_preview_pending = None;

                    if let Some(task) = self.request_score_zoom_preview(self.svg_zoom) {
                        return task;
                    }
                }
            }
            ViewerMessage::NextPage => {
                if let Some(rendered_score) = self.rendered_score.as_mut()
                    && rendered_score.current_page + 1 < rendered_score.pages.len()
                {
                    rendered_score.current_page += 1;
                    self.score_zoom_preview = None;
                    self.score_zoom_preview_pending = None;

                    if let Some(task) = self.request_score_zoom_preview(self.svg_zoom) {
                        return task;
                    }
                }
            }
            ViewerMessage::ZoomIn => {
                self.svg_zoom = next_zoom_step_up(self.svg_zoom, SVG_ZOOM_STEP, MAX_SVG_ZOOM);
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::ZoomOut => {
                self.svg_zoom = next_zoom_step_down(self.svg_zoom, SVG_ZOOM_STEP, MIN_SVG_ZOOM);
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::SmoothZoom(delta) => {
                let previous_zoom = self.svg_zoom;
                let next_zoom = smooth_zoom(self.svg_zoom, delta, MIN_SVG_ZOOM, MAX_SVG_ZOOM);

                if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
                    return Task::none();
                }

                self.svg_zoom = next_zoom;
                self.score_zoom_last_interaction = Some(std::time::Instant::now());
                self.score_zoom_persist_pending = true;

                if let Some(cursor) = self.score_viewport_cursor {
                    let scale = next_zoom / previous_zoom.max(f32::EPSILON);
                    self.svg_scroll_x = anchored_scroll(self.svg_scroll_x, cursor.x, scale);
                    self.svg_scroll_y = anchored_scroll(self.svg_scroll_y, cursor.y, scale);
                    let mut tasks = vec![self.restore_score_scroll()];
                    if let Some(task) = self.request_score_zoom_preview(next_zoom) {
                        tasks.push(task);
                    }
                    return Task::batch(tasks);
                }

                if let Some(task) = self.request_score_zoom_preview(next_zoom) {
                    return task;
                }
            }
            ViewerMessage::DecreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_sub(SVG_PAGE_BRIGHTNESS_STEP);
                self.persist_settings();
            }
            ViewerMessage::IncreasePageBrightness => {
                self.svg_page_brightness = self
                    .svg_page_brightness
                    .saturating_add(SVG_PAGE_BRIGHTNESS_STEP)
                    .min(MAX_SVG_PAGE_BRIGHTNESS);
                self.persist_settings();
            }
            ViewerMessage::ResetZoom => {
                self.svg_zoom = self.default_settings.score_view.zoom;
                self.score_zoom_persist_pending = false;
                self.persist_settings();
            }
            ViewerMessage::ResetPageBrightness => {
                self.svg_page_brightness = self.default_settings.score_view.page_brightness;
                self.persist_settings();
            }
        }

        Task::none()
    }

    fn handle_score_preview_ready(
        &mut self,
        result: Result<super::messages::ScorePreviewReady, String>,
    ) -> Task<Message> {
        let Some(pending) = self.score_zoom_preview_pending else {
            return Task::none();
        };

        self.score_zoom_preview_pending = None;

        match result {
            Ok(preview)
                if preview.page_index == pending.page_index
                    && (preview.zoom - pending.zoom).abs() <= 1e-4
                    && preview.tier == pending.tier =>
            {
                self.score_zoom_preview = Some(ScoreZoomPreview {
                    page_index: preview.page_index,
                    tier: preview.tier,
                    handle: preview.handle,
                });

                if preview.tier == ScoreZoomPreviewTier::Fallback
                    && let Some(task) = self.request_score_zoom_preview(self.svg_zoom)
                {
                    return task;
                }
            }
            Ok(_) => {}
            Err(error) => {
                self.logger.push(format!("[score-preview] {error}"));
            }
        }

        Task::none()
    }

    fn handle_piano_roll_message(&mut self, message: PianoRollMessage) -> Task<Message> {
        let mut task = Task::none();

        match message {
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
                self.persist_settings();
            }
            PianoRollMessage::ZoomOut => {
                self.piano_roll.zoom_out();
                self.persist_settings();
            }
            PianoRollMessage::SmoothZoom(delta) => {
                let previous_zoom = self.piano_roll.zoom_x;
                let next_zoom = self.piano_roll.zoom_for_delta(delta);

                if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
                    return Task::none();
                }

                self.piano_roll.zoom_x = next_zoom;
                self.persist_settings();

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
                self.persist_settings();
            }
            PianoRollMessage::BeatSubdivisionSliderChanged(subdivision) => {
                self.piano_roll.set_beat_subdivision(subdivision);
                self.persist_settings();
            }
            PianoRollMessage::BeatSubdivisionInputChanged(input) => {
                self.piano_roll.set_beat_subdivision_input(input);
                self.persist_settings();
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
                task = self.restore_piano_roll_scroll();
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
            PianoRollMessage::SetRewindFlagTicks(tick) => {
                self.piano_roll.set_rewind_flag_tick(tick);
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
                let target_tick = self.rewind_target_tick();

                if let Some(playback) = self.playback.as_mut() {
                    let was_playing = playback.is_playing();
                    playback.jump_to_tick(target_tick);
                    if was_playing {
                        let _ = playback.play();
                    }
                    self.refresh_playback_position();
                } else {
                    self.seek_playback_ticks(target_tick);
                }
            }
        }

        task
    }

    fn handle_editor_message(&mut self, message: EditorMessage) -> Task<Message> {
        match message {
            EditorMessage::Widget(message) => self
                .editor
                .update(&message)
                .map(|message| Message::Editor(EditorMessage::Widget(message))),
            EditorMessage::SaveRequested => {
                if !self.editor.has_document() {
                    return Task::none();
                }

                match self.editor.save_to_disk() {
                    Ok(path) => {
                        self.logger.push(format!("Saved {}", path.display()));
                        self.queue_compile("Editor saved, recompiling");
                        self.start_compile_if_queued();
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Editor Save Error",
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
            EditorMessage::ReloadRequested => {
                if !self.editor.has_document() {
                    return Task::none();
                }

                match self.editor.reload_from_disk() {
                    Ok(()) => {
                        if let Some(file_name) = self.editor.file_name() {
                            self.logger.push(format!("Reloaded {file_name}"));
                        } else {
                            self.logger.push("Reloaded editor file");
                        }
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Editor Reload Error",
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
            EditorMessage::ToggleThemeMenu(group_id) => {
                self.open_header_overflow_menu = None;
                self.open_editor_theme_menu = if self.open_editor_theme_menu == Some(group_id) {
                    None
                } else {
                    Some(group_id)
                };

                Task::none()
            }
            EditorMessage::CloseThemeMenu => {
                self.open_editor_theme_menu = None;
                Task::none()
            }
            EditorMessage::SetThemeHueOffsetDegrees(value) => {
                self.editor.set_hue_offset_degrees(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeSaturation(value) => {
                self.editor.set_saturation(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeWarmth(value) => {
                self.editor.set_warmth(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeBrightness(value) => {
                self.editor.set_brightness(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeTextDim(value) => {
                self.editor.set_text_dim(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeCommentDim(value) => {
                self.editor.set_comment_dim(value);
                self.persist_settings();
                Task::none()
            }
        }
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
            PaneMessage::WorkspaceResized(event) => {
                let ratio = self.constrained_workspace_split_ratio(event.split, event.ratio);
                self.workspace_panes.resize(event.split, ratio);
                self.open_header_overflow_menu = None;
                self.open_editor_theme_menu = None;
                self.sync_dock_layout_from_workspace_state();
                self.persist_settings();
            }
            PaneMessage::WorkspaceTabPressed(kind) => {
                self.set_active_workspace_pane(kind);
                self.open_header_overflow_menu = None;
                self.open_editor_theme_menu = None;
                self.pressed_workspace_pane = Some(kind);
                self.workspace_drag_origin = None;
                self.dock_drop_target = None;
                self.persist_settings();
                return self.restore_runtime_view_state(kind);
            }
            PaneMessage::WorkspaceTabHovered(kind) => {
                self.hovered_workspace_pane = kind;
            }
            PaneMessage::OpenHeaderOverflowMenu(group_id) => {
                self.open_editor_theme_menu = None;
                self.open_header_overflow_menu = Some(group_id);
            }
            PaneMessage::CloseHeaderOverflowMenu => {
                self.open_header_overflow_menu = None;
            }
            PaneMessage::ToggleWorkspacePane(pane) => {
                self.open_header_overflow_menu = None;
                self.open_editor_theme_menu = None;
                let changed = if self.is_pane_folded(pane) {
                    self.unfold_workspace_pane(pane)
                } else {
                    self.fold_workspace_pane(pane)
                };
                if changed {
                    self.persist_settings();
                    return self.restore_runtime_view_state(pane);
                }
            }
            PaneMessage::WorkspaceDragMoved(position) => {
                if self.dragged_workspace_pane.is_none()
                    && let Some(pressed_pane) = self.pressed_workspace_pane
                {
                    match self.workspace_drag_origin {
                        Some(origin) if drag_distance(origin, position) >= DRAG_START_THRESHOLD => {
                            self.dragged_workspace_pane = Some(pressed_pane);
                            self.dock_drop_target =
                                self.group_for_pane(pressed_pane)
                                    .map(|group_id| DockDropTarget {
                                        group_id,
                                        region: DockDropRegion::Center,
                                    });
                        }
                        Some(_) => {}
                        None => {
                            self.workspace_drag_origin = Some(position);
                        }
                    }
                }

                if self.dragged_workspace_pane.is_some() {
                    self.dock_drop_target = self.dock_drop_target_for(position);
                }
            }
            PaneMessage::WorkspaceDragReleased => {
                self.pressed_workspace_pane = None;

                if let Some(dragged_pane) = self.dragged_workspace_pane
                    && let Some(target) = self.dock_drop_target
                {
                    self.apply_dock_drop(dragged_pane, target);
                    self.persist_settings();
                    self.clear_workspace_drag_state();
                    self.open_editor_theme_menu = None;
                    return self.restore_runtime_view_state(dragged_pane);
                }

                self.clear_workspace_drag_state();
            }
            PaneMessage::WorkspaceDragExited => {
                if self.dragged_workspace_pane.is_some() {
                    self.dock_drop_target = None;
                }
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
        let mut tasks = Vec::new();

        if self.editor.has_document() {
            tasks.push(
                self.editor
                    .update(&iced_code_editor::Message::Tick)
                    .map(|message| Message::Editor(EditorMessage::Widget(message))),
            );
        }

        if self.compile_session.is_some() || self.score_watcher.is_some() {
            self.spinner_step = self.spinner_step.wrapping_add(1);
            self.poll_score_watcher();
            self.poll_compile_logs();
            self.start_compile_if_queued();
            tasks.push(self.apply_initial_piano_roll_center_if_needed());
        }

        if self
            .score_zoom_last_interaction
            .is_some_and(|instant| instant.elapsed() >= SCORE_ZOOM_PREVIEW_SETTLE_DELAY)
        {
            self.score_zoom_last_interaction = None;
        }

        if self.score_zoom_last_interaction.is_none() && self.score_zoom_persist_pending {
            self.score_zoom_persist_pending = false;
            self.persist_settings();
        }

        Task::batch(tasks)
    }

    fn handle_frame(&mut self) -> Task<Message> {
        self.refresh_playback_position();
        Task::none()
    }

    fn activate_score(&mut self, selected_score: SelectedScore) {
        let watched_path = selected_score.path.clone();
        if let Err(error) = self.editor.load_file(&watched_path) {
            self.show_prompt(
                ErrorPrompt::new(
                    "Editor Load Error",
                    error,
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
        }
        self.logger.push(format!(
            "Opened score file {}",
            selected_score.path.display()
        ));
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
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
            self.score_zoom_preview = None;
            self.score_zoom_preview_pending = None;
            self.score_zoom_last_interaction = None;
            self.score_zoom_persist_pending = false;
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
                self.score_zoom_preview = None;
                self.score_zoom_preview_pending = None;
                self.score_zoom_last_interaction = None;
                self.score_zoom_persist_pending = false;
                self.logger.push(format!("Loaded {page_count} SVG page(s)"));
            }
            Err(error) => {
                self.score_zoom_preview = None;
                self.score_zoom_preview_pending = None;
                self.score_zoom_last_interaction = None;
                self.score_zoom_persist_pending = false;
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
            let svg_bytes = Bytes::from(bytes.clone());

            rendered_pages.push(RenderedPage {
                handle: svg::Handle::from_memory(bytes),
                svg_bytes,
                size,
                note_anchors,
                system_bands,
            });
        }

        Ok(rendered_pages)
    }

    fn handle_window_resized(&mut self, size: Size) -> Task<Message> {
        self.window_width = size.width.max(1.0);
        self.window_height = size.height.max(1.0);

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

    fn rebuild_workspace_panes(&mut self) {
        self.workspace_panes = build_workspace_panes(self.dock_layout.as_ref());
    }

    fn sync_dock_layout_from_workspace_state(&mut self) {
        if self.dock_groups.is_empty() {
            self.dock_layout = None;
        } else if let Some(layout) = dock_node_from_workspace_state(&self.workspace_panes) {
            self.dock_layout = Some(layout);
        }
    }

    fn constrained_workspace_split_ratio(&self, split: pane_grid::Split, ratio: f32) -> f32 {
        let split_regions =
            self.workspace_panes
                .layout()
                .split_regions(0.0, 0.0, self.workspace_area_size());
        let Some((axis, region, _)) = split_regions.get(&split).copied() else {
            return ratio.clamp(0.05, 0.95);
        };

        if axis != pane_grid::Axis::Vertical {
            return ratio.clamp(0.05, 0.95);
        }

        let Some((first, second)) = split_children(self.workspace_panes.layout(), split) else {
            return ratio.clamp(0.05, 0.95);
        };

        let total_width = region.width.max(1.0);
        let min_first = dock_node_min_width(first, &self.workspace_panes, self).min(total_width);
        let min_second = dock_node_min_width(second, &self.workspace_panes, self).min(total_width);
        let min_ratio = (min_first / total_width).clamp(0.05, 0.95);
        let max_ratio = (1.0 - min_second / total_width).clamp(0.05, 0.95);

        if min_ratio > max_ratio {
            ratio.clamp(0.05, 0.95)
        } else {
            ratio.clamp(min_ratio, max_ratio)
        }
    }

    fn set_active_workspace_pane(&mut self, pane: WorkspacePaneKind) {
        let Some(group_id) = self.group_for_pane(pane) else {
            return;
        };
        let Some(group) = self.dock_groups.get_mut(&group_id) else {
            return;
        };

        if group.tabs.contains(&pane) {
            group.active = pane;
        }
    }

    fn dock_drop_target_for(&self, position: iced::Point) -> Option<DockDropTarget> {
        let bounds_map = self.workspace_group_bounds();
        let (group_id, bounds) = bounds_map
            .into_iter()
            .find(|(_, bounds)| bounds.contains(position))?;

        Some(DockDropTarget {
            group_id,
            region: dock_drop_region(bounds, position),
        })
    }

    fn workspace_group_bounds(&self) -> std::collections::HashMap<DockGroupId, iced::Rectangle> {
        let mut bounds = std::collections::HashMap::new();
        let root_bounds = self.workspace_bounds();
        collect_workspace_group_bounds(
            &self.workspace_panes,
            self.workspace_panes.layout(),
            root_bounds,
            &mut bounds,
        );
        bounds
    }

    fn fold_workspace_pane(&mut self, pane: WorkspacePaneKind) -> bool {
        if self.is_pane_folded(pane) {
            return false;
        }

        let Some(group_id) = self.group_for_pane(pane) else {
            return false;
        };
        let Some(group) = self.dock_groups.get(&group_id) else {
            return false;
        };

        let restore = if group.tabs.len() > 1 {
            let Some(anchor) = group
                .tabs
                .iter()
                .copied()
                .find(|candidate| *candidate != pane)
            else {
                return false;
            };
            let _ = remove_pane_from_group(&mut self.dock_groups, group_id, pane);
            FoldedPaneRestore::Tab { anchor }
        } else {
            self.dock_groups.remove(&group_id);
            if let Some((axis, ratio, insert_first, anchor, sibling_panes)) =
                self.dock_layout.as_ref().and_then(|layout| {
                    split_restore_target_for_group(layout, group_id, &self.dock_groups)
                })
            {
                let layout = self.dock_layout.take().unwrap_or(DockNode::Group(group_id));
                self.dock_layout = Some(prune_group_from_layout(layout, group_id));
                FoldedPaneRestore::Split {
                    anchor,
                    axis,
                    ratio,
                    insert_first,
                    sibling_panes,
                }
            } else {
                self.dock_layout = None;
                FoldedPaneRestore::Standalone
            }
        };

        self.folded_panes.retain(|folded| folded.pane != pane);
        self.folded_panes.push(FoldedPaneState { pane, restore });
        if pane == WorkspacePaneKind::PianoRoll {
            self.piano_roll.visible = false;
        }

        self.clear_workspace_drag_state();
        self.rebuild_workspace_panes();
        true
    }

    fn unfold_workspace_pane(&mut self, pane: WorkspacePaneKind) -> bool {
        let Some(index) = self
            .folded_panes
            .iter()
            .position(|folded| folded.pane == pane)
        else {
            return false;
        };
        let folded = self.folded_panes.remove(index);

        let restored = match folded.restore {
            FoldedPaneRestore::Tab { anchor } => self.restore_folded_pane_as_tab(pane, anchor),
            FoldedPaneRestore::Standalone => self.restore_folded_pane_as_standalone(pane),
            FoldedPaneRestore::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => self.restore_folded_pane_as_split(
                pane,
                anchor,
                axis,
                ratio,
                insert_first,
                &sibling_panes,
            ),
        };

        if !restored {
            if self.dock_groups.is_empty() {
                let _ = self.restore_folded_pane_as_standalone(pane);
            } else if let Some(group_id) =
                self.dock_layout.as_ref().and_then(first_group_id_in_layout)
            {
                if let Some(group) = self.dock_groups.get_mut(&group_id) {
                    group.tabs.push(pane);
                    group.active = pane;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        if pane == WorkspacePaneKind::PianoRoll {
            self.piano_roll.visible = true;
        }

        self.rebuild_workspace_panes();
        true
    }

    fn restore_folded_pane_as_tab(
        &mut self,
        pane: WorkspacePaneKind,
        anchor: WorkspacePaneKind,
    ) -> bool {
        let Some(group_id) = self.group_for_pane(anchor) else {
            return false;
        };
        let Some(group) = self.dock_groups.get_mut(&group_id) else {
            return false;
        };

        group.tabs.retain(|candidate| *candidate != pane);
        group.tabs.push(pane);
        group.active = pane;
        true
    }

    fn restore_folded_pane_as_standalone(&mut self, pane: WorkspacePaneKind) -> bool {
        if let Some(group_id) = self.dock_layout.as_ref().and_then(first_group_id_in_layout)
            && let Some(group) = self.dock_groups.get_mut(&group_id)
        {
            group.tabs.push(pane);
            group.active = pane;
            return true;
        }

        let new_group_id = self.next_dock_group_id;
        self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
        self.dock_groups.insert(
            new_group_id,
            DockGroup {
                tabs: vec![pane],
                active: pane,
            },
        );
        self.dock_layout = Some(DockNode::Group(new_group_id));
        true
    }

    fn restore_folded_pane_as_split(
        &mut self,
        pane: WorkspacePaneKind,
        anchor: WorkspacePaneKind,
        axis: pane_grid::Axis,
        ratio: f32,
        insert_first: bool,
        sibling_panes: &[WorkspacePaneKind],
    ) -> bool {
        let new_group_id = self.next_dock_group_id;
        self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
        self.dock_groups.insert(
            new_group_id,
            DockGroup {
                tabs: vec![pane],
                active: pane,
            },
        );

        if self.dock_layout.as_mut().is_some_and(|layout| {
            replace_subtree_with_split(
                layout,
                axis,
                ratio,
                new_group_id,
                insert_first,
                sibling_panes,
                &self.dock_groups,
            )
        }) {
            return true;
        }

        let Some(group_id) = self.group_for_pane(anchor) else {
            self.dock_groups.remove(&new_group_id);
            return false;
        };
        let Some(layout) = self.dock_layout.as_mut() else {
            self.dock_groups.remove(&new_group_id);
            return false;
        };

        if replace_group_with_split(layout, group_id, axis, ratio, new_group_id, insert_first) {
            true
        } else {
            self.dock_groups.remove(&new_group_id);
            false
        }
    }

    fn apply_dock_drop(&mut self, dragged: WorkspacePaneKind, target: DockDropTarget) {
        let Some(source_group_id) = self.group_for_pane(dragged) else {
            return;
        };
        if !self.dock_groups.contains_key(&target.group_id) {
            return;
        }

        match target.region {
            DockDropRegion::Center if source_group_id == target.group_id => {
                if let Some(group) = self.dock_groups.get_mut(&source_group_id) {
                    move_tab_to_front(&mut group.tabs, dragged);
                    group.active = dragged;
                }
            }
            DockDropRegion::Center => {
                let source_empty =
                    remove_pane_from_group(&mut self.dock_groups, source_group_id, dragged);

                if source_empty {
                    self.dock_groups.remove(&source_group_id);
                    let layout = self
                        .dock_layout
                        .take()
                        .unwrap_or(DockNode::Group(target.group_id));
                    self.dock_layout = Some(prune_group_from_layout(layout, source_group_id));
                }

                if let Some(target_group) = self.dock_groups.get_mut(&target.group_id) {
                    target_group.tabs.retain(|pane| *pane != dragged);
                    target_group.tabs.push(dragged);
                    target_group.active = dragged;
                }
            }
            region => {
                if source_group_id == target.group_id
                    && self
                        .dock_groups
                        .get(&source_group_id)
                        .is_some_and(|group| group.tabs.len() <= 1)
                {
                    return;
                }

                let source_empty =
                    remove_pane_from_group(&mut self.dock_groups, source_group_id, dragged);

                if source_empty && source_group_id != target.group_id {
                    self.dock_groups.remove(&source_group_id);
                    let layout = self
                        .dock_layout
                        .take()
                        .unwrap_or(DockNode::Group(target.group_id));
                    self.dock_layout = Some(prune_group_from_layout(layout, source_group_id));
                }

                let new_group_id = self.next_dock_group_id;
                self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
                self.dock_groups.insert(
                    new_group_id,
                    DockGroup {
                        tabs: vec![dragged],
                        active: dragged,
                    },
                );

                let (axis, insert_first) = match region {
                    DockDropRegion::Top => (pane_grid::Axis::Horizontal, true),
                    DockDropRegion::Bottom => (pane_grid::Axis::Horizontal, false),
                    DockDropRegion::Left => (pane_grid::Axis::Vertical, true),
                    DockDropRegion::Right => (pane_grid::Axis::Vertical, false),
                    DockDropRegion::Center => unreachable!(),
                };

                if let Some(layout) = self.dock_layout.as_mut() {
                    replace_group_with_split(
                        layout,
                        target.group_id,
                        axis,
                        0.5,
                        new_group_id,
                        insert_first,
                    );
                } else {
                    self.dock_layout = Some(DockNode::Group(new_group_id));
                }
            }
        }

        self.rebuild_workspace_panes();
    }

    fn clear_workspace_drag_state(&mut self) {
        self.hovered_workspace_pane = None;
        self.pressed_workspace_pane = None;
        self.workspace_drag_origin = None;
        self.dragged_workspace_pane = None;
        self.dock_drop_target = None;
    }

    fn persist_settings(&self) {
        let _ = settings::save(&settings::AppSettings {
            workspace_layout: WorkspaceLayoutSettings {
                root: self
                    .dock_layout
                    .as_ref()
                    .map(|layout| dock_node_to_settings(layout, &self.dock_groups)),
                folded_panes: self
                    .folded_panes
                    .iter()
                    .cloned()
                    .map(folded_pane_to_settings)
                    .collect(),
                piano_visible: !self.is_pane_folded(WorkspacePaneKind::PianoRoll),
            },
            score_view: settings::ScoreViewSettings {
                zoom: self.svg_zoom,
                page_brightness: self.svg_page_brightness,
            },
            piano_roll_view: settings::PianoRollViewSettings {
                zoom_x: self.piano_roll.zoom_x,
                beat_subdivision: self.piano_roll.beat_subdivision,
            },
            editor_theme: self.editor.theme_settings(),
        });
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

    fn restore_runtime_view_state(&self, pane: WorkspacePaneKind) -> Task<Message> {
        if self.group_for_pane(pane).is_none() {
            return Task::none();
        }

        match pane {
            WorkspacePaneKind::PianoRoll => self.restore_piano_roll_scroll(),
            WorkspacePaneKind::Score => self.restore_score_scroll(),
            WorkspacePaneKind::Editor | WorkspacePaneKind::Logger => Task::none(),
        }
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

    pub(super) fn score_zoom_preview_active(&self) -> bool {
        self.score_zoom_last_interaction
            .is_some_and(|instant| instant.elapsed() < SCORE_ZOOM_PREVIEW_SETTLE_DELAY)
    }

    fn request_score_zoom_preview(&mut self, zoom: f32) -> Option<Task<Message>> {
        let rendered_score = self.rendered_score.as_ref()?;
        let page = rendered_score.current_page()?;
        let page_index = rendered_score.current_page;

        if self.score_zoom_preview_pending.is_some() {
            return None;
        }

        let request = match self.score_zoom_preview.as_ref() {
            Some(preview)
                if preview.page_index == page_index
                    && preview.tier == ScoreZoomPreviewTier::Primary =>
            {
                return None;
            }
            Some(preview) if preview.page_index == page_index => ScoreZoomPreviewRequest {
                page_index,
                zoom: score_preview_target_zoom(zoom, ScoreZoomPreviewTier::Primary),
                tier: ScoreZoomPreviewTier::Primary,
            },
            _ => ScoreZoomPreviewRequest {
                page_index,
                zoom: score_preview_target_zoom(zoom, ScoreZoomPreviewTier::Fallback),
                tier: ScoreZoomPreviewTier::Fallback,
            },
        };

        let svg_bytes = page.svg_bytes.clone();
        let page_size = page.size;
        self.score_zoom_preview_pending = Some(request);

        Some(Task::perform(
            async move { render_score_zoom_preview(svg_bytes, page_size, request) },
            Message::ScorePreviewReady,
        ))
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

    fn rewind_target_tick(&self) -> u64 {
        let current_tick = self.piano_roll.playback_tick();
        let rewind_flag_tick = self.piano_roll.rewind_flag_tick();

        if current_tick > rewind_flag_tick {
            rewind_flag_tick
        } else {
            0
        }
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

fn dock_node_to_settings(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> DockNodeSettings {
    match node {
        DockNode::Group(group_id) => DockNodeSettings::Group(
            groups
                .get(group_id)
                .map(|group| DockGroupSettings {
                    tabs: group.tabs.clone(),
                    active: group.active,
                })
                .unwrap_or_default(),
        ),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => DockNodeSettings::Split {
            axis: dock_axis_to_settings(*axis),
            ratio: *ratio,
            first: Box::new(dock_node_to_settings(first, groups)),
            second: Box::new(dock_node_to_settings(second, groups)),
        },
    }
}

fn collect_workspace_group_bounds(
    state: &pane_grid::State<DockGroupId>,
    node: &pane_grid::Node,
    bounds: iced::Rectangle,
    group_bounds: &mut std::collections::HashMap<DockGroupId, iced::Rectangle>,
) {
    match node {
        pane_grid::Node::Pane(pane) => {
            if let Some(group_id) = state.get(*pane) {
                group_bounds.insert(*group_id, bounds);
            }
        }
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => match axis {
            pane_grid::Axis::Horizontal => {
                let first_height = bounds.height * ratio;
                collect_workspace_group_bounds(
                    state,
                    a,
                    iced::Rectangle {
                        height: first_height,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_workspace_group_bounds(
                    state,
                    b,
                    iced::Rectangle {
                        y: bounds.y + first_height,
                        height: bounds.height - first_height,
                        ..bounds
                    },
                    group_bounds,
                );
            }
            pane_grid::Axis::Vertical => {
                let first_width = bounds.width * ratio;
                collect_workspace_group_bounds(
                    state,
                    a,
                    iced::Rectangle {
                        width: first_width,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_workspace_group_bounds(
                    state,
                    b,
                    iced::Rectangle {
                        x: bounds.x + first_width,
                        width: bounds.width - first_width,
                        ..bounds
                    },
                    group_bounds,
                );
            }
        },
    }
}

fn split_children(
    node: &pane_grid::Node,
    split: pane_grid::Split,
) -> Option<(&pane_grid::Node, &pane_grid::Node)> {
    match node {
        pane_grid::Node::Pane(_) => None,
        pane_grid::Node::Split { id, a, b, .. } => {
            if *id == split {
                Some((a.as_ref(), b.as_ref()))
            } else {
                split_children(a, split).or_else(|| split_children(b, split))
            }
        }
    }
}

fn dock_node_min_width(
    node: &pane_grid::Node,
    state: &pane_grid::State<DockGroupId>,
    app: &LilyView,
) -> f32 {
    match node {
        pane_grid::Node::Pane(pane) => state
            .get(*pane)
            .map(|group_id| super::dock_view::workspace_group_min_width(app, *group_id))
            .unwrap_or(0.0),
        pane_grid::Node::Split { axis, a, b, .. } => {
            let first = dock_node_min_width(a, state, app);
            let second = dock_node_min_width(b, state, app);

            match axis {
                pane_grid::Axis::Horizontal => first.max(second),
                pane_grid::Axis::Vertical => first + second,
            }
        }
    }
}

fn dock_drop_region(bounds: iced::Rectangle, position: iced::Point) -> DockDropRegion {
    let relative_x = ((position.x - bounds.x) / bounds.width.max(1.0)).clamp(0.0, 1.0);
    let relative_y = ((position.y - bounds.y) / bounds.height.max(1.0)).clamp(0.0, 1.0);
    let center_min = 1.0 / 3.0;
    let center_max = 2.0 / 3.0;

    if (center_min..=center_max).contains(&relative_x)
        && (center_min..=center_max).contains(&relative_y)
    {
        return DockDropRegion::Center;
    }

    let top_distance = relative_y;
    let right_distance = 1.0 - relative_x;
    let bottom_distance = 1.0 - relative_y;
    let left_distance = relative_x;
    let mut closest = (DockDropRegion::Top, top_distance);

    for candidate in [
        (DockDropRegion::Right, right_distance),
        (DockDropRegion::Bottom, bottom_distance),
        (DockDropRegion::Left, left_distance),
    ] {
        if candidate.1 < closest.1 {
            closest = candidate;
        }
    }

    closest.0
}

fn move_tab_to_front(tabs: &mut Vec<WorkspacePaneKind>, pane: WorkspacePaneKind) {
    if let Some(index) = tabs.iter().position(|candidate| *candidate == pane) {
        let pane = tabs.remove(index);
        tabs.insert(0, pane);
    }
}

fn drag_distance(a: iced::Point, b: iced::Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

fn remove_pane_from_group(
    groups: &mut std::collections::HashMap<DockGroupId, DockGroup>,
    group_id: DockGroupId,
    pane: WorkspacePaneKind,
) -> bool {
    let Some(group) = groups.get_mut(&group_id) else {
        return false;
    };

    group.tabs.retain(|candidate| *candidate != pane);

    if group.active == pane {
        group.active = group
            .tabs
            .first()
            .copied()
            .unwrap_or(WorkspacePaneKind::Score);
    }

    group.tabs.is_empty()
}

fn prune_group_from_layout(layout: DockNode, group_id: DockGroupId) -> DockNode {
    prune_group_from_layout_inner(layout, group_id).unwrap_or(DockNode::Group(group_id))
}

fn prune_group_from_layout_inner(layout: DockNode, group_id: DockGroupId) -> Option<DockNode> {
    match layout {
        DockNode::Group(candidate) => (candidate != group_id).then_some(DockNode::Group(candidate)),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let first = prune_group_from_layout_inner(*first, group_id);
            let second = prune_group_from_layout_inner(*second, group_id);

            match (first, second) {
                (Some(first), Some(second)) => Some(DockNode::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            }
        }
    }
}

fn replace_group_with_split(
    node: &mut DockNode,
    target_group_id: DockGroupId,
    axis: pane_grid::Axis,
    ratio: f32,
    new_group_id: DockGroupId,
    insert_first: bool,
) -> bool {
    match node {
        DockNode::Group(group_id) if *group_id == target_group_id => {
            let existing_group = DockNode::Group(*group_id);
            let new_group = DockNode::Group(new_group_id);
            *node = DockNode::Split {
                axis,
                ratio,
                first: Box::new(if insert_first {
                    new_group.clone()
                } else {
                    existing_group.clone()
                }),
                second: Box::new(if insert_first {
                    existing_group
                } else {
                    new_group
                }),
            };
            true
        }
        DockNode::Group(_) => false,
        DockNode::Split { first, second, .. } => {
            replace_group_with_split(
                first,
                target_group_id,
                axis,
                ratio,
                new_group_id,
                insert_first,
            ) || replace_group_with_split(
                second,
                target_group_id,
                axis,
                ratio,
                new_group_id,
                insert_first,
            )
        }
    }
}

fn split_restore_target_for_group(
    node: &DockNode,
    group_id: DockGroupId,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Option<(
    pane_grid::Axis,
    f32,
    bool,
    WorkspacePaneKind,
    Vec<WorkspacePaneKind>,
)> {
    match node {
        DockNode::Group(_) => None,
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
            ..
        } => {
            if contains_group(first, group_id) {
                if let Some(target) = split_restore_target_for_group(first, group_id, groups) {
                    return Some(target);
                }

                let sibling_panes = panes_in_node(second, groups);
                Some((
                    *axis,
                    *ratio,
                    true,
                    first_pane_in_node(second, groups)?,
                    sibling_panes,
                ))
            } else if contains_group(second, group_id) {
                if let Some(target) = split_restore_target_for_group(second, group_id, groups) {
                    return Some(target);
                }

                let sibling_panes = panes_in_node(first, groups);
                Some((
                    *axis,
                    *ratio,
                    false,
                    first_pane_in_node(first, groups)?,
                    sibling_panes,
                ))
            } else {
                None
            }
        }
    }
}

fn first_pane_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Option<WorkspacePaneKind> {
    match node {
        DockNode::Group(group_id) => groups
            .get(group_id)
            .and_then(|group| group.tabs.first().copied()),
        DockNode::Split { first, second, .. } => {
            first_pane_in_node(first, groups).or_else(|| first_pane_in_node(second, groups))
        }
    }
}

fn panes_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Vec<WorkspacePaneKind> {
    let mut panes = Vec::new();
    collect_panes_in_node(node, groups, &mut panes);
    panes.sort_by_key(|pane| pane_sort_key(*pane));
    panes.dedup();
    panes
}

fn collect_panes_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
    panes: &mut Vec<WorkspacePaneKind>,
) {
    match node {
        DockNode::Group(group_id) => {
            if let Some(group) = groups.get(group_id) {
                panes.extend(group.tabs.iter().copied());
            }
        }
        DockNode::Split { first, second, .. } => {
            collect_panes_in_node(first, groups, panes);
            collect_panes_in_node(second, groups, panes);
        }
    }
}

fn replace_subtree_with_split(
    node: &mut DockNode,
    axis: pane_grid::Axis,
    ratio: f32,
    new_group_id: DockGroupId,
    insert_first: bool,
    target_panes: &[WorkspacePaneKind],
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> bool {
    if panes_in_node(node, groups) == target_panes {
        let existing = node.clone();
        let new_group = DockNode::Group(new_group_id);
        *node = DockNode::Split {
            axis,
            ratio,
            first: Box::new(if insert_first {
                new_group.clone()
            } else {
                existing.clone()
            }),
            second: Box::new(if insert_first { existing } else { new_group }),
        };
        return true;
    }

    match node {
        DockNode::Group(_) => false,
        DockNode::Split { first, second, .. } => {
            replace_subtree_with_split(
                first,
                axis,
                ratio,
                new_group_id,
                insert_first,
                target_panes,
                groups,
            ) || replace_subtree_with_split(
                second,
                axis,
                ratio,
                new_group_id,
                insert_first,
                target_panes,
                groups,
            )
        }
    }
}

fn pane_sort_key(pane: WorkspacePaneKind) -> u8 {
    match pane {
        WorkspacePaneKind::Score => 0,
        WorkspacePaneKind::PianoRoll => 1,
        WorkspacePaneKind::Editor => 2,
        WorkspacePaneKind::Logger => 3,
    }
}

fn first_group_id_in_layout(node: &DockNode) -> Option<DockGroupId> {
    match node {
        DockNode::Group(group_id) => Some(*group_id),
        DockNode::Split { first, second, .. } => {
            first_group_id_in_layout(first).or_else(|| first_group_id_in_layout(second))
        }
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

fn score_preview_target_zoom(zoom: f32, tier: ScoreZoomPreviewTier) -> f32 {
    match tier {
        ScoreZoomPreviewTier::Fallback => zoom.max(SCORE_PREVIEW_FALLBACK_MIN_ZOOM),
        ScoreZoomPreviewTier::Primary => zoom.max(SCORE_PREVIEW_PRIMARY_MIN_ZOOM),
    }
}

fn render_score_zoom_preview(
    svg_bytes: Bytes,
    page_size: SvgSize,
    request: ScoreZoomPreviewRequest,
) -> Result<super::messages::ScorePreviewReady, String> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes.as_ref(), &options)
        .map_err(|error| format!("Failed to parse score SVG: {error}"))?;

    let logical_width =
        (page_size.width * super::score_view::score_base_scale() * request.zoom).max(1.0);
    let logical_height =
        (page_size.height * super::score_view::score_base_scale() * request.zoom).max(1.0);
    let longest_edge = logical_width.max(logical_height).max(1.0);
    let max_dimension = match request.tier {
        ScoreZoomPreviewTier::Fallback => SCORE_PREVIEW_FALLBACK_MAX_DIMENSION,
        ScoreZoomPreviewTier::Primary => SCORE_PREVIEW_PRIMARY_MAX_DIMENSION,
    };
    let raster_scale = (max_dimension / longest_edge).min(1.0);
    let raster_width = (logical_width * raster_scale).round().max(1.0) as u32;
    let raster_height = (logical_height * raster_scale).round().max(1.0) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(raster_width, raster_height)
        .ok_or_else(|| "Failed to allocate score preview pixmap".to_string())?;

    let tree_size = tree.size().to_int_size().to_size();
    let transform = tiny_skia::Transform::from_scale(
        raster_width as f32 / tree_size.width(),
        raster_height as f32 / tree_size.height(),
    );

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(super::messages::ScorePreviewReady {
        page_index: request.page_index,
        zoom: request.zoom,
        tier: request.tier,
        handle: image::Handle::from_rgba(raster_width, raster_height, pixmap.take()),
    })
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
