use super::*;

#[cfg(test)]
pub(super) static ICED_SNAPSHOT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) const MIN_WINDOW_WIDTH: f32 = 960.0;
pub(super) const MIN_WINDOW_HEIGHT: f32 = 640.0;
pub(super) const WATCH_POLL_INTERVAL: Duration = Duration::from_millis(500);
pub(super) const EDITOR_TICK_INTERVAL: Duration = Duration::from_millis(500);
pub(super) const PLUGIN_SCAN_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const SPINNER_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const EDITOR_TABBAR_AUTOSCROLL_INTERVAL: Duration = Duration::from_millis(16);
pub(super) const SCORE_SCROLLABLE_ID: &str = "score-scrollable";
pub(super) const EDITOR_TABBAR_SCROLL_ID: &str = "editor-tabbar-scroll";
pub(super) const EDITOR_FILE_BROWSER_SCROLL_ID: &str = "editor-file-browser-scroll";
pub(super) const EDITOR_FILE_BROWSER_HEIGHT: f32 = 176.0;
pub(super) const EDITOR_FILE_BROWSER_COLUMN_WIDTH: f32 = 220.0;
pub(super) const EDITOR_FILE_BROWSER_ENTRY_HEIGHT: f32 = 26.0;
pub(super) const SHORTCUTS_SCROLLABLE_ID: &str = "shortcuts-scrollable";
pub(super) const TRACK_RENAME_INPUT_ID: &str = "track-rename-input";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenameTarget {
    Track(usize),
    Bus(u16),
}
pub(super) const KEYBOARD_SCROLL_STEP: f32 = 84.0;
pub(super) const SHORTCUTS_ACTION_ROW_HEIGHT: f32 = 48.0;
pub(super) const MIN_SVG_ZOOM: f32 = 0.4;
pub(super) const MAX_SVG_ZOOM: f32 = 3.0;
pub(super) const SVG_ZOOM_STEP: f32 = 0.1;
pub(super) const MIN_SVG_PAGE_BRIGHTNESS: u8 = 0;
pub(super) const MAX_SVG_PAGE_BRIGHTNESS: u8 = 100;
pub(super) const SVG_PAGE_BRIGHTNESS_STEP: u8 = 10;
pub(super) const SCORE_ZOOM_PREVIEW_INTERVAL: Duration = Duration::from_millis(16);
pub(super) const SCORE_ZOOM_PREVIEW_SETTLE_DELAY: Duration = Duration::from_millis(120);
pub(super) const ACTIVE_PLAYBACK_POLL_INTERVAL: Duration = Duration::from_millis(33);
pub(super) const PASSIVE_PLAYBACK_POLL_INTERVAL: Duration = Duration::from_millis(120);
pub(super) const EDITOR_HOST_POLL_INTERVAL: Duration = Duration::from_millis(33);
pub(super) const SPINNER_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

pub(super) fn audio_engine_options(playback: &settings::PlaybackSettings) -> AudioEngineOptions {
    AudioEngineOptions {
        device: playback.device.clone(),
        chase_notes_on_seek: playback.chase_notes_on_seek,
        sample_rate: playback.sample_rate,
        block_size: playback.block_size,
        ..AudioEngineOptions::default()
    }
}

pub(super) fn plugin_scan_cache_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "lilypalooza")
        .map(|project_dirs| project_dirs.config_dir().join("plugin-cache.ron"))
        .unwrap_or_else(|| PathBuf::from("plugin-cache.ron"))
}

#[cfg(not(test))]
pub(super) fn plugin_validator_path() -> PathBuf {
    plugin_validator_path_for_exe(
        &std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lilypalooza")),
    )
}

pub(super) fn plugin_validator_path_for_exe(exe: &Path) -> PathBuf {
    let validator_name = format!(
        "lilypalooza-plugin-validator{}",
        std::env::consts::EXE_SUFFIX
    );

    if is_macos_app_bundle_exe(exe)
        && let Some(contents_dir) = exe
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == "Contents"))
    {
        return contents_dir.join("MacOS").join(validator_name);
    }

    exe.parent()
        .map(|parent| parent.join(&validator_name))
        .unwrap_or_else(|| PathBuf::from(validator_name))
}

pub(super) fn is_macos_app_bundle_exe(exe: &Path) -> bool {
    exe.ancestors()
        .any(|path| path.extension().is_some_and(|extension| extension == "app"))
}

pub(super) fn ensure_plugin_validator_available(validator: &Path) -> Result<(), String> {
    if validator.exists() {
        Ok(())
    } else {
        Err(format!(
            "Plugin validator helper not found at {}",
            validator.display()
        ))
    }
}

pub(super) fn editor_file_browser_column_scroll_id(index: usize) -> Id {
    Id::new(Box::leak(
        format!("editor-file-browser-column-{index}").into_boxed_str(),
    ))
}

pub(super) type WorkspacePaneKind = crate::settings::WorkspacePane;

pub(super) type DockGroupId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorHeaderMenuSection {
    File,
    Edit,
    Appearance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorFileMenuSection {
    OpenRecent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectMenuSection {
    Project,
    View,
}

pub(super) struct Lilypalooza {
    pub(super) theme: iced::Theme,
    pub(super) main_window_id: window::Id,
    pub(super) main_window_snapshot: Option<WindowSnapshot>,
    pub(super) window_width: f32,
    pub(super) window_height: f32,
    pub(super) lilypond_status: LilypondStatus,
    pub(super) current_score: Option<SelectedScore>,
    pub(super) error_prompt: Option<ErrorPrompt>,
    pub(super) prompt_ok_action: Option<PromptOkAction>,
    pub(super) prompt_selected_button: PromptSelectedButton,
    pub(super) logger: Logger,
    pub(super) score_watcher: Option<ScoreWatcher>,
    pub(super) editor_file_watcher: Option<EditorFileWatcher>,
    pub(super) browser_file_watcher: Option<BrowserFileWatcher>,
    pub(super) browser_history_dir: Option<TempDir>,
    pub(super) browser_history_next_stash_id: u64,
    pub(super) browser_undo_stack: Vec<BrowserHistoryEntry>,
    pub(super) browser_redo_stack: Vec<BrowserHistoryEntry>,
    pub(super) mixer_undo_stack: Vec<MixerState>,
    pub(super) mixer_redo_stack: Vec<MixerState>,
    pub(super) pending_mixer_undo_snapshot: Option<MixerState>,
    pub(super) pending_mixer_message_after_editor_close: Option<(window::Id, MixerMessage)>,
    pub(super) pending_mixer_message_after_editor_detach: Option<DeferredMixerMessage>,
    pub(super) build_dir: Option<TempDir>,
    pub(super) compile_requested: bool,
    pub(super) compile_outputs_loading: bool,
    pub(super) compile_generation: u64,
    pub(super) spinner_step: usize,
    pub(super) compile_session: Option<lilypond::CompileSession>,
    pub(super) playback: Option<AudioEngine>,
    pub(super) soundfont_status: SoundfontStatus,
    pub(super) playback_settings: settings::PlaybackSettings,
    pub(super) plugin_search_paths: Vec<settings::PluginSearchPath>,
    pub(super) plugin_scan: lilypalooza_plugin_scan::PluginScanState,
    pub(super) plugin_scan_cache: lilypalooza_plugin_scan::PluginScanCache,
    pub(super) saved_project_state: Option<ProjectState>,
    pub(super) project_mixer_state: MixerState,
    pub(super) processor_presets: processor_presets::ProcessorPresetLibrary,
    pub(super) expanded_processor_preset_browser: Option<processor_editor_windows::EditorTarget>,
    pub(super) workspace_panes: pane_grid::State<DockGroupId>,
    pub(super) dock_layout: Option<DockNode>,
    pub(super) dock_groups: HashMap<DockGroupId, DockGroup>,
    pub(super) next_dock_group_id: DockGroupId,
    pub(super) folded_panes: Vec<FoldedPaneState>,
    pub(super) focused_workspace_pane: Option<WorkspacePaneKind>,
    pub(super) hovered_workspace_pane: Option<WorkspacePaneKind>,
    pub(super) pressed_workspace_pane: Option<WorkspacePaneKind>,
    pub(super) workspace_drag_origin: Option<Point>,
    pub(super) dragged_workspace_pane: Option<WorkspacePaneKind>,
    pub(super) dock_drop_target: Option<DockDropTarget>,
    pub(super) open_header_overflow_menu: Option<DockGroupId>,
    pub(super) open_editor_menu_section: Option<EditorHeaderMenuSection>,
    pub(super) open_editor_file_menu_section: Option<EditorFileMenuSection>,
    pub(super) hovered_editor_file_menu_section: Option<EditorFileMenuSection>,
    pub(super) open_project_menu: bool,
    pub(super) open_project_menu_section: Option<ProjectMenuSection>,
    pub(super) open_project_recent: bool,
    pub(super) open_shortcuts_dialog: bool,
    pub(super) processor_editor_windows: EditorWindowManager,
    pub(super) shortcuts_search_query: String,
    pub(super) shortcuts_search_input_id: Id,
    pub(super) shortcuts_selected_action: Option<settings::ShortcutActionId>,
    pub(super) hovered_tooltip_key: Option<String>,
    pub(super) open_tooltip_key: Option<String>,
    pub(super) pressed_editor_tab: Option<u64>,
    pub(super) hovered_editor_tab: Option<u64>,
    pub(super) dragged_editor_tab: Option<u64>,
    pub(super) editor_tab_drag_origin: Option<Point>,
    pub(super) editor_tab_drop_after: bool,
    pub(super) editor_file_browser_focused: bool,
    pub(super) editor_file_browser_scroll_x: f32,
    pub(super) editor_file_browser_viewport_width: f32,
    pub(super) editor_file_browser_column_scroll_y: HashMap<usize, f32>,
    pub(super) editor_file_browser_column_viewport_height: HashMap<usize, f32>,
    pub(super) editor_file_browser_cursor: Option<Point>,
    pub(super) browser_clipboard: Option<BrowserClipboard>,
    pub(super) browser_inline_edit: Option<BrowserInlineEdit>,
    pub(super) browser_inline_edit_value: String,
    pub(super) browser_inline_edit_input_id: Id,
    pub(super) browser_pressed_entry: Option<BrowserPressedEntry>,
    pub(super) browser_drag_state: Option<BrowserDragState>,
    pub(super) browser_drop_target: Option<BrowserDropTarget>,
    pub(super) editor_tabbar_scroll_x: f32,
    pub(super) editor_tabbar_viewport_width: f32,
    pub(super) mixer_instrument_scroll_x: f32,
    pub(super) mixer_instrument_viewport_width: f32,
    pub(super) mixer_bus_scroll_x: f32,
    pub(super) mixer_bus_viewport_width: f32,
    pub(super) open_mixer_effect_rack_tracks: Vec<usize>,
    pub(super) effect_rack_scroll_y: HashMap<usize, f32>,
    pub(super) effect_rack_viewport_height: HashMap<usize, f32>,
    pub(super) editor_tabbar_autoscroll_direction: i8,
    pub(super) editor_tabbar_drag_pointer_x: Option<f32>,
    pub(super) pending_reveal_editor_tab: Option<u64>,
    pub(super) renaming_editor_tab: Option<u64>,
    pub(super) editor_tab_rename_value: String,
    pub(super) editor_tab_rename_input_id: Id,
    pub(super) renaming_target: Option<RenameTarget>,
    pub(super) renaming_origin: Option<WorkspacePaneKind>,
    pub(super) track_color_picker_target: Option<(usize, WorkspacePaneKind)>,
    pub(super) track_rename_was_focused: bool,
    pub(super) track_rename_value: String,
    pub(super) track_rename_color_picker_open: bool,
    pub(super) track_rename_color_value: Color,
    pub(super) selected_track_index: Option<usize>,
    pub(super) open_processor_browser_target: Option<processor_editor_windows::EditorTarget>,
    pub(super) hovered_processor_slot: Option<(
        processor_editor_windows::EditorTarget,
        mixer::ProcessorSlotSegment,
    )>,
    pub(super) effect_rack_hovered_effect: Option<(usize, usize)>,
    pub(super) effect_drag_source: Option<(usize, usize)>,
    pub(super) effect_drag_target: Option<(usize, usize)>,
    pub(super) effect_rack_autoscroll_direction: i8,
    pub(super) effect_rack_drag_pointer_y: Option<f32>,
    pub(super) open_instrument_browser_track: Option<usize>,
    pub(super) instrument_browser_search: String,
    pub(super) processor_browser_expanded_sections: Vec<mixer::ProcessorBrowserSectionKey>,
    pub(super) instrument_browser_search_input_id: Id,
    pub(super) track_name_overrides: Vec<Option<String>>,
    pub(super) track_color_overrides: Vec<Option<Color>>,
    pub(super) metronome: crate::state::MetronomeState,
    pub(super) metronome_menu_open: bool,
    pub(super) editor_recent_files: Vec<PathBuf>,
    pub(super) recent_projects: Vec<PathBuf>,
    pub(super) editor_recent_files_limit: usize,
    pub(super) editor: editor::EditorState,
    pub(super) editor_font_metrics_refresh_pending: bool,
    pub(super) rendered_score: Option<RenderedScore>,
    pub(super) score_cursor_maps: Option<ScoreCursorMaps>,
    pub(super) score_cursor_overlay: Option<ScoreCursorPlacement>,
    pub(super) piano_roll: PianoRollState,
    pub(super) svg_zoom: f32,
    pub(super) svg_page_brightness: u8,
    pub(super) svg_scroll_x: f32,
    pub(super) svg_scroll_y: f32,
    pub(super) score_viewport_cursor: Option<iced::Point>,
    pub(super) score_zoom_last_interaction: Option<Instant>,
    pub(super) score_zoom_persist_pending: bool,
    pub(super) score_zoom_preview: Option<ScoreZoomPreview>,
    pub(super) score_zoom_preview_pending: Option<ScoreZoomPreviewRequest>,
    pub(super) piano_roll_viewport_cursor: Option<iced::Point>,
    pub(super) transport_seek_preview: Option<f32>,
    pub(super) keyboard_modifiers: keyboard::Modifiers,
    pub(super) primary_mouse_pressed: bool,
    pub(super) shortcut_settings: settings::ShortcutSettings,
    pub(super) project_root: Option<PathBuf>,
    pub(super) project_name: Option<String>,
    pub(super) pending_editor_action: Option<PendingEditorAction>,
    pub(super) pending_editor_save_as_tab: Option<u64>,
    pub(super) pending_editor_rename_tab: Option<u64>,
    pub(super) default_global_state: GlobalState,
}

pub(super) struct DeferredMixerMessage {
    pub(super) message: MixerMessage,
    pub(super) frames_remaining: u8,
}

pub(super) struct SelectedScore {
    pub(super) path: PathBuf,
    pub(super) file_name: String,
}

pub(super) struct RenderedScore {
    pub(super) pages: Vec<RenderedPage>,
    pub(super) current_page: usize,
}

#[derive(Debug, Clone)]
pub(super) struct LoadedCompileOutputs {
    pub(super) score_path: PathBuf,
    pub(super) rendered_pages: Vec<LoadedRenderedPage>,
    pub(super) midi_files: Vec<crate::midi::MidiRollFile>,
    pub(super) score_cursor_maps: Option<ScoreCursorMaps>,
    pub(super) point_and_click_disabled: bool,
    pub(super) score_has_repeats: bool,
}

pub(super) struct RenderedPage {
    pub(super) handle: svg::Handle,
    pub(super) svg_bytes: Bytes,
    pub(super) display_size: SvgSize,
    pub(super) coord_size: SvgSize,
    pub(super) note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    pub(super) system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Debug, Clone)]
pub(super) struct LoadedRenderedPage {
    pub(super) svg_bytes: Vec<u8>,
    pub(super) display_size: SvgSize,
    pub(super) coord_size: SvgSize,
    pub(super) note_anchors: Vec<score_cursor::SvgNoteAnchor>,
    pub(super) system_bands: Vec<score_cursor::SystemBand>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SvgSize {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Clone)]
pub(super) struct ScoreZoomPreview {
    pub(super) page_index: usize,
    pub(super) tier: ScoreZoomPreviewTier,
    pub(super) handle: image::Handle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ScoreZoomPreviewRequest {
    pub(super) page_index: usize,
    pub(super) zoom: f32,
    pub(super) tier: ScoreZoomPreviewTier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScoreZoomPreviewTier {
    Fallback,
    Primary,
}

impl RenderedScore {
    pub(super) fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub(super) fn current_page_number(&self) -> usize {
        self.current_page.saturating_add(1)
    }

    pub(super) fn current_page(&self) -> Option<&RenderedPage> {
        self.pages.get(self.current_page)
    }
}

#[derive(Debug, Clone)]
pub(super) struct DockGroup {
    pub(super) tabs: Vec<WorkspacePaneKind>,
    pub(super) active: WorkspacePaneKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FoldedPaneState {
    pub(super) pane: WorkspacePaneKind,
    pub(super) restore: FoldedPaneRestore,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum FoldedPaneRestore {
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
pub(super) enum DockNode {
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

pub(super) enum LilypondStatus {
    Checking,
    Ready { detected: lilypond::Version },
    Unavailable,
}

pub(super) enum SoundfontStatus {
    NotSelected,
    Ready(PathBuf),
    Error,
}

#[derive(Debug, Clone)]
pub(super) enum PromptOkAction {
    ExitApp,
    ClearLogs,
    ReloadEditorTab(u64),
    DeleteBrowserPath(PathBuf),
    RemoveBus(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BrowserInlineEditKind {
    Rename,
    NewFile,
    NewDirectory,
}

#[derive(Debug, Clone)]
pub(super) enum BrowserHistoryEntry {
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
pub(super) enum BrowserClipboardKind {
    Cut,
    Copy,
}

#[derive(Debug, Clone)]
pub(super) struct BrowserClipboard {
    pub(super) path: PathBuf,
    pub(super) kind: BrowserClipboardKind,
}

#[derive(Debug, Clone)]
pub(super) struct BrowserInlineEdit {
    pub(super) column_index: usize,
    pub(super) parent_dir: PathBuf,
    pub(super) target_path: Option<PathBuf>,
    pub(super) kind: BrowserInlineEditKind,
}

#[derive(Debug, Clone)]
pub(super) struct BrowserPressedEntry {
    pub(super) path: PathBuf,
    pub(super) is_dir: bool,
    pub(super) origin: Point,
}

#[derive(Debug, Clone)]
pub(super) struct BrowserDragState {
    pub(super) source_path: PathBuf,
    pub(super) source_is_dir: bool,
}

#[derive(Debug, Clone)]
pub(super) struct BrowserDropTarget {
    pub(super) column_index: usize,
    pub(super) path: PathBuf,
    pub(super) target_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub(super) enum PendingEditorAction {
    ResolveDirtyTabs {
        dirty_tab_ids: Vec<u64>,
        continuation: EditorContinuation,
    },
    ResolveDirtyProject {
        continuation: EditorContinuation,
    },
}

#[derive(Debug, Clone)]
pub(super) enum EditorContinuation {
    CloseTab(u64),
    LoadProject(PathBuf),
    OpenScore(PathBuf),
    ExitApp,
}

pub(crate) fn run(
    startup_soundfont: Option<PathBuf>,
    startup_score: Option<PathBuf>,
    audio_enabled: bool,
) -> iced::Result {
    iced::daemon(
        move || {
            new(
                startup_soundfont.clone(),
                startup_score.clone(),
                audio_enabled,
            )
        },
        update,
        window_view,
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
    .theme(|state: &Lilypalooza, _window| state.theme.clone())
    .title(window_title)
    .subscription(subscription)
    .run()
}

pub(super) fn main_window_settings() -> window::Settings {
    window::Settings {
        min_size: Some(Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

pub(super) fn window_view<'a>(
    app: &'a Lilypalooza,
    window_id: window::Id,
) -> iced::Element<'a, Message> {
    if window_id == app.main_window_id {
        return view(app);
    }

    iced::widget::container(iced::widget::text(""))
        .width(iced::Fill)
        .height(iced::Fill)
        .into()
}

pub(super) fn window_title(app: &Lilypalooza, window_id: window::Id) -> String {
    if window_id == app.main_window_id {
        return "Lilypalooza".to_string();
    }

    app.processor_editor_windows
        .window_title(window_id)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Processor Editor".to_string())
}

pub(super) fn new(
    startup_soundfont: Option<PathBuf>,
    startup_score: Option<PathBuf>,
    audio_enabled: bool,
) -> (Lilypalooza, Task<Message>) {
    let default_settings = settings::AppSettings::default();
    let default_global_state = GlobalState::default();
    let (stored_settings, settings_error) = match settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (default_settings.clone(), Some(error)),
    };
    let (stored_state, state_error) = match state::load_global() {
        Ok(state) => (state, None),
        Err(error) => (default_global_state.clone(), Some(error)),
    };

    new_with_loaded_state(
        startup_soundfont,
        startup_score,
        audio_enabled,
        stored_settings,
        settings_error,
        stored_state,
        state_error,
    )
}

pub(super) fn new_with_loaded_state(
    startup_soundfont: Option<PathBuf>,
    startup_score: Option<PathBuf>,
    audio_enabled: bool,
    stored_settings: settings::AppSettings,
    settings_error: Option<String>,
    mut stored_state: GlobalState,
    state_error: Option<String>,
) -> (Lilypalooza, Task<Message>) {
    let editor_host_error = editor_host::prepare_process().err();
    let (main_window_id, open_main_window) = window::open(main_window_settings());
    let default_global_state = GlobalState::default();
    let browser_history_dir = tempfile::Builder::new()
        .prefix("lilypalooza-browser-history")
        .tempdir()
        .ok();
    let startup_soundfonts = match startup_soundfont {
        Some(path) => vec![path],
        None => stored_settings.playback.soundfonts.clone(),
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
    normalize_loaded_folded_panes(&dock_groups, &mut folded_panes);
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

    let (playback, playback_init_error) = if audio_enabled {
        match AudioEngine::start_cpal(
            MixerState::new(),
            audio_engine_options(&stored_settings.playback),
        ) {
            Ok(engine) => (Some(engine), None),
            Err(error) => (None, Some(error.to_string())),
        }
    } else {
        (None, None)
    };
    let project_mixer_state = MixerState::new();
    let shortcuts_search_input_id = Id::unique();
    let browser_inline_edit_input_id = Id::unique();
    let editor_tab_rename_input_id = Id::unique();
    let instrument_browser_search_input_id = Id::unique();
    let editor = editor::EditorState::new(
        iced::Theme::Dark,
        stored_settings.editor_view,
        stored_settings.editor_theme,
    );
    let logger = Logger::new();
    let processor_editor_windows = EditorWindowManager::default();
    let track_rename_color_value = crate::track_colors::default_track_color(0);
    let editor_recent_files = stored_state.editor_recent_files.clone();
    let recent_projects = stored_state.recent_projects.clone();
    let editor_recent_files_limit = stored_settings.editor_recent_files_limit.max(1);
    let shortcut_settings = stored_settings.shortcuts.clone();
    let svg_zoom = stored_state
        .score_view
        .zoom
        .clamp(MIN_SVG_ZOOM, MAX_SVG_ZOOM);
    let svg_page_brightness = stored_state
        .score_view
        .page_brightness
        .clamp(MIN_SVG_PAGE_BRIGHTNESS, MAX_SVG_PAGE_BRIGHTNESS);

    let mut app = Lilypalooza {
        theme: iced::Theme::Dark,
        main_window_id,
        main_window_snapshot: None,
        window_width: MIN_WINDOW_WIDTH,
        window_height: MIN_WINDOW_HEIGHT,
        lilypond_status: LilypondStatus::Checking,
        current_score: None,
        error_prompt: None,
        prompt_ok_action: None,
        prompt_selected_button: PromptSelectedButton::Ok,
        logger,
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
        pending_mixer_message_after_editor_close: None,
        pending_mixer_message_after_editor_detach: None,
        build_dir: None,
        compile_requested: false,
        compile_outputs_loading: false,
        compile_generation: 0,
        spinner_step: 0,
        compile_session: None,
        playback,
        soundfont_status: SoundfontStatus::NotSelected,
        playback_settings: stored_settings.playback.clone(),
        plugin_search_paths: stored_settings.plugin_search_paths(),
        plugin_scan: lilypalooza_plugin_scan::PluginScanState::default(),
        plugin_scan_cache: lilypalooza_plugin_scan::PluginScanCache::load_from(
            &plugin_scan_cache_path(),
        ),
        saved_project_state: None,
        project_mixer_state,
        processor_presets: stored_state.processor_presets.clone(),
        expanded_processor_preset_browser: None,
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
        processor_editor_windows,
        shortcuts_search_query: String::new(),
        shortcuts_search_input_id,
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
        browser_inline_edit_input_id,
        browser_pressed_entry: None,
        browser_drag_state: None,
        browser_drop_target: None,
        editor_tabbar_scroll_x: 0.0,
        editor_tabbar_viewport_width: 0.0,
        mixer_instrument_scroll_x: 0.0,
        mixer_instrument_viewport_width: 0.0,
        mixer_bus_scroll_x: 0.0,
        mixer_bus_viewport_width: 0.0,
        open_mixer_effect_rack_tracks: Vec::new(),
        effect_rack_scroll_y: HashMap::new(),
        effect_rack_viewport_height: HashMap::new(),
        editor_tabbar_autoscroll_direction: 0,
        editor_tabbar_drag_pointer_x: None,
        pending_reveal_editor_tab: None,
        renaming_editor_tab: None,
        editor_tab_rename_value: String::new(),
        editor_tab_rename_input_id,
        renaming_target: None,
        renaming_origin: None,
        track_color_picker_target: None,
        track_rename_was_focused: false,
        track_rename_value: String::new(),
        track_rename_color_picker_open: false,
        track_rename_color_value,
        selected_track_index: None,
        open_processor_browser_target: None,
        hovered_processor_slot: None,
        effect_rack_hovered_effect: None,
        effect_drag_source: None,
        effect_drag_target: None,
        effect_rack_autoscroll_direction: 0,
        effect_rack_drag_pointer_y: None,
        open_instrument_browser_track: None,
        instrument_browser_search: String::new(),
        processor_browser_expanded_sections: Vec::new(),
        instrument_browser_search_input_id,
        track_name_overrides: Vec::new(),
        track_color_overrides: Vec::new(),
        metronome: crate::state::MetronomeState::default(),
        metronome_menu_open: false,
        editor_recent_files,
        recent_projects,
        editor_recent_files_limit,
        editor,
        editor_font_metrics_refresh_pending: true,
        rendered_score: None,
        score_cursor_maps: None,
        score_cursor_overlay: None,
        piano_roll,
        svg_zoom,
        svg_page_brightness,
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
        shortcut_settings,
        project_root: None,
        project_name: None,
        pending_editor_action: None,
        pending_editor_save_as_tab: None,
        pending_editor_rename_tab: None,
        default_global_state,
    };

    app.logger.push("Checking LilyPond availability");
    if !audio_enabled {
        app.logger.push("Audio engine disabled by --no-audio");
    }
    for path in &startup_soundfonts {
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
    if let Some(error) = editor_host_error {
        app.logger
            .push(format!("Editor host setup failed: {error}"));
    }
    #[cfg(not(test))]
    app.start_plugin_scan();

    app.restore_editor_session(
        &stored_state.editor_tabs,
        stored_state.active_editor_tab.as_deref(),
        stored_state.has_clean_untitled_editor_tab,
    );

    let mut startup_tasks = vec![
        open_main_window.map(|_| Message::Noop),
        Task::perform(
            async { lilypond::check_lilypond().map_err(|error| error.to_string()) },
            Message::StartupChecked,
        ),
    ];
    startup_tasks.push(Task::perform(
        cleanup_stale_browser_history_dirs(
            app.browser_history_dir
                .as_ref()
                .map(|dir| dir.path().to_path_buf()),
        ),
        Message::BrowserHistoryCleanupFinished,
    ));

    if audio_enabled && !startup_soundfonts.is_empty() {
        app.initialize_playback_soundfonts(startup_soundfonts);
    }
    app.saved_project_state = Some(app.current_project_state());
    if let Some(path) = startup_score.or(stored_state.main_score.clone()) {
        startup_tasks.push(Task::done(Message::File(FileMessage::Picked(Some(path)))));
    }

    (app, Task::batch(startup_tasks))
}
