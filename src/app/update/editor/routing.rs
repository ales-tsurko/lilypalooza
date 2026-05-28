use super::*;

impl Lilypalooza {
    pub(in crate::app) fn handle_editor_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        self.route_editor_message(message)
            .unwrap_or_else(Task::none)
    }

    pub(super) fn route_editor_message(&mut self, message: EditorMessage) -> Option<Task<Message>> {
        match message {
            EditorMessage::Widget { tab_id, message } => {
                Some(self.handle_editor_widget_message(tab_id, message))
            }
            EditorMessage::ActiveWidgetMessage(message) => {
                Some(self.handle_active_editor_widget_message(message))
            }
            message => self.route_editor_by_stage(message, EditorRouteStage::State),
        }
    }

    fn route_editor_by_stage(
        &mut self,
        message: EditorMessage,
        stage: EditorRouteStage,
    ) -> Option<Task<Message>> {
        let next_stage = match stage {
            EditorRouteStage::State if editor_tab_message(&message) => {
                return Some(self.handle_editor_tab_message(message));
            }
            EditorRouteStage::State if editor_rename_message(&message) => {
                return Some(self.handle_editor_rename_message(message));
            }
            EditorRouteStage::State => EditorRouteStage::FileBrowser,
            EditorRouteStage::FileBrowser
                if file_browser_command_message(&message)
                    || file_browser_scroll_message(&message)
                    || file_browser_entry_message(&message) =>
            {
                return Some(self.handle_file_browser_message(message));
            }
            EditorRouteStage::FileBrowser => EditorRouteStage::FileOrAppearance,
            EditorRouteStage::FileOrAppearance if editor_file_message(&message) => {
                return Some(self.handle_editor_file_message(message));
            }
            EditorRouteStage::FileOrAppearance if editor_appearance_message(&message) => {
                return Some(self.handle_editor_appearance_message(message));
            }
            EditorRouteStage::FileOrAppearance => return None,
        };
        self.route_editor_by_stage(message, next_stage)
    }
}

#[derive(Debug, Clone, Copy)]
enum EditorRouteStage {
    State,
    FileBrowser,
    FileOrAppearance,
}

impl Lilypalooza {
    pub(super) fn handle_editor_widget_message(
        &mut self,
        tab_id: u64,
        message: iced_code_editor::Message,
    ) -> Task<Message> {
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

    pub(super) fn handle_active_editor_widget_message(
        &mut self,
        message: iced_code_editor::Message,
    ) -> Task<Message> {
        let Some(tab_id) = self.editor.active_tab_id() else {
            return Task::none();
        };
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_text_area();
        self.editor.activate_tab(tab_id);
        let task = self.editor.update(tab_id, &message);
        self.map_editor_widget_task(tab_id, task)
    }

    pub(super) fn handle_editor_tab_message(&mut self, message: EditorMessage) -> Task<Message> {
        if let Some(task) = self.handle_editor_tab_lifecycle_message(&message) {
            return task;
        }
        if editor_tab_drag_message(&message) {
            return self.handle_editor_tab_drag_message(message);
        }
        match message {
            EditorMessage::CloseTabRequested(tab_id) => self.close_editor_tab_from_tabbar(tab_id),
            _ => Task::none(),
        }
    }

    pub(super) fn handle_editor_tab_lifecycle_message(
        &mut self,
        message: &EditorMessage,
    ) -> Option<Task<Message>> {
        match message {
            EditorMessage::NewRequested => Some(self.create_new_editor_tab()),
            EditorMessage::TabPressed(tab_id) => Some(self.press_editor_tab(*tab_id)),
            _ => None,
        }
    }

    pub(super) fn create_new_editor_tab(&mut self) -> Task<Message> {
        self.close_editor_menus();
        self.cancel_editor_tab_rename_state();
        let (tab_id, task, _reused) = self.editor.new_document();
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.focus_editor_text_area();
        self.pending_reveal_editor_tab = Some(tab_id);
        self.map_editor_widget_task(tab_id, task)
    }

    pub(super) fn press_editor_tab(&mut self, tab_id: u64) -> Task<Message> {
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

    pub(super) fn move_editor_tab(&mut self, tab_id: u64, position: iced::Point) {
        self.hovered_editor_tab = Some(tab_id);
        self.editor_tab_drop_after = position.x >= EDITOR_TAB_WIDTH * 0.5;
        self.move_editor_tabbar_drag(position.x, false);
        self.maybe_start_editor_tab_drag(position);
    }

    pub(super) fn handle_editor_tab_drag_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        self.apply_editor_tab_drag_message(message);
        Task::none()
    }

    pub(super) fn apply_editor_tab_drag_message(&mut self, message: EditorMessage) {
        if self.handle_editor_tab_pointer_message(message.clone()) {
            return;
        }
        if self.handle_editor_tabbar_message(message.clone()) {
            return;
        }
        self.handle_editor_tab_drag_lifecycle_message(message);
    }

    pub(super) fn handle_editor_tabbar_message(&mut self, message: EditorMessage) -> bool {
        match message {
            EditorMessage::TabBarEmptyMoved => self.move_editor_tabbar_empty_area(),
            EditorMessage::TabBarMoved(position) => self.handle_editor_tabbar_moved(position),
            _ => return false,
        }
        true
    }

    pub(super) fn handle_editor_tab_drag_lifecycle_message(&mut self, message: EditorMessage) {
        match message {
            EditorMessage::TabDragReleased => self.release_editor_tab_drag(),
            EditorMessage::TabDragExited => self.handle_editor_tab_drag_exited(),
            _ => {}
        }
    }

    pub(super) fn handle_editor_tabbar_moved(&mut self, position: iced::Point) {
        self.move_editor_tabbar_drag(position.x, true);
        self.maybe_start_editor_tab_drag(position);
    }

    pub(super) fn handle_editor_tab_drag_exited(&mut self) {
        if self.dragged_editor_tab.is_none() {
            self.hovered_editor_tab = None;
        }
    }

    pub(super) fn handle_editor_tab_pointer_message(&mut self, message: EditorMessage) -> bool {
        match message {
            EditorMessage::TabMoved { .. }
            | EditorMessage::TabGlobalMoved(_)
            | EditorMessage::TabHovered(_)
            | EditorMessage::TabBarScrolled(_) => {
                self.apply_editor_tab_pointer_message(message);
                true
            }
            _ => false,
        }
    }

    pub(super) fn apply_editor_tab_pointer_message(&mut self, message: EditorMessage) {
        self.apply_editor_tab_move_message(message.clone())
            .or_else(|| self.apply_editor_tab_hover_message(message.clone()))
            .or_else(|| self.apply_editor_tab_scroll_message(message));
    }

    pub(super) fn apply_editor_tab_move_message(&mut self, message: EditorMessage) -> Option<()> {
        match message {
            EditorMessage::TabMoved { tab_id, position } => self.move_editor_tab(tab_id, position),
            EditorMessage::TabGlobalMoved(position) => {
                self.move_editor_tabbar_drag(position.x, true)
            }
            _ => return None,
        }
        Some(())
    }

    pub(super) fn apply_editor_tab_hover_message(&mut self, message: EditorMessage) -> Option<()> {
        let EditorMessage::TabHovered(tab_id) = message else {
            return None;
        };
        self.hovered_editor_tab = tab_id;
        Some(())
    }

    pub(super) fn apply_editor_tab_scroll_message(&mut self, message: EditorMessage) -> Option<()> {
        let EditorMessage::TabBarScrolled(viewport) = message else {
            return None;
        };
        self.scroll_editor_tabbar(viewport);
        Some(())
    }

    pub(super) fn scroll_editor_tabbar(&mut self, viewport: iced::widget::scrollable::Viewport) {
        self.editor_tabbar_scroll_x = viewport.absolute_offset().x;
        self.editor_tabbar_viewport_width = viewport.bounds().width;
        if self.dragged_editor_tab.is_some()
            && let Some(pointer_x) = self.editor_tabbar_drag_pointer_x
        {
            self.update_editor_drag_target_from_x(pointer_x);
        }
    }

    pub(super) fn close_editor_tab_from_tabbar(&mut self, tab_id: u64) -> Task<Message> {
        if self.renaming_editor_tab == Some(tab_id) {
            self.cancel_editor_tab_rename_state();
        }
        self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        self.request_close_editor_tab(tab_id)
    }

    pub(super) fn move_editor_tabbar_drag(&mut self, pointer_x: f32, update_target: bool) {
        if self.dragged_editor_tab.is_none() {
            return;
        }

        self.editor_tabbar_drag_pointer_x = Some(pointer_x);
        self.update_editor_tabbar_autoscroll(pointer_x);
        if update_target {
            self.update_editor_drag_target_from_x(pointer_x);
        }
    }

    pub(super) fn maybe_start_editor_tab_drag(&mut self, position: iced::Point) {
        if self.dragged_editor_tab.is_some() {
            return;
        }

        let Some(pressed_tab) = self.pressed_editor_tab else {
            return;
        };

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

    pub(super) fn move_editor_tabbar_empty_area(&mut self) {
        if let Some(last_tab) = self.editor.tab_ids().last().copied() {
            self.hovered_editor_tab = Some(last_tab);
            self.editor_tab_drop_after = true;
        } else {
            self.hovered_editor_tab = None;
        }
    }

    pub(super) fn release_editor_tab_drag(&mut self) {
        if let (Some(dragged_tab), Some(target_tab)) =
            (self.dragged_editor_tab, self.hovered_editor_tab)
            && self
                .editor
                .reorder_tabs(dragged_tab, target_tab, self.editor_tab_drop_after)
        {
            self.persist_settings();
        }
        self.clear_editor_tab_drag_state();
    }
}
