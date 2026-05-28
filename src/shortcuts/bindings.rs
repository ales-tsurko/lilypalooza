use super::*;

pub(super) fn action_from_id(action_id: ShortcutActionId) -> Option<ShortcutAction> {
    ACTION_BY_ID
        .iter()
        .find_map(|(id, action)| (*id == action_id).then_some(*action))
}

pub(super) fn effective_bindings(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Vec<ShortcutBinding> {
    match binding_override(settings, action) {
        Some(ShortcutBindingOverride::Assigned(binding)) => vec![binding],
        Some(ShortcutBindingOverride::Unassigned) => Vec::new(),
        None => default_bindings(action),
    }
}

pub(super) fn display_binding_for_action(
    settings: &ShortcutSettings,
    action: ShortcutAction,
) -> Option<ShortcutBinding> {
    match binding_override(settings, action) {
        Some(ShortcutBindingOverride::Assigned(binding)) => Some(binding),
        Some(ShortcutBindingOverride::Unassigned) => None,
        None => default_bindings(action).into_iter().next(),
    }
}

pub(super) fn binding_override(
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

pub(super) fn default_bindings(action: ShortcutAction) -> Vec<ShortcutBinding> {
    DEFAULT_BINDINGS
        .iter()
        .find_map(|entry| (entry.action == action).then(|| entry.bindings.to_vec()))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy)]
struct ShortcutDefault {
    action: ShortcutAction,
    bindings: &'static [ShortcutBinding],
}

const fn code(key: ShortcutKeyCode, primary: bool, alt: bool, shift: bool) -> ShortcutBinding {
    binding_code(key, primary, alt, shift)
}

const fn named(key: ShortcutNamedKey, primary: bool, alt: bool, shift: bool) -> ShortcutBinding {
    binding_named(key, primary, alt, shift)
}

const fn named_ctrl(key: ShortcutNamedKey, alt: bool, shift: bool) -> ShortcutBinding {
    binding_named_ctrl(key, alt, shift)
}

const ENTER_BINDINGS: [ShortcutBinding; 2] = [
    named(ShortcutNamedKey::Enter, false, false, false),
    code(ShortcutKeyCode::NumpadEnter, false, false, false),
];
const REDO_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::KeyZ, true, false, true)]
} else {
    &[
        code(ShortcutKeyCode::KeyY, true, false, false),
        code(ShortcutKeyCode::KeyZ, true, false, true),
    ]
};
const FILE_BROWSER_DELETE_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[
        code(ShortcutKeyCode::Backspace, true, false, false),
        code(ShortcutKeyCode::Delete, true, false, false),
    ]
} else {
    &[code(ShortcutKeyCode::Delete, false, false, false)]
};
const EDITOR_COPY_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::KeyC, true, false, false)]
} else {
    &[
        code(ShortcutKeyCode::KeyC, true, false, false),
        code(ShortcutKeyCode::Insert, true, false, false),
    ]
};
const EDITOR_PASTE_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::KeyV, true, false, false)]
} else {
    &[
        code(ShortcutKeyCode::KeyV, true, false, false),
        code(ShortcutKeyCode::Insert, false, false, true),
    ]
};
const SEARCH_REPLACE_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::KeyF, true, true, false)]
} else {
    &[code(ShortcutKeyCode::KeyH, true, false, false)]
};
const MAC_EMPTY_BINDINGS: [ShortcutBinding; 0] = [];
const WORD_PRIMARY: bool = !cfg!(target_os = "macos");
const WORD_ALT: bool = cfg!(target_os = "macos");
const MAC_DELETE_TO_LINE_START: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::Backspace, true, false, false)]
} else {
    &MAC_EMPTY_BINDINGS
};
const MAC_DELETE_TO_LINE_END: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::Delete, true, false, false)]
} else {
    &MAC_EMPTY_BINDINGS
};
const MAC_LINE_START: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowLeft, true, false, false)]
} else {
    &MAC_EMPTY_BINDINGS
};
const MAC_LINE_END: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowRight, true, false, false)]
} else {
    &MAC_EMPTY_BINDINGS
};
const MAC_LINE_START_SELECT: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowLeft, true, false, true)]
} else {
    &MAC_EMPTY_BINDINGS
};
const MAC_LINE_END_SELECT: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowRight, true, false, true)]
} else {
    &MAC_EMPTY_BINDINGS
};
const DOCUMENT_START_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowUp, true, false, false)]
} else {
    &[code(ShortcutKeyCode::Home, true, false, false)]
};
const DOCUMENT_END_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowDown, true, false, false)]
} else {
    &[code(ShortcutKeyCode::End, true, false, false)]
};
const DOCUMENT_START_SELECT_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowUp, true, false, true)]
} else {
    &[code(ShortcutKeyCode::Home, true, false, true)]
};
const DOCUMENT_END_SELECT_BINDINGS: &[ShortcutBinding] = if cfg!(target_os = "macos") {
    &[code(ShortcutKeyCode::ArrowDown, true, false, true)]
} else {
    &[code(ShortcutKeyCode::End, true, false, true)]
};

const DEFAULT_BINDINGS: &[ShortcutDefault] = &[
    ShortcutDefault {
        action: ShortcutAction::QuitApp,
        bindings: &[code(ShortcutKeyCode::KeyQ, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::OpenActions,
        bindings: &[code(ShortcutKeyCode::KeyP, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::OpenSettingsFile,
        bindings: &[code(ShortcutKeyCode::Comma, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::SaveEditor,
        bindings: &[code(ShortcutKeyCode::KeyS, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserUndo,
        bindings: &[code(ShortcutKeyCode::KeyZ, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserRedo,
        bindings: REDO_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserCut,
        bindings: &[code(ShortcutKeyCode::KeyX, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserCopy,
        bindings: &[code(ShortcutKeyCode::KeyC, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserPaste,
        bindings: &[code(ShortcutKeyCode::KeyV, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserRename,
        bindings: &ENTER_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::FileBrowserDelete,
        bindings: FILE_BROWSER_DELETE_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::NewEditor,
        bindings: &[code(ShortcutKeyCode::KeyN, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::OpenEditorFile,
        bindings: &[code(ShortcutKeyCode::KeyO, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::CloseEditorTab,
        bindings: &[code(ShortcutKeyCode::KeyW, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorUndo,
        bindings: &[code(ShortcutKeyCode::KeyZ, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorRedo,
        bindings: REDO_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorCopy,
        bindings: EDITOR_COPY_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorPaste,
        bindings: EDITOR_PASTE_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorOpenSearch,
        bindings: &[code(ShortcutKeyCode::KeyF, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorOpenSearchReplace,
        bindings: SEARCH_REPLACE_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorOpenGotoLine,
        bindings: &[code(ShortcutKeyCode::KeyG, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorTriggerCompletion,
        bindings: &[named_ctrl(ShortcutNamedKey::Space, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorFindNext,
        bindings: &[code(ShortcutKeyCode::F3, false, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorFindPrevious,
        bindings: &[code(ShortcutKeyCode::F3, false, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorWordLeft,
        bindings: &[code(
            ShortcutKeyCode::ArrowLeft,
            WORD_PRIMARY,
            WORD_ALT,
            false,
        )],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorWordRight,
        bindings: &[code(
            ShortcutKeyCode::ArrowRight,
            WORD_PRIMARY,
            WORD_ALT,
            false,
        )],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorWordLeftSelect,
        bindings: &[code(
            ShortcutKeyCode::ArrowLeft,
            WORD_PRIMARY,
            WORD_ALT,
            true,
        )],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorWordRightSelect,
        bindings: &[code(
            ShortcutKeyCode::ArrowRight,
            WORD_PRIMARY,
            WORD_ALT,
            true,
        )],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteWordBackward,
        bindings: &[code(
            ShortcutKeyCode::Backspace,
            WORD_PRIMARY,
            WORD_ALT,
            false,
        )],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteWordForward,
        bindings: &[code(ShortcutKeyCode::Delete, WORD_PRIMARY, WORD_ALT, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteToLineStart,
        bindings: MAC_DELETE_TO_LINE_START,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteToLineEnd,
        bindings: MAC_DELETE_TO_LINE_END,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorLineStart,
        bindings: MAC_LINE_START,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorLineEnd,
        bindings: MAC_LINE_END,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorLineStartSelect,
        bindings: MAC_LINE_START_SELECT,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorLineEndSelect,
        bindings: MAC_LINE_END_SELECT,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDocumentStart,
        bindings: DOCUMENT_START_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDocumentEnd,
        bindings: DOCUMENT_END_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDocumentStartSelect,
        bindings: DOCUMENT_START_SELECT_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDocumentEndSelect,
        bindings: DOCUMENT_END_SELECT_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteSelection,
        bindings: &[code(ShortcutKeyCode::Delete, false, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorSelectAll,
        bindings: &[code(ShortcutKeyCode::KeyA, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorInsertLineBelow,
        bindings: &[named(ShortcutNamedKey::Enter, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorInsertLineAbove,
        bindings: &[named(ShortcutNamedKey::Enter, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorDeleteLine,
        bindings: &[code(ShortcutKeyCode::KeyK, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorMoveLineUp,
        bindings: &[code(ShortcutKeyCode::ArrowUp, false, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorMoveLineDown,
        bindings: &[code(ShortcutKeyCode::ArrowDown, false, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorCopyLineUp,
        bindings: &[code(ShortcutKeyCode::ArrowUp, false, true, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorCopyLineDown,
        bindings: &[code(ShortcutKeyCode::ArrowDown, false, true, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorJoinLines,
        bindings: &[code(ShortcutKeyCode::KeyJ, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorIndent,
        bindings: &[code(ShortcutKeyCode::BracketRight, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorOutdent,
        bindings: &[code(ShortcutKeyCode::BracketLeft, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorToggleLineComment,
        bindings: &[code(ShortcutKeyCode::Slash, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorToggleBlockComment,
        bindings: &[code(ShortcutKeyCode::KeyA, false, true, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorSelectLine,
        bindings: &[code(ShortcutKeyCode::KeyL, true, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::EditorJumpToMatchingBracket,
        bindings: &[code(ShortcutKeyCode::Backslash, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleWorkspacePane(WorkspacePane::Editor),
        bindings: &[
            code(ShortcutKeyCode::Digit1, true, false, false),
            code(ShortcutKeyCode::Numpad1, true, false, false),
        ],
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleWorkspacePane(WorkspacePane::Score),
        bindings: &[
            code(ShortcutKeyCode::Digit2, true, false, false),
            code(ShortcutKeyCode::Numpad2, true, false, false),
        ],
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleWorkspacePane(WorkspacePane::PianoRoll),
        bindings: &[
            code(ShortcutKeyCode::Digit3, true, false, false),
            code(ShortcutKeyCode::Numpad3, true, false, false),
        ],
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleWorkspacePane(WorkspacePane::Mixer),
        bindings: &[
            code(ShortcutKeyCode::Digit4, true, false, false),
            code(ShortcutKeyCode::Numpad4, true, false, false),
        ],
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleWorkspacePane(WorkspacePane::Logger),
        bindings: &[
            code(ShortcutKeyCode::Digit0, true, false, false),
            code(ShortcutKeyCode::Numpad0, true, false, false),
        ],
    },
    ShortcutDefault {
        action: ShortcutAction::SwitchWorkspaceTabPrevious,
        bindings: &[code(ShortcutKeyCode::BracketLeft, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::SwitchWorkspaceTabNext,
        bindings: &[code(ShortcutKeyCode::BracketRight, true, false, true)],
    },
    ShortcutDefault {
        action: ShortcutAction::SwitchEditorTabPrevious,
        bindings: &[code(ShortcutKeyCode::ArrowLeft, true, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::SwitchEditorTabNext,
        bindings: &[code(ShortcutKeyCode::ArrowRight, true, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FocusWorkspacePanePrevious,
        bindings: &[code(ShortcutKeyCode::BracketLeft, true, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::FocusWorkspacePaneNext,
        bindings: &[code(ShortcutKeyCode::BracketRight, true, true, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::ScoreZoomIn,
        bindings: ZOOM_IN_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorZoomIn,
        bindings: ZOOM_IN_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::PianoRollZoomIn,
        bindings: ZOOM_IN_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::ScoreZoomOut,
        bindings: ZOOM_OUT_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorZoomOut,
        bindings: ZOOM_OUT_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::PianoRollZoomOut,
        bindings: ZOOM_OUT_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::ScoreZoomReset,
        bindings: ZOOM_RESET_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::EditorZoomReset,
        bindings: ZOOM_RESET_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::PianoRollZoomReset,
        bindings: ZOOM_RESET_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::TransportPlayPause,
        bindings: &[named(ShortcutNamedKey::Space, false, false, false)],
    },
    ShortcutDefault {
        action: ShortcutAction::TransportRewind,
        bindings: &ENTER_BINDINGS,
    },
    ShortcutDefault {
        action: ShortcutAction::ToggleMetronome,
        bindings: &[code(ShortcutKeyCode::KeyK, true, false, false)],
    },
];

const ZOOM_IN_BINDINGS: &[ShortcutBinding] = &[
    code(ShortcutKeyCode::Equal, true, false, false),
    code(ShortcutKeyCode::Equal, true, false, true),
    code(ShortcutKeyCode::NumpadAdd, true, false, false),
];
const ZOOM_OUT_BINDINGS: &[ShortcutBinding] = &[
    code(ShortcutKeyCode::Minus, true, false, false),
    code(ShortcutKeyCode::Minus, true, false, true),
    code(ShortcutKeyCode::NumpadSubtract, true, false, false),
];
const ZOOM_RESET_BINDINGS: &[ShortcutBinding] = &[
    code(ShortcutKeyCode::Digit0, true, false, false),
    code(ShortcutKeyCode::Numpad0, true, false, false),
];

pub(super) fn binding_matches(binding: ShortcutBinding, input: ShortcutInput<'_>) -> bool {
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
        ShortcutKey::Code(code) => to_iced_key_code(code)
            .is_some_and(|iced| input.physical_key == keyboard::key::Physical::Code(iced)),
        ShortcutKey::Named(named) => {
            input.key.as_ref() == keyboard::Key::Named(to_iced_named_key(named))
        }
    }
}

pub(super) fn format_binding(binding: ShortcutBinding) -> String {
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

pub(super) fn platform_primary_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Cmd"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Ctrl"
    }
}

pub(super) fn platform_alt_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Alt"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Alt"
    }
}

pub(super) fn code_label(code: ShortcutKeyCode) -> &'static str {
    SHORTCUT_CODE_LABELS
        .iter()
        .find_map(|mapping| (mapping.code == code).then_some(mapping.label))
        .unwrap_or("Unknown")
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ShortcutCodeLabel {
    code: ShortcutKeyCode,
    label: &'static str,
}

pub(super) const SHORTCUT_CODE_LABELS: &[ShortcutCodeLabel] = &[
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyA,
        label: "A",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyC,
        label: "C",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Comma,
        label: ",",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyF,
        label: "F",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyG,
        label: "G",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyH,
        label: "H",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyJ,
        label: "J",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyK,
        label: "K",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyL,
        label: "L",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyN,
        label: "N",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyO,
        label: "O",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyP,
        label: "P",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyQ,
        label: "Q",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyS,
        label: "S",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyX,
        label: "X",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyV,
        label: "V",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyW,
        label: "W",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyY,
        label: "Y",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::KeyZ,
        label: "Z",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Digit1,
        label: "1",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Numpad1,
        label: "1",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Digit2,
        label: "2",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Numpad2,
        label: "2",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Digit3,
        label: "3",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Numpad3,
        label: "3",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Digit4,
        label: "4",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Numpad4,
        label: "4",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Slash,
        label: "/",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Backslash,
        label: "\\",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::ArrowLeft,
        label: "Left",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::ArrowRight,
        label: "Right",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::ArrowUp,
        label: "Up",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::ArrowDown,
        label: "Down",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Backspace,
        label: "Backspace",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Delete,
        label: "Delete",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Home,
        label: "Home",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::End,
        label: "End",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Insert,
        label: "Insert",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::F3,
        label: "F3",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Equal,
        label: "+",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::NumpadAdd,
        label: "+",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Minus,
        label: "-",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::NumpadSubtract,
        label: "-",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Digit0,
        label: "0",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::Numpad0,
        label: "0",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::BracketLeft,
        label: "[",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::BracketRight,
        label: "]",
    },
    ShortcutCodeLabel {
        code: ShortcutKeyCode::NumpadEnter,
        label: "Enter",
    },
];
