use editor_host::{
    EditorFrameCommand, EditorHostOptions, EditorPresetItem, EditorPresetOrigin, EditorPresetState,
    InstalledHost, WindowSnapshot, route_app_quit_to_window_close,
};
use iced::window;
use lilypalooza_audio::{
    BUILTIN_SOUNDFONT_ID, BusId, BusSend, EditorParent, ProcessorKind, SlotState, TrackId,
};
use lilypalooza_builtins::soundfont_synth::{self, SoundfontProcessorState};

use super::{super::messages::MixerMessage, *};
use crate::app::{
    mixer::{
        ProcessorBrowserBackend, ProcessorBrowserSectionKey, ProcessorChoice, ProcessorSlotSegment,
    },
    processor_editor_windows::{EditorTarget, snapshot_into_editor_parent},
};

mod editor_windows;
mod helpers;
mod history;
mod more_helpers;
mod playback_apply;

use helpers::*;
use more_helpers::*;

#[cfg(test)]
mod tests_editor_windows;
#[cfg(test)]
mod tests_processors;
#[cfg(test)]
mod tests_routing;
