use super::*;

impl Lilypalooza {
    pub(in crate::app) fn handle_pending_prompt_discard(&mut self) -> Task<Message> {
        match self.pending_editor_action.take() {
            Some(PendingEditorAction::ResolveDirtyProject { continuation }) => {
                self.error_prompt = None;
                self.prompt_selected_button = PromptSelectedButton::Ok;
                self.continue_editor_continuation_without_project_dirty_check(continuation)
            }
            Some(action) => {
                self.pending_editor_action = Some(action);
                self.advance_pending_editor_action()
            }
            None => Task::none(),
        }
    }

    pub(in crate::app) fn advance_pending_editor_action(&mut self) -> Task<Message> {
        if self.pending_action_still_needs_prompt() {
            self.show_current_pending_editor_prompt();
            return Task::none();
        }

        let Some(continuation) = self.take_pending_editor_continuation() else {
            return Task::none();
        };

        self.error_prompt = None;
        self.prompt_selected_button = PromptSelectedButton::Ok;
        self.continue_editor_continuation(continuation)
    }

    pub(super) fn pending_action_still_needs_prompt(&mut self) -> bool {
        let Some(PendingEditorAction::ResolveDirtyTabs { dirty_tab_ids, .. }) =
            self.pending_editor_action.as_mut()
        else {
            return false;
        };
        if !dirty_tab_ids.is_empty() {
            dirty_tab_ids.remove(0);
        }
        !dirty_tab_ids.is_empty()
    }

    pub(super) fn take_pending_editor_continuation(&mut self) -> Option<EditorContinuation> {
        match self.pending_editor_action.take() {
            Some(PendingEditorAction::ResolveDirtyTabs { continuation, .. })
            | Some(PendingEditorAction::ResolveDirtyProject { continuation }) => Some(continuation),
            None => None,
        }
    }

    pub(in crate::app) fn continue_editor_continuation(
        &mut self,
        continuation: EditorContinuation,
    ) -> Task<Message> {
        match continuation {
            EditorContinuation::CloseTab(tab_id) => self.continue_close_tab(tab_id),
            EditorContinuation::LoadProject(project_root) => {
                self.continue_load_project(project_root)
            }
            EditorContinuation::OpenScore(path) => self.continue_open_score(path),
            EditorContinuation::ExitApp => self.continue_exit_app(),
        }
    }

    pub(super) fn continue_close_tab(&mut self, tab_id: u64) -> Task<Message> {
        self.editor.close_tab(tab_id);
        self.sync_editor_file_watcher();
        self.focus_editor_text_area();
        self.persist_settings();
        Task::none()
    }

    pub(super) fn continue_load_project(&mut self, project_root: PathBuf) -> Task<Message> {
        if self.project_root.as_ref() != Some(&project_root) && self.project_is_dirty() {
            return self
                .begin_pending_project_action(EditorContinuation::LoadProject(project_root));
        }
        self.load_project_from_root(project_root)
    }

    pub(super) fn continue_open_score(&mut self, path: PathBuf) -> Task<Message> {
        let next_project_root = state::find_project_root(&path);
        if next_project_root != self.project_root && self.project_is_dirty() {
            return self.begin_pending_project_action(EditorContinuation::OpenScore(path));
        }
        self.open_score_continuation(path)
    }

    pub(super) fn continue_exit_app(&mut self) -> Task<Message> {
        if self.project_is_dirty() {
            return self.begin_pending_project_action(EditorContinuation::ExitApp);
        }
        self.exit_app()
    }

    pub(super) fn continue_editor_continuation_without_project_dirty_check(
        &mut self,
        continuation: EditorContinuation,
    ) -> Task<Message> {
        match continuation {
            EditorContinuation::CloseTab(tab_id) => self.continue_close_tab(tab_id),
            EditorContinuation::LoadProject(project_root) => {
                self.load_project_from_root(project_root)
            }
            EditorContinuation::OpenScore(path) => self.open_score_continuation(path),
            EditorContinuation::ExitApp => self.exit_app(),
        }
    }

    pub(super) fn open_score_continuation(&mut self, path: PathBuf) -> Task<Message> {
        match selected_score_from_path(path) {
            Ok(selected_score) => {
                self.attach_persistence_context_for_score(&selected_score.path);
                self.activate_score(selected_score)
            }
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
                Task::none()
            }
        }
    }

    pub(in crate::app) fn exit_app(&mut self) -> Task<Message> {
        Task::batch([self.destroy_all_editor_windows(), iced::exit()])
    }

    pub(super) fn open_settings_file_in_editor(&mut self) -> Task<Message> {
        let path = match settings::path() {
            Ok(path) => path,
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Settings Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                return Task::none();
            }
        };

        if !path.exists() {
            let (clap_search_paths, vst3_search_paths) =
                settings::split_plugin_search_paths(&self.plugin_search_paths);
            let settings = settings::AppSettings {
                editor_view: self.editor.view_settings(),
                editor_theme: self.editor.theme_settings(),
                editor_recent_files_limit: self.editor_recent_files_limit,
                playback: self.playback_settings.clone(),
                clap_search_paths,
                vst3_search_paths,
                shortcuts: self.shortcut_settings.clone(),
            };

            if let Err(error) = settings::save(&settings) {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Settings Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                return Task::none();
            }
        }

        let _pane_is_visible = self.ensure_workspace_pane_visible(WorkspacePaneKind::Editor);
        let task = self.open_editor_file_in_editor_internal(&path, false, true);
        if let Some(tab_id) = self.editor.find_tab_by_path(&path) {
            self.editor.activate_tab(tab_id);
            self.pending_reveal_editor_tab = Some(tab_id);
        }
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_text_area();
        task
    }

    pub(in crate::app) fn is_settings_file_path(&self, path: &Path) -> bool {
        settings::path()
            .map(|settings_path| {
                state::normalize_path(&settings_path) == state::normalize_path(path)
            })
            .unwrap_or(false)
    }

    pub(in crate::app) fn reload_settings_from_disk(&mut self, path: &Path) -> Result<(), String> {
        let loaded = settings::load_from_path(path)?;
        let previous_playback = self.playback_settings.clone();
        self.editor.apply_view_settings(loaded.editor_view);
        self.editor.apply_theme_settings(loaded.editor_theme);
        self.editor_recent_files_limit = loaded.editor_recent_files_limit.max(1);
        self.editor_recent_files
            .truncate(self.editor_recent_files_limit);
        self.playback_settings = loaded.playback.clone();
        self.apply_reloaded_playback_settings(&loaded.playback, &previous_playback);
        self.shortcut_settings = loaded.shortcuts;
        self.sync_editor_viewport_from_layout();
        self.sync_editor_widget_focus();
        Ok(())
    }

    pub(super) fn apply_reloaded_playback_settings(
        &mut self,
        loaded: &PlaybackSettings,
        previous: &PlaybackSettings,
    ) {
        if playback_engine_settings_changed(loaded, previous) {
            self.restart_playback_engine();
        } else if loaded.soundfonts.is_empty() {
            self.soundfont_status = SoundfontStatus::NotSelected;
            self.unload_playback_file();
        } else {
            self.initialize_playback_soundfonts(loaded.soundfonts.clone());
        }
    }

    pub(super) fn handle_editor_file_browser_move(&mut self, delta: i32) -> Task<Message> {
        if let Err(error) = self.editor.move_file_browser_selection(delta) {
            self.show_prompt(
                ErrorPrompt::new(
                    "File Browser Error",
                    error,
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
            return Task::none();
        }
        self.reveal_editor_file_browser_selection(
            self.editor
                .file_browser_has_preview_column(self.editor.file_browser_active_column_index()),
        )
    }

    pub(super) fn handle_editor_file_browser_column(&mut self, right: bool) -> Task<Message> {
        if let Err(error) = self.editor.move_file_browser_column(right) {
            self.show_prompt(
                ErrorPrompt::new(
                    "File Browser Error",
                    error,
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
            return Task::none();
        }

        self.reveal_editor_file_browser_selection(right)
    }

    pub(in crate::app) fn reveal_editor_file_browser_selection(
        &self,
        reveal_preview: bool,
    ) -> Task<Message> {
        let horizontal = self.reveal_editor_file_browser_columns(reveal_preview);
        let column_index = self.editor.file_browser_active_column_index();
        let vertical = self
            .editor
            .file_browser_selected_index(column_index)
            .map(|selected_index| {
                let current_y = self
                    .editor_file_browser_column_scroll_y
                    .get(&column_index)
                    .copied()
                    .unwrap_or(0.0);
                let viewport_height = self
                    .editor_file_browser_column_viewport_height
                    .get(&column_index)
                    .copied()
                    .filter(|height| *height > 0.0)
                    .unwrap_or(super::EDITOR_FILE_BROWSER_HEIGHT);
                let row_top = selected_index as f32 * super::EDITOR_FILE_BROWSER_ENTRY_HEIGHT;
                let row_bottom = row_top + super::EDITOR_FILE_BROWSER_ENTRY_HEIGHT;
                let target_y = if row_top < current_y {
                    row_top
                } else if row_bottom > current_y + viewport_height {
                    (row_bottom - viewport_height).max(0.0)
                } else {
                    current_y
                };
                iced::widget::operation::scroll_to(
                    crate::app::editor_file_browser_column_scroll_id(column_index),
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: target_y,
                    },
                )
            })
            .unwrap_or_else(Task::none);

        Task::batch([horizontal, vertical])
    }

    pub(in crate::app) fn reveal_editor_file_browser_columns(
        &self,
        reveal_preview: bool,
    ) -> Task<Message> {
        let column_index = self.editor.file_browser_active_column_index();
        let current_x = self.editor_file_browser_scroll_x;
        let viewport_width = if self.editor_file_browser_viewport_width > 0.0 {
            self.editor_file_browser_viewport_width
        } else {
            super::EDITOR_FILE_BROWSER_COLUMN_WIDTH
        };
        let reveal_column_index = self.reveal_editor_file_browser_column_index(reveal_preview);
        let target_x = editor_file_browser_reveal_scroll_x(
            column_index,
            reveal_column_index,
            current_x,
            viewport_width,
        );
        iced::widget::operation::scroll_to(
            super::EDITOR_FILE_BROWSER_SCROLL_ID,
            iced::widget::operation::AbsoluteOffset {
                x: target_x,
                y: 0.0,
            },
        )
    }

    pub(super) fn reveal_editor_file_browser_column_index(&self, reveal_preview: bool) -> usize {
        let column_index = self.editor.file_browser_active_column_index();
        let visible_column_count = self.editor.file_browser_columns().len();
        if reveal_preview && visible_column_count > column_index + 1 {
            column_index + 1
        } else {
            column_index
        }
    }
}
