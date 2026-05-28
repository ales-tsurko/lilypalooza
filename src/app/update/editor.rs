use std::{
    fs,
    path::{Path, PathBuf},
};

use iced::widget::operation::{focus, select_all};

use super::*;
use crate::app::editor::EditorTabFileState;

mod browser_history;
mod browser_navigation;
mod file_tabs;
mod message_helpers;
mod routing;

use message_helpers::*;

const EDITOR_TAB_WIDTH: f32 = 144.0;
const EDITOR_TAB_SLOT_WIDTH: f32 = EDITOR_TAB_WIDTH + 4.0;
const EDITOR_TABBAR_AUTOSCROLL_EDGE: f32 = 32.0;
const EDITOR_TABBAR_AUTOSCROLL_MIN_STEP: f32 = 3.0;
const EDITOR_TABBAR_AUTOSCROLL_MAX_STEP: f32 = 12.0;
const EDITOR_TABBAR_NEW_BUTTON_WIDTH: f32 = 36.0;

struct OpenedEditorFile {
    tab_id: u64,
    task: Task<Message>,
}

enum BrowserInlineEditCommit {
    Noop,
    Finished(Task<Message>),
    Applied(BrowserHistoryEntry),
}
