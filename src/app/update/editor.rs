use super::*;
use crate::app::editor::EditorTabFileState;
use iced::widget::operation::{focus, select_all};

const EDITOR_TAB_WIDTH: f32 = 144.0;
const EDITOR_TAB_SLOT_WIDTH: f32 = EDITOR_TAB_WIDTH + 4.0;
const EDITOR_TABBAR_AUTOSCROLL_EDGE: f32 = 32.0;
const EDITOR_TABBAR_AUTOSCROLL_MIN_STEP: f32 = 3.0;
const EDITOR_TABBAR_AUTOSCROLL_MAX_STEP: f32 = 12.0;
const EDITOR_TABBAR_NEW_BUTTON_WIDTH: f32 = 36.0;

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
            EditorMessage::ActiveWidgetMessage(message) => {
                let Some(tab_id) = self.editor.active_tab_id() else {
                    return Task::none();
                };
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.activate_tab(tab_id);
                let task = self.editor.update(tab_id, &message);
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::NewRequested => {
                self.close_editor_menus();
                self.cancel_editor_tab_rename_state();
                let (tab_id, task, _reused) = self.editor.new_document();
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.pending_reveal_editor_tab = Some(tab_id);
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::TabPressed(tab_id) => {
                if self.renaming_editor_tab != Some(tab_id) {
                    self.cancel_editor_tab_rename_state();
                }
                self.editor.activate_tab(tab_id);
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.pending_reveal_editor_tab = Some(tab_id);
                self.pressed_editor_tab = Some(tab_id);
                self.editor_tab_drag_origin = None;
                self.dragged_editor_tab = None;
                self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id))
            }
            EditorMessage::TabMoved { tab_id, position } => {
                self.hovered_editor_tab = Some(tab_id);
                self.editor_tab_drop_after = position.x >= EDITOR_TAB_WIDTH * 0.5;
                if self.dragged_editor_tab.is_some() {
                    self.editor_tabbar_drag_pointer_x = Some(position.x);
                    self.update_editor_tabbar_autoscroll(position.x);
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
            EditorMessage::TabGlobalMoved(position) => {
                if self.dragged_editor_tab.is_some() {
                    self.editor_tabbar_drag_pointer_x = Some(position.x);
                    self.update_editor_tabbar_autoscroll(position.x);
                    self.update_editor_drag_target_from_x(position.x);
                }
                Task::none()
            }
            EditorMessage::TabHovered(tab_id) => {
                self.hovered_editor_tab = tab_id;
                Task::none()
            }
            EditorMessage::TabBarScrolled(viewport) => {
                self.editor_tabbar_scroll_x = viewport.absolute_offset().x;
                self.editor_tabbar_viewport_width = viewport.bounds().width;
                if self.dragged_editor_tab.is_some()
                    && let Some(pointer_x) = self.editor_tabbar_drag_pointer_x
                {
                    self.update_editor_drag_target_from_x(pointer_x);
                }
                Task::none()
            }
            EditorMessage::TabBarEmptyMoved => {
                if let Some(last_tab) = self.editor.tab_ids().last().copied() {
                    self.hovered_editor_tab = Some(last_tab);
                    self.editor_tab_drop_after = true;
                } else {
                    self.hovered_editor_tab = None;
                }
                Task::none()
            }
            EditorMessage::TabBarMoved(position) => {
                if self.dragged_editor_tab.is_some() {
                    self.editor_tabbar_drag_pointer_x = Some(position.x);
                    self.update_editor_tabbar_autoscroll(position.x);
                    self.update_editor_drag_target_from_x(position.x);
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
                if self.dragged_editor_tab.is_none() {
                    self.hovered_editor_tab = None;
                }
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
            EditorMessage::OpenPicked(Some(paths)) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_editor_files_in_editor(&paths)
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
                self.sync_editor_file_watcher();
                if reused_existing {
                    self.logger
                        .push(format!("Activated editor file {}", path.display()));
                } else {
                    self.logger
                        .push(format!("Opened editor file {}", path.display()));
                }
                self.editor.request_focus();
                let sync_task = self.editor.sync_tab_scroll_state(tab_id);
                self.pending_reveal_editor_tab = Some(tab_id);
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

    pub(in crate::app) fn open_editor_files_in_editor(
        &mut self,
        paths: &[PathBuf],
    ) -> Task<Message> {
        self.cancel_editor_tab_rename_state();

        let mut tasks = Vec::new();
        let mut opened_any = false;
        let mut last_tab_id = None;

        for path in paths {
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
                    tasks.push(self.map_editor_widget_task(
                        tab_id,
                        iced::Task::batch([task, self.editor.sync_tab_scroll_state(tab_id)]),
                    ));
                    last_tab_id = Some(tab_id);
                    opened_any = true;
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
                    break;
                }
            }
        }

        if opened_any {
            self.editor.request_focus();
        }
        if let Some(tab_id) = last_tab_id {
            self.pending_reveal_editor_tab = Some(tab_id);
        }

        Task::batch(tasks)
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
                self.sync_editor_file_watcher();
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
                self.sync_editor_file_watcher();
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

    pub(in crate::app) fn reload_editor_tab_from_disk(&mut self, tab_id: u64) -> Task<Message> {
        match self.editor.reload_tab_from_disk(tab_id) {
            Ok(task) => {
                if let Some(path) = self.editor.tab_path(tab_id) {
                    self.logger
                        .push(format!("Reloaded editor file {}", path.display()));
                }
                self.persist_settings();
                self.map_editor_widget_task(tab_id, task)
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
            self.sync_editor_file_watcher();
        }

        let result = if self.editor.tab_path(tab_id).is_some() {
            self.editor.rename_file(tab_id, &path)
        } else {
            self.editor.save_to_path(tab_id, &path)
        };

        match result {
            Ok(task) => {
                self.register_editor_recent_file(&path);
                self.sync_editor_file_watcher();
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
        self.editor_tabbar_autoscroll_direction = 0;
        self.editor_tabbar_drag_pointer_x = None;
    }

    fn update_editor_drag_target_from_x(&mut self, x: f32) {
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

        let slot_index = (effective_x / EDITOR_TAB_SLOT_WIDTH).floor() as usize;
        let clamped_index = slot_index.min(tab_ids.len() - 1);
        let within_slot = effective_x - clamped_index as f32 * EDITOR_TAB_SLOT_WIDTH;

        self.hovered_editor_tab = Some(tab_ids[clamped_index]);
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

    fn update_editor_tabbar_autoscroll(&mut self, x: f32) {
        let scroll_region_width = self.editor_tabbar_scroll_region_width();

        self.editor_tabbar_autoscroll_direction = if x <= EDITOR_TABBAR_AUTOSCROLL_EDGE {
            -1
        } else if x >= scroll_region_width - EDITOR_TABBAR_AUTOSCROLL_EDGE {
            1
        } else {
            0
        };
    }

    fn editor_tabbar_scroll_region_width(&self) -> f32 {
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
