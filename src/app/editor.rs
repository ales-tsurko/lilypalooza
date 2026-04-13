use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{button, container, keyed_column, text};
use iced::{Element, Fill};
use iced_code_editor::{CodeEditor, Message as EditorWidgetMessage, theme::ThemeTuning};

use crate::fonts;
use crate::settings::{EditorThemeSettings, EditorViewSettings};
use crate::ui_style;

const EMPTY_EDITOR_MESSAGE: &str = "Edit a text file here.";
const MIN_EDITOR_FONT_SIZE: f32 = 9.0;
const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
const EDITOR_FONT_SIZE_STEP: f32 = 1.0;

#[derive(Debug, Clone)]
pub(super) struct EditorTabSummary {
    pub(super) id: u64,
    pub(super) title: String,
    pub(super) dirty: bool,
    pub(super) file_state: EditorTabFileState,
    pub(super) active: bool,
}

#[derive(Debug, Clone)]
pub(super) enum EditorBrowserColumnSummary {
    Directory {
        entries: Vec<EditorBrowserEntrySummary>,
    },
    FilePreview {
        metadata: EditorFilePreviewSummary,
    },
}

#[derive(Debug, Clone)]
pub(super) struct EditorBrowserEntrySummary {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) is_dir: bool,
    pub(super) selected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct EditorFilePreviewSummary {
    pub(super) name: String,
    pub(super) size: Option<String>,
    pub(super) modified: Option<String>,
    pub(super) created: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorTabFileState {
    Ok,
    ChangedOnDisk,
    MissingOnDisk,
}

struct EditorTab {
    id: u64,
    widget: CodeEditor,
    path: Option<PathBuf>,
    saved_content: Option<String>,
    file_state: EditorTabFileState,
}

enum EditorBrowserColumn {
    Directory {
        entries: Vec<EditorBrowserEntry>,
        selected_path: Option<PathBuf>,
    },
    FilePreview(EditorFilePreview),
}

struct EditorBrowserEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

struct EditorFilePreview {
    name: String,
    size: Option<String>,
    modified: Option<String>,
    created: Option<String>,
}

pub(super) struct EditorState {
    tabs: Vec<EditorTab>,
    active_tab_id: Option<u64>,
    next_tab_id: u64,
    project_root: Option<PathBuf>,
    file_browser_expanded: bool,
    file_browser_root: PathBuf,
    file_browser_columns: Vec<EditorBrowserColumn>,
    file_browser_active_column: usize,
    app_theme: iced::Theme,
    view_settings: EditorViewSettings,
    default_view_settings: EditorViewSettings,
    theme_settings: EditorThemeSettings,
}

impl EditorState {
    pub(super) fn new(
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
            file_browser_columns: Vec::new(),
            file_browser_active_column: 0,
            app_theme,
            view_settings,
            default_view_settings: EditorViewSettings::default(),
            theme_settings,
        }
        .with_initialized_file_browser()
    }

    fn with_initialized_file_browser(mut self) -> Self {
        self.rebuild_file_browser();
        self
    }

    pub(super) fn update(
        &mut self,
        tab_id: u64,
        message: &EditorWidgetMessage,
    ) -> iced::Task<EditorWidgetMessage> {
        self.tab_mut(tab_id)
            .map(|tab| tab.widget.update(message))
            .unwrap_or_else(iced::Task::none)
    }

    pub(super) fn sync_tab_scroll_state(&self, tab_id: u64) -> iced::Task<EditorWidgetMessage> {
        self.tab(tab_id)
            .map(|tab| tab.widget.sync_scroll_state())
            .unwrap_or_else(iced::Task::none)
    }

    pub(super) fn refresh_font_metrics(&mut self) {
        for tab in &mut self.tabs {
            tab.widget.refresh_font_metrics();
        }
    }

    pub(super) fn set_viewport_width(&mut self, width: f32) {
        let width = width.max(1.0);
        for tab in &mut self.tabs {
            let height = tab.widget.viewport_height();
            tab.widget.set_viewport_size(width, height);
        }
    }

    pub(super) fn active_tab_id(&self) -> Option<u64> {
        self.active_tab_id
    }

    pub(super) fn set_project_root(&mut self, project_root: Option<PathBuf>) {
        self.project_root = project_root;
        for tab in &mut self.tabs {
            tab.widget.set_project_root(self.project_root.clone());
        }
        self.rebuild_file_browser();
    }

    pub(super) fn file_browser_expanded(&self) -> bool {
        self.file_browser_expanded
    }

    pub(super) fn file_browser_root_label(&self) -> String {
        self.file_browser_root.display().to_string()
    }

    pub(super) fn file_browser_columns(&self) -> Vec<EditorBrowserColumnSummary> {
        self.file_browser_columns
            .iter()
            .map(|column| match column {
                EditorBrowserColumn::Directory {
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

    pub(super) fn toggle_file_browser(&mut self) {
        self.file_browser_expanded = !self.file_browser_expanded;
    }

    pub(super) fn set_file_browser_active_column(&mut self, column_index: usize) {
        self.file_browser_active_column =
            column_index.min(self.file_browser_columns.len().saturating_sub(1));
    }

    pub(super) fn browse_to_path(
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
                entries: read_browser_entries(&selected_path)?,
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

    pub(super) fn has_document(&self) -> bool {
        !self.tabs.is_empty()
    }

    #[cfg(test)]
    pub(super) fn active_content(&self) -> Option<String> {
        self.active_tab().map(|tab| tab.widget.content())
    }

    #[cfg(test)]
    pub(super) fn active_editor_is_focused(&self) -> bool {
        self.active_tab().is_some_and(|tab| tab.widget.is_focused())
    }

    pub(super) fn file_browser_active_column_index(&self) -> usize {
        self.file_browser_active_column
    }

    pub(super) fn file_browser_selected_index(&self, column_index: usize) -> Option<usize> {
        let EditorBrowserColumn::Directory {
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

    pub(super) fn file_browser_has_preview_column(&self, column_index: usize) -> bool {
        self.file_browser_columns.get(column_index + 1).is_some()
    }

    pub(super) fn path(&self) -> Option<&Path> {
        self.active_tab().and_then(|tab| tab.path.as_deref())
    }

    pub(super) fn tab_path(&self, tab_id: u64) -> Option<&Path> {
        self.tab(tab_id).and_then(|tab| tab.path.as_deref())
    }

    pub(super) fn tab_is_modified(&self, tab_id: u64) -> bool {
        self.tab(tab_id).is_some_and(|tab| tab.widget.is_modified())
    }

    pub(super) fn tab_file_state(&self, tab_id: u64) -> Option<EditorTabFileState> {
        self.tab(tab_id).map(|tab| tab.file_state)
    }

    pub(super) fn has_dirty_tabs(&self) -> bool {
        self.tabs.iter().any(|tab| tab.widget.is_modified())
    }

    pub(super) fn tabs_requiring_resolution(&self) -> Vec<u64> {
        self.tabs
            .iter()
            .filter(|tab| {
                tab.widget.is_modified() || tab.file_state == EditorTabFileState::MissingOnDisk
            })
            .map(|tab| tab.id)
            .collect()
    }

    pub(super) fn tab_ids(&self) -> Vec<u64> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    pub(super) fn file_backed_tabs(&self) -> Vec<(u64, PathBuf)> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.path.clone().map(|path| (tab.id, path)))
            .collect()
    }

    pub(super) fn find_tab_by_path(&self, path: &Path) -> Option<u64> {
        let normalized_path = normalize_editor_path(path);
        self.tabs.iter().find_map(|tab| {
            tab.path
                .as_deref()
                .map(normalize_editor_path)
                .filter(|candidate| *candidate == normalized_path)
                .map(|_| tab.id)
        })
    }

    pub(super) fn suggested_save_name(&self) -> String {
        self.path()
            .and_then(Path::file_name)
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("untitled.ly")
            .to_string()
    }

    pub(super) fn suggested_rename_name(&self, tab_id: u64) -> String {
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

    pub(super) fn theme_settings(&self) -> EditorThemeSettings {
        self.theme_settings
    }

    pub(super) fn view_settings(&self) -> EditorViewSettings {
        self.view_settings
    }

    pub(super) fn font_size_points(&self) -> u32 {
        self.view_settings.font_size.round() as u32
    }

    pub(super) fn can_zoom_in(&self) -> bool {
        self.view_settings.font_size < MAX_EDITOR_FONT_SIZE - f32::EPSILON
    }

    pub(super) fn can_zoom_out(&self) -> bool {
        self.view_settings.font_size > MIN_EDITOR_FONT_SIZE + f32::EPSILON
    }

    pub(super) fn can_reset_zoom(&self) -> bool {
        (self.view_settings.font_size - self.default_view_settings.font_size).abs() > 1e-4
    }

    pub(super) fn center_cursor(&self) -> bool {
        self.view_settings.center_cursor
    }

    pub(super) fn set_center_cursor(&mut self, value: bool) {
        self.view_settings.center_cursor = value;
        for tab in &mut self.tabs {
            tab.widget.set_center_cursor(value);
        }
    }

    pub(super) fn apply_view_settings(&mut self, settings: EditorViewSettings) {
        self.set_font_size(settings.font_size);
        self.set_center_cursor(settings.center_cursor);
    }

    pub(super) fn apply_theme_settings(&mut self, settings: EditorThemeSettings) {
        self.theme_settings = settings;
        self.apply_theme();
    }

    pub(super) fn set_hue_offset_degrees(&mut self, value: f32) {
        self.theme_settings.hue_offset_degrees = value;
        self.apply_theme();
    }

    pub(super) fn set_saturation(&mut self, value: f32) {
        self.theme_settings.saturation = value;
        self.apply_theme();
    }

    pub(super) fn set_warmth(&mut self, value: f32) {
        self.theme_settings.warmth = value;
        self.apply_theme();
    }

    pub(super) fn set_brightness(&mut self, value: f32) {
        self.theme_settings.brightness = value;
        self.apply_theme();
    }

    pub(super) fn set_text_dim(&mut self, value: f32) {
        self.theme_settings.text_dim = value;
        self.apply_theme();
    }

    pub(super) fn set_comment_dim(&mut self, value: f32) {
        self.theme_settings.comment_dim = value;
        self.apply_theme();
    }

    pub(super) fn zoom_in(&mut self) {
        let next = (self.view_settings.font_size + EDITOR_FONT_SIZE_STEP).min(MAX_EDITOR_FONT_SIZE);
        self.set_font_size(next);
    }

    pub(super) fn zoom_out(&mut self) {
        let next = (self.view_settings.font_size - EDITOR_FONT_SIZE_STEP).max(MIN_EDITOR_FONT_SIZE);
        self.set_font_size(next);
    }

    pub(super) fn reset_zoom(&mut self) {
        self.set_font_size(self.default_view_settings.font_size);
    }

    pub(super) fn tab_summaries(&self) -> Vec<EditorTabSummary> {
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

    pub(super) fn activate_tab(&mut self, tab_id: u64) -> bool {
        if self.tab(tab_id).is_none() {
            return false;
        }

        self.active_tab_id = Some(tab_id);
        true
    }

    pub(super) fn activate_adjacent_tab(&mut self, next: bool) -> Option<u64> {
        let active_tab_id = self.active_tab_id?;
        let current_index = self.tabs.iter().position(|tab| tab.id == active_tab_id)?;
        let tab_count = self.tabs.len();
        if tab_count <= 1 {
            return Some(active_tab_id);
        }

        let next_index = if next {
            (current_index + 1) % tab_count
        } else if current_index == 0 {
            tab_count - 1
        } else {
            current_index - 1
        };

        let next_tab_id = self.tabs[next_index].id;
        self.active_tab_id = Some(next_tab_id);
        Some(next_tab_id)
    }

    pub(super) fn new_document(&mut self) -> (u64, iced::Task<EditorWidgetMessage>, bool) {
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

    pub(super) fn load_file(
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

    pub(super) fn restore_file_tabs(
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

    pub(super) fn save_to_disk(
        &mut self,
        tab_id: u64,
    ) -> Result<(PathBuf, iced::Task<EditorWidgetMessage>), String> {
        let Some(path) = self.tab(tab_id).and_then(|tab| tab.path.clone()) else {
            return Err("No editor file is currently loaded".to_string());
        };

        let task = self.save_to_path(tab_id, &path)?;

        Ok((path, task))
    }

    pub(super) fn save_to_path(
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

    pub(super) fn rename_file(
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

    pub(super) fn reload_tab_from_disk(
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

    pub(super) fn tab_saved_content(&self, tab_id: u64) -> Option<&str> {
        self.tab(tab_id)
            .and_then(|tab| tab.saved_content.as_deref())
    }

    pub(super) fn set_tab_file_state(
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

    pub(super) fn close_tab(&mut self, tab_id: u64) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return false;
        };

        let was_active = self.active_tab_id == Some(tab_id);
        self.tabs.remove(index);

        if self.tabs.is_empty() {
            self.active_tab_id = None;
        } else if was_active {
            let next_index = index.saturating_sub(1).min(self.tabs.len() - 1);
            self.active_tab_id = Some(self.tabs[next_index].id);
        }

        true
    }

    pub(super) fn mark_tab_saved(&mut self, tab_id: u64) -> bool {
        let Some(tab) = self.tab_mut(tab_id) else {
            return false;
        };

        tab.widget.mark_saved();
        true
    }

    pub(super) fn reorder_tabs(
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

    pub(super) fn file_backed_tab_paths(&self) -> Vec<PathBuf> {
        self.tabs
            .iter()
            .filter_map(|tab| tab.path.clone())
            .collect()
    }

    pub(super) fn active_file_backed_tab_path(&self) -> Option<PathBuf> {
        self.active_tab().and_then(|tab| tab.path.as_ref().cloned())
    }

    pub(super) fn has_clean_untitled_tab(&self) -> bool {
        self.tabs.iter().any(|tab| {
            tab.path.is_none() && !tab.widget.is_modified() && tab.widget.content().is_empty()
        })
    }

    pub(super) fn lose_focus(&mut self) {
        for tab in &mut self.tabs {
            tab.widget.lose_focus();
        }
    }

    pub(super) fn request_focus(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.widget.request_focus();
        }
    }

    pub(super) fn set_tab_cursor(
        &mut self,
        tab_id: u64,
        line: usize,
        column: usize,
    ) -> iced::Task<EditorWidgetMessage> {
        self.tab_mut(tab_id)
            .map(|tab| tab.widget.set_cursor(line, column))
            .unwrap_or_else(iced::Task::none)
    }

    pub(super) fn view<'a, Message>(
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
        .style(ui_style::pane_main_surface)
        .into()
    }

    pub(super) fn tab_title(&self, tab_id: u64) -> String {
        self.tab_summaries()
            .into_iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| tab.title)
            .unwrap_or_else(|| "Untitled".to_string())
    }

    fn apply_theme(&mut self) {
        let theme = iced_code_editor::theme::from_iced_theme_with_tuning(
            &self.app_theme,
            to_editor_theme_tuning(self.theme_settings),
        );
        for tab in &mut self.tabs {
            tab.widget.set_theme(theme);
        }
    }

    fn set_font_size(&mut self, size: f32) {
        let clamped = size.clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE);
        self.view_settings.font_size = clamped;
        for tab in &mut self.tabs {
            tab.widget.set_font_size(clamped, true);
        }
    }

    fn load_document_into_tab(
        &mut self,
        tab_id: u64,
        content: &str,
        path: Option<PathBuf>,
        modified: bool,
    ) -> Result<iced::Task<EditorWidgetMessage>, String> {
        let syntax = path
            .as_deref()
            .map(syntax_for_path)
            .unwrap_or_else(|| "lilypond".to_string());
        let app_theme = self.app_theme.clone();
        let theme_settings = self.theme_settings;
        let font_size = self.view_settings.font_size;
        let center_cursor = self.view_settings.center_cursor;
        let project_root = self.project_root.clone();

        let task = if let Some(tab) = self.tab_mut(tab_id) {
            let task = tab.widget.reset_document(content, &syntax);
            tab.widget.set_font(fonts::MONO);
            tab.widget.set_document_path(path.clone());
            tab.widget.set_project_root(project_root.clone());
            tab.widget
                .set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
                    &app_theme,
                    to_editor_theme_tuning(theme_settings),
                ));
            tab.widget.set_font_size(font_size, true);
            tab.widget.set_center_cursor(center_cursor);
            if !modified {
                tab.widget.mark_saved();
            }
            tab.path = path;
            tab.saved_content = tab.path.as_ref().map(|_| content.to_string());
            tab.file_state = EditorTabFileState::Ok;
            task
        } else {
            let mut tab = EditorTab {
                id: tab_id,
                widget: build_editor(
                    content,
                    &syntax,
                    path.clone(),
                    self.project_root.clone(),
                    &self.app_theme,
                    self.view_settings,
                    self.theme_settings,
                ),
                path,
                saved_content: None,
                file_state: EditorTabFileState::Ok,
            };
            if !modified {
                tab.widget.mark_saved();
            }
            tab.saved_content = tab.path.as_ref().map(|_| content.to_string());
            self.tabs.push(tab);
            iced::Task::none()
        };

        self.active_tab_id = Some(tab_id);

        Ok(task)
    }

    fn find_reusable_empty_tab(&self) -> Option<u64> {
        self.tabs.iter().find_map(|tab| {
            (tab.path.is_none() && !tab.widget.is_modified() && tab.widget.content().is_empty())
                .then_some(tab.id)
        })
    }

    fn allocate_tab_id(&mut self) -> u64 {
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        tab_id
    }

    fn active_tab(&self) -> Option<&EditorTab> {
        self.active_tab_id.and_then(|tab_id| self.tab(tab_id))
    }

    fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        let tab_id = self.active_tab_id?;
        self.tab_mut(tab_id)
    }

    fn tab(&self, tab_id: u64) -> Option<&EditorTab> {
        self.tabs.iter().find(|tab| tab.id == tab_id)
    }

    fn tab_mut(&mut self, tab_id: u64) -> Option<&mut EditorTab> {
        self.tabs.iter_mut().find(|tab| tab.id == tab_id)
    }

    fn rebuild_file_browser(&mut self) {
        self.file_browser_root = current_editor_browser_root(self.project_root.as_deref());
        self.file_browser_columns = vec![EditorBrowserColumn::Directory {
            entries: read_browser_entries(&self.file_browser_root).unwrap_or_default(),
            selected_path: None,
        }];
        self.file_browser_active_column = 0;
    }

    pub(super) fn move_file_browser_selection(&mut self, delta: i32) -> Result<(), String> {
        if self.file_browser_columns.is_empty() {
            return Ok(());
        }

        let column_index = self
            .file_browser_active_column
            .min(self.file_browser_columns.len().saturating_sub(1));
        let EditorBrowserColumn::Directory {
            entries,
            selected_path,
        } = &self.file_browser_columns[column_index]
        else {
            return Ok(());
        };
        if entries.is_empty() {
            return Ok(());
        }

        let next_index = match selected_path
            .as_ref()
            .and_then(|selected| entries.iter().position(|entry| &entry.path == selected))
        {
            Some(current_index) => {
                (current_index as i32 + delta).clamp(0, entries.len() as i32 - 1) as usize
            }
            None if delta >= 0 => 0,
            None => entries.len().saturating_sub(1),
        };
        let entry = &entries[next_index];
        let path = entry.path.clone();
        let is_dir = entry.is_dir;

        self.browse_to_path(column_index, &path, is_dir)
    }

    pub(super) fn move_file_browser_column(&mut self, right: bool) -> Result<(), String> {
        if self.file_browser_columns.is_empty() {
            return Ok(());
        }

        if !right {
            let active_index = self
                .file_browser_active_column
                .min(self.file_browser_columns.len().saturating_sub(1));
            if let Some(EditorBrowserColumn::Directory { selected_path, .. }) =
                self.file_browser_columns.get_mut(active_index)
            {
                *selected_path = None;
            }
            self.file_browser_active_column = self.file_browser_active_column.saturating_sub(1);
            self.sync_file_browser_preview_from_column(self.file_browser_active_column)?;
            return Ok(());
        }

        let column_index = self
            .file_browser_active_column
            .min(self.file_browser_columns.len().saturating_sub(1));
        let EditorBrowserColumn::Directory {
            entries,
            selected_path,
        } = &self.file_browser_columns[column_index]
        else {
            return Ok(());
        };
        if entries.is_empty() {
            return Ok(());
        }

        let selected_index = selected_path
            .as_ref()
            .and_then(|selected| entries.iter().position(|entry| &entry.path == selected))
            .unwrap_or(0);
        let entry = &entries[selected_index];
        if !entry.is_dir {
            return Ok(());
        }

        let path = entry.path.clone();
        self.browse_to_path(column_index, &path, true)?;
        self.file_browser_active_column =
            (column_index + 1).min(self.file_browser_columns.len().saturating_sub(1));
        if let Some(EditorBrowserColumn::Directory {
            entries,
            selected_path,
        }) = self
            .file_browser_columns
            .get_mut(self.file_browser_active_column)
            && selected_path.is_none()
            && let Some(first_entry) = entries.first()
        {
            *selected_path = Some(first_entry.path.clone());
        }
        self.sync_file_browser_preview_from_column(self.file_browser_active_column)?;
        Ok(())
    }

    fn sync_file_browser_preview_from_column(&mut self, column_index: usize) -> Result<(), String> {
        if column_index >= self.file_browser_columns.len() {
            return Ok(());
        }

        let Some(EditorBrowserColumn::Directory {
            entries,
            selected_path,
        }) = self.file_browser_columns.get(column_index)
        else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        let Some(selected_path) = selected_path.clone() else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        let Some((selected_path, selected_is_dir)) = entries
            .iter()
            .find(|entry| entry.path == selected_path)
            .map(|entry| (entry.path.clone(), entry.is_dir))
        else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        self.file_browser_columns.truncate(column_index + 1);
        self.file_browser_columns.push(if selected_is_dir {
            EditorBrowserColumn::Directory {
                entries: read_browser_entries(&selected_path)?,
                selected_path: None,
            }
        } else {
            EditorBrowserColumn::FilePreview(build_file_preview(&selected_path)?)
        });
        Ok(())
    }
}

fn build_editor(
    content: &str,
    syntax: &str,
    document_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
    app_theme: &iced::Theme,
    view_settings: EditorViewSettings,
    theme_settings: EditorThemeSettings,
) -> CodeEditor {
    let mut editor = CodeEditor::new(content, syntax).with_wrap_enabled(false);
    editor.set_document_path(document_path);
    editor.set_project_root(project_root);
    editor.set_font(fonts::MONO);
    editor.set_font_size(view_settings.font_size, true);
    editor.set_center_cursor(view_settings.center_cursor);
    editor.set_lsp_enabled(false);
    editor.set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
        app_theme,
        to_editor_theme_tuning(theme_settings),
    ));
    editor
}

fn to_editor_theme_tuning(settings: EditorThemeSettings) -> ThemeTuning {
    ThemeTuning {
        hue_offset_degrees: settings.hue_offset_degrees,
        saturation: settings.saturation,
        warmth: settings.warmth,
        contrast: settings.brightness,
        text_dim: settings.text_dim,
        comment_dim: settings.comment_dim,
    }
}

fn normalize_editor_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn current_editor_browser_root(project_root: Option<&Path>) -> PathBuf {
    project_root
        .map(normalize_editor_path)
        .or_else(|| {
            env::current_dir()
                .ok()
                .map(|cwd| normalize_editor_path(&cwd))
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_browser_entries(path: &Path) -> Result<Vec<EditorBrowserEntry>, String> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|error| format!("Failed to read directory {}: {error}", path.display()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let entry_path = normalize_editor_path(&entry.path());
            let name = entry.file_name().to_str()?.to_string();

            Some(EditorBrowserEntry {
                path: entry_path,
                name,
                is_dir: file_type.is_dir(),
            })
        })
        .collect();

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(entries)
}

fn build_file_preview(path: &Path) -> Result<EditorFilePreview, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to read metadata for {}: {error}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_string();
    Ok(EditorFilePreview {
        name,
        size: if metadata.is_file() {
            Some(format_file_size(metadata.len()))
        } else {
            None
        },
        modified: metadata
            .modified()
            .ok()
            .and_then(format_relative_system_time),
        created: metadata
            .created()
            .ok()
            .and_then(format_relative_system_time),
    })
}

fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MB {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

fn format_relative_system_time(time: std::time::SystemTime) -> Option<String> {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(time).ok()?;
    let seconds = duration.as_secs();

    Some(if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} h ago", seconds / 3_600)
    } else {
        format!("{} d ago", seconds / 86_400)
    })
}

fn syntax_for_path(path: &Path) -> String {
    if let Some(syntax) = iced_code_editor::language::syntax_for_path(path) {
        return syntax.to_string();
    }

    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        return extension.to_string();
    }

    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        return file_name.to_string();
    }

    "text".to_string()
}
