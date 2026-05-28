use std::time::Duration;

use knyst::{
    controller::KnystCommands,
    prelude::{Beats, MultiThreadedKnystCommands, TransportState},
};

use super::*;

const GRAPH_SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
const PLAYHEAD_SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(2);

pub(super) fn settle_graph_mutation(commands: &mut MultiThreadedKnystCommands) {
    let Ok(receiver) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        commands.request_graph_settled()
    })) else {
        return;
    };
    match receiver.recv_timeout(GRAPH_SETTLE_TIMEOUT) {
        Ok(()) | Err(_) => {}
    }
}

pub(super) fn send_level_only_changed(old: BusSend, new: BusSend) -> bool {
    old.bus_id == new.bus_id
        && old.pre_fader == new.pre_fader
        && (old.enabled != new.enabled || old.gain_db.to_bits() != new.gain_db.to_bits())
}

pub(super) fn current_bus_send(
    state: &MixerState,
    id: BusId,
    index: usize,
) -> Result<BusSend, MixerError> {
    let bus = state.bus(id)?;
    bus.routing
        .sends
        .get(index)
        .copied()
        .ok_or(MixerError::BusSendIndexOutOfBounds {
            index,
            len: bus.routing.sends.len(),
        })
}

pub(super) fn current_playing_beat(commands: &mut MultiThreadedKnystCommands) -> Option<Beats> {
    let start = std::time::Instant::now();
    while start.elapsed() < GRAPH_SETTLE_TIMEOUT {
        if let Some(snapshot) = commands.current_transport_snapshot()
            && snapshot.state == TransportState::Playing
        {
            return snapshot.beats;
        }
        std::thread::sleep(PLAYHEAD_SNAPSHOT_POLL_INTERVAL);
    }
    None
}
