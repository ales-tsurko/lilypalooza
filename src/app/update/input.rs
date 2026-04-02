use super::*;
use crate::app::piano_roll::{adjacent_subdivision_tick, roll_scroll_id};

impl Lilypalooza {
    pub(in crate::app) fn sync_editor_widget_focus(&mut self) {
        if self.focused_workspace_pane == Some(WorkspacePaneKind::Editor) {
            self.editor.request_focus();
        } else {
            self.editor.lose_focus();
        }
    }

    pub(in crate::app) fn handle_key_pressed(&mut self, key_press: KeyPress) -> Task<Message> {
        if self.renaming_editor_tab.is_some()
            && matches!(
                key_press.key,
                keyboard::Key::Named(keyboard::key::Named::Escape)
            )
        {
            return update(self, Message::Editor(EditorMessage::CancelRename));
        }

        let shortcut_input =
            ShortcutInput::new(&key_press.key, key_press.physical_key, key_press.modifiers);

        if let Some(action) = shortcuts::resolve_global(&self.shortcut_settings, shortcut_input) {
            return self.handle_shortcut_action(action);
        }

        if let Some(action) = shortcuts::resolve_navigation(&self.shortcut_settings, shortcut_input)
        {
            return self.handle_shortcut_action(action);
        }

        let Some(focused_pane) = self.focused_workspace_pane() else {
            return Task::none();
        };

        if (key_press.modifiers.command() || key_press.modifiers.control())
            && let Some(action) =
                shortcuts::resolve_contextual(&self.shortcut_settings, focused_pane, shortcut_input)
        {
            return self.handle_shortcut_action(action);
        }

        if matches!(key_press.status, iced::event::Status::Captured) {
            return Task::none();
        }

        if let Some(action) =
            shortcuts::resolve_contextual(&self.shortcut_settings, focused_pane, shortcut_input)
        {
            return self.handle_shortcut_action(action);
        }

        Task::none()
    }

    pub(in crate::app) fn handle_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        match action {
            ShortcutAction::NewEditor => update(self, Message::Editor(EditorMessage::NewRequested)),
            ShortcutAction::OpenEditorFile => {
                update(self, Message::Editor(EditorMessage::OpenRequested))
            }
            ShortcutAction::SaveEditor => {
                update(self, Message::Editor(EditorMessage::SaveRequested))
            }
            ShortcutAction::ToggleWorkspacePane(pane) => {
                update(self, Message::Pane(PaneMessage::ToggleWorkspacePane(pane)))
            }
            ShortcutAction::SwitchWorkspaceTabPrevious => {
                self.switch_focused_workspace_tab(TabDirection::Previous);
                Task::none()
            }
            ShortcutAction::SwitchWorkspaceTabNext => {
                self.switch_focused_workspace_tab(TabDirection::Next);
                Task::none()
            }
            ShortcutAction::FocusWorkspacePanePrevious => {
                self.cycle_workspace_pane_focus(PaneCycleDirection::Previous);
                Task::none()
            }
            ShortcutAction::FocusWorkspacePaneNext => {
                self.cycle_workspace_pane_focus(PaneCycleDirection::Next);
                Task::none()
            }
            ShortcutAction::ScoreZoomIn => update(self, Message::Viewer(ViewerMessage::ZoomIn)),
            ShortcutAction::ScoreZoomOut => update(self, Message::Viewer(ViewerMessage::ZoomOut)),
            ShortcutAction::ScoreZoomReset => {
                update(self, Message::Viewer(ViewerMessage::ResetZoom))
            }
            ShortcutAction::EditorZoomIn => {
                self.editor.zoom_in();
                self.persist_settings();
                Task::none()
            }
            ShortcutAction::EditorZoomOut => {
                self.editor.zoom_out();
                self.persist_settings();
                Task::none()
            }
            ShortcutAction::EditorZoomReset => {
                self.editor.reset_zoom();
                self.persist_settings();
                Task::none()
            }
            ShortcutAction::PianoRollZoomIn => {
                update(self, Message::PianoRoll(PianoRollMessage::ZoomIn))
            }
            ShortcutAction::PianoRollZoomOut => {
                update(self, Message::PianoRoll(PianoRollMessage::ZoomOut))
            }
            ShortcutAction::PianoRollZoomReset => {
                update(self, Message::PianoRoll(PianoRollMessage::ResetZoom))
            }
            ShortcutAction::PianoRollCursorSubdivisionPrevious => {
                if self.piano_roll.playback_is_playing() {
                    return Task::none();
                }
                let Some(file) = self.piano_roll.current_file() else {
                    return Task::none();
                };
                let tick = adjacent_subdivision_tick(
                    &file.data,
                    self.piano_roll.beat_subdivision,
                    self.piano_roll.playback_tick(),
                    false,
                );
                update(
                    self,
                    Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)),
                )
            }
            ShortcutAction::PianoRollCursorSubdivisionNext => {
                if self.piano_roll.playback_is_playing() {
                    return Task::none();
                }
                let Some(file) = self.piano_roll.current_file() else {
                    return Task::none();
                };
                let tick = adjacent_subdivision_tick(
                    &file.data,
                    self.piano_roll.beat_subdivision,
                    self.piano_roll.playback_tick(),
                    true,
                );
                update(
                    self,
                    Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)),
                )
            }
            ShortcutAction::PianoRollScrollUp => iced::widget::operation::scroll_by(
                roll_scroll_id(),
                iced::widget::operation::AbsoluteOffset {
                    x: 0.0,
                    y: -KEYBOARD_SCROLL_STEP,
                },
            ),
            ShortcutAction::PianoRollScrollDown => iced::widget::operation::scroll_by(
                roll_scroll_id(),
                iced::widget::operation::AbsoluteOffset {
                    x: 0.0,
                    y: KEYBOARD_SCROLL_STEP,
                },
            ),
            ShortcutAction::TransportPlayPause => update(
                self,
                Message::PianoRoll(PianoRollMessage::TransportPlayPause),
            ),
            ShortcutAction::TransportRewind => {
                update(self, Message::PianoRoll(PianoRollMessage::TransportRewind))
            }
            ShortcutAction::ScoreScrollUp => update(self, Message::Viewer(ViewerMessage::ScrollUp)),
            ShortcutAction::ScoreScrollDown => {
                update(self, Message::Viewer(ViewerMessage::ScrollDown))
            }
            ShortcutAction::ScorePrevPage => update(self, Message::Viewer(ViewerMessage::PrevPage)),
            ShortcutAction::ScoreNextPage => update(self, Message::Viewer(ViewerMessage::NextPage)),
        }
    }

    pub(in crate::app) fn set_focused_workspace_pane(&mut self, pane: WorkspacePaneKind) {
        if self.group_for_pane(pane).is_some() {
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
            return match message {
                PromptMessage::Acknowledge => self.handle_pending_editor_prompt_save(),
                PromptMessage::Discard => self.advance_pending_editor_action(),
                PromptMessage::Cancel => {
                    self.error_prompt = None;
                    self.pending_editor_action = None;
                    self.pending_editor_save_as_tab = None;
                    Task::none()
                }
            };
        }

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
            PromptMessage::Discard => Task::none(),
            PromptMessage::Cancel => {
                self.error_prompt = None;
                self.prompt_ok_action = None;
                Task::none()
            }
        }
    }

    pub(in crate::app) fn handle_window_close_requested(&mut self) -> Task<Message> {
        let dirty_tabs = self.editor.dirty_tab_ids();
        if dirty_tabs.is_empty() {
            return iced::exit();
        }

        self.pending_editor_action = Some(PendingEditorAction::ResolveDirtyTabs {
            dirty_tab_ids: dirty_tabs,
            continuation: EditorContinuation::ExitApp,
        });
        self.show_current_pending_editor_prompt();
        Task::none()
    }

    pub(in crate::app) fn handle_window_resized(&mut self, size: Size) -> Task<Message> {
        self.window_width = size.width.max(1.0);
        self.window_height = size.height.max(1.0);

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

    pub(in crate::app) fn show_current_pending_editor_prompt(&mut self) {
        let Some(PendingEditorAction::ResolveDirtyTabs { dirty_tab_ids, .. }) =
            self.pending_editor_action.as_ref()
        else {
            return;
        };
        let Some(&tab_id) = dirty_tab_ids.first() else {
            return;
        };

        let title = self.editor.tab_title(tab_id);
        self.error_prompt = Some(ErrorPrompt::new(
            format!("Close {title}?"),
            format!("Save changes to {title} before continuing?"),
            ErrorFatality::Recoverable,
            PromptButtons::SaveDiscardCancel,
        ));
        self.prompt_ok_action = None;
    }

    pub(in crate::app) fn handle_pending_editor_prompt_save(&mut self) -> Task<Message> {
        let Some(PendingEditorAction::ResolveDirtyTabs { dirty_tab_ids, .. }) =
            self.pending_editor_action.as_ref()
        else {
            return Task::none();
        };
        let Some(&tab_id) = dirty_tab_ids.first() else {
            return Task::none();
        };

        self.error_prompt = None;

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

        let save_task = self.save_editor_tab(tab_id);
        let advance_task = self.advance_pending_editor_action();
        Task::batch([save_task, advance_task])
    }

    pub(in crate::app) fn advance_pending_editor_action(&mut self) -> Task<Message> {
        let Some(PendingEditorAction::ResolveDirtyTabs {
            dirty_tab_ids,
            continuation: _,
        }) = self.pending_editor_action.as_mut()
        else {
            return Task::none();
        };

        if !dirty_tab_ids.is_empty() {
            dirty_tab_ids.remove(0);
        }

        if !dirty_tab_ids.is_empty() {
            self.show_current_pending_editor_prompt();
            return Task::none();
        }

        let continuation = match self.pending_editor_action.take() {
            Some(PendingEditorAction::ResolveDirtyTabs { continuation, .. }) => continuation,
            None => return Task::none(),
        };

        self.error_prompt = None;
        self.continue_editor_continuation(continuation)
    }

    pub(in crate::app) fn continue_editor_continuation(
        &mut self,
        continuation: EditorContinuation,
    ) -> Task<Message> {
        match continuation {
            EditorContinuation::CloseTab(tab_id) => {
                self.editor.close_tab(tab_id);
                self.editor.request_focus();
                self.persist_settings();
                Task::none()
            }
            EditorContinuation::LoadProject(project_root) => {
                self.load_project_from_root(project_root)
            }
            EditorContinuation::OpenScore(path) => match selected_score_from_path(path) {
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
            },
            EditorContinuation::ExitApp => iced::exit(),
        }
    }
}
