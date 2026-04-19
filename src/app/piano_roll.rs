use std::collections::HashMap;

use iced::widget::{
    button, canvas, canvas::Cache, column, container, mouse_area, row, scrollable, slider, stack,
    svg, text, text_input, tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Pixels, Point, Rectangle, Renderer, Size, Theme,
    alignment, mouse,
};

use super::{Lilypalooza, Message, PianoRollMessage, dock_view::HeaderControlGroup};
use crate::midi::{MidiNote, MidiRollData, MidiRollFile, TimeSignatureChange};
use crate::settings::PianoRollViewSettings;
use crate::{fonts, icons, ui_style};

const TRACK_PANEL_DEFAULT_WIDTH: f32 = 96.0;
const TRACK_PANEL_MIN_WIDTH: f32 = 92.0;
const TRACK_PANEL_MAX_WIDTH: f32 = 160.0;
const TRACK_RESIZE_HANDLE_WIDTH: f32 = 6.0;
const TRACK_BUTTON_WIDTH: f32 = 18.0;
const TRACK_BUTTON_HEIGHT: f32 = 16.0;
const TRACK_BUTTONS_GAP: f32 = 4.0;
const TRACK_LABEL_BUTTON_GAP: f32 = 6.0;
const DRAG_START_THRESHOLD: f32 = 8.0;
const KEYBOARD_WIDTH: f32 = 30.0;
const TEMPO_LANE_HEIGHT: f32 = 28.0;
const REWIND_FLAG_HITBOX_WIDTH: f32 = 14.0;
const REWIND_FLAG_WIDTH: f32 = 11.0;
const REWIND_FLAG_BANNER_HEIGHT: f32 = 12.0;
const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 18.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
const TEMPO_LABEL_TOP_PADDING: f32 = 1.0;
const BAR_LABEL_BOTTOM_PADDING: f32 = 1.0;
const NOTE_ROW_HEIGHT: f32 = 14.0;
const CONTENT_RIGHT_PADDING: f32 = 24.0;
const TRACK_TOGGLE_ICON_SIZE: f32 = 13.0;
const ZOOM_MIN: f32 = 0.3;
const ZOOM_MAX: f32 = 6.0;
const ZOOM_STEP: f32 = 0.1;
const BASE_PIXELS_PER_QUARTER: f32 = 72.0;
const BEAT_SUBDIVISION_MIN: u8 = 1;
const BEAT_SUBDIVISION_MAX: u8 = 16;
const ROLL_SCROLL_ID: &str = "piano-roll-scroll";

#[derive(Debug, Clone)]
pub(super) struct PianoRollState {
    pub(super) visible: bool,
    pub(super) zoom_x: f32,
    pub(super) beat_subdivision: u8,
    default_view_settings: PianoRollViewSettings,
    beat_subdivision_input: String,
    pending_initial_center: bool,
    horizontal_scroll: f32,
    vertical_scroll: f32,
    playback_tick: u64,
    playback_total_ticks: u64,
    playback_is_playing: bool,
    rewind_flag_ticks: Vec<u64>,
    pub(super) files: Vec<MidiRollFile>,
    track_mix_by_file: Vec<Vec<TrackMixState>>,
    global_solo_active: bool,
    track_panel_visible: bool,
    track_panel_width: f32,
    pub(super) selected_file: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TrackMixState {
    pub(super) muted: bool,
    pub(super) soloed: bool,
}

impl PianoRollState {
    pub(super) fn new(default_view_settings: PianoRollViewSettings) -> Self {
        let zoom_x = default_view_settings.zoom_x.clamp(ZOOM_MIN, ZOOM_MAX);
        let beat_subdivision = default_view_settings
            .beat_subdivision
            .clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);

        Self {
            visible: true,
            zoom_x,
            beat_subdivision,
            default_view_settings,
            beat_subdivision_input: beat_subdivision.to_string(),
            pending_initial_center: false,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            playback_tick: 0,
            playback_total_ticks: 0,
            playback_is_playing: false,
            rewind_flag_ticks: Vec::new(),
            files: Vec::new(),
            track_mix_by_file: Vec::new(),
            global_solo_active: false,
            track_panel_visible: true,
            track_panel_width: TRACK_PANEL_DEFAULT_WIDTH,
            selected_file: 0,
        }
    }

    pub(super) fn clear_files(&mut self) {
        self.files.clear();
        self.selected_file = 0;
        self.pending_initial_center = false;
        self.horizontal_scroll = 0.0;
        self.vertical_scroll = 0.0;
        self.playback_tick = 0;
        self.playback_total_ticks = 0;
        self.playback_is_playing = false;
        self.rewind_flag_ticks.clear();
        self.track_mix_by_file.clear();
        self.global_solo_active = false;
    }

    pub(super) fn replace_files(&mut self, files: Vec<MidiRollFile>) {
        let had_no_files = self.files.is_empty();
        let previous_rewind_flags: HashMap<_, _> = self
            .files
            .iter()
            .zip(self.rewind_flag_ticks.iter().copied())
            .map(|(file, tick)| (file.path.clone(), tick))
            .collect();
        self.files = files;
        self.track_mix_by_file = self
            .files
            .iter()
            .map(|file| vec![TrackMixState::default(); file.data.tracks.len()])
            .collect();
        self.rewind_flag_ticks = self
            .files
            .iter()
            .map(|file| {
                previous_rewind_flags
                    .get(&file.path)
                    .copied()
                    .unwrap_or(0)
                    .min(file.data.total_ticks)
            })
            .collect();

        if had_no_files && !self.files.is_empty() {
            self.pending_initial_center = true;
            self.horizontal_scroll = 0.0;
            self.vertical_scroll = 0.0;
        }

        if self.files.is_empty() || self.selected_file >= self.files.len() {
            self.selected_file = 0;
        }

        self.playback_tick = 0;
        self.playback_total_ticks = self
            .current_file()
            .map(|file| file.data.total_ticks)
            .unwrap_or(0);
        self.playback_is_playing = false;
        self.global_solo_active = false;
    }

    pub(super) fn current_file(&self) -> Option<&MidiRollFile> {
        self.files.get(self.selected_file)
    }

    pub(super) fn apply_view_settings(&mut self, zoom_x: f32, beat_subdivision: u8) {
        self.zoom_x = zoom_x.clamp(ZOOM_MIN, ZOOM_MAX);
        self.set_beat_subdivision(beat_subdivision);
    }

    pub(super) fn zoom_in(&mut self) {
        self.zoom_x = next_zoom_step_up(self.zoom_x, ZOOM_STEP, ZOOM_MAX);
    }

    pub(super) fn zoom_out(&mut self) {
        self.zoom_x = next_zoom_step_down(self.zoom_x, ZOOM_STEP, ZOOM_MIN);
    }

    pub(super) fn reset_zoom(&mut self) {
        self.zoom_x = self.default_view_settings.zoom_x.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub(super) fn zoom_for_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let intensity = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * 0.14,
            mouse::ScrollDelta::Pixels { y, .. } => y * 0.0035,
        };

        (self.zoom_x * intensity.exp()).clamp(ZOOM_MIN, ZOOM_MAX)
    }

    pub(super) fn can_zoom_in(&self) -> bool {
        self.zoom_x < ZOOM_MAX
    }

    pub(super) fn can_zoom_out(&self) -> bool {
        self.zoom_x > ZOOM_MIN
    }

    pub(super) fn can_reset_zoom(&self) -> bool {
        (self.zoom_x - self.default_view_settings.zoom_x.clamp(ZOOM_MIN, ZOOM_MAX)).abs() > 1e-4
    }

    pub(super) fn has_multiple_files(&self) -> bool {
        self.files.len() > 1
    }

    pub(super) fn select_previous_file(&mut self) {
        if self.files.is_empty() {
            self.selected_file = 0;
            return;
        }

        if self.selected_file == 0 {
            self.selected_file = self.files.len().saturating_sub(1);
        } else {
            self.selected_file -= 1;
        }
    }

    pub(super) fn select_next_file(&mut self) {
        if self.files.is_empty() {
            self.selected_file = 0;
            return;
        }

        self.selected_file = (self.selected_file + 1) % self.files.len();
    }

    pub(super) fn track_panel_visible(&self) -> bool {
        self.track_panel_visible
    }

    pub(super) fn track_panel_width(&self) -> f32 {
        self.track_panel_width
    }

    pub(super) fn toggle_track_panel(&mut self) {
        self.track_panel_visible = !self.track_panel_visible;
    }

    pub(super) fn resize_track_panel_by(&mut self, delta: f32) {
        self.track_panel_width =
            (self.track_panel_width + delta).clamp(TRACK_PANEL_MIN_WIDTH, TRACK_PANEL_MAX_WIDTH);
    }

    pub(super) fn current_track_mix(&self) -> &[TrackMixState] {
        self.track_mix_by_file
            .get(self.selected_file)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(super) fn toggle_track_mute(&mut self, track_index: usize) -> Option<bool> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        let state = track_mix.get_mut(track_index)?;
        state.muted = !state.muted;
        Some(state.muted)
    }

    pub(super) fn toggle_track_solo(&mut self, track_index: usize) -> Option<bool> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        let state = track_mix.get_mut(track_index)?;
        state.soloed = !state.soloed;
        Some(state.soloed)
    }

    pub(super) fn set_track_muted(&mut self, track_index: usize, muted: bool) -> Option<()> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        track_mix.get_mut(track_index)?.muted = muted;
        Some(())
    }

    pub(super) fn set_track_soloed(&mut self, track_index: usize, soloed: bool) -> Option<()> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        track_mix.get_mut(track_index)?.soloed = soloed;
        Some(())
    }

    pub(super) fn set_global_solo_active(&mut self, active: bool) {
        self.global_solo_active = active;
    }

    pub(super) fn set_beat_subdivision(&mut self, subdivision: u8) {
        self.beat_subdivision = subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);
        self.beat_subdivision_input = self.beat_subdivision.to_string();
    }

    pub(super) fn set_beat_subdivision_input(&mut self, input: String) {
        let filtered: String = input
            .chars()
            .filter(|character| character.is_ascii_digit())
            .take(2)
            .collect();
        self.beat_subdivision_input = filtered.clone();

        let Ok(parsed) = filtered.parse::<u8>() else {
            return;
        };

        if (BEAT_SUBDIVISION_MIN..=BEAT_SUBDIVISION_MAX).contains(&parsed) {
            self.beat_subdivision = parsed;
        }
    }

    pub(super) fn beat_subdivision_input(&self) -> &str {
        &self.beat_subdivision_input
    }

    pub(super) fn horizontal_scroll(&self) -> f32 {
        self.horizontal_scroll
    }

    pub(super) fn set_horizontal_scroll(&mut self, offset_x: f32) {
        self.horizontal_scroll = offset_x.max(0.0);
    }

    pub(super) fn vertical_scroll(&self) -> f32 {
        self.vertical_scroll
    }

    pub(super) fn set_vertical_scroll(&mut self, offset_y: f32) {
        self.vertical_scroll = offset_y.max(0.0);
    }

    pub(super) fn pending_initial_center(&self) -> bool {
        self.pending_initial_center
    }

    pub(super) fn mark_initial_center_applied(&mut self) {
        self.pending_initial_center = false;
    }

    pub(super) fn set_playback_position(&mut self, tick: u64, total_ticks: u64, is_playing: bool) {
        self.playback_total_ticks = total_ticks;
        self.playback_tick = tick.min(total_ticks);
        self.playback_is_playing = is_playing;
    }

    pub(super) fn playback_tick(&self) -> u64 {
        self.playback_tick
    }

    pub(super) fn playback_is_playing(&self) -> bool {
        self.playback_is_playing
    }

    pub(super) fn playback_position_normalized(&self) -> f32 {
        if self.playback_total_ticks == 0 {
            return 0.0;
        }

        (self.playback_tick as f32 / self.playback_total_ticks as f32).clamp(0.0, 1.0)
    }

    pub(super) fn rewind_flag_tick(&self) -> u64 {
        let total_ticks = self
            .current_file()
            .map(|file| file.data.total_ticks)
            .unwrap_or(0);

        self.rewind_flag_ticks
            .get(self.selected_file)
            .copied()
            .unwrap_or(0)
            .min(total_ticks)
    }

    pub(super) fn set_rewind_flag_tick(&mut self, tick: u64) {
        let Some(total_ticks) = self.current_file().map(|file| file.data.total_ticks) else {
            return;
        };

        if let Some(flag_tick) = self.rewind_flag_ticks.get_mut(self.selected_file) {
            *flag_tick = tick.min(total_ticks);
        }
    }
}

fn next_zoom_step_up(current: f32, step: f32, max_zoom: f32) -> f32 {
    if step <= f32::EPSILON {
        return current;
    }

    let snapped = (current / step).round() * step;

    if (current - snapped).abs() <= 1e-4 {
        (current + step).min(max_zoom)
    } else if current < snapped {
        snapped.min(max_zoom)
    } else {
        (snapped + step).min(max_zoom)
    }
}

fn next_zoom_step_down(current: f32, step: f32, min_zoom: f32) -> f32 {
    if step <= f32::EPSILON {
        return current;
    }

    let snapped = (current / step).round() * step;

    if (current - snapped).abs() <= 1e-4 {
        (current - step).max(min_zoom)
    } else if current > snapped {
        snapped.max(min_zoom)
    } else {
        (snapped - step).max(min_zoom)
    }
}

fn drag_distance(a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn roll_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new(ROLL_SCROLL_ID)
}

pub(super) fn controls<'a>(app: &'a Lilypalooza) -> Vec<HeaderControlGroup<'a>> {
    let state = &app.piano_roll;
    let can_toggle_tracks = state
        .current_file()
        .is_some_and(|file| file.data.tracks.len() > 1);

    let track_toggle_button = button(
        svg(icons::list_music())
            .width(Length::Fixed(TRACK_TOGGLE_ICON_SIZE))
            .height(Length::Fixed(TRACK_TOGGLE_ICON_SIZE))
            .content_fit(ContentFit::Contain)
            .style(move |theme: &Theme, status| {
                let palette = theme.extended_palette();
                svg::Style {
                    color: Some(if can_toggle_tracks && state.track_panel_visible() {
                        match status {
                            svg::Status::Idle => palette.background.weakest.text,
                            svg::Status::Hovered => palette.background.base.text,
                        }
                    } else {
                        match status {
                            svg::Status::Idle => palette.background.base.text,
                            svg::Status::Hovered => palette.primary.weak.text,
                        }
                    }),
                }
            }),
    )
    .style(if can_toggle_tracks && state.track_panel_visible() {
        ui_style::button_toolbar_toggle_active
    } else {
        ui_style::button_neutral
    })
    .padding([6, 7]);
    let track_toggle_button = if can_toggle_tracks {
        track_toggle_button.on_press(Message::PianoRoll(PianoRollMessage::TrackPanelToggle))
    } else {
        track_toggle_button
    };
    let track_toggle_button = super::dock_view::delayed_tooltip(
        app,
        "piano-roll-track-toggle",
        track_toggle_button.into(),
        text("Tracks").size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Top,
    );

    let zoom_out_button = button(super::dock_view::compact_control_icon(icons::zoom_out()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let zoom_out_button = if state.can_zoom_out() {
        zoom_out_button.on_press(Message::PianoRoll(PianoRollMessage::ZoomOut))
    } else {
        zoom_out_button
    };

    let zoom_in_button = button(super::dock_view::compact_control_icon(icons::zoom_in()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let zoom_in_button = if state.can_zoom_in() {
        zoom_in_button.on_press(Message::PianoRoll(PianoRollMessage::ZoomIn))
    } else {
        zoom_in_button
    };

    let subdivision_slider = slider(
        BEAT_SUBDIVISION_MIN..=BEAT_SUBDIVISION_MAX,
        state.beat_subdivision,
        |value| Message::PianoRoll(PianoRollMessage::BeatSubdivisionSliderChanged(value)),
    )
    .step(1u8)
    .width(Length::Fixed(120.0));

    let subdivision_input = text_input("", state.beat_subdivision_input())
        .on_input(|value| Message::PianoRoll(PianoRollMessage::BeatSubdivisionInputChanged(value)))
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ])
        .size(Pixels(ui_style::FONT_SIZE_UI_XS as f32))
        .width(Length::Fixed(44.0));

    let zoom_group = HeaderControlGroup {
        min_width: 132.0,
        content: row![
            zoom_out_button,
            {
                let zoom_value = text(format!("{:.0}%", state.zoom_x * 100.0))
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO);
                let zoom_value = if state.can_reset_zoom() {
                    mouse_area(zoom_value)
                        .on_double_click(Message::PianoRoll(PianoRollMessage::ResetZoom))
                } else {
                    mouse_area(zoom_value)
                };

                super::dock_view::delayed_tooltip(
                    app,
                    "piano-roll-zoom-reset",
                    zoom_value.into(),
                    text("Double-click to reset zoom")
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .into(),
                    tooltip::Position::Top,
                )
            },
            zoom_in_button,
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
    };

    let beat_subdiv_group = HeaderControlGroup {
        min_width: 228.0,
        content: row![
            text("Beat Subdiv").size(ui_style::FONT_SIZE_UI_XS),
            subdivision_slider,
            subdivision_input,
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
    };

    let mut controls = vec![
        HeaderControlGroup {
            min_width: 34.0,
            content: track_toggle_button,
        },
        zoom_group,
        beat_subdiv_group,
    ];

    if state.has_multiple_files() {
        let prev_file_button = button(text("←").size(ui_style::FONT_SIZE_UI_SM))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
            .on_press(Message::PianoRoll(PianoRollMessage::FilePrevious));

        let next_file_button = button(text("→").size(ui_style::FONT_SIZE_UI_SM))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
            .on_press(Message::PianoRoll(PianoRollMessage::FileNext));

        let file_name = state
            .current_file()
            .map(|file| file.file_name.as_str())
            .unwrap_or("No MIDI");

        controls.push(HeaderControlGroup {
            min_width: 182.0,
            content: row![
                text("MIDI").size(ui_style::FONT_SIZE_UI_XS),
                prev_file_button,
                text(file_name)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO),
                next_file_button,
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        });
    }

    controls
}

pub(super) fn content(app: &Lilypalooza) -> Element<'_, Message> {
    let Some(file) = app.piano_roll.current_file() else {
        let is_compiling =
            app.compile_requested || app.compile_session.is_some() || app.compile_outputs_loading;
        let message = if is_compiling {
            "Compiling score to MIDI..."
        } else {
            "No MIDI output yet"
        };
        let content: Element<'_, Message> = if is_compiling {
            row![
                text(app.spinner_frame())
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(fonts::MONO),
                text(message).size(ui_style::FONT_SIZE_UI_SM),
            ]
            .spacing(ui_style::SPACE_SM)
            .align_y(alignment::Vertical::Center)
            .into()
        } else {
            text(message).size(ui_style::FONT_SIZE_UI_SM).into()
        };

        return container(content)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .style(ui_style::piano_roll_surface)
            .into();
    };
    let show_track_panel = app.piano_roll.track_panel_visible() && file.data.tracks.len() > 1;

    piano_roll_body(PianoRollBody {
        file,
        zoom_x: app.piano_roll.zoom_x,
        beat_subdivision: app.piano_roll.beat_subdivision,
        horizontal_offset: app.piano_roll.horizontal_scroll(),
        vertical_offset: app.piano_roll.vertical_scroll(),
        playback_tick: app.piano_roll.playback_tick(),
        rewind_flag_tick: app.piano_roll.rewind_flag_tick(),
        track_mix: app.piano_roll.current_track_mix(),
        global_solo_active: app.piano_roll.global_solo_active,
        track_panel_visible: show_track_panel,
        track_panel_width: app.piano_roll.track_panel_width(),
        zoom_modifier_active: app.zoom_modifier_active(),
    })
}

struct PianoRollBody<'a> {
    file: &'a MidiRollFile,
    zoom_x: f32,
    beat_subdivision: u8,
    horizontal_offset: f32,
    vertical_offset: f32,
    playback_tick: u64,
    rewind_flag_tick: u64,
    track_mix: &'a [TrackMixState],
    global_solo_active: bool,
    track_panel_visible: bool,
    track_panel_width: f32,
    zoom_modifier_active: bool,
}

fn piano_roll_body<'a>(body: PianoRollBody<'a>) -> Element<'a, Message> {
    let PianoRollBody {
        file,
        zoom_x,
        beat_subdivision,
        horizontal_offset,
        vertical_offset,
        playback_tick,
        rewind_flag_tick,
        track_mix,
        global_solo_active,
        track_panel_visible,
        track_panel_width,
        zoom_modifier_active,
    } = body;

    let pitch_rows = f32::from(pitch_count(file.data.min_pitch, file.data.max_pitch));
    let notes_height = pitch_rows * NOTE_ROW_HEIGHT;

    let pixels_per_tick = BASE_PIXELS_PER_QUARTER * zoom_x / f32::from(file.data.ppq.max(1));
    let timeline_width =
        (file.data.total_ticks as f32 * pixels_per_tick + CONTENT_RIGHT_PADDING).max(1.0);

    let track_panel_width = track_panel_width.clamp(TRACK_PANEL_MIN_WIDTH, TRACK_PANEL_MAX_WIDTH);
    let track_stub_canvas = canvas(TempoStubCanvas)
        .width(Length::Fixed(track_panel_width))
        .height(Length::Fixed(TEMPO_LANE_HEIGHT));

    let tempo_stub_canvas = canvas(TempoStubCanvas)
        .width(Length::Fixed(KEYBOARD_WIDTH))
        .height(Length::Fixed(TEMPO_LANE_HEIGHT));

    let tempo_canvas = canvas(TempoCanvas {
        data: &file.data,
        zoom_x,
        beat_subdivision,
        horizontal_scroll: horizontal_offset,
        playback_tick,
        rewind_flag_tick,
    })
    .width(Fill)
    .height(Length::Fixed(TEMPO_LANE_HEIGHT));

    let keyboard_canvas = canvas(KeyCanvas {
        min_pitch: file.data.min_pitch,
        max_pitch: file.data.max_pitch,
        vertical_offset,
    })
    .width(Length::Fixed(KEYBOARD_WIDTH))
    .height(Fill);

    let roll_canvas = canvas(RollNotesCanvas {
        data: &file.data,
        zoom_x,
        beat_subdivision,
        playback_tick,
        track_mix,
        global_solo_active,
    })
    .width(Length::Fixed(timeline_width))
    .height(Length::Fixed(notes_height));

    let roll_scroll = scrollable(roll_canvas)
        .id(roll_scroll_id())
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::new(),
            horizontal: scrollable::Scrollbar::new(),
        })
        .on_scroll(|viewport| {
            let offset = viewport.absolute_offset();
            Message::PianoRoll(PianoRollMessage::RollScrolled {
                x: offset.x,
                y: offset.y,
            })
        })
        .width(Fill)
        .height(Fill)
        .style(ui_style::workspace_scrollable);
    let roll_scroll = stack([
        roll_scroll.into(),
        piano_roll_scroll_position_marker(playback_tick, file.data.total_ticks),
    ])
    .width(Fill)
    .height(Fill);

    let right_content = column![tempo_canvas, roll_scroll]
        .width(Fill)
        .height(Fill)
        .spacing(0);
    let right_content = container(right_content).width(Fill).height(Fill);

    let zoom_overlay: Element<'_, Message> = if zoom_modifier_active {
        mouse_area(container(text("")).width(Fill).height(Fill))
            .on_scroll(|delta| Message::PianoRoll(PianoRollMessage::SmoothZoom(delta)))
            .into()
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };

    let right_content: Element<'_, Message> = mouse_area(
        stack([right_content.into(), zoom_overlay])
            .width(Fill)
            .height(Fill),
    )
    .on_move(|position| Message::PianoRoll(PianoRollMessage::ViewportCursorMoved(position)))
    .on_exit(Message::PianoRoll(PianoRollMessage::ViewportCursorLeft))
    .into();

    let left_content = column![tempo_stub_canvas, keyboard_canvas]
        .width(Length::Fixed(KEYBOARD_WIDTH))
        .height(Fill)
        .spacing(0);

    let content = if track_panel_visible {
        let resize_handle = canvas(TrackResizeHandle)
            .width(Length::Fixed(TRACK_RESIZE_HANDLE_WIDTH))
            .height(Fill);

        let track_panel = column![
            track_stub_canvas,
            container(track_list(file, track_mix, track_panel_width))
                .width(Length::Fixed(track_panel_width))
                .height(Fill),
        ]
        .width(Length::Fixed(track_panel_width))
        .height(Fill)
        .spacing(0);

        row![track_panel, resize_handle, left_content, right_content]
    } else {
        row![left_content, right_content]
    }
    .width(Fill)
    .height(Fill)
    .spacing(0);

    container(content)
        .width(Fill)
        .height(Fill)
        .style(ui_style::piano_roll_surface)
        .into()
}

fn track_list<'a>(
    file: &'a MidiRollFile,
    track_mix: &'a [TrackMixState],
    track_panel_width: f32,
) -> Element<'a, Message> {
    let mut tracks_column = column![]
        .width(Fill)
        .spacing(ui_style::SPACE_XS)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_XS]);
    let label_max_chars = max_track_label_chars(track_panel_width);

    for track in &file.data.tracks {
        let state = track_mix.get(track.index).copied().unwrap_or_default();
        let track_label = shorten_label(&track.label, label_max_chars);

        let solo_button = button(
            container(
                text("S")
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO)
                    .center(),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
        )
        .padding(0)
        .width(Length::Fixed(TRACK_BUTTON_WIDTH))
        .height(Length::Fixed(TRACK_BUTTON_HEIGHT))
        .style(if state.soloed {
            ui_style::button_compact_active
        } else {
            ui_style::button_compact_solid
        })
        .on_press(Message::PianoRoll(PianoRollMessage::TrackSoloToggled(
            track.index,
        )));

        let mute_button = button(
            container(
                text("M")
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO)
                    .center(),
            )
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
        )
        .padding(0)
        .width(Length::Fixed(TRACK_BUTTON_WIDTH))
        .height(Length::Fixed(TRACK_BUTTON_HEIGHT))
        .style(if state.muted {
            ui_style::button_compact_active
        } else {
            ui_style::button_compact_solid
        })
        .on_press(Message::PianoRoll(PianoRollMessage::TrackMuteToggled(
            track.index,
        )));

        tracks_column = tracks_column.push(
            row![
                text(track_label)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .width(Fill),
                container(text("")).width(Length::Fixed(TRACK_LABEL_BUTTON_GAP)),
                solo_button,
                container(text("")).width(Length::Fixed(TRACK_BUTTONS_GAP)),
                mute_button,
            ]
            .align_y(alignment::Vertical::Center)
            .spacing(0)
            .width(Fill),
        );
    }

    if file.data.tracks.len() <= 1 {
        tracks_column = tracks_column.push(
            text("No parts")
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(fonts::MONO),
        );
    }

    scrollable(tracks_column)
        .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new()))
        .style(ui_style::workspace_scrollable)
        .into()
}

struct TempoStubCanvas;

impl<Message> canvas::Program<Message> for TempoStubCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            bounds.size(),
            palette.background.weak.color,
        );

        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Default)]
struct TrackResizeState {
    dragging: bool,
    last_cursor_x: Option<f32>,
}

struct TrackResizeHandle;

impl canvas::Program<Message> for TrackResizeHandle {
    type State = TrackResizeState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.position_in(bounds).is_some() {
                    state.dragging = true;
                    state.last_cursor_x = cursor.position().map(|position| position.x);

                    return Some(canvas::Action::capture());
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging {
                    let cursor_x = cursor.position().map(|position| position.x);
                    if let (Some(last_x), Some(cursor_x)) = (state.last_cursor_x, cursor_x) {
                        let delta = cursor_x - last_x;
                        state.last_cursor_x = Some(cursor_x);

                        return Some(
                            canvas::Action::publish(Message::PianoRoll(
                                PianoRollMessage::TrackPanelResizedBy(delta),
                            ))
                            .and_capture(),
                        );
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                state.dragging = false;
                state.last_cursor_x = None;
            }
            _ => {}
        }

        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            bounds.size(),
            Color::from_rgba(
                palette.background.base.color.r,
                palette.background.base.color.g,
                palette.background.base.color.b,
                0.90,
            ),
        );

        frame.fill_rectangle(
            Point::new((bounds.width * 0.5).floor(), 0.0),
            Size::new(1.0, bounds.height),
            Color::from_rgba(
                palette.background.strong.color.r,
                palette.background.strong.color.g,
                palette.background.strong.color.b,
                0.55,
            ),
        );

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging || cursor.position_in(bounds).is_some() {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::None
        }
    }
}

struct TempoCanvas<'a> {
    data: &'a MidiRollData,
    zoom_x: f32,
    beat_subdivision: u8,
    horizontal_scroll: f32,
    playback_tick: u64,
    rewind_flag_tick: u64,
}

#[derive(Debug, Default)]
struct TempoCanvasState {
    cache: Cache,
    cache_key: Option<TempoCanvasCacheKey>,
    rewind_flag_press_origin: Option<Point>,
    dragging_rewind_flag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TempoCanvasCacheKey {
    total_ticks: u64,
    ppq: u16,
    zoom_x_bits: u32,
    beat_subdivision: u8,
    horizontal_scroll_bits: u32,
    tempo_changes_len: usize,
    bar_lines_len: usize,
}

impl canvas::Program<Message> for TempoCanvas<'_> {
    type State = TempoCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cache_key = TempoCanvasCacheKey {
            total_ticks: self.data.total_ticks,
            ppq: self.data.ppq,
            zoom_x_bits: self.zoom_x.to_bits(),
            beat_subdivision: self.beat_subdivision,
            horizontal_scroll_bits: self.horizontal_scroll.to_bits(),
            tempo_changes_len: self.data.tempo_changes.len(),
            bar_lines_len: self.data.bar_lines.len(),
        };
        if state.cache_key != Some(cache_key) {
            state.cache.clear();
            state.cache_key = Some(cache_key);
        }

        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let cursor_position = cursor.position_in(bounds)?;
                let tick = tick_from_tempo_lane_x(
                    cursor_position.x,
                    pixels_per_tick,
                    self.horizontal_scroll,
                    self.data.total_ticks,
                );
                let tick = snap_tick_to_subdivision_grid(self.data, self.beat_subdivision, tick);
                state.dragging_rewind_flag = false;

                Some(
                    canvas::Action::publish(Message::PianoRoll(
                        PianoRollMessage::SetRewindFlagTicks(tick),
                    ))
                    .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let cursor_position = cursor.position_in(bounds)?;

                if rewind_flag_hitbox(
                    self.rewind_flag_tick,
                    pixels_per_tick,
                    self.horizontal_scroll,
                    bounds,
                )
                .contains(cursor_position)
                {
                    state.rewind_flag_press_origin = Some(cursor_position);
                    state.dragging_rewind_flag = false;
                    return Some(canvas::Action::capture());
                }

                let tick = tick_from_tempo_lane_x(
                    cursor_position.x,
                    pixels_per_tick,
                    self.horizontal_scroll,
                    self.data.total_ticks,
                );

                Some(
                    canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetCursorTicks(
                        tick,
                    )))
                    .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Some(cursor_position) = cursor.position_in(bounds) else {
                    state.rewind_flag_press_origin = None;
                    state.dragging_rewind_flag = false;
                    return None;
                };

                if !state.dragging_rewind_flag {
                    let origin = state.rewind_flag_press_origin?;

                    if drag_distance(origin, cursor_position) < DRAG_START_THRESHOLD {
                        return Some(canvas::Action::capture());
                    }

                    state.dragging_rewind_flag = true;
                }

                let tick = tick_from_tempo_lane_x(
                    cursor_position.x,
                    pixels_per_tick,
                    self.horizontal_scroll,
                    self.data.total_ticks,
                );
                let tick = snap_tick_to_subdivision_grid(self.data, self.beat_subdivision, tick);

                Some(
                    canvas::Action::publish(Message::PianoRoll(
                        PianoRollMessage::SetRewindFlagTicks(tick),
                    ))
                    .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(_))
            | canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                state.rewind_flag_press_origin = None;
                state.dragging_rewind_flag = false;
                None
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));
        let static_geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(
                Point::new(0.0, 0.0),
                bounds.size(),
                palette.background.weak.color,
            );

            draw_bar_lines(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                0.0,
                bounds.height,
                palette,
            );
            draw_bar_numbers(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                bounds.height,
                palette,
            );
            draw_tempo_markers(
                frame,
                self.data,
                pixels_per_tick,
                self.horizontal_scroll,
                bounds.height,
                palette,
            );
        });

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));
        draw_rewind_flag(
            &mut frame,
            self.rewind_flag_tick,
            pixels_per_tick,
            self.horizontal_scroll,
            bounds.height,
            palette,
        );
        draw_playback_cursor(
            &mut frame,
            self.playback_tick,
            pixels_per_tick,
            self.horizontal_scroll,
            0.0,
            bounds.height,
            palette,
        );

        vec![static_geometry, frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));

        if state.dragging_rewind_flag {
            return mouse::Interaction::Grabbing;
        }

        if cursor.position_in(bounds).is_some_and(|position| {
            rewind_flag_hitbox(
                self.rewind_flag_tick,
                pixels_per_tick,
                self.horizontal_scroll,
                bounds,
            )
            .contains(position)
        }) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
    }
}

struct KeyCanvas {
    min_pitch: u8,
    max_pitch: u8,
    vertical_offset: f32,
}

impl<Message> canvas::Program<Message> for KeyCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let white_key = Color::from_rgb8(238, 238, 238);
        let black_key = Color::from_rgb8(44, 44, 44);
        let white_text = Color::from_rgb8(242, 242, 242);
        let black_text = Color::from_rgb8(32, 32, 32);

        for pitch in self.min_pitch..=self.max_pitch {
            let y = pitch_to_y(
                self.max_pitch,
                pitch,
                NOTE_ROW_HEIGHT,
                -self.vertical_offset,
            );
            let is_black = is_black_key(pitch);
            let row_color = if is_black { black_key } else { white_key };
            let text_color = if is_black { white_text } else { black_text };

            frame.fill_rectangle(
                Point::new(0.0, y),
                Size::new(bounds.width, NOTE_ROW_HEIGHT),
                row_color,
            );

            frame.stroke_rectangle(
                Point::new(0.0, y),
                Size::new(bounds.width, NOTE_ROW_HEIGHT),
                canvas::Stroke {
                    width: 1.0,
                    style: canvas::Style::Solid(Color::from_rgba(0.0, 0.0, 0.0, 0.18)),
                    ..canvas::Stroke::default()
                },
            );

            if pitch % 12 == 0 || pitch == self.min_pitch || pitch == self.max_pitch {
                frame.fill_text(canvas::Text {
                    content: pitch_name(pitch),
                    position: Point::new(4.0, y + NOTE_ROW_HEIGHT * 0.5),
                    color: text_color,
                    size: Pixels(ui_style::FONT_SIZE_UI_XS as f32),
                    font: fonts::MONO,
                    align_y: alignment::Vertical::Center,
                    ..canvas::Text::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}

struct RollNotesCanvas<'a> {
    data: &'a MidiRollData,
    zoom_x: f32,
    beat_subdivision: u8,
    playback_tick: u64,
    track_mix: &'a [TrackMixState],
    global_solo_active: bool,
}

#[derive(Debug, Default)]
struct RollNotesState {
    cache: Cache,
    cache_key: Option<RollNotesCacheKey>,
    stack_offsets: HashMap<NoteStackKey, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RollNotesCacheKey {
    total_ticks: u64,
    ppq: u16,
    min_pitch: u8,
    max_pitch: u8,
    notes_len: usize,
    tracks_len: usize,
    time_signatures_len: usize,
    zoom_x_bits: u32,
    beat_subdivision: u8,
    track_mix_hash: u64,
    global_solo_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NoteStackKey {
    start_tick: u64,
    pitch: u8,
}

#[derive(Debug, Clone, Copy)]
struct NoteGeometry {
    index: usize,
    key: NoteStackKey,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
struct VisibilityState<'a> {
    track_mix: &'a [TrackMixState],
    global_solo_active: bool,
}

#[derive(Debug, Clone, Copy)]
struct StackTop {
    note_index: usize,
    position: usize,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct StackBadge {
    position: usize,
    len: usize,
}

impl canvas::Program<Message> for RollNotesCanvas<'_> {
    type State = RollNotesState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cache_key = RollNotesCacheKey {
            total_ticks: self.data.total_ticks,
            ppq: self.data.ppq,
            min_pitch: self.data.min_pitch,
            max_pitch: self.data.max_pitch,
            notes_len: self.data.notes.len(),
            tracks_len: self.data.tracks.len(),
            time_signatures_len: self.data.time_signatures.len(),
            zoom_x_bits: self.zoom_x.to_bits(),
            beat_subdivision: self.beat_subdivision,
            track_mix_hash: track_mix_hash(self.track_mix),
            global_solo_active: self.global_solo_active,
        };
        if self.state_needs_cache_clear(state, cache_key) {
            state.cache.clear();
            state.cache_key = Some(cache_key);
        }

        let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event else {
            return None;
        };
        let cursor_position = cursor.position_in(bounds)?;

        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));
        let tick = ((cursor_position.x / pixels_per_tick).round() as i64)
            .clamp(0, self.data.total_ticks as i64) as u64;
        let notes = build_note_geometries(self.data, pixels_per_tick);
        let stacks = build_note_stacks(&notes);
        let draw_order = compute_note_draw_order(&notes, &stacks, &state.stack_offsets);

        for note_index in draw_order.into_iter().rev() {
            let geometry = notes[note_index];

            if !note_contains_point(geometry, cursor_position) {
                continue;
            }

            let Some(stack) = stacks.get(&geometry.key) else {
                continue;
            };
            if stack.len() <= 1 {
                continue;
            }

            let offset = state.stack_offsets.entry(geometry.key).or_insert(0);
            *offset = (*offset + 1) % stack.len();
            state.cache.clear();

            return Some(
                canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)))
                    .and_capture(),
            );
        }

        Some(
            canvas::Action::publish(Message::PianoRoll(PianoRollMessage::SetCursorTicks(tick)))
                .and_capture(),
        )
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let pixels_per_tick =
            BASE_PIXELS_PER_QUARTER * self.zoom_x / f32::from(self.data.ppq.max(1));
        let static_geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(
                Point::new(0.0, 0.0),
                bounds.size(),
                palette.background.base.color,
            );

            for pitch in self.data.min_pitch..=self.data.max_pitch {
                let y = pitch_to_y(self.data.max_pitch, pitch, NOTE_ROW_HEIGHT, 0.0);
                let row_color = if is_black_key(pitch) {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.11)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                };

                frame.fill_rectangle(
                    Point::new(0.0, y),
                    Size::new(bounds.width, NOTE_ROW_HEIGHT),
                    row_color,
                );
            }

            draw_grid(
                frame,
                self.data,
                self.beat_subdivision,
                pixels_per_tick,
                bounds.height,
                palette,
            );

            let notes = build_note_geometries(self.data, pixels_per_tick);
            let stacks = build_note_stacks(&notes);
            let draw_order = compute_note_draw_order(&notes, &stacks, &state.stack_offsets);
            let label_notes = compute_stack_label_notes(
                &stacks,
                &state.stack_offsets,
                &self.data.notes,
                self.track_mix,
                self.global_solo_active,
            );

            for note_index in draw_order {
                let geometry = notes[note_index];
                let note = &self.data.notes[note_index];
                let stack_badge = label_notes.get(&geometry.key).and_then(|top| {
                    if top.note_index == note_index {
                        Some(StackBadge {
                            position: top.position,
                            len: top.len,
                        })
                    } else {
                        None
                    }
                });
                let show_label = !stacks.contains_key(&geometry.key) || stack_badge.is_some();
                draw_note(
                    frame,
                    self.data,
                    VisibilityState {
                        track_mix: self.track_mix,
                        global_solo_active: self.global_solo_active,
                    },
                    note,
                    geometry,
                    stack_badge,
                    show_label,
                );
            }
        });

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        draw_playback_cursor(
            &mut frame,
            self.playback_tick,
            pixels_per_tick,
            0.0,
            0.0,
            bounds.height,
            palette,
        );

        vec![static_geometry, frame.into_geometry()]
    }
}

impl RollNotesCanvas<'_> {
    fn state_needs_cache_clear(
        &self,
        state: &RollNotesState,
        cache_key: RollNotesCacheKey,
    ) -> bool {
        state.cache_key != Some(cache_key)
    }
}

fn track_mix_hash(track_mix: &[TrackMixState]) -> u64 {
    let mut hash = 0u64;
    for (index, state) in track_mix.iter().enumerate() {
        let bits = ((state.muted as u64) << 1) | state.soloed as u64;
        hash ^= bits.rotate_left((index % 63) as u32);
    }
    hash
}

fn draw_note(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    visibility: VisibilityState<'_>,
    note: &MidiNote,
    geometry: NoteGeometry,
    stack_badge: Option<StackBadge>,
    show_label: bool,
) {
    let mut color = track_color(note.track_index);
    let visibility_alpha = track_visibility_alpha(
        visibility.track_mix,
        note.track_index,
        visibility.global_solo_active,
    );
    color.a *= visibility_alpha;

    let track_label = data
        .tracks
        .get(note.track_index)
        .map(|track| shorten_label(&track.label, 14))
        .unwrap_or_else(|| "?".to_string());

    frame.fill_rectangle(
        Point::new(geometry.x, geometry.y),
        Size::new(geometry.width, geometry.height),
        color,
    );

    frame.stroke_rectangle(
        Point::new(geometry.x, geometry.y),
        Size::new(geometry.width, geometry.height),
        canvas::Stroke {
            width: 1.0,
            style: canvas::Style::Solid(Color::from_rgba(
                0.0,
                0.0,
                0.0,
                0.38 * visibility_alpha.max(0.30),
            )),
            ..canvas::Stroke::default()
        },
    );

    if show_label && geometry.width > 38.0 && visibility_alpha > 0.20 {
        let note_label = if let Some(stack_badge) = stack_badge {
            format!(
                "{track_label} {}/{}",
                stack_badge.position + 1,
                stack_badge.len
            )
        } else {
            track_label
        };

        frame.fill_text(canvas::Text {
            content: note_label,
            position: Point::new(geometry.x + 3.0, geometry.y + NOTE_ROW_HEIGHT * 0.5),
            color: Color::from_rgba(0.08, 0.08, 0.08, visibility_alpha.clamp(0.35, 1.0)),
            size: Pixels(10.0),
            font: fonts::MONO,
            align_y: alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }
}

fn build_note_geometries(data: &MidiRollData, pixels_per_tick: f32) -> Vec<NoteGeometry> {
    data.notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let x = note.start_tick as f32 * pixels_per_tick;
            let width =
                ((note.end_tick.saturating_sub(note.start_tick)) as f32 * pixels_per_tick).max(2.0);
            let y = pitch_to_y(data.max_pitch, note.pitch, NOTE_ROW_HEIGHT, 0.0) + 0.5;
            let height = (NOTE_ROW_HEIGHT - 1.0).max(1.0);

            NoteGeometry {
                index,
                key: NoteStackKey {
                    start_tick: note.start_tick,
                    pitch: note.pitch,
                },
                x,
                y,
                width,
                height,
            }
        })
        .collect()
}

fn build_note_stacks(notes: &[NoteGeometry]) -> HashMap<NoteStackKey, Vec<usize>> {
    let mut stacks: HashMap<NoteStackKey, Vec<usize>> = HashMap::new();

    for note in notes {
        stacks.entry(note.key).or_default().push(note.index);
    }

    stacks.retain(|_key, members| members.len() > 1);
    stacks
}

fn compute_note_draw_order(
    notes: &[NoteGeometry],
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    stack_offsets: &HashMap<NoteStackKey, usize>,
) -> Vec<usize> {
    let top_notes = compute_stack_top_notes(stacks, stack_offsets);

    let mut order = Vec::with_capacity(notes.len());
    let mut deferred = Vec::new();

    for note in notes {
        if let Some(top_index) = top_notes.get(&note.key) {
            if note.index == top_index.note_index {
                deferred.push(note.index);
            } else {
                order.push(note.index);
            }
        } else {
            order.push(note.index);
        }
    }

    order.extend(deferred);
    order
}

fn compute_stack_top_notes(
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    stack_offsets: &HashMap<NoteStackKey, usize>,
) -> HashMap<NoteStackKey, StackTop> {
    let mut top_notes = HashMap::new();

    for (key, members) in stacks {
        let len = members.len();
        let offset = stack_offsets.get(key).copied().unwrap_or(0) % len;
        let top_pos = (len - 1 + len - offset) % len;
        top_notes.insert(
            *key,
            StackTop {
                note_index: members[top_pos],
                position: top_pos,
                len,
            },
        );
    }

    top_notes
}

fn compute_stack_label_notes(
    stacks: &HashMap<NoteStackKey, Vec<usize>>,
    stack_offsets: &HashMap<NoteStackKey, usize>,
    notes: &[MidiNote],
    track_mix: &[TrackMixState],
    global_solo_active: bool,
) -> HashMap<NoteStackKey, StackTop> {
    let mut label_notes = HashMap::new();

    for (key, members) in stacks {
        let len = members.len();
        let offset = stack_offsets.get(key).copied().unwrap_or(0) % len;
        let top_pos = (len - 1 + len - offset) % len;

        let mut selected = StackTop {
            note_index: members[top_pos],
            position: top_pos,
            len,
        };

        for step in 0..len {
            let pos = (top_pos + len - step) % len;
            let candidate_note_index = members[pos];
            let Some(candidate_note) = notes.get(candidate_note_index) else {
                continue;
            };

            if track_visibility_alpha(track_mix, candidate_note.track_index, global_solo_active)
                > 0.20
            {
                selected = StackTop {
                    note_index: candidate_note_index,
                    position: pos,
                    len,
                };
                break;
            }
        }

        label_notes.insert(*key, selected);
    }

    label_notes
}

fn note_contains_point(note: NoteGeometry, point: Point) -> bool {
    point.x >= note.x
        && point.x <= note.x + note.width
        && point.y >= note.y
        && point.y <= note.y + note.height
}

fn draw_grid(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    beat_subdivision: u8,
    pixels_per_tick: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let beat_subdivision = beat_subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);

    for (index, signature) in data.time_signatures.iter().enumerate() {
        let start_tick = signature.tick;
        let end_tick = data
            .time_signatures
            .get(index + 1)
            .map(|next| next.tick)
            .unwrap_or(data.total_ticks.saturating_add(1));

        let beat_step = beat_step_ticks(data.ppq, *signature).max(1);
        let mut beat_tick = start_tick;

        while beat_tick <= data.total_ticks && beat_tick < end_tick {
            if beat_subdivision > 1 {
                for division in 1..beat_subdivision {
                    let subdivision_tick = beat_tick as f32
                        + (f32::from(division) * beat_step as f32 / f32::from(beat_subdivision));

                    if subdivision_tick >= end_tick as f32
                        || subdivision_tick > data.total_ticks as f32
                    {
                        break;
                    }

                    let x = subdivision_tick * pixels_per_tick;
                    frame.stroke_rectangle(
                        Point::new(x, 0.0),
                        Size::new(1.0, height.max(1.0)),
                        canvas::Stroke {
                            width: 0.8,
                            style: canvas::Style::Solid(Color::from_rgba(
                                palette.background.strong.color.r,
                                palette.background.strong.color.g,
                                palette.background.strong.color.b,
                                0.18,
                            )),
                            ..canvas::Stroke::default()
                        },
                    );
                }
            }

            let beat_x = beat_tick as f32 * pixels_per_tick;
            frame.stroke_rectangle(
                Point::new(beat_x, 0.0),
                Size::new(1.0, height.max(1.0)),
                canvas::Stroke {
                    width: 1.0,
                    style: canvas::Style::Solid(Color::from_rgba(
                        palette.background.strong.color.r,
                        palette.background.strong.color.g,
                        palette.background.strong.color.b,
                        0.38,
                    )),
                    ..canvas::Stroke::default()
                },
            );

            beat_tick = beat_tick.saturating_add(beat_step);
        }
    }
    draw_bar_lines(frame, data, pixels_per_tick, 0.0, 0.0, height, palette);
}

fn draw_bar_lines(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    y: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let bar_color = Color::from_rgba(
        palette.background.strong.color.r,
        palette.background.strong.color.g,
        palette.background.strong.color.b,
        0.56,
    );

    for bar_tick in &data.bar_lines {
        let x = *bar_tick as f32 * pixels_per_tick - horizontal_scroll;
        frame.stroke_rectangle(
            Point::new(x, y),
            Size::new(1.0, height.max(1.0)),
            canvas::Stroke {
                width: 1.8,
                style: canvas::Style::Solid(bar_color),
                ..canvas::Stroke::default()
            },
        );
    }
}

fn draw_tempo_markers(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    for (index, tempo) in data.tempo_changes.iter().enumerate() {
        let x = tempo.tick as f32 * pixels_per_tick - horizontal_scroll;
        let next_x = data
            .tempo_changes
            .get(index + 1)
            .map(|next| next.tick as f32 * pixels_per_tick - horizontal_scroll);
        let bpm = if tempo.micros_per_quarter == 0 {
            0.0
        } else {
            60_000_000.0 / tempo.micros_per_quarter as f32
        };
        let label = format!("♩={bpm:.1}");

        frame.stroke_rectangle(
            Point::new(x, 0.0),
            Size::new(1.0, height.max(1.0)),
            canvas::Stroke {
                width: 1.2,
                style: canvas::Style::Solid(Color::from_rgba(
                    palette.secondary.base.color.r,
                    palette.secondary.base.color.g,
                    palette.secondary.base.color.b,
                    0.98,
                )),
                ..canvas::Stroke::default()
            },
        );

        let mut label_x = (x + 4.0).max(4.0);
        if let Some(next_x) = next_x {
            let max_x = next_x - estimate_monospace_text_width(&label) - 6.0;
            if max_x > x + 4.0 {
                label_x = label_x.min(max_x);
            }
        }

        frame.fill_text(canvas::Text {
            content: label,
            position: Point::new(label_x, TEMPO_LABEL_TOP_PADDING),
            color: Color::from_rgba(0.96, 0.97, 0.99, 0.98),
            size: Pixels(ui_style::FONT_SIZE_UI_XS.saturating_sub(1) as f32),
            font: fonts::MONO,
            align_y: alignment::Vertical::Top,
            ..canvas::Text::default()
        });
    }
}

fn draw_rewind_flag(
    frame: &mut canvas::Frame,
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;
    let stem_left = (x - 1.0).round();
    let fill_color = palette.primary.strong.color;

    frame.fill_rectangle(
        Point::new(stem_left, 0.0),
        Size::new(2.0, height.max(1.0)),
        fill_color,
    );

    frame.fill_rectangle(
        Point::new(stem_left + 1.0, 0.0),
        Size::new(REWIND_FLAG_WIDTH, REWIND_FLAG_BANNER_HEIGHT),
        fill_color,
    );
}

fn piano_roll_scroll_position_marker(
    playback_tick: u64,
    total_ticks: u64,
) -> Element<'static, Message> {
    let normalized = if total_ticks == 0 {
        0.0
    } else {
        (playback_tick as f32 / total_ticks as f32).clamp(0.0, 1.0)
    };

    canvas(HorizontalScrollMarkerCanvas { normalized })
        .width(Fill)
        .height(Fill)
        .into()
}

struct HorizontalScrollMarkerCanvas {
    normalized: f32,
}

impl canvas::Program<Message> for HorizontalScrollMarkerCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let marker_width = SCROLL_MARKER_LENGTH.min(bounds.width.max(1.0));
        let marker_center_x = self.normalized * bounds.width.max(1.0);
        let marker_x = (marker_center_x - marker_width * 0.5)
            .clamp(0.0, (bounds.width - marker_width).max(0.0));
        let marker_y =
            (bounds.height - SCROLL_MARKER_THICKNESS - SCROLL_MARKER_EDGE_INSET).max(0.0);

        frame.fill_rectangle(
            Point::new(marker_x, marker_y),
            Size::new(marker_width, SCROLL_MARKER_THICKNESS),
            Color::from_rgba(
                palette.secondary.base.color.r,
                palette.secondary.base.color.g,
                palette.secondary.base.color.b,
                0.72,
            ),
        );

        vec![frame.into_geometry()]
    }
}

fn rewind_flag_hitbox(
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    bounds: Rectangle,
) -> Rectangle {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;

    Rectangle {
        x: x - REWIND_FLAG_HITBOX_WIDTH * 0.5,
        y: 0.0,
        width: REWIND_FLAG_HITBOX_WIDTH,
        height: bounds.height,
    }
}

fn tick_from_tempo_lane_x(
    local_x: f32,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    total_ticks: u64,
) -> u64 {
    let absolute_x = local_x + horizontal_scroll;

    ((absolute_x / pixels_per_tick).round() as i64).clamp(0, total_ticks as i64) as u64
}

fn snap_tick_to_subdivision_grid(data: &MidiRollData, beat_subdivision: u8, tick: u64) -> u64 {
    let beat_subdivision = beat_subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);
    let clamped_tick = tick.min(data.total_ticks);
    let default_signature = TimeSignatureChange {
        tick: 0,
        numerator: 4,
        denominator: 4,
    };
    let signatures: &[TimeSignatureChange] = if data.time_signatures.is_empty() {
        std::slice::from_ref(&default_signature)
    } else {
        &data.time_signatures
    };

    let mut best_tick = 0;
    let mut best_distance = u64::MAX;

    for (index, signature) in signatures.iter().enumerate() {
        let start_tick = signature.tick.min(data.total_ticks);
        let next_signature_tick = signatures
            .get(index + 1)
            .map(|next| next.tick)
            .unwrap_or(data.total_ticks.saturating_add(1));
        let end_tick = next_signature_tick.min(data.total_ticks.saturating_add(1));
        let beat_step = beat_step_ticks(data.ppq, *signature).max(1) as f64;
        let subdivision_step = (beat_step / f64::from(beat_subdivision)).max(f64::EPSILON);
        let relative_tick = clamped_tick.saturating_sub(start_tick) as f64;
        let center_index = (relative_tick / subdivision_step).round() as i64;

        for candidate_index in [center_index - 1, center_index, center_index + 1] {
            if candidate_index < 0 {
                continue;
            }

            let candidate_tick =
                (start_tick as f64 + candidate_index as f64 * subdivision_step).round() as u64;

            if candidate_tick < start_tick
                || candidate_tick > data.total_ticks
                || candidate_tick >= end_tick
            {
                continue;
            }

            let distance = candidate_tick.abs_diff(clamped_tick);
            if distance < best_distance || (distance == best_distance && candidate_tick < best_tick)
            {
                best_tick = candidate_tick;
                best_distance = distance;
            }
        }
    }

    if best_distance == u64::MAX {
        clamped_tick
    } else {
        best_tick
    }
}

pub(super) fn adjacent_subdivision_tick(
    data: &MidiRollData,
    beat_subdivision: u8,
    tick: u64,
    forward: bool,
) -> u64 {
    let beat_subdivision = beat_subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);
    let clamped_tick = tick.min(data.total_ticks);
    let default_signature = TimeSignatureChange {
        tick: 0,
        numerator: 4,
        denominator: 4,
    };
    let signatures: &[TimeSignatureChange] = if data.time_signatures.is_empty() {
        std::slice::from_ref(&default_signature)
    } else {
        &data.time_signatures
    };

    let mut best = None;

    for (index, signature) in signatures.iter().enumerate() {
        let start_tick = signature.tick.min(data.total_ticks);
        let next_signature_tick = signatures
            .get(index + 1)
            .map(|next| next.tick)
            .unwrap_or(data.total_ticks.saturating_add(1));
        let end_tick = next_signature_tick.min(data.total_ticks.saturating_add(1));
        let beat_step = beat_step_ticks(data.ppq, *signature).max(1) as f64;
        let subdivision_step = (beat_step / f64::from(beat_subdivision)).max(f64::EPSILON);
        let relative_tick = clamped_tick.saturating_sub(start_tick) as f64;
        let base_index = if forward {
            (relative_tick / subdivision_step).floor() as i64 + 1
        } else {
            (relative_tick / subdivision_step).ceil() as i64 - 1
        };

        for candidate_index in [base_index - 1, base_index, base_index + 1] {
            if candidate_index < 0 {
                continue;
            }

            let candidate_tick =
                (start_tick as f64 + candidate_index as f64 * subdivision_step).round() as u64;

            if candidate_tick < start_tick
                || candidate_tick > data.total_ticks
                || candidate_tick >= end_tick
            {
                continue;
            }

            if forward {
                if candidate_tick <= clamped_tick {
                    continue;
                }

                if best.is_none_or(|best_tick| candidate_tick < best_tick) {
                    best = Some(candidate_tick);
                }
            } else {
                if candidate_tick >= clamped_tick {
                    continue;
                }

                if best.is_none_or(|best_tick| candidate_tick > best_tick) {
                    best = Some(candidate_tick);
                }
            }
        }
    }

    best.unwrap_or(clamped_tick)
}

fn draw_bar_numbers(
    frame: &mut canvas::Frame,
    data: &MidiRollData,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    for (bar_index, bar_tick) in data.bar_lines.iter().enumerate() {
        let x = *bar_tick as f32 * pixels_per_tick - horizontal_scroll;
        let label = format!("{}", bar_index + 1);
        let mut label_x = (x + 4.0).max(4.0);

        if let Some(next_bar_tick) = data.bar_lines.get(bar_index + 1) {
            let next_x = *next_bar_tick as f32 * pixels_per_tick - horizontal_scroll;
            let max_x = next_x - estimate_monospace_text_width(&label) - 6.0;
            if max_x <= x + 4.0 {
                continue;
            }
            label_x = label_x.min(max_x);
        }

        frame.fill_text(canvas::Text {
            content: label,
            position: Point::new(label_x, height - BAR_LABEL_BOTTOM_PADDING),
            color: Color::from_rgba(
                palette.background.weak.text.r,
                palette.background.weak.text.g,
                palette.background.weak.text.b,
                0.82,
            ),
            size: Pixels(ui_style::FONT_SIZE_UI_XS.saturating_sub(2) as f32),
            font: fonts::MONO,
            align_y: alignment::Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}

fn draw_playback_cursor(
    frame: &mut canvas::Frame,
    tick: u64,
    pixels_per_tick: f32,
    horizontal_scroll: f32,
    y: f32,
    height: f32,
    palette: &iced::theme::palette::Extended,
) {
    let x = tick as f32 * pixels_per_tick - horizontal_scroll;

    frame.stroke_rectangle(
        Point::new(x, y),
        Size::new(1.0, height.max(1.0)),
        canvas::Stroke {
            width: 1.4,
            style: canvas::Style::Solid(Color::from_rgba(
                palette.primary.base.color.r,
                palette.primary.base.color.g,
                palette.primary.base.color.b,
                0.92,
            )),
            ..canvas::Stroke::default()
        },
    );
}

fn estimate_monospace_text_width(text: &str) -> f32 {
    text.chars().count() as f32 * ui_style::FONT_SIZE_UI_XS as f32 * 0.60
}

fn max_track_label_chars(track_panel_width: f32) -> usize {
    let horizontal_padding = f32::from(ui_style::PADDING_XS) * 2.0;
    let reserved_width =
        TRACK_LABEL_BUTTON_GAP + TRACK_BUTTON_WIDTH + TRACK_BUTTONS_GAP + TRACK_BUTTON_WIDTH;
    let available_width = (track_panel_width - horizontal_padding - reserved_width).max(0.0);
    let approx_char_width = ui_style::FONT_SIZE_UI_XS as f32 * 0.60;
    let estimated = (available_width / approx_char_width).floor() as usize;

    estimated.clamp(4, 18)
}

fn pitch_to_y(max_pitch: u8, pitch: u8, row_height: f32, top_offset: f32) -> f32 {
    let row = f32::from(max_pitch.saturating_sub(pitch));
    top_offset + row * row_height
}

fn pitch_count(min_pitch: u8, max_pitch: u8) -> u16 {
    u16::from(max_pitch.saturating_sub(min_pitch)) + 1
}

fn beat_step_ticks(ppq: u16, signature: TimeSignatureChange) -> u64 {
    let quarter = u64::from(ppq.max(1));
    let denominator = u64::from(signature.denominator.max(1));

    quarter.saturating_mul(4) / denominator
}

fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

fn pitch_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let note_name = NAMES[usize::from(pitch % 12)];
    let octave = i16::from(pitch) / 12 - 1;

    format!("{note_name}{octave}")
}

fn track_visibility_alpha(
    track_mix: &[TrackMixState],
    track_index: usize,
    global_solo_active: bool,
) -> f32 {
    let Some(current_state) = track_mix.get(track_index) else {
        return 1.0;
    };

    if current_state.muted {
        return 0.10;
    }

    let has_any_solo = global_solo_active || track_mix.iter().any(|state| state.soloed);
    if has_any_solo && !current_state.soloed {
        return 0.18;
    }

    1.0
}

fn track_color(track_index: usize) -> Color {
    const COLORS: [Color; 12] = [
        Color::from_rgb(0.90, 0.35, 0.35),
        Color::from_rgb(0.90, 0.62, 0.31),
        Color::from_rgb(0.88, 0.82, 0.30),
        Color::from_rgb(0.50, 0.82, 0.33),
        Color::from_rgb(0.29, 0.76, 0.49),
        Color::from_rgb(0.28, 0.75, 0.70),
        Color::from_rgb(0.29, 0.63, 0.90),
        Color::from_rgb(0.44, 0.53, 0.92),
        Color::from_rgb(0.65, 0.47, 0.92),
        Color::from_rgb(0.83, 0.41, 0.82),
        Color::from_rgb(0.86, 0.38, 0.63),
        Color::from_rgb(0.77, 0.43, 0.48),
    ];

    COLORS[track_index % COLORS.len()]
}

fn shorten_label(label: &str, max_len: usize) -> String {
    if label.chars().count() <= max_len {
        return label.to_string();
    }

    let mut shortened: String = label.chars().take(max_len.saturating_sub(1)).collect();
    shortened.push('~');
    shortened
}

#[cfg(test)]
mod tests {
    use super::{TrackMixState, track_visibility_alpha};

    #[test]
    fn external_solo_dims_visible_tracks() {
        let track_mix = vec![TrackMixState::default(), TrackMixState::default()];

        assert!(track_visibility_alpha(&track_mix, 0, true) < 0.20);
        assert!(track_visibility_alpha(&track_mix, 1, true) < 0.20);
    }

    #[test]
    fn local_solo_keeps_soloed_track_visible() {
        let track_mix = vec![
            TrackMixState {
                muted: false,
                soloed: false,
            },
            TrackMixState {
                muted: false,
                soloed: true,
            },
        ];

        assert!(track_visibility_alpha(&track_mix, 0, false) < 0.20);
        assert_eq!(track_visibility_alpha(&track_mix, 1, false), 1.0);
    }
}
