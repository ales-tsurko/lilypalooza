use super::*;

pub(in crate::app) const EMPTY_EDITOR_MESSAGE: &str = "Edit a text file here.";
pub(in crate::app) const MIN_EDITOR_FONT_SIZE: f32 = 9.0;
pub(in crate::app) const MAX_EDITOR_FONT_SIZE: f32 = 32.0;
pub(in crate::app) const EDITOR_FONT_SIZE_STEP: f32 = 1.0;

#[derive(Debug, Clone)]
pub(in crate::app) struct EditorTabSummary {
    pub(in crate::app) id: u64,
    pub(in crate::app) title: String,
    pub(in crate::app) dirty: bool,
    pub(in crate::app) file_state: EditorTabFileState,
    pub(in crate::app) active: bool,
}

#[derive(Debug, Clone)]
pub(in crate::app) enum EditorBrowserColumnSummary {
    Directory {
        entries: Vec<EditorBrowserEntrySummary>,
    },
    FilePreview {
        metadata: EditorFilePreviewSummary,
    },
}

#[derive(Debug, Clone)]
pub(in crate::app) struct EditorBrowserEntrySummary {
    pub(in crate::app) path: PathBuf,
    pub(in crate::app) name: String,
    pub(in crate::app) is_dir: bool,
    pub(in crate::app) selected: bool,
}

#[derive(Debug, Clone)]
pub(in crate::app) struct EditorFilePreviewSummary {
    pub(in crate::app) name: String,
    pub(in crate::app) size: Option<String>,
    pub(in crate::app) modified: Option<String>,
    pub(in crate::app) created: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum EditorTabFileState {
    Ok,
    ChangedOnDisk,
    MissingOnDisk,
}

pub(in crate::app) struct EditorTab {
    pub(in crate::app) id: u64,
    pub(in crate::app) widget: CodeEditor,
    pub(in crate::app) path: Option<PathBuf>,
    pub(in crate::app) saved_content: Option<String>,
    pub(in crate::app) file_state: EditorTabFileState,
}

pub(in crate::app) enum EditorBrowserColumn {
    Directory {
        path: PathBuf,
        entries: Vec<EditorBrowserEntry>,
        selected_path: Option<PathBuf>,
    },
    FilePreview(EditorFilePreview),
}

#[derive(Debug, Clone)]
pub(in crate::app) struct EditorBrowserEntry {
    pub(in crate::app) path: PathBuf,
    pub(in crate::app) name: String,
    pub(in crate::app) is_dir: bool,
}

pub(in crate::app) struct EditorFilePreview {
    pub(in crate::app) name: String,
    pub(in crate::app) size: Option<String>,
    pub(in crate::app) modified: Option<String>,
    pub(in crate::app) created: Option<String>,
}

pub(in crate::app) struct EditorState {
    pub(in crate::app) tabs: Vec<EditorTab>,
    pub(in crate::app) active_tab_id: Option<u64>,
    pub(in crate::app) next_tab_id: u64,
    pub(in crate::app) project_root: Option<PathBuf>,
    pub(in crate::app) file_browser_expanded: bool,
    pub(in crate::app) file_browser_root: PathBuf,
    pub(in crate::app) file_browser_show_hidden: bool,
    pub(in crate::app) file_browser_columns: Vec<EditorBrowserColumn>,
    pub(in crate::app) file_browser_active_column: usize,
    pub(in crate::app) app_theme: iced::Theme,
    pub(in crate::app) view_settings: EditorViewSettings,
    pub(in crate::app) default_view_settings: EditorViewSettings,
    pub(in crate::app) theme_settings: EditorThemeSettings,
}
