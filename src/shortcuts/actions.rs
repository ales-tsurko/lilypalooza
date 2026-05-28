use super::*;

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
    ToggleMetronome,
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

pub(super) const GLOBAL_ACTIONS: [ShortcutAction; 10] = [
    ShortcutAction::QuitApp,
    ShortcutAction::OpenActions,
    ShortcutAction::OpenSettingsFile,
    ShortcutAction::SaveEditor,
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger),
    ShortcutAction::ToggleMetronome,
];

pub(super) const NAVIGATION_ACTIONS: [ShortcutAction; 4] = [
    ShortcutAction::SwitchWorkspaceTabPrevious,
    ShortcutAction::SwitchWorkspaceTabNext,
    ShortcutAction::FocusWorkspacePanePrevious,
    ShortcutAction::FocusWorkspacePaneNext,
];

pub(super) const SCORE_CONTEXTUAL_ACTIONS: [ShortcutAction; 6] = [
    ShortcutAction::ScoreZoomIn,
    ShortcutAction::ScoreZoomOut,
    ShortcutAction::ScoreZoomReset,
    ShortcutAction::TransportPlayPause,
    ShortcutAction::TransportRewind,
    ShortcutAction::ScoreScrollUp,
];

pub(super) const PIANO_ROLL_CONTEXTUAL_ACTIONS: [ShortcutAction; 5] = [
    ShortcutAction::PianoRollZoomIn,
    ShortcutAction::PianoRollZoomOut,
    ShortcutAction::PianoRollZoomReset,
    ShortcutAction::TransportPlayPause,
    ShortcutAction::TransportRewind,
];

pub(super) const MIXER_CONTEXTUAL_ACTIONS: [ShortcutAction; 4] = [
    ShortcutAction::TransportPlayPause,
    ShortcutAction::TransportRewind,
    ShortcutAction::EditorUndo,
    ShortcutAction::EditorRedo,
];

pub(super) const EDITOR_CONTEXTUAL_ACTIONS: &[ShortcutAction] = &[
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

pub(super) const EDITOR_BROWSER_ACTIONS: &[ShortcutAction] = &[
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
        WorkspacePane::Mixer => MIXER_CONTEXTUAL_ACTIONS
            .iter()
            .copied()
            .find(|action| action_matches(settings, *action, input)),
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
    action_from_id(action_id)
        .and_then(|action| display_binding_for_action(settings, action))
        .map(format_binding)
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

pub(crate) fn action_for_id(action_id: ShortcutActionId) -> Option<ShortcutAction> {
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
    ToggleMetronome => ("Toggle Metronome", "Transport: enable or disable the metronome."),
}

pub(super) fn fixed_contextual_action(
    pane: WorkspacePane,
    input: ShortcutInput<'_>,
) -> Option<ShortcutAction> {
    if input.modifiers.command() || input.modifiers.control() || input.modifiers.alt() {
        return None;
    }

    let keyboard::Key::Named(key) = input.key.as_ref() else {
        return None;
    };

    fixed_arrow_action(pane, key)
}

pub(super) fn fixed_arrow_action(
    pane: WorkspacePane,
    key: keyboard::key::Named,
) -> Option<ShortcutAction> {
    const FIXED_ARROW_ACTIONS: &[(WorkspacePane, keyboard::key::Named, ShortcutAction)] = &[
        (
            WorkspacePane::Score,
            keyboard::key::Named::ArrowUp,
            ShortcutAction::ScoreScrollUp,
        ),
        (
            WorkspacePane::Score,
            keyboard::key::Named::ArrowDown,
            ShortcutAction::ScoreScrollDown,
        ),
        (
            WorkspacePane::Score,
            keyboard::key::Named::ArrowLeft,
            ShortcutAction::ScorePrevPage,
        ),
        (
            WorkspacePane::Score,
            keyboard::key::Named::ArrowRight,
            ShortcutAction::ScoreNextPage,
        ),
        (
            WorkspacePane::PianoRoll,
            keyboard::key::Named::ArrowLeft,
            ShortcutAction::PianoRollCursorSubdivisionPrevious,
        ),
        (
            WorkspacePane::PianoRoll,
            keyboard::key::Named::ArrowRight,
            ShortcutAction::PianoRollCursorSubdivisionNext,
        ),
        (
            WorkspacePane::PianoRoll,
            keyboard::key::Named::ArrowUp,
            ShortcutAction::PianoRollScrollUp,
        ),
        (
            WorkspacePane::PianoRoll,
            keyboard::key::Named::ArrowDown,
            ShortcutAction::PianoRollScrollDown,
        ),
    ];

    FIXED_ARROW_ACTIONS
        .iter()
        .find_map(|(candidate_pane, candidate_key, action)| {
            (*candidate_pane == pane && *candidate_key == key).then_some(*action)
        })
}

pub(super) fn action_matches(
    settings: &ShortcutSettings,
    action: ShortcutAction,
    input: ShortcutInput<'_>,
) -> bool {
    effective_bindings(settings, action)
        .iter()
        .any(|binding| binding_matches(*binding, input))
}

pub(super) const ACTION_BY_ID: &[(ShortcutActionId, ShortcutAction)] = &[
    (ShortcutActionId::QuitApp, ShortcutAction::QuitApp),
    (ShortcutActionId::OpenActions, ShortcutAction::OpenActions),
    (
        ShortcutActionId::OpenSettingsFile,
        ShortcutAction::OpenSettingsFile,
    ),
    (ShortcutActionId::NewEditor, ShortcutAction::NewEditor),
    (
        ShortcutActionId::OpenEditorFile,
        ShortcutAction::OpenEditorFile,
    ),
    (
        ShortcutActionId::ToggleFileBrowser,
        ShortcutAction::ToggleFileBrowser,
    ),
    (
        ShortcutActionId::FileBrowserUndo,
        ShortcutAction::FileBrowserUndo,
    ),
    (
        ShortcutActionId::FileBrowserRedo,
        ShortcutAction::FileBrowserRedo,
    ),
    (
        ShortcutActionId::FileBrowserCut,
        ShortcutAction::FileBrowserCut,
    ),
    (
        ShortcutActionId::FileBrowserCopy,
        ShortcutAction::FileBrowserCopy,
    ),
    (
        ShortcutActionId::FileBrowserPaste,
        ShortcutAction::FileBrowserPaste,
    ),
    (
        ShortcutActionId::FileBrowserRename,
        ShortcutAction::FileBrowserRename,
    ),
    (
        ShortcutActionId::FileBrowserDelete,
        ShortcutAction::FileBrowserDelete,
    ),
    (ShortcutActionId::SaveEditor, ShortcutAction::SaveEditor),
    (
        ShortcutActionId::CloseEditorTab,
        ShortcutAction::CloseEditorTab,
    ),
    (ShortcutActionId::EditorUndo, ShortcutAction::EditorUndo),
    (ShortcutActionId::EditorRedo, ShortcutAction::EditorRedo),
    (ShortcutActionId::EditorCopy, ShortcutAction::EditorCopy),
    (ShortcutActionId::EditorPaste, ShortcutAction::EditorPaste),
    (
        ShortcutActionId::EditorOpenSearch,
        ShortcutAction::EditorOpenSearch,
    ),
    (
        ShortcutActionId::EditorOpenSearchReplace,
        ShortcutAction::EditorOpenSearchReplace,
    ),
    (
        ShortcutActionId::EditorOpenGotoLine,
        ShortcutAction::EditorOpenGotoLine,
    ),
    (
        ShortcutActionId::EditorTriggerCompletion,
        ShortcutAction::EditorTriggerCompletion,
    ),
    (
        ShortcutActionId::EditorFindNext,
        ShortcutAction::EditorFindNext,
    ),
    (
        ShortcutActionId::EditorFindPrevious,
        ShortcutAction::EditorFindPrevious,
    ),
    (
        ShortcutActionId::EditorWordLeft,
        ShortcutAction::EditorWordLeft,
    ),
    (
        ShortcutActionId::EditorWordRight,
        ShortcutAction::EditorWordRight,
    ),
    (
        ShortcutActionId::EditorWordLeftSelect,
        ShortcutAction::EditorWordLeftSelect,
    ),
    (
        ShortcutActionId::EditorWordRightSelect,
        ShortcutAction::EditorWordRightSelect,
    ),
    (
        ShortcutActionId::EditorDeleteWordBackward,
        ShortcutAction::EditorDeleteWordBackward,
    ),
    (
        ShortcutActionId::EditorDeleteWordForward,
        ShortcutAction::EditorDeleteWordForward,
    ),
    (
        ShortcutActionId::EditorDeleteToLineStart,
        ShortcutAction::EditorDeleteToLineStart,
    ),
    (
        ShortcutActionId::EditorDeleteToLineEnd,
        ShortcutAction::EditorDeleteToLineEnd,
    ),
    (
        ShortcutActionId::EditorLineStart,
        ShortcutAction::EditorLineStart,
    ),
    (
        ShortcutActionId::EditorLineEnd,
        ShortcutAction::EditorLineEnd,
    ),
    (
        ShortcutActionId::EditorLineStartSelect,
        ShortcutAction::EditorLineStartSelect,
    ),
    (
        ShortcutActionId::EditorLineEndSelect,
        ShortcutAction::EditorLineEndSelect,
    ),
    (
        ShortcutActionId::EditorDocumentStart,
        ShortcutAction::EditorDocumentStart,
    ),
    (
        ShortcutActionId::EditorDocumentEnd,
        ShortcutAction::EditorDocumentEnd,
    ),
    (
        ShortcutActionId::EditorDocumentStartSelect,
        ShortcutAction::EditorDocumentStartSelect,
    ),
    (
        ShortcutActionId::EditorDocumentEndSelect,
        ShortcutAction::EditorDocumentEndSelect,
    ),
    (
        ShortcutActionId::EditorDeleteSelection,
        ShortcutAction::EditorDeleteSelection,
    ),
    (
        ShortcutActionId::EditorSelectAll,
        ShortcutAction::EditorSelectAll,
    ),
    (
        ShortcutActionId::EditorInsertLineBelow,
        ShortcutAction::EditorInsertLineBelow,
    ),
    (
        ShortcutActionId::EditorInsertLineAbove,
        ShortcutAction::EditorInsertLineAbove,
    ),
    (
        ShortcutActionId::EditorDeleteLine,
        ShortcutAction::EditorDeleteLine,
    ),
    (
        ShortcutActionId::EditorMoveLineUp,
        ShortcutAction::EditorMoveLineUp,
    ),
    (
        ShortcutActionId::EditorMoveLineDown,
        ShortcutAction::EditorMoveLineDown,
    ),
    (
        ShortcutActionId::EditorCopyLineUp,
        ShortcutAction::EditorCopyLineUp,
    ),
    (
        ShortcutActionId::EditorCopyLineDown,
        ShortcutAction::EditorCopyLineDown,
    ),
    (
        ShortcutActionId::EditorJoinLines,
        ShortcutAction::EditorJoinLines,
    ),
    (ShortcutActionId::EditorIndent, ShortcutAction::EditorIndent),
    (
        ShortcutActionId::EditorOutdent,
        ShortcutAction::EditorOutdent,
    ),
    (
        ShortcutActionId::EditorToggleLineComment,
        ShortcutAction::EditorToggleLineComment,
    ),
    (
        ShortcutActionId::EditorToggleBlockComment,
        ShortcutAction::EditorToggleBlockComment,
    ),
    (
        ShortcutActionId::EditorSelectLine,
        ShortcutAction::EditorSelectLine,
    ),
    (
        ShortcutActionId::EditorJumpToMatchingBracket,
        ShortcutAction::EditorJumpToMatchingBracket,
    ),
    (
        ShortcutActionId::ToggleEditorPane,
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor),
    ),
    (
        ShortcutActionId::ToggleScorePane,
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score),
    ),
    (
        ShortcutActionId::TogglePianoRollPane,
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll),
    ),
    (
        ShortcutActionId::ToggleMixerPane,
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer),
    ),
    (
        ShortcutActionId::ToggleLoggerPane,
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger),
    ),
    (
        ShortcutActionId::PreviousTab,
        ShortcutAction::SwitchWorkspaceTabPrevious,
    ),
    (
        ShortcutActionId::NextTab,
        ShortcutAction::SwitchWorkspaceTabNext,
    ),
    (
        ShortcutActionId::PreviousEditorTab,
        ShortcutAction::SwitchEditorTabPrevious,
    ),
    (
        ShortcutActionId::NextEditorTab,
        ShortcutAction::SwitchEditorTabNext,
    ),
    (
        ShortcutActionId::PreviousPane,
        ShortcutAction::FocusWorkspacePanePrevious,
    ),
    (
        ShortcutActionId::NextPane,
        ShortcutAction::FocusWorkspacePaneNext,
    ),
    (ShortcutActionId::ScoreZoomIn, ShortcutAction::ScoreZoomIn),
    (ShortcutActionId::ScoreZoomOut, ShortcutAction::ScoreZoomOut),
    (
        ShortcutActionId::ScoreZoomReset,
        ShortcutAction::ScoreZoomReset,
    ),
    (ShortcutActionId::EditorZoomIn, ShortcutAction::EditorZoomIn),
    (
        ShortcutActionId::EditorZoomOut,
        ShortcutAction::EditorZoomOut,
    ),
    (
        ShortcutActionId::EditorZoomReset,
        ShortcutAction::EditorZoomReset,
    ),
    (
        ShortcutActionId::PianoRollZoomIn,
        ShortcutAction::PianoRollZoomIn,
    ),
    (
        ShortcutActionId::PianoRollZoomOut,
        ShortcutAction::PianoRollZoomOut,
    ),
    (
        ShortcutActionId::PianoRollZoomReset,
        ShortcutAction::PianoRollZoomReset,
    ),
    (
        ShortcutActionId::TransportPlayPause,
        ShortcutAction::TransportPlayPause,
    ),
    (
        ShortcutActionId::TransportRewind,
        ShortcutAction::TransportRewind,
    ),
    (
        ShortcutActionId::ToggleMetronome,
        ShortcutAction::ToggleMetronome,
    ),
];
