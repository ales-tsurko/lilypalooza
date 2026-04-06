use std::fs;
use std::io;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub(crate) enum WorkspacePane {
    #[default]
    Score,
    PianoRoll,
    Editor,
    Logger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum DockAxis {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum FoldedPaneRestoreSettings {
    Tab {
        anchor: WorkspacePane,
    },
    Standalone,
    Split {
        anchor: WorkspacePane,
        axis: DockAxis,
        ratio: f32,
        insert_first: bool,
        #[serde(default)]
        sibling_panes: Vec<WorkspacePane>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FoldedPaneSettings {
    pub(crate) pane: WorkspacePane,
    pub(crate) restore: FoldedPaneRestoreSettings,
}

impl Default for FoldedPaneSettings {
    fn default() -> Self {
        Self {
            pane: WorkspacePane::PianoRoll,
            restore: FoldedPaneRestoreSettings::Standalone,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct DockGroupSettings {
    pub(crate) tabs: Vec<WorkspacePane>,
    pub(crate) active: WorkspacePane,
}

impl Default for DockGroupSettings {
    fn default() -> Self {
        Self {
            tabs: vec![WorkspacePane::Score],
            active: WorkspacePane::Score,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum DockNodeSettings {
    Group(DockGroupSettings),
    Split {
        axis: DockAxis,
        ratio: f32,
        first: Box<DockNodeSettings>,
        second: Box<DockNodeSettings>,
    },
}

impl Default for DockNodeSettings {
    fn default() -> Self {
        Self::Split {
            axis: DockAxis::Horizontal,
            ratio: 0.74,
            first: Box::new(Self::Split {
                axis: DockAxis::Vertical,
                ratio: 0.38,
                first: Box::new(Self::Group(DockGroupSettings {
                    tabs: vec![WorkspacePane::Editor],
                    active: WorkspacePane::Editor,
                })),
                second: Box::new(Self::Group(DockGroupSettings {
                    tabs: vec![WorkspacePane::Score, WorkspacePane::PianoRoll],
                    active: WorkspacePane::Score,
                })),
            }),
            second: Box::new(Self::Group(DockGroupSettings {
                tabs: vec![WorkspacePane::Logger],
                active: WorkspacePane::Logger,
            })),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct WorkspaceLayoutSettings {
    pub(crate) root: Option<DockNodeSettings>,
    pub(crate) folded_panes: Vec<FoldedPaneSettings>,
    pub(crate) piano_visible: bool,
}

impl Default for WorkspaceLayoutSettings {
    fn default() -> Self {
        Self {
            root: Some(DockNodeSettings::default()),
            folded_panes: Vec::new(),
            piano_visible: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ScoreViewSettings {
    pub(crate) zoom: f32,
    pub(crate) page_brightness: u8,
}

impl Default for ScoreViewSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            page_brightness: 70,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PianoRollViewSettings {
    pub(crate) zoom_x: f32,
    pub(crate) beat_subdivision: u8,
}

impl Default for PianoRollViewSettings {
    fn default() -> Self {
        Self {
            zoom_x: 1.0,
            beat_subdivision: 4,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EditorThemeSettings {
    pub(crate) hue_offset_degrees: f32,
    pub(crate) saturation: f32,
    pub(crate) warmth: f32,
    pub(crate) brightness: f32,
    pub(crate) text_dim: f32,
    pub(crate) comment_dim: f32,
}

impl Default for EditorThemeSettings {
    fn default() -> Self {
        Self {
            hue_offset_degrees: 0.0,
            saturation: 1.0,
            warmth: 0.0,
            brightness: 1.0,
            text_dim: 1.0,
            comment_dim: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EditorViewSettings {
    pub(crate) font_size: f32,
    pub(crate) center_cursor: bool,
}

impl Default for EditorViewSettings {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            center_cursor: false,
        }
    }
}

fn default_editor_recent_files_limit() -> usize {
    7
}

fn is_default_editor_recent_files_limit(value: &usize) -> bool {
    *value == default_editor_recent_files_limit()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ShortcutKeyCode {
    KeyA,
    KeyC,
    KeyF,
    KeyG,
    KeyH,
    KeyJ,
    KeyK,
    KeyL,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyS,
    KeyV,
    KeyW,
    KeyY,
    KeyZ,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Slash,
    Backslash,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Backspace,
    Delete,
    Home,
    End,
    Insert,
    F3,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Equal,
    Minus,
    Digit0,
    NumpadAdd,
    NumpadSubtract,
    Numpad0,
    BracketLeft,
    BracketRight,
    NumpadEnter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ShortcutNamedKey {
    Space,
    Enter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ShortcutKey {
    Code(ShortcutKeyCode),
    Named(ShortcutNamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ShortcutBinding {
    pub(crate) key: ShortcutKey,
    pub(crate) primary: bool,
    pub(crate) control: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
}

impl Default for ShortcutBinding {
    fn default() -> Self {
        Self {
            key: ShortcutKey::Code(ShortcutKeyCode::KeyS),
            primary: false,
            control: false,
            alt: false,
            shift: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ShortcutBindingOverride {
    Assigned(ShortcutBinding),
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ShortcutActionId {
    QuitApp,
    OpenActions,
    NewEditor,
    OpenEditorFile,
    SaveEditor,
    CloseEditorTab,
    EditorUndo,
    EditorRedo,
    EditorCopy,
    EditorPaste,
    EditorOpenSearch,
    EditorOpenSearchReplace,
    EditorOpenGotoLine,
    EditorTriggerCompletion,
    EditorFindNext,
    EditorFindPrevious,
    EditorWordLeft,
    EditorWordRight,
    EditorWordLeftSelect,
    EditorWordRightSelect,
    EditorDeleteWordBackward,
    EditorDeleteWordForward,
    EditorDeleteToLineStart,
    EditorDeleteToLineEnd,
    EditorLineStart,
    EditorLineEnd,
    EditorLineStartSelect,
    EditorLineEndSelect,
    EditorDocumentStart,
    EditorDocumentEnd,
    EditorDocumentStartSelect,
    EditorDocumentEndSelect,
    EditorDeleteSelection,
    EditorSelectAll,
    EditorInsertLineBelow,
    EditorInsertLineAbove,
    EditorDeleteLine,
    EditorMoveLineUp,
    EditorMoveLineDown,
    EditorCopyLineUp,
    EditorCopyLineDown,
    EditorJoinLines,
    EditorIndent,
    EditorOutdent,
    EditorToggleLineComment,
    EditorToggleBlockComment,
    EditorSelectLine,
    EditorJumpToMatchingBracket,
    ToggleEditorPane,
    ToggleScorePane,
    TogglePianoRollPane,
    ToggleLoggerPane,
    PreviousTab,
    NextTab,
    PreviousEditorTab,
    NextEditorTab,
    PreviousPane,
    NextPane,
    ScoreZoomIn,
    ScoreZoomOut,
    ScoreZoomReset,
    EditorZoomIn,
    EditorZoomOut,
    EditorZoomReset,
    PianoRollZoomIn,
    PianoRollZoomOut,
    PianoRollZoomReset,
    TransportPlayPause,
    TransportRewind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ShortcutOverride {
    pub(crate) action: ShortcutActionId,
    pub(crate) binding: ShortcutBindingOverride,
}

impl Default for ShortcutOverride {
    fn default() -> Self {
        Self {
            action: ShortcutActionId::SaveEditor,
            binding: ShortcutBindingOverride::Unassigned,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ShortcutSettings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) overrides: Vec<ShortcutOverride>,
}

impl ShortcutSettings {
    pub(crate) fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct AppSettings {
    pub(crate) editor_view: EditorViewSettings,
    pub(crate) editor_theme: EditorThemeSettings,
    #[serde(
        default = "default_editor_recent_files_limit",
        skip_serializing_if = "is_default_editor_recent_files_limit"
    )]
    pub(crate) editor_recent_files_limit: usize,
    #[serde(skip_serializing_if = "ShortcutSettings::is_empty")]
    pub(crate) shortcuts: ShortcutSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_view: EditorViewSettings::default(),
            editor_theme: EditorThemeSettings::default(),
            editor_recent_files_limit: default_editor_recent_files_limit(),
            shortcuts: ShortcutSettings::default(),
        }
    }
}

pub(crate) fn load() -> Result<AppSettings, String> {
    let path = settings_load_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents)
            .map_err(|error| format!("Failed to parse settings {}: {error}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(error) => Err(format!(
            "Failed to read settings {}: {error}",
            path.display()
        )),
    }
}

pub(crate) fn save(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let Some(parent) = path.parent() else {
        return Err(format!("Settings path has no parent: {}", path.display()));
    };

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create settings directory {}: {error}",
            parent.display()
        )
    })?;

    let contents = toml::to_string_pretty(settings)
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;

    fs::write(&path, contents)
        .map_err(|error| format!("Failed to write settings {}: {error}", path.display()))
}

fn settings_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("", "", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("settings.toml"))
}

fn legacy_settings_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("by", "alestsurko", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("settings.toml"))
}

fn settings_load_path() -> Result<PathBuf, String> {
    let path = settings_path()?;
    if path.is_file() {
        return Ok(path);
    }

    let legacy = legacy_settings_path()?;
    if legacy.is_file() {
        return Ok(legacy);
    }

    Ok(path)
}
