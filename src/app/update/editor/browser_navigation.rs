use super::*;

enum BrowserMessageRoute {
    Handled(Task<Message>),
    Unhandled(EditorMessage),
}

macro_rules! route_browser_messages {
    ($app:expr, $message:expr, $($route:ident),+ $(,)?) => {{
        let mut message = $message;
        $(
            message = match $app.$route(message) {
                BrowserMessageRoute::Handled(task) => return task,
                BrowserMessageRoute::Unhandled(message) => message,
            };
        )+
        message
    }};
}

macro_rules! browser_route {
    (fn $name:ident($self:ident, $message:ident) {
        $($pattern:pat => $body:expr),+ $(,)?
    }) => {
        fn $name(&mut $self, $message: EditorMessage) -> BrowserMessageRoute {
            match $message {
                $($pattern => BrowserMessageRoute::Handled($body),)+
                other => BrowserMessageRoute::Unhandled(other),
            }
        }
    };
}

impl Lilypalooza {
    browser_route! {
        fn route_file_browser_visibility_message(self, message) {
            EditorMessage::ToggleFileBrowser => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.clear_browser_drag_state();
                self.editor.toggle_file_browser();
                self.sync_browser_file_watcher();
                if self.editor.file_browser_expanded() {
                    self.focus_editor_file_browser();
                } else {
                    self.focus_editor_text_area()
                }
                Task::none()
            },
            EditorMessage::FileBrowserFocused => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.clear_browser_drag_state();
                self.focus_editor_file_browser();
                if self.browser_inline_edit.is_some() {
                    focus(self.browser_inline_edit_input_id.clone())
                } else {
                    Task::none()
                }
            },
            EditorMessage::FileBrowserToggleHiddenRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.toggle_file_browser_hidden_files()
            },
        }
    }

    browser_route! {
        fn route_file_browser_clipboard_message(self, message) {
            EditorMessage::FileBrowserCutRequested => self.focus_and_copy_file_browser_selection(BrowserClipboardKind::Cut),
            EditorMessage::FileBrowserCopyRequested => self.focus_and_copy_file_browser_selection(BrowserClipboardKind::Copy),
            EditorMessage::FileBrowserPasteRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.focus_editor_file_browser();
                self.paste_file_browser_clipboard()
            },
        }
    }

    browser_route! {
        fn route_file_browser_create_message(self, message) {
            EditorMessage::FileBrowserNewFileRequested => self.start_focused_browser_inline_create(false),
            EditorMessage::FileBrowserNewDirectoryRequested => self.start_focused_browser_inline_create(true),
            EditorMessage::FileBrowserRenameRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.start_browser_inline_rename()
            },
        }
    }

    browser_route! {
        fn route_file_browser_inline_message(self, message) {
            EditorMessage::FileBrowserInlineEditChanged(value) => self.update_file_browser_inline_edit_value(value),
            EditorMessage::CommitFileBrowserInlineEdit => self.commit_browser_inline_edit(),
            EditorMessage::CancelFileBrowserInlineEdit => self.cancel_file_browser_inline_edit(),
            EditorMessage::FileBrowserTrashRequested => self.request_file_browser_trash(),
        }
    }

    browser_route! {
        fn route_file_browser_scroll_message(self, message) {
            EditorMessage::FileBrowserScrolled(viewport) => {
                self.editor_file_browser_scroll_x = viewport.absolute_offset().x;
                self.editor_file_browser_viewport_width = viewport.bounds().width;
                Task::none()
            },
            EditorMessage::FileBrowserColumnScrolled {
                column_index,
                viewport,
            } => {
                self.editor_file_browser_column_scroll_y
                    .insert(column_index, viewport.absolute_offset().y);
                self.editor_file_browser_column_viewport_height
                    .insert(column_index, viewport.bounds().height);
                Task::none()
            },
        }
    }

    browser_route! {
        fn route_file_browser_press_message(self, message) {
            EditorMessage::FileBrowserEntryPressed {
                column_index,
                path,
                is_dir,
            } => self.press_file_browser_entry(column_index, path, is_dir),
            EditorMessage::FileBrowserEntryDoublePressed {
                column_index,
                path,
                is_dir,
            } => self.double_press_file_browser_entry(column_index, path, is_dir),
            EditorMessage::FileBrowserEntryHovered {
                column_index,
                path,
                is_dir,
            } => {
                self.update_browser_drop_target(Some((column_index, path, is_dir)));
                Task::none()
            },
        }
    }

    browser_route! {
        fn route_file_browser_drag_message(self, message) {
            EditorMessage::FileBrowserEntryDragReleased { path, is_dir } => self.release_file_browser_entry_drag(path, is_dir),
            EditorMessage::FileBrowserDragMoved(position) => self.handle_file_browser_drag_move(position),
            EditorMessage::FileBrowserDragReleased => self.release_file_browser_drag(),
        }
    }

    pub(super) fn handle_editor_rename_message(&mut self, message: EditorMessage) -> Task<Message> {
        if let Some(task) = self.handle_editor_rename_lifecycle_message(&message) {
            return task;
        }
        self.handle_editor_rename_dialog_message(message)
    }

    pub(super) fn handle_editor_rename_lifecycle_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        self.handle_editor_rename_input_message(message)
            .or_else(|| self.handle_editor_rename_start_message(message))
            .or_else(|| self.handle_editor_rename_finish_message(message))
    }

    pub(super) fn handle_editor_rename_input_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        if let EditorMessage::RenameInputChanged(value) = message {
            return Some(self.update_editor_tab_rename_value(value.clone()));
        }
        None
    }

    pub(super) fn handle_editor_rename_start_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        if let EditorMessage::StartRename(tab_id) = message {
            return Some(self.start_editor_tab_rename(*tab_id));
        }
        None
    }

    pub(super) fn handle_editor_rename_finish_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        match message {
            EditorMessage::CommitRename => Some(self.commit_editor_tab_rename()),
            EditorMessage::CancelRename => Some(self.cancel_editor_tab_rename()),
            _ => None,
        }
    }

    pub(super) fn update_editor_tab_rename_value(&mut self, value: String) -> Task<Message> {
        if self.shortcut_modifier_active() {
            return Task::none();
        }
        self.editor_tab_rename_value = value;
        Task::none()
    }

    pub(super) fn cancel_editor_tab_rename(&mut self) -> Task<Message> {
        self.cancel_editor_tab_rename_state();
        Task::none()
    }

    pub(super) fn handle_editor_rename_dialog_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        match message {
            EditorMessage::RenameRequested => self.request_active_editor_tab_rename(),
            EditorMessage::RenamePicked(Some(path)) => self.finish_picked_editor_tab_rename(path),
            EditorMessage::RenamePicked(None) => self.cancel_picked_editor_tab_rename(),
            _ => Task::none(),
        }
    }

    pub(super) fn request_active_editor_tab_rename(&mut self) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        let Some(tab_id) = self.editor.active_tab_id() else {
            return Task::none();
        };
        self.start_editor_tab_rename(tab_id)
    }

    pub(super) fn finish_picked_editor_tab_rename(&mut self, path: PathBuf) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        let Some(tab_id) = self.pending_editor_rename_tab.take() else {
            return Task::none();
        };
        self.rename_editor_tab_to_path(tab_id, path)
    }

    pub(super) fn cancel_picked_editor_tab_rename(&mut self) -> Task<Message> {
        self.pending_editor_rename_tab = None;
        Task::none()
    }

    pub(super) fn handle_file_browser_message(&mut self, message: EditorMessage) -> Task<Message> {
        let _message = route_browser_messages!(
            self,
            message,
            route_file_browser_visibility_message,
            route_file_browser_clipboard_message,
            route_file_browser_create_message,
            route_file_browser_inline_message,
            route_file_browser_scroll_message,
            route_file_browser_press_message,
            route_file_browser_drag_message,
        );
        Task::none()
    }

    pub(super) fn toggle_file_browser_hidden_files(&mut self) -> Task<Message> {
        match self.editor.toggle_file_browser_show_hidden() {
            Ok(()) => self.reveal_editor_file_browser_selection(
                self.editor.file_browser_has_preview_column(
                    self.editor.file_browser_active_column_index(),
                ),
            ),
            Err(error) => self.show_file_browser_error(error),
        }
    }

    pub(super) fn start_focused_browser_inline_create(&mut self, is_dir: bool) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.start_browser_inline_create(is_dir)
    }

    pub(super) fn update_file_browser_inline_edit_value(&mut self, value: String) -> Task<Message> {
        if self.shortcut_modifier_active() {
            return Task::none();
        }
        self.browser_inline_edit_value = value;
        Task::none()
    }

    pub(super) fn cancel_file_browser_inline_edit(&mut self) -> Task<Message> {
        self.clear_browser_drag_state();
        self.cancel_browser_inline_edit_state();
        Task::none()
    }

    pub(super) fn focus_and_copy_file_browser_selection(
        &mut self,
        kind: BrowserClipboardKind,
    ) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_file_browser();
        self.copy_or_cut_file_browser_selection(kind)
    }

    pub(super) fn request_file_browser_trash(&mut self) -> Task<Message> {
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

    pub(super) fn release_file_browser_entry_drag(
        &mut self,
        path: PathBuf,
        is_dir: bool,
    ) -> Task<Message> {
        let task = if is_dir {
            self.move_dragged_browser_path_to_directory(&path)
        } else {
            Task::none()
        };
        self.clear_browser_drag_state();
        task
    }

    pub(super) fn release_file_browser_drag(&mut self) -> Task<Message> {
        let task = self
            .browser_drop_target
            .as_ref()
            .map(|target| target.target_dir.clone())
            .map(|target_dir| self.move_dragged_browser_path_to_directory(&target_dir))
            .unwrap_or_else(Task::none);
        self.clear_browser_drag_state();
        task
    }

    pub(super) fn press_file_browser_entry(
        &mut self,
        column_index: usize,
        path: PathBuf,
        is_dir: bool,
    ) -> Task<Message> {
        self.begin_browser_entry_press(path.clone(), is_dir);
        self.browse_file_browser_entry(column_index, &path, is_dir, false)
    }

    pub(super) fn double_press_file_browser_entry(
        &mut self,
        column_index: usize,
        path: PathBuf,
        is_dir: bool,
    ) -> Task<Message> {
        self.browse_file_browser_entry(column_index, &path, is_dir, true)
    }

    fn browse_file_browser_entry(
        &mut self,
        column_index: usize,
        path: &Path,
        is_dir: bool,
        open_file: bool,
    ) -> Task<Message> {
        self.cancel_browser_inline_edit_state();
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_file_browser();
        self.editor.set_file_browser_active_column(column_index);
        match self.editor.browse_to_path(column_index, path, is_dir) {
            Ok(()) if is_dir || !open_file => self.reveal_editor_file_browser_selection(true),
            Ok(()) => Task::batch([
                self.reveal_editor_file_browser_selection(true),
                self.open_editor_file_in_editor_internal(path, false, false),
            ]),
            Err(error) => self.show_file_browser_error(error),
        }
    }

    pub(super) fn show_file_browser_error(&mut self, error: impl ToString) -> Task<Message> {
        self.show_prompt(
            ErrorPrompt::new(
                "File Browser Error",
                error.to_string(),
                ErrorFatality::Recoverable,
                PromptButtons::Ok,
            ),
            None,
        );
        Task::none()
    }
}
