use super::*;

#[derive(Debug, Clone, Copy)]
enum MixerHistoryStep {
    Undo,
    Redo,
}

impl MixerHistoryStep {
    fn inverse(self) -> Self {
        match self {
            Self::Undo => Self::Redo,
            Self::Redo => Self::Undo,
        }
    }
}

type MixerUiRoute = fn(&mut Lilypalooza, &MixerMessage) -> Option<Task<Message>>;

const MIXER_UI_ROUTES: &[MixerUiRoute] = &[
    Lilypalooza::handle_mixer_browser_ui_message,
    Lilypalooza::handle_mixer_effect_drag_message,
    Lilypalooza::handle_mixer_rename_message,
    Lilypalooza::handle_mixer_color_message,
];

macro_rules! mixer_ui_route {
    (fn $name:ident($self:ident, $message:ident) {
        $($pattern:pat => $body:expr),+ $(,)?
    }) => {
        fn $name(&mut $self, $message: &MixerMessage) -> Option<Task<Message>> {
            match $message {
                $($pattern => Some($body),)+
                _ => None,
            }
        }
    };
}

impl Lilypalooza {
    mixer_ui_route! {
        fn handle_mixer_browser_ui_message(self, message) {
            MixerMessage::SelectTrack(track_index) => self.select_track(
                *track_index,
                super::track_selection::TrackSelectionOrigin::Mixer,
            ),
            MixerMessage::ToggleProcessorBrowser(target) => self.toggle_processor_browser(*target),
            MixerMessage::CloseProcessorBrowser => {
                self.close_processor_browser();
                Task::none()
            },
            MixerMessage::ProcessorBrowserSearchChanged(value) => {
                self.instrument_browser_search = value.clone();
                Task::none()
            },
            MixerMessage::ToggleProcessorBrowserSection(key) => {
                self.toggle_processor_browser_section(key);
                Task::none()
            },
            MixerMessage::ToggleMixerEffectRack(strip_index) => {
                self.toggle_mixer_effect_rack(*strip_index);
                Task::none()
            },
            MixerMessage::SetProcessorSlotHovered(target) => {
                self.set_processor_slot_hovered(*target);
                Task::none()
            },
        }
    }

    mixer_ui_route! {
        fn handle_mixer_effect_drag_lifecycle_message(self, message) {
            MixerMessage::StartTrackEffectDrag {
                strip_index,
                effect_index,
            } => {
                self.effect_drag_source = Some((*strip_index, *effect_index));
                self.effect_drag_target = Some((*strip_index, *effect_index));
                Task::none()
            },
            MixerMessage::DropTrackEffect {
                strip_index,
                effect_index,
            } => self.finish_effect_drag(Some((*strip_index, *effect_index))),
        }
    }

    mixer_ui_route! {
        fn handle_mixer_effect_drag_motion_message(self, message) {
            MixerMessage::TrackEffectDragMoved { strip_index, y } => {
                self.update_effect_rack_drag_position(*strip_index, *y);
                self.tick_effect_rack_autoscroll()
            },
            MixerMessage::EffectRackCursorLeft(strip_index) => {
                self.clear_effect_rack_hover_for_strip(*strip_index);
                self.clear_effect_rack_drag_for_strip(*strip_index);
                Task::none()
            },
            MixerMessage::EffectRackViewportScrolled {
                strip_index,
                viewport,
            } => {
                self.effect_rack_scroll_y
                    .insert(*strip_index, viewport.absolute_offset().y);
                self.effect_rack_viewport_height
                    .insert(*strip_index, viewport.bounds().height);
                Task::none()
            },
        }
    }

    mixer_ui_route! {
        fn handle_mixer_rename_message(self, message) {
            MixerMessage::StartTrackRename(track_index) => self.start_track_rename(*track_index, WorkspacePaneKind::Mixer),
            MixerMessage::StartBusRename(bus_id) => self.start_mixer_bus_rename(*bus_id),
            MixerMessage::TrackRenameInputChanged(value) => {
                self.update_track_rename_value(value.clone());
                Task::none()
            },
            MixerMessage::CommitTrackRename => self.commit_track_rename(),
            MixerMessage::CancelTrackRename => {
                self.cancel_track_rename();
                Task::none()
            },
        }
    }

    mixer_ui_route! {
        fn handle_mixer_color_message(self, message) {
            MixerMessage::OpenTrackColorPicker => {
                self.open_track_color_picker();
                Task::none()
            },
            MixerMessage::SubmitTrackColor(color) => {
                self.submit_track_color(*color);
                Task::none()
            },
            MixerMessage::PreviewTrackColor(color) => {
                self.preview_track_color(*color);
                Task::none()
            },
        }
    }

    pub(in crate::app) fn undo_mixer_operation(&mut self) -> Task<Message> {
        self.apply_mixer_history_step(MixerHistoryStep::Undo)
    }

    pub(in crate::app) fn redo_mixer_operation(&mut self) -> Task<Message> {
        self.apply_mixer_history_step(MixerHistoryStep::Redo)
    }

    fn apply_mixer_history_step(&mut self, step: MixerHistoryStep) -> Task<Message> {
        let close_editors = self.destroy_all_editor_windows();
        self.pending_mixer_undo_snapshot = None;
        let Some(restored) = self.pop_mixer_history_state(step) else {
            return close_editors;
        };
        let Some(playback) = self.playback.as_mut() else {
            return close_editors;
        };

        let current = playback.mixer_state().clone();
        let restored_state = restored.clone();
        let mut mixer = playback.mixer();
        if mixer.replace_state(restored).is_ok() {
            self.push_mixer_history_state(step.inverse(), current);
            if let Err(error) =
                sync_piano_roll_mix_from_mixer_state(&mut self.piano_roll, &restored_state)
            {
                self.logger.push(format!("Piano roll sync failed: {error}"));
            }
        }
        close_editors
    }

    fn pop_mixer_history_state(&mut self, step: MixerHistoryStep) -> Option<MixerState> {
        match step {
            MixerHistoryStep::Undo => self.mixer_undo_stack.pop(),
            MixerHistoryStep::Redo => self.mixer_redo_stack.pop(),
        }
    }

    fn push_mixer_history_state(&mut self, step: MixerHistoryStep, state: MixerState) {
        match step {
            MixerHistoryStep::Undo => self.mixer_undo_stack.push(state),
            MixerHistoryStep::Redo => self.mixer_redo_stack.push(state),
        }
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

        if let Some(task) = self.handle_mixer_ui_message(&message) {
            return task;
        }

        if matches!(message, MixerMessage::SelectProcessor(_, _)) {
            self.close_processor_browser();
        }

        if let Some(target) = editor_target_destroyed_by_mixer_message(&message)
            && let Some(task) =
                self.close_editor_before_deferred_mixer_message(target, message.clone())
        {
            return task;
        }

        let editor_cleanup = match &message {
            MixerMessage::SelectProcessor(target, _) => self.destroy_editor_target(*target),
            _ => Task::none(),
        };

        if let MixerMessage::RemoveBus(id) = message {
            return Task::batch([editor_cleanup, self.remove_bus(id, false)]);
        }

        let Some(mut playback) = self.playback.take() else {
            return editor_cleanup;
        };
        let (task, editor_target_to_open) =
            self.apply_mixer_message_with_playback(&mut playback, message, editor_cleanup);
        self.project_mixer_state = playback.mixer_state().clone();
        self.playback = Some(playback);
        if let Some(target) = editor_target_to_open {
            return Task::batch([task, self.open_editor_target(target)]);
        }
        task
    }

    pub(super) fn handle_mixer_ui_message(
        &mut self,
        message: &MixerMessage,
    ) -> Option<Task<Message>> {
        for route in MIXER_UI_ROUTES {
            if let Some(task) = route(self, message) {
                return Some(task);
            }
        }
        self.handle_mixer_editor_message(message)
    }

    fn handle_mixer_effect_drag_message(
        &mut self,
        message: &MixerMessage,
    ) -> Option<Task<Message>> {
        self.handle_mixer_effect_drag_lifecycle_message(message)
            .or_else(|| self.handle_mixer_effect_drag_motion_message(message))
    }

    pub(super) fn toggle_processor_browser_section(&mut self, key: &ProcessorBrowserSectionKey) {
        if let Some(index) = self
            .processor_browser_expanded_sections
            .iter()
            .position(|expanded| expanded == key)
        {
            self.processor_browser_expanded_sections.remove(index);
        } else {
            self.processor_browser_expanded_sections.push(key.clone());
        }
    }

    pub(super) fn toggle_mixer_effect_rack(&mut self, strip_index: usize) {
        if let Some(index) = self
            .open_mixer_effect_rack_tracks
            .iter()
            .position(|open_strip| *open_strip == strip_index)
        {
            self.open_mixer_effect_rack_tracks.remove(index);
        } else {
            self.open_mixer_effect_rack_tracks.push(strip_index);
            self.open_mixer_effect_rack_tracks.sort_unstable();
            self.open_mixer_effect_rack_tracks.dedup();
        }
    }

    pub(super) fn set_processor_slot_hovered(
        &mut self,
        target: Option<(EditorTarget, ProcessorSlotSegment)>,
    ) {
        if let Some((target, _segment)) = target {
            if target.slot_index > 0 {
                self.effect_rack_hovered_effect = Some((target.strip_index, target.slot_index - 1));
            }
        } else if self.effect_drag_source.is_none() {
            self.effect_rack_hovered_effect = None;
        }
        self.hovered_processor_slot = target;
    }

    pub(super) fn clear_effect_rack_hover_for_strip(&mut self, strip_index: usize) {
        if self
            .effect_rack_hovered_effect
            .is_some_and(|(hovered_strip_index, _)| hovered_strip_index == strip_index)
        {
            self.effect_rack_hovered_effect = None;
        }
    }

    pub(super) fn clear_effect_rack_drag_for_strip(&mut self, strip_index: usize) {
        if self
            .effect_drag_source
            .is_some_and(|(source_strip_index, _)| source_strip_index == strip_index)
        {
            self.effect_rack_autoscroll_direction = 0;
            self.effect_rack_drag_pointer_y = None;
        }
    }

    pub(super) fn start_mixer_bus_rename(&mut self, bus_id: u16) -> Task<Message> {
        let Some(name) = self
            .playback
            .as_ref()
            .and_then(|playback| playback.mixer_state().bus(BusId(bus_id)).ok())
            .map(|bus| bus.name.clone())
        else {
            return Task::none();
        };
        self.start_bus_rename(bus_id, WorkspacePaneKind::Mixer, name)
    }

    pub(super) fn handle_mixer_editor_message(
        &mut self,
        message: &MixerMessage,
    ) -> Option<Task<Message>> {
        match *message {
            MixerMessage::OpenEditor(target) => Some(self.open_editor_target(target)),
            _ => None,
        }
    }
}
