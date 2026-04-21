use super::*;
use crate::app::piano_roll::roll_scroll_id;

impl Lilypalooza {
    pub(in crate::app) fn handle_piano_roll_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Task<Message> {
        let mut task = Task::none();
        let focus_only_click = matches!(message, PianoRollMessage::SetCursorTicks(_))
            && self.focused_workspace_pane != Some(WorkspacePaneKind::PianoRoll);

        match message {
            PianoRollMessage::ZoomIn
            | PianoRollMessage::ZoomOut
            | PianoRollMessage::SmoothZoom(_)
            | PianoRollMessage::ResetZoom
            | PianoRollMessage::SetCursorTicks(_)
            | PianoRollMessage::SetRewindFlagTicks(_)
            | PianoRollMessage::BeatSubdivisionSliderChanged(_)
            | PianoRollMessage::BeatSubdivisionInputChanged(_)
            | PianoRollMessage::FilePrevious
            | PianoRollMessage::FileNext
            | PianoRollMessage::StartTrackRename(_)
            | PianoRollMessage::OpenTrackColorPickerForTrack(_)
            | PianoRollMessage::TrackRenameInputChanged(_)
            | PianoRollMessage::OpenTrackColorPicker
            | PianoRollMessage::SubmitTrackColor(_)
            | PianoRollMessage::PreviewTrackColor(_)
            | PianoRollMessage::CancelTrackRename
            | PianoRollMessage::CommitTrackRename
            | PianoRollMessage::TrackPanelToggle
            | PianoRollMessage::TrackPanelResizedBy(_)
            | PianoRollMessage::TrackMuteToggled(_)
            | PianoRollMessage::TrackSoloToggled(_) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
            }
            PianoRollMessage::TransportSeekNormalized(_)
            | PianoRollMessage::TransportSeekReleased
            | PianoRollMessage::TransportPlayPause
            | PianoRollMessage::TransportRewind
            | PianoRollMessage::TransportToggleMetronome
            | PianoRollMessage::TransportOpenMetronomeMenu
            | PianoRollMessage::TransportCloseMetronomeMenu
            | PianoRollMessage::TransportMetronomeGainChanged(_)
            | PianoRollMessage::TransportMetronomePitchChanged(_) => {}
            PianoRollMessage::ViewportCursorMoved(_)
            | PianoRollMessage::ViewportCursorLeft
            | PianoRollMessage::RollScrolled { .. } => {}
        }

        if focus_only_click {
            return task;
        }

        match message {
            PianoRollMessage::ViewportCursorMoved(position) => {
                self.piano_roll_viewport_cursor = Some(position);
            }
            PianoRollMessage::ViewportCursorLeft => {
                self.piano_roll_viewport_cursor = None;
            }
            PianoRollMessage::RollScrolled { x, y } => {
                self.piano_roll.set_horizontal_scroll(x);
                self.piano_roll.set_vertical_scroll(y);
            }
            PianoRollMessage::ZoomIn => {
                self.piano_roll.zoom_in();
                self.persist_settings();
            }
            PianoRollMessage::ZoomOut => {
                self.piano_roll.zoom_out();
                self.persist_settings();
            }
            PianoRollMessage::SmoothZoom(delta) => {
                let previous_zoom = self.piano_roll.zoom_x;
                let next_zoom = self.piano_roll.zoom_for_delta(delta);

                if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
                    return Task::none();
                }

                self.piano_roll.zoom_x = next_zoom;
                self.persist_settings();

                if let Some(cursor) = self.piano_roll_viewport_cursor {
                    let scale = next_zoom / previous_zoom.max(f32::EPSILON);
                    let anchored =
                        anchored_scroll(self.piano_roll.horizontal_scroll(), cursor.x, scale);
                    self.piano_roll.set_horizontal_scroll(anchored);
                    return self.restore_piano_roll_scroll();
                }
            }
            PianoRollMessage::ResetZoom => {
                self.piano_roll.reset_zoom();
                self.persist_settings();
            }
            PianoRollMessage::BeatSubdivisionSliderChanged(subdivision) => {
                self.piano_roll.set_beat_subdivision(subdivision);
                self.persist_settings();
            }
            PianoRollMessage::BeatSubdivisionInputChanged(input) => {
                self.piano_roll.set_beat_subdivision_input(input);
                self.persist_settings();
            }
            PianoRollMessage::FilePrevious => {
                self.piano_roll.select_previous_file();
                self.sync_playback_file();
            }
            PianoRollMessage::FileNext => {
                self.piano_roll.select_next_file();
                self.sync_playback_file();
            }
            PianoRollMessage::StartTrackRename(track_index) => {
                return self.start_track_rename(track_index, WorkspacePaneKind::PianoRoll);
            }
            PianoRollMessage::OpenTrackColorPickerForTrack(track_index) => {
                self.open_track_color_picker_for_track(track_index, WorkspacePaneKind::PianoRoll);
            }
            PianoRollMessage::TrackRenameInputChanged(value) => {
                self.update_track_rename_value(value);
            }
            PianoRollMessage::OpenTrackColorPicker => {
                self.open_track_color_picker();
            }
            PianoRollMessage::SubmitTrackColor(color) => {
                self.submit_track_color(color);
            }
            PianoRollMessage::PreviewTrackColor(color) => {
                self.preview_track_color(color);
            }
            PianoRollMessage::CommitTrackRename => {
                return self.commit_track_rename();
            }
            PianoRollMessage::CancelTrackRename => {
                self.cancel_track_rename();
            }
            PianoRollMessage::TrackPanelToggle => {
                self.piano_roll.toggle_track_panel();
                task = self.restore_piano_roll_scroll();
            }
            PianoRollMessage::TrackPanelResizedBy(delta) => {
                self.piano_roll.resize_track_panel_by(delta);
            }
            PianoRollMessage::TrackMuteToggled(track_index) => {
                if let Some(muted) = self.piano_roll.toggle_track_mute(track_index)
                    && let Some(playback) = self.playback.as_mut()
                {
                    let _ = playback
                        .mixer()
                        .set_track_muted(lilypalooza_audio::TrackId(track_index as u16), muted);
                }
            }
            PianoRollMessage::TrackSoloToggled(track_index) => {
                if let Some(soloed) = self.piano_roll.toggle_track_solo(track_index)
                    && let Some(playback) = self.playback.as_mut()
                {
                    let mut mixer = playback.mixer();
                    let _ = mixer
                        .set_track_soloed(lilypalooza_audio::TrackId(track_index as u16), soloed);
                    self.piano_roll.set_global_solo_active(
                        mixer.tracks().iter().any(|track| track.state.soloed)
                            || mixer.buses().iter().any(|bus| bus.state.soloed),
                    );
                }
            }
            PianoRollMessage::SetCursorTicks(tick) => {
                self.seek_playback_ticks(tick);
            }
            PianoRollMessage::SetRewindFlagTicks(tick) => {
                self.piano_roll.set_rewind_flag_tick(tick);
            }
            PianoRollMessage::TransportSeekNormalized(position) => {
                self.transport_seek_preview = Some(position.clamp(0.0, 1.0));
            }
            PianoRollMessage::TransportSeekReleased => {
                if let Some(position) = self.transport_seek_preview.take() {
                    self.seek_playback_normalized(position);
                }
            }
            PianoRollMessage::TransportPlayPause => {
                self.transport_seek_preview = None;
                if let Some(playback) = self.playback.as_mut() {
                    let is_playing = self.piano_roll.playback_is_playing();

                    if is_playing {
                        playback.transport().pause_immediate();
                    } else {
                        playback.transport().play_immediate();
                    }

                    let current_tick = self.piano_roll.playback_tick();
                    let total_ticks = self.current_midi_total_ticks();
                    self.piano_roll
                        .set_playback_position(current_tick, total_ticks, !is_playing);
                } else {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "Playback Error",
                            "No SoundFont configured. Set playback.soundfont in settings.toml or use --soundfont",
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                }
            }
            PianoRollMessage::TransportRewind => {
                self.transport_seek_preview = None;
                let target_tick = self.rewind_target_tick();

                self.seek_playback_ticks(target_tick);
            }
            PianoRollMessage::TransportToggleMetronome => {
                self.metronome.enabled = !self.metronome.enabled;
                self.apply_metronome_state_to_playback();
                self.persist_settings();
            }
            PianoRollMessage::TransportOpenMetronomeMenu => {
                self.metronome_menu_open = true;
            }
            PianoRollMessage::TransportCloseMetronomeMenu => {
                self.metronome_menu_open = false;
            }
            PianoRollMessage::TransportMetronomeGainChanged(gain_db) => {
                self.metronome.gain_db = gain_db.clamp(-36.0, 6.0);
                self.apply_metronome_state_to_playback();
                self.persist_settings();
            }
            PianoRollMessage::TransportMetronomePitchChanged(pitch) => {
                self.metronome.pitch = pitch.clamp(0.0, 1.0);
                self.apply_metronome_state_to_playback();
                self.persist_settings();
            }
        }

        task
    }

    pub(in crate::app) fn restore_piano_roll_scroll(&self) -> Task<Message> {
        iced::widget::operation::scroll_to(
            roll_scroll_id(),
            iced::widget::operation::AbsoluteOffset {
                x: Some(self.piano_roll.horizontal_scroll()),
                y: Some(self.piano_roll.vertical_scroll()),
            },
        )
    }

    pub(in crate::app) fn apply_initial_piano_roll_center_if_needed(&mut self) -> Task<Message> {
        if !self.piano_roll.visible
            || !self.piano_roll.pending_initial_center()
            || self.piano_roll.current_file().is_none()
        {
            return Task::none();
        }

        self.piano_roll.mark_initial_center_applied();
        iced::widget::operation::snap_to(
            roll_scroll_id(),
            iced::widget::operation::RelativeOffset {
                x: None,
                y: Some(0.5),
            },
        )
    }
}
