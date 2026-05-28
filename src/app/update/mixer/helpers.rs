use super::*;

pub(super) const EDITOR_DETACH_SETTLE_FRAMES: u8 = 1;

pub(super) enum MixerPlaybackResult {
    Continue,
    Return(Task<Message>, Option<EditorTarget>),
}

pub(super) fn apply_track_instrument_choice(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    target: EditorTarget,
    choice: ProcessorChoice,
    mixer_error: &mut Option<String>,
    editor_target_to_open: &mut Option<EditorTarget>,
) -> MixerPlaybackResult {
    if target.strip_index == 0 || target.strip_index > mixer.track_count() {
        return MixerPlaybackResult::Return(Task::none(), None);
    }
    let track_id = TrackId((target.strip_index - 1) as u16);
    let open_editor_after_select = matches!(&choice, ProcessorChoice::Processor { .. });
    let slot = track_instrument_slot_for_choice(mixer, track_id, choice);
    if let Err(error) = mixer.set_track_instrument(track_id, slot) {
        *mixer_error = Some(error.to_string());
    } else if open_editor_after_select {
        *editor_target_to_open = Some(target);
    }
    MixerPlaybackResult::Continue
}

pub(super) fn track_instrument_slot_for_choice(
    mixer: &lilypalooza_audio::MixerHandle<'_>,
    track_id: TrackId,
    choice: ProcessorChoice,
) -> SlotState {
    match choice {
        ProcessorChoice::None => SlotState::default(),
        ProcessorChoice::Processor {
            ref processor_id,
            backend,
            ..
        } => default_track_instrument_slot(mixer, track_id, processor_id, backend),
    }
}

pub(super) fn apply_effect_slot_choice(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    target: EditorTarget,
    choice: ProcessorChoice,
    mixer_error: &mut Option<String>,
    editor_target_to_open: &mut Option<EditorTarget>,
) -> MixerPlaybackResult {
    let effect_index = target.slot_index - 1;
    let Some(strip) = mixer.strip_by_index(target.strip_index) else {
        return MixerPlaybackResult::Return(Task::none(), None);
    };
    let bus_id = strip.bus_id;
    let mut effects = strip.effects().to_vec();
    apply_effect_choice_to_slots(
        &mut effects,
        effect_index,
        choice,
        target,
        editor_target_to_open,
    );
    match set_strip_effects(mixer, target.strip_index, bus_id, effects) {
        Ok(()) => MixerPlaybackResult::Continue,
        Err(error) => {
            *mixer_error = Some(error);
            MixerPlaybackResult::Continue
        }
    }
}

pub(super) fn apply_effect_choice_to_slots(
    effects: &mut Vec<SlotState>,
    effect_index: usize,
    choice: ProcessorChoice,
    target: EditorTarget,
    editor_target_to_open: &mut Option<EditorTarget>,
) {
    match choice {
        ProcessorChoice::None => remove_effect_slot(effects, effect_index),
        ProcessorChoice::Processor {
            ref processor_id,
            ref name,
            backend,
            ..
        } => {
            upsert_effect_slot(effects, effect_index, processor_id, backend, name);
            *editor_target_to_open = Some(target);
        }
    }
}

pub(super) fn remove_effect_slot(effects: &mut Vec<SlotState>, effect_index: usize) {
    if effect_index < effects.len() {
        effects.remove(effect_index);
    }
}

pub(super) fn upsert_effect_slot(
    effects: &mut Vec<SlotState>,
    effect_index: usize,
    processor_id: &str,
    backend: ProcessorBrowserBackend,
    name: &str,
) {
    let mut slot = processor_slot(processor_id, backend);
    assign_effect_instance_label_index(effects, effect_index, name, &mut slot);
    if let Some(effect) = effects.get_mut(effect_index) {
        *effect = slot;
    } else {
        effects.push(slot);
    }
}

pub(super) fn toggle_mixer_slot_bypass(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    target: EditorTarget,
    mixer_error: &mut Option<String>,
) {
    let address = lilypalooza_audio::SlotAddress {
        strip_index: target.strip_index,
        slot_index: target.slot_index,
    };
    let next = mixer
        .slot(address)
        .map(|slot| !slot.bypassed)
        .unwrap_or(false);
    if let Err(error) = mixer.set_slot_bypassed(address, next) {
        *mixer_error = Some(error.to_string());
    }
}

pub(super) fn set_strip_effects(
    mixer: &mut lilypalooza_audio::MixerHandle<'_>,
    strip_index: usize,
    bus_id: Option<BusId>,
    effects: Vec<SlotState>,
) -> Result<(), String> {
    if strip_index == 0 {
        return mixer
            .set_master_effects(effects)
            .map_err(|error| error.to_string());
    }
    if strip_index <= mixer.track_count() {
        return mixer
            .set_track_effects(TrackId((strip_index - 1) as u16), effects)
            .map_err(|error| error.to_string());
    }
    let Some(bus_id) = bus_id else {
        return Err("Mixer strip no longer exists".to_string());
    };
    mixer
        .set_bus_effects(bus_id, effects)
        .map_err(|error| error.to_string())
}

pub(super) fn processor_editor_window_settings(
    descriptor: lilypalooza_audio::EditorDescriptor,
    initial_size: Option<lilypalooza_audio::EditorSize>,
) -> window::Settings {
    let size = initial_size.unwrap_or(descriptor.default_size);
    window::Settings {
        size: Size::new(size.width as f32, size.height as f32),
        min_size: descriptor
            .min_size
            .map(|size| Size::new(size.width as f32, size.height as f32)),
        resizable: false,
        closeable: true,
        minimizable: false,
        decorations: false,
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}
