use lilypalooza_audio::{BusId, InstrumentSlotState, TrackId};

use super::super::messages::MixerMessage;
use super::*;

impl Lilypalooza {
    pub(in crate::app) fn handle_mixer_message(&mut self, message: MixerMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::Mixer);

        let Some(playback) = self.playback.as_mut() else {
            return Task::none();
        };
        let mut mixer = playback.mixer();

        match message {
            MixerMessage::AddBus => {
                let _ = mixer.add_bus(format!("Bus {}", mixer.bus_count() + 1));
            }
            MixerMessage::SetMasterGain(gain) => mixer.set_master_gain_db(gain),
            MixerMessage::SetMasterPan(pan) => mixer.set_master_pan(pan),
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
        }

        Task::none()
    }
}

fn mixer_has_any_solo(mixer: &lilypalooza_audio::MixerHandle<'_>) -> bool {
    mixer.tracks().iter().any(|track| track.state.soloed)
        || mixer.buses().iter().any(|bus| bus.state.soloed)
}
