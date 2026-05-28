use super::*;

pub(super) fn track_effective_amplitude(mixer: &MixerState, track: &Track) -> f32 {
    let any_solo = mixer.tracks().iter().any(|track| track.state.soloed)
        || mixer.buses().iter().any(|bus| bus.state.soloed);
    let routed_to_soloed_bus = route_bus_id(track.routing.main)
        .is_some_and(|bus_id| mixer.bus(bus_id).is_ok_and(|bus| bus.state.soloed))
        || track
            .routing
            .sends
            .iter()
            .any(|send| mixer.bus(send.bus_id).is_ok_and(|bus| bus.state.soloed));
    if track.state.muted || (any_solo && !track.state.soloed && !routed_to_soloed_bus) {
        0.0
    } else {
        db_to_amplitude(track.state.gain_db)
    }
}

pub(super) fn bus_effective_amplitude(bus: &Track) -> f32 {
    if bus.state.muted {
        0.0
    } else {
        db_to_amplitude(bus.state.gain_db)
    }
}

pub(super) fn route_bus_id(route: TrackRoute) -> Option<BusId> {
    match route {
        TrackRoute::Master => None,
        TrackRoute::Bus(bus_id) => Some(bus_id),
    }
}

pub(super) fn track_needs_runtime(track: &Track) -> bool {
    !track
        .instrument_slot()
        .is_some_and(|slot| registry::is_empty(&slot.kind))
        || track.effect_count() > 0
}

pub(super) fn track_prefers_combined_signal_path(_track: &Track) -> bool {
    // Keep a stable instrument node when the first effect/send is inserted during playback.
    // Rebuilding the instrument graph here detaches audio from the editor-owned plugin instance.
    false
}

pub(super) fn db_to_amplitude(db: f32) -> f32 {
    if db <= -60.0 {
        0.0
    } else {
        knyst::db_to_amplitude(db)
    }
}
