use iced::Task;
use iced::widget::Id;
use iced::widget::operation::{focus, select_all};

use super::*;
use crate::app::RenameTarget;
use crate::track_names::{effective_track_name, normalized_track_name_override};

impl Lilypalooza {
    pub(in crate::app) fn effective_track_name(&self, track_index: usize) -> String {
        effective_track_name(
            track_index,
            self.current_midi_track_label(track_index),
            self.track_name_override(track_index),
        )
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

    pub(in crate::app) fn start_track_rename(&mut self, track_index: usize) -> Task<Message> {
        self.start_rename(
            RenameTarget::Track(track_index),
            self.effective_track_name(track_index),
        )
    }

    pub(in crate::app) fn start_bus_rename(&mut self, bus_id: u16, name: String) -> Task<Message> {
        self.start_rename(RenameTarget::Bus(bus_id), name)
    }

    fn start_rename(&mut self, target: RenameTarget, name: String) -> Task<Message> {
        if self.renaming_target.is_some() && self.renaming_target != Some(target) {
            self.apply_pending_track_rename();
        }
        self.renaming_target = Some(target);
        self.track_rename_was_focused = false;
        self.track_rename_value = name;
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
        self.renaming_target = None;
        self.track_rename_was_focused = false;
        self.track_rename_value.clear();
    }

    pub(in crate::app) fn apply_pending_track_rename(&mut self) {
        let Some(target) = self.renaming_target.take() else {
            return;
        };

        match target {
            RenameTarget::Track(track_index) => {
                self.track_name_overrides.resize(track_index + 1, None);
                self.track_name_overrides[track_index] =
                    normalized_track_name_override(&self.track_rename_value);
                self.track_rename_was_focused = false;
                self.track_rename_value.clear();
                self.sync_track_name_to_playback(track_index);
                self.persist_settings();
            }
            RenameTarget::Bus(bus_id) => {
                let name = self.track_rename_value.trim().to_string();
                self.track_rename_was_focused = false;
                self.track_rename_value.clear();
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

        if self.track_rename_was_focused {
            return self.commit_track_rename();
        }

        Task::none()
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
}
