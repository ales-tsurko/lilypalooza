use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::settings::{PianoRollViewSettings, ScoreViewSettings, WorkspaceLayoutSettings};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(crate) struct TrackColorOverride {
    pub(crate) r: f32,
    pub(crate) g: f32,
    pub(crate) b: f32,
    pub(crate) a: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct GlobalState {
    pub(crate) workspace_layout: WorkspaceLayoutSettings,
    pub(crate) score_view: ScoreViewSettings,
    pub(crate) piano_roll_view: PianoRollViewSettings,
    pub(crate) main_score: Option<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) editor_tabs: Vec<PathBuf>,
    pub(crate) active_editor_tab: Option<PathBuf>,
    pub(crate) has_clean_untitled_editor_tab: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) editor_recent_files: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) recent_projects: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ProjectState {
    pub(crate) project_name: Option<String>,
    pub(crate) workspace_layout: WorkspaceLayoutSettings,
    pub(crate) score_view: ScoreViewSettings,
    pub(crate) piano_roll_view: PianoRollViewSettings,
    pub(crate) main_score: Option<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) editor_tabs: Vec<PathBuf>,
    pub(crate) active_editor_tab: Option<PathBuf>,
    pub(crate) has_clean_untitled_editor_tab: bool,
    #[serde(skip_serializing_if = "track_name_overrides_is_empty")]
    pub(crate) track_name_overrides: Vec<Option<String>>,
    #[serde(skip_serializing_if = "track_color_overrides_is_empty")]
    pub(crate) track_color_overrides: Vec<Option<TrackColorOverride>>,
}

pub(crate) fn load_global() -> Result<GlobalState, String> {
    let path = global_state_load_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => {
            let mut state: GlobalState = ron::from_str(&contents)
                .map_err(|error| format!("Failed to parse state {}: {error}", path.display()))?;
            normalize_unique_paths(&mut state.editor_recent_files);
            normalize_unique_paths(&mut state.recent_projects);
            Ok(state)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(GlobalState::default()),
        Err(error) => Err(format!("Failed to read state {}: {error}", path.display())),
    }
}

pub(crate) fn save_global(state: &GlobalState) -> Result<(), String> {
    let path = global_state_path()?;
    let Some(parent) = path.parent() else {
        return Err(format!("State path has no parent: {}", path.display()));
    };

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create state directory {}: {error}",
            parent.display()
        )
    })?;

    let contents = ron::ser::to_string_pretty(state, PrettyConfig::new())
        .map_err(|error| format!("Failed to serialize global state: {error}"))?;

    fs::write(&path, contents)
        .map_err(|error| format!("Failed to write state {}: {error}", path.display()))
}

pub(crate) fn load_project(project_root: &Path) -> Result<ProjectState, String> {
    let path = project_file_path(project_root);

    match fs::read_to_string(&path) {
        Ok(contents) => ron::from_str(&contents)
            .map_err(|error| format!("Failed to parse project {}: {error}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ProjectState::default()),
        Err(error) => Err(format!(
            "Failed to read project file {}: {error}",
            path.display()
        )),
    }
}

pub(crate) fn save_project(project_root: &Path, state: &ProjectState) -> Result<(), String> {
    let path = project_file_path(project_root);
    let Some(parent) = path.parent() else {
        return Err(format!("Project path has no parent: {}", path.display()));
    };

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create project directory {}: {error}",
            parent.display()
        )
    })?;

    let contents = ron::ser::to_string_pretty(state, PrettyConfig::new())
        .map_err(|error| format!("Failed to serialize project state: {error}"))?;

    fs::write(&path, contents)
        .map_err(|error| format!("Failed to write project file {}: {error}", path.display()))
}

pub(crate) fn find_project_root(path: &Path) -> Option<PathBuf> {
    let start_dir = if path.is_dir() { path } else { path.parent()? };

    for ancestor in start_dir.ancestors() {
        if project_file_path(ancestor).is_file() {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

pub(crate) fn project_file_path(project_root: &Path) -> PathBuf {
    project_root.join(".lilypalooza").join("project.ron")
}

pub(crate) fn main_score_relative_to(
    project_root: &Path,
    score_path: &Path,
) -> Result<PathBuf, String> {
    score_path
        .strip_prefix(project_root)
        .map(Path::to_path_buf)
        .map_err(|_| {
            format!(
                "Main score {} is outside the selected project directory {}",
                score_path.display(),
                project_root.display()
            )
        })
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
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

fn normalize_unique_paths(paths: &mut Vec<PathBuf>) {
    let mut normalized = Vec::with_capacity(paths.len());

    for path in paths.drain(..) {
        let path = normalize_path(&path);
        if !normalized.contains(&path) {
            normalized.push(path);
        }
    }

    *paths = normalized;
}

fn track_name_overrides_is_empty(overrides: &[Option<String>]) -> bool {
    overrides.iter().all(Option::is_none)
}

fn track_color_overrides_is_empty(overrides: &[Option<TrackColorOverride>]) -> bool {
    overrides.iter().all(Option::is_none)
}

fn global_state_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("", "", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("state.ron"))
}

fn legacy_global_state_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("by", "alestsurko", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("state.ron"))
}

fn global_state_load_path() -> Result<PathBuf, String> {
    let path = global_state_path()?;
    if path.is_file() {
        return Ok(path);
    }

    let legacy = legacy_global_state_path()?;
    if legacy.is_file() {
        return Ok(legacy);
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{ProjectState, TrackColorOverride};

    #[test]
    fn project_state_roundtrip_preserves_track_name_overrides() {
        let state = ProjectState {
            track_name_overrides: vec![Some("Lead".to_string()), None, Some("Bass".to_string())],
            ..ProjectState::default()
        };

        let serialized = ron::to_string(&state).expect("state should serialize");
        let parsed: ProjectState = ron::from_str(&serialized).expect("state should parse");

        assert_eq!(parsed.track_name_overrides, state.track_name_overrides);
    }

    #[test]
    fn project_state_roundtrip_preserves_track_color_overrides() {
        let state = ProjectState {
            track_color_overrides: vec![
                Some(TrackColorOverride {
                    r: 0.25,
                    g: 0.5,
                    b: 0.75,
                    a: 1.0,
                }),
                None,
            ],
            ..ProjectState::default()
        };

        let serialized = ron::to_string(&state).expect("state should serialize");
        let parsed: ProjectState = ron::from_str(&serialized).expect("state should parse");

        assert_eq!(parsed.track_color_overrides, state.track_color_overrides);
    }
}
