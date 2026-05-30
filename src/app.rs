use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use editor_host::WindowSnapshot;
use iced::{
    Color, Point, Rectangle, Size, Subscription, Task, event, keyboard, mouse,
    widget::{Id, pane_grid, svg},
    window,
};
use iced_core::{Bytes, image};
use lilypalooza_audio::{AudioEngine, AudioEngineOptions, MixerState};
use messages::{
    EditorMessage, FileMessage, KeyPress, LoggerMessage, Message, MixerMessage, PaneMessage,
    PianoRollMessage, PromptMessage, ViewerMessage,
};
use piano_roll::PianoRollState;
use processor_editor_windows::EditorWindowManager;
use score_cursor::{ScoreCursorMaps, ScoreCursorPlacement};
use tempfile::TempDir;
use update::update;
use view::view;

use crate::{
    browser_file_watcher::BrowserFileWatcher,
    editor_file_watcher::EditorFileWatcher,
    error_prompt::{ErrorPrompt, PromptSelectedButton},
    lilypond,
    logger::Logger,
    score_watcher::ScoreWatcher,
    settings::{self, DockAxis, DockNodeSettings, FoldedPaneRestoreSettings, FoldedPaneSettings},
    state::{self, GlobalState, ProjectState},
};

mod application;
mod controls;
mod dock_view;
mod editor;
mod editor_frame;
mod messages;
mod meters;
mod mixer;
mod piano_roll;
mod processor_editor_windows;
pub(crate) mod processor_presets;
mod score_cursor;
mod score_view;
mod transport_bar;
mod update;
mod view;

mod workspace_runtime;

pub(crate) use application::run;
use application::*;
pub(in crate::app) use editor_frame::{
    AppEditorFrame, EDITOR_FRAME_ZOOM_MAX_PERCENT, EDITOR_FRAME_ZOOM_MIN_PERCENT,
    GENERIC_CONTROLLER_DEFAULT_SIZE, GenericControllerEditor, SharedController,
};
use workspace_runtime::*;
#[cfg(test)]
mod application_tests;
