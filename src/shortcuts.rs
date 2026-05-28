use iced::keyboard;

use crate::settings::{
    ShortcutActionId,
    ShortcutBinding,
    ShortcutBindingOverride,
    ShortcutKey,
    ShortcutKeyCode,
    ShortcutNamedKey,
    ShortcutSettings,
    WorkspacePane,
};

mod actions;
mod bindings;
mod keymap;

pub(crate) use actions::*;
use bindings::*;
use keymap::*;
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
    fn resolves_mixer_transport_play_pause_binding() {
        let input = named_input(keyboard::key::Named::Space);
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Mixer, input),
            Some(ShortcutAction::TransportPlayPause)
        );
    }

    #[test]
    fn resolves_mixer_transport_rewind_binding() {
        let input = named_input(keyboard::key::Named::Enter);
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Mixer, input),
            Some(ShortcutAction::TransportRewind)
        );
    }

    #[test]
    fn resolves_global_toggle_metronome_binding() {
        let input = code_input(keyboard::key::Code::KeyK, true, false, false);
        assert_eq!(
            resolve_global(&ShortcutSettings::default(), input),
            Some(ShortcutAction::ToggleMetronome)
        );
    }

    #[test]
    fn resolves_mixer_undo_binding() {
        let input = code_input(keyboard::key::Code::KeyZ, true, false, false);
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Mixer, input),
            Some(ShortcutAction::EditorUndo)
        );
    }

    #[test]
    fn resolves_mixer_redo_binding() {
        let input = if cfg!(target_os = "macos") {
            code_input(keyboard::key::Code::KeyZ, true, false, true)
        } else {
            code_input(keyboard::key::Code::KeyY, true, false, false)
        };
        assert_eq!(
            resolve_contextual(&ShortcutSettings::default(), WorkspacePane::Mixer, input),
            Some(ShortcutAction::EditorRedo)
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

    #[test]
    fn all_action_ids_have_actions() {
        for action_id in ALL_ACTION_IDS {
            assert!(
                action_from_id(*action_id).is_some(),
                "missing shortcut action for {action_id:?}"
            );
        }
    }

    #[test]
    fn shortcut_actions_round_trip_to_ids() {
        for (expected_id, action) in ACTION_BY_ID {
            assert_eq!(expected_id, &action_id(*action).unwrap());
        }
    }
}
