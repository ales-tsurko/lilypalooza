use std::{cell::RefCell, rc::Rc};

use lilypalooza_audio::{AudioEngine, AudioEngineOptions, MixerState, SlotState, TrackId};

use super::{tests_editor_windows::*, *};

#[test]
pub(super) fn rack_hover_press_drag_release_reorders_effect_slots() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let first = SlotState::built_in(
        lilypalooza_audio::BUILTIN_GAIN_ID,
        lilypalooza_audio::ProcessorState(vec![1]),
    );
    let second = SlotState::built_in(
        lilypalooza_audio::BUILTIN_GAIN_ID,
        lilypalooza_audio::ProcessorState(vec![2]),
    );
    let third = SlotState::built_in(
        lilypalooza_audio::BUILTIN_GAIN_ID,
        lilypalooza_audio::ProcessorState(vec![3]),
    );
    playback
        .mixer()
        .set_track_effects(
            TrackId(0),
            vec![first.clone(), second.clone(), third.clone()],
        )
        .expect("effects should be installed");
    app.playback = Some(playback);
    let row_height = crate::app::mixer::EFFECT_RACK_HEIGHT / 7.0;

    let _discarded = app.handle_mixer_message(MixerMessage::TrackEffectDragMoved {
        strip_index: 1,
        y: row_height * 0.5,
    });
    let _discarded = app.handle_primary_mouse_pressed(true);
    let _discarded = app.handle_mixer_message(MixerMessage::TrackEffectDragMoved {
        strip_index: 1,
        y: row_height * 2.5,
    });
    let _discarded = app.handle_primary_mouse_pressed(false);

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");
    assert_eq!(track.effect(0), Some(&second));
    assert_eq!(track.effect(1), Some(&third));
    assert_eq!(track.effect(2), Some(&first));
}

#[test]
pub(super) fn processor_slot_hover_can_start_effect_drag() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_track_effects(
            TrackId(0),
            vec![SlotState::built_in(
                lilypalooza_audio::BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState(vec![1]),
            )],
        )
        .expect("effects should be installed");
    app.playback = Some(playback);

    let target = EditorTarget {
        strip_index: 1,
        slot_index: 1,
    };
    let _discarded = app.handle_mixer_message(MixerMessage::SetProcessorSlotHovered(Some((
        target,
        crate::app::mixer::ProcessorSlotSegment::Editor,
    ))));
    let _discarded = app.handle_primary_mouse_pressed(true);

    assert_eq!(app.effect_drag_source, Some((1, 0)));
    assert_eq!(app.effect_drag_target, Some((1, 0)));
}

#[test]
pub(super) fn rack_drag_target_uses_scroll_offset() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let effects: Vec<_> = (1..=4)
        .map(|value| {
            SlotState::built_in(
                lilypalooza_audio::BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState(vec![value]),
            )
        })
        .collect();
    playback
        .mixer()
        .set_track_effects(TrackId(0), effects.clone())
        .expect("effects should be installed");
    app.playback = Some(playback);
    let row_height = crate::app::mixer::EFFECT_RACK_ROW_HEIGHT;
    app.effect_rack_scroll_y.insert(1, row_height * 2.0);

    let _discarded = app.handle_mixer_message(MixerMessage::StartTrackEffectDrag {
        strip_index: 1,
        effect_index: 0,
    });
    let _discarded = app.handle_mixer_message(MixerMessage::TrackEffectDragMoved {
        strip_index: 1,
        y: row_height * 1.5,
    });
    let _discarded = app.handle_primary_mouse_pressed(false);

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");
    assert_eq!(track.effect(0), Some(&effects[1]));
    assert_eq!(track.effect(1), Some(&effects[2]));
    assert_eq!(track.effect(2), Some(&effects[3]));
    assert_eq!(track.effect(3), Some(&effects[0]));
}

#[test]
pub(super) fn rack_drag_near_bottom_enables_push_to_scroll_until_release() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_track_effects(
            TrackId(0),
            vec![SlotState::built_in(
                lilypalooza_audio::BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState(vec![1]),
            )],
        )
        .expect("effects should be installed");
    app.playback = Some(playback);
    app.effect_rack_viewport_height
        .insert(1, crate::app::mixer::EFFECT_RACK_HEIGHT);

    let _discarded = app.handle_mixer_message(MixerMessage::StartTrackEffectDrag {
        strip_index: 1,
        effect_index: 0,
    });
    let _discarded = app.handle_mixer_message(MixerMessage::TrackEffectDragMoved {
        strip_index: 1,
        y: crate::app::mixer::EFFECT_RACK_HEIGHT - 1.0,
    });

    assert_eq!(app.effect_rack_autoscroll_direction, 1);

    let _discarded = app.handle_primary_mouse_pressed(false);

    assert_eq!(app.effect_rack_autoscroll_direction, 0);
}

#[test]
pub(super) fn rack_cursor_exit_clears_hovered_effect_before_mouse_press() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_track_effects(
            TrackId(0),
            vec![SlotState::built_in(
                lilypalooza_audio::BUILTIN_GAIN_ID,
                lilypalooza_audio::ProcessorState(vec![1]),
            )],
        )
        .expect("effects should be installed");
    app.playback = Some(playback);

    let _discarded = app.handle_mixer_message(MixerMessage::TrackEffectDragMoved {
        strip_index: 1,
        y: crate::app::mixer::EFFECT_RACK_ROW_HEIGHT * 0.5,
    });
    let _discarded = app.handle_mixer_message(MixerMessage::EffectRackCursorLeft(1));
    let _discarded = app.handle_primary_mouse_pressed(true);

    assert_eq!(app.effect_rack_hovered_effect, None);
    assert_eq!(app.effect_drag_source, None);
}

#[test]
pub(super) fn selecting_track_instrument_opens_editor_when_available() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_soundfont(lilypalooza_audio::SoundfontResource {
            id: "default".to_string(),
            name: "FluidR3".to_string(),
            path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/soundfonts/lilypalooza-test.sf2")
                .canonicalize()
                .expect("test SoundFont should exist"),
        })
        .expect("test SoundFont should load");
    app.playback = Some(playback);

    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        target,
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
            name: "SF-01".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));

    assert!(
        app.processor_editor_windows
            .focus_existing(EditorTarget {
                strip_index: 1,
                slot_index: 0,
            })
            .is_some()
    );
}

pub(super) fn app_with_soundfont_track() -> (Lilypalooza, EditorTarget) {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    playback
        .mixer()
        .set_soundfont(lilypalooza_audio::SoundfontResource {
            id: "default".to_string(),
            name: "FluidR3".to_string(),
            path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/soundfonts/lilypalooza-test.sf2")
                .canonicalize()
                .expect("test SoundFont should exist"),
        })
        .expect("test SoundFont should load");
    app.playback = Some(playback);
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        target,
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
            name: "SF-01".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));
    let temp = tempfile::tempdir().expect("temp project dir should exist");
    app.project_root = Some(temp.path().to_path_buf());
    (app, target)
}

#[test]
pub(super) fn replacing_processor_with_open_editor_waits_for_window_close_and_one_settle_frame() {
    let (mut app, target) = app_with_soundfont_track();
    let window_id = app
        .processor_editor_windows
        .window_for_target(target)
        .expect("editor window should be pending or open");

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        target,
        crate::app::mixer::ProcessorChoice::None,
    ));

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some()
    );

    let _discarded = app.handle_window_closed(window_id);

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some()
    );

    let _discarded = app.handle_frame(std::time::Instant::now());

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some()
    );

    let _discarded = app.handle_frame(std::time::Instant::now());

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some_and(SlotState::is_empty)
    );
}

#[test]
pub(super) fn replacing_processor_with_hidden_editor_does_not_wait_for_window_close_event() {
    let (mut app, target) = app_with_soundfont_track();
    let _discarded = app.processor_editor_windows.remove_target(target);
    let detached = Rc::new(RefCell::new(0));
    attach_recording_editor(&mut app, target, Rc::clone(&detached));
    let window_id = app
        .processor_editor_windows
        .window_for_target(target)
        .expect("editor should exist");
    app.processor_editor_windows
        .hide_window(window_id)
        .expect("editor should hide");

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        target,
        crate::app::mixer::ProcessorChoice::None,
    ));

    assert_eq!(*detached.borrow(), 1);
    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some()
    );

    let _discarded = app.handle_frame(std::time::Instant::now());

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some()
    );

    let _discarded = app.handle_frame(std::time::Instant::now());

    assert!(
        app.playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .track(TrackId(0))
            .expect("track")
            .instrument_slot()
            .is_some_and(SlotState::is_empty)
    );
}

#[test]
pub(super) fn processor_frame_save_command_creates_user_preset_for_slot() {
    let (mut app, target) = app_with_soundfont_track();

    app.handle_processor_editor_frame_command(target, editor_host::EditorFrameCommand::SavePreset);

    let kind = app.processor_kind_for_target(target).expect("slot kind");
    let presets = app.processor_presets.presets_for(&kind);
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "User Preset 1");
}

#[test]
pub(super) fn processor_frame_load_command_updates_slot_state() {
    let (mut app, target) = app_with_soundfont_track();
    let kind = app.processor_kind_for_target(target).expect("slot kind");
    let state = soundfont_synth::encode_state(&SoundfontProcessorState {
        program: 7,
        output_gain: 0.25,
        ..SoundfontProcessorState::default()
    });
    let id = app
        .processor_presets
        .save_user_preset("Muted Piano", kind, state.clone());

    app.handle_processor_editor_frame_command(
        target,
        editor_host::EditorFrameCommand::LoadPreset(id),
    );

    let slot_state = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .strip_by_index(target.strip_index)
        .and_then(|strip| strip.slot(target.slot_index))
        .map(|slot| slot.state.clone());
    assert_eq!(slot_state, Some(state));
}

#[test]
pub(super) fn processor_frame_next_from_placeholder_loads_first_preset() {
    let (mut app, target) = app_with_soundfont_track();
    let kind = app.processor_kind_for_target(target).expect("slot kind");
    let first_state = soundfont_synth::encode_state(&SoundfontProcessorState {
        program: 1,
        ..SoundfontProcessorState::default()
    });
    let second_state = soundfont_synth::encode_state(&SoundfontProcessorState {
        program: 2,
        ..SoundfontProcessorState::default()
    });
    app.processor_presets
        .save_user_preset("First", kind.clone(), first_state.clone());
    app.processor_presets
        .save_user_preset("Second", kind, second_state);
    app.refresh_editor_preset_state(target, None);

    app.handle_processor_editor_frame_command(target, editor_host::EditorFrameCommand::NextPreset);

    let slot_state = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .strip_by_index(target.strip_index)
        .and_then(|strip| strip.slot(target.slot_index))
        .map(|slot| slot.state.clone());
    assert_eq!(slot_state, Some(first_state));
}

#[test]
pub(super) fn processor_frame_rename_command_updates_user_preset() {
    let (mut app, target) = app_with_soundfont_track();
    let kind = app.processor_kind_for_target(target).expect("slot kind");
    let id = app.processor_presets.save_user_preset(
        "Warm Piano",
        kind.clone(),
        lilypalooza_audio::ProcessorState(vec![]),
    );

    app.handle_processor_editor_frame_command(
        target,
        editor_host::EditorFrameCommand::RenamePreset {
            id: id.clone(),
            name: "Soft Piano".to_string(),
        },
    );

    assert_eq!(
        app.processor_presets.presets_for(&kind)[0].name,
        "Soft Piano"
    );
}

#[test]
pub(super) fn processor_frame_delete_command_removes_user_preset() {
    let (mut app, target) = app_with_soundfont_track();
    let kind = app.processor_kind_for_target(target).expect("slot kind");
    let id = app.processor_presets.save_user_preset(
        "Warm Piano",
        kind.clone(),
        lilypalooza_audio::ProcessorState(vec![]),
    );

    app.handle_processor_editor_frame_command(
        target,
        editor_host::EditorFrameCommand::DeletePreset(id),
    );

    assert!(app.processor_presets.presets_for(&kind).is_empty());
}

#[test]
pub(super) fn processor_frame_toggle_command_tracks_expanded_browser_target() {
    let (mut app, target) = app_with_soundfont_track();

    app.handle_processor_editor_frame_command(
        target,
        editor_host::EditorFrameCommand::TogglePresetBrowser,
    );

    assert_eq!(app.expanded_processor_preset_browser, Some(target));

    app.handle_processor_editor_frame_command(
        target,
        editor_host::EditorFrameCommand::TogglePresetBrowser,
    );

    assert_eq!(app.expanded_processor_preset_browser, None);
}

#[test]
pub(super) fn mixer_main_route_message_updates_track_and_bus_routes() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_ids: Vec<_> = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .buses()
        .iter()
        .filter_map(|bus| bus.bus_id.map(|id| id.0))
        .collect();

    let _discarded = app.handle_mixer_message(MixerMessage::SetMainRoute(
        crate::app::mixer::RoutingStrip::Track(0),
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_ids[0])),
    ));
    let _discarded = app.handle_mixer_message(MixerMessage::SetMainRoute(
        crate::app::mixer::RoutingStrip::Bus(bus_ids[0]),
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_ids[1])),
    ));
    let _discarded = app.handle_mixer_message(MixerMessage::SetMainRoute(
        crate::app::mixer::RoutingStrip::Bus(bus_ids[0]),
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_ids[0])),
    ));

    let mixer = app.playback.as_ref().expect("playback").mixer_state();
    assert_eq!(
        mixer.track(TrackId(0)).expect("track").routing.main,
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_ids[0]))
    );
    assert_eq!(
        mixer.bus(BusId(bus_ids[0])).expect("bus").routing.main,
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_ids[1]))
    );
}

#[test]
pub(super) fn mixer_send_messages_update_destination_gain_enabled_and_prepost() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_ids: Vec<_> = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .buses()
        .iter()
        .filter_map(|bus| bus.bus_id.map(|id| id.0))
        .collect();
    let source = crate::app::mixer::RoutingStrip::Track(0);

    let _discarded = app.handle_mixer_message(MixerMessage::AddSend(source, bus_ids[0]));
    let _discarded =
        app.handle_mixer_message(MixerMessage::SetSendDestination(source, 0, bus_ids[1]));
    let _discarded = app.handle_mixer_message(MixerMessage::SetSendGain(source, 0, -7.5));
    let _discarded = app.handle_mixer_message(MixerMessage::ToggleSendEnabled(source, 0));
    let _discarded = app.handle_mixer_message(MixerMessage::ToggleSendPreFader(source, 0));

    let send = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .track(TrackId(0))
        .expect("track")
        .routing
        .sends[0];
    assert_eq!(send.bus_id, BusId(bus_ids[1]));
    crate::test_assertions::assert_float_eq!(send.gain_db, -7.5);
    assert!(!send.enabled);
    assert!(send.pre_fader);
}

#[test]
pub(super) fn removing_bus_from_app_clears_routes_and_sends() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_id = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .buses()
        .first()
        .and_then(|bus| bus.bus_id.map(|id| id.0))
        .expect("bus");

    let _discarded = app.handle_mixer_message(MixerMessage::SetMainRoute(
        crate::app::mixer::RoutingStrip::Track(0),
        lilypalooza_audio::TrackRoute::Bus(BusId(bus_id)),
    ));
    let _discarded = app.handle_mixer_message(MixerMessage::AddSend(
        crate::app::mixer::RoutingStrip::Track(0),
        bus_id,
    ));
    let _discarded = app.remove_bus_confirmed(bus_id);

    let track = app
        .playback
        .as_ref()
        .expect("playback")
        .mixer_state()
        .track(TrackId(0))
        .expect("track");
    assert_eq!(track.routing.main, lilypalooza_audio::TrackRoute::Master);
    assert!(track.routing.sends.is_empty());
}

#[test]
pub(super) fn mixer_changes_mark_project_dirty() {
    let mut app = test_app();
    let temp = tempfile::tempdir().expect("temp dir should exist");
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.apply_project_state(temp.path().to_path_buf(), ProjectState::default());
    assert!(!app.project_is_dirty());

    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_id = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .buses()
        .first()
        .and_then(|bus| bus.bus_id.map(|id| id.0))
        .expect("bus should be added");
    let _discarded = app.handle_mixer_message(MixerMessage::SetBusGain(bus_id, -6.0));

    assert!(app.project_is_dirty());
}

#[test]
pub(super) fn unsaved_project_mixer_changes_prompt_on_close() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.saved_project_state = Some(app.current_project_state());

    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);

    let _discarded = app.handle_window_close_requested(app.main_window_id);

    assert!(matches!(
        app.pending_editor_action,
        Some(crate::app::PendingEditorAction::ResolveDirtyProject {
            continuation: crate::app::EditorContinuation::ExitApp
        })
    ));
}

#[test]
pub(super) fn editor_window_close_request_hides_only_editor_window() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.saved_project_state = Some(app.current_project_state());
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);

    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = iced::window::Id::unique();
    app.processor_editor_windows.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(FakeEditorSession),
        window_id,
    );
    app.processor_editor_windows
        .attach(
            window_id,
            None,
            EditorParent {
                window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                    iced::window::raw_window_handle::AppKitWindowHandle::new(std::ptr::NonNull::<
                        std::ffi::c_void,
                    >::dangling(
                    )),
                ),
                display: None,
            },
        )
        .expect("attach should succeed");

    let _discarded = app.handle_window_close_requested(window_id);

    assert!(app.processor_editor_windows.contains_window(target));
    assert!(app.pending_editor_action.is_none());
}
