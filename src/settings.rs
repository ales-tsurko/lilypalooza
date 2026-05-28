use std::{fs, io, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};

mod formatting;
mod model;

pub(crate) use formatting::{path, shortcut_action_id_key};
pub(crate) use model::*;
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        AppSettings,
        PlaybackSettings,
        PluginFormat,
        PluginSearchPath,
        ShortcutBinding,
        ShortcutKey,
        ShortcutKeyCode,
        default_plugin_search_paths,
        formatting::{
            format_shortcut_binding,
            parse_shortcut_binding,
            parse_shortcut_key,
            shortcut_key_code_string,
        },
        render_settings_file,
    };

    #[test]
    fn settings_template_contains_playback_section() {
        let contents =
            render_settings_file(&AppSettings::default()).expect("default settings should render");

        assert!(contents.contains("[playback]"));
        assert!(contents.contains("# soundfonts = [\"/absolute/path/to/file.sf2\"]"));
        assert!(contents.contains("# device = \"default\""));
        assert!(contents.contains("# sample_rate = 48000"));
        assert!(contents.contains("# block_size = 64"));
        assert!(contents.contains("# chase_notes_on_seek = false"));
    }

    #[test]
    fn settings_roundtrip_parses_playback_settings() {
        let settings = AppSettings {
            playback: PlaybackSettings {
                soundfonts: vec![
                    PathBuf::from("/tmp/test.sf2"),
                    PathBuf::from("/tmp/other.sf2"),
                ],
                device: Some("Built-in Output".into()),
                sample_rate: Some(48_000),
                block_size: Some(128),
                chase_notes_on_seek: true,
            },
            ..AppSettings::default()
        };

        let contents = render_settings_file(&settings)
            .expect("settings with playback soundfont should render");
        let parsed: AppSettings =
            toml::from_str(&contents).expect("rendered settings should parse back");

        assert_eq!(
            parsed.playback.soundfonts,
            vec![
                PathBuf::from("/tmp/test.sf2"),
                PathBuf::from("/tmp/other.sf2")
            ]
        );
        assert_eq!(parsed.playback.device.as_deref(), Some("Built-in Output"));
        assert_eq!(parsed.playback.sample_rate, Some(48_000));
        assert_eq!(parsed.playback.block_size, Some(128));
        assert!(parsed.playback.chase_notes_on_seek);
    }

    #[test]
    fn default_plugin_search_paths_include_all_platform_formats() {
        let paths = default_plugin_search_paths();

        assert!(paths.iter().any(|path| path.format == PluginFormat::Clap));
        assert!(paths.iter().any(|path| path.format == PluginFormat::Vst3));
        assert!(paths.iter().all(|path| path.enabled));
        assert!(paths.iter().any(|path| {
            path.path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("clap")
        }));
        assert!(paths.iter().any(|path| {
            path.path
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("vst3")
        }));
    }

    #[test]
    fn default_settings_file_persists_active_plugin_search_paths() {
        let contents =
            render_settings_file(&AppSettings::default()).expect("default settings should render");
        let parsed: AppSettings =
            toml::from_str(&contents).expect("rendered default settings should parse back");

        assert!(contents.contains("clap_search_paths = ["));
        assert!(contents.contains("vst3_search_paths = ["));
        assert!(!contents.contains("[[plugin_search_paths]]"));
        assert_eq!(parsed.plugin_search_paths(), default_plugin_search_paths());
    }

    #[test]
    fn settings_roundtrip_parses_plugin_search_path_lists() {
        let settings = AppSettings {
            clap_search_paths: vec![PathBuf::from("/plugins/clap")],
            vst3_search_paths: vec![PathBuf::from("/plugins/vst3")],
            ..AppSettings::default()
        };

        let contents =
            render_settings_file(&settings).expect("settings with plugin paths should render");
        let parsed: AppSettings =
            toml::from_str(&contents).expect("rendered settings should parse back");

        assert_eq!(parsed.clap_search_paths, settings.clap_search_paths);
        assert_eq!(parsed.vst3_search_paths, settings.vst3_search_paths);
        assert_eq!(
            parsed.plugin_search_paths(),
            vec![
                PluginSearchPath {
                    format: PluginFormat::Clap,
                    path: PathBuf::from("/plugins/clap"),
                    enabled: true,
                },
                PluginSearchPath {
                    format: PluginFormat::Vst3,
                    path: PathBuf::from("/plugins/vst3"),
                    enabled: true,
                },
            ]
        );
    }

    #[test]
    fn shortcut_key_formatting_roundtrips_supported_code_keys() {
        for code in [
            ShortcutKeyCode::KeyA,
            ShortcutKeyCode::KeyC,
            ShortcutKeyCode::Comma,
            ShortcutKeyCode::KeyF,
            ShortcutKeyCode::KeyG,
            ShortcutKeyCode::KeyH,
            ShortcutKeyCode::KeyJ,
            ShortcutKeyCode::KeyK,
            ShortcutKeyCode::KeyL,
            ShortcutKeyCode::KeyN,
            ShortcutKeyCode::KeyO,
            ShortcutKeyCode::KeyP,
            ShortcutKeyCode::KeyQ,
            ShortcutKeyCode::KeyS,
            ShortcutKeyCode::KeyX,
            ShortcutKeyCode::KeyV,
            ShortcutKeyCode::KeyW,
            ShortcutKeyCode::KeyY,
            ShortcutKeyCode::KeyZ,
            ShortcutKeyCode::Digit0,
            ShortcutKeyCode::Digit1,
            ShortcutKeyCode::Digit2,
            ShortcutKeyCode::Digit3,
            ShortcutKeyCode::Digit4,
            ShortcutKeyCode::Slash,
            ShortcutKeyCode::Backslash,
            ShortcutKeyCode::ArrowLeft,
            ShortcutKeyCode::ArrowRight,
            ShortcutKeyCode::ArrowUp,
            ShortcutKeyCode::ArrowDown,
            ShortcutKeyCode::Backspace,
            ShortcutKeyCode::Delete,
            ShortcutKeyCode::Home,
            ShortcutKeyCode::End,
            ShortcutKeyCode::Insert,
            ShortcutKeyCode::F3,
            ShortcutKeyCode::Equal,
            ShortcutKeyCode::Minus,
            ShortcutKeyCode::BracketLeft,
            ShortcutKeyCode::BracketRight,
        ] {
            let label = shortcut_key_code_string(code);
            assert_eq!(
                parse_shortcut_key(label).expect("formatted shortcut key should parse"),
                ShortcutKey::Code(code),
                "{label}"
            );
        }
    }

    #[test]
    fn shortcut_binding_roundtrip_preserves_modifiers_and_key() {
        let binding = ShortcutBinding {
            key: ShortcutKey::Code(ShortcutKeyCode::ArrowLeft),
            primary: true,
            control: true,
            alt: true,
            shift: true,
        };

        let formatted = format_shortcut_binding(&binding);
        let parsed = parse_shortcut_binding(&formatted).expect("binding should parse");

        assert_eq!(parsed, binding);
    }
}
