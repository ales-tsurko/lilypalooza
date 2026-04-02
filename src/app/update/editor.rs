use super::*;
use iced::widget::operation::{focus, select_all};

const EDITOR_TAB_WIDTH: f32 = 144.0;

impl Lilypalooza {
    pub(in crate::app) fn handle_editor_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        match message {
            EditorMessage::Widget { tab_id, message } => {
                if matches!(
                    message,
                    iced_code_editor::Message::CanvasFocusGained
                        | iced_code_editor::Message::MouseClick(_)
                        | iced_code_editor::Message::MouseDrag(_)
                        | iced_code_editor::Message::JumpClick(_)
                ) {
                    self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                    self.editor.activate_tab(tab_id);
                }

                let task = self.editor.update(tab_id, &message);
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::NewRequested => {
                self.close_editor_menus();
                self.cancel_editor_tab_rename_state();
                let (tab_id, task, _reused) = self.editor.new_document();
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::TabPressed(tab_id) => {
                if self.renaming_editor_tab != Some(tab_id) {
                    self.cancel_editor_tab_rename_state();
                }
                self.editor.activate_tab(tab_id);
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.pressed_editor_tab = Some(tab_id);
                self.editor_tab_drag_origin = None;
                self.dragged_editor_tab = None;
                self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id))
            }
            EditorMessage::TabMoved { tab_id, position } => {
                self.hovered_editor_tab = Some(tab_id);
                self.editor_tab_drop_after = position.x >= EDITOR_TAB_WIDTH * 0.5;
                if self.dragged_editor_tab.is_none()
                    && let Some(pressed_tab) = self.pressed_editor_tab
                {
                    match self.editor_tab_drag_origin {
                        Some(origin) if drag_distance(origin, position) >= DRAG_START_THRESHOLD => {
                            self.dragged_editor_tab = Some(pressed_tab);
                        }
                        Some(_) => {}
                        None => {
                            self.editor_tab_drag_origin = Some(position);
                        }
                    }
                }
                Task::none()
            }
            EditorMessage::TabHovered(tab_id) => {
                self.hovered_editor_tab = tab_id;
                Task::none()
            }
            EditorMessage::TabBarMoved(position) => {
                if self.dragged_editor_tab.is_some() {
                    if let Some(last_tab) = self.editor.tab_ids().last().copied() {
                        self.hovered_editor_tab = Some(last_tab);
                        self.editor_tab_drop_after = true;
                    } else {
                        self.hovered_editor_tab = None;
                    }
                }
                if self.dragged_editor_tab.is_none()
                    && let Some(pressed_tab) = self.pressed_editor_tab
                {
                    match self.editor_tab_drag_origin {
                        Some(origin) if drag_distance(origin, position) >= DRAG_START_THRESHOLD => {
                            self.dragged_editor_tab = Some(pressed_tab);
                        }
                        Some(_) => {}
                        None => {
                            self.editor_tab_drag_origin = Some(position);
                        }
                    }
                }
                Task::none()
            }
            EditorMessage::TabDragReleased => {
                if let (Some(dragged_tab), Some(target_tab)) =
                    (self.dragged_editor_tab, self.hovered_editor_tab)
                    && self
                        .editor
                        .reorder_tabs(dragged_tab, target_tab, self.editor_tab_drop_after)
                {
                    self.persist_settings();
                }
                self.clear_editor_tab_drag_state();
                Task::none()
            }
            EditorMessage::TabDragExited => {
                self.clear_editor_tab_drag_state();
                Task::none()
            }
            EditorMessage::StartRename(tab_id) => self.start_editor_tab_rename(tab_id),
            EditorMessage::RenameInputChanged(value) => {
                self.editor_tab_rename_value = value;
                Task::none()
            }
            EditorMessage::CommitRename => self.commit_editor_tab_rename(),
            EditorMessage::CancelRename => {
                self.cancel_editor_tab_rename_state();
                Task::none()
            }
            EditorMessage::CloseTabRequested(tab_id) => {
                if self.renaming_editor_tab == Some(tab_id) {
                    self.cancel_editor_tab_rename_state();
                }
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.request_close_editor_tab(tab_id)
            }
            EditorMessage::RenameRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                let Some(tab_id) = self.editor.active_tab_id() else {
                    return Task::none();
                };
                self.start_editor_tab_rename(tab_id)
            }
            EditorMessage::RenamePicked(Some(path)) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                let Some(tab_id) = self.pending_editor_rename_tab.take() else {
                    return Task::none();
                };
                self.rename_editor_tab_to_path(tab_id, path)
            }
            EditorMessage::RenamePicked(None) => {
                self.pending_editor_rename_tab = None;
                Task::none()
            }
            EditorMessage::OpenRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.close_editor_menus();
                self.cancel_editor_tab_rename_state();
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    |picked| Message::Editor(EditorMessage::OpenPicked(picked)),
                )
            }
            EditorMessage::OpenPicked(Some(path)) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_editor_file_in_editor(&path)
            }
            EditorMessage::OpenRecent(path) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.close_editor_menus();
                self.cancel_editor_tab_rename_state();
                self.open_editor_file_in_editor(&path)
            }
            EditorMessage::OpenPicked(None) => Task::none(),
            EditorMessage::SaveRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.save_active_editor_tab()
            }
            EditorMessage::SaveAsRequested => {
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
            EditorMessage::SaveAsPicked(Some(path)) => {
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
            EditorMessage::SaveAsPicked(None) => {
                self.pending_editor_save_as_tab = None;
                self.error_prompt = None;
                self.pending_editor_action = None;
                Task::none()
            }
            EditorMessage::ZoomIn => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.zoom_in();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::ZoomOut => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.zoom_out();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::ResetZoom => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.reset_zoom();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeHueOffsetDegrees(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_hue_offset_degrees(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeSaturation(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_saturation(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeWarmth(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_warmth(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeBrightness(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_brightness(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeTextDim(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_text_dim(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeCommentDim(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_comment_dim(value);
                self.persist_settings();
                Task::none()
            }
        }
    }

    pub(in crate::app) fn editor_tab_targets_main_score(&self, tab_id: u64) -> bool {
        self.editor.tab_path(tab_id)
            == self
                .current_score
                .as_ref()
                .map(|score| score.path.as_path())
    }

    pub(in crate::app) fn open_editor_file_in_editor(&mut self, path: &Path) -> Task<Message> {
        self.cancel_editor_tab_rename_state();
        match self.editor.load_file(path) {
            Ok((tab_id, task, reused_existing)) => {
                self.register_editor_recent_file(path);
                if reused_existing {
                    self.logger
                        .push(format!("Activated editor file {}", path.display()));
                } else {
                    self.logger
                        .push(format!("Opened editor file {}", path.display()));
                }
                self.editor.request_focus();
                let sync_task = self.editor.sync_tab_scroll_state(tab_id);
                self.map_editor_widget_task(tab_id, iced::Task::batch([task, sync_task]))
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

    pub(in crate::app) fn map_editor_widget_task(
        &self,
        tab_id: u64,
        task: Task<iced_code_editor::Message>,
    ) -> Task<Message> {
        task.map(move |message| Message::Editor(EditorMessage::Widget { tab_id, message }))
    }

    pub(in crate::app) fn request_close_editor_tab(&mut self, tab_id: u64) -> Task<Message> {
        if self.editor.tab_is_modified(tab_id) {
            let title = self.editor.tab_title(tab_id);
            self.pending_editor_action = Some(PendingEditorAction::ResolveDirtyTabs {
                dirty_tab_ids: vec![tab_id],
                continuation: EditorContinuation::CloseTab(tab_id),
            });
            self.error_prompt = Some(ErrorPrompt::new(
                format!("Close {title}?"),
                format!("Save changes to {title} before closing it?"),
                ErrorFatality::Recoverable,
                PromptButtons::SaveDiscardCancel,
            ));
            self.prompt_ok_action = None;
            return Task::none();
        }

        self.editor.close_tab(tab_id);
        self.editor.request_focus();
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
            self.pending_editor_save_as_tab = Some(tab_id);
            let suggested_name = self.editor.suggested_rename_name(tab_id);
            return Task::perform(
                async move {
                    rfd::AsyncFileDialog::new()
                        .set_file_name(&suggested_name)
                        .save_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |picked| Message::Editor(EditorMessage::SaveAsPicked(picked)),
            );
        }

        match self.editor.save_to_disk(tab_id) {
            Ok((path, task)) => {
                self.register_editor_recent_file(&path);
                self.logger.push(format!("Saved {}", path.display()));
                if self.editor_tab_targets_main_score(tab_id) {
                    self.queue_compile("Editor saved, recompiling");
                    self.start_compile_if_queued();
                }
                self.persist_settings();
                self.map_editor_widget_task(tab_id, task)
            }
            Err(error) => {
                self.pending_editor_action = None;
                self.pending_editor_save_as_tab = None;
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Save Error",
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
            Ok(task) => {
                self.register_editor_recent_file(&path);
                if log_save {
                    self.logger.push(format!("Saved {}", path.display()));
                }
                if self.editor_tab_targets_main_score(tab_id) {
                    self.queue_compile("Editor saved, recompiling");
                    self.start_compile_if_queued();
                }
                self.persist_settings();
                self.map_editor_widget_task(tab_id, task)
            }
            Err(error) => {
                self.pending_editor_action = None;
                self.pending_editor_save_as_tab = None;
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Save Error",
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

    pub(in crate::app) fn rename_editor_tab_to_path(
        &mut self,
        tab_id: u64,
        path: PathBuf,
    ) -> Task<Message> {
        if let Some(existing_tab_id) = self.editor.find_tab_by_path(&path)
            && existing_tab_id != tab_id
        {
            if self.editor.tab_is_modified(existing_tab_id) {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Rename Error",
                        format!(
                            "The target file {} is already open and modified in another tab.",
                            path.display()
                        ),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                return Task::none();
            }

            self.editor.close_tab(existing_tab_id);
        }

        let result = if self.editor.tab_path(tab_id).is_some() {
            self.editor.rename_file(tab_id, &path)
        } else {
            self.editor.save_to_path(tab_id, &path)
        };

        match result {
            Ok(task) => {
                self.register_editor_recent_file(&path);
                self.logger.push(format!("Renamed to {}", path.display()));
                if self.editor_tab_targets_main_score(tab_id) {
                    if let Ok(selected_score) = selected_score_from_path(path.clone()) {
                        self.current_score = Some(selected_score);
                    }
                    self.restart_score_watcher(&path);
                    self.queue_compile("Editor file renamed, recompiling");
                    self.start_compile_if_queued();
                }
                self.persist_settings();
                self.map_editor_widget_task(tab_id, task)
            }
            Err(error) => {
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
        }
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
            self.editor.request_focus();
        }
    }

    fn close_editor_menus(&mut self) {
        self.open_header_overflow_menu = None;
        self.open_editor_menu_section = None;
        self.open_editor_file_menu_section = None;
    }

    fn clear_editor_tab_drag_state(&mut self) {
        self.pressed_editor_tab = None;
        self.dragged_editor_tab = None;
        self.editor_tab_drag_origin = None;
        self.hovered_editor_tab = None;
        self.editor_tab_drop_after = false;
    }
}

fn normalize_editor_tab_file_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}
