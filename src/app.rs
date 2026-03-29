use std::path::PathBuf;
use std::time::Duration;

use iced::event;
use iced::keyboard;
use iced::widget::{pane_grid, svg};
use iced::{Size, Subscription, Task, window};
use tempfile::TempDir;

use crate::error_prompt::ErrorPrompt;
use crate::lilypond;
use crate::logger::Logger;
use crate::playback::MidiPlayback;
use crate::score_watcher::ScoreWatcher;
use crate::settings::{
    self, ActiveScorePane, AppSettings, PaneAxis, PaneOrder, ScoreLayoutSettings,
};

use messages::{
    FileMessage, LoggerMessage, Message, PaneMessage, PianoRollMessage, PromptMessage,
    ViewerMessage,
};
use piano_roll::PianoRollState;
use score_cursor::{ScoreCursorMaps, ScoreCursorPlacement};
use update::update;
use view::view;

mod messages;
mod piano_roll;
mod score_cursor;
mod score_view;
mod transport_bar;
mod update;
mod view;

const MIN_WINDOW_WIDTH: f32 = 960.0;
const MIN_WINDOW_HEIGHT: f32 = 640.0;
const LOGGER_DEFAULT_SPLIT_RATIO: f32 = 0.74;
const MIN_LOGGER_PANEL_HEIGHT: f32 = 140.0;
const PIANO_COLLAPSED_PANEL_HEIGHT: f32 = piano_roll::COLLAPSED_HEIGHT;
const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const SCORE_SCROLLABLE_ID: &str = "score-scrollable";
pub(super) const KEYBOARD_SCROLL_STEP: f32 = 84.0;
const MIN_SVG_ZOOM: f32 = 0.4;
const MAX_SVG_ZOOM: f32 = 3.0;
const SVG_ZOOM_STEP: f32 = 0.1;
const MIN_SVG_PAGE_BRIGHTNESS: u8 = 0;
const MAX_SVG_PAGE_BRIGHTNESS: u8 = 100;
const SVG_PAGE_BRIGHTNESS_STEP: u8 = 10;
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

struct LilyView {
    panes: pane_grid::State<PaneKind>,
    main_pane: pane_grid::Pane,
    logger_pane: Option<pane_grid::Pane>,
    logger_split: Option<pane_grid::Split>,
    logger_ratio: f32,
    window_width: f32,
    window_height: f32,
    lilypond_status: LilypondStatus,
    current_score: Option<SelectedScore>,
    error_prompt: Option<ErrorPrompt>,
    prompt_ok_action: Option<PromptOkAction>,
    logger: Logger,
    score_watcher: Option<ScoreWatcher>,
    build_dir: Option<TempDir>,
    compile_requested: bool,
    spinner_step: usize,
    compile_session: Option<lilypond::CompileSession>,
    playback: Option<MidiPlayback>,
    soundfont_status: SoundfontStatus,
    score_panes: pane_grid::State<ScorePaneKind>,
    score_split: pane_grid::Split,
    score_split_axis: pane_grid::Axis,
    score_layout_axis: PaneAxis,
    score_pane_order: PaneOrder,
    stacked_active_pane: ScorePaneKind,
    stacked_hovered_pane: Option<ScorePaneKind>,
    stacked_pressed_pane: Option<ScorePaneKind>,
    stacked_dragging_pane: Option<ScorePaneKind>,
    stacked_drop_target: Option<StackedDropTarget>,
    piano_ratio: f32,
    piano_expanded_ratio: f32,
    rendered_score: Option<RenderedScore>,
    score_cursor_maps: Option<ScoreCursorMaps>,
    score_cursor_overlay: Option<ScoreCursorPlacement>,
    piano_roll: PianoRollState,
    svg_zoom: f32,
    svg_page_brightness: u8,
    svg_scroll_x: f32,
    svg_scroll_y: f32,
    score_viewport_cursor: Option<iced::Point>,
    piano_roll_viewport_cursor: Option<iced::Point>,
    keyboard_modifiers: keyboard::Modifiers,
    default_settings: AppSettings,
}

struct SelectedScore {
    path: PathBuf,
    file_name: String,
}

struct RenderedScore {
    pages: Vec<RenderedPage>,
    current_page: usize,
}

struct RenderedPage {
    handle: svg::Handle,
    size: SvgSize,
    note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Clone, Copy)]
struct SvgSize {
    width: f32,
    height: f32,
}

impl RenderedScore {
    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn current_page_number(&self) -> usize {
        self.current_page.saturating_add(1)
    }

    fn current_page(&self) -> Option<&RenderedPage> {
        self.pages.get(self.current_page)
    }
}

#[derive(Debug, Clone, Copy)]
enum PaneKind {
    Main,
    Logger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScorePaneKind {
    Score,
    PianoRoll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackedDropTarget {
    Top,
    Right,
    Bottom,
    Left,
}

enum LilypondStatus {
    Checking,
    Ready {
        detected: lilypond::Version,
        min_required: lilypond::Version,
    },
    Unavailable,
}

enum SoundfontStatus {
    NotSelected,
    Ready(PathBuf),
    Error(String),
}

#[derive(Debug, Clone, Copy)]
enum PromptOkAction {
    ExitApp,
    ClearLogs,
}

pub fn run(startup_soundfont: Option<PathBuf>, startup_score: Option<PathBuf>) -> iced::Result {
    iced::application(
        move || new(startup_soundfont.clone(), startup_score.clone()),
        update,
        view,
    )
    .title("lily-view")
    .window(window::Settings {
        min_size: Some(Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
        ..window::Settings::default()
    })
    .subscription(subscription)
    .run()
}

fn new(
    startup_soundfont: Option<PathBuf>,
    startup_score: Option<PathBuf>,
) -> (LilyView, Task<Message>) {
    let (panes, main_pane) = pane_grid::State::new(PaneKind::Main);
    let logger_ratio = constrained_logger_ratio(MIN_WINDOW_HEIGHT, LOGGER_DEFAULT_SPLIT_RATIO);
    let default_settings = AppSettings::default();
    let (stored_settings, settings_error) = match settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (default_settings.clone(), Some(error)),
    };
    let stored_layout = stored_settings.score_layout;
    let score_height = estimated_score_area_height(MIN_WINDOW_HEIGHT, false, logger_ratio);
    let (score_panes, score_split, score_split_axis, piano_ratio, piano_expanded_ratio) =
        build_score_panes(stored_layout, score_height);
    let mut piano_roll = PianoRollState::new(default_settings.piano_roll_view);
    piano_roll.visible = if stored_layout.pane_axis == PaneAxis::Stacked {
        true
    } else {
        stored_layout.piano_visible
    };
    piano_roll.apply_view_settings(
        stored_settings.piano_roll_view.zoom_x,
        stored_settings.piano_roll_view.beat_subdivision,
    );

    let mut app = LilyView {
        panes,
        main_pane,
        logger_pane: None,
        logger_split: None,
        logger_ratio,
        window_width: MIN_WINDOW_WIDTH,
        window_height: MIN_WINDOW_HEIGHT,
        lilypond_status: LilypondStatus::Checking,
        current_score: None,
        error_prompt: None,
        prompt_ok_action: None,
        logger: Logger::new(),
        score_watcher: None,
        build_dir: None,
        compile_requested: false,
        spinner_step: 0,
        compile_session: None,
        playback: None,
        soundfont_status: SoundfontStatus::NotSelected,
        score_panes,
        score_split,
        score_split_axis,
        score_layout_axis: stored_layout.pane_axis,
        score_pane_order: stored_layout.pane_order,
        stacked_active_pane: score_pane_kind_from_settings(stored_layout.active_pane),
        stacked_hovered_pane: None,
        stacked_pressed_pane: None,
        stacked_dragging_pane: None,
        stacked_drop_target: None,
        piano_ratio,
        piano_expanded_ratio,
        rendered_score: None,
        score_cursor_maps: None,
        score_cursor_overlay: None,
        piano_roll,
        svg_zoom: stored_settings
            .score_view
            .zoom
            .clamp(MIN_SVG_ZOOM, MAX_SVG_ZOOM),
        svg_page_brightness: stored_settings
            .score_view
            .page_brightness
            .clamp(MIN_SVG_PAGE_BRIGHTNESS, MAX_SVG_PAGE_BRIGHTNESS),
        svg_scroll_x: 0.0,
        svg_scroll_y: 0.0,
        score_viewport_cursor: None,
        piano_roll_viewport_cursor: None,
        keyboard_modifiers: keyboard::Modifiers::default(),
        default_settings,
    };

    app.logger.push("Checking LilyPond availability");
    if let Some(path) = startup_soundfont.as_ref() {
        app.logger
            .push(format!("Startup soundfont requested: {}", path.display()));
    }
    if let Some(path) = startup_score.as_ref() {
        app.logger
            .push(format!("Startup score requested: {}", path.display()));
    }
    if let Some(error) = settings_error {
        app.logger.push(format!("Settings load failed: {error}"));
    }

    let mut startup_tasks = vec![Task::perform(
        async { lilypond::check_lilypond().map_err(|error| error.to_string()) },
        Message::StartupChecked,
    )];

    if let Some(path) = startup_soundfont {
        startup_tasks.push(Task::done(Message::File(FileMessage::SoundfontPicked(
            Some(path),
        ))));
    }
    if let Some(path) = startup_score {
        startup_tasks.push(Task::done(Message::File(FileMessage::Picked(Some(path)))));
    }

    (app, Task::batch(startup_tasks))
}

fn subscription(app: &LilyView) -> Subscription<Message> {
    let mut subscriptions = vec![
        window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
        event::listen_with(runtime_event_to_message),
    ];

    if app.compile_session.is_some() || app.score_watcher.is_some() {
        subscriptions.push(iced::time::every(BACKGROUND_POLL_INTERVAL).map(|_| Message::Tick));
    }

    if app.playback.as_ref().is_some_and(MidiPlayback::is_playing) {
        subscriptions.push(window::frames().map(Message::Frame));
    }

    Subscription::batch(subscriptions)
}

fn runtime_event_to_message(
    event: iced::Event,
    status: event::Status,
    _window_id: window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key,
            modifiers,
            ..
        }) => {
            if matches!(status, event::Status::Captured) {
                return None;
            }

            let has_zoom_modifier = modifiers.command() || modifiers.control();
            if has_zoom_modifier {
                match modified_key.as_ref() {
                    keyboard::Key::Character("+") | keyboard::Key::Character("=") => {
                        return Some(Message::Viewer(ViewerMessage::ZoomIn));
                    }
                    keyboard::Key::Character("-") | keyboard::Key::Character("_") => {
                        return Some(Message::Viewer(ViewerMessage::ZoomOut));
                    }
                    keyboard::Key::Character("0") => {
                        return Some(Message::Viewer(ViewerMessage::ResetZoom));
                    }
                    _ => {}
                }

                match physical_key {
                    keyboard::key::Physical::Code(keyboard::key::Code::NumpadAdd) => {
                        return Some(Message::Viewer(ViewerMessage::ZoomIn));
                    }
                    keyboard::key::Physical::Code(keyboard::key::Code::NumpadSubtract) => {
                        return Some(Message::Viewer(ViewerMessage::ZoomOut));
                    }
                    keyboard::key::Physical::Code(keyboard::key::Code::Numpad0) => {
                        return Some(Message::Viewer(ViewerMessage::ResetZoom));
                    }
                    _ => {}
                }
            }

            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Space)
                    if !modifiers.command() && !modifiers.control() && !modifiers.alt() =>
                {
                    Some(Message::PianoRoll(PianoRollMessage::TransportPlayPause))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                    Some(Message::Viewer(ViewerMessage::ScrollUp))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                    Some(Message::Viewer(ViewerMessage::ScrollDown))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                    Some(Message::Viewer(ViewerMessage::PrevPage))
                }
                keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                    Some(Message::Viewer(ViewerMessage::NextPage))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

impl LilyView {
    pub(super) fn zoom_modifier_active(&self) -> bool {
        self.keyboard_modifiers.command() || self.keyboard_modifiers.control()
    }
}

fn build_score_panes(
    layout: ScoreLayoutSettings,
    score_area_height: f32,
) -> (
    pane_grid::State<ScorePaneKind>,
    pane_grid::Split,
    pane_grid::Axis,
    f32,
    f32,
) {
    let split_axis = match layout.pane_axis {
        PaneAxis::Horizontal => pane_grid::Axis::Horizontal,
        PaneAxis::Vertical => pane_grid::Axis::Vertical,
        PaneAxis::Stacked => pane_grid::Axis::Horizontal,
    };
    let available_extent = score_extent_for_axis(MIN_WINDOW_WIDTH, score_area_height, split_axis);
    let piano_expanded_ratio =
        constrained_piano_ratio(available_extent, layout.piano_expanded_ratio);
    let piano_ratio = if layout.piano_visible {
        piano_expanded_ratio
    } else {
        collapsed_piano_ratio_for_layout(available_extent, layout.pane_order)
    };

    let configuration = match layout.pane_order {
        PaneOrder::ScoreFirst => pane_grid::Configuration::Split {
            axis: split_axis,
            ratio: piano_ratio,
            a: Box::new(pane_grid::Configuration::Pane(ScorePaneKind::Score)),
            b: Box::new(pane_grid::Configuration::Pane(ScorePaneKind::PianoRoll)),
        },
        PaneOrder::PianoFirst => pane_grid::Configuration::Split {
            axis: split_axis,
            ratio: piano_ratio,
            a: Box::new(pane_grid::Configuration::Pane(ScorePaneKind::PianoRoll)),
            b: Box::new(pane_grid::Configuration::Pane(ScorePaneKind::Score)),
        },
    };

    let score_panes = pane_grid::State::with_configuration(configuration);
    let score_split = *score_panes
        .layout()
        .splits()
        .next()
        .expect("score pane split must initialize");

    (
        score_panes,
        score_split,
        split_axis,
        piano_ratio,
        piano_expanded_ratio,
    )
}

fn score_pane_kind_from_settings(pane: ActiveScorePane) -> ScorePaneKind {
    match pane {
        ActiveScorePane::Score => ScorePaneKind::Score,
        ActiveScorePane::PianoRoll => ScorePaneKind::PianoRoll,
    }
}

fn score_pane_kind_to_settings(pane: ScorePaneKind) -> ActiveScorePane {
    match pane {
        ScorePaneKind::Score => ActiveScorePane::Score,
        ScorePaneKind::PianoRoll => ActiveScorePane::PianoRoll,
    }
}

fn pane_order_from_first(first: ScorePaneKind) -> PaneOrder {
    match first {
        ScorePaneKind::Score => PaneOrder::ScoreFirst,
        ScorePaneKind::PianoRoll => PaneOrder::PianoFirst,
    }
}

fn pane_order_for_split(dragged: ScorePaneKind, dragged_first: bool) -> PaneOrder {
    match (dragged, dragged_first) {
        (ScorePaneKind::Score, true) | (ScorePaneKind::PianoRoll, false) => PaneOrder::ScoreFirst,
        (ScorePaneKind::PianoRoll, true) | (ScorePaneKind::Score, false) => PaneOrder::PianoFirst,
    }
}

fn constrained_logger_ratio(window_height: f32, requested_ratio: f32) -> f32 {
    if window_height <= 0.0 {
        return requested_ratio.clamp(0.05, 0.95);
    }

    let max_ratio_for_min_logger = 1.0 - (MIN_LOGGER_PANEL_HEIGHT / window_height);
    requested_ratio.clamp(0.05, max_ratio_for_min_logger.clamp(0.05, 0.95))
}

fn constrained_piano_ratio(score_area_height: f32, requested_ratio: f32) -> f32 {
    constrained_bottom_panel_ratio(score_area_height, requested_ratio, MIN_LOGGER_PANEL_HEIGHT)
}

fn collapsed_piano_ratio(score_area_height: f32) -> f32 {
    constrained_bottom_panel_ratio(
        score_area_height,
        1.0 - (PIANO_COLLAPSED_PANEL_HEIGHT / score_area_height.max(1.0)),
        PIANO_COLLAPSED_PANEL_HEIGHT,
    )
}

fn collapsed_first_pane_ratio(available_extent: f32) -> f32 {
    constrained_top_panel_ratio(
        available_extent,
        PIANO_COLLAPSED_PANEL_HEIGHT / available_extent.max(1.0),
        PIANO_COLLAPSED_PANEL_HEIGHT,
    )
}

fn collapsed_piano_ratio_for_layout(available_extent: f32, order: PaneOrder) -> f32 {
    match order {
        PaneOrder::ScoreFirst => collapsed_piano_ratio(available_extent),
        PaneOrder::PianoFirst => collapsed_first_pane_ratio(available_extent),
    }
}

fn constrained_bottom_panel_ratio(
    available_height: f32,
    requested_ratio: f32,
    min_bottom_height: f32,
) -> f32 {
    if available_height <= 0.0 {
        return requested_ratio.clamp(0.05, 0.95);
    }

    let max_ratio = 1.0 - (min_bottom_height / available_height);
    requested_ratio.clamp(0.05, max_ratio.clamp(0.05, 0.95))
}

fn constrained_top_panel_ratio(
    available_extent: f32,
    requested_ratio: f32,
    min_top_extent: f32,
) -> f32 {
    if available_extent <= 0.0 {
        return requested_ratio.clamp(0.05, 0.95);
    }

    let min_ratio = min_top_extent / available_extent;
    requested_ratio.clamp(min_ratio.clamp(0.05, 0.95), 0.95)
}

fn estimated_score_area_height(window_height: f32, logger_visible: bool, logger_ratio: f32) -> f32 {
    let mut height = (window_height - crate::status_bar::HEIGHT).max(1.0);

    if logger_visible {
        height *= logger_ratio;
    }

    height.max(1.0)
}

fn score_extent_for_axis(window_width: f32, score_area_height: f32, axis: pane_grid::Axis) -> f32 {
    match axis {
        pane_grid::Axis::Horizontal => score_area_height,
        pane_grid::Axis::Vertical => window_width.max(1.0),
    }
}

fn selected_score_from_path(path: PathBuf) -> Result<SelectedScore, String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| "Selected path has no file name".to_string())?
        .to_str()
        .ok_or_else(|| "Selected file name is not valid UTF-8".to_string())?
        .to_string();

    Ok(SelectedScore { path, file_name })
}

fn selected_score_stem(selected_file_name: &str) -> Result<&str, String> {
    std::path::Path::new(selected_file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "Selected score name has no valid stem".to_string())
}
