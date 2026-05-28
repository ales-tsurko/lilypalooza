use super::*;

#[derive(Debug, Clone, Copy)]
enum EditorTabDiskProblem {
    Missing,
    Changed,
}

impl EditorTabDiskProblem {
    fn file_state(self) -> EditorTabFileState {
        match self {
            Self::Missing => EditorTabFileState::MissingOnDisk,
            Self::Changed => EditorTabFileState::ChangedOnDisk,
        }
    }

    fn log_message(self) -> &'static str {
        match self {
            Self::Missing => "Editor file missing on disk",
            Self::Changed => "Editor file changed on disk while modified",
        }
    }
}

impl Lilypalooza {
    pub(in crate::app) fn handle_tick(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();

        self.refresh_active_playback_position();
        tasks.push(self.tick_editor_tabbar_autoscroll());
        tasks.push(self.tick_effect_rack_autoscroll());
        self.queue_pending_editor_tab_reveal(&mut tasks);
        self.refresh_editor_font_metrics_if_needed(&mut tasks);

        self.poll_editor_file_watcher(&mut tasks);
        self.poll_browser_file_watcher(&mut tasks);
        tasks.push(self.poll_plugin_scan());
        self.queue_editor_tick(&mut tasks);
        self.advance_spinner_tasks(&mut tasks);
        self.settle_score_zoom_preview();

        Task::batch(tasks)
    }

    pub(super) fn refresh_active_playback_position(&mut self) {
        if self.playback.is_some() && self.piano_roll.playback_is_playing() {
            self.refresh_playback_position();
        }
    }

    pub(super) fn queue_pending_editor_tab_reveal(&mut self, tasks: &mut Vec<Task<Message>>) {
        let Some(tab_id) = self.pending_reveal_editor_tab else {
            return;
        };
        if !self.editor.tab_ids().contains(&tab_id) {
            self.pending_reveal_editor_tab = None;
            return;
        }
        if self.editor_tab_reveal_target_x(tab_id).is_some() {
            tasks.push(self.reveal_editor_tab(tab_id));
        } else {
            self.pending_reveal_editor_tab = None;
        }
    }

    pub(super) fn refresh_editor_font_metrics_if_needed(&mut self, tasks: &mut Vec<Task<Message>>) {
        if !self.editor_font_metrics_refresh_pending {
            return;
        }
        self.editor.refresh_font_metrics();
        self.editor_font_metrics_refresh_pending = false;
        if let Some(tab_id) = self.editor.active_tab_id() {
            tasks.push(
                self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id)),
            );
        }
    }

    pub(super) fn queue_editor_tick(&mut self, tasks: &mut Vec<Task<Message>>) {
        if self.editor_tick_active()
            && let Some(tab_id) = self.editor.active_tab_id()
        {
            let task = self.editor.update(tab_id, &iced_code_editor::Message::Tick);
            tasks.push(self.map_editor_widget_task(tab_id, task));
        }
    }

    pub(super) fn advance_spinner_tasks(&mut self, tasks: &mut Vec<Task<Message>>) {
        if !self.spinner_active() {
            return;
        }
        self.spinner_step = self.spinner_step.wrapping_add(1);
        self.poll_score_watcher();
        tasks.push(self.poll_compile_logs());
        self.start_compile_if_queued();
        tasks.push(self.apply_initial_piano_roll_center_if_needed());
    }

    pub(super) fn settle_score_zoom_preview(&mut self) {
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
    }

    pub(in crate::app) fn handle_frame(&mut self, now: std::time::Instant) -> Task<Message> {
        if let Some(task) = self.advance_deferred_mixer_message_after_detach() {
            return task;
        }

        self.refresh_frame_playback_position();
        self.drain_processor_editor_resize_errors(now);
        self.drain_processor_editor_frame_commands();
        let close_requests = self.processor_editor_windows.close_requested_windows();
        Task::batch(
            close_requests
                .into_iter()
                .map(|window_id| self.handle_processor_editor_close_requested(window_id)),
        )
    }

    pub(super) fn advance_deferred_mixer_message_after_detach(&mut self) -> Option<Task<Message>> {
        if let Some(deferred) = self.pending_mixer_message_after_editor_detach.as_mut()
            && deferred.frames_remaining > 0
        {
            deferred.frames_remaining -= 1;
            return None;
        }
        self.pending_mixer_message_after_editor_detach
            .take()
            .map(|deferred| self.handle_mixer_message(deferred.message))
    }

    pub(super) fn refresh_frame_playback_position(&mut self) {
        if self.playback.is_some() {
            self.refresh_playback_position();
        }
    }

    pub(super) fn drain_processor_editor_resize_errors(&mut self, now: std::time::Instant) {
        let mut resize_errors = Vec::new();
        self.processor_editor_windows
            .apply_requested_content_resizes(|error| resize_errors.push(error));
        resize_errors.extend(self.processor_editor_windows.sync_native_content_resizes());
        resize_errors.extend(
            self.processor_editor_windows
                .expire_deferred_outer_resizes(now),
        );
        for error in resize_errors {
            self.log_processor_editor_error("resize", error);
        }
    }

    pub(super) fn drain_processor_editor_frame_commands(&mut self) {
        for (target, command) in self.processor_editor_windows.drain_frame_commands() {
            self.handle_processor_editor_frame_command(target, command);
        }
    }

    pub(in crate::app) fn activate_score(
        &mut self,
        selected_score: SelectedScore,
    ) -> Task<Message> {
        let watched_path = selected_score.path.clone();
        let editor_task = match self.editor.load_file(&watched_path) {
            Ok((tab_id, task, _)) => {
                self.register_editor_recent_file(&watched_path);
                self.pending_reveal_editor_tab = Some(tab_id);
                let sync_task = self.editor.sync_tab_scroll_state(tab_id);
                self.map_editor_widget_task(tab_id, Task::batch([task, sync_task]))
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Load Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        };
        self.logger.push(format!(
            "Opened score file {}",
            selected_score.path.display()
        ));
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.compile_outputs_loading = false;
        self.piano_roll.clear_files();
        self.unload_playback_file();
        self.current_score = Some(selected_score);
        self.persist_settings();
        if self.project_root.is_none() {
            self.saved_project_state = Some(self.current_project_state());
        }
        self.sync_editor_file_watcher();
        self.restart_score_watcher(&watched_path);
        self.queue_compile("Score loaded, compiling SVG and MIDI");
        self.start_compile_if_queued();
        editor_task
    }

    pub(in crate::app) fn restart_score_watcher(&mut self, path: &Path) {
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

    pub(in crate::app) fn poll_score_watcher(&mut self) {
        let Some(score_watcher) = &self.score_watcher else {
            return;
        };

        let watched_path = score_watcher.watched_path().to_path_buf();
        let poll = drain_score_watcher(score_watcher, &watched_path);
        for error in poll.errors {
            self.logger.push(format!("[watcher:error] {error}"));
        }

        if poll.state.disconnected {
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

        if poll.state.changed {
            self.queue_compile("Score changed, recompiling");
        }
    }

    pub(in crate::app) fn sync_editor_file_watcher(&mut self) {
        let paths = self.editor.file_backed_tab_paths();

        if paths.is_empty() {
            self.editor_file_watcher = None;
            return;
        }

        if !self.ensure_editor_file_watcher_started() {
            return;
        }

        if let Err(error) = self.sync_editor_file_watcher_paths(&paths) {
            self.editor_file_watcher = None;
            self.show_editor_file_watcher_error(format!(
                "Failed to update editor file watches: {error}"
            ));
        }
    }

    pub(super) fn ensure_editor_file_watcher_started(&mut self) -> bool {
        if self.editor_file_watcher.is_some() {
            return true;
        }
        match crate::editor_file_watcher::EditorFileWatcher::start() {
            Ok(watcher) => {
                self.editor_file_watcher = Some(watcher);
                true
            }
            Err(error) => {
                self.show_editor_file_watcher_error(format!(
                    "Failed to watch editor file changes: {error}"
                ));
                false
            }
        }
    }

    pub(super) fn sync_editor_file_watcher_paths(
        &mut self,
        paths: &[PathBuf],
    ) -> Result<(), String> {
        let Some(watcher) = &mut self.editor_file_watcher else {
            return Ok(());
        };
        watcher.sync_paths(paths).map_err(|error| error.to_string())
    }

    pub(super) fn show_editor_file_watcher_error(&mut self, error: String) {
        self.show_prompt(
            ErrorPrompt::new(
                "File Watcher Error",
                error,
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(in crate::app) fn sync_browser_file_watcher(&mut self) {
        if !self.editor.file_browser_expanded() {
            self.browser_file_watcher = None;
            return;
        }

        let root = self.editor.file_browser_root().to_path_buf();
        if self
            .browser_file_watcher
            .as_ref()
            .is_some_and(|watcher| watcher.watched_root() == root)
        {
            return;
        }

        match crate::browser_file_watcher::BrowserFileWatcher::start(&root) {
            Ok(watcher) => self.browser_file_watcher = Some(watcher),
            Err(error) => {
                self.browser_file_watcher = None;
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
                        format!("Failed to watch browser directory changes: {error}"),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    pub(in crate::app) fn poll_editor_file_watcher(&mut self, tasks: &mut Vec<Task<Message>>) {
        if self.editor_file_watcher.is_none() {
            return;
        }

        let watched_tabs = self.editor.file_backed_tabs();
        let drain = self.drain_editor_file_watcher(&watched_tabs);
        self.logger.extend(drain.errors);

        if drain.disconnected {
            self.handle_editor_file_watcher_disconnected();
            return;
        }

        for tab_id in drain.affected_tabs {
            if let Some(task) = self.reconcile_editor_tab_file(tab_id) {
                tasks.push(task);
            }
        }
    }

    pub(super) fn drain_editor_file_watcher(
        &self,
        watched_tabs: &[(u64, PathBuf)],
    ) -> EditorFileWatcherDrain {
        let mut drain = EditorFileWatcherDrain::default();

        let Some(watcher) = &self.editor_file_watcher else {
            return drain;
        };
        loop {
            if !poll_editor_file_watcher_event(watcher, watched_tabs, &mut drain) {
                break;
            }
        }

        drain
    }

    pub(super) fn handle_editor_file_watcher_disconnected(&mut self) {
        self.editor_file_watcher = None;
        self.show_prompt(
            ErrorPrompt::new(
                "File Watcher Error",
                "Editor file watcher disconnected",
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(in crate::app) fn poll_browser_file_watcher(&mut self, tasks: &mut Vec<Task<Message>>) {
        let Some(watcher) = &self.browser_file_watcher else {
            return;
        };

        let watched_root = watcher.watched_root().to_path_buf();
        let poll = drain_browser_file_watcher(watcher, &watched_root);

        if poll.disconnected {
            self.browser_file_watcher = None;
            self.show_prompt(
                ErrorPrompt::new(
                    "File Browser Error",
                    "Browser directory watcher disconnected",
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
            return;
        }

        if poll.changed {
            tasks.push(self.refresh_browser_after_fs_change());
        }
    }

    pub(super) fn reconcile_editor_tab_file(&mut self, tab_id: u64) -> Option<Task<Message>> {
        let state = self.editor_tab_disk_state(tab_id)?;
        self.reconcile_editor_tab_disk_state(tab_id, state)
    }

    pub(super) fn reconcile_editor_tab_disk_state(
        &mut self,
        tab_id: u64,
        state: EditorTabDiskState,
    ) -> Option<Task<Message>> {
        match state {
            EditorTabDiskState::Missing(path) => self.reconcile_missing_editor_tab(tab_id, &path),
            EditorTabDiskState::ReadError { path, error } => {
                self.reconcile_unreadable_editor_tab(&path, &error)
            }
            EditorTabDiskState::Unchanged => self.reconcile_unchanged_editor_tab(tab_id),
            EditorTabDiskState::Changed { path, modified } => {
                self.reconcile_changed_editor_tab_file(tab_id, &path, modified)
            }
        }
    }

    pub(super) fn reconcile_missing_editor_tab(
        &mut self,
        tab_id: u64,
        path: &Path,
    ) -> Option<Task<Message>> {
        self.mark_editor_tab_missing(tab_id, path);
        None
    }

    pub(super) fn reconcile_unreadable_editor_tab(
        &mut self,
        path: &Path,
        error: &str,
    ) -> Option<Task<Message>> {
        self.log_editor_tab_read_error(path, error);
        None
    }

    pub(super) fn reconcile_unchanged_editor_tab(&mut self, tab_id: u64) -> Option<Task<Message>> {
        self.mark_editor_tab_unchanged(tab_id);
        None
    }

    pub(super) fn editor_tab_disk_state(&self, tab_id: u64) -> Option<EditorTabDiskState> {
        let path = self.editor.tab_path(tab_id)?.to_path_buf();
        let disk_content = match editor_tab_existing_disk_content(&path) {
            Ok(content) => content,
            Err(state) => return Some(state),
        };
        if self.editor.tab_saved_content(tab_id) == Some(disk_content.as_str()) {
            return Some(EditorTabDiskState::Unchanged);
        }

        Some(EditorTabDiskState::Changed {
            path,
            modified: self.editor.tab_is_modified(tab_id),
        })
    }

    pub(super) fn mark_editor_tab_missing(&mut self, tab_id: u64, path: &Path) {
        self.mark_editor_tab_disk_problem(tab_id, path, EditorTabDiskProblem::Missing);
    }

    pub(super) fn show_missing_editor_tab_prompt(&mut self, path: &Path) {
        if self.error_prompt.is_some() {
            return;
        }

        self.show_prompt(
            ErrorPrompt::new(
                "File Missing on Disk",
                format!(
                    "{} was removed or moved outside Lilypalooza. The tab stays open and you can \
                     save to recreate it.",
                    path.display()
                ),
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            )
            .with_ok_label("Keep Editing"),
            None,
        );
    }

    pub(super) fn log_editor_tab_read_error(&mut self, path: &Path, error: &str) {
        self.logger.push(format!(
            "[editor-watcher:error] Failed to read {}: {error}",
            path.display()
        ));
    }

    pub(super) fn mark_editor_tab_unchanged(&mut self, tab_id: u64) {
        if self
            .editor
            .set_tab_file_state(tab_id, EditorTabFileState::Ok)
        {
            self.persist_settings();
        }
    }

    pub(super) fn reconcile_changed_editor_tab_file(
        &mut self,
        tab_id: u64,
        path: &Path,
        modified: bool,
    ) -> Option<Task<Message>> {
        if modified {
            self.mark_editor_tab_changed_on_disk(tab_id, path);
            return None;
        }

        self.reload_unmodified_editor_tab(tab_id, path)
    }

    pub(super) fn mark_editor_tab_changed_on_disk(&mut self, tab_id: u64, path: &Path) {
        self.mark_editor_tab_disk_problem(tab_id, path, EditorTabDiskProblem::Changed);
    }

    fn mark_editor_tab_disk_problem(
        &mut self,
        tab_id: u64,
        path: &Path,
        problem: EditorTabDiskProblem,
    ) {
        if !self.editor.set_tab_file_state(tab_id, problem.file_state()) {
            return;
        }
        self.logger
            .push(format!("{}: {}", problem.log_message(), path.display()));
        match problem {
            EditorTabDiskProblem::Missing => self.show_missing_editor_tab_prompt(path),
            EditorTabDiskProblem::Changed => self.show_changed_editor_tab_prompt(tab_id, path),
        }
        self.persist_settings();
    }

    pub(super) fn show_changed_editor_tab_prompt(&mut self, tab_id: u64, path: &Path) {
        if self.error_prompt.is_some() {
            return;
        }

        self.show_prompt(
            ErrorPrompt::new(
                "File Changed on Disk",
                format!(
                    "{} changed on disk while this tab has unsaved changes.",
                    path.display()
                ),
                ErrorFatality::Recoverable,
                PromptButtons::OkCancel,
            )
            .with_ok_label("Reload")
            .with_cancel_label("Keep Editor Version"),
            Some(PromptOkAction::ReloadEditorTab(tab_id)),
        );
    }

    pub(super) fn reload_unmodified_editor_tab(
        &mut self,
        tab_id: u64,
        path: &Path,
    ) -> Option<Task<Message>> {
        match self.editor.reload_tab_from_disk(tab_id) {
            Ok(task) => {
                self.logger
                    .push(format!("Reloaded editor file {}", path.display()));
                self.persist_settings();
                Some(self.map_editor_widget_task(tab_id, task))
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
                None
            }
        }
    }
}
