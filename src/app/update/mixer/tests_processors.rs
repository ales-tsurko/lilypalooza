use std::{cell::RefCell, rc::Rc};

use iced::Size;
use lilypalooza_audio::{AudioEngine, AudioEngineOptions, EditorSize, MixerState};

use super::{tests_editor_windows::*, *};

#[test]
pub(super) fn processor_editor_window_settings_use_session_reported_content_size() {
    let descriptor = lilypalooza_audio::EditorDescriptor {
        default_size: EditorSize {
            width: 720,
            height: 480,
        },
        min_size: Some(EditorSize {
            width: 320,
            height: 220,
        }),
        resizable: true,
    };

    let settings = super::processor_editor_window_settings(
        descriptor,
        Some(EditorSize {
            width: 936,
            height: 612,
        }),
    );

    assert_eq!(settings.size, Size::new(936.0, 612.0));
    assert_eq!(settings.min_size, Some(Size::new(320.0, 220.0)));
    assert!(!settings.decorations);
}

#[test]
pub(super) fn open_processor_editor_defers_initial_size_until_parent_attach() {
    let mut app = test_app();
    let calls = Rc::new(RefCell::new(0));

    let _discarded = app.open_editor(
        EditorTarget {
            strip_index: 1,
            slot_index: 0,
        },
        "Track 1".to_string(),
        Some(lilypalooza_audio::EditorDescriptor {
            default_size: EditorSize {
                width: 720,
                height: 480,
            },
            min_size: None,
            resizable: true,
        }),
        Ok(Some(Box::new(InitialSizeEditorSession {
            calls: Rc::clone(&calls),
        }))),
        crate::app::processor_editor_windows::empty_editor_controller(),
    );

    assert_eq!(*calls.borrow(), 0);
}

#[test]
pub(super) fn open_processor_editor_without_native_session_opens_generic_controls() {
    let mut app = test_app();
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };

    let _discarded = app.open_editor(
        target,
        "Track 1".to_string(),
        None,
        Ok(None),
        crate::app::processor_editor_windows::empty_editor_controller(),
    );

    assert_eq!(
        app.processor_editor_windows.editor_view_state(target),
        Some((false, true))
    );
}

#[test]
pub(super) fn processor_editor_window_settings_can_disable_resizing() {
    let descriptor = lilypalooza_audio::EditorDescriptor {
        default_size: EditorSize {
            width: 720,
            height: 480,
        },
        min_size: None,
        resizable: false,
    };

    let settings = super::processor_editor_window_settings(descriptor, None);

    assert!(!settings.resizable);
}

#[test]
pub(super) fn main_window_close_request_hides_editors_before_prompt() {
    let mut app = test_app();
    app.playback = Some(
        AudioEngine::start_test(MixerState::new(), AudioEngineOptions::default())
            .expect("test audio engine should start"),
    );
    app.saved_project_state = Some(app.current_project_state());
    let _discarded = app.handle_mixer_message(MixerMessage::AddBus);

    let visible = Rc::new(RefCell::new(Vec::new()));
    let detached = Rc::new(RefCell::new(0));
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = iced::window::Id::unique();
    app.processor_editor_windows.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(RecordingEditorSession {
            visible: Rc::clone(&visible),
            detached: Rc::clone(&detached),
        }),
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

    let _discarded = app.handle_window_close_requested(app.main_window_id);

    assert_eq!(*visible.borrow(), vec![false]);
    assert_eq!(*detached.borrow(), 0);
    assert!(app.processor_editor_windows.contains_window(target));
    assert!(matches!(
        app.pending_editor_action,
        Some(crate::app::PendingEditorAction::ResolveDirtyProject {
            continuation: crate::app::EditorContinuation::ExitApp
        })
    ));
}

#[test]
pub(super) fn exit_app_detaches_editor_sessions() {
    let mut app = test_app();
    let visible = Rc::new(RefCell::new(Vec::new()));
    let detached = Rc::new(RefCell::new(0));
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = iced::window::Id::unique();
    app.processor_editor_windows.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(RecordingEditorSession {
            visible,
            detached: Rc::clone(&detached),
        }),
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

    let _discarded = app.exit_app();

    assert_eq!(*detached.borrow(), 1);
    assert!(!app.processor_editor_windows.contains_window(target));
}
