use super::*;
use crate::app::mixer::{instrument_scroll_id, instrument_track_scroll_x};
use crate::app::piano_roll::{track_list_scroll_id, track_list_scroll_y};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum TrackSelectionOrigin {
    PianoRoll,
    Mixer,
}

impl Lilypalooza {
    pub(in crate::app) fn select_track(
        &mut self,
        track_index: usize,
        origin: TrackSelectionOrigin,
    ) -> Task<Message> {
        self.selected_track_index = Some(track_index);

        match origin {
            TrackSelectionOrigin::PianoRoll => self.reveal_track_in_mixer(track_index),
            TrackSelectionOrigin::Mixer => self.reveal_track_in_piano_roll(track_index),
        }
    }

    fn reveal_track_in_mixer(&self, track_index: usize) -> Task<Message> {
        if self.group_for_pane(WorkspacePaneKind::Mixer).is_none() {
            return Task::none();
        }

        iced::widget::operation::scroll_to(
            instrument_scroll_id(),
            iced::widget::operation::AbsoluteOffset {
                x: Some(instrument_track_scroll_x(track_index)),
                y: None,
            },
        )
    }

    fn reveal_track_in_piano_roll(&self, track_index: usize) -> Task<Message> {
        if self.group_for_pane(WorkspacePaneKind::PianoRoll).is_none() {
            return Task::none();
        }
        if self
            .piano_roll
            .current_file()
            .is_none_or(|file| track_index >= file.data.tracks.len())
        {
            return Task::none();
        }

        iced::widget::operation::scroll_to(
            track_list_scroll_id(),
            iced::widget::operation::AbsoluteOffset {
                x: None,
                y: Some(track_list_scroll_y(track_index)),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::update;
    use crate::app::messages::{Message, MixerMessage, PianoRollMessage};

    #[test]
    fn piano_roll_track_selection_updates_shared_state() {
        let (mut app, _task) = super::super::super::new(None, None, false);

        let _ = update(
            &mut app,
            Message::PianoRoll(PianoRollMessage::SelectTrack(3)),
        );

        assert_eq!(app.selected_track_index, Some(3));
    }

    #[test]
    fn mixer_track_selection_updates_shared_state_without_playback() {
        let (mut app, _task) = super::super::super::new(None, None, false);

        let _ = update(&mut app, Message::Mixer(MixerMessage::SelectTrack(5)));

        assert_eq!(app.selected_track_index, Some(5));
    }
}
