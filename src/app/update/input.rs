use std::path::Path;

use super::*;
use crate::{
    app::{
        editor::EditorTabFileState,
        messages::ShortcutsMessage,
        piano_roll::{adjacent_subdivision_tick, roll_scroll_id},
    },
    error_prompt::{PromptButtons, PromptSelectedButton},
    settings::PlaybackSettings,
};

mod continuations;
mod focus;
mod helpers;
mod prompts;
mod shortcut_dialog;

use helpers::*;
