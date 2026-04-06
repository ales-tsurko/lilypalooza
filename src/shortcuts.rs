use iced::keyboard;

use crate::settings::{
    ShortcutActionId, ShortcutBinding, ShortcutBindingOverride, ShortcutKey, ShortcutKeyCode,
    ShortcutNamedKey, ShortcutSettings, WorkspacePane,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutAction {
    QuitApp,
    NewEditor,
    OpenEditorFile,
    SaveEditor,
    CloseEditorTab,
    EditorUndo,
    EditorRedo,
    EditorCopy,
    EditorPaste,
    EditorOpenSearch,
    EditorOpenSearchReplace,
    EditorOpenGotoLine,
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

const GLOBAL_ACTIONS: [ShortcutAction; 6] = [
    ShortcutAction::QuitApp,
    ShortcutAction::SaveEditor,
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score),
    ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll),
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
    ShortcutAction::EditorUndo,
    ShortcutAction::EditorRedo,
    ShortcutAction::EditorCopy,
    ShortcutAction::EditorPaste,
    ShortcutAction::EditorOpenSearch,
    ShortcutAction::EditorOpenSearchReplace,
    ShortcutAction::EditorOpenGotoLine,
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
        WorkspacePane::Editor => EDITOR_CONTEXTUAL_ACTIONS
            .iter()
            .copied()
            .find(|action| action_matches(settings, *action, input)),
        WorkspacePane::Logger => None,
    };

    remappable_match.or_else(|| fixed_contextual_action(pane, input))
}

pub(crate) fn label_for_action(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Option<String> {
    display_binding_for_action(settings, action).map(format_binding)
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
        WorkspacePane::Editor | WorkspacePane::Logger => None,
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
        ShortcutAction::NewEditor => vec![binding_code(ShortcutKeyCode::KeyN, true, false, false)],
        ShortcutAction::OpenEditorFile => {
            vec![binding_code(ShortcutKeyCode::KeyO, true, false, false)]
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
        ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger) => vec![
            binding_code(ShortcutKeyCode::Digit4, true, false, false),
            binding_code(ShortcutKeyCode::Numpad4, true, false, false),
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
    let has_primary_modifier = input.modifiers.command() || input.modifiers.control();
    if has_primary_modifier != binding.primary
        || input.modifiers.alt() != binding.alt
        || input.modifiers.shift() != binding.shift
    {
        return false;
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
        ShortcutKeyCode::KeyF => "F",
        ShortcutKeyCode::KeyG => "G",
        ShortcutKeyCode::KeyH => "H",
        ShortcutKeyCode::KeyJ => "J",
        ShortcutKeyCode::KeyK => "K",
        ShortcutKeyCode::KeyL => "L",
        ShortcutKeyCode::KeyN => "N",
        ShortcutKeyCode::KeyO => "O",
        ShortcutKeyCode::KeyQ => "Q",
        ShortcutKeyCode::KeyS => "S",
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
        alt,
        shift,
    }
}

fn to_iced_key_code(code: ShortcutKeyCode) -> keyboard::key::Code {
    match code {
        ShortcutKeyCode::KeyA => keyboard::key::Code::KeyA,
        ShortcutKeyCode::KeyC => keyboard::key::Code::KeyC,
        ShortcutKeyCode::KeyF => keyboard::key::Code::KeyF,
        ShortcutKeyCode::KeyG => keyboard::key::Code::KeyG,
        ShortcutKeyCode::KeyH => keyboard::key::Code::KeyH,
        ShortcutKeyCode::KeyJ => keyboard::key::Code::KeyJ,
        ShortcutKeyCode::KeyK => keyboard::key::Code::KeyK,
        ShortcutKeyCode::KeyL => keyboard::key::Code::KeyL,
        ShortcutKeyCode::KeyN => keyboard::key::Code::KeyN,
        ShortcutKeyCode::KeyO => keyboard::key::Code::KeyO,
        ShortcutKeyCode::KeyQ => keyboard::key::Code::KeyQ,
        ShortcutKeyCode::KeyS => keyboard::key::Code::KeyS,
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
        ShortcutAction::NewEditor => Some(ShortcutActionId::NewEditor),
        ShortcutAction::OpenEditorFile => Some(ShortcutActionId::OpenEditorFile),
        ShortcutAction::SaveEditor => Some(ShortcutActionId::SaveEditor),
        ShortcutAction::CloseEditorTab => Some(ShortcutActionId::CloseEditorTab),
        ShortcutAction::EditorUndo => Some(ShortcutActionId::EditorUndo),
        ShortcutAction::EditorRedo => Some(ShortcutActionId::EditorRedo),
        ShortcutAction::EditorCopy => Some(ShortcutActionId::EditorCopy),
        ShortcutAction::EditorPaste => Some(ShortcutActionId::EditorPaste),
        ShortcutAction::EditorOpenSearch => Some(ShortcutActionId::EditorOpenSearch),
        ShortcutAction::EditorOpenSearchReplace => Some(ShortcutActionId::EditorOpenSearchReplace),
        ShortcutAction::EditorOpenGotoLine => Some(ShortcutActionId::EditorOpenGotoLine),
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
            modifiers.insert(keyboard::Modifiers::COMMAND);
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
}
