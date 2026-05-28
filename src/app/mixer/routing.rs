use iced::widget::column;

use super::*;

pub(super) fn effect_slot_dependencies(
    strip: &lilypalooza_audio::mixer::Track,
) -> Vec<EffectSlotDependency> {
    strip
        .effects()
        .iter()
        .enumerate()
        .map(|(effect_index, slot)| EffectSlotDependency {
            slot_index: effect_index + 1,
            instance_id: slot.instance_id,
            instance_label_index: slot.instance_label_index,
            selected: selected_processor_choice(Some(slot), ProcessorSlotRole::Effect),
            editor_enabled: slot
                .descriptor()
                .and_then(|descriptor| descriptor.editor)
                .is_some(),
            bypassed: slot.bypassed,
        })
        .collect()
}

pub(super) fn effect_slot_display_labels(effects: &[EffectSlotDependency]) -> Vec<Option<String>> {
    let mut counts_by_name = std::collections::BTreeMap::<String, usize>::new();
    for effect in effects {
        if let Some(name) = effect_slot_name(effect) {
            *counts_by_name.entry(name.to_string()).or_default() += 1;
        }
    }

    effects
        .iter()
        .map(|effect| {
            let name = effect_slot_name(effect)?;
            if counts_by_name.get(name).copied().unwrap_or_default() <= 1 {
                return Some(name.to_string());
            }

            Some(format!("{name} [{}]", effect.instance_label_index))
        })
        .collect()
}

pub(super) fn effect_slot_name(effect: &EffectSlotDependency) -> Option<&str> {
    match effect.selected.as_ref()? {
        ProcessorChoice::None => None,
        ProcessorChoice::Processor { name, .. } => Some(name),
    }
}

pub(super) fn route_choices(mixer: &MixerState, source: RoutingStrip) -> Vec<RouteChoice> {
    let mut choices = vec![RouteChoice {
        route: TrackRoute::Master,
        label: "Master".to_string(),
    }];
    choices.extend(mixer.buses().iter().filter_map(|bus| {
        let bus_id = bus.bus_id?;
        if matches!(source, RoutingStrip::Bus(source_id) if source_id == bus_id.0) {
            return None;
        }
        if matches!(source, RoutingStrip::Bus(source_id) if !mixer.can_route_bus_to_bus(BusId(source_id), bus_id))
        {
            return None;
        }
        Some(RouteChoice {
            route: TrackRoute::Bus(bus_id),
            label: route_label(&bus.name),
        })
    }));
    choices
}

pub(super) fn selected_route_choice(route: TrackRoute, choices: &[RouteChoice]) -> RouteChoice {
    choices
        .iter()
        .find(|choice| choice.route == route)
        .cloned()
        .or_else(|| choices.first().cloned())
        .unwrap_or(RouteChoice {
            route: TrackRoute::Master,
            label: "Main".to_string(),
        })
}

pub(super) fn route_menu_height_for_items(item_count: usize) -> f32 {
    ROUTE_MENU_ITEM_HEIGHT * item_count.clamp(1, ROUTE_MENU_MAX_ITEMS) as f32
}

pub(super) fn send_destination_choices(
    mixer: &MixerState,
    source: RoutingStrip,
) -> Vec<SendDestinationChoice> {
    mixer
        .buses()
        .iter()
        .filter_map(|bus| {
            let bus_id = bus.bus_id?;
            if matches!(source, RoutingStrip::Bus(source_id) if source_id == bus_id.0) {
                return None;
            }
            if matches!(source, RoutingStrip::Bus(source_id) if !mixer.can_route_bus_to_bus(BusId(source_id), bus_id))
            {
                return None;
            }
            Some(SendDestinationChoice {
                action: SendDestinationAction::Route(bus_id.0),
                label: route_label(&bus.name),
            })
        })
        .collect()
}

pub(super) fn send_menu_choices(
    mut choices: Vec<SendDestinationChoice>,
) -> Vec<SendDestinationChoice> {
    choices.insert(
        0,
        SendDestinationChoice {
            action: SendDestinationAction::Remove,
            label: "Remove".to_string(),
        },
    );
    choices
}

pub(super) fn first_send_bus_id(choices: &[SendDestinationChoice]) -> Option<u16> {
    choices.iter().find_map(|choice| match choice.action {
        SendDestinationAction::Route(bus_id) => Some(bus_id),
        SendDestinationAction::Remove => None,
    })
}

pub(super) fn selected_send_destination_choice(
    bus_id: u16,
    choices: &[SendDestinationChoice],
) -> Option<SendDestinationChoice> {
    choices.iter().find_map(|choice| match choice.action {
        SendDestinationAction::Route(choice_bus_id) if choice_bus_id == bus_id => {
            Some(choice.clone())
        }
        SendDestinationAction::Route(_) | SendDestinationAction::Remove => None,
    })
}

pub(super) fn send_dependencies(routing: &TrackRouting) -> Vec<SendDependency> {
    routing
        .sends
        .iter()
        .map(|send| SendDependency {
            bus_id: send.bus_id.0,
            gain_bits: send.gain_db.to_bits(),
            enabled: send.enabled,
            pre_fader: send.pre_fader,
        })
        .collect()
}

pub(super) fn track_effect_rack_panel(
    strip_index: usize,
    effects: Vec<EffectSlotDependency>,
    routing: Option<EffectRackPanelRouting>,
    hovered_processor_slot: Option<(
        crate::app::processor_editor_windows::EditorTarget,
        ProcessorSlotSegment,
    )>,
    effect_drag: Option<EffectRackDragState>,
    controls_enabled: bool,
    strip_height: f32,
) -> Element<'static, Message> {
    let hovered_processor_slot = hovered_processor_slot
        .filter(|(target, _)| target.strip_index == strip_index)
        .map(|(target, segment)| (target.slot_index, segment));
    let rack = effect_rack(
        strip_index,
        effects,
        hovered_processor_slot,
        effect_drag,
        controls_enabled,
        EFFECT_RACK_VISIBLE_SLOTS,
    );
    let has_routing = routing.is_some();
    let (rack_height, routing_height) = effect_rack_panel_heights(strip_height, has_routing);
    let mut panel = column![
        container(rack)
            .width(Fill)
            .height(Length::Fixed(rack_height))
            .style(effect_rack_surface)
    ]
    .spacing(0);

    if let Some(routing) = routing {
        panel = panel
            .push(
                container(text(""))
                    .width(Fill)
                    .height(Length::Fixed(EFFECT_RACK_SEPARATOR_HEIGHT))
                    .style(effect_rack_separator_surface),
            )
            .push(
                container(send_panel(
                    routing.source,
                    routing.sends,
                    routing.send_choices,
                    controls_enabled,
                ))
                .width(Fill)
                .height(Length::Fixed(routing_height))
                .style(effect_rack_surface),
            );
    }

    container(panel)
        .width(Length::Fixed(EFFECT_RACK_PANEL_WIDTH))
        .height(Length::Fixed(strip_height))
        .style(effect_rack_surface)
        .into()
}

pub(super) fn effect_rack_panel_heights(strip_height: f32, has_routing: bool) -> (f32, f32) {
    if !has_routing {
        return (strip_height, 0.0);
    }

    let available_height = (strip_height - EFFECT_RACK_SEPARATOR_HEIGHT).max(0.0);
    let rack_height = available_height / 2.0;
    (rack_height, available_height - rack_height)
}
