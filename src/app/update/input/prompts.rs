use super::*;

impl Lilypalooza {
    pub(in crate::app) fn set_focused_workspace_pane(&mut self, pane: WorkspacePaneKind) {
        if self.group_for_pane(pane).is_some() {
            if pane != WorkspacePaneKind::Editor {
                self.editor_file_browser_focused = false;
            }
            self.focused_workspace_pane = Some(pane);
            self.sync_editor_widget_focus();
        }
    }

    pub(in crate::app) fn normalize_focused_workspace_pane(&mut self) {
        if self
            .focused_workspace_pane
            .is_some_and(|pane| self.group_for_pane(pane).is_some())
        {
            return;
        }

        self.focused_workspace_pane = self
            .dock_layout
            .as_ref()
            .and_then(|layout| first_active_workspace_pane(layout, &self.dock_groups))
            .or_else(|| self.dock_groups.values().next().map(|group| group.active));
        self.sync_editor_widget_focus();
    }

    pub(in crate::app) fn handle_logger_message(
        &mut self,
        message: LoggerMessage,
    ) -> Task<Message> {
        match message {
            LoggerMessage::RequestClear => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Logger);
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
                self.set_focused_workspace_pane(WorkspacePaneKind::Logger);
                self.logger.handle_editor_action(action);
                Task::none()
            }
        }
    }

    pub(in crate::app) fn handle_prompt_message(
        &mut self,
        message: PromptMessage,
    ) -> Task<Message> {
        if self.pending_editor_action.is_some() {
            return self.handle_pending_prompt_message(message);
        }

        match message {
            PromptMessage::Acknowledge => self.acknowledge_prompt(),
            PromptMessage::Discard => Task::none(),
            PromptMessage::Cancel => self.cancel_prompt(),
        }
    }

    pub(super) fn handle_pending_prompt_message(
        &mut self,
        message: PromptMessage,
    ) -> Task<Message> {
        match message {
            PromptMessage::Acknowledge => self.handle_pending_prompt_save(),
            PromptMessage::Discard => self.handle_pending_prompt_discard(),
            PromptMessage::Cancel => self.cancel_pending_prompt(),
        }
    }

    pub(super) fn acknowledge_prompt(&mut self) -> Task<Message> {
        if self.error_prompt.take().is_none() {
            return Task::none();
        }
        self.prompt_selected_button = PromptSelectedButton::Ok;
        self.run_prompt_ok_action()
    }

    pub(super) fn run_prompt_ok_action(&mut self) -> Task<Message> {
        match self.prompt_ok_action.take() {
            Some(PromptOkAction::ExitApp) => self.exit_app(),
            Some(PromptOkAction::ClearLogs) => self.clear_logs_from_prompt(),
            Some(PromptOkAction::ReloadEditorTab(tab_id)) => {
                self.reload_editor_tab_from_disk(tab_id)
            }
            Some(PromptOkAction::RemoveBus(bus_id)) => self.remove_bus_confirmed(bus_id),
            Some(PromptOkAction::DeleteBrowserPath(path)) => {
                self.delete_browser_path_from_prompt(&path)
            }
            None => Task::none(),
        }
    }

    pub(super) fn clear_logs_from_prompt(&mut self) -> Task<Message> {
        self.logger.clear();
        Task::none()
    }

    pub(super) fn delete_browser_path_from_prompt(&mut self, path: &Path) -> Task<Message> {
        match self.delete_browser_path_with_history(path) {
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

    pub(super) fn cancel_prompt(&mut self) -> Task<Message> {
        self.error_prompt = None;
        self.prompt_ok_action = None;
        self.prompt_selected_button = PromptSelectedButton::Ok;
        Task::none()
    }

    pub(super) fn cancel_pending_prompt(&mut self) -> Task<Message> {
        self.error_prompt = None;
        self.prompt_selected_button = PromptSelectedButton::Ok;
        self.pending_editor_action = None;
        self.pending_editor_save_as_tab = None;
        Task::none()
    }

    pub(in crate::app) fn handle_window_close_requested(
        &mut self,
        window_id: window::Id,
    ) -> Task<Message> {
        if window_id != self.main_window_id {
            return self.handle_processor_editor_close_requested(window_id);
        }
        let hide_editors = self.hide_all_editor_windows();
        let dirty_tabs = self.editor.tabs_requiring_resolution();
        if dirty_tabs.is_empty() {
            if self.project_is_dirty() {
                return Task::batch([
                    hide_editors,
                    self.begin_pending_project_action(EditorContinuation::ExitApp),
                ]);
            }
            return Task::batch([hide_editors, self.exit_app()]);
        }

        self.pending_editor_action = Some(PendingEditorAction::ResolveDirtyTabs {
            dirty_tab_ids: dirty_tabs,
            continuation: EditorContinuation::ExitApp,
        });
        self.show_current_pending_editor_prompt();
        hide_editors
    }

    pub(in crate::app) fn handle_window_resized(
        &mut self,
        window_id: window::Id,
        size: Size,
    ) -> Task<Message> {
        if window_id != self.main_window_id {
            log::trace!(
                target: "lilypalooza::editor_windows",
                "thread={:?} iced resize event for editor window_id={window_id:?} size={size:?}",
                std::thread::current().id(),
            );
            for error in self.processor_editor_windows.resize_window_outer(
                window_id,
                editor_host::Size {
                    width: f64::from(size.width),
                    height: f64::from(size.height),
                },
            ) {
                self.log_processor_editor_error("resize", error);
            }
            return Task::none();
        }

        self.window_width = size.width.max(1.0);
        self.window_height = size.height.max(1.0);
        self.sync_editor_viewport_from_layout();

        Task::none()
    }

    pub(in crate::app) fn handle_modifiers_changed(
        &mut self,
        modifiers: iced::keyboard::Modifiers,
    ) -> Task<Message> {
        self.keyboard_modifiers = modifiers;
        Task::none()
    }

    pub(in crate::app) fn show_prompt(
        &mut self,
        prompt: ErrorPrompt,
        ok_action: Option<PromptOkAction>,
    ) {
        self.error_prompt = Some(prompt);
        self.prompt_ok_action = ok_action;
        self.prompt_selected_button = PromptSelectedButton::Ok;
    }

    pub(in crate::app) fn begin_pending_editor_action(
        &mut self,
        dirty_tab_ids: Vec<u64>,
        continuation: EditorContinuation,
    ) -> Task<Message> {
        if dirty_tab_ids.is_empty() {
            return self.continue_editor_continuation(continuation);
        }

        self.pending_editor_action = Some(PendingEditorAction::ResolveDirtyTabs {
            dirty_tab_ids,
            continuation,
        });
        self.show_current_pending_editor_prompt();
        Task::none()
    }

    pub(in crate::app) fn begin_pending_project_action(
        &mut self,
        continuation: EditorContinuation,
    ) -> Task<Message> {
        if !self.project_is_dirty() {
            return self.continue_editor_continuation(continuation);
        }

        self.pending_editor_action =
            Some(PendingEditorAction::ResolveDirtyProject { continuation });
        self.show_current_pending_editor_prompt();
        Task::none()
    }

    pub(in crate::app) fn show_current_pending_editor_prompt(&mut self) {
        let Some(action) = self.pending_editor_action.as_ref() else {
            return;
        };

        let prompt = match action {
            PendingEditorAction::ResolveDirtyTabs { dirty_tab_ids, .. } => {
                let Some(&tab_id) = dirty_tab_ids.first() else {
                    return;
                };
                let title = self.editor.tab_title(tab_id);
                if self.editor.tab_file_state(tab_id) == Some(EditorTabFileState::MissingOnDisk) {
                    ErrorPrompt::new(
                        format!("Save {title}?"),
                        format!(
                            "{title} is missing on disk. Save it before continuing to recreate \
                             the file?"
                        ),
                        ErrorFatality::Recoverable,
                        PromptButtons::SaveDiscardCancel,
                    )
                    .with_discard_label("Close Without Saving")
                } else {
                    ErrorPrompt::new(
                        format!("Close {title}?"),
                        format!("Save changes to {title} before continuing?"),
                        ErrorFatality::Recoverable,
                        PromptButtons::SaveDiscardCancel,
                    )
                }
            }
            PendingEditorAction::ResolveDirtyProject { .. } => ErrorPrompt::new(
                "Save project changes?",
                "Project state changed. Save the project before continuing?",
                ErrorFatality::Recoverable,
                PromptButtons::SaveDiscardCancel,
            ),
        };
        self.error_prompt = Some(prompt);
        self.prompt_ok_action = None;
        self.prompt_selected_button = PromptSelectedButton::Ok;
    }

    pub(in crate::app) fn handle_pending_prompt_save(&mut self) -> Task<Message> {
        self.error_prompt = None;
        self.prompt_selected_button = PromptSelectedButton::Ok;
        match self.pending_editor_action.as_ref() {
            Some(PendingEditorAction::ResolveDirtyTabs { dirty_tab_ids, .. }) => {
                let tab_id = dirty_tab_ids.first().copied();
                self.save_next_pending_dirty_tab(tab_id)
            }
            Some(PendingEditorAction::ResolveDirtyProject { .. }) => self.save_pending_project(),
            None => Task::none(),
        }
    }

    pub(super) fn save_next_pending_dirty_tab(&mut self, tab_id: Option<u64>) -> Task<Message> {
        let Some(tab_id) = tab_id else {
            return Task::none();
        };

        if self.editor.tab_path(tab_id).is_none() {
            return self.request_save_pending_dirty_tab_as(tab_id);
        }

        let save_task = self.save_editor_tab(tab_id);
        let advance_task = self.advance_pending_editor_action();
        Task::batch([save_task, advance_task])
    }

    pub(super) fn request_save_pending_dirty_tab_as(&mut self, tab_id: u64) -> Task<Message> {
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

    pub(super) fn save_pending_project(&mut self) -> Task<Message> {
        match self.project_root.clone() {
            Some(project_root) => {
                let save_task = self.save_project_to_root(project_root);
                let advance_task = self.advance_pending_editor_action();
                Task::batch([save_task, advance_task])
            }
            None => update(self, Message::File(FileMessage::RequestCreateProject)),
        }
    }
}
