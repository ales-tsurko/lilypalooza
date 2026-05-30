use super::*;

impl Lilypalooza {
    pub(in crate::app) fn log_processor_editor_error(
        &mut self,
        action: &str,
        error: impl std::fmt::Display,
    ) {
        self.logger
            .push(format!("Processor editor {action} failed: {error}"));
    }

    pub(super) fn toggle_processor_browser(&mut self, target: EditorTarget) -> Task<Message> {
        if self.open_processor_browser_target == Some(target) {
            self.close_processor_browser();
            return Task::none();
        }
        self.open_processor_browser_target = Some(target);
        self.open_instrument_browser_track =
            (target.slot_index == 0 && target.strip_index > 0).then(|| target.strip_index - 1);
        self.instrument_browser_search.clear();
        iced::widget::operation::focus(self.instrument_browser_search_input_id.clone())
    }

    pub(super) fn close_processor_browser(&mut self) {
        self.open_processor_browser_target = None;
        self.open_instrument_browser_track = None;
        self.instrument_browser_search.clear();
    }

    pub(super) fn defer_mixer_message_after_editor_detach(&mut self, message: MixerMessage) {
        self.pending_mixer_message_after_editor_detach = Some(DeferredMixerMessage {
            message,
            frames_remaining: EDITOR_DETACH_SETTLE_FRAMES,
        });
    }

    pub(in crate::app) fn destroy_editor_target(&mut self, target: EditorTarget) -> Task<Message> {
        let Some(removed) = self.processor_editor_windows.remove_target(target) else {
            return Task::none();
        };
        let window_id = removed.window_id;
        if let Err(error) = removed.detach() {
            self.log_processor_editor_error("detach", error);
        }
        window::close(window_id)
    }

    pub(super) fn close_editor_before_deferred_mixer_message(
        &mut self,
        target: EditorTarget,
        message: MixerMessage,
    ) -> Option<Task<Message>> {
        let window_id = self.processor_editor_windows.window_for_target(target)?;
        if !self.processor_editor_windows.pending_contains(window_id)
            && !self.processor_editor_windows.window_visible(window_id)
            && let Some(removed) = self.processor_editor_windows.remove_target(target)
        {
            let window_id = removed.window_id;
            if let Err(error) = removed.detach() {
                self.log_processor_editor_error("detach", error);
            }
            self.defer_mixer_message_after_editor_detach(message);
            return Some(window::close(window_id));
        }
        self.pending_mixer_message_after_editor_close = Some((window_id, message));
        Some(window::close(window_id))
    }

    pub(super) fn destroy_editor_strip_and_shift_later(
        &mut self,
        strip_index: usize,
    ) -> Task<Message> {
        let tasks = self
            .processor_editor_windows
            .targets_for_strip(strip_index)
            .into_iter()
            .map(|target| self.destroy_editor_target(target))
            .collect::<Vec<_>>();
        self.processor_editor_windows
            .shift_targets_after_removed_strip(strip_index);
        Task::batch(tasks)
    }

    pub(in crate::app) fn request_remove_bus_confirmation(&mut self, id: u16) -> Task<Message> {
        let name = self
            .playback
            .as_ref()
            .and_then(|playback| playback.mixer_state().bus(BusId(id)).ok())
            .map(|bus| bus.name.clone())
            .unwrap_or_else(|| format!("Bus {id}"));
        self.show_prompt(
            ErrorPrompt::new(
                format!("Remove {name}?"),
                "This removes the bus and clears routes or sends that target it.",
                ErrorFatality::Recoverable,
                PromptButtons::OkCancel,
            ),
            Some(PromptOkAction::RemoveBus(id)),
        );
        Task::none()
    }

    pub(in crate::app) fn remove_bus_confirmed(&mut self, id: u16) -> Task<Message> {
        self.remove_bus(id, true)
    }

    pub(super) fn remove_bus(&mut self, id: u16, confirmed: bool) -> Task<Message> {
        if !confirmed {
            return self.request_remove_bus_confirmation(id);
        }

        let mut editor_cleanup = Task::none();
        let removed_bus_strip_index = self.playback.as_ref().and_then(|playback| {
            playback
                .mixer_state()
                .buses()
                .iter()
                .position(|bus| bus.bus_id == Some(BusId(id)))
                .map(|index| 1 + playback.mixer_state().track_count() + index)
        });
        if let Some(strip_index) = removed_bus_strip_index {
            editor_cleanup = Task::batch([
                editor_cleanup,
                self.destroy_editor_strip_and_shift_later(strip_index),
            ]);
        }

        let Some(playback) = self.playback.as_mut() else {
            return editor_cleanup;
        };
        let mut mixer = playback.mixer();
        if let Err(error) = mixer.remove_bus(BusId(id)) {
            self.logger.push(error.to_string());
        } else {
            self.piano_roll
                .set_global_solo_active(mixer_has_any_solo(&mixer));
        }
        editor_cleanup
    }

    pub(in crate::app) fn destroy_all_editor_windows(&mut self) -> Task<Message> {
        let removed = self.processor_editor_windows.remove_all_windows();
        if removed.is_empty() {
            return Task::none();
        }

        let mut tasks = Vec::with_capacity(removed.len());
        for removed in removed {
            let window_id = removed.window_id;
            if let Err(error) = removed.detach() {
                self.log_processor_editor_error("detach", error);
            }
            tasks.push(window::close(window_id));
        }
        Task::batch(tasks)
    }

    pub(in crate::app) fn hide_all_editor_windows(&mut self) -> Task<Message> {
        for errors in self.processor_editor_windows.hide_all_windows() {
            for error in errors {
                self.log_processor_editor_error("hide", error);
            }
        }
        Task::none()
    }

    pub(in crate::app) fn handle_window_opened(&mut self, window_id: window::Id) -> Task<Message> {
        if window_id == self.main_window_id {
            return window::run(window_id, move |window| {
                let parent = window
                    .window_handle()
                    .map_err(|error| error.to_string())
                    .and_then(|handle| {
                        WindowSnapshot::capture(
                            handle.as_raw(),
                            window.display_handle().ok().map(|display| display.as_raw()),
                        )
                        .map_err(|error| error.to_string())
                    });

                Message::WindowSnapshotCaptured {
                    window_id,
                    host: parent.clone(),
                    parent,
                }
            });
        }

        if !self.processor_editor_windows.pending_contains(window_id) {
            return Task::none();
        }

        window::run(window_id, move |window| {
            let host = window
                .window_handle()
                .map_err(|error| error.to_string())
                .and_then(|handle| {
                    WindowSnapshot::capture(
                        handle.as_raw(),
                        window.display_handle().ok().map(|display| display.as_raw()),
                    )
                    .map_err(|error| error.to_string())
                });
            Message::WindowSnapshotCaptured {
                window_id,
                host,
                parent: Err("editor host is installed in app state".to_string()),
            }
        })
    }

    pub(in crate::app) fn handle_window_closed(&mut self, window_id: window::Id) -> Task<Message> {
        let deferred = match self.pending_mixer_message_after_editor_close.take() {
            Some((pending_window_id, message)) if pending_window_id == window_id => Some(message),
            other => {
                self.pending_mixer_message_after_editor_close = other;
                None
            }
        };
        if let Some(removed) = self.processor_editor_windows.remove_window(window_id)
            && let Err(error) = removed.detach()
        {
            self.log_processor_editor_error("detach", error);
        }
        if let Some(message) = deferred {
            self.defer_mixer_message_after_editor_detach(message);
        }
        Task::none()
    }

    pub(in crate::app) fn handle_processor_editor_attached(
        &mut self,
        window_id: window::Id,
        host: Result<WindowSnapshot, String>,
        parent: Result<WindowSnapshot, String>,
    ) -> Task<Message> {
        if window_id == self.main_window_id {
            self.handle_main_window_attached(parent);
            return Task::none();
        }

        let Some(host) = self.captured_processor_host_window(host) else {
            return Task::none();
        };
        let Some(installed_host) = self.install_processor_editor_host(window_id, &host) else {
            return Task::none();
        };
        let Some(parent) = self.processor_editor_parent_from_host(&installed_host) else {
            return Task::none();
        };
        self.attach_processor_editor_host(window_id, installed_host, parent);
        Task::none()
    }

    pub(super) fn handle_main_window_attached(&mut self, parent: Result<WindowSnapshot, String>) {
        match parent {
            Ok(snapshot) => self.store_main_window_snapshot(snapshot),
            Err(error) => {
                self.log_processor_editor_error("capture main window", error);
                self.main_window_snapshot = None;
            }
        }
    }

    pub(super) fn store_main_window_snapshot(&mut self, snapshot: WindowSnapshot) {
        if let Err(error) = route_app_quit_to_window_close(&snapshot) {
            self.log_processor_editor_error("route app quit", error);
        }
        self.main_window_snapshot = Some(snapshot);
    }

    pub(super) fn captured_processor_host_window(
        &mut self,
        host: Result<WindowSnapshot, String>,
    ) -> Option<WindowSnapshot> {
        match host {
            Ok(host) => Some(host),
            Err(error) => {
                self.log_processor_editor_error("capture host window", error);
                None
            }
        }
    }

    pub(super) fn install_processor_editor_host(
        &mut self,
        window_id: window::Id,
        host: &WindowSnapshot,
    ) -> Option<InstalledHost> {
        let resizable = self
            .processor_editor_windows
            .window_resizable(window_id)
            .unwrap_or(true);
        let title = self
            .processor_editor_windows
            .window_title(window_id)
            .unwrap_or("Processor Editor")
            .to_string();

        let mut options = EditorHostOptions::new(title).with_resizable(resizable);
        if let Some(owner) = self.main_window_snapshot {
            options = options.with_owner(owner);
        }

        let mut frame = crate::app::AppEditorFrame::from_theme(&self.theme);
        if let Some((controller, native_editor_available, controls_visible)) = self
            .processor_editor_windows
            .frame_controller_for_window(window_id)
        {
            frame = frame.with_generic_controls(
                native_editor_available,
                controls_visible,
                crate::app::GenericControllerEditor::new(controller),
            );
        }

        match editor_host::install_editor_host(host, &options, frame) {
            Ok(host) => Some(host),
            Err(error) => {
                self.log_processor_editor_error("install host", error);
                None
            }
        }
    }

    pub(super) fn processor_editor_parent_from_host(
        &mut self,
        installed_host: &InstalledHost,
    ) -> Option<EditorParent> {
        match snapshot_into_editor_parent(installed_host.content()) {
            Ok(parent) => Some(parent),
            Err(error) => {
                self.log_processor_editor_error("capture content window", error);
                None
            }
        }
    }

    pub(super) fn attach_processor_editor_host(
        &mut self,
        window_id: window::Id,
        installed_host: InstalledHost,
        parent: EditorParent,
    ) {
        if let Err(error) =
            self.processor_editor_windows
                .attach(window_id, Some(installed_host), parent)
        {
            self.log_processor_editor_error("attach", error);
        } else if let Some(target) = self.processor_editor_windows.target_for_window(window_id) {
            self.refresh_editor_preset_state(target, None);
        }
    }

    pub(in crate::app) fn handle_processor_editor_close_requested(
        &mut self,
        window_id: window::Id,
    ) -> Task<Message> {
        let Some((_target, errors)) = self.processor_editor_windows.hide_window(window_id) else {
            return Task::none();
        };
        for error in errors {
            self.log_processor_editor_error("hide", error);
        }
        Task::none()
    }

    pub(in crate::app) fn handle_processor_editor_focused(
        &mut self,
        window_id: window::Id,
    ) -> Task<Message> {
        for error in self.processor_editor_windows.focus_window(window_id) {
            self.log_processor_editor_error("raise", error);
        }
        Task::none()
    }

    pub(in crate::app) fn handle_primary_mouse_pressed(&mut self, pressed: bool) -> Task<Message> {
        self.primary_mouse_pressed = pressed;
        if pressed {
            self.begin_effect_drag_from_hover();
            return Task::none();
        }

        let effect_drag_task = self.finish_effect_drag(None);
        self.commit_pending_mixer_history();
        if self.renaming_target.is_some() {
            return Task::batch([
                effect_drag_task,
                iced::widget::operation::is_focused(super::super::TRACK_RENAME_INPUT_ID)
                    .map(Message::TrackRenameFocusChanged),
            ]);
        }
        effect_drag_task
    }
}
