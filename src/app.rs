use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use iced::event;
use iced::keyboard;
use iced::widget::{pane_grid, svg};
use iced::{Point, Rectangle, Size, Subscription, Task, window};
use tempfile::TempDir;

use crate::error_prompt::ErrorPrompt;
use crate::lilypond;
use crate::logger::Logger;
use crate::playback::MidiPlayback;
use crate::score_watcher::ScoreWatcher;
use crate::settings::{self, AppSettings, DockAxis, DockNodeSettings};

use messages::{
    EditorMessage, FileMessage, LoggerMessage, Message, PaneMessage, PianoRollMessage,
    PromptMessage, ViewerMessage,
};
use piano_roll::PianoRollState;
use score_cursor::{ScoreCursorMaps, ScoreCursorPlacement};
use update::update;
use view::view;

mod editor;
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

pub(super) type WorkspacePaneKind = crate::settings::WorkspacePane;

type DockGroupId = u64;

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
    workspace_panes: pane_grid::State<DockGroupId>,
    dock_layout: DockNode,
    dock_groups: HashMap<DockGroupId, DockGroup>,
    next_dock_group_id: DockGroupId,
    hovered_workspace_pane: Option<WorkspacePaneKind>,
    pressed_workspace_pane: Option<WorkspacePaneKind>,
    workspace_drag_origin: Option<Point>,
    dragged_workspace_pane: Option<WorkspacePaneKind>,
    dock_drop_target: Option<DockDropTarget>,
    editor: editor::EditorState,
    rendered_score: Option<RenderedScore>,
    score_cursor_maps: Option<ScoreCursorMaps>,
    score_cursor_overlay: Option<ScoreCursorPlacement>,
    piano_roll: PianoRollState,
    piano_roll_expanded_ratio: f32,
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

#[derive(Debug, Clone)]
struct DockGroup {
    tabs: Vec<WorkspacePaneKind>,
    active: WorkspacePaneKind,
}

#[derive(Debug, Clone)]
enum DockNode {
    Group(DockGroupId),
    Split {
        axis: pane_grid::Axis,
        ratio: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DockDropRegion {
    Top,
    Right,
    Bottom,
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DockDropTarget {
    pub(super) group_id: DockGroupId,
    pub(super) region: DockDropRegion,
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

    let (dock_layout, dock_groups, next_dock_group_id, workspace_panes) =
        build_dock_runtime(&stored_settings.workspace_layout.root);
    let piano_roll_visible =
        if workspace_pane_is_tabbed_in_groups(&dock_groups, WorkspacePaneKind::PianoRoll) {
            true
        } else {
            stored_settings.workspace_layout.piano_visible
        };

    let mut piano_roll = PianoRollState::new(default_settings.piano_roll_view);
    piano_roll.visible = piano_roll_visible;
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
        workspace_panes,
        dock_layout,
        dock_groups,
        next_dock_group_id,
        hovered_workspace_pane: None,
        pressed_workspace_pane: None,
        workspace_drag_origin: None,
        dragged_workspace_pane: None,
        dock_drop_target: None,
        editor: editor::EditorState::new(),
        rendered_score: None,
        score_cursor_maps: None,
        score_cursor_overlay: None,
        piano_roll,
        piano_roll_expanded_ratio: 0.70,
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

    if app.piano_roll.visible
        && let Some(group_id) = app.group_for_pane(WorkspacePaneKind::PianoRoll)
        && let Some((_, _, ratio)) = app.parent_split_for_group(group_id)
    {
        app.piano_roll_expanded_ratio = ratio;
    }

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
                keyboard::Key::Named(keyboard::key::Named::Enter)
                    if !modifiers.command() && !modifiers.control() && !modifiers.alt() =>
                {
                    Some(Message::PianoRoll(PianoRollMessage::TransportRewind))
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
                _ => match physical_key {
                    keyboard::key::Physical::Code(keyboard::key::Code::NumpadEnter)
                        if !modifiers.command() && !modifiers.control() && !modifiers.alt() =>
                    {
                        Some(Message::PianoRoll(PianoRollMessage::TransportRewind))
                    }
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

impl LilyView {
    pub(super) fn zoom_modifier_active(&self) -> bool {
        self.keyboard_modifiers.command() || self.keyboard_modifiers.control()
    }

    pub(super) fn workspace_group(&self, group_id: DockGroupId) -> Option<&DockGroup> {
        self.dock_groups.get(&group_id)
    }

    pub(super) fn group_for_pane(&self, pane: WorkspacePaneKind) -> Option<DockGroupId> {
        self.dock_groups
            .iter()
            .find_map(|(group_id, group)| group.tabs.contains(&pane).then_some(*group_id))
    }

    pub(super) fn workspace_pane_is_tabbed(&self, pane: WorkspacePaneKind) -> bool {
        self.group_for_pane(pane)
            .and_then(|group_id| self.workspace_group(group_id))
            .is_some_and(|group| group.tabs.len() > 1)
    }

    pub(super) fn piano_roll_effectively_visible(&self) -> bool {
        !self.can_fold_piano_roll() || self.piano_roll.visible
    }

    pub(super) fn can_fold_piano_roll(&self) -> bool {
        !self.workspace_pane_is_tabbed(WorkspacePaneKind::PianoRoll)
            && self
                .group_for_pane(WorkspacePaneKind::PianoRoll)
                .and_then(|group_id| self.parent_split_for_group(group_id))
                .is_some()
    }

    pub(super) fn workspace_area_size(&self) -> Size {
        Size::new(self.window_width.max(1.0), self.score_area_height())
    }

    pub(super) fn workspace_bounds(&self) -> Rectangle {
        let size = self.workspace_area_size();
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
        }
    }

    fn score_area_height(&self) -> f32 {
        estimated_score_area_height(
            self.window_height,
            self.logger_pane.is_some(),
            self.logger_ratio,
        )
    }

    fn parent_split_for_group(
        &self,
        group_id: DockGroupId,
    ) -> Option<(pane_grid::Axis, bool, f32)> {
        parent_split_for_group(&self.dock_layout, group_id)
    }
}

fn build_dock_runtime(
    root: &DockNodeSettings,
) -> (
    DockNode,
    HashMap<DockGroupId, DockGroup>,
    DockGroupId,
    pane_grid::State<DockGroupId>,
) {
    let mut next_id = 1;
    let mut groups = HashMap::new();
    let layout = dock_node_from_settings(root, &mut next_id, &mut groups);
    let workspace_panes = build_workspace_panes(&layout);

    (layout, groups, next_id, workspace_panes)
}

fn dock_node_from_settings(
    node: &DockNodeSettings,
    next_id: &mut DockGroupId,
    groups: &mut HashMap<DockGroupId, DockGroup>,
) -> DockNode {
    match node {
        DockNodeSettings::Group(group) => {
            let group_id = *next_id;
            *next_id = next_id.saturating_add(1);
            let mut tabs = group.tabs.clone();
            if tabs.is_empty() {
                tabs.push(WorkspacePaneKind::Score);
            }
            let active = if tabs.contains(&group.active) {
                group.active
            } else {
                tabs[0]
            };
            groups.insert(group_id, DockGroup { tabs, active });
            DockNode::Group(group_id)
        }
        DockNodeSettings::Split {
            axis,
            ratio,
            first,
            second,
        } => DockNode::Split {
            axis: pane_grid_axis_from_settings(*axis),
            ratio: ratio.clamp(0.05, 0.95),
            first: Box::new(dock_node_from_settings(first, next_id, groups)),
            second: Box::new(dock_node_from_settings(second, next_id, groups)),
        },
    }
}

fn build_workspace_panes(layout: &DockNode) -> pane_grid::State<DockGroupId> {
    let configuration = configuration_from_dock_node(layout);

    match configuration {
        pane_grid::Configuration::Pane(group_id) => pane_grid::State::new(group_id).0,
        configuration => pane_grid::State::with_configuration(configuration),
    }
}

fn configuration_from_dock_node(layout: &DockNode) -> pane_grid::Configuration<DockGroupId> {
    match layout {
        DockNode::Group(group_id) => pane_grid::Configuration::Pane(*group_id),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => pane_grid::Configuration::Split {
            axis: *axis,
            ratio: *ratio,
            a: Box::new(configuration_from_dock_node(first)),
            b: Box::new(configuration_from_dock_node(second)),
        },
    }
}

fn dock_node_from_workspace_state(state: &pane_grid::State<DockGroupId>) -> Option<DockNode> {
    dock_node_from_layout_node(state, state.layout())
}

fn dock_node_from_layout_node(
    state: &pane_grid::State<DockGroupId>,
    node: &pane_grid::Node,
) -> Option<DockNode> {
    match node {
        pane_grid::Node::Pane(pane) => Some(DockNode::Group(*state.get(*pane)?)),
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => Some(DockNode::Split {
            axis: *axis,
            ratio: *ratio,
            first: Box::new(dock_node_from_layout_node(state, a.as_ref())?),
            second: Box::new(dock_node_from_layout_node(state, b.as_ref())?),
        }),
    }
}

fn pane_grid_axis_from_settings(axis: DockAxis) -> pane_grid::Axis {
    match axis {
        DockAxis::Horizontal => pane_grid::Axis::Horizontal,
        DockAxis::Vertical => pane_grid::Axis::Vertical,
    }
}

pub(super) fn dock_axis_to_settings(axis: pane_grid::Axis) -> DockAxis {
    match axis {
        pane_grid::Axis::Horizontal => DockAxis::Horizontal,
        pane_grid::Axis::Vertical => DockAxis::Vertical,
    }
}

fn workspace_pane_is_tabbed_in_groups(
    groups: &HashMap<DockGroupId, DockGroup>,
    pane: WorkspacePaneKind,
) -> bool {
    groups
        .values()
        .any(|group| group.tabs.contains(&pane) && group.tabs.len() > 1)
}

fn parent_split_for_group(
    node: &DockNode,
    group_id: DockGroupId,
) -> Option<(pane_grid::Axis, bool, f32)> {
    match node {
        DockNode::Group(_) => None,
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            if contains_group(first, group_id) {
                Some((*axis, true, *ratio))
            } else if contains_group(second, group_id) {
                Some((*axis, false, *ratio))
            } else {
                parent_split_for_group(first, group_id)
                    .or_else(|| parent_split_for_group(second, group_id))
            }
        }
    }
}

fn contains_group(node: &DockNode, group_id: DockGroupId) -> bool {
    match node {
        DockNode::Group(candidate) => *candidate == group_id,
        DockNode::Split { first, second, .. } => {
            contains_group(first, group_id) || contains_group(second, group_id)
        }
    }
}

fn constrained_logger_ratio(window_height: f32, requested_ratio: f32) -> f32 {
    if window_height <= 0.0 {
        return requested_ratio.clamp(0.05, 0.95);
    }

    let max_ratio_for_min_logger = 1.0 - (MIN_LOGGER_PANEL_HEIGHT / window_height);
    requested_ratio.clamp(0.05, max_ratio_for_min_logger.clamp(0.05, 0.95))
}

pub(super) fn constrained_piano_ratio(score_area_height: f32, requested_ratio: f32) -> f32 {
    constrained_bottom_panel_ratio(score_area_height, requested_ratio, MIN_LOGGER_PANEL_HEIGHT)
}

pub(super) fn collapsed_piano_ratio(score_area_height: f32) -> f32 {
    constrained_bottom_panel_ratio(
        score_area_height,
        1.0 - (PIANO_COLLAPSED_PANEL_HEIGHT / score_area_height.max(1.0)),
        PIANO_COLLAPSED_PANEL_HEIGHT,
    )
}

pub(super) fn collapsed_first_pane_ratio(available_extent: f32) -> f32 {
    constrained_top_panel_ratio(
        available_extent,
        PIANO_COLLAPSED_PANEL_HEIGHT / available_extent.max(1.0),
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
