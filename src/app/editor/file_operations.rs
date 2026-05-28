use super::*;

impl EditorState {
    pub(in crate::app) fn new_document(&mut self) -> (u64, iced::Task<EditorWidgetMessage>, bool) {
        if let Some(tab_id) = self.find_reusable_empty_tab() {
            self.active_tab_id = Some(tab_id);
            return (tab_id, iced::Task::none(), true);
        }

        let tab_id = self.allocate_tab_id();
        let tab = EditorTab {
            id: tab_id,
            widget: build_editor(
                "",
                "lilypond",
                None,
                self.project_root.clone(),
                &self.app_theme,
                self.view_settings,
                self.theme_settings,
            ),
            path: None,
            saved_content: None,
            file_state: EditorTabFileState::Ok,
        };
        self.tabs.push(tab);
        self.active_tab_id = Some(tab_id);
        (tab_id, iced::Task::none(), false)
    }

    pub(in crate::app) fn load_file(
        &mut self,
        path: &Path,
    ) -> Result<(u64, iced::Task<EditorWidgetMessage>, bool), String> {
        let normalized_path = normalize_editor_path(path);

        if let Some(tab_id) = self.find_tab_by_path(&normalized_path) {
            self.active_tab_id = Some(tab_id);
            return Ok((tab_id, iced::Task::none(), true));
        }

        let text = fs::read_to_string(&normalized_path).map_err(|error| {
            format!(
                "Failed to read editor file {}: {error}",
                normalized_path.display()
            )
        })?;
        let tab_id = self.allocate_tab_id();
        let task = self.load_document_into_tab(tab_id, &text, Some(normalized_path), false)?;
        Ok((tab_id, task, false))
    }

    pub(in crate::app) fn restore_file_tabs(
        &mut self,
        paths: &[PathBuf],
        active_path: Option<&Path>,
        include_clean_untitled: bool,
    ) -> (Vec<(u64, iced::Task<EditorWidgetMessage>)>, Vec<String>) {
        self.tabs.clear();
        self.active_tab_id = None;

        let mut tasks = Vec::new();
        let mut warnings = Vec::new();

        for path in paths {
            match self.load_file(path) {
                Ok((tab_id, task, _)) => tasks.push((tab_id, task)),
                Err(error) => warnings.push(error),
            }
        }

        if include_clean_untitled {
            let (tab_id, task, _) = self.new_document();
            tasks.push((tab_id, task));
        }

        if let Some(active_path) = active_path
            && let Some(tab_id) = self.find_tab_by_path(active_path)
        {
            self.active_tab_id = Some(tab_id);
        }

        if self.active_tab_id.is_none() {
            self.active_tab_id = self.tabs.first().map(|tab| tab.id);
        }

        (tasks, warnings)
    }

    pub(in crate::app) fn save_to_disk(
        &mut self,
        tab_id: u64,
    ) -> Result<(PathBuf, iced::Task<EditorWidgetMessage>), String> {
        let Some(path) = self.tab(tab_id).and_then(|tab| tab.path.clone()) else {
            return Err("No editor file is currently loaded".to_string());
        };

        let task = self.save_to_path(tab_id, &path)?;

        Ok((path, task))
    }

    pub(in crate::app) fn save_to_path(
        &mut self,
        tab_id: u64,
        path: &Path,
    ) -> Result<iced::Task<EditorWidgetMessage>, String> {
        let Some(content) = self.tab(tab_id).map(|tab| tab.widget.content()) else {
            return Err("Editor tab no longer exists".to_string());
        };

        fs::write(path, &content)
            .map_err(|error| format!("Failed to save editor file {}: {error}", path.display()))?;

        let normalized_path = normalize_editor_path(path);
        if let Some(tab) = self.tab_mut(tab_id) {
            let next_syntax = syntax_for_path(&normalized_path);
            tab.widget.set_syntax(&next_syntax);
            tab.widget.set_document_path(Some(normalized_path.clone()));
            tab.path = Some(normalized_path);
            tab.saved_content = Some(content);
            tab.file_state = EditorTabFileState::Ok;
            tab.widget.mark_saved();
        }
        Ok(iced::Task::none())
    }

    pub(in crate::app) fn rename_file(
        &mut self,
        tab_id: u64,
        new_path: &Path,
    ) -> Result<iced::Task<EditorWidgetMessage>, String> {
        let Some(old_path) = self.tab(tab_id).and_then(|tab| tab.path.clone()) else {
            return self.save_to_path(tab_id, new_path);
        };

        if !old_path.exists() {
            return self.save_to_path(tab_id, new_path);
        }

        fs::rename(&old_path, new_path).map_err(|error| {
            format!(
                "Failed to rename editor file {} to {}: {error}",
                old_path.display(),
                new_path.display()
            )
        })?;

        let normalized_path = normalize_editor_path(new_path);
        if let Some(tab) = self.tab_mut(tab_id) {
            tab.widget.set_document_path(Some(normalized_path.clone()));
            tab.path = Some(normalized_path);
            tab.file_state = EditorTabFileState::Ok;
        }

        Ok(iced::Task::none())
    }

    pub(in crate::app) fn remap_open_paths_under(
        &mut self,
        old_root: &Path,
        new_root: &Path,
    ) -> Vec<u64> {
        let old_root = normalize_editor_path(old_root);
        let new_root = normalize_editor_path(new_root);
        let mut updated = Vec::new();

        for tab in &mut self.tabs {
            let Some(path) = tab.path.clone() else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(&old_root) else {
                continue;
            };
            let next_path = new_root.join(relative);
            let next_syntax = syntax_for_path(&next_path);
            tab.widget.set_syntax(&next_syntax);
            tab.widget.set_document_path(Some(next_path.clone()));
            tab.path = Some(next_path);
            tab.file_state = EditorTabFileState::Ok;
            updated.push(tab.id);
        }

        updated
    }

    pub(in crate::app) fn reload_tab_from_disk(
        &mut self,
        tab_id: u64,
    ) -> Result<iced::Task<EditorWidgetMessage>, String> {
        let Some(path) = self.tab(tab_id).and_then(|tab| tab.path.clone()) else {
            return Err("Editor tab is not file-backed".to_string());
        };

        let content = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read editor file {}: {error}", path.display()))?;

        self.load_document_into_tab(tab_id, &content, Some(path), false)
    }

    pub(in crate::app) fn tab_saved_content(&self, tab_id: u64) -> Option<&str> {
        self.tab(tab_id)
            .and_then(|tab| tab.saved_content.as_deref())
    }

    pub(in crate::app) fn set_tab_file_state(
        &mut self,
        tab_id: u64,
        file_state: EditorTabFileState,
    ) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };

        if tab.file_state == file_state {
            return false;
        }

        tab.file_state = file_state;
        true
    }

    pub(in crate::app) fn close_tab(&mut self, tab_id: u64) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };

        let was_active = self.active_tab_id == Some(tab_id);
        self.tabs.remove(index);

        if self.tabs.is_empty() {
            self.active_tab_id = None;
        } else if was_active {
            let next_index = index.saturating_sub(1).min(self.tabs.len() - 1);
            self.active_tab_id = self.tabs.get(next_index).map(|tab| tab.id);
        }

        true
    }

    pub(in crate::app) fn mark_tab_saved(&mut self, tab_id: u64) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };

        tab.widget.mark_saved();
        true
    }

    pub(in crate::app) fn reorder_tabs(
        &mut self,
        dragged_tab_id: u64,
        target_tab_id: u64,
        insert_after_target: bool,
    ) -> bool {
        let Some(from_index) = self.tabs.iter().position(|tab| tab.id == dragged_tab_id) else {
            return false;
        };
        if dragged_tab_id == target_tab_id {
            return false;
        }

        let tab = self.tabs.remove(from_index);
        let Some(target_index) = self.tabs.iter().position(|tab| tab.id == target_tab_id) else {
            self.tabs.insert(from_index.min(self.tabs.len()), tab);
            return false;
        };
        let insert_index = if insert_after_target {
            target_index + 1
        } else {
            target_index
        };
        self.tabs.insert(insert_index.min(self.tabs.len()), tab);
        true
    }

    pub(in crate::app) fn file_backed_tab_paths(&self) -> Vec<PathBuf> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.path.clone())
            .collect()
    }

    pub(in crate::app) fn active_file_backed_tab_path(&self) -> Option<PathBuf> {
        self.active_tab().and_then(|tab| tab.path.as_ref().cloned())
    }

    pub(in crate::app) fn has_clean_untitled_tab(&self) -> bool {
        self.tabs.iter().any(|tab| {
            tab.path.is_none() && !tab.widget.is_modified() && tab.widget.content().is_empty()
        })
    }

    pub(in crate::app) fn lose_focus(&mut self) {
        for tab in &mut self.tabs {
            tab.widget.lose_focus();
        }
    }

    pub(in crate::app) fn request_focus(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.widget.request_focus();
        }
    }

    pub(in crate::app) fn set_tab_cursor(
        &mut self,
        tab_id: u64,
        line: usize,
        column: usize,
    ) -> iced::Task<EditorWidgetMessage> {
        self.tab_mut(tab_id)
            .map(|tab| tab.widget.set_cursor(line, column))
            .unwrap_or_else(iced::Task::none)
    }

    pub(in crate::app) fn view<'a, Message>(
        &'a self,
        open_message: Message,
        map_message: impl Fn(u64, EditorWidgetMessage) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let Some(tab) = self.active_tab() else {
            return container(
                iced::widget::column![
                    text(EMPTY_EDITOR_MESSAGE).size(ui_style::FONT_SIZE_UI_SM),
                    button(text("Open...").size(ui_style::FONT_SIZE_UI_SM))
                        .style(ui_style::button_neutral)
                        .padding([
                            ui_style::PADDING_BUTTON_COMPACT_V,
                            ui_style::PADDING_BUTTON_COMPACT_H,
                        ])
                        .on_press(open_message)
                ]
                .spacing(ui_style::SPACE_SM)
                .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
        };

        let tab_id = tab.id;
        container(
            keyed_column([(
                tab_id,
                tab.widget
                    .view()
                    .map(move |message| map_message(tab_id, message)),
            )])
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.base.color.into()),
                text_color: Some(palette.background.base.text),
                border: border::rounded(ui_style::RADIUS_NONE)
                    .width(0)
                    .color(Color::TRANSPARENT),
                ..container::Style::default()
            }
        })
        .into()
    }

    pub(in crate::app) fn tab_title(&self, tab_id: u64) -> String {
        self.tab_summaries()
            .into_iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| tab.title)
            .unwrap_or_else(|| "Untitled".to_string())
    }
}
