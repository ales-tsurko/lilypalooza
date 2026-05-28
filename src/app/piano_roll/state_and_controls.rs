use iced::widget::column;

use super::*;

pub(in crate::app) const HEADER_CONTROL_HEIGHT: f32 = crate::app::dock_view::HEADER_CONTROL_HEIGHT;
pub(in crate::app) const TRACK_PANEL_DEFAULT_WIDTH: f32 = 168.0;
pub(in crate::app) const TRACK_PANEL_MIN_WIDTH: f32 = 124.0;
pub(in crate::app) const TRACK_PANEL_MAX_WIDTH: f32 = ui_style::grid_f32(56);
pub(in crate::app) const TRACK_RESIZE_HANDLE_WIDTH: f32 = ui_style::grid_f32(2);
pub(in crate::app) const TRACK_COLOR_BUTTON_SIZE: f32 = ui_style::grid_f32(4);
pub(in crate::app) const TRACK_COLOR_BUTTON_GAP: f32 = ui_style::grid_f32(2);
pub(in crate::app) const TRACK_BUTTON_WIDTH: f32 = ui_style::grid_f32(4);
pub(in crate::app) const TRACK_BUTTON_HEIGHT: f32 = ui_style::grid_f32(4);
pub(in crate::app) const TRACK_ROW_HEIGHT: f32 = ui_style::grid_f32(7);
pub(in crate::app) const TRACK_LIST_SCROLLBAR_GUTTER: u16 = 12;
pub(in crate::app) const TRACK_BUTTONS_GAP: f32 = 4.0;
pub(in crate::app) const TRACK_LABEL_BUTTON_GAP: f32 = ui_style::grid_f32(3);
pub(in crate::app) const TRACK_BUTTONS_GROUP_WIDTH: f32 =
    TRACK_BUTTON_WIDTH * 2.0 + TRACK_BUTTONS_GAP;
pub(in crate::app) const DRAG_START_THRESHOLD: f32 = 8.0;
pub(in crate::app) const KEYBOARD_WIDTH: f32 = ui_style::grid_f32(8);
pub(in crate::app) const TEMPO_LANE_HEIGHT: f32 = 28.0;
pub(in crate::app) const REWIND_FLAG_HITBOX_WIDTH: f32 = ui_style::grid_f32(4);
pub(in crate::app) const REWIND_FLAG_WIDTH: f32 = ui_style::grid_f32(3);
pub(in crate::app) const REWIND_FLAG_BANNER_HEIGHT: f32 = 12.0;
pub(in crate::app) const SCROLL_MARKER_THICKNESS: f32 = 3.0;
pub(in crate::app) const SCROLL_MARKER_LENGTH: f32 = ui_style::grid_f32(4);
pub(in crate::app) const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
pub(in crate::app) const TEMPO_LABEL_TOP_PADDING: f32 = ui_style::grid_f32(1);
pub(in crate::app) const BAR_LABEL_BOTTOM_PADDING: f32 = ui_style::grid_f32(1);
pub(in crate::app) const NOTE_ROW_HEIGHT: f32 = ui_style::grid_f32(4);
pub(in crate::app) const CONTENT_RIGHT_PADDING: f32 = 24.0;
pub(in crate::app) const TRACK_TOGGLE_ICON_SIZE: f32 = ui_style::grid_f32(3);
pub(in crate::app) const ZOOM_MIN: f32 = 0.3;
pub(in crate::app) const ZOOM_MAX: f32 = 6.0;
pub(in crate::app) const ZOOM_STEP: f32 = 0.1;
pub(in crate::app) const BASE_PIXELS_PER_QUARTER: f32 = 72.0;
pub(in crate::app) const BEAT_SUBDIVISION_MIN: u8 = 1;
pub(in crate::app) const BEAT_SUBDIVISION_MAX: u8 = 16;
pub(in crate::app) const ROLL_SCROLL_ID: &str = "piano-roll-scroll";
pub(in crate::app) const TRACK_LIST_SCROLL_ID: &str = "piano-roll-track-list-scroll";

#[derive(Debug, Clone)]
pub(in crate::app) struct PianoRollState {
    pub(in crate::app) visible: bool,
    pub(in crate::app) zoom_x: f32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) default_view_settings: PianoRollViewSettings,
    pub(in crate::app) beat_subdivision_input: String,
    pub(in crate::app) pending_initial_center: bool,
    pub(in crate::app) horizontal_scroll: f32,
    pub(in crate::app) vertical_scroll: f32,
    pub(in crate::app) playback_tick: u64,
    pub(in crate::app) playback_total_ticks: u64,
    pub(in crate::app) playback_is_playing: bool,
    pub(in crate::app) rewind_flag_ticks: Vec<u64>,
    pub(in crate::app) files: Vec<MidiRollFile>,
    pub(in crate::app) track_mix_by_file: Vec<Vec<TrackMixState>>,
    pub(in crate::app) global_solo_active: bool,
    pub(in crate::app) track_panel_visible: bool,
    pub(in crate::app) track_panel_width: f32,
    pub(in crate::app) selected_file: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::app) struct TrackMixState {
    pub(in crate::app) muted: bool,
    pub(in crate::app) soloed: bool,
}

impl PianoRollState {
    pub(in crate::app) fn new(default_view_settings: PianoRollViewSettings) -> Self {
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

    pub(in crate::app) fn clear_files(&mut self) {
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

    pub(in crate::app) fn replace_files(&mut self, files: Vec<MidiRollFile>) {
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

    pub(in crate::app) fn current_file(&self) -> Option<&MidiRollFile> {
        self.files.get(self.selected_file)
    }

    pub(in crate::app) fn apply_view_settings(&mut self, zoom_x: f32, beat_subdivision: u8) {
        self.zoom_x = zoom_x.clamp(ZOOM_MIN, ZOOM_MAX);
        self.set_beat_subdivision(beat_subdivision);
    }

    pub(in crate::app) fn zoom_in(&mut self) {
        self.zoom_x = next_zoom_step_up(self.zoom_x, ZOOM_STEP, ZOOM_MAX);
    }

    pub(in crate::app) fn zoom_out(&mut self) {
        self.zoom_x = next_zoom_step_down(self.zoom_x, ZOOM_STEP, ZOOM_MIN);
    }

    pub(in crate::app) fn reset_zoom(&mut self) {
        self.zoom_x = self.default_view_settings.zoom_x.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub(in crate::app) fn zoom_for_delta(&self, delta: mouse::ScrollDelta) -> f32 {
        let intensity = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * 0.14,
            mouse::ScrollDelta::Pixels { y, .. } => y * 0.0035,
        };

        (self.zoom_x * intensity.exp()).clamp(ZOOM_MIN, ZOOM_MAX)
    }

    pub(in crate::app) fn can_zoom_in(&self) -> bool {
        self.zoom_x < ZOOM_MAX
    }

    pub(in crate::app) fn can_zoom_out(&self) -> bool {
        self.zoom_x > ZOOM_MIN
    }

    pub(in crate::app) fn can_reset_zoom(&self) -> bool {
        (self.zoom_x - self.default_view_settings.zoom_x.clamp(ZOOM_MIN, ZOOM_MAX)).abs() > 1e-4
    }

    pub(in crate::app) fn has_multiple_files(&self) -> bool {
        self.files.len() > 1
    }

    pub(in crate::app) fn select_previous_file(&mut self) {
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

    pub(in crate::app) fn select_next_file(&mut self) {
        if self.files.is_empty() {
            self.selected_file = 0;
            return;
        }

        self.selected_file = (self.selected_file + 1) % self.files.len();
    }

    pub(in crate::app) fn track_panel_visible(&self) -> bool {
        self.track_panel_visible
    }

    pub(in crate::app) fn track_panel_width(&self) -> f32 {
        self.track_panel_width
    }

    pub(in crate::app) fn toggle_track_panel(&mut self) {
        self.track_panel_visible = !self.track_panel_visible;
    }

    pub(in crate::app) fn resize_track_panel_by(&mut self, delta: f32) {
        self.track_panel_width =
            (self.track_panel_width + delta).clamp(TRACK_PANEL_MIN_WIDTH, TRACK_PANEL_MAX_WIDTH);
    }

    pub(in crate::app) fn current_track_mix(&self) -> &[TrackMixState] {
        self.track_mix_by_file
            .get(self.selected_file)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(in crate::app) fn toggle_track_mute(&mut self, track_index: usize) -> Option<bool> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        let state = track_mix.get_mut(track_index)?;
        state.muted = !state.muted;
        Some(state.muted)
    }

    pub(in crate::app) fn toggle_track_solo(&mut self, track_index: usize) -> Option<bool> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        let state = track_mix.get_mut(track_index)?;
        state.soloed = !state.soloed;
        Some(state.soloed)
    }

    pub(in crate::app) fn set_track_muted(
        &mut self,
        track_index: usize,
        muted: bool,
    ) -> Option<()> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        track_mix.get_mut(track_index)?.muted = muted;
        Some(())
    }

    pub(in crate::app) fn set_track_soloed(
        &mut self,
        track_index: usize,
        soloed: bool,
    ) -> Option<()> {
        let track_mix = self.track_mix_by_file.get_mut(self.selected_file)?;
        track_mix.get_mut(track_index)?.soloed = soloed;
        Some(())
    }

    pub(in crate::app) fn set_global_solo_active(&mut self, active: bool) {
        self.global_solo_active = active;
    }

    pub(in crate::app) fn set_beat_subdivision(&mut self, subdivision: u8) {
        self.beat_subdivision = subdivision.clamp(BEAT_SUBDIVISION_MIN, BEAT_SUBDIVISION_MAX);
        self.beat_subdivision_input = self.beat_subdivision.to_string();
    }

    pub(in crate::app) fn set_beat_subdivision_input(&mut self, input: String) {
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

    pub(in crate::app) fn beat_subdivision_input(&self) -> &str {
        &self.beat_subdivision_input
    }

    pub(in crate::app) fn horizontal_scroll(&self) -> f32 {
        self.horizontal_scroll
    }

    pub(in crate::app) fn set_horizontal_scroll(&mut self, offset_x: f32) {
        self.horizontal_scroll = offset_x.max(0.0);
    }

    pub(in crate::app) fn vertical_scroll(&self) -> f32 {
        self.vertical_scroll
    }

    pub(in crate::app) fn set_vertical_scroll(&mut self, offset_y: f32) {
        self.vertical_scroll = offset_y.max(0.0);
    }

    pub(in crate::app) fn pending_initial_center(&self) -> bool {
        self.pending_initial_center
    }

    pub(in crate::app) fn mark_initial_center_applied(&mut self) {
        self.pending_initial_center = false;
    }

    pub(in crate::app) fn set_playback_position(
        &mut self,
        tick: u64,
        total_ticks: u64,
        is_playing: bool,
    ) {
        self.playback_total_ticks = total_ticks;
        self.playback_tick = tick.min(total_ticks);
        self.playback_is_playing = is_playing;
    }

    pub(in crate::app) fn playback_tick(&self) -> u64 {
        self.playback_tick
    }

    pub(in crate::app) fn playback_is_playing(&self) -> bool {
        self.playback_is_playing
    }

    pub(in crate::app) fn playback_position_normalized(&self) -> f32 {
        if self.playback_total_ticks == 0 {
            return 0.0;
        }

        (self.playback_tick as f32 / self.playback_total_ticks as f32).clamp(0.0, 1.0)
    }

    pub(in crate::app) fn rewind_flag_tick(&self) -> u64 {
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

    pub(in crate::app) fn set_rewind_flag_tick(&mut self, tick: u64) {
        let Some(total_ticks) = self.current_file().map(|file| file.data.total_ticks) else {
            return;
        };

        if let Some(flag_tick) = self.rewind_flag_ticks.get_mut(self.selected_file) {
            *flag_tick = tick.min(total_ticks);
        }
    }
}

pub(in crate::app) fn next_zoom_step_up(current: f32, step: f32, max_zoom: f32) -> f32 {
    next_zoom_step(current, step, max_zoom, ZoomStepDirection::Up)
}

pub(in crate::app) fn next_zoom_step_down(current: f32, step: f32, min_zoom: f32) -> f32 {
    next_zoom_step(current, step, min_zoom, ZoomStepDirection::Down)
}

#[derive(Debug, Clone, Copy)]
enum ZoomStepDirection {
    Up,
    Down,
}

fn next_zoom_step(current: f32, step: f32, limit: f32, direction: ZoomStepDirection) -> f32 {
    if step <= f32::EPSILON {
        return current;
    }

    let snapped = (current / step).round() * step;
    match direction {
        ZoomStepDirection::Up => next_zoom_step_up_from_snap(current, snapped, step, limit),
        ZoomStepDirection::Down => next_zoom_step_down_from_snap(current, snapped, step, limit),
    }
}

fn next_zoom_step_up_from_snap(current: f32, snapped: f32, step: f32, limit: f32) -> f32 {
    if (current - snapped).abs() <= 1e-4 {
        (current + step).min(limit)
    } else if current < snapped {
        snapped.min(limit)
    } else {
        (snapped + step).min(limit)
    }
}

fn next_zoom_step_down_from_snap(current: f32, snapped: f32, step: f32, limit: f32) -> f32 {
    if (current - snapped).abs() <= 1e-4 {
        (current - step).max(limit)
    } else if current > snapped {
        snapped.max(limit)
    } else {
        (snapped - step).max(limit)
    }
}

pub(in crate::app) fn drag_distance(a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

pub(in crate::app) fn roll_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new(ROLL_SCROLL_ID)
}

pub(in crate::app) fn track_list_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new(TRACK_LIST_SCROLL_ID)
}

pub(in crate::app) fn track_list_scroll_y(track_index: usize) -> f32 {
    track_index as f32 * TRACK_ROW_HEIGHT
}

pub(in crate::app) fn controls<'a>(app: &'a Lilypalooza) -> Vec<HeaderControlGroup<'a>> {
    let state = &app.piano_roll;
    let can_toggle_tracks = state
        .current_file()
        .is_some_and(|file| file.data.tracks.len() > 1);
    let mut controls = vec![
        HeaderControlGroup {
            min_width: ui_style::grid_f32(9),
            content: track_toggle_control(app, state, can_toggle_tracks),
        },
        zoom_control_group(app, state),
        beat_subdivision_control_group(state),
    ];

    if let Some(group) = file_navigation_control_group(state) {
        controls.push(group);
    }

    controls
}

pub(in crate::app) fn track_toggle_control<'a>(
    app: &'a Lilypalooza,
    state: &'a PianoRollState,
    can_toggle_tracks: bool,
) -> Element<'a, Message> {
    let track_toggle_button = button(ui_style::icon(
        icons::list_music(),
        TRACK_TOGGLE_ICON_SIZE,
        move |theme: &Theme, status| {
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
        },
    ))
    .style(if can_toggle_tracks && state.track_panel_visible() {
        ui_style::button_pane_header_control_active
    } else {
        ui_style::button_pane_header_control
    })
    .padding([ui_style::grid(1), ui_style::grid(2)])
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT));
    let track_toggle_button = if can_toggle_tracks {
        track_toggle_button.on_press(Message::PianoRoll(PianoRollMessage::TrackPanelToggle))
    } else {
        track_toggle_button
    };
    crate::app::dock_view::delayed_tooltip(
        app,
        "piano-roll-track-toggle",
        track_toggle_button.into(),
        text("Tracks").size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Top,
    )
}

pub(in crate::app) fn zoom_control_group<'a>(
    app: &'a Lilypalooza,
    state: &'a PianoRollState,
) -> HeaderControlGroup<'a> {
    let zoom_out_button = button(crate::app::dock_view::compact_control_icon(
        icons::zoom_out(),
    ))
    .style(ui_style::button_pane_header_control)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT));
    let zoom_out_button = if state.can_zoom_out() {
        zoom_out_button.on_press(Message::PianoRoll(PianoRollMessage::ZoomOut))
    } else {
        zoom_out_button
    };

    let zoom_in_button = button(crate::app::dock_view::compact_control_icon(icons::zoom_in()))
        .style(ui_style::button_pane_header_control)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ])
        .height(Length::Fixed(HEADER_CONTROL_HEIGHT));
    let zoom_in_button = if state.can_zoom_in() {
        zoom_in_button.on_press(Message::PianoRoll(PianoRollMessage::ZoomIn))
    } else {
        zoom_in_button
    };

    HeaderControlGroup {
        min_width: 132.0,
        content: row![
            zoom_out_button,
            zoom_reset_control(app, state),
            zoom_in_button,
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
    }
}

pub(in crate::app) fn zoom_reset_control<'a>(
    app: &'a Lilypalooza,
    state: &'a PianoRollState,
) -> Element<'a, Message> {
    let zoom_value = text(format!("{:.0}%", state.zoom_x * 100.0))
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(fonts::MONO);
    let zoom_value = if state.can_reset_zoom() {
        mouse_area(zoom_value).on_double_click(Message::PianoRoll(PianoRollMessage::ResetZoom))
    } else {
        mouse_area(zoom_value)
    };

    crate::app::dock_view::delayed_tooltip(
        app,
        "piano-roll-zoom-reset",
        zoom_value.into(),
        text("Double-click to reset zoom")
            .size(ui_style::FONT_SIZE_UI_XS)
            .into(),
        tooltip::Position::Top,
    )
}

pub(in crate::app) fn beat_subdivision_control_group<'a>(
    state: &'a PianoRollState,
) -> HeaderControlGroup<'a> {
    let subdivision_slider = slider(
        BEAT_SUBDIVISION_MIN..=BEAT_SUBDIVISION_MAX,
        state.beat_subdivision,
        |value| Message::PianoRoll(PianoRollMessage::BeatSubdivisionSliderChanged(value)),
    )
    .step(1u8)
    .width(Length::Fixed(120.0));

    let subdivision_input = container(
        text_input("", state.beat_subdivision_input())
            .on_input(|value| {
                Message::PianoRoll(PianoRollMessage::BeatSubdivisionInputChanged(value))
            })
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
            .size(Pixels(ui_style::FONT_SIZE_UI_XS as f32))
            .width(Length::Fixed(44.0)),
    )
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
    .center_y(Length::Fixed(HEADER_CONTROL_HEIGHT));

    HeaderControlGroup {
        min_width: 228.0,
        content: row![
            text("Beat Subdiv").size(ui_style::FONT_SIZE_UI_XS),
            subdivision_slider,
            subdivision_input,
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
    }
}

pub(in crate::app) fn file_navigation_control_group<'a>(
    state: &'a PianoRollState,
) -> Option<HeaderControlGroup<'a>> {
    state.has_multiple_files().then(|| HeaderControlGroup {
        min_width: ui_style::grid_f32(46),
        content: row![
            text("MIDI").size(ui_style::FONT_SIZE_UI_XS),
            file_navigation_button("←", PianoRollMessage::FilePrevious),
            text(current_midi_file_name(state))
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(fonts::MONO),
            file_navigation_button("→", PianoRollMessage::FileNext),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
    })
}

pub(in crate::app) fn file_navigation_button(
    label: &'static str,
    message: PianoRollMessage,
) -> iced::widget::Button<'static, Message> {
    button(text(label).size(ui_style::FONT_SIZE_UI_SM))
        .style(ui_style::button_pane_header_control)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ])
        .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
        .on_press(Message::PianoRoll(message))
}

pub(in crate::app) fn current_midi_file_name(state: &PianoRollState) -> &str {
    state
        .current_file()
        .map(|file| file.file_name.as_str())
        .unwrap_or("No MIDI")
}

pub(in crate::app) fn content(app: &Lilypalooza) -> Element<'_, Message> {
    let Some(file) = app.piano_roll.current_file() else {
        return empty_piano_roll_content(app);
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
        app,
    })
}

pub(in crate::app) fn empty_piano_roll_content(app: &Lilypalooza) -> Element<'_, Message> {
    container(empty_piano_roll_message(app))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .style(ui_style::piano_roll_surface)
        .into()
}

pub(in crate::app) fn empty_piano_roll_message(app: &Lilypalooza) -> Element<'_, Message> {
    if piano_roll_is_waiting_for_compile(app) {
        row![
            text(app.spinner_frame())
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(fonts::MONO),
            text("Compiling score to MIDI...").size(ui_style::FONT_SIZE_UI_SM),
        ]
        .spacing(ui_style::SPACE_SM)
        .align_y(alignment::Vertical::Center)
        .into()
    } else {
        text("No MIDI output yet")
            .size(ui_style::FONT_SIZE_UI_SM)
            .into()
    }
}

pub(in crate::app) fn piano_roll_is_waiting_for_compile(app: &Lilypalooza) -> bool {
    app.compile_requested || app.compile_session.is_some() || app.compile_outputs_loading
}

pub(in crate::app) struct PianoRollBody<'a> {
    pub(in crate::app) app: &'a Lilypalooza,
    pub(in crate::app) file: &'a MidiRollFile,
    pub(in crate::app) zoom_x: f32,
    pub(in crate::app) beat_subdivision: u8,
    pub(in crate::app) horizontal_offset: f32,
    pub(in crate::app) vertical_offset: f32,
    pub(in crate::app) playback_tick: u64,
    pub(in crate::app) rewind_flag_tick: u64,
    pub(in crate::app) track_mix: &'a [TrackMixState],
    pub(in crate::app) global_solo_active: bool,
    pub(in crate::app) track_panel_visible: bool,
    pub(in crate::app) track_panel_width: f32,
    pub(in crate::app) zoom_modifier_active: bool,
}

pub(in crate::app) fn piano_roll_body<'a>(body: PianoRollBody<'a>) -> Element<'a, Message> {
    let PianoRollBody {
        app,
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
    let track_colors: Vec<_> = file
        .data
        .tracks
        .iter()
        .map(|track| app.effective_track_color(track.index))
        .collect();

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
        track_colors,
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
            container(track_list(app, file, track_mix, track_panel_width))
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
