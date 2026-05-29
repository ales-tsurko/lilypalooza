use std::{
    env, fs,
    path::{Path, PathBuf},
};

use iced::{
    Color, Element, Fill, Theme, border,
    widget::{button, container, keyed_column, text},
};
use iced_code_editor::{CodeEditor, Message as EditorWidgetMessage, theme::ThemeTuning};

use crate::{
    fonts,
    settings::{EditorThemeSettings, EditorViewSettings},
    ui_style,
};

mod browser_state;
mod file_operations;
mod filesystem_helpers;
mod internal_browser;
mod model;
mod tab_queries_theme;

use filesystem_helpers::*;
pub(in crate::app) use model::*;
