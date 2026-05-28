use baseview::{MouseButton, ScrollDelta};
use egui::Vec2;
use keyboard_types::{Key, KeyState, KeyboardEvent, Modifiers};

use super::*;

pub(crate) fn keyboard_event_has_text_input(
    event: &KeyboardEvent,
    modifiers: egui::Modifiers,
    pressed: bool,
) -> bool {
    pressed
        && !event.is_composing
        && !modifiers.command
        && !modifiers.ctrl
        && matches!(&event.key, Key::Character(text) if is_text_input(text))
}

pub(crate) fn egui_modifiers(modifiers: Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.contains(Modifiers::ALT),
        ctrl: modifiers.contains(Modifiers::CONTROL),
        shift: modifiers.contains(Modifiers::SHIFT),
        mac_cmd: modifiers.contains(Modifiers::META),
        command: if cfg!(target_os = "macos") {
            modifiers.contains(Modifiers::META)
        } else {
            modifiers.contains(Modifiers::CONTROL)
        },
    }
}

pub(crate) fn is_command_quit(event: &KeyboardEvent) -> bool {
    event.state == KeyState::Down
        && event.modifiers.contains(Modifiers::META)
        && matches!(&event.key, Key::Character(value) if value.eq_ignore_ascii_case("q"))
}

pub(crate) fn is_text_input(text: &str) -> bool {
    !text.chars().any(char::is_control)
}

pub(crate) fn egui_key(key: &Key) -> Option<egui::Key> {
    egui_named_key(key).or_else(|| egui_character_key(key))
}

pub(crate) fn egui_named_key(key: &Key) -> Option<egui::Key> {
    [
        (Key::ArrowDown, egui::Key::ArrowDown),
        (Key::ArrowLeft, egui::Key::ArrowLeft),
        (Key::ArrowRight, egui::Key::ArrowRight),
        (Key::ArrowUp, egui::Key::ArrowUp),
        (Key::Escape, egui::Key::Escape),
        (Key::Tab, egui::Key::Tab),
        (Key::Backspace, egui::Key::Backspace),
        (Key::Enter, egui::Key::Enter),
        (Key::Delete, egui::Key::Delete),
        (Key::Home, egui::Key::Home),
        (Key::End, egui::Key::End),
        (Key::PageUp, egui::Key::PageUp),
        (Key::PageDown, egui::Key::PageDown),
    ]
    .into_iter()
    .find_map(|(candidate, mapped)| (&candidate == key).then_some(mapped))
}

pub(crate) fn egui_character_key(key: &Key) -> Option<egui::Key> {
    let Key::Character(value) = key else {
        return None;
    };
    let [byte] = value.as_bytes() else {
        return None;
    };

    egui_digit_key(*byte).or_else(|| egui_action_key(*byte))
}

pub(crate) fn egui_digit_key(byte: u8) -> Option<egui::Key> {
    const DIGITS: [egui::Key; 10] = [
        egui::Key::Num0,
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
        egui::Key::Num6,
        egui::Key::Num7,
        egui::Key::Num8,
        egui::Key::Num9,
    ];

    DIGITS.get(usize::from(byte.checked_sub(b'0')?)).copied()
}

pub(crate) fn egui_action_key(byte: u8) -> Option<egui::Key> {
    [
        (b'a', egui::Key::A),
        (b'c', egui::Key::C),
        (b'v', egui::Key::V),
        (b'x', egui::Key::X),
        (b'z', egui::Key::Z),
    ]
    .into_iter()
    .find_map(|(candidate, mapped)| byte.eq_ignore_ascii_case(&candidate).then_some(mapped))
}

pub(crate) fn pointer_button(button: MouseButton) -> Option<egui::PointerButton> {
    [
        (MouseButton::Left, egui::PointerButton::Primary),
        (MouseButton::Right, egui::PointerButton::Secondary),
        (MouseButton::Middle, egui::PointerButton::Middle),
        (MouseButton::Back, egui::PointerButton::Extra1),
        (MouseButton::Forward, egui::PointerButton::Extra2),
    ]
    .into_iter()
    .find_map(|(candidate, mapped)| (button == candidate).then_some(mapped))
}

pub(crate) fn egui_scroll_delta(delta: ScrollDelta) -> Vec2 {
    match delta {
        ScrollDelta::Lines { x, y } => Vec2::new(x, y) * 24.0,
        ScrollDelta::Pixels { x, y } => Vec2::new(x, y),
    }
}
