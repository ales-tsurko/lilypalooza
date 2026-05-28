use super::*;

type ShortcutRoute = fn(&mut Lilypalooza, ShortcutAction) -> Option<Task<Message>>;

impl Lilypalooza {
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
                if self.shortcut_modifier_active() {
                    return Task::none();
                }
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
                let Some(action) = shortcuts::action_for_id(action_id) else {
                    return Task::none();
                };
                self.handle_shortcut_action(action)
            }
        }
    }

    pub(super) fn shortcut_palette_actions(&self) -> Vec<crate::shortcuts::ShortcutActionMetadata> {
        shortcuts::filtered_action_metadata(&self.shortcuts_search_query)
    }

    pub(super) fn reconcile_shortcut_palette_selection(&mut self) -> Task<Message> {
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

    pub(super) fn move_shortcut_palette_selection(&mut self, delta: i32) -> Task<Message> {
        let actions = self.shortcut_palette_actions();
        if actions.is_empty() {
            self.shortcuts_selected_action = None;
            return Task::none();
        }

        let current_index = self
            .shortcuts_selected_action
            .and_then(|selected| actions.iter().position(|metadata| metadata.id == selected))
            .unwrap_or(0);
        let next_index = if delta < 0 {
            current_index
                .saturating_sub(usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX))
        } else {
            current_index
                .saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
                .min(actions.len().saturating_sub(1))
        };
        self.shortcuts_selected_action = actions.get(next_index).map(|metadata| metadata.id);
        self.reveal_selected_shortcut_action()
    }

    pub(super) fn reveal_selected_shortcut_action(&self) -> Task<Message> {
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
            ShortcutAction::QuitApp
            | ShortcutAction::OpenActions
            | ShortcutAction::OpenSettingsFile => self.handle_app_shortcut_action(action),
            ShortcutAction::FileBrowserUndo
            | ShortcutAction::FileBrowserRedo
            | ShortcutAction::FileBrowserCut
            | ShortcutAction::FileBrowserCopy
            | ShortcutAction::FileBrowserPaste
            | ShortcutAction::FileBrowserRename
            | ShortcutAction::FileBrowserDelete => self.handle_file_browser_shortcut_action(action),
            ShortcutAction::NewEditor
            | ShortcutAction::OpenEditorFile
            | ShortcutAction::ToggleFileBrowser
            | ShortcutAction::SaveEditor
            | ShortcutAction::CloseEditorTab
            | ShortcutAction::SwitchEditorTabPrevious
            | ShortcutAction::SwitchEditorTabNext
            | ShortcutAction::EditorZoomIn
            | ShortcutAction::EditorZoomOut
            | ShortcutAction::EditorZoomReset => self.handle_editor_shell_shortcut_action(action),
            ShortcutAction::EditorUndo
            | ShortcutAction::EditorRedo
            | ShortcutAction::EditorCopy
            | ShortcutAction::EditorPaste
            | ShortcutAction::EditorOpenSearch
            | ShortcutAction::EditorOpenSearchReplace
            | ShortcutAction::EditorOpenGotoLine
            | ShortcutAction::EditorTriggerCompletion
            | ShortcutAction::EditorFindNext
            | ShortcutAction::EditorFindPrevious
            | ShortcutAction::EditorWordLeft
            | ShortcutAction::EditorWordRight
            | ShortcutAction::EditorWordLeftSelect
            | ShortcutAction::EditorWordRightSelect
            | ShortcutAction::EditorDeleteWordBackward
            | ShortcutAction::EditorDeleteWordForward
            | ShortcutAction::EditorDeleteToLineStart
            | ShortcutAction::EditorDeleteToLineEnd
            | ShortcutAction::EditorLineStart
            | ShortcutAction::EditorLineEnd
            | ShortcutAction::EditorLineStartSelect
            | ShortcutAction::EditorLineEndSelect
            | ShortcutAction::EditorDocumentStart
            | ShortcutAction::EditorDocumentEnd
            | ShortcutAction::EditorDocumentStartSelect
            | ShortcutAction::EditorDocumentEndSelect
            | ShortcutAction::EditorDeleteSelection
            | ShortcutAction::EditorSelectAll
            | ShortcutAction::EditorInsertLineBelow
            | ShortcutAction::EditorInsertLineAbove
            | ShortcutAction::EditorDeleteLine
            | ShortcutAction::EditorMoveLineUp
            | ShortcutAction::EditorMoveLineDown
            | ShortcutAction::EditorCopyLineUp
            | ShortcutAction::EditorCopyLineDown
            | ShortcutAction::EditorJoinLines
            | ShortcutAction::EditorIndent
            | ShortcutAction::EditorOutdent
            | ShortcutAction::EditorToggleLineComment
            | ShortcutAction::EditorToggleBlockComment
            | ShortcutAction::EditorSelectLine
            | ShortcutAction::EditorJumpToMatchingBracket => {
                self.handle_editor_widget_shortcut_action(action)
            }
            ShortcutAction::ToggleWorkspacePane(_)
            | ShortcutAction::SwitchWorkspaceTabPrevious
            | ShortcutAction::SwitchWorkspaceTabNext
            | ShortcutAction::FocusWorkspacePanePrevious
            | ShortcutAction::FocusWorkspacePaneNext => {
                self.handle_workspace_shortcut_action(action)
            }
            ShortcutAction::ScoreZoomIn
            | ShortcutAction::ScoreZoomOut
            | ShortcutAction::ScoreZoomReset
            | ShortcutAction::ScoreScrollUp
            | ShortcutAction::ScoreScrollDown
            | ShortcutAction::ScorePrevPage
            | ShortcutAction::ScoreNextPage => self.handle_score_shortcut_action(action),
            ShortcutAction::PianoRollZoomIn
            | ShortcutAction::PianoRollZoomOut
            | ShortcutAction::PianoRollZoomReset
            | ShortcutAction::PianoRollCursorSubdivisionPrevious
            | ShortcutAction::PianoRollCursorSubdivisionNext
            | ShortcutAction::PianoRollScrollUp
            | ShortcutAction::PianoRollScrollDown => self.handle_piano_roll_shortcut_action(action),
            ShortcutAction::TransportPlayPause
            | ShortcutAction::TransportRewind
            | ShortcutAction::ToggleMetronome => self.handle_transport_shortcut_action(action),
        }
    }

    pub(super) fn handle_app_shortcut_action(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::QuitApp => self.handle_window_close_requested(self.main_window_id),
            ShortcutAction::OpenActions => {
                update(self, Message::Shortcuts(ShortcutsMessage::OpenDialog))
            }
            ShortcutAction::OpenSettingsFile => self.open_settings_file_in_editor(),
            _ => Task::none(),
        }
    }

    pub(super) fn handle_file_browser_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        if action == ShortcutAction::FileBrowserUndo {
            return self.undo_browser_operation();
        }
        if action == ShortcutAction::FileBrowserRedo {
            return self.redo_browser_operation();
        }
        file_browser_shortcut_message(action)
            .map(|message| update(self, Message::Editor(message)))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn handle_editor_shell_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        self.route_shortcut_action(
            action,
            [
                Self::editor_file_shell_shortcut_task,
                Self::handle_editor_tab_shell_shortcut,
            ],
            Self::handle_editor_zoom_shell_shortcut,
        )
    }

    fn route_shortcut_action<const N: usize>(
        &mut self,
        action: ShortcutAction,
        routes: [ShortcutRoute; N],
        fallback: fn(&mut Self, ShortcutAction) -> Task<Message>,
    ) -> Task<Message> {
        routes
            .into_iter()
            .find_map(|route| route(self, action))
            .unwrap_or_else(|| fallback(self, action))
    }

    fn editor_file_shell_shortcut_task(&mut self, action: ShortcutAction) -> Option<Task<Message>> {
        editor_file_shell_message(action).map(|message| update(self, Message::Editor(message)))
    }

    pub(super) fn handle_editor_tab_shell_shortcut(
        &mut self,
        action: ShortcutAction,
    ) -> Option<Task<Message>> {
        match action {
            ShortcutAction::CloseEditorTab => Some(self.close_active_editor_tab()),
            ShortcutAction::SwitchEditorTabPrevious => Some(self.switch_editor_tab(false)),
            ShortcutAction::SwitchEditorTabNext => Some(self.switch_editor_tab(true)),
            _ => None,
        }
    }

    pub(super) fn handle_editor_zoom_shell_shortcut(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        match action {
            ShortcutAction::EditorZoomIn => self.zoom_editor(EditorZoomDirection::In),
            ShortcutAction::EditorZoomOut => self.zoom_editor(EditorZoomDirection::Out),
            ShortcutAction::EditorZoomReset => self.zoom_editor(EditorZoomDirection::Reset),
            _ => Task::none(),
        }
    }

    pub(super) fn close_active_editor_tab(&mut self) -> Task<Message> {
        let Some(tab_id) = self.editor.active_tab_id() else {
            return Task::none();
        };
        update(
            self,
            Message::Editor(EditorMessage::CloseTabRequested(tab_id)),
        )
    }

    pub(super) fn switch_editor_tab(&mut self, next: bool) -> Task<Message> {
        let Some(tab_id) = self.editor.activate_adjacent_tab(next) else {
            return Task::none();
        };
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_text_area();
        self.pending_reveal_editor_tab = Some(tab_id);
        self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id))
    }

    pub(super) fn zoom_editor(&mut self, direction: EditorZoomDirection) -> Task<Message> {
        match direction {
            EditorZoomDirection::In => self.editor.zoom_in(),
            EditorZoomDirection::Out => self.editor.zoom_out(),
            EditorZoomDirection::Reset => self.editor.reset_zoom(),
        }
        self.persist_settings();
        Task::none()
    }

    pub(super) fn handle_editor_widget_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        if matches!(action, ShortcutAction::EditorUndo)
            && self.focused_workspace_pane == Some(WorkspacePaneKind::Mixer)
        {
            return self.undo_mixer_operation();
        }
        if matches!(action, ShortcutAction::EditorRedo)
            && self.focused_workspace_pane == Some(WorkspacePaneKind::Mixer)
        {
            return self.redo_mixer_operation();
        }

        editor_widget_message_for_shortcut(action)
            .map(|message| self.dispatch_active_editor_widget_message(message))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn handle_workspace_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        self.route_shortcut_action(
            action,
            [
                Self::workspace_pane_shortcut_task,
                Self::workspace_tab_shortcut_task,
            ],
            Self::workspace_focus_shortcut_task,
        )
    }

    fn workspace_pane_shortcut_task(&mut self, action: ShortcutAction) -> Option<Task<Message>> {
        toggle_workspace_pane_shortcut(action)
            .map(|pane| update(self, Message::Pane(PaneMessage::ToggleWorkspacePane(pane))))
    }

    fn workspace_tab_shortcut_task(&mut self, action: ShortcutAction) -> Option<Task<Message>> {
        workspace_tab_shortcut_direction(action)
            .map(|direction| self.switch_workspace_tab_shortcut(direction))
    }

    fn workspace_focus_shortcut_task(&mut self, action: ShortcutAction) -> Task<Message> {
        workspace_focus_shortcut_direction(action)
            .map(|direction| self.focus_workspace_pane_shortcut(direction))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn switch_workspace_tab_shortcut(
        &mut self,
        direction: TabDirection,
    ) -> Task<Message> {
        self.switch_focused_workspace_tab(direction);
        Task::none()
    }

    pub(super) fn focus_workspace_pane_shortcut(
        &mut self,
        direction: PaneCycleDirection,
    ) -> Task<Message> {
        self.cycle_workspace_pane_focus(direction);
        Task::none()
    }

    pub(super) fn handle_score_shortcut_action(&mut self, action: ShortcutAction) -> Task<Message> {
        if let Some(message) = score_shortcut_message(action) {
            return update(self, Message::Viewer(message));
        }
        Task::none()
    }

    pub(super) fn handle_piano_roll_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        if let Some(message) = piano_roll_shortcut_message(action) {
            return update(self, Message::PianoRoll(message));
        }
        self.handle_piano_roll_navigation_shortcut(action)
    }

    pub(super) fn handle_piano_roll_navigation_shortcut(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        if let Some(next) = piano_roll_cursor_subdivision_direction(action) {
            return self.move_piano_roll_cursor_by_subdivision(next);
        }
        match action {
            ShortcutAction::PianoRollScrollUp => self.scroll_piano_roll(-KEYBOARD_SCROLL_STEP),
            ShortcutAction::PianoRollScrollDown => self.scroll_piano_roll(KEYBOARD_SCROLL_STEP),
            _ => Task::none(),
        }
    }

    pub(super) fn move_piano_roll_cursor_by_subdivision(&mut self, next: bool) -> Task<Message> {
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
            next,
        );
        update(
            self,
            Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)),
        )
    }

    pub(super) fn scroll_piano_roll(&self, y: f32) -> Task<Message> {
        iced::widget::operation::scroll_by(
            roll_scroll_id(),
            iced::widget::operation::AbsoluteOffset { x: 0.0, y },
        )
    }

    pub(super) fn handle_transport_shortcut_action(
        &mut self,
        action: ShortcutAction,
    ) -> Task<Message> {
        match action {
            ShortcutAction::TransportPlayPause => update(
                self,
                Message::PianoRoll(PianoRollMessage::TransportPlayPause),
            ),
            ShortcutAction::TransportRewind => {
                update(self, Message::PianoRoll(PianoRollMessage::TransportRewind))
            }
            ShortcutAction::ToggleMetronome => {
                if self.focused_workspace_pane() == Some(WorkspacePaneKind::Editor) {
                    Task::none()
                } else {
                    update(
                        self,
                        Message::PianoRoll(PianoRollMessage::TransportToggleMetronome),
                    )
                }
            }
            _ => Task::none(),
        }
    }
}
