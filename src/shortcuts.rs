use iced::keyboard;

use crate::settings::{
    ShortcutActionId, ShortcutBinding, ShortcutBindingOverride, ShortcutKey, ShortcutKeyCode,
    ShortcutNamedKey, ShortcutSettings, WorkspacePane,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ShortcutActionMetadata {
    pub(crate) id: ShortcutActionId,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutAction {
    QuitApp,
    OpenActions,
    OpenSettingsFile,
    NewEditor,
    OpenEditorFile,
    ToggleFileBrowser,
    FileBrowserUndo,
    FileBrowserRedo,
    FileBrowserCut,
    FileBrowserCopy,
    FileBrowserPaste,
    FileBrowserRename,
    FileBrowserDelete,
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
    ToggleWorkspacePane(WorkspacePane),
    SwitchWorkspaceTabPrevious,
    SwitchWorkspaceTabNext,
    SwitchEditorTabPrevious,
    SwitchEditorTabNext,
    FocusWorkspacePanePrevious,
    FocusWorkspacePaneNext,
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
    PianoRollCursorSubdivisionPrevious,
    PianoRollCursorSubdivisionNext,
    PianoRollScrollUp,
    PianoRollScrollDown,
    ScoreScrollUp,
    ScoreScrollDown,
    ScorePrevPage,
    ScoreNextPage,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ShortcutInput<'a> {
    pub(crate) key: &'a keyboard::Key,
    pub(crate) physical_key: keyboard::key::Physical,
    pub(crate) modifiers: keyboard::Modifiers,
}

impl<'a> ShortcutInput<'a> {
    pub(crate) fn new(
        key: &'a keyboard::Key,
        physical_key: keyboard::key::Physical,
        modifiers: keyboard::Modifiers,
    ) -> Self {
        Self {
            key,
            physical_key,
            modifiers,
        }
    }
}

const GLOBAL_ACTIONS: [ShortcutAction; 9] = [
    ShortcutAction::QuitApp,
    ShortcutAction::OpenActions,
    ShortcutAction::OpenSettingsFile,
    ShortcutAction::SaveEditor,
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger),
];

const NAVIGATION_ACTIONS: [ShortcutAction; 4] = [
    ShortcutAction::SwitchWorkspaceTabPrevious,
    ShortcutAction::SwitchWorkspaceTabNext,
    ShortcutAction::FocusWorkspacePanePrevious,
    ShortcutAction::FocusWorkspacePaneNext,
];

const SCORE_CONTEXTUAL_ACTIONS: [ShortcutAction; 6] = [
    ShortcutAction::ScoreZoomIn,
    ShortcutAction::ScoreZoomOut,
    ShortcutAction::ScoreZoomReset,
    ShortcutAction::TransportPlayPause,
    ShortcutAction::TransportRewind,
    ShortcutAction::ScoreScrollUp,
];

const PIANO_ROLL_CONTEXTUAL_ACTIONS: [ShortcutAction; 5] = [
    ShortcutAction::PianoRollZoomIn,
    ShortcutAction::PianoRollZoomOut,
    ShortcutAction::PianoRollZoomReset,
    ShortcutAction::TransportPlayPause,
    ShortcutAction::TransportRewind,
];

const EDITOR_CONTEXTUAL_ACTIONS: &[ShortcutAction] = &[
    ShortcutAction::NewEditor,
    ShortcutAction::OpenEditorFile,
    ShortcutAction::ToggleFileBrowser,
    ShortcutAction::EditorUndo,
    ShortcutAction::EditorRedo,
    ShortcutAction::EditorCopy,
    ShortcutAction::EditorPaste,
    ShortcutAction::EditorOpenSearch,
    ShortcutAction::EditorOpenSearchReplace,
    ShortcutAction::EditorOpenGotoLine,
    ShortcutAction::EditorTriggerCompletion,
    ShortcutAction::EditorFindNext,
    ShortcutAction::EditorFindPrevious,
    ShortcutAction::EditorWordLeft,
    ShortcutAction::EditorWordRight,
    ShortcutAction::EditorWordLeftSelect,
    ShortcutAction::EditorWordRightSelect,
    ShortcutAction::EditorDeleteWordBackward,
    ShortcutAction::EditorDeleteWordForward,
    ShortcutAction::EditorDeleteToLineStart,
    ShortcutAction::EditorDeleteToLineEnd,
    ShortcutAction::EditorLineStart,
    ShortcutAction::EditorLineEnd,
    ShortcutAction::EditorLineStartSelect,
    ShortcutAction::EditorLineEndSelect,
    ShortcutAction::EditorDocumentStart,
    ShortcutAction::EditorDocumentEnd,
    ShortcutAction::EditorDocumentStartSelect,
    ShortcutAction::EditorDocumentEndSelect,
    ShortcutAction::EditorDeleteSelection,
    ShortcutAction::EditorSelectAll,
    ShortcutAction::EditorInsertLineBelow,
    ShortcutAction::EditorInsertLineAbove,
    ShortcutAction::EditorDeleteLine,
    ShortcutAction::EditorMoveLineUp,
    ShortcutAction::EditorMoveLineDown,
    ShortcutAction::EditorCopyLineUp,
    ShortcutAction::EditorCopyLineDown,
    ShortcutAction::EditorJoinLines,
    ShortcutAction::EditorIndent,
    ShortcutAction::EditorOutdent,
    ShortcutAction::EditorToggleLineComment,
    ShortcutAction::EditorToggleBlockComment,
    ShortcutAction::EditorSelectLine,
    ShortcutAction::EditorJumpToMatchingBracket,
    ShortcutAction::CloseEditorTab,
    ShortcutAction::SwitchEditorTabPrevious,
    ShortcutAction::SwitchEditorTabNext,
    ShortcutAction::EditorZoomIn,
    ShortcutAction::EditorZoomOut,
    ShortcutAction::EditorZoomReset,
];

const EDITOR_BROWSER_ACTIONS: &[ShortcutAction] = &[
    ShortcutAction::FileBrowserUndo,
    ShortcutAction::FileBrowserRedo,
    ShortcutAction::FileBrowserCut,
    ShortcutAction::FileBrowserCopy,
    ShortcutAction::FileBrowserPaste,
    ShortcutAction::FileBrowserRename,
    ShortcutAction::FileBrowserDelete,
];

pub(crate) fn resolve_global(
    settings: &ShortcutSettings,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    GLOBAL_ACTIONS
        .into_iter()
        .find(|action| action_matches(settings, *action, input))
}

pub(crate) fn resolve_navigation(
    settings: &ShortcutSettings,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    NAVIGATION_ACTIONS
        .into_iter()
        .find(|action| action_matches(settings, *action, input))
}

pub(crate) fn resolve_contextual(
    settings: &ShortcutSettings,
    pane: WorkspacePane,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    let remappable_match = match pane {
        WorkspacePane::Score => SCORE_CONTEXTUAL_ACTIONS[..5]
            .iter()
            .copied()
            .find(|action| action_matches(settings, *action, input)),
        WorkspacePane::PianoRoll => PIANO_ROLL_CONTEXTUAL_ACTIONS
            .iter()
            .copied()
            .find(|action| action_matches(settings, *action, input)),
        WorkspacePane::Mixer => None,
        WorkspacePane::Editor => EDITOR_CONTEXTUAL_ACTIONS
            .iter()
            .copied()
            .find(|action| action_matches(settings, *action, input)),
        WorkspacePane::Logger => None,
    };

    remappable_match.or_else(|| fixed_contextual_action(pane, input))
}

pub(crate) fn resolve_editor_browser(
    settings: &ShortcutSettings,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    EDITOR_BROWSER_ACTIONS
        .iter()
        .copied()
        .find(|action| action_matches(settings, *action, input))
}

pub(crate) fn label_for_action(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Option<String> {
    display_binding_for_action(settings, action).map(format_binding)
}

pub(crate) fn label_for_action_id(
    settings: &ShortcutSettings,
    action_id: ShortcutActionId,
) -> Option<String> {
    display_binding_for_action(settings, action_from_id(action_id)).map(format_binding)
}

pub(crate) fn filtered_action_metadata(query: &str) -> Vec<ShortcutActionMetadata> {
    let query = query.trim().to_lowercase();
    let mut actions: Vec<_> = ALL_ACTION_IDS
        .iter()
        .copied()
        .filter_map(|action_id| {
            let metadata = action_metadata(action_id);
            let action_id_key = crate::settings::shortcut_action_id_key(action_id);
            let matches = query.is_empty()
                || metadata.name.to_lowercase().contains(&query)
                || metadata.description.to_lowercase().contains(&query)
                || action_id_key.contains(&query);
            matches.then_some(metadata)
        })
        .collect();
    actions.sort_by_key(|metadata| metadata.name);
    actions
}

pub(crate) fn action_for_id(action_id: ShortcutActionId) -> ShortcutAction {
    action_from_id(action_id)
}

macro_rules! shortcut_metadata {
    ($($id:ident => ($name:expr, $description:expr)),+ $(,)?) => {
        pub(crate) const ALL_ACTION_IDS: &[ShortcutActionId] = &[
            $(ShortcutActionId::$id,)+
        ];

        pub(crate) fn action_metadata(action_id: ShortcutActionId) -> ShortcutActionMetadata {
            match action_id {
                $(ShortcutActionId::$id => ShortcutActionMetadata {
                    id: ShortcutActionId::$id,
                    name: $name,
                    description: $description,
                },)+
            }
        }
    };
}

shortcut_metadata! {
    QuitApp => ("Quit App", "Global: close the application window and quit Lilypalooza."),
    OpenActions => ("Open Actions", "Global: open the actions palette."),
    OpenSettingsFile => ("Open Settings File", "Global: open settings.toml in the editor."),
    NewEditor => ("New File", "Editor: create a new file tab in the text editor."),
    OpenEditorFile => ("Open File", "Editor: open one or more files into editor tabs."),
    ToggleFileBrowser => ("Toggle File Browser", "Editor: show or hide the file browser above the editor tabs."),
    FileBrowserUndo => ("Undo Browser Operation", "File Browser: undo the last browser file operation."),
    FileBrowserRedo => ("Redo Browser Operation", "File Browser: redo the last undone browser file operation."),
    FileBrowserCut => ("Cut Browser Item", "File Browser: mark the selected file or folder to move on paste."),
    FileBrowserCopy => ("Copy Browser Item", "File Browser: copy the selected file or folder on paste."),
    FileBrowserPaste => ("Paste Browser Item", "File Browser: paste the copied or cut file or folder into the current folder."),
    FileBrowserRename => ("Rename Browser Item", "File Browser: rename the selected file or folder."),
    FileBrowserDelete => ("Delete Browser Item", "File Browser: delete the selected file or folder."),
    SaveEditor => ("Save File", "Editor: save the active file tab."),
    CloseEditorTab => ("Close Editor Tab", "Editor: close the active editor tab."),
    EditorUndo => ("Undo", "Editor: undo the last text editing change."),
    EditorRedo => ("Redo", "Editor: redo the last undone text editing change."),
    EditorCopy => ("Copy", "Editor: copy the current editor selection."),
    EditorPaste => ("Paste", "Editor: paste clipboard text into the editor."),
    EditorOpenSearch => ("Find", "Editor: open the find dialog for the current file."),
    EditorOpenSearchReplace => ("Find and Replace", "Editor: open the find and replace dialog for the current file."),
    EditorOpenGotoLine => ("Go to Line", "Editor: open the go to line dialog for the current file."),
    EditorTriggerCompletion => ("Trigger Completion", "Editor: open the autocomplete popup manually."),
    EditorFindNext => ("Find Next", "Editor: jump to the next match in the current search."),
    EditorFindPrevious => ("Find Previous", "Editor: jump to the previous match in the current search."),
    EditorWordLeft => ("Move Word Left", "Editor: move the cursor one word to the left."),
    EditorWordRight => ("Move Word Right", "Editor: move the cursor one word to the right."),
    EditorWordLeftSelect => ("Select Word Left", "Editor: extend the selection one word to the left."),
    EditorWordRightSelect => ("Select Word Right", "Editor: extend the selection one word to the right."),
    EditorDeleteWordBackward => ("Delete Word Backward", "Editor: delete the previous word."),
    EditorDeleteWordForward => ("Delete Word Forward", "Editor: delete the next word."),
    EditorDeleteToLineStart => ("Delete to Line Start", "Editor: delete from the cursor to the start of the line."),
    EditorDeleteToLineEnd => ("Delete to Line End", "Editor: delete from the cursor to the end of the line."),
    EditorLineStart => ("Move to Line Start", "Editor: move the cursor to the current line start."),
    EditorLineEnd => ("Move to Line End", "Editor: move the cursor to the current line end."),
    EditorLineStartSelect => ("Select to Line Start", "Editor: extend the selection to the current line start."),
    EditorLineEndSelect => ("Select to Line End", "Editor: extend the selection to the current line end."),
    EditorDocumentStart => ("Move to Document Start", "Editor: move the cursor to the start of the file."),
    EditorDocumentEnd => ("Move to Document End", "Editor: move the cursor to the end of the file."),
    EditorDocumentStartSelect => ("Select to Document Start", "Editor: extend the selection to the start of the file."),
    EditorDocumentEndSelect => ("Select to Document End", "Editor: extend the selection to the end of the file."),
    EditorDeleteSelection => ("Delete Selection", "Editor: delete the current selection."),
    EditorSelectAll => ("Select All", "Editor: select the whole file."),
    EditorInsertLineBelow => ("Insert Line Below", "Editor: insert a new line below the current line."),
    EditorInsertLineAbove => ("Insert Line Above", "Editor: insert a new line above the current line."),
    EditorDeleteLine => ("Delete Line", "Editor: delete the current line or selected lines."),
    EditorMoveLineUp => ("Move Line Up", "Editor: move the current line or selected lines upward."),
    EditorMoveLineDown => ("Move Line Down", "Editor: move the current line or selected lines downward."),
    EditorCopyLineUp => ("Copy Line Up", "Editor: duplicate the current line or selection above."),
    EditorCopyLineDown => ("Copy Line Down", "Editor: duplicate the current line or selection below."),
    EditorJoinLines => ("Join Lines", "Editor: join the current line with the next line."),
    EditorIndent => ("Indent", "Editor: indent the current line or selection."),
    EditorOutdent => ("Outdent", "Editor: outdent the current line or selection."),
    EditorToggleLineComment => ("Toggle Line Comment", "Editor: comment or uncomment the current line or selection."),
    EditorToggleBlockComment => ("Toggle Block Comment", "Editor: wrap or unwrap the current selection with block comments."),
    EditorSelectLine => ("Select Line", "Editor: select the current line."),
    EditorJumpToMatchingBracket => ("Jump to Matching Bracket", "Editor: jump to the matching bracket near the cursor."),
    ToggleEditorPane => ("Toggle Editor Pane", "Workspace: show or hide the editor pane."),
    ToggleScorePane => ("Toggle Score Pane", "Workspace: show or hide the score preview pane."),
    TogglePianoRollPane => ("Toggle Piano Roll Pane", "Workspace: show or hide the piano roll pane."),
    ToggleMixerPane => ("Toggle Mixer Pane", "Workspace: show or hide the mixer pane."),
    ToggleLoggerPane => ("Toggle Logger Pane", "Workspace: show or hide the logger pane."),
    PreviousTab => ("Previous Workspace Tab", "Workspace: switch to the previous tab in the active pane group."),
    NextTab => ("Next Workspace Tab", "Workspace: switch to the next tab in the active pane group."),
    PreviousEditorTab => ("Previous Editor Tab", "Editor: switch to the previous editor file tab."),
    NextEditorTab => ("Next Editor Tab", "Editor: switch to the next editor file tab."),
    PreviousPane => ("Previous Pane", "Workspace: move keyboard focus to the previous pane."),
    NextPane => ("Next Pane", "Workspace: move keyboard focus to the next pane."),
    ScoreZoomIn => ("Zoom In Score", "Score: increase rendered score zoom."),
    ScoreZoomOut => ("Zoom Out Score", "Score: decrease rendered score zoom."),
    ScoreZoomReset => ("Reset Score Zoom", "Score: reset rendered score zoom."),
    EditorZoomIn => ("Zoom In Editor", "Editor: increase editor font size."),
    EditorZoomOut => ("Zoom Out Editor", "Editor: decrease editor font size."),
    EditorZoomReset => ("Reset Editor Zoom", "Editor: reset editor font size."),
    PianoRollZoomIn => ("Zoom In Piano Roll", "Piano Roll: increase piano roll zoom."),
    PianoRollZoomOut => ("Zoom Out Piano Roll", "Piano Roll: decrease piano roll zoom."),
    PianoRollZoomReset => ("Reset Piano Roll Zoom", "Piano Roll: reset piano roll zoom."),
    TransportPlayPause => ("Play or Pause", "Transport: start or pause playback."),
    TransportRewind => ("Rewind", "Transport: return playback to the start."),
}

fn fixed_contextual_action(
    pane: WorkspacePane,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    if input.modifiers.command() || input.modifiers.control() || input.modifiers.alt() {
        return None;
    }

    match pane {
        WorkspacePane::Score => match input.key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(ShortcutAction::ScoreScrollUp)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(ShortcutAction::ScoreScrollDown)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                Some(ShortcutAction::ScorePrevPage)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                Some(ShortcutAction::ScoreNextPage)
            }
            _ => None,
        },
        WorkspacePane::PianoRoll => match input.key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                Some(ShortcutAction::PianoRollCursorSubdivisionPrevious)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                Some(ShortcutAction::PianoRollCursorSubdivisionNext)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(ShortcutAction::PianoRollScrollUp)
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(ShortcutAction::PianoRollScrollDown)
            }
            _ => None,
        },
        WorkspacePane::Mixer | WorkspacePane::Editor | WorkspacePane::Logger => None,
    }
}

fn action_matches(
    settings: &ShortcutSettings,
    action: ShortcutAction,
    input: ShortcutInput<'_>,
) -> bool {
    effective_bindings(settings, action)
        .iter()
        .any(|binding| binding_matches(*binding, input))
}

fn action_from_id(action_id: ShortcutActionId) -> ShortcutAction {
    match action_id {
        ShortcutActionId::QuitApp => ShortcutAction::QuitApp,
        ShortcutActionId::OpenActions => ShortcutAction::OpenActions,
        ShortcutActionId::OpenSettingsFile => ShortcutAction::OpenSettingsFile,
        ShortcutActionId::NewEditor => ShortcutAction::NewEditor,
        ShortcutActionId::OpenEditorFile => ShortcutAction::OpenEditorFile,
        ShortcutActionId::ToggleFileBrowser => ShortcutAction::ToggleFileBrowser,
        ShortcutActionId::FileBrowserUndo => ShortcutAction::FileBrowserUndo,
        ShortcutActionId::FileBrowserRedo => ShortcutAction::FileBrowserRedo,
        ShortcutActionId::FileBrowserCut => ShortcutAction::FileBrowserCut,
        ShortcutActionId::FileBrowserCopy => ShortcutAction::FileBrowserCopy,
        ShortcutActionId::FileBrowserPaste => ShortcutAction::FileBrowserPaste,
        ShortcutActionId::FileBrowserRename => ShortcutAction::FileBrowserRename,
        ShortcutActionId::FileBrowserDelete => ShortcutAction::FileBrowserDelete,
        ShortcutActionId::SaveEditor => ShortcutAction::SaveEditor,
        ShortcutActionId::CloseEditorTab => ShortcutAction::CloseEditorTab,
        ShortcutActionId::EditorUndo => ShortcutAction::EditorUndo,
        ShortcutActionId::EditorRedo => ShortcutAction::EditorRedo,
        ShortcutActionId::EditorCopy => ShortcutAction::EditorCopy,
        ShortcutActionId::EditorPaste => ShortcutAction::EditorPaste,
        ShortcutActionId::EditorOpenSearch => ShortcutAction::EditorOpenSearch,
        ShortcutActionId::EditorOpenSearchReplace => ShortcutAction::EditorOpenSearchReplace,
        ShortcutActionId::EditorOpenGotoLine => ShortcutAction::EditorOpenGotoLine,
        ShortcutActionId::EditorTriggerCompletion => ShortcutAction::EditorTriggerCompletion,
        ShortcutActionId::EditorFindNext => ShortcutAction::EditorFindNext,
        ShortcutActionId::EditorFindPrevious => ShortcutAction::EditorFindPrevious,
        ShortcutActionId::EditorWordLeft => ShortcutAction::EditorWordLeft,
        ShortcutActionId::EditorWordRight => ShortcutAction::EditorWordRight,
        ShortcutActionId::EditorWordLeftSelect => ShortcutAction::EditorWordLeftSelect,
        ShortcutActionId::EditorWordRightSelect => ShortcutAction::EditorWordRightSelect,
        ShortcutActionId::EditorDeleteWordBackward => ShortcutAction::EditorDeleteWordBackward,
        ShortcutActionId::EditorDeleteWordForward => ShortcutAction::EditorDeleteWordForward,
        ShortcutActionId::EditorDeleteToLineStart => ShortcutAction::EditorDeleteToLineStart,
        ShortcutActionId::EditorDeleteToLineEnd => ShortcutAction::EditorDeleteToLineEnd,
        ShortcutActionId::EditorLineStart => ShortcutAction::EditorLineStart,
        ShortcutActionId::EditorLineEnd => ShortcutAction::EditorLineEnd,
        ShortcutActionId::EditorLineStartSelect => ShortcutAction::EditorLineStartSelect,
        ShortcutActionId::EditorLineEndSelect => ShortcutAction::EditorLineEndSelect,
        ShortcutActionId::EditorDocumentStart => ShortcutAction::EditorDocumentStart,
        ShortcutActionId::EditorDocumentEnd => ShortcutAction::EditorDocumentEnd,
        ShortcutActionId::EditorDocumentStartSelect => ShortcutAction::EditorDocumentStartSelect,
        ShortcutActionId::EditorDocumentEndSelect => ShortcutAction::EditorDocumentEndSelect,
        ShortcutActionId::EditorDeleteSelection => ShortcutAction::EditorDeleteSelection,
        ShortcutActionId::EditorSelectAll => ShortcutAction::EditorSelectAll,
        ShortcutActionId::EditorInsertLineBelow => ShortcutAction::EditorInsertLineBelow,
        ShortcutActionId::EditorInsertLineAbove => ShortcutAction::EditorInsertLineAbove,
        ShortcutActionId::EditorDeleteLine => ShortcutAction::EditorDeleteLine,
        ShortcutActionId::EditorMoveLineUp => ShortcutAction::EditorMoveLineUp,
        ShortcutActionId::EditorMoveLineDown => ShortcutAction::EditorMoveLineDown,
        ShortcutActionId::EditorCopyLineUp => ShortcutAction::EditorCopyLineUp,
        ShortcutActionId::EditorCopyLineDown => ShortcutAction::EditorCopyLineDown,
        ShortcutActionId::EditorJoinLines => ShortcutAction::EditorJoinLines,
        ShortcutActionId::EditorIndent => ShortcutAction::EditorIndent,
        ShortcutActionId::EditorOutdent => ShortcutAction::EditorOutdent,
        ShortcutActionId::EditorToggleLineComment => ShortcutAction::EditorToggleLineComment,
        ShortcutActionId::EditorToggleBlockComment => ShortcutAction::EditorToggleBlockComment,
        ShortcutActionId::EditorSelectLine => ShortcutAction::EditorSelectLine,
        ShortcutActionId::EditorJumpToMatchingBracket => {
            ShortcutAction::EditorJumpToMatchingBracket
        }
        ShortcutActionId::ToggleEditorPane => {
            ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor)
        }
        ShortcutActionId::ToggleScorePane => {
            ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score)
        }
        ShortcutActionId::TogglePianoRollPane => {
            ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll)
        }
        ShortcutActionId::ToggleMixerPane => {
            ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer)
        }
        ShortcutActionId::ToggleLoggerPane => {
            ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger)
        }
        ShortcutActionId::PreviousTab => ShortcutAction::SwitchWorkspaceTabPrevious,
        ShortcutActionId::NextTab => ShortcutAction::SwitchWorkspaceTabNext,
        ShortcutActionId::PreviousEditorTab => ShortcutAction::SwitchEditorTabPrevious,
        ShortcutActionId::NextEditorTab => ShortcutAction::SwitchEditorTabNext,
        ShortcutActionId::PreviousPane => ShortcutAction::FocusWorkspacePanePrevious,
        ShortcutActionId::NextPane => ShortcutAction::FocusWorkspacePaneNext,
        ShortcutActionId::ScoreZoomIn => ShortcutAction::ScoreZoomIn,
        ShortcutActionId::ScoreZoomOut => ShortcutAction::ScoreZoomOut,
        ShortcutActionId::ScoreZoomReset => ShortcutAction::ScoreZoomReset,
        ShortcutActionId::EditorZoomIn => ShortcutAction::EditorZoomIn,
        ShortcutActionId::EditorZoomOut => ShortcutAction::EditorZoomOut,
        ShortcutActionId::EditorZoomReset => ShortcutAction::EditorZoomReset,
        ShortcutActionId::PianoRollZoomIn => ShortcutAction::PianoRollZoomIn,
        ShortcutActionId::PianoRollZoomOut => ShortcutAction::PianoRollZoomOut,
        ShortcutActionId::PianoRollZoomReset => ShortcutAction::PianoRollZoomReset,
        ShortcutActionId::TransportPlayPause => ShortcutAction::TransportPlayPause,
        ShortcutActionId::TransportRewind => ShortcutAction::TransportRewind,
    }
}

fn effective_bindings(settings: &ShortcutSettings, action: ShortcutAction) -> Vec<ShortcutBinding> {
    match binding_override(settings, action) {
        Some(ShortcutBindingOverride::Assigned(binding)) => vec![binding],
        Some(ShortcutBindingOverride::Unassigned) => Vec::new(),
        None => default_bindings(action),
    }
}

fn display_binding_for_action(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Option<ShortcutBinding> {
    match binding_override(settings, action) {
        Some(ShortcutBindingOverride::Assigned(binding)) => Some(binding),
        Some(ShortcutBindingOverride::Unassigned) => None,
        None => default_bindings(action).into_iter().next(),
    }
}

fn binding_override(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Option<ShortcutBindingOverride> {
    let action_id = action_id(action)?;
    settings
        .overrides
        .iter()
        .find(|override_entry| override_entry.action == action_id)
        .map(|override_entry| override_entry.binding)
}

fn default_bindings(action: ShortcutAction) -> Vec<ShortcutBinding> {
    match action {
        ShortcutAction::QuitApp => vec![binding_code(ShortcutKeyCode::KeyQ, true, false, false)],
        ShortcutAction::OpenActions => {
            vec![binding_code(ShortcutKeyCode::KeyP, true, false, true)]
        }
        ShortcutAction::OpenSettingsFile => {
            vec![binding_code(ShortcutKeyCode::Comma, true, false, false)]
        }
        ShortcutAction::NewEditor => vec![binding_code(ShortcutKeyCode::KeyN, true, false, false)],
        ShortcutAction::OpenEditorFile => {
            vec![binding_code(ShortcutKeyCode::KeyO, true, false, false)]
        }
        ShortcutAction::ToggleFileBrowser => Vec::new(),
        ShortcutAction::FileBrowserUndo => {
            vec![binding_code(ShortcutKeyCode::KeyZ, true, false, false)]
        }
        ShortcutAction::FileBrowserRedo => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::KeyZ, true, false, true)]
            } else {
                vec![
                    binding_code(ShortcutKeyCode::KeyY, true, false, false),
                    binding_code(ShortcutKeyCode::KeyZ, true, false, true),
                ]
            }
        }
        ShortcutAction::FileBrowserCut => {
            vec![binding_code(ShortcutKeyCode::KeyX, true, false, false)]
        }
        ShortcutAction::FileBrowserCopy => {
            vec![binding_code(ShortcutKeyCode::KeyC, true, false, false)]
        }
        ShortcutAction::FileBrowserPaste => {
            vec![binding_code(ShortcutKeyCode::KeyV, true, false, false)]
        }
        ShortcutAction::FileBrowserRename => vec![
            binding_named(ShortcutNamedKey::Enter, false, false, false),
            binding_code(ShortcutKeyCode::NumpadEnter, false, false, false),
        ],
        ShortcutAction::FileBrowserDelete => {
            if cfg!(target_os = "macos") {
                vec![
                    binding_code(ShortcutKeyCode::Backspace, true, false, false),
                    binding_code(ShortcutKeyCode::Delete, true, false, false),
                ]
            } else {
                vec![binding_code(ShortcutKeyCode::Delete, false, false, false)]
            }
        }
        ShortcutAction::SaveEditor => vec![binding_code(ShortcutKeyCode::KeyS, true, false, false)],
        ShortcutAction::EditorUndo => {
            vec![binding_code(ShortcutKeyCode::KeyZ, true, false, false)]
        }
        ShortcutAction::EditorRedo => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::KeyZ, true, false, true)]
            } else {
                vec![
                    binding_code(ShortcutKeyCode::KeyY, true, false, false),
                    binding_code(ShortcutKeyCode::KeyZ, true, false, true),
                ]
            }
        }
        ShortcutAction::EditorCopy => {
            let mut bindings = vec![binding_code(ShortcutKeyCode::KeyC, true, false, false)];
            if !cfg!(target_os = "macos") {
                bindings.push(binding_code(ShortcutKeyCode::Insert, true, false, false));
            }
            bindings
        }
        ShortcutAction::EditorPaste => {
            let mut bindings = vec![binding_code(ShortcutKeyCode::KeyV, true, false, false)];
            if !cfg!(target_os = "macos") {
                bindings.push(binding_code(ShortcutKeyCode::Insert, false, false, true));
            }
            bindings
        }
        ShortcutAction::EditorOpenSearch => {
            vec![binding_code(ShortcutKeyCode::KeyF, true, false, false)]
        }
        ShortcutAction::EditorOpenSearchReplace => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::KeyF, true, true, false)]
            } else {
                vec![binding_code(ShortcutKeyCode::KeyH, true, false, false)]
            }
        }
        ShortcutAction::EditorOpenGotoLine => {
            vec![binding_code(ShortcutKeyCode::KeyG, true, false, false)]
        }
        ShortcutAction::EditorTriggerCompletion => {
            vec![binding_named_ctrl(ShortcutNamedKey::Space, false, false)]
        }
        ShortcutAction::EditorFindNext => {
            vec![binding_code(ShortcutKeyCode::F3, false, false, false)]
        }
        ShortcutAction::EditorFindPrevious => {
            vec![binding_code(ShortcutKeyCode::F3, false, false, true)]
        }
        ShortcutAction::EditorWordLeft => vec![binding_code(
            ShortcutKeyCode::ArrowLeft,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            false,
        )],
        ShortcutAction::EditorWordRight => vec![binding_code(
            ShortcutKeyCode::ArrowRight,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            false,
        )],
        ShortcutAction::EditorWordLeftSelect => vec![binding_code(
            ShortcutKeyCode::ArrowLeft,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            true,
        )],
        ShortcutAction::EditorWordRightSelect => vec![binding_code(
            ShortcutKeyCode::ArrowRight,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            true,
        )],
        ShortcutAction::EditorDeleteWordBackward => vec![binding_code(
            ShortcutKeyCode::Backspace,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            false,
        )],
        ShortcutAction::EditorDeleteWordForward => vec![binding_code(
            ShortcutKeyCode::Delete,
            !cfg!(target_os = "macos"),
            cfg!(target_os = "macos"),
            false,
        )],
        ShortcutAction::EditorDeleteToLineStart => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::Backspace, true, false, false)]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorDeleteToLineEnd => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::Delete, true, false, false)]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorLineStart => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowLeft, true, false, false)]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorLineEnd => {
            if cfg!(target_os = "macos") {
                vec![binding_code(
                    ShortcutKeyCode::ArrowRight,
                    true,
                    false,
                    false,
                )]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorLineStartSelect => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowLeft, true, false, true)]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorLineEndSelect => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowRight, true, false, true)]
            } else {
                Vec::new()
            }
        }
        ShortcutAction::EditorDocumentStart => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowUp, true, false, false)]
            } else {
                vec![binding_code(ShortcutKeyCode::Home, true, false, false)]
            }
        }
        ShortcutAction::EditorDocumentEnd => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowDown, true, false, false)]
            } else {
                vec![binding_code(ShortcutKeyCode::End, true, false, false)]
            }
        }
        ShortcutAction::EditorDocumentStartSelect => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowUp, true, false, true)]
            } else {
                vec![binding_code(ShortcutKeyCode::Home, true, false, true)]
            }
        }
        ShortcutAction::EditorDocumentEndSelect => {
            if cfg!(target_os = "macos") {
                vec![binding_code(ShortcutKeyCode::ArrowDown, true, false, true)]
            } else {
                vec![binding_code(ShortcutKeyCode::End, true, false, true)]
            }
        }
        ShortcutAction::EditorDeleteSelection => {
            vec![binding_code(ShortcutKeyCode::Delete, false, false, true)]
        }
        ShortcutAction::EditorSelectAll => {
            vec![binding_code(ShortcutKeyCode::KeyA, true, false, false)]
        }
        ShortcutAction::EditorInsertLineBelow => {
            vec![binding_named(ShortcutNamedKey::Enter, true, false, false)]
        }
        ShortcutAction::EditorInsertLineAbove => {
            vec![binding_named(ShortcutNamedKey::Enter, true, false, true)]
        }
        ShortcutAction::EditorDeleteLine => {
            vec![binding_code(ShortcutKeyCode::KeyK, true, false, true)]
        }
        ShortcutAction::EditorMoveLineUp => {
            vec![binding_code(ShortcutKeyCode::ArrowUp, false, true, false)]
        }
        ShortcutAction::EditorMoveLineDown => {
            vec![binding_code(ShortcutKeyCode::ArrowDown, false, true, false)]
        }
        ShortcutAction::EditorCopyLineUp => {
            vec![binding_code(ShortcutKeyCode::ArrowUp, false, true, true)]
        }
        ShortcutAction::EditorCopyLineDown => {
            vec![binding_code(ShortcutKeyCode::ArrowDown, false, true, true)]
        }
        ShortcutAction::EditorJoinLines => {
            vec![binding_code(ShortcutKeyCode::KeyJ, true, false, false)]
        }
        ShortcutAction::EditorIndent => {
            vec![binding_code(
                ShortcutKeyCode::BracketRight,
                true,
                false,
                false,
            )]
        }
        ShortcutAction::EditorOutdent => {
            vec![binding_code(
                ShortcutKeyCode::BracketLeft,
                true,
                false,
                false,
            )]
        }
        ShortcutAction::EditorToggleLineComment => {
            vec![binding_code(ShortcutKeyCode::Slash, true, false, false)]
        }
        ShortcutAction::EditorToggleBlockComment => {
            vec![binding_code(ShortcutKeyCode::KeyA, false, true, true)]
        }
        ShortcutAction::EditorSelectLine => {
            vec![binding_code(ShortcutKeyCode::KeyL, true, false, false)]
        }
        ShortcutAction::EditorJumpToMatchingBracket => {
            vec![binding_code(ShortcutKeyCode::Backslash, true, false, true)]
        }
        ShortcutAction::CloseEditorTab => {
            vec![binding_code(ShortcutKeyCode::KeyW, true, false, false)]
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor) => vec![
            binding_code(ShortcutKeyCode::Digit1, true, false, false),
            binding_code(ShortcutKeyCode::Numpad1, true, false, false),
        ],
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score) => vec![
            binding_code(ShortcutKeyCode::Digit2, true, false, false),
            binding_code(ShortcutKeyCode::Numpad2, true, false, false),
        ],
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll) => vec![
            binding_code(ShortcutKeyCode::Digit3, true, false, false),
            binding_code(ShortcutKeyCode::Numpad3, true, false, false),
        ],
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer) => vec![
            binding_code(ShortcutKeyCode::Digit4, true, false, false),
            binding_code(ShortcutKeyCode::Numpad4, true, false, false),
        ],
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger) => vec![
            binding_code(ShortcutKeyCode::Digit0, true, false, false),
            binding_code(ShortcutKeyCode::Numpad0, true, false, false),
        ],
        ShortcutAction::SwitchWorkspaceTabPrevious => {
            vec![binding_code(
                ShortcutKeyCode::BracketLeft,
                true,
                false,
                true,
            )]
        }
        ShortcutAction::SwitchWorkspaceTabNext => {
            vec![binding_code(
                ShortcutKeyCode::BracketRight,
                true,
                false,
                true,
            )]
        }
        ShortcutAction::SwitchEditorTabPrevious => {
            vec![binding_code(ShortcutKeyCode::ArrowLeft, true, true, false)]
        }
        ShortcutAction::SwitchEditorTabNext => {
            vec![binding_code(ShortcutKeyCode::ArrowRight, true, true, false)]
        }
        ShortcutAction::FocusWorkspacePanePrevious => {
            vec![binding_code(
                ShortcutKeyCode::BracketLeft,
                true,
                true,
                false,
            )]
        }
        ShortcutAction::FocusWorkspacePaneNext => {
            vec![binding_code(
                ShortcutKeyCode::BracketRight,
                true,
                true,
                false,
            )]
        }
        ShortcutAction::ScoreZoomIn
        | ShortcutAction::EditorZoomIn
        | ShortcutAction::PianoRollZoomIn => vec![
            binding_code(ShortcutKeyCode::Equal, true, false, false),
            binding_code(ShortcutKeyCode::Equal, true, false, true),
            binding_code(ShortcutKeyCode::NumpadAdd, true, false, false),
        ],
        ShortcutAction::ScoreZoomOut
        | ShortcutAction::EditorZoomOut
        | ShortcutAction::PianoRollZoomOut => vec![
            binding_code(ShortcutKeyCode::Minus, true, false, false),
            binding_code(ShortcutKeyCode::Minus, true, false, true),
            binding_code(ShortcutKeyCode::NumpadSubtract, true, false, false),
        ],
        ShortcutAction::ScoreZoomReset
        | ShortcutAction::EditorZoomReset
        | ShortcutAction::PianoRollZoomReset => vec![
            binding_code(ShortcutKeyCode::Digit0, true, false, false),
            binding_code(ShortcutKeyCode::Numpad0, true, false, false),
        ],
        ShortcutAction::TransportPlayPause => {
            vec![binding_named(ShortcutNamedKey::Space, false, false, false)]
        }
        ShortcutAction::TransportRewind => vec![
            binding_named(ShortcutNamedKey::Enter, false, false, false),
            binding_code(ShortcutKeyCode::NumpadEnter, false, false, false),
        ],
        ShortcutAction::PianoRollCursorSubdivisionPrevious
        | ShortcutAction::PianoRollCursorSubdivisionNext
        | ShortcutAction::PianoRollScrollUp
        | ShortcutAction::PianoRollScrollDown
        | ShortcutAction::ScoreScrollUp
        | ShortcutAction::ScoreScrollDown
        | ShortcutAction::ScorePrevPage
        | ShortcutAction::ScoreNextPage => Vec::new(),
    }
}

fn binding_matches(binding: ShortcutBinding, input: ShortcutInput<'_>) -> bool {
    #[cfg(target_os = "macos")]
    {
        if input.modifiers.command() != binding.primary
            || input.modifiers.control() != binding.control
            || input.modifiers.alt() != binding.alt
            || input.modifiers.shift() != binding.shift
        {
            return false;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let expects_ctrl = binding.primary || binding.control;
        if input.modifiers.control() != expects_ctrl
            || input.modifiers.alt() != binding.alt
            || input.modifiers.shift() != binding.shift
            || input.modifiers.command()
        {
            return false;
        }
    }

    match binding.key {
        ShortcutKey::Code(code) => {
            input.physical_key == keyboard::key::Physical::Code(to_iced_key_code(code))
        }
        ShortcutKey::Named(named) => {
            input.key.as_ref() == keyboard::Key::Named(to_iced_named_key(named))
        }
    }
}

fn format_binding(binding: ShortcutBinding) -> String {
    let mut parts = Vec::new();

    if binding.primary {
        parts.push(platform_primary_label());
    }
    if binding.control {
        parts.push("Ctrl");
    }
    if binding.alt {
        parts.push(platform_alt_label());
    }
    if binding.shift {
        parts.push("Shift");
    }

    parts.push(match binding.key {
        ShortcutKey::Code(code) => code_label(code),
        ShortcutKey::Named(named) => named_label(named),
    });

    parts.join("+")
}

fn platform_primary_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Cmd"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Ctrl"
    }
}

fn platform_alt_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Alt"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Alt"
    }
}

fn code_label(code: ShortcutKeyCode) -> &'static str {
    match code {
        ShortcutKeyCode::KeyA => "A",
        ShortcutKeyCode::KeyC => "C",
        ShortcutKeyCode::Comma => ",",
        ShortcutKeyCode::KeyF => "F",
        ShortcutKeyCode::KeyG => "G",
        ShortcutKeyCode::KeyH => "H",
        ShortcutKeyCode::KeyJ => "J",
        ShortcutKeyCode::KeyK => "K",
        ShortcutKeyCode::KeyL => "L",
        ShortcutKeyCode::KeyN => "N",
        ShortcutKeyCode::KeyO => "O",
        ShortcutKeyCode::KeyP => "P",
        ShortcutKeyCode::KeyQ => "Q",
        ShortcutKeyCode::KeyS => "S",
        ShortcutKeyCode::KeyX => "X",
        ShortcutKeyCode::KeyV => "V",
        ShortcutKeyCode::KeyW => "W",
        ShortcutKeyCode::KeyY => "Y",
        ShortcutKeyCode::KeyZ => "Z",
        ShortcutKeyCode::Digit1 | ShortcutKeyCode::Numpad1 => "1",
        ShortcutKeyCode::Digit2 | ShortcutKeyCode::Numpad2 => "2",
        ShortcutKeyCode::Digit3 | ShortcutKeyCode::Numpad3 => "3",
        ShortcutKeyCode::Digit4 | ShortcutKeyCode::Numpad4 => "4",
        ShortcutKeyCode::Slash => "/",
        ShortcutKeyCode::Backslash => "\\",
        ShortcutKeyCode::ArrowLeft => "Left",
        ShortcutKeyCode::ArrowRight => "Right",
        ShortcutKeyCode::ArrowUp => "Up",
        ShortcutKeyCode::ArrowDown => "Down",
        ShortcutKeyCode::Backspace => "Backspace",
        ShortcutKeyCode::Delete => "Delete",
        ShortcutKeyCode::Home => "Home",
        ShortcutKeyCode::End => "End",
        ShortcutKeyCode::Insert => "Insert",
        ShortcutKeyCode::F3 => "F3",
        ShortcutKeyCode::Equal | ShortcutKeyCode::NumpadAdd => "+",
        ShortcutKeyCode::Minus | ShortcutKeyCode::NumpadSubtract => "-",
        ShortcutKeyCode::Digit0 | ShortcutKeyCode::Numpad0 => "0",
        ShortcutKeyCode::BracketLeft => "[",
        ShortcutKeyCode::BracketRight => "]",
        ShortcutKeyCode::NumpadEnter => "Enter",
    }
}

fn named_label(named: ShortcutNamedKey) -> &'static str {
    match named {
        ShortcutNamedKey::Space => "Space",
        ShortcutNamedKey::Enter => "Enter",
    }
}

const fn binding_code(
    code: ShortcutKeyCode,
    primary: bool,
    alt: bool,
    shift: bool,
) -> ShortcutBinding {
    ShortcutBinding {
        key: ShortcutKey::Code(code),
        primary,
        control: false,
        alt,
        shift,
    }
}

const fn binding_named(
    named: ShortcutNamedKey,
    primary: bool,
    alt: bool,
    shift: bool,
) -> ShortcutBinding {
    ShortcutBinding {
        key: ShortcutKey::Named(named),
        primary,
        control: false,
        alt,
        shift,
    }
}

const fn binding_named_ctrl(named: ShortcutNamedKey, alt: bool, shift: bool) -> ShortcutBinding {
    ShortcutBinding {
        key: ShortcutKey::Named(named),
        primary: false,
        control: true,
        alt,
        shift,
    }
}

fn to_iced_key_code(code: ShortcutKeyCode) -> keyboard::key::Code {
    match code {
        ShortcutKeyCode::KeyA => keyboard::key::Code::KeyA,
        ShortcutKeyCode::KeyC => keyboard::key::Code::KeyC,
        ShortcutKeyCode::Comma => keyboard::key::Code::Comma,
        ShortcutKeyCode::KeyF => keyboard::key::Code::KeyF,
        ShortcutKeyCode::KeyG => keyboard::key::Code::KeyG,
        ShortcutKeyCode::KeyH => keyboard::key::Code::KeyH,
        ShortcutKeyCode::KeyJ => keyboard::key::Code::KeyJ,
        ShortcutKeyCode::KeyK => keyboard::key::Code::KeyK,
        ShortcutKeyCode::KeyL => keyboard::key::Code::KeyL,
        ShortcutKeyCode::KeyN => keyboard::key::Code::KeyN,
        ShortcutKeyCode::KeyO => keyboard::key::Code::KeyO,
        ShortcutKeyCode::KeyP => keyboard::key::Code::KeyP,
        ShortcutKeyCode::KeyQ => keyboard::key::Code::KeyQ,
        ShortcutKeyCode::KeyS => keyboard::key::Code::KeyS,
        ShortcutKeyCode::KeyX => keyboard::key::Code::KeyX,
        ShortcutKeyCode::KeyV => keyboard::key::Code::KeyV,
        ShortcutKeyCode::KeyW => keyboard::key::Code::KeyW,
        ShortcutKeyCode::KeyY => keyboard::key::Code::KeyY,
        ShortcutKeyCode::KeyZ => keyboard::key::Code::KeyZ,
        ShortcutKeyCode::Digit1 => keyboard::key::Code::Digit1,
        ShortcutKeyCode::Digit2 => keyboard::key::Code::Digit2,
        ShortcutKeyCode::Digit3 => keyboard::key::Code::Digit3,
        ShortcutKeyCode::Digit4 => keyboard::key::Code::Digit4,
        ShortcutKeyCode::Slash => keyboard::key::Code::Slash,
        ShortcutKeyCode::Backslash => keyboard::key::Code::Backslash,
        ShortcutKeyCode::ArrowLeft => keyboard::key::Code::ArrowLeft,
        ShortcutKeyCode::ArrowRight => keyboard::key::Code::ArrowRight,
        ShortcutKeyCode::ArrowUp => keyboard::key::Code::ArrowUp,
        ShortcutKeyCode::ArrowDown => keyboard::key::Code::ArrowDown,
        ShortcutKeyCode::Backspace => keyboard::key::Code::Backspace,
        ShortcutKeyCode::Delete => keyboard::key::Code::Delete,
        ShortcutKeyCode::Home => keyboard::key::Code::Home,
        ShortcutKeyCode::End => keyboard::key::Code::End,
        ShortcutKeyCode::Insert => keyboard::key::Code::Insert,
        ShortcutKeyCode::F3 => keyboard::key::Code::F3,
        ShortcutKeyCode::Numpad1 => keyboard::key::Code::Numpad1,
        ShortcutKeyCode::Numpad2 => keyboard::key::Code::Numpad2,
        ShortcutKeyCode::Numpad3 => keyboard::key::Code::Numpad3,
        ShortcutKeyCode::Numpad4 => keyboard::key::Code::Numpad4,
        ShortcutKeyCode::Equal => keyboard::key::Code::Equal,
        ShortcutKeyCode::Minus => keyboard::key::Code::Minus,
        ShortcutKeyCode::Digit0 => keyboard::key::Code::Digit0,
        ShortcutKeyCode::NumpadAdd => keyboard::key::Code::NumpadAdd,
        ShortcutKeyCode::NumpadSubtract => keyboard::key::Code::NumpadSubtract,
        ShortcutKeyCode::Numpad0 => keyboard::key::Code::Numpad0,
        ShortcutKeyCode::BracketLeft => keyboard::key::Code::BracketLeft,
        ShortcutKeyCode::BracketRight => keyboard::key::Code::BracketRight,
        ShortcutKeyCode::NumpadEnter => keyboard::key::Code::NumpadEnter,
    }
}

fn to_iced_named_key(named: ShortcutNamedKey) -> keyboard::key::Named {
    match named {
        ShortcutNamedKey::Space => keyboard::key::Named::Space,
        ShortcutNamedKey::Enter => keyboard::key::Named::Enter,
    }
}

fn action_id(action: ShortcutAction) -> Option<ShortcutActionId> {
    match action {
        ShortcutAction::QuitApp => Some(ShortcutActionId::QuitApp),
        ShortcutAction::OpenActions => Some(ShortcutActionId::OpenActions),
        ShortcutAction::OpenSettingsFile => Some(ShortcutActionId::OpenSettingsFile),
        ShortcutAction::NewEditor => Some(ShortcutActionId::NewEditor),
        ShortcutAction::OpenEditorFile => Some(ShortcutActionId::OpenEditorFile),
        ShortcutAction::ToggleFileBrowser => Some(ShortcutActionId::ToggleFileBrowser),
        ShortcutAction::FileBrowserUndo => Some(ShortcutActionId::FileBrowserUndo),
        ShortcutAction::FileBrowserRedo => Some(ShortcutActionId::FileBrowserRedo),
        ShortcutAction::FileBrowserCut => Some(ShortcutActionId::FileBrowserCut),
        ShortcutAction::FileBrowserCopy => Some(ShortcutActionId::FileBrowserCopy),
        ShortcutAction::FileBrowserPaste => Some(ShortcutActionId::FileBrowserPaste),
        ShortcutAction::FileBrowserRename => Some(ShortcutActionId::FileBrowserRename),
        ShortcutAction::FileBrowserDelete => Some(ShortcutActionId::FileBrowserDelete),
        ShortcutAction::SaveEditor => Some(ShortcutActionId::SaveEditor),
        ShortcutAction::CloseEditorTab => Some(ShortcutActionId::CloseEditorTab),
        ShortcutAction::EditorUndo => Some(ShortcutActionId::EditorUndo),
        ShortcutAction::EditorRedo => Some(ShortcutActionId::EditorRedo),
        ShortcutAction::EditorCopy => Some(ShortcutActionId::EditorCopy),
        ShortcutAction::EditorPaste => Some(ShortcutActionId::EditorPaste),
        ShortcutAction::EditorOpenSearch => Some(ShortcutActionId::EditorOpenSearch),
        ShortcutAction::EditorOpenSearchReplace => Some(ShortcutActionId::EditorOpenSearchReplace),
        ShortcutAction::EditorOpenGotoLine => Some(ShortcutActionId::EditorOpenGotoLine),
        ShortcutAction::EditorTriggerCompletion => Some(ShortcutActionId::EditorTriggerCompletion),
        ShortcutAction::EditorFindNext => Some(ShortcutActionId::EditorFindNext),
        ShortcutAction::EditorFindPrevious => Some(ShortcutActionId::EditorFindPrevious),
        ShortcutAction::EditorWordLeft => Some(ShortcutActionId::EditorWordLeft),
        ShortcutAction::EditorWordRight => Some(ShortcutActionId::EditorWordRight),
        ShortcutAction::EditorWordLeftSelect => Some(ShortcutActionId::EditorWordLeftSelect),
        ShortcutAction::EditorWordRightSelect => Some(ShortcutActionId::EditorWordRightSelect),
        ShortcutAction::EditorDeleteWordBackward => {
            Some(ShortcutActionId::EditorDeleteWordBackward)
        }
        ShortcutAction::EditorDeleteWordForward => Some(ShortcutActionId::EditorDeleteWordForward),
        ShortcutAction::EditorDeleteToLineStart => Some(ShortcutActionId::EditorDeleteToLineStart),
        ShortcutAction::EditorDeleteToLineEnd => Some(ShortcutActionId::EditorDeleteToLineEnd),
        ShortcutAction::EditorLineStart => Some(ShortcutActionId::EditorLineStart),
        ShortcutAction::EditorLineEnd => Some(ShortcutActionId::EditorLineEnd),
        ShortcutAction::EditorLineStartSelect => Some(ShortcutActionId::EditorLineStartSelect),
        ShortcutAction::EditorLineEndSelect => Some(ShortcutActionId::EditorLineEndSelect),
        ShortcutAction::EditorDocumentStart => Some(ShortcutActionId::EditorDocumentStart),
        ShortcutAction::EditorDocumentEnd => Some(ShortcutActionId::EditorDocumentEnd),
        ShortcutAction::EditorDocumentStartSelect => {
            Some(ShortcutActionId::EditorDocumentStartSelect)
        }
        ShortcutAction::EditorDocumentEndSelect => Some(ShortcutActionId::EditorDocumentEndSelect),
        ShortcutAction::EditorDeleteSelection => Some(ShortcutActionId::EditorDeleteSelection),
        ShortcutAction::EditorSelectAll => Some(ShortcutActionId::EditorSelectAll),
        ShortcutAction::EditorInsertLineBelow => Some(ShortcutActionId::EditorInsertLineBelow),
        ShortcutAction::EditorInsertLineAbove => Some(ShortcutActionId::EditorInsertLineAbove),
        ShortcutAction::EditorDeleteLine => Some(ShortcutActionId::EditorDeleteLine),
        ShortcutAction::EditorMoveLineUp => Some(ShortcutActionId::EditorMoveLineUp),
        ShortcutAction::EditorMoveLineDown => Some(ShortcutActionId::EditorMoveLineDown),
        ShortcutAction::EditorCopyLineUp => Some(ShortcutActionId::EditorCopyLineUp),
        ShortcutAction::EditorCopyLineDown => Some(ShortcutActionId::EditorCopyLineDown),
        ShortcutAction::EditorJoinLines => Some(ShortcutActionId::EditorJoinLines),
        ShortcutAction::EditorIndent => Some(ShortcutActionId::EditorIndent),
        ShortcutAction::EditorOutdent => Some(ShortcutActionId::EditorOutdent),
        ShortcutAction::EditorToggleLineComment => Some(ShortcutActionId::EditorToggleLineComment),
        ShortcutAction::EditorToggleBlockComment => {
            Some(ShortcutActionId::EditorToggleBlockComment)
        }
        ShortcutAction::EditorSelectLine => Some(ShortcutActionId::EditorSelectLine),
        ShortcutAction::EditorJumpToMatchingBracket => {
            Some(ShortcutActionId::EditorJumpToMatchingBracket)
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor) => {
            Some(ShortcutActionId::ToggleEditorPane)
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score) => {
            Some(ShortcutActionId::ToggleScorePane)
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll) => {
            Some(ShortcutActionId::TogglePianoRollPane)
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer) => {
            Some(ShortcutActionId::ToggleMixerPane)
        }
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger) => {
            Some(ShortcutActionId::ToggleLoggerPane)
        }
        ShortcutAction::SwitchWorkspaceTabPrevious => Some(ShortcutActionId::PreviousTab),
        ShortcutAction::SwitchWorkspaceTabNext => Some(ShortcutActionId::NextTab),
        ShortcutAction::SwitchEditorTabPrevious => Some(ShortcutActionId::PreviousEditorTab),
        ShortcutAction::SwitchEditorTabNext => Some(ShortcutActionId::NextEditorTab),
        ShortcutAction::FocusWorkspacePanePrevious => Some(ShortcutActionId::PreviousPane),
        ShortcutAction::FocusWorkspacePaneNext => Some(ShortcutActionId::NextPane),
        ShortcutAction::ScoreZoomIn => Some(ShortcutActionId::ScoreZoomIn),
        ShortcutAction::ScoreZoomOut => Some(ShortcutActionId::ScoreZoomOut),
        ShortcutAction::ScoreZoomReset => Some(ShortcutActionId::ScoreZoomReset),
        ShortcutAction::EditorZoomIn => Some(ShortcutActionId::EditorZoomIn),
        ShortcutAction::EditorZoomOut => Some(ShortcutActionId::EditorZoomOut),
        ShortcutAction::EditorZoomReset => Some(ShortcutActionId::EditorZoomReset),
        ShortcutAction::PianoRollZoomIn => Some(ShortcutActionId::PianoRollZoomIn),
        ShortcutAction::PianoRollZoomOut => Some(ShortcutActionId::PianoRollZoomOut),
        ShortcutAction::PianoRollZoomReset => Some(ShortcutActionId::PianoRollZoomReset),
        ShortcutAction::TransportPlayPause => Some(ShortcutActionId::TransportPlayPause),
        ShortcutAction::TransportRewind => Some(ShortcutActionId::TransportRewind),
        ShortcutAction::PianoRollCursorSubdivisionPrevious
        | ShortcutAction::PianoRollCursorSubdivisionNext
        | ShortcutAction::PianoRollScrollUp
        | ShortcutAction::PianoRollScrollDown
        | ShortcutAction::ScoreScrollUp
        | ShortcutAction::ScoreScrollDown
        | ShortcutAction::ScorePrevPage
        | ShortcutAction::ScoreNextPage => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_input(
        code: keyboard::key::Code,
        primary: bool,
        alt: bool,
        shift: bool,
    ) -> ShortcutInput<'static> {
        let mut modifiers = keyboard::Modifiers::default();
        if primary {
            if cfg!(target_os = "macos") {
                modifiers.insert(keyboard::Modifiers::COMMAND);
            } else {
                modifiers.insert(keyboard::Modifiers::CTRL);
            }
        }
        if alt {
            modifiers.insert(keyboard::Modifiers::ALT);
        }
        if shift {
            modifiers.insert(keyboard::Modifiers::SHIFT);
        }
        ShortcutInput {
            key: Box::leak(Box::new(keyboard::Key::Character("".into()))),
            physical_key: keyboard::key::Physical::Code(code),
            modifiers,
        }
    }

    fn named_input(named: keyboard::key::Named) -> ShortcutInput<'static> {
        ShortcutInput {
            key: Box::leak(Box::new(keyboard::Key::Named(named))),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            modifiers: keyboard::Modifiers::default(),
        }
    }

    #[test]
    fn resolves_editor_toggle_line_comment_binding() {
        let input = code_input(keyboard::key::Code::Slash, true, false, false);
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Editor, input),
            Some(ShortcutAction::EditorToggleLineComment)
        );
    }

    #[test]
    fn resolves_editor_move_line_down_binding() {
        let input = code_input(keyboard::key::Code::ArrowDown, false, true, false);
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Editor, input),
            Some(ShortcutAction::EditorMoveLineDown)
        );
    }

    #[test]
    fn resolves_browser_rename_binding() {
        let input = named_input(keyboard::key::Named::Enter);
        assert_eq!(
            resolve_editor_browser(&ShortcutSettings::default(), input),
            Some(ShortcutAction::FileBrowserRename)
        );
    }

    #[test]
    fn resolves_browser_delete_binding() {
        let input = if cfg!(target_os = "macos") {
            code_input(keyboard::key::Code::Delete, true, false, false)
        } else {
            code_input(keyboard::key::Code::Delete, false, false, false)
        };
        assert_eq!(
            resolve_editor_browser(&ShortcutSettings::default(), input),
            Some(ShortcutAction::FileBrowserDelete)
        );
    }
}
