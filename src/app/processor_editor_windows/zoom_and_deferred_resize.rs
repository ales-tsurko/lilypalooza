use super::*;

pub(in crate::app) fn set_zoom_percent_for_window(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window: &mut EditorWindow,
    percent: u32,
) -> Vec<String> {
    reset_pending_zoom(window);
    let requested = zoomed_content_size(window.base_content_size, percent);
    let mut errors = Vec::new();
    if !window.resizable {
        trace_ignored_header_zoom(trace_id, target, window, requested, "fixed size", None);
        return errors;
    }

    let window_id = window.host_window_id;
    let base_content_size = window.base_content_size;
    let pending_programmatic_outer_resizes = window.pending_programmatic_outer_resizes.clone();
    let Some(host) = window.host.as_mut() else {
        trace_ignored_header_zoom(trace_id, target, window, requested, "missing host", None);
        return errors;
    };

    let current = host.content_size();
    if same_host_size(current, requested) {
        trace_ignored_header_zoom(
            trace_id,
            target,
            window,
            requested,
            "same size",
            Some(current),
        );
        return errors;
    }

    trace_header_zoom_begin(trace_id, target, window_id, current, requested);
    let accepted = match negotiate_editor_content_resize(window.session.as_mut(), requested) {
        Ok(accepted) => accepted,
        Err(error) => {
            trace_header_zoom_error(
                trace_id,
                target,
                window_id,
                current,
                requested,
                None,
                "session resize failed",
            );
            errors.push(error.to_string());
            return errors;
        }
    };

    let resize = HeaderZoomResize {
        trace_id,
        target,
        window_id,
        base_content_size,
        pending_programmatic_outer_resizes,
        current,
        requested,
        accepted,
    };
    apply_header_zoom_resize(resize, host, &mut errors);
    errors
}

#[derive(Clone)]
pub(in crate::app) struct HeaderZoomResize {
    pub(in crate::app) trace_id: EditorResizeTraceId,
    pub(in crate::app) target: EditorTarget,
    pub(in crate::app) window_id: window::Id,
    pub(in crate::app) base_content_size: editor_host::Size,
    pub(in crate::app) pending_programmatic_outer_resizes: SharedProgrammaticOuterResizes,
    pub(in crate::app) current: editor_host::Size,
    pub(in crate::app) requested: editor_host::Size,
    pub(in crate::app) accepted: editor_host::Size,
}

pub(in crate::app) fn reset_pending_zoom(window: &mut EditorWindow) {
    window.pending_zoom_percent = None;
    window.pending_zoom_percent_until = None;
}

pub(in crate::app) fn trace_ignored_header_zoom(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window: &EditorWindow,
    requested: editor_host::Size,
    note: &'static str,
    current_content: Option<editor_host::Size>,
) {
    trace_resize_event(EditorResizeTraceEvent {
        id: trace_id,
        source: EditorResizeSource::HeaderZoom,
        stage: EditorResizeStage::Ignored,
        target,
        window_id: Some(window.host_window_id),
        current_content: current_content
            .or_else(|| window.host.as_ref().map(InstalledHost::content_size)),
        requested_content: Some(requested),
        accepted_content: None,
        outer_size: None,
        note: Some(note),
    });
}

pub(in crate::app) fn trace_header_zoom_begin(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    current: editor_host::Size,
    requested: editor_host::Size,
) {
    trace_resize_event(EditorResizeTraceEvent {
        id: trace_id,
        source: EditorResizeSource::HeaderZoom,
        stage: EditorResizeStage::Begin,
        target,
        window_id: Some(window_id),
        current_content: Some(current),
        requested_content: Some(requested),
        accepted_content: None,
        outer_size: None,
        note: None,
    });
}

pub(in crate::app) fn trace_header_zoom_error(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    current: editor_host::Size,
    requested: editor_host::Size,
    accepted: Option<editor_host::Size>,
    note: &'static str,
) {
    trace_resize_event(EditorResizeTraceEvent {
        id: trace_id,
        source: EditorResizeSource::HeaderZoom,
        stage: EditorResizeStage::Error,
        target,
        window_id: Some(window_id),
        current_content: Some(current),
        requested_content: Some(requested),
        accepted_content: accepted,
        outer_size: None,
        note: Some(note),
    });
}

pub(in crate::app) fn apply_header_zoom_resize(
    resize: HeaderZoomResize,
    host: &mut InstalledHost,
    errors: &mut Vec<String>,
) {
    record_programmatic_outer_resize(
        &resize.pending_programmatic_outer_resizes,
        host,
        resize.accepted,
    );
    match host.resize_content_from_top(resize.accepted) {
        Ok(()) => {
            host.set_zoom_percent(zoom_percent_for_content_size(
                resize.base_content_size,
                resize.accepted,
            ));
            trace_resize_event(EditorResizeTraceEvent {
                id: resize.trace_id,
                source: EditorResizeSource::HeaderZoom,
                stage: EditorResizeStage::Applied,
                target: resize.target,
                window_id: Some(resize.window_id),
                current_content: Some(host.content_size()),
                requested_content: Some(resize.requested),
                accepted_content: Some(resize.accepted),
                outer_size: None,
                note: None,
            });
        }
        Err(error) => {
            trace_header_zoom_error(
                resize.trace_id,
                resize.target,
                resize.window_id,
                resize.current,
                resize.requested,
                Some(resize.accepted),
                "host resize failed",
            );
            errors.push(error.to_string());
        }
    }
}

pub(in crate::app) fn resize_window_outer_for_target(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    window: &mut EditorWindow,
    outer_size: editor_host::Size,
) -> Vec<String> {
    if !window.resizable {
        trace_ignored_iced_outer_resize(trace_id, target, window, outer_size, "fixed size");
        return Vec::new();
    }
    if window.host.is_none() {
        trace_ignored_iced_outer_resize(trace_id, target, window, outer_size, "missing host");
        return Vec::new();
    }

    let mut errors = Vec::new();
    if consume_pending_outer_resize_echo(window, trace_id, target, window_id, outer_size) {
        return errors;
    }
    if !outer_resize_is_live(window, trace_id, target, window_id, outer_size, &mut errors) {
        return errors;
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

pub(in crate::app) fn trace_ignored_iced_outer_resize(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window: &EditorWindow,
    outer_size: editor_host::Size,
    note: &'static str,
) {
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
        note: Some(note),
    });
}

pub(in crate::app) fn consume_pending_outer_resize_echo(
    window: &EditorWindow,
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    outer_size: editor_host::Size,
) -> bool {
    if !consume_pending_programmatic_outer_resize(
        &window.pending_programmatic_outer_resizes,
        outer_size,
    ) {
        return false;
    }
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
    true
}

pub(in crate::app) fn outer_resize_is_live(
    window: &EditorWindow,
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    outer_size: editor_host::Size,
    errors: &mut Vec<String>,
) -> bool {
    let Some(host) = window.host.as_ref() else {
        return false;
    };
    match host.is_live_resizing() {
        Ok(true) => true,
        Ok(false) => {
            trace_non_live_outer_resize_echo(trace_id, target, window_id, host, outer_size);
            false
        }
        Err(error) => {
            errors.push(error.to_string());
            false
        }
    }
}

pub(in crate::app) fn trace_non_live_outer_resize_echo(
    trace_id: EditorResizeTraceId,
    target: EditorTarget,
    window_id: window::Id,
    host: &InstalledHost,
    outer_size: editor_host::Size,
) {
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
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct DeferredResizeDue {
    pub(in crate::app) outer: bool,
    pub(in crate::app) zoom: bool,
}

impl DeferredResizeDue {
    fn for_window(window: &EditorWindow, now: Instant) -> Self {
        Self {
            outer: deadline_due(window.pending_outer_resize_until, now),
            zoom: deadline_due(window.pending_zoom_percent_until, now),
        }
    }

    fn any(self) -> bool {
        self.outer || self.zoom
    }
}

pub(in crate::app) fn deadline_due(deadline: Option<Instant>, now: Instant) -> bool {
    deadline.is_some_and(|deadline| deadline <= now)
}

pub(in crate::app) fn expire_deferred_window(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    now: Instant,
    errors: &mut Vec<String>,
) {
    let due = DeferredResizeDue::for_window(window, now);
    if !due.any() || defer_due_resize_while_live(window, now, due, errors) {
        return;
    }

    apply_due_deferred_outer_resize(trace_counter, target, window, due, errors);
    apply_due_deferred_zoom(window, due);
}

pub(in crate::app) fn defer_due_resize_while_live(
    window: &mut EditorWindow,
    now: Instant,
    due: DeferredResizeDue,
    errors: &mut Vec<String>,
) -> bool {
    let Some(host) = window.host.as_mut() else {
        return false;
    };

    match host.is_live_resizing() {
        Ok(true) => {
            refresh_due_deferred_deadlines(window, now, due);
            true
        }
        Ok(false) => false,
        Err(error) => {
            errors.push(error.to_string());
            false
        }
    }
}

pub(in crate::app) fn refresh_due_deferred_deadlines(
    window: &mut EditorWindow,
    now: Instant,
    due: DeferredResizeDue,
) {
    if due.outer {
        window.pending_outer_resize_until = Some(now + RESIZE_IDLE_TIMEOUT);
    }
    if due.zoom {
        window.pending_zoom_percent_until = Some(now + RESIZE_IDLE_TIMEOUT);
    }
}

pub(in crate::app) fn apply_due_deferred_outer_resize(
    trace_counter: &mut u64,
    target: EditorTarget,
    window: &mut EditorWindow,
    due: DeferredResizeDue,
    errors: &mut Vec<String>,
) {
    if !due.outer {
        return;
    }

    window.pending_outer_resize_until = None;
    if let Some(outer_size) = window.pending_outer_resize.take() {
        errors.extend(apply_deferred_outer_resize(
            trace_counter,
            target,
            window,
            outer_size,
        ));
    }
}

pub(in crate::app) fn apply_due_deferred_zoom(window: &mut EditorWindow, due: DeferredResizeDue) {
    if !due.zoom {
        return;
    }

    window.pending_zoom_percent_until = None;
    if let Some(zoom_percent) = window.pending_zoom_percent.take()
        && let Some(host) = window.host.as_mut()
    {
        host.set_zoom_percent(zoom_percent);
    }
}

pub(in crate::app) fn trace_editor_resize(message: impl FnOnce() -> String) {
    log::trace!(
        target: "lilypalooza::editor_windows",
        "thread={:?} {}",
        std::thread::current().id(),
        message()
    );
}

#[cfg(test)]
impl EditorWindowManager {
    pub(in crate::app) fn contains_window(&self, target: EditorTarget) -> bool {
        self.windows.contains_key(&target)
    }
}
