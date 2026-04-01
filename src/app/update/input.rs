use super::*;
use crate::app::piano_roll::{adjacent_subdivision_tick, roll_scroll_id};

impl LilyView {
    pub(in crate::app) fn sync_editor_widget_focus(&mut self) {
        if self.focused_workspace_pane == Some(WorkspacePaneKind::Editor) {
            self.editor.request_focus();
        } else {
            self.editor.lose_focus();
        }
    }

    pub(in crate::app) fn handle_key_pressed(&mut self, key_press: KeyPress) -> Task<Message> {
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
            PromptMessage::Cancel => {
                self.error_prompt = None;
                self.prompt_ok_action = None;
                Task::none()
            }
        }
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
}
