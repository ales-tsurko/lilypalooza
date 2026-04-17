use std::fmt;
use std::sync::Arc;

use iced::widget::{
    button, column, container, lazy, pick_list, responsive, row, scrollable, text, vertical_slider,
};
use iced::{Element, Fill, FillPortion, Length, alignment};
use lilypalooza_audio::{InstrumentSlotState, MixerState, SoundfontProcessorState};

use super::knob::{gain_knob, pan_knob};
use super::messages::MixerMessage;
use super::{Lilypalooza, Message};
use crate::{fonts, ui_style};

pub(super) const MIXER_MIN_HEIGHT: f32 = 340.0;
pub(super) const MIXER_MIN_WIDTH: f32 = 520.0;

const MAIN_STRIP_WIDTH: f32 = 148.0;
const STRIP_WIDTH: f32 = 156.0;
const STRIP_SPACING: f32 = 10.0;
const INSTRUMENT_PICKER_HEIGHT: f32 = 28.0;
const SECTION_HEADER_HEIGHT: f32 = 28.0;
const STRIP_MIN_HEIGHT: f32 = 140.0;
const STRIP_TOGGLE_SIZE: f32 = 28.0;
const COMPACT_GAIN_SWITCH_OFFSET: f32 = 20.0;

struct StripActions<'a> {
    solo: Option<(bool, Message)>,
    mute: Option<(bool, Message)>,
    on_gain: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    on_pan: Option<Box<dyn Fn(f32) -> Message + 'a>>,
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
    compact_gain: bool,
    strip_height_bits: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TrackStripDependency {
    index: usize,
    name: String,
    selected: Option<InstrumentChoice>,
    gain_bits: u32,
    pan_bits: u32,
    compact_gain: bool,
    strip_height_bits: u32,
    soloed: bool,
    muted: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct BusStripDependency {
    id: u16,
    name: String,
    gain_bits: u32,
    pan_bits: u32,
    compact_gain: bool,
    strip_height_bits: u32,
    soloed: bool,
    muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GainControlMode {
    Fader,
    Knob,
}

pub(super) fn content(app: &Lilypalooza) -> Element<'_, Message> {
    let Some(playback) = app.playback.as_ref() else {
        return container(text("Mixer unavailable"))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
    };
    let mixer = playback.mixer_state();

    responsive(move |size| {
        let gain_mode = gain_control_mode(size.height);
        let strip_height =
            (size.height - (ui_style::PADDING_SM as f32 * 2.0) - SECTION_HEADER_HEIGHT)
                .max(STRIP_MIN_HEIGHT);

        row![
            container(master_track_area(mixer, strip_height, gain_mode))
                .width(Length::Fixed(MAIN_STRIP_WIDTH))
                .height(Fill),
            container(instrument_track_area(mixer, strip_height, gain_mode))
                .width(FillPortion(5))
                .height(Fill),
            container(bus_track_area(mixer, strip_height, gain_mode))
                .width(FillPortion(2))
                .height(Fill)
        ]
        .spacing(ui_style::SPACE_SM)
        .padding(ui_style::PADDING_SM)
        .width(Fill)
        .height(Fill)
        .into()
    })
    .into()
}

fn master_track_area(
    mixer: &MixerState,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'_, Message> {
    let master_row = row![sticky_master_strip(mixer, strip_height, gain_mode)]
        .align_y(alignment::Vertical::Top)
        .height(Length::Fixed(strip_height));

    column![
        section_title("Main"),
        scrollable(master_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
            ))
            .style(ui_style::workspace_scrollable)
            .width(Fill)
            .height(Fill)
    ]
    .spacing(ui_style::SPACE_XS)
    .height(Fill)
    .into()
}

fn sticky_master_strip(
    mixer: &MixerState,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'_, Message> {
    let master = mixer.master();
    lazy(
        MainStripDependency {
            gain_bits: master.state.gain_db.to_bits(),
            pan_bits: master.state.pan.to_bits(),
            compact_gain: matches!(gain_mode, GainControlMode::Knob),
            strip_height_bits: strip_height.to_bits(),
        },
        move |dependency| {
            strip_panel(
                strip_shell(
                    "Main",
                    None,
                    f32::from_bits(dependency.gain_bits),
                    f32::from_bits(dependency.pan_bits),
                    StripActions {
                        solo: None,
                        mute: None,
                        on_gain: Some(Box::new(|value| {
                            Message::Mixer(MixerMessage::SetMasterGain(value))
                        })),
                        on_pan: Some(Box::new(|value| {
                            Message::Mixer(MixerMessage::SetMasterPan(value))
                        })),
                    },
                    f32::from_bits(dependency.strip_height_bits),
                    if dependency.compact_gain {
                        GainControlMode::Knob
                    } else {
                        GainControlMode::Fader
                    },
                ),
                MAIN_STRIP_WIDTH,
                f32::from_bits(dependency.strip_height_bits),
            )
        },
    )
    .into()
}

fn instrument_track_area(
    mixer: &MixerState,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'_, Message> {
    let options: Arc<[InstrumentChoice]> = instrument_choices(mixer).into();
    let track_row = mixer.tracks().iter().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height)),
        move |row, track| {
            let track_index = track.id.index();
            let selected = selected_instrument_choice(&track.instrument, mixer);
            let options = options.clone();
            row.push(lazy(
                TrackStripDependency {
                    index: track_index,
                    name: track.name.clone(),
                    selected,
                    gain_bits: track.state.gain_db.to_bits(),
                    pan_bits: track.state.pan.to_bits(),
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: track.state.soloed,
                    muted: track.state.muted,
                },
                move |dependency| {
                    let name = dependency.name.clone();
                    let selected = dependency.selected.clone();
                    let strip_height = f32::from_bits(dependency.strip_height_bits);
                    strip_panel(
                        strip_shell(
                            name,
                            Some(
                                pick_list(options.clone(), selected, move |choice| {
                                    Message::Mixer(MixerMessage::SelectTrackInstrument(
                                        track_index,
                                        choice,
                                    ))
                                })
                                .placeholder("Instrument")
                                .text_size(ui_style::FONT_SIZE_UI_XS.saturating_sub(3))
                                .width(Fill)
                                .into(),
                            ),
                            f32::from_bits(dependency.gain_bits),
                            f32::from_bits(dependency.pan_bits),
                            StripActions {
                                solo: Some((
                                    dependency.soloed,
                                    Message::Mixer(MixerMessage::ToggleTrackSolo(track_index)),
                                )),
                                mute: Some((
                                    dependency.muted,
                                    Message::Mixer(MixerMessage::ToggleTrackMute(track_index)),
                                )),
                                on_gain: Some(Box::new(move |value| {
                                    Message::Mixer(MixerMessage::SetTrackGain(track_index, value))
                                })),
                                on_pan: Some(Box::new(move |value| {
                                    Message::Mixer(MixerMessage::SetTrackPan(track_index, value))
                                })),
                            },
                            strip_height,
                            if dependency.compact_gain {
                                GainControlMode::Knob
                            } else {
                                GainControlMode::Fader
                            },
                        ),
                        STRIP_WIDTH,
                        strip_height,
                    )
                },
            ))
        },
    );

    column![
        section_title("Instrument Tracks"),
        scrollable(track_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
            ))
            .style(ui_style::workspace_scrollable)
            .width(Fill)
            .height(Fill)
    ]
    .spacing(ui_style::SPACE_XS)
    .height(Fill)
    .into()
}

fn bus_track_area(
    mixer: &MixerState,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'_, Message> {
    let bus_row = mixer.buses().iter().fold(
        row![]
            .spacing(STRIP_SPACING)
            .align_y(alignment::Vertical::Top)
            .height(Length::Fixed(strip_height)),
        |row, bus| {
            row.push(lazy(
                BusStripDependency {
                    id: bus.id.0,
                    name: bus.name.clone(),
                    gain_bits: bus.state.gain_db.to_bits(),
                    pan_bits: bus.state.pan.to_bits(),
                    compact_gain: matches!(gain_mode, GainControlMode::Knob),
                    strip_height_bits: strip_height.to_bits(),
                    soloed: bus.state.soloed,
                    muted: bus.state.muted,
                },
                move |dependency| {
                    let name = dependency.name.clone();
                    let bus_id = dependency.id;
                    let gain_db = f32::from_bits(dependency.gain_bits);
                    let pan = f32::from_bits(dependency.pan_bits);
                    let strip_height = f32::from_bits(dependency.strip_height_bits);
                    let soloed = dependency.soloed;
                    let muted = dependency.muted;
                    strip_panel(
                        strip_shell(
                            name,
                            None,
                            gain_db,
                            pan,
                            StripActions {
                                solo: Some((
                                    soloed,
                                    Message::Mixer(MixerMessage::ToggleBusSolo(bus_id)),
                                )),
                                mute: Some((
                                    muted,
                                    Message::Mixer(MixerMessage::ToggleBusMute(bus_id)),
                                )),
                                on_gain: Some(Box::new(move |value| {
                                    Message::Mixer(MixerMessage::SetBusGain(bus_id, value))
                                })),
                                on_pan: Some(Box::new(move |value| {
                                    Message::Mixer(MixerMessage::SetBusPan(bus_id, value))
                                })),
                            },
                            strip_height,
                            if dependency.compact_gain {
                                GainControlMode::Knob
                            } else {
                                GainControlMode::Fader
                            },
                        ),
                        STRIP_WIDTH,
                        strip_height,
                    )
                },
            ))
        },
    );

    column![
        row![
            section_title("Buses"),
            container(text("")).width(Fill),
            button(text("+ Bus").size(ui_style::FONT_SIZE_UI_XS))
                .style(ui_style::button_neutral)
                .padding([
                    ui_style::PADDING_BUTTON_COMPACT_V,
                    ui_style::PADDING_BUTTON_COMPACT_H
                ])
                .on_press(Message::Mixer(MixerMessage::AddBus)),
        ]
        .align_y(alignment::Vertical::Center),
        scrollable(bus_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
            ))
            .style(ui_style::workspace_scrollable)
            .width(Fill)
            .height(Fill)
    ]
    .spacing(ui_style::SPACE_XS)
    .height(Fill)
    .into()
}

fn section_title<'a>(label: impl Into<String>) -> Element<'a, Message> {
    text(label.into())
        .size(ui_style::FONT_SIZE_UI_SM)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..fonts::UI
        })
        .into()
}

fn strip_shell<'a>(
    title: impl Into<String>,
    instrument_picker: Option<Element<'a, Message>>,
    gain_db: f32,
    pan: f32,
    actions: StripActions<'a>,
    strip_height: f32,
    gain_mode: GainControlMode,
) -> Element<'a, Message> {
    let mut content = column![section_title(title.into())]
        .spacing(ui_style::SPACE_XS)
        .align_x(alignment::Horizontal::Center)
        .width(Fill)
        .height(Fill);

    content = content.push(
        container(instrument_picker.unwrap_or_else(|| container(text("")).into()))
            .width(Fill)
            .height(Length::Fixed(INSTRUMENT_PICKER_HEIGHT)),
    );

    content = content.push(
        row![
            actions
                .solo
                .map_or_else(strip_toggle_placeholder, |(active, message)| {
                    strip_toggle_button("S", active, message)
                },),
            actions
                .mute
                .map_or_else(strip_toggle_placeholder, |(active, message)| {
                    strip_toggle_button("M", active, message)
                },),
        ]
        .spacing(ui_style::SPACE_XS)
        .width(Fill)
        .align_y(alignment::Vertical::Center),
    );

    if let Some(on_pan) = actions.on_pan {
        content = content
            .push(text(format!("{pan:+.2}")).size(ui_style::FONT_SIZE_UI_XS))
            .push(pan_knob(pan, on_pan));
    }

    if let Some(on_gain) = actions.on_gain {
        content = content.push(text(format!("{gain_db:.1} dB")).size(ui_style::FONT_SIZE_UI_XS));

        content = match gain_mode {
            GainControlMode::Fader => content.push(
                container(
                    vertical_slider(-60.0..=12.0, gain_db.clamp(-60.0, 12.0), on_gain)
                        .step(0.1)
                        .height(Fill),
                )
                .width(Fill)
                .height(Length::Fill)
                .center_x(Fill),
            ),
            GainControlMode::Knob => content.push(gain_knob(gain_db, on_gain)),
        };
    }

    container(content)
        .padding(ui_style::PADDING_SM)
        .width(Fill)
        .height(Length::Fixed(strip_height))
        .style(ui_style::pane_main_surface)
        .into()
}

fn gain_control_mode(pane_height: f32) -> GainControlMode {
    if pane_height <= MIXER_MIN_HEIGHT + COMPACT_GAIN_SWITCH_OFFSET {
        GainControlMode::Knob
    } else {
        GainControlMode::Fader
    }
}

fn strip_panel<'a>(content: Element<'a, Message>, width: f32, height: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(ui_style::pane_main_surface)
        .into()
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
    use lilypalooza_audio::{InstrumentSlotState, MixerState};

    use super::{
        COMPACT_GAIN_SWITCH_OFFSET, GainControlMode, InstrumentChoice, MIXER_MIN_HEIGHT,
        STRIP_TOGGLE_SIZE, gain_control_mode, selected_instrument_choice,
    };

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
        assert_eq!(STRIP_TOGGLE_SIZE, 28.0);
    }
}
