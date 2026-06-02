use std::sync::{Arc, Mutex};

use lilypalooza_audio::{EditorDescriptor, EditorError, EditorSession, EditorSize};

use crate::runtime;

pub(super) const DEFAULT_AU_EDITOR_DESCRIPTOR: EditorDescriptor = EditorDescriptor {
    default_size: EditorSize {
        width: 640,
        height: 480,
    },
    min_size: None,
    resizable: false,
};

pub(super) fn create_editor_session(
    shared: Arc<Mutex<runtime::AuRuntime>>,
) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
    platform::create_editor_session(shared)
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{ffi::CStr, os::raw::c_char};

    use lilypalooza_audio::{EditorError, EditorSession, EditorSize, instrument::EditorParent};
    use num_traits::ToPrimitive;
    use objc2::{
        encode::{Encode, Encoding, RefEncode},
        msg_send,
        rc::Retained,
        runtime::AnyObject,
        sel,
    };
    use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
    use objc2_foundation::{NSBundle, NSClassFromString, NSRect, NSSize, NSString};
    use raw_window_handle::RawWindowHandle;

    use super::{Arc, DEFAULT_AU_EDITOR_DESCRIPTOR, Mutex, runtime};

    pub(super) fn create_editor_session(
        shared: Arc<Mutex<runtime::AuRuntime>>,
    ) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        if !shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .has_native_editor()
        {
            return Ok(None);
        }
        Ok(Some(Box::new(AuEditorSession {
            shared,
            cocoa_view: None,
            initial_size: None,
        })))
    }

    struct AuEditorSession {
        shared: Arc<Mutex<runtime::AuRuntime>>,
        cocoa_view: Option<AuCocoaView>,
        initial_size: Option<EditorSize>,
    }

    impl EditorSession for AuEditorSession {
        fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
            Ok(self
                .cocoa_view
                .as_ref()
                .map(|cocoa_view| cocoa_view.resizable))
        }

        fn initial_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
            Ok(self.initial_size)
        }

        fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError> {
            let parent = appkit_parent_view(parent)?;
            trace_cocoa_view_tree("attach-parent-before", &parent);
            let unit = self
                .shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .raw_unit();
            let cocoa_view = create_cocoa_view(unit, DEFAULT_AU_EDITOR_DESCRIPTOR.default_size)?;
            let initial_size = editor_size_from_ns_size(cocoa_view.view.frame().size)
                .unwrap_or(DEFAULT_AU_EDITOR_DESCRIPTOR.default_size);
            configure_cocoa_root_view(&cocoa_view.view);
            resize_cocoa_root_view(&cocoa_view.view, initial_size);
            trace_cocoa_view_tree("attach-view-before-add", &cocoa_view.view);
            parent.addSubview(&cocoa_view.view);
            trace_cocoa_view_tree("attach-parent-after-add", &parent);
            trace_cocoa_view_tree("attach-view-after-add", &cocoa_view.view);
            self.initial_size = Some(initial_size);
            self.cocoa_view = Some(cocoa_view);
            Ok(())
        }

        fn detach(&mut self) -> Result<(), EditorError> {
            if let Some(cocoa_view) = self.cocoa_view.take() {
                cocoa_view.view.removeFromSuperview();
            }
            Ok(())
        }

        fn set_visible(&mut self, visible: bool) -> Result<(), EditorError> {
            if let Some(cocoa_view) = &self.cocoa_view {
                cocoa_view.view.setHidden(!visible);
            }
            Ok(())
        }

        fn tracks_native_content_resize(&self) -> bool {
            au_editor_tracks_native_content_resize()
        }

        fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
            let before = self.current_size();
            if let Some(cocoa_view) = &self.cocoa_view {
                resize_cocoa_root_view(&cocoa_view.view, size);
            }
            let accepted = self.current_size().unwrap_or(size);
            log::trace!(
                target: "lilypalooza_au::editor",
                "AU editor resize requested={size:?} before={before:?} accepted={accepted:?}",
            );
            self.initial_size = Some(accepted);
            Ok(accepted)
        }
    }

    impl AuEditorSession {
        fn current_size(&self) -> Option<EditorSize> {
            self.cocoa_view
                .as_ref()
                .and_then(|cocoa_view| editor_size_from_ns_size(cocoa_view.view.frame().size))
                .or(self.initial_size)
        }
    }

    impl Drop for AuEditorSession {
        fn drop(&mut self) {
            if let Err(error) = self.detach() {
                log::warn!("AU editor detach failed during drop: {error}");
            }
        }
    }

    struct AuCocoaView {
        _bundle: Retained<NSBundle>,
        _factory: Retained<AnyObject>,
        view: Retained<NSView>,
        resizable: bool,
    }

    #[repr(C)]
    struct AuOpaqueAudioComponentInstance {
        _private: [u8; 0],
    }

    // SAFETY: `AudioUnit` is `OpaqueAudioComponentInstance *` in modern AudioToolbox headers.
    unsafe impl Encode for AuOpaqueAudioComponentInstance {
        const ENCODING: Encoding = Encoding::Struct("OpaqueAudioComponentInstance", &[]);
    }

    // SAFETY: Cocoa AU factories receive the opaque AudioUnit pointer.
    unsafe impl RefEncode for AuOpaqueAudioComponentInstance {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    #[repr(C)]
    struct AuComponentInstanceRecord {
        _private: [i64; 1],
    }

    // SAFETY: Older AU Cocoa UI factories were compiled against AudioUnit as
    // `ComponentInstanceRecord *`.
    unsafe impl Encode for AuComponentInstanceRecord {
        const ENCODING: Encoding = Encoding::Struct(
            "ComponentInstanceRecord",
            &[Encoding::Array(1, &i64::ENCODING)],
        );
    }

    // SAFETY: Used only when the factory method signature advertises this ABI.
    unsafe impl RefEncode for AuComponentInstanceRecord {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AuUnitPointerAbi {
        Opaque,
        ComponentInstanceRecord,
    }

    struct AuCocoaViewInfo {
        bundle_path: String,
        class_names: Vec<String>,
    }

    struct CreatedCocoaView {
        factory: Retained<AnyObject>,
        view: Retained<NSView>,
        resizable: bool,
    }

    fn create_cocoa_view(
        unit: coreaudio_sys::AudioUnit,
        preferred_size: EditorSize,
    ) -> Result<AuCocoaView, EditorError> {
        let info = cocoa_view_info(unit).map_err(editor_runtime_error)?;
        log::trace!(
            target: "lilypalooza_au::editor",
            "AU Cocoa UI info bundle={} classes={:?}",
            info.bundle_path,
            info.class_names,
        );
        let bundle = load_cocoa_view_bundle(&info.bundle_path)?;

        for class_name in info.class_names {
            if let Some(created) = create_cocoa_view_for_class(&class_name, unit, preferred_size) {
                return Ok(AuCocoaView {
                    _bundle: bundle,
                    _factory: created.factory,
                    view: created.view,
                    resizable: created.resizable,
                });
            }
        }

        Err(EditorError::Unsupported)
    }

    fn load_cocoa_view_bundle(bundle_path: &str) -> Result<Retained<NSBundle>, EditorError> {
        let path = NSString::from_str(bundle_path);
        let bundle = NSBundle::bundleWithPath(&path).ok_or_else(|| {
            EditorError::Backend(format!("AU Cocoa UI bundle not found: {bundle_path}"))
        })?;
        // SAFETY: Loading the bundle is required before resolving its AU Cocoa UI class.
        if !bundle.isLoaded() && !unsafe { bundle.load() } {
            return Err(EditorError::Backend(format!(
                "AU Cocoa UI bundle failed to load: {bundle_path}"
            )));
        }
        Ok(bundle)
    }

    fn create_cocoa_view_for_class(
        class_name: &str,
        unit: coreaudio_sys::AudioUnit,
        preferred_size: EditorSize,
    ) -> Option<CreatedCocoaView> {
        let class = NSClassFromString(&NSString::from_str(class_name))?;
        // SAFETY: The class name comes from the AU Cocoa UI property and is expected to
        // implement AUCocoaUIBase.
        let factory: Retained<AnyObject> = unsafe { msg_send![class, new] };
        let Some(unit_abi) = cocoa_factory_unit_argument_abi_for_class(class) else {
            warn_unsupported_cocoa_view_signature(class_name);
            return None;
        };
        let view = create_cocoa_view_for_unit_abi(&factory, unit, unit_abi, preferred_size)?;
        let resizable = cocoa_view_resizable(&view);
        log_cocoa_view_created(class_name, &view, resizable);
        Some(CreatedCocoaView {
            factory,
            view,
            resizable,
        })
    }

    fn warn_unsupported_cocoa_view_signature(class_name: &str) {
        log::warn!(
            target: "lilypalooza_au::editor",
            "AU Cocoa UI class {class_name} has unsupported uiViewForAudioUnit:withSize: signature"
        );
    }

    fn log_cocoa_view_created(class_name: &str, view: &NSView, resizable: bool) {
        log::trace!(
            target: "lilypalooza_au::editor",
            "AU Cocoa UI created class={} view={:p} frame={} bounds={} hidden={} resizable={}",
            class_name,
            view,
            format_ns_rect(view.frame()),
            format_ns_rect(view.bounds()),
            view.isHidden(),
            resizable,
        );
    }

    fn create_cocoa_view_for_unit_abi(
        factory: &AnyObject,
        unit: coreaudio_sys::AudioUnit,
        unit_abi: AuUnitPointerAbi,
        preferred_size: EditorSize,
    ) -> Option<Retained<NSView>> {
        match unit_abi {
            AuUnitPointerAbi::Opaque => {
                let unit = unit.cast::<AuOpaqueAudioComponentInstance>();
                // SAFETY: The factory signature was checked before this call. The AudioUnit
                // pointer stays live while this editor session is attached.
                unsafe {
                    msg_send![
                        factory,
                        uiViewForAudioUnit: unit,
                        withSize: ns_size(preferred_size)
                    ]
                }
            }
            AuUnitPointerAbi::ComponentInstanceRecord => {
                let unit = unit.cast::<AuComponentInstanceRecord>();
                // SAFETY: The factory signature was checked before this call. The AudioUnit
                // pointer stays live while this editor session is attached.
                unsafe {
                    msg_send![
                        factory,
                        uiViewForAudioUnit: unit,
                        withSize: ns_size(preferred_size)
                    ]
                }
            }
        }
    }

    fn cocoa_factory_unit_argument_abi_for_class(
        class: &objc2::runtime::AnyClass,
    ) -> Option<AuUnitPointerAbi> {
        let method = class.instance_method(sel!(uiViewForAudioUnit:withSize:))?;
        let unit_argument = method.argument_type(2)?;
        cocoa_factory_unit_argument_abi(unit_argument.to_str().ok()?)
    }

    fn cocoa_factory_unit_argument_abi(argument_type: &str) -> Option<AuUnitPointerAbi> {
        if AuOpaqueAudioComponentInstance::ENCODING_REF.equivalent_to_str(argument_type) {
            Some(AuUnitPointerAbi::Opaque)
        } else if AuComponentInstanceRecord::ENCODING_REF.equivalent_to_str(argument_type) {
            Some(AuUnitPointerAbi::ComponentInstanceRecord)
        } else {
            None
        }
    }

    fn cocoa_view_info(
        unit: coreaudio_sys::AudioUnit,
    ) -> Result<AuCocoaViewInfo, runtime::AuRuntimeError> {
        let size = runtime::property_size(
            unit,
            coreaudio_sys::kAudioUnitProperty_CocoaUI,
            coreaudio_sys::kAudioUnitScope_Global,
            0,
        )?;
        let mut storage = vec![0usize; cocoa_view_storage_words(size)];
        let mut actual_size = size;
        let byte_count =
            u32::try_from(std::mem::size_of_val(storage.as_slice())).unwrap_or(u32::MAX);
        actual_size = actual_size.min(byte_count);
        // SAFETY: `storage` is writable pointer-aligned storage for the Core Audio property data.
        let status = unsafe {
            coreaudio_sys::AudioUnitGetProperty(
                unit,
                coreaudio_sys::kAudioUnitProperty_CocoaUI,
                coreaudio_sys::kAudioUnitScope_Global,
                0,
                storage.as_mut_ptr().cast(),
                &mut actual_size,
            )
        };
        runtime::core_audio_status("AudioUnitGetProperty(CocoaUI)", status)?;
        cocoa_view_info_from_storage(&storage, actual_size).ok_or(
            runtime::AuRuntimeError::CoreAudio {
                operation: "AudioUnitGetProperty(CocoaUI)",
                status: -1,
            },
        )
    }

    fn cocoa_view_info_from_storage(storage: &[usize], size: u32) -> Option<AuCocoaViewInfo> {
        if storage.is_empty() {
            return None;
        }
        let info = storage
            .as_ptr()
            .cast::<coreaudio_sys::AudioUnitCocoaViewInfo>();
        // SAFETY: `storage` was filled by AudioUnitGetProperty with AudioUnitCocoaViewInfo bytes.
        let bundle_location = unsafe { (*info).mCocoaAUViewBundleLocation };
        if bundle_location.is_null() {
            return None;
        }
        // SAFETY: `bundle_location` is a live CFURLRef returned by the AU Cocoa UI property.
        let bundle_path_ref = unsafe {
            coreaudio_sys::CFURLCopyFileSystemPath(
                bundle_location,
                coreaudio_sys::kCFURLPOSIXPathStyle.into(),
            )
        };
        let bundle_path = cf_string_to_string(bundle_path_ref);
        release_cf(bundle_path_ref);
        let Some(bundle_path) = bundle_path else {
            release_cf(bundle_location);
            return None;
        };

        let class_count = cocoa_view_class_count(size);
        // SAFETY: Class refs start immediately after the CFURLRef field in AudioUnitCocoaViewInfo.
        let class_ptr = unsafe {
            storage
                .as_ptr()
                .cast::<u8>()
                .add(std::mem::size_of::<coreaudio_sys::CFURLRef>())
                .cast::<coreaudio_sys::CFStringRef>()
        };
        let mut class_names = Vec::with_capacity(class_count);
        for index in 0..class_count {
            // SAFETY: `index` is bounded by the byte size reported by Core Audio.
            let class_name_ptr = unsafe { class_ptr.add(index) };
            // SAFETY: `class_name_ptr` points into the class tail array returned by Core Audio.
            let class_name_ref = unsafe { *class_name_ptr };
            if let Some(class_name) = cf_string_to_string(class_name_ref) {
                class_names.push(class_name);
            }
            release_cf(class_name_ref);
        }
        release_cf(bundle_location);

        (!class_names.is_empty()).then_some(AuCocoaViewInfo {
            bundle_path,
            class_names,
        })
    }

    fn cocoa_view_storage_words(size: u32) -> usize {
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        size.div_ceil(std::mem::size_of::<usize>()).max(1)
    }

    fn cocoa_view_class_count(size: u32) -> usize {
        let Ok(size) = usize::try_from(size) else {
            return 0;
        };
        let class_offset = std::mem::size_of::<coreaudio_sys::CFURLRef>();
        let class_size = std::mem::size_of::<coreaudio_sys::CFStringRef>();
        size.saturating_sub(class_offset) / class_size
    }

    fn cf_string_to_string(value: coreaudio_sys::CFStringRef) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let mut buffer = vec![0 as c_char; cf_string_utf8_buffer_len(value)?];
        copy_cf_string_to_buffer(value, &mut buffer)?;
        c_char_buffer_to_string(&buffer)
    }

    fn cf_string_utf8_buffer_len(value: coreaudio_sys::CFStringRef) -> Option<usize> {
        // SAFETY: `value` is checked non-null and expected to be a CFStringRef.
        let length = unsafe { coreaudio_sys::CFStringGetLength(value) };
        // SAFETY: Pure size query for a valid CFString length and encoding.
        let max_size = unsafe {
            coreaudio_sys::CFStringGetMaximumSizeForEncoding(
                length,
                coreaudio_sys::kCFStringEncodingUTF8,
            )
        };
        usize::try_from(max_size).ok()?.checked_add(1)
    }

    fn copy_cf_string_to_buffer(
        value: coreaudio_sys::CFStringRef,
        buffer: &mut [c_char],
    ) -> Option<()> {
        // SAFETY: `buffer` is writable and large enough for the UTF-8 representation plus NUL.
        let ok = unsafe {
            coreaudio_sys::CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                i64::try_from(buffer.len()).ok()?,
                coreaudio_sys::kCFStringEncodingUTF8,
            )
        };
        (ok != 0).then_some(())
    }

    fn c_char_buffer_to_string(buffer: &[c_char]) -> Option<String> {
        // SAFETY: CFStringGetCString wrote a NUL-terminated string when it returned true.
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .ok()
            .map(str::to_string)
    }

    fn release_cf<T>(value: *const T) {
        if !value.is_null() {
            // SAFETY: Core Audio's Cocoa UI property returns retained CF objects; each copied ref
            // is released exactly once after conversion.
            unsafe { coreaudio_sys::CFRelease(value.cast()) };
        }
    }

    fn appkit_parent_view(parent: EditorParent) -> Result<Retained<NSView>, EditorError> {
        match parent.window {
            RawWindowHandle::AppKit(handle) => {
                let view = handle.ns_view.as_ptr().cast::<NSView>();
                // SAFETY: The raw handle belongs to the live editor-host parent view.
                unsafe { Retained::retain(view) }.ok_or_else(|| {
                    EditorError::HostUnavailable("AU editor parent view is null".to_string())
                })
            }
            other => Err(EditorError::HostUnavailable(format!(
                "unsupported AU editor parent: {other:?}"
            ))),
        }
    }

    fn editor_runtime_error(error: runtime::AuRuntimeError) -> EditorError {
        EditorError::Backend(error.to_string())
    }

    fn ns_size(size: EditorSize) -> NSSize {
        NSSize::new(f64::from(size.width), f64::from(size.height))
    }

    fn editor_frame(size: EditorSize) -> NSRect {
        NSRect::new(Default::default(), ns_size(size))
    }

    fn editor_size_from_ns_size(size: NSSize) -> Option<EditorSize> {
        if size.width <= 1.0 || size.height <= 1.0 {
            return None;
        }
        Some(EditorSize {
            width: size.width.round().to_u32()?,
            height: size.height.round().to_u32()?,
        })
    }

    fn resize_cocoa_root_view(view: &NSView, size: EditorSize) {
        let old_size = view.frame().size;
        let target_frame = editor_frame(size);
        trace_cocoa_view_tree("before-resize", view);
        view.setFrame(target_frame);
        resize_cocoa_root_bounds(view, size);
        resize_primary_content_subview(view, target_frame);
        view.resizeSubviewsWithOldSize(old_size);
        view.setNeedsLayout(true);
        view.layoutSubtreeIfNeeded();
        view.setNeedsDisplay(true);
        trace_cocoa_view_tree("after-resize", view);
    }

    fn configure_cocoa_root_view(view: &NSView) {
        view.setAutoresizesSubviews(true);
        let subviews = view.subviews().to_vec();
        if should_resize_primary_content_subview(subviews.len())
            && let Some(subview) = subviews.first()
        {
            configure_primary_content_subview(subview);
        }
    }

    fn resize_primary_content_subview(view: &NSView, frame: NSRect) {
        let subviews = view.subviews().to_vec();
        if !should_resize_primary_content_subview(subviews.len()) {
            return;
        }
        let Some(subview) = subviews.into_iter().next() else {
            return;
        };
        configure_primary_content_subview(&subview);
        subview.setFrame(frame);
        subview.setNeedsLayout(true);
        subview.layoutSubtreeIfNeeded();
        subview.setNeedsDisplay(true);
    }

    fn configure_primary_content_subview(view: &NSView) {
        view.setAutoresizingMask(cocoa_editor_autoresizing_mask());
        view.setAutoresizesSubviews(true);
    }

    fn cocoa_editor_autoresizing_mask() -> NSAutoresizingMaskOptions {
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable
    }

    fn cocoa_view_resizable(view: &NSView) -> bool {
        cocoa_autoresizing_mask_resizable(view.autoresizingMask())
    }

    fn cocoa_autoresizing_mask_resizable(mask: NSAutoresizingMaskOptions) -> bool {
        mask.intersects(cocoa_editor_autoresizing_mask())
    }

    fn should_resize_primary_content_subview(subview_count: usize) -> bool {
        subview_count == 1
    }

    fn au_editor_tracks_native_content_resize() -> bool {
        true
    }

    fn resize_cocoa_root_bounds(view: &NSView, target_size: EditorSize) {
        view.setBounds(editor_frame(target_size));
    }

    fn trace_cocoa_view_tree(stage: &str, view: &NSView) {
        if !log::log_enabled!(target: "lilypalooza_au::editor", log::Level::Trace) {
            return;
        }
        trace_cocoa_view(stage, 0, view);
    }

    fn trace_cocoa_view(stage: &str, depth: usize, view: &NSView) {
        let frame = view.frame();
        let bounds = view.bounds();
        let subviews = view.subviews().to_vec();
        log::trace!(
            target: "lilypalooza_au::editor",
            "AU view tree {stage} depth={depth} ptr={:p} frame={} bounds={} flipped={} wants_layer={} layer={} subviews={} autoresizes={} mask={:?}",
            view,
            format_ns_rect(frame),
            format_ns_rect(bounds),
            view.isFlipped(),
            view.wantsLayer(),
            format_layer(view),
            subviews.len(),
            view.autoresizesSubviews(),
            view.autoresizingMask(),
        );
        if depth >= 4 {
            return;
        }
        for subview in subviews {
            trace_cocoa_view(stage, depth + 1, &subview);
        }
    }

    fn format_ns_rect(rect: NSRect) -> String {
        format!(
            "x={:.1} y={:.1} w={:.1} h={:.1}",
            rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
        )
    }

    fn format_layer(view: &NSView) -> String {
        let Some(layer) = view.layer() else {
            return "none".to_string();
        };
        // SAFETY: `layer` is retained from a live NSView and these are CALayer value accessors.
        let frame: NSRect = unsafe { msg_send![&*layer, frame] };
        // SAFETY: `layer` is retained from a live NSView and these are CALayer value accessors.
        let bounds: NSRect = unsafe { msg_send![&*layer, bounds] };
        // SAFETY: `layer` is retained from a live NSView and these are CALayer value accessors.
        let position: objc2_foundation::NSPoint = unsafe { msg_send![&*layer, position] };
        // SAFETY: `layer` is retained from a live NSView and these are CALayer value accessors.
        let anchor: objc2_foundation::NSPoint = unsafe { msg_send![&*layer, anchorPoint] };
        format!(
            "frame=x={:.1} y={:.1} w={:.1} h={:.1} bounds=x={:.1} y={:.1} w={:.1} h={:.1} \
             position=x={:.1} y={:.1} anchor=x={:.2} y={:.2}",
            frame.origin.x,
            frame.origin.y,
            frame.size.width,
            frame.size.height,
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height,
            position.x,
            position.y,
            anchor.x,
            anchor.y,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn cocoa_view_class_count_uses_flexible_tail_array() {
            let size = u32::try_from(
                std::mem::size_of::<coreaudio_sys::CFURLRef>()
                    + 3 * std::mem::size_of::<coreaudio_sys::CFStringRef>(),
            )
            .unwrap();

            assert_eq!(cocoa_view_class_count(size), 3);
        }

        #[test]
        fn au_editor_audio_unit_argument_uses_modern_opaque_encoding() {
            assert!(
                AuOpaqueAudioComponentInstance::ENCODING_REF
                    .equivalent_to_str("^{OpaqueAudioComponentInstance=}")
            );
        }

        #[test]
        fn au_editor_audio_unit_argument_supports_legacy_component_instance_encoding() {
            assert!(
                AuComponentInstanceRecord::ENCODING_REF
                    .equivalent_to_str("^{ComponentInstanceRecord=[1q]}")
            );
        }

        #[test]
        fn au_editor_selects_unit_pointer_abi_from_factory_signature() {
            assert_eq!(
                cocoa_factory_unit_argument_abi("^{OpaqueAudioComponentInstance=}"),
                Some(AuUnitPointerAbi::Opaque)
            );
            assert_eq!(
                cocoa_factory_unit_argument_abi("^{ComponentInstanceRecord=[1q]}"),
                Some(AuUnitPointerAbi::ComponentInstanceRecord)
            );
            assert_eq!(cocoa_factory_unit_argument_abi("^v"), None);
        }

        #[test]
        fn au_editor_ignores_zero_sized_initial_view_frame() {
            assert_eq!(editor_size_from_ns_size(NSSize::new(0.0, 465.0)), None);
            assert_eq!(editor_size_from_ns_size(NSSize::new(775.0, 0.0)), None);
            assert_eq!(editor_size_from_ns_size(NSSize::new(1.0, 1.0)), None);
            assert_eq!(
                editor_size_from_ns_size(NSSize::new(775.0, 465.0)),
                Some(EditorSize {
                    width: 775,
                    height: 465,
                })
            );
        }

        #[test]
        fn cocoa_editor_view_autoresizing_mask_resizes_width_and_height() {
            let mask = cocoa_editor_autoresizing_mask();

            assert!(mask.contains(NSAutoresizingMaskOptions::ViewWidthSizable));
            assert!(mask.contains(NSAutoresizingMaskOptions::ViewHeightSizable));
        }

        #[test]
        fn au_editor_resizable_comes_from_autoresizing_mask() {
            assert!(cocoa_autoresizing_mask_resizable(
                NSAutoresizingMaskOptions::ViewWidthSizable
            ));
            assert!(cocoa_autoresizing_mask_resizable(
                NSAutoresizingMaskOptions::ViewHeightSizable
            ));
            assert!(!cocoa_autoresizing_mask_resizable(
                NSAutoresizingMaskOptions::empty()
            ));
        }

        #[test]
        fn au_resize_only_treats_single_direct_subview_as_content_wrapper() {
            assert!(should_resize_primary_content_subview(1));
            assert!(!should_resize_primary_content_subview(0));
            assert!(!should_resize_primary_content_subview(2));
        }

        #[test]
        fn au_editor_opts_into_native_content_resize_tracking() {
            assert!(au_editor_tracks_native_content_resize());
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use lilypalooza_audio::{EditorError, EditorSession};

    use super::{Arc, Mutex, runtime};

    pub(super) fn create_editor_session(
        _shared: Arc<Mutex<runtime::AuRuntime>>,
    ) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        Ok(None)
    }
}
