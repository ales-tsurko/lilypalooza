use super::{
    probe::*,
    processor_editor::{ClapEditorSession, clap_midi_event, midi_event_to_clap},
    *,
};

pub(super) fn host_extension_for_id(id: &CStr) -> Option<*const c_void> {
    [
        (
            CLAP_EXT_GUI,
            (&HOST_GUI as *const clap_host_gui).cast::<c_void>(),
        ),
        (
            CLAP_EXT_LATENCY,
            (&HOST_LATENCY as *const clap_host_latency).cast::<c_void>(),
        ),
        (
            CLAP_EXT_STATE,
            (&HOST_STATE as *const clap_host_state).cast::<c_void>(),
        ),
        (
            CLAP_EXT_PARAMS,
            (&HOST_PARAMS as *const clap_host_params).cast::<c_void>(),
        ),
    ]
    .into_iter()
    .find_map(|(extension_id, extension)| (id == extension_id).then_some(extension))
}

pub(super) unsafe extern "C" fn host_request_restart(_host: *const clap_host) {}
pub(super) unsafe extern "C" fn host_request_process(_host: *const clap_host) {}
pub(super) unsafe extern "C" fn host_request_callback(_host: *const clap_host) {}

pub(super) static HOST_GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(host_gui_resize_hints_changed),
    request_resize: Some(host_gui_request_resize),
    request_show: Some(host_gui_request_show),
    request_hide: Some(host_gui_request_hide),
    closed: Some(host_gui_closed),
};

pub(super) static HOST_LATENCY: clap_host_latency = clap_host_latency {
    changed: Some(host_latency_changed),
};

pub(super) static HOST_STATE: clap_host_state = clap_host_state {
    mark_dirty: Some(host_state_mark_dirty),
};

pub(super) static HOST_PARAMS: clap_host_params = clap_host_params {
    rescan: Some(host_params_rescan),
    clear: Some(host_params_clear),
    request_flush: Some(host_params_request_flush),
};

pub(super) unsafe extern "C" fn host_gui_resize_hints_changed(_host: *const clap_host) {}
pub(super) unsafe extern "C" fn host_gui_request_resize(
    host: *const clap_host,
    width: u32,
    height: u32,
) -> bool {
    if host.is_null() {
        return false;
    }
    // SAFETY: CLAP passes back the host pointer we supplied; `host_data` stores `HostState`.
    // SAFETY: `host` was checked non-null above.
    let host_data = unsafe { (*host).host_data };
    // SAFETY: CLAP passes back the host pointer we supplied; `host_data` stores `HostState`.
    let state = unsafe { host_data.cast::<HostState>().as_ref() };
    state.is_some_and(|state| state.set_requested_gui_size(width, height))
}
pub(super) unsafe extern "C" fn host_gui_request_show(_host: *const clap_host) -> bool {
    true
}
pub(super) unsafe extern "C" fn host_gui_request_hide(_host: *const clap_host) -> bool {
    true
}
pub(super) unsafe extern "C" fn host_gui_closed(_host: *const clap_host, _was_destroyed: bool) {}
pub(super) unsafe extern "C" fn host_latency_changed(_host: *const clap_host) {}
pub(super) unsafe extern "C" fn host_state_mark_dirty(_host: *const clap_host) {}
pub(super) unsafe extern "C" fn host_params_rescan(_host: *const clap_host, _flags: u32) {}
pub(super) unsafe extern "C" fn host_params_clear(
    _host: *const clap_host,
    _param_id: u32,
    _flags: u32,
) {
}
pub(super) unsafe extern "C" fn host_params_request_flush(host: *const clap_host) {
    if host.is_null() {
        return;
    }
    // SAFETY: CLAP passes back the host pointer we supplied; `host_data` stores `HostState`.
    // SAFETY: `host` was checked non-null above.
    let host_data = unsafe { (*host).host_data };
    // SAFETY: CLAP passes back the host pointer we supplied; `host_data` stores `HostState`.
    if let Some(state) = unsafe { host_data.cast::<HostState>().as_ref() } {
        state.request_params_flush();
    }
}

pub(super) struct ClapRuntimeInner {
    pub(super) _module: Arc<LoadedModule>,
    pub(super) host: Box<HostContext>,
    pub(super) plugin: NonNull<clap_plugin>,
    pub(super) descriptor: &'static ProcessorDescriptor,
    pub(super) process: unsafe extern "C" fn(*const clap_plugin, *const clap_process) -> i32,
    pub(super) activated: bool,
    pub(super) processing: bool,
    pub(super) destroyed: bool,
}

// SAFETY: The runtime is always shared behind a `Mutex`; raw CLAP pointers are accessed only while
// holding that mutex and remain valid until the paired destroy call in `Drop`.
unsafe impl Send for ClapRuntimeInner {}

pub(super) struct ClapPluginInstance {
    pub(super) module: Arc<LoadedModule>,
    pub(super) host: Box<HostContext>,
    pub(super) plugin: NonNull<clap_plugin>,
    pub(super) process: unsafe extern "C" fn(*const clap_plugin, *const clap_process) -> i32,
}

pub(super) fn create_clap_plugin_instance(
    metadata: &ClapPluginMetadata,
) -> Result<ClapPluginInstance, ClapRuntimeError> {
    let module = load_module(&metadata.path, &metadata.library_path)?;
    let host = HostContext::new();
    let plugin = create_clap_plugin(&module, &host, &metadata.clap_id)?;
    init_clap_plugin(plugin)?;
    let process = clap_plugin_process(plugin)?;
    Ok(ClapPluginInstance {
        module,
        host,
        plugin,
        process,
    })
}

pub(super) fn create_clap_plugin(
    module: &LoadedModule,
    host: &HostContext,
    clap_id: &str,
) -> Result<NonNull<clap_plugin>, ClapRuntimeError> {
    let clap_id_c = CString::new(clap_id)
        .map_err(|_error| ClapRuntimeError::InvalidPluginId(clap_id.to_string()))?;
    let factory = module.plugin_factory()?;
    // SAFETY: Factory pointer comes from the initialized CLAP entry and is checked for null.
    let factory_ref = unsafe { factory.as_ref() }.ok_or(ClapRuntimeError::MissingFactory)?;
    let create_plugin = factory_ref
        .create_plugin
        .ok_or(ClapRuntimeError::MissingFunction("create_plugin"))?;
    // SAFETY: Factory and host pointers are valid for this call. `clap_id_c` is NUL-terminated.
    let plugin = unsafe { create_plugin(factory, host.as_ptr(), clap_id_c.as_ptr()) };
    NonNull::new(plugin as *mut clap_plugin).ok_or(ClapRuntimeError::CreatePluginFailed)
}

pub(super) fn init_clap_plugin(plugin: NonNull<clap_plugin>) -> Result<(), ClapRuntimeError> {
    // SAFETY: The plugin pointer comes from the CLAP factory and is checked non-null by `NonNull`.
    let plugin_ref = unsafe { plugin.as_ref() };
    let init = plugin_ref
        .init
        .ok_or(ClapRuntimeError::MissingFunction("plugin.init"))?;
    // SAFETY: CLAP plugin init is called once before activation.
    if unsafe { !init(plugin.as_ptr()) } {
        return Err(ClapRuntimeError::PluginInitFailed);
    }
    Ok(())
}

pub(super) fn clap_plugin_process(
    plugin: NonNull<clap_plugin>,
) -> Result<unsafe extern "C" fn(*const clap_plugin, *const clap_process) -> i32, ClapRuntimeError>
{
    // SAFETY: The plugin pointer comes from the CLAP factory and is checked non-null by `NonNull`.
    let plugin_ref = unsafe { plugin.as_ref() };
    plugin_ref.process.ok_or(ClapRuntimeError::MissingProcess)
}

impl ClapRuntimeInner {
    pub(super) fn instantiate(
        metadata: &ClapPluginMetadata,
        descriptor: &'static ProcessorDescriptor,
        sample_rate: usize,
        block_size: usize,
    ) -> Result<Self, ClapRuntimeError> {
        let instance = create_clap_plugin_instance(metadata)?;
        let mut runtime = Self {
            _module: instance.module,
            host: instance.host,
            plugin: instance.plugin,
            descriptor,
            process: instance.process,
            activated: false,
            processing: false,
            destroyed: false,
        };
        runtime.activate(sample_rate, block_size)?;
        Ok(runtime)
    }

    pub(super) fn activate(
        &mut self,
        sample_rate: usize,
        block_size: usize,
    ) -> Result<(), ClapRuntimeError> {
        if self.destroyed {
            return Err(ClapRuntimeError::PluginInitFailed);
        }
        // SAFETY: `plugin` is a live CLAP plugin pointer.
        let plugin = unsafe { self.plugin.as_ref() };
        self.activate_plugin(plugin, sample_rate, block_size)?;
        self.start_plugin_processing(plugin)?;
        Ok(())
    }

    pub(super) fn activate_plugin(
        &mut self,
        plugin: &clap_plugin,
        sample_rate: usize,
        block_size: usize,
    ) -> Result<(), ClapRuntimeError> {
        let Some(activate) = plugin.activate else {
            return Ok(());
        };
        // SAFETY: Activation uses the current engine settings and a valid plugin pointer.
        if unsafe {
            !activate(
                self.plugin.as_ptr(),
                sample_rate.max(1) as f64,
                1,
                block_size.max(1) as u32,
            )
        } {
            return Err(ClapRuntimeError::ActivateFailed);
        }
        self.activated = true;
        Ok(())
    }

    pub(super) fn start_plugin_processing(
        &mut self,
        plugin: &clap_plugin,
    ) -> Result<(), ClapRuntimeError> {
        let Some(start_processing) = plugin.start_processing else {
            return Ok(());
        };
        // SAFETY: Called after successful initialization/activation.
        if unsafe { !start_processing(self.plugin.as_ptr()) } {
            return Err(ClapRuntimeError::StartProcessingFailed);
        }
        self.processing = true;
        Ok(())
    }

    pub(super) fn plugin_extension<T>(&self, id: &CStr) -> Option<&T> {
        if self.destroyed {
            return None;
        }
        // SAFETY: `plugin` is live and extension pointer is checked for null.
        let plugin = unsafe { self.plugin.as_ref() };
        let get_extension = plugin.get_extension?;
        // SAFETY: CLAP extension id is a static NUL-terminated string.
        let extension = unsafe { get_extension(self.plugin.as_ptr(), id.as_ptr()) };
        // SAFETY: CLAP returns an extension table matching the requested id.
        unsafe { (extension as *const T).as_ref() }
    }

    pub(super) fn process_block(
        &mut self,
        input_left: Option<&[f32]>,
        input_right: Option<&[f32]>,
        output_left: &mut [f32],
        output_right: &mut [f32],
        events: &[clap_event_midi],
    ) -> bool {
        if self.destroyed {
            return false;
        }
        let _ = clap_flush_params_if_requested(
            &self.host,
            self.plugin.as_ptr(),
            self.plugin_extension::<clap_plugin_params>(CLAP_EXT_PARAMS),
        );
        let frames = output_left.len().min(output_right.len());
        let in_left = input_left
            .map(|input| input.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());
        let in_right = input_right
            .map(|input| input.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());
        let out_left = output_left.as_mut_ptr();
        let out_right = output_right.as_mut_ptr();
        let mut input_channels = [in_left, in_right];
        let mut output_channels = [out_left, out_right];
        let input_buffer = clap_audio_buffer {
            data32: input_channels.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut output_buffer = clap_audio_buffer {
            data32: output_channels.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut event_list = ClapInputEventList { events };
        let in_events = clap_input_events {
            ctx: (&mut event_list as *mut ClapInputEventList<'_>).cast(),
            size: Some(clap_input_events_size),
            get: Some(clap_input_events_get),
        };
        let out_events = clap_output_events {
            ctx: std::ptr::null_mut(),
            try_push: Some(clap_output_events_try_push),
        };
        let process = clap_process {
            steady_time: -1,
            frames_count: frames as u32,
            transport: std::ptr::null(),
            audio_inputs: input_left.map_or(std::ptr::null(), |_| &input_buffer),
            audio_outputs: &mut output_buffer,
            audio_inputs_count: u32::from(input_left.is_some()),
            audio_outputs_count: 1,
            in_events: &in_events,
            out_events: &out_events,
        };
        // SAFETY: The process struct and buffers remain valid for the duration of the call.
        unsafe { (self.process)(self.plugin.as_ptr(), &process) != CLAP_PROCESS_ERROR }
    }

    pub(super) fn reset(&mut self) {
        if self.destroyed {
            return;
        }
        // SAFETY: `plugin` is live while the runtime exists.
        if let Some(reset) = unsafe { self.plugin.as_ref() }.reset {
            // SAFETY: Reset is a CLAP callback on a live plugin.
            unsafe { reset(self.plugin.as_ptr()) };
        }
    }

    pub(super) fn latency_samples(&self) -> u32 {
        self.plugin_extension::<clap_plugin_latency>(CLAP_EXT_LATENCY)
            .and_then(|extension| extension.get)
            .map_or(0, |get| {
                // SAFETY: Latency extension table belongs to this live plugin.
                unsafe { get(self.plugin.as_ptr()) }
            })
    }

    pub(super) fn parameters(&self) -> Vec<lilypalooza_audio::ParameterInfo> {
        let Some(params) = self.plugin_extension::<clap_plugin_params>(CLAP_EXT_PARAMS) else {
            return self
                .descriptor
                .params
                .iter()
                .map(lilypalooza_audio::ParameterInfo::from)
                .collect();
        };
        let (Some(count), Some(get_info)) = (params.count, params.get_info) else {
            return Vec::new();
        };
        // SAFETY: Params extension table belongs to this live plugin.
        let count = unsafe { count(self.plugin.as_ptr()) };
        let mut out = Vec::new();
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<clap_param_info>::zeroed();
            // SAFETY: `info` points to writable storage for the CLAP parameter info result.
            if !unsafe { get_info(self.plugin.as_ptr(), index, info.as_mut_ptr()) } {
                continue;
            }
            // SAFETY: CLAP returned success and initialized the output struct.
            let info = unsafe { info.assume_init() };
            if info.flags & CLAP_PARAM_IS_HIDDEN != 0 {
                continue;
            }
            out.push(clap_parameter_info(info));
        }
        out
    }

    pub(super) fn get_param(&self, id: u32) -> Result<f32, ControllerError> {
        let params = self
            .plugin_extension::<clap_plugin_params>(CLAP_EXT_PARAMS)
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))?;
        let get_value = params
            .get_value
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))?;
        let info = self
            .parameter_info_by_id(id)
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))?;
        let mut value = 0.0;
        // SAFETY: Params extension table belongs to this live plugin and `value` is writable.
        if !unsafe { get_value(self.plugin.as_ptr(), id, &mut value) } {
            return Err(ControllerError::UnknownParameter(id.to_string()));
        }
        Ok(normalize_clap_parameter_value(&info, value))
    }

    pub(super) fn set_param(&self, id: u32, normalized: f32) -> Result<(), ControllerError> {
        let info = self
            .parameter_info_by_id(id)
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))?;
        if info.flags & CLAP_PARAM_IS_READONLY != 0 {
            return Err(ControllerError::Backend(format!(
                "CLAP parameter {id} is read-only"
            )));
        }
        let value = denormalize_clap_parameter_value(&info, normalized);
        let event = clap_event_param_value {
            header: clap_param_event_header(
                CLAP_EVENT_PARAM_VALUE,
                std::mem::size_of::<clap_event_param_value>(),
            ),
            param_id: id,
            cookie: info.cookie,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        };
        self.flush_single_param_event(&event.header)
    }

    pub(super) fn flush_param_gesture(
        &self,
        id: u32,
        event_type: u16,
    ) -> Result<(), ControllerError> {
        let event = clap_event_param_gesture {
            header: clap_param_event_header(
                event_type,
                std::mem::size_of::<clap_event_param_gesture>(),
            ),
            param_id: id,
        };
        self.flush_single_param_event(&event.header)
    }

    fn flush_single_param_event(&self, event: &clap_event_header) -> Result<(), ControllerError> {
        let Some(flush) = self
            .plugin_extension::<clap_plugin_params>(CLAP_EXT_PARAMS)
            .and_then(|params| params.flush)
        else {
            return Err(ControllerError::Backend(
                "CLAP params.flush is unavailable".to_string(),
            ));
        };
        let mut event_list = SingleClapEventList { event };
        let in_events = clap_input_events {
            ctx: (&mut event_list as *mut SingleClapEventList<'_>).cast(),
            size: Some(single_clap_event_list_size),
            get: Some(single_clap_event_list_get),
        };
        let out_events = clap_output_events {
            ctx: std::ptr::null_mut(),
            try_push: Some(clap_output_events_try_push),
        };
        // SAFETY: The plugin pointer and event lists are valid for the duration of the call.
        unsafe { flush(self.plugin.as_ptr(), &in_events, &out_events) };
        Ok(())
    }

    fn parameter_info_by_id(&self, id: u32) -> Option<clap_param_info> {
        let params = self.plugin_extension::<clap_plugin_params>(CLAP_EXT_PARAMS)?;
        let (Some(count), Some(get_info)) = (params.count, params.get_info) else {
            return None;
        };
        // SAFETY: Params extension table belongs to this live plugin.
        let count = unsafe { count(self.plugin.as_ptr()) };
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<clap_param_info>::zeroed();
            // SAFETY: `info` points to writable storage for the CLAP parameter info result.
            if unsafe { get_info(self.plugin.as_ptr(), index, info.as_mut_ptr()) } {
                // SAFETY: CLAP returned success and initialized the output struct.
                let info = unsafe { info.assume_init() };
                if info.id == id {
                    return Some(info);
                }
            }
        }
        None
    }

    pub(super) fn save_state(&mut self) -> Result<ProcessorState, ControllerError> {
        let Some(state) = self.plugin_extension::<clap_plugin_state>(CLAP_EXT_STATE) else {
            return Ok(ProcessorState::default());
        };
        let Some(save) = state.save else {
            return Ok(ProcessorState::default());
        };
        let mut bytes = Vec::new();
        let stream = clap_ostream {
            ctx: (&mut bytes as *mut Vec<u8>).cast(),
            write: Some(ostream_write),
        };
        // SAFETY: Stream callback appends to `bytes`, which outlives the call.
        if unsafe { save(self.plugin.as_ptr(), &stream) } {
            Ok(ProcessorState(bytes))
        } else {
            Err(ControllerError::Backend(
                "CLAP state save failed".to_string(),
            ))
        }
    }

    pub(super) fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        if state.0.is_empty() {
            return Ok(());
        }
        let Some(extension) = self.plugin_extension::<clap_plugin_state>(CLAP_EXT_STATE) else {
            return Ok(());
        };
        let Some(load) = extension.load else {
            return Ok(());
        };
        let mut input = InputStreamState {
            bytes: &state.0,
            offset: 0,
        };
        let stream = clap_istream {
            ctx: (&mut input as *mut InputStreamState<'_>).cast(),
            read: Some(istream_read),
        };
        // SAFETY: Stream callback reads from `state`, which outlives the call.
        if unsafe { load(self.plugin.as_ptr(), &stream) } {
            Ok(())
        } else {
            Err(ProcessorStateError::Decode(
                "CLAP state load failed".to_string(),
            ))
        }
    }

    pub(super) fn prepare_destroy(&mut self) {
        if self.destroyed {
            return;
        }
        // SAFETY: `plugin` is live until `destroy` is called below.
        let plugin = unsafe { self.plugin.as_ref() };
        self.stop_processing_for_destroy(plugin);
        self.deactivate_for_destroy(plugin);
        self.destroy_plugin(plugin);
        self.destroyed = true;
    }

    pub(super) fn stop_processing_for_destroy(&mut self, plugin: &clap_plugin) {
        if self.processing
            && let Some(stop_processing) = plugin.stop_processing
        {
            // SAFETY: Called once before deactivation/destroy.
            unsafe { stop_processing(self.plugin.as_ptr()) };
        }
        self.processing = false;
    }

    pub(super) fn deactivate_for_destroy(&mut self, plugin: &clap_plugin) {
        if self.activated
            && let Some(deactivate) = plugin.deactivate
        {
            // SAFETY: Paired with successful activation.
            unsafe { deactivate(self.plugin.as_ptr()) };
        }
        self.activated = false;
    }

    pub(super) fn destroy_plugin(&mut self, plugin: &clap_plugin) {
        if let Some(destroy) = plugin.destroy {
            // SAFETY: Final plugin lifecycle call. No further plugin access happens after this.
            unsafe { destroy(self.plugin.as_ptr()) };
        }
    }
}

impl LoadedModule {
    pub(super) fn plugin_factory(&self) -> Result<*const clap_plugin_factory, ClapRuntimeError> {
        // SAFETY: `entry` points into a loaded library kept alive by `self`.
        let entry = unsafe { self.entry.as_ref() };
        let get_factory = entry
            .get_factory
            .ok_or(ClapRuntimeError::MissingFunction("get_factory"))?;
        // SAFETY: Factory id is a static C string.
        let factory = unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
        if factory.is_null() {
            Err(ClapRuntimeError::MissingFactory)
        } else {
            Ok(factory.cast())
        }
    }
}

impl Drop for ClapRuntimeInner {
    fn drop(&mut self) {
        self.prepare_destroy();
    }
}

pub(super) struct ClapInputEventList<'a> {
    pub(super) events: &'a [clap_event_midi],
}

unsafe fn clap_input_event_list<'a>(
    list: *const clap_input_events,
) -> Option<&'a ClapInputEventList<'a>> {
    // SAFETY: callers pass the CLAP list pointer from the active process callback.
    let list = unsafe { list.as_ref() }?;
    // SAFETY: `ctx` is set to a valid `ClapInputEventList` for the process call.
    unsafe { (list.ctx as *const ClapInputEventList<'a>).as_ref() }
}

pub(super) unsafe extern "C" fn clap_input_events_size(list: *const clap_input_events) -> u32 {
    if list.is_null() {
        return 0;
    }
    // SAFETY: `ctx` is set to a valid `ClapInputEventList` for the process call.
    unsafe { clap_input_event_list(list) }.map_or(0, |list| list.events.len() as u32)
}

pub(super) unsafe extern "C" fn clap_input_events_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    if list.is_null() {
        return std::ptr::null();
    }
    // SAFETY: `ctx` is set to a valid `ClapInputEventList` for the process call.
    let Some(list) = (unsafe { clap_input_event_list(list) }) else {
        return std::ptr::null();
    };
    list.events
        .get(index as usize)
        .map_or(std::ptr::null(), |event| &event.header)
}

pub(super) unsafe extern "C" fn clap_output_events_try_push(
    _list: *const clap_output_events,
    _event: *const clap_event_header,
) -> bool {
    true
}

pub(super) struct SingleClapEventList<'a> {
    pub(super) event: &'a clap_event_header,
}

unsafe fn single_clap_event_list<'a>(
    list: *const clap_input_events,
) -> Option<&'a SingleClapEventList<'a>> {
    // SAFETY: callers pass the CLAP list pointer created for one synchronous flush call.
    let list = unsafe { list.as_ref() }?;
    // SAFETY: `ctx` is set to a valid `SingleClapEventList` for the flush call.
    unsafe { (list.ctx as *const SingleClapEventList<'a>).as_ref() }
}

pub(super) unsafe extern "C" fn single_clap_event_list_size(list: *const clap_input_events) -> u32 {
    if list.is_null() {
        return 0;
    }
    // SAFETY: `ctx` is set to a valid `SingleClapEventList` for the flush call.
    u32::from(unsafe { single_clap_event_list(list) }.is_some())
}

pub(super) unsafe extern "C" fn single_clap_event_list_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    if index != 0 {
        return std::ptr::null();
    }
    // SAFETY: `ctx` is set to a valid `SingleClapEventList` for the flush call.
    unsafe { single_clap_event_list(list) }.map_or(std::ptr::null(), |list| list.event)
}

pub(super) fn parse_clap_param_id(id: &str) -> Result<u32, ControllerError> {
    id.parse::<u32>()
        .map_err(|_error| ControllerError::UnknownParameter(id.to_string()))
}

pub(super) fn clap_parameter_info(info: clap_param_info) -> lilypalooza_audio::ParameterInfo {
    let name = clap_char_array_to_string(&info.name);
    lilypalooza_audio::ParameterInfo {
        id: info.id.to_string(),
        name: if name.is_empty() {
            format!("Parameter {}", info.id)
        } else {
            name
        },
        default: normalize_clap_parameter_value(&info, info.default_value),
        automatable: info.flags & CLAP_PARAM_IS_AUTOMATABLE != 0,
        readonly: info.flags & CLAP_PARAM_IS_READONLY != 0,
    }
}

pub(super) fn normalize_clap_parameter_value(info: &clap_param_info, value: f64) -> f32 {
    let range = info.max_value - info.min_value;
    if !range.is_finite() || range.abs() <= f64::EPSILON {
        return 0.0;
    }
    ((value - info.min_value) / range).clamp(0.0, 1.0) as f32
}

pub(super) fn denormalize_clap_parameter_value(info: &clap_param_info, normalized: f32) -> f64 {
    let range = info.max_value - info.min_value;
    if !range.is_finite() || range.abs() <= f64::EPSILON {
        return info.default_value;
    }
    info.min_value + range * f64::from(normalized.clamp(0.0, 1.0))
}

pub(super) fn clap_param_event_header(event_type: u16, size: usize) -> clap_event_header {
    clap_event_header {
        size: size as u32,
        time: 0,
        space_id: CLAP_CORE_EVENT_SPACE_ID,
        type_: event_type,
        flags: CLAP_EVENT_IS_LIVE,
    }
}

pub(super) fn clap_char_array_to_string(bytes: &[c_char]) -> String {
    let len = bytes
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(bytes.len());
    let bytes = bytes
        .get(..len)
        .unwrap_or(bytes)
        .iter()
        .map(|value| value.cast_unsigned())
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

pub(super) fn clap_flush_params_if_requested(
    host: &HostContext,
    plugin: *const clap_plugin,
    params: Option<&clap_plugin_params>,
) -> bool {
    if !host.take_params_flush_request() {
        return false;
    }
    let Some(flush) = params.and_then(|params| params.flush) else {
        return false;
    };
    let events: [clap_event_midi; 0] = [];
    let mut event_list = ClapInputEventList { events: &events };
    let in_events = clap_input_events {
        ctx: (&mut event_list as *mut ClapInputEventList<'_>).cast(),
        size: Some(clap_input_events_size),
        get: Some(clap_input_events_get),
    };
    let out_events = clap_output_events {
        ctx: std::ptr::null_mut(),
        try_push: Some(clap_output_events_try_push),
    };
    // SAFETY: The plugin pointer and event lists are valid for the duration of the call.
    unsafe { flush(plugin, &in_events, &out_events) };
    true
}

pub(super) unsafe extern "C" fn ostream_write(
    stream: *const clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    // SAFETY: `stream` was checked non-null above.
    let ctx = unsafe { (*stream).ctx };
    // SAFETY: `ctx` points to the Vec owned by `save_state` for this callback.
    let bytes = unsafe { &mut *(ctx as *mut Vec<u8>) };
    // SAFETY: CLAP provides a readable buffer of `size` bytes for the callback.
    let input = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), size as usize) };
    bytes.extend_from_slice(input);
    size as i64
}

pub(super) struct InputStreamState<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) offset: usize,
}

pub(super) unsafe extern "C" fn istream_read(
    stream: *const clap_istream,
    buffer: *mut c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    // SAFETY: `stream` was checked non-null above.
    let ctx = unsafe { (*stream).ctx };
    // SAFETY: `ctx` points to the input stream state owned by `load_state`.
    let input = unsafe { &mut *(ctx as *mut InputStreamState<'_>) };
    let remaining = input.bytes.len().saturating_sub(input.offset);
    let to_copy = remaining.min(size as usize);
    let source = input.bytes.get(input.offset..).unwrap_or(&[]);
    // SAFETY: CLAP provides a writable output buffer of `size` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(source.as_ptr(), buffer.cast::<u8>(), to_copy);
    }
    input.offset += to_copy;
    to_copy as i64
}

#[derive(Clone)]
pub(super) struct ClapBinding {
    pub(super) shared: Arc<Mutex<ClapRuntimeInner>>,
}

impl RuntimeBinding for ClapBinding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(ClapController {
            shared: self.shared.clone(),
        })
    }

    fn latency_samples(&self) -> u32 {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latency_samples()
    }

    fn prepare_destroy(&self) {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .prepare_destroy();
    }
}

pub(super) struct ClapController {
    pub(super) shared: Arc<Mutex<ClapRuntimeInner>>,
}

impl Controller for ClapController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .descriptor
    }

    fn parameters(&self) -> Vec<lilypalooza_audio::ParameterInfo> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parameters()
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        let param_id = parse_clap_param_id(id)?;
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_param(param_id)
    }

    fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        let param_id = parse_clap_param_id(id)?;
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_param(param_id, normalized)
    }

    fn begin_edit(&self, id: &str) -> Result<(), ControllerError> {
        let param_id = parse_clap_param_id(id)?;
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flush_param_gesture(param_id, CLAP_EVENT_PARAM_GESTURE_BEGIN)
    }

    fn end_edit(&self, id: &str) -> Result<(), ControllerError> {
        let param_id = parse_clap_param_id(id)?;
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flush_param_gesture(param_id, CLAP_EVENT_PARAM_GESTURE_END)
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_state()
    }

    fn load_state(&self, state: &ProcessorState) -> Result<(), ControllerError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .load_state(state)
            .map_err(|error| ControllerError::Backend(error.to_string()))
    }

    fn create_editor_session(&self) -> Result<Option<Box<dyn EditorSession>>, EditorError> {
        let has_gui = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .plugin_extension::<clap_plugin_gui>(CLAP_EXT_GUI)
            .is_some();
        Ok(has_gui.then(|| {
            Box::new(ClapEditorSession {
                shared: self.shared.clone(),
                created: false,
                attached: false,
                initial_size: None,
            }) as Box<dyn EditorSession>
        }))
    }
}

pub(super) struct ClapProcessor {
    pub(super) shared: Arc<Mutex<ClapRuntimeInner>>,
    pub(super) midi: ClapMidiEventQueue,
}

pub(super) struct ClapMidiEventQueue {
    pub(super) pending: Vec<clap_event_midi>,
    pub(super) active_notes: [[bool; 128]; 16],
}

impl ClapMidiEventQueue {
    pub(super) fn new() -> Self {
        Self {
            pending: Vec::new(),
            active_notes: [[false; 128]; 16],
        }
    }

    pub(super) fn push(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => self.push_note_on(channel, note, velocity),
            MidiEvent::NoteOff {
                channel,
                note,
                velocity,
            } => self.push_note_off(channel, note, velocity),
            MidiEvent::AllNotesOff { channel } => self.push_all_notes_off(channel),
            MidiEvent::AllSoundOff { channel } => self.push_all_sound_off(channel),
            event => self.push_other_event(event),
        }
    }

    pub(super) fn push_note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        if velocity == 0 {
            self.push_note_off(channel, note, 0);
            return;
        }
        self.set_active_note(channel, note, true);
        self.pending
            .push(clap_midi_event([0x90 | (channel & 0x0f), note, velocity]));
    }

    pub(super) fn push_note_off(&mut self, channel: u8, note: u8, velocity: u8) {
        self.set_active_note(channel, note, false);
        self.pending
            .push(clap_midi_event([0x80 | (channel & 0x0f), note, velocity]));
    }

    pub(super) fn push_all_notes_off(&mut self, channel: u8) {
        self.push_active_note_offs(channel);
        self.pending
            .push(clap_midi_event([0xb0 | (channel & 0x0f), 123, 0]));
    }

    pub(super) fn push_all_sound_off(&mut self, channel: u8) {
        self.push_active_note_offs(channel);
        self.pending
            .push(clap_midi_event([0xb0 | (channel & 0x0f), 120, 0]));
    }

    pub(super) fn push_other_event(&mut self, event: MidiEvent) {
        if let Some(event) = midi_event_to_clap(event) {
            self.pending.push(event);
        }
    }

    pub(super) fn set_active_note(&mut self, channel: u8, note: u8, value: bool) {
        if let Some(active) = self.active_note_mut(channel, note) {
            *active = value;
        }
    }

    pub(super) fn push_active_note_offs(&mut self, channel: u8) {
        let channel = channel & 0x0f;
        let Some(notes) = self.active_notes.get_mut(usize::from(channel)) else {
            return;
        };
        for (note, active) in notes.iter_mut().enumerate() {
            if *active {
                let note = u8::try_from(note).unwrap_or(0);
                self.pending
                    .push(clap_midi_event([0x80 | channel, note, 0]));
                *active = false;
            }
        }
    }

    pub(super) fn active_note_mut(&mut self, channel: u8, note: u8) -> Option<&mut bool> {
        self.active_notes
            .get_mut(usize::from(channel & 0x0f))?
            .get_mut(usize::from(note))
    }

    pub(super) fn push_panic(&mut self) {
        for channel in 0..16_u8 {
            self.push_active_note_offs(channel);
            self.pending.push(clap_midi_event([0xb0 | channel, 120, 0]));
            self.pending.push(clap_midi_event([0xb0 | channel, 123, 0]));
            self.pending.push(clap_midi_event([0xb0 | channel, 121, 0]));
        }
    }

    pub(super) fn take(&mut self) -> Vec<clap_event_midi> {
        std::mem::take(&mut self.pending)
    }
}
