use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

use iced::{
    Color, ContentFit, Element, Fill, Length, Padding, Point, Rectangle, Size, Theme, alignment,
    border, mouse,
    widget::{
        Column, Tooltip, button, canvas, container, mouse_area, opaque, pane_grid, responsive, row,
        scrollable, slider, stack, svg, text, text_input, tooltip,
    },
};

use super::{
    DockDropRegion, EditorFileMenuSection, EditorHeaderMenuSection, Lilypalooza, Message,
    PaneMessage, ProjectMenuSection, WorkspacePaneKind, messages::ShortcutsMessage, mixer,
    piano_roll, score_view, transport_bar,
};
use crate::{fonts, icons, shortcuts, ui_style};

mod editor_menus;
mod editor_tabs;
mod toolbar_and_panes;
mod workspace_layout;

use editor_menus::*;
use editor_tabs::*;
use toolbar_and_panes::*;
pub(super) use toolbar_and_panes::{
    HEADER_CONTROL_HEIGHT, HeaderControlGroup, TOOLBAR_HEIGHT, delayed_tooltip, view,
};
use workspace_layout::*;
pub(super) use workspace_layout::{
    compact_control_icon, workspace_group_min_height, workspace_group_min_width,
};
#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use iced_test::simulator;

    use super::*;

    fn assert_snapshot_matches(ui: &mut iced_test::Simulator<'_, Message>, baseline_name: &str) {
        let _snapshot_guard = super::super::ICED_SNAPSHOT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let snapshot = ui
            .snapshot(&iced::Theme::Dark)
            .expect("snapshot should render");
        let baseline_path = Path::new(baseline_name);

        assert!(
            snapshot
                .matches_hash(baseline_name)
                .expect("snapshot hash should be readable"),
            "snapshot hash mismatch for: {baseline_name}"
        );
        assert!(
            snapshot
                .matches_image(baseline_path)
                .expect("snapshot image should be readable"),
            "snapshot image mismatch for: {baseline_name}"
        );
    }

    fn is_grid_multiple(value: f32) -> bool {
        ((value / 4.0).round() - (value / 4.0)).abs() < 1.0e-4
    }

    #[test]
    fn fixed_dock_sizes_follow_four_px_grid() {
        for value in [
            TOOLBAR_ICON_SIZE,
            HEADER_CONTROL_HEIGHT,
            HEADER_MENU_ICON_SIZE,
            HEADER_CLOSE_ICON_SIZE,
            HEADER_MENU_BUTTON_WIDTH,
            EDITOR_MENU_ROOT_WIDTH,
            EDITOR_FILE_SUBMENU_WIDTH,
            EDITOR_EDIT_SUBMENU_WIDTH,
            EDITOR_APPEARANCE_SUBMENU_WIDTH,
            EDITOR_MENU_ITEM_HEIGHT,
            HEADER_WIDTH_SAFETY,
            TOOLBAR_HEIGHT,
            TOOLBAR_TOGGLE_ICON_SIZE,
            TOOLBAR_BUTTON_HEIGHT,
            PROJECT_MENU_ROOT_WIDTH,
            PROJECT_MENU_WIDTH,
            PROJECT_SETTINGS_SUBMENU_WIDTH,
            EDITOR_FILE_BROWSER_ICON_SIZE,
            EDITOR_TAB_WIDTH,
            EDITOR_TAB_HEIGHT,
        ] {
            assert!(is_grid_multiple(value), "{value} should use the 4px grid");
        }
    }

    #[test]
    fn toolbar_and_pane_header_use_swapped_height_scales() {
        let toolbar_height = std::hint::black_box(TOOLBAR_HEIGHT);
        let toolbar_button_height = std::hint::black_box(TOOLBAR_BUTTON_HEIGHT);
        let header_control_height = std::hint::black_box(HEADER_CONTROL_HEIGHT);
        let pane_header_height =
            header_control_height + (PANE_HEADER_VERTICAL_PADDING as f32 * 2.0);
        crate::test_assertions::assert_float_eq!(
            pane_header_height,
            header_control_height + (PANE_HEADER_VERTICAL_PADDING as f32 * 2.0)
        );
        crate::test_assertions::assert_float_eq!(
            toolbar_height,
            toolbar_button_height + (TOOLBAR_VERTICAL_PADDING as f32 * 2.0)
        );
        assert!(toolbar_height > pane_header_height);
        assert!(toolbar_button_height > header_control_height);
    }

    #[test]
    fn close_button_icon_matches_other_pane_header_icons() {
        crate::test_assertions::assert_float_eq!(
            std::hint::black_box(HEADER_CLOSE_ICON_SIZE),
            std::hint::black_box(HEADER_MENU_ICON_SIZE)
        );
    }

    #[test]
    fn popup_menu_items_do_not_use_vertical_padding() {
        assert_eq!(EDITOR_MENU_ITEM_PADDING_V, 0);
    }

    #[test]
    fn editor_menu_item_matches_snapshot() {
        let mut ui = simulator(
            container(editor_menu_item("Open...", true, None))
                .width(Length::Fixed(EDITOR_MENU_ROOT_WIDTH))
                .padding(ui_style::PADDING_SM),
        );
        assert_snapshot_matches(&mut ui, "tests/snapshots/editor_menu_item");
    }

    #[test]
    fn project_root_menu_item_matches_snapshot() {
        let mut ui = simulator(
            container(project_root_menu_item(
                "Project",
                false,
                ProjectMenuSection::Project,
            ))
            .width(Length::Fixed(PROJECT_MENU_ROOT_WIDTH))
            .padding(ui_style::PADDING_SM),
        );
        assert_snapshot_matches(&mut ui, "tests/snapshots/project_root_menu_item");
    }

    #[test]
    fn editor_pane_header_layout_keeps_tabs_menu_and_close_control() {
        let (mut app, _task) = super::super::new_with_default_test_state();
        let _was_unfolded = app.unfold_workspace_pane(WorkspacePaneKind::Editor);
        app.set_active_workspace_pane(WorkspacePaneKind::Editor);
        let group_id = app
            .group_for_pane(WorkspacePaneKind::Editor)
            .expect("editor pane should exist");
        let group = app
            .dock_groups
            .get_mut(&group_id)
            .expect("editor pane group should exist");
        group.tabs = vec![
            WorkspacePaneKind::PianoRoll,
            WorkspacePaneKind::Score,
            WorkspacePaneKind::Editor,
        ];
        group.active = WorkspacePaneKind::Editor;

        let group = app
            .workspace_group(group_id)
            .expect("editor pane group should exist");
        let title_width = group_tabs_min_width(group);
        let layout = group_header_layout_from_parts(
            group.active,
            title_width,
            (600.0 - title_width).max(0.0),
            false,
        );

        assert_eq!(layout.active_pane, WorkspacePaneKind::Editor);
        crate::test_assertions::assert_float_eq!(
            layout.title_width,
            workspace_tab_min_width(WorkspacePaneKind::PianoRoll)
                + workspace_tab_min_width(WorkspacePaneKind::Score)
                + workspace_tab_min_width(WorkspacePaneKind::Editor)
                + ui_style::SPACE_XS as f32 * 2.0
        );
        crate::test_assertions::assert_float_eq!(
            layout.available_controls_width,
            600.0 - layout.title_width
        );
        assert!(layout.shows_menu_button);
        assert!(layout.shows_close_button);
        assert_eq!(
            workspace_pane_title(WorkspacePaneKind::PianoRoll),
            "Piano Roll"
        );
        assert_eq!(workspace_pane_title(WorkspacePaneKind::Score), "Score");
        assert_eq!(workspace_pane_title(WorkspacePaneKind::Editor), "Editor");
    }

    #[test]
    fn browser_column_click_emits_entry_press_message() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let alpha = root.path().join("alpha");
        fs::create_dir(&alpha).expect("alpha dir");

        let (app, _task) = super::super::new_with_default_test_state();
        let column = crate::app::editor::EditorBrowserColumnSummary::Directory {
            entries: vec![crate::app::editor::EditorBrowserEntrySummary {
                path: alpha.clone(),
                name: "alpha".to_string(),
                is_dir: true,
                selected: false,
            }],
        };

        let mut ui = simulator(editor_file_browser_column(&app, 0, column));
        ui.click("alpha").expect("alpha should be clickable");

        assert!(ui.into_messages().any(|message| matches!(
            message,
            Message::Editor(super::super::EditorMessage::FileBrowserEntryPressed {
                column_index: 0,
                is_dir: true,
                ref path,
            }) if path == &alpha
        )));
    }

    #[test]
    fn browser_header_click_emits_focus_message() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let (mut app, _task) = super::super::new_with_default_test_state();
        app.project_root = Some(root.path().to_path_buf());
        app.editor.set_project_root(Some(root.path().to_path_buf()));
        app.editor.toggle_file_browser();
        let root_label = app.editor.file_browser_root_label();

        let mut ui = simulator(editor_file_browser(&app));
        ui.click(root_label.as_str())
            .expect("browser header should be clickable");

        assert!(ui.into_messages().any(|message| matches!(
            message,
            Message::Editor(super::super::EditorMessage::FileBrowserFocused)
        )));
    }

    #[test]
    fn header_overflow_menu_x_clamps_to_viewport() {
        let bounds = Rectangle {
            x: 40.0,
            y: 0.0,
            width: 220.0,
            height: 300.0,
        };

        let x = header_overflow_menu_x(bounds, 600.0, 420.0);
        assert!(x >= ui_style::SPACE_XS as f32);
        assert!(x <= 600.0 - 420.0 - ui_style::SPACE_XS as f32);
    }
}
