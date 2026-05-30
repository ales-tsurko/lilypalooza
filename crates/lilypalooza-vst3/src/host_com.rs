use super::{
    editor::{
        call_plug_view_on_size, editor_size_from_rect, format_view_rect, trace_vst3,
        trace_vst3_editor,
    },
    probe::*,
    runtime::{
        activate_buses, connect_component_and_controller, create_controller,
        log_vst3_lifecycle_result, process_input_count, process_inputs_ptr,
        sync_controller_component_state, trace_process_done, trace_process_start,
    },
    *,
};

impl Class for Vst3AttributeList {
    type Interfaces = (IAttributeList,);
}

impl IAttributeListTrait for Vst3AttributeList {
    unsafe fn setInt(&self, id: IAttrID, value: int64) -> tresult {
        self.set_attr(id, Vst3AttributeValue::Int(value))
    }

    unsafe fn getInt(&self, id: IAttrID, value: *mut int64) -> tresult {
        if value.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: `value` is checked non-null above.
        unsafe { self.write_numeric_attr(id, value, Vst3AttributeValue::into_int) }
    }

    unsafe fn setFloat(&self, id: IAttrID, value: f64) -> tresult {
        self.set_attr(id, Vst3AttributeValue::Float(value))
    }

    unsafe fn getFloat(&self, id: IAttrID, value: *mut f64) -> tresult {
        if value.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: `value` is checked non-null above.
        unsafe { self.write_numeric_attr(id, value, Vst3AttributeValue::into_float) }
    }

    unsafe fn setString(&self, id: IAttrID, string: *const TChar) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let mut stored = Vec::new();
        let mut offset = 0;
        loop {
            // SAFETY: VST3 strings are null-terminated TChar arrays.
            let ptr = unsafe { string.add(offset) };
            // SAFETY: VST3 strings are null-terminated TChar arrays.
            let ch = unsafe { *ptr };
            stored.push(ch);
            if ch == 0 {
                break;
            }
            offset += 1;
        }
        self.set_attr(id, Vst3AttributeValue::String(stored))
    }

    unsafe fn getString(&self, id: IAttrID, string: *mut TChar, size_in_bytes: uint32) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let capacity = (size_in_bytes as usize) / std::mem::size_of::<TChar>();
        if capacity == 0 {
            return kInvalidArgument;
        }
        match self.get_attr(id) {
            Some(Vst3AttributeValue::String(stored)) => {
                let copy_len = stored.len().min(capacity);
                // SAFETY: `string` points to `capacity` TChar slots by VST3 contract.
                unsafe { std::ptr::copy_nonoverlapping(stored.as_ptr(), string, copy_len) };
                // SAFETY: `string` points to `capacity` TChar slots by VST3 contract.
                let terminator = unsafe { string.add(copy_len.saturating_sub(1)) };
                // SAFETY: `terminator` stays within the caller-provided buffer.
                unsafe { *terminator = 0 };
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setBinary(&self, id: IAttrID, data: *const c_void, size_in_bytes: uint32) -> tresult {
        if data.is_null() && size_in_bytes > 0 {
            return kInvalidArgument;
        }
        // SAFETY: VST3 provides `size_in_bytes` readable bytes when non-zero.
        let bytes =
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size_in_bytes as usize) };
        self.set_attr(id, Vst3AttributeValue::Binary(bytes.to_vec()))
    }

    unsafe fn getBinary(
        &self,
        id: IAttrID,
        data: *mut *const c_void,
        size_in_bytes: *mut uint32,
    ) -> tresult {
        if data.is_null() || size_in_bytes.is_null() {
            return kInvalidArgument;
        }
        match self.get_attr(id) {
            Some(Vst3AttributeValue::Binary(stored)) => {
                // SAFETY: Out pointers are checked non-null above.
                unsafe { *data = stored.as_ptr().cast() };
                // SAFETY: Out pointers are checked non-null above.
                unsafe { *size_in_bytes = stored.len() as uint32 };
                kResultOk
            }
            _ => kResultFalse,
        }
    }
}

impl Vst3AttributeList {
    pub(super) fn set_attr(&self, id: IAttrID, value: Vst3AttributeValue) -> tresult {
        let Some(id) = attr_id_to_string(id) else {
            return kInvalidArgument;
        };
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, value);
        kResultOk
    }

    pub(super) fn get_attr(&self, id: IAttrID) -> Option<Vst3AttributeValue> {
        let id = attr_id_to_string(id)?;
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .map(Vst3AttributeValue::clone_value)
    }

    unsafe fn write_numeric_attr<T: Copy>(
        &self,
        id: IAttrID,
        value: *mut T,
        extract: impl FnOnce(Vst3AttributeValue) -> Option<T>,
    ) -> tresult {
        let Some(stored) = self.get_attr(id).and_then(extract) else {
            return kResultFalse;
        };
        // SAFETY: Caller checked that `value` is a valid out pointer.
        unsafe {
            *value = stored;
        }
        kResultOk
    }
}

impl Vst3AttributeValue {
    fn into_int(self) -> Option<int64> {
        match self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }

    fn into_float(self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    pub(super) fn clone_value(&self) -> Self {
        match self {
            Self::Int(value) => Self::Int(*value),
            Self::Float(value) => Self::Float(*value),
            Self::String(value) => Self::String(value.clone()),
            Self::Binary(value) => Self::Binary(value.clone()),
        }
    }
}

pub(super) fn attr_id_to_string(id: IAttrID) -> Option<String> {
    if id.is_null() {
        return None;
    }
    // SAFETY: VST3 AttrID is a null-terminated C string.
    Some(unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned())
}

pub(super) struct Vst3Message {
    pub(super) message_id: Mutex<Option<CString>>,
    pub(super) attributes: ComPtr<IAttributeList>,
}

impl Vst3Message {
    pub(super) fn new() -> Self {
        let attributes = ComWrapper::new(Vst3AttributeList::default())
            .to_com_ptr::<IAttributeList>()
            .expect("Vst3AttributeList exposes IAttributeList");
        Self {
            message_id: Mutex::new(None),
            attributes,
        }
    }
}

impl Class for Vst3Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Vst3Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.message_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or(std::ptr::null(), |id| id.as_ptr())
    }

    unsafe fn setMessageID(&self, id: FIDString) {
        let message_id = if id.is_null() {
            None
        } else {
            // SAFETY: VST3 message IDs are null-terminated C strings.
            Some(unsafe { CStr::from_ptr(id) }.to_owned())
        };
        *self
            .message_id
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = message_id;
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}

impl IPlugFrameTrait for Vst3Host {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        // SAFETY: VST3 provides a readable ViewRect pointer or null.
        let Some(rect) = (unsafe { new_size.as_ref() }) else {
            return kInvalidArgument;
        };
        if let Some(size) = editor_size_from_rect(*rect) {
            if let Some(handler) = self.resize_handler() {
                let accepted = match handler.resize_editor(size) {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        trace_vst3_editor(|| {
                            format!(
                                "IPlugFrame::resizeView live resize failed view={view:p} rect={} \
                                 size={size:?}: {error}",
                                format_view_rect(*rect)
                            )
                        });
                        return kResultFalse;
                    }
                };
                // SAFETY: `view` is supplied by the plugin for this resize callback.
                let on_size_result = unsafe { call_plug_view_on_size(view, accepted) };
                if on_size_result != kResultOk {
                    trace_vst3_editor(|| {
                        format!(
                            "IPlugFrame::resizeView onSize failed view={view:p} \
                             accepted={accepted:?} result={on_size_result}"
                        )
                    });
                    return on_size_result;
                }
                self.store_requested_size(accepted);
                trace_vst3_editor(|| {
                    format!(
                        "IPlugFrame::resizeView applied live resize view={view:p} rect={} \
                         requested={size:?} accepted={accepted:?}",
                        format_view_rect(*rect)
                    )
                });
                return kResultOk;
            }
            self.store_requested_size(size);
            trace_vst3_editor(|| {
                format!(
                    "IPlugFrame::resizeView queued deferred resize view={view:p} rect={} \
                     size={size:?}",
                    format_view_rect(*rect)
                )
            });
            return kResultOk;
        } else {
            trace_vst3_editor(|| {
                format!(
                    "IPlugFrame::resizeView ignored invalid rect={}",
                    format_view_rect(*rect)
                )
            });
        }
        kResultFalse
    }
}

impl IComponentHandlerTrait for Vst3Host {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, _id: ParamID, _value_normalized: ParamValue) -> tresult {
        kResultOk
    }

    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, _flags: int32) -> tresult {
        kResultOk
    }
}

pub(super) struct Vst3EventList {
    pub(super) events: Vec<Event>,
}

impl Class for Vst3EventList {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for Vst3EventList {
    unsafe fn getEventCount(&self) -> int32 {
        self.events.len() as int32
    }

    unsafe fn getEvent(&self, index: int32, event: *mut Event) -> tresult {
        // SAFETY: VST3 provides a writable Event pointer or null.
        let Some(out) = (unsafe { event.as_mut() }) else {
            return kInvalidArgument;
        };
        let Ok(index) = usize::try_from(index) else {
            return kInvalidArgument;
        };
        let Some(input) = self.events.get(index) else {
            return kInvalidArgument;
        };
        *out = *input;
        kResultOk
    }

    unsafe fn addEvent(&self, _event: *mut Event) -> tresult {
        kResultOk
    }
}

pub(super) struct EmptyParameterChanges;

impl Class for EmptyParameterChanges {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for EmptyParameterChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        0
    }

    unsafe fn getParameterData(&self, _index: int32) -> *mut IParamValueQueue {
        std::ptr::null_mut()
    }

    unsafe fn addParameterData(
        &self,
        _id: *const ParamID,
        _index: *mut int32,
    ) -> *mut IParamValueQueue {
        std::ptr::null_mut()
    }
}

pub(super) struct Vst3RuntimeInner {
    pub(super) _module: Arc<LoadedModule>,
    pub(super) host: ComWrapper<Vst3Host>,
    pub(super) component: ComPtr<IComponent>,
    pub(super) processor: ComPtr<IAudioProcessor>,
    pub(super) controller: Option<ComPtr<IEditController>>,
    pub(super) component_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) controller_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) descriptor: &'static ProcessorDescriptor,
    pub(super) controller_lifecycle: Option<ControllerLifecycle>,
    pub(super) active: bool,
    pub(super) processing: bool,
    pub(super) destroyed: bool,
    pub(super) process_trace_remaining: u8,
}

pub(super) struct CreatedController {
    pub(super) controller: ComPtr<IEditController>,
    pub(super) lifecycle: ControllerLifecycle,
}

pub(super) struct Vst3PluginInstance {
    pub(super) module: Arc<LoadedModule>,
    pub(super) host: ComWrapper<Vst3Host>,
    pub(super) component: ComPtr<IComponent>,
    pub(super) processor: ComPtr<IAudioProcessor>,
    pub(super) controller: Option<ComPtr<IEditController>>,
    pub(super) component_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) controller_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) controller_lifecycle: Option<ControllerLifecycle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControllerLifecycle {
    ComponentIntegrated,
    Separate,
}

#[derive(Clone, Copy, Debug)]
enum ControllerDestroyStep {
    Disconnect,
    Terminate,
}

impl ControllerLifecycle {
    pub(super) fn connects_component(self) -> bool {
        matches!(self, Self::Separate)
    }

    pub(super) fn terminates_controller(self) -> bool {
        matches!(self, Self::Separate)
    }
}

// SAFETY: The runtime is always shared behind a `Mutex`; raw VST3 pointers are accessed only
// while holding that mutex and remain valid until the paired terminate calls.
unsafe impl Send for Vst3RuntimeInner {}

pub(super) fn create_vst3_plugin_instance(
    metadata: &Vst3PluginMetadata,
) -> Result<Vst3PluginInstance, Vst3RuntimeError> {
    let created = create_vst3_instance_parts(metadata)?;
    Ok(created.into_instance())
}

pub(super) struct CreatedVst3InstanceParts {
    pub(super) module: Arc<LoadedModule>,
    pub(super) host: ComWrapper<Vst3Host>,
    pub(super) component: ComPtr<IComponent>,
    pub(super) processor: ComPtr<IAudioProcessor>,
    pub(super) controller: Option<ComPtr<IEditController>>,
    pub(super) component_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) controller_connection: Option<ComPtr<IConnectionPoint>>,
    pub(super) controller_lifecycle: Option<ControllerLifecycle>,
}

impl CreatedVst3InstanceParts {
    pub(super) fn into_instance(self) -> Vst3PluginInstance {
        Vst3PluginInstance {
            module: self.module,
            host: self.host,
            component: self.component,
            processor: self.processor,
            controller: self.controller,
            component_connection: self.component_connection,
            controller_connection: self.controller_connection,
            controller_lifecycle: self.controller_lifecycle,
        }
    }
}

pub(super) fn create_vst3_instance_parts(
    metadata: &Vst3PluginMetadata,
) -> Result<CreatedVst3InstanceParts, Vst3RuntimeError> {
    let module = load_vst3_runtime_module(metadata)?;
    let host = ComWrapper::new(Vst3Host::new());
    let component = create_initialized_vst3_component(metadata, &module, &host)?;
    let processor = vst3_audio_processor(&component)?;
    let created_controller = create_vst3_controller_parts(&module.factory, &component, &host)?;
    let (component_connection, controller_connection) = connect_component_and_controller(
        &component,
        created_controller.controller.as_ref(),
        created_controller.lifecycle,
    );
    sync_vst3_controller_state(&component, created_controller.controller.as_ref());

    Ok(CreatedVst3InstanceParts {
        module,
        host,
        component,
        processor,
        controller: created_controller.controller,
        component_connection,
        controller_connection,
        controller_lifecycle: created_controller.lifecycle,
    })
}

pub(super) fn load_vst3_runtime_module(
    metadata: &Vst3PluginMetadata,
) -> Result<Arc<LoadedModule>, Vst3RuntimeError> {
    Ok(load_module(&metadata.path, &metadata.library_path)?)
}

pub(super) fn create_initialized_vst3_component(
    metadata: &Vst3PluginMetadata,
    module: &LoadedModule,
    host: &ComWrapper<Vst3Host>,
) -> Result<ComPtr<IComponent>, Vst3RuntimeError> {
    let class_id = hex_to_tuid(&metadata.class_id)
        .ok_or_else(|| Vst3RuntimeError::InvalidClassId(metadata.class_id.clone()))?;
    let component = create_vst3_component(&module.factory, &class_id)?;
    initialize_vst3_component(&component, host)?;
    Ok(component)
}

pub(super) fn vst3_audio_processor(
    component: &ComPtr<IComponent>,
) -> Result<ComPtr<IAudioProcessor>, Vst3RuntimeError> {
    component
        .cast::<IAudioProcessor>()
        .ok_or(Vst3RuntimeError::MissingAudioProcessor)
}

pub(super) struct CreatedVst3ControllerParts {
    pub(super) controller: Option<ComPtr<IEditController>>,
    pub(super) lifecycle: Option<ControllerLifecycle>,
}

pub(super) fn create_vst3_controller_parts(
    factory: &ComPtr<IPluginFactory>,
    component: &ComPtr<IComponent>,
    host: &ComWrapper<Vst3Host>,
) -> Result<CreatedVst3ControllerParts, Vst3RuntimeError> {
    let created_controller = create_vst3_controller(factory, component, host)?;
    Ok(CreatedVst3ControllerParts {
        lifecycle: created_controller.as_ref().map(|created| created.lifecycle),
        controller: created_controller.map(|created| created.controller),
    })
}

pub(super) fn create_vst3_component(
    factory: &ComPtr<IPluginFactory>,
    class_id: &[i8; 16],
) -> Result<ComPtr<IComponent>, Vst3RuntimeError> {
    let mut component_raw = std::ptr::null_mut::<c_void>();
    trace_vst3(|| "instantiate createInstance IComponent start".to_string());
    // SAFETY: Factory is live and writes the requested processor COM pointer.
    unsafe {
        factory.createInstance(
            class_id.as_ptr(),
            IComponent_iid.as_ptr(),
            &mut component_raw,
        )
    };
    trace_vst3(|| "instantiate createInstance IComponent done".to_string());
    // SAFETY: Successful factory creation returns an owning IComponent pointer.
    unsafe { ComPtr::from_raw(component_raw.cast::<IComponent>()) }
        .ok_or(Vst3RuntimeError::CreateProcessorFailed)
}

pub(super) fn initialize_vst3_component(
    component: &ComPtr<IComponent>,
    host: &ComWrapper<Vst3Host>,
) -> Result<(), Vst3RuntimeError> {
    let host_application = host
        .to_com_ptr::<IHostApplication>()
        .ok_or(Vst3RuntimeError::InitializeFailed)?;
    trace_vst3(|| "instantiate component.initialize start".to_string());
    // SAFETY: Component is initialized once with a live host application object.
    if unsafe { component.initialize(host_application.as_ptr().cast()) } != kResultOk {
        return Err(Vst3RuntimeError::InitializeFailed);
    }
    trace_vst3(|| "instantiate component.initialize done".to_string());
    Ok(())
}

pub(super) fn create_vst3_controller(
    factory: &ComPtr<IPluginFactory>,
    component: &ComPtr<IComponent>,
    host: &ComWrapper<Vst3Host>,
) -> Result<Option<CreatedController>, Vst3RuntimeError> {
    trace_vst3(|| "instantiate create_controller start".to_string());
    let created_controller = create_controller(factory, component, host)?;
    trace_vst3(|| "instantiate create_controller done".to_string());
    Ok(created_controller)
}

pub(super) fn sync_vst3_controller_state(
    component: &ComPtr<IComponent>,
    controller: Option<&ComPtr<IEditController>>,
) {
    if let Some(controller) = controller {
        sync_controller_component_state(component, controller);
    }
}

impl Vst3RuntimeInner {
    pub(super) fn instantiate(
        metadata: &Vst3PluginMetadata,
        descriptor: &'static ProcessorDescriptor,
        sample_rate: usize,
        block_size: usize,
        role: registry::Role,
    ) -> Result<Self, Vst3RuntimeError> {
        trace_vst3(|| {
            format!(
                "instantiate start name={} role={role:?} path={}",
                metadata.name,
                metadata.path.display()
            )
        });
        let instance = create_vst3_plugin_instance(metadata)?;

        let mut runtime = Self {
            _module: instance.module,
            host: instance.host,
            component: instance.component,
            processor: instance.processor,
            controller: instance.controller,
            component_connection: instance.component_connection,
            controller_connection: instance.controller_connection,
            descriptor,
            controller_lifecycle: instance.controller_lifecycle,
            active: false,
            processing: false,
            destroyed: false,
            process_trace_remaining: 8,
        };
        runtime.configure_audio(sample_rate, block_size, role)?;
        trace_vst3(|| "instantiate done".to_string());
        Ok(runtime)
    }

    pub(super) fn configure_audio(
        &mut self,
        sample_rate: usize,
        block_size: usize,
        role: registry::Role,
    ) -> Result<(), Vst3RuntimeError> {
        trace_vst3(|| "configure_audio setBusArrangements start".to_string());
        let mut input = SpeakerArr::kStereo;
        let mut output = SpeakerArr::kStereo;
        let input_count = i32::from(role == registry::Role::Effect);
        // SAFETY: Component/processor are initialized and bus setup uses stack-owned values.
        let set_bus_arrangements_result = unsafe {
            self.processor
                .setBusArrangements(&mut input, input_count, &mut output, 1)
        };
        trace_vst3(|| {
            format!(
                "configure_audio setBusArrangements done result={set_bus_arrangements_result} \
                 input_count={input_count}"
            )
        });
        trace_vst3(|| "configure_audio activate buses start".to_string());
        activate_buses(
            &self.component,
            MediaTypes_::kAudio as MediaType,
            BusDirections_::kInput as BusDirection,
        );
        activate_buses(
            &self.component,
            MediaTypes_::kAudio as MediaType,
            BusDirections_::kOutput as BusDirection,
        );
        activate_buses(
            &self.component,
            MediaTypes_::kEvent as MediaType,
            BusDirections_::kInput as BusDirection,
        );

        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as int32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
            maxSamplesPerBlock: block_size.max(1) as int32,
            sampleRate: sample_rate.max(1) as SampleRate,
        };
        trace_vst3(|| "configure_audio setupProcessing start".to_string());
        // SAFETY: Processor is initialized and `setup` lives for the call.
        let setup_processing_result = unsafe { self.processor.setupProcessing(&mut setup) };
        trace_vst3(|| {
            format!("configure_audio setupProcessing done result={setup_processing_result}")
        });
        if setup_processing_result != kResultOk {
            return Err(Vst3RuntimeError::SetupFailed);
        }
        trace_vst3(|| "configure_audio setActive(1) start".to_string());
        // SAFETY: Component is initialized and ready to activate after setupProcessing.
        let set_active_result = unsafe { self.component.setActive(1) };
        trace_vst3(|| format!("configure_audio setActive(1) done result={set_active_result}"));
        if set_active_result != kResultOk {
            return Err(Vst3RuntimeError::ActivateFailed);
        }
        self.active = true;
        trace_vst3(|| "configure_audio setProcessing(1) start".to_string());
        // SAFETY: Processor is initialized and component is active.
        let set_processing_result = unsafe { self.processor.setProcessing(1) };
        trace_vst3(|| {
            format!("configure_audio setProcessing(1) done result={set_processing_result}")
        });
        if set_processing_result == kResultOk {
            self.processing = true;
        } else if set_processing_result == kNotImplemented {
            trace_vst3(|| {
                "configure_audio setProcessing(1) not implemented; continuing active".to_string()
            });
        } else {
            return Err(Vst3RuntimeError::ActivateFailed);
        }
        trace_vst3(|| "configure_audio done".to_string());
        Ok(())
    }

    pub(super) fn process_block(
        &mut self,
        input_left: Option<&[f32]>,
        input_right: Option<&[f32]>,
        output_left: &mut [f32],
        output_right: &mut [f32],
        events: &[Event],
    ) -> bool {
        if self.destroyed || !self.processing {
            return false;
        }
        let frames = output_left.len().min(output_right.len());
        let mut input_left_buffer = input_left.map(|input| input.to_vec());
        let mut input_right_buffer = input_right.map(|input| input.to_vec());
        let in_left = input_left_buffer
            .as_mut()
            .map_or(std::ptr::null_mut(), Vec::as_mut_ptr);
        let in_right = input_right_buffer
            .as_mut()
            .map_or(std::ptr::null_mut(), Vec::as_mut_ptr);
        let mut input_channels = [in_left, in_right];
        let mut output_channels = [output_left.as_mut_ptr(), output_right.as_mut_ptr()];
        let mut inputs = [AudioBusBuffers {
            numChannels: 2,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: input_channels.as_mut_ptr(),
            },
        }];
        let mut outputs = [AudioBusBuffers {
            numChannels: 2,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: output_channels.as_mut_ptr(),
            },
        }];
        let event_list = ComWrapper::new(Vst3EventList {
            events: events.to_vec(),
        });
        let empty_events = ComWrapper::new(Vst3EventList { events: Vec::new() });
        let input_events = event_list.to_com_ptr::<IEventList>();
        let output_events = empty_events.to_com_ptr::<IEventList>();
        let input_params = ComWrapper::new(EmptyParameterChanges);
        let output_params = ComWrapper::new(EmptyParameterChanges);
        let input_params = input_params.to_com_ptr::<IParameterChanges>();
        let output_params = output_params.to_com_ptr::<IParameterChanges>();
        let mut data = ProcessData {
            processMode: ProcessModes_::kRealtime as int32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
            numSamples: frames as int32,
            numInputs: process_input_count(input_left),
            numOutputs: 1,
            inputs: process_inputs_ptr(input_left, &mut inputs),
            outputs: outputs.as_mut_ptr(),
            inputParameterChanges: input_params
                .as_ref()
                .map_or(std::ptr::null_mut(), ComPtr::as_ptr),
            outputParameterChanges: output_params
                .as_ref()
                .map_or(std::ptr::null_mut(), ComPtr::as_ptr),
            inputEvents: input_events
                .as_ref()
                .map_or(std::ptr::null_mut(), ComPtr::as_ptr),
            outputEvents: output_events
                .as_ref()
                .map_or(std::ptr::null_mut(), ComPtr::as_ptr),
            processContext: std::ptr::null_mut(),
        };
        trace_process_start(
            self.process_trace_remaining,
            frames,
            data.numInputs,
            events.len(),
        );
        // SAFETY: ProcessData points to buffers and COM lists that outlive this process call.
        let result = unsafe { self.processor.process(&mut data) == kResultOk };
        trace_process_done(&mut self.process_trace_remaining, result);
        result
    }

    pub(super) fn reset(&mut self) {}

    pub(super) fn latency_samples(&self) -> u32 {
        // SAFETY: Processor is live while runtime is not destroyed.
        unsafe { self.processor.getLatencySamples() }
    }

    pub(super) fn parameters(&self) -> Vec<lilypalooza_audio::ParameterInfo> {
        let Some(controller) = self.controller.as_ref() else {
            return self
                .descriptor
                .params
                .iter()
                .map(lilypalooza_audio::ParameterInfo::from)
                .collect();
        };
        // SAFETY: Controller is initialized and owned by this runtime.
        let count = unsafe { controller.getParameterCount() }.max(0);
        let mut parameters = Vec::new();
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<ParameterInfo>::zeroed();
            // SAFETY: `info` points to writable storage for the VST3 parameter info result.
            let result = unsafe { controller.getParameterInfo(index, info.as_mut_ptr()) };
            if result != kResultOk {
                continue;
            }
            // SAFETY: VST3 returned success and initialized the output struct.
            let info = unsafe { info.assume_init() };
            if info.flags & ParameterInfo_::ParameterFlags_::kIsHidden != 0 {
                continue;
            }
            let name = tchar_array_to_string(&info.title);
            parameters.push(lilypalooza_audio::ParameterInfo {
                id: info.id.to_string(),
                name: if name.is_empty() {
                    format!("Parameter {}", info.id)
                } else {
                    name
                },
                default: normalized_f64_to_f32(info.defaultNormalizedValue),
                automatable: info.flags & ParameterInfo_::ParameterFlags_::kCanAutomate != 0,
                readonly: info.flags & ParameterInfo_::ParameterFlags_::kIsReadOnly != 0,
            });
        }
        parameters
    }

    pub(super) fn get_param(&self, id: ParamID) -> Result<f32, ControllerError> {
        let Some(controller) = self.controller.as_ref() else {
            return Err(ControllerError::UnknownParameter(id.to_string()));
        };
        // SAFETY: Controller is initialized and owned by this runtime.
        Ok(normalized_f64_to_f32(unsafe {
            controller.getParamNormalized(id)
        }))
    }

    pub(super) fn set_param(&self, id: ParamID, normalized: f32) -> Result<(), ControllerError> {
        let Some(controller) = self.controller.as_ref() else {
            return Err(ControllerError::UnknownParameter(id.to_string()));
        };
        // SAFETY: Controller is initialized and owned by this runtime.
        let result =
            unsafe { controller.setParamNormalized(id, f64::from(normalized.clamp(0.0, 1.0))) };
        if result == kResultOk {
            Ok(())
        } else {
            Err(ControllerError::Backend(format!(
                "VST3 setParamNormalized({id}) failed: {result}"
            )))
        }
    }

    pub(super) fn save_state(&mut self) -> Result<ProcessorState, ControllerError> {
        Ok(ProcessorState::default())
    }

    pub(super) fn load_state(
        &mut self,
        _state: &ProcessorState,
    ) -> Result<(), ProcessorStateError> {
        Ok(())
    }

    pub(super) fn prepare_destroy(&mut self) {
        if self.destroyed {
            return;
        }
        trace_vst3(|| "prepare_destroy start".to_string());
        self.stop_processing_for_destroy();
        self.deactivate_component_for_destroy();
        self.disconnect_controller_for_destroy();
        self.terminate_controller_for_destroy();
        self.terminate_component_for_destroy();
        self.destroyed = true;
        trace_vst3(|| "prepare_destroy done".to_string());
    }

    pub(super) fn stop_processing_for_destroy(&mut self) {
        if self.processing {
            trace_vst3(|| "prepare_destroy setProcessing(0) start".to_string());
            // SAFETY: Lifecycle calls are paired with successful initialization/activation.
            log_vst3_lifecycle_result("prepare_destroy setProcessing(0)", unsafe {
                self.processor.setProcessing(0)
            });
        }
        self.processing = false;
    }

    pub(super) fn deactivate_component_for_destroy(&mut self) {
        if self.active {
            trace_vst3(|| "prepare_destroy setActive(0) start".to_string());
            // SAFETY: Lifecycle calls are paired with successful initialization/activation.
            log_vst3_lifecycle_result("prepare_destroy setActive(0)", unsafe {
                self.component.setActive(0)
            });
        }
        self.active = false;
    }

    pub(super) fn disconnect_controller_for_destroy(&mut self) {
        self.run_controller_destroy_step(ControllerDestroyStep::Disconnect);
    }

    pub(super) fn terminate_controller_for_destroy(&mut self) {
        self.run_controller_destroy_step(ControllerDestroyStep::Terminate);
    }

    fn run_controller_destroy_step(&mut self, step: ControllerDestroyStep) {
        match step {
            ControllerDestroyStep::Disconnect => self.disconnect_connected_controller_for_destroy(),
            ControllerDestroyStep::Terminate => self.terminate_initialized_controller_for_destroy(),
        }
    }

    fn disconnect_connected_controller_for_destroy(&mut self) {
        if !self
            .controller_lifecycle
            .is_some_and(ControllerLifecycle::connects_component)
        {
            return;
        }
        let (Some(component), Some(controller)) =
            (&self.component_connection, &self.controller_connection)
        else {
            return;
        };
        trace_vst3(|| "prepare_destroy disconnect start".to_string());
        // SAFETY: Connection points are live until runtime destruction.
        log_vst3_lifecycle_result("prepare_destroy component.disconnect", unsafe {
            component.disconnect(controller.as_ptr())
        });
        // SAFETY: Connection points are live until runtime destruction.
        log_vst3_lifecycle_result("prepare_destroy controller.disconnect", unsafe {
            controller.disconnect(component.as_ptr())
        });
    }

    fn terminate_initialized_controller_for_destroy(&mut self) {
        if !self
            .controller_lifecycle
            .is_some_and(ControllerLifecycle::terminates_controller)
        {
            return;
        }
        let Some(controller) = &self.controller else {
            return;
        };
        trace_vst3(|| "prepare_destroy controller.terminate start".to_string());
        // SAFETY: Controller lifecycle requires terminating this initialized controller.
        log_vst3_lifecycle_result("prepare_destroy controller.terminate", unsafe {
            controller.terminate()
        });
    }

    pub(super) fn terminate_component_for_destroy(&mut self) {
        trace_vst3(|| "prepare_destroy component.terminate start".to_string());
        // SAFETY: Component was initialized and has not been destroyed yet.
        log_vst3_lifecycle_result("prepare_destroy component.terminate", unsafe {
            self.component.terminate()
        });
    }
}

pub(super) fn normalized_f64_to_f32(value: f64) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}
