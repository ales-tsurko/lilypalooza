use super::*;

pub(super) fn store_mixer_result(
    result: Result<(), impl ToString>,
    mixer_error: &mut Option<String>,
) {
    if let Err(error) = result {
        *mixer_error = Some(error.to_string());
    }
}

pub(super) fn apply_mixer_route_message(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    message: &MixerMessage,
    mixer_error: &mut Option<String>,
) -> bool {
    let MixerMessage::SetMainRoute(source, route) = *message else {
        return false;
    };
    let result = match source {
        crate::app::mixer::RoutingStrip::Track(index) => {
            mixer.set_track_route(TrackId(index as u16), route)
        }
        crate::app::mixer::RoutingStrip::Bus(id) => mixer.set_bus_route(BusId(id), route),
    };
    store_mixer_result(result, mixer_error);
    true
}

pub(super) fn next_track_mute_state(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    index: usize,
) -> bool {
    mixer
        .track(TrackId(index as u16))
        .map(|track| !track.state.muted)
        .unwrap_or(false)
}

pub(super) fn next_track_solo_state(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    index: usize,
) -> bool {
    mixer
        .track(TrackId(index as u16))
        .map(|track| !track.state.soloed)
        .unwrap_or(false)
}

pub(super) fn next_bus_mute_state(mixer: &lilypalooza_audio::MixerHandle<'_>, id: u16) -> bool {
    mixer
        .bus(BusId(id))
        .map(|bus| !bus.state.muted)
        .unwrap_or(false)
}

pub(super) fn next_bus_solo_state(mixer: &lilypalooza_audio::MixerHandle<'_>, id: u16) -> bool {
    mixer
        .bus(BusId(id))
        .map(|bus| !bus.state.soloed)
        .unwrap_or(false)
}

pub(super) fn apply_mixer_master_message(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    message: &MixerMessage,
) -> bool {
    match *message {
        MixerMessage::ResetMasterMeter => mixer.reset_master_meter(),
        MixerMessage::SetMasterGain(gain) => mixer.set_master_gain_db(gain),
        MixerMessage::SetMasterPan(pan) => mixer.set_master_pan(pan),
        _ => return false,
    }
    true
}

pub(super) fn apply_mixer_send_message(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    message: &MixerMessage,
    mixer_error: &mut Option<String>,
) -> bool {
    let result = match *message {
        MixerMessage::AddSend(source, bus_id) => {
            let send = BusSend::new(BusId(bus_id), 0.0, false);
            match source {
                crate::app::mixer::RoutingStrip::Track(index) => {
                    mixer.add_track_send(TrackId(index as u16), send)
                }
                crate::app::mixer::RoutingStrip::Bus(id) => mixer.add_bus_send(BusId(id), send),
            }
            .map_err(|error| error.to_string())
        }
        MixerMessage::SetSendDestination(source, send_index, bus_id) => {
            update_send(mixer, source, send_index, |send| {
                send.bus_id = BusId(bus_id);
            })
        }
        MixerMessage::SetSendGain(source, send_index, gain) => {
            update_send(mixer, source, send_index, |send| {
                send.gain_db = gain;
            })
        }
        MixerMessage::ToggleSendEnabled(source, send_index) => {
            update_send(mixer, source, send_index, |send| {
                send.enabled = !send.enabled;
            })
        }
        MixerMessage::ToggleSendPreFader(source, send_index) => {
            update_send(mixer, source, send_index, |send| {
                send.pre_fader = !send.pre_fader;
            })
        }
        MixerMessage::RemoveSend(source, send_index) => match source {
            crate::app::mixer::RoutingStrip::Track(index) => mixer
                .remove_track_send(TrackId(index as u16), send_index)
                .map(drop)
                .map_err(|error| error.to_string()),
            crate::app::mixer::RoutingStrip::Bus(id) => mixer
                .remove_bus_send(BusId(id), send_index)
                .map(drop)
                .map_err(|error| error.to_string()),
        },
        _ => return false,
    };

    if let Err(error) = result {
        *mixer_error = Some(error);
    }
    true
}

pub(super) fn editor_target_destroyed_by_mixer_message(
    message: &MixerMessage,
) -> Option<EditorTarget> {
    match message {
        MixerMessage::SelectProcessor(target, _) => Some(*target),
        _ => None,
    }
}

impl Lilypalooza {
    pub(super) fn commit_pending_mixer_history(&mut self) {
        if let Some(snapshot) = self.pending_mixer_undo_snapshot.take() {
            self.mixer_undo_stack.push(snapshot);
            self.mixer_redo_stack.clear();
        }
    }

    pub(super) fn begin_effect_drag_from_hover(&mut self) {
        let Some(source) = self.effect_rack_hovered_effect else {
            return;
        };
        self.effect_drag_source = Some(source);
        self.effect_drag_target = Some(source);
    }

    pub(super) fn finish_effect_drag(&mut self, target: Option<(usize, usize)>) -> Task<Message> {
        let source = self.effect_drag_source.take();
        let target = target.or(self.effect_drag_target);
        self.effect_drag_target = None;
        self.effect_rack_autoscroll_direction = 0;
        self.effect_rack_drag_pointer_y = None;

        let Some((from_strip_index, from_effect_index)) = source else {
            return Task::none();
        };
        let Some((to_strip_index, to_effect_index)) = target else {
            return Task::none();
        };
        if from_strip_index != to_strip_index || from_effect_index == to_effect_index {
            return Task::none();
        }

        self.handle_mixer_message(MixerMessage::MoveTrackEffect {
            strip_index: from_strip_index,
            from_effect_index,
            to_effect_index,
        })
    }

    pub(super) fn update_effect_rack_drag_position(&mut self, strip_index: usize, y: f32) {
        let hover = self.effect_rack_hovered_index_at_y(strip_index, y);
        self.effect_rack_hovered_effect = hover.map(|index| (strip_index, index));

        if self
            .effect_drag_source
            .is_some_and(|(source_strip, _)| source_strip == strip_index)
            && let Some(target_index) = self.effect_rack_drop_index_at_y(strip_index, y)
        {
            self.effect_drag_target = Some((strip_index, target_index));
            self.effect_rack_drag_pointer_y = Some(y);
            self.update_effect_rack_autoscroll_direction(strip_index, y);
        }
    }

    pub(super) fn effect_rack_hovered_index_at_y(
        &self,
        strip_index: usize,
        y: f32,
    ) -> Option<usize> {
        let index = self.effect_rack_raw_index_at_y(strip_index, y);
        (index < self.effect_count_for_strip(strip_index)?).then_some(index)
    }

    pub(super) fn effect_rack_drop_index_at_y(&self, strip_index: usize, y: f32) -> Option<usize> {
        let effect_count = self.effect_count_for_strip(strip_index)?;
        if effect_count == 0 {
            return None;
        }
        Some(
            self.effect_rack_raw_index_at_y(strip_index, y)
                .min(effect_count - 1),
        )
    }

    pub(super) fn effect_rack_raw_index_at_y(&self, strip_index: usize, y: f32) -> usize {
        let scroll_y = self
            .effect_rack_scroll_y
            .get(&strip_index)
            .copied()
            .unwrap_or(0.0);
        crate::number::f32_to_usize(
            ((scroll_y + y).max(0.0) / crate::app::mixer::EFFECT_RACK_ROW_HEIGHT).floor(),
        )
    }

    pub(super) fn effect_count_for_strip(&self, strip_index: usize) -> Option<usize> {
        let mixer = self.playback.as_ref()?.mixer_state();
        mixer
            .strip_by_index(strip_index)
            .map(lilypalooza_audio::mixer::Track::effect_count)
    }

    pub(super) fn update_effect_rack_autoscroll_direction(&mut self, strip_index: usize, y: f32) {
        let viewport_height = self
            .effect_rack_viewport_height
            .get(&strip_index)
            .copied()
            .unwrap_or(crate::app::mixer::EFFECT_RACK_HEIGHT);
        let edge = crate::app::mixer::EFFECT_RACK_EDGE_SCROLL_ZONE.min(viewport_height / 2.0);
        self.effect_rack_autoscroll_direction = if y <= edge {
            -1
        } else if y >= viewport_height - edge {
            1
        } else {
            0
        };
    }

    pub(in crate::app) fn tick_effect_rack_autoscroll(&mut self) -> Task<Message> {
        let Some((strip_index, _)) = self.effect_drag_source else {
            return Task::none();
        };
        if self.effect_rack_autoscroll_direction == 0 {
            return Task::none();
        }
        if let Some(y) = self.effect_rack_drag_pointer_y {
            self.effect_drag_target = self
                .effect_rack_drop_index_at_y(strip_index, y)
                .map(|index| (strip_index, index));
        }

        iced::widget::operation::scroll_by(
            crate::app::mixer::effect_rack_scroll_id(strip_index),
            iced::widget::operation::AbsoluteOffset {
                x: 0.0,
                y: crate::app::mixer::EFFECT_RACK_EDGE_SCROLL_STEP
                    * f32::from(self.effect_rack_autoscroll_direction),
            },
        )
    }
}

pub(super) fn mixer_has_any_solo(mixer: &lilypalooza_audio::MixerHandle<'_>) -> bool {
    mixer.tracks().iter().any(|track| track.state.soloed)
        || mixer.buses().iter().any(|bus| bus.state.soloed)
}

pub(super) fn update_send(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    source: crate::app::mixer::RoutingStrip,
    send_index: usize,
    update: impl FnOnce(&mut BusSend),
) -> Result<(), String> {
    match source {
        crate::app::mixer::RoutingStrip::Track(index) => {
            let id = TrackId(index as u16);
            let mut send = *mixer
                .track(id)
                .map_err(|error| error.to_string())?
                .routing
                .sends
                .get(send_index)
                .ok_or_else(|| format!("track send index {send_index} is out of bounds"))?;
            update(&mut send);
            mixer
                .set_track_send(id, send_index, send)
                .map_err(|error| error.to_string())
        }
        crate::app::mixer::RoutingStrip::Bus(id) => {
            let id = BusId(id);
            let mut send = *mixer
                .bus(id)
                .map_err(|error| error.to_string())?
                .routing
                .sends
                .get(send_index)
                .ok_or_else(|| format!("bus send index {send_index} is out of bounds"))?;
            update(&mut send);
            mixer
                .set_bus_send(id, send_index, send)
                .map_err(|error| error.to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MixerHistoryMode {
    None,
    Immediate,
    Gesture,
}

pub(super) fn mixer_message_history_mode(
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
        | MixerMessage::ToggleProcessorBrowser(_)
        | MixerMessage::CloseProcessorBrowser
        | MixerMessage::ProcessorBrowserSearchChanged(_)
        | MixerMessage::ToggleProcessorBrowserSection(_)
        | MixerMessage::ToggleMixerEffectRack(_)
        | MixerMessage::SetProcessorSlotHovered(_)
        | MixerMessage::StartTrackEffectDrag { .. }
        | MixerMessage::DropTrackEffect { .. }
        | MixerMessage::TrackEffectDragMoved { .. }
        | MixerMessage::EffectRackCursorLeft(_)
        | MixerMessage::EffectRackViewportScrolled { .. }
        | MixerMessage::OpenEditor(_)
        | MixerMessage::PreviewTrackColor(_) => MixerHistoryMode::None,
        MixerMessage::SetMasterGain(_)
        | MixerMessage::SetMasterPan(_)
        | MixerMessage::SetTrackGain(_, _)
        | MixerMessage::SetTrackPan(_, _)
        | MixerMessage::SetBusGain(_, _)
        | MixerMessage::SetBusPan(_, _)
        | MixerMessage::SetSendGain(_, _, _) => {
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
        | MixerMessage::SetMainRoute(_, _)
        | MixerMessage::AddSend(_, _)
        | MixerMessage::SetSendDestination(_, _, _)
        | MixerMessage::ToggleSendEnabled(_, _)
        | MixerMessage::ToggleSendPreFader(_, _)
        | MixerMessage::RemoveSend(_, _)
        | MixerMessage::SelectProcessor(_, _)
        | MixerMessage::ToggleSlotBypass(_)
        | MixerMessage::MoveTrackEffect { .. }
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
        if slot_index == 0 {
            return strip_name.to_string();
        }

        slot.title(strip_name, slot_index)
    }

    pub(super) fn open_editor_target(&mut self, target: EditorTarget) -> Task<Message> {
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

    pub(super) fn open_editor(
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

        let (window_id, open_task) =
            window::open(processor_editor_window_settings(descriptor, None));
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
            EditorFrameCommand::SetZoomPercent(percent) => {
                let errors = self
                    .processor_editor_windows
                    .set_zoom_percent(target, percent);
                for error in errors {
                    self.log_processor_editor_error("resize editor", error);
                }
            }
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

    pub(super) fn processor_kind_for_target(&self, target: EditorTarget) -> Option<ProcessorKind> {
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

    pub(super) fn refresh_editor_preset_state(
        &mut self,
        target: EditorTarget,
        selected_id: Option<String>,
    ) {
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

    pub(super) fn save_processor_preset_from_target(&mut self, target: EditorTarget) {
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

    pub(super) fn rename_processor_preset_for_target(
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

    pub(super) fn delete_processor_preset_for_target(&mut self, target: EditorTarget, id: String) {
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

    pub(super) fn load_processor_preset_for_target(&mut self, target: EditorTarget, id: String) {
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

    pub(super) fn step_processor_preset(&mut self, target: EditorTarget, direction: isize) {
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
        if let Some(preset) = presets.get(next_index) {
            self.load_processor_preset_for_target(target, preset.id.clone());
        }
    }
}

pub(super) fn default_track_instrument_slot(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    track_id: TrackId,
    processor_id: &str,
    backend: crate::app::mixer::ProcessorBrowserBackend,
) -> SlotState {
    if backend == crate::app::mixer::ProcessorBrowserBackend::BuiltIn
        && processor_id == BUILTIN_SOUNDFONT_ID
    {
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

    processor_slot(processor_id, backend)
}

pub(super) fn processor_slot(
    processor_id: &str,
    backend: crate::app::mixer::ProcessorBrowserBackend,
) -> SlotState {
    match backend {
        crate::app::mixer::ProcessorBrowserBackend::BuiltIn => {
            SlotState::built_in(processor_id, lilypalooza_audio::ProcessorState::default())
        }
        crate::app::mixer::ProcessorBrowserBackend::Clap
        | crate::app::mixer::ProcessorBrowserBackend::Vst3 => SlotState::new(
            ProcessorKind::Plugin {
                plugin_id: processor_id.to_string(),
            },
            lilypalooza_audio::ProcessorState::default(),
        ),
    }
}

pub(super) fn assign_effect_instance_label_index(
    effects: &[SlotState],
    target_effect_index: usize,
    processor_name: &str,
    slot: &mut SlotState,
) {
    if let Some(existing) = effects.get(target_effect_index)
        && effect_slot_name(existing).as_deref() == Some(processor_name)
        && existing.instance_label_index > 0
    {
        slot.instance_label_index = existing.instance_label_index;
        return;
    }

    let mut used = std::collections::BTreeSet::new();
    for (effect_index, effect) in effects.iter().enumerate() {
        if effect_index != target_effect_index
            && effect_slot_name(effect).as_deref() == Some(processor_name)
        {
            used.insert(effect.instance_label_index);
        }
    }

    slot.instance_label_index = (1..)
        .find(|index| !used.contains(index))
        .expect("infinite label index range should have a free value");
}

pub(super) fn effect_slot_name(slot: &SlotState) -> Option<String> {
    lilypalooza_audio::instrument::registry::resolve(&slot.kind)
        .map(|entry| entry.name.into_owned())
}

pub(super) fn sync_piano_roll_mix_from_mixer_state(
    piano_roll: &mut crate::app::PianoRollState,
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
