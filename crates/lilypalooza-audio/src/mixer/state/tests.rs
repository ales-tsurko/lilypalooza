use std::path::PathBuf;

use super::{MixerError, MixerState, send_level_only_changed};
use crate::{
    instrument::{BUILTIN_GAIN_ID, BUILTIN_SOUNDFONT_ID, ProcessorKind, ProcessorState, SlotState},
    mixer::{BusId, BusSend, INSTRUMENT_TRACK_COUNT, TrackId, TrackRoute},
    soundfont::SoundfontResource,
};

fn soundfont_slot(program: u8) -> SlotState {
    SlotState::built_in(BUILTIN_SOUNDFONT_ID, ProcessorState(vec![program]))
}

#[test]
fn send_level_change_is_detected_without_routing_rebuild() {
    let bus_id = BusId(1);

    assert!(send_level_only_changed(
        BusSend::new(bus_id, -6.0, false),
        BusSend::new(bus_id, -3.0, false)
    ));
    let mut disabled = BusSend::new(bus_id, -6.0, false);
    disabled.enabled = false;
    assert!(send_level_only_changed(
        BusSend::new(bus_id, -6.0, false),
        disabled
    ));
    assert!(!send_level_only_changed(
        BusSend::new(bus_id, -6.0, false),
        BusSend::new(BusId(2), -3.0, false)
    ));
    assert!(!send_level_only_changed(
        BusSend::new(bus_id, -6.0, false),
        BusSend::new(bus_id, -3.0, true)
    ));
}

#[test]
fn mixer_preallocates_instrument_tracks_and_master() {
    let mixer = MixerState::new();
    assert_eq!(mixer.track_count(), INSTRUMENT_TRACK_COUNT);
    assert_eq!(mixer.bus_count(), 0);
    assert_eq!(mixer.strip_count(), 1 + INSTRUMENT_TRACK_COUNT);
    assert_eq!(mixer.master().name, "Master");
    assert_eq!(mixer.master().bus_id, None);
    assert_eq!(
        mixer.track(TrackId(0)).expect("track should exist").bus_id,
        None
    );
    assert_eq!(
        mixer
            .track(TrackId((INSTRUMENT_TRACK_COUNT - 1) as u16))
            .expect("track should exist")
            .bus_id,
        None
    );
}

#[test]
fn track_routing_rejects_missing_bus_targets() {
    let mut mixer = MixerState::new();
    let error = mixer
        .set_track_route(TrackId(0), TrackRoute::Bus(BusId(1)))
        .expect_err("missing bus should be rejected");
    assert_eq!(error, MixerError::InvalidBusId(BusId(1)));
}

#[test]
fn dynamic_buses_accept_routes_and_sends() {
    let mut mixer = MixerState::new();
    let bus_id = mixer.add_bus("Verb");
    mixer
        .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
        .expect("bus route should succeed");
    mixer
        .add_track_bus_send(TrackId(0), BusSend::new(bus_id, -6.0, false))
        .expect("bus send should succeed");
    assert_eq!(mixer.bus_count(), 1);
    assert_eq!(
        mixer
            .track(TrackId(0))
            .expect("track should exist")
            .routing
            .sends
            .len(),
        1
    );
    assert!(
        mixer
            .track(TrackId(0))
            .expect("track should exist")
            .routing
            .sends[0]
            .enabled
    );
}

#[test]
fn bus_routes_reject_feedback_cycles() {
    assert_bus_feedback_cycle_rejected(BusCycleOperation::Route);
}

#[test]
fn bus_sends_reject_feedback_cycles() {
    assert_bus_feedback_cycle_rejected(BusCycleOperation::Send);
}

#[derive(Debug, Clone, Copy)]
enum BusCycleOperation {
    Route,
    Send,
}

fn assert_bus_feedback_cycle_rejected(operation: BusCycleOperation) {
    let mut mixer = MixerState::new();
    let verb = mixer.add_bus("Verb");
    let delay = mixer.add_bus("Delay");
    mixer
        .set_bus_route(verb, TrackRoute::Bus(delay))
        .expect("forward bus route should succeed");

    let error = match operation {
        BusCycleOperation::Route => mixer.set_bus_route(delay, TrackRoute::Bus(verb)),
        BusCycleOperation::Send => mixer.add_bus_send(delay, BusSend::new(verb, -6.0, false)),
    }
    .expect_err("cycle should be rejected");

    assert_eq!(
        error,
        MixerError::FeedbackRouting {
            source_id: delay,
            destination_id: verb,
        }
    );
}

#[test]
fn disabled_bus_sends_roundtrip_without_losing_gain_or_pre_fader() {
    let mut mixer = MixerState::new();
    let bus_id = mixer.add_bus("Verb");
    let mut send = BusSend::new(bus_id, -9.0, true);
    send.enabled = false;

    mixer
        .add_track_bus_send(TrackId(0), send)
        .expect("bus send should succeed");

    let restored: MixerState =
        ron::from_str(&ron::to_string(&mixer).expect("mixer should serialize"))
            .expect("mixer should deserialize");
    assert_eq!(
        restored
            .track(TrackId(0))
            .expect("track should exist")
            .routing
            .sends[0],
        send
    );
}

#[test]
fn removing_bus_reroutes_tracks_and_clears_sends() {
    let mut mixer = MixerState::new();
    let bus_id = mixer.add_bus("Verb");
    mixer
        .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
        .expect("bus route should succeed");
    mixer
        .add_track_bus_send(TrackId(0), BusSend::new(bus_id, -6.0, false))
        .expect("bus send should succeed");

    mixer
        .remove_bus(bus_id)
        .expect("bus removal should succeed");

    let track = mixer.track(TrackId(0)).expect("track should exist");
    assert_eq!(track.routing.main, TrackRoute::Master);
    assert!(track.routing.sends.is_empty());
}

#[test]
fn mixer_roundtrips_through_ron() {
    let mut mixer = MixerState::new();
    let bus_id = mixer.add_bus("Verb");
    mixer.set_soundfont(SoundfontResource {
        id: "fluid".to_string(),
        name: "FluidR3".to_string(),
        path: PathBuf::from("/tmp/FluidR3.sf2"),
    });
    mixer
        .set_track_route(TrackId(0), TrackRoute::Bus(bus_id))
        .expect("bus route should succeed");
    mixer
        .add_track_bus_send(TrackId(1), BusSend::new(bus_id, -3.0, true))
        .expect("bus send should succeed");

    let ron = ron::to_string(&mixer).expect("mixer should serialize");
    let restored: MixerState = ron::from_str(&ron).expect("mixer should deserialize");

    assert_eq!(restored, mixer);
}

#[test]
fn replacing_soundfont_keeps_one_entry_per_id() {
    let mut mixer = MixerState::new();
    mixer.set_soundfont(SoundfontResource {
        id: "fluid".to_string(),
        name: "FluidR3".to_string(),
        path: PathBuf::from("/tmp/FluidR3.sf2"),
    });
    mixer.set_soundfont(SoundfontResource {
        id: "fluid".to_string(),
        name: "GeneralUser".to_string(),
        path: PathBuf::from("/tmp/GeneralUser.sf2"),
    });

    assert_eq!(mixer.soundfonts().len(), 1);
    assert_eq!(mixer.soundfonts()[0].name, "GeneralUser");
}

#[test]
fn mixer_roundtrips_effect_slots() {
    let (mixer, restored) = roundtrip_mixer_with_gain_slot(false);
    assert_eq!(restored, mixer);
}

#[test]
fn mixer_roundtrips_bypassed_effect_slots() {
    let (_mixer, restored) = roundtrip_mixer_with_gain_slot(true);
    assert!(
        restored
            .track(TrackId(0))
            .expect("track should exist")
            .effect(0)
            .expect("effect should exist")
            .bypassed
    );
}

fn roundtrip_mixer_with_gain_slot(bypassed: bool) -> (MixerState, MixerState) {
    let mut mixer = MixerState::new();
    let mut slot = SlotState::built_in(BUILTIN_GAIN_ID, ProcessorState::default());
    slot.bypassed = bypassed;
    mixer
        .track_mut(TrackId(0))
        .expect("track should exist")
        .push_effect(slot);

    let ron = ron::to_string(&mixer).expect("mixer should serialize");
    let restored: MixerState = ron::from_str(&ron).expect("mixer should deserialize");
    (mixer, restored)
}

#[test]
fn strip_by_index_uses_visible_mixer_order() {
    let mut mixer = MixerState::new();
    let bus_id = mixer.add_bus("Verb");

    let master = mixer.strip_by_index(0).expect("master strip should exist");
    assert_eq!(master.name, "Master");
    assert_eq!(master.bus_id, None);

    let first_track = mixer.strip_by_index(1).expect("track strip should exist");
    assert_eq!(first_track.name, "Track 1");
    assert_eq!(first_track.bus_id, None);

    let bus = mixer
        .strip_by_index(1 + INSTRUMENT_TRACK_COUNT)
        .expect("bus strip should exist");
    assert_eq!(bus.name, "Verb");
    assert_eq!(bus.bus_id, Some(bus_id));
}

#[test]
fn strip_slots_use_shared_index_convention() {
    let mut mixer = MixerState::new();
    mixer
        .track_mut(TrackId(0))
        .expect("track should exist")
        .set_instrument_slot(soundfont_slot(0));
    mixer
        .track_mut(TrackId(0))
        .expect("track should exist")
        .push_effect(SlotState::built_in(
            BUILTIN_GAIN_ID,
            ProcessorState::default(),
        ));
    let bus_id = mixer.add_bus("Verb");
    mixer
        .bus_mut(bus_id)
        .expect("bus should exist")
        .push_effect(SlotState::built_in(
            BUILTIN_GAIN_ID,
            ProcessorState::default(),
        ));

    let track = mixer.strip_by_index(1).expect("track strip should exist");
    assert_eq!(track.slot(0), track.instrument_slot());
    assert!(track.slot(1).is_some());

    let master = mixer.strip_by_index(0).expect("master strip should exist");
    assert!(master.slot(0).is_some());
    assert_eq!(master.effect_count(), 0);

    let bus = mixer
        .strip_by_index(1 + INSTRUMENT_TRACK_COUNT)
        .expect("bus strip should exist");
    assert!(bus.slot(0).is_some());
    assert_eq!(
        bus.slot(1).and_then(|slot| match &slot.kind {
            ProcessorKind::BuiltIn { processor_id } => Some(processor_id.as_str()),
            ProcessorKind::Plugin { .. } => None,
        }),
        Some(BUILTIN_GAIN_ID)
    );
}
