use std::fs;

use iced::{Point, event, keyboard};
use iced_test::simulator;
use tempfile::TempDir;

use super::*;
use crate::shortcuts;

fn test_app() -> Lilypalooza {
    let (mut app, _task) = new_with_default_test_state();
    let _discarded = update(
        &mut app,
        Message::Shortcuts(messages::ShortcutsMessage::OpenDialog),
    );
    app
}

fn test_editor_app() -> Lilypalooza {
    let (app, _task) = new_with_default_test_state();
    app
}

fn apply_messages(app: &mut Lilypalooza, messages: Vec<Message>) {
    for message in messages {
        let _discarded = update(app, message);
    }
}

fn hide_workspace_pane(app: &mut Lilypalooza, pane: WorkspacePaneKind) {
    for group in app.dock_groups.values_mut() {
        group.tabs.retain(|candidate| *candidate != pane);
        if group.active == pane && !group.tabs.is_empty() {
            group.active = group.tabs[0];
        }
    }
}

fn set_workspace_to_score_only(app: &mut Lilypalooza) {
    app.dock_groups.clear();
    app.dock_groups.insert(
        1,
        DockGroup {
            tabs: vec![WorkspacePaneKind::Score],
            active: WorkspacePaneKind::Score,
        },
    );
    app.next_dock_group_id = 2;
    app.dock_layout = Some(DockNode::Group(1));
    app.folded_panes.clear();
    app.rebuild_workspace_panes();
}

fn named_key_press(named: keyboard::key::Named, code: keyboard::key::Code) -> Message {
    Message::KeyPressed(KeyPress {
        status: event::Status::Ignored,
        key: keyboard::Key::Named(named),
        physical_key: keyboard::key::Physical::Code(code),
        modifiers: keyboard::Modifiers::default(),
    })
}

#[test]
fn processor_preset_library_saves_user_preset_for_processor_kind() {
    let mut library = crate::app::processor_presets::ProcessorPresetLibrary::default();
    let kind = lilypalooza_audio::ProcessorKind::BuiltIn {
        processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
    };
    let state = lilypalooza_audio::ProcessorState(vec![1, 2, 3]);

    let id = library.save_user_preset("Warm Piano", kind.clone(), state.clone());

    let presets = library.presets_for(&kind);
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].id, id);
    assert_eq!(presets[0].name, "Warm Piano");
    assert_eq!(library.state_for(&kind, &id), Some(&state));
}

#[test]
fn processor_preset_library_renames_user_preset() {
    let mut library = crate::app::processor_presets::ProcessorPresetLibrary::default();
    let kind = lilypalooza_audio::ProcessorKind::BuiltIn {
        processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
    };
    let id = library.save_user_preset(
        "Warm Piano",
        kind.clone(),
        lilypalooza_audio::ProcessorState(vec![]),
    );

    assert!(library.rename_user_preset(&kind, &id, "Soft Piano"));

    assert_eq!(library.presets_for(&kind)[0].name, "Soft Piano");
}

#[test]
fn processor_preset_library_deletes_user_preset() {
    let mut library = crate::app::processor_presets::ProcessorPresetLibrary::default();
    let kind = lilypalooza_audio::ProcessorKind::BuiltIn {
        processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
    };
    let id = library.save_user_preset(
        "Warm Piano",
        kind.clone(),
        lilypalooza_audio::ProcessorState(vec![]),
    );

    assert!(library.delete_user_preset(&kind, &id));

    assert!(library.presets_for(&kind).is_empty());
}

#[test]
fn project_state_roundtrips_processor_presets() {
    let mut state = ProjectState::default();
    let kind = lilypalooza_audio::ProcessorKind::BuiltIn {
        processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
    };
    let preset_state = lilypalooza_audio::ProcessorState(vec![7, 8, 9]);
    state
        .processor_presets
        .save_user_preset("Saved", kind.clone(), preset_state.clone());

    let serialized = ron::to_string(&state).expect("state should serialize");
    let parsed: ProjectState = ron::from_str(&serialized).expect("state should parse");

    assert_eq!(
        parsed.processor_presets.presets_for(&kind)[0].state,
        preset_state
    );
}

#[test]
fn startup_restores_global_processor_presets() {
    let kind = lilypalooza_audio::ProcessorKind::BuiltIn {
        processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
    };
    let mut stored_state = GlobalState::default();
    stored_state.processor_presets.save_user_preset(
        "Saved",
        kind.clone(),
        lilypalooza_audio::ProcessorState(vec![4, 5, 6]),
    );

    let (app, _) = new_with_loaded_state(
        None,
        None,
        false,
        settings::AppSettings::default(),
        None,
        stored_state,
        None,
    );

    assert_eq!(app.processor_presets.presets_for(&kind)[0].name, "Saved");
}

fn char_key_press(value: &str, code: keyboard::key::Code) -> Message {
    Message::KeyPressed(KeyPress {
        status: event::Status::Ignored,
        key: keyboard::Key::Character(value.into()),
        physical_key: keyboard::key::Physical::Code(code),
        modifiers: keyboard::Modifiers::default(),
    })
}

fn primary_char_key_press(value: &str, code: keyboard::key::Code) -> Message {
    let mut modifiers = keyboard::Modifiers::default();
    if cfg!(target_os = "macos") {
        modifiers.insert(keyboard::Modifiers::COMMAND);
    } else {
        modifiers.insert(keyboard::Modifiers::CTRL);
    }
    Message::KeyPressed(KeyPress {
        status: event::Status::Ignored,
        key: keyboard::Key::Character(value.into()),
        physical_key: keyboard::key::Physical::Code(code),
        modifiers,
    })
}

fn active_browser_column_index(app: &Lilypalooza) -> Option<usize> {
    Some(app.editor.file_browser_active_column_index())
}

fn selected_browser_entry_name(app: &Lilypalooza, column_index: usize) -> Option<String> {
    app.editor
        .file_browser_columns()
        .get(column_index)
        .and_then(|column| match column {
            editor::EditorBrowserColumnSummary::Directory { entries } => entries
                .iter()
                .find(|entry| entry.selected)
                .map(|entry| entry.name.clone()),
            editor::EditorBrowserColumnSummary::FilePreview { .. } => None,
        })
}

fn file_preview_name(app: &Lilypalooza, column_index: usize) -> Option<String> {
    app.editor
        .file_browser_columns()
        .get(column_index)
        .and_then(|column| match column {
            editor::EditorBrowserColumnSummary::FilePreview { metadata } => {
                Some(metadata.name.clone())
            }
            editor::EditorBrowserColumnSummary::Directory { .. } => None,
        })
}

fn browser_entry_names(app: &Lilypalooza, column_index: usize) -> Vec<String> {
    app.editor
        .file_browser_columns()
        .get(column_index)
        .and_then(|column| match column {
            editor::EditorBrowserColumnSummary::Directory { entries } => Some(
                entries
                    .iter()
                    .map(|entry| entry.name.clone())
                    .collect::<Vec<_>>(),
            ),
            editor::EditorBrowserColumnSummary::FilePreview { .. } => None,
        })
        .unwrap_or_default()
}

#[test]
fn playback_poll_interval_is_visibility_aware() {
    let (mut app, _task) = new_with_default_test_state();
    app.playback = Some(
        AudioEngine::start_test(
            MixerState::new(),
            audio_engine_options(&app.playback_settings),
        )
        .expect("test audio engine should start"),
    );
    app.piano_roll.set_playback_position(0, 1, true);

    assert_eq!(
        app.playback_poll_interval(),
        Some(ACTIVE_PLAYBACK_POLL_INTERVAL)
    );

    hide_workspace_pane(&mut app, WorkspacePaneKind::Score);
    assert_eq!(
        app.playback_poll_interval(),
        Some(ACTIVE_PLAYBACK_POLL_INTERVAL)
    );

    hide_workspace_pane(&mut app, WorkspacePaneKind::PianoRoll);
    let _pane_was_unfolded = app.unfold_workspace_pane(WorkspacePaneKind::Mixer);
    assert_eq!(
        app.playback_poll_interval(),
        Some(ACTIVE_PLAYBACK_POLL_INTERVAL)
    );

    hide_workspace_pane(&mut app, WorkspacePaneKind::Mixer);
    assert_eq!(
        app.playback_poll_interval(),
        Some(PASSIVE_PLAYBACK_POLL_INTERVAL)
    );
}

#[test]
fn actions_palette_search_input_filters_actions() {
    let mut app = test_app();
    let mut ui = simulator(view(&app));

    ui.click("Search actions")
        .expect("search input should be clickable");
    let _status = ui.typewrite("settings");
    let messages: Vec<_> = ui.into_messages().collect();
    apply_messages(&mut app, messages);

    assert_eq!(app.shortcuts_search_query, "settings");
    assert!(
        shortcuts::filtered_action_metadata(&app.shortcuts_search_query)
            .iter()
            .any(|action| action.id == settings::ShortcutActionId::OpenSettingsFile)
    );
}

#[test]
fn actions_palette_clicking_action_emits_activation_message() {
    let mut app = test_app();
    let mut ui = simulator(view(&app));

    ui.click("Search actions")
        .expect("search input should be clickable");
    let _status = ui.typewrite("settings");
    let messages: Vec<_> = ui.into_messages().collect();
    apply_messages(&mut app, messages);

    let mut ui = simulator(view(&app));
    ui.click("Open Settings File")
        .expect("open settings action should be clickable");

    assert!(ui.into_messages().any(|message| matches!(
        message,
        Message::Shortcuts(messages::ShortcutsMessage::ActivateAction(
            settings::ShortcutActionId::OpenSettingsFile
        ))
    )));
}

#[test]
fn plugin_validator_path_uses_sibling_helper_for_dev_binary() {
    let path = plugin_validator_path_for_exe(Path::new("/repo/target/debug/lilypalooza"));

    assert_eq!(
        path,
        PathBuf::from("/repo/target/debug/lilypalooza-plugin-validator")
    );
}

#[test]
fn plugin_validator_path_uses_app_bundle_macos_directory() {
    let path = plugin_validator_path_for_exe(Path::new(
        "/Applications/Lilypalooza.app/Contents/MacOS/lilypalooza",
    ));

    assert_eq!(
        path,
        PathBuf::from("/Applications/Lilypalooza.app/Contents/MacOS/lilypalooza-plugin-validator")
    );
}

#[test]
fn starting_plugin_scan_logs_immediately() {
    let mut app = test_app();
    app.logger.clear();
    app.plugin_search_paths.clear();

    app.start_plugin_scan_with_validator(std::env::current_exe().expect("test exe"));

    assert_eq!(
        app.logger.last_line(),
        Some("Scanning plugins from 0 path(s)")
    );
    assert!(app.plugin_scan.is_active());
}

#[test]
fn open_settings_file_does_not_log_editor_open_noise() {
    let mut app = test_editor_app();
    app.logger.clear();

    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::OpenSettingsFile);

    assert!(
        app.logger
            .last_line()
            .is_none_or(|line| !line.contains("editor file"))
    );
}

#[test]
fn open_settings_file_restores_editor_missing_from_loaded_workspace() {
    let mut app = test_editor_app();
    set_workspace_to_score_only(&mut app);

    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::OpenSettingsFile);

    assert!(app.group_for_pane(WorkspacePaneKind::Editor).is_some());
    assert!(!app.is_pane_folded(WorkspacePaneKind::Editor));
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Editor));
}

#[test]
fn workspace_load_drops_folded_metadata_for_visible_panes() {
    let mut app = test_editor_app();
    app.apply_workspace_state(
        settings::WorkspaceLayoutSettings {
            root: Some(settings::DockNodeSettings::Group(
                settings::DockGroupSettings {
                    tabs: vec![WorkspacePaneKind::Score, WorkspacePaneKind::Editor],
                    active: WorkspacePaneKind::Score,
                },
            )),
            folded_panes: vec![settings::FoldedPaneSettings {
                pane: WorkspacePaneKind::Editor,
                restore: settings::FoldedPaneRestoreSettings::Tab {
                    anchor: WorkspacePaneKind::Score,
                },
            }],
            piano_visible: true,
        },
        settings::ScoreViewSettings::default(),
        settings::PianoRollViewSettings::default(),
    );

    assert!(app.group_for_pane(WorkspacePaneKind::Editor).is_some());
    assert!(!app.is_pane_folded(WorkspacePaneKind::Editor));
}

#[test]
fn workspace_load_marks_missing_mixer_folded_like_startup_load() {
    let mut app = test_editor_app();
    app.apply_workspace_state(
        settings::WorkspaceLayoutSettings {
            root: Some(settings::DockNodeSettings::Group(
                settings::DockGroupSettings {
                    tabs: vec![WorkspacePaneKind::Score],
                    active: WorkspacePaneKind::Score,
                },
            )),
            folded_panes: Vec::new(),
            piano_visible: true,
        },
        settings::ScoreViewSettings::default(),
        settings::PianoRollViewSettings::default(),
    );

    assert!(app.is_pane_folded(WorkspacePaneKind::Mixer));
}

#[test]
fn project_dirty_only_tracks_project_state_changes() {
    let (mut app, _task) = new_with_default_test_state();
    let temp = tempfile::tempdir().expect("temp dir should exist");

    app.apply_project_state(temp.path().to_path_buf(), ProjectState::default());
    assert!(!app.project_is_dirty());

    app.editor_recent_files.push(PathBuf::from("/tmp/test.ly"));
    assert!(!app.project_is_dirty());

    app.metronome.enabled = true;
    assert!(app.project_is_dirty());
}

#[test]
fn window_close_prompts_for_dirty_project_changes() {
    let (mut app, _task) = new_with_default_test_state();
    let temp = tempfile::tempdir().expect("temp dir should exist");

    app.apply_project_state(temp.path().to_path_buf(), ProjectState::default());
    app.metronome.enabled = true;

    let _discarded = app.handle_window_close_requested(app.main_window_id);

    assert!(matches!(
        app.pending_editor_action,
        Some(PendingEditorAction::ResolveDirtyProject {
            continuation: EditorContinuation::ExitApp
        })
    ));
    assert!(matches!(
        app.error_prompt.as_ref().map(|prompt| prompt.buttons()),
        Some(crate::error_prompt::PromptButtons::SaveDiscardCancel)
    ));
}

#[test]
fn discard_dirty_project_prompt_clears_pending_action() {
    let (mut app, _task) = new_with_default_test_state();
    let temp = tempfile::tempdir().expect("temp dir should exist");

    app.apply_project_state(temp.path().to_path_buf(), ProjectState::default());
    app.metronome.enabled = true;
    let _discarded = app.handle_window_close_requested(app.main_window_id);

    let _discarded = app.handle_prompt_message(PromptMessage::Discard);

    assert!(app.pending_editor_action.is_none());
    assert!(app.error_prompt.is_none());
}

#[test]
fn metronome_shortcut_toggles_outside_editor() {
    let mut app = test_editor_app();
    app.metronome.enabled = false;
    app.focused_workspace_pane = Some(WorkspacePaneKind::PianoRoll);

    let _discarded = update(
        &mut app,
        primary_char_key_press("k", keyboard::key::Code::KeyK),
    );

    assert!(app.metronome.enabled);
}

#[test]
fn metronome_shortcut_is_suppressed_in_editor() {
    let mut app = test_editor_app();
    app.metronome.enabled = false;
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::OpenSettingsFile);

    let _discarded = update(
        &mut app,
        primary_char_key_press("k", keyboard::key::Code::KeyK),
    );

    assert!(!app.metronome.enabled);
}

#[test]
fn metronome_popup_escape_closes_menu() {
    let mut app = test_editor_app();
    app.metronome_menu_open = true;

    let _discarded = update(
        &mut app,
        named_key_press(keyboard::key::Named::Escape, keyboard::key::Code::Escape),
    );

    assert!(!app.metronome_menu_open);
}

#[test]
fn instrument_browser_escape_closes_menu() {
    let mut app = test_editor_app();
    app.open_processor_browser_target = Some(super::processor_editor_windows::EditorTarget {
        strip_index: 1,
        slot_index: 0,
    });

    let _discarded = update(
        &mut app,
        named_key_press(keyboard::key::Named::Escape, keyboard::key::Code::Escape),
    );

    assert_eq!(app.open_processor_browser_target, None);
}

#[test]
fn metronome_transport_messages_toggle_and_close_menu() {
    let mut app = test_editor_app();

    let _discarded = update(
        &mut app,
        Message::PianoRoll(PianoRollMessage::TransportOpenMetronomeMenu),
    );
    assert!(app.metronome_menu_open);

    let _discarded = update(
        &mut app,
        Message::PianoRoll(PianoRollMessage::TransportToggleMetronome),
    );
    assert!(app.metronome.enabled);

    let _discarded = update(
        &mut app,
        Message::PianoRoll(PianoRollMessage::TransportCloseMetronomeMenu),
    );
    assert!(!app.metronome_menu_open);
}

#[test]
fn browser_arrow_keys_navigate_columns_after_browser_focus_click() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("alpha")).expect("alpha dir");
    fs::create_dir(root.path().join("alpha").join("child")).expect("child dir");
    fs::write(
        root.path().join("alpha").join("child").join("inner.txt"),
        "inner",
    )
    .expect("inner file");
    fs::write(root.path().join("alpha").join("b.txt"), "b").expect("b file");
    fs::create_dir(root.path().join("beta")).expect("beta dir");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("alpha"),
            is_dir: true,
        }),
    );

    assert!(app.editor_file_browser_focused);
    assert_eq!(app.editor.file_browser_columns().len(), 2);
    assert_eq!(active_browser_column_index(&app), Some(0));
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("alpha")
    );
    assert_eq!(selected_browser_entry_name(&app, 1), None);

    let _discarded = update(
        &mut app,
        named_key_press(
            keyboard::key::Named::ArrowRight,
            keyboard::key::Code::ArrowRight,
        ),
    );
    assert_eq!(active_browser_column_index(&app), Some(1));
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("alpha")
    );
    assert_eq!(
        selected_browser_entry_name(&app, 1).as_deref(),
        Some("child")
    );
    assert_eq!(app.editor.file_browser_columns().len(), 3);
    assert_eq!(selected_browser_entry_name(&app, 2), None);

    let _discarded = update(
        &mut app,
        named_key_press(
            keyboard::key::Named::ArrowDown,
            keyboard::key::Code::ArrowDown,
        ),
    );
    assert_eq!(
        selected_browser_entry_name(&app, 1).as_deref(),
        Some("b.txt")
    );
    assert_eq!(app.editor.file_browser_columns().len(), 3);
    assert_eq!(file_preview_name(&app, 2).as_deref(), Some("b.txt"));

    let _discarded = update(
        &mut app,
        named_key_press(
            keyboard::key::Named::ArrowLeft,
            keyboard::key::Code::ArrowLeft,
        ),
    );
    assert_eq!(active_browser_column_index(&app), Some(0));
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("alpha")
    );
    assert_eq!(selected_browser_entry_name(&app, 1), None);

    let _discarded = update(
        &mut app,
        named_key_press(
            keyboard::key::Named::ArrowRight,
            keyboard::key::Code::ArrowRight,
        ),
    );
    assert_eq!(active_browser_column_index(&app), Some(1));
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("alpha")
    );
    assert_eq!(
        selected_browser_entry_name(&app, 1).as_deref(),
        Some("child")
    );
    assert_eq!(app.editor.file_browser_columns().len(), 3);
    assert_eq!(selected_browser_entry_name(&app, 2), None);
}

#[test]
fn browser_focus_blocks_editor_text_input() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("alpha")).expect("alpha dir");
    let file_path = root.path().join("note.txt");
    fs::write(&file_path, "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.set_focused_workspace_pane(WorkspacePaneKind::Editor);
    let _discarded = app.open_editor_file_in_editor(&file_path);
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("alpha"),
            is_dir: true,
        }),
    );

    let before = app.editor.active_content().expect("active content");
    assert!(app.editor_file_browser_focused);
    assert!(!app.editor.active_editor_is_focused());

    let _discarded = update(&mut app, char_key_press("x", keyboard::key::Code::KeyX));

    assert_eq!(
        app.editor.active_content().as_deref(),
        Some(before.as_str())
    );
    assert!(!app.editor.active_editor_is_focused());
}

#[test]
fn point_and_click_unfolds_and_focuses_editor_pane() {
    let root = TempDir::new().expect("tempdir");
    let score_path = root.path().join("score.ly");
    let target_path = root.path().join("part.ly");
    let other_path = root.path().join("other.ly");
    fs::write(&score_path, "\\version \"2.24.0\"").expect("score file");
    fs::write(&target_path, "{ c'4 }").expect("target file");
    fs::write(&other_path, "{ d'4 }").expect("other file");

    let svg_source = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"><a xlink:href="textedit://{}:3:2:3"><g transform="translate(20,30)"></g></a></svg>"#,
        target_path.display()
    );
    let note_anchors = score_cursor::parse_svg_note_anchors(&svg_source, 0);
    assert_eq!(note_anchors.len(), 1);

    let mut app = test_editor_app();
    app.current_score = Some(SelectedScore {
        path: score_path,
        file_name: "score.ly".into(),
    });
    let svg_bytes = svg_source.clone().into_bytes();
    app.rendered_score = Some(RenderedScore {
        pages: vec![RenderedPage {
            handle: svg::Handle::from_memory(svg_bytes.clone()),
            svg_bytes: Bytes::from(svg_bytes),
            display_size: SvgSize {
                width: 100.0,
                height: 100.0,
            },
            coord_size: SvgSize {
                width: 100.0,
                height: 100.0,
            },
            note_anchors,
            system_bands: Vec::new(),
        }],
        current_page: 0,
    });
    let scale = score_view::score_base_scale() * app.svg_zoom;
    let inset = f32::from(crate::ui_style::PADDING_SM);
    app.score_viewport_cursor = Some(iced::Point::new(20.0 * scale + inset, 30.0 * scale + inset));

    if !app.is_pane_folded(WorkspacePaneKind::Editor) {
        assert!(app.fold_workspace_pane(WorkspacePaneKind::Editor));
    }
    assert!(app.is_pane_folded(WorkspacePaneKind::Editor));

    let _discarded = update(&mut app, Message::Viewer(ViewerMessage::OpenPointAndClick));

    assert!(!app.is_pane_folded(WorkspacePaneKind::Editor));
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Editor));
    assert!(app.editor.find_tab_by_path(&target_path).is_some());
}

#[test]
fn point_and_click_activates_editor_pane_tab_and_target_editor_tab() {
    let root = TempDir::new().expect("tempdir");
    let score_path = root.path().join("score.ly");
    let target_path = root.path().join("part.ly");
    let other_path = root.path().join("other.ly");
    fs::write(&score_path, "\\version \"2.24.0\"").expect("score file");
    fs::write(&target_path, "{ c'4 }").expect("target file");
    fs::write(&other_path, "{ d'4 }").expect("other file");

    let svg_source = format!(
        r#"<svg width="100" height="100" viewBox="0 0 100 100"><a xlink:href="textedit://{}:3:2:3"><g transform="translate(20,30)"></g></a></svg>"#,
        target_path.display()
    );
    let note_anchors = score_cursor::parse_svg_note_anchors(&svg_source, 0);

    let mut app = test_editor_app();
    app.current_score = Some(SelectedScore {
        path: score_path,
        file_name: "score.ly".into(),
    });
    let svg_bytes = svg_source.clone().into_bytes();
    app.rendered_score = Some(RenderedScore {
        pages: vec![RenderedPage {
            handle: svg::Handle::from_memory(svg_bytes.clone()),
            svg_bytes: Bytes::from(svg_bytes),
            display_size: SvgSize {
                width: 100.0,
                height: 100.0,
            },
            coord_size: SvgSize {
                width: 100.0,
                height: 100.0,
            },
            note_anchors,
            system_bands: Vec::new(),
        }],
        current_page: 0,
    });
    let _pane_was_unfolded = app.unfold_workspace_pane(WorkspacePaneKind::Editor);
    let _discarded = app.open_editor_file_in_editor(&other_path);

    if let Some(group_id) = app.group_for_pane(WorkspacePaneKind::Editor)
        && let Some(group) = app.dock_groups.get_mut(&group_id)
        && let Some(other_pane) = group
            .tabs
            .iter()
            .copied()
            .find(|pane| *pane != WorkspacePaneKind::Editor)
    {
        group.active = other_pane;
    }

    let scale = score_view::score_base_scale() * app.svg_zoom;
    let inset = f32::from(crate::ui_style::PADDING_SM);
    app.score_viewport_cursor = Some(iced::Point::new(20.0 * scale + inset, 30.0 * scale + inset));

    let _discarded = update(&mut app, Message::Viewer(ViewerMessage::OpenPointAndClick));

    let group_id = app
        .group_for_pane(WorkspacePaneKind::Editor)
        .expect("editor group");
    let group = app.dock_groups.get(&group_id).expect("editor group state");

    assert_eq!(group.active, WorkspacePaneKind::Editor);
    assert_eq!(
        app.editor
            .active_file_backed_tab_path()
            .as_ref()
            .map(|path| state::normalize_path(path)),
        Some(state::normalize_path(&target_path))
    );
}

#[test]
fn browser_single_click_selects_file_and_double_click_opens_it() {
    let root = TempDir::new().expect("tempdir");
    let file_path = root.path().join("note.txt");
    fs::write(&file_path, "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let before = app.editor.active_content();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: file_path.clone(),
            is_dir: false,
        }),
    );

    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("note.txt")
    );
    assert_eq!(file_preview_name(&app, 1).as_deref(), Some("note.txt"));
    assert_eq!(app.editor.active_content(), before);

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryDoublePressed {
            column_index: 0,
            path: file_path,
            is_dir: false,
        }),
    );

    assert_eq!(app.editor.active_content().as_deref(), Some("hello"));
}

#[test]
fn browser_rename_action_starts_inline_rename() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );

    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);

    assert!(matches!(
        app.browser_inline_edit.as_ref().map(|edit| edit.kind),
        Some(BrowserInlineEditKind::Rename)
    ));
}

#[test]
fn browser_inline_rename_enter_commits_edit() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
            "renamed.txt".to_string(),
        )),
    );

    let _discarded = update(
        &mut app,
        named_key_press(keyboard::key::Named::Enter, keyboard::key::Code::Enter),
    );

    assert!(app.browser_inline_edit.is_none());
    assert!(root.path().join("renamed.txt").exists());
}

#[test]
fn browser_captured_enter_after_commit_does_not_restart_rename() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
            "renamed.txt".to_string(),
        )),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
    );

    let _discarded = update(
        &mut app,
        Message::KeyPressed(KeyPress {
            status: event::Status::Captured,
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            modifiers: keyboard::Modifiers::default(),
        }),
    );

    assert!(app.browser_inline_edit.is_none());
    assert!(root.path().join("renamed.txt").exists());
}

#[test]
fn browser_delete_action_opens_delete_prompt() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );

    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserDelete);

    assert!(app.error_prompt.is_some());
    assert!(matches!(
        app.prompt_ok_action,
        Some(PromptOkAction::DeleteBrowserPath(_))
    ));
}

#[test]
fn prompt_enter_runs_selected_button_action() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserDelete);

    let _discarded = update(
        &mut app,
        Message::KeyPressed(KeyPress {
            status: event::Status::Ignored,
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            modifiers: keyboard::Modifiers::default(),
        }),
    );

    assert!(app.error_prompt.is_none());
    assert!(app.browser_inline_edit.is_none());
    assert!(!root.path().join("note.txt").exists());
}

#[test]
fn browser_copy_paste_copies_selected_entry() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("folder")).expect("folder dir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCopy);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("folder"),
            is_dir: true,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

    assert!(root.path().join("note.txt").exists());
    assert!(root.path().join("folder").join("note.txt").exists());
}

#[test]
fn browser_cut_paste_moves_selected_entry() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("folder")).expect("folder dir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCut);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("folder"),
            is_dir: true,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

    assert!(!root.path().join("note.txt").exists());
    assert!(root.path().join("folder").join("note.txt").exists());
    assert!(app.browser_clipboard.is_none());
}

#[test]
fn browser_drag_release_moves_item_into_directory() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("folder")).expect("folder dir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserDragMoved(Point::new(
            0.0, 0.0,
        ))),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserDragMoved(Point::new(
            24.0, 0.0,
        ))),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryHovered {
            column_index: 0,
            path: root.path().join("folder"),
            is_dir: true,
        }),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserDragReleased),
    );

    assert!(!root.path().join("note.txt").exists());
    assert!(root.path().join("folder").join("note.txt").exists());
}

#[test]
fn browser_delete_undo_redo_restores_item() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );

    let _discarded = app.delete_browser_path_with_history(&root.path().join("note.txt"));
    assert!(!root.path().join("note.txt").exists());

    let _discarded = app.undo_browser_operation();
    assert!(root.path().join("note.txt").exists());

    let _discarded = app.redo_browser_operation();
    assert!(!root.path().join("note.txt").exists());
}

#[test]
fn browser_cut_paste_undo_redo_moves_item_back_and_forth() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("folder")).expect("folder dir");
    fs::write(root.path().join("note.txt"), "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("note.txt"),
            is_dir: false,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCut);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("folder"),
            is_dir: true,
        }),
    );
    let _discarded = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

    assert!(!root.path().join("note.txt").exists());
    assert!(root.path().join("folder").join("note.txt").exists());

    let _discarded = app.undo_browser_operation();
    assert!(root.path().join("note.txt").exists());
    assert!(!root.path().join("folder").join("note.txt").exists());

    let _discarded = app.redo_browser_operation();
    assert!(!root.path().join("note.txt").exists());
    assert!(root.path().join("folder").join("note.txt").exists());
}

#[test]
fn browser_focus_message_moves_focus_from_editor() {
    let root = TempDir::new().expect("tempdir");
    let file_path = root.path().join("note.txt");
    fs::write(&file_path, "hello").expect("note file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    app.set_focused_workspace_pane(WorkspacePaneKind::Editor);
    let _discarded = app.open_editor_file_in_editor(&file_path);
    app.editor.request_focus();

    assert!(app.editor.active_editor_is_focused());

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserFocused),
    );

    assert!(app.editor_file_browser_focused);
    assert!(!app.editor.active_editor_is_focused());
}

#[test]
fn browser_toolbar_hidden_toggle_updates_entries() {
    let root = TempDir::new().expect("tempdir");
    fs::create_dir(root.path().join("alpha")).expect("alpha dir");
    fs::write(root.path().join("beta.txt"), "b").expect("beta file");
    fs::write(root.path().join(".hidden.txt"), "h").expect("hidden file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    assert_eq!(browser_entry_names(&app, 0), vec!["alpha", "beta.txt"]);
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserToggleHiddenRequested),
    );
    assert_eq!(
        browser_entry_names(&app, 0),
        vec!["alpha", ".hidden.txt", "beta.txt"]
    );
}

#[test]
fn browser_new_file_requested_starts_inline_edit() {
    let root = TempDir::new().expect("tempdir");
    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserNewFileRequested),
    );

    assert!(matches!(
        app.browser_inline_edit.as_ref().map(|edit| edit.kind),
        Some(BrowserInlineEditKind::NewFile)
    ));
    assert_eq!(app.browser_inline_edit_value, "untitled");
    assert!(app.editor_file_browser_focused);
    assert!(!app.editor.active_editor_is_focused());
}

#[test]
fn browser_commit_new_file_creates_and_selects_file() {
    let root = TempDir::new().expect("tempdir");
    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserNewFileRequested),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
            "created.txt".to_string(),
        )),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
    );

    assert!(root.path().join("created.txt").exists());
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("created.txt")
    );
    assert_eq!(file_preview_name(&app, 1).as_deref(), Some("created.txt"));
}

#[test]
fn browser_commit_rename_renames_selected_entry() {
    let root = TempDir::new().expect("tempdir");
    fs::write(root.path().join("old.txt"), "old").expect("old file");

    let mut app = test_editor_app();
    app.project_root = Some(root.path().to_path_buf());
    app.editor.set_project_root(Some(root.path().to_path_buf()));
    app.editor.toggle_file_browser();
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
            column_index: 0,
            path: root.path().join("old.txt"),
            is_dir: false,
        }),
    );

    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserRenameRequested),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
            "renamed.txt".to_string(),
        )),
    );
    let _discarded = update(
        &mut app,
        Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
    );

    assert!(!root.path().join("old.txt").exists());
    assert!(root.path().join("renamed.txt").exists());
    assert_eq!(
        selected_browser_entry_name(&app, 0).as_deref(),
        Some("renamed.txt")
    );
    assert_eq!(file_preview_name(&app, 1).as_deref(), Some("renamed.txt"));
}
