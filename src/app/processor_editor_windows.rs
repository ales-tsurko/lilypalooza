use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use editor_host::{
    EditorFrameCommand, EditorPresetState, InstalledHost, InstalledHostResizeHandle, WindowSnapshot,
};
use iced::window;
use lilypalooza_audio::{
    EditorError, EditorParent, EditorResizeHandler, EditorSession, EditorSize,
};

const RESIZE_IDLE_TIMEOUT: Duration = Duration::from_millis(140);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorResizeSource {
    SessionRequestedSize,
    HeaderZoom,
    NativeContentSize,
    IcedOuterEvent,
    DeferredOuterResize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorResizeStage {
    Begin,
    Ignored,
    Accepted,
    Applied,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditorResizeTraceId(u64);

#[derive(Debug, Clone, Copy)]
struct EditorResizeTraceEvent<'a> {
    id: EditorResizeTraceId,
    source: EditorResizeSource,
    stage: EditorResizeStage,
    target: EditorTarget,
    window_id: Option<window::Id>,
    current_content: Option<editor_host::Size>,
    requested_content: Option<editor_host::Size>,
    accepted_content: Option<editor_host::Size>,
    outer_size: Option<editor_host::Size>,
    note: Option<&'a str>,
}

struct HostEditorResizeHandler {
    host: InstalledHostResizeHandle,
    base_content_size: Arc<SharedContentSize>,
    startup_baseline_pending: Arc<AtomicBool>,
    programmatic_outer_resizes: SharedProgrammaticOuterResizes,
}

impl EditorResizeHandler for HostEditorResizeHandler {
    fn resize_editor(&self, size: EditorSize) -> Result<EditorSize, EditorError> {
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
struct SharedContentSize {
    width: AtomicU64,
    height: AtomicU64,
}

type SharedProgrammaticOuterResizes = Arc<ProgrammaticOuterResizeEchoes>;

impl SharedContentSize {
    fn new(size: editor_host::Size) -> Self {
        Self {
            width: AtomicU64::new(size.width.to_bits()),
            height: AtomicU64::new(size.height.to_bits()),
        }
    }

    #[cfg(test)]
    fn load(&self) -> editor_host::Size {
        editor_host::Size {
            width: f64::from_bits(self.width.load(Ordering::Relaxed)),
            height: f64::from_bits(self.height.load(Ordering::Relaxed)),
        }
    }

    fn store(&self, size: editor_host::Size) {
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct ProgrammaticOuterResizeEchoes {
    next_sequence: AtomicU64,
    slots: [ProgrammaticOuterResizeEcho; 8],
}

#[derive(Debug)]
struct ProgrammaticOuterResizeEcho {
    sequence: AtomicU64,
    width: AtomicU64,
    height: AtomicU64,
}

impl ProgrammaticOuterResizeEchoes {
    fn new() -> Self {
        Self {
            next_sequence: AtomicU64::new(1),
            slots: std::array::from_fn(|_| ProgrammaticOuterResizeEcho::new()),
        }
    }

    fn record(&self, size: editor_host::Size) {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let slot = &self.slots[sequence as usize % self.slots.len()];
        slot.store(sequence, size);
    }

    fn consume(&self, size: editor_host::Size) -> bool {
        self.slots.iter().any(|slot| slot.consume(size))
    }
}

impl ProgrammaticOuterResizeEcho {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            width: AtomicU64::new(0),
            height: AtomicU64::new(0),
        }
    }

    fn store(&self, sequence: u64, size: editor_host::Size) {
        self.sequence.store(0, Ordering::Relaxed);
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
        self.sequence.store(sequence, Ordering::Relaxed);
    }

    fn consume(&self, size: editor_host::Size) -> bool {
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
pub(super) struct EditorTarget {
    pub(super) strip_index: usize,
    pub(super) slot_index: usize,
}

pub(super) struct EditorWindow {
    pub(super) title: String,
    pub(super) resizable: bool,
    pub(super) host_window_id: window::Id,
    pub(super) host: Option<InstalledHost>,
    pub(super) session: Box<dyn EditorSession>,
    pub(super) visible: bool,
    tracks_native_content_resize: bool,
    base_content_size: editor_host::Size,
    base_content_size_shared: Option<Arc<SharedContentSize>>,
    startup_baseline_pending: Option<Arc<AtomicBool>>,
    pending_programmatic_outer_resizes: SharedProgrammaticOuterResizes,
    pending_outer_resize: Option<editor_host::Size>,
    pending_outer_resize_until: Option<Instant>,
    pending_zoom_percent: Option<u32>,
    pending_zoom_percent_until: Option<Instant>,
    resize_aspect_ratio: f64,
}

pub(super) struct PendingEditorWindow {
    pub(super) target: EditorTarget,
    pub(super) title: String,
    pub(super) resizable: bool,
    pub(super) host_window_id: window::Id,
    pub(super) session: Box<dyn EditorSession>,
}

pub(super) struct RemovedEditorWindow {
    pub(super) window_id: window::Id,
    host: Option<InstalledHost>,
    session: Box<dyn EditorSession>,
}

impl RemovedEditorWindow {
    pub(super) fn detach(mut self) -> Result<(), EditorError> {
        detach_session_before_dropping_host(self.session.as_mut(), self.host.take())
    }
}

fn detach_session_before_dropping_host<Host>(
    session: &mut dyn EditorSession,
    host: Option<Host>,
) -> Result<(), EditorError> {
    let result = session.detach();
    drop(host);
    result
}

#[derive(Default)]
pub(super) struct EditorWindowManager {
    windows: HashMap<EditorTarget, EditorWindow>,
    pending: HashMap<window::Id, PendingEditorWindow>,
    windows_by_id: HashMap<window::Id, EditorTarget>,
    focused: Option<EditorTarget>,
    next_resize_trace_id: u64,
}

pub(super) fn snapshot_into_editor_parent(
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

fn editor_size_from_host_size(size: editor_host::Size) -> EditorSize {
    EditorSize {
        width: size.width.round().max(1.0) as u32,
        height: size.height.round().max(1.0) as u32,
    }
}

fn host_size_from_editor_size(size: EditorSize) -> editor_host::Size {
    editor_host::Size {
        width: f64::from(size.width),
        height: f64::from(size.height),
    }
}

fn editor_content_resize_request(
    requested: EditorSize,
    host: &InstalledHost,
) -> Option<editor_host::Size> {
    let requested = host_size_from_editor_size(requested);
    (!same_host_size(requested, host.content_size())).then_some(requested)
}

fn same_host_size(a: editor_host::Size, b: editor_host::Size) -> bool {
    (a.width - b.width).abs() < 0.5 && (a.height - b.height).abs() < 0.5
}

fn record_programmatic_outer_resize(
    pending: &SharedProgrammaticOuterResizes,
    host: &InstalledHost,
    content_size: editor_host::Size,
) {
    record_programmatic_outer_resize_size(pending, host.outer_size_from_content_size(content_size));
}

fn needs_programmatic_outer_resize(
    host: &InstalledHost,
    actual_outer_size: editor_host::Size,
    content_size: editor_host::Size,
) -> bool {
    needs_outer_writeback(
        actual_outer_size,
        host.outer_size_from_content_size(content_size),
    )
}

fn needs_outer_writeback(
    actual_outer_size: editor_host::Size,
    desired_outer_size: editor_host::Size,
) -> bool {
    !same_host_size(actual_outer_size, desired_outer_size)
}

fn record_programmatic_outer_resize_size(
    pending: &SharedProgrammaticOuterResizes,
    outer_size: editor_host::Size,
) {
    pending.record(outer_size);
}

fn consume_pending_programmatic_outer_resize(
    pending: &SharedProgrammaticOuterResizes,
    outer_size: editor_host::Size,
) -> bool {
    pending.consume(outer_size)
}

fn negotiate_editor_content_resize(
    session: &mut dyn EditorSession,
    requested: editor_host::Size,
) -> Result<editor_host::Size, EditorError> {
    session
        .resize(editor_size_from_host_size(requested))
        .map(host_size_from_editor_size)
}

fn aspect_preserved_resize(
    current: editor_host::Size,
    requested: editor_host::Size,
    aspect_ratio: f64,
) -> editor_host::Size {
    if !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
        return requested;
    }
    let width_delta = ((requested.width - current.width) / current.width.max(1.0)).abs();
    let height_delta = ((requested.height - current.height) / current.height.max(1.0)).abs();
    if height_delta > width_delta {
        editor_host::Size {
            width: requested.height * aspect_ratio,
            height: requested.height,
        }
    } else {
        editor_host::Size {
            width: requested.width,
            height: requested.width / aspect_ratio,
        }
    }
}

fn zoomed_content_size(base: editor_host::Size, percent: u32) -> editor_host::Size {
    let scale = f64::from(percent.clamp(
        super::EDITOR_FRAME_ZOOM_MIN_PERCENT,
        super::EDITOR_FRAME_ZOOM_MAX_PERCENT,
    )) / 100.0;
    let width = (base.width * scale).round().max(1.0);
    let height = if base.width.is_finite() && base.width > 0.0 && base.height.is_finite() {
        (width * base.height / base.width).round().max(1.0)
    } else {
        (base.height * scale).round().max(1.0)
    };
    editor_host::Size { width, height }
}

fn zoom_percent_for_content_size(base: editor_host::Size, content: editor_host::Size) -> u32 {
    let width_scale = content.width / base.width.max(1.0);
    let height_scale = content.height / base.height.max(1.0);
    let scale = if width_scale.is_finite() && height_scale.is_finite() {
        (width_scale + height_scale) * 0.5
    } else if width_scale.is_finite() {
        width_scale
    } else if height_scale.is_finite() {
        height_scale
    } else {
        1.0
    };
    (scale * 100.0).round().clamp(
        f64::from(super::EDITOR_FRAME_ZOOM_MIN_PERCENT),
        f64::from(super::EDITOR_FRAME_ZOOM_MAX_PERCENT),
    ) as u32
}

fn observed_native_content_size(
    embedded_content_size: Option<editor_host::Size>,
    host_content_size: Option<editor_host::Size>,
) -> Option<editor_host::Size> {
    embedded_content_size.or(host_content_size)
}

fn native_content_resize_request(native_content_size: editor_host::Size) -> editor_host::Size {
    native_content_size
}

fn attached_start_content_size(
    current: editor_host::Size,
    embedded: Option<editor_host::Size>,
    initial: Option<EditorSize>,
) -> editor_host::Size {
    if let Some(embedded) = embedded {
        return embedded;
    }
    initial.map_or(current, host_size_from_editor_size)
}

fn attached_baseline_content_size(
    host_content_size: editor_host::Size,
    embedded_content_size: Option<editor_host::Size>,
) -> editor_host::Size {
    embedded_content_size.unwrap_or(host_content_size)
}

fn startup_embedded_baseline_size(
    startup_baseline_pending: bool,
    embedded_content_size: Option<editor_host::Size>,
) -> Option<editor_host::Size> {
    startup_baseline_pending.then_some(embedded_content_size)?
}

fn adopt_startup_resize_baseline(
    startup_baseline_pending: &AtomicBool,
    base_content_size: &SharedContentSize,
    content_size: editor_host::Size,
) -> bool {
    if !startup_baseline_pending.swap(false, Ordering::Relaxed) {
        return false;
    }
    base_content_size.store(content_size);
    true
}

fn set_editor_resize_baseline(window: &mut EditorWindow, content_size: editor_host::Size) {
    window.base_content_size = content_size;
    if let Some(shared) = window.base_content_size_shared.as_ref() {
        shared.store(content_size);
    }
}

fn should_sync_native_content_resize(visible: bool, tracks_native_content_resize: bool) -> bool {
    visible && tracks_native_content_resize
}

fn next_deferred_outer_resize_deadline(
    pending_outer_resize: Option<editor_host::Size>,
    outer_size: editor_host::Size,
    current_deadline: Option<Instant>,
    now: Instant,
) -> Instant {
    if pending_outer_resize.is_some_and(|pending| same_host_size(pending, outer_size))
        && let Some(deadline) = current_deadline
    {
        return deadline;
    }
    now + RESIZE_IDLE_TIMEOUT
}

fn next_deferred_zoom_deadline(
    pending_zoom_percent: Option<u32>,
    zoom_percent: u32,
    current_deadline: Option<Instant>,
    now: Instant,
) -> Instant {
    if pending_zoom_percent == Some(zoom_percent)
        && let Some(deadline) = current_deadline
    {
        return deadline;
    }
    now + RESIZE_IDLE_TIMEOUT
}

fn defer_zoom_percent(window: &mut EditorWindow, zoom_percent: u32, now: Instant) {
    window.pending_zoom_percent_until = Some(next_deferred_zoom_deadline(
        window.pending_zoom_percent,
        zoom_percent,
        window.pending_zoom_percent_until,
        now,
    ));
    window.pending_zoom_percent = Some(zoom_percent);
}

fn next_resize_trace_id(next: &mut u64) -> EditorResizeTraceId {
    *next += 1;
    EditorResizeTraceId(*next)
}

fn resize_source_label(source: EditorResizeSource) -> &'static str {
    match source {
        EditorResizeSource::SessionRequestedSize => "session-requested-size",
        EditorResizeSource::HeaderZoom => "header-zoom",
        EditorResizeSource::NativeContentSize => "native-content-size",
        EditorResizeSource::IcedOuterEvent => "iced-outer-event",
        EditorResizeSource::DeferredOuterResize => "deferred-outer-resize",
    }
}

fn resize_stage_label(stage: EditorResizeStage) -> &'static str {
    match stage {
        EditorResizeStage::Begin => "begin",
        EditorResizeStage::Ignored => "ignored",
        EditorResizeStage::Accepted => "accepted",
        EditorResizeStage::Applied => "applied",
        EditorResizeStage::Error => "error",
    }
}

fn format_resize_size(size: editor_host::Size) -> String {
    format!(
        "{}x{}",
        size.width.round() as u32,
        size.height.round() as u32
    )
}

fn format_resize_trace_event(event: EditorResizeTraceEvent<'_>) -> String {
    let mut fields = vec![
        format!("resize#{}", event.id.0),
        format!("source={}", resize_source_label(event.source)),
        format!("stage={}", resize_stage_label(event.stage)),
        format!("target={:?}", event.target),
    ];
    if let Some(window_id) = event.window_id {
        fields.push(format!("window_id={window_id:?}"));
    }
    if let Some(size) = event.current_content {
        fields.push(format!("current={}", format_resize_size(size)));
    }
    if let Some(size) = event.requested_content {
        fields.push(format!("requested={}", format_resize_size(size)));
    }
    if let Some(size) = event.accepted_content {
        fields.push(format!("accepted={}", format_resize_size(size)));
    }
    if let Some(size) = event.outer_size {
        fields.push(format!("outer={}", format_resize_size(size)));
    }
    if let Some(note) = event.note {
        fields.push(format!("note={note}"));
    }
    fields.join(" ")
}

fn trace_resize_event(event: EditorResizeTraceEvent<'_>) {
    trace_editor_resize(|| format_resize_trace_event(event));
}

fn defer_outer_resize(
    trace_id: EditorResizeTraceId,
    source: EditorResizeSource,
    target: EditorTarget,
    window: &mut EditorWindow,
    outer_size: editor_host::Size,
) -> Vec<String> {
    let errors = Vec::new();
    let Some(host) = window.host.as_mut() else {
        trace_resize_event(EditorResizeTraceEvent {
            id: trace_id,
            source,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: Some(window.host_window_id),
            current_content: None,
            requested_content: None,
            accepted_content: None,
            outer_size: Some(outer_size),
            note: Some("missing host"),
        });
        return errors;
    };
    let requested = aspect_preserved_resize(
        host.content_size(),
        host.content_size_from_outer_size(outer_size),
        window.resize_aspect_ratio,
    );
    let now = Instant::now();
    window.pending_outer_resize_until = Some(next_deferred_outer_resize_deadline(
        window.pending_outer_resize,
        outer_size,
        window.pending_outer_resize_until,
        now,
    ));
    window.pending_outer_resize = Some(outer_size);
    host.preview_outer_resize(outer_size);
    trace_resize_event(EditorResizeTraceEvent {
        id: trace_id,
        source,
        stage: EditorResizeStage::Applied,
        target,
        window_id: Some(window.host_window_id),
        current_content: Some(host.content_size()),
        requested_content: Some(requested),
        accepted_content: None,
        outer_size: Some(outer_size),
        note: Some("deferred"),
    });
    errors
}

fn apply_deferred_outer_resize(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    outer_size: editor_host::Size,
) -> Vec<String> {
    let mut errors = Vec::new();
    let trace_id = next_resize_trace_id(trace_counter);
    let Some(host) = window.host.as_mut() else {
        trace_resize_event(EditorResizeTraceEvent {
            id: trace_id,
            source: EditorResizeSource::DeferredOuterResize,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: Some(window.host_window_id),
            current_content: None,
            requested_content: None,
            accepted_content: None,
            outer_size: Some(outer_size),
            note: Some("missing host"),
        });
        return errors;
    };
    let requested = aspect_preserved_resize(
        host.content_size(),
        host.content_size_from_outer_size(outer_size),
        window.resize_aspect_ratio,
    );
    trace_resize_event(EditorResizeTraceEvent {
        id: trace_id,
        source: EditorResizeSource::DeferredOuterResize,
        stage: EditorResizeStage::Begin,
        target,
        window_id: Some(window.host_window_id),
        current_content: Some(host.content_size()),
        requested_content: Some(requested),
        accepted_content: None,
        outer_size: Some(outer_size),
        note: None,
    });
    if same_host_size(requested, host.content_size()) {
        host.set_frame_content_size(host.content_size());
        trace_resize_event(EditorResizeTraceEvent {
            id: trace_id,
            source: EditorResizeSource::DeferredOuterResize,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: Some(window.host_window_id),
            current_content: Some(host.content_size()),
            requested_content: Some(requested),
            accepted_content: None,
            outer_size: Some(outer_size),
            note: Some("same size"),
        });
        return errors;
    }
    match negotiate_editor_content_resize(window.session.as_mut(), requested) {
        Ok(content_size) => {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::DeferredOuterResize,
                stage: EditorResizeStage::Accepted,
                target,
                window_id: Some(window.host_window_id),
                current_content: Some(host.content_size()),
                requested_content: Some(requested),
                accepted_content: Some(content_size),
                outer_size: Some(outer_size),
                note: None,
            });
            if same_host_size(content_size, requested)
                && !needs_programmatic_outer_resize(host, outer_size, content_size)
            {
                if let Err(error) = host.adopt_content_size_from_outer_resize(content_size) {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::DeferredOuterResize,
                        stage: EditorResizeStage::Error,
                        target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: Some(content_size),
                        outer_size: Some(outer_size),
                        note: Some("adopt failed"),
                    });
                    errors.push(error.to_string());
                } else {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::DeferredOuterResize,
                        stage: EditorResizeStage::Applied,
                        target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: Some(content_size),
                        outer_size: Some(outer_size),
                        note: Some("adopted"),
                    });
                }
            } else {
                record_programmatic_outer_resize(
                    &window.pending_programmatic_outer_resizes,
                    host,
                    content_size,
                );
                if let Err(error) = host.resize_content_from_top(content_size) {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::DeferredOuterResize,
                        stage: EditorResizeStage::Error,
                        target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: Some(content_size),
                        outer_size: Some(outer_size),
                        note: Some("host resize failed"),
                    });
                    errors.push(error.to_string());
                } else {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::DeferredOuterResize,
                        stage: EditorResizeStage::Applied,
                        target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: Some(content_size),
                        outer_size: Some(outer_size),
                        note: Some("plugin adjusted"),
                    });
                }
            }
        }
        Err(error) => {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::DeferredOuterResize,
                stage: EditorResizeStage::Error,
                target,
                window_id: Some(window.host_window_id),
                current_content: Some(host.content_size()),
                requested_content: Some(requested),
                accepted_content: None,
                outer_size: Some(outer_size),
                note: Some("session rejected"),
            });
            errors.push(error.to_string());
        }
    }
    errors
}

impl EditorWindowManager {
    fn next_resize_trace_id(&mut self) -> EditorResizeTraceId {
        next_resize_trace_id(&mut self.next_resize_trace_id)
    }

    pub(super) fn focus_existing(&mut self, target: EditorTarget) -> Option<window::Id> {
        if let Some(window) = self.windows.get(&target) {
            self.focused = Some(target);
            return Some(window.host_window_id);
        }
        if let Some((window_id, _)) = self
            .pending
            .iter()
            .find(|(_, window)| window.target == target)
        {
            self.focused = Some(target);
            return Some(*window_id);
        }
        None
    }

    pub(super) fn begin_open(
        &mut self,
        target: EditorTarget,
        title: String,
        resizable: bool,
        session: Box<dyn EditorSession>,
        window_id: window::Id,
    ) {
        self.pending.insert(
            window_id,
            PendingEditorWindow {
                target,
                title,
                resizable,
                host_window_id: window_id,
                session,
            },
        );
        self.focused = Some(target);
    }

    pub(super) fn attach(
        &mut self,
        window_id: window::Id,
        mut host: Option<InstalledHost>,
        parent: EditorParent,
    ) -> Result<(), EditorError> {
        let Some(mut pending) = self.pending.remove(&window_id) else {
            return Err(EditorError::HostUnavailable(format!(
                "pending editor window `{window_id:?}` is missing"
            )));
        };
        let resize_base_content_size = host
            .as_ref()
            .map(|host| Arc::new(SharedContentSize::new(host.content_size())));
        let startup_baseline_pending = host.as_ref().map(|_| Arc::new(AtomicBool::new(true)));
        let pending_programmatic_outer_resizes = Arc::new(ProgrammaticOuterResizeEchoes::new());
        if let Some(host) = host.as_ref() {
            pending
                .session
                .set_resize_handler(Some(Arc::new(HostEditorResizeHandler {
                    host: host.resize_handle(),
                    base_content_size: Arc::clone(
                        resize_base_content_size
                            .as_ref()
                            .expect("host creates resize base content size"),
                    ),
                    startup_baseline_pending: Arc::clone(
                        startup_baseline_pending
                            .as_ref()
                            .expect("host creates startup baseline flag"),
                    ),
                    programmatic_outer_resizes: Arc::clone(&pending_programmatic_outer_resizes),
                })))?;
        }
        pending.session.attach(parent)?;
        let tracks_native_content_resize = pending.session.tracks_native_content_resize();
        if let Some(host) = host.as_ref() {
            if tracks_native_content_resize {
                host.enable_native_content_resize_tracking()
                    .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
            } else {
                trace_editor_resize(|| {
                    format!(
                        "native content resize tracking disabled target={:?} window_id={window_id:?}",
                        pending.target
                    )
                });
            }
        }
        if let Ok(Some(resizable)) = pending.session.resizable() {
            pending.resizable = resizable;
            if let Some(host) = host.as_mut() {
                host.set_resizable(resizable)
                    .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
            }
        }
        let mut wait_for_embedded_startup_baseline = false;
        if let Some(host) = host.as_mut() {
            record_programmatic_outer_resize(
                &pending_programmatic_outer_resizes,
                host,
                host.content_size(),
            );
            let embedded_size = host
                .embedded_content_size()
                .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
            wait_for_embedded_startup_baseline = embedded_size.is_none();
            if !wait_for_embedded_startup_baseline
                && let Some(startup_baseline_pending) = startup_baseline_pending.as_ref()
            {
                startup_baseline_pending.store(false, Ordering::Relaxed);
            }
            let initial_size = pending.session.initial_size().unwrap_or(None);
            let content_size =
                attached_start_content_size(host.content_size(), embedded_size, initial_size);
            if !same_host_size(host.content_size(), content_size) {
                record_programmatic_outer_resize(
                    &pending_programmatic_outer_resizes,
                    host,
                    content_size,
                );
                host.resize_content(content_size)
                    .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
            }
            let baseline_content_size =
                attached_baseline_content_size(host.content_size(), embedded_size);
            if !same_host_size(host.content_size(), baseline_content_size) {
                record_programmatic_outer_resize(
                    &pending_programmatic_outer_resizes,
                    host,
                    baseline_content_size,
                );
                host.resize_content(baseline_content_size)
                    .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
            }
        }
        let content_size = host.as_ref().map_or(
            editor_host::Size {
                width: 1.0,
                height: 1.0,
            },
            InstalledHost::content_size,
        );
        if let Some(base_content_size) = resize_base_content_size.as_ref() {
            base_content_size.store(content_size);
        }
        if let Some(host) = host.as_mut() {
            host.set_zoom_percent(100);
        }
        self.focused = Some(pending.target);
        self.windows.insert(
            pending.target,
            EditorWindow {
                title: pending.title,
                resizable: pending.resizable,
                host_window_id: pending.host_window_id,
                host,
                session: pending.session,
                visible: true,
                tracks_native_content_resize,
                base_content_size: content_size,
                base_content_size_shared: resize_base_content_size,
                startup_baseline_pending: startup_baseline_pending.filter(|pending| {
                    wait_for_embedded_startup_baseline && pending.load(Ordering::Relaxed)
                }),
                pending_programmatic_outer_resizes,
                pending_outer_resize: None,
                pending_outer_resize_until: None,
                pending_zoom_percent: None,
                pending_zoom_percent_until: None,
                resize_aspect_ratio: content_size.width / content_size.height.max(1.0),
            },
        );
        self.windows_by_id.insert(window_id, pending.target);
        Ok(())
    }

    pub(super) fn pending_contains(&self, window_id: window::Id) -> bool {
        self.pending.contains_key(&window_id)
    }

    pub(super) fn window_title(&self, window_id: window::Id) -> Option<&str> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.title.as_str()))
            .or_else(|| {
                self.pending
                    .get(&window_id)
                    .map(|window| window.title.as_str())
            })
    }

    pub(super) fn window_resizable(&self, window_id: window::Id) -> Option<bool> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.resizable))
            .or_else(|| self.pending.get(&window_id).map(|window| window.resizable))
    }

    pub(super) fn target_for_window(&self, window_id: window::Id) -> Option<EditorTarget> {
        self.windows_by_id.get(&window_id).copied()
    }

    pub(super) fn window_for_target(&self, target: EditorTarget) -> Option<window::Id> {
        self.windows
            .get(&target)
            .map(|window| window.host_window_id)
            .or_else(|| {
                self.pending.iter().find_map(|(window_id, pending)| {
                    (pending.target == target).then_some(*window_id)
                })
            })
    }

    pub(super) fn focus_window(&mut self, window_id: window::Id) -> Vec<String> {
        let Some(target) = self.windows_by_id.get(&window_id).copied() else {
            return Vec::new();
        };
        let Some(window) = self.windows.get_mut(&target) else {
            return Vec::new();
        };
        self.focused = Some(target);
        let mut errors = Vec::new();
        if let Some(host) = window.host.as_mut()
            && let Err(error) = host.raise()
        {
            errors.push(error.to_string());
        }
        errors
    }

    pub(super) fn remove_window(&mut self, window_id: window::Id) -> Option<RemovedEditorWindow> {
        if let Some(pending) = self.pending.remove(&window_id) {
            if self.focused == Some(pending.target) {
                self.focused = None;
            }
            return Some(RemovedEditorWindow {
                window_id: pending.host_window_id,
                host: None,
                session: pending.session,
            });
        }

        let target = self.windows_by_id.remove(&window_id)?;
        let window = self.windows.remove(&target)?;
        if self.focused == Some(target) {
            self.focused = None;
        }
        Some(RemovedEditorWindow {
            window_id: window.host_window_id,
            host: window.host,
            session: window.session,
        })
    }

    pub(super) fn remove_target(&mut self, target: EditorTarget) -> Option<RemovedEditorWindow> {
        if let Some(window) = self.windows.remove(&target) {
            self.windows_by_id.remove(&window.host_window_id);
            if self.focused == Some(target) {
                self.focused = None;
            }
            return Some(RemovedEditorWindow {
                window_id: window.host_window_id,
                host: window.host,
                session: window.session,
            });
        }
        let window_id = self
            .pending
            .iter()
            .find_map(|(window_id, pending)| (pending.target == target).then_some(*window_id))?;
        let pending = self.pending.remove(&window_id)?;
        if self.focused == Some(target) {
            self.focused = None;
        }
        Some(RemovedEditorWindow {
            window_id: pending.host_window_id,
            host: None,
            session: pending.session,
        })
    }

    pub(super) fn shift_targets_after_removed_strip(&mut self, removed_strip_index: usize) {
        let targets_to_shift = self
            .windows
            .keys()
            .copied()
            .filter(|target| target.strip_index > removed_strip_index)
            .collect::<Vec<_>>();
        for target in targets_to_shift {
            if let Some(window) = self.windows.remove(&target) {
                let shifted = EditorTarget {
                    strip_index: target.strip_index - 1,
                    slot_index: target.slot_index,
                };
                self.windows_by_id.insert(window.host_window_id, shifted);
                self.windows.insert(shifted, window);
            }
        }

        for pending in self.pending.values_mut() {
            if pending.target.strip_index > removed_strip_index {
                pending.target.strip_index -= 1;
            }
        }

        if let Some(target) = self.focused
            && target.strip_index > removed_strip_index
        {
            self.focused = Some(EditorTarget {
                strip_index: target.strip_index - 1,
                slot_index: target.slot_index,
            });
        }
    }

    pub(super) fn move_slot_targets_within_strip(
        &mut self,
        strip_index: usize,
        from_slot_index: usize,
        to_slot_index: usize,
    ) {
        if from_slot_index == to_slot_index {
            return;
        }

        let shift = |target: EditorTarget| -> EditorTarget {
            if target.strip_index != strip_index {
                return target;
            }
            let slot_index = if target.slot_index == from_slot_index {
                to_slot_index
            } else if from_slot_index < to_slot_index
                && target.slot_index > from_slot_index
                && target.slot_index <= to_slot_index
            {
                target.slot_index - 1
            } else if from_slot_index > to_slot_index
                && target.slot_index >= to_slot_index
                && target.slot_index < from_slot_index
            {
                target.slot_index + 1
            } else {
                target.slot_index
            };
            EditorTarget {
                slot_index,
                ..target
            }
        };

        let moved_windows = self.windows.drain().collect::<Vec<_>>();
        self.windows_by_id.clear();
        for (target, window) in moved_windows {
            let moved = shift(target);
            self.windows_by_id.insert(window.host_window_id, moved);
            self.windows.insert(moved, window);
        }

        for pending in self.pending.values_mut() {
            pending.target = shift(pending.target);
        }

        if let Some(target) = self.focused {
            self.focused = Some(shift(target));
        }
    }

    pub(super) fn remove_all_windows(&mut self) -> Vec<RemovedEditorWindow> {
        let windows = self
            .windows
            .drain()
            .map(|(target, window)| {
                if self.focused == Some(target) {
                    self.focused = None;
                }
                RemovedEditorWindow {
                    window_id: window.host_window_id,
                    host: window.host,
                    session: window.session,
                }
            })
            .collect::<Vec<_>>();
        self.windows_by_id.clear();
        self.pending.clear();
        windows
    }

    pub(super) fn hide_window(
        &mut self,
        window_id: window::Id,
    ) -> Option<(EditorTarget, Vec<String>)> {
        let target = *self.windows_by_id.get(&window_id)?;
        let window = self.windows.get_mut(&target)?;
        let mut errors = Vec::new();
        if self.focused == Some(target) {
            self.focused = None;
        }
        window.visible = false;
        if let Err(error) = window.session.set_visible(false) {
            errors.push(error.to_string());
        }
        if let Some(host) = window.host.as_mut()
            && let Err(error) = host.set_visible(false)
        {
            errors.push(error.to_string());
        } else if let Some(host) = window.host.as_ref() {
            host.clear_close_requested();
        }
        Some((target, errors))
    }

    pub(super) fn hide_all_windows(&mut self) -> Vec<Vec<String>> {
        self.focused = None;
        self.windows
            .values_mut()
            .map(|window| {
                let mut errors = Vec::new();
                window.visible = false;
                if let Err(error) = window.session.set_visible(false) {
                    errors.push(error.to_string());
                }
                if let Some(host) = window.host.as_mut()
                    && let Err(error) = host.set_visible(false)
                {
                    errors.push(error.to_string());
                }
                errors
            })
            .collect()
    }

    pub(super) fn show_window(&mut self, window_id: window::Id) -> Vec<String> {
        let Some(target) = self.windows_by_id.get(&window_id).copied() else {
            return Vec::new();
        };
        let Some(window) = self.windows.get_mut(&target) else {
            return Vec::new();
        };
        let mut errors = Vec::new();
        if let Some(host) = window.host.as_mut() {
            host.clear_close_requested();
            if let Err(error) = host.set_visible(true) {
                errors.push(error.to_string());
            }
        }
        if let Err(error) = window.session.set_visible(true) {
            errors.push(error.to_string());
        }
        window.visible = true;
        self.focused = Some(target);
        errors
    }

    pub(super) fn window_visible(&self, window_id: window::Id) -> bool {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target))
            .is_some_and(|window| window.visible)
    }

    pub(super) fn targets_for_strip(&self, strip_index: usize) -> Vec<EditorTarget> {
        self.windows
            .keys()
            .chain(self.pending.values().map(|window| &window.target))
            .filter(|target| target.strip_index == strip_index)
            .copied()
            .collect()
    }

    pub(super) fn set_window_title(&mut self, target: EditorTarget, title: String) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(window) = self.windows.get_mut(&target) {
            window.title.clone_from(&title);
            if let Some(host) = window.host.as_mut()
                && let Err(error) = host.set_title(title.clone())
            {
                errors.push(error.to_string());
            }
        }
        for pending in self
            .pending
            .values_mut()
            .filter(|pending| pending.target == target)
        {
            pending.title.clone_from(&title);
        }
        errors
    }

    pub(super) fn set_preset_state(
        &mut self,
        target: EditorTarget,
        state: Option<EditorPresetState>,
    ) {
        if let Some(window) = self.windows.get_mut(&target)
            && let Some(host) = window.host.as_mut()
        {
            host.set_preset_state(state);
            record_programmatic_outer_resize(
                &window.pending_programmatic_outer_resizes,
                host,
                host.content_size(),
            );
        }
    }

    pub(super) fn preset_state(&self, target: EditorTarget) -> Option<EditorPresetState> {
        self.windows
            .get(&target)
            .and_then(|window| window.host.as_ref())
            .and_then(InstalledHost::preset_state)
    }

    pub(super) fn drain_frame_commands(&mut self) -> Vec<(EditorTarget, EditorFrameCommand)> {
        let mut commands = Vec::new();
        for (target, window) in &mut self.windows {
            let Some(host) = window.host.as_mut() else {
                continue;
            };
            commands.extend(
                host.drain_frame_commands()
                    .into_iter()
                    .map(|command| (*target, command)),
            );
        }
        commands
    }

    pub(super) fn apply_requested_content_resizes(&mut self, mut on_error: impl FnMut(String)) {
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            match window.session.requested_size() {
                Ok(Some(size)) => {
                    let trace_id = next_resize_trace_id(trace_counter);
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::SessionRequestedSize,
                        stage: EditorResizeStage::Begin,
                        target: *target,
                        window_id: Some(window.host_window_id),
                        current_content: window.host.as_ref().map(InstalledHost::content_size),
                        requested_content: Some(host_size_from_editor_size(size)),
                        accepted_content: None,
                        outer_size: None,
                        note: None,
                    });
                    if let Some(host) = window.host.as_mut()
                        && let Some(content_size) = editor_content_resize_request(size, host)
                    {
                        record_programmatic_outer_resize(
                            &window.pending_programmatic_outer_resizes,
                            host,
                            content_size,
                        );
                        if let Err(error) = host.resize_content_from_top(content_size) {
                            trace_resize_event(EditorResizeTraceEvent {
                                id: trace_id,
                                source: EditorResizeSource::SessionRequestedSize,
                                stage: EditorResizeStage::Error,
                                target: *target,
                                window_id: Some(window.host_window_id),
                                current_content: Some(host.content_size()),
                                requested_content: Some(content_size),
                                accepted_content: None,
                                outer_size: None,
                                note: Some("host resize failed"),
                            });
                            on_error(error.to_string());
                        } else {
                            trace_resize_event(EditorResizeTraceEvent {
                                id: trace_id,
                                source: EditorResizeSource::SessionRequestedSize,
                                stage: EditorResizeStage::Applied,
                                target: *target,
                                window_id: Some(window.host_window_id),
                                current_content: Some(host.content_size()),
                                requested_content: Some(content_size),
                                accepted_content: Some(content_size),
                                outer_size: None,
                                note: None,
                            });
                        }
                    } else if let Some(host) = window.host.as_ref() {
                        trace_resize_event(EditorResizeTraceEvent {
                            id: trace_id,
                            source: EditorResizeSource::SessionRequestedSize,
                            stage: EditorResizeStage::Ignored,
                            target: *target,
                            window_id: Some(window.host_window_id),
                            current_content: Some(host.content_size()),
                            requested_content: Some(host_size_from_editor_size(size)),
                            accepted_content: None,
                            outer_size: None,
                            note: Some("same size"),
                        });
                    } else {
                        trace_resize_event(EditorResizeTraceEvent {
                            id: trace_id,
                            source: EditorResizeSource::SessionRequestedSize,
                            stage: EditorResizeStage::Ignored,
                            target: *target,
                            window_id: Some(window.host_window_id),
                            current_content: None,
                            requested_content: Some(host_size_from_editor_size(size)),
                            accepted_content: None,
                            outer_size: None,
                            note: Some("missing host"),
                        });
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    on_error(error.to_string());
                }
            };
        }
    }

    pub(super) fn sync_native_content_resizes(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            if !should_sync_native_content_resize(
                window.visible,
                window.tracks_native_content_resize,
            ) {
                continue;
            }
            let (embedded_content_size, native_content_size, native_outer_size, current_content) = {
                let Some(host) = window.host.as_mut() else {
                    continue;
                };
                let embedded_content_size = match host.embedded_content_size() {
                    Ok(size) => size,
                    Err(error) => {
                        errors.push(error.to_string());
                        continue;
                    }
                };
                let native_host_content_size = match host.native_content_size() {
                    Ok(size) => size,
                    Err(error) => {
                        errors.push(error.to_string());
                        continue;
                    }
                };
                let native_content_size = match observed_native_content_size(
                    embedded_content_size,
                    native_host_content_size,
                ) {
                    Some(size) => size,
                    None => continue,
                };
                (
                    embedded_content_size,
                    native_content_size,
                    host.outer_size_from_content_size(native_content_size),
                    host.content_size(),
                )
            };
            let requested = native_content_resize_request(native_content_size);
            if let Some(startup_baseline_pending) = window.startup_baseline_pending.clone()
                && let Some(startup_baseline) = startup_embedded_baseline_size(
                    startup_baseline_pending.load(Ordering::Relaxed),
                    embedded_content_size,
                )
            {
                startup_baseline_pending.store(false, Ordering::Relaxed);
                window.startup_baseline_pending = None;
                set_editor_resize_baseline(window, startup_baseline);
                let Some(host) = window.host.as_mut() else {
                    continue;
                };
                if same_host_size(startup_baseline, current_content) {
                    host.set_zoom_percent(100);
                    continue;
                }
                let trace_id = next_resize_trace_id(trace_counter);
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::NativeContentSize,
                    stage: EditorResizeStage::Begin,
                    target: *target,
                    window_id: Some(window.host_window_id),
                    current_content: Some(current_content),
                    requested_content: Some(startup_baseline),
                    accepted_content: None,
                    outer_size: Some(native_outer_size),
                    note: Some("startup embedded baseline"),
                });
                record_programmatic_outer_resize(
                    &window.pending_programmatic_outer_resizes,
                    host,
                    startup_baseline,
                );
                match host.resize_content_from_top(startup_baseline) {
                    Ok(()) => {
                        host.set_zoom_percent(100);
                        trace_resize_event(EditorResizeTraceEvent {
                            id: trace_id,
                            source: EditorResizeSource::NativeContentSize,
                            stage: EditorResizeStage::Applied,
                            target: *target,
                            window_id: Some(window.host_window_id),
                            current_content: Some(host.content_size()),
                            requested_content: Some(startup_baseline),
                            accepted_content: Some(startup_baseline),
                            outer_size: Some(native_outer_size),
                            note: Some("startup baseline adopted"),
                        });
                    }
                    Err(error) => {
                        trace_resize_event(EditorResizeTraceEvent {
                            id: trace_id,
                            source: EditorResizeSource::NativeContentSize,
                            stage: EditorResizeStage::Error,
                            target: *target,
                            window_id: Some(window.host_window_id),
                            current_content: Some(host.content_size()),
                            requested_content: Some(startup_baseline),
                            accepted_content: None,
                            outer_size: Some(native_outer_size),
                            note: Some("host resize failed"),
                        });
                        errors.push(error.to_string());
                    }
                }
                continue;
            }
            if same_host_size(native_content_size, current_content) {
                defer_zoom_percent(
                    window,
                    zoom_percent_for_content_size(window.base_content_size, native_content_size),
                    Instant::now(),
                );
                continue;
            }
            if consume_pending_programmatic_outer_resize(
                &window.pending_programmatic_outer_resizes,
                native_outer_size,
            ) {
                let trace_id = next_resize_trace_id(trace_counter);
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::NativeContentSize,
                    stage: EditorResizeStage::Ignored,
                    target: *target,
                    window_id: Some(window.host_window_id),
                    current_content: Some(current_content),
                    requested_content: Some(native_content_size),
                    accepted_content: None,
                    outer_size: Some(native_outer_size),
                    note: Some("programmatic echo"),
                });
                continue;
            }
            let trace_id = next_resize_trace_id(trace_counter);
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::NativeContentSize,
                stage: EditorResizeStage::Begin,
                target: *target,
                window_id: Some(window.host_window_id),
                current_content: Some(current_content),
                requested_content: Some(requested),
                accepted_content: None,
                outer_size: Some(native_outer_size),
                note: Some("embedded view changed"),
            });
            let Some(host) = window.host.as_mut() else {
                continue;
            };
            record_programmatic_outer_resize(
                &window.pending_programmatic_outer_resizes,
                host,
                requested,
            );
            let applied = match host.resize_content_from_top(requested) {
                Ok(()) => {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::NativeContentSize,
                        stage: EditorResizeStage::Applied,
                        target: *target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: Some(requested),
                        outer_size: Some(native_outer_size),
                        note: Some("adopted embedded view size"),
                    });
                    true
                }
                Err(error) => {
                    trace_resize_event(EditorResizeTraceEvent {
                        id: trace_id,
                        source: EditorResizeSource::NativeContentSize,
                        stage: EditorResizeStage::Error,
                        target: *target,
                        window_id: Some(window.host_window_id),
                        current_content: Some(host.content_size()),
                        requested_content: Some(requested),
                        accepted_content: None,
                        outer_size: Some(native_outer_size),
                        note: Some("host resize failed"),
                    });
                    errors.push(error.to_string());
                    false
                }
            };
            if applied {
                defer_zoom_percent(
                    window,
                    zoom_percent_for_content_size(window.base_content_size, requested),
                    Instant::now(),
                );
            }
        }
        errors
    }

    pub(super) fn resize_window_outer(
        &mut self,
        window_id: window::Id,
        outer_size: editor_host::Size,
    ) -> Vec<String> {
        let Some(target) = self.windows_by_id.get(&window_id).copied() else {
            return Vec::new();
        };
        let trace_id = self.next_resize_trace_id();
        let Some(window) = self.windows.get_mut(&target) else {
            return Vec::new();
        };
        if !window.resizable {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::IcedOuterEvent,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window.host_window_id),
                current_content: window.host.as_ref().map(InstalledHost::content_size),
                requested_content: None,
                accepted_content: None,
                outer_size: Some(outer_size),
                note: Some("fixed size"),
            });
            return Vec::new();
        }
        if window.host.is_none() {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::IcedOuterEvent,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window.host_window_id),
                current_content: None,
                requested_content: None,
                accepted_content: None,
                outer_size: Some(outer_size),
                note: Some("missing host"),
            });
            return Vec::new();
        }

        let mut errors = Vec::new();

        if consume_pending_programmatic_outer_resize(
            &window.pending_programmatic_outer_resizes,
            outer_size,
        ) {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::IcedOuterEvent,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window_id),
                current_content: window.host.as_ref().map(InstalledHost::content_size),
                requested_content: None,
                accepted_content: None,
                outer_size: Some(outer_size),
                note: Some("programmatic echo"),
            });
            return errors;
        }

        let Some(host) = window.host.as_ref() else {
            return errors;
        };
        match host.is_live_resizing() {
            Ok(true) => {}
            Ok(false) => {
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::IcedOuterEvent,
                    stage: EditorResizeStage::Ignored,
                    target,
                    window_id: Some(window_id),
                    current_content: Some(host.content_size()),
                    requested_content: Some(host.content_size_from_outer_size(outer_size)),
                    accepted_content: None,
                    outer_size: Some(outer_size),
                    note: Some("non-live outer echo"),
                });
                return errors;
            }
            Err(error) => {
                errors.push(error.to_string());
                return errors;
            }
        }

        errors.extend(defer_outer_resize(
            trace_id,
            EditorResizeSource::IcedOuterEvent,
            target,
            window,
            outer_size,
        ));
        errors
    }

    pub(super) fn set_zoom_percent(&mut self, target: EditorTarget, percent: u32) -> Vec<String> {
        let trace_id = self.next_resize_trace_id();
        let Some(window) = self.windows.get_mut(&target) else {
            return Vec::new();
        };
        window.pending_zoom_percent = None;
        window.pending_zoom_percent_until = None;
        let requested = zoomed_content_size(window.base_content_size, percent);
        let mut errors = Vec::new();
        if !window.resizable {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::HeaderZoom,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window.host_window_id),
                current_content: window.host.as_ref().map(InstalledHost::content_size),
                requested_content: Some(requested),
                accepted_content: None,
                outer_size: None,
                note: Some("fixed size"),
            });
            return errors;
        }

        let Some(host) = window.host.as_mut() else {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::HeaderZoom,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window.host_window_id),
                current_content: None,
                requested_content: Some(requested),
                accepted_content: None,
                outer_size: None,
                note: Some("missing host"),
            });
            return errors;
        };

        let current = host.content_size();
        if same_host_size(current, requested) {
            trace_resize_event(EditorResizeTraceEvent {
                id: trace_id,
                source: EditorResizeSource::HeaderZoom,
                stage: EditorResizeStage::Ignored,
                target,
                window_id: Some(window.host_window_id),
                current_content: Some(current),
                requested_content: Some(requested),
                accepted_content: None,
                outer_size: None,
                note: Some("same size"),
            });
            return errors;
        }

        trace_resize_event(EditorResizeTraceEvent {
            id: trace_id,
            source: EditorResizeSource::HeaderZoom,
            stage: EditorResizeStage::Begin,
            target,
            window_id: Some(window.host_window_id),
            current_content: Some(current),
            requested_content: Some(requested),
            accepted_content: None,
            outer_size: None,
            note: None,
        });

        let accepted = match negotiate_editor_content_resize(window.session.as_mut(), requested) {
            Ok(accepted) => accepted,
            Err(error) => {
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::HeaderZoom,
                    stage: EditorResizeStage::Error,
                    target,
                    window_id: Some(window.host_window_id),
                    current_content: Some(current),
                    requested_content: Some(requested),
                    accepted_content: None,
                    outer_size: None,
                    note: Some("session resize failed"),
                });
                errors.push(error.to_string());
                return errors;
            }
        };

        record_programmatic_outer_resize(
            &window.pending_programmatic_outer_resizes,
            host,
            accepted,
        );
        match host.resize_content_from_top(accepted) {
            Ok(()) => {
                host.set_zoom_percent(zoom_percent_for_content_size(
                    window.base_content_size,
                    accepted,
                ));
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::HeaderZoom,
                    stage: EditorResizeStage::Applied,
                    target,
                    window_id: Some(window.host_window_id),
                    current_content: Some(host.content_size()),
                    requested_content: Some(requested),
                    accepted_content: Some(accepted),
                    outer_size: None,
                    note: None,
                });
            }
            Err(error) => {
                trace_resize_event(EditorResizeTraceEvent {
                    id: trace_id,
                    source: EditorResizeSource::HeaderZoom,
                    stage: EditorResizeStage::Error,
                    target,
                    window_id: Some(window.host_window_id),
                    current_content: Some(host.content_size()),
                    requested_content: Some(requested),
                    accepted_content: Some(accepted),
                    outer_size: None,
                    note: Some("host resize failed"),
                });
                errors.push(error.to_string());
            }
        }
        errors
    }

    pub(super) fn expire_deferred_outer_resizes(&mut self, now: Instant) -> Vec<String> {
        let mut errors = Vec::new();
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            let outer_due = window
                .pending_outer_resize_until
                .is_some_and(|deadline| deadline <= now);
            let zoom_due = window
                .pending_zoom_percent_until
                .is_some_and(|deadline| deadline <= now);
            if !outer_due && !zoom_due {
                continue;
            }
            if let Some(host) = window.host.as_mut() {
                match host.is_live_resizing() {
                    Ok(true) => {
                        if outer_due {
                            window.pending_outer_resize_until = Some(now + RESIZE_IDLE_TIMEOUT);
                        }
                        if zoom_due {
                            window.pending_zoom_percent_until = Some(now + RESIZE_IDLE_TIMEOUT);
                        }
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => errors.push(error.to_string()),
                }
            }
            if outer_due {
                window.pending_outer_resize_until = None;
            }
            if outer_due && let Some(outer_size) = window.pending_outer_resize.take() {
                errors.extend(apply_deferred_outer_resize(
                    trace_counter,
                    *target,
                    window,
                    outer_size,
                ));
            }
            if zoom_due {
                window.pending_zoom_percent_until = None;
                if let Some(zoom_percent) = window.pending_zoom_percent.take()
                    && let Some(host) = window.host.as_mut()
                {
                    host.set_zoom_percent(zoom_percent);
                }
            }
        }
        errors
    }

    pub(super) fn close_requested_windows(&self) -> Vec<window::Id> {
        self.windows
            .values()
            .filter(|window| {
                window
                    .host
                    .as_ref()
                    .is_some_and(InstalledHost::close_requested)
            })
            .map(|window| window.host_window_id)
            .collect()
    }

    pub(super) fn has_installed_hosts(&self) -> bool {
        self.windows.values().any(|window| window.host.is_some())
    }
}

fn trace_editor_resize(message: impl FnOnce() -> String) {
    log::trace!(
        target: "lilypalooza::editor_windows",
        "thread={:?} {}",
        std::thread::current().id(),
        message()
    );
}

#[cfg(test)]
impl EditorWindowManager {
    pub(super) fn contains_window(&self, target: EditorTarget) -> bool {
        self.windows.contains_key(&target)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ptr::NonNull;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use editor_host::WindowSnapshot;
    use iced::window;
    use lilypalooza_audio::{EditorError, EditorParent, EditorSession, EditorSize};

    use super::{EditorTarget, EditorWindowManager, snapshot_into_editor_parent};

    struct FakeEditorSession;
    struct RequestedSizeEditorSession {
        calls: Arc<AtomicUsize>,
    }
    struct ReportingResizableEditorSession {
        resizable: bool,
    }
    struct AdjustingResizeEditorSession {
        requested: Arc<std::sync::Mutex<Vec<EditorSize>>>,
        accepted: EditorSize,
    }
    struct RecordingDetachSession {
        events: Rc<RefCell<Vec<&'static str>>>,
    }
    struct RecordingHost {
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Drop for RecordingHost {
        fn drop(&mut self) {
            self.events.borrow_mut().push("host-drop");
        }
    }

    impl EditorSession for FakeEditorSession {
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

    impl EditorSession for RecordingDetachSession {
        fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
            Ok(())
        }

        fn detach(&mut self) -> Result<(), EditorError> {
            self.events.borrow_mut().push("session-detach");
            Ok(())
        }

        fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
            Ok(())
        }

        fn resize(&mut self, size: EditorSize) -> Result<EditorSize, EditorError> {
            Ok(size)
        }
    }

    impl EditorSession for RequestedSizeEditorSession {
        fn requested_size(&mut self) -> Result<Option<EditorSize>, EditorError> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Ok(Some(EditorSize {
                width: 640,
                height: 480,
            }))
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

    impl EditorSession for ReportingResizableEditorSession {
        fn resizable(&mut self) -> Result<Option<bool>, EditorError> {
            Ok(Some(self.resizable))
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

    impl EditorSession for AdjustingResizeEditorSession {
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
            self.requested
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(size);
            Ok(self.accepted)
        }
    }

    #[test]
    fn processor_editor_window_manager_reuses_existing_target_window() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 3,
            slot_index: 0,
        };

        let first_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 4".to_string(),
            true,
            Box::new(FakeEditorSession),
            first_id,
        );
        let second_token = manager.focus_existing(target);

        assert_eq!(Some(first_id), second_token);
        assert_eq!(manager.focused, Some(target));
    }

    #[test]
    fn processor_editor_window_manager_attaches_pending_session_once_parent_arrives() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };

        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );

        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");
        assert!(manager.windows.contains_key(&target));
        assert!(manager.window_visible(window_id));
    }

    #[test]
    fn processor_editor_window_manager_tracks_visibility_for_toggle() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );
        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");

        assert!(manager.window_visible(window_id));
        manager.hide_window(window_id).expect("window should hide");
        assert!(!manager.window_visible(window_id));
        assert!(manager.show_window(window_id).is_empty());
        assert!(manager.window_visible(window_id));
    }

    #[test]
    fn processor_editor_window_manager_uses_live_resizable_after_attach() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 1".to_string(),
            false,
            Box::new(ReportingResizableEditorSession { resizable: true }),
            window_id,
        );
        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");

        assert_eq!(manager.window_resizable(window_id), Some(true));
    }

    #[test]
    fn processor_editor_window_manager_polls_requested_editor_resize() {
        let mut manager = EditorWindowManager::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let window_id = window::Id::unique();
        manager.begin_open(
            EditorTarget {
                strip_index: 1,
                slot_index: 0,
            },
            "Track 1".to_string(),
            true,
            Box::new(RequestedSizeEditorSession {
                calls: Arc::clone(&calls),
            }),
            window_id,
        );
        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");

        let mut errors = Vec::new();
        manager.apply_requested_content_resizes(|error| errors.push(error));

        assert!(errors.is_empty());
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }

    #[test]
    fn processor_editor_resize_negotiation_uses_session_accepted_size() {
        let requested = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut session = AdjustingResizeEditorSession {
            requested: Arc::clone(&requested),
            accepted: EditorSize {
                width: 512,
                height: 384,
            },
        };

        let accepted = super::negotiate_editor_content_resize(
            &mut session,
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
        )
        .expect("resize should be accepted");

        assert_eq!(
            *requested
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            vec![EditorSize {
                width: 640,
                height: 480,
            }]
        );
        assert_eq!(
            accepted,
            editor_host::Size {
                width: 512.0,
                height: 384.0,
            }
        );
    }

    #[test]
    fn startup_programmatic_outer_resize_echoes_are_consumed() {
        let pending = Arc::new(super::ProgrammaticOuterResizeEchoes::new());
        super::record_programmatic_outer_resize_size(
            &pending,
            editor_host::Size {
                width: 644.0,
                height: 518.0,
            },
        );
        super::record_programmatic_outer_resize_size(
            &pending,
            editor_host::Size {
                width: 404.0,
                height: 438.0,
            },
        );

        assert!(super::consume_pending_programmatic_outer_resize(
            &pending,
            editor_host::Size {
                width: 644.25,
                height: 517.75,
            },
        ));
        assert!(super::consume_pending_programmatic_outer_resize(
            &pending,
            editor_host::Size {
                width: 404.25,
                height: 437.75,
            },
        ));
        assert!(!super::consume_pending_programmatic_outer_resize(
            &pending,
            editor_host::Size {
                width: 644.0,
                height: 749.0,
            },
        ));
    }

    #[test]
    fn programmatic_outer_resize_echoes_do_not_return_torn_sizes() {
        let pending = Arc::new(super::ProgrammaticOuterResizeEchoes::new());
        let mut writers = Vec::new();

        for writer in 0..4 {
            let pending = Arc::clone(&pending);
            writers.push(std::thread::spawn(move || {
                for index in 0..2_000 {
                    let width = f64::from(writer * 10_000 + index);
                    super::record_programmatic_outer_resize_size(
                        &pending,
                        editor_host::Size {
                            width,
                            height: width + 34.0,
                        },
                    );
                }
            }));
        }

        for writer in writers {
            writer.join().expect("writer should not panic");
        }

        for width in 0..42_000 {
            let width = f64::from(width);
            assert!(!super::consume_pending_programmatic_outer_resize(
                &pending,
                editor_host::Size {
                    width,
                    height: width + 33.0,
                },
            ));
        }
    }

    #[test]
    fn same_pending_outer_resize_keeps_existing_deferred_deadline() {
        let now = Instant::now();
        let previous_deadline = now + Duration::from_secs(1);
        let outer_size = editor_host::Size {
            width: 414.0,
            height: 395.0,
        };

        assert_eq!(
            super::next_deferred_outer_resize_deadline(
                Some(outer_size),
                outer_size,
                Some(previous_deadline),
                now,
            ),
            previous_deadline
        );
    }

    #[test]
    fn changed_pending_outer_resize_refreshes_deferred_deadline() {
        let now = Instant::now();
        let previous_deadline = now + Duration::from_secs(1);

        assert_eq!(
            super::next_deferred_outer_resize_deadline(
                Some(editor_host::Size {
                    width: 414.0,
                    height: 395.0,
                }),
                editor_host::Size {
                    width: 500.0,
                    height: 500.0,
                },
                Some(previous_deadline),
                now,
            ),
            now + super::RESIZE_IDLE_TIMEOUT
        );
    }

    #[test]
    fn same_pending_zoom_keeps_existing_deferred_deadline() {
        let now = Instant::now();
        let previous_deadline = now + Duration::from_secs(1);

        assert_eq!(
            super::next_deferred_zoom_deadline(Some(137), 137, Some(previous_deadline), now),
            previous_deadline
        );
    }

    #[test]
    fn changed_pending_zoom_refreshes_deferred_deadline() {
        let now = Instant::now();
        let previous_deadline = now + Duration::from_secs(1);

        assert_eq!(
            super::next_deferred_zoom_deadline(Some(137), 138, Some(previous_deadline), now),
            now + super::RESIZE_IDLE_TIMEOUT
        );
    }

    #[test]
    fn same_host_size_treats_subpixel_resize_as_noop() {
        assert!(super::same_host_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            editor_host::Size {
                width: 640.25,
                height: 479.75,
            },
        ));
        assert!(!super::same_host_size(
            editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            editor_host::Size {
                width: 641.0,
                height: 480.0,
            },
        ));
    }

    #[test]
    fn aspect_preserved_resize_uses_dominant_drag_axis() {
        let current = editor_host::Size {
            width: 640.0,
            height: 480.0,
        };

        assert_eq!(
            super::aspect_preserved_resize(
                current,
                editor_host::Size {
                    width: 640.0,
                    height: 540.0,
                },
                640.0 / 480.0,
            ),
            editor_host::Size {
                width: 720.0,
                height: 540.0,
            }
        );
        assert_eq!(
            super::aspect_preserved_resize(
                current,
                editor_host::Size {
                    width: 720.0,
                    height: 480.0,
                },
                640.0 / 480.0,
            ),
            editor_host::Size {
                width: 720.0,
                height: 540.0,
            }
        );
    }

    #[test]
    fn editor_zoom_size_uses_default_content_size() {
        assert_eq!(
            super::zoomed_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                },
                150,
            ),
            editor_host::Size {
                width: 960.0,
                height: 720.0,
            }
        );
    }

    #[test]
    fn editor_zoom_size_allows_minimum_scale() {
        assert_eq!(
            super::zoomed_content_size(
                editor_host::Size {
                    width: 400.0,
                    height: 300.0,
                },
                50,
            ),
            editor_host::Size {
                width: 200.0,
                height: 150.0,
            }
        );
    }

    #[test]
    fn plugin_owned_resize_uses_attached_editor_size_as_zoom_baseline() {
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
            ),
            100
        );
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
                editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                },
            ),
            160
        );
    }

    #[test]
    fn startup_baseline_uses_actual_embedded_plugin_size_after_attach() {
        assert_eq!(
            super::attached_start_content_size(
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
                Some(editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                }),
                Some(EditorSize {
                    width: 400,
                    height: 400,
                }),
            ),
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            }
        );
        assert_eq!(
            super::attached_baseline_content_size(
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
                Some(editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                }),
            ),
            editor_host::Size {
                width: 640.0,
                height: 640.0,
            }
        );
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                },
                editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                },
            ),
            100
        );
    }

    #[test]
    fn startup_size_falls_back_to_session_initial_size_without_embedded_view() {
        assert_eq!(
            super::attached_start_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                },
                None,
                Some(EditorSize {
                    width: 400,
                    height: 300,
                }),
            ),
            editor_host::Size {
                width: 400.0,
                height: 300.0,
            }
        );
    }

    #[test]
    fn first_embedded_size_after_attach_becomes_startup_baseline() {
        assert_eq!(
            super::startup_embedded_baseline_size(
                true,
                Some(editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                }),
            ),
            Some(editor_host::Size {
                width: 640.0,
                height: 640.0,
            })
        );
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                },
                editor_host::Size {
                    width: 640.0,
                    height: 640.0,
                },
            ),
            100
        );
    }

    #[test]
    fn startup_baseline_waits_for_real_embedded_size() {
        assert_eq!(super::startup_embedded_baseline_size(true, None), None);
        assert_eq!(
            super::startup_embedded_baseline_size(
                false,
                Some(editor_host::Size {
                    width: 800.0,
                    height: 600.0,
                }),
            ),
            None
        );
    }

    #[test]
    fn first_host_resize_callback_after_attach_becomes_startup_baseline() {
        let pending = AtomicBool::new(true);
        let base = super::SharedContentSize::new(editor_host::Size {
            width: 400.0,
            height: 400.0,
        });
        let requested = editor_host::Size {
            width: 640.0,
            height: 640.0,
        };

        assert!(super::adopt_startup_resize_baseline(
            &pending, &base, requested
        ));
        assert_eq!(base.load(), requested);
        assert_eq!(
            super::zoom_percent_for_content_size(base.load(), requested),
            100
        );
        assert!(!super::adopt_startup_resize_baseline(
            &pending,
            &base,
            editor_host::Size {
                width: 800.0,
                height: 800.0,
            },
        ));
    }

    #[test]
    fn old_host_default_baseline_would_show_wrong_initial_plugin_zoom() {
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                },
                editor_host::Size {
                    width: 400.0,
                    height: 400.0,
                },
            ),
            73
        );
    }

    #[test]
    fn plugin_owned_resize_updates_zoom_percent_from_baseline() {
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                },
                editor_host::Size {
                    width: 800.0,
                    height: 600.0,
                },
            ),
            125
        );
        assert_eq!(
            super::zoom_percent_for_content_size(
                editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                },
                editor_host::Size {
                    width: 320.0,
                    height: 240.0,
                },
            ),
            50
        );
    }

    #[test]
    fn observed_native_content_size_prefers_embedded_plugin_view() {
        assert_eq!(
            super::observed_native_content_size(
                Some(editor_host::Size {
                    width: 800.0,
                    height: 600.0,
                }),
                Some(editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                }),
            ),
            Some(editor_host::Size {
                width: 800.0,
                height: 600.0,
            })
        );
        assert_eq!(
            super::observed_native_content_size(
                None,
                Some(editor_host::Size {
                    width: 640.0,
                    height: 480.0,
                }),
            ),
            Some(editor_host::Size {
                width: 640.0,
                height: 480.0,
            })
        );
    }

    #[test]
    fn native_plugin_resize_uses_plugin_size_without_aspect_correction() {
        assert_eq!(
            super::native_content_resize_request(editor_host::Size {
                width: 900.0,
                height: 500.0,
            }),
            editor_host::Size {
                width: 900.0,
                height: 500.0,
            }
        );
    }

    #[test]
    fn native_plugin_resize_is_polled_even_when_plugin_reports_fixed_size() {
        assert!(super::should_sync_native_content_resize(true, true));
        assert!(!super::should_sync_native_content_resize(false, true));
        assert!(!super::should_sync_native_content_resize(true, false));
    }

    #[test]
    fn outer_writeback_is_required_when_aspect_corrected_outer_differs_from_native_outer() {
        assert!(super::needs_outer_writeback(
            editor_host::Size {
                width: 430.0,
                height: 442.0,
            },
            editor_host::Size {
                width: 430.0,
                height: 464.0,
            },
        ));
        assert!(!super::needs_outer_writeback(
            editor_host::Size {
                width: 430.0,
                height: 464.0,
            },
            editor_host::Size {
                width: 430.25,
                height: 463.75,
            },
        ));
    }

    #[test]
    fn resize_trace_ids_are_monotonic() {
        let mut manager = EditorWindowManager::default();

        assert_eq!(
            manager.next_resize_trace_id(),
            super::EditorResizeTraceId(1)
        );
        assert_eq!(
            manager.next_resize_trace_id(),
            super::EditorResizeTraceId(2)
        );
    }

    #[test]
    fn resize_trace_log_labels_source_stage_and_sizes() {
        let window_id = window::Id::unique();
        let message = super::format_resize_trace_event(super::EditorResizeTraceEvent {
            id: super::EditorResizeTraceId(7),
            source: super::EditorResizeSource::IcedOuterEvent,
            stage: super::EditorResizeStage::Accepted,
            target: EditorTarget {
                strip_index: 1,
                slot_index: 2,
            },
            window_id: Some(window_id),
            current_content: Some(editor_host::Size {
                width: 640.0,
                height: 480.0,
            }),
            requested_content: Some(editor_host::Size {
                width: 800.0,
                height: 600.0,
            }),
            accepted_content: Some(editor_host::Size {
                width: 768.0,
                height: 576.0,
            }),
            outer_size: None,
            note: Some("plugin adjusted"),
        });

        assert!(message.contains("resize#7"));
        assert!(message.contains("source=iced-outer-event"));
        assert!(message.contains("stage=accepted"));
        assert!(message.contains("target=EditorTarget"));
        assert!(message.contains("current=640x480"));
        assert!(message.contains("requested=800x600"));
        assert!(message.contains("accepted=768x576"));
        assert!(message.contains("note=plugin adjusted"));
    }

    #[test]
    fn processor_editor_window_manager_updates_titles_for_open_and_pending_windows() {
        let mut manager = EditorWindowManager::default();
        let open_target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let pending_target = EditorTarget {
            strip_index: 1,
            slot_index: 1,
        };
        let open_id = window::Id::unique();
        let pending_id = window::Id::unique();
        manager.begin_open(
            open_target,
            "Old".to_string(),
            true,
            Box::new(FakeEditorSession),
            open_id,
        );
        manager
            .attach(
                open_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");
        manager.begin_open(
            pending_target,
            "Old pending".to_string(),
            true,
            Box::new(FakeEditorSession),
            pending_id,
        );

        assert!(
            manager
                .set_window_title(open_target, "New".to_string())
                .is_empty()
        );
        assert!(
            manager
                .set_window_title(pending_target, "New pending".to_string())
                .is_empty()
        );

        assert_eq!(manager.window_title(open_id), Some("New"));
        assert_eq!(manager.window_title(pending_id), Some("New pending"));
    }

    #[test]
    fn editor_parent_snapshot_roundtrips_appkit_window_handle() {
        let snapshot = WindowSnapshot::capture(
            iced::window::raw_window_handle::RawWindowHandle::AppKit(
                iced::window::raw_window_handle::AppKitWindowHandle::new(
                    NonNull::<std::ffi::c_void>::dangling(),
                ),
            ),
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(
                iced::window::raw_window_handle::AppKitDisplayHandle::new(),
            )),
        )
        .expect("snapshot should capture appkit");

        let parent = snapshot_into_editor_parent(snapshot).expect("snapshot should restore appkit");

        assert!(matches!(
            parent.window,
            iced::window::raw_window_handle::RawWindowHandle::AppKit(_)
        ));
        assert!(matches!(
            parent.display,
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(_))
        ));
    }

    #[test]
    fn processor_editor_window_manager_removes_window_by_host_id() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 2,
            slot_index: 1,
        };
        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 2".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );
        manager
            .attach(
                window_id,
                None,
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");

        let removed = manager.remove_window(window_id);

        assert!(removed.is_some());
        assert!(!manager.windows.contains_key(&target));
    }

    #[test]
    fn removed_editor_detaches_session_before_dropping_host() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut session = RecordingDetachSession {
            events: Rc::clone(&events),
        };

        super::detach_session_before_dropping_host(
            &mut session,
            Some(RecordingHost {
                events: Rc::clone(&events),
            }),
        )
        .expect("detach should succeed");

        assert_eq!(&*events.borrow(), &["session-detach", "host-drop"]);
    }

    #[test]
    fn processor_editor_window_manager_moves_effect_slot_targets_with_reorder() {
        let mut manager = EditorWindowManager::default();
        let targets = [
            EditorTarget {
                strip_index: 2,
                slot_index: 1,
            },
            EditorTarget {
                strip_index: 2,
                slot_index: 2,
            },
            EditorTarget {
                strip_index: 2,
                slot_index: 3,
            },
        ];
        for target in targets {
            let window_id = window::Id::unique();
            manager.begin_open(
                target,
                format!("Slot {}", target.slot_index),
                true,
                Box::new(FakeEditorSession),
                window_id,
            );
            manager
                .attach(
                    window_id,
                    None,
                    EditorParent {
                        window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                            iced::window::raw_window_handle::AppKitWindowHandle::new(
                                std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                            ),
                        ),
                        display: None,
                    },
                )
                .expect("attach should succeed");
        }

        manager.move_slot_targets_within_strip(2, 1, 3);

        assert!(manager.windows.contains_key(&EditorTarget {
            strip_index: 2,
            slot_index: 1,
        }));
        assert!(manager.windows.contains_key(&EditorTarget {
            strip_index: 2,
            slot_index: 2,
        }));
        assert!(manager.windows.contains_key(&EditorTarget {
            strip_index: 2,
            slot_index: 3,
        }));
        assert_eq!(
            manager
                .windows
                .get(&EditorTarget {
                    strip_index: 2,
                    slot_index: 3,
                })
                .map(|window| window.title.as_str()),
            Some("Slot 1")
        );
    }
}
