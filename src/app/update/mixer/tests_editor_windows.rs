use std::{cell::RefCell, rc::Rc};

use lilypalooza_audio::{
    AudioEngine, AudioEngineOptions, BusId, EditorError, EditorParent, EditorSession, EditorSize,
    MixerState, SlotState, TrackId,
};

use super::{Lilypalooza, MixerHistoryMode, mixer_message_history_mode};
use crate::app::{
    RenameTarget,
    messages::{MixerMessage, PromptMessage},
    processor_editor_windows::EditorTarget,
};

pub(super) struct FakeEditorSession;

impl EditorSession for FakeEditorSession {
    fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        Ok(())
    }

    fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        Ok(size)
    }
}

pub(super) struct RecordingEditorSession {
    pub(super) visible: Rc<RefCell<Vec<bool>>>,
    pub(super) detached: Rc<RefCell<usize>>,
}
pub(super) struct InitialSizeEditorSession {
    pub(super) calls: Rc<RefCell<usize>>,
}

impl EditorSession for RecordingEditorSession {
    fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        *self.detached.borrow_mut() += 1;
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), EditorError> {
        self.visible.borrow_mut().push(visible);
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        Ok(size)
    }
}

impl EditorSession for InitialSizeEditorSession {
    fn initial_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        *self.calls.borrow_mut() += 1;
        Ok(Some(EditorSize {
            width: 1200,
            height: 600,
        }))
    }

    fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        Ok(())
    }

    fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        Ok(size)
    }
}

pub(super) fn test_app() -> Lilypalooza {
    let (app, _task) = super::super::super::new_with_default_test_state();
    app
}

#[test]
pub(super) fn processor_editor_window_settings_disable_native_resizing() {
    let settings = super::processor_editor_window_settings(
        lilypalooza_audio::EditorDescriptor {
            default_size: EditorSize {
                width: 640,
                height: 480,
            },
            min_size: None,
            resizable: true,
        },
        None,
    );

    assert!(!settings.resizable);
}

pub(super) fn fake_editor_parent() -> EditorParent {
    EditorParent {
        window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
            iced::window::raw_window_handle::AppKitWindowHandle::new(std::ptr::NonNull::<
                std::ffi::c_void,
            >::dangling()),
        ),
        display: None,
    }
}

pub(super) fn attach_recording_editor(
    app: &mut Lilypalooza,
    target: EditorTarget,
    detached: Rc<RefCell<usize>>,
) {
    let window_id = iced::window::Id::unique();
    app.processor_editor_windows.begin_open(
        target,
        "Editor".to_string(),
        true,
        Box::new(RecordingEditorSession {
            visible: Rc::new(RefCell::new(Vec::new())),
            detached,
        }),
        window_id,
    );
    app.processor_editor_windows
        .attach(window_id, None, fake_editor_parent())
        .expect("attach should succeed");
}

#[test]
pub(super) fn mixer_drag_value_changes_use_gesture_history() {
    assert_eq!(
        mixer_message_history_mode(&MixerMessage::SetTrackGain(0, -3.0), true),
        MixerHistoryMode::Gesture
    );
    assert_eq!(
        mixer_message_history_mode(&MixerMessage::SetTrackPan(0, 0.25), true),
        MixerHistoryMode::Gesture
    );
}

#[test]
pub(super) fn mixer_discrete_value_changes_use_immediate_history() {
    assert_eq!(
        mixer_message_history_mode(&MixerMessage::SetTrackGain(0, -3.0), false),
        MixerHistoryMode::Immediate
    );
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    assert_eq!(
        mixer_message_history_mode(
            &MixerMessage::SelectProcessor(target, crate::app::mixer::ProcessorChoice::None,),
            false
        ),
        MixerHistoryMode::Immediate
    );
    assert_eq!(
        mixer_message_history_mode(&MixerMessage::ToggleProcessorBrowser(target), false),
        MixerHistoryMode::None
    );
}

#[test]
pub(super) fn mixer_meter_resets_do_not_record_history() {
    assert_eq!(
        mixer_message_history_mode(&MixerMessage::ResetTrackMeter(0), false),
        MixerHistoryMode::None
    );
}

#[test]
pub(super) fn editor_window_title_uses_track_name_only_for_instrument_slot() {
    lilypalooza_builtins::register_all();
    let app = test_app();
    let slot = lilypalooza_audio::SlotState::built_in(
        lilypalooza_audio::BUILTIN_SOUNDFONT_ID,
        lilypalooza_audio::ProcessorState::default(),
    );

    assert_eq!(app.editor_window_title("Violin", &slot, 0), "Violin");
}

#[test]
pub(super) fn toggle_mixer_effect_rack_opens_and_closes_track_panel() {
    let mut app = test_app();

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleMixerEffectRack(3));
    assert_eq!(app.open_mixer_effect_rack_tracks, vec![3]);

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleMixerEffectRack(5));
    assert_eq!(app.open_mixer_effect_rack_tracks, vec![3, 5]);

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleMixerEffectRack(3));
    assert_eq!(app.open_mixer_effect_rack_tracks, vec![5]);
}

#[test]
pub(super) fn track_rename_commits_on_focus_loss() {
    let mut app = test_app();
    app.renaming_target = Some(RenameTarget::Track(0));
    app.track_rename_was_focused = true;
    app.track_rename_value = "Lead".into();

    let _discarded = app.handle_track_rename_focus_changed(false);

    assert_eq!(app.track_name_override(0), Some("Lead"));
    assert!(app.renaming_target.is_none());
    assert!(app.track_rename_value.is_empty());
}

#[test]
pub(super) fn remove_bus_message_removes_bus() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );

    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_id = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .buses()
        .first()
        .and_then(|bus| bus.bus_id)
        .expect("bus should be added");

    let _discarded = app.handle_mixer_message(MixerMessage::RemoveBus(bus_id.0));

    app.playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .bus(BusId(bus_id.0))
        .expect("bus should exist before confirmation");
    assert_eq!(
        app.error_prompt.as_ref().map(|prompt| prompt.title()),
        Some("Remove Bus 1?")
    );

    let _discarded = app.handle_prompt_message(PromptMessage::Acknowledge);

    app.playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .bus(BusId(bus_id.0))
        .expect_err("bus should be removed after confirmation");
}

#[test]
pub(super) fn canceling_remove_bus_prompt_keeps_bus() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );

    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_id = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .buses()
        .first()
        .and_then(|bus| bus.bus_id)
        .expect("bus should be added");

    let _discarded = app.handle_mixer_message(MixerMessage::RemoveBus(bus_id.0));
    let _discarded = app.handle_prompt_message(PromptMessage::Cancel);

    assert!(app.error_prompt.is_none());
    app.playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .bus(BusId(bus_id.0))
        .expect("bus should remain after cancellation");
}

#[test]
pub(super) fn adding_bus_keeps_open_processor_editor_session() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let detached = Rc::new(RefCell::new(0));
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    attach_recording_editor(&mut app, target, Rc::clone(&detached));

    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);

    assert_eq!(*detached.borrow(), 0);
    assert!(app.processor_editor_windows.contains_window(target));
}

#[test]
pub(super) fn removing_bus_detaches_only_removed_bus_and_reindexes_later_bus_editors() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let (first_bus_id, track_count) = {
        let mixer = app
            .playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state();
        (
            mixer
                .buses()
                .first()
                .and_then(|bus| bus.bus_id)
                .expect("first bus should exist")
                .0,
            mixer.track_count(),
        )
    };
    let track_target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let removed_bus_target = EditorTarget {
        strip_index: 1 + track_count,
        slot_index: 1,
    };
    let later_bus_target = EditorTarget {
        strip_index: 1 + track_count + 1,
        slot_index: 1,
    };
    let reindexed_later_bus_target = EditorTarget {
        strip_index: 1 + track_count,
        slot_index: 1,
    };
    let track_detached = Rc::new(RefCell::new(0));
    let removed_bus_detached = Rc::new(RefCell::new(0));
    let later_bus_detached = Rc::new(RefCell::new(0));
    attach_recording_editor(&mut app, track_target, Rc::clone(&track_detached));
    attach_recording_editor(
        &mut app,
        removed_bus_target,
        Rc::clone(&removed_bus_detached),
    );
    attach_recording_editor(&mut app, later_bus_target, Rc::clone(&later_bus_detached));

    let _discarded = app.remove_bus_confirmed(first_bus_id);

    assert_eq!(*track_detached.borrow(), 0);
    assert_eq!(*removed_bus_detached.borrow(), 1);
    assert_eq!(*later_bus_detached.borrow(), 0);
    assert!(app.processor_editor_windows.contains_window(track_target));
    assert!(
        app.processor_editor_windows
            .contains_window(reindexed_later_bus_target)
    );
    assert!(
        !app.processor_editor_windows
            .contains_window(later_bus_target)
    );
}

#[test]
pub(super) fn track_instrument_without_editor_does_not_open_processor_window() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );

    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let _discarded = app.handle_mixer_message(MixerMessage::OpenEditor(target));

    assert!(!app.processor_editor_windows.contains_window(target));
}

#[test]
pub(super) fn processor_browser_toggle_opens_and_closes_same_track_instrument() {
    let mut app = test_app();
    let target = EditorTarget {
        strip_index: 4,
        slot_index: 0,
    };

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleProcessorBrowser(target));

    assert_eq!(app.open_instrument_browser_track, Some(3));
    assert_eq!(app.open_processor_browser_target, Some(target));
    assert!(app.instrument_browser_search.is_empty());

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleProcessorBrowser(target));

    assert_eq!(app.open_instrument_browser_track, None);
    assert_eq!(app.open_processor_browser_target, None);
}

#[test]
pub(super) fn processor_browser_opens_for_master_effect_without_track_underflow() {
    let mut app = test_app();
    let target = EditorTarget {
        strip_index: 0,
        slot_index: 1,
    };

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleProcessorBrowser(target));

    assert_eq!(app.open_processor_browser_target, Some(target));
    assert_eq!(app.open_instrument_browser_track, None);
}

#[test]
pub(super) fn processor_browser_section_toggle_expands_and_collapses_for_session() {
    let mut app = test_app();
    let key = crate::app::mixer::ProcessorBrowserSectionKey::new(
        crate::app::mixer::ProcessorSlotRole::Effect,
        crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        "Utility".to_string(),
    );

    let _discarded =
        app.handle_mixer_message(MixerMessage::ToggleProcessorBrowserSection(key.clone()));
    assert_eq!(app.processor_browser_expanded_sections, vec![key.clone()]);

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleProcessorBrowserSection(key));
    assert!(app.processor_browser_expanded_sections.is_empty());
}

#[test]
pub(super) fn selecting_track_instrument_closes_open_browser() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.open_processor_browser_target = Some(EditorTarget {
        strip_index: 1,
        slot_index: 0,
    });
    app.open_instrument_browser_track = Some(0);
    app.instrument_browser_search = "piano".into();

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        EditorTarget {
            strip_index: 1,
            slot_index: 0,
        },
        crate::app::mixer::ProcessorChoice::None,
    ));

    assert_eq!(app.open_instrument_browser_track, None);
    assert_eq!(app.open_processor_browser_target, None);
    assert!(app.instrument_browser_search.is_empty());
}

#[test]
pub(super) fn selecting_track_effect_adds_effect_slot() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        EditorTarget {
            strip_index: 1,
            slot_index: 1,
        },
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_GAIN_ID.to_string(),
            name: "Gain".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");
    assert_eq!(track.effect_count(), 1);
    assert!(matches!(
        &track.effect(0).expect("effect slot should exist").kind,
        lilypalooza_audio::ProcessorKind::BuiltIn { processor_id }
            if processor_id == lilypalooza_audio::BUILTIN_GAIN_ID
    ));
}

#[test]
pub(super) fn selecting_track_effect_uses_lowest_free_duplicate_label_index() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    let mut playback = AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
        .expect("test audio engine should start");
    let mut first = SlotState::built_in(
        lilypalooza_audio::BUILTIN_GAIN_ID,
        lilypalooza_audio::ProcessorState::default(),
    );
    first.instance_label_index = 1;
    let mut third = SlotState::built_in(
        lilypalooza_audio::BUILTIN_GAIN_ID,
        lilypalooza_audio::ProcessorState::default(),
    );
    third.instance_label_index = 3;
    playback
        .mixer()
        .set_track_effects(TrackId(0), vec![first, third])
        .expect("effects should be installed");
    app.playback = Some(playback);

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        EditorTarget {
            strip_index: 1,
            slot_index: 3,
        },
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_GAIN_ID.to_string(),
            name: "Gain".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");

    assert_eq!(
        track
            .effect(2)
            .expect("new effect should exist")
            .instance_label_index,
        2
    );
}

#[test]
pub(super) fn selecting_master_effect_adds_effect_slot() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        EditorTarget {
            strip_index: 0,
            slot_index: 1,
        },
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_GAIN_ID.to_string(),
            name: "Gain".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));

    assert_eq!(
        app.playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .master()
            .effect_count(),
        1
    );
}

#[test]
pub(super) fn selecting_bus_effect_adds_effect_slot() {
    lilypalooza_builtins::register_all();
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);
    let bus_strip_index = {
        let mixer = app
            .playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state();
        1 + mixer.track_count()
    };

    let _discarded = app.handle_mixer_message(MixerMessage::SelectProcessor(
        EditorTarget {
            strip_index: bus_strip_index,
            slot_index: 1,
        },
        crate::app::mixer::ProcessorChoice::Processor {
            processor_id: lilypalooza_audio::BUILTIN_GAIN_ID.to_string(),
            name: "Gain".to_string(),
            backend: crate::app::mixer::ProcessorBrowserBackend::BuiltIn,
        },
    ));

    assert_eq!(
        app.playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .buses()
            .first()
            .expect("bus should exist")
            .effect_count(),
        1
    );
}

#[test]
pub(super) fn toggling_effect_slot_bypass_updates_slot_state() {
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
                lilypalooza_audio::ProcessorState::default(),
            )],
        )
        .expect("effect should be installed");
    app.playback = Some(playback);

    let _discarded = app.handle_mixer_message(MixerMessage::ToggleSlotBypass(EditorTarget {
        strip_index: 1,
        slot_index: 1,
    }));

    assert!(
        app.playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .track(TrackId(0))
            .expect("track should exist")
            .effect(0)
            .expect("effect should exist")
            .bypassed
    );
}

#[test]
pub(super) fn moving_track_effect_reorders_effect_slots() {
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
    playback
        .mixer()
        .set_track_effects(TrackId(0), vec![first.clone(), second.clone()])
        .expect("effects should be installed");
    app.playback = Some(playback);

    let _discarded = app.handle_mixer_message(MixerMessage::MoveTrackEffect {
        strip_index: 1,
        from_effect_index: 0,
        to_effect_index: 1,
    });

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");
    assert_eq!(track.effect(0), Some(&second));
    assert_eq!(track.effect(1), Some(&first));
}

#[test]
pub(super) fn dropping_dragged_track_effect_reorders_effect_slots() {
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
    playback
        .mixer()
        .set_track_effects(TrackId(0), vec![first.clone(), second.clone()])
        .expect("effects should be installed");
    app.playback = Some(playback);

    let _discarded = app.handle_mixer_message(MixerMessage::StartTrackEffectDrag {
        strip_index: 1,
        effect_index: 0,
    });
    let _discarded = app.handle_mixer_message(MixerMessage::DropTrackEffect {
        strip_index: 1,
        effect_index: 1,
    });

    let track = app
        .playback
        .as_ref()
        .expect("playback should exist")
        .mixer_state()
        .track(TrackId(0))
        .expect("track should exist");
    assert_eq!(track.effect(0), Some(&second));
    assert_eq!(track.effect(1), Some(&first));
    assert_eq!(app.effect_drag_source, None);
}
