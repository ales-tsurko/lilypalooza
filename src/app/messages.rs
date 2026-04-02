use std::path::PathBuf;

use iced::event;
use iced::time::Instant;
use iced::widget::{pane_grid, text_editor};
use iced::{Size, keyboard, mouse};
use iced_code_editor::Message as EditorWidgetMessage;
use iced_core::image;

use super::{EditorFileMenuSection, EditorHeaderMenuSection, WorkspacePaneKind};

#[derive(Debug, Clone)]
pub(super) enum Message {
    StartupChecked(Result<crate::lilypond::VersionCheck, String>),
    Pane(PaneMessage),
    File(FileMessage),
    Viewer(ViewerMessage),
    ScorePreviewReady(Result<ScorePreviewReady, String>),
    PianoRoll(PianoRollMessage),
    Editor(EditorMessage),
    Logger(LoggerMessage),
    Prompt(PromptMessage),
    KeyPressed(KeyPress),
    ModifiersChanged(keyboard::Modifiers),
    Tick,
    Frame(Instant),
    WindowResized(Size),
    WindowCloseRequested,
}

#[derive(Debug, Clone)]
pub(super) struct KeyPress {
    pub(super) status: event::Status,
    pub(super) key: keyboard::Key,
    pub(super) physical_key: keyboard::key::Physical,
    pub(super) modifiers: keyboard::Modifiers,
}

#[derive(Debug, Clone)]
pub(super) struct ScorePreviewReady {
    pub(super) page_index: usize,
    pub(super) zoom: f32,
    pub(super) tier: super::ScoreZoomPreviewTier,
    pub(super) handle: image::Handle,
}

#[derive(Debug, Clone)]
pub(super) enum PaneMessage {
    WorkspaceResized(pane_grid::ResizeEvent),
    WorkspaceTabPressed(WorkspacePaneKind),
    FocusWorkspacePane(WorkspacePaneKind),
    WorkspaceTabHovered(Option<WorkspacePaneKind>),
    OpenHeaderOverflowMenu(u64),
    SetEditorHeaderMenuSection(Option<EditorHeaderMenuSection>),
    HoverEditorFileMenuSection {
        section: Option<EditorFileMenuSection>,
        expanded: bool,
    },
    ToggleProjectMenu,
    CloseProjectMenu,
    SetProjectRecentOpen(bool),
    CloseHeaderOverflowMenu,
    ToggleWorkspacePane(WorkspacePaneKind),
    WorkspaceDragMoved(iced::Point),
    WorkspaceDragReleased,
    WorkspaceDragExited,
}

#[derive(Debug, Clone)]
pub(super) enum FileMessage {
    RequestOpen,
    Picked(Option<PathBuf>),
    RequestCreateProject,
    RequestSaveProject,
    RequestLoadProject,
    CreateProjectPicked(Option<PathBuf>),
    LoadProjectPicked(Option<PathBuf>),
    OpenRecentProject(PathBuf),
    RequestSoundfont,
    SoundfontPicked(Option<PathBuf>),
}

#[derive(Debug, Clone)]
pub(super) enum EditorMessage {
    Widget {
        tab_id: u64,
        message: EditorWidgetMessage,
    },
    NewRequested,
    TabPressed(u64),
    TabMoved {
        tab_id: u64,
        position: iced::Point,
    },
    TabGlobalMoved(iced::Point),
    TabHovered(Option<u64>),
    TabBarMoved(iced::Point),
    TabBarScrolled(iced::widget::scrollable::Viewport),
    TabBarEmptyMoved,
    StartRename(u64),
    RenameInputChanged(String),
    CommitRename,
    CancelRename,
    TabDragReleased,
    TabDragExited,
    CloseTabRequested(u64),
    RenameRequested,
    RenamePicked(Option<PathBuf>),
    OpenRequested,
    OpenPicked(Option<Vec<PathBuf>>),
    OpenRecent(PathBuf),
    SaveRequested,
    SaveAsRequested,
    SaveAsPicked(Option<PathBuf>),
    ZoomIn,
    ZoomOut,
    ResetZoom,
    SetThemeHueOffsetDegrees(f32),
    SetThemeSaturation(f32),
    SetThemeWarmth(f32),
    SetThemeBrightness(f32),
    SetThemeTextDim(f32),
    SetThemeCommentDim(f32),
}

#[derive(Debug, Clone)]
pub(super) enum LoggerMessage {
    RequestClear,
    TextAction(text_editor::Action),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ViewerMessage {
    ScrollUp,
    ScrollDown,
    ScrollPositionChanged { x: f32, y: f32 },
    ViewportCursorMoved(iced::Point),
    ViewportCursorLeft,
    PrevPage,
    NextPage,
    ZoomIn,
    ZoomOut,
    SmoothZoom(mouse::ScrollDelta),
    DecreasePageBrightness,
    IncreasePageBrightness,
    ResetZoom,
    ResetPageBrightness,
}

#[derive(Debug, Clone)]
pub(super) enum PianoRollMessage {
    ViewportCursorMoved(iced::Point),
    ViewportCursorLeft,
    RollScrolled { x: f32, y: f32 },
    SetCursorTicks(u64),
    SetRewindFlagTicks(u64),
    ZoomIn,
    ZoomOut,
    SmoothZoom(mouse::ScrollDelta),
    ResetZoom,
    BeatSubdivisionSliderChanged(u8),
    BeatSubdivisionInputChanged(String),
    FilePrevious,
    FileNext,
    TrackPanelToggle,
    TrackPanelResizedBy(f32),
    TrackMuteToggled(usize),
    TrackSoloToggled(usize),
    TransportSeekNormalized(f32),
    TransportSeekReleased,
    TransportPlayPause,
    TransportRewind,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PromptMessage {
    Acknowledge,
    Discard,
    Cancel,
}
