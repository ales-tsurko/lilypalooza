use num_traits::ToPrimitive;

use super::*;

/// AU runtime error.
#[derive(Debug, thiserror::Error)]
pub enum AuRuntimeError {
    /// AUv2 hosting is only available on macOS.
    #[cfg(not(target_os = "macos"))]
    #[error("AUv2 hosting is only available on macOS")]
    UnsupportedPlatform,
    /// AudioComponent lookup failed.
    #[error("AU component was not found")]
    ComponentNotFound,
    /// Core Audio call failed.
    #[error("{operation} failed with OSStatus {status}")]
    CoreAudio {
        /// Operation name.
        operation: &'static str,
        /// OSStatus value.
        status: i32,
    },
}

/// Live AU runtime.
#[cfg(target_os = "macos")]
pub struct AuRuntime {
    unit: coreaudio_sys::AudioUnit,
    input: Box<AuInputState>,
    descriptor: &'static ProcessorDescriptor,
    parameter_cache: Vec<AuParameter>,
    sample_rate: f64,
    sample_time: f64,
    render_calls: u64,
    disposed: bool,
}

/// Live AU runtime.
#[cfg(not(target_os = "macos"))]
pub struct AuRuntime {
    descriptor: &'static ProcessorDescriptor,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct AuParameter {
    id: u32,
    name: String,
    min: f32,
    max: f32,
    default: f32,
    writable: bool,
    readable: bool,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct AuInputState {
    left: *const f32,
    right: *const f32,
    frames: usize,
}

#[cfg(target_os = "macos")]
// SAFETY: `AuRuntime` is always stored behind the engine's runtime mutex. Raw AudioUnit access is
// serialized through that mutex, and the unit is disposed exactly once in `prepare_destroy`.
unsafe impl Send for AuRuntime {}

#[cfg(target_os = "macos")]
impl AuRuntime {
    pub fn instantiate(
        metadata: &AuPluginMetadata,
        descriptor: &'static ProcessorDescriptor,
        sample_rate: usize,
        block_size: usize,
    ) -> Result<Self, AuRuntimeError> {
        let unit = instantiate_audio_unit(metadata.component)?;
        let mut runtime = Self {
            unit,
            input: Box::new(AuInputState {
                left: std::ptr::null(),
                right: std::ptr::null(),
                frames: 0,
            }),
            descriptor,
            parameter_cache: Vec::new(),
            sample_rate: sample_rate.max(1) as f64,
            sample_time: 0.0,
            render_calls: 0,
            disposed: false,
        };
        log::trace!(
            target: "lilypalooza_au",
            "instantiate AU processor={} role={:?} component={:?} sample_rate={} block_size={}",
            metadata.processor_id,
            metadata.role,
            metadata.component,
            sample_rate,
            block_size
        );
        runtime.configure(sample_rate, block_size, metadata.role)?;
        runtime.parameter_cache = runtime.collect_parameters();
        log::trace!(
            target: "lilypalooza_au",
            "AU instantiated processor={} parameters={}",
            metadata.processor_id,
            runtime.parameter_cache.len()
        );
        Ok(runtime)
    }

    pub fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.descriptor
    }

    pub fn parameters(&self) -> Vec<ParameterInfo> {
        self.parameter_cache
            .iter()
            .map(|param| ParameterInfo {
                id: param.id.to_string(),
                name: param.name.clone(),
                default: normalize(param.default, param.min, param.max),
                automatable: param.writable,
                readonly: !param.writable,
            })
            .collect()
    }

    pub fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        let param = self.parameter(id)?;
        if !param.readable {
            return Err(ControllerError::Backend(format!(
                "AU parameter `{id}` is not readable"
            )));
        }
        let mut value = 0.0;
        // SAFETY: AudioUnitGetParameter reads one global parameter from a live unit.
        let status = unsafe {
            coreaudio_sys::AudioUnitGetParameter(
                self.unit,
                param.id,
                coreaudio_sys::kAudioUnitScope_Global,
                0,
                &mut value,
            )
        };
        core_audio_status("AudioUnitGetParameter", status)
            .map_err(|error| ControllerError::Backend(error.to_string()))?;
        Ok(normalize(value, param.min, param.max))
    }

    pub fn set_param(&self, id: &str, normalized: f32) -> Result<(), ControllerError> {
        let param = self.parameter(id)?;
        if !param.writable {
            return Err(ControllerError::Backend(format!(
                "AU parameter `{id}` is read-only"
            )));
        }
        let value = denormalize(normalized, param.min, param.max, param.default);
        // SAFETY: AudioUnitSetParameter writes one global parameter on a live unit.
        let status = unsafe {
            coreaudio_sys::AudioUnitSetParameter(
                self.unit,
                param.id,
                coreaudio_sys::kAudioUnitScope_Global,
                0,
                value,
                0,
            )
        };
        core_audio_status("AudioUnitSetParameter", status)
            .map_err(|error| ControllerError::Backend(error.to_string()))
    }

    pub fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        au_class_info(self.unit).map_err(|error| ControllerError::Backend(error.to_string()))
    }

    pub fn load_state(&mut self, state: &ProcessorState) -> Result<(), ProcessorStateError> {
        if state.0.is_empty() {
            return Ok(());
        }
        set_au_class_info(self.unit, &state.0)
            .map_err(|error| ProcessorStateError::Decode(error.to_string()))
    }

    pub fn reset(&mut self) {
        if let Err(error) = self.reinitialize() {
            log::warn!("AU reset failed: {error}");
        }
    }

    pub fn latency_samples(&self) -> u32 {
        get_property::<f64>(
            self.unit,
            coreaudio_sys::kAudioUnitProperty_Latency,
            coreaudio_sys::kAudioUnitScope_Global,
            0,
        )
        .map(|seconds| {
            (seconds * self.sample_rate)
                .max(0.0)
                .round()
                .to_u32()
                .unwrap_or(u32::MAX)
        })
        .unwrap_or(0)
    }

    pub fn render_instrument(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        events: &[MidiEvent],
    ) -> Result<(), AuRuntimeError> {
        self.render_calls = self.render_calls.saturating_add(1);
        let render_call = self.render_calls;
        for event in events.iter().filter_map(|event| midi_event_data(*event)) {
            if should_trace_render(render_call, events.len()) {
                log::trace!(
                    target: "lilypalooza_au",
                    "AU MIDI render_call={} status={:#04x} data1={} data2={}",
                    render_call,
                    event[0],
                    event[1],
                    event[2]
                );
            }
            // SAFETY: The AudioUnit is live and MusicDeviceMIDIEvent accepts plain MIDI bytes.
            let status = unsafe {
                coreaudio_sys::MusicDeviceMIDIEvent(
                    self.unit,
                    u32::from(event[0]),
                    u32::from(event[1]),
                    u32::from(event[2]),
                    0,
                )
            };
            core_audio_status("MusicDeviceMIDIEvent", status)?;
        }
        let trace_render = should_trace_render(render_call, events.len());
        let result = render_stereo(self.unit, left, right, self.sample_time, trace_render);
        self.sample_time += left.len().min(right.len()) as f64;
        if trace_render {
            log::trace!(
                target: "lilypalooza_au",
                "AU instrument render_call={} sample_time={} frames={} midi_events={} result={:?} peak={:.6}",
                render_call,
                self.sample_time,
                left.len().min(right.len()),
                events.len(),
                result,
                stereo_peak(left, right)
            );
        }
        result
    }

    pub fn process_effect(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) -> Result<(), AuRuntimeError> {
        self.render_calls = self.render_calls.saturating_add(1);
        let render_call = self.render_calls;
        let input_peak = stereo_peak(in_left, in_right);
        self.input.left = in_left.as_ptr();
        self.input.right = in_right.as_ptr();
        self.input.frames = in_left.len().min(in_right.len());
        let trace_render = should_trace_render(render_call, 0);
        let result = render_stereo(
            self.unit,
            out_left,
            out_right,
            self.sample_time,
            trace_render,
        );
        self.sample_time += out_left.len().min(out_right.len()) as f64;
        self.input.left = std::ptr::null();
        self.input.right = std::ptr::null();
        self.input.frames = 0;
        if trace_render {
            log::trace!(
                target: "lilypalooza_au",
                "AU effect render_call={} sample_time={} frames={} result={:?} input_peak={:.6} output_peak={:.6}",
                render_call,
                self.sample_time,
                out_left.len().min(out_right.len()),
                result,
                input_peak,
                stereo_peak(out_left, out_right)
            );
        }
        result
    }

    pub fn prepare_destroy(&mut self) {
        if self.disposed {
            return;
        }
        // SAFETY: Unit was created by AudioComponentInstanceNew and may be uninitialized here.
        let status = unsafe { coreaudio_sys::AudioUnitUninitialize(self.unit) };
        if let Err(error) = core_audio_status("AudioUnitUninitialize", status) {
            log::warn!("AU uninitialize failed during destroy: {error}");
        }
        // SAFETY: Unit was created by AudioComponentInstanceNew and is disposed only once.
        let status = unsafe { coreaudio_sys::AudioComponentInstanceDispose(self.unit) };
        if let Err(error) = core_audio_status("AudioComponentInstanceDispose", status) {
            log::warn!("AU dispose failed during destroy: {error}");
        }
        self.disposed = true;
    }

    fn configure(
        &mut self,
        sample_rate: usize,
        block_size: usize,
        role: registry::Role,
    ) -> Result<(), AuRuntimeError> {
        log::trace!(
            target: "lilypalooza_au",
            "configure AU role={role:?} sample_rate={} block_size={}",
            sample_rate,
            block_size
        );
        self.uninitialize_if_needed();
        let format = stereo_format(sample_rate.max(1) as f64);
        configure_max_frames(self.unit, block_size)?;
        configure_output_stream(self.unit, &format)?;
        if role == registry::Role::Effect {
            configure_effect_input(self.unit, &format, &mut self.input)?;
        }
        initialize_unit(self.unit)
    }

    fn collect_parameters(&self) -> Vec<AuParameter> {
        parameter_ids(self.unit)
            .into_iter()
            .filter_map(|id| au_parameter(self.unit, id))
            .collect()
    }

    fn parameter(&self, id: &str) -> Result<&AuParameter, ControllerError> {
        let id = id
            .parse::<u32>()
            .map_err(|_error| ControllerError::UnknownParameter(id.to_string()))?;
        self.parameter_cache
            .iter()
            .find(|param| param.id == id)
            .ok_or_else(|| ControllerError::UnknownParameter(id.to_string()))
    }

    fn reinitialize(&mut self) -> Result<(), AuRuntimeError> {
        self.uninitialize_if_needed();
        self.sample_time = 0.0;
        log::trace!(target: "lilypalooza_au", "reinitialize AU");
        // SAFETY: Reinitializes a live unit after reset/uninitialize.
        let status = unsafe { coreaudio_sys::AudioUnitInitialize(self.unit) };
        core_audio_status("AudioUnitInitialize", status)
    }

    fn uninitialize_if_needed(&self) {
        // SAFETY: Uninitializing an already uninitialized AudioUnit is handled by Core Audio.
        let status = unsafe { coreaudio_sys::AudioUnitUninitialize(self.unit) };
        if let Err(error) = core_audio_status("AudioUnitUninitialize", status) {
            log::trace!("AU uninitialize ignored: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for AuRuntime {
    fn drop(&mut self) {
        self.prepare_destroy();
    }
}

#[cfg(target_os = "macos")]
fn configure_max_frames(
    unit: coreaudio_sys::AudioUnit,
    block_size: usize,
) -> Result<(), AuRuntimeError> {
    let max_frames = u32::try_from(block_size.max(1)).unwrap_or(u32::MAX);
    set_property(
        unit,
        coreaudio_sys::kAudioUnitProperty_MaximumFramesPerSlice,
        coreaudio_sys::kAudioUnitScope_Global,
        0,
        &max_frames,
    )
}

#[cfg(target_os = "macos")]
fn configure_output_stream(
    unit: coreaudio_sys::AudioUnit,
    format: &coreaudio_sys::AudioStreamBasicDescription,
) -> Result<(), AuRuntimeError> {
    set_property(
        unit,
        coreaudio_sys::kAudioUnitProperty_StreamFormat,
        coreaudio_sys::kAudioUnitScope_Output,
        0,
        format,
    )
}

#[cfg(target_os = "macos")]
fn configure_effect_input(
    unit: coreaudio_sys::AudioUnit,
    format: &coreaudio_sys::AudioStreamBasicDescription,
    input: &mut Box<AuInputState>,
) -> Result<(), AuRuntimeError> {
    set_property(
        unit,
        coreaudio_sys::kAudioUnitProperty_StreamFormat,
        coreaudio_sys::kAudioUnitScope_Input,
        0,
        format,
    )?;
    let callback = coreaudio_sys::AURenderCallbackStruct {
        inputProc: Some(effect_input_callback),
        inputProcRefCon: (&mut **input as *mut AuInputState).cast(),
    };
    set_property(
        unit,
        coreaudio_sys::kAudioUnitProperty_SetRenderCallback,
        coreaudio_sys::kAudioUnitScope_Input,
        0,
        &callback,
    )
}

#[cfg(target_os = "macos")]
fn initialize_unit(unit: coreaudio_sys::AudioUnit) -> Result<(), AuRuntimeError> {
    // SAFETY: The unit is fully configured before initialization.
    let status = unsafe { coreaudio_sys::AudioUnitInitialize(unit) };
    core_audio_status("AudioUnitInitialize", status)
}

#[cfg(not(target_os = "macos"))]
impl AuRuntime {
    pub fn instantiate(
        _metadata: &AuPluginMetadata,
        descriptor: &'static ProcessorDescriptor,
        _sample_rate: usize,
        _block_size: usize,
    ) -> Result<Self, AuRuntimeError> {
        Err(AuRuntimeError::UnsupportedPlatform)
    }

    pub fn descriptor(&self) -> &'static ProcessorDescriptor {
        self.descriptor
    }

    pub fn parameters(&self) -> Vec<ParameterInfo> {
        Vec::new()
    }

    pub fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    pub fn set_param(&self, id: &str, _normalized: f32) -> Result<(), ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    pub fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        Ok(ProcessorState::default())
    }

    pub fn load_state(&mut self, _state: &ProcessorState) -> Result<(), ProcessorStateError> {
        Ok(())
    }

    pub fn reset(&mut self) {}

    pub fn latency_samples(&self) -> u32 {
        0
    }

    pub fn render_instrument(
        &mut self,
        _left: &mut [f32],
        _right: &mut [f32],
        _events: &[MidiEvent],
    ) -> Result<(), AuRuntimeError> {
        Err(AuRuntimeError::UnsupportedPlatform)
    }

    pub fn process_effect(
        &mut self,
        _in_left: &[f32],
        _in_right: &[f32],
        _out_left: &mut [f32],
        _out_right: &mut [f32],
    ) -> Result<(), AuRuntimeError> {
        Err(AuRuntimeError::UnsupportedPlatform)
    }

    pub fn prepare_destroy(&mut self) {}
}

#[cfg(target_os = "macos")]
fn instantiate_audio_unit(
    component: AuComponentId,
) -> Result<coreaudio_sys::AudioUnit, AuRuntimeError> {
    let desc = coreaudio_sys::AudioComponentDescription {
        componentType: component.component_type,
        componentSubType: component.component_subtype,
        componentManufacturer: component.component_manufacturer,
        componentFlags: 0,
        componentFlagsMask: 0,
    };
    // SAFETY: The description is a valid stack value and Core Audio only reads it during the call.
    let component = unsafe { coreaudio_sys::AudioComponentFindNext(std::ptr::null_mut(), &desc) };
    if component.is_null() {
        return Err(AuRuntimeError::ComponentNotFound);
    }
    let mut unit = std::ptr::null_mut();
    // SAFETY: `component` is non-null and returned by AudioComponentFindNext.
    let status = unsafe { coreaudio_sys::AudioComponentInstanceNew(component, &mut unit) };
    core_audio_status("AudioComponentInstanceNew", status)?;
    Ok(unit)
}

#[cfg(target_os = "macos")]
fn stereo_format(sample_rate: f64) -> coreaudio_sys::AudioStreamBasicDescription {
    coreaudio_sys::AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: coreaudio_sys::kAudioFormatLinearPCM,
        mFormatFlags: coreaudio_sys::kAudioFormatFlagIsFloat
            | coreaudio_sys::kAudioFormatFlagIsPacked
            | coreaudio_sys::kAudioFormatFlagIsNonInterleaved,
        mBytesPerPacket: std::mem::size_of::<f32>() as u32,
        mFramesPerPacket: 1,
        mBytesPerFrame: std::mem::size_of::<f32>() as u32,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct StereoBufferList {
    buffers: u32,
    buffer: [coreaudio_sys::AudioBuffer; 2],
}

#[cfg(target_os = "macos")]
fn render_stereo(
    unit: coreaudio_sys::AudioUnit,
    left: &mut [f32],
    right: &mut [f32],
    sample_time: f64,
    trace_render: bool,
) -> Result<(), AuRuntimeError> {
    let frames = left.len().min(right.len());
    let mut buffers = StereoBufferList {
        buffers: 2,
        buffer: [audio_buffer(left, frames), audio_buffer(right, frames)],
    };
    let mut flags = 0;
    let timestamp = render_timestamp(sample_time);
    // SAFETY: `buffers` is a C-compatible two-buffer AudioBufferList for the duration of render.
    let status = unsafe {
        coreaudio_sys::AudioUnitRender(
            unit,
            &mut flags,
            &timestamp,
            0,
            u32::try_from(frames).unwrap_or(u32::MAX),
            (&mut buffers as *mut StereoBufferList).cast(),
        )
    };
    if trace_render {
        log::trace!(
            target: "lilypalooza_au",
            "AudioUnitRender status={status} flags={flags:#x} sample_time={sample_time} frames={frames}"
        );
    }
    core_audio_status("AudioUnitRender", status)
}

#[cfg(target_os = "macos")]
fn render_timestamp(sample_time: f64) -> coreaudio_sys::AudioTimeStamp {
    coreaudio_sys::AudioTimeStamp {
        mSampleTime: sample_time,
        mFlags: coreaudio_sys::kAudioTimeStampSampleTimeValid,
        ..coreaudio_sys::AudioTimeStamp::default()
    }
}

fn should_trace_render(render_call: u64, midi_events: usize) -> bool {
    render_call <= 16 || midi_events > 0 || render_call.is_multiple_of(512)
}

fn stereo_peak(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .chain(right.iter())
        .map(|sample| sample.abs())
        .fold(0.0, f32::max)
}

#[cfg(target_os = "macos")]
fn audio_buffer(buffer: &mut [f32], frames: usize) -> coreaudio_sys::AudioBuffer {
    coreaudio_sys::AudioBuffer {
        mNumberChannels: 1,
        mDataByteSize: u32::try_from(frames.saturating_mul(std::mem::size_of::<f32>()))
            .unwrap_or(u32::MAX),
        mData: buffer.as_mut_ptr().cast(),
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn effect_input_callback(
    refcon: *mut std::ffi::c_void,
    _flags: *mut coreaudio_sys::AudioUnitRenderActionFlags,
    _timestamp: *const coreaudio_sys::AudioTimeStamp,
    _bus: u32,
    frames: u32,
    data: *mut coreaudio_sys::AudioBufferList,
) -> i32 {
    if refcon.is_null() || data.is_null() {
        return -1;
    }
    // SAFETY: `refcon` is the `AuInputState` pointer installed in the render callback property.
    let input = unsafe { &*(refcon.cast::<AuInputState>()) };
    let frames = usize::try_from(frames)
        .unwrap_or(input.frames)
        .min(input.frames);
    // SAFETY: The host requests non-interleaved stereo and receives two AudioBuffer entries.
    let buffers = unsafe { &mut *(data.cast::<StereoBufferList>()) };
    copy_input_channel(input.left, frames, buffers.buffer[0].mData);
    copy_input_channel(input.right, frames, buffers.buffer[1].mData);
    0
}

#[cfg(target_os = "macos")]
fn copy_input_channel(source: *const f32, frames: usize, destination: *mut std::ffi::c_void) {
    if source.is_null() || destination.is_null() {
        return;
    }
    // SAFETY: `source` points to the current input slice for at least `frames` samples.
    let source = unsafe { std::slice::from_raw_parts(source, frames) };
    // SAFETY: Core Audio provided a writable output channel for at least `frames` samples.
    let destination = unsafe { std::slice::from_raw_parts_mut(destination.cast::<f32>(), frames) };
    destination.copy_from_slice(source);
}

#[cfg(target_os = "macos")]
fn parameter_ids(unit: coreaudio_sys::AudioUnit) -> Vec<u32> {
    let Ok(size) = property_size(
        unit,
        coreaudio_sys::kAudioUnitProperty_ParameterList,
        coreaudio_sys::kAudioUnitScope_Global,
        0,
    ) else {
        return Vec::new();
    };
    let count = usize::try_from(size).unwrap_or(0) / std::mem::size_of::<u32>();
    let mut ids = vec![0; count];
    if get_property_into(
        unit,
        coreaudio_sys::kAudioUnitProperty_ParameterList,
        coreaudio_sys::kAudioUnitScope_Global,
        0,
        &mut ids,
    )
    .is_err()
    {
        return Vec::new();
    }
    ids
}

#[cfg(target_os = "macos")]
fn au_parameter(unit: coreaudio_sys::AudioUnit, id: u32) -> Option<AuParameter> {
    let info = get_property::<coreaudio_sys::AudioUnitParameterInfo>(
        unit,
        coreaudio_sys::kAudioUnitProperty_ParameterInfo,
        coreaudio_sys::kAudioUnitScope_Global,
        id,
    )
    .ok()?;
    let name = parameter_name(id, &info);
    let readable = info.flags & coreaudio_sys::kAudioUnitParameterFlag_IsReadable != 0;
    let writable = info.flags & coreaudio_sys::kAudioUnitParameterFlag_IsWritable != 0;
    Some(AuParameter {
        id,
        name,
        min: info.minValue,
        max: info.maxValue,
        default: info.defaultValue,
        writable,
        readable,
    })
}

#[cfg(target_os = "macos")]
fn parameter_name(id: u32, info: &coreaudio_sys::AudioUnitParameterInfo) -> String {
    let end = info
        .name
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(info.name.len());
    let bytes = info
        .name
        .get(..end)
        .unwrap_or(&info.name)
        .iter()
        .map(|value| value.cast_unsigned())
        .collect::<Vec<_>>();
    let name = String::from_utf8_lossy(&bytes).trim().to_string();
    if name.is_empty() {
        format!("Parameter {id}")
    } else {
        name
    }
}

#[cfg(target_os = "macos")]
fn au_class_info(unit: coreaudio_sys::AudioUnit) -> Result<ProcessorState, AuRuntimeError> {
    let plist = get_property::<coreaudio_sys::CFPropertyListRef>(
        unit,
        coreaudio_sys::kAudioUnitProperty_ClassInfo,
        coreaudio_sys::kAudioUnitScope_Global,
        0,
    )?;
    if plist.is_null() {
        return Ok(ProcessorState::default());
    }
    // SAFETY: `plist` is a valid CF property list returned by AudioUnitGetProperty.
    let data = unsafe {
        coreaudio_sys::CFPropertyListCreateData(
            std::ptr::null(),
            plist,
            coreaudio_sys::kCFPropertyListBinaryFormat_v1_0 as i64,
            0,
            std::ptr::null_mut(),
        )
    };
    if data.is_null() {
        // SAFETY: `plist` follows Create/Copy ownership and must be released on this path.
        unsafe { coreaudio_sys::CFRelease(plist.cast()) };
        return Ok(ProcessorState::default());
    }
    // SAFETY: `data` is a non-null CFData returned by CFPropertyListCreateData.
    let len = unsafe { coreaudio_sys::CFDataGetLength(data) };
    // SAFETY: `data` is non-null and lives until the matching CFRelease below.
    let ptr = unsafe { coreaudio_sys::CFDataGetBytePtr(data) };
    // SAFETY: CFData exposes a stable byte buffer for `len` bytes while `data` is live.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, usize::try_from(len).unwrap_or(0)) };
    let state = ProcessorState(bytes.to_vec());
    // SAFETY: `data` follows Create ownership and is no longer used.
    unsafe { coreaudio_sys::CFRelease(data.cast()) };
    // SAFETY: `plist` follows Create/Copy ownership and is no longer used.
    unsafe { coreaudio_sys::CFRelease(plist.cast()) };
    Ok(state)
}

#[cfg(target_os = "macos")]
fn set_au_class_info(unit: coreaudio_sys::AudioUnit, bytes: &[u8]) -> Result<(), AuRuntimeError> {
    // SAFETY: `bytes` is readable for its length and CFData copies the buffer.
    let data = unsafe {
        coreaudio_sys::CFDataCreate(
            std::ptr::null(),
            bytes.as_ptr(),
            i64::try_from(bytes.len()).unwrap_or(i64::MAX),
        )
    };
    if data.is_null() {
        return Err(AuRuntimeError::CoreAudio {
            operation: "CFDataCreate",
            status: -1,
        });
    }
    let mut format = 0;
    // SAFETY: `data` is a non-null CFData containing serialized property-list bytes.
    let plist = unsafe {
        coreaudio_sys::CFPropertyListCreateWithData(
            std::ptr::null(),
            data,
            0,
            &mut format,
            std::ptr::null_mut(),
        )
    };
    // SAFETY: `data` follows Create ownership and is no longer used after plist creation.
    unsafe { coreaudio_sys::CFRelease(data.cast()) };
    if plist.is_null() {
        return Err(AuRuntimeError::CoreAudio {
            operation: "CFPropertyListCreateWithData",
            status: -1,
        });
    }
    // SAFETY: `plist` is a valid CFPropertyListRef passed by reference as required by AudioUnit.
    let status = unsafe {
        coreaudio_sys::AudioUnitSetProperty(
            unit,
            coreaudio_sys::kAudioUnitProperty_ClassInfo,
            coreaudio_sys::kAudioUnitScope_Global,
            0,
            (&plist as *const coreaudio_sys::CFPropertyListRef).cast(),
            std::mem::size_of::<coreaudio_sys::CFPropertyListRef>() as u32,
        )
    };
    // SAFETY: `plist` follows Create ownership and is no longer used.
    unsafe { coreaudio_sys::CFRelease(plist.cast()) };
    core_audio_status("AudioUnitSetProperty(ClassInfo)", status)
}

#[cfg(target_os = "macos")]
fn set_property<T>(
    unit: coreaudio_sys::AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
    value: &T,
) -> Result<(), AuRuntimeError> {
    // SAFETY: The value pointer is valid for the duration of AudioUnitSetProperty.
    let status = unsafe {
        coreaudio_sys::AudioUnitSetProperty(
            unit,
            property,
            scope,
            element,
            (value as *const T).cast(),
            std::mem::size_of::<T>() as u32,
        )
    };
    core_audio_status("AudioUnitSetProperty", status)
}

#[cfg(target_os = "macos")]
fn get_property<T: Default>(
    unit: coreaudio_sys::AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
) -> Result<T, AuRuntimeError> {
    let mut value = T::default();
    let mut size = std::mem::size_of::<T>() as u32;
    // SAFETY: `value` points to writable storage for the requested property type.
    let status = unsafe {
        coreaudio_sys::AudioUnitGetProperty(
            unit,
            property,
            scope,
            element,
            (&mut value as *mut T).cast(),
            &mut size,
        )
    };
    core_audio_status("AudioUnitGetProperty", status).map(|()| value)
}

#[cfg(target_os = "macos")]
fn get_property_into<T>(
    unit: coreaudio_sys::AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
    value: &mut [T],
) -> Result<(), AuRuntimeError> {
    let mut size = u32::try_from(std::mem::size_of_val(value)).unwrap_or(u32::MAX);
    // SAFETY: `value` points to writable contiguous storage for the property payload.
    let status = unsafe {
        coreaudio_sys::AudioUnitGetProperty(
            unit,
            property,
            scope,
            element,
            value.as_mut_ptr().cast(),
            &mut size,
        )
    };
    core_audio_status("AudioUnitGetProperty", status)
}

#[cfg(target_os = "macos")]
fn property_size(
    unit: coreaudio_sys::AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
) -> Result<u32, AuRuntimeError> {
    let mut size = 0;
    let mut writable = 0;
    // SAFETY: Output pointers are valid stack slots for Core Audio to fill.
    let status = unsafe {
        coreaudio_sys::AudioUnitGetPropertyInfo(
            unit,
            property,
            scope,
            element,
            &mut size,
            &mut writable,
        )
    };
    core_audio_status("AudioUnitGetPropertyInfo", status).map(|()| size)
}

#[cfg(target_os = "macos")]
fn core_audio_status(operation: &'static str, status: i32) -> Result<(), AuRuntimeError> {
    if status == 0 {
        Ok(())
    } else {
        Err(AuRuntimeError::CoreAudio { operation, status })
    }
}

fn normalize(value: f32, min: f32, max: f32) -> f32 {
    let range = max - min;
    if !range.is_finite() || range.abs() <= f32::EPSILON {
        return 0.0;
    }
    ((value - min) / range).clamp(0.0, 1.0)
}

fn denormalize(normalized: f32, min: f32, max: f32, default: f32) -> f32 {
    let range = max - min;
    if !range.is_finite() || range.abs() <= f32::EPSILON {
        return default;
    }
    min + range * normalized.clamp(0.0, 1.0)
}

fn midi_event_data(event: MidiEvent) -> Option<[u8; 3]> {
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
        MidiEvent::ControlChange {
            channel,
            controller,
            value,
        } => Some([0xb0 | (channel & 0x0f), controller, value]),
        MidiEvent::ProgramChange { channel, program } => {
            Some([0xc0 | (channel & 0x0f), program, 0])
        }
        MidiEvent::ChannelPressure { channel, pressure } => {
            Some([0xd0 | (channel & 0x0f), pressure, 0])
        }
        MidiEvent::PitchBend { channel, value } => {
            let value = u16::try_from((i32::from(value) + 8192).clamp(0, 16_383)).ok()?;
            Some([
                0xe0 | (channel & 0x0f),
                (value & 0x7f) as u8,
                (value >> 7) as u8,
            ])
        }
        MidiEvent::AllNotesOff { channel } => Some([0xb0 | (channel & 0x0f), 123, 0]),
        MidiEvent::AllSoundOff { channel } => Some([0xb0 | (channel & 0x0f), 120, 0]),
        MidiEvent::ResetAllControllers { channel } => Some([0xb0 | (channel & 0x0f), 121, 0]),
        MidiEvent::PolyPressure {
            channel,
            note,
            pressure,
        } => Some([0xa0 | (channel & 0x0f), note, pressure]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_au_parameter_values() {
        assert!((normalize(15.0, 10.0, 20.0) - 0.5).abs() < f32::EPSILON);
        assert!((denormalize(0.25, 10.0, 20.0, 0.0) - 12.5).abs() < f32::EPSILON);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn render_timestamp_marks_sample_time_valid() {
        let timestamp = render_timestamp(128.0);

        assert!((timestamp.mSampleTime - 128.0).abs() < f64::EPSILON);
        assert_eq!(
            timestamp.mFlags,
            coreaudio_sys::kAudioTimeStampSampleTimeValid
        );
    }

    #[test]
    fn midi_event_data_maps_channel_messages() {
        assert_eq!(
            midi_event_data(MidiEvent::NoteOn {
                channel: 17,
                note: 64,
                velocity: 100
            }),
            Some([0x91, 64, 100])
        );
        assert_eq!(
            midi_event_data(MidiEvent::NoteOff {
                channel: 1,
                note: 64,
                velocity: 32
            }),
            Some([0x81, 64, 32])
        );
        assert_eq!(
            midi_event_data(MidiEvent::ControlChange {
                channel: 2,
                controller: 7,
                value: 96
            }),
            Some([0xb2, 7, 96])
        );
        assert_eq!(
            midi_event_data(MidiEvent::ProgramChange {
                channel: 2,
                program: 10
            }),
            Some([0xc2, 10, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::ChannelPressure {
                channel: 3,
                pressure: 80
            }),
            Some([0xd3, 80, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::PitchBend {
                channel: 4,
                value: 0
            }),
            Some([0xe4, 0, 64])
        );
        assert_eq!(
            midi_event_data(MidiEvent::PitchBend {
                channel: 4,
                value: -8192
            }),
            Some([0xe4, 0, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::PitchBend {
                channel: 4,
                value: 8191
            }),
            Some([0xe4, 127, 127])
        );
        assert_eq!(
            midi_event_data(MidiEvent::AllNotesOff { channel: 5 }),
            Some([0xb5, 123, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::AllSoundOff { channel: 6 }),
            Some([0xb6, 120, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::ResetAllControllers { channel: 7 }),
            Some([0xb7, 121, 0])
        );
        assert_eq!(
            midi_event_data(MidiEvent::PolyPressure {
                channel: 8,
                note: 72,
                pressure: 40
            }),
            Some([0xa8, 72, 40])
        );
    }
}
