use std::fmt;
use std::sync::Arc;

use iced::widget::Id;
use iced::widget::{
    button, column, container, lazy, mouse_area, pick_list, responsive, row, scrollable, text,
    text_input,
};
use iced::{Color, Element, Fill, FillPortion, Length, alignment};
use iced_aw::helpers::color_picker_with_change;
use lilypalooza_audio::mixer::{
    ChannelMeterSnapshot, MixerMeterSnapshot, MixerMeterSnapshotWindow, STRIP_METER_MIN_DB,
    StripMeterSnapshot,
};
use lilypalooza_audio::{InstrumentSlotState, MixerState, SoundfontProcessorState};

use super::controls::{GAIN_MIN_DB, gain_control_width, gain_fader, gain_knob, pan_knob};
use super::messages::MixerMessage;
use super::meters::{
    MeterColors, meter_colors, stereo_meter, stereo_meter_bar_width, stereo_meter_width,
    stereo_meter_with_scale,
};
use super::{Lilypalooza, Message};
use crate::{fonts, ui_style};

pub(super) const MIXER_MIN_HEIGHT: f32 = 340.0;
pub(super) const MIXER_MIN_WIDTH: f32 = 520.0;

const GROUP_SIDE_BORDER_WIDTH: f32 = 1.0;
const MAIN_STRIP_WIDTH: f32 = 141.0;
const MAIN_SECTION_WIDTH: f32 = MAIN_STRIP_WIDTH + GROUP_SIDE_BORDER_WIDTH * 2.0;
const STRIP_WIDTH: f32 = 146.0;
const STRIP_SPACING: f32 = 0.0;
const INSTRUMENT_PICKER_HEIGHT: f32 = 28.0;
const SECTION_HEADER_HEIGHT: f32 = 24.0;
const STRIP_MIN_HEIGHT: f32 = 140.0;
const STRIP_TOGGLE_SIZE: f32 = INSTRUMENT_PICKER_HEIGHT - 4.0;
const COMPACT_GAIN_SWITCH_OFFSET: f32 = 20.0;
const VALUE_LABEL_HEIGHT: f32 = 14.0;
const HEADER_SIDE_INSET: f32 = 12.0;
const METER_STACK_SPACING: f32 = 6.0;
const STRIP_STACK_SPACING: f32 = 2.0;
const LABEL_CONTROL_SPACING: f32 = ui_style::SPACE_XS as f32;
const TITLE_TOP_SPACING: f32 = 6.0;
const STRIP_VIRTUALIZATION_OVERSCAN: usize = 2;

struct StripActions<'a> {
    solo: Option<(bool, Message)>,
    mute: Option<(bool, Message)>,
    on_gain: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    on_pan: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

fn noop_message() -> Message {
    Message::Noop
}

const GM_PROGRAM_NAMES: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavinet",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Acoustic Guitar (nylon)",
    "Acoustic Guitar (steel)",
    "Electric Guitar (jazz)",
    "Electric Guitar (clean)",
    "Electric Guitar (muted)",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Electric Bass (finger)",
    "Electric Bass (pick)",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "SynthStrings 1",
    "SynthStrings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "SynthBrass 1",
    "SynthBrass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Lead 1 (square)",
    "Lead 2 (sawtooth)",
    "Lead 3 (calliope)",
    "Lead 4 (chiff)",
    "Lead 5 (charang)",
    "Lead 6 (voice)",
    "Lead 7 (fifths)",
    "Lead 8 (bass + lead)",
    "Pad 1 (new age)",
    "Pad 2 (warm)",
    "Pad 3 (polysynth)",
    "Pad 4 (choir)",
    "Pad 5 (bowed)",
    "Pad 6 (metallic)",
    "Pad 7 (halo)",
    "Pad 8 (sweep)",
    "FX 1 (rain)",
    "FX 2 (soundtrack)",
    "FX 3 (crystal)",
    "FX 4 (atmosphere)",
    "FX 5 (brightness)",
    "FX 6 (goblins)",
    "FX 7 (echoes)",
    "FX 8 (sci-fi)",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bag pipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum InstrumentChoice {
    None,
    SoundfontProgram {
        soundfont_id: String,
        soundfont_name: String,
        bank: u16,
        program: u8,
    },
}

impl fmt::Display for InstrumentChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::SoundfontProgram { bank, program, .. } => {
                if *bank == 0 {
                    f.write_str(GM_PROGRAM_NAMES[*program as usize])
                } else {
                    write!(f, "Bank {} / {}", bank, GM_PROGRAM_NAMES[*program as usize])
                }
            }
        }
    }
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
    color_bits: [u32; 4],
    gain_bits: u32,
    pan_bits: u32,
    meter: MeterDependency,
    compact_gain: bool,
    strip_height_bits: u32,
    soloed: bool,
    muted: bool,
    tint_enabled: bool,
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

    content_without_audio(app, colors)
}

fn content_without_audio(_app: &Lilypalooza, _colors: MeterColors) -> Element<'_, Message> {
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
                    section_title("Main"),
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
    controls_enabled: bool,
) -> Element<'static, Message> {
    let options: Arc<[InstrumentChoice]> = if controls_enabled {
        instrument_choices(mixer).into()
    } else {
        vec![InstrumentChoice::None].into()
    };
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = strip_span_width(mixer.tracks().len().saturating_sub(visible.end));
    let track_row = mixer.tracks()[visible.clone()].iter().enumerate().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height))
            .push(horizontal_spacer(left_spacer)),
        move |row, (local_index, track)| {
            let track_index = track.id.index();
            let selected = selected_instrument_choice(&track.instrument, mixer);
            let track_color = track_colors
                .get(track_index)
                .copied()
                .unwrap_or_else(|| crate::track_colors::default_track_color(track_index));
            let options = options.clone();
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
                    selected,
                    color_bits: color_bits(track_color),
                    gain_bits: track.state.gain_db.to_bits(),
                    pan_bits: track.state.pan.to_bits(),
                    meter: meter_dependency.meter,
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: track.state.soloed,
                    muted: track.state.muted,
                    tint_enabled: track_should_use_roll_tint(track_index, existing_track_count),
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
                    let selected = dependency.selected.clone();
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
                            pick_list(options.clone(), selected, move |choice| {
                                if controls_enabled {
                                    Message::Mixer(MixerMessage::SelectTrackInstrument(
                                        track_index,
                                        choice,
                                    ))
                                } else {
                                    noop_message()
                                }
                            })
                            .placeholder("Instrument")
                            .text_size(ui_style::FONT_SIZE_UI_XS.saturating_sub(3))
                            .width(Fill)
                            .into()
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
                        tinted_track_strip_panel(shell, STRIP_WIDTH, strip_height, track_color)
                    } else {
                        strip_panel(shell, STRIP_WIDTH, strip_height)
                    }
                },
            ))
        },
    );
    let track_row = track_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(row![section_title("Instrument Tracks")]))
            .style(ui_style::workspace_toolbar_surface),
        row![
            container(text(""))
                .width(Length::Fixed(GROUP_SIDE_BORDER_WIDTH))
                .height(Fill)
                .style(ui_style::chrome_separator),
            scrollable(track_row)
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
    let left_spacer = strip_span_width(visible.start);
    let right_spacer = strip_span_width(mixer.buses().len().saturating_sub(visible.end));
    let bus_row = mixer.buses()[visible.clone()].iter().enumerate().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height))
            .push(horizontal_spacer(left_spacer)),
        |row, (local_index, bus)| {
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
                    id: bus.id.0,
                    name: bus.name.clone(),
                    gain_bits: bus.state.gain_db.to_bits(),
                    pan_bits: bus.state.pan.to_bits(),
                    meter: meter_dependency.meter,
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: bus.state.soloed,
                    muted: bus.state.muted,
                    renaming: renaming_target == Some(super::RenameTarget::Bus(bus.id.0))
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
                    strip_panel(
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
                    )
                },
            ))
        },
    );
    let bus_row = bus_row.push(horizontal_spacer(right_spacer));

    column![
        container(section_header_bar(
            row![
                section_title("Buses"),
                container(text("")).width(Fill),
                button(text("+ Bus").size(ui_style::FONT_SIZE_UI_XS))
                    .style(ui_style::button_neutral)
                    .padding([
                        ui_style::PADDING_BUTTON_COMPACT_V,
                        ui_style::PADDING_BUTTON_COMPACT_H
                    ])
                    .on_press_maybe(Some(if controls_enabled {
                        Message::Mixer(MixerMessage::AddBus)
                    } else {
                        noop_message()
                    })),
                container(text("")).width(Length::Fixed(HEADER_SIDE_INSET)),
            ]
            .align_y(alignment::Vertical::Center),
        ),)
        .style(ui_style::workspace_toolbar_surface),
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
        .width(Fill)
        .height(Fill);

    content = content.push(
        container(instrument_picker.unwrap_or_else(|| container(text("")).into()))
            .width(Fill)
            .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT)),
    );

    if let Some(on_pan) = actions.on_pan {
        content = content.push(
            column![
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

    content = content.push(container(title).padding([TITLE_TOP_SPACING as u16, 0]));

    container(content)
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
        let swatch = button(container(text("")).width(16).height(16))
            .padding(0)
            .width(18)
            .height(18)
            .style(move |theme, status| ui_style::track_color_swatch_button(theme, status, color))
            .on_press(Message::Mixer(MixerMessage::OpenTrackColorPicker));
        let input = text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
            .id(Id::new(super::TRACK_RENAME_INPUT_ID))
            .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
            .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
            .style(ui_style::track_name_input)
            .size(ui_style::FONT_SIZE_UI_SM)
            .padding([2, 4])
            .width(Fill);
        let focused = color_picker_open;
        let editor_row = container(
            row![
                swatch,
                container(text(""))
                    .width(1)
                    .height(18)
                    .style(move |theme| { ui_style::track_name_editor_divider(theme, focused) }),
                input
            ]
            .spacing(0)
            .align_y(alignment::Vertical::Center)
            .width(Fill),
        )
        .padding(0)
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
        return text_input::<Message, iced::Theme, iced::Renderer>("", rename_value)
            .id(Id::new(super::TRACK_RENAME_INPUT_ID))
            .on_input(|value| Message::Mixer(MixerMessage::TrackRenameInputChanged(value)))
            .on_submit(Message::Mixer(MixerMessage::CommitTrackRename))
            .size(ui_style::FONT_SIZE_UI_SM)
            .padding([2, 4])
            .width(Fill)
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

fn strip_panel<'a>(content: Element<'a, Message>, width: f32, height: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(ui_style::pane_main_surface)
        .into()
}

fn tinted_track_strip_panel<'a>(
    content: Element<'a, Message>,
    width: f32,
    height: f32,
    track_color: Color,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(move |theme| ui_style::mixer_track_strip_surface(theme, track_color))
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

fn instrument_choices(mixer: &MixerState) -> Vec<InstrumentChoice> {
    let mut choices = Vec::with_capacity(1 + mixer.soundfonts().len() * GM_PROGRAM_NAMES.len());
    choices.push(InstrumentChoice::None);
    for soundfont in mixer.soundfonts() {
        for (program, _) in GM_PROGRAM_NAMES.iter().enumerate() {
            choices.push(InstrumentChoice::SoundfontProgram {
                soundfont_id: soundfont.id.clone(),
                soundfont_name: soundfont.name.clone(),
                bank: 0,
                program: program as u8,
            });
        }
    }
    choices
}

fn selected_instrument_choice(
    slot: &InstrumentSlotState,
    mixer: &MixerState,
) -> Option<InstrumentChoice> {
    if slot.is_empty() {
        return Some(InstrumentChoice::None);
    }

    let Ok(Some(SoundfontProcessorState {
        soundfont_id,
        bank,
        program,
    })) = slot.soundfont_state()
    else {
        return None;
    };

    let soundfont_name = mixer
        .soundfonts()
        .iter()
        .find(|resource| resource.id == soundfont_id)
        .map(|resource| resource.name.clone())
        .unwrap_or(soundfont_id.clone());

    Some(InstrumentChoice::SoundfontProgram {
        soundfont_id,
        soundfont_name,
        bank,
        program,
    })
}

#[cfg(test)]
mod tests {
    use crate::ui_style;
    use lilypalooza_audio::{InstrumentSlotState, MixerState};

    use super::{
        COMPACT_GAIN_SWITCH_OFFSET, GROUP_SIDE_BORDER_WIDTH, GainControlMode,
        INSTRUMENT_PICKER_HEIGHT, InstrumentChoice, MAIN_SECTION_WIDTH, MAIN_STRIP_WIDTH,
        MIXER_MIN_HEIGHT, MeterDependency, STRIP_TOGGLE_SIZE, STRIP_VIRTUALIZATION_OVERSCAN,
        STRIP_WIDTH, StripMeterSnapshot, TrackStripDependency, VALUE_LABEL_HEIGHT, color_bits,
        control_stack_height, gain_control_height, gain_control_mode, gain_label,
        meter_control_height, meter_peak_label, meter_scale_visible, selected_instrument_choice,
        track_should_use_roll_tint, visible_strip_window,
    };
    use lilypalooza_audio::mixer::ChannelMeterSnapshot;

    #[test]
    fn empty_slot_maps_to_none_choice() {
        let mixer = MixerState::new();
        assert_eq!(
            selected_instrument_choice(&InstrumentSlotState::empty(), &mixer),
            Some(InstrumentChoice::None)
        );
    }

    #[test]
    fn soundfont_slot_maps_to_soundfont_choice() {
        let mixer = MixerState::new();
        assert_eq!(
            selected_instrument_choice(&InstrumentSlotState::soundfont("default", 0, 2), &mixer),
            Some(InstrumentChoice::SoundfontProgram {
                soundfont_id: "default".to_string(),
                soundfont_name: "default".to_string(),
                bank: 0,
                program: 2,
            })
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
        assert_eq!(VALUE_LABEL_HEIGHT, 14.0);
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
            color_bits: color_bits(iced::Color::from_rgb(0.1, 0.2, 0.3)),
            gain_bits: 0.0f32.to_bits(),
            pan_bits: 0.0f32.to_bits(),
            meter: MeterDependency::from_snapshot(StripMeterSnapshot::default()),
            compact_gain: false,
            strip_height_bits: 140.0f32.to_bits(),
            soloed: false,
            muted: false,
            tint_enabled: false,
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
