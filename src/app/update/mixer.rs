use lilypalooza_audio::{BusId, InstrumentSlotState, TrackId};

use super::super::messages::MixerMessage;
use super::*;

impl Lilypalooza {
    pub(in crate::app) fn handle_primary_mouse_pressed(&mut self, pressed: bool) -> Task<Message> {
        self.primary_mouse_pressed = pressed;
        if !pressed {
            self.commit_pending_mixer_history();
        }
        if pressed && self.renaming_target.is_some() {
            return iced::widget::operation::is_focused(super::super::TRACK_RENAME_INPUT_ID)
                .map(Message::TrackRenameFocusChanged);
        }
        Task::none()
    }

    pub(in crate::app) fn undo_mixer_operation(&mut self) -> Task<Message> {
        self.pending_mixer_undo_snapshot = None;
        let Some(previous) = self.mixer_undo_stack.pop() else {
            return Task::none();
        };
        let Some(playback) = self.playback.as_mut() else {
            return Task::none();
        };

        let current = playback.mixer_state().clone();
        let restored_state = previous.clone();
        let mut mixer = playback.mixer();
        if mixer.replace_state(previous).is_ok() {
            self.mixer_redo_stack.push(current);
            sync_piano_roll_mix_from_mixer_state(&mut self.piano_roll, &restored_state);
        }
        Task::none()
    }

    pub(in crate::app) fn redo_mixer_operation(&mut self) -> Task<Message> {
        self.pending_mixer_undo_snapshot = None;
        let Some(next) = self.mixer_redo_stack.pop() else {
            return Task::none();
        };
        let Some(playback) = self.playback.as_mut() else {
            return Task::none();
        };

        let current = playback.mixer_state().clone();
        let restored_state = next.clone();
        let mut mixer = playback.mixer();
        if mixer.replace_state(next).is_ok() {
            self.mixer_undo_stack.push(current);
            sync_piano_roll_mix_from_mixer_state(&mut self.piano_roll, &restored_state);
        }
        Task::none()
    }

    pub(in crate::app) fn handle_mixer_message(&mut self, message: MixerMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Mixer);

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
            MixerMessage::StartTrackRename(track_index) => {
                return self.start_track_rename(track_index);
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
                return self.start_bus_rename(bus_id, name);
            }
            MixerMessage::TrackRenameInputChanged(value) => {
                self.update_track_rename_value(value);
                return Task::none();
            }
            MixerMessage::CommitTrackRename => return self.commit_track_rename(),
            _ => {}
        }

        let Some(playback) = self.playback.as_mut() else {
            return Task::none();
        };
        let mut mixer = playback.mixer();

        match message {
            MixerMessage::AddBus => {
                let _ = mixer.add_bus(format!("Bus {}", mixer.bus_count() + 1));
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
                let _ = mixer.reset_track_meter(TrackId(index as u16));
            }
            MixerMessage::SetTrackGain(index, gain) => {
                let _ = mixer.set_track_gain_db(TrackId(index as u16), gain);
            }
            MixerMessage::SetTrackPan(index, pan) => {
                let _ = mixer.set_track_pan(TrackId(index as u16), pan);
            }
            MixerMessage::ToggleTrackMute(index) => {
                let next = mixer
                    .track(TrackId(index as u16))
                    .map(|track| !track.state.muted)
                    .unwrap_or(false);
                let _ = mixer.set_track_muted(TrackId(index as u16), next);
                self.piano_roll.set_track_muted(index, next);
            }
            MixerMessage::ToggleTrackSolo(index) => {
                let next = mixer
                    .track(TrackId(index as u16))
                    .map(|track| !track.state.soloed)
                    .unwrap_or(false);
                let _ = mixer.set_track_soloed(TrackId(index as u16), next);
                self.piano_roll.set_track_soloed(index, next);
                self.piano_roll
                    .set_global_solo_active(mixer_has_any_solo(&mixer));
            }
            MixerMessage::SelectTrackInstrument(index, choice) => {
                let slot = match choice {
                    super::super::mixer::InstrumentChoice::None => InstrumentSlotState::empty(),
                    super::super::mixer::InstrumentChoice::SoundfontProgram {
                        soundfont_id,
                        bank,
                        program,
                        ..
                    } => InstrumentSlotState::soundfont(soundfont_id, bank, program),
                };
                let _ = mixer.set_track_instrument(TrackId(index as u16), slot);
            }
            MixerMessage::ResetBusMeter(id) => {
                let _ = mixer.reset_bus_meter(BusId(id));
            }
            MixerMessage::SetBusGain(id, gain) => {
                let _ = mixer.set_bus_gain_db(BusId(id), gain);
            }
            MixerMessage::SetBusPan(id, pan) => {
                let _ = mixer.set_bus_pan(BusId(id), pan);
            }
            MixerMessage::ToggleBusMute(id) => {
                let next = mixer
                    .bus(BusId(id))
                    .map(|bus| !bus.state.muted)
                    .unwrap_or(false);
                let _ = mixer.set_bus_muted(BusId(id), next);
            }
            MixerMessage::ToggleBusSolo(id) => {
                let next = mixer
                    .bus(BusId(id))
                    .map(|bus| !bus.state.soloed)
                    .unwrap_or(false);
                let _ = mixer.set_bus_soloed(BusId(id), next);
                self.piano_roll
                    .set_global_solo_active(mixer_has_any_solo(&mixer));
            }
            MixerMessage::StartTrackRename(_)
            | MixerMessage::StartBusRename(_)
            | MixerMessage::TrackRenameInputChanged(_)
            | MixerMessage::CommitTrackRename => {}
        }

        Task::none()
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
        | MixerMessage::BusViewportScrolled(_) => MixerHistoryMode::None,
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
        | MixerMessage::StartTrackRename(_)
        | MixerMessage::StartBusRename(_)
        | MixerMessage::TrackRenameInputChanged(_)
        | MixerMessage::CommitTrackRename
        | MixerMessage::ToggleTrackMute(_)
        | MixerMessage::ToggleTrackSolo(_)
        | MixerMessage::SelectTrackInstrument(_, _)
        | MixerMessage::ToggleBusMute(_)
        | MixerMessage::ToggleBusSolo(_) => MixerHistoryMode::Immediate,
    }
}

fn sync_piano_roll_mix_from_mixer_state(
    piano_roll: &mut super::super::piano_roll::PianoRollState,
    mixer: &lilypalooza_audio::MixerState,
) {
    for track in mixer.tracks() {
        let index = track.id.index();
        let _ = piano_roll.set_track_muted(index, track.state.muted);
        let _ = piano_roll.set_track_soloed(index, track.state.soloed);
    }
    piano_roll.set_global_solo_active(
        mixer.tracks().iter().any(|track| track.state.soloed)
            || mixer.buses().iter().any(|bus| bus.state.soloed),
    );
}

#[cfg(test)]
mod tests {
    use super::{MixerHistoryMode, mixer_message_history_mode};
    use crate::app::messages::MixerMessage;

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
    }

    #[test]
    fn mixer_meter_resets_do_not_record_history() {
        assert_eq!(
            mixer_message_history_mode(&MixerMessage::ResetTrackMeter(0), false),
            MixerHistoryMode::None
        );
    }
}
