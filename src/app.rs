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
const PIANO_DEFAULT_SPLIT_RATIO: f32 = 0.70;
const PIANO_COLLAPSED_PANEL_HEIGHT: f32 = piano_roll::COLLAPSED_HEIGHT;
const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const SCORE_SCROLLABLE_ID: &str = "score-scrollable";
pub(super) const KEYBOARD_SCROLL_STEP: f32 = 84.0;
const DEFAULT_SVG_ZOOM: f32 = 0.7;
const MIN_SVG_ZOOM: f32 = 0.4;
const MAX_SVG_ZOOM: f32 = 3.0;
const SVG_ZOOM_STEP: f32 = 0.1;
const MIN_SVG_PAGE_BRIGHTNESS: u8 = 0;
const MAX_SVG_PAGE_BRIGHTNESS: u8 = 100;
const DEFAULT_SVG_PAGE_BRIGHTNESS: u8 = 70;
const SVG_PAGE_BRIGHTNESS_STEP: u8 = 10;
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

struct LilyView {
    panes: pane_grid::State<PaneKind>,
    main_pane: pane_grid::Pane,
    logger_pane: Option<pane_grid::Pane>,
    logger_split: Option<pane_grid::Split>,
    logger_ratio: f32,
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
    piano_ratio: f32,
    piano_expanded_ratio: f32,
    rendered_score: Option<RenderedScore>,
    score_cursor_maps: Option<ScoreCursorMaps>,
    score_cursor_overlay: Option<ScoreCursorPlacement>,
    piano_roll: PianoRollState,
    svg_zoom: f32,
    svg_page_brightness: u8,
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

#[derive(Debug, Clone, Copy)]
enum ScorePaneKind {
    Score,
    PianoRoll,
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
    let (mut score_panes, score_pane) = pane_grid::State::new(ScorePaneKind::Score);
    let (_piano_pane, score_split) = score_panes
        .split(
            pane_grid::Axis::Horizontal,
            score_pane,
            ScorePaneKind::PianoRoll,
        )
        .expect("score pane split must initialize");
    let score_height = estimated_score_area_height(MIN_WINDOW_HEIGHT, false, logger_ratio);
    let piano_ratio = constrained_piano_ratio(score_height, PIANO_DEFAULT_SPLIT_RATIO);
    score_panes.resize(score_split, piano_ratio);

    let mut app = LilyView {
        panes,
        main_pane,
        logger_pane: None,
        logger_split: None,
        logger_ratio,
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
        piano_ratio,
        piano_expanded_ratio: piano_ratio,
        rendered_score: None,
        score_cursor_maps: None,
        score_cursor_overlay: None,
        piano_roll: PianoRollState::new(),
        svg_zoom: DEFAULT_SVG_ZOOM,
        svg_page_brightness: DEFAULT_SVG_PAGE_BRIGHTNESS,
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

fn estimated_score_area_height(window_height: f32, logger_visible: bool, logger_ratio: f32) -> f32 {
    let mut height = (window_height - crate::status_bar::HEIGHT).max(1.0);

    if logger_visible {
        height *= logger_ratio;
    }

    height.max(1.0)
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
