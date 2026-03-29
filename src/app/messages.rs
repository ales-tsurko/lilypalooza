use std::path::PathBuf;

use iced::time::Instant;
use iced::widget::{pane_grid, text_editor};
use iced::{Size, keyboard, mouse};

use super::ScorePaneKind;

#[derive(Debug, Clone)]
pub(super) enum Message {
    StartupChecked(Result<crate::lilypond::VersionCheck, String>),
    Pane(PaneMessage),
    File(FileMessage),
    Viewer(ViewerMessage),
    PianoRoll(PianoRollMessage),
    Logger(LoggerMessage),
    Prompt(PromptMessage),
    ModifiersChanged(keyboard::Modifiers),
    Tick,
    Frame(Instant),
    WindowResized(Size),
}

#[derive(Debug, Clone)]
pub(super) enum PaneMessage {
    Resized(pane_grid::ResizeEvent),
    ScoreDragged(pane_grid::DragEvent),
    StackedTabPressed(ScorePaneKind),
    StackedTabHovered(Option<ScorePaneKind>),
    StackedTabDragStarted(ScorePaneKind),
    StackedDragMoved(iced::Point),
    StackedDragReleased,
    StackedDragExited,
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
    Resized(pane_grid::ResizeEvent),
    ToggleVisible,
    ViewportCursorMoved(iced::Point),
    ViewportCursorLeft,
    RollScrolled { x: f32, y: f32 },
    SetCursorTicks(u64),
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
