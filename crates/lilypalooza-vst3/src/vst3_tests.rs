use lilypalooza_audio::instrument::EditorResizeHandler;

use super::{editor::*, host_com::*, probe::*, *};

#[test]
fn vst3_candidate_detection_is_extension_based() {
    assert!(is_vst3_candidate(Path::new("Plugin.vst3")));
    assert!(is_vst3_candidate(Path::new("Plugin.VST3")));
    assert!(!is_vst3_candidate(Path::new("Plugin.clap")));
}

#[test]
fn host_application_creates_vst3_messages() {
    let host = Vst3Host::new();
    let mut cid = IMessage_iid;
    let mut iid = IMessage_iid;
    let mut obj = std::ptr::null_mut();

    // SAFETY: The test passes valid TUID pointers and a writable out pointer.
    let result = unsafe { host.createInstance(&mut cid, &mut iid, &mut obj) };

    assert_eq!(result, kResultOk);
    // SAFETY: `createInstance` returned ownership of an IMessage pointer.
    let message =
        unsafe { ComPtr::from_raw(obj.cast::<IMessage>()) }.expect("host should create IMessage");
    let id = CString::new("hello").expect("static string has no interior nul");
    // SAFETY: The message pointer is live and `id` is a valid null-terminated string.
    unsafe { message.setMessageID(id.as_ptr()) };
    // SAFETY: The message pointer is live.
    let message_id = unsafe { message.getMessageID() };
    // SAFETY: The returned ID is a null-terminated string owned by the message.
    assert_eq!(unsafe { CStr::from_ptr(message_id) }.to_bytes(), b"hello");
    // SAFETY: The message pointer is live.
    assert!(!unsafe { message.getAttributes() }.is_null());
}

#[test]
fn host_application_creates_attribute_lists() {
    let host = Vst3Host::new();
    let mut cid = IAttributeList_iid;
    let mut iid = IAttributeList_iid;
    let mut obj = std::ptr::null_mut();

    // SAFETY: The test passes valid TUID pointers and a writable out pointer.
    let result = unsafe { host.createInstance(&mut cid, &mut iid, &mut obj) };

    assert_eq!(result, kResultOk);
    // SAFETY: `createInstance` returned ownership of an IAttributeList pointer.
    let attributes = unsafe { ComPtr::from_raw(obj.cast::<IAttributeList>()) }
        .expect("host should create IAttributeList");
    let key = CString::new("value").expect("static string has no interior nul");
    let mut value = 0;
    // SAFETY: The attribute list pointer is live and `key` is null-terminated.
    assert_eq!(unsafe { attributes.setInt(key.as_ptr(), 42) }, kResultOk);
    // SAFETY: The attribute list pointer is live and `key` is null-terminated.
    let get_result = unsafe { attributes.getInt(key.as_ptr(), &mut value) };
    assert_eq!(get_result, kResultOk);
    assert_eq!(value, 42);
}

#[test]
fn vst3_candidate_paths_recurse() {
    let dir = tempfile::tempdir().expect("temp dir");
    let nested = dir.path().join("Vendor").join("Plugin.vst3");
    std::fs::create_dir_all(&nested).expect("nested vst3 dir");
    std::fs::write(dir.path().join("Root.vst3"), "").expect("root vst3 file");
    std::fs::write(dir.path().join("Other.clap"), "").expect("clap file");

    let candidates = candidate_paths(dir.path()).expect("candidate scan");

    assert_eq!(candidates, vec![dir.path().join("Root.vst3"), nested]);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_vst3_loader_uses_sdk_entry_symbol_names() {
    assert_eq!(GET_PLUGIN_FACTORY_SYMBOL, "GetPluginFactory");
    assert_eq!(BUNDLE_ENTRY_SYMBOL, "bundleEntry");
}

#[test]
fn stable_processor_id_includes_path_and_class_id() {
    assert_eq!(
        stable_processor_id(Path::new("/Plug/Test.vst3"), "001122"),
        "vst3:/Plug/Test.vst3#001122"
    );
}

#[test]
fn validation_report_serializes() {
    let report = ValidationReport {
        format: FORMAT.to_string(),
        path: PathBuf::from("/Plug/Test.vst3"),
        result: Ok(vec![Vst3PluginMetadata {
            processor_id: "vst3:/Plug/Test.vst3#00112233445566778899aabbccddeeff".to_string(),
            class_id: "00112233445566778899aabbccddeeff".to_string(),
            name: "Test".to_string(),
            vendor: Some("Vendor".to_string()),
            version: Some("1.0".to_string()),
            category: Some("Instrument|Synth".to_string()),
            role: registry::Role::Instrument,
            path: PathBuf::from("/Plug/Test.vst3"),
            library_path: PathBuf::from("/Plug/Test.vst3"),
        }]),
    };

    let json = serde_json::to_string(&report).expect("report json");

    assert!(json.contains("\"format\":\"vst3\""));
    assert!(json.contains("00112233445566778899aabbccddeeff"));
}

#[test]
fn role_detection_treats_instrument_subcategories_as_instruments() {
    assert_eq!(
        role_from_subcategories(Some("Instrument|Synth")),
        registry::Role::Instrument
    );
    assert_eq!(
        role_from_subcategories(Some("Fx|Delay")),
        registry::Role::Effect
    );
}

#[test]
fn legacy_class_info_uses_factory_vendor() {
    let mut info = zeroed::<PClassInfo>();
    write_c_char_array(&mut info.name, "Legacy Plugin");

    let metadata = metadata_from_class_info(
        info,
        Path::new("/Plug/Legacy.vst3"),
        Path::new("/Plug/Legacy.vst3"),
        Some("Factory Vendor"),
    );

    assert_eq!(metadata.name, "Legacy Plugin");
    assert_eq!(metadata.vendor.as_deref(), Some("Factory Vendor"));
}

#[test]
fn class_info2_prefers_class_vendor_over_factory_vendor() {
    let mut info = zeroed::<PClassInfo2>();
    write_c_char_array(&mut info.vendor, "Class Vendor");

    let metadata = metadata_from_class_info2(
        info,
        Path::new("/Plug/ClassVendor.vst3"),
        Path::new("/Plug/ClassVendor.vst3"),
        Some("Factory Vendor"),
    );

    assert_eq!(metadata.vendor.as_deref(), Some("Class Vendor"));
}

#[test]
fn class_info2_empty_vendor_uses_factory_vendor() {
    let info = zeroed::<PClassInfo2>();

    let metadata = metadata_from_class_info2(
        info,
        Path::new("/Plug/FactoryVendor.vst3"),
        Path::new("/Plug/FactoryVendor.vst3"),
        Some("Factory Vendor"),
    );

    assert_eq!(metadata.vendor.as_deref(), Some("Factory Vendor"));
}

#[test]
fn component_integrated_controller_uses_component_lifecycle_only() {
    let lifecycle = ControllerLifecycle::ComponentIntegrated;

    assert!(!lifecycle.connects_component());
    assert!(!lifecycle.terminates_controller());
}

#[test]
fn separate_controller_uses_separate_controller_lifecycle() {
    let lifecycle = ControllerLifecycle::Separate;

    assert!(lifecycle.connects_component());
    assert!(lifecycle.terminates_controller());
}

#[test]
fn tuid_hex_roundtrips() {
    let hex = "00112233445566778899aabbccddeeff";
    assert_eq!(tuid_to_hex(&hex_to_tuid(hex).expect("tuid")), hex);
}

fn write_c_char_array<const N: usize>(dst: &mut [c_char; N], value: &str) {
    for (dst, src) in dst.iter_mut().zip(value.bytes().chain(std::iter::once(0))) {
        *dst = src as c_char;
    }
}

#[test]
fn registered_vst3_plugin_entry_exposes_editor_descriptor() {
    let plugin = Vst3PluginMetadata {
        processor_id: "vst3:/Plug/Editor.vst3#00112233445566778899aabbccddeeff".to_string(),
        class_id: "00112233445566778899aabbccddeeff".to_string(),
        name: "Editor".to_string(),
        vendor: Some("Vendor".to_string()),
        version: None,
        category: Some("Fx".to_string()),
        role: registry::Role::Effect,
        path: PathBuf::from("/Plug/Editor.vst3"),
        library_path: PathBuf::from("/Plug/Editor.vst3"),
    };

    register_plugins([plugin.clone()]);
    let entry = registry::entry(&plugin.processor_id).expect("registered plugin");

    assert_eq!(entry.backend, registry::Backend::Vst3);
    assert_eq!(entry.role, registry::Role::Effect);
    assert!(entry.descriptor.editor.is_some());
}

#[test]
fn unchanged_vst3_editor_size_request_is_ignored() {
    let mut current = Some(EditorSize {
        width: 640,
        height: 480,
    });

    assert_eq!(
        changed_editor_size_request(
            &mut current,
            Some(EditorSize {
                width: 640,
                height: 480,
            })
        ),
        None
    );
    assert_eq!(
        changed_editor_size_request(
            &mut current,
            Some(EditorSize {
                width: 800,
                height: 600,
            })
        ),
        Some(EditorSize {
            width: 800,
            height: 600,
        })
    );
    assert_eq!(
        current,
        Some(EditorSize {
            width: 800,
            height: 600,
        })
    );
}

#[test]
fn vst3_resize_view_without_live_handler_records_deferred_resize_request() {
    let host = Vst3Host::new();
    let mut rect = rect_from_editor_size(EditorSize {
        width: 800,
        height: 600,
    });

    // SAFETY: The test passes a live stack-owned ViewRect pointer to the host callback.
    let result = unsafe { host.resizeView(std::ptr::null_mut(), &raw mut rect) };

    assert_eq!(result, kResultOk);
    assert_eq!(
        host.take_requested_size(),
        Some(EditorSize {
            width: 800,
            height: 600,
        })
    );
}

#[test]
fn vst3_resize_view_resizes_host_and_calls_on_size_synchronously() {
    let host = Vst3Host::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(TestResizeHandler {
        events: Arc::clone(&events),
        ..TestResizeHandler::default()
    });
    host.set_resize_handler(Some(handler.clone()));
    let view = ComWrapper::new(TestPlugView {
        events: Arc::clone(&events),
        ..TestPlugView::default()
    });
    let view_ptr = view.to_com_ptr::<IPlugView>().expect("test view");
    let mut rect = rect_from_editor_size(EditorSize {
        width: 800,
        height: 600,
    });

    // SAFETY: The test passes live COM view and ViewRect pointers to the host callback.
    let result = unsafe { host.resizeView(view_ptr.as_ptr(), &raw mut rect) };

    assert_eq!(result, kResultOk);
    assert_eq!(
        handler
            .requested
            .lock()
            .expect("requested sizes")
            .as_slice(),
        &[EditorSize {
            width: 800,
            height: 600,
        }]
    );
    assert_eq!(
        view.on_size.lock().expect("onSize calls").as_slice(),
        &[EditorSize {
            width: 800,
            height: 600,
        }]
    );
    assert_eq!(
        host.take_requested_size(),
        Some(EditorSize {
            width: 800,
            height: 600,
        })
    );
    assert_eq!(
        events.lock().expect("resize events").as_slice(),
        &["resize_editor", "onSize"]
    );
}

#[test]
fn vst3_missing_native_resize_does_not_override_static_editor_resizability() {
    let view = ComWrapper::new(TestPlugView {
        native_resizable: false,
        ..TestPlugView::default()
    });
    let view_ptr = view.to_com_ptr::<IPlugView>().expect("test view");

    assert_eq!(vst3_editor_view_resizability(&view_ptr), Some(false));
}

#[test]
fn vst3_resize_without_native_resize_does_not_call_content_scale() {
    let view = ComWrapper::new(TestPlugView {
        native_resizable: false,
        content_scale_result: kResultFalse,
        ..TestPlugView::default()
    });
    let view_ptr = view.to_com_ptr::<IPlugView>().expect("test view");

    let accepted = resize_vst3_editor_view(
        &view_ptr,
        EditorSize {
            width: 960,
            height: 720,
        },
    )
    .expect("resize should use content scale support");

    assert_eq!(
        accepted,
        EditorSize {
            width: 960,
            height: 720,
        }
    );
    assert_eq!(view.on_size.lock().expect("onSize calls").as_slice(), &[]);
    assert!(
        view.content_scale_factors
            .lock()
            .expect("content scale factors")
            .is_empty()
    );
}

#[derive(Default)]
struct TestResizeHandler {
    requested: Mutex<Vec<EditorSize>>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl EditorResizeHandler for TestResizeHandler {
    fn resize_editor(&self, size: EditorSize) -> Result<EditorSize, EditorError> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("resize_editor");
        self.requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(size);
        Ok(size)
    }
}

struct TestPlugView {
    native_resizable: bool,
    content_scale_result: tresult,
    on_size: Mutex<Vec<EditorSize>>,
    content_scale_factors: Mutex<Vec<f32>>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl Default for TestPlugView {
    fn default() -> Self {
        Self {
            native_resizable: true,
            content_scale_result: kResultOk,
            on_size: Mutex::default(),
            content_scale_factors: Mutex::default(),
            events: Arc::default(),
        }
    }
}

impl Class for TestPlugView {
    type Interfaces = (IPlugView, IPlugViewContentScaleSupport);
}

impl IPlugViewTrait for TestPlugView {
    unsafe fn isPlatformTypeSupported(&self, _type: FIDString) -> tresult {
        kResultOk
    }

    unsafe fn attached(&self, _parent: *mut c_void, _type: FIDString) -> tresult {
        kResultOk
    }

    unsafe fn removed(&self) -> tresult {
        kResultOk
    }

    unsafe fn onWheel(&self, _distance: f32) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyDown(&self, _key: char16, _key_code: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyUp(&self, _key: char16, _key_code: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn getSize(&self, size: *mut ViewRect) -> tresult {
        // SAFETY: VST3 supplies a writable ViewRect pointer or null.
        let Some(size) = (unsafe { size.as_mut() }) else {
            return kInvalidArgument;
        };
        *size = rect_from_editor_size(EditorSize {
            width: 640,
            height: 480,
        });
        kResultOk
    }

    unsafe fn onSize(&self, new_size: *mut ViewRect) -> tresult {
        // SAFETY: VST3 supplies a readable ViewRect pointer or null.
        let rect = unsafe { new_size.as_ref() };
        let Some(size) = rect.and_then(|rect| editor_size_from_rect(*rect)) else {
            return kInvalidArgument;
        };
        self.events.lock().expect("resize events").push("onSize");
        self.on_size.lock().expect("onSize calls").push(size);
        kResultOk
    }

    unsafe fn onFocus(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setFrame(&self, _frame: *mut IPlugFrame) -> tresult {
        kResultOk
    }

    unsafe fn canResize(&self) -> tresult {
        if self.native_resizable {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn checkSizeConstraint(&self, _rect: *mut ViewRect) -> tresult {
        kResultOk
    }
}

impl IPlugViewContentScaleSupportTrait for TestPlugView {
    unsafe fn setContentScaleFactor(&self, factor: f32) -> tresult {
        self.content_scale_factors
            .lock()
            .expect("content scale factors")
            .push(factor);
        self.content_scale_result
    }
}
