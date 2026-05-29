use super::{editor::*, host_com::*, probe::*, *};

pub(super) fn log_vst3_lifecycle_result(operation: &str, result: tresult) {
    trace_vst3(|| format!("{operation} done result={result}"));
    if result != kResultOk {
        log::warn!("{operation} returned {result}");
    }
}

pub(super) fn connect_component_and_controller(
    component: &ComPtr<IComponent>,
    controller: Option<&ComPtr<IEditController>>,
    controller_lifecycle: Option<ControllerLifecycle>,
) -> (
    Option<ComPtr<IConnectionPoint>>,
    Option<ComPtr<IConnectionPoint>>,
) {
    if !controller_lifecycle.is_some_and(ControllerLifecycle::connects_component) {
        return (None, None);
    }
    let component_connection = component.cast::<IConnectionPoint>();
    let controller_connection =
        controller.and_then(|controller| controller.cast::<IConnectionPoint>());
    if let (Some(component_connection), Some(controller_connection)) =
        (&component_connection, &controller_connection)
    {
        trace_vst3(|| "connect_component_and_controller component.connect start".to_string());
        // SAFETY: Both connection points are live and owned by this runtime.
        let _ = unsafe { component_connection.connect(controller_connection.as_ptr()) };
        trace_vst3(|| "connect_component_and_controller component.connect done".to_string());
        trace_vst3(|| "connect_component_and_controller controller.connect start".to_string());
        // SAFETY: Both connection points are live and owned by this runtime.
        let _ = unsafe { controller_connection.connect(component_connection.as_ptr()) };
        trace_vst3(|| "connect_component_and_controller controller.connect done".to_string());
    }
    (component_connection, controller_connection)
}

pub(super) fn sync_controller_component_state(
    component: &ComPtr<IComponent>,
    controller: &ComPtr<IEditController>,
) {
    let stream_wrapper = ComWrapper::new(Vst3MemoryStream::default());
    let Some(stream) = stream_wrapper.to_com_ptr::<IBStream>() else {
        return;
    };
    trace_vst3(|| "sync_controller_component_state getState start".to_string());
    // SAFETY: Component is initialized and the stream stays live for the call.
    if unsafe { component.getState(stream.as_ptr()) } != kResultOk {
        trace_vst3(|| "sync_controller_component_state getState skipped".to_string());
        return;
    }
    stream_wrapper.rewind();
    trace_vst3(|| "sync_controller_component_state setComponentState start".to_string());
    // SAFETY: Controller is initialized and the stream stays live for the call.
    let result = unsafe { controller.setComponentState(stream.as_ptr()) };
    trace_vst3(|| {
        format!("sync_controller_component_state setComponentState done result={result}")
    });
}

#[derive(Default)]
pub(super) struct Vst3MemoryStream {
    pub(super) state: Mutex<Vst3MemoryStreamState>,
}

#[derive(Default)]
pub(super) struct Vst3MemoryStreamState {
    pub(super) data: Vec<u8>,
    pub(super) position: usize,
}

impl Vst3MemoryStream {
    pub(super) fn rewind(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .position = 0;
    }
}

impl Class for Vst3MemoryStream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for Vst3MemoryStream {
    unsafe fn read(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_read: *mut int32,
    ) -> tresult {
        if num_bytes < 0 || buffer.is_null() {
            return kInvalidArgument;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Ok(requested) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        let available = state.data.len().saturating_sub(state.position);
        let count = requested.min(available);
        if count > 0 {
            // SAFETY: `state.position` is within `state.data` by stream invariant.
            let source = unsafe { state.data.as_ptr().add(state.position) };
            // SAFETY: `buffer` is non-null and writable for `count` bytes by IBStream contract.
            unsafe { std::ptr::copy_nonoverlapping(source, buffer.cast::<u8>(), count) };
        }
        state.position += count;
        // SAFETY: VST3 may pass null when it does not need the byte count.
        if let Some(num_bytes_read) = unsafe { num_bytes_read.as_mut() } {
            *num_bytes_read = count as int32;
        }
        kResultOk
    }

    unsafe fn write(
        &self,
        buffer: *mut c_void,
        num_bytes: int32,
        num_bytes_written: *mut int32,
    ) -> tresult {
        if num_bytes < 0 || buffer.is_null() {
            return kInvalidArgument;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Ok(count) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        let end = state.position.saturating_add(count);
        if end > state.data.len() {
            state.data.resize(end, 0);
        }
        // SAFETY: `buffer` is non-null and readable for `count` bytes by IBStream contract.
        let input = unsafe { slice::from_raw_parts(buffer.cast::<u8>(), count) };
        let position = state.position;
        let Some(output) = state.data.get_mut(position..end) else {
            return kInvalidArgument;
        };
        output.copy_from_slice(input);
        state.position = end;
        // SAFETY: VST3 may pass null when it does not need the byte count.
        if let Some(num_bytes_written) = unsafe { num_bytes_written.as_mut() } {
            *num_bytes_written = count as int32;
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(next) = stream_seek_position(&state, pos, mode) else {
            return kInvalidArgument;
        };
        state.position = next;
        // SAFETY: VST3 may pass null when it does not need the new position.
        if let Some(result) = unsafe { result.as_mut() } {
            *result = next as int64;
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        let position = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .position as int64;
        // SAFETY: VST3 provides a writable position pointer or null.
        let Some(pos) = (unsafe { pos.as_mut() }) else {
            return kInvalidArgument;
        };
        *pos = position;
        kResultOk
    }
}

pub(super) fn stream_seek_position(
    state: &Vst3MemoryStreamState,
    pos: int64,
    mode: int32,
) -> Option<usize> {
    let base = stream_seek_base(state, mode)?;
    let next = base.checked_add(pos)?;
    usize::try_from(next).ok()
}

pub(super) fn stream_seek_base(state: &Vst3MemoryStreamState, mode: int32) -> Option<int64> {
    match mode {
        mode if mode == IBStream_::IStreamSeekMode_::kIBSeekSet as int32 => Some(0),
        mode if mode == IBStream_::IStreamSeekMode_::kIBSeekCur as int32 => {
            Some(state.position as int64)
        }
        mode if mode == IBStream_::IStreamSeekMode_::kIBSeekEnd as int32 => {
            Some(state.data.len() as int64)
        }
        _ => None,
    }
}

impl Drop for Vst3RuntimeInner {
    fn drop(&mut self) {
        self.prepare_destroy();
    }
}

pub(super) fn create_controller(
    factory: &ComPtr<IPluginFactory>,
    component: &ComPtr<IComponent>,
    host: &ComWrapper<Vst3Host>,
) -> Result<Option<CreatedController>, Vst3RuntimeError> {
    if let Some(controller) = component.cast::<IEditController>() {
        return integrated_controller(controller, host);
    }
    let Some(controller_id) = component_controller_class_id(component) else {
        return Ok(None);
    };
    let Some(controller) = instantiate_separate_controller(factory, &controller_id)? else {
        return Ok(None);
    };
    initialize_controller(&controller, host)?;
    Ok(Some(CreatedController {
        controller,
        lifecycle: ControllerLifecycle::Separate,
    }))
}

pub(super) fn integrated_controller(
    controller: ComPtr<IEditController>,
    host: &ComWrapper<Vst3Host>,
) -> Result<Option<CreatedController>, Vst3RuntimeError> {
    trace_vst3(|| "create_controller component has IEditController".to_string());
    set_controller_component_handler(&controller, host)?;
    Ok(Some(CreatedController {
        controller,
        lifecycle: ControllerLifecycle::ComponentIntegrated,
    }))
}

pub(super) fn component_controller_class_id(
    component: &ComPtr<IComponent>,
) -> Option<[c_char; 16]> {
    let mut controller_id = [0 as c_char; 16];
    trace_vst3(|| "create_controller getControllerClassId start".to_string());
    // SAFETY: Component writes the controller class id into the provided TUID.
    if unsafe { component.getControllerClassId(&mut controller_id) } != kResultOk {
        trace_vst3(|| "create_controller no controller class id".to_string());
        return None;
    }
    Some(controller_id)
}

pub(super) fn instantiate_separate_controller(
    factory: &ComPtr<IPluginFactory>,
    controller_id: &[c_char; 16],
) -> Result<Option<ComPtr<IEditController>>, Vst3RuntimeError> {
    let mut controller_raw = std::ptr::null_mut::<c_void>();
    trace_vst3(|| "create_controller factory.createInstance IEditController start".to_string());
    // SAFETY: Factory is live and writes an optional controller COM pointer.
    unsafe {
        factory.createInstance(
            controller_id.as_ptr(),
            IEditController_iid.as_ptr(),
            &mut controller_raw,
        )
    };
    trace_vst3(|| "create_controller factory.createInstance IEditController done".to_string());
    // SAFETY: Successful controller creation returns an owning IEditController pointer.
    let Some(controller) = (unsafe { ComPtr::from_raw(controller_raw.cast::<IEditController>()) })
    else {
        trace_vst3(|| "create_controller factory returned null".to_string());
        return Ok(None);
    };
    Ok(Some(controller))
}

pub(super) fn initialize_controller(
    controller: &ComPtr<IEditController>,
    host: &ComWrapper<Vst3Host>,
) -> Result<(), Vst3RuntimeError> {
    let host_application = host
        .to_com_ptr::<IHostApplication>()
        .ok_or(Vst3RuntimeError::InitializeFailed)?;
    trace_vst3(|| "initialize_controller initialize start".to_string());
    // SAFETY: Controller is initialized once with a live host application object.
    if unsafe { controller.initialize(host_application.as_ptr().cast()) } != kResultOk {
        return Err(Vst3RuntimeError::InitializeFailed);
    }
    trace_vst3(|| "initialize_controller initialize done".to_string());
    set_controller_component_handler(controller, host)?;
    Ok(())
}

pub(super) fn set_controller_component_handler(
    controller: &ComPtr<IEditController>,
    host: &ComWrapper<Vst3Host>,
) -> Result<(), Vst3RuntimeError> {
    if let Some(handler) = host.to_com_ptr::<IComponentHandler>() {
        trace_vst3(|| "initialize_controller setComponentHandler start".to_string());
        // SAFETY: Component handler COM pointer is owned by the host wrapper and remains live.
        unsafe {
            let _ = controller.setComponentHandler(handler.as_ptr());
        }
        trace_vst3(|| "initialize_controller setComponentHandler done".to_string());
    }
    Ok(())
}

pub(super) fn activate_buses(
    component: &ComPtr<IComponent>,
    media_type: MediaType,
    direction: BusDirection,
) {
    // SAFETY: Component is live and queried with VST3 media/direction constants.
    let count = unsafe { component.getBusCount(media_type, direction) }.max(0);
    trace_vst3(|| {
        format!("activate_buses media_type={media_type} direction={direction} count={count}")
    });
    for index in 0..count {
        // SAFETY: Bus indices are bounded by `getBusCount`.
        let result = unsafe { component.activateBus(media_type, direction, index, 1) };
        trace_vst3(|| {
            format!(
                "activate_buses media_type={media_type} direction={direction} index={index} \
                 result={result}"
            )
        });
    }
}

#[derive(Clone)]
pub(super) struct Vst3Binding {
    pub(super) shared: Arc<Mutex<Vst3RuntimeInner>>,
}

impl RuntimeBinding for Vst3Binding {
    fn controller(&self) -> Box<dyn Controller> {
        Box::new(Vst3Controller {
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

pub(super) struct Vst3Controller {
    pub(super) shared: Arc<Mutex<Vst3RuntimeInner>>,
}

impl Controller for Vst3Controller {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .descriptor
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    fn set_param(&self, id: &str, _normalized: f32) -> Result<(), ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
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
        let has_editor = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .controller
            .is_some();
        Ok(has_editor.then(|| {
            Box::new(Vst3EditorSession {
                shared: self.shared.clone(),
                view: None,
                current_size: None,
            }) as Box<dyn EditorSession>
        }))
    }
}

pub(super) struct Vst3Processor {
    pub(super) shared: Arc<Mutex<Vst3RuntimeInner>>,
    pub(super) midi: Vst3MidiEventQueue,
}

pub(super) struct Vst3MidiEventQueue {
    pub(super) pending: Vec<Event>,
    pub(super) active_notes: [[bool; 128]; 16],
}

impl Vst3MidiEventQueue {
    pub(super) fn new() -> Self {
        Self {
            pending: Vec::new(),
            active_notes: [[false; 128]; 16],
        }
    }

    pub(super) fn push(&mut self, event: MidiEvent) {
        if self.push_note_event(event) {
            return;
        }
        self.push_other_event(event);
    }

    pub(super) fn push_note_event(&mut self, event: MidiEvent) -> bool {
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
            _ => return false,
        }
        true
    }

    pub(super) fn push_note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        if velocity == 0 {
            self.push_note_off(channel, note, 0);
            return;
        }

        if let Some(active) = self.active_note_mut(channel, note) {
            *active = true;
        }
        self.pending
            .push(vst3_note_on_event(channel, note, velocity));
    }

    pub(super) fn push_note_off(&mut self, channel: u8, note: u8, velocity: u8) {
        if let Some(active) = self.active_note_mut(channel, note) {
            *active = false;
        }
        self.pending
            .push(vst3_note_off_event(channel, note, velocity));
    }

    pub(super) fn push_other_event(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::AllNotesOff { channel } | MidiEvent::AllSoundOff { channel } => {
                self.push_active_note_offs(channel);
            }
            MidiEvent::PolyPressure {
                channel,
                note,
                pressure,
            } => self
                .pending
                .push(vst3_poly_pressure_event(channel, note, pressure)),
            _ => {}
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
                self.pending.push(vst3_note_off_event(channel, note, 0));
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
        }
    }

    pub(super) fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }
}

impl Processor for Vst3Processor {
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
        Vst3Controller {
            shared: self.shared.clone(),
        }
        .create_editor_session()
    }
}

impl InstrumentProcessor for Vst3Processor {
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

impl EffectProcessor for Vst3Processor {
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

pub(super) fn vst3_note_on_event(channel: u8, note: u8, velocity: u8) -> Event {
    vst3_note_event(channel, note, velocity, Vst3NoteEventKind::On)
}

pub(super) fn vst3_note_off_event(channel: u8, note: u8, velocity: u8) -> Event {
    vst3_note_event(channel, note, velocity, Vst3NoteEventKind::Off)
}

#[derive(Debug, Clone, Copy)]
enum Vst3NoteEventKind {
    On,
    Off,
}

fn vst3_note_event(channel: u8, note: u8, velocity: u8, kind: Vst3NoteEventKind) -> Event {
    let channel = (channel & 0x0f) as int16;
    let pitch = note as int16;
    let velocity = f32::from(velocity) / 127.0;
    let (r#type, __field0) = match kind {
        Vst3NoteEventKind::On => (
            Event_::EventTypes_::kNoteOnEvent as u16,
            Event__type0 {
                noteOn: NoteOnEvent {
                    channel,
                    pitch,
                    tuning: 0.0,
                    velocity,
                    length: 0,
                    noteId: -1,
                },
            },
        ),
        Vst3NoteEventKind::Off => (
            Event_::EventTypes_::kNoteOffEvent as u16,
            Event__type0 {
                noteOff: NoteOffEvent {
                    channel,
                    pitch,
                    velocity,
                    noteId: -1,
                    tuning: 0.0,
                },
            },
        ),
    };
    Event {
        busIndex: 0,
        sampleOffset: 0,
        ppqPosition: 0.0,
        flags: Event_::EventFlags_::kIsLive as u16,
        r#type,
        __field0,
    }
}

pub(super) fn vst3_poly_pressure_event(channel: u8, note: u8, pressure: u8) -> Event {
    Event {
        busIndex: 0,
        sampleOffset: 0,
        ppqPosition: 0.0,
        flags: Event_::EventFlags_::kIsLive as u16,
        r#type: Event_::EventTypes_::kPolyPressureEvent as u16,
        __field0: Event__type0 {
            polyPressure: PolyPressureEvent {
                channel: (channel & 0x0f) as int16,
                pitch: note as int16,
                pressure: f32::from(pressure) / 127.0,
                noteId: -1,
            },
        },
    }
}

pub(super) fn process_input_count(input_left: Option<&[f32]>) -> int32 {
    i32::from(input_left.is_some())
}

pub(super) fn process_inputs_ptr(
    input_left: Option<&[f32]>,
    inputs: &mut [AudioBusBuffers; 1],
) -> *mut AudioBusBuffers {
    if input_left.is_some() {
        inputs.as_mut_ptr()
    } else {
        std::ptr::null_mut()
    }
}

pub(super) fn trace_process_start(
    remaining: u8,
    frames: usize,
    input_count: int32,
    event_count: usize,
) {
    if remaining > 0 {
        trace_vst3(|| {
            format!("process start frames={frames} inputs={input_count} events={event_count}")
        });
    }
}

pub(super) fn trace_process_done(remaining: &mut u8, result: bool) {
    if *remaining == 0 {
        return;
    }
    *remaining -= 1;
    trace_vst3(|| format!("process done result={result}"));
}

pub(super) struct Vst3EditorSession {
    pub(super) shared: Arc<Mutex<Vst3RuntimeInner>>,
    pub(super) view: Option<ComPtr<IPlugView>>,
    pub(super) current_size: Option<EditorSize>,
}

impl EditorSession for Vst3EditorSession {
    fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
        let Some(view) = &self.view else {
            return Ok(None);
        };
        let resizable = vst3_editor_view_resizability(view);
        trace_vst3_editor(|| format!("session resizable={resizable:?}"));
        Ok(resizable)
    }

    fn initial_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        trace_vst3_editor(|| format!("session initial_size {:?}", self.current_size));
        Ok(self.current_size)
    }

    fn requested_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
        let requested = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .host
            .take_requested_size();
        let changed = changed_editor_size_request(&mut self.current_size, requested);
        trace_vst3_editor(|| {
            format!(
                "session requested_size requested={requested:?} changed={changed:?} current={:?}",
                self.current_size
            )
        });
        Ok(changed)
    }

    fn set_resize_handler(
        &mut self,
        handler: Option<Arc<dyn EditorResizeHandler>>,
    ) -> Result<(), EditorError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .host
            .set_resize_handler(handler);
        Ok(())
    }

    fn attach(&mut self, parent: EditorParent) -> Result<(), EditorError> {
        let view = attach_vst3_editor_view(self, parent)?;
        self.current_size = vst3_view_size(&view);
        trace_vst3_editor(|| format!("session attach current_size={:?}", self.current_size));
        self.view = Some(view);
        Ok(())
    }

    fn detach(&mut self) -> Result<(), EditorError> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .host
            .set_resize_handler(None);
        let result = self.detach_current_view();
        self.current_size = None;
        result
    }

    fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
        Ok(())
    }

    fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
        let Some(view) = &self.view else {
            return Ok(size);
        };
        let accepted = resize_vst3_editor_view(view, size)?;
        trace_vst3_editor(|| format!("session resize requested={size:?} accepted={accepted:?}"));
        self.current_size = Some(accepted);
        Ok(accepted)
    }
}
