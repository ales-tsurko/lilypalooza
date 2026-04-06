use super::*;
use crate::app::editor::EditorTabFileState;
use crate::app::messages::ShortcutsMessage;
use crate::app::piano_roll::{adjacent_subdivision_tick, roll_scroll_id};

impl Lilypalooza {
    fn dispatch_active_editor_widget_message(
        &mut self,
        message: iced_code_editor::Message,
    ) -> Task<Message> {
        update(
            self,
            Message::Editor(EditorMessage::ActiveWidgetMessage(message)),
        )
    }

    pub(in crate::app) fn sync_editor_widget_focus(&mut self) {
        if self.focused_workspace_pane == Some(WorkspacePaneKind::Editor) {
            self.editor.request_focus();
        } else {
            self.editor.lose_focus();
        }
    }

    pub(in crate::app) fn handle_key_pressed(&mut self, key_press: KeyPress) -> Task<Message> {
        if self.open_shortcuts_dialog {
            match key_press.key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    return update(self, Message::Shortcuts(ShortcutsMessage::CloseDialog));
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    return update(self, Message::Shortcuts(ShortcutsMessage::SelectNext));
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    return update(self, Message::Shortcuts(ShortcutsMessage::SelectPrevious));
                }
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    return update(self, Message::Shortcuts(ShortcutsMessage::ActivateSelected));
                }
                _ => {}
            }

            return Task::none();
        }

        if self.renaming_editor_tab.is_some()
            && matches!(
                key_press.key,
                keyboard::Key::Named(keyboard::key::Named::Escape)
            )
        {
            return update(self, Message::Editor(EditorMessage::CancelRename));
        }
        if self.renaming_editor_tab.is_some()
            && matches!(key_press.status, iced::event::Status::Captured)
        {
            return Task::none();
        }

        if self.focused_workspace_pane() == Some(WorkspacePaneKind::Editor)
            && matches!(
                key_press.key,
                keyboard::Key::Named(keyboard::key::Named::Escape)
            )
        {
            return Task::batch([
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::CloseSearch),
                self.dispatch_active_editor_widget_message(
                    iced_code_editor::Message::CloseGotoLine,
                ),
            ]);
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

        if focused_pane == WorkspacePaneKind::Editor {
            if let Some(action) =
                shortcuts::resolve_contextual(&self.shortcut_settings, focused_pane, shortcut_input)
            {
                return self.handle_shortcut_action(action);
            }
        } else if (key_press.modifiers.command() || key_press.modifiers.control())
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

    pub(in crate::app) fn handle_shortcuts_message(
        &mut self,
        message: ShortcutsMessage,
    ) -> Task<Message> {
        match message {
            ShortcutsMessage::OpenDialog => {
                self.open_project_menu = false;
                self.open_project_menu_section = None;
                self.open_project_recent = false;
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.hovered_editor_file_menu_section = None;
                self.open_shortcuts_dialog = true;
                self.editor.lose_focus();
                self.shortcuts_search_query.clear();
                self.shortcuts_selected_action = shortcuts::filtered_action_metadata("")
                    .first()
                    .map(|metadata| metadata.id);
                Task::batch([
                    iced::widget::operation::focus(self.shortcuts_search_input_id.clone()),
                    self.reveal_selected_shortcut_action(),
                ])
            }
            ShortcutsMessage::CloseDialog => {
                self.open_shortcuts_dialog = false;
                self.shortcuts_search_query.clear();
                self.shortcuts_selected_action = None;
                self.sync_editor_widget_focus();
                Task::none()
            }
            ShortcutsMessage::SearchChanged(value) => {
                self.shortcuts_search_query = value;
                self.reconcile_shortcut_palette_selection()
            }
            ShortcutsMessage::SelectNext => self.move_shortcut_palette_selection(1),
            ShortcutsMessage::SelectPrevious => self.move_shortcut_palette_selection(-1),
            ShortcutsMessage::ActivateSelected => {
                let Some(action_id) = self.shortcuts_selected_action else {
                    return Task::none();
                };
                self.handle_shortcuts_message(ShortcutsMessage::ActivateAction(action_id))
            }
            ShortcutsMessage::ActivateAction(action_id) => {
                self.open_shortcuts_dialog = false;
                self.shortcuts_search_query.clear();
                self.shortcuts_selected_action = None;
                self.sync_editor_widget_focus();
                self.handle_shortcut_action(shortcuts::action_for_id(action_id))
            }
        }
    }

    fn shortcut_palette_actions(&self) -> Vec<crate::shortcuts::ShortcutActionMetadata> {
        shortcuts::filtered_action_metadata(&self.shortcuts_search_query)
    }

    fn reconcile_shortcut_palette_selection(&mut self) -> Task<Message> {
        let actions = self.shortcut_palette_actions();
        self.shortcuts_selected_action = if actions
            .iter()
            .any(|metadata| Some(metadata.id) == self.shortcuts_selected_action)
        {
            self.shortcuts_selected_action
        } else {
            actions.first().map(|metadata| metadata.id)
        };
        self.reveal_selected_shortcut_action()
    }

    fn move_shortcut_palette_selection(&mut self, delta: i32) -> Task<Message> {
        let actions = self.shortcut_palette_actions();
        if actions.is_empty() {
            self.shortcuts_selected_action = None;
            return Task::none();
        }

        let current_index = self
            .shortcuts_selected_action
            .and_then(|selected| actions.iter().position(|metadata| metadata.id == selected))
            .unwrap_or(0);
        let next_index = (current_index as i32 + delta).clamp(0, actions.len() as i32 - 1);
        self.shortcuts_selected_action = Some(actions[next_index as usize].id);
        self.reveal_selected_shortcut_action()
    }

    fn reveal_selected_shortcut_action(&self) -> Task<Message> {
        let Some(selected) = self.shortcuts_selected_action else {
            return Task::none();
        };
        let Some(index) = self
            .shortcut_palette_actions()
            .iter()
            .position(|metadata| metadata.id == selected)
        else {
            return Task::none();
        };

        iced::widget::operation::scroll_to(
            super::SHORTCUTS_SCROLLABLE_ID,
            iced::widget::operation::AbsoluteOffset {
                x: 0.0,
                y: index as f32 * super::SHORTCUTS_ACTION_ROW_HEIGHT,
            },
        )
    }

    pub(in crate::app) fn handle_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        match action {
            ShortcutAction::QuitApp => self.handle_window_close_requested(),
            ShortcutAction::OpenActions => {
                update(self, Message::Shortcuts(ShortcutsMessage::OpenDialog))
            }
            ShortcutAction::NewEditor => update(self, Message::Editor(EditorMessage::NewRequested)),
            ShortcutAction::OpenEditorFile => {
                update(self, Message::Editor(EditorMessage::OpenRequested))
            }
            ShortcutAction::SaveEditor => {
                update(self, Message::Editor(EditorMessage::SaveRequested))
            }
            ShortcutAction::EditorUndo => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Undo)
            }
            ShortcutAction::EditorRedo => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Redo)
            }
            ShortcutAction::EditorCopy => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Copy)
            }
            ShortcutAction::EditorPaste => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::Paste(String::new()),
            ),
            ShortcutAction::EditorOpenSearch => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::OpenSearch)
            }
            ShortcutAction::EditorOpenSearchReplace => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::OpenSearchReplace,
            ),
            ShortcutAction::EditorOpenGotoLine => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::OpenGotoLine)
            }
            ShortcutAction::EditorTriggerCompletion => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::TriggerCompletion,
            ),
            ShortcutAction::EditorFindNext => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::FindNext)
            }
            ShortcutAction::EditorFindPrevious => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::FindPrevious)
            }
            ShortcutAction::EditorWordLeft => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::WordArrowKey(
                    iced_code_editor::ArrowDirection::Left,
                    false,
                ))
            }
            ShortcutAction::EditorWordRight => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::WordArrowKey(
                    iced_code_editor::ArrowDirection::Right,
                    false,
                ))
            }
            ShortcutAction::EditorWordLeftSelect => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::WordArrowKey(
                    iced_code_editor::ArrowDirection::Left,
                    true,
                ))
            }
            ShortcutAction::EditorWordRightSelect => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::WordArrowKey(
                    iced_code_editor::ArrowDirection::Right,
                    true,
                ))
            }
            ShortcutAction::EditorDeleteWordBackward => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DeleteWordBackward,
            ),
            ShortcutAction::EditorDeleteWordForward => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DeleteWordForward,
            ),
            ShortcutAction::EditorDeleteToLineStart => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DeleteToLineStart,
            ),
            ShortcutAction::EditorDeleteToLineEnd => self
                .dispatch_active_editor_widget_message(iced_code_editor::Message::DeleteToLineEnd),
            ShortcutAction::EditorLineStart => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Home(false))
            }
            ShortcutAction::EditorLineEnd => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::End(false))
            }
            ShortcutAction::EditorLineStartSelect => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Home(true))
            }
            ShortcutAction::EditorLineEndSelect => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::End(true))
            }
            ShortcutAction::EditorDocumentStart => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DocumentHome(false),
            ),
            ShortcutAction::EditorDocumentEnd => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DocumentEnd(false),
            ),
            ShortcutAction::EditorDocumentStartSelect => self
                .dispatch_active_editor_widget_message(iced_code_editor::Message::DocumentHome(
                    true,
                )),
            ShortcutAction::EditorDocumentEndSelect => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::DocumentEnd(true),
            ),
            ShortcutAction::EditorDeleteSelection => self
                .dispatch_active_editor_widget_message(iced_code_editor::Message::DeleteSelection),
            ShortcutAction::EditorSelectAll => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::SelectAll)
            }
            ShortcutAction::EditorInsertLineBelow => self
                .dispatch_active_editor_widget_message(iced_code_editor::Message::InsertLineBelow),
            ShortcutAction::EditorInsertLineAbove => self
                .dispatch_active_editor_widget_message(iced_code_editor::Message::InsertLineAbove),
            ShortcutAction::EditorDeleteLine => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::DeleteLine)
            }
            ShortcutAction::EditorMoveLineUp => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::MoveLineUp)
            }
            ShortcutAction::EditorMoveLineDown => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::MoveLineDown)
            }
            ShortcutAction::EditorCopyLineUp => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::CopyLineUp)
            }
            ShortcutAction::EditorCopyLineDown => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::CopyLineDown)
            }
            ShortcutAction::EditorJoinLines => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::JoinLines)
            }
            ShortcutAction::EditorIndent => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::Tab)
            }
            ShortcutAction::EditorOutdent => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::ShiftTab)
            }
            ShortcutAction::EditorToggleLineComment => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::ToggleLineComment,
            ),
            ShortcutAction::EditorToggleBlockComment => self.dispatch_active_editor_widget_message(
                iced_code_editor::Message::ToggleBlockComment,
            ),
            ShortcutAction::EditorSelectLine => {
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::SelectLine)
            }
            ShortcutAction::EditorJumpToMatchingBracket => self
                .dispatch_active_editor_widget_message(
                    iced_code_editor::Message::JumpToMatchingBracket,
                ),
            ShortcutAction::CloseEditorTab => {
                let Some(tab_id) = self.editor.active_tab_id() else {
                    return Task::none();
                };
                update(
                    self,
                    Message::Editor(EditorMessage::CloseTabRequested(tab_id)),
                )
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
            ShortcutAction::SwitchEditorTabPrevious => {
                let Some(tab_id) = self.editor.activate_adjacent_tab(false) else {
                    return Task::none();
                };
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.pending_reveal_editor_tab = Some(tab_id);
                self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id))
            }
            ShortcutAction::SwitchEditorTabNext => {
                let Some(tab_id) = self.editor.activate_adjacent_tab(true) else {
                    return Task::none();
                };
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.request_focus();
                self.pending_reveal_editor_tab = Some(tab_id);
                self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id))
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
                        Some(PromptOkAction::ReloadEditorTab(tab_id)) => {
                            self.reload_editor_tab_from_disk(tab_id)
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
        let dirty_tabs = self.editor.tabs_requiring_resolution();
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
        self.patch_macos_quit_menu();
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
        let prompt =
            if self.editor.tab_file_state(tab_id) == Some(EditorTabFileState::MissingOnDisk) {
                ErrorPrompt::new(
                format!("Save {title}?"),
                format!(
                    "{title} is missing on disk. Save it before continuing to recreate the file?"
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
            };
        self.error_prompt = Some(prompt);
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
                self.sync_editor_file_watcher();
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
