use std::collections::HashMap;

use iced::Color;

use crate::ui_style;

mod helpers;
mod model;
mod render;

use helpers::*;
use model::*;
pub(super) use model::{
    AppEditorFrame,
    EDITOR_FRAME_ZOOM_MAX_PERCENT,
    EDITOR_FRAME_ZOOM_MIN_PERCENT,
};
#[cfg(test)]
mod tests {
    use iced::Color;

    use super::*;

    #[test]
    fn app_editor_frame_takes_chrome_colors_from_iced_theme() {
        let theme = iced::Theme::Dark;
        let palette = theme.extended_palette();
        let frame = AppEditorFrame::from_theme(&theme);

        assert_eq!(
            frame.style.frame_color,
            egui_color(palette.background.base.color)
        );
        assert_eq!(
            frame.style.titlebar_color,
            egui_color(mix_iced_color(
                palette.background.weak.color,
                Color::WHITE,
                0.04
            ))
        );
        assert_eq!(
            frame.style.title_color,
            egui_color(palette.background.base.text)
        );
    }

    #[test]
    fn app_editor_frame_close_button_has_hover_feedback() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        assert_ne!(
            frame.style.close_background,
            frame.style.close_background_hovered
        );
        assert_ne!(frame.style.close_icon, frame.style.close_icon_hovered);
    }

    #[test]
    fn app_editor_frame_border_is_thin() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        crate::test_assertions::assert_float_eq!(frame.frame_thickness, EDITOR_FRAME_THICKNESS);
        assert!(frame.frame_thickness < 4.0);
        assert!(frame.border_width < 1.0);
    }

    #[test]
    fn app_editor_frame_starts_compact() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        crate::test_assertions::assert_float_eq!(
            frame.titlebar_height,
            EDITOR_FRAME_COMPACT_CHROME_HEIGHT
        );
    }

    #[test]
    fn app_editor_frame_rect_uses_accepted_content_size_not_live_window_size() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let state = editor_host::EditorHostState {
            title: "Editor".to_string(),
            resizable: true,
            zoom_percent: 100,
            close_requested: false,
            content_size: editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            preset: None,
        };
        let live_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(900.0, 700.0),
        );

        let rect = frame.frame_rect(live_rect, &state);

        crate::test_assertions::assert_float_eq!(
            rect.width(),
            640.0 + (EDITOR_FRAME_THICKNESS * 2.0) as f32
        );
        crate::test_assertions::assert_float_eq!(
            rect.height(),
            480.0
                + EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32
                + (EDITOR_FRAME_THICKNESS * 2.0) as f32
        );
        assert!(rect.width() < live_rect.width());
        assert!(rect.height() < live_rect.height());
    }

    #[test]
    fn app_editor_frame_paints_full_live_window_to_avoid_black_gaps() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let live_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(900.0, 700.0),
        );

        assert_eq!(frame.paint_rect(live_rect), live_rect);
    }

    #[test]
    fn app_editor_frame_zoom_command_uses_selected_percent() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        assert_eq!(
            frame.zoom_command(125),
            editor_host::EditorFrameCommand::SetZoomPercent(125)
        );
        assert_eq!(
            frame.zoom_command(25),
            editor_host::EditorFrameCommand::SetZoomPercent(50)
        );
        assert_eq!(
            frame.zoom_command(250),
            editor_host::EditorFrameCommand::SetZoomPercent(200)
        );
    }

    #[test]
    fn app_editor_frame_reserves_visible_zoom_controls() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(titlebar.contains_rect(layout.zoom_row));
        assert!(layout.zoom_row.right() <= layout.close_button.left() - 8.0);
        assert!(layout.zoom_row.left() >= layout.preset_row.right() + 8.0);
        crate::test_assertions::assert_float_eq!(
            layout.zoom_row.width(),
            EDITOR_FRAME_ZOOM_CONTROL_WIDTH
        );
        assert!(!layout.zoom_row.intersects(layout.title_text));
        assert!(!layout.zoom_row.intersects(layout.close_button));
    }

    #[test]
    fn app_editor_frame_changes_preset_control_width_smoothly_during_resize() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let mut previous_width = None;

        for width in [400.0, 480.0, 560.0, 640.0, 720.0] {
            let titlebar = editor_host::egui::Rect::from_min_size(
                editor_host::egui::pos2(0.0, 0.0),
                editor_host::egui::vec2(width, EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32),
            );
            let layout = frame.preset_layout(titlebar);

            if let Some(previous_width) = previous_width {
                let delta = layout.preset_row.width() - previous_width;
                assert!((0.0..=80.0).contains(&delta));
            }
            previous_width = Some(layout.preset_row.width());
        }
    }

    #[test]
    fn app_editor_frame_dropdown_stays_inside_reserved_chrome() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let layout = frame.preset_layout(editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_EXPANDED_CHROME_HEIGHT as f32),
        ));

        assert!(layout.browser.bottom() <= EDITOR_FRAME_EXPANDED_CHROME_HEIGHT as f32);
    }

    #[test]
    fn app_editor_frame_browser_fits_view_width() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_EXPANDED_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(layout.browser.width() > layout.preset_row.width());
        crate::test_assertions::assert_float_eq!(layout.browser.left(), titlebar.left() + 8.0);
        crate::test_assertions::assert_float_eq!(layout.browser.right(), titlebar.right() - 8.0);
    }

    #[test]
    fn app_editor_frame_browser_list_starts_below_save_button() {
        let browser = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(8.0, 38.0),
            editor_host::egui::vec2(624.0, EDITOR_FRAME_PRESET_BROWSER_HEIGHT),
        );
        let save = AppEditorFrame::preset_browser_save_rect(browser);
        let list = AppEditorFrame::preset_browser_list_rect(browser);

        assert!(list.top() > save.bottom());
        assert!(browser.bottom() - list.bottom() >= 10.0);
        assert!(list.height() >= 72.0);
    }

    #[test]
    fn app_editor_frame_icons_render_from_lucide_assets() {
        for icon in [
            AppEditorFrameIcon::ChevronLeft,
            AppEditorFrameIcon::ChevronRight,
            AppEditorFrameIcon::ChevronDown,
            AppEditorFrameIcon::ChevronUp,
            AppEditorFrameIcon::Pencil,
            AppEditorFrameIcon::Save,
            AppEditorFrameIcon::Trash,
        ] {
            let svg = std::str::from_utf8(icon.svg_bytes()).expect("icon should be utf8 svg");
            assert!(svg.contains("<svg"));
            assert!(render_lucide_icon(icon).is_some());
        }
    }

    #[test]
    fn app_editor_frame_uses_stateful_preset_chevron() {
        assert_eq!(
            AppEditorFrame::preset_menu_icon(false),
            AppEditorFrameIcon::ChevronUp
        );
        assert_eq!(
            AppEditorFrame::preset_menu_icon(true),
            AppEditorFrameIcon::ChevronDown
        );
    }

    #[test]
    fn app_editor_frame_cuts_preset_text_to_fixed_width() {
        let text = ellipsize_for_width("Extremely Long Preset Name", 72.0, 13.0);

        assert!(text.ends_with("..."));
        assert!(text.len() < "Extremely Long Preset Name".len());
    }

    #[test]
    fn app_editor_frame_delete_requires_confirmation() {
        let mut frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        assert_eq!(frame.request_preset_delete("preset-1"), None);
        assert_eq!(
            frame.delete_confirmation_preset_id.as_deref(),
            Some("preset-1")
        );
        assert_eq!(
            frame.confirm_preset_delete("preset-1"),
            Some(editor_host::EditorFrameCommand::DeletePreset(
                "preset-1".to_string()
            ))
        );
        assert_eq!(frame.delete_confirmation_preset_id, None);
    }

    #[test]
    fn app_editor_frame_delete_confirmation_cuts_long_preset_name() {
        let message = AppEditorFrame::delete_confirmation_label(
            "Very Long Evolving Pad With Too Much Text",
            128.0,
        );

        assert!(message.starts_with("Remove \""));
        assert!(message.ends_with("\"?"));
        assert!(message.contains("..."));
        assert!(message.len() < "Remove \"Very Long Evolving Pad With Too Much Text\"?".len());
    }

    #[test]
    fn app_editor_frame_rename_can_cancel_without_command() {
        let mut frame = AppEditorFrame::from_theme(&iced::Theme::Dark);

        frame.begin_preset_rename("preset-1", "Warm Piano");
        frame.cancel_preset_rename();

        assert_eq!(frame.renaming_preset_id, None);
        assert!(frame.renaming_preset_value.is_empty());
    }

    #[test]
    fn app_editor_frame_compact_preset_controls_live_in_title_row() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(titlebar.contains_rect(layout.preset_row));
        assert!(layout.preset_row.right() <= layout.title_text.left());
        assert!(layout.title_text.right() <= layout.zoom_row.left());
        assert!(layout.zoom_row.right() <= layout.close_button.left());
        assert!(layout.browser.top() >= titlebar.top() + EDITOR_FRAME_TITLE_ROW_HEIGHT);
    }

    #[test]
    fn app_editor_frame_header_orders_presets_title_and_close_button() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(layout.preset_row.left() < layout.title_text.left());
        assert!(layout.title_text.right() < layout.close_button.left());
        assert!(layout.title_text.width() <= 180.0);
        crate::test_assertions::assert_float_eq!(
            layout.close_button.center().x,
            titlebar.right() - 18.0
        );
    }

    #[test]
    fn app_editor_frame_starts_native_drag_only_from_header_gap() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let state = editor_host::EditorHostState {
            title: "Editor".to_string(),
            resizable: true,
            zoom_percent: 100,
            close_requested: false,
            content_size: editor_host::Size {
                width: 640.0,
                height: 480.0,
            },
            preset: None,
        };
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(EDITOR_FRAME_THICKNESS as f32, EDITOR_FRAME_THICKNESS as f32),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_COMPACT_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(editor_host::EditorFrame::should_begin_window_drag(
            &frame,
            editor_host::egui::pos2(
                (layout.preset_row.right() + layout.title_text.left()) / 2.0,
                layout.title_text.center().y
            ),
            &state
        ));
        assert!(!editor_host::EditorFrame::should_begin_window_drag(
            &frame,
            layout.preset_row.center(),
            &state
        ));
        assert!(!editor_host::EditorFrame::should_begin_window_drag(
            &frame,
            layout.close_button.center(),
            &state
        ));
    }

    #[test]
    fn app_editor_frame_previous_button_does_not_overlap_dropdown() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let layout = frame.preset_layout(editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_EXPANDED_CHROME_HEIGHT as f32),
        ));

        assert!(!layout.previous.intersects(layout.browser));
    }

    #[test]
    fn app_editor_frame_close_button_stays_in_title_row_when_presets_expand() {
        let frame = AppEditorFrame::from_theme(&iced::Theme::Dark);
        let titlebar = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(640.0, EDITOR_FRAME_EXPANDED_CHROME_HEIGHT as f32),
        );
        let layout = frame.preset_layout(titlebar);

        assert!(titlebar.contains_rect(layout.close_button));
        assert!(!layout.close_button.intersects(layout.browser));
    }
}
