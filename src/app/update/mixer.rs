use editor_host::{
    EditorFrameCommand, EditorHostOptions, EditorPresetItem, EditorPresetOrigin, EditorPresetState,
    WindowSnapshot, route_app_quit_to_window_close,
};
use lilypalooza_audio::{BUILTIN_SOUNDFONT_ID, BusId, ProcessorKind, SlotState, TrackId};
use lilypalooza_builtins::soundfont_synth::{self, SoundfontProcessorState};

use super::super::messages::MixerMessage;
use super::*;
use crate::app::processor_editor_windows::{EditorTarget, snapshot_into_editor_parent};
use iced::window;

impl Lilypalooza {
    pub(in crate::app) fn log_processor_editor_error(
        &mut self,
        action: &str,
        error: impl std::fmt::Display,
    ) {
        self.logger
            .push(format!("Processor editor {action} failed: {error}"));
    }

    pub(in crate::app) fn destroy_editor_target(&mut self, target: EditorTarget) -> Task<Message> {
        let Some((window_id, mut session)) = self.processor_editor_windows.remove_target(target)
        else {
            return Task::none();
        };
        if let Err(error) = session.detach() {
            self.log_processor_editor_error("detach", error);
        }
        window::close(window_id)
    }

    pub(in crate::app) fn destroy_all_editor_windows(&mut self) -> Task<Message> {
        let removed = self.processor_editor_windows.remove_all_windows();
        if removed.is_empty() {
            return Task::none();
        }

        let mut tasks = Vec::with_capacity(removed.len());
        for (window_id, mut session) in removed {
            if let Err(error) = session.detach() {
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
        if let Some((_target, mut session)) = self.processor_editor_windows.remove_window(window_id)
            && let Err(error) = session.detach()
        {
            self.log_processor_editor_error("detach", error);
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
            match parent {
                Ok(snapshot) => {
                    if let Err(error) = route_app_quit_to_window_close(&snapshot) {
                        self.log_processor_editor_error("route app quit", error);
                    }
                    self.main_window_snapshot = Some(snapshot);
                }
                Err(error) => {
                    self.log_processor_editor_error("capture main window", error);
                    self.main_window_snapshot = None;
                }
            }
            return Task::none();
        }

        let host = match host {
            Ok(host) => host,
            Err(error) => {
                self.log_processor_editor_error("capture host window", error);
                return Task::none();
            }
        };
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
        let installed_host = match editor_host::install_editor_host(
            &host,
            &options,
            crate::app::AppEditorFrame::from_theme(&self.theme),
        ) {
            Ok(host) => host,
            Err(error) => {
                self.log_processor_editor_error("install host", error);
                return Task::none();
            }
        };
        let parent = match snapshot_into_editor_parent(installed_host.content()) {
            Ok(parent) => parent,
            Err(error) => {
                self.log_processor_editor_error("capture content window", error);
                return Task::none();
            }
        };
        if let Err(error) =
            self.processor_editor_windows
                .attach(window_id, Some(installed_host), parent)
        {
            self.log_processor_editor_error("attach", error);
        } else if let Some(target) = self.processor_editor_windows.target_for_window(window_id) {
            self.refresh_editor_preset_state(target, None);
        }
        Task::none()
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

    pub(in crate::app) fn handle_primary_mouse_pressed(&mut self, pressed: bool) -> Task<Message> {
        self.primary_mouse_pressed = pressed;
        if !pressed {
            self.commit_pending_mixer_history();
            if self.renaming_target.is_some() {
                return iced::widget::operation::is_focused(super::super::TRACK_RENAME_INPUT_ID)
                    .map(Message::TrackRenameFocusChanged);
            }
        }
        Task::none()
    }

    pub(in crate::app) fn undo_mixer_operation(&mut self) -> Task<Message> {
        let close_editors = self.destroy_all_editor_windows();
        self.pending_mixer_undo_snapshot = None;
        let Some(previous) = self.mixer_undo_stack.pop() else {
            return close_editors;
        };
        let Some(playback) = self.playback.as_mut() else {
            return close_editors;
        };

        let current = playback.mixer_state().clone();
        let restored_state = previous.clone();
        let mut mixer = playback.mixer();
        if mixer.replace_state(previous).is_ok() {
            self.mixer_redo_stack.push(current);
            if let Err(error) =
                sync_piano_roll_mix_from_mixer_state(&mut self.piano_roll, &restored_state)
            {
                self.logger.push(format!("Piano roll sync failed: {error}"));
            }
        }
        close_editors
    }

    pub(in crate::app) fn redo_mixer_operation(&mut self) -> Task<Message> {
        let close_editors = self.destroy_all_editor_windows();
        self.pending_mixer_undo_snapshot = None;
        let Some(next) = self.mixer_redo_stack.pop() else {
            return close_editors;
        };
        let Some(playback) = self.playback.as_mut() else {
            return close_editors;
        };

        let current = playback.mixer_state().clone();
        let restored_state = next.clone();
        let mut mixer = playback.mixer();
        if mixer.replace_state(next).is_ok() {
            self.mixer_undo_stack.push(current);
            if let Err(error) =
                sync_piano_roll_mix_from_mixer_state(&mut self.piano_roll, &restored_state)
            {
                self.logger.push(format!("Piano roll sync failed: {error}"));
            }
        }
        close_editors
    }

    pub(in crate::app) fn handle_mixer_message(&mut self, message: MixerMessage) -> Task<Message> {
        if !matches!(
            message,
            MixerMessage::SelectTrack(_)
                | MixerMessage::InstrumentViewportScrolled(_)
                | MixerMessage::BusViewportScrolled(_)
        ) {
            self.set_focused_workspace_pane(WorkspacePaneKind::Mixer);
        }

        let history_mode = mixer_message_history_mode(&message, self.primary_mouse_pressed);
        match history_mode {
            MixerHistoryMode::None => {}
            MixerHistoryMode::Immediate => {
                self.commit_pending_mixer_history();
                if let Some(snapshot) = self
                    .playback
                    .as_ref()
                    .map(|playback| playback.mixer_state().clone())
                {
                    self.mixer_undo_stack.push(snapshot);
                    self.mixer_redo_stack.clear();
                }
            }
            MixerHistoryMode::Gesture => {
                if self.pending_mixer_undo_snapshot.is_none()
                    && let Some(snapshot) = self
                        .playback
                        .as_ref()
                        .map(|playback| playback.mixer_state().clone())
                {
                    self.pending_mixer_undo_snapshot = Some(snapshot);
                }
            }
        }

        match message {
            MixerMessage::SelectTrack(track_index) => {
                return self.select_track(
                    track_index,
                    super::track_selection::TrackSelectionOrigin::Mixer,
                );
            }
            MixerMessage::ToggleTrackInstrumentBrowser(track_index) => {
                if self.open_instrument_browser_track == Some(track_index) {
                    self.close_track_instrument_browser();
                    return Task::none();
                }
                self.open_instrument_browser_track = Some(track_index);
                self.instrument_browser_backend =
                    super::super::mixer::InstrumentBrowserBackend::BuiltIn;
                self.instrument_browser_search.clear();
                return iced::widget::operation::focus(
                    self.instrument_browser_search_input_id.clone(),
                );
            }
            MixerMessage::CloseTrackInstrumentBrowser => {
                self.close_track_instrument_browser();
                return Task::none();
            }
            MixerMessage::InstrumentBrowserSearchChanged(value) => {
                self.instrument_browser_search = value;
                return Task::none();
            }
            MixerMessage::SelectInstrumentBrowserBackend(backend) => {
                self.instrument_browser_backend = backend;
                return Task::none();
            }
            MixerMessage::StartTrackRename(track_index) => {
                return self.start_track_rename(track_index, WorkspacePaneKind::Mixer);
            }
            MixerMessage::StartBusRename(bus_id) => {
                let Some(name) = self
                    .playback
                    .as_ref()
                    .and_then(|playback| playback.mixer_state().bus(BusId(bus_id)).ok())
                    .map(|bus| bus.name.clone())
                else {
                    return Task::none();
                };
                return self.start_bus_rename(bus_id, WorkspacePaneKind::Mixer, name);
            }
            MixerMessage::OpenEditor(target) => {
                return self.open_editor_target(target);
            }
            MixerMessage::TrackRenameInputChanged(value) => {
                self.update_track_rename_value(value);
                return Task::none();
            }
            MixerMessage::OpenTrackColorPicker => {
                self.open_track_color_picker();
                return Task::none();
            }
            MixerMessage::SubmitTrackColor(color) => {
                self.submit_track_color(color);
                return Task::none();
            }
            MixerMessage::PreviewTrackColor(color) => {
                self.preview_track_color(color);
                return Task::none();
            }
            MixerMessage::CommitTrackRename => return self.commit_track_rename(),
            MixerMessage::CancelTrackRename => {
                self.cancel_track_rename();
                return Task::none();
            }
            _ => {}
        }

        if matches!(message, MixerMessage::SelectTrackInstrument(_, _)) {
            self.close_track_instrument_browser();
        }

        let editor_cleanup = match &message {
            MixerMessage::AddBus | MixerMessage::RemoveBus(_) => self.destroy_all_editor_windows(),
            MixerMessage::SelectTrackInstrument(index, _) => {
                self.destroy_editor_target(EditorTarget {
                    strip_index: index + 1,
                    slot_index: 0,
                })
            }
            _ => Task::none(),
        };

        let Some(playback) = self.playback.as_mut() else {
            return editor_cleanup;
        };
        let mut editor_target_to_open = None;
        let mut mixer_error = None;

        {
            let mut mixer = playback.mixer();

            match message {
                MixerMessage::AddBus => {
                    if let Err(error) = mixer.add_bus(format!("Bus {}", mixer.bus_count() + 1)) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::RemoveBus(id) => {
                    if let Err(error) = mixer.remove_bus(BusId(id)) {
                        mixer_error = Some(error.to_string());
                    } else {
                        self.piano_roll
                            .set_global_solo_active(mixer_has_any_solo(&mixer));
                    }
                }
                MixerMessage::InstrumentViewportScrolled(viewport) => {
                    self.mixer_instrument_scroll_x = viewport.absolute_offset().x;
                    self.mixer_instrument_viewport_width = viewport.bounds().width;
                }
                MixerMessage::BusViewportScrolled(viewport) => {
                    self.mixer_bus_scroll_x = viewport.absolute_offset().x;
                    self.mixer_bus_viewport_width = viewport.bounds().width;
                }
                MixerMessage::ResetMasterMeter => mixer.reset_master_meter(),
                MixerMessage::SetMasterGain(gain) => mixer.set_master_gain_db(gain),
                MixerMessage::SetMasterPan(pan) => mixer.set_master_pan(pan),
                MixerMessage::ResetTrackMeter(index) => {
                    if let Err(error) = mixer.reset_track_meter(TrackId(index as u16)) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::SetTrackGain(index, gain) => {
                    if let Err(error) = mixer.set_track_gain_db(TrackId(index as u16), gain) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::SetTrackPan(index, pan) => {
                    if let Err(error) = mixer.set_track_pan(TrackId(index as u16), pan) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::ToggleTrackMute(index) => {
                    let next = mixer
                        .track(TrackId(index as u16))
                        .map(|track| !track.state.muted)
                        .unwrap_or(false);
                    if let Err(error) = mixer.set_track_muted(TrackId(index as u16), next) {
                        mixer_error = Some(error.to_string());
                    } else {
                        self.piano_roll.set_track_muted(index, next);
                    }
                }
                MixerMessage::ToggleTrackSolo(index) => {
                    let next = mixer
                        .track(TrackId(index as u16))
                        .map(|track| !track.state.soloed)
                        .unwrap_or(false);
                    if let Err(error) = mixer.set_track_soloed(TrackId(index as u16), next) {
                        mixer_error = Some(error.to_string());
                    } else {
                        self.piano_roll.set_track_soloed(index, next);
                        self.piano_roll
                            .set_global_solo_active(mixer_has_any_solo(&mixer));
                    }
                }
                MixerMessage::SelectTrackInstrument(index, choice) => {
                    let open_editor_after_select = matches!(
                        &choice,
                        super::super::mixer::InstrumentChoice::Processor { .. }
                    );
                    let slot = match choice {
                        super::super::mixer::InstrumentChoice::None => SlotState::default(),
                        super::super::mixer::InstrumentChoice::Processor {
                            ref processor_id,
                            ..
                        } => default_track_instrument_slot(
                            &mixer,
                            TrackId(index as u16),
                            processor_id.as_str(),
                        ),
                    };
                    if let Err(error) = mixer.set_track_instrument(TrackId(index as u16), slot) {
                        mixer_error = Some(error.to_string());
                    } else if open_editor_after_select {
                        editor_target_to_open = Some(EditorTarget {
                            strip_index: index + 1,
                            slot_index: 0,
                        });
                    }
                }
                MixerMessage::ResetBusMeter(id) => {
                    if let Err(error) = mixer.reset_bus_meter(BusId(id)) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::SetBusGain(id, gain) => {
                    if let Err(error) = mixer.set_bus_gain_db(BusId(id), gain) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::SetBusPan(id, pan) => {
                    if let Err(error) = mixer.set_bus_pan(BusId(id), pan) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::ToggleBusMute(id) => {
                    let next = mixer
                        .bus(BusId(id))
                        .map(|bus| !bus.state.muted)
                        .unwrap_or(false);
                    if let Err(error) = mixer.set_bus_muted(BusId(id), next) {
                        mixer_error = Some(error.to_string());
                    }
                }
                MixerMessage::ToggleBusSolo(id) => {
                    let next = mixer
                        .bus(BusId(id))
                        .map(|bus| !bus.state.soloed)
                        .unwrap_or(false);
                    if let Err(error) = mixer.set_bus_soloed(BusId(id), next) {
                        mixer_error = Some(error.to_string());
                    } else {
                        self.piano_roll
                            .set_global_solo_active(mixer_has_any_solo(&mixer));
                    }
                }
                MixerMessage::SelectTrack(_) => {}
                MixerMessage::StartTrackRename(_)
                | MixerMessage::ToggleTrackInstrumentBrowser(_)
                | MixerMessage::CloseTrackInstrumentBrowser
                | MixerMessage::InstrumentBrowserSearchChanged(_)
                | MixerMessage::SelectInstrumentBrowserBackend(_)
                | MixerMessage::StartBusRename(_)
                | MixerMessage::OpenEditor(_)
                | MixerMessage::TrackRenameInputChanged(_)
                | MixerMessage::OpenTrackColorPicker
                | MixerMessage::SubmitTrackColor(_)
                | MixerMessage::PreviewTrackColor(_)
                | MixerMessage::CommitTrackRename => {}
                MixerMessage::CancelTrackRename => {}
            }
        }

        if let Some(error) = mixer_error {
            self.logger.push(format!("Mixer update failed: {error}"));
        }
        self.project_mixer_state = playback.mixer_state().clone();
        if let Some(target) = editor_target_to_open {
            return Task::batch([editor_cleanup, self.open_editor_target(target)]);
        }
        editor_cleanup
    }
}

impl Lilypalooza {
    fn commit_pending_mixer_history(&mut self) {
        if let Some(snapshot) = self.pending_mixer_undo_snapshot.take() {
            self.mixer_undo_stack.push(snapshot);
            self.mixer_redo_stack.clear();
        }
    }
}

fn mixer_has_any_solo(mixer: &lilypalooza_audio::MixerHandle<'_>) -> bool {
    mixer.tracks().iter().any(|track| track.state.soloed)
        || mixer.buses().iter().any(|bus| bus.state.soloed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MixerHistoryMode {
    None,
    Immediate,
    Gesture,
}

fn mixer_message_history_mode(
    message: &MixerMessage,
    primary_mouse_pressed: bool,
) -> MixerHistoryMode {
    match message {
        MixerMessage::ResetMasterMeter
        | MixerMessage::ResetTrackMeter(_)
        | MixerMessage::ResetBusMeter(_)
        | MixerMessage::InstrumentViewportScrolled(_)
        | MixerMessage::BusViewportScrolled(_)
        | MixerMessage::OpenTrackColorPicker
        | MixerMessage::ToggleTrackInstrumentBrowser(_)
        | MixerMessage::CloseTrackInstrumentBrowser
        | MixerMessage::InstrumentBrowserSearchChanged(_)
        | MixerMessage::SelectInstrumentBrowserBackend(_)
        | MixerMessage::OpenEditor(_)
        | MixerMessage::PreviewTrackColor(_) => MixerHistoryMode::None,
        MixerMessage::SetMasterGain(_)
        | MixerMessage::SetMasterPan(_)
        | MixerMessage::SetTrackGain(_, _)
        | MixerMessage::SetTrackPan(_, _)
        | MixerMessage::SetBusGain(_, _)
        | MixerMessage::SetBusPan(_, _) => {
            if primary_mouse_pressed {
                MixerHistoryMode::Gesture
            } else {
                MixerHistoryMode::Immediate
            }
        }
        MixerMessage::AddBus
        | MixerMessage::RemoveBus(_)
        | MixerMessage::SelectTrack(_)
        | MixerMessage::StartTrackRename(_)
        | MixerMessage::StartBusRename(_)
        | MixerMessage::TrackRenameInputChanged(_)
        | MixerMessage::CancelTrackRename
        | MixerMessage::CommitTrackRename
        | MixerMessage::SubmitTrackColor(_)
        | MixerMessage::ToggleTrackMute(_)
        | MixerMessage::ToggleTrackSolo(_)
        | MixerMessage::SelectTrackInstrument(_, _)
        | MixerMessage::ToggleBusMute(_)
        | MixerMessage::ToggleBusSolo(_) => MixerHistoryMode::Immediate,
    }
}

impl Lilypalooza {
    pub(in crate::app) fn editor_window_title(
        &self,
        strip_name: &str,
        slot: &SlotState,
        slot_index: usize,
    ) -> String {
        if slot_index != 0 {
            return slot.title(strip_name, slot_index);
        }

        let Some(descriptor) = slot.descriptor() else {
            return slot.title(strip_name, slot_index);
        };

        format!("{strip_name}: {}", descriptor.name)
    }

    fn close_track_instrument_browser(&mut self) {
        self.open_instrument_browser_track = None;
        self.instrument_browser_search.clear();
    }

    fn open_editor_target(&mut self, target: EditorTarget) -> Task<Message> {
        if let Some(window_id) = self.processor_editor_windows.focus_existing(target) {
            return if self.processor_editor_windows.pending_contains(window_id) {
                Task::none()
            } else if self.processor_editor_windows.window_visible(window_id) {
                self.handle_processor_editor_close_requested(window_id)
            } else {
                for error in self.processor_editor_windows.show_window(window_id) {
                    self.log_processor_editor_error("show", error);
                }
                window::gain_focus(window_id)
            };
        }

        let Some(playback) = self.playback.as_ref() else {
            return Task::none();
        };
        let Some(strip) = playback.mixer_state().strip_by_index(target.strip_index) else {
            return Task::none();
        };
        let Some(slot) = strip.slot(target.slot_index) else {
            return Task::none();
        };
        let Ok(Some(controller)) = playback.controller(lilypalooza_audio::SlotAddress {
            strip_index: target.strip_index,
            slot_index: target.slot_index,
        }) else {
            return Task::none();
        };
        let title = self.editor_window_title(&strip.name, slot, target.slot_index);
        let descriptor = controller.descriptor().editor;
        let session_result = controller.create_editor_session();

        self.open_editor(target, title, descriptor, session_result)
    }

    fn open_editor(
        &mut self,
        target: EditorTarget,
        title: String,
        descriptor: Option<lilypalooza_audio::EditorDescriptor>,
        session_result: Result<
            Option<Box<dyn lilypalooza_audio::EditorSession>>,
            lilypalooza_audio::EditorError,
        >,
    ) -> Task<Message> {
        let Some(descriptor) = descriptor else {
            return Task::none();
        };
        let Ok(Some(session)) = session_result else {
            return Task::none();
        };

        let (window_id, open_task) = window::open(window::Settings {
            size: Size::new(
                descriptor.default_size.width as f32,
                descriptor.default_size.height as f32,
            ),
            min_size: descriptor
                .min_size
                .map(|size| Size::new(size.width as f32, size.height as f32)),
            resizable: descriptor.resizable,
            closeable: true,
            minimizable: false,
            decorations: false,
            exit_on_close_request: false,
            ..window::Settings::default()
        });
        self.processor_editor_windows.begin_open(
            target,
            title,
            descriptor.resizable,
            session,
            window_id,
        );
        open_task.map(|_| Message::Noop)
    }

    pub(in crate::app) fn handle_processor_editor_frame_command(
        &mut self,
        target: EditorTarget,
        command: EditorFrameCommand,
    ) {
        match command {
            EditorFrameCommand::PreviousPreset => self.step_processor_preset(target, -1),
            EditorFrameCommand::NextPreset => self.step_processor_preset(target, 1),
            EditorFrameCommand::LoadPreset(id) => self.load_processor_preset_for_target(target, id),
            EditorFrameCommand::RenamePreset { id, name } => {
                self.rename_processor_preset_for_target(target, id, name);
            }
            EditorFrameCommand::DeletePreset(id) => {
                self.delete_processor_preset_for_target(target, id);
            }
            EditorFrameCommand::SavePreset => self.save_processor_preset_from_target(target),
            EditorFrameCommand::TogglePresetBrowser => {
                self.expanded_processor_preset_browser =
                    (self.expanded_processor_preset_browser != Some(target)).then_some(target);
                let selected_id = self
                    .processor_editor_windows
                    .preset_state(target)
                    .and_then(|state| state.selected_id);
                self.refresh_editor_preset_state(target, selected_id);
            }
        }
    }

    fn processor_kind_for_target(&self, target: EditorTarget) -> Option<ProcessorKind> {
        self.playback
            .as_ref()
            .and_then(|playback| {
                playback
                    .mixer_state()
                    .strip_by_index(target.strip_index)
                    .and_then(|strip| strip.slot(target.slot_index))
            })
            .map(|slot| slot.kind.clone())
    }

    fn refresh_editor_preset_state(&mut self, target: EditorTarget, selected_id: Option<String>) {
        let Some(kind) = self.processor_kind_for_target(target) else {
            self.processor_editor_windows.set_preset_state(target, None);
            return;
        };
        let items = self
            .processor_presets
            .presets_for(&kind)
            .into_iter()
            .map(|preset| EditorPresetItem {
                id: preset.id.clone(),
                name: preset.name.clone(),
                origin: EditorPresetOrigin::User,
            })
            .collect::<Vec<_>>();
        let current_name = selected_id
            .as_deref()
            .and_then(|id| items.iter().find(|item| item.id == id))
            .map(|item| item.name.clone())
            .unwrap_or_else(|| "Preset".to_string());
        let expanded = self.expanded_processor_preset_browser == Some(target);
        self.processor_editor_windows.set_preset_state(
            target,
            Some(EditorPresetState {
                current_name,
                selected_id,
                expanded,
                items,
            }),
        );
    }

    fn save_processor_preset_from_target(&mut self, target: EditorTarget) {
        let Some(playback) = self.playback.as_ref() else {
            return;
        };
        let Some(kind) = self.processor_kind_for_target(target) else {
            return;
        };
        let Ok(Some(controller)) = playback.controller(lilypalooza_audio::SlotAddress {
            strip_index: target.strip_index,
            slot_index: target.slot_index,
        }) else {
            return;
        };
        let Ok(state) = controller.save_state() else {
            return;
        };
        let name = format!(
            "User Preset {}",
            self.processor_presets.presets_for(&kind).len() + 1
        );
        let id = self.processor_presets.save_user_preset(name, kind, state);
        self.refresh_editor_preset_state(target, Some(id));
        self.persist_settings();
    }

    fn rename_processor_preset_for_target(
        &mut self,
        target: EditorTarget,
        id: String,
        name: String,
    ) {
        let Some(kind) = self.processor_kind_for_target(target) else {
            return;
        };
        if self.processor_presets.rename_user_preset(&kind, &id, name) {
            self.refresh_editor_preset_state(target, Some(id));
            self.persist_settings();
        }
    }

    fn delete_processor_preset_for_target(&mut self, target: EditorTarget, id: String) {
        let Some(kind) = self.processor_kind_for_target(target) else {
            return;
        };
        if self.processor_presets.delete_user_preset(&kind, &id) {
            let selected_id = self
                .processor_editor_windows
                .preset_state(target)
                .and_then(|state| state.selected_id)
                .filter(|selected| selected != &id);
            self.refresh_editor_preset_state(target, selected_id);
            self.persist_settings();
        }
    }

    fn load_processor_preset_for_target(&mut self, target: EditorTarget, id: String) {
        let Some(kind) = self.processor_kind_for_target(target) else {
            return;
        };
        let Some(state) = self.processor_presets.state_for(&kind, &id).cloned() else {
            return;
        };
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let Ok(Some(controller)) = playback.controller(lilypalooza_audio::SlotAddress {
            strip_index: target.strip_index,
            slot_index: target.slot_index,
        }) else {
            return;
        };
        if controller.load_state(&state).is_err() {
            return;
        }
        if target.slot_index == 0 && target.strip_index > 0 {
            let track_id = TrackId((target.strip_index - 1) as u16);
            let next_slot = playback
                .mixer_state()
                .track(track_id)
                .ok()
                .and_then(|track| track.instrument_slot())
                .cloned()
                .map(|mut slot| {
                    slot.state = state;
                    slot
                });
            if let Some(slot) = next_slot
                && playback
                    .mixer()
                    .set_track_instrument(track_id, slot)
                    .is_ok()
            {
                self.project_mixer_state = playback.mixer_state().clone();
            }
        }
        self.refresh_editor_preset_state(target, Some(id));
    }

    fn step_processor_preset(&mut self, target: EditorTarget, direction: isize) {
        let Some(kind) = self.processor_kind_for_target(target) else {
            return;
        };
        let presets = self.processor_presets.presets_for(&kind);
        if presets.is_empty() {
            return;
        }
        let current_id = self
            .processor_editor_windows
            .preset_state(target)
            .and_then(|state| state.selected_id);
        let next_index = current_id
            .as_deref()
            .and_then(|id| presets.iter().position(|preset| preset.id == id))
            .map_or_else(
                || {
                    if direction >= 0 { 0 } else { presets.len() - 1 }
                },
                |current_index| {
                    (current_index as isize + direction).rem_euclid(presets.len() as isize) as usize
                },
            );
        let id = presets[next_index].id.clone();
        self.load_processor_preset_for_target(target, id);
    }
}

fn default_track_instrument_slot(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    track_id: TrackId,
    processor_id: &str,
) -> SlotState {
    if processor_id == BUILTIN_SOUNDFONT_ID {
        let current_state = mixer
            .track(track_id)
            .ok()
            .and_then(|track| track.instrument_slot())
            .and_then(|slot| {
                slot.decode_built_in(BUILTIN_SOUNDFONT_ID, soundfont_synth::decode_state)
                    .ok()
                    .flatten()
            });
        let soundfont_id = current_state
            .as_ref()
            .map(|state| state.soundfont_id.clone())
            .or_else(|| {
                mixer
                    .soundfonts()
                    .first()
                    .map(|soundfont| soundfont.id.clone())
            })
            .unwrap_or_else(|| SoundfontProcessorState::default().soundfont_id);
        let bank = current_state.as_ref().map_or(0, |state| state.bank);
        let program = current_state.as_ref().map_or(0, |state| state.program);

        return SlotState::built_in(
            BUILTIN_SOUNDFONT_ID,
            soundfont_synth::encode_state(&SoundfontProcessorState {
                soundfont_id,
                bank,
                program,
                ..SoundfontProcessorState::default()
            }),
        );
    }

    SlotState::built_in(processor_id, lilypalooza_audio::ProcessorState::default())
}

fn sync_piano_roll_mix_from_mixer_state(
    piano_roll: &mut super::super::piano_roll::PianoRollState,
    mixer: &lilypalooza_audio::MixerState,
) -> Result<(), String> {
    for (track_id, track) in mixer.tracks_with_ids() {
        let index = track_id.index();
        piano_roll
            .set_track_muted(index, track.state.muted)
            .ok_or_else(|| format!("track {index} is missing"))?;
        piano_roll
            .set_track_soloed(index, track.state.soloed)
            .ok_or_else(|| format!("track {index} is missing"))?;
    }
    piano_roll.set_global_solo_active(
        mixer.tracks().iter().any(|track| track.state.soloed)
            || mixer.buses().iter().any(|bus| bus.state.soloed),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{Lilypalooza, MixerHistoryMode, mixer_message_history_mode};
    use crate::app::RenameTarget;
    use crate::app::messages::MixerMessage;
    use crate::app::processor_editor_windows::EditorTarget;
    use crate::state::ProjectState;
    use lilypalooza_audio::{
        AudioEngine, AudioEngineOptions, BusId, EditorError, EditorParent, EditorSession,
        EditorSize, MixerState,
    };
    use lilypalooza_builtins::soundfont_synth::{self, SoundfontProcessorState};

    struct FakeEditorSession;

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

        fn resize(&mut self, _size: EditorSize) -> Result<(), EditorError> {
            Ok(())
        }
    }

    struct RecordingEditorSession {
        visible: Rc<RefCell<Vec<bool>>>,
        detached: Rc<RefCell<usize>>,
    }

    impl EditorSession for RecordingEditorSession {
        fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
            Ok(())
        }

        fn detach(&mut self) -> Result<(), EditorError> {
            *self.detached.borrow_mut() += 1;
            Ok(())
        }

        fn set_visible(&mut self, visible: bool) -> Result<(), EditorError> {
            self.visible.borrow_mut().push(visible);
            Ok(())
        }

        fn resize(&mut self, _size: EditorSize) -> Result<(), EditorError> {
            Ok(())
        }
    }

    fn test_app() -> Lilypalooza {
        let (app, _task) = super::super::super::new_with_default_test_state();
        app
    }

    #[test]
    fn mixer_drag_value_changes_use_gesture_history() {
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::SetTrackGain(0, -3.0), true),
            MixerHistoryMode::Gesture
        );
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::SetTrackPan(0, 0.25), true),
            MixerHistoryMode::Gesture
        );
    }

    #[test]
    fn mixer_discrete_value_changes_use_immediate_history() {
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::SetTrackGain(0, -3.0), false),
            MixerHistoryMode::Immediate
        );
        assert_eq!(
            mixer_message_history_mode(
                &MixerMessage::SelectTrackInstrument(0, crate::app::mixer::InstrumentChoice::None,),
                false
            ),
            MixerHistoryMode::Immediate
        );
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::ToggleTrackInstrumentBrowser(0), false),
            MixerHistoryMode::None
        );
    }

    #[test]
    fn mixer_meter_resets_do_not_record_history() {
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::ResetTrackMeter(0), false),
            MixerHistoryMode::None
        );
    }

    #[test]
    fn track_rename_commits_on_focus_loss() {
        let mut app = test_app();
        app.renaming_target = Some(RenameTarget::Track(0));
        app.track_rename_was_focused = true;
        app.track_rename_value = "Lead".into();

        let _ = app.handle_track_rename_focus_changed(false);

        assert_eq!(app.track_name_override(0), Some("Lead"));
        assert!(app.renaming_target.is_none());
        assert!(app.track_rename_value.is_empty());
    }

    #[test]
    fn remove_bus_message_removes_bus() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );

        let _ = app.handle_mixer_message(MixerMessage::AddBus);
        let bus_id = app
            .playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .buses()
            .first()
            .and_then(|bus| bus.bus_id)
            .expect("bus should be added");

        let _ = app.handle_mixer_message(MixerMessage::RemoveBus(bus_id.0));

        assert!(
            app.playback
                .as_ref()
                .expect("playback should exist")
                .mixer_state()
                .bus(BusId(bus_id.0))
                .is_err()
        );
    }

    #[test]
    fn track_instrument_without_editor_does_not_open_processor_window() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );

        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let _ = app.handle_mixer_message(MixerMessage::OpenEditor(target));

        assert!(!app.processor_editor_windows.contains_window(target));
    }

    #[test]
    fn instrument_browser_toggle_opens_and_closes_same_track() {
        let mut app = test_app();

        let _ = app.handle_mixer_message(MixerMessage::ToggleTrackInstrumentBrowser(3));

        assert_eq!(app.open_instrument_browser_track, Some(3));
        assert_eq!(
            app.instrument_browser_backend,
            crate::app::mixer::InstrumentBrowserBackend::BuiltIn
        );
        assert!(app.instrument_browser_search.is_empty());

        let _ = app.handle_mixer_message(MixerMessage::ToggleTrackInstrumentBrowser(3));

        assert_eq!(app.open_instrument_browser_track, None);
    }

    #[test]
    fn selecting_track_instrument_closes_open_browser() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        app.open_instrument_browser_track = Some(0);
        app.instrument_browser_search = "piano".into();

        let _ = app.handle_mixer_message(MixerMessage::SelectTrackInstrument(
            0,
            crate::app::mixer::InstrumentChoice::None,
        ));

        assert_eq!(app.open_instrument_browser_track, None);
        assert!(app.instrument_browser_search.is_empty());
    }

    #[test]
    fn selecting_track_instrument_opens_editor_when_available() {
        lilypalooza_builtins::register_all();
        let mut app = test_app();
        let mut playback =
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start");
        playback
            .mixer()
            .set_soundfont(lilypalooza_audio::SoundfontResource {
                id: "default".to_string(),
                name: "FluidR3".to_string(),
                path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/soundfonts/FluidR3_GM.sf2")
                    .canonicalize()
                    .expect("test SoundFont should exist"),
            })
            .expect("test SoundFont should load");
        app.playback = Some(playback);

        let _ = app.handle_mixer_message(MixerMessage::SelectTrackInstrument(
            0,
            crate::app::mixer::InstrumentChoice::Processor {
                processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SF-01".to_string(),
                backend: crate::app::mixer::InstrumentBrowserBackend::BuiltIn,
            },
        ));

        assert!(
            app.processor_editor_windows
                .focus_existing(EditorTarget {
                    strip_index: 1,
                    slot_index: 0,
                })
                .is_some()
        );
    }

    fn app_with_soundfont_track() -> (Lilypalooza, EditorTarget) {
        lilypalooza_builtins::register_all();
        let mut app = test_app();
        let mut playback =
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start");
        playback
            .mixer()
            .set_soundfont(lilypalooza_audio::SoundfontResource {
                id: "default".to_string(),
                name: "FluidR3".to_string(),
                path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/soundfonts/FluidR3_GM.sf2")
                    .canonicalize()
                    .expect("test SoundFont should exist"),
            })
            .expect("test SoundFont should load");
        app.playback = Some(playback);
        let _ = app.handle_mixer_message(MixerMessage::SelectTrackInstrument(
            0,
            crate::app::mixer::InstrumentChoice::Processor {
                processor_id: lilypalooza_audio::BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SF-01".to_string(),
                backend: crate::app::mixer::InstrumentBrowserBackend::BuiltIn,
            },
        ));
        let temp = tempfile::tempdir().expect("temp project dir should exist");
        app.project_root = Some(temp.path().to_path_buf());
        (
            app,
            EditorTarget {
                strip_index: 1,
                slot_index: 0,
            },
        )
    }

    #[test]
    fn processor_frame_save_command_creates_user_preset_for_slot() {
        let (mut app, target) = app_with_soundfont_track();

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::SavePreset,
        );

        let kind = app.processor_kind_for_target(target).expect("slot kind");
        let presets = app.processor_presets.presets_for(&kind);
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "User Preset 1");
    }

    #[test]
    fn processor_frame_load_command_updates_slot_state() {
        let (mut app, target) = app_with_soundfont_track();
        let kind = app.processor_kind_for_target(target).expect("slot kind");
        let state = soundfont_synth::encode_state(&SoundfontProcessorState {
            program: 7,
            output_gain: 0.25,
            ..SoundfontProcessorState::default()
        });
        let id = app
            .processor_presets
            .save_user_preset("Muted Piano", kind, state.clone());

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::LoadPreset(id),
        );

        let slot_state = app
            .playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .strip_by_index(target.strip_index)
            .and_then(|strip| strip.slot(target.slot_index))
            .map(|slot| slot.state.clone());
        assert_eq!(slot_state, Some(state));
    }

    #[test]
    fn processor_frame_next_from_placeholder_loads_first_preset() {
        let (mut app, target) = app_with_soundfont_track();
        let kind = app.processor_kind_for_target(target).expect("slot kind");
        let first_state = soundfont_synth::encode_state(&SoundfontProcessorState {
            program: 1,
            ..SoundfontProcessorState::default()
        });
        let second_state = soundfont_synth::encode_state(&SoundfontProcessorState {
            program: 2,
            ..SoundfontProcessorState::default()
        });
        app.processor_presets
            .save_user_preset("First", kind.clone(), first_state.clone());
        app.processor_presets
            .save_user_preset("Second", kind, second_state);
        app.refresh_editor_preset_state(target, None);

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::NextPreset,
        );

        let slot_state = app
            .playback
            .as_ref()
            .expect("playback")
            .mixer_state()
            .strip_by_index(target.strip_index)
            .and_then(|strip| strip.slot(target.slot_index))
            .map(|slot| slot.state.clone());
        assert_eq!(slot_state, Some(first_state));
    }

    #[test]
    fn processor_frame_rename_command_updates_user_preset() {
        let (mut app, target) = app_with_soundfont_track();
        let kind = app.processor_kind_for_target(target).expect("slot kind");
        let id = app.processor_presets.save_user_preset(
            "Warm Piano",
            kind.clone(),
            lilypalooza_audio::ProcessorState(vec![]),
        );

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::RenamePreset {
                id: id.clone(),
                name: "Soft Piano".to_string(),
            },
        );

        assert_eq!(
            app.processor_presets.presets_for(&kind)[0].name,
            "Soft Piano"
        );
    }

    #[test]
    fn processor_frame_delete_command_removes_user_preset() {
        let (mut app, target) = app_with_soundfont_track();
        let kind = app.processor_kind_for_target(target).expect("slot kind");
        let id = app.processor_presets.save_user_preset(
            "Warm Piano",
            kind.clone(),
            lilypalooza_audio::ProcessorState(vec![]),
        );

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::DeletePreset(id),
        );

        assert!(app.processor_presets.presets_for(&kind).is_empty());
    }

    #[test]
    fn processor_frame_toggle_command_tracks_expanded_browser_target() {
        let (mut app, target) = app_with_soundfont_track();

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::TogglePresetBrowser,
        );

        assert_eq!(app.expanded_processor_preset_browser, Some(target));

        app.handle_processor_editor_frame_command(
            target,
            editor_host::EditorFrameCommand::TogglePresetBrowser,
        );

        assert_eq!(app.expanded_processor_preset_browser, None);
    }

    #[test]
    fn mixer_changes_mark_project_dirty() {
        let mut app = test_app();
        let temp = tempfile::tempdir().expect("temp dir should exist");
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        app.apply_project_state(temp.path().to_path_buf(), ProjectState::default());
        assert!(!app.project_is_dirty());

        let _ = app.handle_mixer_message(MixerMessage::AddBus);
        let bus_id = app
            .playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .buses()
            .first()
            .and_then(|bus| bus.bus_id.map(|id| id.0))
            .expect("bus should be added");
        let _ = app.handle_mixer_message(MixerMessage::SetBusGain(bus_id, -6.0));

        assert!(app.project_is_dirty());
    }

    #[test]
    fn unsaved_project_mixer_changes_prompt_on_close() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        app.saved_project_state = Some(app.current_project_state());

        let _ = app.handle_mixer_message(MixerMessage::AddBus);

        let _ = app.handle_window_close_requested(app.main_window_id);

        assert!(matches!(
            app.pending_editor_action,
            Some(crate::app::PendingEditorAction::ResolveDirtyProject {
                continuation: crate::app::EditorContinuation::ExitApp
            })
        ));
    }

    #[test]
    fn editor_window_close_request_hides_only_editor_window() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        app.saved_project_state = Some(app.current_project_state());
        let _ = app.handle_mixer_message(MixerMessage::AddBus);

        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let window_id = iced::window::Id::unique();
        app.processor_editor_windows.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );
        app.processor_editor_windows
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

        let _ = app.handle_window_close_requested(window_id);

        assert!(app.processor_editor_windows.contains_window(target));
        assert!(app.pending_editor_action.is_none());
    }

    #[test]
    fn main_window_close_request_hides_editors_before_prompt() {
        let mut app = test_app();
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        app.saved_project_state = Some(app.current_project_state());
        let _ = app.handle_mixer_message(MixerMessage::AddBus);

        let visible = Rc::new(RefCell::new(Vec::new()));
        let detached = Rc::new(RefCell::new(0));
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let window_id = iced::window::Id::unique();
        app.processor_editor_windows.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(RecordingEditorSession {
                visible: Rc::clone(&visible),
                detached: Rc::clone(&detached),
            }),
            window_id,
        );
        app.processor_editor_windows
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

        let _ = app.handle_window_close_requested(app.main_window_id);

        assert_eq!(*visible.borrow(), vec![false]);
        assert_eq!(*detached.borrow(), 0);
        assert!(app.processor_editor_windows.contains_window(target));
        assert!(matches!(
            app.pending_editor_action,
            Some(crate::app::PendingEditorAction::ResolveDirtyProject {
                continuation: crate::app::EditorContinuation::ExitApp
            })
        ));
    }

    #[test]
    fn exit_app_detaches_editor_sessions() {
        let mut app = test_app();
        let visible = Rc::new(RefCell::new(Vec::new()));
        let detached = Rc::new(RefCell::new(0));
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };
        let window_id = iced::window::Id::unique();
        app.processor_editor_windows.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(RecordingEditorSession {
                visible,
                detached: Rc::clone(&detached),
            }),
            window_id,
        );
        app.processor_editor_windows
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

        let _ = app.exit_app();

        assert_eq!(*detached.borrow(), 1);
        assert!(!app.processor_editor_windows.contains_window(target));
    }
}
