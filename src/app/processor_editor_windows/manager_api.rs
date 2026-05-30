use super::*;

impl EditorWindowManager {
    pub(in crate::app) fn next_resize_trace_id(&mut self) -> EditorResizeTraceId {
        next_resize_trace_id(&mut self.next_resize_trace_id)
    }

    pub(in crate::app) fn focus_existing(&mut self, target: EditorTarget) -> Option<window::Id> {
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

    #[cfg(test)]
    pub(in crate::app) fn begin_open(
        &mut self,
        target: EditorTarget,
        title: String,
        resizable: bool,
        session: Box<dyn EditorSession>,
        window_id: window::Id,
    ) {
        self.begin_open_with_controller(EditorOpenRequest {
            target,
            title,
            resizable,
            session,
            controller: empty_editor_controller(),
            native_editor_available: true,
            controls_visible: Arc::new(AtomicBool::new(false)),
            window_id,
        });
    }

    pub(in crate::app) fn begin_open_with_controller(&mut self, request: EditorOpenRequest) {
        self.pending.insert(
            request.window_id,
            PendingEditorWindow {
                target: request.target,
                title: request.title,
                resizable: request.resizable,
                host_window_id: request.window_id,
                session: request.session,
                controller: request.controller,
                native_editor_available: request.native_editor_available,
                controls_visible: request.controls_visible,
            },
        );
        self.focused = Some(request.target);
    }

    pub(in crate::app) fn attach(
        &mut self,
        window_id: window::Id,
        mut host: Option<InstalledHost>,
        parent: EditorParent,
    ) -> Result<(), EditorError> {
        let mut pending = self.take_pending_editor_window(window_id)?;
        let resize_base_content_size = host
            .as_ref()
            .map(|host| Arc::new(SharedContentSize::new(host.content_size())));
        let startup_baseline_pending = host.as_ref().map(|_| Arc::new(AtomicBool::new(true)));
        let pending_programmatic_outer_resizes = Arc::new(ProgrammaticOuterResizeEchoes::new());
        install_host_resize_handler(
            pending.session.as_mut(),
            host.as_ref(),
            resize_base_content_size.as_ref(),
            startup_baseline_pending.as_ref(),
            &pending_programmatic_outer_resizes,
            &pending.controls_visible,
        )?;
        pending.session.attach(parent)?;
        let tracks_native_content_resize = configure_native_resize_tracking(
            window_id,
            pending.target,
            pending.session.as_ref(),
            host.as_ref(),
        )?;
        apply_session_resizability(
            pending.session.as_mut(),
            &mut pending.resizable,
            host.as_mut(),
        )?;
        let sizing = initialize_attached_host_size(
            pending.session.as_mut(),
            host.as_mut(),
            startup_baseline_pending.as_ref(),
            &pending_programmatic_outer_resizes,
        );
        let sizing = sizing?;
        if pending.controls_visible.load(Ordering::Relaxed)
            && let Some(host) = host.as_mut()
        {
            host.set_content_visible(false)
                .map_err(|error| EditorError::HostUnavailable(error.to_string()))?;
        }
        if let Some(base_content_size) = resize_base_content_size.as_ref() {
            base_content_size.store(sizing.content_size);
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
                controller: pending.controller,
                native_editor_available: pending.native_editor_available,
                controls_visible: pending.controls_visible,
                native_view_content_size: None,
                visible: true,
                tracks_native_content_resize,
                base_content_size: sizing.content_size,
                base_content_size_shared: resize_base_content_size,
                startup_baseline_pending: startup_baseline_pending.filter(|pending| {
                    sizing.wait_for_embedded_startup_baseline && pending.load(Ordering::Relaxed)
                }),
                pending_programmatic_outer_resizes,
                pending_outer_resize: None,
                pending_outer_resize_until: None,
                pending_zoom_percent: None,
                pending_zoom_percent_until: None,
                resize_aspect_ratio: sizing.content_size.width
                    / sizing.content_size.height.max(1.0),
            },
        );
        self.windows_by_id.insert(window_id, pending.target);
        Ok(())
    }

    fn take_pending_editor_window(
        &mut self,
        window_id: window::Id,
    ) -> Result<PendingEditorWindow, EditorError> {
        self.pending.remove(&window_id).ok_or_else(|| {
            EditorError::HostUnavailable(format!(
                "pending editor window `{window_id:?}` is missing"
            ))
        })
    }

    pub(in crate::app) fn pending_contains(&self, window_id: window::Id) -> bool {
        self.pending.contains_key(&window_id)
    }

    pub(in crate::app) fn window_title(&self, window_id: window::Id) -> Option<&str> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.title.as_str()))
            .or_else(|| {
                self.pending
                    .get(&window_id)
                    .map(|window| window.title.as_str())
            })
    }

    pub(in crate::app) fn window_resizable(&self, window_id: window::Id) -> Option<bool> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.resizable))
            .or_else(|| self.pending.get(&window_id).map(|window| window.resizable))
    }

    pub(in crate::app) fn target_for_window(&self, window_id: window::Id) -> Option<EditorTarget> {
        self.windows_by_id.get(&window_id).copied()
    }

    pub(in crate::app) fn window_for_target(&self, target: EditorTarget) -> Option<window::Id> {
        self.windows
            .get(&target)
            .map(|window| window.host_window_id)
            .or_else(|| {
                self.pending.iter().find_map(|(window_id, pending)| {
                    (pending.target == target).then_some(*window_id)
                })
            })
    }

    pub(in crate::app) fn frame_controller_for_window(
        &self,
        window_id: window::Id,
    ) -> Option<(SharedController, bool, Arc<AtomicBool>)> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| {
                self.windows.get(target).map(|window| {
                    (
                        Arc::clone(&window.controller),
                        window.native_editor_available,
                        Arc::clone(&window.controls_visible),
                    )
                })
            })
            .or_else(|| {
                self.pending.get(&window_id).map(|window| {
                    (
                        Arc::clone(&window.controller),
                        window.native_editor_available,
                        Arc::clone(&window.controls_visible),
                    )
                })
            })
    }

    pub(in crate::app) fn set_controls_visible(
        &mut self,
        target: EditorTarget,
        visible: bool,
    ) -> Vec<String> {
        let Some(window) = self.windows.get_mut(&target) else {
            for pending in self.pending.values() {
                if pending.target == target {
                    pending.controls_visible.store(visible, Ordering::Relaxed);
                    break;
                }
            }
            return Vec::new();
        };
        window.controls_visible.store(visible, Ordering::Relaxed);
        let mut errors = Vec::new();
        apply_editor_view_visibility(window, visible, &mut errors);
        errors
    }

    #[cfg(test)]
    pub(in crate::app) fn editor_view_state(&self, target: EditorTarget) -> Option<(bool, bool)> {
        self.windows
            .get(&target)
            .map(|window| {
                (
                    window.native_editor_available,
                    window.controls_visible.load(Ordering::Relaxed),
                )
            })
            .or_else(|| {
                self.pending
                    .values()
                    .find(|window| window.target == target)
                    .map(|window| {
                        (
                            window.native_editor_available,
                            window.controls_visible.load(Ordering::Relaxed),
                        )
                    })
            })
    }

    pub(in crate::app) fn focus_window(&mut self, window_id: window::Id) -> Vec<String> {
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

    pub(in crate::app) fn remove_window(
        &mut self,
        window_id: window::Id,
    ) -> Option<RemovedEditorWindow> {
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

    pub(in crate::app) fn remove_target(
        &mut self,
        target: EditorTarget,
    ) -> Option<RemovedEditorWindow> {
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

    pub(in crate::app) fn shift_targets_after_removed_strip(&mut self, removed_strip_index: usize) {
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

    pub(in crate::app) fn move_slot_targets_within_strip(
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

    pub(in crate::app) fn remove_all_windows(&mut self) -> Vec<RemovedEditorWindow> {
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

    pub(in crate::app) fn hide_window(
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

    pub(in crate::app) fn hide_all_windows(&mut self) -> Vec<Vec<String>> {
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

    pub(in crate::app) fn show_window(&mut self, window_id: window::Id) -> Vec<String> {
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

    pub(in crate::app) fn window_visible(&self, window_id: window::Id) -> bool {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target))
            .is_some_and(|window| window.visible)
    }

    pub(in crate::app) fn targets_for_strip(&self, strip_index: usize) -> Vec<EditorTarget> {
        self.windows
            .keys()
            .chain(self.pending.values().map(|window| &window.target))
            .filter(|target| target.strip_index == strip_index)
            .copied()
            .collect()
    }

    pub(in crate::app) fn set_window_title(
        &mut self,
        target: EditorTarget,
        title: String,
    ) -> Vec<String> {
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

    pub(in crate::app) fn set_preset_state(
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

    pub(in crate::app) fn preset_state(&self, target: EditorTarget) -> Option<EditorPresetState> {
        self.windows
            .get(&target)
            .and_then(|window| window.host.as_ref())
            .and_then(InstalledHost::preset_state)
    }

    pub(in crate::app) fn drain_frame_commands(
        &mut self,
    ) -> Vec<(EditorTarget, EditorFrameCommand)> {
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

    pub(in crate::app) fn apply_requested_content_resizes(
        &mut self,
        mut on_error: impl FnMut(String),
    ) {
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            apply_requested_content_resize_for_window(
                trace_counter,
                *target,
                window,
                &mut on_error,
            );
        }
    }

    pub(in crate::app) fn sync_native_content_resizes(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            let Some(observation) = native_content_resize_observation(window, &mut errors) else {
                continue;
            };
            sync_native_content_resize_for_window(
                trace_counter,
                *target,
                window,
                observation,
                &mut errors,
            );
        }
        errors
    }

    pub(in crate::app) fn resize_window_outer(
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
        resize_window_outer_for_target(trace_id, target, window_id, window, outer_size)
    }

    pub(in crate::app) fn set_zoom_percent(
        &mut self,
        target: EditorTarget,
        percent: u32,
    ) -> Vec<String> {
        let trace_id = self.next_resize_trace_id();
        let Some(window) = self.windows.get_mut(&target) else {
            return Vec::new();
        };
        set_zoom_percent_for_window(trace_id, target, window, percent)
    }

    pub(in crate::app) fn expire_deferred_outer_resizes(&mut self, now: Instant) -> Vec<String> {
        let mut errors = Vec::new();
        let trace_counter = &mut self.next_resize_trace_id;
        for (target, window) in &mut self.windows {
            expire_deferred_window(trace_counter, *target, window, now, &mut errors);
        }
        errors
    }

    pub(in crate::app) fn close_requested_windows(&self) -> Vec<window::Id> {
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

    pub(in crate::app) fn has_installed_hosts(&self) -> bool {
        self.windows.values().any(|window| window.host.is_some())
    }
}
