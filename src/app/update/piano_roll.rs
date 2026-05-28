use super::*;
use crate::app::piano_roll::roll_scroll_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PianoRollMixToggle {
    Mute,
    Solo,
}

enum PianoRollRoute {
    Viewport(PianoRollViewportRoute),
    Zoom(PianoRollMessage),
    Timeline(PianoRollMessage),
    Track(PianoRollTrackRoute),
    Transport(PianoRollMessage),
}

enum PianoRollViewportRoute {
    CursorMoved(iced::Point),
    CursorLeft,
    Scrolled { x: f32, y: f32 },
}

enum PianoRollTrackRoute {
    Select(usize),
    Edit(PianoRollTrackEditRoute),
    Panel(PianoRollTrackPanelRoute),
}

enum PianoRollTrackEditRoute {
    RenameEdit(PianoRollMessage),
    RenameFinish(PianoRollMessage),
    Color(PianoRollMessage),
}

enum PianoRollTrackPanelRoute {
    Toggle,
    Resize(f32),
    ToggleMute(usize),
    ToggleSolo(usize),
}

enum PianoRollFileStep {
    Previous,
    Next,
}

impl PianoRollFileStep {
    fn from_message(message: &PianoRollMessage) -> Option<Self> {
        match message {
            PianoRollMessage::FilePrevious => Some(Self::Previous),
            PianoRollMessage::FileNext => Some(Self::Next),
            _ => None,
        }
    }
}

impl From<PianoRollMessage> for PianoRollRoute {
    fn from(message: PianoRollMessage) -> Self {
        match message {
            PianoRollMessage::ViewportCursorMoved(position) => {
                Self::Viewport(PianoRollViewportRoute::CursorMoved(position))
            }
            PianoRollMessage::ViewportCursorLeft => {
                Self::Viewport(PianoRollViewportRoute::CursorLeft)
            }
            PianoRollMessage::RollScrolled { x, y } => {
                Self::Viewport(PianoRollViewportRoute::Scrolled { x, y })
            }
            PianoRollMessage::ZoomIn
            | PianoRollMessage::ZoomOut
            | PianoRollMessage::SmoothZoom(_)
            | PianoRollMessage::ResetZoom => Self::Zoom(message),
            PianoRollMessage::SetCursorTicks(_)
            | PianoRollMessage::SetRewindFlagTicks(_)
            | PianoRollMessage::BeatSubdivisionSliderChanged(_)
            | PianoRollMessage::BeatSubdivisionInputChanged(_)
            | PianoRollMessage::FilePrevious
            | PianoRollMessage::FileNext => Self::Timeline(message),
            PianoRollMessage::SelectTrack(track_index) => {
                Self::Track(PianoRollTrackRoute::Select(track_index))
            }
            PianoRollMessage::StartTrackRename(_)
            | PianoRollMessage::TrackRenameInputChanged(_) => Self::Track(
                PianoRollTrackRoute::Edit(PianoRollTrackEditRoute::RenameEdit(message)),
            ),
            PianoRollMessage::CommitTrackRename | PianoRollMessage::CancelTrackRename => {
                Self::Track(PianoRollTrackRoute::Edit(
                    PianoRollTrackEditRoute::RenameFinish(message),
                ))
            }
            PianoRollMessage::OpenTrackColorPickerForTrack(_)
            | PianoRollMessage::OpenTrackColorPicker
            | PianoRollMessage::SubmitTrackColor(_)
            | PianoRollMessage::PreviewTrackColor(_) => Self::Track(PianoRollTrackRoute::Edit(
                PianoRollTrackEditRoute::Color(message),
            )),
            PianoRollMessage::TrackPanelToggle => {
                Self::Track(PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::Toggle))
            }
            PianoRollMessage::TrackPanelResizedBy(delta) => Self::Track(
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::Resize(delta)),
            ),
            PianoRollMessage::TrackMuteToggled(track_index) => Self::Track(
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::ToggleMute(track_index)),
            ),
            PianoRollMessage::TrackSoloToggled(track_index) => Self::Track(
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::ToggleSolo(track_index)),
            ),
            PianoRollMessage::TransportSeekNormalized(_)
            | PianoRollMessage::TransportSeekReleased
            | PianoRollMessage::TransportPlayPause
            | PianoRollMessage::TransportRewind
            | PianoRollMessage::TransportToggleMetronome
            | PianoRollMessage::TransportOpenMetronomeMenu
            | PianoRollMessage::TransportCloseMetronomeMenu
            | PianoRollMessage::TransportMetronomeGainChanged(_)
            | PianoRollMessage::TransportMetronomePitchChanged(_) => Self::Transport(message),
        }
    }
}

impl PianoRollMixToggle {
    fn apply_to_piano_roll(
        self,
        piano_roll: &mut crate::app::piano_roll::PianoRollState,
        track_index: usize,
    ) -> Option<bool> {
        match self {
            Self::Mute => piano_roll.toggle_track_mute(track_index),
            Self::Solo => piano_roll.toggle_track_solo(track_index),
        }
    }

    fn apply_to_mixer(
        self,
        mixer: &mut lilypalooza_audio::MixerHandle<'_>,
        track_index: usize,
        enabled: bool,
    ) -> Result<(), lilypalooza_audio::AudioEngineError> {
        let track_id = lilypalooza_audio::TrackId(track_index as u16);
        match self {
            Self::Mute => mixer.set_track_muted(track_id, enabled),
            Self::Solo => mixer.set_track_soloed(track_id, enabled),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mute => "mute",
            Self::Solo => "solo",
        }
    }
}

impl Lilypalooza {
    pub(in crate::app) fn handle_piano_roll_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Task<Message> {
        match PianoRollRoute::from(message) {
            PianoRollRoute::Viewport(route) => self.handle_piano_roll_viewport_route(route),
            PianoRollRoute::Zoom(message) => self.handle_piano_roll_zoom_message(message),
            PianoRollRoute::Timeline(message) => self.handle_piano_roll_timeline_message(message),
            PianoRollRoute::Track(route) => self.handle_piano_roll_track_route(route),
            PianoRollRoute::Transport(message) => self.handle_piano_roll_transport_message(message),
        }
    }

    fn handle_piano_roll_viewport_route(&mut self, route: PianoRollViewportRoute) -> Task<Message> {
        match route {
            PianoRollViewportRoute::CursorMoved(position) => {
                self.piano_roll_viewport_cursor = Some(position);
            }
            PianoRollViewportRoute::CursorLeft => {
                self.piano_roll_viewport_cursor = None;
            }
            PianoRollViewportRoute::Scrolled { x, y } => {
                self.piano_roll.set_horizontal_scroll(x);
                self.piano_roll.set_vertical_scroll(y);
            }
        }
        Task::none()
    }

    fn handle_piano_roll_zoom_message(&mut self, message: PianoRollMessage) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
        self.apply_piano_roll_step_zoom(&message)
            .or_else(|| self.apply_piano_roll_smooth_zoom(message))
            .unwrap_or_else(Task::none)
    }

    fn apply_piano_roll_step_zoom(&mut self, message: &PianoRollMessage) -> Option<Task<Message>> {
        match message {
            PianoRollMessage::ZoomIn => self.piano_roll.zoom_in(),
            PianoRollMessage::ZoomOut => self.piano_roll.zoom_out(),
            PianoRollMessage::ResetZoom => self.piano_roll.reset_zoom(),
            _ => return None,
        }
        self.persist_settings();
        Some(Task::none())
    }

    fn apply_piano_roll_smooth_zoom(&mut self, message: PianoRollMessage) -> Option<Task<Message>> {
        let PianoRollMessage::SmoothZoom(delta) = message else {
            return None;
        };
        Some(self.smooth_zoom_piano_roll(delta))
    }

    fn smooth_zoom_piano_roll(&mut self, delta: iced::mouse::ScrollDelta) -> Task<Message> {
        let previous_zoom = self.piano_roll.zoom_x;
        let next_zoom = self.piano_roll.zoom_for_delta(delta);

        if (next_zoom - previous_zoom).abs() <= f32::EPSILON {
            return Task::none();
        }

        self.piano_roll.zoom_x = next_zoom;
        self.persist_settings();
        self.anchor_piano_roll_zoom(previous_zoom, next_zoom)
    }

    fn anchor_piano_roll_zoom(&mut self, previous_zoom: f32, next_zoom: f32) -> Task<Message> {
        let Some(cursor) = self.piano_roll_viewport_cursor else {
            return Task::none();
        };
        let scale = next_zoom / previous_zoom.max(f32::EPSILON);
        let anchored = anchored_scroll(self.piano_roll.horizontal_scroll(), cursor.x, scale);
        self.piano_roll.set_horizontal_scroll(anchored);
        self.restore_piano_roll_scroll()
    }

    fn handle_piano_roll_timeline_message(&mut self, message: PianoRollMessage) -> Task<Message> {
        let already_focused = self.focused_workspace_pane == Some(WorkspacePaneKind::PianoRoll);
        self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
        if let Some(task) =
            self.handle_piano_roll_timeline_cursor_message(&message, already_focused)
        {
            return task;
        }
        self.handle_piano_roll_timeline_action_message(message)
            .unwrap_or_else(Task::none)
    }

    fn handle_piano_roll_timeline_cursor_message(
        &mut self,
        message: &PianoRollMessage,
        already_focused: bool,
    ) -> Option<Task<Message>> {
        match message {
            PianoRollMessage::SetCursorTicks(tick) => {
                if already_focused {
                    self.seek_playback_ticks(*tick);
                }
                Some(Task::none())
            }
            PianoRollMessage::SetRewindFlagTicks(tick) => {
                self.piano_roll.set_rewind_flag_tick(*tick);
                Some(Task::none())
            }
            _ => None,
        }
    }

    fn handle_piano_roll_timeline_action_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Option<Task<Message>> {
        if let Some(step) = PianoRollFileStep::from_message(&message) {
            self.step_piano_roll_file(step);
            return Some(Task::none());
        }

        match message {
            PianoRollMessage::BeatSubdivisionSliderChanged(subdivision) => {
                self.piano_roll.set_beat_subdivision(subdivision);
                self.persist_settings();
                Some(Task::none())
            }
            PianoRollMessage::BeatSubdivisionInputChanged(input) => {
                self.piano_roll.set_beat_subdivision_input(input);
                self.persist_settings();
                Some(Task::none())
            }
            _ => None,
        }
    }

    fn step_piano_roll_file(&mut self, step: PianoRollFileStep) {
        match step {
            PianoRollFileStep::Previous => self.piano_roll.select_previous_file(),
            PianoRollFileStep::Next => self.piano_roll.select_next_file(),
        }
        self.sync_playback_file();
    }

    fn handle_piano_roll_track_route(&mut self, route: PianoRollTrackRoute) -> Task<Message> {
        match route {
            PianoRollTrackRoute::Select(track_index) => self.select_track(
                track_index,
                super::track_selection::TrackSelectionOrigin::PianoRoll,
            ),
            PianoRollTrackRoute::Edit(route) => self.handle_piano_roll_track_edit_route(route),
            PianoRollTrackRoute::Panel(route) => self.handle_piano_roll_track_panel_route(route),
        }
    }

    fn handle_piano_roll_track_edit_route(
        &mut self,
        route: PianoRollTrackEditRoute,
    ) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
        match route {
            PianoRollTrackEditRoute::RenameEdit(message)
            | PianoRollTrackEditRoute::RenameFinish(message) => {
                self.handle_piano_roll_track_rename_message(message)
            }
            PianoRollTrackEditRoute::Color(message) => {
                self.handle_piano_roll_track_color(message);
                Task::none()
            }
        }
    }

    fn handle_piano_roll_track_panel_route(
        &mut self,
        route: PianoRollTrackPanelRoute,
    ) -> Task<Message> {
        self.set_focused_workspace_pane(WorkspacePaneKind::PianoRoll);
        match route {
            PianoRollTrackPanelRoute::Toggle => {
                self.piano_roll.toggle_track_panel();
                self.restore_piano_roll_scroll()
            }
            PianoRollTrackPanelRoute::Resize(delta) => {
                self.piano_roll.resize_track_panel_by(delta);
                Task::none()
            }
            PianoRollTrackPanelRoute::ToggleMute(track_index) => {
                self.toggle_piano_roll_track_mute(track_index);
                Task::none()
            }
            PianoRollTrackPanelRoute::ToggleSolo(track_index) => {
                self.toggle_piano_roll_track_solo(track_index);
                Task::none()
            }
        }
    }

    fn handle_piano_roll_track_rename_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Task<Message> {
        if let Some(track_index) = start_track_rename_message(&message) {
            return self.start_track_rename(track_index, WorkspacePaneKind::PianoRoll);
        }
        if let PianoRollMessage::TrackRenameInputChanged(value) = message {
            self.update_track_rename_value(value);
            return Task::none();
        }
        self.handle_piano_roll_track_rename_finish_message(message)
    }

    fn handle_piano_roll_track_rename_finish_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Task<Message> {
        match message {
            PianoRollMessage::CommitTrackRename => self.commit_track_rename(),
            PianoRollMessage::CancelTrackRename => {
                self.cancel_track_rename();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn handle_piano_roll_track_color(&mut self, message: PianoRollMessage) {
        if let Some(track_index) = track_color_picker_target(&message) {
            self.open_track_color_picker_for_track(track_index, WorkspacePaneKind::PianoRoll);
            return;
        }
        if let Some(color) = submit_track_color_message(&message) {
            self.submit_track_color(color);
            return;
        }
        if let Some(color) = preview_track_color_message(&message) {
            self.preview_track_color(color);
            return;
        }
        if matches!(message, PianoRollMessage::OpenTrackColorPicker) {
            self.open_track_color_picker();
        }
    }

    fn toggle_piano_roll_track_mute(&mut self, track_index: usize) {
        self.toggle_piano_roll_track_mix_state(track_index, PianoRollMixToggle::Mute);
    }

    fn toggle_piano_roll_track_solo(&mut self, track_index: usize) {
        self.toggle_piano_roll_track_mix_state(track_index, PianoRollMixToggle::Solo);
    }

    fn toggle_piano_roll_track_mix_state(
        &mut self,
        track_index: usize,
        toggle: PianoRollMixToggle,
    ) {
        let Some(enabled) = toggle.apply_to_piano_roll(&mut self.piano_roll, track_index) else {
            return;
        };
        let Some(playback) = self.playback.as_mut() else {
            return;
        };
        let mut mixer = playback.mixer();
        if let Err(error) = toggle.apply_to_mixer(&mut mixer, track_index, enabled) {
            self.logger.push(format!(
                "[mixer:error] Failed to set track {track_index} {} state: {error}",
                toggle.label(),
            ));
        }
        if toggle == PianoRollMixToggle::Solo {
            self.piano_roll.set_global_solo_active(
                mixer.tracks().iter().any(|track| track.state.soloed)
                    || mixer.buses().iter().any(|bus| bus.state.soloed),
            );
        }
    }

    fn handle_piano_roll_transport_message(&mut self, message: PianoRollMessage) -> Task<Message> {
        self.handle_piano_roll_seek_transport_message(&message)
            .or_else(|| self.handle_piano_roll_playback_transport_message(&message))
            .or_else(|| self.handle_piano_roll_metronome_transport_message(message));
        Task::none()
    }

    fn handle_piano_roll_seek_transport_message(
        &mut self,
        message: &PianoRollMessage,
    ) -> Option<()> {
        match message {
            PianoRollMessage::TransportSeekNormalized(position) => {
                self.transport_seek_preview = Some(position.clamp(0.0, 1.0));
            }
            PianoRollMessage::TransportSeekReleased => {
                if let Some(position) = self.transport_seek_preview.take() {
                    self.seek_playback_normalized(position);
                }
            }
            _ => return None,
        }
        Some(())
    }

    fn handle_piano_roll_playback_transport_message(
        &mut self,
        message: &PianoRollMessage,
    ) -> Option<()> {
        match message {
            PianoRollMessage::TransportPlayPause => self.toggle_piano_roll_playback(),
            PianoRollMessage::TransportRewind => self.rewind_piano_roll_playback(),
            _ => return None,
        }
        Some(())
    }

    fn rewind_piano_roll_playback(&mut self) {
        self.transport_seek_preview = None;
        self.seek_playback_ticks(self.rewind_target_tick());
    }

    fn handle_piano_roll_metronome_transport_message(
        &mut self,
        message: PianoRollMessage,
    ) -> Option<()> {
        if !matches!(
            message,
            PianoRollMessage::TransportToggleMetronome
                | PianoRollMessage::TransportOpenMetronomeMenu
                | PianoRollMessage::TransportCloseMetronomeMenu
                | PianoRollMessage::TransportMetronomeGainChanged(_)
                | PianoRollMessage::TransportMetronomePitchChanged(_)
        ) {
            return None;
        }
        self.handle_piano_roll_metronome_message(message);
        Some(())
    }

    fn toggle_piano_roll_playback(&mut self) {
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
                    "No SoundFont configured. Set playback.soundfonts in settings.toml or use \
                     --soundfont",
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
        }
    }

    fn handle_piano_roll_metronome_message(&mut self, message: PianoRollMessage) {
        if self.handle_metronome_menu_message(&message) {
            return;
        }
        self.handle_metronome_value_message(message);
    }

    fn handle_metronome_menu_message(&mut self, message: &PianoRollMessage) -> bool {
        match message {
            PianoRollMessage::TransportToggleMetronome => {
                self.metronome.enabled = !self.metronome.enabled;
                self.commit_metronome_settings_change();
                true
            }
            PianoRollMessage::TransportOpenMetronomeMenu => {
                self.metronome_menu_open = true;
                true
            }
            PianoRollMessage::TransportCloseMetronomeMenu => {
                self.metronome_menu_open = false;
                true
            }
            _ => false,
        }
    }

    fn handle_metronome_value_message(&mut self, message: PianoRollMessage) {
        match message {
            PianoRollMessage::TransportMetronomeGainChanged(gain_db) => {
                self.metronome.gain_db = gain_db.clamp(-36.0, 6.0);
                self.commit_metronome_settings_change();
            }
            PianoRollMessage::TransportMetronomePitchChanged(pitch) => {
                self.metronome.pitch = pitch.clamp(0.0, 1.0);
                self.commit_metronome_settings_change();
            }
            _ => {}
        }
    }

    fn commit_metronome_settings_change(&mut self) {
        self.apply_metronome_state_to_playback();
        self.persist_settings();
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

fn track_color_picker_target(message: &PianoRollMessage) -> Option<usize> {
    match message {
        PianoRollMessage::OpenTrackColorPickerForTrack(track_index) => Some(*track_index),
        _ => None,
    }
}

fn start_track_rename_message(message: &PianoRollMessage) -> Option<usize> {
    match message {
        PianoRollMessage::StartTrackRename(track_index) => Some(*track_index),
        _ => None,
    }
}

fn submit_track_color_message(message: &PianoRollMessage) -> Option<Color> {
    match message {
        PianoRollMessage::SubmitTrackColor(color) => Some(*color),
        _ => None,
    }
}

fn preview_track_color_message(message: &PianoRollMessage) -> Option<Color> {
    match message {
        PianoRollMessage::PreviewTrackColor(color) => Some(*color),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_route(message: PianoRollMessage, expected: fn(PianoRollRoute) -> bool) {
        assert!(expected(PianoRollRoute::from(message)));
    }

    fn assert_track_route(message: PianoRollMessage, expected: fn(PianoRollTrackRoute) -> bool) {
        let PianoRollRoute::Track(route) = PianoRollRoute::from(message) else {
            panic!("message should route to track handler");
        };
        assert!(expected(route));
    }

    #[test]
    fn piano_roll_route_classifies_message_groups() {
        assert_route(
            PianoRollMessage::ViewportCursorMoved(iced::Point::ORIGIN),
            |route| {
                matches!(
                    route,
                    PianoRollRoute::Viewport(PianoRollViewportRoute::CursorMoved(_))
                )
            },
        );
        assert_route(PianoRollMessage::ViewportCursorLeft, |route| {
            matches!(
                route,
                PianoRollRoute::Viewport(PianoRollViewportRoute::CursorLeft)
            )
        });
        assert_route(PianoRollMessage::RollScrolled { x: 1.0, y: 2.0 }, |route| {
            matches!(
                route,
                PianoRollRoute::Viewport(PianoRollViewportRoute::Scrolled { x, y })
                    if (x - 1.0).abs() <= f32::EPSILON
                        && (y - 2.0).abs() <= f32::EPSILON
            )
        });
        assert_route(PianoRollMessage::ZoomIn, |route| {
            matches!(route, PianoRollRoute::Zoom(_))
        });
        assert_route(PianoRollMessage::SetCursorTicks(24), |route| {
            matches!(route, PianoRollRoute::Timeline(_))
        });
        assert_route(PianoRollMessage::SelectTrack(1), |route| {
            matches!(route, PianoRollRoute::Track(_))
        });
        assert_route(PianoRollMessage::TransportPlayPause, |route| {
            matches!(route, PianoRollRoute::Transport(_))
        });
    }

    #[test]
    fn piano_roll_track_route_classifies_edit_messages() {
        assert_track_route(PianoRollMessage::SelectTrack(2), |route| {
            matches!(route, PianoRollTrackRoute::Select(2))
        });
        assert_track_route(PianoRollMessage::StartTrackRename(2), |route| {
            matches!(
                route,
                PianoRollTrackRoute::Edit(PianoRollTrackEditRoute::RenameEdit(_))
            )
        });
        assert_track_route(PianoRollMessage::CommitTrackRename, |route| {
            matches!(
                route,
                PianoRollTrackRoute::Edit(PianoRollTrackEditRoute::RenameFinish(_))
            )
        });
        assert_track_route(
            PianoRollMessage::PreviewTrackColor(iced::Color::BLACK),
            |route| {
                matches!(
                    route,
                    PianoRollTrackRoute::Edit(PianoRollTrackEditRoute::Color(_))
                )
            },
        );
    }

    #[test]
    fn piano_roll_track_route_classifies_panel_messages() {
        assert_track_route(PianoRollMessage::TrackPanelToggle, |route| {
            matches!(
                route,
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::Toggle)
            )
        });
        assert_track_route(PianoRollMessage::TrackPanelResizedBy(12.0), |route| {
            matches!(
                route,
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::Resize(12.0))
            )
        });
        assert_track_route(PianoRollMessage::TrackMuteToggled(1), |route| {
            matches!(
                route,
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::ToggleMute(1))
            )
        });
        assert_track_route(PianoRollMessage::TrackSoloToggled(1), |route| {
            matches!(
                route,
                PianoRollTrackRoute::Panel(PianoRollTrackPanelRoute::ToggleSolo(1))
            )
        });
    }
}
