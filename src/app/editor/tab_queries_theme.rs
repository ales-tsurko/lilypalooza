use super::*;

impl EditorState {
    pub(in crate::app) fn has_document(&self) -> bool {
        !self.tabs.is_empty()
    }

    #[cfg(test)]
    pub(in crate::app) fn active_content(&self) -> Option<String> {
        self.active_tab().map(|tab| tab.widget.content())
    }

    #[cfg(test)]
    pub(in crate::app) fn active_editor_is_focused(&self) -> bool {
        self.active_tab().is_some_and(|tab| tab.widget.is_focused())
    }

    pub(in crate::app) fn file_browser_active_column_index(&self) -> usize {
        self.file_browser_active_column
    }

    pub(in crate::app) fn file_browser_selected_index(&self, column_index: usize) -> Option<usize> {
        let EditorBrowserColumn::Directory {
            path: _,
            entries,
            selected_path,
        } = self.file_browser_columns.get(column_index)?
        else {
            return None;
        };
        selected_path
            .as_ref()
            .and_then(|selected| entries.iter().position(|entry| &entry.path == selected))
    }

    pub(in crate::app) fn file_browser_has_preview_column(&self, column_index: usize) -> bool {
        self.file_browser_columns.get(column_index + 1).is_some()
    }

    pub(in crate::app) fn path(&self) -> Option<&Path> {
        self.active_tab().and_then(|tab| tab.path.as_deref())
    }

    pub(in crate::app) fn tab_path(&self, tab_id: u64) -> Option<&Path> {
        self.tab(tab_id).and_then(|tab| tab.path.as_deref())
    }

    pub(in crate::app) fn tab_is_modified(&self, tab_id: u64) -> bool {
        self.tab(tab_id).is_some_and(|tab| tab.widget.is_modified())
    }

    pub(in crate::app) fn tab_file_state(&self, tab_id: u64) -> Option<EditorTabFileState> {
        self.tab(tab_id).map(|tab| tab.file_state)
    }

    pub(in crate::app) fn has_dirty_tabs(&self) -> bool {
        self.tabs.iter().any(|tab| tab.widget.is_modified())
    }

    pub(in crate::app) fn tabs_requiring_resolution(&self) -> Vec<u64> {
        self.tabs
            .iter()
            .filter(|tab| {
                tab.widget.is_modified() || tab.file_state == EditorTabFileState::MissingOnDisk
            })
            .map(|tab| tab.id)
            .collect()
    }

    pub(in crate::app) fn tab_ids(&self) -> Vec<u64> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    pub(in crate::app) fn file_backed_tabs(&self) -> Vec<(u64, PathBuf)> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.path.clone().map(|path| (tab.id, path)))
            .collect()
    }

    pub(in crate::app) fn find_tab_by_path(&self, path: &Path) -> Option<u64> {
        let normalized_path = normalize_editor_path(path);
        self.tabs.iter().find_map(|tab| {
            tab.path
                .as_deref()
                .map(normalize_editor_path)
                .filter(|candidate| *candidate == normalized_path)
                .map(|_| tab.id)
        })
    }

    pub(in crate::app) fn suggested_save_name(&self) -> String {
        self.path()
            .and_then(Path::file_name)
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("untitled.ly")
            .to_string()
    }

    pub(in crate::app) fn suggested_rename_name(&self, tab_id: u64) -> String {
        self.tab(tab_id)
            .and_then(|tab| {
                tab.path
                    .as_deref()
                    .and_then(Path::file_name)
                    .and_then(|file_name| file_name.to_str())
            })
            .unwrap_or("untitled.ly")
            .to_string()
    }

    pub(in crate::app) fn theme_settings(&self) -> EditorThemeSettings {
        self.theme_settings
    }

    pub(in crate::app) fn view_settings(&self) -> EditorViewSettings {
        self.view_settings
    }

    pub(in crate::app) fn font_size_points(&self) -> u32 {
        crate::number::f32_to_u32(self.view_settings.font_size)
    }

    pub(in crate::app) fn can_zoom_in(&self) -> bool {
        self.view_settings.font_size < MAX_EDITOR_FONT_SIZE - f32::EPSILON
    }

    pub(in crate::app) fn can_zoom_out(&self) -> bool {
        self.view_settings.font_size > MIN_EDITOR_FONT_SIZE + f32::EPSILON
    }

    pub(in crate::app) fn can_reset_zoom(&self) -> bool {
        (self.view_settings.font_size - self.default_view_settings.font_size).abs() > 1e-4
    }

    pub(in crate::app) fn center_cursor(&self) -> bool {
        self.view_settings.center_cursor
    }

    pub(in crate::app) fn set_center_cursor(&mut self, value: bool) {
        self.view_settings.center_cursor = value;
        for tab in &mut self.tabs {
            tab.widget.set_center_cursor(value);
        }
    }

    pub(in crate::app) fn apply_view_settings(&mut self, settings: EditorViewSettings) {
        self.set_font_size(settings.font_size);
        self.set_center_cursor(settings.center_cursor);
    }

    pub(in crate::app) fn apply_theme_settings(&mut self, settings: EditorThemeSettings) {
        self.theme_settings = settings;
        self.apply_theme();
    }

    pub(in crate::app) fn set_hue_offset_degrees(&mut self, value: f32) {
        self.theme_settings.hue_offset_degrees = value;
        self.apply_theme();
    }

    pub(in crate::app) fn set_saturation(&mut self, value: f32) {
        self.theme_settings.saturation = value;
        self.apply_theme();
    }

    pub(in crate::app) fn set_warmth(&mut self, value: f32) {
        self.theme_settings.warmth = value;
        self.apply_theme();
    }

    pub(in crate::app) fn set_brightness(&mut self, value: f32) {
        self.theme_settings.brightness = value;
        self.apply_theme();
    }

    pub(in crate::app) fn set_text_dim(&mut self, value: f32) {
        self.theme_settings.text_dim = value;
        self.apply_theme();
    }

    pub(in crate::app) fn set_comment_dim(&mut self, value: f32) {
        self.theme_settings.comment_dim = value;
        self.apply_theme();
    }

    pub(in crate::app) fn zoom_in(&mut self) {
        let next = (self.view_settings.font_size + EDITOR_FONT_SIZE_STEP).min(MAX_EDITOR_FONT_SIZE);
        self.set_font_size(next);
    }

    pub(in crate::app) fn zoom_out(&mut self) {
        let next = (self.view_settings.font_size - EDITOR_FONT_SIZE_STEP).max(MIN_EDITOR_FONT_SIZE);
        self.set_font_size(next);
    }

    pub(in crate::app) fn reset_zoom(&mut self) {
        self.set_font_size(self.default_view_settings.font_size);
    }

    pub(in crate::app) fn tab_summaries(&self) -> Vec<EditorTabSummary> {
        let mut untitled_counter = 0usize;

        self.tabs
            .iter()
            .map(|tab| {
                let title = if let Some(path) = &tab.path {
                    path.file_name()
                        .and_then(|file_name| file_name.to_str())
                        .unwrap_or("Untitled")
                        .to_string()
                } else {
                    untitled_counter += 1;
                    if untitled_counter == 1 {
                        "Untitled".to_string()
                    } else {
                        format!("Untitled {untitled_counter}")
                    }
                };

                EditorTabSummary {
                    id: tab.id,
                    title,
                    dirty: tab.widget.is_modified(),
                    file_state: tab.file_state,
                    active: self.active_tab_id == Some(tab.id),
                }
            })
            .collect()
    }

    pub(in crate::app) fn activate_tab(&mut self, tab_id: u64) -> bool {
        if self.tab(tab_id).is_none() {
            return false;
        }

        self.active_tab_id = Some(tab_id);
        true
    }

    pub(in crate::app) fn activate_adjacent_tab(&mut self, next: bool) -> Option<u64> {
        let active_tab_id = self.active_tab_id?;
        let current_index = self.tabs.iter().position(|tab| tab.id == active_tab_id)?;
        let tab_count = self.tabs.len();
        if tab_count <= 1 {
            return Some(active_tab_id);
        }

        let next_index = adjacent_tab_index(current_index, tab_count, next);

        let next_tab_id = self.tabs.get(next_index)?.id;
        self.active_tab_id = Some(next_tab_id);
        Some(next_tab_id)
    }
}
