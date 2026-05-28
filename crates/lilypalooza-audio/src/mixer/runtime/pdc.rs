use std::collections::HashMap;

use crate::mixer::{BusId, BusSend, MixerState, TrackId, TrackRoute};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StripLatency {
    pub(super) pre_fader: u32,
    pub(super) post_fader: u32,
    pub(super) output: u32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PdcPlan {
    pub(super) master_input_latency: u32,
    pub(super) bus_input_latencies: HashMap<BusId, u32>,
}

impl PdcPlan {
    pub(super) fn destination_latency(&self, route: TrackRoute) -> u32 {
        match route {
            TrackRoute::Master => self.master_input_latency,
            TrackRoute::Bus(bus_id) => self.bus_input_latencies.get(&bus_id).copied().unwrap_or(0),
        }
    }

    pub(super) fn bus_input_latency(&self, bus_id: BusId) -> u32 {
        self.bus_input_latencies.get(&bus_id).copied().unwrap_or(0)
    }

    pub(super) fn route_delay(&self, route: TrackRoute, source_latency: u32) -> u32 {
        self.destination_latency(route)
            .saturating_sub(source_latency)
    }

    pub(super) fn bus_send_delay(&self, bus_id: BusId, source_latency: u32) -> u32 {
        self.bus_input_latency(bus_id)
            .saturating_sub(source_latency)
    }
}

pub(super) fn compute_pdc_plan_from_latencies(
    mixer: &MixerState,
    track_latencies: &HashMap<TrackId, StripLatency>,
    bus_effect_latencies: &HashMap<BusId, u32>,
) -> PdcPlan {
    let mut plan = PdcPlan::default();
    for (bus_id, _) in mixer.buses_with_ids() {
        plan.bus_input_latencies.insert(bus_id, 0);
    }

    for _ in 0..=mixer.buses().len() {
        let next_bus_inputs =
            compute_bus_input_latencies(mixer, track_latencies, bus_effect_latencies, &plan);
        if next_bus_inputs == plan.bus_input_latencies {
            break;
        }
        plan.bus_input_latencies = next_bus_inputs;
    }
    plan.master_input_latency =
        compute_master_input_latency(mixer, track_latencies, bus_effect_latencies, &plan);
    plan
}

fn compute_bus_input_latencies(
    mixer: &MixerState,
    track_latencies: &HashMap<TrackId, StripLatency>,
    bus_effect_latencies: &HashMap<BusId, u32>,
    plan: &PdcPlan,
) -> HashMap<BusId, u32> {
    let mut latencies: HashMap<BusId, u32> = mixer
        .buses_with_ids()
        .map(|(bus_id, _)| (bus_id, 0))
        .collect();

    for (track_id, track) in mixer.tracks_with_ids() {
        let strip_latency = track_latencies.get(&track_id).copied().unwrap_or_default();
        collect_bus_input_latency(&mut latencies, track.routing.main, strip_latency.output);
        collect_send_input_latencies(&mut latencies, &track.routing.sends, strip_latency);
    }

    for (bus_id, bus) in mixer.buses_with_ids() {
        let strip_latency = bus_strip_latency(
            plan.bus_input_latency(bus_id),
            bus_effect_latencies
                .get(&bus_id)
                .copied()
                .unwrap_or_default(),
        );
        collect_bus_input_latency(&mut latencies, bus.routing.main, strip_latency.output);
        collect_send_input_latencies(&mut latencies, &bus.routing.sends, strip_latency);
    }

    latencies
}

fn compute_master_input_latency(
    mixer: &MixerState,
    track_latencies: &HashMap<TrackId, StripLatency>,
    bus_effect_latencies: &HashMap<BusId, u32>,
    plan: &PdcPlan,
) -> u32 {
    let mut latency = 0;
    for (track_id, track) in mixer.tracks_with_ids() {
        if track.routing.main == TrackRoute::Master {
            latency = latency.max(
                track_latencies
                    .get(&track_id)
                    .copied()
                    .unwrap_or_default()
                    .output,
            );
        }
    }
    for (bus_id, bus) in mixer.buses_with_ids() {
        if bus.routing.main == TrackRoute::Master {
            latency = latency.max(
                bus_strip_latency(
                    plan.bus_input_latency(bus_id),
                    bus_effect_latencies
                        .get(&bus_id)
                        .copied()
                        .unwrap_or_default(),
                )
                .output,
            );
        }
    }
    latency
}

fn collect_send_input_latencies(
    latencies: &mut HashMap<BusId, u32>,
    sends: &[BusSend],
    strip_latency: StripLatency,
) {
    for send in sends {
        if send.enabled {
            let source_latency = if send.pre_fader {
                strip_latency.pre_fader
            } else {
                strip_latency.post_fader
            };
            latencies
                .entry(send.bus_id)
                .and_modify(|latency| *latency = (*latency).max(source_latency));
        }
    }
}

fn bus_strip_latency(input_latency: u32, effect_latency: u32) -> StripLatency {
    let post_fader = input_latency.saturating_add(effect_latency);
    StripLatency {
        pre_fader: input_latency,
        post_fader,
        output: post_fader,
    }
}

fn collect_bus_input_latency(
    latencies: &mut HashMap<BusId, u32>,
    route: TrackRoute,
    source_latency: u32,
) {
    if let TrackRoute::Bus(bus_id) = route {
        latencies
            .entry(bus_id)
            .and_modify(|latency| *latency = (*latency).max(source_latency));
    }
}
