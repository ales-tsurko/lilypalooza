use super::*;

#[derive(Debug, Clone, Copy)]
enum MixerLevelChange {
    ResetTrackMeter(usize),
    SetTrackGain(usize, f32),
    SetTrackPan(usize, f32),
    ResetBusMeter(u16),
    SetBusGain(u16, f32),
    SetBusPan(u16, f32),
}

impl MixerLevelChange {
    fn from_track_message(message: &MixerMessage) -> Option<Self> {
        match *message {
            MixerMessage::ResetTrackMeter(index) => Some(Self::ResetTrackMeter(index)),
            MixerMessage::SetTrackGain(index, gain) => Some(Self::SetTrackGain(index, gain)),
            MixerMessage::SetTrackPan(index, pan) => Some(Self::SetTrackPan(index, pan)),
            _ => None,
        }
    }

    fn from_bus_message(message: &MixerMessage) -> Option<Self> {
        match *message {
            MixerMessage::ResetBusMeter(id) => Some(Self::ResetBusMeter(id)),
            MixerMessage::SetBusGain(id, gain) => Some(Self::SetBusGain(id, gain)),
            MixerMessage::SetBusPan(id, pan) => Some(Self::SetBusPan(id, pan)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MixerToggleChange {
    TrackMute(usize),
    TrackSolo(usize),
    BusMute(u16),
    BusSolo(u16),
}

#[derive(Debug, Clone, Copy)]
enum TrackToggleKind {
    Mute,
    Solo,
}

#[derive(Debug, Clone, Copy)]
enum MixerPlaybackChange {
    Level(MixerLevelChange),
    Toggle(MixerToggleChange),
}

impl MixerToggleChange {
    fn from_track_message(message: &MixerMessage) -> Option<Self> {
        match *message {
            MixerMessage::ToggleTrackMute(index) => Some(Self::TrackMute(index)),
            MixerMessage::ToggleTrackSolo(index) => Some(Self::TrackSolo(index)),
            _ => None,
        }
    }

    fn from_bus_message(message: &MixerMessage) -> Option<Self> {
        match *message {
            MixerMessage::ToggleBusMute(id) => Some(Self::BusMute(id)),
            MixerMessage::ToggleBusSolo(id) => Some(Self::BusSolo(id)),
            _ => None,
        }
    }
}

impl Lilypalooza {
    pub(super) fn apply_mixer_message_with_playback(
        &mut self,
        playback: &mut AudioEngine,
        message: MixerMessage,
        editor_cleanup: Task<Message>,
    ) -> (Task<Message>, Option<EditorTarget>) {
        let mut editor_target_to_open = None;
        let mut mixer_error = None;
        let mut editor_cleanup = Some(editor_cleanup);

        {
            let mut mixer = playback.mixer();

            let handled = self.apply_mixer_viewport_message(&message)
                || apply_mixer_master_message(&mut mixer, &message)
                || apply_mixer_send_message(&mut mixer, &message, &mut mixer_error)
                || self.apply_mixer_track_message(&mut mixer, &message, &mut mixer_error)
                || self.apply_mixer_bus_message(&mut mixer, &message, &mut mixer_error);

            if !handled
                && let MixerPlaybackResult::Return(task, target) = self
                    .apply_mixer_fallback_message(
                        &mut mixer,
                        message,
                        &mut mixer_error,
                        &mut editor_target_to_open,
                        &mut editor_cleanup,
                    )
            {
                return (task, target);
            }
        }

        if let Some(error) = mixer_error {
            self.logger.push(format!("Mixer update failed: {error}"));
        }
        (
            editor_cleanup.unwrap_or_else(Task::none),
            editor_target_to_open,
        )
    }

    pub(super) fn apply_mixer_fallback_message(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        message: MixerMessage,
        mixer_error: &mut Option<String>,
        editor_target_to_open: &mut Option<EditorTarget>,
        editor_cleanup: &mut Option<Task<Message>>,
    ) -> MixerPlaybackResult {
        match message {
            MixerMessage::AddBus => {
                self.add_numbered_bus(mixer, mixer_error);
                MixerPlaybackResult::Continue
            }
            MixerMessage::RemoveBus(_) => {
                MixerPlaybackResult::Return(editor_cleanup.take().unwrap_or_else(Task::none), None)
            }
            MixerMessage::SelectProcessor(target, choice) => self.apply_selected_processor(
                mixer,
                target,
                choice,
                mixer_error,
                editor_target_to_open,
            ),
            MixerMessage::ToggleSlotBypass(target) => {
                toggle_mixer_slot_bypass(mixer, target, mixer_error);
                MixerPlaybackResult::Continue
            }
            MixerMessage::MoveTrackEffect {
                strip_index,
                from_effect_index,
                to_effect_index,
            } => self.move_track_effect(
                mixer,
                strip_index,
                from_effect_index,
                to_effect_index,
                mixer_error,
            ),
            _ => MixerPlaybackResult::Continue,
        }
    }

    pub(super) fn add_numbered_bus(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        mixer_error: &mut Option<String>,
    ) {
        if let Err(error) = mixer.add_bus(format!("Bus {}", mixer.bus_count() + 1)) {
            *mixer_error = Some(error.to_string());
        }
    }

    pub(super) fn apply_selected_processor(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        target: EditorTarget,
        choice: super::super::mixer::ProcessorChoice,
        mixer_error: &mut Option<String>,
        editor_target_to_open: &mut Option<EditorTarget>,
    ) -> MixerPlaybackResult {
        if target.slot_index == 0 {
            return apply_track_instrument_choice(
                mixer,
                target,
                choice,
                mixer_error,
                editor_target_to_open,
            );
        }
        apply_effect_slot_choice(mixer, target, choice, mixer_error, editor_target_to_open)
    }

    pub(super) fn move_track_effect(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        strip_index: usize,
        from_effect_index: usize,
        to_effect_index: usize,
        mixer_error: &mut Option<String>,
    ) -> MixerPlaybackResult {
        let Some(strip) = mixer.strip_by_index(strip_index) else {
            return MixerPlaybackResult::Return(Task::none(), None);
        };
        let bus_id = strip.bus_id;
        let mut effects = strip.effects().to_vec();
        if from_effect_index >= effects.len() || to_effect_index >= effects.len() {
            return MixerPlaybackResult::Continue;
        }
        let effect = effects.remove(from_effect_index);
        effects.insert(to_effect_index, effect);
        let result = set_strip_effects(mixer, strip_index, bus_id, effects);
        match result {
            Ok(()) => self
                .processor_editor_windows
                .move_slot_targets_within_strip(
                    strip_index,
                    from_effect_index + 1,
                    to_effect_index + 1,
                ),
            Err(error) => *mixer_error = Some(error),
        }
        MixerPlaybackResult::Continue
    }

    pub(super) fn apply_mixer_viewport_message(&mut self, message: &MixerMessage) -> bool {
        match message {
            MixerMessage::InstrumentViewportScrolled(viewport) => {
                self.mixer_instrument_scroll_x = viewport.absolute_offset().x;
                self.mixer_instrument_viewport_width = viewport.bounds().width;
                true
            }
            MixerMessage::BusViewportScrolled(viewport) => {
                self.mixer_bus_scroll_x = viewport.absolute_offset().x;
                self.mixer_bus_viewport_width = viewport.bounds().width;
                true
            }
            _ => false,
        }
    }

    pub(super) fn apply_mixer_track_message(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        message: &MixerMessage,
        mixer_error: &mut Option<String>,
    ) -> bool {
        if self.apply_optional_mixer_change(mixer, track_playback_change(message), mixer_error)
            || apply_mixer_route_message(mixer, message, mixer_error)
        {
            return true;
        }
        false
    }

    pub(super) fn apply_mixer_bus_message(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        message: &MixerMessage,
        mixer_error: &mut Option<String>,
    ) -> bool {
        self.apply_optional_mixer_change(mixer, bus_playback_change(message), mixer_error)
    }

    fn apply_optional_mixer_change(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        change: Option<MixerPlaybackChange>,
        mixer_error: &mut Option<String>,
    ) -> bool {
        let Some(change) = change else {
            return false;
        };
        match change {
            MixerPlaybackChange::Level(change) => {
                apply_mixer_level_change(mixer, change, mixer_error);
            }
            MixerPlaybackChange::Toggle(change) => {
                self.apply_mixer_toggle_change(mixer, change, mixer_error);
            }
        }
        true
    }

    fn apply_mixer_toggle_change(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        change: MixerToggleChange,
        mixer_error: &mut Option<String>,
    ) {
        match change {
            MixerToggleChange::TrackMute(index) => {
                self.apply_track_mix_toggle(mixer, index, TrackToggleKind::Mute, mixer_error);
            }
            MixerToggleChange::TrackSolo(index) => {
                self.apply_track_mix_toggle(mixer, index, TrackToggleKind::Solo, mixer_error);
            }
            MixerToggleChange::BusMute(id) => {
                let next = next_bus_mute_state(mixer, id);
                store_mixer_result(mixer.set_bus_muted(BusId(id), next), mixer_error);
            }
            MixerToggleChange::BusSolo(id) => {
                self.apply_bus_solo_toggle(mixer, id, mixer_error);
            }
        }
    }

    fn apply_track_mix_toggle(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        index: usize,
        toggle: TrackToggleKind,
        mixer_error: &mut Option<String>,
    ) {
        let next = next_track_toggle_state(mixer, index, toggle);
        let result = apply_track_toggle_to_mixer(mixer, index, next, toggle);
        self.apply_track_toggle_state(result, index, next, toggle, mixer_error);
        if matches!(toggle, TrackToggleKind::Solo) {
            self.sync_global_solo_after_result(mixer, mixer_error);
        }
    }

    fn apply_track_toggle_state(
        &mut self,
        result: Result<(), lilypalooza_audio::AudioEngineError>,
        index: usize,
        enabled: bool,
        toggle: TrackToggleKind,
        mixer_error: &mut Option<String>,
    ) {
        match result {
            Ok(()) => match toggle {
                TrackToggleKind::Mute => {
                    self.piano_roll.set_track_muted(index, enabled);
                }
                TrackToggleKind::Solo => {
                    self.piano_roll.set_track_soloed(index, enabled);
                }
            },
            Err(error) => *mixer_error = Some(error.to_string()),
        }
    }

    fn sync_global_solo_after_result(
        &mut self,
        mixer: &lilypalooza_audio::MixerHandle<'_>,
        mixer_error: &Option<String>,
    ) {
        if mixer_error.is_none() {
            self.piano_roll
                .set_global_solo_active(mixer_has_any_solo(mixer));
        }
    }

    fn apply_bus_solo_toggle(
        &mut self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        id: u16,
        mixer_error: &mut Option<String>,
    ) {
        let next = next_bus_solo_state(mixer, id);
        match mixer.set_bus_soloed(BusId(id), next) {
            Ok(()) => self
                .piano_roll
                .set_global_solo_active(mixer_has_any_solo(mixer)),
            Err(error) => *mixer_error = Some(error.to_string()),
        }
    }
}

fn next_track_toggle_state(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    index: usize,
    toggle: TrackToggleKind,
) -> bool {
    match toggle {
        TrackToggleKind::Mute => next_track_mute_state(mixer, index),
        TrackToggleKind::Solo => next_track_solo_state(mixer, index),
    }
}

fn apply_track_toggle_to_mixer(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    index: usize,
    enabled: bool,
    toggle: TrackToggleKind,
) -> Result<(), lilypalooza_audio::AudioEngineError> {
    match toggle {
        TrackToggleKind::Mute => mixer.set_track_muted(TrackId(index as u16), enabled),
        TrackToggleKind::Solo => mixer.set_track_soloed(TrackId(index as u16), enabled),
    }
}

fn track_playback_change(message: &MixerMessage) -> Option<MixerPlaybackChange> {
    MixerLevelChange::from_track_message(message)
        .map(MixerPlaybackChange::Level)
        .or_else(|| MixerToggleChange::from_track_message(message).map(MixerPlaybackChange::Toggle))
}

fn bus_playback_change(message: &MixerMessage) -> Option<MixerPlaybackChange> {
    MixerLevelChange::from_bus_message(message)
        .map(MixerPlaybackChange::Level)
        .or_else(|| MixerToggleChange::from_bus_message(message).map(MixerPlaybackChange::Toggle))
}

fn apply_mixer_level_change(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    change: MixerLevelChange,
    mixer_error: &mut Option<String>,
) {
    let result = match change {
        MixerLevelChange::ResetTrackMeter(index) => mixer.reset_track_meter(TrackId(index as u16)),
        MixerLevelChange::SetTrackGain(index, gain) => {
            mixer.set_track_gain_db(TrackId(index as u16), gain)
        }
        MixerLevelChange::SetTrackPan(index, pan) => {
            mixer.set_track_pan(TrackId(index as u16), pan)
        }
        MixerLevelChange::ResetBusMeter(id) => mixer.reset_bus_meter(BusId(id)),
        MixerLevelChange::SetBusGain(id, gain) => mixer.set_bus_gain_db(BusId(id), gain),
        MixerLevelChange::SetBusPan(id, pan) => mixer.set_bus_pan(BusId(id), pan),
    };
    store_mixer_result(result, mixer_error);
}
