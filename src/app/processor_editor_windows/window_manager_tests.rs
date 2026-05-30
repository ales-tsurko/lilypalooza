use std::{
    cell::RefCell,
    ptr::NonNull,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use editor_host::WindowSnapshot;
use iced::window;
use lilypalooza_audio::{EditorError, EditorParent, EditorSession, EditorSize};

use super::{EditorTarget, EditorWindowManager, snapshot_into_editor_parent};

struct FakeEditorSession;
struct RequestedSizeEditorSession {
    calls: Arc<AtomicUsize>,
}
struct ReportingResizableEditorSession {
    resizable: bool,
}
struct AdjustingResizeEditorSession {
    requested: Arc<std::sync::Mutex<Vec<EditorSize>>>,
    accepted: EditorSize,
}
struct RecordingDetachSession {
    events: Rc<RefCell<Vec<&'static str>>>,
}
struct RecordingHost {
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Drop for RecordingHost {
    fn drop(&mut self) {
        self.events.borrow_mut().push("host-drop");
    }
}

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

impl EditorSession for RecordingDetachSession {
    fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        self.events.borrow_mut().push("session-detach");
        Ok(())
    }

    fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        Ok(size)
    }
}

impl EditorSession for RequestedSizeEditorSession {
    fn requested_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        Ok(Some(EditorSize {
            width: 640,
            height: 480,
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

impl EditorSession for ReportingResizableEditorSession {
    fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
        Ok(Some(self.resizable))
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

impl EditorSession for AdjustingResizeEditorSession {
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
        self.requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(size);
        Ok(self.accepted)
    }
}

#[test]
fn processor_editor_window_manager_reuses_existing_target_window() {
    let mut manager = EditorWindowManager::default();
    let target = EditorTarget {
        strip_index: 3,
        slot_index: 0,
    };

    let first_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 4".to_string(),
        true,
        Box::new(FakeEditorSession),
        first_id,
    );
    let second_token = manager.focus_existing(target);

    assert_eq!(Some(first_id), second_token);
    assert_eq!(manager.focused, Some(target));
}

#[test]
fn processor_editor_window_manager_attaches_pending_session_once_parent_arrives() {
    let mut manager = EditorWindowManager::default();
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };

    let window_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(FakeEditorSession),
        window_id,
    );

    manager
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
    assert!(manager.windows.contains_key(&target));
    assert!(manager.window_visible(window_id));
}

#[test]
fn processor_editor_window_manager_tracks_visibility_for_toggle() {
    let mut manager = EditorWindowManager::default();
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(FakeEditorSession),
        window_id,
    );
    manager
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

    assert!(manager.window_visible(window_id));
    manager.hide_window(window_id).expect("window should hide");
    assert!(!manager.window_visible(window_id));
    assert!(manager.show_window(window_id).is_empty());
    assert!(manager.window_visible(window_id));
}

#[test]
fn processor_editor_window_manager_uses_live_resizable_after_attach() {
    let mut manager = EditorWindowManager::default();
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 1".to_string(),
        false,
        Box::new(ReportingResizableEditorSession { resizable: true }),
        window_id,
    );
    manager
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

    assert_eq!(manager.window_resizable(window_id), Some(true));
}

#[test]
fn processor_editor_window_manager_polls_requested_editor_resize() {
    let mut manager = EditorWindowManager::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let window_id = window::Id::unique();
    manager.begin_open(
        EditorTarget {
            strip_index: 1,
            slot_index: 0,
        },
        "Track 1".to_string(),
        true,
        Box::new(RequestedSizeEditorSession {
            calls: Arc::clone(&calls),
        }),
        window_id,
    );
    manager
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

    let mut errors = Vec::new();
    manager.apply_requested_content_resizes(|error| errors.push(error));

    assert!(errors.is_empty());
    assert_eq!(calls.load(Ordering::Acquire), 1);
}

#[test]
fn processor_editor_window_manager_skips_requested_editor_resize_while_controls_visible() {
    let mut manager = EditorWindowManager::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let window_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 1".to_string(),
        true,
        Box::new(RequestedSizeEditorSession {
            calls: Arc::clone(&calls),
        }),
        window_id,
    );
    manager
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
    manager.set_controls_visible(target, true);

    let mut errors = Vec::new();
    manager.apply_requested_content_resizes(|error| errors.push(error));

    assert!(errors.is_empty());
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn processor_editor_resize_negotiation_uses_session_accepted_size() {
    let requested = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut session = AdjustingResizeEditorSession {
        requested: Arc::clone(&requested),
        accepted: EditorSize {
            width: 512,
            height: 384,
        },
    };

    let accepted = super::negotiate_editor_content_resize(
        &mut session,
        editor_host::Size {
            width: 640.0,
            height: 480.0,
        },
    )
    .expect("resize should be accepted");

    assert_eq!(
        *requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![EditorSize {
            width: 640,
            height: 480,
        }]
    );
    assert_eq!(
        accepted,
        editor_host::Size {
            width: 512.0,
            height: 384.0,
        }
    );
}

#[test]
fn startup_programmatic_outer_resize_echoes_are_consumed() {
    let pending = Arc::new(super::ProgrammaticOuterResizeEchoes::new());
    super::record_programmatic_outer_resize_size(
        &pending,
        editor_host::Size {
            width: 644.0,
            height: 518.0,
        },
    );
    super::record_programmatic_outer_resize_size(
        &pending,
        editor_host::Size {
            width: 404.0,
            height: 438.0,
        },
    );

    assert!(super::consume_pending_programmatic_outer_resize(
        &pending,
        editor_host::Size {
            width: 644.25,
            height: 517.75,
        },
    ));
    assert!(super::consume_pending_programmatic_outer_resize(
        &pending,
        editor_host::Size {
            width: 404.25,
            height: 437.75,
        },
    ));
    assert!(!super::consume_pending_programmatic_outer_resize(
        &pending,
        editor_host::Size {
            width: 644.0,
            height: 749.0,
        },
    ));
}

#[test]
fn programmatic_outer_resize_echoes_do_not_return_torn_sizes() {
    let pending = Arc::new(super::ProgrammaticOuterResizeEchoes::new());
    let mut writers = Vec::new();

    for writer in 0..4 {
        let pending = Arc::clone(&pending);
        writers.push(std::thread::spawn(move || {
            for index in 0..2_000 {
                let width = f64::from(writer * 10_000 + index);
                super::record_programmatic_outer_resize_size(
                    &pending,
                    editor_host::Size {
                        width,
                        height: width + 34.0,
                    },
                );
            }
        }));
    }

    for writer in writers {
        writer.join().expect("writer should not panic");
    }

    for width in 0..42_000 {
        let width = f64::from(width);
        assert!(!super::consume_pending_programmatic_outer_resize(
            &pending,
            editor_host::Size {
                width,
                height: width + 33.0,
            },
        ));
    }
}

#[test]
fn same_pending_outer_resize_keeps_existing_deferred_deadline() {
    let now = Instant::now();
    let previous_deadline = now + Duration::from_secs(1);
    let outer_size = editor_host::Size {
        width: 414.0,
        height: 395.0,
    };

    assert_eq!(
        super::next_deferred_outer_resize_deadline(
            Some(outer_size),
            outer_size,
            Some(previous_deadline),
            now,
        ),
        previous_deadline
    );
}

#[test]
fn changed_pending_outer_resize_refreshes_deferred_deadline() {
    let now = Instant::now();
    let previous_deadline = now + Duration::from_secs(1);

    assert_eq!(
        super::next_deferred_outer_resize_deadline(
            Some(editor_host::Size {
                width: 414.0,
                height: 395.0,
            }),
            editor_host::Size {
                width: 500.0,
                height: 500.0,
            },
            Some(previous_deadline),
            now,
        ),
        now + super::RESIZE_IDLE_TIMEOUT
    );
}

#[test]
fn same_pending_zoom_keeps_existing_deferred_deadline() {
    let now = Instant::now();
    let previous_deadline = now + Duration::from_secs(1);

    assert_eq!(
        super::next_deferred_zoom_deadline(Some(137), 137, Some(previous_deadline), now),
        previous_deadline
    );
}

#[test]
fn changed_pending_zoom_refreshes_deferred_deadline() {
    let now = Instant::now();
    let previous_deadline = now + Duration::from_secs(1);

    assert_eq!(
        super::next_deferred_zoom_deadline(Some(137), 138, Some(previous_deadline), now),
        now + super::RESIZE_IDLE_TIMEOUT
    );
}

#[test]
fn same_host_size_treats_subpixel_resize_as_noop() {
    assert!(super::same_host_size(
        editor_host::Size {
            width: 640.0,
            height: 480.0,
        },
        editor_host::Size {
            width: 640.25,
            height: 479.75,
        },
    ));
    assert!(!super::same_host_size(
        editor_host::Size {
            width: 640.0,
            height: 480.0,
        },
        editor_host::Size {
            width: 641.0,
            height: 480.0,
        },
    ));
}

#[test]
fn aspect_preserved_resize_uses_dominant_drag_axis() {
    let current = editor_host::Size {
        width: 640.0,
        height: 480.0,
    };

    assert_eq!(
        super::aspect_preserved_resize(
            current,
            editor_host::Size {
                width: 640.0,
                height: 540.0,
            },
            640.0 / 480.0,
        ),
        editor_host::Size {
            width: 720.0,
            height: 540.0,
        }
    );
    assert_eq!(
        super::aspect_preserved_resize(
            current,
            editor_host::Size {
                width: 720.0,
                height: 480.0,
            },
            640.0 / 480.0,
        ),
        editor_host::Size {
            width: 720.0,
            height: 540.0,
        }
    );
}

#[test]
fn editor_zoom_size_uses_default_content_size() {
    assert_eq!(
        super::zoomed_content_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            150,
        ),
        editor_host::Size {
            width: 960.0,
            height: 720.0,
        }
    );
}

#[test]
fn editor_zoom_size_allows_minimum_scale() {
    assert_eq!(
        super::zoomed_content_size(
            editor_host::Size {
                width: 400.0,
                height: 300.0,
            },
            50,
        ),
        editor_host::Size {
            width: 200.0,
            height: 150.0,
        }
    );
}

#[test]
fn plugin_owned_resize_uses_attached_editor_size_as_zoom_baseline() {
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
        ),
        100
    );
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            },
        ),
        160
    );
}

#[test]
fn startup_baseline_uses_actual_embedded_plugin_size_after_attach() {
    assert_eq!(
        super::attached_start_content_size(
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
            Some(editor_host::Size {
                width: 640.0,
                height: 640.0,
            }),
            Some(EditorSize {
                width: 400,
                height: 400,
            }),
        ),
        editor_host::Size {
            width: 640.0,
            height: 640.0,
        }
    );
    assert_eq!(
        super::attached_baseline_content_size(
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
            Some(editor_host::Size {
                width: 640.0,
                height: 640.0,
            }),
        ),
        editor_host::Size {
            width: 640.0,
            height: 640.0,
        }
    );
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            },
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            },
        ),
        100
    );
}

#[test]
fn startup_size_falls_back_to_session_initial_size_without_embedded_view() {
    assert_eq!(
        super::attached_start_content_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            None,
            Some(EditorSize {
                width: 400,
                height: 300,
            }),
        ),
        editor_host::Size {
            width: 400.0,
            height: 300.0,
        }
    );
}

#[test]
fn first_embedded_size_after_attach_becomes_startup_baseline() {
    assert_eq!(
        super::startup_embedded_baseline_size(
            true,
            Some(editor_host::Size {
                width: 640.0,
                height: 640.0,
            }),
        ),
        Some(editor_host::Size {
            width: 640.0,
            height: 640.0,
        })
    );
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            },
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            },
        ),
        100
    );
}

#[test]
fn startup_baseline_waits_for_real_embedded_size() {
    assert_eq!(super::startup_embedded_baseline_size(true, None), None);
    assert_eq!(
        super::startup_embedded_baseline_size(
            false,
            Some(editor_host::Size {
                width: 800.0,
                height: 600.0,
            }),
        ),
        None
    );
}

#[test]
fn first_host_resize_callback_after_attach_becomes_startup_baseline() {
    let pending = AtomicBool::new(true);
    let base = super::SharedContentSize::new(editor_host::Size {
        width: 400.0,
        height: 400.0,
    });
    let requested = editor_host::Size {
        width: 640.0,
        height: 640.0,
    };

    assert!(super::adopt_startup_resize_baseline(
        &pending, &base, requested
    ));
    assert_eq!(base.load(), requested);
    assert_eq!(
        super::zoom_percent_for_content_size(base.load(), requested),
        100
    );
    assert!(!super::adopt_startup_resize_baseline(
        &pending,
        &base,
        editor_host::Size {
            width: 800.0,
            height: 800.0,
        },
    ));
}

#[test]
fn old_host_default_baseline_would_show_wrong_initial_plugin_zoom() {
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            editor_host::Size {
                width: 400.0,
                height: 400.0,
            },
        ),
        73
    );
}

#[test]
fn plugin_owned_resize_updates_zoom_percent_from_baseline() {
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            editor_host::Size {
                width: 800.0,
                height: 600.0,
            },
        ),
        125
    );
    assert_eq!(
        super::zoom_percent_for_content_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            editor_host::Size {
                width: 320.0,
                height: 240.0,
            },
        ),
        50
    );
}

#[test]
fn observed_native_content_size_prefers_embedded_plugin_view() {
    assert_eq!(
        super::observed_native_content_size(
            Some(editor_host::Size {
                width: 800.0,
                height: 600.0,
            }),
            Some(editor_host::Size {
                width: 640.0,
                height: 480.0,
            }),
        ),
        Some(editor_host::Size {
            width: 800.0,
            height: 600.0,
        })
    );
    assert_eq!(
        super::observed_native_content_size(
            None,
            Some(editor_host::Size {
                width: 640.0,
                height: 480.0,
            }),
        ),
        Some(editor_host::Size {
            width: 640.0,
            height: 480.0,
        })
    );
}

#[test]
fn native_plugin_resize_uses_plugin_size_without_aspect_correction() {
    assert_eq!(
        super::native_content_resize_request(editor_host::Size {
            width: 900.0,
            height: 500.0,
        }),
        editor_host::Size {
            width: 900.0,
            height: 500.0,
        }
    );
}

#[test]
fn native_plugin_resize_is_polled_even_when_plugin_reports_fixed_size() {
    assert!(super::should_sync_native_content_resize(true, true));
    assert!(!super::should_sync_native_content_resize(false, true));
    assert!(!super::should_sync_native_content_resize(true, false));
}

#[test]
fn outer_writeback_is_required_when_aspect_corrected_outer_differs_from_native_outer() {
    assert!(super::needs_outer_writeback(
        editor_host::Size {
            width: 430.0,
            height: 442.0,
        },
        editor_host::Size {
            width: 430.0,
            height: 464.0,
        },
    ));
    assert!(!super::needs_outer_writeback(
        editor_host::Size {
            width: 430.0,
            height: 464.0,
        },
        editor_host::Size {
            width: 430.25,
            height: 463.75,
        },
    ));
}

#[test]
fn resize_trace_ids_are_monotonic() {
    let mut manager = EditorWindowManager::default();

    assert_eq!(
        manager.next_resize_trace_id(),
        super::EditorResizeTraceId(1)
    );
    assert_eq!(
        manager.next_resize_trace_id(),
        super::EditorResizeTraceId(2)
    );
}

#[test]
fn resize_trace_log_labels_source_stage_and_sizes() {
    let window_id = window::Id::unique();
    let message = super::format_resize_trace_event(super::EditorResizeTraceEvent {
        id: super::EditorResizeTraceId(7),
        source: super::EditorResizeSource::IcedOuterEvent,
        stage: super::EditorResizeStage::Accepted,
        target: EditorTarget {
            strip_index: 1,
            slot_index: 2,
        },
        window_id: Some(window_id),
        current_content: Some(editor_host::Size {
            width: 640.0,
            height: 480.0,
        }),
        requested_content: Some(editor_host::Size {
            width: 800.0,
            height: 600.0,
        }),
        accepted_content: Some(editor_host::Size {
            width: 768.0,
            height: 576.0,
        }),
        outer_size: None,
        note: Some("plugin adjusted"),
    });

    assert!(message.contains("resize#7"));
    assert!(message.contains("source=iced-outer-event"));
    assert!(message.contains("stage=accepted"));
    assert!(message.contains("target=EditorTarget"));
    assert!(message.contains("current=640x480"));
    assert!(message.contains("requested=800x600"));
    assert!(message.contains("accepted=768x576"));
    assert!(message.contains("note=plugin adjusted"));
}

#[test]
fn processor_editor_window_manager_updates_titles_for_open_and_pending_windows() {
    let mut manager = EditorWindowManager::default();
    let open_target = EditorTarget {
        strip_index: 1,
        slot_index: 0,
    };
    let pending_target = EditorTarget {
        strip_index: 1,
        slot_index: 1,
    };
    let open_id = window::Id::unique();
    let pending_id = window::Id::unique();
    manager.begin_open(
        open_target,
        "Old".to_string(),
        true,
        Box::new(FakeEditorSession),
        open_id,
    );
    manager
        .attach(
            open_id,
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
    manager.begin_open(
        pending_target,
        "Old pending".to_string(),
        true,
        Box::new(FakeEditorSession),
        pending_id,
    );

    assert!(
        manager
            .set_window_title(open_target, "New".to_string())
            .is_empty()
    );
    assert!(
        manager
            .set_window_title(pending_target, "New pending".to_string())
            .is_empty()
    );

    assert_eq!(manager.window_title(open_id), Some("New"));
    assert_eq!(manager.window_title(pending_id), Some("New pending"));
}

#[test]
fn editor_parent_snapshot_roundtrips_appkit_window_handle() {
    let snapshot = WindowSnapshot::capture(
        iced::window::raw_window_handle::RawWindowHandle::AppKit(
            iced::window::raw_window_handle::AppKitWindowHandle::new(
                NonNull::<std::ffi::c_void>::dangling(),
            ),
        ),
        Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(
            iced::window::raw_window_handle::AppKitDisplayHandle::new(),
        )),
    )
    .expect("snapshot should capture appkit");

    let parent = snapshot_into_editor_parent(snapshot).expect("snapshot should restore appkit");

    assert!(matches!(
        parent.window,
        iced::window::raw_window_handle::RawWindowHandle::AppKit(_)
    ));
    assert!(matches!(
        parent.display,
        Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(_))
    ));
}

#[test]
fn processor_editor_window_manager_removes_window_by_host_id() {
    let mut manager = EditorWindowManager::default();
    let target = EditorTarget {
        strip_index: 2,
        slot_index: 1,
    };
    let window_id = window::Id::unique();
    manager.begin_open(
        target,
        "Track 2".to_string(),
        true,
        Box::new(FakeEditorSession),
        window_id,
    );
    manager
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

    let removed = manager.remove_window(window_id);

    assert!(removed.is_some());
    assert!(!manager.windows.contains_key(&target));
}

#[test]
fn removed_editor_detaches_session_before_dropping_host() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut session = RecordingDetachSession {
        events: Rc::clone(&events),
    };

    super::detach_session_before_dropping_host(
        &mut session,
        Some(RecordingHost {
            events: Rc::clone(&events),
        }),
    )
    .expect("detach should succeed");

    assert_eq!(&*events.borrow(), &["session-detach", "host-drop"]);
}

#[test]
fn processor_editor_window_manager_moves_effect_slot_targets_with_reorder() {
    let mut manager = EditorWindowManager::default();
    let targets = [
        EditorTarget {
            strip_index: 2,
            slot_index: 1,
        },
        EditorTarget {
            strip_index: 2,
            slot_index: 2,
        },
        EditorTarget {
            strip_index: 2,
            slot_index: 3,
        },
    ];
    for target in targets {
        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            format!("Slot {}", target.slot_index),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );
        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");
    }

    manager.move_slot_targets_within_strip(2, 1, 3);

    assert!(manager.windows.contains_key(&EditorTarget {
        strip_index: 2,
        slot_index: 1,
    }));
    assert!(manager.windows.contains_key(&EditorTarget {
        strip_index: 2,
        slot_index: 2,
    }));
    assert!(manager.windows.contains_key(&EditorTarget {
        strip_index: 2,
        slot_index: 3,
    }));
    assert_eq!(
        manager
            .windows
            .get(&EditorTarget {
                strip_index: 2,
                slot_index: 3,
            })
            .map(|window| window.title.as_str()),
        Some("Slot 1")
    );
}
