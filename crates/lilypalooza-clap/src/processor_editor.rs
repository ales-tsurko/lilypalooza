use super::{probe::*, runtime::*, *};

impl Processor for ClapProcessor {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .descriptor
    }

    fn set_param(&mut self, _id: &str, _normalized: f32) -> bool {
        false
    }

    fn get_param(&self, _id: &str) -> Option<f32> {
        None
    }

    fn save_state(&self) -> ProcessorState {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_state()
            .unwrap_or_default()
    }

    fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_state(state)
    }

    fn reset(&mut self) {
        self.midi.push_panic();
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reset();
    }

    fn latency_samples(&self) -> u32 {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latency_samples()
    }

    fn create_editor_session(&self) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        ClapController {
            shared: self.shared.clone(),
        }
        .create_editor_session()
    }
}

impl InstrumentProcessor for ClapProcessor {
    fn handle_midi(&mut self, event: MidiEvent) {
        self.midi.push(event);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let events = self.midi.take();
        let _ = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .process_block(None, None, left, right, &events);
    }
}

impl EffectProcessor for ClapProcessor {
    fn process(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        let events = self.midi.take();
        if !self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .process_block(Some(in_left), Some(in_right), out_left, out_right, &events)
        {
            out_left.copy_from_slice(in_left);
            out_right.copy_from_slice(in_right);
        }
    }
}

pub(super) fn midi_event_to_clap(event: MidiEvent) -> Option<clap_event_midi> {
    midi_event_data(event).map(clap_midi_event)
}

pub(super) fn midi_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    midi_note_event_data(event)
        .or_else(|| midi_controller_event_data(event))
        .or_else(|| midi_channel_event_data(event))
        .or_else(|| pitch_bend_event_data(event))
}

pub(super) fn midi_note_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    match event {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => Some([0x90 | (channel & 0x0f), note, velocity]),
        MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        } => Some([0x80 | (channel & 0x0f), note, velocity]),
        MidiEvent::PolyPressure {
            channel,
            note,
            pressure,
        } => Some([0xa0 | (channel & 0x0f), note, pressure]),
        _ => None,
    }
}

pub(super) fn midi_controller_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    match event {
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => Some([0xb0 | (channel & 0x0f), controller, value]),
        event => midi_channel_mode_event_data(event),
    }
}

pub(super) fn midi_channel_mode_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    match event {
        MidiEvent::AllNotesOff { channel } => Some([0xb0 | (channel & 0x0f), 123, 0]),
        MidiEvent::AllSoundOff { channel } => Some([0xb0 | (channel & 0x0f), 120, 0]),
        MidiEvent::ResetAllControllers { channel } => Some([0xb0 | (channel & 0x0f), 121, 0]),
        _ => None,
    }
}

pub(super) fn midi_channel_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    match event {
        MidiEvent::ProgramChange { channel, program } => {
            Some([0xc0 | (channel & 0x0f), program, 0])
        }
        MidiEvent::ChannelPressure { channel, pressure } => {
            Some([0xd0 | (channel & 0x0f), pressure, 0])
        }
        _ => None,
    }
}

pub(super) fn pitch_bend_event_data(event: MidiEvent) -> Option<[u8; 3]> {
    let MidiEvent::PitchBend { channel, value } = event else {
        return None;
    };
    let value = u16::try_from((i32::from(value) + 8192).clamp(0, 16_383)).unwrap_or_default();
    Some([
        0xe0 | (channel & 0x0f),
        (value & 0x7f) as u8,
        (value >> 7) as u8,
    ])
}

pub(super) fn clap_midi_event(data: [u8; 3]) -> clap_event_midi {
    clap_event_midi {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_midi>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: CLAP_EVENT_MIDI,
            flags: CLAP_EVENT_IS_LIVE,
        },
        port_index: 0,
        data,
    }
}

pub(super) struct ClapEditorSession {
    pub(super) shared: Arc<Mutex<ClapRuntimeInner>>,
    pub(super) created: bool,
    pub(super) attached: bool,
    pub(super) initial_size: Option<EditorSize>,
}

impl EditorSession for ClapEditorSession {
    fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(gui) = runtime.plugin_extension::<clap_plugin_gui>(CLAP_EXT_GUI) else {
            return Ok(None);
        };
        Ok(clap_gui_can_resize(gui, runtime.plugin.as_ptr()))
    }

    fn initial_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        trace_clap_editor(|| format!("session initial_size {:?}", self.initial_size));
        Ok(self.initial_size)
    }

    fn requested_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let requested = runtime.host.take_requested_gui_size();
        trace_clap_editor(|| format!("session requested_size {requested:?}"));
        Ok(requested)
    }

    fn set_resize_handler(
        &mut self,
        handler: Option<Arc<dyn EditorResizeHandler>>,
    ) -> Result<(), EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        runtime.host.set_resize_handler(handler);
        Ok(())
    }

    fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError> {
        let window = clap_window_for_parent(parent)?;
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prepare_clap_gui_for_parent(&mut self.created, &runtime, &window)?;
        let gui = clap_runtime_gui(&runtime)?;
        attach_clap_gui_parent(gui, &runtime, &window)?;
        self.initial_size = clap_initial_editor_size(gui, &runtime);
        trace_clap_editor(|| format!("session attach initial_size={:?}", self.initial_size));
        self.attached = true;
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(gui) = runtime.plugin_extension::<clap_plugin_gui>(CLAP_EXT_GUI) {
            clap_gui_destroy_created(gui, runtime.plugin.as_ptr(), self.created);
        }
        self.created = false;
        self.attached = false;
        runtime.host.set_resize_handler(None);
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), EditorError> {
        clap_gui_set_embedded_visible(visible)
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        let runtime = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(gui) = runtime.plugin_extension::<clap_plugin_gui>(CLAP_EXT_GUI) else {
            return Ok(size);
        };
        let mut size = size;
        if !clap_gui_adjusted_resize(gui, runtime.plugin.as_ptr(), &mut size) {
            trace_clap_editor(|| format!("session resize ignored requested={size:?}"));
            return Ok(size);
        }
        self.initial_size = Some(size);
        trace_clap_editor(|| format!("session resize applied accepted={size:?}"));
        Ok(size)
    }
}

pub(super) fn clap_runtime_gui(
    runtime: &ClapRuntimeInner,
) -> Result<&clap_plugin_gui, EditorError> {
    runtime
        .plugin_extension::<clap_plugin_gui>(CLAP_EXT_GUI)
        .ok_or(EditorError::Unsupported)
}

pub(super) fn prepare_clap_gui_for_parent<'a>(
    created: &mut bool,
    runtime: &'a ClapRuntimeInner,
    window: &clap_window,
) -> Result<&'a clap_plugin_gui, EditorError> {
    let gui = clap_runtime_gui(runtime)?;
    ensure_clap_gui_api_supported(gui, runtime, window)?;
    create_clap_gui_if_needed(created, gui, runtime, window)?;
    Ok(gui)
}

pub(super) fn ensure_clap_gui_api_supported(
    gui: &clap_plugin_gui,
    runtime: &ClapRuntimeInner,
    window: &clap_window,
) -> Result<(), EditorError> {
    let Some(is_api_supported) = gui.is_api_supported else {
        return Ok(());
    };
    // SAFETY: GUI extension table and API string are valid for this live plugin.
    if unsafe { is_api_supported(runtime.plugin.as_ptr(), window.api, false) } {
        Ok(())
    } else {
        Err(EditorError::Unsupported)
    }
}

pub(super) fn create_clap_gui_if_needed(
    created: &mut bool,
    gui: &clap_plugin_gui,
    runtime: &ClapRuntimeInner,
    window: &clap_window,
) -> Result<(), EditorError> {
    if *created {
        return Ok(());
    }
    let create = gui.create.ok_or(EditorError::Unsupported)?;
    // SAFETY: GUI is created once with an API supported by the host window.
    if unsafe { !create(runtime.plugin.as_ptr(), window.api, false) } {
        return Err(EditorError::Backend("CLAP GUI creation failed".to_string()));
    }
    *created = true;
    Ok(())
}

pub(super) fn attach_clap_gui_parent(
    gui: &clap_plugin_gui,
    runtime: &ClapRuntimeInner,
    window: &clap_window,
) -> Result<(), EditorError> {
    let set_parent = gui.set_parent.ok_or(EditorError::Unsupported)?;
    // SAFETY: The host parent window stays valid for the editor-host window lifetime.
    if unsafe { set_parent(runtime.plugin.as_ptr(), window) } {
        Ok(())
    } else {
        Err(EditorError::Backend(
            "CLAP GUI parenting failed".to_string(),
        ))
    }
}

pub(super) fn clap_initial_editor_size(
    gui: &clap_plugin_gui,
    runtime: &ClapRuntimeInner,
) -> Option<EditorSize> {
    runtime
        .host
        .take_requested_gui_size()
        .or_else(|| clap_gui_reported_size(gui, runtime.plugin.as_ptr()))
}

pub(super) fn clap_gui_destroy_created(
    gui: &clap_plugin_gui,
    plugin: *const clap_plugin,
    created: bool,
) {
    if created && let Some(destroy) = gui.destroy {
        // SAFETY: Destroy is paired with successful GUI creation.
        unsafe { destroy(plugin) };
    }
}

pub(super) fn clap_gui_set_embedded_visible(_visible: bool) -> Result<(), EditorError> {
    Ok(())
}

impl Drop for ClapEditorSession {
    fn drop(&mut self) {
        if self.created || self.attached {
            match self.detach() {
                Ok(()) | Err(_) => {}
            }
        }
    }
}

pub(super) fn clap_gui_reported_size(
    gui: &clap_plugin_gui,
    plugin: *const clap_plugin,
) -> Option<EditorSize> {
    let get_size = gui.get_size?;
    let mut width = 0;
    let mut height = 0;
    // SAFETY: The GUI extension belongs to the live plugin and receives valid out-pointers.
    if unsafe { !get_size(plugin, &mut width, &mut height) } || width == 0 || height == 0 {
        return None;
    }
    Some(EditorSize { width, height })
}

pub(super) fn clap_gui_can_resize(
    gui: &clap_plugin_gui,
    plugin: *const clap_plugin,
) -> Option<bool> {
    let can_resize = gui.can_resize?;
    // SAFETY: The GUI extension belongs to the live plugin.
    Some(unsafe { can_resize(plugin) })
}

pub(super) fn clap_gui_adjusted_resize(
    gui: &clap_plugin_gui,
    plugin: *const clap_plugin,
    size: &mut EditorSize,
) -> bool {
    if clap_gui_can_resize(gui, plugin) != Some(true) {
        return false;
    }

    let mut width = size.width;
    let mut height = size.height;
    if let Some(adjust_size) = gui.adjust_size {
        // SAFETY: The GUI extension belongs to the live plugin and receives valid out-pointers.
        if unsafe { !adjust_size(plugin, &mut width, &mut height) } || width == 0 || height == 0 {
            return false;
        }
    }

    let Some(set_size) = gui.set_size else {
        return false;
    };
    // SAFETY: The GUI extension belongs to the live plugin.
    if unsafe { !set_size(plugin, width, height) } {
        return false;
    }

    *size = EditorSize { width, height };
    true
}

pub(super) fn clap_window_for_parent(parent: EditorParent) -> Result<clap_window, EditorError> {
    match parent.window {
        RawWindowHandle::AppKit(handle) => Ok(clap_window {
            api: CLAP_WINDOW_API_COCOA.as_ptr(),
            specific: clap_window_handle {
                cocoa: handle.ns_view.as_ptr(),
            },
        }),
        RawWindowHandle::Win32(handle) => Ok(clap_window {
            api: CLAP_WINDOW_API_WIN32.as_ptr(),
            specific: clap_window_handle {
                win32: handle.hwnd.get() as *mut c_void,
            },
        }),
        RawWindowHandle::Xlib(handle) => Ok(clap_window {
            api: CLAP_WINDOW_API_X11.as_ptr(),
            specific: clap_window_handle { x11: handle.window },
        }),
        RawWindowHandle::Xcb(handle) => Ok(clap_window {
            api: CLAP_WINDOW_API_X11.as_ptr(),
            specific: clap_window_handle {
                x11: handle.window.get().into(),
            },
        }),
        RawWindowHandle::Wayland(handle) => Ok(clap_window {
            api: CLAP_WINDOW_API_WAYLAND.as_ptr(),
            specific: clap_window_handle {
                ptr: handle.surface.as_ptr().cast(),
            },
        }),
        other => Err(EditorError::HostUnavailable(format!(
            "unsupported CLAP editor parent: {other:?}"
        ))),
    }
}

/// Registers validated CLAP plugins in the shared audio registry.
pub fn register_plugins(plugins: impl IntoIterator<Item = ClapPluginMetadata>) {
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

pub(super) fn registry_entry_for_plugin(plugin: ClapPluginMetadata) -> registry::Entry {
    let descriptor = Box::leak(Box::new(ProcessorDescriptor {
        name: Box::leak(plugin.name.clone().into_boxed_str()),
        params: &[],
        editor: Some(DEFAULT_CLAP_EDITOR_DESCRIPTOR),
    }));
    let runtime = match plugin.role {
        registry::Role::Instrument => {
            registry::RuntimeFactory::Instrument(create_clap_instrument_runtime)
        }
        registry::Role::Effect => registry::RuntimeFactory::Effect(create_clap_effect_runtime),
    };
    registry::Entry::plugin_processor(
        plugin.processor_id,
        plugin.name,
        registry::Backend::Clap,
        plugin.vendor,
        descriptor,
        runtime,
    )
}

pub(super) fn create_clap_instrument_runtime(
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
        &slot.state,
    )?;
    Ok(Some(InstrumentRuntimeSpec {
        processor: Box::new(ClapProcessor {
            shared: shared.clone(),
            midi: ClapMidiEventQueue::new(),
        }),
        binding: Box::new(ClapBinding { shared }),
    }))
}

pub(super) fn create_clap_effect_runtime(
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
        &slot.state,
    )?;
    Ok(Some(EffectRuntimeSpec {
        processor: Box::new(ClapProcessor {
            shared: shared.clone(),
            midi: ClapMidiEventQueue::new(),
        }),
        binding: Some(Box::new(ClapBinding { shared })),
    }))
}

pub(super) fn metadata_and_descriptor(
    slot: &SlotState,
) -> Result<Option<(ClapPluginMetadata, &'static ProcessorDescriptor)>, RuntimeFactoryError> {
    let lilypalooza_audio::ProcessorKind::Plugin { plugin_id } = &slot.kind else {
        return Ok(None);
    };
    let metadata = plugin_metadata(plugin_id)
        .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    let descriptor = registry::entry(plugin_id)
        .map(|entry| entry.descriptor)
        .ok_or_else(|| {
            RuntimeFactoryError::Backend(format!("CLAP plugin `{plugin_id}` is not registered"))
        })?;
    Ok(Some((metadata, descriptor)))
}

pub(super) fn instantiate_shared(
    metadata: &ClapPluginMetadata,
    descriptor: &'static ProcessorDescriptor,
    sample_rate: usize,
    block_size: usize,
    state: &ProcessorState,
) -> Result<Arc<Mutex<ClapRuntimeInner>>, RuntimeFactoryError> {
    let mut runtime = ClapRuntimeInner::instantiate(metadata, descriptor, sample_rate, block_size)
        .map_err(|error| RuntimeFactoryError::Backend(error.to_string()))?;
    runtime
        .load_state(state)
        .map_err(RuntimeFactoryError::State)?;
    Ok(Arc::new(Mutex::new(runtime)))
}
