use iced::widget::Id;
use iced::widget::{
    button, column, container, lazy, mouse_area, opaque, responsive, row, scrollable, stack, svg,
    text, text_input,
};
use iced::{Color, Element, Fill, FillPortion, Length, alignment};
use iced_aw::helpers::color_picker_with_change;
use lilypalooza_audio::instrument::registry;
use lilypalooza_audio::mixer::{
    ChannelMeterSnapshot, MixerMeterSnapshot, MixerMeterSnapshotWindow, STRIP_METER_MIN_DB,
    StripMeterSnapshot,
};
use lilypalooza_audio::{BUILTIN_METRONOME_ID, BUILTIN_NONE_ID, MixerState, SlotState};

use super::controls::{GAIN_MIN_DB, gain_control_width, gain_fader, gain_knob, pan_knob};
use super::messages::MixerMessage;
use super::meters::{
    MeterColors, meter_colors, stereo_meter, stereo_meter_bar_width, stereo_meter_width,
    stereo_meter_with_scale,
};
use super::{Lilypalooza, Message};
use crate::{fonts, icons, ui_style};

pub(super) const MIXER_MIN_HEIGHT: f32 = 340.0;
pub(super) const MIXER_MIN_WIDTH: f32 = 520.0;
const INSTRUMENT_SCROLL_ID: &str = "mixer-instrument-scroll";

pub(super) fn instrument_scroll_id() -> Id {
    Id::new(INSTRUMENT_SCROLL_ID)
}

const GROUP_SIDE_BORDER_WIDTH: f32 = 1.0;
const MAIN_STRIP_WIDTH: f32 = ui_style::grid_f32(36);
const MAIN_SECTION_WIDTH: f32 = MAIN_STRIP_WIDTH + GROUP_SIDE_BORDER_WIDTH * 2.0;
const STRIP_WIDTH: f32 = ui_style::grid_f32(37);
const STRIP_SPACING: f32 = 0.0;
const INSTRUMENT_PICKER_HEIGHT: f32 = ui_style::grid_f32(6);
const INSTRUMENT_SLOT_BUTTON_HEIGHT: f32 = ui_style::grid_f32(5);
const INSTRUMENT_SLOT_WIDTH: f32 = 112.0;
const INSTRUMENT_SLOT_LABEL_MAX_LEN: usize = 11;
const INSTRUMENT_BROWSER_WIDTH: f32 = 520.0;
const INSTRUMENT_BROWSER_HEIGHT: f32 = 360.0;
const INSTRUMENT_BROWSER_ICON_SIZE: f32 = ui_style::grid_f32(3);
const SECTION_HEADER_HEIGHT: f32 = 24.0;
const STRIP_MIN_HEIGHT: f32 = 140.0;
const STRIP_TOGGLE_SIZE: f32 = INSTRUMENT_PICKER_HEIGHT - 4.0;
const TRACK_TITLE_EDITOR_HEIGHT: f32 = ui_style::grid_f32(5);
const TRACK_TITLE_EDITOR_CONTROL_HEIGHT: f32 = TRACK_TITLE_EDITOR_HEIGHT;
const TRACK_TITLE_EDITOR_SWATCH_SIZE: f32 = TRACK_TITLE_EDITOR_HEIGHT;
const TRACK_TITLE_EDITOR_INPUT_PADDING_V: u16 = 2;
const TRACK_TITLE_EDITOR_INPUT_PADDING_H: u16 = ui_style::grid(1);
const COMPACT_GAIN_SWITCH_OFFSET: f32 = 20.0;
const VALUE_LABEL_HEIGHT: f32 = ui_style::grid_f32(4);
const HEADER_SIDE_INSET: f32 = 12.0;
const SECTION_BODY_GAP: f32 = 0.0;
const METER_STACK_SPACING: f32 = ui_style::grid_f32(2);
const STRIP_STACK_SPACING: f32 = ui_style::grid_f32(1);
const LABEL_CONTROL_SPACING: f32 = ui_style::SPACE_XS as f32;
const TITLE_TOP_SPACING: f32 = ui_style::grid_f32(3);
const STRIP_VIRTUALIZATION_OVERSCAN: usize = 2;

pub(super) fn instrument_track_scroll_x(track_index: usize) -> f32 {
    strip_span_width(track_index)
}

struct StripActions<'a> {
    solo: Option<(bool, Message)>,
    mute: Option<(bool, Message)>,
    on_gain: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    on_pan: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

fn noop_message() -> Message {
    Message::Noop
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum InstrumentChoice {
    None,
    Processor {
        processor_id: String,
        name: String,
        backend: InstrumentBrowserBackend,
    },
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(super) enum InstrumentBrowserBackend {
    BuiltIn,
    Clap,
    Vst3,
}

impl InstrumentBrowserBackend {
    fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "Built-in",
            Self::Clap => "CLAP",
            Self::Vst3 => "VST3",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstrumentBrowserEntries {
    show_none: bool,
    entries: Vec<InstrumentChoice>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MainStripDependency {
    gain_bits: u32,
    pan_bits: u32,
    meter: MeterDependency,
    compact_gain: bool,
    strip_height_bits: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TrackStripDependency {
    index: usize,
    name: String,
    selected: Option<InstrumentChoice>,
    editor_enabled: bool,
    color_bits: [u32; 4],
    gain_bits: u32,
    pan_bits: u32,
    meter: MeterDependency,
    compact_gain: bool,
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
struct BusStripDependency {
    id: u16,
    name: String,
    gain_bits: u32,
    pan_bits: u32,
    meter: MeterDependency,
    compact_gain: bool,
    strip_height_bits: u32,
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

pub(super) fn content(app: &Lilypalooza) -> Element<'_, Message> {
    let colors = meter_colors(&app.theme);
    let existing_track_count = app
        .piano_roll
        .current_file()
        .map(|file| file.data.tracks.len())
        .unwrap_or(0);
    let track_colors: Vec<_> = (0..existing_track_count)
        .map(|track_index| app.effective_track_color(track_index))
        .collect();
    let renaming_target = app.renaming_target;
    let renaming_origin = app.renaming_origin;
    let track_rename_value = app.track_rename_value.clone();
    let track_rename_color_value = app.track_rename_color_value;
    let track_rename_color_picker_open = app.track_rename_color_picker_open;

    if let Some(playback) = app.playback.as_ref() {
        let mixer = playback.mixer_state();

        return responsive(move |size| {
            let gain_mode = gain_control_mode(size.height);
            let strip_height =
                (size.height - (ui_style::PADDING_SM as f32 * 2.0) - SECTION_HEADER_HEIGHT)
                    .max(STRIP_MIN_HEIGHT);
            let instrument_visible = visible_strip_window(
                mixer.tracks().len(),
                app.mixer_instrument_scroll_x,
                app.mixer_instrument_viewport_width.max(size.width * 0.5),
            );
            let bus_visible = visible_strip_window(
                mixer.buses().len(),
                app.mixer_bus_scroll_x,
                app.mixer_bus_viewport_width.max(size.width * 0.2),
            );
            let meter_window =
                playback.meter_snapshot_window(instrument_visible.clone(), bus_visible.clone());

            row![
                container(master_track_area(
                    mixer,
                    meter_window.main,
                    colors,
                    strip_height,
                    gain_mode,
                    true,
                ))
                .width(Length::Fixed(MAIN_SECTION_WIDTH))
                .height(Fill)
                .style(ui_style::mixer_side_group_surface),
                container(instrument_track_area(
                    mixer,
                    &meter_window,
                    colors,
                    strip_height,
                    gain_mode,
                    instrument_visible,
                    existing_track_count,
                    &track_colors,
                    renaming_target,
                    renaming_origin,
                    &track_rename_value,
                    track_rename_color_value,
                    track_rename_color_picker_open,
                    app.selected_track_index,
                    true,
                ))
                .width(FillPortion(5))
                .height(Fill)
                .style(ui_style::mixer_instrument_group_surface),
                container(bus_track_area(
                    mixer,
                    &meter_window,
                    colors,
                    strip_height,
                    gain_mode,
                    bus_visible,
                    renaming_target,
                    renaming_origin,
                    &track_rename_value,
                    true,
                ))
                .width(FillPortion(2))
                .height(Fill)
                .style(ui_style::mixer_side_group_surface)
            ]
            .spacing(ui_style::SPACE_SM)
            .padding(ui_style::PADDING_SM)
            .width(Fill)
            .height(Fill)
            .into()
        })
        .into();
    }

    content_without_audio()
}

fn content_without_audio() -> Element<'static, Message> {
    container(
        text("Audio engine disabled")
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill)
    .style(ui_style::mixer_instrument_group_surface)
    .into()
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn mixer_layout_without_audio<'a>(
    mixer: &'a MixerState,
    meter_snapshot: &'a MixerMeterSnapshot,
    colors: MeterColors,
    size: iced::Size,
    instrument_scroll_x: f32,
    instrument_viewport_width: f32,
    bus_scroll_x: f32,
    bus_viewport_width: f32,
    renaming_target: Option<super::RenameTarget>,
    track_rename_value: String,
    selected_track_index: Option<usize>,
) -> Element<'a, Message> {
    let existing_track_count = 0;
    let gain_mode = gain_control_mode(size.height);
    let strip_height = (size.height - (ui_style::PADDING_SM as f32 * 2.0) - SECTION_HEADER_HEIGHT)
        .max(STRIP_MIN_HEIGHT);
    let instrument_visible = visible_strip_window(
        mixer.tracks().len(),
        instrument_scroll_x,
        instrument_viewport_width.max(size.width * 0.5),
    );
    let bus_visible = visible_strip_window(
        mixer.buses().len(),
        bus_scroll_x,
        bus_viewport_width.max(size.width * 0.2),
    );
    let meter_window = MixerMeterSnapshotWindow {
        main: meter_snapshot.main,
        tracks: meter_snapshot.tracks[instrument_visible.clone()].to_vec(),
        buses: meter_snapshot.buses[bus_visible.clone()]
            .iter()
            .map(|(_, snapshot)| *snapshot)
            .collect(),
    };

    row![
        container(master_track_area(
            mixer,
            meter_window.main,
            colors,
            strip_height,
            gain_mode,
            false,
        ))
        .width(Length::Fixed(MAIN_SECTION_WIDTH))
        .height(Fill)
        .style(ui_style::mixer_side_group_surface),
        container(instrument_track_area(
            mixer,
            &meter_window,
            colors,
            strip_height,
            gain_mode,
            instrument_visible,
            existing_track_count,
            &[],
            renaming_target,
            None,
            &track_rename_value,
            Color::TRANSPARENT,
            false,
            selected_track_index,
            false,
        ))
        .width(FillPortion(5))
        .height(Fill)
        .style(ui_style::mixer_instrument_group_surface),
        container(bus_track_area(
            mixer,
            &meter_window,
            colors,
            strip_height,
            gain_mode,
            bus_visible,
            renaming_target,
            None,
            &track_rename_value,
            false,
        ))
        .width(FillPortion(2))
        .height(Fill)
        .style(ui_style::mixer_side_group_surface)
    ]
    .spacing(ui_style::SPACE_SM)
    .padding(ui_style::PADDING_SM)
    .width(Fill)
    .height(Fill)
    .into()
}

fn master_track_area(
    mixer: &MixerState,
    meter_snapshot: StripMeterSnapshot,
    colors: MeterColors,
    strip_height: f32,
    gain_mode: GainControlMode,
    controls_enabled: bool,
) -> Element<'_, Message> {
    let master_row = row![sticky_master_strip(
        mixer,
        meter_snapshot,
        colors,
        strip_height,
        gain_mode,
        controls_enabled,
    )]
    .align_y(alignment::Vertical::Top)
    .height(Length::Fixed(strip_height));

    column![
        container(section_header_bar(row![section_title("Main")]))
            .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(master_row)
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .style(ui_style::workspace_scrollable)
                .width(Fill)
                .height(Fill),
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
        ]
        .height(Fill),
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(1.0))
            .style(ui_style::chrome_separator)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

fn sticky_master_strip(
    mixer: &MixerState,
    meter_snapshot: StripMeterSnapshot,
    colors: MeterColors,
    strip_height: f32,
    gain_mode: GainControlMode,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let master = mixer.master();
    lazy(
        MainStripDependency {
            gain_bits: master.state.gain_db.to_bits(),
            pan_bits: master.state.pan.to_bits(),
            meter: MeterDependency::from_snapshot(meter_snapshot),
            compact_gain: matches!(gain_mode, GainControlMode::Knob),
            strip_height_bits: strip_height.to_bits(),
        },
        move |dependency| {
            let gain_mode = if dependency.compact_gain {
                GainControlMode::Knob
            } else {
                GainControlMode::Fader
            };
            strip_panel(
                strip_shell(
                    container(section_title("Main"))
                        .width(Fill)
                        .center_x(Fill)
                        .into(),
                    None,
                    f32::from_bits(dependency.gain_bits),
                    f32::from_bits(dependency.pan_bits),
                    meter_stack(
                        MeterStackDependency {
                            meter: dependency.meter,
                            colors: MeterColorsDependency::from_colors(colors),
                            compact_gain: dependency.compact_gain,
                            strip_height_bits: dependency.strip_height_bits,
                        },
                        Some(if controls_enabled {
                            Message::Mixer(MixerMessage::ResetMasterMeter)
                        } else {
                            noop_message()
                        }),
                    ),
                    StripActions {
                        solo: None,
                        mute: None,
                        on_gain: Some(Box::new(move |value| {
                            if controls_enabled {
                                Message::Mixer(MixerMessage::SetMasterGain(value))
                            } else {
                                noop_message()
                            }
                        })),
                        on_pan: Some(Box::new(move |value| {
                            if controls_enabled {
                                Message::Mixer(MixerMessage::SetMasterPan(value))
                            } else {
                                noop_message()
                            }
                        })),
                    },
                    f32::from_bits(dependency.strip_height_bits),
                    gain_mode,
                ),
                MAIN_STRIP_WIDTH,
                f32::from_bits(dependency.strip_height_bits),
                false,
                None,
            )
        },
    )
    .into()
}

#[allow(clippy::too_many_arguments)]
fn instrument_track_area(
    mixer: &MixerState,
    meters: &MixerMeterSnapshotWindow,
    colors: MeterColors,
    strip_height: f32,
    gain_mode: GainControlMode,
    visible: std::ops::Range<usize>,
    existing_track_count: usize,
    track_colors: &[Color],
    renaming_target: Option<super::RenameTarget>,
    renaming_origin: Option<super::WorkspacePaneKind>,
    track_rename_value: &str,
    track_rename_color_value: Color,
    track_rename_color_picker_open: bool,
    selected_track_index: Option<usize>,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = strip_span_width(mixer.tracks().len().saturating_sub(visible.end));
    let track_row = mixer.tracks()[visible.clone()].iter().enumerate().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height))
            .push(horizontal_spacer(left_spacer)),
        move |row, (local_index, track)| {
            let track_index = visible.start + local_index;
            let selected_choice = selected_instrument_choice(track.instrument_slot(), mixer);
            let track_color = track_colors
                .get(track_index)
                .copied()
                .unwrap_or_else(|| crate::track_colors::default_track_color(track_index));
            let meter_dependency = MeterStackDependency {
                meter: MeterDependency::from_snapshot(
                    meters.tracks.get(local_index).copied().unwrap_or_default(),
                ),
                colors: MeterColorsDependency::from_colors(colors),
                compact_gain: matches!(gain_mode, GainControlMode::Knob),
                strip_height_bits: strip_height.to_bits(),
            };
            row.push(lazy(
                TrackStripDependency {
                    index: track_index,
                    name: track.name.clone(),
                    selected: selected_choice.clone(),
                    editor_enabled: track
                        .instrument_slot()
                        .filter(|slot| !slot.is_empty())
                        .and_then(|slot| slot.descriptor())
                        .and_then(|descriptor| descriptor.editor)
                        .is_some(),
                    color_bits: color_bits(track_color),
                    gain_bits: track.state.gain_db.to_bits(),
                    pan_bits: track.state.pan.to_bits(),
                    meter: meter_dependency.meter,
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: track.state.soloed,
                    muted: track.state.muted,
                    tint_enabled: track_should_use_roll_tint(track_index, existing_track_count),
                    highlighted: selected_track_index == Some(track_index),
                    renaming: renaming_target == Some(super::RenameTarget::Track(track_index))
                        && renaming_origin == Some(super::WorkspacePaneKind::Mixer),
                    rename_value: track_rename_value.to_string(),
                    color_picker_open: renaming_target
                        == Some(super::RenameTarget::Track(track_index))
                        && renaming_origin == Some(super::WorkspacePaneKind::Mixer)
                        && track_rename_color_picker_open,
                },
                move |dependency| {
                    let name = dependency.name.clone();
                    let is_selected = dependency.highlighted;
                    let track_color = if dependency.renaming {
                        track_rename_color_value
                    } else {
                        color_from_bits(dependency.color_bits)
                    };
                    let strip_height = f32::from_bits(dependency.strip_height_bits);
                    let gain_mode = if dependency.compact_gain {
                        GainControlMode::Knob
                    } else {
                        GainControlMode::Fader
                    };
                    let shell = strip_shell(
                        track_title_content(
                            track_index,
                            &name,
                            dependency.renaming,
                            &dependency.rename_value,
                            track_color,
                            dependency.color_picker_open,
                        ),
                        Some({
                            instrument_slot_controls(
                                track_index,
                                dependency.selected.clone(),
                                dependency.editor_enabled,
                                controls_enabled,
                            )
                        }),
                        f32::from_bits(dependency.gain_bits),
                        f32::from_bits(dependency.pan_bits),
                        meter_stack(
                            meter_dependency,
                            Some(if controls_enabled {
                                Message::Mixer(MixerMessage::ResetTrackMeter(track_index))
                            } else {
                                noop_message()
                            }),
                        ),
                        StripActions {
                            solo: Some((
                                dependency.soloed,
                                if controls_enabled {
                                    Message::Mixer(MixerMessage::ToggleTrackSolo(track_index))
                                } else {
                                    noop_message()
                                },
                            )),
                            mute: Some((
                                dependency.muted,
                                if controls_enabled {
                                    Message::Mixer(MixerMessage::ToggleTrackMute(track_index))
                                } else {
                                    noop_message()
                                },
                            )),
                            on_gain: Some(Box::new(move |value| {
                                if controls_enabled {
                                    Message::Mixer(MixerMessage::SetTrackGain(track_index, value))
                                } else {
                                    noop_message()
                                }
                            })),
                            on_pan: Some(Box::new(move |value| {
                                if controls_enabled {
                                    Message::Mixer(MixerMessage::SetTrackPan(track_index, value))
                                } else {
                                    noop_message()
                                }
                            })),
                        },
                        strip_height,
                        gain_mode,
                    );

                    if dependency.tint_enabled {
                        tinted_track_strip_panel(
                            shell,
                            STRIP_WIDTH,
                            strip_height,
                            track_color,
                            is_selected,
                            Some(Message::Mixer(MixerMessage::SelectTrack(track_index))),
                        )
                    } else {
                        strip_panel(
                            shell,
                            STRIP_WIDTH,
                            strip_height,
                            is_selected,
                            Some(Message::Mixer(MixerMessage::SelectTrack(track_index))),
                        )
                    }
                },
            ))
        },
    );
    let track_row = track_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(row![section_title("Instrument Tracks")]))
            .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(track_row)
                .id(instrument_scroll_id())
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .on_scroll(
                    |viewport| Message::Mixer(MixerMessage::InstrumentViewportScrolled(viewport))
                )
                .style(ui_style::workspace_scrollable)
                .width(Fill)
                .height(Fill),
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
        ]
        .height(Fill),
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(1.0))
            .style(ui_style::chrome_separator)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

#[allow(clippy::too_many_arguments)]
fn bus_track_area(
    mixer: &MixerState,
    meters: &MixerMeterSnapshotWindow,
    colors: MeterColors,
    strip_height: f32,
    gain_mode: GainControlMode,
    visible: std::ops::Range<usize>,
    renaming_target: Option<super::RenameTarget>,
    renaming_origin: Option<super::WorkspacePaneKind>,
    track_rename_value: &str,
    controls_enabled: bool,
) -> Element<'static, Message> {
    if mixer.buses().is_empty() {
        return column![
            container(section_header_bar(row![section_title("Buses")]))
                .style(ui_style::workspace_toolbar_surface),
            container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
            row![
                container(text(""))
                    .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                    .height(Fill)
                    .style(ui_style::chrome_separator),
                container(add_bus_button(controls_enabled))
                    .width(Fill)
                    .height(Fill)
                    .center_x(Fill)
                    .center_y(Fill),
                container(text(""))
                    .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                    .height(Fill)
                    .style(ui_style::chrome_separator),
            ]
            .height(Fill),
            container(text(""))
                .width(Fill)
                .height(Length::Fixed(1.0))
                .style(ui_style::chrome_separator)
        ]
        .spacing(0)
        .height(Fill)
        .into();
    }

    let total_buses = mixer.buses().len();
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = if visible.end < total_buses {
        strip_span_width(total_buses.saturating_sub(visible.end) + 1)
    } else {
        0.0
    };
    let bus_row = mixer.buses()[visible.clone()].iter().enumerate().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height))
            .push(horizontal_spacer(left_spacer)),
        |row, (local_index, bus)| {
            let Some(bus_id) = bus.bus_id else {
                return row;
            };
            let meter_dependency = MeterStackDependency {
                meter: MeterDependency::from_snapshot(
                    meters.buses.get(local_index).copied().unwrap_or_default(),
                ),
                colors: MeterColorsDependency::from_colors(colors),
                compact_gain: matches!(gain_mode, GainControlMode::Knob),
                strip_height_bits: strip_height.to_bits(),
            };
            row.push(lazy(
                BusStripDependency {
                    id: bus_id.0,
                    name: bus.name.clone(),
                    gain_bits: bus.state.gain_db.to_bits(),
                    pan_bits: bus.state.pan.to_bits(),
                    meter: meter_dependency.meter,
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: bus.state.soloed,
                    muted: bus.state.muted,
                    renaming: renaming_target == Some(super::RenameTarget::Bus(bus_id.0))
                        && renaming_origin == Some(super::WorkspacePaneKind::Mixer),
                    rename_value: track_rename_value.to_string(),
                },
                move |dependency| {
                    let name = dependency.name.clone();
                    let bus_id = dependency.id;
                    let gain_db = f32::from_bits(dependency.gain_bits);
                    let pan = f32::from_bits(dependency.pan_bits);
                    let strip_height = f32::from_bits(dependency.strip_height_bits);
                    let soloed = dependency.soloed;
                    let muted = dependency.muted;
                    let gain_mode = if dependency.compact_gain {
                        GainControlMode::Knob
                    } else {
                        GainControlMode::Fader
                    };
                    let base_strip = strip_panel(
                        strip_shell(
                            bus_title_content(
                                bus_id,
                                &name,
                                dependency.renaming,
                                &dependency.rename_value,
                            ),
                            None,
                            gain_db,
                            pan,
                            meter_stack(
                                meter_dependency,
                                Some(if controls_enabled {
                                    Message::Mixer(MixerMessage::ResetBusMeter(bus_id))
                                } else {
                                    noop_message()
                                }),
                            ),
                            StripActions {
                                solo: Some((
                                    soloed,
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::ToggleBusSolo(bus_id))
                                    } else {
                                        noop_message()
                                    },
                                )),
                                mute: Some((
                                    muted,
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::ToggleBusMute(bus_id))
                                    } else {
                                        noop_message()
                                    },
                                )),
                                on_gain: Some(Box::new(move |value| {
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::SetBusGain(bus_id, value))
                                    } else {
                                        noop_message()
                                    }
                                })),
                                on_pan: Some(Box::new(move |value| {
                                    if controls_enabled {
                                        Message::Mixer(MixerMessage::SetBusPan(bus_id, value))
                                    } else {
                                        noop_message()
                                    }
                                })),
                            },
                            strip_height,
                            gain_mode,
                        ),
                        STRIP_WIDTH,
                        strip_height,
                        false,
                        None,
                    );
                    let remove_button: Element<'static, Message> = container(
                        ui_style::flat_icon_button(
                            icons::x(),
                            ui_style::grid_f32(4),
                            ui_style::grid_f32(3),
                            ui_style::button_flat_compact_control,
                            ui_style::svg_dimmed_control,
                        )
                        .on_press(Message::Mixer(MixerMessage::RemoveBus(bus_id))),
                    )
                    .width(Fill)
                    .height(Fill)
                    .align_x(alignment::Horizontal::Right)
                    .align_y(alignment::Vertical::Top)
                    .padding([ui_style::grid(2), ui_style::grid(2)])
                    .into();
                    let layered: Element<'static, Message> =
                        stack([base_strip, remove_button]).into();
                    layered
                },
            ))
        },
    );
    let bus_row = if visible.end >= total_buses {
        bus_row.push(add_bus_lane(strip_height, controls_enabled))
    } else {
        bus_row
    };
    let bus_row = bus_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(row![section_title("Buses")]))
            .style(ui_style::workspace_toolbar_surface),
        container(text("")).height(Length::Fixed(SECTION_BODY_GAP)),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(bus_row)
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .on_scroll(|viewport| Message::Mixer(MixerMessage::BusViewportScrolled(viewport)))
                .style(ui_style::workspace_scrollable)
                .width(Fill)
                .height(Fill),
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
        ]
        .height(Fill),
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(1.0))
            .style(ui_style::chrome_separator)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

fn add_bus_button(controls_enabled: bool) -> Element<'static, Message> {
    ui_style::flat_icon_button(
        icons::plus(),
        ui_style::grid_f32(7),
        ui_style::grid_f32(4),
        ui_style::button_flat_compact_control,
        ui_style::svg_muted_control,
    )
    .on_press_maybe(Some(if controls_enabled {
        Message::Mixer(MixerMessage::AddBus)
    } else {
        noop_message()
    }))
    .into()
}

fn add_bus_lane(strip_height: f32, controls_enabled: bool) -> Element<'static, Message> {
    container(add_bus_button(controls_enabled))
        .width(Length::Fixed(STRIP_WIDTH))
        .height(Length::Fixed(strip_height))
        .center_x(Length::Fixed(STRIP_WIDTH))
        .center_y(Length::Fixed(strip_height))
        .into()
}

fn section_title<'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..fonts::UI
            }),
    )
    .height(Length::Fixed(SECTION_HEADER_HEIGHT))
    .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    .into()
}

fn visible_strip_window(
    total: usize,
    scroll_x: f32,
    viewport_width: f32,
) -> std::ops::Range<usize> {
    if total == 0 {
        return 0..0;
    }

    let stride = STRIP_WIDTH + STRIP_SPACING;
    let first_visible = (scroll_x.max(0.0) / stride.max(1.0)).floor() as usize;
    let visible_count = ((viewport_width.max(stride) / stride.max(1.0)).ceil() as usize)
        .saturating_add(STRIP_VIRTUALIZATION_OVERSCAN * 2);
    let start = first_visible
        .saturating_sub(STRIP_VIRTUALIZATION_OVERSCAN)
        .min(total);
    let end = start.saturating_add(visible_count).min(total);
    start..end
}

fn strip_span_width(count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * STRIP_WIDTH + count.saturating_sub(1) as f32 * STRIP_SPACING
    }
}

fn horizontal_spacer(width: f32) -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(width.max(0.0)))
        .height(Fill)
        .into()
}

fn section_header_bar<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(row![
        container(text("")).width(Length::Fixed(HEADER_SIDE_INSET)),
        container(content.into())
            .height(Length::Fixed(SECTION_HEADER_HEIGHT))
            .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    ])
    .align_y(alignment::Vertical::Center)
    .height(Length::Fixed(SECTION_HEADER_HEIGHT))
    .width(Fill)
    .center_y(Length::Fixed(SECTION_HEADER_HEIGHT))
    .into()
}

fn value_label_slot<'a>(
    width: f32,
    label: impl Into<String>,
    color: Option<iced::Color>,
) -> Element<'a, Message> {
    let text = text(label.into()).size(ui_style::FONT_SIZE_UI_XS.saturating_sub(1));
    let text = if let Some(color) = color {
        text.color(color)
    } else {
        text
    };

    container(text)
        .width(Length::Fixed(width))
        .height(Length::Fixed(VALUE_LABEL_HEIGHT))
        .center_x(Length::Fixed(width))
        .align_y(alignment::Vertical::Bottom)
        .into()
}

fn gain_label(gain_db: f32) -> String {
    if gain_db <= GAIN_MIN_DB {
        "-inf".to_string()
    } else {
        format!("{gain_db:.1}")
    }
}

#[allow(clippy::too_many_arguments)]
fn strip_shell<'a>(
    title: Element<'a, Message>,
    instrument_picker: Option<Element<'a, Message>>,
    gain_db: f32,
    pan: f32,
    meter_stack: Element<'a, Message>,
    actions: StripActions<'a>,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'a, Message> {
    let mut content = column![]
        .spacing(STRIP_STACK_SPACING)
        .align_x(alignment::Horizontal::Center)
        .width(Fill);

    content = content.push(
        container(instrument_picker.unwrap_or_else(|| container(text("")).into()))
            .width(Fill)
            .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT)),
    );

    if let Some(on_pan) = actions.on_pan {
        content = content.push(
            column![
                container(text("")).height(Length::Fixed(ui_style::SPACE_XS as f32)),
                value_label_slot(INSTRUMENT_PICKER_HEIGHT, format!("{:+.2}", pan), None),
                pan_knob(pan, on_pan),
            ]
            .spacing(LABEL_CONTROL_SPACING)
            .align_x(alignment::Horizontal::Center),
        );
    }

    if let Some(on_gain) = actions.on_gain {
        let control_height = gain_control_height(strip_height, gain_mode);
        let gain_width = gain_control_width(matches!(gain_mode, GainControlMode::Knob));
        let stack_height = control_stack_height(control_height);

        let gain_control = match gain_mode {
            GainControlMode::Fader => container(gain_fader(gain_db, on_gain))
                .width(Fill)
                .height(Length::Fixed(control_height))
                .center_x(Fill)
                .into(),
            GainControlMode::Knob => gain_knob(gain_db, on_gain),
        };

        content = content.push(
            row![
                column![
                    value_label_slot(gain_width, gain_label(gain_db), None),
                    container(gain_control)
                        .width(Length::Fixed(gain_width))
                        .height(Length::Fixed(control_height))
                        .center_x(Length::Fixed(gain_width))
                        .align_y(alignment::Vertical::Bottom),
                ]
                .spacing(LABEL_CONTROL_SPACING)
                .height(Length::Fixed(stack_height))
                .align_x(alignment::Horizontal::Center)
                .width(Length::Shrink),
                meter_stack
            ]
            .spacing(METER_STACK_SPACING)
            .height(Length::Fixed(stack_height))
            .width(Length::Shrink),
        );
    }

    content = content.push(
        container(
            row![
                actions
                    .mute
                    .map_or_else(strip_toggle_placeholder, |(active, message)| {
                        strip_toggle_button("M", active, message)
                    },),
                actions
                    .solo
                    .map_or_else(strip_toggle_placeholder, |(active, message)| {
                        strip_toggle_button("S", active, message)
                    },),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .center_x(Fill),
    );

    let title = container(title)
        .width(Fill)
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .align_y(alignment::Vertical::Center);

    container(
        column![
            container(content).width(Fill),
            container(text("")).height(Length::Fixed(TITLE_TOP_SPACING)),
            title,
        ]
        .spacing(0)
        .width(Fill),
    )
    .padding(ui_style::PADDING_SM)
    .width(Fill)
    .height(Length::Fixed(strip_height))
    .style(ui_style::transparent_surface)
    .into()
}

fn track_title_content<'a>(
    track_index: usize,
    title: &str,
    renaming: bool,
    rename_value: &str,
    color: Color,
    color_picker_open: bool,
) -> Element<'a, Message> {
    if renaming {
        let swatch = button(
            container(text(""))
                .width(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE))
                .height(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE)),
        )
        .padding(0)
        .width(Length::Fixed(TRACK_TITLE_EDITOR_SWATCH_SIZE))
        .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
        .style(move |theme, status| ui_style::track_color_swatch_button(theme, status, color))
        .on_press(Message::Mixer(MixerMessage::OpenTrackColorPicker));
        let input = text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
            .id(Id::new(super::TRACK_RENAME_INPUT_ID))
            .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
            .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
            .style(ui_style::track_name_input)
            .size(ui_style::FONT_SIZE_UI_SM)
            .padding([
                TRACK_TITLE_EDITOR_INPUT_PADDING_V,
                TRACK_TITLE_EDITOR_INPUT_PADDING_H,
            ])
            .width(Fill);
        let focused = color_picker_open;
        let editor_row = container(
            row![
                swatch,
                container(text(""))
                    .width(1)
                    .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
                    .style(move |theme| { ui_style::track_name_editor_divider(theme, focused) }),
                container(input)
                    .height(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
                    .center_y(Length::Fixed(TRACK_TITLE_EDITOR_CONTROL_HEIGHT))
            ]
            .spacing(0)
            .align_y(alignment::Vertical::Center)
            .width(Fill),
        )
        .padding(0)
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .style(move |theme| ui_style::track_name_editor_shell(theme, focused))
        .width(Fill);
        return color_picker_with_change(
            color_picker_open,
            color,
            editor_row,
            Message::Mixer(MixerMessage::CancelTrackRename),
            |color| Message::Mixer(MixerMessage::SubmitTrackColor(color)),
            |color| Message::Mixer(MixerMessage::PreviewTrackColor(color)),
        )
        .style(ui_style::color_picker_widget_style)
        .into();
    }

    mouse_area(
        container(
            text(crate::track_names::ellipsize_middle(title, 18))
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                })
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .center_x(Fill),
    )
    .on_press(Message::Mixer(MixerMessage::StartTrackRename(track_index)))
    .into()
}

fn bus_title_content<'a>(
    bus_id: u16,
    title: &str,
    renaming: bool,
    rename_value: &str,
) -> Element<'a, Message> {
    if renaming {
        return container(
            text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
                .id(Id::new(super::TRACK_RENAME_INPUT_ID))
                .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
                .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
                .size(ui_style::FONT_SIZE_UI_SM)
                .padding([
                    TRACK_TITLE_EDITOR_INPUT_PADDING_V,
                    TRACK_TITLE_EDITOR_INPUT_PADDING_H,
                ])
                .width(Fill),
        )
        .height(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .center_y(Length::Fixed(TRACK_TITLE_EDITOR_HEIGHT))
        .into();
    }

    mouse_area(
        container(
            text(crate::track_names::ellipsize_middle(title, 18))
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                })
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .center_x(Fill),
    )
    .on_press(Message::Mixer(MixerMessage::StartBusRename(bus_id)))
    .into()
}

fn meter_stack<'a>(
    dependency: MeterStackDependency,
    meter_reset: Option<Message>,
) -> Element<'a, Message> {
    lazy(dependency, move |dependency| -> Element<'static, Message> {
        let gain_mode = if dependency.compact_gain {
            GainControlMode::Knob
        } else {
            GainControlMode::Fader
        };
        let strip_height = f32::from_bits(dependency.strip_height_bits);
        let meter_snapshot = dependency.meter.snapshot();
        let meter_colors = dependency.colors.colors();
        let meter_height = meter_control_height(strip_height, gain_mode);
        let meter_width = stereo_meter_width(meter_scale_visible(gain_mode));
        let meter_bar_width = stereo_meter_bar_width();
        let meter_scale_width = (meter_width - meter_bar_width).max(0.0);
        let meter_label = meter_peak_label(meter_snapshot);
        let meter_label_color = if meter_snapshot.clip_latched {
            meter_colors.clip
        } else {
            meter_colors.scale_text
        };
        let meter = if meter_scale_visible(gain_mode) {
            stereo_meter_with_scale(meter_snapshot, meter_colors, meter_height)
        } else {
            stereo_meter(meter_snapshot, meter_colors, meter_height)
        };
        let meter = if let Some(message) = meter_reset.clone() {
            mouse_area(meter).on_press(message).into()
        } else {
            meter
        };

        column![
            row![
                value_label_slot(meter_bar_width, meter_label, Some(meter_label_color)),
                container(text("")).width(Length::Fixed(meter_scale_width)),
            ]
            .width(Length::Fixed(meter_width))
            .height(Length::Fixed(VALUE_LABEL_HEIGHT))
            .align_y(alignment::Vertical::Bottom),
            container(meter)
                .width(Length::Fixed(meter_width))
                .height(Length::Fixed(meter_height))
                .center_x(Length::Fixed(meter_width))
                .align_y(alignment::Vertical::Bottom),
        ]
        .spacing(LABEL_CONTROL_SPACING)
        .height(Length::Fixed(control_stack_height(meter_height)))
        .align_x(alignment::Horizontal::Center)
        .width(Length::Shrink)
        .into()
    })
    .into()
}

fn gain_control_height(strip_height: f32, gain_mode: GainControlMode) -> f32 {
    match gain_mode {
        GainControlMode::Knob => 48.0,
        GainControlMode::Fader => (strip_height
            - (ui_style::PADDING_SM as f32 * 2.0)
            - SECTION_HEADER_HEIGHT
            - INSTRUMENT_PICKER_HEIGHT
            - STRIP_TOGGLE_SIZE
            - 42.0
            - (VALUE_LABEL_HEIGHT * 3.0)
            - (ui_style::SPACE_XS as f32 * 5.0))
            .max(96.0),
    }
}

fn meter_control_height(strip_height: f32, gain_mode: GainControlMode) -> f32 {
    gain_control_height(strip_height, gain_mode)
}

fn control_stack_height(control_height: f32) -> f32 {
    control_height + VALUE_LABEL_HEIGHT + ui_style::SPACE_XS as f32
}

fn meter_peak_label(snapshot: StripMeterSnapshot) -> String {
    let hold_db = snapshot.left.hold_db.max(snapshot.right.hold_db);
    if hold_db <= STRIP_METER_MIN_DB {
        "-inf".to_string()
    } else {
        format!("{hold_db:.1}")
    }
}

fn gain_control_mode(pane_height: f32) -> GainControlMode {
    if pane_height <= MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET {
        GainControlMode::Knob
    } else {
        GainControlMode::Fader
    }
}

fn meter_scale_visible(gain_mode: GainControlMode) -> bool {
    matches!(gain_mode, GainControlMode::Fader)
}

fn strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    selected: bool,
    on_select: Option<Message>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if let Some(message) = on_select {
        stack![
            mouse_area(container(text("")).width(Fill).height(Fill)).on_press(message),
            content
        ]
        .into()
    } else {
        content
    };

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |theme| ui_style::mixer_track_strip_surface(theme, None, selected))
        .into()
}

fn tinted_track_strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    track_color: Color,
    selected: bool,
    on_select: Option<Message>,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if let Some(message) = on_select {
        stack![
            mouse_area(container(text("")).width(Fill).height(Fill)).on_press(message),
            content
        ]
        .into()
    } else {
        content
    };

    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |theme| ui_style::mixer_track_strip_surface(theme, Some(track_color), selected))
        .into()
}

fn track_should_use_roll_tint(track_index: usize, existing_track_count: usize) -> bool {
    track_index < existing_track_count
}

fn strip_toggle_button(
    label: &'static str,
    active: bool,
    message: Message,
) -> Element<'static, Message> {
    button(
        container(text(label).size(ui_style::FONT_SIZE_UI_XS))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    )
    .style(if active {
        ui_style::button_compact_active
    } else {
        ui_style::button_compact_solid
    })
    .padding(0)
    .width(Length::Fixed(STRIP_TOGGLE_SIZE))
    .height(Length::Fixed(STRIP_TOGGLE_SIZE))
    .on_press(message)
    .into()
}

fn strip_toggle_placeholder() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(STRIP_TOGGLE_SIZE))
        .height(Length::Fixed(STRIP_TOGGLE_SIZE))
        .into()
}

fn instrument_slot_controls(
    track_index: usize,
    selected: Option<InstrumentChoice>,
    editor_enabled: bool,
    controls_enabled: bool,
) -> Element<'static, Message> {
    let primary_action = instrument_slot_primary_action(
        track_index,
        selected.as_ref(),
        editor_enabled,
        controls_enabled,
    );
    let secondary_action = controls_enabled.then_some(Message::Mixer(
        MixerMessage::ToggleTrackInstrumentBrowser(track_index),
    ));

    slot_selector_controls(
        instrument_trigger_label(selected.as_ref()),
        primary_action,
        secondary_action,
    )
}

fn slot_selector_controls(
    label: String,
    primary_action: Option<Message>,
    secondary_action: Option<Message>,
) -> Element<'static, Message> {
    let button: Element<'static, Message> = button(
        container(
            row![
                container(ui_style::icon(
                    icons::keyboard_music(),
                    INSTRUMENT_BROWSER_ICON_SIZE,
                    |theme, _status| { ui_style::svg_muted_control(theme, svg::Status::Idle) },
                ),)
                .width(Length::Fixed(INSTRUMENT_BROWSER_ICON_SIZE))
                .center_x(Length::Fixed(INSTRUMENT_BROWSER_ICON_SIZE))
                .center_y(Fill),
                container(
                    text(label)
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Fill)
                .height(Fill)
                .clip(true)
                .center_x(Fill)
                .align_y(alignment::Vertical::Center),
                container(text(""))
                    .width(Length::Fixed(INSTRUMENT_BROWSER_ICON_SIZE))
                    .height(Fill),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT)),
    )
    .style(|theme, status| ui_style::button_selector_field(theme, status, false))
    .padding([0, ui_style::grid(2)])
    .width(Length::Fixed(INSTRUMENT_SLOT_WIDTH))
    .height(Length::Fixed(INSTRUMENT_SLOT_BUTTON_HEIGHT))
    .on_press_maybe(primary_action)
    .into();

    let button = if let Some(message) = secondary_action {
        mouse_area(button).on_right_press(message).into()
    } else {
        button
    };

    container(button)
        .width(Fill)
        .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT))
        .center_x(Fill)
        .center_y(Length::Fixed(INSTRUMENT_PICKER_HEIGHT))
        .into()
}

fn instrument_slot_primary_action(
    track_index: usize,
    selected: Option<&InstrumentChoice>,
    editor_enabled: bool,
    controls_enabled: bool,
) -> Option<Message> {
    if !controls_enabled {
        return None;
    }

    match (selected, editor_enabled) {
        (Some(InstrumentChoice::Processor { .. }), true) => Some(Message::Mixer(
            MixerMessage::OpenEditor(super::processor_editor_windows::EditorTarget {
                strip_index: track_index + 1,
                slot_index: 0,
            }),
        )),
        (Some(InstrumentChoice::None) | None, _) => Some(Message::Mixer(
            MixerMessage::ToggleTrackInstrumentBrowser(track_index),
        )),
        (Some(InstrumentChoice::Processor { .. }), false) => None,
    }
}

pub(super) fn instrument_browser_overlay(app: &Lilypalooza) -> Element<'_, Message> {
    let Some(track_index) = app.open_instrument_browser_track else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let Some(playback) = app.playback.as_ref() else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let mixer = playback.mixer_state();
    let Some(track) = mixer.tracks().get(track_index) else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let choices = instrument_choices();
    let selected = selected_instrument_choice(track.instrument_slot(), mixer);

    let header = container(
        row![
            column![
                text("Choose Instrument")
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..fonts::UI
                    }),
                text(track.name.clone())
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO),
            ]
            .spacing(ui_style::SPACE_XS),
            container(text("")).width(Fill),
            ui_style::flat_icon_button(
                icons::x(),
                ui_style::grid_f32(5),
                ui_style::grid_f32(3),
                ui_style::button_pane_header_control,
                ui_style::svg_dimmed_control,
            )
            .width(Length::Fixed(ui_style::grid_f32(5)))
            .height(Length::Fixed(ui_style::grid_f32(5)))
            .on_press(Message::Mixer(MixerMessage::CloseTrackInstrumentBrowser)),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
    .style(ui_style::prompt_header);

    let tabs = row![
        instrument_browser_tab_button(
            InstrumentBrowserBackend::BuiltIn,
            app.instrument_browser_backend
        ),
        instrument_browser_tab_button(
            InstrumentBrowserBackend::Clap,
            app.instrument_browser_backend
        ),
        instrument_browser_tab_button(
            InstrumentBrowserBackend::Vst3,
            app.instrument_browser_backend
        ),
    ]
    .spacing(ui_style::SPACE_XS)
    .width(Fill);

    let search = text_input("Search instruments", &app.instrument_browser_search)
        .on_input(|value| Message::Mixer(MixerMessage::InstrumentBrowserSearchChanged(value)))
        .id(app.instrument_browser_search_input_id.clone())
        .style(ui_style::browser_search_input)
        .size(ui_style::FONT_SIZE_UI_SM)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
        .width(Fill);

    let body = match app.instrument_browser_backend {
        InstrumentBrowserBackend::BuiltIn => instrument_browser_built_in_list(
            track_index,
            &choices,
            selected.as_ref(),
            &app.instrument_browser_search,
        ),
        InstrumentBrowserBackend::Clap => instrument_browser_empty_state("No CLAP instruments yet"),
        InstrumentBrowserBackend::Vst3 => instrument_browser_empty_state("No VST3 instruments yet"),
    };

    let dialog = container(
        column![
            header,
            container(column![tabs, search, body].spacing(ui_style::SPACE_SM))
                .padding(ui_style::PADDING_SM)
        ]
        .spacing(0),
    )
    .width(Length::Fixed(INSTRUMENT_BROWSER_WIDTH))
    .style(ui_style::prompt_dialog);

    let centered_dialog = container(
        mouse_area(opaque(dialog))
            .on_press(Message::Noop)
            .interaction(iced::mouse::Interaction::Pointer),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill);

    let backdrop = mouse_area(
        container(centered_dialog)
            .width(Fill)
            .height(Fill)
            .style(ui_style::prompt_backdrop),
    )
    .on_press(Message::Mixer(MixerMessage::CloseTrackInstrumentBrowser));

    opaque(backdrop)
}

fn instrument_browser_tab_button(
    tab: InstrumentBrowserBackend,
    active: InstrumentBrowserBackend,
) -> Element<'static, Message> {
    button(text(tab.label()).size(ui_style::FONT_SIZE_UI_XS))
        .style(move |theme, status| ui_style::button_pane_tab(theme, status, tab == active))
        .padding([ui_style::grid(1), ui_style::grid(3)])
        .height(Length::Fixed(ui_style::grid_f32(6)))
        .on_press(Message::Mixer(
            MixerMessage::SelectInstrumentBrowserBackend(tab),
        ))
        .into()
}

fn instrument_browser_built_in_list(
    track_index: usize,
    choices: &[InstrumentChoice],
    selected: Option<&InstrumentChoice>,
    search: &str,
) -> Element<'static, Message> {
    let browser = instrument_browser_entries(choices, InstrumentBrowserBackend::BuiltIn, search);
    let InstrumentBrowserEntries { show_none, entries } = browser;
    let mut content = column![].spacing(0).width(Fill);
    if show_none {
        content = content.push(instrument_browser_choice_button(
            track_index,
            InstrumentChoice::None,
            selected == Some(&InstrumentChoice::None),
        ));
    }

    let has_entries = !entries.is_empty();
    for choice in entries {
        content = content.push(instrument_browser_choice_button(
            track_index,
            choice.clone(),
            selected == Some(&choice),
        ));
    }

    if !show_none && !has_entries {
        return instrument_browser_empty_state("No matching instruments");
    }

    scrollable(content)
        .height(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
        .style(ui_style::workspace_scrollable)
        .into()
}

fn instrument_browser_choice_button(
    track_index: usize,
    choice: InstrumentChoice,
    selected: bool,
) -> Element<'static, Message> {
    button(
        container(
            text(instrument_choice_primary_label(&choice))
                .size(ui_style::FONT_SIZE_UI_SM)
                .width(Fill)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Fill)
        .center_y(Fill),
    )
    .style(move |theme, status| ui_style::button_browser_entry(theme, status, selected))
    .padding([ui_style::grid(2), ui_style::grid(3)])
    .width(Fill)
    .on_press(Message::Mixer(MixerMessage::SelectTrackInstrument(
        track_index,
        choice,
    )))
    .into()
}

fn instrument_browser_empty_state(label: &'static str) -> Element<'static, Message> {
    container(
        text(label)
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO),
    )
    .width(Fill)
    .height(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
    .center_x(Fill)
    .center_y(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
    .into()
}

#[allow(clippy::too_many_arguments)]
fn instrument_choice_primary_label(choice: &InstrumentChoice) -> String {
    match choice {
        InstrumentChoice::None => "Empty".to_string(),
        InstrumentChoice::Processor { name, .. } => name.clone(),
    }
}

fn instrument_trigger_label(choice: Option<&InstrumentChoice>) -> String {
    crate::track_names::ellipsize_middle(
        &choice
            .map(instrument_choice_primary_label)
            .unwrap_or_else(|| "Empty".to_string()),
        INSTRUMENT_SLOT_LABEL_MAX_LEN,
    )
}

fn instrument_choice_search_haystack(choice: &InstrumentChoice) -> String {
    match choice {
        InstrumentChoice::None => "empty none no instrument".to_string(),
        InstrumentChoice::Processor { name, backend, .. } => {
            format!("{} {}", name.to_lowercase(), backend.label().to_lowercase())
        }
    }
}

fn instrument_browser_entries(
    choices: &[InstrumentChoice],
    active_backend: InstrumentBrowserBackend,
    search: &str,
) -> InstrumentBrowserEntries {
    if active_backend != InstrumentBrowserBackend::BuiltIn {
        return InstrumentBrowserEntries {
            show_none: false,
            entries: Vec::new(),
        };
    }

    let query = search.trim().to_lowercase();
    let matches = |choice: &InstrumentChoice| {
        query.is_empty() || instrument_choice_search_haystack(choice).contains(&query)
    };

    let mut entries = Vec::new();
    let mut show_none = false;
    for choice in choices {
        match choice {
            InstrumentChoice::None => {
                if matches(choice) {
                    show_none = true;
                }
            }
            InstrumentChoice::Processor { backend, .. } if *backend != active_backend => {}
            InstrumentChoice::Processor { .. } if matches(choice) => {
                entries.push(choice.clone());
            }
            InstrumentChoice::Processor { .. } => {}
        }
    }

    InstrumentBrowserEntries { show_none, entries }
}

fn instrument_choices() -> Vec<InstrumentChoice> {
    let mut choices = Vec::new();
    choices.push(InstrumentChoice::None);
    choices.extend(
        registry::all()
            .iter()
            .filter(|entry| entry.role == registry::Role::Instrument)
            .filter(|entry| entry.id != BUILTIN_NONE_ID && entry.id != BUILTIN_METRONOME_ID)
            .map(|entry| InstrumentChoice::Processor {
                processor_id: entry.id.to_string(),
                name: entry.name.to_string(),
                backend: match entry.backend {
                    registry::Backend::BuiltIn => InstrumentBrowserBackend::BuiltIn,
                    registry::Backend::Clap => InstrumentBrowserBackend::Clap,
                    registry::Backend::Vst3 => InstrumentBrowserBackend::Vst3,
                },
            }),
    );
    choices
}

fn selected_instrument_choice(
    slot: Option<&SlotState>,
    _mixer: &MixerState,
) -> Option<InstrumentChoice> {
    let slot = slot?;
    if slot.is_empty() {
        return Some(InstrumentChoice::None);
    }
    let entry = registry::resolve(&slot.kind)?;
    Some(InstrumentChoice::Processor {
        processor_id: entry.id.to_string(),
        name: entry.name.to_string(),
        backend: match entry.backend {
            registry::Backend::BuiltIn => InstrumentBrowserBackend::BuiltIn,
            registry::Backend::Clap => InstrumentBrowserBackend::Clap,
            registry::Backend::Vst3 => InstrumentBrowserBackend::Vst3,
        },
    })
}

#[cfg(test)]
mod tests {
    use crate::ui_style;
    use iced::widget::{container, row};
    use iced::{Color, Element, Length, Theme};
    use iced_test::Simulator;
    use lilypalooza_audio::mixer::MixerMeterSnapshotWindow;
    use lilypalooza_audio::{AudioEngine, AudioEngineOptions};
    use lilypalooza_audio::{BUILTIN_SOUNDFONT_ID, MixerState, SlotState};
    use std::path::{Path, PathBuf};

    use super::{
        COMPACT_GAIN_SWITCH_OFFSET, GROUP_SIDE_BORDER_WIDTH, GainControlMode,
        INSTRUMENT_PICKER_HEIGHT, InstrumentBrowserBackend, InstrumentChoice, MAIN_SECTION_WIDTH,
        MAIN_STRIP_WIDTH, MIXER_MIN_HEIGHT, MeterDependency, SECTION_BODY_GAP, STRIP_MIN_HEIGHT,
        STRIP_TOGGLE_SIZE, STRIP_VIRTUALIZATION_OVERSCAN, STRIP_WIDTH, StripMeterSnapshot,
        TITLE_TOP_SPACING, TRACK_TITLE_EDITOR_CONTROL_HEIGHT, TRACK_TITLE_EDITOR_HEIGHT,
        TRACK_TITLE_EDITOR_INPUT_PADDING_H, TRACK_TITLE_EDITOR_INPUT_PADDING_V,
        TRACK_TITLE_EDITOR_SWATCH_SIZE, TrackStripDependency, VALUE_LABEL_HEIGHT, color_bits,
        control_stack_height, gain_control_height, gain_control_mode, gain_label,
        instrument_browser_entries, instrument_slot_primary_action, instrument_trigger_label,
        meter_colors, meter_control_height, meter_peak_label, meter_scale_visible,
        selected_instrument_choice, track_should_use_roll_tint, visible_strip_window,
    };
    use crate::app::Message;
    use crate::app::messages::MixerMessage;
    use crate::icons;
    use lilypalooza_audio::mixer::ChannelMeterSnapshot;

    fn assert_snapshot_matches(
        ui: &mut iced_test::Simulator<'_, crate::app::Message>,
        baseline_name: &str,
    ) -> Result<(), iced_test::Error> {
        let snapshot = ui.snapshot(&Theme::Dark)?;
        let baseline_path = Path::new(baseline_name);

        assert!(
            snapshot.matches_hash(baseline_name)?,
            "snapshot hash mismatch for: {baseline_name}"
        );
        assert!(
            snapshot.matches_image(baseline_path)?,
            "snapshot image mismatch for: {baseline_name}"
        );

        Ok(())
    }

    fn assert_snapshots_equal(
        first: &mut iced_test::Simulator<'_, crate::app::Message>,
        second: &mut iced_test::Simulator<'_, crate::app::Message>,
        baseline_name: &str,
    ) -> Result<(), iced_test::Error> {
        let mut baseline_path = PathBuf::from("/tmp");
        baseline_path.push(baseline_name);

        let png = baseline_path.with_file_name(format!("{baseline_name}-wgpu.png"));
        let sha = baseline_path.with_file_name(format!("{baseline_name}-wgpu.sha256"));
        let _ = std::fs::remove_file(&png);
        let _ = std::fs::remove_file(&sha);

        let first_snapshot = first.snapshot(&Theme::Dark)?;
        assert!(first_snapshot.matches_hash(&baseline_path)?);
        assert!(first_snapshot.matches_image(&baseline_path)?);

        let second_snapshot = second.snapshot(&Theme::Dark)?;
        assert!(
            second_snapshot.matches_hash(&baseline_path)?,
            "snapshot hash mismatch for: {baseline_name}"
        );
        assert!(
            second_snapshot.matches_image(&baseline_path)?,
            "snapshot image mismatch for: {baseline_name}"
        );

        Ok(())
    }

    fn assert_snapshots_differ(
        first: &mut iced_test::Simulator<'_, crate::app::Message>,
        second: &mut iced_test::Simulator<'_, crate::app::Message>,
        baseline_name: &str,
    ) -> Result<(), iced_test::Error> {
        let mut baseline_path = PathBuf::from("/tmp");
        baseline_path.push(baseline_name);

        let png = baseline_path.with_file_name(format!("{baseline_name}-wgpu.png"));
        let sha = baseline_path.with_file_name(format!("{baseline_name}-wgpu.sha256"));
        let _ = std::fs::remove_file(&png);
        let _ = std::fs::remove_file(&sha);

        let first_snapshot = first.snapshot(&Theme::Dark)?;
        assert!(first_snapshot.matches_hash(&baseline_path)?);
        assert!(first_snapshot.matches_image(&baseline_path)?);

        let second_snapshot = second.snapshot(&Theme::Dark)?;
        assert!(
            !second_snapshot.matches_hash(&baseline_path)?,
            "snapshot hash unexpectedly matched for: {baseline_name}"
        );

        Ok(())
    }

    fn is_grid_multiple(value: f32) -> bool {
        ((value / 4.0).round() - (value / 4.0)).abs() < 1.0e-4
    }

    #[test]
    fn fixed_strip_sizes_follow_four_px_grid() {
        for value in [
            MAIN_STRIP_WIDTH,
            STRIP_WIDTH,
            INSTRUMENT_PICKER_HEIGHT,
            TRACK_TITLE_EDITOR_HEIGHT,
            VALUE_LABEL_HEIGHT,
            COMPACT_GAIN_SWITCH_OFFSET,
        ] {
            assert!(is_grid_multiple(value), "{value} should use the 4px grid");
        }
    }

    #[test]
    fn mixer_track_title_editor_height_increases_by_one_grid_unit() {
        assert_eq!(TRACK_TITLE_EDITOR_HEIGHT, ui_style::grid_f32(5));
    }

    #[test]
    fn mixer_track_title_editor_input_padding_matches_taller_height() {
        assert_eq!(TRACK_TITLE_EDITOR_INPUT_PADDING_V, 2);
        assert_eq!(TRACK_TITLE_EDITOR_INPUT_PADDING_H, ui_style::grid(1));
    }

    #[test]
    fn mixer_track_title_editor_controls_scale_with_taller_shell() {
        assert_eq!(TRACK_TITLE_EDITOR_CONTROL_HEIGHT, TRACK_TITLE_EDITOR_HEIGHT);
    }

    #[test]
    fn mixer_track_title_editor_swatch_is_square() {
        assert_eq!(TRACK_TITLE_EDITOR_SWATCH_SIZE, TRACK_TITLE_EDITOR_HEIGHT);
    }

    #[test]
    fn mixer_strip_title_gap_uses_three_grid_units() {
        assert_eq!(TITLE_TOP_SPACING, ui_style::grid_f32(3));
    }

    #[test]
    fn mixer_section_header_has_no_body_gap() {
        assert_eq!(SECTION_BODY_GAP, 0.0);
    }

    #[test]
    fn mixer_track_title_editor_widget_matches_snapshot() -> Result<(), iced_test::Error> {
        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [
                STRIP_WIDTH + ui_style::grid_f32(4),
                TRACK_TITLE_EDITOR_HEIGHT + ui_style::grid_f32(4),
            ],
            container(super::track_title_content(
                0,
                "Track 1",
                true,
                "Track 1",
                Color::from_rgb(0.42, 0.58, 0.86),
                false,
            ))
            .width(Length::Fixed(STRIP_WIDTH))
            .padding(ui_style::PADDING_SM),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_track_title_editor_widget_five_grid",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_strip_title_editor_matches_snapshot() -> Result<(), iced_test::Error> {
        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [
                STRIP_WIDTH + ui_style::grid_f32(8),
                STRIP_MIN_HEIGHT + ui_style::grid_f32(8),
            ],
            container(super::strip_shell(
                super::track_title_content(
                    0,
                    "Track 1",
                    true,
                    "Track 1",
                    Color::from_rgb(0.42, 0.58, 0.86),
                    false,
                ),
                None,
                0.0,
                0.0,
                container(iced::widget::text("")).into(),
                super::StripActions {
                    solo: None,
                    mute: None,
                    on_gain: None,
                    on_pan: None,
                },
                STRIP_MIN_HEIGHT,
                GainControlMode::Knob,
            ))
            .padding(ui_style::PADDING_SM),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_strip_title_editor_integration_five_grid",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_strip_controls_leave_gap_above_title_matches_snapshot() -> Result<(), iced_test::Error>
    {
        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [STRIP_WIDTH + ui_style::grid_f32(8), 520.0],
            container(super::strip_shell(
                super::track_title_content(
                    0,
                    "Violin",
                    true,
                    "Violin",
                    Color::from_rgb(0.92, 0.34, 0.34),
                    false,
                ),
                Some(super::instrument_slot_controls(
                    0,
                    Some(InstrumentChoice::None),
                    false,
                    true,
                )),
                0.0,
                0.0,
                container(iced::widget::text(""))
                    .width(Length::Fixed(72.0))
                    .height(Length::Fixed(220.0))
                    .into(),
                super::StripActions {
                    solo: Some((false, super::noop_message())),
                    mute: Some((false, super::noop_message())),
                    on_gain: Some(Box::new(|_| super::noop_message())),
                    on_pan: Some(Box::new(|_| super::noop_message())),
                },
                480.0,
                GainControlMode::Knob,
            ))
            .padding(ui_style::PADDING_SM),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_strip_controls_title_gap_fixed",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_full_track_strip_rename_matches_snapshot() -> Result<(), iced_test::Error> {
        let mixer = MixerState::new();
        let meters = MixerMeterSnapshotWindow::default();
        let colors = meter_colors(&Theme::Dark);

        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [STRIP_WIDTH + ui_style::grid_f32(8), 520.0],
            super::instrument_track_area(
                &mixer,
                &meters,
                colors,
                STRIP_MIN_HEIGHT,
                GainControlMode::Knob,
                0..1,
                1,
                &[crate::track_colors::default_track_color(0)],
                Some(crate::app::RenameTarget::Track(0)),
                Some(crate::app::WorkspacePaneKind::Mixer),
                "Violin",
                Color::from_rgb(0.92, 0.34, 0.34),
                false,
                None,
                true,
            ),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_full_track_strip_rename_no_section_gap",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_master_strip_main_title_matches_snapshot() -> Result<(), iced_test::Error> {
        let mixer = MixerState::new();
        let colors = meter_colors(&Theme::Dark);

        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [
                MAIN_STRIP_WIDTH + ui_style::grid_f32(8),
                STRIP_MIN_HEIGHT + ui_style::grid_f32(8),
            ],
            super::sticky_master_strip(
                &mixer,
                StripMeterSnapshot::default(),
                colors,
                STRIP_MIN_HEIGHT,
                GainControlMode::Knob,
                true,
            ),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_master_strip_main_title_no_section_gap",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_bus_area_empty_centers_add_bus_matches_snapshot() -> Result<(), iced_test::Error> {
        let mixer = MixerState::new();
        let meters = MixerMeterSnapshotWindow::default();
        let colors = meter_colors(&Theme::Dark);

        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [STRIP_WIDTH + ui_style::grid_f32(8), 320.0],
            super::bus_track_area(
                &mixer,
                &meters,
                colors,
                STRIP_MIN_HEIGHT,
                GainControlMode::Knob,
                0..0,
                None,
                None,
                "",
                true,
            ),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_bus_area_empty_centered_add_icon_no_section_gap",
        )?;

        Ok(())
    }

    #[test]
    fn mixer_bus_area_nonempty_appends_add_bus_lane_matches_snapshot()
    -> Result<(), iced_test::Error> {
        let (mut app, _task) = crate::app::new(None, None, false);
        app.playback = Some(
            AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default())
                .expect("test audio engine should start"),
        );
        let _ = app.handle_mixer_message(MixerMessage::AddBus);

        let mixer = app
            .playback
            .as_ref()
            .expect("playback should exist")
            .mixer_state()
            .clone();
        let meters = MixerMeterSnapshotWindow::default();
        let colors = meter_colors(&Theme::Dark);

        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [STRIP_WIDTH * 2.0 + ui_style::grid_f32(8), 320.0],
            super::bus_track_area(
                &mixer,
                &meters,
                colors,
                STRIP_MIN_HEIGHT,
                GainControlMode::Knob,
                0..mixer.bus_count(),
                None,
                None,
                "",
                true,
            ),
        );
        assert_snapshot_matches(
            &mut ui,
            "tests/snapshots/mixer_bus_area_nonempty_add_icon_lane_flat_close",
        )?;

        Ok(())
    }

    #[test]
    fn add_bus_button_hover_is_consistent_across_whole_button() -> Result<(), iced_test::Error> {
        let view = || -> Element<'static, Message> {
            container(super::add_bus_button(true))
                .width(Length::Fixed(ui_style::grid_f32(12)))
                .height(Length::Fixed(ui_style::grid_f32(12)))
                .center_x(Length::Fixed(ui_style::grid_f32(12)))
                .center_y(Length::Fixed(ui_style::grid_f32(12)))
                .into()
        };

        let mut button_hover = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(12), ui_style::grid_f32(12)],
            view(),
        );
        button_hover.point_at(iced::Point::new(
            ui_style::grid_f32(4),
            ui_style::grid_f32(6),
        ));

        let mut icon_hover = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(12), ui_style::grid_f32(12)],
            view(),
        );
        icon_hover.point_at(iced::Point::new(
            ui_style::grid_f32(6),
            ui_style::grid_f32(6),
        ));

        assert_snapshots_equal(
            &mut button_hover,
            &mut icon_hover,
            "add_bus_button_hover_consistency",
        )?;

        Ok(())
    }

    #[test]
    fn instrument_slot_button_hover_is_consistent_across_whole_button()
    -> Result<(), iced_test::Error> {
        let view = || -> Element<'static, Message> {
            super::slot_selector_controls("Violin".to_string(), Some(Message::Noop), None)
        };

        let mut label_hover = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(32), ui_style::grid_f32(10)],
            view(),
        );
        label_hover.point_at(iced::Point::new(
            ui_style::grid_f32(18),
            ui_style::grid_f32(5),
        ));

        let mut icon_hover = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(32), ui_style::grid_f32(10)],
            view(),
        );
        icon_hover.point_at(iced::Point::new(
            ui_style::grid_f32(5),
            ui_style::grid_f32(5),
        ));

        assert_snapshots_equal(
            &mut label_hover,
            &mut icon_hover,
            "instrument_slot_button_hover_consistency",
        )?;

        Ok(())
    }

    #[test]
    fn instrument_browser_tabs_match_snapshot() -> Result<(), iced_test::Error> {
        let mut ui = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(48), ui_style::grid_f32(8)],
            row![
                super::instrument_browser_tab_button(
                    InstrumentBrowserBackend::BuiltIn,
                    InstrumentBrowserBackend::BuiltIn,
                ),
                super::instrument_browser_tab_button(
                    InstrumentBrowserBackend::Clap,
                    InstrumentBrowserBackend::BuiltIn,
                ),
                super::instrument_browser_tab_button(
                    InstrumentBrowserBackend::Vst3,
                    InstrumentBrowserBackend::BuiltIn,
                ),
            ]
            .spacing(ui_style::SPACE_XS),
        );

        assert_snapshot_matches(&mut ui, "tests/snapshots/instrument_browser_tabs")?;

        Ok(())
    }

    #[test]
    fn remove_bus_button_idle_and_hover_render_differ() -> Result<(), iced_test::Error> {
        let view = || -> Element<'static, Message> {
            container(
                ui_style::flat_icon_button(
                    icons::x(),
                    ui_style::grid_f32(4),
                    ui_style::grid_f32(3),
                    ui_style::button_flat_compact_control,
                    ui_style::svg_dimmed_control,
                )
                .on_press(Message::Noop),
            )
            .width(Length::Fixed(ui_style::grid_f32(8)))
            .height(Length::Fixed(ui_style::grid_f32(8)))
            .center_x(Length::Fixed(ui_style::grid_f32(8)))
            .center_y(Length::Fixed(ui_style::grid_f32(8)))
            .into()
        };

        let mut idle = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(8), ui_style::grid_f32(8)],
            view(),
        );

        let mut hover = Simulator::with_size(
            iced::Settings::default(),
            [ui_style::grid_f32(8), ui_style::grid_f32(8)],
            view(),
        );
        hover.point_at(iced::Point::new(
            ui_style::grid_f32(4),
            ui_style::grid_f32(4),
        ));

        assert_snapshots_differ(
            &mut idle,
            &mut hover,
            "remove_bus_button_idle_hover_difference",
        )?;

        Ok(())
    }

    #[test]
    fn empty_slot_maps_to_none_choice() {
        let mixer = MixerState::new();
        assert_eq!(
            selected_instrument_choice(Some(&SlotState::default()), &mixer),
            Some(InstrumentChoice::None)
        );
    }

    #[test]
    fn soundfont_slot_maps_to_soundfont_choice() {
        let mixer = MixerState::new();
        assert_eq!(
            selected_instrument_choice(
                Some(&SlotState::built_in(
                    BUILTIN_SOUNDFONT_ID,
                    lilypalooza_builtins::soundfont_synth::state("default", 0, 2),
                )),
                &mixer
            ),
            Some(InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SoundFont".to_string(),
                backend: InstrumentBrowserBackend::BuiltIn,
            })
        );
    }

    #[test]
    fn built_in_browser_entries_filter_by_instrument_name_without_section_headers() {
        let choices = vec![
            InstrumentChoice::None,
            InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SoundFont".to_string(),
                backend: InstrumentBrowserBackend::BuiltIn,
            },
        ];

        let browser =
            instrument_browser_entries(&choices, InstrumentBrowserBackend::BuiltIn, "sound");

        assert!(!browser.show_none);
        assert_eq!(
            browser.entries,
            vec![InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "SoundFont".to_string(),
                backend: InstrumentBrowserBackend::BuiltIn,
            }]
        );
    }

    #[test]
    fn instrument_trigger_label_uses_none_and_truncates_long_names() {
        assert_eq!(instrument_trigger_label(None), "Empty");

        assert_eq!(
            instrument_trigger_label(Some(&InstrumentChoice::Processor {
                processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                name: "Extremely Long SoundFont Synth Name".to_string(),
                backend: InstrumentBrowserBackend::BuiltIn,
            })),
            "Extre… Name"
        );
    }

    #[test]
    fn instrument_trigger_label_uses_none_for_empty_choice() {
        assert_eq!(
            instrument_trigger_label(Some(&InstrumentChoice::None)),
            "Empty"
        );
    }

    #[test]
    fn instrument_slot_primary_action_opens_editor_when_available() {
        assert!(matches!(
            instrument_slot_primary_action(
                2,
                Some(&InstrumentChoice::Processor {
                    processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                    name: "SoundFont".to_string(),
                    backend: InstrumentBrowserBackend::BuiltIn,
                }),
                true,
                true,
            ),
            Some(crate::app::messages::Message::Mixer(
                crate::app::messages::MixerMessage::OpenEditor(_)
            ))
        ));
    }

    #[test]
    fn instrument_slot_primary_action_opens_picker_only_when_empty() {
        assert!(matches!(
            instrument_slot_primary_action(2, Some(&InstrumentChoice::None), false, true),
            Some(crate::app::messages::Message::Mixer(
                crate::app::messages::MixerMessage::ToggleTrackInstrumentBrowser(2)
            ))
        ));
        assert!(
            instrument_slot_primary_action(
                2,
                Some(&InstrumentChoice::Processor {
                    processor_id: BUILTIN_SOUNDFONT_ID.to_string(),
                    name: "SoundFont".to_string(),
                    backend: InstrumentBrowserBackend::BuiltIn,
                }),
                false,
                true,
            )
            .is_none()
        );
    }

    #[test]
    fn short_mixer_height_uses_gain_knob_mode() {
        assert_eq!(gain_control_mode(MIXER_MIN_HEIGHT), GainControlMode::Knob);
        assert_eq!(
            gain_control_mode(MIXER_MIN_HEIGHT - 10.0),
            GainControlMode::Knob
        );
        assert_eq!(
            gain_control_mode(MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET - 1.0),
            GainControlMode::Knob
        );
        assert_eq!(
            gain_control_mode(MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET + 1.0),
            GainControlMode::Fader
        );
    }

    #[test]
    fn empty_toggle_slots_use_track_toggle_size() {
        assert_eq!(STRIP_TOGGLE_SIZE, INSTRUMENT_PICKER_HEIGHT - 4.0);
    }

    #[test]
    fn meter_and_gain_controls_share_same_height() {
        let strip_height = 280.0;
        assert_eq!(
            meter_control_height(strip_height, GainControlMode::Fader),
            gain_control_height(strip_height, GainControlMode::Fader)
        );
        assert_eq!(
            meter_control_height(strip_height, GainControlMode::Knob),
            gain_control_height(strip_height, GainControlMode::Knob)
        );
        assert_eq!(
            control_stack_height(meter_control_height(strip_height, GainControlMode::Fader)),
            control_stack_height(gain_control_height(strip_height, GainControlMode::Fader))
        );
    }

    #[test]
    fn meter_peak_label_uses_hold_and_floor() {
        assert_eq!(meter_peak_label(StripMeterSnapshot::default()), "-inf");
        let snapshot = StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: 0.2,
                hold: 1.0,
                hold_db: 3.2,
            },
            right: ChannelMeterSnapshot {
                level: 0.2,
                hold: 0.5,
                hold_db: -6.0,
            },
            clip_latched: false,
        };
        assert_eq!(meter_peak_label(snapshot), "3.2");
    }

    #[test]
    fn gain_label_uses_negative_infinity_at_floor() {
        assert_eq!(gain_label(-60.0), "-inf");
        assert_eq!(gain_label(-24.0), "-24.0");
    }

    #[test]
    fn main_section_width_includes_group_borders() {
        assert_eq!(
            MAIN_SECTION_WIDTH,
            MAIN_STRIP_WIDTH + GROUP_SIDE_BORDER_WIDTH * 2.0
        );
    }

    #[test]
    fn value_labels_use_shared_slot_height() {
        assert!(is_grid_multiple(VALUE_LABEL_HEIGHT));
    }

    #[test]
    fn control_stack_height_adds_shared_label_slot() {
        let control: f32 = 100.0;
        assert_eq!(
            control_stack_height(control),
            control + VALUE_LABEL_HEIGHT + ui_style::SPACE_XS as f32
        );
    }

    #[test]
    fn compact_gain_mode_hides_meter_scale() {
        assert!(!meter_scale_visible(GainControlMode::Knob));
        assert!(meter_scale_visible(GainControlMode::Fader));
    }

    #[test]
    fn visible_strip_window_limits_rendered_strip_count() {
        let visible = visible_strip_window(128, 0.0, STRIP_WIDTH * 4.0);
        assert!(visible.end - visible.start <= 4 + STRIP_VIRTUALIZATION_OVERSCAN * 2);

        let scrolled = visible_strip_window(128, STRIP_WIDTH * 40.0, STRIP_WIDTH * 4.0);
        assert!(scrolled.start >= 38);
        assert!(scrolled.end <= 46);
    }

    #[test]
    fn only_existing_roll_tracks_use_tint() {
        assert!(track_should_use_roll_tint(0, 4));
        assert!(track_should_use_roll_tint(3, 4));
        assert!(!track_should_use_roll_tint(4, 4));
        assert!(!track_should_use_roll_tint(127, 4));
    }

    #[test]
    fn track_strip_dependency_includes_tint_state() {
        let base = TrackStripDependency {
            index: 0,
            name: "Track".to_string(),
            selected: Some(InstrumentChoice::None),
            editor_enabled: false,
            color_bits: color_bits(iced::Color::from_rgb(0.1, 0.2, 0.3)),
            gain_bits: 0.0f32.to_bits(),
            pan_bits: 0.0f32.to_bits(),
            meter: MeterDependency::from_snapshot(StripMeterSnapshot::default()),
            compact_gain: false,
            strip_height_bits: 140.0f32.to_bits(),
            soloed: false,
            muted: false,
            tint_enabled: false,
            highlighted: false,
            renaming: false,
            rename_value: String::new(),
            color_picker_open: false,
        };
        let tinted = TrackStripDependency {
            tint_enabled: true,
            ..base.clone()
        };

        assert_ne!(base, tinted);
    }
}
