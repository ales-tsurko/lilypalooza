use iced::Color;
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::{focus, select_all};

use super::*;
use crate::app::RenameTarget;
use crate::track_names::{effective_track_name, normalized_track_name_override};

impl Lilypalooza {
    fn resolved_track_color(&self, track_index: usize) -> Color {
        crate::track_colors::effective_track_color(
            track_index,
            self.track_color_override(track_index),
        )
    }

    pub(in crate::app) fn effective_track_name(&self, track_index: usize) -> String {
        effective_track_name(
            track_index,
            self.current_midi_track_label(track_index),
            self.track_name_override(track_index),
        )
    }

    pub(in crate::app) fn effective_track_color(&self, track_index: usize) -> Color {
        if self.renaming_target == Some(RenameTarget::Track(track_index)) {
            return self.track_rename_color_value;
        }
        if self.track_color_picker_target == Some((track_index, WorkspacePaneKind::PianoRoll)) {
            return self.track_rename_color_value;
        }
        self.resolved_track_color(track_index)
    }

    pub(in crate::app) fn current_midi_track_label(&self, track_index: usize) -> Option<&str> {
        self.piano_roll
            .current_file()
            .and_then(|file| {
                file.data
                    .tracks
                    .iter()
                    .find(|track| track.index == track_index)
            })
            .map(|track| track.label.as_str())
    }

    pub(in crate::app) fn track_name_override(&self, track_index: usize) -> Option<&str> {
        self.track_name_overrides
            .get(track_index)
            .and_then(|name| name.as_deref())
    }

    pub(in crate::app) fn track_color_override(&self, track_index: usize) -> Option<Color> {
        self.track_color_overrides
            .get(track_index)
            .copied()
            .flatten()
    }

    pub(in crate::app) fn start_track_rename(
        &mut self,
        track_index: usize,
        origin: WorkspacePaneKind,
    ) -> Task<Message> {
        self.start_rename(
            RenameTarget::Track(track_index),
            origin,
            self.effective_track_name(track_index),
        )
    }

    pub(in crate::app) fn start_bus_rename(
        &mut self,
        bus_id: u16,
        origin: WorkspacePaneKind,
        name: String,
    ) -> Task<Message> {
        self.start_rename(RenameTarget::Bus(bus_id), origin, name)
    }

    fn start_rename(
        &mut self,
        target: RenameTarget,
        origin: WorkspacePaneKind,
        name: String,
    ) -> Task<Message> {
        if self.renaming_target.is_some() && self.renaming_target != Some(target) {
            self.apply_pending_track_rename();
        }
        self.renaming_target = Some(target);
        self.renaming_origin = Some(origin);
        self.track_color_picker_target = None;
        self.track_rename_was_focused = false;
        self.track_rename_value = name;
        self.track_rename_color_picker_open = false;
        self.track_rename_color_value = match target {
            RenameTarget::Track(track_index) => self.resolved_track_color(track_index),
            RenameTarget::Bus(_) => Color::TRANSPARENT,
        };
        Task::batch([
            focus(Id::new(super::super::TRACK_RENAME_INPUT_ID)),
            select_all(Id::new(super::super::TRACK_RENAME_INPUT_ID)),
        ])
    }

    pub(in crate::app) fn update_track_rename_value(&mut self, value: String) {
        self.track_rename_value = value
            .chars()
            .take(crate::track_names::MAX_TRACK_NAME_LEN)
            .collect();
    }

    pub(in crate::app) fn commit_track_rename(&mut self) -> Task<Message> {
        self.apply_pending_track_rename();
        Task::none()
    }

    pub(in crate::app) fn cancel_track_rename(&mut self) {
        if let Some(RenameTarget::Track(track_index)) = self.renaming_target {
            self.track_rename_color_value = self.resolved_track_color(track_index);
        }
        if let Some((track_index, _)) = self.track_color_picker_target {
            self.track_rename_color_value = self.resolved_track_color(track_index);
        }
        self.renaming_target = None;
        self.renaming_origin = None;
        self.track_color_picker_target = None;
        self.track_rename_was_focused = false;
        self.track_rename_value.clear();
        self.track_rename_color_picker_open = false;
    }

    pub(in crate::app) fn apply_pending_track_rename(&mut self) {
        let Some(target) = self.renaming_target.take() else {
            return;
        };
        self.renaming_origin = None;

        match target {
            RenameTarget::Track(track_index) => {
                self.track_name_overrides.resize(track_index + 1, None);
                self.track_name_overrides[track_index] =
                    normalized_track_name_override(&self.track_rename_value);
                self.track_rename_was_focused = false;
                self.track_rename_value.clear();
                self.track_rename_color_picker_open = false;
                self.track_color_picker_target = None;
                self.sync_track_name_to_playback(track_index);
                self.persist_settings();
            }
            RenameTarget::Bus(bus_id) => {
                let name = self.track_rename_value.trim().to_string();
                self.track_rename_was_focused = false;
                self.track_rename_value.clear();
                self.track_rename_color_picker_open = false;
                self.track_color_picker_target = None;
                if !name.is_empty()
                    && let Some(playback) = self.playback.as_mut()
                {
                    let _ = playback
                        .mixer()
                        .set_bus_name(lilypalooza_audio::BusId(bus_id), name);
                }
            }
        }
    }

    pub(in crate::app) fn handle_track_rename_focus_changed(
        &mut self,
        focused: bool,
    ) -> Task<Message> {
        if self.renaming_target.is_none() {
            self.track_rename_was_focused = false;
            return Task::none();
        }

        if focused {
            self.track_rename_was_focused = true;
            return Task::none();
        }

        if self.track_rename_color_picker_open {
            return Task::none();
        }

        if self.track_rename_was_focused {
            return self.commit_track_rename();
        }

        Task::none()
    }

    pub(in crate::app) fn is_renaming_track_in(
        &self,
        track_index: usize,
        pane: WorkspacePaneKind,
    ) -> bool {
        self.renaming_target == Some(RenameTarget::Track(track_index))
            && self.renaming_origin == Some(pane)
    }

    pub(in crate::app) fn is_picking_track_color_in(
        &self,
        track_index: usize,
        pane: WorkspacePaneKind,
    ) -> bool {
        self.track_color_picker_target == Some((track_index, pane))
    }

    pub(in crate::app) fn sync_track_name_to_playback(&mut self, track_index: usize) {
        let name = self.effective_track_name(track_index);
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let _ = playback
            .mixer()
            .set_track_name(lilypalooza_audio::TrackId(track_index as u16), name);
    }

    pub(in crate::app) fn open_track_color_picker(&mut self) {
        self.track_rename_color_picker_open = true;
        if let Some(RenameTarget::Track(track_index)) = self.renaming_target {
            self.track_rename_color_value = self.resolved_track_color(track_index);
        }
    }

    pub(in crate::app) fn open_track_color_picker_for_track(
        &mut self,
        track_index: usize,
        origin: WorkspacePaneKind,
    ) {
        if self.renaming_target.is_some() {
            self.apply_pending_track_rename();
        }
        self.track_color_picker_target = Some((track_index, origin));
        self.track_rename_color_picker_open = true;
        self.track_rename_color_value = self.resolved_track_color(track_index);
    }

    pub(in crate::app) fn submit_track_color(&mut self, color: Color) {
        self.track_rename_color_picker_open = false;
        self.set_track_color_override(color);
        self.track_color_picker_target = None;
    }

    pub(in crate::app) fn preview_track_color(&mut self, color: Color) {
        self.track_rename_color_value = color;
    }

    fn set_track_color_override(&mut self, color: Color) {
        let track_index = match self.renaming_target {
            Some(RenameTarget::Track(track_index)) => Some(track_index),
            _ => self
                .track_color_picker_target
                .map(|(track_index, _)| track_index),
        };
        let Some(track_index) = track_index else {
            return;
        };

        self.track_rename_color_value = color;
        self.track_color_overrides.resize(track_index + 1, None);
        self.track_color_overrides[track_index] = Some(color);
        self.persist_settings();
    }
}
