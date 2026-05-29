use std::collections::BTreeMap;

use iced::{
    Color, Element, Fill, FillPortion, Length, Padding, alignment, border, mouse,
    widget::{
        Id, button, container, lazy, mouse_area, opaque, pick_list, responsive, row, scrollable,
        stack, text, text_input,
    },
};
use iced_aw::helpers::color_picker_with_change;
use lilypalooza_audio::{
    BUILTIN_METRONOME_ID, BUILTIN_NONE_ID, MixerState, SlotState,
    instrument::registry,
    mixer::{
        BusId, ChannelMeterSnapshot, MixerMeterSnapshotWindow, STRIP_METER_MIN_DB,
        StripMeterSnapshot, TrackRoute, TrackRouting,
    },
};

use super::{
    Lilypalooza, Message,
    controls::{
        COMPACT_HORIZONTAL_SLIDER_METRICS, GAIN_MIN_DB, HorizontalSliderScale, gain_control_width,
        gain_fader, gain_fader_scale, gain_fader_scale_width, gain_knob, horizontal_slider,
        pan_knob,
    },
    messages::MixerMessage,
    meters::{
        MeterColors, meter_colors, stereo_meter, stereo_meter_bar_width, stereo_meter_width,
        stereo_meter_with_scale,
    },
};
use crate::{fonts, icons, ui_style};

mod browser;
mod bus_area;
mod content;
mod effect_rack;
mod processor_slot;
mod routing;
mod strip;

pub(super) use browser::instrument_browser_overlay;
use browser::*;
use bus_area::*;
pub(super) use content::content;
#[cfg(test)]
use content::*;
use effect_rack::*;
use processor_slot::*;
use routing::*;
use strip::*;

pub(super) const MIXER_MIN_HEIGHT: f32 = ui_style::grid_f32(102);
pub(super) const MIXER_MIN_WIDTH: f32 = 520.0;
const INSTRUMENT_SCROLL_ID: &str = "mixer-instrument-scroll";

pub(super) fn instrument_scroll_id() -> Id {
    Id::new(INSTRUMENT_SCROLL_ID)
}

pub(in crate::app) fn effect_rack_scroll_id(track_index: usize) -> Id {
    Id::from(format!("effect-rack-scroll-{track_index}"))
}

const GROUP_SIDE_BORDER_WIDTH: f32 = 1.0;
const MAIN_STRIP_WIDTH: f32 = ui_style::grid_f32(36);
const MAIN_SECTION_WIDTH: f32 = MAIN_STRIP_WIDTH + GROUP_SIDE_BORDER_WIDTH * 2.0;
const STRIP_WIDTH: f32 = ui_style::grid_f32(37);
const STRIP_SPACING: f32 = 0.0;
const PROCESSOR_SLOT_HEIGHT: f32 = ui_style::grid_f32(7);
const PROCESSOR_SLOT_BUTTON_HEIGHT: f32 = ui_style::grid_f32(6);
const PROCESSOR_SLOT_WIDTH: f32 = 112.0;
const PROCESSOR_SLOT_SEGMENT_WIDTH: f32 = ui_style::grid_f32(4);
const PROCESSOR_SLOT_SEPARATOR_WIDTH: f32 = 1.0;
const PROCESSOR_SLOT_LABEL_MAX_LEN: usize = 11;
const PROCESSOR_BROWSER_WIDTH: f32 = 520.0;
const PROCESSOR_BROWSER_HEIGHT: f32 = 360.0;
const PROCESSOR_BROWSER_ICON_SIZE: f32 = ui_style::grid_f32(3);
const EFFECT_RACK_VISIBLE_SLOTS: usize = 7;
pub(in crate::app) const EFFECT_RACK_HEIGHT: f32 =
    EFFECT_RACK_ROW_HEIGHT * EFFECT_RACK_VISIBLE_SLOTS as f32;
pub(in crate::app) const EFFECT_RACK_EDGE_SCROLL_ZONE: f32 = ui_style::grid_f32(3);
pub(in crate::app) const EFFECT_RACK_EDGE_SCROLL_STEP: f32 = ui_style::grid_f32(2);
const EFFECT_RACK_PANEL_WIDTH: f32 = ui_style::grid_f32(36);
const EFFECT_RACK_SLOT_WIDTH: f32 = EFFECT_RACK_PANEL_WIDTH;
const EFFECT_RACK_SEPARATOR_HEIGHT: f32 = 1.0;
pub(in crate::app) const EFFECT_RACK_ROW_HEIGHT: f32 =
    PROCESSOR_SLOT_BUTTON_HEIGHT + EFFECT_RACK_SEPARATOR_HEIGHT;
const EFFECT_RACK_SEPARATOR_INSET: f32 = ui_style::grid_f32(4);
const EFFECT_RACK_SCROLLBAR_WIDTH: f32 = 6.0;
const EFFECT_RACK_SCROLLBAR_SCROLLER_WIDTH: f32 = 3.0;
const EFFECT_RACK_SCROLLBAR_SPACING: f32 = 2.0;
const EFFECT_RACK_SCROLLBAR_MARGIN: f32 = 1.0;
const ROUTE_PICKER_HEIGHT: f32 = ui_style::grid_f32(6);
const ROUTE_PICKER_TOP_SPACING: f32 = ui_style::grid_f32(3);
const ROUTE_PICKER_BOTTOM_INSET: f32 = 0.0;
const ROUTE_MENU_ITEM_HEIGHT: f32 = ui_style::grid_f32(7);
const ROUTE_MENU_MAX_ITEMS: usize = 6;
const ROUTE_PICKER_MAX_LEN: usize = 12;
const SEND_ROW_HEIGHT: f32 = ui_style::grid_f32(14);
const SEND_ROW_CONTENT_BOTTOM_SPACING: f32 = ui_style::grid_f32(1);
const SEND_PANEL_TOP_SPACING: f32 = ui_style::grid_f32(1);
const SEND_PANEL_HEADER_HEIGHT: f32 = ui_style::grid_f32(5);
const SEND_CONTROL_HEIGHT: f32 = ui_style::grid_f32(5);
const SEND_MODE_HEIGHT: f32 = SEND_CONTROL_HEIGHT;
const SEND_MODE_WIDTH: f32 = ui_style::grid_f32(10);
const SEND_PICKER_WIDTH: f32 = ui_style::grid_f32(16);
const SEND_GAIN_MIN_DB: f32 = GAIN_MIN_DB;
const SEND_GAIN_MAX_DB: f32 = 6.0;
const SEND_GAIN_STEP_DB: f32 = 0.5;
const INSTRUMENT_PICKER_HEIGHT: f32 = PROCESSOR_SLOT_HEIGHT;
const INSTRUMENT_SLOT_BUTTON_HEIGHT: f32 = PROCESSOR_SLOT_BUTTON_HEIGHT;
#[cfg(test)]
const INSTRUMENT_SLOT_WIDTH: f32 = PROCESSOR_SLOT_WIDTH;
#[cfg(test)]
const INSTRUMENT_SLOT_EDITOR_AREA_WIDTH: f32 = PROCESSOR_SLOT_SEGMENT_WIDTH;
const INSTRUMENT_SLOT_SEPARATOR_WIDTH: f32 = PROCESSOR_SLOT_SEPARATOR_WIDTH;
const INSTRUMENT_BROWSER_WIDTH: f32 = PROCESSOR_BROWSER_WIDTH;
const INSTRUMENT_BROWSER_HEIGHT: f32 = PROCESSOR_BROWSER_HEIGHT;
#[cfg(test)]
const INSTRUMENT_BROWSER_ICON_SIZE: f32 = PROCESSOR_BROWSER_ICON_SIZE;
const SECTION_HEADER_HEIGHT: f32 = 24.0;
const STRIP_MIN_HEIGHT: f32 = ui_style::grid_f32(88);
const STRIP_FOOTER_HEIGHT: f32 = TITLE_TOP_SPACING
    + TRACK_TITLE_EDITOR_HEIGHT
    + ROUTE_PICKER_TOP_SPACING
    + ROUTE_PICKER_HEIGHT
    + ROUTE_PICKER_BOTTOM_INSET;
const STRIP_TOGGLE_SIZE: f32 = INSTRUMENT_PICKER_HEIGHT - 4.0;
const TRACK_TITLE_EDITOR_HEIGHT: f32 = ui_style::grid_f32(5);
const TRACK_TITLE_EDITOR_CONTROL_HEIGHT: f32 = TRACK_TITLE_EDITOR_HEIGHT;
const TRACK_TITLE_EDITOR_SWATCH_SIZE: f32 = TRACK_TITLE_EDITOR_HEIGHT;
const TRACK_TITLE_EDITOR_INPUT_PADDING_V: u16 = 2;
const TRACK_TITLE_EDITOR_INPUT_PADDING_H: u16 = ui_style::grid(1);
const COMPACT_GAIN_SWITCH_OFFSET: f32 = ui_style::grid_f32(8);
const VALUE_LABEL_HEIGHT: f32 = ui_style::grid_f32(4);
const HEADER_SIDE_INSET: f32 = 12.0;
const SECTION_BODY_GAP: f32 = 0.0;
const METER_STACK_SPACING: f32 = ui_style::grid_f32(2);
const STRIP_STACK_SPACING: f32 = ui_style::grid_f32(1);
const LABEL_CONTROL_SPACING: f32 = ui_style::SPACE_XS as f32;
const GAIN_SCALE_SPACING: f32 = 1.0;
const TITLE_TOP_SPACING: f32 = ui_style::grid_f32(3);
const STRIP_VIRTUALIZATION_OVERSCAN: usize = 2;

pub(super) fn instrument_track_scroll_x(track_index: usize) -> f32 {
    strip_span_width(track_index)
}

struct StripActions<'a> {
    panel: Option<(bool, bool, Message)>,
    solo: Option<(bool, Message)>,
    mute: Option<(bool, Message)>,
    on_gain: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    on_pan: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum RoutingStrip {
    Track(usize),
    Bus(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RouteChoice {
    route: TrackRoute,
    label: String,
}

impl std::fmt::Display for RouteChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SendDestinationAction {
    Route(u16),
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SendDestinationChoice {
    action: SendDestinationAction,
    label: String,
}

impl std::fmt::Display for SendDestinationChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SendDependency {
    bus_id: u16,
    gain_bits: u32,
    enabled: bool,
    pre_fader: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectRackPanelRouting {
    source: RoutingStrip,
    sends: Vec<SendDependency>,
    send_choices: Vec<SendDestinationChoice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectRackDragState {
    source_effect_index: usize,
    target_effect_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectRackDropIndicator {
    Top,
    After(usize),
}

fn effect_rack_drag_state(
    source: Option<(usize, usize)>,
    target: Option<(usize, usize)>,
    strip_index: usize,
) -> Option<EffectRackDragState> {
    let (source_strip, source_effect_index) = source?;
    let (target_strip, target_effect_index) = target?;

    (source_strip == strip_index
        && target_strip == strip_index
        && source_effect_index != target_effect_index)
        .then_some(EffectRackDragState {
            source_effect_index,
            target_effect_index,
        })
}

fn effect_rack_drop_indicator(
    drag_state: Option<EffectRackDragState>,
) -> Option<EffectRackDropIndicator> {
    let drag_state = drag_state?;
    if drag_state.source_effect_index < drag_state.target_effect_index {
        Some(EffectRackDropIndicator::After(
            drag_state.target_effect_index,
        ))
    } else if drag_state.target_effect_index == 0 {
        Some(EffectRackDropIndicator::Top)
    } else {
        Some(EffectRackDropIndicator::After(
            drag_state.target_effect_index - 1,
        ))
    }
}

fn noop_message() -> Message {
    Message::Noop
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum ProcessorChoice {
    None,
    Processor {
        processor_id: String,
        name: String,
        backend: ProcessorBrowserBackend,
    },
}

pub(super) type InstrumentChoice = ProcessorChoice;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProcessorBrowserBackend {
    BuiltIn,
    Clap,
    Vst3,
}

impl ProcessorBrowserBackend {
    fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "Built-in",
            Self::Clap => "CLAP",
            Self::Vst3 => "VST3",
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(super) enum ProcessorSlotRole {
    Instrument,
    Effect,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(super) enum ProcessorSlotSegment {
    Bypass,
    Editor,
    Picker,
}

impl ProcessorSlotRole {
    fn registry_role(self) -> registry::Role {
        match self {
            Self::Instrument => registry::Role::Instrument,
            Self::Effect => registry::Role::Effect,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Instrument => "Choose Instrument",
            Self::Effect => "Choose Effect",
        }
    }

    fn search_placeholder(self) -> &'static str {
        match self {
            Self::Instrument => "Search instruments",
            Self::Effect => "Search effects",
        }
    }

    fn empty_search_label(self) -> &'static str {
        match self {
            Self::Instrument => "No matching instruments",
            Self::Effect => "No matching effects",
        }
    }

    fn slot_icon(self) -> iced::widget::svg::Handle {
        match self {
            Self::Instrument => icons::keyboard_music(),
            Self::Effect => icons::audio_waveform(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstrumentBrowserEntries {
    show_none: bool,
    backends: Vec<ProcessorBrowserBackendSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessorBrowserBackendSection {
    key: ProcessorBrowserSectionKey,
    title: String,
    sections: Vec<ProcessorBrowserSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessorBrowserSection {
    key: ProcessorBrowserSectionKey,
    title: String,
    entries: Vec<ProcessorChoice>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(super) struct ProcessorBrowserSectionKey {
    role: ProcessorSlotRole,
    backend: ProcessorBrowserBackend,
    depth: ProcessorBrowserSectionDepth,
    title: String,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
enum ProcessorBrowserSectionDepth {
    Backend,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessorBrowserRowDepth {
    Root,
    Group,
    Leaf,
}

impl ProcessorBrowserSectionKey {
    pub(super) fn new(
        role: ProcessorSlotRole,
        backend: ProcessorBrowserBackend,
        title: String,
    ) -> Self {
        Self {
            role,
            backend,
            depth: ProcessorBrowserSectionDepth::Group,
            title,
        }
    }

    fn backend(role: ProcessorSlotRole, backend: ProcessorBrowserBackend) -> Self {
        Self {
            role,
            backend,
            depth: ProcessorBrowserSectionDepth::Backend,
            title: backend.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MainStripDependency {
    gain_bits: u32,
    pan_bits: u32,
    meter: MeterDependency,
    compact_gain: bool,
    effect_rack_open: bool,
    panel_has_content: bool,
    strip_height_bits: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TrackStripDependency {
    index: usize,
    name: String,
    selected: Option<ProcessorChoice>,
    editor_enabled: bool,
    effects: Vec<EffectSlotDependency>,
    hovered_processor_slot: Option<(usize, ProcessorSlotSegment)>,
    color_bits: [u32; 4],
    gain_bits: u32,
    pan_bits: u32,
    route: RouteChoice,
    route_choices: Vec<RouteChoice>,
    meter: MeterDependency,
    compact_gain: bool,
    effect_rack_open: bool,
    panel_has_content: bool,
    strip_height_bits: u32,
    soloed: bool,
    muted: bool,
    tint_enabled: bool,
    highlighted: bool,
    renaming: bool,
    rename_value: String,
    color_picker_open: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EffectSlotDependency {
    slot_index: usize,
    instance_id: u64,
    instance_label_index: u32,
    selected: Option<ProcessorChoice>,
    editor_enabled: bool,
    bypassed: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct BusStripDependency {
    strip_index: usize,
    id: u16,
    name: String,
    effects: Vec<EffectSlotDependency>,
    effect_rack_open: bool,
    gain_bits: u32,
    pan_bits: u32,
    route: RouteChoice,
    route_choices: Vec<RouteChoice>,
    meter: MeterDependency,
    compact_gain: bool,
    strip_height_bits: u32,
    panel_has_content: bool,
    soloed: bool,
    muted: bool,
    renaming: bool,
    rename_value: String,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct MeterStackDependency {
    meter: MeterDependency,
    colors: MeterColorsDependency,
    compact_gain: bool,
    strip_height_bits: u32,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct MeterDependency {
    left_level_bits: u32,
    right_level_bits: u32,
    left_hold_bits: u32,
    right_hold_bits: u32,
    left_hold_db_bits: u32,
    right_hold_db_bits: u32,
    clip_latched: bool,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct MeterColorsDependency {
    rail: [u32; 4],
    safe: [u32; 4],
    warning: [u32; 4],
    hot: [u32; 4],
    hold: [u32; 4],
    clip: [u32; 4],
    scale_text: [u32; 4],
    scale_tick: [u32; 4],
}

impl MeterDependency {
    fn from_snapshot(snapshot: StripMeterSnapshot) -> Self {
        Self {
            left_level_bits: snapshot.left.level.to_bits(),
            right_level_bits: snapshot.right.level.to_bits(),
            left_hold_bits: snapshot.left.hold.to_bits(),
            right_hold_bits: snapshot.right.hold.to_bits(),
            left_hold_db_bits: snapshot.left.hold_db.to_bits(),
            right_hold_db_bits: snapshot.right.hold_db.to_bits(),
            clip_latched: snapshot.clip_latched,
        }
    }

    fn snapshot(self) -> StripMeterSnapshot {
        StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: f32::from_bits(self.left_level_bits),
                hold: f32::from_bits(self.left_hold_bits),
                hold_db: f32::from_bits(self.left_hold_db_bits),
            },
            right: ChannelMeterSnapshot {
                level: f32::from_bits(self.right_level_bits),
                hold: f32::from_bits(self.right_hold_bits),
                hold_db: f32::from_bits(self.right_hold_db_bits),
            },
            clip_latched: self.clip_latched,
        }
    }
}

impl MeterColorsDependency {
    fn from_colors(colors: MeterColors) -> Self {
        Self {
            rail: color_bits(colors.rail),
            safe: color_bits(colors.safe),
            warning: color_bits(colors.warning),
            hot: color_bits(colors.hot),
            hold: color_bits(colors.hold),
            clip: color_bits(colors.clip),
            scale_text: color_bits(colors.scale_text),
            scale_tick: color_bits(colors.scale_tick),
        }
    }

    fn colors(self) -> MeterColors {
        MeterColors {
            rail: color_from_bits(self.rail),
            safe: color_from_bits(self.safe),
            warning: color_from_bits(self.warning),
            hot: color_from_bits(self.hot),
            hold: color_from_bits(self.hold),
            clip: color_from_bits(self.clip),
            scale_text: color_from_bits(self.scale_text),
            scale_tick: color_from_bits(self.scale_tick),
        }
    }
}

fn color_bits(color: iced::Color) -> [u32; 4] {
    [
        color.r.to_bits(),
        color.g.to_bits(),
        color.b.to_bits(),
        color.a.to_bits(),
    ]
}

fn color_from_bits(bits: [u32; 4]) -> iced::Color {
    iced::Color {
        r: f32::from_bits(bits[0]),
        g: f32::from_bits(bits[1]),
        b: f32::from_bits(bits[2]),
        a: f32::from_bits(bits[3]),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GainControlMode {
    Fader,
    Knob,
}

#[cfg(test)]
mod tests_interactions;
#[cfg(test)]
mod tests_layout;
#[cfg(test)]
mod tests_routing;
