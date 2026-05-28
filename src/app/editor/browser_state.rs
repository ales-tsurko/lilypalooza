use super::*;

impl EditorState {
    pub(in crate::app) fn new(
        app_theme: iced::Theme,
        view_settings: EditorViewSettings,
        theme_settings: EditorThemeSettings,
    ) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
            next_tab_id: 1,
            project_root: None,
            file_browser_expanded: false,
            file_browser_root: current_editor_browser_root(None),
            file_browser_show_hidden: false,
            file_browser_columns: Vec::new(),
            file_browser_active_column: 0,
            app_theme,
            view_settings,
            default_view_settings: EditorViewSettings::default(),
            theme_settings,
        }
    }

    pub(in crate::app) fn update(
        &mut self,
        tab_id: u64,
        message: &EditorWidgetMessage,
    ) -> iced::Task<EditorWidgetMessage> {
        self.tab_mut(tab_id)
            .map(|tab| tab.widget.update(message))
            .unwrap_or_else(iced::Task::none)
    }

    pub(in crate::app) fn sync_tab_scroll_state(
        &self,
        tab_id: u64,
    ) -> iced::Task<EditorWidgetMessage> {
        self.tab(tab_id)
            .map(|tab| tab.widget.sync_scroll_state())
            .unwrap_or_else(iced::Task::none)
    }

    pub(in crate::app) fn refresh_font_metrics(&mut self) {
        for tab in &mut self.tabs {
            tab.widget.refresh_font_metrics();
        }
    }

    pub(in crate::app) fn set_viewport_width(&mut self, width: f32) {
        let width = width.max(1.0);
        for tab in &mut self.tabs {
            let height = tab.widget.viewport_height();
            tab.widget.set_viewport_size(width, height);
        }
    }

    pub(in crate::app) fn active_tab_id(&self) -> Option<u64> {
        self.active_tab_id
    }

    pub(in crate::app) fn set_project_root(&mut self, project_root: Option<PathBuf>) {
        self.project_root = project_root;
        for tab in &mut self.tabs {
            tab.widget.set_project_root(self.project_root.clone());
        }
        self.rebuild_file_browser();
    }

    pub(in crate::app) fn file_browser_expanded(&self) -> bool {
        self.file_browser_expanded
    }

    pub(in crate::app) fn file_browser_root_label(&self) -> String {
        self.file_browser_root.display().to_string()
    }

    pub(in crate::app) fn file_browser_root(&self) -> &Path {
        &self.file_browser_root
    }

    pub(in crate::app) fn file_browser_show_hidden(&self) -> bool {
        self.file_browser_show_hidden
    }

    pub(in crate::app) fn file_browser_columns(&self) -> Vec<EditorBrowserColumnSummary> {
        self.file_browser_columns
            .iter()
            .map(|column| match column {
                EditorBrowserColumn::Directory {
                    path: _,
                    entries,
                    selected_path,
                } => EditorBrowserColumnSummary::Directory {
                    entries: entries
                        .iter()
                        .map(|entry| EditorBrowserEntrySummary {
                            path: entry.path.clone(),
                            name: entry.name.clone(),
                            is_dir: entry.is_dir,
                            selected: selected_path.as_ref() == Some(&entry.path),
                        })
                        .collect(),
                },
                EditorBrowserColumn::FilePreview(preview) => {
                    EditorBrowserColumnSummary::FilePreview {
                        metadata: EditorFilePreviewSummary {
                            name: preview.name.clone(),
                            size: preview.size.clone(),
                            modified: preview.modified.clone(),
                            created: preview.created.clone(),
                        },
                    }
                }
            })
            .collect()
    }

    pub(in crate::app) fn toggle_file_browser(&mut self) {
        self.file_browser_expanded = !self.file_browser_expanded;
        if self.file_browser_expanded {
            self.ensure_file_browser_initialized();
        }
    }

    pub(in crate::app) fn toggle_file_browser_show_hidden(&mut self) -> Result<(), String> {
        self.file_browser_show_hidden = !self.file_browser_show_hidden;
        self.rebuild_file_browser_preserving_selection()
    }

    pub(in crate::app) fn refresh_file_browser(&mut self) -> Result<(), String> {
        self.rebuild_file_browser_preserving_selection()
    }

    pub(in crate::app) fn selected_file_browser_path(&self) -> Option<PathBuf> {
        self.file_browser_columns
            .iter()
            .take(self.file_browser_active_column.saturating_add(1))
            .rev()
            .find_map(|column| match column {
                EditorBrowserColumn::Directory { selected_path, .. } => selected_path.clone(),
                EditorBrowserColumn::FilePreview(_) => None,
            })
    }

    pub(in crate::app) fn current_file_browser_directory_path(&self) -> PathBuf {
        self.file_browser_current_directory()
            .unwrap_or(self.file_browser_root.as_path())
            .to_path_buf()
    }

    pub(in crate::app) fn current_file_browser_directory_column_index(&self) -> Option<usize> {
        self.file_browser_columns
            .iter()
            .take(self.file_browser_active_column.saturating_add(1))
            .enumerate()
            .rev()
            .find_map(|(index, column)| match column {
                EditorBrowserColumn::Directory { .. } => Some(index),
                EditorBrowserColumn::FilePreview(_) => None,
            })
    }

    pub(in crate::app) fn set_file_browser_active_column(&mut self, column_index: usize) {
        self.file_browser_active_column =
            column_index.min(self.file_browser_columns.len().saturating_sub(1));
    }

    pub(in crate::app) fn select_file_browser_path(
        &mut self,
        column_index: usize,
        path: &Path,
    ) -> Result<(), String> {
        let normalized_path = normalize_editor_path(path);
        let Some(EditorBrowserColumn::Directory {
            entries,
            selected_path,
            ..
        }) = self.file_browser_columns.get_mut(column_index)
        else {
            return Ok(());
        };

        if !entries.iter().any(|entry| entry.path == normalized_path) {
            return Ok(());
        }

        *selected_path = Some(normalized_path);
        self.file_browser_active_column = column_index;
        self.sync_file_browser_preview_from_column(column_index)
    }

    pub(in crate::app) fn browse_to_path(
        &mut self,
        column_index: usize,
        path: &Path,
        is_dir: bool,
    ) -> Result<(), String> {
        if column_index >= self.file_browser_columns.len() {
            return Ok(());
        }

        let selected_path = normalize_editor_path(path);
        let next_column = if is_dir {
            EditorBrowserColumn::Directory {
                path: selected_path.clone(),
                entries: self.read_browser_entries(&selected_path)?,
                selected_path: None,
            }
        } else {
            EditorBrowserColumn::FilePreview(build_file_preview(&selected_path)?)
        };

        self.file_browser_columns.truncate(column_index + 1);
        self.file_browser_active_column = column_index;
        if let Some(EditorBrowserColumn::Directory {
            selected_path: current,
            ..
        }) = self.file_browser_columns.get_mut(column_index)
        {
            *current = Some(selected_path.clone());
        }

        self.file_browser_columns.push(next_column);

        Ok(())
    }
}
