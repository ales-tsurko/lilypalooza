use super::*;
use crate::app::editor::EditorTabFileState;
use iced::widget::operation::{focus, select_all};
use std::fs;
use std::path::{Path, PathBuf};

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
                    self.focus_editor_text_area();
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
                self.focus_editor_text_area();
                self.editor.activate_tab(tab_id);
                let task = self.editor.update(tab_id, &message);
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::NewRequested => {
                self.close_editor_menus();
                self.cancel_editor_tab_rename_state();
                let (tab_id, task, _reused) = self.editor.new_document();
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_text_area();
                self.pending_reveal_editor_tab = Some(tab_id);
                self.map_editor_widget_task(tab_id, task)
            }
            EditorMessage::TabPressed(tab_id) => {
                if self.renaming_editor_tab != Some(tab_id) {
                    self.cancel_editor_tab_rename_state();
                }
                self.editor.activate_tab(tab_id);
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_text_area();
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
                if self.shortcut_modifier_active() {
                    return Task::none();
                }
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
            EditorMessage::ToggleFileBrowser => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.clear_browser_drag_state();
                self.editor.toggle_file_browser();
                self.sync_browser_file_watcher();
                if self.editor.file_browser_expanded() {
                    self.focus_editor_file_browser();
                } else {
                    self.focus_editor_text_area();
                }
                Task::none()
            }
            EditorMessage::FileBrowserFocused => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.clear_browser_drag_state();
                self.focus_editor_file_browser();
                if self.browser_inline_edit.is_some() {
                    focus(self.browser_inline_edit_input_id.clone())
                } else {
                    Task::none()
                }
            }
            EditorMessage::FileBrowserToggleHiddenRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                match self.editor.toggle_file_browser_show_hidden() {
                    Ok(()) => self.reveal_editor_file_browser_selection(
                        self.editor.file_browser_has_preview_column(
                            self.editor.file_browser_active_column_index(),
                        ),
                    ),
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "File Browser Error",
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
            EditorMessage::FileBrowserCutRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.copy_or_cut_file_browser_selection(BrowserClipboardKind::Cut)
            }
            EditorMessage::FileBrowserCopyRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.copy_or_cut_file_browser_selection(BrowserClipboardKind::Copy)
            }
            EditorMessage::FileBrowserPasteRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.paste_file_browser_clipboard()
            }
            EditorMessage::FileBrowserNewFileRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.start_browser_inline_create(false)
            }
            EditorMessage::FileBrowserNewDirectoryRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.start_browser_inline_create(true)
            }
            EditorMessage::FileBrowserRenameRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.start_browser_inline_rename()
            }
            EditorMessage::FileBrowserInlineEditChanged(value) => {
                if self.shortcut_modifier_active() {
                    return Task::none();
                }
                self.browser_inline_edit_value = value;
                Task::none()
            }
            EditorMessage::CommitFileBrowserInlineEdit => self.commit_browser_inline_edit(),
            EditorMessage::CancelFileBrowserInlineEdit => {
                self.clear_browser_drag_state();
                self.cancel_browser_inline_edit_state();
                Task::none()
            }
            EditorMessage::FileBrowserTrashRequested => {
                let Some(path) = self.editor.selected_file_browser_path() else {
                    return Task::none();
                };
                let title = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("item")
                    .to_string();
                self.show_prompt(
                    ErrorPrompt::new(
                        format!("Delete {title}?"),
                        format!("{title} will be deleted immediately."),
                        ErrorFatality::Recoverable,
                        PromptButtons::OkCancel,
                    ),
                    Some(PromptOkAction::DeleteBrowserPath(path)),
                );
                Task::none()
            }
            EditorMessage::FileBrowserScrolled(viewport) => {
                self.editor_file_browser_scroll_x = viewport.absolute_offset().x;
                self.editor_file_browser_viewport_width = viewport.bounds().width;
                Task::none()
            }
            EditorMessage::FileBrowserColumnScrolled {
                column_index,
                viewport,
            } => {
                self.editor_file_browser_column_scroll_y
                    .insert(column_index, viewport.absolute_offset().y);
                self.editor_file_browser_column_viewport_height
                    .insert(column_index, viewport.bounds().height);
                Task::none()
            }
            EditorMessage::FileBrowserEntryPressed {
                column_index,
                path,
                is_dir,
            } => {
                self.cancel_browser_inline_edit_state();
                self.begin_browser_entry_press(path.clone(), is_dir);
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.editor.set_file_browser_active_column(column_index);
                match self.editor.browse_to_path(column_index, &path, is_dir) {
                    Ok(()) => self.reveal_editor_file_browser_selection(true),
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "File Browser Error",
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
            EditorMessage::FileBrowserEntryHovered {
                column_index,
                path,
                is_dir,
            } => {
                self.update_browser_drop_target(Some((column_index, path, is_dir)));
                Task::none()
            }
            EditorMessage::FileBrowserEntryDragReleased { path, is_dir } => {
                let task = if is_dir {
                    self.move_dragged_browser_path_to_directory(&path)
                } else {
                    Task::none()
                };
                self.clear_browser_drag_state();
                task
            }
            EditorMessage::FileBrowserEntryDoublePressed {
                column_index,
                path,
                is_dir,
            } => {
                self.cancel_browser_inline_edit_state();
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.editor.set_file_browser_active_column(column_index);
                match self.editor.browse_to_path(column_index, &path, is_dir) {
                    Ok(()) if is_dir => self.reveal_editor_file_browser_selection(true),
                    Ok(()) => Task::batch([
                        self.reveal_editor_file_browser_selection(true),
                        self.open_editor_file_in_editor_internal(&path, false, false),
                    ]),
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "File Browser Error",
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
            EditorMessage::FileBrowserDragMoved(position) => {
                self.handle_file_browser_drag_move(position)
            }
            EditorMessage::FileBrowserDragReleased => {
                let task = if let Some(target_dir) = self
                    .browser_drop_target
                    .as_ref()
                    .map(|target| target.target_dir.clone())
                {
                    self.move_dragged_browser_path_to_directory(&target_dir)
                } else {
                    Task::none()
                };
                self.clear_browser_drag_state();
                task
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
                self.prompt_selected_button = crate::error_prompt::PromptSelectedButton::Ok;
                self.pending_editor_action = None;
                Task::none()
            }
            EditorMessage::SetCenterCursor(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_center_cursor(value);
                self.persist_settings();
                if let Some(tab_id) = self.editor.active_tab_id() {
                    let task = self.editor.sync_tab_scroll_state(tab_id);
                    return self.map_editor_widget_task(tab_id, task);
                }
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
        self.open_editor_file_in_editor_internal(path, true, true)
    }

    fn open_editor_file_in_editor_internal(
        &mut self,
        path: &Path,
        log_open: bool,
        focus_text_area: bool,
    ) -> Task<Message> {
        self.cancel_editor_tab_rename_state();
        match self.editor.load_file(path) {
            Ok((tab_id, task, reused_existing)) => {
                self.register_editor_recent_file(path);
                self.sync_editor_file_watcher();
                if log_open {
                    if reused_existing {
                        self.logger
                            .push(format!("Activated editor file {}", path.display()));
                    } else {
                        self.logger
                            .push(format!("Opened editor file {}", path.display()));
                    }
                }
                if focus_text_area {
                    self.focus_editor_text_area();
                }
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
            self.focus_editor_text_area();
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
                if self.is_settings_file_path(&path) {
                    match self.reload_settings_from_disk(&path) {
                        Ok(()) => {
                            self.editor.mark_tab_saved(tab_id);
                            self.logger
                                .push("Reloaded settings from settings.toml".to_string());
                        }
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
                        }
                    }
                    return self.map_editor_widget_task(tab_id, task);
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
                if self.is_settings_file_path(&path) {
                    match self.reload_settings_from_disk(&path) {
                        Ok(()) => {
                            self.editor.mark_tab_saved(tab_id);
                            self.logger
                                .push("Reloaded settings from settings.toml".to_string());
                        }
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
                        }
                    }
                    return self.map_editor_widget_task(tab_id, task);
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
                if let Some(path) = self.editor.tab_path(tab_id).map(Path::to_path_buf) {
                    self.logger
                        .push(format!("Reloaded editor file {}", path.display()));
                    if self.is_settings_file_path(&path) {
                        match self.reload_settings_from_disk(&path) {
                            Ok(()) => {
                                self.editor.mark_tab_saved(tab_id);
                                self.logger
                                    .push("Reloaded settings from settings.toml".to_string());
                            }
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
                            }
                        }
                    }
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
        self.rename_editor_tab_to_path_internal(tab_id, path, true)
    }

    fn rename_editor_tab_to_path_internal(
        &mut self,
        tab_id: u64,
        path: PathBuf,
        log_rename: bool,
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
                if log_rename {
                    self.logger.push(format!("Renamed to {}", path.display()));
                }
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
            self.focus_editor_text_area();
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

    fn start_browser_inline_create(&mut self, directory: bool) -> Task<Message> {
        let Some(column_index) = self.editor.current_file_browser_directory_column_index() else {
            return Task::none();
        };
        let parent_dir = self.editor.current_file_browser_directory_path();
        self.browser_inline_edit = Some(BrowserInlineEdit {
            column_index,
            parent_dir,
            target_path: None,
            kind: if directory {
                BrowserInlineEditKind::NewDirectory
            } else {
                BrowserInlineEditKind::NewFile
            },
        });
        self.browser_inline_edit_value = if directory {
            "New Folder".to_string()
        } else {
            "untitled".to_string()
        };
        self.editor.set_file_browser_active_column(column_index);
        self.focus_editor_file_browser();
        Task::batch([
            iced::widget::operation::scroll_to(
                super::editor_file_browser_column_scroll_id(column_index),
                iced::widget::operation::AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
            focus(self.browser_inline_edit_input_id.clone()),
            select_all(self.browser_inline_edit_input_id.clone()),
        ])
    }

    fn start_browser_inline_rename(&mut self) -> Task<Message> {
        let Some(target_path) = self.editor.selected_file_browser_path() else {
            return Task::none();
        };
        let Some(parent_dir) = target_path.parent().map(PathBuf::from) else {
            return Task::none();
        };
        let Some(column_index) = self.editor.current_file_browser_directory_column_index() else {
            return Task::none();
        };
        self.browser_inline_edit = Some(BrowserInlineEdit {
            column_index,
            parent_dir,
            target_path: Some(target_path.clone()),
            kind: BrowserInlineEditKind::Rename,
        });
        self.browser_inline_edit_value = target_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled")
            .to_string();
        self.focus_editor_file_browser();
        Task::batch([
            focus(self.browser_inline_edit_input_id.clone()),
            select_all(self.browser_inline_edit_input_id.clone()),
        ])
    }

    fn copy_or_cut_file_browser_selection(&mut self, kind: BrowserClipboardKind) -> Task<Message> {
        let Some(path) = self.editor.selected_file_browser_path() else {
            return Task::none();
        };
        self.browser_clipboard = Some(BrowserClipboard { path, kind });
        Task::none()
    }

    fn paste_file_browser_clipboard(&mut self) -> Task<Message> {
        let Some(clipboard) = self.browser_clipboard.clone() else {
            return Task::none();
        };
        let Some(file_name) = clipboard.path.file_name() else {
            return Task::none();
        };

        let target_dir = self
            .editor
            .selected_file_browser_path()
            .filter(|path| path.is_dir())
            .unwrap_or_else(|| self.editor.current_file_browser_directory_path());
        let destination = target_dir.join(file_name);

        if destination == clipboard.path {
            return Task::none();
        }
        if clipboard.path.is_dir() && destination.starts_with(&clipboard.path) {
            self.show_prompt(
                ErrorPrompt::new(
                    "File Browser Error",
                    "Cannot paste a folder into itself.".to_string(),
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
            return Task::none();
        }

        match clipboard.kind {
            BrowserClipboardKind::Copy => match copy_browser_path(&clipboard.path, &destination) {
                Ok(()) => {
                    let column_index = self
                        .editor
                        .current_file_browser_directory_column_index()
                        .unwrap_or(0);
                    self.finish_browser_path_transfer(
                        column_index,
                        destination.clone(),
                        Task::none(),
                        Some(BrowserHistoryEntry::Create {
                            path: destination,
                            stash_path: None,
                        }),
                    )
                }
                Err(error) => {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "File Browser Error",
                            error,
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                    Task::none()
                }
            },
            BrowserClipboardKind::Cut => {
                match self.move_browser_path(&clipboard.path, &destination, true) {
                    Ok(task) => {
                        self.browser_clipboard = None;
                        task
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "File Browser Error",
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
        }
    }

    fn begin_browser_entry_press(&mut self, path: PathBuf, is_dir: bool) {
        self.browser_pressed_entry = Some(BrowserPressedEntry {
            path,
            is_dir,
            origin: self.editor_file_browser_cursor.unwrap_or(Point::ORIGIN),
        });
        self.browser_drag_state = None;
        self.browser_drop_target = None;
    }

    fn handle_file_browser_drag_move(&mut self, position: iced::Point) -> Task<Message> {
        self.editor_file_browser_cursor = Some(position);
        if self.browser_inline_edit.is_some() {
            return Task::none();
        }
        if self.browser_drag_state.is_some() {
            return Task::none();
        }
        let Some(pressed) = self.browser_pressed_entry.as_ref() else {
            return Task::none();
        };
        if drag_distance(pressed.origin, position) < DRAG_START_THRESHOLD {
            return Task::none();
        }
        self.browser_drag_state = Some(BrowserDragState {
            source_path: pressed.path.clone(),
            source_is_dir: pressed.is_dir,
        });
        Task::none()
    }

    fn update_browser_drop_target(&mut self, hovered: Option<(usize, PathBuf, bool)>) {
        let Some(drag) = self.browser_drag_state.as_ref() else {
            self.browser_drop_target = None;
            return;
        };
        let Some((column_index, path, is_dir)) = hovered else {
            self.browser_drop_target = None;
            return;
        };
        if !is_dir || path == drag.source_path {
            self.browser_drop_target = None;
            return;
        }
        if drag.source_is_dir && path.starts_with(&drag.source_path) {
            self.browser_drop_target = None;
            return;
        }
        self.browser_drop_target = Some(BrowserDropTarget {
            column_index,
            path: path.clone(),
            target_dir: path,
        });
    }

    fn clear_browser_drag_state(&mut self) {
        self.browser_pressed_entry = None;
        self.browser_drag_state = None;
        self.browser_drop_target = None;
    }

    fn move_dragged_browser_path_to_directory(&mut self, target_dir: &Path) -> Task<Message> {
        let Some(drag) = self.browser_drag_state.clone() else {
            return Task::none();
        };
        if let Some(drop_target) = self.browser_drop_target.as_ref() {
            let _ = self
                .editor
                .select_file_browser_path(drop_target.column_index, &drop_target.target_dir);
        }
        let Some(file_name) = drag.source_path.file_name() else {
            return Task::none();
        };
        let destination = target_dir.join(file_name);
        match self.move_browser_path(&drag.source_path, &destination, true) {
            Ok(task) => task,
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
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

    fn move_browser_path(
        &mut self,
        source: &Path,
        destination: &Path,
        record_history: bool,
    ) -> Result<Task<Message>, String> {
        if destination.exists() {
            return Err(format!("{} already exists", destination.display()));
        }
        if source == destination {
            return Ok(Task::none());
        }
        if source.is_dir() && destination.starts_with(source) {
            return Err("Cannot move a folder into itself.".to_string());
        }

        let maybe_tab_id = self.editor.find_tab_by_path(source);
        let extra_task = if let Some(tab_id) = maybe_tab_id {
            self.rename_editor_tab_to_path_internal(tab_id, destination.to_path_buf(), false)
        } else {
            move_browser_path(source, destination)?;
            Task::none()
        };

        if source.is_dir() {
            let _ = self.editor.remap_open_paths_under(source, destination);
            let mut next_score_path = None;
            if let Some(current_score) = self.current_score.as_mut()
                && current_score.path.starts_with(source)
                && let Ok(relative) = current_score.path.strip_prefix(source)
            {
                current_score.path = destination.join(relative);
                current_score.file_name = current_score
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("score.ly")
                    .to_string();
                next_score_path = Some(current_score.path.clone());
            }
            if let Some(next_score_path) = next_score_path {
                self.restart_score_watcher(&next_score_path);
            }
        }

        let column_index = self
            .editor
            .current_file_browser_directory_column_index()
            .unwrap_or(0);
        Ok(self.finish_browser_path_transfer(
            column_index,
            destination.to_path_buf(),
            extra_task,
            record_history.then(|| BrowserHistoryEntry::Move {
                from: source.to_path_buf(),
                to: destination.to_path_buf(),
            }),
        ))
    }

    fn finish_browser_path_transfer(
        &mut self,
        column_index: usize,
        path: PathBuf,
        extra_task: Task<Message>,
        history_entry: Option<BrowserHistoryEntry>,
    ) -> Task<Message> {
        if let Some(history_entry) = history_entry {
            self.browser_undo_stack.push(history_entry);
            self.browser_redo_stack.clear();
        }
        match self.editor.refresh_file_browser() {
            Ok(()) => {
                let parent_dir = path.parent().map(PathBuf::from);
                let select_task = if let Some(selected) = self.editor.selected_file_browser_path() {
                    if let Some(parent_dir) = parent_dir.as_ref()
                        && selected == *parent_dir
                        && self.editor.file_browser_has_preview_column(column_index)
                    {
                        self.editor
                            .select_file_browser_path(column_index + 1, &path)
                            .ok()
                            .map(|()| self.reveal_editor_file_browser_selection(true))
                            .unwrap_or_else(Task::none)
                    } else {
                        self.editor
                            .select_file_browser_path(column_index, &path)
                            .ok()
                            .map(|()| self.reveal_editor_file_browser_selection(true))
                            .unwrap_or_else(Task::none)
                    }
                } else {
                    Task::none()
                };
                Task::batch([
                    self.reveal_editor_file_browser_selection(true),
                    select_task,
                    extra_task,
                ])
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                extra_task
            }
        }
    }

    fn next_browser_stash_path(&mut self, path: &Path) -> Result<PathBuf, String> {
        let Some(history_dir) = self.browser_history_dir.as_ref() else {
            return Err("Browser undo history is unavailable.".to_string());
        };
        let id = self.browser_history_next_stash_id;
        self.browser_history_next_stash_id = self.browser_history_next_stash_id.saturating_add(1);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("item");
        Ok(history_dir.path().join(format!("{id}-{file_name}")))
    }

    pub(in crate::app) fn undo_browser_operation(&mut self) -> Task<Message> {
        let Some(entry) = self.browser_undo_stack.pop() else {
            return Task::none();
        };
        match self.apply_browser_history_entry(entry, false) {
            Ok((redo_entry, task)) => {
                self.browser_redo_stack.push(redo_entry);
                task
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
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

    pub(in crate::app) fn redo_browser_operation(&mut self) -> Task<Message> {
        let Some(entry) = self.browser_redo_stack.pop() else {
            return Task::none();
        };
        match self.apply_browser_history_entry(entry, true) {
            Ok((undo_entry, task)) => {
                self.browser_undo_stack.push(undo_entry);
                task
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
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

    fn apply_browser_history_entry(
        &mut self,
        mut entry: BrowserHistoryEntry,
        redo: bool,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        match &mut entry {
            BrowserHistoryEntry::Create { path, stash_path } => {
                if redo {
                    let Some(stash_path) = stash_path.as_ref() else {
                        return Ok((entry, Task::none()));
                    };
                    move_browser_path(stash_path, path)?;
                    Ok((entry, self.refresh_browser_after_fs_change()))
                } else {
                    let stash = match stash_path.clone() {
                        Some(stash) => stash,
                        None => {
                            let stash = self.next_browser_stash_path(path)?;
                            *stash_path = Some(stash.clone());
                            stash
                        }
                    };
                    move_browser_path(path, &stash)?;
                    Ok((entry, self.refresh_browser_after_fs_change()))
                }
            }
            BrowserHistoryEntry::Move { from, to } => {
                let (source, destination) = if redo { (&*from, &*to) } else { (&*to, &*from) };
                self.move_browser_path(source, destination, false)
                    .map(|task| (entry, task))
            }
            BrowserHistoryEntry::Delete { path, stash_path } => {
                let (source, destination) = if redo {
                    (&*path, &*stash_path)
                } else {
                    (&*stash_path, &*path)
                };
                move_browser_path(source, destination)?;
                Ok((entry, self.refresh_browser_after_fs_change()))
            }
        }
    }

    pub(in crate::app) fn delete_browser_path_with_history(
        &mut self,
        path: &Path,
    ) -> Result<Task<Message>, String> {
        let stash_path = self.next_browser_stash_path(path)?;
        move_browser_path(path, &stash_path)?;
        self.browser_undo_stack.push(BrowserHistoryEntry::Delete {
            path: path.to_path_buf(),
            stash_path,
        });
        self.browser_redo_stack.clear();
        Ok(self.refresh_browser_after_fs_change())
    }

    fn commit_browser_inline_edit(&mut self) -> Task<Message> {
        let Some(edit) = self.browser_inline_edit.clone() else {
            return Task::none();
        };
        let Some(name) = normalize_editor_tab_file_name(&self.browser_inline_edit_value) else {
            return Task::none();
        };

        let destination = edit.parent_dir.join(name);
        let result = match edit.kind {
            BrowserInlineEditKind::Rename => {
                let Some(source) = edit.target_path.clone() else {
                    return Task::none();
                };
                if source == destination {
                    self.cancel_browser_inline_edit_state();
                    return Task::none();
                }
                if destination.exists() {
                    Err(format!("{} already exists", destination.display()))
                } else if source.is_file() {
                    if let Some(tab_id) = self.editor.find_tab_by_path(&source) {
                        let rename_task = self.rename_editor_tab_to_path_internal(
                            tab_id,
                            destination.clone(),
                            false,
                        );
                        return self.finish_browser_inline_edit_with_selection(
                            edit.column_index,
                            destination.clone(),
                            rename_task,
                            Some(BrowserHistoryEntry::Move {
                                from: source,
                                to: destination,
                            }),
                        );
                    }
                    fs::rename(&source, &destination).map_err(|error| {
                        format!(
                            "Failed to rename {} to {}: {error}",
                            source.display(),
                            destination.display()
                        )
                    })
                } else {
                    fs::rename(&source, &destination).map_err(|error| {
                        format!(
                            "Failed to rename {} to {}: {error}",
                            source.display(),
                            destination.display()
                        )
                    })
                }
            }
            BrowserInlineEditKind::NewFile => {
                if destination.exists() {
                    Err(format!("{} already exists", destination.display()))
                } else {
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&destination)
                        .map(|_| ())
                        .map_err(|error| {
                            format!("Failed to create file {}: {error}", destination.display())
                        })
                }
            }
            BrowserInlineEditKind::NewDirectory => {
                if destination.exists() {
                    Err(format!("{} already exists", destination.display()))
                } else {
                    fs::create_dir(&destination).map_err(|error| {
                        format!(
                            "Failed to create directory {}: {error}",
                            destination.display()
                        )
                    })
                }
            }
        };

        let history_entry = match edit.kind {
            BrowserInlineEditKind::Rename => {
                let Some(source) = edit.target_path.clone() else {
                    return Task::none();
                };
                BrowserHistoryEntry::Move {
                    from: source,
                    to: destination.clone(),
                }
            }
            BrowserInlineEditKind::NewFile | BrowserInlineEditKind::NewDirectory => {
                BrowserHistoryEntry::Create {
                    path: destination.clone(),
                    stash_path: None,
                }
            }
        };

        match result {
            Ok(()) => self.finish_browser_inline_edit_with_selection(
                edit.column_index,
                destination,
                Task::none(),
                Some(history_entry),
            ),
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
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

    fn finish_browser_inline_edit_with_selection(
        &mut self,
        column_index: usize,
        path: PathBuf,
        extra_task: Task<Message>,
        history_entry: Option<BrowserHistoryEntry>,
    ) -> Task<Message> {
        self.cancel_browser_inline_edit_state();
        if let Some(history_entry) = history_entry {
            self.browser_undo_stack.push(history_entry);
            self.browser_redo_stack.clear();
        }
        let refresh = match self.editor.refresh_file_browser() {
            Ok(()) => self.reveal_editor_file_browser_selection(true),
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        };
        let select = match self.editor.select_file_browser_path(column_index, &path) {
            Ok(()) => self.reveal_editor_file_browser_selection(true),
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        };
        Task::batch([refresh, select, extra_task])
    }

    fn cancel_browser_inline_edit_state(&mut self) {
        self.browser_inline_edit = None;
        self.browser_inline_edit_value.clear();
    }

    pub(in crate::app) fn refresh_browser_after_fs_change(&mut self) -> Task<Message> {
        match self.editor.refresh_file_browser() {
            Ok(()) => self.reveal_editor_file_browser_selection(
                self.editor.file_browser_has_preview_column(
                    self.editor.file_browser_active_column_index(),
                ),
            ),
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Browser Error",
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
}

pub(in crate::app) fn delete_browser_path(path: &std::path::Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to delete directory {}: {error}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to delete file {}: {error}", path.display()))
    }
}

fn copy_browser_path(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("{} already exists", destination.display()));
    }

    if source.is_dir() {
        fs::create_dir(destination).map_err(|error| {
            format!(
                "Failed to create directory {}: {error}",
                destination.display()
            )
        })?;

        for entry in fs::read_dir(source)
            .map_err(|error| format!("Failed to read directory {}: {error}", source.display()))?
        {
            let entry = entry.map_err(|error| {
                format!(
                    "Failed to read directory entry in {}: {error}",
                    source.display()
                )
            })?;
            let child_source = entry.path();
            let child_destination = destination.join(entry.file_name());
            copy_browser_path(&child_source, &child_destination)?;
        }

        Ok(())
    } else {
        fs::copy(source, destination).map(|_| ()).map_err(|error| {
            format!(
                "Failed to copy {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })
    }
}

fn move_browser_path(source: &Path, destination: &Path) -> Result<(), String> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_browser_path(source, destination)?;
            delete_browser_path(source).map_err(|delete_error| {
                format!(
                    "Failed to move {} to {}: rename failed with {rename_error}; cleanup failed with {delete_error}",
                    source.display(),
                    destination.display()
                )
            })
        }
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
