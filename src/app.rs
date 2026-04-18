use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::event;
use iced::keyboard;
use iced::widget::{Id, pane_grid, svg};
use iced::{Point, Rectangle, Size, Subscription, Task, mouse, window};
use iced_core::{Bytes, image};
use lilypalooza_audio::MixerState;
use lilypalooza_audio::{AudioEngine, AudioEngineOptions};
use tempfile::TempDir;

use crate::browser_file_watcher::BrowserFileWatcher;
use crate::editor_file_watcher::EditorFileWatcher;
use crate::error_prompt::{ErrorPrompt, PromptSelectedButton};
use crate::lilypond;
use crate::logger::Logger;
use crate::score_watcher::ScoreWatcher;
use crate::settings::{
    self, DockAxis, DockNodeSettings, FoldedPaneRestoreSettings, FoldedPaneSettings,
};
use crate::state::{self, GlobalState};

use messages::{
    EditorMessage, FileMessage, KeyPress, LoggerMessage, Message, PaneMessage, PianoRollMessage,
    PromptMessage, ViewerMessage,
};
use piano_roll::PianoRollState;
use score_cursor::{ScoreCursorMaps, ScoreCursorPlacement};
use update::update;
use view::view;

mod controls;
mod dock_view;
mod editor;
mod messages;
mod meters;
mod mixer;
mod piano_roll;
mod score_cursor;
mod score_view;
mod transport_bar;
mod update;
mod view;

const MIN_WINDOW_WIDTH: f32 = 960.0;
const MIN_WINDOW_HEIGHT: f32 = 640.0;
const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_millis(120);
const EDITOR_TABBAR_AUTOSCROLL_INTERVAL: Duration = Duration::from_millis(16);
pub(super) const SCORE_SCROLLABLE_ID: &str = "score-scrollable";
pub(super) const EDITOR_TABBAR_SCROLL_ID: &str = "editor-tabbar-scroll";
pub(super) const EDITOR_FILE_BROWSER_SCROLL_ID: &str = "editor-file-browser-scroll";
pub(super) const EDITOR_FILE_BROWSER_HEIGHT: f32 = 176.0;
pub(super) const EDITOR_FILE_BROWSER_COLUMN_WIDTH: f32 = 220.0;
pub(super) const EDITOR_FILE_BROWSER_ENTRY_HEIGHT: f32 = 26.0;
pub(super) const SHORTCUTS_SCROLLABLE_ID: &str = "shortcuts-scrollable";
pub(super) const KEYBOARD_SCROLL_STEP: f32 = 84.0;
pub(super) const SHORTCUTS_ACTION_ROW_HEIGHT: f32 = 48.0;
const MIN_SVG_ZOOM: f32 = 0.4;
const MAX_SVG_ZOOM: f32 = 3.0;
const SVG_ZOOM_STEP: f32 = 0.1;
const MIN_SVG_PAGE_BRIGHTNESS: u8 = 0;
const MAX_SVG_PAGE_BRIGHTNESS: u8 = 100;
const SVG_PAGE_BRIGHTNESS_STEP: u8 = 10;
const SCORE_ZOOM_PREVIEW_INTERVAL: Duration = Duration::from_millis(16);
const SCORE_ZOOM_PREVIEW_SETTLE_DELAY: Duration = Duration::from_millis(120);
const PLAYBACK_POLL_INTERVAL: Duration = Duration::from_millis(33);
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub(super) fn editor_file_browser_column_scroll_id(index: usize) -> Id {
    Id::new(Box::leak(
        format!("editor-file-browser-column-{index}").into_boxed_str(),
    ))
}

pub(super) type WorkspacePaneKind = crate::settings::WorkspacePane;

type DockGroupId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorHeaderMenuSection {
    File,
    Edit,
    Appearance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorFileMenuSection {
    OpenRecent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectMenuSection {
    Project,
    View,
}

struct Lilypalooza {
    theme: iced::Theme,
    window_width: f32,
    window_height: f32,
    lilypond_status: LilypondStatus,
    current_score: Option<SelectedScore>,
    error_prompt: Option<ErrorPrompt>,
    prompt_ok_action: Option<PromptOkAction>,
    prompt_selected_button: PromptSelectedButton,
    logger: Logger,
    score_watcher: Option<ScoreWatcher>,
    editor_file_watcher: Option<EditorFileWatcher>,
    browser_file_watcher: Option<BrowserFileWatcher>,
    browser_history_dir: Option<TempDir>,
    browser_history_next_stash_id: u64,
    browser_undo_stack: Vec<BrowserHistoryEntry>,
    browser_redo_stack: Vec<BrowserHistoryEntry>,
    mixer_undo_stack: Vec<MixerState>,
    mixer_redo_stack: Vec<MixerState>,
    pending_mixer_undo_snapshot: Option<MixerState>,
    build_dir: Option<TempDir>,
    compile_requested: bool,
    compile_outputs_loading: bool,
    compile_generation: u64,
    spinner_step: usize,
    compile_session: Option<lilypond::CompileSession>,
    playback: Option<AudioEngine>,
    soundfont_status: SoundfontStatus,
    workspace_panes: pane_grid::State<DockGroupId>,
    dock_layout: Option<DockNode>,
    dock_groups: HashMap<DockGroupId, DockGroup>,
    next_dock_group_id: DockGroupId,
    folded_panes: Vec<FoldedPaneState>,
    focused_workspace_pane: Option<WorkspacePaneKind>,
    hovered_workspace_pane: Option<WorkspacePaneKind>,
    pressed_workspace_pane: Option<WorkspacePaneKind>,
    workspace_drag_origin: Option<Point>,
    dragged_workspace_pane: Option<WorkspacePaneKind>,
    dock_drop_target: Option<DockDropTarget>,
    open_header_overflow_menu: Option<DockGroupId>,
    open_editor_menu_section: Option<EditorHeaderMenuSection>,
    open_editor_file_menu_section: Option<EditorFileMenuSection>,
    hovered_editor_file_menu_section: Option<EditorFileMenuSection>,
    open_project_menu: bool,
    open_project_menu_section: Option<ProjectMenuSection>,
    open_project_recent: bool,
    open_shortcuts_dialog: bool,
    shortcuts_search_query: String,
    shortcuts_search_input_id: Id,
    shortcuts_selected_action: Option<settings::ShortcutActionId>,
    hovered_tooltip_key: Option<String>,
    open_tooltip_key: Option<String>,
    pressed_editor_tab: Option<u64>,
    hovered_editor_tab: Option<u64>,
    dragged_editor_tab: Option<u64>,
    editor_tab_drag_origin: Option<Point>,
    editor_tab_drop_after: bool,
    editor_file_browser_focused: bool,
    editor_file_browser_scroll_x: f32,
    editor_file_browser_viewport_width: f32,
    editor_file_browser_column_scroll_y: HashMap<usize, f32>,
    editor_file_browser_column_viewport_height: HashMap<usize, f32>,
    editor_file_browser_cursor: Option<Point>,
    browser_clipboard: Option<BrowserClipboard>,
    browser_inline_edit: Option<BrowserInlineEdit>,
    browser_inline_edit_value: String,
    browser_inline_edit_input_id: Id,
    browser_pressed_entry: Option<BrowserPressedEntry>,
    browser_drag_state: Option<BrowserDragState>,
    browser_drop_target: Option<BrowserDropTarget>,
    editor_tabbar_scroll_x: f32,
    editor_tabbar_viewport_width: f32,
    editor_tabbar_autoscroll_direction: i8,
    editor_tabbar_drag_pointer_x: Option<f32>,
    pending_reveal_editor_tab: Option<u64>,
    renaming_editor_tab: Option<u64>,
    editor_tab_rename_value: String,
    editor_tab_rename_input_id: Id,
    editor_recent_files: Vec<PathBuf>,
    recent_projects: Vec<PathBuf>,
    editor_recent_files_limit: usize,
    editor: editor::EditorState,
    editor_font_metrics_refresh_pending: bool,
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
    transport_seek_preview: Option<f32>,
    keyboard_modifiers: keyboard::Modifiers,
    primary_mouse_pressed: bool,
    shortcut_settings: settings::ShortcutSettings,
    project_root: Option<PathBuf>,
    project_name: Option<String>,
    pending_editor_action: Option<PendingEditorAction>,
    pending_editor_save_as_tab: Option<u64>,
    pending_editor_rename_tab: Option<u64>,
    default_global_state: GlobalState,
    #[cfg(target_os = "macos")]
    macos_quit_menu_patched: bool,
}

struct SelectedScore {
    path: PathBuf,
    file_name: String,
}

struct RenderedScore {
    pages: Vec<RenderedPage>,
    current_page: usize,
}

#[derive(Debug, Clone)]
struct LoadedCompileOutputs {
    score_path: PathBuf,
    rendered_pages: Vec<LoadedRenderedPage>,
    midi_files: Vec<crate::midi::MidiRollFile>,
    score_cursor_maps: Option<ScoreCursorMaps>,
    point_and_click_disabled: bool,
    score_has_repeats: bool,
}

struct RenderedPage {
    handle: svg::Handle,
    svg_bytes: Bytes,
    size: SvgSize,
    note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Debug, Clone)]
struct LoadedRenderedPage {
    svg_bytes: Vec<u8>,
    size: SvgSize,
    note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
enum PromptOkAction {
    ExitApp,
    ClearLogs,
    ReloadEditorTab(u64),
    DeleteBrowserPath(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserInlineEditKind {
    Rename,
    NewFile,
    NewDirectory,
}

#[derive(Debug, Clone)]
enum BrowserHistoryEntry {
    Create {
        path: PathBuf,
        stash_path: Option<PathBuf>,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Delete {
        path: PathBuf,
        stash_path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserClipboardKind {
    Cut,
    Copy,
}

#[derive(Debug, Clone)]
struct BrowserClipboard {
    path: PathBuf,
    kind: BrowserClipboardKind,
}

#[derive(Debug, Clone)]
struct BrowserInlineEdit {
    column_index: usize,
    parent_dir: PathBuf,
    target_path: Option<PathBuf>,
    kind: BrowserInlineEditKind,
}

#[derive(Debug, Clone)]
struct BrowserPressedEntry {
    path: PathBuf,
    is_dir: bool,
    origin: Point,
}

#[derive(Debug, Clone)]
struct BrowserDragState {
    source_path: PathBuf,
    source_is_dir: bool,
}

#[derive(Debug, Clone)]
struct BrowserDropTarget {
    column_index: usize,
    path: PathBuf,
    target_dir: PathBuf,
}

#[derive(Debug, Clone)]
enum PendingEditorAction {
    ResolveDirtyTabs {
        dirty_tab_ids: Vec<u64>,
        continuation: EditorContinuation,
    },
}

#[derive(Debug, Clone)]
enum EditorContinuation {
    CloseTab(u64),
    LoadProject(PathBuf),
    OpenScore(PathBuf),
    ExitApp,
}

pub fn run(startup_soundfont: Option<PathBuf>, startup_score: Option<PathBuf>) -> iced::Result {
    iced::application(
        move || new(startup_soundfont.clone(), startup_score.clone()),
        update,
        view,
    )
    .default_font(crate::fonts::UI)
    .font(crate::fonts::MANROPE_REGULAR_BYTES)
    .font(crate::fonts::MANROPE_MEDIUM_BYTES)
    .font(crate::fonts::MANROPE_BOLD_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_BOLD_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_ITALIC_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_BOLD_ITALIC_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_MEDIUM_BYTES)
    .font(crate::fonts::JETBRAINS_MONO_MEDIUM_ITALIC_BYTES)
    .theme(|state: &Lilypalooza| state.theme.clone())
    .title("Lilypalooza")
    .window(window::Settings {
        min_size: Some(Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
        exit_on_close_request: false,
        ..window::Settings::default()
    })
    .subscription(subscription)
    .run()
}

fn new(
    startup_soundfont: Option<PathBuf>,
    startup_score: Option<PathBuf>,
) -> (Lilypalooza, Task<Message>) {
    let default_settings = settings::AppSettings::default();
    let default_global_state = GlobalState::default();
    let browser_history_dir = tempfile::Builder::new()
        .prefix("lilypalooza-browser-history")
        .tempdir()
        .ok();
    let (stored_settings, settings_error) = match settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (default_settings.clone(), Some(error)),
    };
    let (mut stored_state, state_error) = match state::load_global() {
        Ok(state) => (state, None),
        Err(error) => (default_global_state.clone(), Some(error)),
    };
    migrate_workspace_layout(
        &mut stored_state.workspace_layout.root,
        &stored_state.workspace_layout.folded_panes,
    );

    let (dock_layout, dock_groups, next_dock_group_id, workspace_panes) =
        build_dock_runtime(stored_state.workspace_layout.root.as_ref());
    let mut folded_panes: Vec<_> = stored_state
        .workspace_layout
        .folded_panes
        .iter()
        .cloned()
        .map(folded_pane_from_settings)
        .collect();
    if folded_panes.is_empty() && !stored_state.workspace_layout.piano_visible {
        folded_panes.push(FoldedPaneState {
            pane: WorkspacePaneKind::PianoRoll,
            restore: FoldedPaneRestore::Tab {
                anchor: WorkspacePaneKind::Score,
            },
        });
    }
    let mixer_visible = dock_groups
        .values()
        .any(|group| group.tabs.contains(&WorkspacePaneKind::Mixer));
    if !mixer_visible
        && !folded_panes
            .iter()
            .any(|folded| folded.pane == WorkspacePaneKind::Mixer)
    {
        folded_panes.push(FoldedPaneState {
            pane: WorkspacePaneKind::Mixer,
            restore: FoldedPaneRestore::Standalone,
        });
    }
    let piano_roll_visible = !folded_panes
        .iter()
        .any(|folded| folded.pane == WorkspacePaneKind::PianoRoll);

    let mut piano_roll = PianoRollState::new(default_global_state.piano_roll_view);
    piano_roll.visible = piano_roll_visible;
    piano_roll.apply_view_settings(
        stored_state.piano_roll_view.zoom_x,
        stored_state.piano_roll_view.beat_subdivision,
    );

    let initial_focused_workspace_pane = dock_layout
        .as_ref()
        .and_then(|layout| first_active_workspace_pane(layout, &dock_groups))
        .or_else(|| dock_groups.values().next().map(|group| group.active));

    let (playback, playback_init_error) =
        match AudioEngine::start_cpal(MixerState::new(), AudioEngineOptions::default()) {
            Ok(engine) => (Some(engine), None),
            Err(error) => (None, Some(error.to_string())),
        };

    let mut app = Lilypalooza {
        theme: iced::Theme::Dark,
        window_width: MIN_WINDOW_WIDTH,
        window_height: MIN_WINDOW_HEIGHT,
        lilypond_status: LilypondStatus::Checking,
        current_score: None,
        error_prompt: None,
        prompt_ok_action: None,
        prompt_selected_button: PromptSelectedButton::Ok,
        logger: Logger::new(),
        score_watcher: None,
        editor_file_watcher: None,
        browser_file_watcher: None,
        browser_history_dir,
        browser_history_next_stash_id: 1,
        browser_undo_stack: Vec::new(),
        browser_redo_stack: Vec::new(),
        mixer_undo_stack: Vec::new(),
        mixer_redo_stack: Vec::new(),
        pending_mixer_undo_snapshot: None,
        build_dir: None,
        compile_requested: false,
        compile_outputs_loading: false,
        compile_generation: 0,
        spinner_step: 0,
        compile_session: None,
        playback,
        soundfont_status: SoundfontStatus::NotSelected,
        workspace_panes,
        dock_layout,
        dock_groups,
        next_dock_group_id,
        folded_panes,
        focused_workspace_pane: initial_focused_workspace_pane,
        hovered_workspace_pane: None,
        pressed_workspace_pane: None,
        workspace_drag_origin: None,
        dragged_workspace_pane: None,
        dock_drop_target: None,
        open_header_overflow_menu: None,
        open_editor_menu_section: None,
        open_editor_file_menu_section: None,
        hovered_editor_file_menu_section: None,
        open_project_menu: false,
        open_project_menu_section: None,
        open_project_recent: false,
        open_shortcuts_dialog: false,
        shortcuts_search_query: String::new(),
        shortcuts_search_input_id: Id::unique(),
        shortcuts_selected_action: None,
        hovered_tooltip_key: None,
        open_tooltip_key: None,
        pressed_editor_tab: None,
        hovered_editor_tab: None,
        dragged_editor_tab: None,
        editor_tab_drag_origin: None,
        editor_tab_drop_after: false,
        editor_file_browser_focused: false,
        editor_file_browser_scroll_x: 0.0,
        editor_file_browser_viewport_width: 0.0,
        editor_file_browser_column_scroll_y: HashMap::new(),
        editor_file_browser_column_viewport_height: HashMap::new(),
        editor_file_browser_cursor: None,
        browser_clipboard: None,
        browser_inline_edit: None,
        browser_inline_edit_value: String::new(),
        browser_inline_edit_input_id: Id::unique(),
        browser_pressed_entry: None,
        browser_drag_state: None,
        browser_drop_target: None,
        editor_tabbar_scroll_x: 0.0,
        editor_tabbar_viewport_width: 0.0,
        editor_tabbar_autoscroll_direction: 0,
        editor_tabbar_drag_pointer_x: None,
        pending_reveal_editor_tab: None,
        renaming_editor_tab: None,
        editor_tab_rename_value: String::new(),
        editor_tab_rename_input_id: Id::unique(),
        editor_recent_files: stored_state.editor_recent_files.clone(),
        recent_projects: stored_state.recent_projects.clone(),
        editor_recent_files_limit: stored_settings.editor_recent_files_limit.max(1),
        editor: editor::EditorState::new(
            iced::Theme::Dark,
            stored_settings.editor_view,
            stored_settings.editor_theme,
        ),
        editor_font_metrics_refresh_pending: true,
        rendered_score: None,
        score_cursor_maps: None,
        score_cursor_overlay: None,
        piano_roll,
        svg_zoom: stored_state
            .score_view
            .zoom
            .clamp(MIN_SVG_ZOOM, MAX_SVG_ZOOM),
        svg_page_brightness: stored_state
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
        transport_seek_preview: None,
        keyboard_modifiers: keyboard::Modifiers::default(),
        primary_mouse_pressed: false,
        shortcut_settings: stored_settings.shortcuts.clone(),
        project_root: None,
        project_name: None,
        pending_editor_action: None,
        pending_editor_save_as_tab: None,
        pending_editor_rename_tab: None,
        default_global_state,
        #[cfg(target_os = "macos")]
        macos_quit_menu_patched: false,
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
    if let Some(error) = state_error {
        app.logger.push(format!("State load failed: {error}"));
    }
    if let Some(error) = playback_init_error {
        app.logger
            .push(format!("Playback engine startup failed: {error}"));
    }

    app.restore_editor_session(
        &stored_state.editor_tabs,
        stored_state.active_editor_tab.as_deref(),
        stored_state.has_clean_untitled_editor_tab,
    );

    let mut startup_tasks = vec![Task::perform(
        async { lilypond::check_lilypond().map_err(|error| error.to_string()) },
        Message::StartupChecked,
    )];
    startup_tasks.push(Task::perform(
        cleanup_stale_browser_history_dirs(
            app.browser_history_dir
                .as_ref()
                .map(|dir| dir.path().to_path_buf()),
        ),
        Message::BrowserHistoryCleanupFinished,
    ));

    if let Some(path) = startup_soundfont {
        startup_tasks.push(Task::done(Message::File(FileMessage::SoundfontPicked(
            Some(path),
        ))));
    }
    if let Some(path) = startup_score.or(stored_state.main_score.clone()) {
        startup_tasks.push(Task::done(Message::File(FileMessage::Picked(Some(path)))));
    }

    (app, Task::batch(startup_tasks))
}

fn subscription(app: &Lilypalooza) -> Subscription<Message> {
    let mut subscriptions = vec![
        window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
        event::listen_with(runtime_event_to_message),
    ];

    if app.compile_session.is_some()
        || app.score_watcher.is_some()
        || app.browser_file_watcher.is_some()
        || app.editor.has_document()
        || app.spinner_active()
        || app.playback.is_some()
    {
        subscriptions.push(iced::time::every(BACKGROUND_POLL_INTERVAL).map(|_| Message::Tick));
    }

    if app.dragged_editor_tab.is_some() {
        subscriptions
            .push(iced::time::every(EDITOR_TABBAR_AUTOSCROLL_INTERVAL).map(|_| Message::Tick));
    }

    if app.score_zoom_preview_active() || app.score_zoom_persist_pending {
        subscriptions.push(iced::time::every(SCORE_ZOOM_PREVIEW_INTERVAL).map(|_| Message::Tick));
    }

    if app.playback.is_some() && app.piano_roll.playback_is_playing() {
        subscriptions.push(iced::time::every(PLAYBACK_POLL_INTERVAL).map(Message::Frame));
    }

    Subscription::batch(subscriptions)
}

async fn cleanup_stale_browser_history_dirs(current_dir: Option<PathBuf>) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let entries = fs::read_dir(&temp_dir)
        .map_err(|error| format!("Failed to read temp dir {}: {error}", temp_dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read temp dir entry in {}: {error}",
                temp_dir.display()
            )
        })?;
        let path = entry.path();
        if current_dir.as_ref() == Some(&path) {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.starts_with("lilypalooza-browser-history") {
            continue;
        }
        if let Ok(file_type) = entry.file_type()
            && file_type.is_dir()
        {
            let _ = fs::remove_dir_all(&path);
        }
    }

    Ok(())
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
            physical_key,
            modifiers,
            ..
        }) => Some(Message::KeyPressed(KeyPress {
            status,
            key,
            physical_key,
            modifiers,
        })),
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            Some(Message::PrimaryMousePressed(true))
        }
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::PrimaryMousePressed(false))
        }
        iced::Event::Window(window::Event::CloseRequested) => Some(Message::WindowCloseRequested),
        _ => None,
    }
}

impl Lilypalooza {
    fn patch_macos_quit_menu(&mut self) {
        #[cfg(target_os = "macos")]
        {
            use objc2::MainThreadMarker;
            use objc2_app_kit::NSApplication;
            use objc2_foundation::ns_string;

            if self.macos_quit_menu_patched {
                return;
            }

            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let app = NSApplication::sharedApplication(mtm);
            let Some(main_menu) = app.mainMenu() else {
                return;
            };
            let Some(app_menu_item) = main_menu.itemAtIndex(0) else {
                return;
            };
            let Some(app_menu) = app_menu_item.submenu() else {
                return;
            };
            let Some(quit_item) = app_menu.itemAtIndex(app_menu.numberOfItems() - 1) else {
                return;
            };

            quit_item.setKeyEquivalent(ns_string!(""));
            self.macos_quit_menu_patched = true;
        }
    }

    pub(super) fn zoom_modifier_active(&self) -> bool {
        self.keyboard_modifiers.command() || self.keyboard_modifiers.control()
    }

    pub(super) fn shortcut_modifier_active(&self) -> bool {
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

    pub(super) fn focused_workspace_pane(&self) -> Option<WorkspacePaneKind> {
        self.focused_workspace_pane
            .filter(|pane| self.group_for_pane(*pane).is_some())
    }

    pub(super) fn has_saved_project(&self) -> bool {
        self.project_root.is_some()
    }

    pub(super) fn is_tooltip_open(&self, key: &str) -> bool {
        self.open_tooltip_key.as_deref() == Some(key)
    }

    pub(super) fn project_title(&self) -> String {
        self.project_name
            .as_ref()
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| "Unsaved Project".to_string())
    }

    pub(super) fn spinner_active(&self) -> bool {
        self.compile_requested
            || self.compile_outputs_loading
            || self.compile_session.is_some()
            || matches!(self.lilypond_status, LilypondStatus::Checking)
    }

    pub(super) fn spinner_frame(&self) -> &'static str {
        if self.spinner_active() {
            SPINNER_FRAMES[self.spinner_step % SPINNER_FRAMES.len()]
        } else {
            " "
        }
    }

    pub(super) fn is_workspace_group_focused(&self, group_id: DockGroupId) -> bool {
        let Some(focused_pane) = self.focused_workspace_pane() else {
            return false;
        };

        self.workspace_group(group_id)
            .is_some_and(|group| group.active == focused_pane)
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

fn first_active_workspace_pane(
    node: &DockNode,
    groups: &HashMap<DockGroupId, DockGroup>,
) -> Option<WorkspacePaneKind> {
    match node {
        DockNode::Group(group_id) => groups.get(group_id).map(|group| group.active),
        DockNode::Split { first, second, .. } => first_active_workspace_pane(first, groups)
            .or_else(|| first_active_workspace_pane(second, groups)),
    }
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
    let path = std::fs::canonicalize(&path).unwrap_or(path);
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

fn default_project_name(project_root: &std::path::Path) -> String {
    project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Untitled Project".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts;
    use iced::event;
    use iced_test::simulator;
    use std::fs;

    fn test_app() -> Lilypalooza {
        let (mut app, _task) = new(None, None);
        let _ = update(
            &mut app,
            Message::Shortcuts(messages::ShortcutsMessage::OpenDialog),
        );
        app
    }

    fn test_editor_app() -> Lilypalooza {
        let (app, _task) = new(None, None);
        app
    }

    fn apply_messages(app: &mut Lilypalooza, messages: Vec<Message>) {
        for message in messages {
            let _ = update(app, message);
        }
    }

    fn named_key_press(named: keyboard::key::Named, code: keyboard::key::Code) -> Message {
        Message::KeyPressed(KeyPress {
            status: event::Status::Ignored,
            key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Code(code),
            modifiers: keyboard::Modifiers::default(),
        })
    }

    fn char_key_press(value: &str, code: keyboard::key::Code) -> Message {
        Message::KeyPressed(KeyPress {
            status: event::Status::Ignored,
            key: keyboard::Key::Character(value.into()),
            physical_key: keyboard::key::Physical::Code(code),
            modifiers: keyboard::Modifiers::default(),
        })
    }

    fn active_browser_column_index(app: &Lilypalooza) -> Option<usize> {
        Some(app.editor.file_browser_active_column_index())
    }

    fn selected_browser_entry_name(app: &Lilypalooza, column_index: usize) -> Option<String> {
        app.editor
            .file_browser_columns()
            .get(column_index)
            .and_then(|column| match column {
                editor::EditorBrowserColumnSummary::Directory { entries } => entries
                    .iter()
                    .find(|entry| entry.selected)
                    .map(|entry| entry.name.clone()),
                editor::EditorBrowserColumnSummary::FilePreview { .. } => None,
            })
    }

    fn file_preview_name(app: &Lilypalooza, column_index: usize) -> Option<String> {
        app.editor
            .file_browser_columns()
            .get(column_index)
            .and_then(|column| match column {
                editor::EditorBrowserColumnSummary::FilePreview { metadata } => {
                    Some(metadata.name.clone())
                }
                editor::EditorBrowserColumnSummary::Directory { .. } => None,
            })
    }

    fn browser_entry_names(app: &Lilypalooza, column_index: usize) -> Vec<String> {
        app.editor
            .file_browser_columns()
            .get(column_index)
            .and_then(|column| match column {
                editor::EditorBrowserColumnSummary::Directory { entries } => Some(
                    entries
                        .iter()
                        .map(|entry| entry.name.clone())
                        .collect::<Vec<_>>(),
                ),
                editor::EditorBrowserColumnSummary::FilePreview { .. } => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn actions_palette_search_input_filters_actions() {
        let mut app = test_app();
        let mut ui = simulator(view(&app));

        ui.click("Search actions")
            .expect("search input should be clickable");
        let _ = ui.typewrite("settings");
        let messages: Vec<_> = ui.into_messages().collect();
        apply_messages(&mut app, messages);

        assert_eq!(app.shortcuts_search_query, "settings");
        assert!(
            shortcuts::filtered_action_metadata(&app.shortcuts_search_query)
                .iter()
                .any(|action| action.id == settings::ShortcutActionId::OpenSettingsFile)
        );
    }

    #[test]
    fn actions_palette_clicking_action_emits_activation_message() {
        let mut app = test_app();
        let mut ui = simulator(view(&app));

        ui.click("Search actions")
            .expect("search input should be clickable");
        let _ = ui.typewrite("settings");
        let messages: Vec<_> = ui.into_messages().collect();
        apply_messages(&mut app, messages);

        let mut ui = simulator(view(&app));
        ui.click("Open Settings File")
            .expect("open settings action should be clickable");

        assert!(ui.into_messages().any(|message| matches!(
            message,
            Message::Shortcuts(messages::ShortcutsMessage::ActivateAction(
                settings::ShortcutActionId::OpenSettingsFile
            ))
        )));
    }

    #[test]
    fn browser_arrow_keys_navigate_columns_after_browser_focus_click() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("alpha")).expect("alpha dir");
        fs::create_dir(root.path().join("alpha").join("child")).expect("child dir");
        fs::write(
            root.path().join("alpha").join("child").join("inner.txt"),
            "inner",
        )
        .expect("inner file");
        fs::write(root.path().join("alpha").join("b.txt"), "b").expect("b file");
        fs::create_dir(root.path().join("beta")).expect("beta dir");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("alpha"),
                is_dir: true,
            }),
        );

        assert!(app.editor_file_browser_focused);
        assert_eq!(app.editor.file_browser_columns().len(), 2);
        assert_eq!(active_browser_column_index(&app), Some(0));
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("alpha")
        );
        assert_eq!(selected_browser_entry_name(&app, 1), None);

        let _ = update(
            &mut app,
            named_key_press(
                keyboard::key::Named::ArrowRight,
                keyboard::key::Code::ArrowRight,
            ),
        );
        assert_eq!(active_browser_column_index(&app), Some(1));
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("alpha")
        );
        assert_eq!(
            selected_browser_entry_name(&app, 1).as_deref(),
            Some("child")
        );
        assert_eq!(app.editor.file_browser_columns().len(), 3);
        assert_eq!(selected_browser_entry_name(&app, 2), None);

        let _ = update(
            &mut app,
            named_key_press(
                keyboard::key::Named::ArrowDown,
                keyboard::key::Code::ArrowDown,
            ),
        );
        assert_eq!(
            selected_browser_entry_name(&app, 1).as_deref(),
            Some("b.txt")
        );
        assert_eq!(app.editor.file_browser_columns().len(), 3);
        assert_eq!(file_preview_name(&app, 2).as_deref(), Some("b.txt"));

        let _ = update(
            &mut app,
            named_key_press(
                keyboard::key::Named::ArrowLeft,
                keyboard::key::Code::ArrowLeft,
            ),
        );
        assert_eq!(active_browser_column_index(&app), Some(0));
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("alpha")
        );
        assert_eq!(selected_browser_entry_name(&app, 1), None);

        let _ = update(
            &mut app,
            named_key_press(
                keyboard::key::Named::ArrowRight,
                keyboard::key::Code::ArrowRight,
            ),
        );
        assert_eq!(active_browser_column_index(&app), Some(1));
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("alpha")
        );
        assert_eq!(
            selected_browser_entry_name(&app, 1).as_deref(),
            Some("child")
        );
        assert_eq!(app.editor.file_browser_columns().len(), 3);
        assert_eq!(selected_browser_entry_name(&app, 2), None);
    }

    #[test]
    fn browser_focus_blocks_editor_text_input() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("alpha")).expect("alpha dir");
        let file_path = root.path().join("note.txt");
        fs::write(&file_path, "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        let _ = app.open_editor_file_in_editor(&file_path);
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("alpha"),
                is_dir: true,
            }),
        );

        let before = app.editor.active_content().expect("active content");
        assert!(app.editor_file_browser_focused);
        assert!(!app.editor.active_editor_is_focused());

        let _ = update(&mut app, char_key_press("x", keyboard::key::Code::KeyX));

        assert_eq!(
            app.editor.active_content().as_deref(),
            Some(before.as_str())
        );
        assert!(!app.editor.active_editor_is_focused());
    }

    #[test]
    fn browser_single_click_selects_file_and_double_click_opens_it() {
        let root = TempDir::new().expect("tempdir");
        let file_path = root.path().join("note.txt");
        fs::write(&file_path, "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let before = app.editor.active_content();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: file_path.clone(),
                is_dir: false,
            }),
        );

        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("note.txt")
        );
        assert_eq!(file_preview_name(&app, 1).as_deref(), Some("note.txt"));
        assert_eq!(app.editor.active_content(), before);

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryDoublePressed {
                column_index: 0,
                path: file_path,
                is_dir: false,
            }),
        );

        assert_eq!(app.editor.active_content().as_deref(), Some("hello"));
    }

    #[test]
    fn browser_rename_action_starts_inline_rename() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );

        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);

        assert!(matches!(
            app.browser_inline_edit.as_ref().map(|edit| edit.kind),
            Some(BrowserInlineEditKind::Rename)
        ));
    }

    #[test]
    fn browser_inline_rename_enter_commits_edit() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
                "renamed.txt".to_string(),
            )),
        );

        let _ = update(
            &mut app,
            named_key_press(keyboard::key::Named::Enter, keyboard::key::Code::Enter),
        );

        assert!(app.browser_inline_edit.is_none());
        assert!(root.path().join("renamed.txt").exists());
    }

    #[test]
    fn browser_captured_enter_after_commit_does_not_restart_rename() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserRename);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
                "renamed.txt".to_string(),
            )),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
        );

        let _ = update(
            &mut app,
            Message::KeyPressed(KeyPress {
                status: event::Status::Captured,
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
                modifiers: keyboard::Modifiers::default(),
            }),
        );

        assert!(app.browser_inline_edit.is_none());
        assert!(root.path().join("renamed.txt").exists());
    }

    #[test]
    fn browser_delete_action_opens_delete_prompt() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );

        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserDelete);

        assert!(app.error_prompt.is_some());
        assert!(matches!(
            app.prompt_ok_action,
            Some(PromptOkAction::DeleteBrowserPath(_))
        ));
    }

    #[test]
    fn prompt_enter_runs_selected_button_action() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserDelete);

        let _ = update(
            &mut app,
            Message::KeyPressed(KeyPress {
                status: event::Status::Ignored,
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
                modifiers: keyboard::Modifiers::default(),
            }),
        );

        assert!(app.error_prompt.is_none());
        assert!(app.browser_inline_edit.is_none());
        assert!(!root.path().join("note.txt").exists());
    }

    #[test]
    fn browser_copy_paste_copies_selected_entry() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("folder")).expect("folder dir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCopy);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("folder"),
                is_dir: true,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

        assert!(root.path().join("note.txt").exists());
        assert!(root.path().join("folder").join("note.txt").exists());
    }

    #[test]
    fn browser_cut_paste_moves_selected_entry() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("folder")).expect("folder dir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCut);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("folder"),
                is_dir: true,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

        assert!(!root.path().join("note.txt").exists());
        assert!(root.path().join("folder").join("note.txt").exists());
        assert!(app.browser_clipboard.is_none());
    }

    #[test]
    fn browser_drag_release_moves_item_into_directory() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("folder")).expect("folder dir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserDragMoved(Point::new(
                0.0, 0.0,
            ))),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserDragMoved(Point::new(
                24.0, 0.0,
            ))),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryHovered {
                column_index: 0,
                path: root.path().join("folder"),
                is_dir: true,
            }),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserDragReleased),
        );

        assert!(!root.path().join("note.txt").exists());
        assert!(root.path().join("folder").join("note.txt").exists());
    }

    #[test]
    fn browser_delete_undo_redo_restores_item() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );

        let _ = app.delete_browser_path_with_history(&root.path().join("note.txt"));
        assert!(!root.path().join("note.txt").exists());

        let _ = app.undo_browser_operation();
        assert!(root.path().join("note.txt").exists());

        let _ = app.redo_browser_operation();
        assert!(!root.path().join("note.txt").exists());
    }

    #[test]
    fn browser_cut_paste_undo_redo_moves_item_back_and_forth() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("folder")).expect("folder dir");
        fs::write(root.path().join("note.txt"), "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("note.txt"),
                is_dir: false,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserCut);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("folder"),
                is_dir: true,
            }),
        );
        let _ = app.handle_shortcut_action(shortcuts::ShortcutAction::FileBrowserPaste);

        assert!(!root.path().join("note.txt").exists());
        assert!(root.path().join("folder").join("note.txt").exists());

        let _ = app.undo_browser_operation();
        assert!(root.path().join("note.txt").exists());
        assert!(!root.path().join("folder").join("note.txt").exists());

        let _ = app.redo_browser_operation();
        assert!(!root.path().join("note.txt").exists());
        assert!(root.path().join("folder").join("note.txt").exists());
    }

    #[test]
    fn browser_focus_message_moves_focus_from_editor() {
        let root = TempDir::new().expect("tempdir");
        let file_path = root.path().join("note.txt");
        fs::write(&file_path, "hello").expect("note file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        app.set_focused_workspace_pane(WorkspacePaneKind::Editor);
        let _ = app.open_editor_file_in_editor(&file_path);
        app.editor.request_focus();

        assert!(app.editor.active_editor_is_focused());

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserFocused),
        );

        assert!(app.editor_file_browser_focused);
        assert!(!app.editor.active_editor_is_focused());
    }

    #[test]
    fn browser_toolbar_hidden_toggle_updates_entries() {
        let root = TempDir::new().expect("tempdir");
        fs::create_dir(root.path().join("alpha")).expect("alpha dir");
        fs::write(root.path().join("beta.txt"), "b").expect("beta file");
        fs::write(root.path().join(".hidden.txt"), "h").expect("hidden file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        assert_eq!(browser_entry_names(&app, 0), vec!["alpha", "beta.txt"]);
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserToggleHiddenRequested),
        );
        assert_eq!(
            browser_entry_names(&app, 0),
            vec!["alpha", ".hidden.txt", "beta.txt"]
        );
    }

    #[test]
    fn browser_new_file_requested_starts_inline_edit() {
        let root = TempDir::new().expect("tempdir");
        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserNewFileRequested),
        );

        assert!(matches!(
            app.browser_inline_edit.as_ref().map(|edit| edit.kind),
            Some(BrowserInlineEditKind::NewFile)
        ));
        assert_eq!(app.browser_inline_edit_value, "untitled");
        assert!(app.editor_file_browser_focused);
        assert!(!app.editor.active_editor_is_focused());
    }

    #[test]
    fn browser_commit_new_file_creates_and_selects_file() {
        let root = TempDir::new().expect("tempdir");
        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserNewFileRequested),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
                "created.txt".to_string(),
            )),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
        );

        assert!(root.path().join("created.txt").exists());
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("created.txt")
        );
        assert_eq!(file_preview_name(&app, 1).as_deref(), Some("created.txt"));
    }

    #[test]
    fn browser_commit_rename_renames_selected_entry() {
        let root = TempDir::new().expect("tempdir");
        fs::write(root.path().join("old.txt"), "old").expect("old file");

        let mut app = test_editor_app();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                path: root.path().join("old.txt"),
                is_dir: false,
            }),
        );

        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserRenameRequested),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::FileBrowserInlineEditChanged(
                "renamed.txt".to_string(),
            )),
        );
        let _ = update(
            &mut app,
            Message::Editor(messages::EditorMessage::CommitFileBrowserInlineEdit),
        );

        assert!(!root.path().join("old.txt").exists());
        assert!(root.path().join("renamed.txt").exists());
        assert_eq!(
            selected_browser_entry_name(&app, 0).as_deref(),
            Some("renamed.txt")
        );
        assert_eq!(file_preview_name(&app, 1).as_deref(), Some("renamed.txt"));
    }
}
