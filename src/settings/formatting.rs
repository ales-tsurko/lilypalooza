use super::*;

pub(super) fn push_documented_value(
    out: &mut String,
    comment: &str,
    key: &str,
    value: &str,
    default: &str,
) {
    out.push_str("# ");
    out.push_str(comment);
    out.push('\n');
    if value == default {
        out.push_str("# ");
    }
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(value);
    out.push_str("\n\n");
}

pub(super) fn format_f32(value: f32) -> String {
    let mut s = format!("{value:.3}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    s
}

pub(crate) fn path() -> Result<PathBuf, String> {
    settings_path()
}

pub(super) fn settings_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("", "", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("settings.toml"))
}

pub(super) fn legacy_settings_path() -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("by", "alestsurko", "lilypalooza")
        .ok_or_else(|| "Failed to resolve user config directory".to_string())?;

    Ok(project_dirs.config_dir().join("settings.toml"))
}

pub(super) fn settings_load_path() -> Result<PathBuf, String> {
    let path = settings_path()?;
    if path.is_file() {
        return Ok(path);
    }

    let legacy = legacy_settings_path()?;
    if legacy.is_file() {
        return Ok(legacy);
    }

    Ok(path)
}

pub(crate) fn shortcut_action_id_key(action_id: ShortcutActionId) -> String {
    let debug = format!("{action_id:?}");
    let mut out = String::with_capacity(debug.len() + 8);

    for (index, ch) in debug.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index != 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }

    out
}

pub(super) fn parse_shortcut_action_id_key(key: &str) -> Option<ShortcutActionId> {
    #[derive(Deserialize)]
    struct ActionIdWrapper {
        action: ShortcutActionId,
    }

    let source = format!("action = {key:?}");
    toml::from_str::<ActionIdWrapper>(&source)
        .ok()
        .map(|wrapper| wrapper.action)
}

pub(crate) fn format_shortcut_binding_override(binding: &ShortcutBindingOverride) -> String {
    match binding {
        ShortcutBindingOverride::Assigned(binding) => format_shortcut_binding(binding),
        ShortcutBindingOverride::Unassigned => "Unassigned".to_string(),
    }
}

pub(super) fn push_path_list(out: &mut String, key: &str, paths: &[PathBuf]) {
    if paths.is_empty() {
        out.push_str(key);
        out.push_str(" = []\n");
        return;
    }

    out.push_str(key);
    out.push_str(" = [\n");
    for path in paths {
        out.push_str("    ");
        out.push_str(&format!("{:?}", path.display().to_string()));
        out.push_str(",\n");
    }
    out.push_str("]\n");
}

pub(crate) fn format_shortcut_binding(binding: &ShortcutBinding) -> String {
    let mut parts = Vec::new();

    if binding.primary {
        parts.push("Cmd");
    }
    if binding.control {
        parts.push("Ctrl");
    }
    if binding.alt {
        parts.push("Alt");
    }
    if binding.shift {
        parts.push("Shift");
    }

    parts.push(match binding.key {
        ShortcutKey::Code(code) => shortcut_key_code_string(code),
        ShortcutKey::Named(named) => shortcut_named_key_string(named),
    });

    parts.join("+")
}

pub(crate) fn parse_shortcut_binding_override(
    value: &str,
) -> Result<ShortcutBindingOverride, String> {
    if value.trim().eq_ignore_ascii_case("unassigned") {
        return Ok(ShortcutBindingOverride::Unassigned);
    }

    parse_shortcut_binding(value).map(ShortcutBindingOverride::Assigned)
}

pub(crate) fn parse_shortcut_binding(value: &str) -> Result<ShortcutBinding, String> {
    let mut binding = ShortcutBinding {
        key: ShortcutKey::Code(ShortcutKeyCode::KeyS),
        primary: false,
        control: false,
        alt: false,
        shift: false,
    };

    let mut tokens: Vec<_> = value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    let key_token = tokens
        .pop()
        .ok_or_else(|| "Shortcut must include a key".to_string())?;

    for token in tokens {
        if token.eq_ignore_ascii_case("cmd")
            || token.eq_ignore_ascii_case("primary")
            || (!cfg!(target_os = "macos") && token.eq_ignore_ascii_case("ctrl"))
        {
            binding.primary = true;
        } else if token.eq_ignore_ascii_case("ctrl") || token.eq_ignore_ascii_case("control") {
            binding.control = true;
        } else if token.eq_ignore_ascii_case("alt") || token.eq_ignore_ascii_case("option") {
            binding.alt = true;
        } else if token.eq_ignore_ascii_case("shift") {
            binding.shift = true;
        } else {
            return Err(format!("Unknown shortcut modifier: {token}"));
        }
    }

    binding.key = parse_shortcut_key(key_token)?;
    Ok(binding)
}

pub(crate) fn parse_shortcut_key(value: &str) -> Result<ShortcutKey, String> {
    parse_shortcut_code_key(value)
        .map(ShortcutKey::Code)
        .or_else(|| parse_shortcut_named_key(value).map(ShortcutKey::Named))
        .ok_or_else(|| format!("Unknown shortcut key: {value}"))
}

pub(crate) fn shortcut_key_code_string(code: ShortcutKeyCode) -> &'static str {
    SHORTCUT_CODE_NAMES
        .iter()
        .find_map(|mapping| (mapping.code == code).then_some(mapping.label))
        .expect("every ShortcutKeyCode should have a label")
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ShortcutCodeName {
    code: ShortcutKeyCode,
    label: &'static str,
}

pub(super) const SHORTCUT_CODE_NAMES: &[ShortcutCodeName] = &[
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyA,
        label: "A",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyC,
        label: "C",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Comma,
        label: ",",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyF,
        label: "F",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyG,
        label: "G",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyH,
        label: "H",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyJ,
        label: "J",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyK,
        label: "K",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyL,
        label: "L",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyN,
        label: "N",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyO,
        label: "O",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyP,
        label: "P",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyQ,
        label: "Q",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyS,
        label: "S",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyX,
        label: "X",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyV,
        label: "V",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyW,
        label: "W",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyY,
        label: "Y",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::KeyZ,
        label: "Z",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Digit0,
        label: "0",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Digit1,
        label: "1",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Digit2,
        label: "2",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Digit3,
        label: "3",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Digit4,
        label: "4",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Slash,
        label: "/",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Backslash,
        label: "\\",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowLeft,
        label: "Left",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowRight,
        label: "Right",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowUp,
        label: "Up",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowDown,
        label: "Down",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Backspace,
        label: "Backspace",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Delete,
        label: "Delete",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Home,
        label: "Home",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::End,
        label: "End",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Insert,
        label: "Insert",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::F3,
        label: "F3",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Equal,
        label: "Plus",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Minus,
        label: "-",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::BracketLeft,
        label: "[",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::BracketRight,
        label: "]",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Numpad0,
        label: "0",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Numpad1,
        label: "1",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Numpad2,
        label: "2",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Numpad3,
        label: "3",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Numpad4,
        label: "4",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::NumpadAdd,
        label: "Plus",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::NumpadSubtract,
        label: "-",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::NumpadEnter,
        label: "Enter",
    },
];

pub(super) const SHORTCUT_CODE_ALIASES: &[ShortcutCodeName] = &[
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowLeft,
        label: "left",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowRight,
        label: "right",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowUp,
        label: "up",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::ArrowDown,
        label: "down",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Backspace,
        label: "backspace",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Delete,
        label: "delete",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Home,
        label: "home",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::End,
        label: "end",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Insert,
        label: "insert",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::F3,
        label: "f3",
    },
    ShortcutCodeName {
        code: ShortcutKeyCode::Equal,
        label: "plus",
    },
];

pub(super) fn parse_shortcut_code_key(value: &str) -> Option<ShortcutKeyCode> {
    parse_single_character_shortcut_code(value)
        .or_else(|| shortcut_code_from_alias(value, SHORTCUT_CODE_ALIASES))
}

pub(super) fn parse_single_character_shortcut_code(value: &str) -> Option<ShortcutKeyCode> {
    let mut chars = value.chars();
    let character = chars.next()?;
    if chars.next().is_some() {
        return None;
    }

    shortcut_code_from_alias(
        &character.to_ascii_uppercase().to_string(),
        SHORTCUT_CODE_NAMES,
    )
}

pub(super) fn shortcut_code_from_alias(
    value: &str,
    aliases: &[ShortcutCodeName],
) -> Option<ShortcutKeyCode> {
    aliases.iter().find_map(|mapping| {
        value
            .eq_ignore_ascii_case(mapping.label)
            .then_some(mapping.code)
    })
}

pub(super) fn parse_shortcut_named_key(value: &str) -> Option<ShortcutNamedKey> {
    if value.eq_ignore_ascii_case("space") {
        Some(ShortcutNamedKey::Space)
    } else if value.eq_ignore_ascii_case("enter") {
        Some(ShortcutNamedKey::Enter)
    } else {
        None
    }
}

pub(super) fn shortcut_named_key_string(named: ShortcutNamedKey) -> &'static str {
    match named {
        ShortcutNamedKey::Space => "Space",
        ShortcutNamedKey::Enter => "Enter",
    }
}
