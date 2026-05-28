use super::*;

pub(super) fn editor_file_browser_reveal_scroll_x(
    active_column_index: usize,
    reveal_column_index: usize,
    current_x: f32,
    viewport_width: f32,
) -> f32 {
    let active_column_left = active_column_index as f32 * super::EDITOR_FILE_BROWSER_COLUMN_WIDTH;
    let reveal_column_right =
        (reveal_column_index + 1) as f32 * super::EDITOR_FILE_BROWSER_COLUMN_WIDTH;
    if active_column_left < current_x {
        return active_column_left;
    }
    if reveal_column_right > current_x + viewport_width {
        return (reveal_column_right - viewport_width).max(0.0);
    }
    current_x
}

#[derive(Debug, Clone, Copy)]
pub(super) enum EditorZoomDirection {
    In,
    Out,
    Reset,
}

pub(super) fn editor_widget_message_for_shortcut(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_history_message(action)
        .or_else(|| editor_search_message(action))
        .or_else(|| editor_word_message(action))
        .or_else(|| editor_line_message(action))
        .or_else(|| editor_document_message(action))
        .or_else(|| editor_misc_message(action))
}

pub(super) fn editor_file_shell_message(action: ShortcutAction) -> Option<EditorMessage> {
    editor_file_lifecycle_message(action).or_else(|| editor_file_browser_shell_message(action))
}

pub(super) fn editor_file_lifecycle_message(action: ShortcutAction) -> Option<EditorMessage> {
    match action {
        ShortcutAction::NewEditor => Some(EditorMessage::NewRequested),
        ShortcutAction::OpenEditorFile => Some(EditorMessage::OpenRequested),
        ShortcutAction::SaveEditor => Some(EditorMessage::SaveRequested),
        _ => None,
    }
}

pub(super) fn editor_file_browser_shell_message(action: ShortcutAction) -> Option<EditorMessage> {
    match action {
        ShortcutAction::ToggleFileBrowser => Some(EditorMessage::ToggleFileBrowser),
        _ => None,
    }
}

pub(super) fn score_shortcut_message(action: ShortcutAction) -> Option<ViewerMessage> {
    score_zoom_shortcut_message(action)
        .or_else(|| score_scroll_shortcut_message(action))
        .or_else(|| score_page_shortcut_message(action))
}

pub(super) fn score_zoom_shortcut_message(action: ShortcutAction) -> Option<ViewerMessage> {
    match action {
        ShortcutAction::ScoreZoomIn => Some(ViewerMessage::ZoomIn),
        ShortcutAction::ScoreZoomOut => Some(ViewerMessage::ZoomOut),
        ShortcutAction::ScoreZoomReset => Some(ViewerMessage::ResetZoom),
        _ => None,
    }
}

pub(super) fn score_scroll_shortcut_message(action: ShortcutAction) -> Option<ViewerMessage> {
    match action {
        ShortcutAction::ScoreScrollUp => Some(ViewerMessage::ScrollUp),
        ShortcutAction::ScoreScrollDown => Some(ViewerMessage::ScrollDown),
        _ => None,
    }
}

pub(super) fn score_page_shortcut_message(action: ShortcutAction) -> Option<ViewerMessage> {
    match action {
        ShortcutAction::ScorePrevPage => Some(ViewerMessage::PrevPage),
        ShortcutAction::ScoreNextPage => Some(ViewerMessage::NextPage),
        _ => None,
    }
}

pub(super) fn piano_roll_shortcut_message(action: ShortcutAction) -> Option<PianoRollMessage> {
    match action {
        ShortcutAction::PianoRollZoomIn => Some(PianoRollMessage::ZoomIn),
        ShortcutAction::PianoRollZoomOut => Some(PianoRollMessage::ZoomOut),
        ShortcutAction::PianoRollZoomReset => Some(PianoRollMessage::ResetZoom),
        _ => None,
    }
}

pub(super) fn editor_history_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_undo_redo_message(action).or_else(|| editor_clipboard_message(action))
}

pub(super) fn editor_undo_redo_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorUndo => Some(iced_code_editor::Message::Undo),
        ShortcutAction::EditorRedo => Some(iced_code_editor::Message::Redo),
        _ => None,
    }
}

pub(super) fn editor_clipboard_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorCopy => Some(iced_code_editor::Message::Copy),
        ShortcutAction::EditorPaste => Some(iced_code_editor::Message::Paste(String::new())),
        _ => None,
    }
}

pub(super) fn editor_search_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_search_panel_message(action).or_else(|| editor_search_navigation_message(action))
}

pub(super) fn editor_search_panel_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_search_open_message(action).or_else(|| editor_goto_completion_message(action))
}

pub(super) fn editor_search_open_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorOpenSearch => Some(iced_code_editor::Message::OpenSearch),
        ShortcutAction::EditorOpenSearchReplace => {
            Some(iced_code_editor::Message::OpenSearchReplace)
        }
        _ => None,
    }
}

pub(super) fn editor_goto_completion_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorOpenGotoLine => Some(iced_code_editor::Message::OpenGotoLine),
        ShortcutAction::EditorTriggerCompletion => {
            Some(iced_code_editor::Message::TriggerCompletion)
        }
        _ => None,
    }
}

pub(super) fn editor_search_navigation_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorFindNext => Some(iced_code_editor::Message::FindNext),
        ShortcutAction::EditorFindPrevious => Some(iced_code_editor::Message::FindPrevious),
        _ => None,
    }
}

pub(super) fn editor_word_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_word_navigation_message(action).or_else(|| editor_word_delete_message(action))
}

pub(super) fn editor_word_navigation_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_word_arrow_message(action, false).or_else(|| editor_word_arrow_message(action, true))
}

pub(super) fn editor_word_arrow_message(
    action: ShortcutAction,
    select: bool,
) -> Option<iced_code_editor::Message> {
    let direction = match (action, select) {
        (ShortcutAction::EditorWordLeft, false) | (ShortcutAction::EditorWordLeftSelect, true) => {
            iced_code_editor::ArrowDirection::Left
        }
        (ShortcutAction::EditorWordRight, false)
        | (ShortcutAction::EditorWordRightSelect, true) => iced_code_editor::ArrowDirection::Right,
        _ => return None,
    };
    Some(iced_code_editor::Message::WordArrowKey(direction, select))
}

pub(super) fn editor_word_delete_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_word_boundary_delete_message(action).or_else(|| editor_line_edge_delete_message(action))
}

pub(super) fn editor_word_boundary_delete_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorDeleteWordBackward => {
            Some(iced_code_editor::Message::DeleteWordBackward)
        }
        ShortcutAction::EditorDeleteWordForward => {
            Some(iced_code_editor::Message::DeleteWordForward)
        }
        _ => None,
    }
}

pub(super) fn editor_line_edge_delete_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorDeleteToLineStart => {
            Some(iced_code_editor::Message::DeleteToLineStart)
        }
        ShortcutAction::EditorDeleteToLineEnd => Some(iced_code_editor::Message::DeleteToLineEnd),
        _ => None,
    }
}

pub(super) fn editor_line_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_line_navigation_message(action).or_else(|| editor_line_edit_message(action))
}

pub(super) fn editor_line_navigation_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_line_arrow_message(action, false).or_else(|| editor_line_arrow_message(action, true))
}

pub(super) fn editor_line_arrow_message(
    action: ShortcutAction,
    select: bool,
) -> Option<iced_code_editor::Message> {
    editor_edge_message(action, select, EditorEdgeScope::Line)
}

pub(super) fn editor_document_edge_message(
    action: ShortcutAction,
    select: bool,
) -> Option<iced_code_editor::Message> {
    editor_edge_message(action, select, EditorEdgeScope::Document)
}

#[derive(Debug, Clone, Copy)]
enum EditorEdgeScope {
    Line,
    Document,
}

fn editor_edge_message(
    action: ShortcutAction,
    select: bool,
    scope: EditorEdgeScope,
) -> Option<iced_code_editor::Message> {
    match scope {
        EditorEdgeScope::Line => editor_scoped_edge_message(
            action,
            select,
            EdgeActions {
                start: ShortcutAction::EditorLineStart,
                start_select: ShortcutAction::EditorLineStartSelect,
                end: ShortcutAction::EditorLineEnd,
                end_select: ShortcutAction::EditorLineEndSelect,
            },
            iced_code_editor::Message::Home,
            iced_code_editor::Message::End,
        ),
        EditorEdgeScope::Document => editor_scoped_edge_message(
            action,
            select,
            EdgeActions {
                start: ShortcutAction::EditorDocumentStart,
                start_select: ShortcutAction::EditorDocumentStartSelect,
                end: ShortcutAction::EditorDocumentEnd,
                end_select: ShortcutAction::EditorDocumentEndSelect,
            },
            iced_code_editor::Message::DocumentHome,
            iced_code_editor::Message::DocumentEnd,
        ),
    }
}

#[derive(Debug, Clone, Copy)]
struct EdgeActions {
    start: ShortcutAction,
    start_select: ShortcutAction,
    end: ShortcutAction,
    end_select: ShortcutAction,
}

fn editor_scoped_edge_message(
    action: ShortcutAction,
    select: bool,
    actions: EdgeActions,
    start_message: fn(bool) -> iced_code_editor::Message,
    end_message: fn(bool) -> iced_code_editor::Message,
) -> Option<iced_code_editor::Message> {
    if (action, select) == (actions.start, false)
        || (action, select) == (actions.start_select, true)
    {
        return Some(start_message(select));
    }
    if (action, select) == (actions.end, false) || (action, select) == (actions.end_select, true) {
        return Some(end_message(select));
    }
    None
}

pub(super) fn editor_line_edit_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    editor_line_insert_delete_message(action)
        .or_else(|| editor_line_move_message(action))
        .or_else(|| editor_line_copy_join_message(action))
}

pub(super) fn editor_line_insert_delete_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorInsertLineBelow => Some(iced_code_editor::Message::InsertLineBelow),
        ShortcutAction::EditorInsertLineAbove => Some(iced_code_editor::Message::InsertLineAbove),
        ShortcutAction::EditorDeleteLine => Some(iced_code_editor::Message::DeleteLine),
        _ => None,
    }
}

pub(super) fn editor_line_move_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorMoveLineUp => Some(iced_code_editor::Message::MoveLineUp),
        ShortcutAction::EditorMoveLineDown => Some(iced_code_editor::Message::MoveLineDown),
        _ => None,
    }
}

pub(super) fn editor_line_copy_join_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorCopyLineUp => Some(iced_code_editor::Message::CopyLineUp),
        ShortcutAction::EditorCopyLineDown => Some(iced_code_editor::Message::CopyLineDown),
        ShortcutAction::EditorJoinLines => Some(iced_code_editor::Message::JoinLines),
        _ => None,
    }
}

pub(super) fn editor_document_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_document_edge_message(action, false)
        .or_else(|| editor_document_edge_message(action, true))
}

pub(super) fn editor_misc_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    editor_selection_message(action)
        .or_else(|| editor_indent_message(action))
        .or_else(|| editor_comment_message(action))
        .or_else(|| editor_bracket_message(action))
}

pub(super) fn editor_selection_message(
    action: ShortcutAction,
) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorDeleteSelection => Some(iced_code_editor::Message::DeleteSelection),
        ShortcutAction::EditorSelectAll => Some(iced_code_editor::Message::SelectAll),
        ShortcutAction::EditorSelectLine => Some(iced_code_editor::Message::SelectLine),
        _ => None,
    }
}

pub(super) fn editor_indent_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorIndent => Some(iced_code_editor::Message::Tab),
        ShortcutAction::EditorOutdent => Some(iced_code_editor::Message::ShiftTab),
        _ => None,
    }
}

pub(super) fn editor_comment_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorToggleLineComment => {
            Some(iced_code_editor::Message::ToggleLineComment)
        }
        ShortcutAction::EditorToggleBlockComment => {
            Some(iced_code_editor::Message::ToggleBlockComment)
        }
        _ => None,
    }
}

pub(super) fn editor_bracket_message(action: ShortcutAction) -> Option<iced_code_editor::Message> {
    match action {
        ShortcutAction::EditorJumpToMatchingBracket => {
            Some(iced_code_editor::Message::JumpToMatchingBracket)
        }
        _ => None,
    }
}

pub(super) fn key_is_escape(key: &keyboard::Key) -> bool {
    matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
}

pub(super) fn key_is_enter(key: &keyboard::Key) -> bool {
    matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter))
}

pub(super) fn event_is_captured(key_press: &KeyPress) -> bool {
    matches!(key_press.status, iced::event::Status::Captured)
}

pub(super) fn shortcut_requires_pre_capture_modifier(key_press: &KeyPress) -> bool {
    key_press.modifiers.command() || key_press.modifiers.control()
}

pub(super) fn shortcuts_dialog_key_message(key: &keyboard::Key) -> Option<ShortcutsMessage> {
    shortcuts_dialog_close_or_activate_message(key)
        .or_else(|| shortcuts_dialog_selection_message(key))
}

pub(super) fn shortcuts_dialog_close_or_activate_message(
    key: &keyboard::Key,
) -> Option<ShortcutsMessage> {
    match key {
        keyboard::Key::Named(keyboard::key::Named::Escape) => Some(ShortcutsMessage::CloseDialog),
        keyboard::Key::Named(keyboard::key::Named::Enter) => {
            Some(ShortcutsMessage::ActivateSelected)
        }
        _ => None,
    }
}

pub(super) fn shortcuts_dialog_selection_message(key: &keyboard::Key) -> Option<ShortcutsMessage> {
    match key {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(ShortcutsMessage::SelectNext),
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
            Some(ShortcutsMessage::SelectPrevious)
        }
        _ => None,
    }
}

pub(super) fn file_browser_vertical_navigation(key: &keyboard::Key) -> Option<i32> {
    match key {
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(-1),
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(1),
        _ => None,
    }
}

pub(super) fn file_browser_horizontal_navigation(key: &keyboard::Key) -> Option<bool> {
    match key {
        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some(false),
        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(true),
        _ => None,
    }
}

pub(super) fn prompt_key_cycles_forward(key: &keyboard::Key) -> bool {
    matches!(
        key,
        keyboard::Key::Named(keyboard::key::Named::Tab)
            | keyboard::Key::Named(keyboard::key::Named::ArrowRight)
    )
}

pub(super) fn file_browser_shortcut_message(action: ShortcutAction) -> Option<EditorMessage> {
    file_browser_clipboard_shortcut_message(action)
        .or_else(|| file_browser_inline_shortcut_message(action))
        .or_else(|| file_browser_destructive_shortcut_message(action))
}

pub(super) fn file_browser_clipboard_shortcut_message(
    action: ShortcutAction,
) -> Option<EditorMessage> {
    match action {
        ShortcutAction::FileBrowserCut => Some(EditorMessage::FileBrowserCutRequested),
        ShortcutAction::FileBrowserCopy => Some(EditorMessage::FileBrowserCopyRequested),
        ShortcutAction::FileBrowserPaste => Some(EditorMessage::FileBrowserPasteRequested),
        _ => None,
    }
}

pub(super) fn file_browser_inline_shortcut_message(
    action: ShortcutAction,
) -> Option<EditorMessage> {
    match action {
        ShortcutAction::FileBrowserRename => Some(EditorMessage::FileBrowserRenameRequested),
        _ => None,
    }
}

pub(super) fn file_browser_destructive_shortcut_message(
    action: ShortcutAction,
) -> Option<EditorMessage> {
    match action {
        ShortcutAction::FileBrowserDelete => Some(EditorMessage::FileBrowserTrashRequested),
        _ => None,
    }
}

pub(super) fn piano_roll_cursor_subdivision_direction(action: ShortcutAction) -> Option<bool> {
    match action {
        ShortcutAction::PianoRollCursorSubdivisionPrevious => Some(false),
        ShortcutAction::PianoRollCursorSubdivisionNext => Some(true),
        _ => None,
    }
}

pub(super) fn playback_engine_settings_changed(
    loaded: &PlaybackSettings,
    previous: &PlaybackSettings,
) -> bool {
    loaded.sample_rate != previous.sample_rate
        || loaded.device != previous.device
        || loaded.block_size != previous.block_size
        || loaded.chase_notes_on_seek != previous.chase_notes_on_seek
}

pub(super) fn toggle_workspace_pane_shortcut(action: ShortcutAction) -> Option<WorkspacePaneKind> {
    match action {
        ShortcutAction::ToggleWorkspacePane(pane) => Some(pane),
        _ => None,
    }
}

pub(super) fn workspace_tab_shortcut_direction(action: ShortcutAction) -> Option<TabDirection> {
    match action {
        ShortcutAction::SwitchWorkspaceTabPrevious => Some(TabDirection::Previous),
        ShortcutAction::SwitchWorkspaceTabNext => Some(TabDirection::Next),
        _ => None,
    }
}

pub(super) fn workspace_focus_shortcut_direction(
    action: ShortcutAction,
) -> Option<PaneCycleDirection> {
    match action {
        ShortcutAction::FocusWorkspacePanePrevious => Some(PaneCycleDirection::Previous),
        ShortcutAction::FocusWorkspacePaneNext => Some(PaneCycleDirection::Next),
        _ => None,
    }
}

pub(super) fn prompt_buttons(buttons: PromptButtons) -> &'static [PromptSelectedButton] {
    match buttons {
        PromptButtons::Ok => &[PromptSelectedButton::Ok],
        PromptButtons::OkCancel => &[PromptSelectedButton::Cancel, PromptSelectedButton::Ok],
        PromptButtons::SaveDiscardCancel => &[
            PromptSelectedButton::Cancel,
            PromptSelectedButton::Discard,
            PromptSelectedButton::Ok,
        ],
    }
}
