use iced::keyboard;

use crate::settings::{
    ShortcutActionId, ShortcutBinding, ShortcutBindingOverride, ShortcutKey, ShortcutKeyCode,
    ShortcutNamedKey, ShortcutSettings, WorkspacePane,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutAction {
    SaveEditor,
    ToggleWorkspacePane(WorkspacePane),
    SwitchWorkspaceTabPrevious,
    SwitchWorkspaceTabNext,
    FocusWorkspacePanePrevious,
    FocusWorkspacePaneNext,
    ScoreZoomIn,
    ScoreZoomOut,
    ScoreZoomReset,
    PianoRollZoomIn,
    PianoRollZoomOut,
    PianoRollZoomReset,
    TransportPlayPause,
    TransportRewind,
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

const GLOBAL_ACTIONS: [ShortcutAction; 5] = [
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
        WorkspacePane::Editor | WorkspacePane::Logger => None,
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
        WorkspacePane::PianoRoll | WorkspacePane::Editor | WorkspacePane::Logger => None,
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
        ShortcutAction::SaveEditor => vec![binding_code(ShortcutKeyCode::KeyS, true, false, false)],
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
        ShortcutAction::ScoreZoomIn | ShortcutAction::PianoRollZoomIn => vec![
            binding_code(ShortcutKeyCode::Equal, true, false, false),
            binding_code(ShortcutKeyCode::Equal, true, false, true),
            binding_code(ShortcutKeyCode::NumpadAdd, true, false, false),
        ],
        ShortcutAction::ScoreZoomOut | ShortcutAction::PianoRollZoomOut => vec![
            binding_code(ShortcutKeyCode::Minus, true, false, false),
            binding_code(ShortcutKeyCode::Minus, true, false, true),
            binding_code(ShortcutKeyCode::NumpadSubtract, true, false, false),
        ],
        ShortcutAction::ScoreZoomReset | ShortcutAction::PianoRollZoomReset => vec![
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
        ShortcutAction::ScoreScrollUp
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
        ShortcutKeyCode::KeyS => "S",
        ShortcutKeyCode::Digit1 | ShortcutKeyCode::Numpad1 => "1",
        ShortcutKeyCode::Digit2 | ShortcutKeyCode::Numpad2 => "2",
        ShortcutKeyCode::Digit3 | ShortcutKeyCode::Numpad3 => "3",
        ShortcutKeyCode::Digit4 | ShortcutKeyCode::Numpad4 => "4",
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
        ShortcutKeyCode::KeyS => keyboard::key::Code::KeyS,
        ShortcutKeyCode::Digit1 => keyboard::key::Code::Digit1,
        ShortcutKeyCode::Digit2 => keyboard::key::Code::Digit2,
        ShortcutKeyCode::Digit3 => keyboard::key::Code::Digit3,
        ShortcutKeyCode::Digit4 => keyboard::key::Code::Digit4,
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
        ShortcutAction::SaveEditor => Some(ShortcutActionId::SaveEditor),
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
        ShortcutAction::FocusWorkspacePanePrevious => Some(ShortcutActionId::PreviousPane),
        ShortcutAction::FocusWorkspacePaneNext => Some(ShortcutActionId::NextPane),
        ShortcutAction::ScoreZoomIn => Some(ShortcutActionId::ScoreZoomIn),
        ShortcutAction::ScoreZoomOut => Some(ShortcutActionId::ScoreZoomOut),
        ShortcutAction::ScoreZoomReset => Some(ShortcutActionId::ScoreZoomReset),
        ShortcutAction::PianoRollZoomIn => Some(ShortcutActionId::PianoRollZoomIn),
        ShortcutAction::PianoRollZoomOut => Some(ShortcutActionId::PianoRollZoomOut),
        ShortcutAction::PianoRollZoomReset => Some(ShortcutActionId::PianoRollZoomReset),
        ShortcutAction::TransportPlayPause => Some(ShortcutActionId::TransportPlayPause),
        ShortcutAction::TransportRewind => Some(ShortcutActionId::TransportRewind),
        ShortcutAction::ScoreScrollUp
        | ShortcutAction::ScoreScrollDown
        | ShortcutAction::ScorePrevPage
        | ShortcutAction::ScoreNextPage => None,
    }
}
