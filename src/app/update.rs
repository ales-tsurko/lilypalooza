use std::{fs, path::Path, sync::mpsc::TryRecvError};

use iced::widget::{pane_grid, svg};
use iced_core::{Bytes, image};
use notify::event::EventKind;
use resvg::{tiny_skia, usvg};

use super::{
    messages::{KeyPress, MixerMessage},
    score_cursor, *,
};
use crate::{
    error_prompt::{ErrorFatality, ErrorPrompt, PromptButtons},
    midi,
    settings::{DockGroupSettings, DockNodeSettings},
    shortcuts::{self, ShortcutAction, ShortcutInput},
    state::{self, GlobalState, ProjectState},
};

mod editor;
mod files;
mod input;
mod mixer;
mod orchestration;
mod pane;
mod persistence;
mod piano_roll;
mod playback;
mod score;
mod score_preview;
mod track_names;
mod track_selection;

pub(super) use orchestration::update;
use orchestration::*;
use score_preview::*;
#[cfg(test)]
mod tests {
    use super::{
        parse_svg_coordinate_size_from_source, parse_svg_size_from_source,
        should_commit_track_rename_before_message,
    };
    use crate::app::{
        WorkspacePaneKind,
        messages::{Message, MixerMessage, PaneMessage, PianoRollMessage},
    };

    #[test]
    fn track_rename_commit_filter_keeps_editing_messages() {
        assert!(!should_commit_track_rename_before_message(
            &Message::PianoRoll(PianoRollMessage::TrackRenameInputChanged("x".into()),)
        ));
        assert!(!should_commit_track_rename_before_message(&Message::Mixer(
            MixerMessage::CommitTrackRename,
        )));
    }

    #[test]
    fn track_rename_commit_filter_commits_on_unrelated_actions() {
        assert!(should_commit_track_rename_before_message(
            &Message::PianoRoll(PianoRollMessage::TrackMuteToggled(0),)
        ));
        assert!(should_commit_track_rename_before_message(&Message::Mixer(
            MixerMessage::ToggleTrackSolo(0),
        )));
    }

    #[test]
    fn track_rename_commit_filter_ignores_passive_messages() {
        assert!(!should_commit_track_rename_before_message(&Message::Pane(
            PaneMessage::FocusWorkspacePane(WorkspacePaneKind::Mixer),
        )));
        assert!(!should_commit_track_rename_before_message(
            &Message::PianoRoll(PianoRollMessage::ViewportCursorMoved(iced::Point::ORIGIN),)
        ));
    }

    #[test]
    fn svg_size_parser_normalizes_old_and_cairo_a4_outputs() {
        let old_backend = r#"<svg width="210.00mm" height="297.00mm" viewBox="0.0000 -0.0000 119.5016 169.0094"></svg>"#;
        let cairo_backend = r#"<svg width="596" height="842" viewBox="0 0 596 842"></svg>"#;

        let old_size = parse_svg_size_from_source(old_backend).expect("old backend size");
        let cairo_size = parse_svg_size_from_source(cairo_backend).expect("cairo backend size");

        assert!((old_size.width - 595.2756).abs() < 0.001);
        assert!((old_size.height - 841.8898).abs() < 0.001);
        assert!((cairo_size.width - 596.0).abs() < 0.001);
        assert!((cairo_size.height - 842.0).abs() < 0.001);
    }

    #[test]
    fn old_and_cairo_svg_sizes_render_at_same_zoom_scale() {
        let old_backend = r#"<svg width="210.00mm" height="297.00mm" viewBox="0.0000 -0.0000 119.5016 169.0094"></svg>"#;
        let cairo_backend = r#"<svg width="596" height="842" viewBox="0 0 596 842"></svg>"#;

        let old_size = parse_svg_size_from_source(old_backend).expect("old backend size");
        let cairo_size = parse_svg_size_from_source(cairo_backend).expect("cairo backend size");
        let scale = super::score_view::score_base_scale();

        let old_width = old_size.width * scale;
        let cairo_width = cairo_size.width * scale;

        assert!((old_width - cairo_width).abs() < 1.0);
    }

    #[test]
    fn svg_coordinate_size_prefers_viewbox_for_interaction_mapping() {
        let old_backend = r#"<svg width="210.00mm" height="297.00mm" viewBox="0.0000 -0.0000 119.5016 169.0094"></svg>"#;

        let size = parse_svg_coordinate_size_from_source(old_backend).expect("coord size");

        assert!((size.width - 119.5016).abs() < 0.001);
        assert!((size.height - 169.0094).abs() < 0.001);
    }

    #[test]
    fn svg_coordinate_size_falls_back_to_display_size_without_viewbox() {
        let svg = r#"<svg width="596" height="842"></svg>"#;

        let size = parse_svg_coordinate_size_from_source(svg).expect("coord size");

        assert!((size.width - 596.0).abs() < 0.001);
        assert!((size.height - 842.0).abs() < 0.001);
    }

    #[test]
    fn display_to_svg_coordinate_mapping_preserves_old_backend_click_space() {
        let display_width = 595.2756_f32;
        let coord_width = 119.5016_f32;
        let display_x = 250.0_f32;

        let svg_x = display_x * coord_width / display_width;

        assert!((svg_x - 50.1875).abs() < 0.001);
    }
}
