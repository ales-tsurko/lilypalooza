use super::*;

pub(in crate::app) fn editor_size_from_host_size(size: editor_host::Size) -> EditorSize {
    EditorSize {
        width: crate::number::f64_to_u32(size.width.round().max(1.0)),
        height: crate::number::f64_to_u32(size.height.round().max(1.0)),
    }
}

pub(in crate::app) fn host_size_from_editor_size(size: EditorSize) -> editor_host::Size {
    editor_host::Size {
        width: f64::from(size.width),
        height: f64::from(size.height),
    }
}

pub(in crate::app) fn editor_content_resize_request(
    requested: EditorSize,
    host: &InstalledHost,
) -> Option<editor_host::Size> {
    let requested = host_size_from_editor_size(requested);
    (!same_host_size(requested, host.content_size())).then_some(requested)
}

pub(in crate::app) fn same_host_size(a: editor_host::Size, b: editor_host::Size) -> bool {
    (a.width - b.width).abs() < 0.5 && (a.height - b.height).abs() < 0.5
}

pub(in crate::app) fn record_programmatic_outer_resize(
    pending: &SharedProgrammaticOuterResizes,
    host: &InstalledHost,
    content_size: editor_host::Size,
) {
    record_programmatic_outer_resize_size(pending, host.outer_size_from_content_size(content_size));
}

pub(in crate::app) fn needs_programmatic_outer_resize(
    host: &InstalledHost,
    actual_outer_size: editor_host::Size,
    content_size: editor_host::Size,
) -> bool {
    needs_outer_writeback(
        actual_outer_size,
        host.outer_size_from_content_size(content_size),
    )
}

pub(in crate::app) fn needs_outer_writeback(
    actual_outer_size: editor_host::Size,
    desired_outer_size: editor_host::Size,
) -> bool {
    !same_host_size(actual_outer_size, desired_outer_size)
}

pub(in crate::app) fn record_programmatic_outer_resize_size(
    pending: &SharedProgrammaticOuterResizes,
    outer_size: editor_host::Size,
) {
    pending.record(outer_size);
}

pub(in crate::app) fn consume_pending_programmatic_outer_resize(
    pending: &SharedProgrammaticOuterResizes,
    outer_size: editor_host::Size,
) -> bool {
    pending.consume(outer_size)
}

pub(in crate::app) fn negotiate_editor_content_resize(
    session: &mut dyn EditorSession,
    requested: editor_host::Size,
) -> Result<editor_host::Size, EditorError> {
    session
        .resize(editor_size_from_host_size(requested))
        .map(host_size_from_editor_size)
}

pub(in crate::app) fn aspect_preserved_resize(
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

pub(in crate::app) fn zoomed_content_size(
    base: editor_host::Size,
    percent: u32,
) -> editor_host::Size {
    let scale = f64::from(percent.clamp(
        crate::app::EDITOR_FRAME_ZOOM_MIN_PERCENT,
        crate::app::EDITOR_FRAME_ZOOM_MAX_PERCENT,
    )) / 100.0;
    let width = (base.width * scale).round().max(1.0);
    let height = if base.width.is_finite() && base.width > 0.0 && base.height.is_finite() {
        (width * base.height / base.width).round().max(1.0)
    } else {
        (base.height * scale).round().max(1.0)
    };
    editor_host::Size { width, height }
}

pub(in crate::app) fn zoom_percent_for_content_size(
    base: editor_host::Size,
    content: editor_host::Size,
) -> u32 {
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
    crate::number::f64_to_u32((scale * 100.0).round().clamp(
        f64::from(crate::app::EDITOR_FRAME_ZOOM_MIN_PERCENT),
        f64::from(crate::app::EDITOR_FRAME_ZOOM_MAX_PERCENT),
    ))
}

pub(in crate::app) fn observed_native_content_size(
    embedded_content_size: Option<editor_host::Size>,
    host_content_size: Option<editor_host::Size>,
) -> Option<editor_host::Size> {
    embedded_content_size.or(host_content_size)
}

pub(in crate::app) fn native_content_resize_request(
    native_content_size: editor_host::Size,
) -> editor_host::Size {
    native_content_size
}

pub(in crate::app) fn attached_start_content_size(
    current: editor_host::Size,
    embedded: Option<editor_host::Size>,
    initial: Option<EditorSize>,
) -> editor_host::Size {
    if let Some(embedded) = embedded {
        return embedded;
    }
    initial.map_or(current, host_size_from_editor_size)
}

pub(in crate::app) fn attached_baseline_content_size(
    host_content_size: editor_host::Size,
    embedded_content_size: Option<editor_host::Size>,
) -> editor_host::Size {
    embedded_content_size.unwrap_or(host_content_size)
}

pub(in crate::app) fn startup_embedded_baseline_size(
    startup_baseline_pending: bool,
    embedded_content_size: Option<editor_host::Size>,
) -> Option<editor_host::Size> {
    startup_baseline_pending.then_some(embedded_content_size)?
}

pub(in crate::app) fn adopt_startup_resize_baseline(
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

pub(in crate::app) fn set_editor_resize_baseline(
    window: &mut EditorWindow,
    content_size: editor_host::Size,
) {
    window.base_content_size = content_size;
    if let Some(shared) = window.base_content_size_shared.as_ref() {
        shared.store(content_size);
    }
}

pub(in crate::app) fn should_sync_native_content_resize(
    visible: bool,
    tracks_native_content_resize: bool,
) -> bool {
    visible && tracks_native_content_resize
}

pub(in crate::app) fn native_content_resize_observation(
    window: &mut EditorWindow,
    errors: &mut Vec<String>,
) -> Option<NativeContentResizeObservation> {
    if !should_sync_native_content_resize(window.visible, window.tracks_native_content_resize) {
        return None;
    }
    let host = window.host.as_mut()?;
    let (embedded_content_size, native_host_content_size, current_content) =
        native_resize_host_sizes(host, errors)?;
    let native_content_size =
        observed_native_content_size(embedded_content_size, native_host_content_size)?;

    Some(NativeContentResizeObservation {
        embedded_content_size,
        native_content_size,
        native_outer_size: host.outer_size_from_content_size(native_content_size),
        current_content,
    })
}

pub(in crate::app) fn native_resize_host_sizes(
    host: &mut InstalledHost,
    errors: &mut Vec<String>,
) -> Option<(
    Option<editor_host::Size>,
    Option<editor_host::Size>,
    editor_host::Size,
)> {
    let embedded_content_size = match host.embedded_content_size() {
        Ok(size) => size,
        Err(error) => {
            errors.push(error.to_string());
            return None;
        }
    };
    let native_host_content_size = match host.native_content_size() {
        Ok(size) => size,
        Err(error) => {
            errors.push(error.to_string());
            return None;
        }
    };
    Some((
        embedded_content_size,
        native_host_content_size,
        host.content_size(),
    ))
}

pub(in crate::app) fn sync_native_content_resize_for_window(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
    errors: &mut Vec<String>,
) {
    if apply_startup_native_content_baseline(trace_counter, target, window, observation, errors) {
        return;
    }
    if defer_same_native_content_zoom(window, observation) {
        return;
    }
    if ignore_programmatic_native_resize_echo(trace_counter, target, window, observation) {
        return;
    }
    apply_observed_native_content_resize(trace_counter, target, window, observation, errors);
}

pub(in crate::app) fn defer_same_native_content_zoom(
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
) -> bool {
    if !same_host_size(observation.native_content_size, observation.current_content) {
        return false;
    }
    defer_zoom_percent(
        window,
        zoom_percent_for_content_size(window.base_content_size, observation.native_content_size),
        Instant::now(),
    );
    true
}

pub(in crate::app) fn next_deferred_outer_resize_deadline(
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

pub(in crate::app) fn next_deferred_zoom_deadline(
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

pub(in crate::app) fn defer_zoom_percent(
    window: &mut EditorWindow,
    zoom_percent: u32,
    now: Instant,
) {
    window.pending_zoom_percent_until = Some(next_deferred_zoom_deadline(
        window.pending_zoom_percent,
        zoom_percent,
        window.pending_zoom_percent_until,
        now,
    ));
    window.pending_zoom_percent = Some(zoom_percent);
}

pub(in crate::app) fn apply_startup_native_content_baseline(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
    errors: &mut Vec<String>,
) -> bool {
    let Some(startup_baseline_pending) = window.startup_baseline_pending.clone() else {
        return false;
    };
    let Some(startup_baseline) = startup_embedded_baseline_size(
        startup_baseline_pending.load(Ordering::Relaxed),
        observation.embedded_content_size,
    ) else {
        return false;
    };

    startup_baseline_pending.store(false, Ordering::Relaxed);
    window.startup_baseline_pending = None;
    set_editor_resize_baseline(window, startup_baseline);
    resize_to_startup_native_baseline(
        trace_counter,
        target,
        window,
        observation,
        startup_baseline,
        errors,
    );
    true
}

pub(in crate::app) fn resize_to_startup_native_baseline(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
    startup_baseline: editor_host::Size,
    errors: &mut Vec<String>,
) {
    let Some(host) = window.host.as_mut() else {
        return;
    };
    if same_host_size(startup_baseline, observation.current_content) {
        host.set_zoom_percent(100);
        return;
    }
    let trace_id = next_resize_trace_id(trace_counter);
    trace_native_content_resize(NativeContentResizeTrace {
        id: trace_id,
        stage: EditorResizeStage::Begin,
        target,
        window_id: window.host_window_id,
        current_content: observation.current_content,
        requested_content: startup_baseline,
        accepted_content: None,
        outer_size: observation.native_outer_size,
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
            trace_native_content_resize(NativeContentResizeTrace {
                id: trace_id,
                stage: EditorResizeStage::Applied,
                target,
                window_id: window.host_window_id,
                current_content: host.content_size(),
                requested_content: startup_baseline,
                accepted_content: Some(startup_baseline),
                outer_size: observation.native_outer_size,
                note: Some("startup baseline adopted"),
            });
        }
        Err(error) => {
            trace_native_content_resize(NativeContentResizeTrace {
                id: trace_id,
                stage: EditorResizeStage::Error,
                target,
                window_id: window.host_window_id,
                current_content: host.content_size(),
                requested_content: startup_baseline,
                accepted_content: None,
                outer_size: observation.native_outer_size,
                note: Some("host resize failed"),
            });
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn ignore_programmatic_native_resize_echo(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
) -> bool {
    if !consume_pending_programmatic_outer_resize(
        &window.pending_programmatic_outer_resizes,
        observation.native_outer_size,
    ) {
        return false;
    }
    trace_native_content_resize(NativeContentResizeTrace {
        id: next_resize_trace_id(trace_counter),
        stage: EditorResizeStage::Ignored,
        target,
        window_id: window.host_window_id,
        current_content: observation.current_content,
        requested_content: observation.native_content_size,
        accepted_content: None,
        outer_size: observation.native_outer_size,
        note: Some("programmatic echo"),
    });
    true
}

pub(in crate::app) fn apply_observed_native_content_resize(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    observation: NativeContentResizeObservation,
    errors: &mut Vec<String>,
) {
    let requested = native_content_resize_request(observation.native_content_size);
    let trace_id = next_resize_trace_id(trace_counter);
    trace_native_content_resize(NativeContentResizeTrace {
        id: trace_id,
        stage: EditorResizeStage::Begin,
        target,
        window_id: window.host_window_id,
        current_content: observation.current_content,
        requested_content: requested,
        accepted_content: None,
        outer_size: observation.native_outer_size,
        note: Some("embedded view changed"),
    });
    let Some(host) = window.host.as_mut() else {
        return;
    };
    record_programmatic_outer_resize(&window.pending_programmatic_outer_resizes, host, requested);
    match host.resize_content_from_top(requested) {
        Ok(()) => {
            trace_native_content_resize(NativeContentResizeTrace {
                id: trace_id,
                stage: EditorResizeStage::Applied,
                target,
                window_id: window.host_window_id,
                current_content: host.content_size(),
                requested_content: requested,
                accepted_content: Some(requested),
                outer_size: observation.native_outer_size,
                note: Some("adopted embedded view size"),
            });
            defer_zoom_percent(
                window,
                zoom_percent_for_content_size(window.base_content_size, requested),
                Instant::now(),
            );
        }
        Err(error) => {
            trace_native_content_resize(NativeContentResizeTrace {
                id: trace_id,
                stage: EditorResizeStage::Error,
                target,
                window_id: window.host_window_id,
                current_content: host.content_size(),
                requested_content: requested,
                accepted_content: None,
                outer_size: observation.native_outer_size,
                note: Some("host resize failed"),
            });
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn trace_native_content_resize(event: NativeContentResizeTrace) {
    trace_resize_event(EditorResizeTraceEvent {
        id: event.id,
        source: EditorResizeSource::NativeContentSize,
        stage: event.stage,
        target: event.target,
        window_id: Some(event.window_id),
        current_content: Some(event.current_content),
        requested_content: Some(event.requested_content),
        accepted_content: event.accepted_content,
        outer_size: Some(event.outer_size),
        note: event.note,
    });
}

pub(in crate::app) fn next_resize_trace_id(next: &mut u64) -> EditorResizeTraceId {
    *next += 1;
    EditorResizeTraceId(*next)
}

pub(in crate::app) fn resize_source_label(source: EditorResizeSource) -> &'static str {
    const LABELS: [&str; 5] = [
        "session-requested-size",
        "header-zoom",
        "native-content-size",
        "iced-outer-event",
        "deferred-outer-resize",
    ];
    LABELS.get(source as usize).copied().unwrap_or("unknown")
}

pub(in crate::app) fn resize_stage_label(stage: EditorResizeStage) -> &'static str {
    const LABELS: [&str; 5] = ["begin", "ignored", "accepted", "applied", "error"];
    LABELS.get(stage as usize).copied().unwrap_or("unknown")
}

pub(in crate::app) fn format_resize_size(size: editor_host::Size) -> String {
    format!(
        "{}x{}",
        crate::number::f64_to_u32(size.width),
        crate::number::f64_to_u32(size.height)
    )
}

pub(in crate::app) fn format_resize_trace_event(event: EditorResizeTraceEvent<'_>) -> String {
    let mut fields = vec![
        format!("resize#{}", event.id.0),
        format!("source={}", resize_source_label(event.source)),
        format!("stage={}", resize_stage_label(event.stage)),
        format!("target={:?}", event.target),
    ];
    append_resize_trace_context(&mut fields, event);
    fields.join(" ")
}

pub(in crate::app) fn append_resize_trace_context(
    fields: &mut Vec<String>,
    event: EditorResizeTraceEvent<'_>,
) {
    append_resize_window_id(fields, event.window_id);
    append_resize_size_field(fields, "current", event.current_content);
    append_resize_size_field(fields, "requested", event.requested_content);
    append_resize_size_field(fields, "accepted", event.accepted_content);
    append_resize_size_field(fields, "outer", event.outer_size);
    append_resize_note(fields, event.note);
}

pub(in crate::app) fn append_resize_window_id(
    fields: &mut Vec<String>,
    window_id: Option<window::Id>,
) {
    if let Some(window_id) = window_id {
        fields.push(format!("window_id={window_id:?}"));
    }
}

pub(in crate::app) fn append_resize_size_field(
    fields: &mut Vec<String>,
    label: &str,
    size: Option<editor_host::Size>,
) {
    if let Some(size) = size {
        fields.push(format!("{label}={}", format_resize_size(size)));
    }
}

pub(in crate::app) fn append_resize_note(fields: &mut Vec<String>, note: Option<&str>) {
    if let Some(note) = note {
        fields.push(format!("note={note}"));
    }
}

pub(in crate::app) fn trace_resize_event(event: EditorResizeTraceEvent<'_>) {
    trace_editor_resize(|| format_resize_trace_event(event));
}

pub(in crate::app) fn defer_outer_resize(
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

pub(in crate::app) fn apply_deferred_outer_resize(
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
        trace_deferred_outer_resize(DeferredOuterResizeTrace {
            id: trace_id,
            stage: EditorResizeStage::Ignored,
            target,
            window_id: window.host_window_id,
            current_content: Some(host.content_size()),
            requested_content: requested,
            accepted_content: None,
            outer_size,
            note: Some("same size"),
        });
        return errors;
    }
    let negotiated = negotiate_editor_content_resize(window.session.as_mut(), requested);
    let request = DeferredOuterResizeRequest {
        trace_id,
        target,
        outer_size,
        requested,
    };
    finish_deferred_outer_resize(request, window, negotiated, &mut errors);
    errors
}
