use std::path::PathBuf;

use iced::time::Instant;
use iced::widget::{pane_grid, text_editor};
use iced::{Size, keyboard, mouse};

use super::WorkspacePaneKind;

#[derive(Debug, Clone)]
pub(super) enum Message {
    StartupChecked(Result<crate::lilypond::VersionCheck, String>),
    Pane(PaneMessage),
    File(FileMessage),
    Viewer(ViewerMessage),
    PianoRoll(PianoRollMessage),
    Editor(EditorMessage),
    Logger(LoggerMessage),
    Prompt(PromptMessage),
    ModifiersChanged(keyboard::Modifiers),
    Tick,
    Frame(Instant),
    WindowResized(Size),
}

#[derive(Debug, Clone)]
pub(super) enum PaneMessage {
    LoggerResized(pane_grid::ResizeEvent),
    WorkspaceResized(pane_grid::ResizeEvent),
    WorkspaceTabPressed(WorkspacePaneKind),
    WorkspaceTabHovered(Option<WorkspacePaneKind>),
    FoldWorkspacePane(WorkspacePaneKind),
    UnfoldWorkspacePane(WorkspacePaneKind),
    WorkspaceDragMoved(iced::Point),
    WorkspaceDragReleased,
    WorkspaceDragExited,
    ToggleLogger,
}

#[derive(Debug, Clone)]
pub(super) enum FileMessage {
    RequestOpen,
    Picked(Option<PathBuf>),
    RequestSoundfont,
    SoundfontPicked(Option<PathBuf>),
}

#[derive(Debug, Clone)]
pub(super) enum EditorMessage {
    Action(text_editor::Action),
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
    TransportPlayPause,
    TransportRewind,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PromptMessage {
    Acknowledge,
    Cancel,
}
