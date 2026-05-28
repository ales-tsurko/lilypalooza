use super::*;

impl Lilypalooza {
    pub(super) fn handle_editor_file_message(&mut self, message: EditorMessage) -> Task<Message> {
        match message {
            EditorMessage::OpenRequested
            | EditorMessage::OpenPicked(_)
            | EditorMessage::OpenRecent(_) => self.handle_editor_open_message(message),
            EditorMessage::SaveRequested
            | EditorMessage::SaveAsRequested
            | EditorMessage::SaveAsPicked(_) => self.handle_editor_save_message(message),
            _ => Task::none(),
        }
    }

    pub(super) fn handle_editor_open_message(&mut self, message: EditorMessage) -> Task<Message> {
        editor_open_task(self, message).unwrap_or_else(Task::none)
    }

    pub(super) fn handle_editor_save_message(&mut self, message: EditorMessage) -> Task<Message> {
        editor_save_task(self, message).unwrap_or_else(Task::none)
    }

    pub(super) fn request_open_editor_files(&mut self) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.close_editor_menus();
        self.cancel_editor_tab_rename_state();
        Task::perform(
            async {
                rfd::AsyncFileDialog::new().pick_files().await.map(|files| {
                    files
                        .into_iter()
                        .map(|file| file.path().to_path_buf())
                        .collect()
                })
            },
            |picked| Message::Editor(EditorMessage::OpenPicked(picked)),
        )
    }

    pub(super) fn open_picked_editor_files(&mut self, paths: Vec<PathBuf>) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.open_editor_files_in_editor(&paths)
    }

    pub(super) fn open_recent_editor_file(&mut self, path: PathBuf) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.close_editor_menus();
        self.cancel_editor_tab_rename_state();
        self.open_editor_file_in_editor(&path)
    }

    pub(super) fn save_requested_editor_file(&mut self) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.save_active_editor_tab()
    }

    pub(super) fn cancel_save_active_editor_tab_as(&mut self) -> Task<Message> {
        self.pending_editor_save_as_tab = None;
        self.error_prompt = None;
        self.prompt_selected_button = crate::error_prompt::PromptSelectedButton::Ok;
        self.pending_editor_action = None;
        Task::none()
    }

    pub(super) fn request_save_active_editor_tab_as(&mut self) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        if !self.editor.has_document() {
            return Task::none();
        }
        self.close_editor_menus();
        let suggested_name = self.editor.suggested_save_name();
        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_file_name(&suggested_name)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |picked| Message::Editor(EditorMessage::SaveAsPicked(picked)),
        )
    }

    pub(super) fn finish_save_active_editor_tab_as(&mut self, path: PathBuf) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        if let Some(tab_id) = self.pending_editor_save_as_tab.take() {
            let save_task = self.save_editor_tab_to_path(tab_id, path, true);
            if self.pending_editor_action.is_some() {
                let advance_task = self.advance_pending_editor_action();
                return Task::batch([save_task, advance_task]);
            }
            return save_task;
        }
        self.save_active_editor_tab_to_path(path)
    }

    pub(super) fn handle_editor_appearance_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        if let Some(task) = self.handle_center_cursor_message(&message) {
            return task;
        }
        if self.handle_editor_zoom_message(&message) || self.handle_editor_theme_message(message) {
            self.persist_settings();
        }
        Task::none()
    }

    pub(super) fn handle_center_cursor_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        let EditorMessage::SetCenterCursor(value) = message else {
            return None;
        };

        self.editor.set_center_cursor(*value);
        self.persist_settings();
        if let Some(tab_id) = self.editor.active_tab_id() {
            let task = self.editor.sync_tab_scroll_state(tab_id);
            return Some(self.map_editor_widget_task(tab_id, task));
        }
        Some(Task::none())
    }

    pub(super) fn handle_editor_zoom_message(&mut self, message: &EditorMessage) -> bool {
        match message {
            EditorMessage::ZoomIn => {
                self.editor.zoom_in();
                true
            }
            EditorMessage::ZoomOut => {
                self.editor.zoom_out();
                true
            }
            EditorMessage::ResetZoom => {
                self.editor.reset_zoom();
                true
            }
            _ => false,
        }
    }

    pub(super) fn handle_editor_theme_message(&mut self, message: EditorMessage) -> bool {
        self.handle_editor_color_theme_message(&message)
            || self.handle_editor_dim_theme_message(&message)
    }

    pub(super) fn handle_editor_color_theme_message(&mut self, message: &EditorMessage) -> bool {
        self.apply_editor_hue_message(message)
            || self.apply_editor_saturation_message(message)
            || self.apply_editor_warmth_message(message)
            || self.apply_editor_brightness_message(message)
    }

    pub(super) fn handle_editor_dim_theme_message(&mut self, message: &EditorMessage) -> bool {
        self.apply_editor_text_dim_message(message)
            || self.apply_editor_comment_dim_message(message)
    }

    pub(super) fn apply_editor_hue_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeHueOffsetDegrees(value) = *message else {
            return false;
        };
        self.editor.set_hue_offset_degrees(value);
        true
    }

    pub(super) fn apply_editor_saturation_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeSaturation(value) = *message else {
            return false;
        };
        self.editor.set_saturation(value);
        true
    }

    pub(super) fn apply_editor_warmth_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeWarmth(value) = *message else {
            return false;
        };
        self.editor.set_warmth(value);
        true
    }

    pub(super) fn apply_editor_brightness_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeBrightness(value) = *message else {
            return false;
        };
        self.editor.set_brightness(value);
        true
    }

    pub(super) fn apply_editor_text_dim_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeTextDim(value) = *message else {
            return false;
        };
        self.editor.set_text_dim(value);
        true
    }

    pub(super) fn apply_editor_comment_dim_message(&mut self, message: &EditorMessage) -> bool {
        let EditorMessage::SetThemeCommentDim(value) = *message else {
            return false;
        };
        self.editor.set_comment_dim(value);
        true
    }

    pub(in crate::app) fn editor_tab_targets_main_score(&self, tab_id: u64) -> bool {
        self.editor.tab_path(tab_id)
            == self
                .current_score
                .as_ref()
                .map(|score| score.path.as_path())
    }

    pub(in crate::app) fn open_editor_file_in_editor(&mut self, path: &Path) -> Task<Message> {
        self.open_editor_file_in_editor_internal(path, true, true)
    }

    pub(in crate::app) fn open_editor_file_in_editor_internal(
        &mut self,
        path: &Path,
        log_open: bool,
        focus_text_area: bool,
    ) -> Task<Message> {
        self.cancel_editor_tab_rename_state();
        match self.editor.load_file(path) {
            Ok((tab_id, task, reused_existing)) => self.finish_open_editor_file(
                path,
                tab_id,
                task,
                reused_existing,
                log_open,
                focus_text_area,
            ),
            Err(error) => self.show_editor_open_error_task(error),
        }
    }

    pub(super) fn finish_open_editor_file(
        &mut self,
        path: &Path,
        tab_id: u64,
        task: iced::Task<iced_code_editor::Message>,
        reused_existing: bool,
        log_open: bool,
        focus_text_area: bool,
    ) -> Task<Message> {
        self.register_editor_recent_file(path);
        self.sync_editor_file_watcher();
        if log_open {
            self.log_loaded_editor_file(path, reused_existing);
        }
        let _pane_is_visible = self.ensure_workspace_pane_visible(WorkspacePaneKind::Editor);
        self.set_active_workspace_pane(WorkspacePaneKind::Editor);
        if focus_text_area {
            self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
            self.focus_editor_text_area();
        }
        let sync_task = self.editor.sync_tab_scroll_state(tab_id);
        self.pending_reveal_editor_tab = Some(tab_id);
        self.map_editor_widget_task(tab_id, iced::Task::batch([task, sync_task]))
    }

    pub(super) fn show_editor_open_error_task(&mut self, error: String) -> Task<Message> {
        self.show_editor_open_error(error);
        Task::none()
    }

    pub(in crate::app) fn open_editor_file_at_location(
        &mut self,
        path: &Path,
        line: usize,
        column: usize,
    ) -> Task<Message> {
        self.cancel_editor_tab_rename_state();
        match self.editor.load_file(path) {
            Ok((tab_id, task, reused_existing)) => {
                self.register_editor_recent_file(path);
                self.sync_editor_file_watcher();
                if reused_existing {
                    self.logger
                        .push(format!("Activated editor file {}", path.display()));
                } else {
                    self.logger
                        .push(format!("Opened editor file {}", path.display()));
                }
                self.editor.activate_tab(tab_id);
                let _pane_is_visible =
                    self.ensure_workspace_pane_visible(WorkspacePaneKind::Editor);
                self.set_active_workspace_pane(WorkspacePaneKind::Editor);
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_text_area();
                let cursor_task = self.editor.set_tab_cursor(tab_id, line, column);
                self.pending_reveal_editor_tab = Some(tab_id);
                self.map_editor_widget_task(tab_id, iced::Task::batch([task, cursor_task]))
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Open Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        }
    }

    pub(in crate::app) fn open_editor_files_in_editor(
        &mut self,
        paths: &[PathBuf],
    ) -> Task<Message> {
        self.cancel_editor_tab_rename_state();

        let mut tasks = Vec::new();
        let mut last_tab_id = None;

        for path in paths {
            match self.load_editor_file_for_batch(path) {
                Ok(opened) => {
                    last_tab_id = Some(opened.tab_id);
                    tasks.push(opened.task);
                }
                Err(error) => {
                    self.show_editor_open_error(error);
                    break;
                }
            }
        }

        if let Some(tab_id) = last_tab_id {
            self.focus_editor_text_area();
            self.pending_reveal_editor_tab = Some(tab_id);
        }

        Task::batch(tasks)
    }

    pub(super) fn load_editor_file_for_batch(
        &mut self,
        path: &Path,
    ) -> Result<OpenedEditorFile, String> {
        let (tab_id, task, reused_existing) = self.editor.load_file(path)?;
        self.register_loaded_editor_file(path, reused_existing);
        Ok(OpenedEditorFile {
            tab_id,
            task: self.map_editor_widget_task(
                tab_id,
                iced::Task::batch([task, self.editor.sync_tab_scroll_state(tab_id)]),
            ),
        })
    }

    pub(super) fn register_loaded_editor_file(&mut self, path: &Path, reused_existing: bool) {
        self.register_editor_recent_file(path);
        self.sync_editor_file_watcher();
        self.log_loaded_editor_file(path, reused_existing);
    }

    pub(super) fn log_loaded_editor_file(&mut self, path: &Path, reused_existing: bool) {
        let action = if reused_existing {
            "Activated"
        } else {
            "Opened"
        };
        self.logger
            .push(format!("{action} editor file {}", path.display()));
    }

    pub(super) fn show_editor_open_error(&mut self, error: String) {
        self.show_prompt(
            ErrorPrompt::new(
                "Editor Open Error",
                error,
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(in crate::app) fn map_editor_widget_task(
        &self,
        tab_id: u64,
        task: Task<iced_code_editor::Message>,
    ) -> Task<Message> {
        task.map(move |message| Message::Editor(EditorMessage::Widget { tab_id, message }))
    }

    pub(in crate::app) fn request_close_editor_tab(&mut self, tab_id: u64) -> Task<Message> {
        if self.editor.tab_is_modified(tab_id)
            || self.editor.tab_file_state(tab_id) == Some(EditorTabFileState::MissingOnDisk)
        {
            self.pending_editor_action = Some(PendingEditorAction::ResolveDirtyTabs {
                dirty_tab_ids: vec![tab_id],
                continuation: EditorContinuation::CloseTab(tab_id),
            });
            self.show_current_pending_editor_prompt();
            self.prompt_ok_action = None;
            return Task::none();
        }

        self.editor.close_tab(tab_id);
        self.sync_editor_file_watcher();
        self.focus_editor_text_area();
        self.persist_settings();
        Task::none()
    }

    pub(in crate::app) fn save_active_editor_tab(&mut self) -> Task<Message> {
        let Some(tab_id) = self.editor.active_tab_id() else {
            return Task::none();
        };
        self.save_editor_tab(tab_id)
    }

    pub(in crate::app) fn save_editor_tab(&mut self, tab_id: u64) -> Task<Message> {
        if self.editor.tab_path(tab_id).is_none() {
            return self.request_editor_save_as(tab_id);
        }

        match self.editor.save_to_disk(tab_id) {
            Ok((path, task)) => self.handle_saved_editor_tab(tab_id, path, task, true),
            Err(error) => self.handle_editor_save_error(error),
        }
    }

    pub(super) fn request_editor_save_as(&mut self, tab_id: u64) -> Task<Message> {
        self.pending_editor_save_as_tab = Some(tab_id);
        let suggested_name = self.editor.suggested_rename_name(tab_id);
        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_file_name(&suggested_name)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |picked| Message::Editor(EditorMessage::SaveAsPicked(picked)),
        )
    }

    pub(in crate::app) fn save_active_editor_tab_to_path(
        &mut self,
        path: PathBuf,
    ) -> Task<Message> {
        let Some(tab_id) = self.editor.active_tab_id() else {
            return Task::none();
        };
        self.save_editor_tab_to_path(tab_id, path, true)
    }

    pub(in crate::app) fn save_editor_tab_to_path(
        &mut self,
        tab_id: u64,
        path: PathBuf,
        log_save: bool,
    ) -> Task<Message> {
        match self.editor.save_to_path(tab_id, &path) {
            Ok(task) => self.handle_saved_editor_tab(tab_id, path, task, log_save),
            Err(error) => self.handle_editor_save_error(error),
        }
    }

    pub(super) fn handle_saved_editor_tab(
        &mut self,
        tab_id: u64,
        path: PathBuf,
        task: Task<iced_code_editor::Message>,
        log_save: bool,
    ) -> Task<Message> {
        self.register_editor_recent_file(&path);
        self.sync_editor_file_watcher();
        if log_save {
            self.logger.push(format!("Saved {}", path.display()));
        }
        if self.is_settings_file_path(&path) {
            self.reload_saved_settings_file(tab_id, &path);
            return self.map_editor_widget_task(tab_id, task);
        }
        if self.editor_tab_targets_main_score(tab_id) {
            self.queue_compile("Editor saved, recompiling");
            self.start_compile_if_queued();
        }
        self.persist_settings();
        self.map_editor_widget_task(tab_id, task)
    }

    pub(super) fn reload_saved_settings_file(&mut self, tab_id: u64, path: &Path) {
        match self.reload_settings_from_disk(path) {
            Ok(()) => {
                self.editor.mark_tab_saved(tab_id);
                self.logger
                    .push("Reloaded settings from settings.toml".to_string());
            }
            Err(error) => self.show_settings_error(error),
        }
    }

    pub(super) fn show_settings_error(&mut self, error: impl ToString) {
        self.show_prompt(
            ErrorPrompt::new(
                "Settings Error",
                error.to_string(),
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(super) fn handle_editor_save_error(&mut self, error: impl ToString) -> Task<Message> {
        self.pending_editor_action = None;
        self.pending_editor_save_as_tab = None;
        self.show_prompt(
            ErrorPrompt::new(
                "Editor Save Error",
                error.to_string(),
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
        Task::none()
    }

    pub(in crate::app) fn reload_editor_tab_from_disk(&mut self, tab_id: u64) -> Task<Message> {
        match self.editor.reload_tab_from_disk(tab_id) {
            Ok(task) => self.handle_reloaded_editor_tab(tab_id, task),
            Err(error) => self.handle_editor_reload_error(error),
        }
    }

    pub(super) fn handle_reloaded_editor_tab(
        &mut self,
        tab_id: u64,
        task: Task<iced_code_editor::Message>,
    ) -> Task<Message> {
        if let Some(path) = self.editor.tab_path(tab_id).map(Path::to_path_buf) {
            self.logger
                .push(format!("Reloaded editor file {}", path.display()));
            if self.is_settings_file_path(&path) {
                self.reload_saved_settings_file(tab_id, &path);
            }
        }
        self.persist_settings();
        self.map_editor_widget_task(tab_id, task)
    }

    pub(super) fn handle_editor_reload_error(&mut self, error: impl ToString) -> Task<Message> {
        self.show_prompt(
            ErrorPrompt::new(
                "Editor Reload Error",
                error.to_string(),
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
        Task::none()
    }

    pub(in crate::app) fn rename_editor_tab_to_path(
        &mut self,
        tab_id: u64,
        path: PathBuf,
    ) -> Task<Message> {
        self.rename_editor_tab_to_path_internal(tab_id, path, true)
    }

    pub(super) fn rename_editor_tab_to_path_internal(
        &mut self,
        tab_id: u64,
        path: PathBuf,
        log_rename: bool,
    ) -> Task<Message> {
        if let Err(task) = self.close_conflicting_editor_tab(tab_id, &path) {
            return task;
        }

        match self.rename_or_save_editor_tab_to_path(tab_id, &path) {
            Ok(task) => self.finish_renamed_editor_tab(tab_id, path, log_rename, task),
            Err(error) => self.show_editor_rename_error(error),
        }
    }

    pub(super) fn close_conflicting_editor_tab(
        &mut self,
        tab_id: u64,
        path: &Path,
    ) -> Result<(), Task<Message>> {
        if let Some(existing_tab_id) = self.editor.find_tab_by_path(path)
            && existing_tab_id != tab_id
        {
            if self.editor.tab_is_modified(existing_tab_id) {
                return Err(self.show_editor_rename_error(format!(
                    "The target file {} is already open and modified in another tab.",
                    path.display()
                )));
            }

            self.editor.close_tab(existing_tab_id);
            self.sync_editor_file_watcher();
        }

        Ok(())
    }

    pub(super) fn rename_or_save_editor_tab_to_path(
        &mut self,
        tab_id: u64,
        path: &Path,
    ) -> Result<Task<iced_code_editor::Message>, String> {
        if self.editor.tab_path(tab_id).is_some() {
            self.editor.rename_file(tab_id, path)
        } else {
            self.editor.save_to_path(tab_id, path)
        }
    }

    pub(super) fn finish_renamed_editor_tab(
        &mut self,
        tab_id: u64,
        path: PathBuf,
        log_rename: bool,
        task: Task<iced_code_editor::Message>,
    ) -> Task<Message> {
        self.register_editor_recent_file(&path);
        self.sync_editor_file_watcher();
        if log_rename {
            self.logger.push(format!("Renamed to {}", path.display()));
        }
        self.recompile_renamed_main_score(tab_id, &path);
        self.persist_settings();
        self.map_editor_widget_task(tab_id, task)
    }

    pub(super) fn recompile_renamed_main_score(&mut self, tab_id: u64, path: &Path) {
        if !self.editor_tab_targets_main_score(tab_id) {
            return;
        }
        if let Ok(selected_score) = selected_score_from_path(path.to_path_buf()) {
            self.current_score = Some(selected_score);
        }
        self.restart_score_watcher(path);
        self.queue_compile("Editor file renamed, recompiling");
        self.start_compile_if_queued();
    }

    pub(super) fn show_editor_rename_error(&mut self, error: String) -> Task<Message> {
        self.show_prompt(
            ErrorPrompt::new(
                "Editor Rename Error",
                error,
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
        Task::none()
    }

    pub(in crate::app) fn start_editor_tab_rename(&mut self, tab_id: u64) -> Task<Message> {
        self.clear_editor_tab_drag_state();
        if self.editor.tab_path(tab_id).is_none() && self.editor.tab_title(tab_id) == "Untitled" {
            self.editor_tab_rename_value = "untitled.ly".to_string();
        } else {
            self.editor_tab_rename_value = self.editor.suggested_rename_name(tab_id);
        }
        self.renaming_editor_tab = Some(tab_id);
        self.editor.activate_tab(tab_id);
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.close_editor_menus();
        self.editor.lose_focus();
        Task::batch([
            focus(self.editor_tab_rename_input_id.clone()),
            select_all(self.editor_tab_rename_input_id.clone()),
        ])
    }

    pub(in crate::app) fn commit_editor_tab_rename(&mut self) -> Task<Message> {
        let Some(tab_id) = self.renaming_editor_tab else {
            return Task::none();
        };

        let Some(file_name) = normalize_editor_tab_file_name(&self.editor_tab_rename_value) else {
            return Task::none();
        };

        self.cancel_editor_tab_rename_state();

        if let Some(current_path) = self.editor.tab_path(tab_id).map(Path::to_path_buf) {
            let Some(parent) = current_path.parent() else {
                return Task::none();
            };
            let target_path = parent.join(&file_name);

            if target_path == current_path {
                return Task::none();
            }

            return self.rename_editor_tab_to_path(tab_id, target_path);
        }

        self.pending_editor_rename_tab = Some(tab_id);
        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_file_name(&file_name)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |picked| Message::Editor(EditorMessage::RenamePicked(picked)),
        )
    }

    pub(in crate::app) fn cancel_editor_tab_rename_state(&mut self) {
        self.renaming_editor_tab = None;
        self.editor_tab_rename_value.clear();
        if self.focused_workspace_pane == Some(WorkspacePaneKind::Editor) {
            self.focus_editor_text_area();
        }
    }

    pub(super) fn close_editor_menus(&mut self) {
        self.open_header_overflow_menu = None;
        self.open_editor_menu_section = None;
        self.open_editor_file_menu_section = None;
    }

    pub(super) fn clear_editor_tab_drag_state(&mut self) {
        self.pressed_editor_tab = None;
        self.dragged_editor_tab = None;
        self.editor_tab_drag_origin = None;
        self.hovered_editor_tab = None;
        self.editor_tab_drop_after = false;
        self.editor_tabbar_autoscroll_direction = 0;
        self.editor_tabbar_drag_pointer_x = None;
    }

    pub(super) fn update_editor_drag_target_from_x(&mut self, x: f32) {
        let tab_ids = self.editor.tab_ids();
        if tab_ids.is_empty() {
            self.hovered_editor_tab = None;
            self.editor_tab_drop_after = false;
            return;
        }

        let effective_x = (x + self.editor_tabbar_scroll_x).max(0.0);
        let total_tabs_width = EDITOR_TAB_SLOT_WIDTH * tab_ids.len() as f32;

        if effective_x >= total_tabs_width {
            self.hovered_editor_tab = tab_ids.last().copied();
            self.editor_tab_drop_after = true;
            return;
        }

        let slot_index = crate::number::f32_to_usize((effective_x / EDITOR_TAB_SLOT_WIDTH).floor());
        let clamped_index = slot_index.min(tab_ids.len() - 1);
        let within_slot = effective_x - clamped_index as f32 * EDITOR_TAB_SLOT_WIDTH;

        self.hovered_editor_tab = tab_ids.get(clamped_index).copied();
        self.editor_tab_drop_after = within_slot >= (2.0 + EDITOR_TAB_WIDTH / 2.0);
    }

    pub(in crate::app) fn tick_editor_tabbar_autoscroll(&mut self) -> Task<Message> {
        if self.dragged_editor_tab.is_none() || self.editor_tabbar_autoscroll_direction == 0 {
            return Task::none();
        }

        let Some(pointer_x) = self.editor_tabbar_drag_pointer_x else {
            return Task::none();
        };
        self.update_editor_drag_target_from_x(pointer_x);

        let scroll_region_width = self.editor_tabbar_scroll_region_width();
        if scroll_region_width <= 0.0 {
            return Task::none();
        }

        let edge_ratio = if self.editor_tabbar_autoscroll_direction < 0 {
            ((EDITOR_TABBAR_AUTOSCROLL_EDGE - pointer_x) / EDITOR_TABBAR_AUTOSCROLL_EDGE)
                .clamp(0.0, 1.0)
        } else {
            ((pointer_x - (scroll_region_width - EDITOR_TABBAR_AUTOSCROLL_EDGE))
                / EDITOR_TABBAR_AUTOSCROLL_EDGE)
                .clamp(0.0, 1.0)
        };
        let scroll_step = EDITOR_TABBAR_AUTOSCROLL_MIN_STEP
            + (EDITOR_TABBAR_AUTOSCROLL_MAX_STEP - EDITOR_TABBAR_AUTOSCROLL_MIN_STEP) * edge_ratio;

        iced::widget::operation::scroll_by(
            super::EDITOR_TABBAR_SCROLL_ID,
            iced::widget::operation::AbsoluteOffset {
                x: scroll_step * f32::from(self.editor_tabbar_autoscroll_direction),
                y: 0.0,
            },
        )
    }

    pub(super) fn update_editor_tabbar_autoscroll(&mut self, x: f32) {
        let scroll_region_width = self.editor_tabbar_scroll_region_width();

        self.editor_tabbar_autoscroll_direction = if x <= EDITOR_TABBAR_AUTOSCROLL_EDGE {
            -1
        } else if x >= scroll_region_width - EDITOR_TABBAR_AUTOSCROLL_EDGE {
            1
        } else {
            0
        };
    }

    pub(super) fn editor_tabbar_scroll_region_width(&self) -> f32 {
        if self.editor_tabbar_viewport_width > 0.0 {
            self.editor_tabbar_viewport_width
        } else {
            (self.window_width - EDITOR_TABBAR_NEW_BUTTON_WIDTH).max(0.0)
        }
    }

    pub(in crate::app) fn editor_tab_reveal_target_x(&self, tab_id: u64) -> Option<f32> {
        let tab_ids = self.editor.tab_ids();
        let index = tab_ids.iter().position(|candidate| *candidate == tab_id)?;

        let viewport_width = self.editor_tabbar_scroll_region_width();
        if viewport_width <= 0.0 {
            return None;
        }

        let tab_start = index as f32 * EDITOR_TAB_SLOT_WIDTH;
        let tab_end = tab_start + EDITOR_TAB_SLOT_WIDTH;
        let visible_start = self.editor_tabbar_scroll_x;
        let visible_end = visible_start + viewport_width;

        if tab_start < visible_start {
            Some(tab_start.max(0.0))
        } else if tab_end > visible_end {
            Some((tab_end - viewport_width).max(0.0))
        } else {
            None
        }
    }

    pub(in crate::app) fn reveal_editor_tab(&self, tab_id: u64) -> Task<Message> {
        let Some(target_x) = self.editor_tab_reveal_target_x(tab_id) else {
            return Task::none();
        };

        iced::widget::operation::scroll_to(
            super::EDITOR_TABBAR_SCROLL_ID,
            iced::widget::operation::AbsoluteOffset {
                x: target_x.max(0.0),
                y: 0.0,
            },
        )
    }
}
