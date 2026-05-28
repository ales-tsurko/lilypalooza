use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserHistoryDirection {
    Undo,
    Redo,
}

impl BrowserHistoryDirection {
    fn is_redo(self) -> bool {
        matches!(self, Self::Redo)
    }

    fn inverse(self) -> Self {
        match self {
            Self::Undo => Self::Redo,
            Self::Redo => Self::Undo,
        }
    }
}

fn browser_create_entry(path: PathBuf, stash_path: Option<PathBuf>) -> BrowserHistoryEntry {
    BrowserHistoryEntry::Create { path, stash_path }
}

fn browser_create_move_paths<'a>(
    path: &'a Path,
    stash_path: &'a Path,
    direction: BrowserHistoryDirection,
) -> (&'a Path, &'a Path) {
    match direction {
        BrowserHistoryDirection::Redo => (stash_path, path),
        BrowserHistoryDirection::Undo => (path, stash_path),
    }
}

impl Lilypalooza {
    pub(super) fn start_browser_inline_create(&mut self, directory: bool) -> Task<Message> {
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

    pub(super) fn start_browser_inline_rename(&mut self) -> Task<Message> {
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

    pub(super) fn copy_or_cut_file_browser_selection(
        &mut self,
        kind: BrowserClipboardKind,
    ) -> Task<Message> {
        let Some(path) = self.editor.selected_file_browser_path() else {
            return Task::none();
        };
        self.browser_clipboard = Some(BrowserClipboard { path, kind });
        Task::none()
    }

    pub(super) fn paste_file_browser_clipboard(&mut self) -> Task<Message> {
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

    pub(super) fn begin_browser_entry_press(&mut self, path: PathBuf, is_dir: bool) {
        self.browser_pressed_entry = Some(BrowserPressedEntry {
            path,
            is_dir,
            origin: self.editor_file_browser_cursor.unwrap_or(Point::ORIGIN),
        });
        self.browser_drag_state = None;
        self.browser_drop_target = None;
    }

    pub(super) fn handle_file_browser_drag_move(&mut self, position: iced::Point) -> Task<Message> {
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

    pub(super) fn update_browser_drop_target(&mut self, hovered: Option<(usize, PathBuf, bool)>) {
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

    pub(super) fn clear_browser_drag_state(&mut self) {
        self.browser_pressed_entry = None;
        self.browser_drag_state = None;
        self.browser_drop_target = None;
    }

    pub(super) fn move_dragged_browser_path_to_directory(
        &mut self,
        target_dir: &Path,
    ) -> Task<Message> {
        let Some(drag) = self.browser_drag_state.clone() else {
            return Task::none();
        };
        if let Some(drop_target) = self.browser_drop_target.as_ref()
            && let Err(error) = self
                .editor
                .select_file_browser_path(drop_target.column_index, &drop_target.target_dir)
        {
            self.logger.push(format!(
                "[file-browser:error] Failed to select drop target {}: {error}",
                drop_target.target_dir.display()
            ));
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

    pub(super) fn move_browser_path(
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
            let remapped_tabs = self.editor.remap_open_paths_under(source, destination);
            if !remapped_tabs.is_empty() {
                self.sync_editor_file_watcher();
            }
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

    pub(super) fn finish_browser_path_transfer(
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

    pub(super) fn next_browser_stash_path(&mut self, path: &Path) -> Result<PathBuf, String> {
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
        self.apply_browser_operation(BrowserHistoryDirection::Undo)
    }

    pub(in crate::app) fn redo_browser_operation(&mut self) -> Task<Message> {
        self.apply_browser_operation(BrowserHistoryDirection::Redo)
    }

    fn apply_browser_operation(&mut self, direction: BrowserHistoryDirection) -> Task<Message> {
        let Some(entry) = self.pop_browser_history_entry(direction) else {
            return Task::none();
        };
        match self.apply_browser_history_entry(entry, direction.is_redo()) {
            Ok((inverse_entry, task)) => {
                self.push_browser_history_entry(direction.inverse(), inverse_entry);
                task
            }
            Err(error) => {
                self.show_browser_history_error(error);
                Task::none()
            }
        }
    }

    fn pop_browser_history_entry(
        &mut self,
        direction: BrowserHistoryDirection,
    ) -> Option<BrowserHistoryEntry> {
        match direction {
            BrowserHistoryDirection::Undo => self.browser_undo_stack.pop(),
            BrowserHistoryDirection::Redo => self.browser_redo_stack.pop(),
        }
    }

    fn push_browser_history_entry(
        &mut self,
        direction: BrowserHistoryDirection,
        entry: BrowserHistoryEntry,
    ) {
        match direction {
            BrowserHistoryDirection::Undo => self.browser_undo_stack.push(entry),
            BrowserHistoryDirection::Redo => self.browser_redo_stack.push(entry),
        }
    }

    fn show_browser_history_error(&mut self, error: String) {
        self.show_prompt(
            ErrorPrompt::new(
                "File Browser Error",
                error,
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
    }

    pub(super) fn apply_browser_history_entry(
        &mut self,
        entry: BrowserHistoryEntry,
        redo: bool,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        match entry {
            BrowserHistoryEntry::Create { path, stash_path } => {
                self.apply_browser_create_history(path, stash_path, redo)
            }
            BrowserHistoryEntry::Move { from, to } => {
                self.apply_browser_move_history(from, to, redo)
            }
            BrowserHistoryEntry::Delete { path, stash_path } => {
                self.apply_browser_delete_history(path, stash_path, redo)
            }
        }
    }

    pub(super) fn apply_browser_create_history(
        &mut self,
        path: PathBuf,
        stash_path: Option<PathBuf>,
        redo: bool,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        if redo {
            return self.redo_browser_create_history(path, stash_path);
        }
        self.undo_browser_create_history(path, stash_path)
    }

    pub(super) fn redo_browser_create_history(
        &mut self,
        path: PathBuf,
        stash_path: Option<PathBuf>,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        self.apply_browser_create_history_direction(path, stash_path, BrowserHistoryDirection::Redo)
    }

    pub(super) fn undo_browser_create_history(
        &mut self,
        path: PathBuf,
        stash_path: Option<PathBuf>,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        self.apply_browser_create_history_direction(path, stash_path, BrowserHistoryDirection::Undo)
    }

    fn apply_browser_create_history_direction(
        &mut self,
        path: PathBuf,
        stash_path: Option<PathBuf>,
        direction: BrowserHistoryDirection,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        let Some(stash_path) = self.browser_create_stash_path(&path, stash_path, direction)? else {
            return Ok((browser_create_entry(path, None), Task::none()));
        };
        let (source, destination) = browser_create_move_paths(&path, &stash_path, direction);
        move_browser_path(source, destination)?;
        Ok((
            browser_create_entry(path, Some(stash_path)),
            self.refresh_browser_after_fs_change(),
        ))
    }

    fn browser_create_stash_path(
        &mut self,
        path: &Path,
        stash_path: Option<PathBuf>,
        direction: BrowserHistoryDirection,
    ) -> Result<Option<PathBuf>, String> {
        match (direction, stash_path) {
            (BrowserHistoryDirection::Redo, None) => Ok(None),
            (_, Some(stash_path)) => Ok(Some(stash_path)),
            (BrowserHistoryDirection::Undo, None) => self.next_browser_stash_path(path).map(Some),
        }
    }

    pub(super) fn apply_browser_move_history(
        &mut self,
        from: PathBuf,
        to: PathBuf,
        redo: bool,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        let (source, destination) = browser_move_history_paths(&from, &to, redo);
        self.move_browser_path(source, destination, false)
            .map(|task| (BrowserHistoryEntry::Move { from, to }, task))
    }

    pub(super) fn apply_browser_delete_history(
        &mut self,
        path: PathBuf,
        stash_path: PathBuf,
        redo: bool,
    ) -> Result<(BrowserHistoryEntry, Task<Message>), String> {
        let (source, destination) = browser_delete_history_paths(&path, &stash_path, redo);
        move_browser_path(source, destination)?;
        Ok((
            BrowserHistoryEntry::Delete { path, stash_path },
            self.refresh_browser_after_fs_change(),
        ))
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

    pub(super) fn commit_browser_inline_edit(&mut self) -> Task<Message> {
        let Some(edit) = self.browser_inline_edit.clone() else {
            return Task::none();
        };
        let Some(name) = normalize_editor_tab_file_name(&self.browser_inline_edit_value) else {
            return Task::none();
        };

        let destination = edit.parent_dir.join(name);
        match self.apply_browser_inline_edit(&edit, destination.clone()) {
            Ok(BrowserInlineEditCommit::Noop) => Task::none(),
            Ok(BrowserInlineEditCommit::Finished(task)) => task,
            Ok(BrowserInlineEditCommit::Applied(history_entry)) => self
                .finish_browser_inline_edit_with_selection(
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

    pub(super) fn apply_browser_inline_edit(
        &mut self,
        edit: &BrowserInlineEdit,
        destination: PathBuf,
    ) -> Result<BrowserInlineEditCommit, String> {
        match edit.kind {
            BrowserInlineEditKind::Rename => self.rename_browser_inline_edit(edit, destination),
            BrowserInlineEditKind::NewFile => create_browser_inline_file(destination),
            BrowserInlineEditKind::NewDirectory => create_browser_inline_directory(destination),
        }
    }

    pub(super) fn rename_browser_inline_edit(
        &mut self,
        edit: &BrowserInlineEdit,
        destination: PathBuf,
    ) -> Result<BrowserInlineEditCommit, String> {
        let Some(source) = edit.target_path.clone() else {
            return Ok(BrowserInlineEditCommit::Noop);
        };
        if source == destination {
            self.cancel_browser_inline_edit_state();
            return Ok(BrowserInlineEditCommit::Noop);
        }
        ensure_browser_destination_available(&destination)?;
        if let Some(task) = self.rename_open_browser_inline_file(edit, &source, &destination) {
            return Ok(BrowserInlineEditCommit::Finished(task));
        }
        rename_browser_path(&source, &destination)?;
        Ok(BrowserInlineEditCommit::Applied(
            BrowserHistoryEntry::Move {
                from: source,
                to: destination,
            },
        ))
    }

    pub(super) fn rename_open_browser_inline_file(
        &mut self,
        edit: &BrowserInlineEdit,
        source: &Path,
        destination: &Path,
    ) -> Option<Task<Message>> {
        let tab_id = source
            .is_file()
            .then(|| self.editor.find_tab_by_path(source))
            .flatten()?;
        let rename_task =
            self.rename_editor_tab_to_path_internal(tab_id, destination.to_path_buf(), false);
        Some(self.finish_browser_inline_edit_with_selection(
            edit.column_index,
            destination.to_path_buf(),
            rename_task,
            Some(BrowserHistoryEntry::Move {
                from: source.to_path_buf(),
                to: destination.to_path_buf(),
            }),
        ))
    }

    pub(super) fn finish_browser_inline_edit_with_selection(
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

    pub(super) fn cancel_browser_inline_edit_state(&mut self) {
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
