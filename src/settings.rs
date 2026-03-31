use std::fs;
use std::io;
use std::path::PathBuf;

use directories::ProjectDirs;
use ron::ser::PrettyConfig;
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
    pub(crate) brightness: f32,
    pub(crate) text_dim: f32,
    pub(crate) comment_dim: f32,
}

impl Default for EditorThemeSettings {
    fn default() -> Self {
        Self {
            hue_offset_degrees: 0.0,
            saturation: 1.0,
            brightness: 1.0,
            text_dim: 1.0,
            comment_dim: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct AppSettings {
    pub(crate) workspace_layout: WorkspaceLayoutSettings,
    pub(crate) score_view: ScoreViewSettings,
    pub(crate) piano_roll_view: PianoRollViewSettings,
    pub(crate) editor_theme: EditorThemeSettings,
}

pub(crate) fn load() -> Result<AppSettings, String> {
    let path = settings_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => ron::from_str(&contents)
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

    let contents = ron::ser::to_string_pretty(settings, PrettyConfig::new())
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;

    fs::write(&path, contents)
        .map_err(|error| format!("Failed to write settings {}: {error}", path.display()))
}

fn settings_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("rs", "alestsurko", "lily-view")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("settings.ron"))
}
