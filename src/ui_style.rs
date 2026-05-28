use iced::{
    Border,
    Color,
    ContentFit,
    Length,
    Shadow,
    Theme,
    Vector,
    border,
    widget::{button, container, scrollable, svg, text_editor, text_input},
};
use iced_aw::style::{Status as AwStatus, color_picker::Style as AwColorPickerStyle};

use crate::control_icon;

mod buttons;
mod foundations;
mod icons;

pub(crate) use buttons::*;
pub(crate) use foundations::*;
pub(crate) use icons::*;
#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    fn is_grid_multiple(value: u32) -> bool {
        value.is_multiple_of(4)
    }

    fn collect_rust_files(root: &Path, out: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(root).expect("read dir");
        for entry in entries {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn transparent_surface_has_no_background() {
        let style = transparent_surface(&Theme::Dark);
        assert!(style.background.is_none());
    }

    #[test]
    fn mixer_track_strip_surface_differs_from_plain_pane_surface() {
        let palette = Theme::Dark.extended_palette();
        let plain = container::Style {
            background: Some(palette.background.base.color.into()),
            text_color: Some(palette.background.base.text),
            border: border::rounded(RADIUS_NONE)
                .width(0)
                .color(Color::TRANSPARENT),
            ..container::Style::default()
        };
        let tinted =
            mixer_track_strip_surface(&Theme::Dark, Some(Color::from_rgb(0.3, 0.4, 0.5)), false);
        assert_ne!(plain.background, tinted.background);
    }

    #[test]
    fn shared_spacing_and_surface_tokens_follow_four_px_grid() {
        for value in [
            SIZE_SURFACE_LG,
            SPACE_XS,
            SPACE_SM,
            SPACE_MD,
            u32::from(PADDING_XS),
            u32::from(PADDING_SM),
            u32::from(PADDING_MD),
            u32::from(PADDING_BUTTON_V),
            u32::from(PADDING_BUTTON_H),
            u32::from(PADDING_BUTTON_COMPACT_V),
            u32::from(PADDING_BUTTON_COMPACT_H),
            u32::from(PADDING_STATUS_BAR_V),
            u32::from(PADDING_STATUS_BAR_H),
        ] {
            assert!(is_grid_multiple(value), "{value} should use the 4px grid");
        }
    }

    #[test]
    fn compact_and_window_buttons_use_tighter_radius_than_general_ui() {
        let tight_radius = std::hint::black_box(RADIUS_TIGHT);
        let ui_radius = std::hint::black_box(RADIUS_UI);
        for style in [
            button_compact_solid(&Theme::Dark, button::Status::Active),
            button_compact_active(&Theme::Dark, button::Status::Active),
            button_window_control(&Theme::Dark, button::Status::Active),
        ] {
            crate::test_assertions::assert_float_eq!(style.border.radius.top_left, tight_radius);
            crate::test_assertions::assert_float_eq!(style.border.radius.top_right, tight_radius);
            crate::test_assertions::assert_float_eq!(style.border.radius.bottom_left, tight_radius);
            crate::test_assertions::assert_float_eq!(
                style.border.radius.bottom_right,
                tight_radius
            );
            assert!(tight_radius < ui_radius);
        }
    }

    #[test]
    fn pane_header_buttons_use_general_ui_radius() {
        for style in [
            button_pane_header_control(&Theme::Dark, button::Status::Active),
            button_pane_header_control_active(&Theme::Dark, button::Status::Active),
        ] {
            crate::test_assertions::assert_float_eq!(style.border.radius.top_left, RADIUS_UI);
            crate::test_assertions::assert_float_eq!(style.border.radius.top_right, RADIUS_UI);
            crate::test_assertions::assert_float_eq!(style.border.radius.bottom_left, RADIUS_UI);
            crate::test_assertions::assert_float_eq!(style.border.radius.bottom_right, RADIUS_UI);
        }
    }

    #[test]
    fn pane_tab_inactive_is_flat() {
        let style = button_pane_tab(&Theme::Dark, button::Status::Active, false);

        assert_eq!(style.background, None);
        crate::test_assertions::assert_float_eq!(style.border.width, 0.0);
    }

    #[test]
    fn browser_section_header_uses_tree_row_background() {
        let theme = Theme::Dark;
        let row = button_browser_child_entry(&theme, button::Status::Active, false);
        let header = button_browser_section_header(&theme, button::Status::Active);

        assert_eq!(header.background, row.background);
    }

    #[test]
    fn browser_child_entry_uses_dark_inset_background() {
        let theme = Theme::Dark;
        let palette = theme.extended_palette();
        let child = button_browser_child_entry(&theme, button::Status::Active, false);

        assert_eq!(child.background, Some(palette.background.base.color.into()));
    }

    #[test]
    fn app_code_does_not_use_icons_without_the_shared_helper() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut rust_files = Vec::new();
        collect_rust_files(&src_root, &mut rust_files);

        let offenders: Vec<_> = rust_files
            .into_iter()
            .filter(|path| !is_ui_style_source_file(path, &src_root))
            .filter_map(|path| {
                let content = fs::read_to_string(&path).expect("read source");
                content.contains("svg(icons::").then_some(path)
            })
            .collect();

        assert!(
            offenders.is_empty(),
            "icons must go through the shared helper, direct svg(icons::...) found in: \
             {offenders:?}"
        );
    }

    fn is_ui_style_source_file(path: &Path, src_root: &Path) -> bool {
        path == src_root.join("ui_style.rs") || path.starts_with(src_root.join("ui_style"))
    }
}
