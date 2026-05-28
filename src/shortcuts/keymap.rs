use super::*;

pub(super) fn named_label(named: ShortcutNamedKey) -> &'static str {
    match named {
        ShortcutNamedKey::Space => "Space",
        ShortcutNamedKey::Enter => "Enter",
    }
}

pub(super) const fn binding_code(
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

pub(super) const fn binding_named(
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

pub(super) const fn binding_named_ctrl(
    named: ShortcutNamedKey,
    alt: bool,
    shift: bool,
) -> ShortcutBinding {
    ShortcutBinding {
        key: ShortcutKey::Named(named),
        primary: false,
        control: true,
        alt,
        shift,
    }
}

pub(super) fn to_iced_key_code(code: ShortcutKeyCode) -> Option<keyboard::key::Code> {
    KEY_CODE_MAP
        .iter()
        .find_map(|(shortcut, iced)| (*shortcut == code).then_some(*iced))
}

pub(super) const KEY_CODE_MAP: &[(ShortcutKeyCode, keyboard::key::Code)] = &[
    (ShortcutKeyCode::KeyA, keyboard::key::Code::KeyA),
    (ShortcutKeyCode::KeyC, keyboard::key::Code::KeyC),
    (ShortcutKeyCode::Comma, keyboard::key::Code::Comma),
    (ShortcutKeyCode::KeyF, keyboard::key::Code::KeyF),
    (ShortcutKeyCode::KeyG, keyboard::key::Code::KeyG),
    (ShortcutKeyCode::KeyH, keyboard::key::Code::KeyH),
    (ShortcutKeyCode::KeyJ, keyboard::key::Code::KeyJ),
    (ShortcutKeyCode::KeyK, keyboard::key::Code::KeyK),
    (ShortcutKeyCode::KeyL, keyboard::key::Code::KeyL),
    (ShortcutKeyCode::KeyN, keyboard::key::Code::KeyN),
    (ShortcutKeyCode::KeyO, keyboard::key::Code::KeyO),
    (ShortcutKeyCode::KeyP, keyboard::key::Code::KeyP),
    (ShortcutKeyCode::KeyQ, keyboard::key::Code::KeyQ),
    (ShortcutKeyCode::KeyS, keyboard::key::Code::KeyS),
    (ShortcutKeyCode::KeyX, keyboard::key::Code::KeyX),
    (ShortcutKeyCode::KeyV, keyboard::key::Code::KeyV),
    (ShortcutKeyCode::KeyW, keyboard::key::Code::KeyW),
    (ShortcutKeyCode::KeyY, keyboard::key::Code::KeyY),
    (ShortcutKeyCode::KeyZ, keyboard::key::Code::KeyZ),
    (ShortcutKeyCode::Digit1, keyboard::key::Code::Digit1),
    (ShortcutKeyCode::Digit2, keyboard::key::Code::Digit2),
    (ShortcutKeyCode::Digit3, keyboard::key::Code::Digit3),
    (ShortcutKeyCode::Digit4, keyboard::key::Code::Digit4),
    (ShortcutKeyCode::Slash, keyboard::key::Code::Slash),
    (ShortcutKeyCode::Backslash, keyboard::key::Code::Backslash),
    (ShortcutKeyCode::ArrowLeft, keyboard::key::Code::ArrowLeft),
    (ShortcutKeyCode::ArrowRight, keyboard::key::Code::ArrowRight),
    (ShortcutKeyCode::ArrowUp, keyboard::key::Code::ArrowUp),
    (ShortcutKeyCode::ArrowDown, keyboard::key::Code::ArrowDown),
    (ShortcutKeyCode::Backspace, keyboard::key::Code::Backspace),
    (ShortcutKeyCode::Delete, keyboard::key::Code::Delete),
    (ShortcutKeyCode::Home, keyboard::key::Code::Home),
    (ShortcutKeyCode::End, keyboard::key::Code::End),
    (ShortcutKeyCode::Insert, keyboard::key::Code::Insert),
    (ShortcutKeyCode::F3, keyboard::key::Code::F3),
    (ShortcutKeyCode::Numpad1, keyboard::key::Code::Numpad1),
    (ShortcutKeyCode::Numpad2, keyboard::key::Code::Numpad2),
    (ShortcutKeyCode::Numpad3, keyboard::key::Code::Numpad3),
    (ShortcutKeyCode::Numpad4, keyboard::key::Code::Numpad4),
    (ShortcutKeyCode::Equal, keyboard::key::Code::Equal),
    (ShortcutKeyCode::Minus, keyboard::key::Code::Minus),
    (ShortcutKeyCode::Digit0, keyboard::key::Code::Digit0),
    (ShortcutKeyCode::NumpadAdd, keyboard::key::Code::NumpadAdd),
    (
        ShortcutKeyCode::NumpadSubtract,
        keyboard::key::Code::NumpadSubtract,
    ),
    (ShortcutKeyCode::Numpad0, keyboard::key::Code::Numpad0),
    (
        ShortcutKeyCode::BracketLeft,
        keyboard::key::Code::BracketLeft,
    ),
    (
        ShortcutKeyCode::BracketRight,
        keyboard::key::Code::BracketRight,
    ),
    (
        ShortcutKeyCode::NumpadEnter,
        keyboard::key::Code::NumpadEnter,
    ),
];

pub(super) fn to_iced_named_key(named: ShortcutNamedKey) -> keyboard::key::Named {
    match named {
        ShortcutNamedKey::Space => keyboard::key::Named::Space,
        ShortcutNamedKey::Enter => keyboard::key::Named::Enter,
    }
}

pub(super) fn action_id(action: ShortcutAction) -> Option<ShortcutActionId> {
    ACTION_BY_ID
        .iter()
        .find_map(|(id, candidate)| (*candidate == action).then_some(*id))
}
