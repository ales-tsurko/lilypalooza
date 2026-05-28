use super::*;

impl Lilypalooza {
    pub(super) fn dispatch_active_editor_widget_message(
        &mut self,
        message: iced_code_editor::Message,
    ) -> Task<Message> {
        update(
            self,
            Message::Editor(EditorMessage::ActiveWidgetMessage(message)),
        )
    }

    pub(in crate::app) fn sync_editor_widget_focus(&mut self) {
        if self.focused_workspace_pane == Some(WorkspacePaneKind::Editor)
            && !self.editor_file_browser_focused
        {
            self.editor.request_focus();
        } else {
            self.editor.lose_focus();
        }
    }

    pub(in crate::app) fn focus_editor_file_browser(&mut self) {
        self.focused_workspace_pane = Some(WorkspacePaneKind::Editor);
        self.editor_file_browser_focused = true;
        self.editor.lose_focus();
    }

    pub(in crate::app) fn focus_editor_text_area(&mut self) {
        self.editor_file_browser_focused = false;
        self.sync_editor_widget_focus();
    }

    pub(in crate::app) fn handle_key_pressed(&mut self, key_press: KeyPress) -> Task<Message> {
        if let Some(task) = self.handle_modal_key_pressed(&key_press) {
            return task;
        }

        let shortcut_input =
            ShortcutInput::new(&key_press.key, key_press.physical_key, key_press.modifiers);

        if let Some(task) = self.handle_global_shortcut_key(shortcut_input) {
            return task;
        }

        if let Some(task) = self.handle_file_browser_navigation_key(&key_press) {
            return task;
        }

        let Some(focused_pane) = self.focused_workspace_pane() else {
            return Task::none();
        };

        if let Some(task) =
            self.handle_pre_capture_contextual_shortcut(focused_pane, shortcut_input, &key_press)
        {
            return task;
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

    pub(super) fn handle_modal_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        if self.error_prompt.is_some() {
            return Some(self.handle_prompt_key_pressed(key_press.clone()));
        }
        if self.open_shortcuts_dialog {
            return Some(self.handle_shortcuts_dialog_key_pressed(key_press));
        }
        if let Some(task) = self.handle_inline_editor_key_pressed(key_press) {
            return Some(task);
        }
        if let Some(task) = self.handle_transient_overlay_key_pressed(key_press) {
            return Some(task);
        }
        if self.editor_escape_closes_popups(key_press) {
            return Some(Task::batch([
                self.dispatch_active_editor_widget_message(iced_code_editor::Message::CloseSearch),
                self.dispatch_active_editor_widget_message(
                    iced_code_editor::Message::CloseGotoLine,
                ),
            ]));
        }
        None
    }

    pub(super) fn handle_shortcuts_dialog_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Task<Message> {
        shortcuts_dialog_key_message(&key_press.key)
            .map(|message| update(self, Message::Shortcuts(message)))
            .unwrap_or_else(Task::none)
    }

    pub(super) fn handle_inline_editor_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        self.handle_editor_tab_rename_key_pressed(key_press)
            .or_else(|| self.handle_browser_inline_edit_key_pressed(key_press))
            .or_else(|| self.handle_focused_browser_key_pressed(key_press))
    }

    pub(super) fn handle_editor_tab_rename_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        self.handle_active_key_modal(
            self.renaming_editor_tab.is_some(),
            key_press,
            Some(Message::Editor(EditorMessage::CancelRename)),
            None,
        )
    }

    pub(super) fn handle_browser_inline_edit_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        self.handle_active_key_modal(
            self.browser_inline_edit.is_some(),
            key_press,
            Some(Message::Editor(EditorMessage::CancelFileBrowserInlineEdit)),
            Some(Message::Editor(EditorMessage::CommitFileBrowserInlineEdit)),
        )
    }

    fn handle_active_key_modal(
        &mut self,
        active: bool,
        key_press: &KeyPress,
        escape_message: Option<Message>,
        enter_message: Option<Message>,
    ) -> Option<Task<Message>> {
        if !active {
            return None;
        }
        if key_is_escape(&key_press.key)
            && let Some(message) = escape_message
        {
            return Some(update(self, message));
        }
        if key_is_enter(&key_press.key)
            && let Some(message) = enter_message
        {
            return Some(update(self, message));
        }
        event_is_captured(key_press).then(Task::none)
    }

    pub(super) fn handle_focused_browser_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        if self.editor_file_browser_focused
            && event_is_captured(key_press)
            && key_is_enter(&key_press.key)
        {
            return Some(Task::none());
        }
        None
    }

    pub(super) fn handle_transient_overlay_key_pressed(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        if (self.renaming_target.is_some() || self.track_color_picker_target.is_some())
            && key_is_escape(&key_press.key)
        {
            return Some(match self.renaming_origin {
                Some(WorkspacePaneKind::PianoRoll) => update(
                    self,
                    Message::PianoRoll(PianoRollMessage::CancelTrackRename),
                ),
                Some(WorkspacePaneKind::Mixer) => {
                    update(self, Message::Mixer(MixerMessage::CancelTrackRename))
                }
                _ => {
                    self.cancel_track_rename();
                    Task::none()
                }
            });
        }
        if self.metronome_menu_open && key_is_escape(&key_press.key) {
            return Some(update(
                self,
                Message::PianoRoll(PianoRollMessage::TransportCloseMetronomeMenu),
            ));
        }
        if self.open_processor_browser_target.is_some() && key_is_escape(&key_press.key) {
            return Some(update(
                self,
                Message::Mixer(MixerMessage::CloseProcessorBrowser),
            ));
        }
        if self.renaming_target.is_some() && event_is_captured(key_press) {
            return Some(Task::none());
        }
        None
    }

    pub(super) fn editor_escape_closes_popups(&self, key_press: &KeyPress) -> bool {
        self.focused_workspace_pane() == Some(WorkspacePaneKind::Editor)
            && key_is_escape(&key_press.key)
    }

    pub(super) fn handle_global_shortcut_key(
        &mut self,
        shortcut_input: ShortcutInput<'_>,
    ) -> Option<Task<Message>> {
        if let Some(action) = shortcuts::resolve_global(&self.shortcut_settings, shortcut_input) {
            if action == ShortcutAction::ToggleMetronome
                && self.focused_workspace_pane() == Some(WorkspacePaneKind::Editor)
            {
                return Some(Task::none());
            }
            return Some(self.handle_shortcut_action(action));
        }

        shortcuts::resolve_navigation(&self.shortcut_settings, shortcut_input)
            .map(|action| self.handle_shortcut_action(action))
    }

    pub(super) fn handle_file_browser_navigation_key(
        &mut self,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        if !self.editor_file_browser_focused {
            return None;
        }

        if let Some(direction) = file_browser_vertical_navigation(&key_press.key) {
            return Some(self.handle_editor_file_browser_move(direction));
        }
        file_browser_horizontal_navigation(&key_press.key)
            .map(|forward| self.handle_editor_file_browser_column(forward))
    }

    pub(super) fn handle_pre_capture_contextual_shortcut(
        &mut self,
        focused_pane: WorkspacePaneKind,
        shortcut_input: ShortcutInput<'_>,
        key_press: &KeyPress,
    ) -> Option<Task<Message>> {
        if focused_pane == WorkspacePaneKind::Editor {
            return self.handle_editor_pre_capture_shortcut(focused_pane, shortcut_input);
        }

        if !shortcut_requires_pre_capture_modifier(key_press) {
            return None;
        }

        self.contextual_shortcut_task(focused_pane, shortcut_input)
    }

    pub(super) fn handle_editor_pre_capture_shortcut(
        &mut self,
        focused_pane: WorkspacePaneKind,
        shortcut_input: ShortcutInput<'_>,
    ) -> Option<Task<Message>> {
        if self.editor_file_browser_focused
            && let Some(action) =
                shortcuts::resolve_editor_browser(&self.shortcut_settings, shortcut_input)
        {
            return Some(self.handle_shortcut_action(action));
        }
        self.contextual_shortcut_task(focused_pane, shortcut_input)
    }

    pub(super) fn contextual_shortcut_task(
        &mut self,
        focused_pane: WorkspacePaneKind,
        shortcut_input: ShortcutInput<'_>,
    ) -> Option<Task<Message>> {
        shortcuts::resolve_contextual(&self.shortcut_settings, focused_pane, shortcut_input)
            .map(|action| self.handle_shortcut_action(action))
    }

    pub(super) fn handle_prompt_key_pressed(&mut self, key_press: KeyPress) -> Task<Message> {
        if key_is_enter(&key_press.key) {
            return update(self, Message::Prompt(self.selected_prompt_message()));
        }
        if prompt_key_cycles_forward(&key_press.key) {
            self.cycle_prompt_selection(if key_press.modifiers.shift() { -1 } else { 1 });
            return Task::none();
        }
        if matches!(
            key_press.key,
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft)
        ) {
            self.cycle_prompt_selection(-1);
            return Task::none();
        }
        self.handle_prompt_escape_key(&key_press.key)
    }

    pub(super) fn handle_prompt_escape_key(&mut self, key: &keyboard::Key) -> Task<Message> {
        if key_is_escape(key) && self.prompt_has_button(PromptSelectedButton::Cancel) {
            return update(self, Message::Prompt(PromptMessage::Cancel));
        }
        Task::none()
    }

    pub(super) fn selected_prompt_message(&self) -> PromptMessage {
        match self.prompt_selected_button {
            PromptSelectedButton::Ok => PromptMessage::Acknowledge,
            PromptSelectedButton::Discard => PromptMessage::Discard,
            PromptSelectedButton::Cancel => PromptMessage::Cancel,
        }
    }

    pub(super) fn prompt_has_button(&self, button: PromptSelectedButton) -> bool {
        let Some(prompt) = &self.error_prompt else {
            return false;
        };
        prompt_buttons(prompt.buttons()).contains(&button)
    }

    pub(super) fn cycle_prompt_selection(&mut self, direction: i8) {
        let Some(prompt) = &self.error_prompt else {
            return;
        };
        let buttons = prompt_buttons(prompt.buttons());
        let Some(current_index) = buttons
            .iter()
            .position(|candidate| *candidate == self.prompt_selected_button)
        else {
            if let Some(button) = buttons.first().copied() {
                self.prompt_selected_button = button;
            }
            return;
        };
        let next_index = if direction < 0 {
            current_index.checked_sub(1).unwrap_or(buttons.len() - 1)
        } else {
            (current_index + 1) % buttons.len()
        };
        if let Some(button) = buttons.get(next_index).copied() {
            self.prompt_selected_button = button;
        }
    }
}
