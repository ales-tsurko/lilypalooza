use super::*;
use crate::app::piano_roll::roll_scroll_id;

impl LilyView {
    pub(in crate::app) fn handle_piano_roll_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Task<Message> {
        let mut task = Task::none();

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
            | PianoRollMessage::TrackPanelToggle
            | PianoRollMessage::TrackPanelResizedBy(_)
            | PianoRollMessage::TrackMuteToggled(_)
            | PianoRollMessage::TrackSoloToggled(_) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
            }
            PianoRollMessage::TransportSeekNormalized(_)
            | PianoRollMessage::TransportSeekReleased
            | PianoRollMessage::TransportPlayPause
            | PianoRollMessage::TransportRewind => {}
            PianoRollMessage::ViewportCursorMoved(_)
            | PianoRollMessage::ViewportCursorLeft
            | PianoRollMessage::RollScrolled { .. } => {}
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
                    let _ = playback.set_track_muted(track_index, muted);
                }
            }
            PianoRollMessage::TrackSoloToggled(track_index) => {
                if let Some(soloed) = self.piano_roll.toggle_track_solo(track_index)
                    && let Some(playback) = self.playback.as_mut()
                {
                    let _ = playback.set_track_solo(track_index, soloed);
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
                    if playback.is_playing() {
                        playback.pause();
                    } else {
                        let _ = playback.play();
                    }

                    self.refresh_playback_position();
                } else {
                    self.show_prompt(
                        ErrorPrompt::new(
                            "Playback Error",
                            "No soundfont selected. Choose a SoundFont file first",
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

                if let Some(playback) = self.playback.as_mut() {
                    let was_playing = playback.is_playing();
                    playback.jump_to_tick(target_tick);
                    if was_playing {
                        let _ = playback.play();
                    }
                    self.refresh_playback_position();
                } else {
                    self.seek_playback_ticks(target_tick);
                }
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
