use std::fs;
use std::io;
use std::path::PathBuf;

use directories::ProjectDirs;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum PaneOrder {
    #[default]
    ScoreFirst,
    PianoFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum PaneAxis {
    #[default]
    Horizontal,
    Vertical,
    Stacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum ActiveScorePane {
    #[default]
    Score,
    PianoRoll,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ScoreLayoutSettings {
    pub(crate) pane_axis: PaneAxis,
    pub(crate) pane_order: PaneOrder,
    pub(crate) active_pane: ActiveScorePane,
    pub(crate) piano_visible: bool,
    pub(crate) piano_expanded_ratio: f32,
}

impl Default for ScoreLayoutSettings {
    fn default() -> Self {
        Self {
            pane_axis: PaneAxis::Horizontal,
            pane_order: PaneOrder::ScoreFirst,
            active_pane: ActiveScorePane::Score,
            piano_visible: true,
            piano_expanded_ratio: 0.70,
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
            zoom: 0.7,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct AppSettings {
    pub(crate) score_layout: ScoreLayoutSettings,
    pub(crate) score_view: ScoreViewSettings,
    pub(crate) piano_roll_view: PianoRollViewSettings,
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
