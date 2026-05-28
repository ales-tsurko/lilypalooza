use super::{host_com::*, probe::*, runtime::*, *};

impl Vst3EditorSession {
    pub(super) fn create_editor_view(&self) -> Result<ComPtr<IPlugView>, EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let controller = runtime
            .controller
            .as_ref()
            .ok_or(EditorError::Unsupported)?;
        trace_vst3(|| "editor attach createView start".to_string());
        // SAFETY: Controller is live and returns an owning view pointer or null.
        let view = unsafe { controller.createView(EDITOR_VIEW_NAME.as_ptr().cast()) };
        trace_vst3(|| "editor attach createView done".to_string());
        // SAFETY: Non-null view pointer is owned by the caller.
        unsafe { ComPtr::from_raw(view) }.ok_or(EditorError::Unsupported)
    }

    pub(super) fn set_editor_frame(&self, view: &ComPtr<IPlugView>) -> Result<(), EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let frame = runtime
            .host
            .to_com_ptr::<IPlugFrame>()
            .ok_or(EditorError::Unsupported)?;
        trace_vst3(|| "editor attach setFrame start".to_string());
        // SAFETY: View and frame are live for the editor window lifetime.
        let result = unsafe { view.setFrame(frame.as_ptr()) };
        trace_vst3(|| format!("editor attach setFrame done result={result}"));
        vst3_editor_result(result, "VST3 editor setFrame failed")
    }

    pub(super) fn detach_current_view(&mut self) -> Result<(), EditorError> {
        if let Some(view) = self.view.take() {
            trace_vst3_editor(|| format!("session detach current_size={:?}", self.current_size));
            remove_editor_view(&view)?;
            clear_editor_frame(&view)?;
        }
        Ok(())
    }
}

pub(super) fn ensure_editor_platform_supported(
    view: &ComPtr<IPlugView>,
    platform: *const c_char,
) -> Result<(), EditorError> {
    trace_vst3(|| "editor attach isPlatformTypeSupported start".to_string());
    // SAFETY: View is live and `platform` is a static null-terminated VST3 platform name.
    let result = unsafe { view.isPlatformTypeSupported(platform) };
    trace_vst3(|| format!("editor attach isPlatformTypeSupported done result={result}"));
    if result == kResultOk {
        Ok(())
    } else {
        Err(EditorError::Unsupported)
    }
}

pub(super) fn attach_editor_view_to_parent(
    view: &ComPtr<IPlugView>,
    parent: *mut c_void,
    platform: *const c_char,
) -> Result<(), EditorError> {
    trace_vst3(|| "editor attach attached start".to_string());
    // SAFETY: View and parent handle are live for the editor window lifetime.
    let result = unsafe { view.attached(parent, platform) };
    trace_vst3(|| format!("editor attach attached done result={result}"));
    vst3_editor_result(result, "VST3 editor attach failed")
}

pub(super) fn remove_editor_view(view: &ComPtr<IPlugView>) -> Result<(), EditorError> {
    trace_vst3(|| "editor detach removed start".to_string());
    // SAFETY: View was attached by this session and can be detached once.
    let result = unsafe { view.removed() };
    trace_vst3(|| format!("editor detach removed done result={result}"));
    vst3_editor_result(result, "VST3 editor remove failed")
}

pub(super) fn clear_editor_frame(view: &ComPtr<IPlugView>) -> Result<(), EditorError> {
    trace_vst3(|| "editor detach setFrame(null) start".to_string());
    // SAFETY: Clearing the frame is paired with the previous setFrame call.
    let result = unsafe { view.setFrame(std::ptr::null_mut()) };
    trace_vst3(|| format!("editor detach setFrame(null) done result={result}"));
    vst3_editor_result(result, "VST3 editor clear frame failed")
}

pub(super) fn vst3_editor_result(
    result: tresult,
    message: &'static str,
) -> Result<(), EditorError> {
    if result == kResultOk {
        Ok(())
    } else {
        Err(EditorError::Backend(message.to_string()))
    }
}

pub(super) fn trace_vst3_editor(message: impl FnOnce() -> String) {
    trace_vst3_prefixed("vst3-editor", message);
}

pub(super) fn trace_vst3(message: impl FnOnce() -> String) {
    trace_vst3_prefixed("vst3", message);
}

pub(super) fn trace_vst3_prefixed(prefix: &str, message: impl FnOnce() -> String) {
    log::trace!(
        target: "lilypalooza_vst3",
        "{prefix} thread={:?} {}",
        std::thread::current().id(),
        message()
    );
}

pub(super) fn attach_vst3_editor_view(
    session: &mut Vst3EditorSession,
    parent: EditorParent,
) -> Result<ComPtr<IPlugView>, EditorError> {
    let attachment = prepare_vst3_editor_attachment(session, parent)?;
    attach_editor_view_to_parent(&attachment.view, attachment.parent, attachment.platform)?;
    Ok(attachment.view)
}

pub(super) struct Vst3EditorAttachment {
    pub(super) view: ComPtr<IPlugView>,
    pub(super) parent: *mut c_void,
    pub(super) platform: FIDString,
}

pub(super) fn prepare_vst3_editor_attachment(
    session: &mut Vst3EditorSession,
    parent: EditorParent,
) -> Result<Vst3EditorAttachment, EditorError> {
    let (parent, platform) = vst3_parent_for_parent(parent)?;
    let view = session.create_editor_view()?;
    session.set_editor_frame(&view)?;
    ensure_editor_platform_supported(&view, platform)?;
    Ok(Vst3EditorAttachment {
        view,
        parent,
        platform,
    })
}

pub(super) fn changed_editor_size_request(
    current_size: &mut Option<EditorSize>,
    requested: Option<EditorSize>,
) -> Option<EditorSize> {
    let requested = requested?;
    if *current_size == Some(requested) {
        return None;
    }
    *current_size = Some(requested);
    Some(requested)
}

impl Drop for Vst3EditorSession {
    fn drop(&mut self) {
        match self.detach() {
            Ok(()) | Err(_) => {}
        }
    }
}

pub(super) fn vst3_view_size(view: &ComPtr<IPlugView>) -> Option<EditorSize> {
    let mut rect = zeroed::<ViewRect>();
    // SAFETY: View is live and writes its current size into `rect`.
    if unsafe { view.getSize(&mut rect) } != kResultOk {
        return None;
    }
    editor_size_from_rect(rect)
}

pub(super) fn editor_size_from_rect(rect: ViewRect) -> Option<EditorSize> {
    let width = editor_rect_axis_size(rect.left, rect.right)?;
    let height = editor_rect_axis_size(rect.top, rect.bottom)?;
    non_zero_editor_size(width, height)
}

pub(super) fn editor_rect_axis_size(start: int32, end: int32) -> Option<u32> {
    u32::try_from(end.checked_sub(start)?).ok()
}

pub(super) fn non_zero_editor_size(width: u32, height: u32) -> Option<EditorSize> {
    (width > 0 && height > 0).then_some(EditorSize { width, height })
}

pub(super) fn rect_from_editor_size(size: EditorSize) -> ViewRect {
    ViewRect {
        left: 0,
        top: 0,
        right: size.width as int32,
        bottom: size.height as int32,
    }
}

pub(super) unsafe fn call_plug_view_on_size(view: *mut IPlugView, size: EditorSize) -> tresult {
    if view.is_null() {
        return kInvalidArgument;
    }
    let mut rect = rect_from_editor_size(size);
    // SAFETY: `view` is supplied by the plugin to `IPlugFrame::resizeView`, and the VST3
    // resize sequence requires the host to call `IPlugView::onSize` after resizing the frame.
    let vtbl = unsafe { (*view).vtbl };
    // SAFETY: `view` is non-null and points to a valid VST3 plug view.
    let on_size = unsafe { (*vtbl).onSize };
    // SAFETY: The method pointer belongs to `view` and receives a valid rect pointer.
    unsafe { on_size(view, &mut rect) }
}

pub(super) fn format_view_rect(rect: ViewRect) -> String {
    format!(
        "left={} top={} right={} bottom={} w={} h={}",
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
        rect.right - rect.left,
        rect.bottom - rect.top
    )
}

pub(super) fn vst3_parent_for_parent(
    parent: EditorParent,
) -> Result<(*mut c_void, FIDString), EditorError> {
    appkit_vst3_parent(&parent.window)
        .or_else(|| win32_vst3_parent(&parent.window))
        .or_else(|| xlib_vst3_parent(&parent.window))
        .or_else(|| xcb_vst3_parent(&parent.window))
        .or_else(|| wayland_vst3_parent(&parent.window))
        .ok_or_else(|| {
            EditorError::HostUnavailable(format!(
                "unsupported VST3 editor parent: {:?}",
                parent.window
            ))
        })
}

pub(super) fn appkit_vst3_parent(window: &RawWindowHandle) -> Option<(*mut c_void, FIDString)> {
    let RawWindowHandle::AppKit(handle) = window else {
        return None;
    };
    Some((handle.ns_view.as_ptr().cast(), kPlatformTypeNSView))
}

pub(super) fn win32_vst3_parent(window: &RawWindowHandle) -> Option<(*mut c_void, FIDString)> {
    let RawWindowHandle::Win32(handle) = window else {
        return None;
    };
    Some((handle.hwnd.get() as *mut c_void, kPlatformTypeHWND))
}

pub(super) fn xlib_vst3_parent(window: &RawWindowHandle) -> Option<(*mut c_void, FIDString)> {
    let RawWindowHandle::Xlib(XlibWindowHandle { window, .. }) = window else {
        return None;
    };
    Some((
        *window as usize as *mut c_void,
        kPlatformTypeX11EmbedWindowID,
    ))
}

pub(super) fn xcb_vst3_parent(window: &RawWindowHandle) -> Option<(*mut c_void, FIDString)> {
    let RawWindowHandle::Xcb(handle) = window else {
        return None;
    };
    Some((
        handle.window.get() as usize as *mut c_void,
        kPlatformTypeX11EmbedWindowID,
    ))
}

pub(super) fn wayland_vst3_parent(window: &RawWindowHandle) -> Option<(*mut c_void, FIDString)> {
    let RawWindowHandle::Wayland(handle) = window else {
        return None;
    };
    Some((
        handle.surface.as_ptr().cast(),
        kPlatformTypeWaylandSurfaceID,
    ))
}

/// Registers validated VST3 plugins in the shared audio registry.
pub fn register_plugins(plugins: impl IntoIterator<Item = Vst3PluginMetadata>) {
    let plugins = plugins.into_iter().collect::<Vec<_>>();
    {
        let mut metadata = metadata_store()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for plugin in &plugins {
            metadata.insert(plugin.processor_id.clone(), plugin.clone());
        }
    }
    let entries = plugins.into_iter().map(registry_entry_for_plugin);
    registry::register(entries);
}

pub(super) fn registry_entry_for_plugin(plugin: Vst3PluginMetadata) -> registry::Entry {
    let descriptor = Box::leak(Box::new(ProcessorDescriptor {
        name: Box::leak(plugin.name.clone().into_boxed_str()),
        params: &[],
        editor: Some(DEFAULT_VST3_EDITOR_DESCRIPTOR),
    }));
    let runtime = match plugin.role {
        registry::Role::Instrument => {
            registry::RuntimeFactory::Instrument(create_vst3_instrument_runtime)
        }
        registry::Role::Effect => registry::RuntimeFactory::Effect(create_vst3_effect_runtime),
    };
    registry::Entry::plugin_processor(
        plugin.processor_id,
        plugin.name,
        registry::Backend::Vst3,
        plugin.vendor,
        descriptor,
        runtime,
    )
}

pub(super) fn create_vst3_instrument_runtime(
    slot: &SlotState,
    context: &InstrumentRuntimeContext<'_>,
) -> Result<Option<InstrumentRuntimeSpec>, RuntimeFactoryError> {
    let Some((metadata, descriptor)) = metadata_and_descriptor(slot)? else {
        return Ok(None);
    };
    let shared = instantiate_shared(
        &metadata,
        descriptor,
        usize::try_from(context.soundfont_settings.sample_rate.max(1)).unwrap_or(44_100),
        context.soundfont_settings.block_size.max(1),
        registry::Role::Instrument,
        &slot.state,
    )?;
    Ok(Some(InstrumentRuntimeSpec {
        processor: Box::new(Vst3Processor {
            shared: shared.clone(),
            midi: Vst3MidiEventQueue::new(),
        }),
        binding: Box::new(Vst3Binding { shared }),
    }))
}

pub(super) fn create_vst3_effect_runtime(
    slot: &SlotState,
    context: &EffectRuntimeContext,
) -> Result<Option<EffectRuntimeSpec>, RuntimeFactoryError> {
    let Some((metadata, descriptor)) = metadata_and_descriptor(slot)? else {
        return Ok(None);
    };
    let shared = instantiate_shared(
        &metadata,
        descriptor,
        context.sample_rate,
        context.block_size,
        registry::Role::Effect,
        &slot.state,
    )?;
    Ok(Some(EffectRuntimeSpec {
        processor: Box::new(Vst3Processor {
            shared: shared.clone(),
            midi: Vst3MidiEventQueue::new(),
        }),
        binding: Some(Box::new(Vst3Binding { shared })),
    }))
}

pub(super) fn metadata_and_descriptor(
    slot: &SlotState,
) -> Result<Option<(Vst3PluginMetadata, &'static ProcessorDescriptor)>, RuntimeFactoryError> {
    let lilypalooza_audio::ProcessorKind::Plugin { plugin_id } = &slot.kind else {
        return Ok(None);
    };
    let metadata = plugin_metadata(plugin_id)
        .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    let descriptor = registry::entry(plugin_id)
        .map(|entry| entry.descriptor)
        .ok_or_else(|| {
            RuntimeFactoryError::Backend(format!("VST3 plugin `{plugin_id}` is not registered"))
        })?;
    Ok(Some((metadata, descriptor)))
}

pub(super) fn instantiate_shared(
    metadata: &Vst3PluginMetadata,
    descriptor: &'static ProcessorDescriptor,
    sample_rate: usize,
    block_size: usize,
    role: registry::Role,
    state: &ProcessorState,
) -> Result<Arc<Mutex<Vst3RuntimeInner>>, RuntimeFactoryError> {
    let mut runtime =
        Vst3RuntimeInner::instantiate(metadata, descriptor, sample_rate, block_size, role)
            .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    runtime
        .load_state(state)
        .map_err(RuntimeFactoryError::State)?;
    Ok(Arc::new(Mutex::new(runtime)))
}
