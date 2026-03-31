use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::event;
use iced::keyboard;
use iced::widget::{pane_grid, svg};
use iced::{Point, Rectangle, Size, Subscription, Task, window};
use iced_core::{Bytes, image};
use tempfile::TempDir;

use crate::error_prompt::ErrorPrompt;
use crate::lilypond;
use crate::logger::Logger;
use crate::playback::MidiPlayback;
use crate::score_watcher::ScoreWatcher;
use crate::settings::{
    self, AppSettings, DockAxis, DockNodeSettings, FoldedPaneRestoreSettings, FoldedPaneSettings,
};

use messages::{
    EditorMessage, FileMessage, LoggerMessage, Message, PaneMessage, PianoRollMessage,
    PromptMessage, ViewerMessage,
};
use piano_roll::PianoRollState;
use score_cursor::{ScoreCursorMaps, ScoreCursorPlacement};
use update::update;
use view::view;

mod dock_view;
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
const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const SCORE_SCROLLABLE_ID: &str = "score-scrollable";
pub(super) const KEYBOARD_SCROLL_STEP: f32 = 84.0;
const MIN_SVG_ZOOM: f32 = 0.4;
const MAX_SVG_ZOOM: f32 = 3.0;
const SVG_ZOOM_STEP: f32 = 0.1;
const MIN_SVG_PAGE_BRIGHTNESS: u8 = 0;
const MAX_SVG_PAGE_BRIGHTNESS: u8 = 100;
const SVG_PAGE_BRIGHTNESS_STEP: u8 = 10;
const SCORE_ZOOM_PREVIEW_INTERVAL: Duration = Duration::from_millis(16);
const SCORE_ZOOM_PREVIEW_SETTLE_DELAY: Duration = Duration::from_millis(120);
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub(super) type WorkspacePaneKind = crate::settings::WorkspacePane;

type DockGroupId = u64;

struct LilyView {
    theme: iced::Theme,
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
    dock_layout: Option<DockNode>,
    dock_groups: HashMap<DockGroupId, DockGroup>,
    next_dock_group_id: DockGroupId,
    folded_panes: Vec<FoldedPaneState>,
    hovered_workspace_pane: Option<WorkspacePaneKind>,
    pressed_workspace_pane: Option<WorkspacePaneKind>,
    workspace_drag_origin: Option<Point>,
    dragged_workspace_pane: Option<WorkspacePaneKind>,
    dock_drop_target: Option<DockDropTarget>,
    open_header_overflow_menu: Option<DockGroupId>,
    open_editor_theme_menu: Option<DockGroupId>,
    editor: editor::EditorState,
    rendered_score: Option<RenderedScore>,
    score_cursor_maps: Option<ScoreCursorMaps>,
    score_cursor_overlay: Option<ScoreCursorPlacement>,
    piano_roll: PianoRollState,
    svg_zoom: f32,
    svg_page_brightness: u8,
    svg_scroll_x: f32,
    svg_scroll_y: f32,
    score_viewport_cursor: Option<iced::Point>,
    score_zoom_last_interaction: Option<Instant>,
    score_zoom_persist_pending: bool,
    score_zoom_preview: Option<ScoreZoomPreview>,
    score_zoom_preview_pending: Option<ScoreZoomPreviewRequest>,
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
    svg_bytes: Bytes,
    size: SvgSize,
    note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Clone, Copy)]
struct SvgSize {
    width: f32,
    height: f32,
}

#[derive(Clone)]
struct ScoreZoomPreview {
    page_index: usize,
    tier: ScoreZoomPreviewTier,
    handle: image::Handle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScoreZoomPreviewRequest {
    page_index: usize,
    zoom: f32,
    tier: ScoreZoomPreviewTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreZoomPreviewTier {
    Fallback,
    Primary,
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

#[derive(Debug, Clone)]
struct DockGroup {
    tabs: Vec<WorkspacePaneKind>,
    active: WorkspacePaneKind,
}

#[derive(Debug, Clone, PartialEq)]
struct FoldedPaneState {
    pane: WorkspacePaneKind,
    restore: FoldedPaneRestore,
}

#[derive(Debug, Clone, PartialEq)]
enum FoldedPaneRestore {
    Tab {
        anchor: WorkspacePaneKind,
    },
    Standalone,
    Split {
        anchor: WorkspacePaneKind,
        axis: pane_grid::Axis,
        ratio: f32,
        insert_first: bool,
        sibling_panes: Vec<WorkspacePaneKind>,
    },
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
    .theme(|state: &LilyView| state.theme.clone())
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
    let default_settings = AppSettings::default();
    let (mut stored_settings, settings_error) = match settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (default_settings.clone(), Some(error)),
    };
    migrate_workspace_layout(
        &mut stored_settings.workspace_layout.root,
        &stored_settings.workspace_layout.folded_panes,
    );

    let (dock_layout, dock_groups, next_dock_group_id, workspace_panes) =
        build_dock_runtime(stored_settings.workspace_layout.root.as_ref());
    let mut folded_panes: Vec<_> = stored_settings
        .workspace_layout
        .folded_panes
        .iter()
        .cloned()
        .map(folded_pane_from_settings)
        .collect();
    if folded_panes.is_empty() && !stored_settings.workspace_layout.piano_visible {
        folded_panes.push(FoldedPaneState {
            pane: WorkspacePaneKind::PianoRoll,
            restore: FoldedPaneRestore::Tab {
                anchor: WorkspacePaneKind::Score,
            },
        });
    }
    let piano_roll_visible = !folded_panes
        .iter()
        .any(|folded| folded.pane == WorkspacePaneKind::PianoRoll);

    let mut piano_roll = PianoRollState::new(default_settings.piano_roll_view);
    piano_roll.visible = piano_roll_visible;
    piano_roll.apply_view_settings(
        stored_settings.piano_roll_view.zoom_x,
        stored_settings.piano_roll_view.beat_subdivision,
    );

    let mut app = LilyView {
        theme: iced::Theme::Dark,
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
        folded_panes,
        hovered_workspace_pane: None,
        pressed_workspace_pane: None,
        workspace_drag_origin: None,
        dragged_workspace_pane: None,
        dock_drop_target: None,
        open_header_overflow_menu: None,
        open_editor_theme_menu: None,
        editor: editor::EditorState::new(iced::Theme::Dark, stored_settings.editor_theme),
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
        score_zoom_last_interaction: None,
        score_zoom_persist_pending: false,
        score_zoom_preview: None,
        score_zoom_preview_pending: None,
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

    if app.compile_session.is_some() || app.score_watcher.is_some() || app.editor.has_document() {
        subscriptions.push(iced::time::every(BACKGROUND_POLL_INTERVAL).map(|_| Message::Tick));
    }

    if app.score_zoom_preview_active() || app.score_zoom_persist_pending {
        subscriptions.push(iced::time::every(SCORE_ZOOM_PREVIEW_INTERVAL).map(|_| Message::Tick));
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
            let has_primary_modifier = modifiers.command() || modifiers.control();

            if has_primary_modifier
                && matches!(
                    physical_key,
                    keyboard::key::Physical::Code(keyboard::key::Code::KeyS)
                )
            {
                return Some(Message::Editor(EditorMessage::SaveRequested));
            }

            if matches!(status, event::Status::Captured) {
                return None;
            }

            let has_zoom_modifier = has_primary_modifier;
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
                    keyboard::key::Physical::Code(keyboard::key::Code::Digit1)
                    | keyboard::key::Physical::Code(keyboard::key::Code::Numpad1) => {
                        return Some(Message::Pane(PaneMessage::ToggleWorkspacePane(
                            WorkspacePaneKind::Editor,
                        )));
                    }
                    keyboard::key::Physical::Code(keyboard::key::Code::Digit2)
                    | keyboard::key::Physical::Code(keyboard::key::Code::Numpad2) => {
                        return Some(Message::Pane(PaneMessage::ToggleWorkspacePane(
                            WorkspacePaneKind::Score,
                        )));
                    }
                    keyboard::key::Physical::Code(keyboard::key::Code::Digit3)
                    | keyboard::key::Physical::Code(keyboard::key::Code::Numpad3) => {
                        return Some(Message::Pane(PaneMessage::ToggleWorkspacePane(
                            WorkspacePaneKind::PianoRoll,
                        )));
                    }
                    keyboard::key::Physical::Code(keyboard::key::Code::Digit4)
                    | keyboard::key::Physical::Code(keyboard::key::Code::Numpad4) => {
                        return Some(Message::Pane(PaneMessage::ToggleWorkspacePane(
                            WorkspacePaneKind::Logger,
                        )));
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

    pub(super) fn is_pane_folded(&self, pane: WorkspacePaneKind) -> bool {
        self.folded_panes.iter().any(|folded| folded.pane == pane)
    }

    pub(super) fn workspace_visible_pane_count(&self) -> usize {
        self.dock_groups
            .values()
            .map(|group| group.tabs.len())
            .sum()
    }

    pub(super) fn workspace_area_size(&self) -> Size {
        Size::new(self.window_width.max(1.0), self.workspace_height())
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

    fn workspace_height(&self) -> f32 {
        let reserved_height =
            crate::status_bar::HEIGHT + transport_bar::HEIGHT + dock_view::TOOLBAR_HEIGHT;

        (self.window_height - reserved_height).max(1.0)
    }
}

fn build_dock_runtime(
    root: Option<&DockNodeSettings>,
) -> (
    Option<DockNode>,
    HashMap<DockGroupId, DockGroup>,
    DockGroupId,
    pane_grid::State<DockGroupId>,
) {
    let mut next_id = 1;
    let mut groups = HashMap::new();
    let layout = root.map(|root| dock_node_from_settings(root, &mut next_id, &mut groups));
    let workspace_panes = build_workspace_panes(layout.as_ref());

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

fn build_workspace_panes(layout: Option<&DockNode>) -> pane_grid::State<DockGroupId> {
    let Some(layout) = layout else {
        return pane_grid::State::new(0).0;
    };
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

fn folded_pane_from_settings(settings: FoldedPaneSettings) -> FoldedPaneState {
    FoldedPaneState {
        pane: settings.pane,
        restore: match settings.restore {
            FoldedPaneRestoreSettings::Tab { anchor } => FoldedPaneRestore::Tab { anchor },
            FoldedPaneRestoreSettings::Standalone => FoldedPaneRestore::Standalone,
            FoldedPaneRestoreSettings::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => FoldedPaneRestore::Split {
                anchor,
                axis: pane_grid_axis_from_settings(axis),
                ratio,
                insert_first,
                sibling_panes,
            },
        },
    }
}

fn folded_pane_to_settings(state: FoldedPaneState) -> FoldedPaneSettings {
    FoldedPaneSettings {
        pane: state.pane,
        restore: match state.restore {
            FoldedPaneRestore::Tab { anchor } => FoldedPaneRestoreSettings::Tab { anchor },
            FoldedPaneRestore::Standalone => FoldedPaneRestoreSettings::Standalone,
            FoldedPaneRestore::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => FoldedPaneRestoreSettings::Split {
                anchor,
                axis: dock_axis_to_settings(axis),
                ratio,
                insert_first,
                sibling_panes,
            },
        },
    }
}

fn migrate_workspace_layout(
    root: &mut Option<DockNodeSettings>,
    folded_panes: &[FoldedPaneSettings],
) {
    if root
        .as_ref()
        .is_some_and(|root| dock_node_settings_contains_pane(root, WorkspacePaneKind::Logger))
        || folded_panes
            .iter()
            .any(|folded| folded.pane == WorkspacePaneKind::Logger)
    {
        return;
    }

    let previous_root = root.take().unwrap_or_default();
    *root = Some(DockNodeSettings::Split {
        axis: DockAxis::Horizontal,
        ratio: 0.74,
        first: Box::new(previous_root),
        second: Box::new(DockNodeSettings::Group(settings::DockGroupSettings {
            tabs: vec![WorkspacePaneKind::Logger],
            active: WorkspacePaneKind::Logger,
        })),
    });
}

fn dock_node_settings_contains_pane(node: &DockNodeSettings, pane: WorkspacePaneKind) -> bool {
    match node {
        DockNodeSettings::Group(group) => group.tabs.contains(&pane),
        DockNodeSettings::Split { first, second, .. } => {
            dock_node_settings_contains_pane(first, pane)
                || dock_node_settings_contains_pane(second, pane)
        }
    }
}

pub(super) fn dock_axis_to_settings(axis: pane_grid::Axis) -> DockAxis {
    match axis {
        pane_grid::Axis::Horizontal => DockAxis::Horizontal,
        pane_grid::Axis::Vertical => DockAxis::Vertical,
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
