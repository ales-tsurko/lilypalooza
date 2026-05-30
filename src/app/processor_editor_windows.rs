#[cfg(test)]
use std::sync::Mutex;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use editor_host::{
    EditorFrameCommand, EditorPresetState, InstalledHost, InstalledHostResizeHandle, WindowSnapshot,
};
use iced::window;
#[cfg(test)]
use lilypalooza_audio::{Controller, ControllerError, ProcessorDescriptor, ProcessorState};
use lilypalooza_audio::{
    EditorError, EditorParent, EditorResizeHandler, EditorSession, EditorSize,
};

use crate::app::SharedController;

mod manager_api;
mod resize_helpers;
mod window_state;
mod zoom_and_deferred_resize;

use resize_helpers::*;
pub(in crate::app) use window_state::*;
use zoom_and_deferred_resize::*;
#[cfg(test)]
mod window_manager_tests;
