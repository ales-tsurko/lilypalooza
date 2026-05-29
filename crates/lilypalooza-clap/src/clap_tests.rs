use std::{ptr::NonNull, sync::atomic::AtomicUsize};

use raw_window_handle::{AppKitWindowHandle, RawWindowHandle};

use super::{probe::*, processor_editor::*, runtime::*, *};

#[test]
fn clap_candidate_detection_is_extension_based() {
    assert!(is_clap_candidate(Path::new("Plugin.clap")));
    assert!(is_clap_candidate(Path::new("Plugin.CLAP")));
    assert!(!is_clap_candidate(Path::new("Plugin.vst3")));
}

#[test]
fn candidate_paths_recurses_into_subdirectories() {
    let dir = tempfile::tempdir().expect("temp dir");
    let nested = dir.path().join("Vendor").join("Plugin.clap");
    std::fs::create_dir_all(&nested).expect("nested clap dir");
    std::fs::write(dir.path().join("Root.clap"), "").expect("root clap file");

    let candidates = candidate_paths(dir.path()).expect("candidate paths");

    assert_eq!(candidates, vec![dir.path().join("Root.clap"), nested]);
}

#[test]
fn stable_processor_id_includes_path_and_clap_id() {
    assert_eq!(
        stable_processor_id(Path::new("/Plug/Test.clap"), "org.test"),
        "clap:/Plug/Test.clap#org.test"
    );
}

#[test]
fn validation_report_serializes_structured_success() {
    let report = ValidationReport {
        format: FORMAT.to_string(),
        path: PathBuf::from("/Plug/Test.clap"),
        result: Ok(vec![ClapPluginMetadata {
            processor_id: "clap:/Plug/Test.clap#org.test".to_string(),
            clap_id: "org.test".to_string(),
            name: "Test".to_string(),
            vendor: Some("Vendor".to_string()),
            version: Some("1.0".to_string()),
            features: vec!["audio-effect".to_string()],
            role: registry::Role::Effect,
            path: PathBuf::from("/Plug/Test.clap"),
            library_path: PathBuf::from("/Plug/Test.clap"),
        }]),
    };

    let json = serde_json::to_string(&report).expect("report should serialize");
    let parsed: ValidationReport = serde_json::from_str(&json).expect("report should deserialize");

    assert_eq!(parsed, report);
}

#[test]
fn probe_rejects_factory_without_plugin_descriptors() {
    let error = probe_initialized_factory(
        Path::new("/Plug/Empty.clap"),
        Path::new("/Plug/Empty.clap"),
        empty_factory,
    )
    .expect_err("factory with zero plugins should be invalid");

    assert!(matches!(error, ClapProbeError::NoPluginDescriptors));
}

#[test]
fn clap_gui_reported_size_reads_plugin_native_size() {
    let size = clap_gui_reported_size(
        &SIZED_PLUGIN_GUI,
        NonNull::<clap_plugin>::dangling().as_ptr(),
    )
    .expect("plugin GUI should report size");

    assert_eq!(
        size,
        EditorSize {
            width: 936,
            height: 612,
        }
    );
}

#[test]
fn host_gui_request_resize_records_requested_editor_size() {
    let host = HostContext::new();

    // SAFETY: `host` owns a valid CLAP host pointer for this test.
    assert!(unsafe { host_gui_request_resize(host.as_ptr(), 880, 540) });

    assert_eq!(
        host.take_requested_gui_size(),
        Some(EditorSize {
            width: 880,
            height: 540,
        })
    );
    assert_eq!(host.take_requested_gui_size(), None);
}

struct TestResizeHandler {
    requested: Mutex<Vec<EditorSize>>,
    accepted: EditorSize,
}

impl EditorResizeHandler for TestResizeHandler {
    fn resize_editor(&self, size: EditorSize) -> Result<EditorSize, EditorError> {
        self.requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(size);
        Ok(self.accepted)
    }
}

#[test]
fn host_gui_request_resize_uses_live_resize_handler() {
    let host = HostContext::new();
    let handler = Arc::new(TestResizeHandler {
        requested: Mutex::new(Vec::new()),
        accepted: EditorSize {
            width: 640,
            height: 480,
        },
    });
    host.set_resize_handler(Some(handler.clone()));

    // SAFETY: `host` owns a valid CLAP host pointer for this test.
    assert!(unsafe { host_gui_request_resize(host.as_ptr(), 880, 540) });

    assert_eq!(
        handler
            .requested
            .lock()
            .expect("resize log lock")
            .as_slice(),
        &[EditorSize {
            width: 880,
            height: 540,
        }]
    );
    assert_eq!(
        host.take_requested_gui_size(),
        Some(EditorSize {
            width: 640,
            height: 480,
        })
    );
}

#[test]
fn host_params_request_flush_is_consumed_by_plugin_flush() {
    PARAM_FLUSH_COUNT.store(0, Ordering::Release);
    let host = HostContext::new();
    let params = clap_plugin_params {
        count: None,
        get_info: None,
        get_value: None,
        value_to_text: None,
        text_to_value: None,
        flush: Some(test_params_flush),
    };

    // SAFETY: `host` owns a valid CLAP host pointer for this test.
    unsafe { host_params_request_flush(host.as_ptr()) };

    assert!(clap_flush_params_if_requested(
        &host,
        NonNull::<clap_plugin>::dangling().as_ptr(),
        Some(&params),
    ));
    assert_eq!(PARAM_FLUSH_COUNT.load(Ordering::Acquire), 1);
    assert!(!clap_flush_params_if_requested(
        &host,
        NonNull::<clap_plugin>::dangling().as_ptr(),
        Some(&params),
    ));
}

#[test]
fn clap_midi_queue_expands_all_notes_off_to_active_note_offs() {
    let mut queue = ClapMidiEventQueue::new();
    queue.push(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    queue.push(MidiEvent::NoteOn {
        channel: 0,
        note: 64,
        velocity: 100,
    });
    queue.push(MidiEvent::NoteOn {
        channel: 1,
        note: 67,
        velocity: 100,
    });
    let _ = queue.take();

    queue.push(MidiEvent::AllNotesOff { channel: 0 });
    let data: Vec<_> = queue.take().into_iter().map(|event| event.data).collect();

    assert_eq!(data, vec![[0x80, 60, 0], [0x80, 64, 0], [0xb0, 123, 0]]);

    queue.push(MidiEvent::AllNotesOff { channel: 1 });
    let data: Vec<_> = queue.take().into_iter().map(|event| event.data).collect();

    assert_eq!(data, vec![[0x81, 67, 0], [0xb1, 123, 0]]);
}

#[test]
fn clap_midi_queue_panic_emits_active_note_offs_before_controller_panic() {
    let mut queue = ClapMidiEventQueue::new();
    queue.push(MidiEvent::NoteOn {
        channel: 0,
        note: 60,
        velocity: 100,
    });
    queue.push(MidiEvent::NoteOn {
        channel: 1,
        note: 64,
        velocity: 100,
    });
    let _ = queue.take();

    queue.push_panic();
    let data: Vec<_> = queue.take().into_iter().map(|event| event.data).collect();

    assert_eq!(
        &data[..4],
        &[
            [0x80, 60, 0],
            [0xb0, 120, 0],
            [0xb0, 123, 0],
            [0xb0, 121, 0]
        ]
    );
    assert_eq!(
        &data[4..8],
        &[
            [0x81, 64, 0],
            [0xb1, 120, 0],
            [0xb1, 123, 0],
            [0xb1, 121, 0]
        ]
    );
    assert_eq!(data.len(), 50);
}

#[test]
fn clap_gui_resize_returns_none_when_plugin_cannot_resize() {
    let mut requested = EditorSize {
        width: 936,
        height: 612,
    };

    let resized = clap_gui_adjusted_resize(
        &NON_RESIZABLE_PLUGIN_GUI,
        NonNull::<clap_plugin>::dangling().as_ptr(),
        &mut requested,
    );

    assert!(!resized);
    assert_eq!(
        requested,
        EditorSize {
            width: 936,
            height: 612,
        }
    );
}

#[test]
fn clap_gui_resize_adjusts_before_setting_size() {
    let mut requested = EditorSize {
        width: 935,
        height: 611,
    };

    let resized = clap_gui_adjusted_resize(
        &RESIZABLE_PLUGIN_GUI,
        NonNull::<clap_plugin>::dangling().as_ptr(),
        &mut requested,
    );

    assert!(resized);
    assert_eq!(
        requested,
        EditorSize {
            width: 936,
            height: 612,
        }
    );
}

#[test]
fn clap_gui_destroy_created_does_not_hide_embedded_gui() {
    GUI_DESTROY_COUNT.store(0, Ordering::Release);
    GUI_HIDE_COUNT.store(0, Ordering::Release);

    clap_gui_destroy_created(
        &DESTROY_TRACKING_PLUGIN_GUI,
        NonNull::<clap_plugin>::dangling().as_ptr(),
        true,
    );

    assert_eq!(GUI_DESTROY_COUNT.load(Ordering::Acquire), 1);
    assert_eq!(GUI_HIDE_COUNT.load(Ordering::Acquire), 0);
}

#[test]
fn clap_embedded_visibility_is_host_window_only() {
    clap_gui_set_embedded_visible(false).expect("embedded hide should not touch plugin");
    clap_gui_set_embedded_visible(true).expect("embedded show should not touch plugin");
}

static PARAM_FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);
static GUI_DESTROY_COUNT: AtomicUsize = AtomicUsize::new(0);
static GUI_HIDE_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn test_params_flush(
    _: *const clap_plugin,
    _: *const clap_input_events,
    _: *const clap_output_events,
) {
    PARAM_FLUSH_COUNT.fetch_add(1, Ordering::AcqRel);
}

static SIZED_PLUGIN_GUI: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: None,
    get_preferred_api: None,
    create: None,
    destroy: None,
    set_scale: None,
    get_size: Some(sized_plugin_gui_get_size),
    can_resize: None,
    get_resize_hints: None,
    adjust_size: None,
    set_size: None,
    set_parent: None,
    set_transient: None,
    suggest_title: None,
    show: None,
    hide: None,
};

static NON_RESIZABLE_PLUGIN_GUI: clap_plugin_gui = clap_plugin_gui {
    can_resize: Some(non_resizable_plugin_gui_can_resize),
    set_size: Some(resizable_plugin_gui_set_size),
    ..SIZED_PLUGIN_GUI
};

static RESIZABLE_PLUGIN_GUI: clap_plugin_gui = clap_plugin_gui {
    can_resize: Some(resizable_plugin_gui_can_resize),
    adjust_size: Some(resizable_plugin_gui_adjust_size),
    set_size: Some(resizable_plugin_gui_set_size),
    ..SIZED_PLUGIN_GUI
};

static DESTROY_TRACKING_PLUGIN_GUI: clap_plugin_gui = clap_plugin_gui {
    destroy: Some(destroy_tracking_plugin_gui_destroy),
    hide: Some(destroy_tracking_plugin_gui_hide),
    ..SIZED_PLUGIN_GUI
};

unsafe extern "C" fn non_resizable_plugin_gui_can_resize(_: *const clap_plugin) -> bool {
    false
}

unsafe extern "C" fn resizable_plugin_gui_can_resize(_: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn resizable_plugin_gui_adjust_size(
    _: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    // SAFETY: The test passes valid out-pointers.
    unsafe { *width += 1 };
    // SAFETY: The test passes valid out-pointers.
    unsafe { *height += 1 };
    true
}

unsafe extern "C" fn resizable_plugin_gui_set_size(
    _: *const clap_plugin,
    width: u32,
    height: u32,
) -> bool {
    width == 936 && height == 612
}

unsafe extern "C" fn destroy_tracking_plugin_gui_destroy(_: *const clap_plugin) {
    GUI_DESTROY_COUNT.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn destroy_tracking_plugin_gui_hide(_: *const clap_plugin) -> bool {
    GUI_HIDE_COUNT.fetch_add(1, Ordering::AcqRel);
    true
}

unsafe extern "C" fn sized_plugin_gui_get_size(
    _: *const clap_plugin,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    // SAFETY: The test passes valid out-pointers.
    unsafe { *width = 936 };
    // SAFETY: The test passes valid out-pointers.
    unsafe { *height = 612 };
    true
}

unsafe extern "C" fn empty_factory(factory_id: *const c_char) -> *const c_void {
    if factory_id.is_null() {
        return std::ptr::null();
    }
    // SAFETY: `factory_id` is checked for null and points to the static CLAP factory id.
    let factory_id = unsafe { CStr::from_ptr(factory_id) };
    if factory_id == CLAP_PLUGIN_FACTORY_ID {
        &raw const EMPTY_PLUGIN_FACTORY as *const c_void
    } else {
        std::ptr::null()
    }
}

static EMPTY_PLUGIN_FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(empty_plugin_count),
    get_plugin_descriptor: Some(empty_plugin_descriptor),
    create_plugin: Some(empty_create_plugin),
};

unsafe extern "C" fn empty_plugin_count(_: *const clap_plugin_factory) -> u32 {
    0
}

unsafe extern "C" fn empty_plugin_descriptor(
    _: *const clap_plugin_factory,
    _: u32,
) -> *const clap_plugin_descriptor {
    std::ptr::null()
}

unsafe extern "C" fn empty_create_plugin(
    _: *const clap_plugin_factory,
    _: *const clap_sys::host::clap_host,
    _: *const c_char,
) -> *const clap_sys::plugin::clap_plugin {
    std::ptr::null()
}

#[test]
fn registered_clap_plugin_entry_exposes_editor_descriptor() {
    let plugin = ClapPluginMetadata {
        processor_id: "clap:/Plug/Editor.clap#org.test.editor".to_string(),
        clap_id: "org.test.editor".to_string(),
        name: "Editor Plugin".to_string(),
        vendor: None,
        version: None,
        features: vec!["audio-effect".to_string()],
        role: registry::Role::Effect,
        path: PathBuf::from("/Plug/Editor.clap"),
        library_path: PathBuf::from("/Plug/Editor.clap"),
    };

    register_plugins([plugin.clone()]);
    let entry = registry::entry(&plugin.processor_id).expect("plugin should be registered");

    assert_eq!(entry.backend, registry::Backend::Clap);
    assert_eq!(entry.descriptor.name, "Editor Plugin");
    assert_eq!(
        entry.descriptor.editor,
        Some(EditorDescriptor {
            resizable: false,
            ..DEFAULT_CLAP_EDITOR_DESCRIPTOR
        })
    );
}

#[test]
fn clap_editor_default_size_uses_conservative_host_fallback() {
    assert_eq!(
        DEFAULT_CLAP_EDITOR_DESCRIPTOR.default_size,
        EditorSize {
            width: 720,
            height: 480,
        }
    );
}

#[test]
fn runtime_key_parses_stable_processor_id() {
    let key = RuntimeKey::parse("clap:/Plug/Test.clap#org.test")
        .expect("stable CLAP processor id should parse");

    assert_eq!(key.path, PathBuf::from("/Plug/Test.clap"));
    assert_eq!(key.clap_id, "org.test");
}

#[test]
fn appkit_parent_converts_to_cocoa_clap_window() {
    let ptr = NonNull::<std::ffi::c_void>::dangling();
    let parent = lilypalooza_audio::EditorParent {
        window: RawWindowHandle::AppKit(AppKitWindowHandle::new(ptr)),
        display: None,
    };

    let window = clap_window_for_parent(parent).expect("appkit parent should be supported");

    // SAFETY: The tested conversion returns a static CLAP API C string.
    let api = unsafe { CStr::from_ptr(window.api) };
    // SAFETY: The window was constructed as a Cocoa CLAP window in this test.
    let cocoa = unsafe { window.specific.cocoa };
    assert_eq!(api, clap_sys::ext::gui::CLAP_WINDOW_API_COCOA);
    assert_eq!(cocoa, ptr.as_ptr());
}
