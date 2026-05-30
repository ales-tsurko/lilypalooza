use super::*;

pub(in crate::app) const RESIZE_IDLE_TIMEOUT: Duration = Duration::from_millis(140);

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum EditorResizeSource {
    SessionRequestedSize,
    HeaderZoom,
    NativeContentSize,
    IcedOuterEvent,
    DeferredOuterResize,
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum EditorResizeStage {
    Begin,
    Ignored,
    Accepted,
    Applied,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct EditorResizeTraceId(pub(in crate::app) u64);

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct EditorResizeTraceEvent<'a> {
    pub(in crate::app) id: EditorResizeTraceId,
    pub(in crate::app) source: EditorResizeSource,
    pub(in crate::app) stage: EditorResizeStage,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) window_id: Option<window::Id>,
    pub(in crate::app) current_content: Option<editor_host::Size>,
    pub(in crate::app) requested_content: Option<editor_host::Size>,
    pub(in crate::app) accepted_content: Option<editor_host::Size>,
    pub(in crate::app) outer_size: Option<editor_host::Size>,
    pub(in crate::app) note: Option<&'a str>,
}

pub(in crate::app) struct HostEditorResizeHandler {
    pub(in crate::app) host: InstalledHostResizeHandle,
    pub(in crate::app) base_content_size: Arc<SharedContentSize>,
    pub(in crate::app) startup_baseline_pending: Arc<AtomicBool>,
    pub(in crate::app) programmatic_outer_resizes: SharedProgrammaticOuterResizes,
    pub(in crate::app) controls_visible: Arc<AtomicBool>,
}

impl EditorResizeHandler for HostEditorResizeHandler {
    fn resize_editor(&self, size: EditorSize) -> Result<EditorSize, EditorError> {
        if self.controls_visible.load(Ordering::Relaxed) {
            return Ok(size);
        }
        let content_size = host_size_from_editor_size(size);
        let outer_size = self.host.outer_size_from_content_size(content_size);
        record_programmatic_outer_resize_size(&self.programmatic_outer_resizes, outer_size);
        trace_editor_resize(|| {
            format!("vst live resize applying content={content_size:?} outer={outer_size:?}")
        });
        self.host
            .resize_content_from_top(content_size)
            .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
        if adopt_startup_resize_baseline(
            &self.startup_baseline_pending,
            &self.base_content_size,
            content_size,
        ) {
            self.host.set_zoom_percent(100);
        }
        Ok(size)
    }
}

#[derive(Debug)]
pub(in crate::app) struct SharedContentSize {
    pub(in crate::app) width: AtomicU64,
    pub(in crate::app) height: AtomicU64,
}

pub(in crate::app) type SharedProgrammaticOuterResizes = Arc<ProgrammaticOuterResizeEchoes>;

impl SharedContentSize {
    pub(in crate::app) fn new(size: editor_host::Size) -> Self {
        Self {
            width: AtomicU64::new(size.width.to_bits()),
            height: AtomicU64::new(size.height.to_bits()),
        }
    }

    #[cfg(test)]
    pub(in crate::app) fn load(&self) -> editor_host::Size {
        editor_host::Size {
            width: f64::from_bits(self.width.load(Ordering::Relaxed)),
            height: f64::from_bits(self.height.load(Ordering::Relaxed)),
        }
    }

    pub(in crate::app) fn store(&self, size: editor_host::Size) {
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub(in crate::app) struct ProgrammaticOuterResizeEchoes {
    pub(in crate::app) next_sequence: AtomicU64,
    pub(in crate::app) slots: [ProgrammaticOuterResizeEcho; 8],
}

#[derive(Debug)]
pub(in crate::app) struct ProgrammaticOuterResizeEcho {
    pub(in crate::app) sequence: AtomicU64,
    pub(in crate::app) width: AtomicU64,
    pub(in crate::app) height: AtomicU64,
}

impl ProgrammaticOuterResizeEchoes {
    pub(in crate::app) fn new() -> Self {
        Self {
            next_sequence: AtomicU64::new(1),
            slots: std::array::from_fn(|_| ProgrammaticOuterResizeEcho::new()),
        }
    }

    pub(in crate::app) fn record(&self, size: editor_host::Size) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let index = usize::try_from(sequence).unwrap_or(0) % self.slots.len();
        if let Some(slot) = self.slots.get(index) {
            slot.store(sequence, size);
        }
    }

    pub(in crate::app) fn consume(&self, size: editor_host::Size) -> bool {
        self.slots.iter().any(|slot| slot.consume(size))
    }
}

impl ProgrammaticOuterResizeEcho {
    pub(in crate::app) fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            width: AtomicU64::new(0),
            height: AtomicU64::new(0),
        }
    }

    pub(in crate::app) fn store(&self, sequence: u64, size: editor_host::Size) {
        self.sequence.store(0, Ordering::Relaxed);
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
        self.sequence.store(sequence, Ordering::Relaxed);
    }

    pub(in crate::app) fn consume(&self, size: editor_host::Size) -> bool {
        let sequence = self.sequence.load(Ordering::Relaxed);
        if sequence == 0 {
            return false;
        }
        let stored = editor_host::Size {
            width: f64::from_bits(self.width.load(Ordering::Relaxed)),
            height: f64::from_bits(self.height.load(Ordering::Relaxed)),
        };
        if !same_host_size(stored, size) {
            return false;
        }
        self.sequence
            .compare_exchange(sequence, 0, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

/// Mixer-strip processor target.
///
/// `strip_index` follows the visible mixer strip order:
/// - `0` is the master strip
/// - `1..=track_count` are instrument tracks
/// - the remaining indices are bus strips
///
/// `slot_index` follows one shared convention on every strip:
/// - `0` is the instrument slot
/// - `1..` are effect slots
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::app) struct EditorTarget {
    pub(in crate::app) strip_index: usize,
    pub(in crate::app) slot_index: usize,
}

pub(in crate::app) struct EditorWindow {
    pub(in crate::app) title: String,
    pub(in crate::app) resizable: bool,
    pub(in crate::app) host_window_id: window::Id,
    pub(in crate::app) host: Option<InstalledHost>,
    pub(in crate::app) session: Box<dyn EditorSession>,
    pub(in crate::app) controller: SharedController,
    pub(in crate::app) native_editor_available: bool,
    pub(in crate::app) controls_visible: Arc<AtomicBool>,
    pub(in crate::app) native_view_content_size: Option<editor_host::Size>,
    pub(in crate::app) visible: bool,
    pub(in crate::app) tracks_native_content_resize: bool,
    pub(in crate::app) base_content_size: editor_host::Size,
    pub(in crate::app) base_content_size_shared: Option<Arc<SharedContentSize>>,
    pub(in crate::app) startup_baseline_pending: Option<Arc<AtomicBool>>,
    pub(in crate::app) pending_programmatic_outer_resizes: SharedProgrammaticOuterResizes,
    pub(in crate::app) pending_outer_resize: Option<editor_host::Size>,
    pub(in crate::app) pending_outer_resize_until: Option<Instant>,
    pub(in crate::app) pending_zoom_percent: Option<u32>,
    pub(in crate::app) pending_zoom_percent_until: Option<Instant>,
    pub(in crate::app) resize_aspect_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct NativeContentResizeObservation {
    pub(in crate::app) embedded_content_size: Option<editor_host::Size>,
    pub(in crate::app) native_content_size: editor_host::Size,
    pub(in crate::app) native_outer_size: editor_host::Size,
    pub(in crate::app) current_content: editor_host::Size,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct NativeContentResizeTrace {
    pub(in crate::app) id: EditorResizeTraceId,
    pub(in crate::app) stage: EditorResizeStage,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) window_id: window::Id,
    pub(in crate::app) current_content: editor_host::Size,
    pub(in crate::app) requested_content: editor_host::Size,
    pub(in crate::app) accepted_content: Option<editor_host::Size>,
    pub(in crate::app) outer_size: editor_host::Size,
    pub(in crate::app) note: Option<&'static str>,
}

pub(in crate::app) struct RequestedContentResizeTrace {
    pub(in crate::app) id: EditorResizeTraceId,
    pub(in crate::app) stage: EditorResizeStage,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) window_id: window::Id,
    pub(in crate::app) current_content: Option<editor_host::Size>,
    pub(in crate::app) requested_content: editor_host::Size,
    pub(in crate::app) accepted_content: Option<editor_host::Size>,
    pub(in crate::app) note: Option<&'static str>,
}

pub(in crate::app) struct DeferredOuterResizeTrace {
    pub(in crate::app) id: EditorResizeTraceId,
    pub(in crate::app) stage: EditorResizeStage,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) window_id: window::Id,
    pub(in crate::app) current_content: Option<editor_host::Size>,
    pub(in crate::app) requested_content: editor_host::Size,
    pub(in crate::app) accepted_content: Option<editor_host::Size>,
    pub(in crate::app) outer_size: editor_host::Size,
    pub(in crate::app) note: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub(in crate::app) struct DeferredOuterResizeRequest {
    pub(in crate::app) trace_id: EditorResizeTraceId,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) outer_size: editor_host::Size,
    pub(in crate::app) requested: editor_host::Size,
}

pub(in crate::app) struct PendingEditorWindow {
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) title: String,
    pub(in crate::app) resizable: bool,
    pub(in crate::app) host_window_id: window::Id,
    pub(in crate::app) session: Box<dyn EditorSession>,
    pub(in crate::app) controller: SharedController,
    pub(in crate::app) native_editor_available: bool,
    pub(in crate::app) controls_visible: Arc<AtomicBool>,
}

pub(in crate::app) struct EditorOpenRequest {
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) title: String,
    pub(in crate::app) resizable: bool,
    pub(in crate::app) session: Box<dyn EditorSession>,
    pub(in crate::app) controller: SharedController,
    pub(in crate::app) native_editor_available: bool,
    pub(in crate::app) controls_visible: Arc<AtomicBool>,
    pub(in crate::app) window_id: window::Id,
}

pub(in crate::app) struct RemovedEditorWindow {
    pub(in crate::app) window_id: window::Id,
    pub(in crate::app) host: Option<InstalledHost>,
    pub(in crate::app) session: Box<dyn EditorSession>,
}

pub(in crate::app) struct GenericEditorSession;

impl EditorSession for GenericEditorSession {
    fn tracks_native_content_resize(&self) -> bool {
        false
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

pub(in crate::app) struct AttachedHostSizing {
    pub(in crate::app) content_size: editor_host::Size,
    pub(in crate::app) wait_for_embedded_startup_baseline: bool,
}

#[cfg(test)]
pub(in crate::app) struct EmptyEditorController;

#[cfg(test)]
impl Controller for EmptyEditorController {
    fn descriptor(&self) -> &'static ProcessorDescriptor {
        static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
            name: "Processor",
            params: &[],
            editor: None,
        };
        &DESCRIPTOR
    }

    fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    fn set_param(&self, id: &str, _normalized: f32) -> Result<(), ControllerError> {
        Err(ControllerError::UnknownParameter(id.to_string()))
    }

    fn save_state(&self) -> Result<ProcessorState, ControllerError> {
        Ok(ProcessorState::default())
    }

    fn load_state(&self, _state: &ProcessorState) -> Result<(), ControllerError> {
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::app) fn empty_editor_controller() -> SharedController {
    Arc::new(Mutex::new(Box::new(EmptyEditorController)))
}

impl RemovedEditorWindow {
    pub(in crate::app) fn detach(mut self) -> Result<(), EditorError> {
        detach_session_before_dropping_host(self.session.as_mut(), self.host.take())
    }
}

pub(in crate::app) fn detach_session_before_dropping_host<Host>(
    session: &mut dyn EditorSession,
    host: Option<Host>,
) -> Result<(), EditorError> {
    let result = session.detach();
    drop(host);
    result
}

pub(in crate::app) fn install_host_resize_handler(
    session: &mut dyn EditorSession,
    host: Option<&InstalledHost>,
    base_content_size: Option<&Arc<SharedContentSize>>,
    startup_baseline_pending: Option<&Arc<AtomicBool>>,
    programmatic_outer_resizes: &SharedProgrammaticOuterResizes,
    controls_visible: &Arc<AtomicBool>,
) -> Result<(), EditorError> {
    let Some(host) = host else {
        return Ok(());
    };
    let (Some(base_content_size), Some(startup_baseline_pending)) =
        (base_content_size, startup_baseline_pending)
    else {
        return Err(EditorError::HostUnavailable(
            "host resize state is missing".to_string(),
        ));
    };
    session.set_resize_handler(Some(Arc::new(HostEditorResizeHandler {
        host: host.resize_handle(),
        base_content_size: Arc::clone(base_content_size),
        startup_baseline_pending: Arc::clone(startup_baseline_pending),
        programmatic_outer_resizes: Arc::clone(programmatic_outer_resizes),
        controls_visible: Arc::clone(controls_visible),
    })))
}

pub(in crate::app) fn configure_native_resize_tracking(
    window_id: window::Id,
    target: EditorTarget,
    session: &dyn EditorSession,
    host: Option<&InstalledHost>,
) -> Result<bool, EditorError> {
    let tracks_native_content_resize = session.tracks_native_content_resize();
    let Some(host) = host else {
        return Ok(tracks_native_content_resize);
    };
    if tracks_native_content_resize {
        host.enable_native_content_resize_tracking()
            .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
    } else {
        trace_editor_resize(|| {
            format!(
                "native content resize tracking disabled target={target:?} window_id={window_id:?}"
            )
        });
    }
    Ok(tracks_native_content_resize)
}

pub(in crate::app) fn apply_session_resizability(
    session: &mut dyn EditorSession,
    resizable: &mut bool,
    host: Option<&mut InstalledHost>,
) -> Result<(), EditorError> {
    let Some(session_resizable) = session.resizable()? else {
        return Ok(());
    };
    *resizable = session_resizable;
    if let Some(host) = host {
        host.set_resizable(session_resizable)
            .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
    }
    Ok(())
}

pub(in crate::app) fn initialize_attached_host_size(
    session: &mut dyn EditorSession,
    host: Option<&mut InstalledHost>,
    startup_baseline_pending: Option<&Arc<AtomicBool>>,
    programmatic_outer_resizes: &SharedProgrammaticOuterResizes,
) -> Result<AttachedHostSizing, EditorError> {
    let Some(host) = host else {
        return Ok(AttachedHostSizing {
            content_size: editor_host::Size {
                width: 1.0,
                height: 1.0,
            },
            wait_for_embedded_startup_baseline: false,
        });
    };
    record_programmatic_outer_resize(programmatic_outer_resizes, host, host.content_size());
    let embedded_size = host
        .embedded_content_size()
        .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
    let wait_for_embedded_startup_baseline = embedded_size.is_none();
    mark_startup_baseline_ready(startup_baseline_pending, wait_for_embedded_startup_baseline);
    resize_host_to_initial_content(session, host, embedded_size, programmatic_outer_resizes)?;
    resize_host_to_embedded_baseline(host, embedded_size, programmatic_outer_resizes)?;
    Ok(AttachedHostSizing {
        content_size: host.content_size(),
        wait_for_embedded_startup_baseline,
    })
}

pub(in crate::app) fn mark_startup_baseline_ready(
    startup_baseline_pending: Option<&Arc<AtomicBool>>,
    wait_for_embedded_startup_baseline: bool,
) {
    if !wait_for_embedded_startup_baseline
        && let Some(startup_baseline_pending) = startup_baseline_pending
    {
        startup_baseline_pending.store(false, Ordering::Relaxed);
    }
}

pub(in crate::app) fn resize_host_to_initial_content(
    session: &mut dyn EditorSession,
    host: &mut InstalledHost,
    embedded_size: Option<editor_host::Size>,
    programmatic_outer_resizes: &SharedProgrammaticOuterResizes,
) -> Result<(), EditorError> {
    let initial_size = session.initial_size()?;
    let content_size =
        attached_start_content_size(host.content_size(), embedded_size, initial_size);
    resize_host_content_if_needed(host, content_size, programmatic_outer_resizes)
}

pub(in crate::app) fn resize_host_to_embedded_baseline(
    host: &mut InstalledHost,
    embedded_size: Option<editor_host::Size>,
    programmatic_outer_resizes: &SharedProgrammaticOuterResizes,
) -> Result<(), EditorError> {
    let baseline_content_size = attached_baseline_content_size(host.content_size(), embedded_size);
    resize_host_content_if_needed(host, baseline_content_size, programmatic_outer_resizes)
}

pub(in crate::app) fn resize_host_content_if_needed(
    host: &mut InstalledHost,
    content_size: editor_host::Size,
    programmatic_outer_resizes: &SharedProgrammaticOuterResizes,
) -> Result<(), EditorError> {
    if same_host_size(host.content_size(), content_size) {
        return Ok(());
    }
    record_programmatic_outer_resize(programmatic_outer_resizes, host, content_size);
    host.resize_content(content_size)
        .map_err(|error| EditorError::HostUnavailable(error.to_string()))
}

pub(in crate::app) fn apply_requested_content_resize_for_window(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    on_error: &mut impl FnMut(String),
) {
    if window.controls_visible.load(Ordering::Relaxed) {
        return;
    }
    match window.session.requested_size() {
        Ok(Some(size)) => {
            apply_requested_content_size(trace_counter, target, window, size, on_error);
        }
        Ok(None) => {}
        Err(error) => on_error(error.to_string()),
    }
}

pub(in crate::app) fn apply_requested_content_size(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    size: EditorSize,
    on_error: &mut impl FnMut(String),
) {
    let trace_id = next_resize_trace_id(trace_counter);
    let requested_content = host_size_from_editor_size(size);
    trace_requested_content_resize(RequestedContentResizeTrace {
        id: trace_id,
        stage: EditorResizeStage::Begin,
        target,
        window_id: window.host_window_id,
        current_content: window.host.as_ref().map(InstalledHost::content_size),
        requested_content,
        accepted_content: None,
        note: None,
    });
    let Some(host) = window.host.as_mut() else {
        trace_requested_content_resize(RequestedContentResizeTrace {
            id: trace_id,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: window.host_window_id,
            current_content: None,
            requested_content,
            accepted_content: None,
            note: Some("missing host"),
        });
        return;
    };
    let Some(content_size) = editor_content_resize_request(size, host) else {
        trace_requested_content_resize(RequestedContentResizeTrace {
            id: trace_id,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: window.host_window_id,
            current_content: Some(host.content_size()),
            requested_content,
            accepted_content: None,
            note: Some("same size"),
        });
        return;
    };
    record_programmatic_outer_resize(
        &window.pending_programmatic_outer_resizes,
        host,
        content_size,
    );
    resize_host_from_session_request(
        trace_id,
        target,
        window.host_window_id,
        host,
        content_size,
        on_error,
    );
}

pub(in crate::app) fn resize_host_from_session_request(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    host: &mut InstalledHost,
    content_size: editor_host::Size,
    on_error: &mut impl FnMut(String),
) {
    match host.resize_content_from_top(content_size) {
        Ok(()) => trace_requested_content_resize(RequestedContentResizeTrace {
            id: trace_id,
            stage: EditorResizeStage::Applied,
            target,
            window_id,
            current_content: Some(host.content_size()),
            requested_content: content_size,
            accepted_content: Some(content_size),
            note: None,
        }),
        Err(error) => {
            trace_requested_content_resize(RequestedContentResizeTrace {
                id: trace_id,
                stage: EditorResizeStage::Error,
                target,
                window_id,
                current_content: Some(host.content_size()),
                requested_content: content_size,
                accepted_content: None,
                note: Some("host resize failed"),
            });
            on_error(error.to_string());
        }
    }
}

pub(in crate::app) fn trace_requested_content_resize(trace: RequestedContentResizeTrace) {
    trace_resize_event(EditorResizeTraceEvent {
        id: trace.id,
        source: EditorResizeSource::SessionRequestedSize,
        stage: trace.stage,
        target: trace.target,
        window_id: Some(trace.window_id),
        current_content: trace.current_content,
        requested_content: Some(trace.requested_content),
        accepted_content: trace.accepted_content,
        outer_size: None,
        note: trace.note,
    });
}

pub(in crate::app) fn finish_deferred_outer_resize(
    request: DeferredOuterResizeRequest,
    window: &mut EditorWindow,
    negotiated: Result<editor_host::Size, EditorError>,
    errors: &mut Vec<String>,
) {
    match negotiated {
        Ok(content_size) => {
            trace_deferred_outer_resize_accepted(request, window, content_size);
            apply_deferred_content_size(request, window, content_size, errors);
        }
        Err(error) => {
            trace_deferred_outer_resize_error(request, &*window, None, "session rejected");
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn trace_deferred_outer_resize_accepted(
    request: DeferredOuterResizeRequest,
    window: &EditorWindow,
    content_size: editor_host::Size,
) {
    trace_deferred_outer_resize(DeferredOuterResizeTrace {
        id: request.trace_id,
        stage: EditorResizeStage::Accepted,
        target: request.target,
        window_id: window.host_window_id,
        current_content: window.host.as_ref().map(InstalledHost::content_size),
        requested_content: request.requested,
        accepted_content: Some(content_size),
        outer_size: request.outer_size,
        note: None,
    });
}

pub(in crate::app) fn apply_deferred_content_size(
    request: DeferredOuterResizeRequest,
    window: &mut EditorWindow,
    content_size: editor_host::Size,
    errors: &mut Vec<String>,
) {
    let Some(host) = window.host.as_mut() else {
        return;
    };
    if same_host_size(content_size, request.requested)
        && !needs_programmatic_outer_resize(host, request.outer_size, content_size)
    {
        apply_deferred_adopted_size(request, window.host_window_id, host, content_size, errors);
    } else {
        apply_deferred_programmatic_size(request, window, content_size, errors);
    }
}

pub(in crate::app) fn apply_deferred_adopted_size(
    request: DeferredOuterResizeRequest,
    window_id: window::Id,
    host: &mut InstalledHost,
    content_size: editor_host::Size,
    errors: &mut Vec<String>,
) {
    match host.adopt_content_size_from_outer_resize(content_size) {
        Ok(()) => {
            trace_deferred_outer_resize_applied(request, window_id, host, content_size, "adopted")
        }
        Err(error) => {
            trace_deferred_outer_resize_error(
                request,
                (&*host, window_id),
                Some(content_size),
                "adopt failed",
            );
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn apply_deferred_programmatic_size(
    request: DeferredOuterResizeRequest,
    window: &mut EditorWindow,
    content_size: editor_host::Size,
    errors: &mut Vec<String>,
) {
    let Some(host) = window.host.as_mut() else {
        return;
    };
    record_programmatic_outer_resize(
        &window.pending_programmatic_outer_resizes,
        host,
        content_size,
    );
    match host.resize_content_from_top(content_size) {
        Ok(()) => trace_deferred_outer_resize_applied(
            request,
            window.host_window_id,
            host,
            content_size,
            "plugin adjusted",
        ),
        Err(error) => {
            trace_deferred_outer_resize_error(
                request,
                (&*host, window.host_window_id),
                Some(content_size),
                "host resize failed",
            );
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn trace_deferred_outer_resize_applied(
    request: DeferredOuterResizeRequest,
    window_id: window::Id,
    host: &InstalledHost,
    content_size: editor_host::Size,
    note: &'static str,
) {
    trace_deferred_outer_resize(DeferredOuterResizeTrace {
        id: request.trace_id,
        stage: EditorResizeStage::Applied,
        target: request.target,
        window_id,
        current_content: Some(host.content_size()),
        requested_content: request.requested,
        accepted_content: Some(content_size),
        outer_size: request.outer_size,
        note: Some(note),
    });
}

fn trace_deferred_outer_resize_error(
    request: DeferredOuterResizeRequest,
    window: impl DeferredResizeWindowTrace,
    content_size: Option<editor_host::Size>,
    note: &'static str,
) {
    trace_deferred_outer_resize(DeferredOuterResizeTrace {
        id: request.trace_id,
        stage: EditorResizeStage::Error,
        target: request.target,
        window_id: window.window_id(),
        current_content: window.current_content(),
        requested_content: request.requested,
        accepted_content: content_size,
        outer_size: request.outer_size,
        note: Some(note),
    });
}

trait DeferredResizeWindowTrace {
    fn window_id(&self) -> window::Id;
    fn current_content(&self) -> Option<editor_host::Size>;
}

impl DeferredResizeWindowTrace for &EditorWindow {
    fn window_id(&self) -> window::Id {
        self.host_window_id
    }

    fn current_content(&self) -> Option<editor_host::Size> {
        self.host.as_ref().map(InstalledHost::content_size)
    }
}

impl DeferredResizeWindowTrace for (&InstalledHost, window::Id) {
    fn window_id(&self) -> window::Id {
        self.1
    }

    fn current_content(&self) -> Option<editor_host::Size> {
        Some(self.0.content_size())
    }
}

pub(in crate::app) fn trace_deferred_outer_resize(trace: DeferredOuterResizeTrace) {
    trace_resize_event(EditorResizeTraceEvent {
        id: trace.id,
        source: EditorResizeSource::DeferredOuterResize,
        stage: trace.stage,
        target: trace.target,
        window_id: Some(trace.window_id),
        current_content: trace.current_content,
        requested_content: Some(trace.requested_content),
        accepted_content: trace.accepted_content,
        outer_size: Some(trace.outer_size),
        note: trace.note,
    });
}

#[derive(Default)]
pub(in crate::app) struct EditorWindowManager {
    pub(in crate::app) windows: HashMap<EditorTarget, EditorWindow>,
    pub(in crate::app) pending: HashMap<window::Id, PendingEditorWindow>,
    pub(in crate::app) windows_by_id: HashMap<window::Id, EditorTarget>,
    pub(in crate::app) focused: Option<EditorTarget>,
    pub(in crate::app) next_resize_trace_id: u64,
}

pub(in crate::app) fn snapshot_into_editor_parent(
    snapshot: WindowSnapshot,
) -> Result<EditorParent, String> {
    let window = snapshot
        .raw_window_handle()
        .map_err(|error| error.to_string())?;
    let display = snapshot
        .raw_display_handle()
        .map_err(|error| error.to_string())?;
    Ok(EditorParent { window, display })
}
