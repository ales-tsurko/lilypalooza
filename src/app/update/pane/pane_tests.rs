use super::*;

fn app_with_editor_and_mixer_tabs() -> Lilypalooza {
    let (mut app, _task) = crate::app::new_with_default_test_state();
    app.dock_groups.clear();
    app.dock_groups.insert(
        1,
        DockGroup {
            tabs: vec![WorkspacePaneKind::Editor, WorkspacePaneKind::Mixer],
            active: WorkspacePaneKind::Editor,
        },
    );
    app.next_dock_group_id = 2;
    app.dock_layout = Some(DockNode::Group(1));
    app.folded_panes.clear();
    app.rebuild_workspace_panes();
    app
}

#[test]
fn pane_messages_drive_project_and_header_menus() {
    let (mut app, _task) = crate::app::new_with_default_test_state();
    let group_id = *app.dock_groups.keys().next().expect("dock group");

    let _task = app.handle_pane_message(PaneMessage::ToggleProjectMenu);
    assert!(app.open_project_menu);
    assert_eq!(
        app.open_project_menu_section,
        Some(super::super::ProjectMenuSection::Project)
    );

    let _task = app.handle_pane_message(PaneMessage::SetProjectRecentOpen(true));
    assert!(app.open_project_recent);

    let _task = app.handle_pane_message(PaneMessage::SetProjectMenuSection(Some(
        super::super::ProjectMenuSection::View,
    )));
    assert_eq!(
        app.open_project_menu_section,
        Some(super::super::ProjectMenuSection::View)
    );
    assert!(!app.open_project_recent);

    let _task = app.handle_pane_message(PaneMessage::OpenHeaderOverflowMenu(group_id));
    assert!(!app.open_project_menu);
    assert_eq!(app.open_header_overflow_menu, Some(group_id));

    let _task = app.handle_pane_message(PaneMessage::SetEditorHeaderMenuSection(Some(
        super::super::EditorHeaderMenuSection::File,
    )));
    assert_eq!(
        app.open_editor_menu_section,
        Some(super::super::EditorHeaderMenuSection::File)
    );

    let _task = app.handle_pane_message(PaneMessage::HoverEditorFileMenuSection {
        section: Some(super::super::EditorFileMenuSection::OpenRecent),
        expanded: true,
    });
    assert_eq!(
        app.open_editor_file_menu_section,
        Some(super::super::EditorFileMenuSection::OpenRecent)
    );

    let _task = app.handle_pane_message(PaneMessage::CloseHeaderOverflowMenu);
    assert_eq!(app.open_header_overflow_menu, None);
    assert_eq!(app.open_editor_menu_section, None);
    assert_eq!(app.open_editor_file_menu_section, None);

    let _task = app.handle_pane_message(PaneMessage::CloseProjectMenu);
    assert!(!app.open_project_menu);
}

#[test]
fn pane_messages_update_focus_hover_and_tooltips() {
    let mut app = app_with_editor_and_mixer_tabs();

    let _task = app.handle_pane_message(PaneMessage::FocusWorkspacePane(WorkspacePaneKind::Mixer));
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Mixer));

    let _task = app.handle_pane_message(PaneMessage::WorkspaceTabHovered(Some(
        WorkspacePaneKind::Score,
    )));
    assert_eq!(app.hovered_workspace_pane, Some(WorkspacePaneKind::Score));

    let _task = app.handle_pane_message(PaneMessage::TooltipHovered(Some("score".into())));
    assert_eq!(app.hovered_tooltip_key.as_deref(), Some("score"));
    assert_eq!(app.open_tooltip_key.as_deref(), Some("score"));

    let _task =
        app.handle_pane_message(PaneMessage::WorkspaceTabPressed(WorkspacePaneKind::Editor));
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Editor));
    assert_eq!(app.pressed_workspace_pane, Some(WorkspacePaneKind::Editor));
}

#[test]
fn pane_drag_messages_start_and_clear_drag_state() {
    let mut app = app_with_editor_and_mixer_tabs();

    app.pressed_workspace_pane = Some(WorkspacePaneKind::Mixer);
    let _task =
        app.handle_pane_message(PaneMessage::WorkspaceDragMoved(iced::Point::new(4.0, 4.0)));
    assert_eq!(app.workspace_drag_origin, Some(iced::Point::new(4.0, 4.0)));
    assert_eq!(app.dragged_workspace_pane, None);

    let _task =
        app.handle_pane_message(PaneMessage::WorkspaceDragMoved(iced::Point::new(40.0, 4.0)));
    assert_eq!(app.dragged_workspace_pane, Some(WorkspacePaneKind::Mixer));

    let _task = app.handle_pane_message(PaneMessage::WorkspaceDragExited);
    assert_eq!(app.dock_drop_target, None);

    let _task = app.handle_pane_message(PaneMessage::WorkspaceDragReleased);
    assert_eq!(app.pressed_workspace_pane, None);
    assert_eq!(app.dragged_workspace_pane, None);
    assert_eq!(app.workspace_drag_origin, None);
}

#[test]
fn pane_toggle_message_folds_and_restores_workspace_pane() {
    let mut app = app_with_editor_and_mixer_tabs();

    let _task = app.handle_pane_message(PaneMessage::ToggleWorkspacePane(WorkspacePaneKind::Mixer));
    assert!(app.is_pane_folded(WorkspacePaneKind::Mixer));

    let _task = app.handle_pane_message(PaneMessage::ToggleWorkspacePane(WorkspacePaneKind::Mixer));
    assert!(!app.is_pane_folded(WorkspacePaneKind::Mixer));
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Mixer));
}

#[test]
fn pane_toggle_restores_missing_unfolded_pane_after_state_restore() {
    let (mut app, _task) = crate::app::new_with_default_test_state();
    app.dock_groups.clear();
    app.dock_groups.insert(
        1,
        DockGroup {
            tabs: vec![WorkspacePaneKind::Score],
            active: WorkspacePaneKind::Score,
        },
    );
    app.next_dock_group_id = 2;
    app.dock_layout = Some(DockNode::Group(1));
    app.folded_panes.clear();
    app.rebuild_workspace_panes();

    let _task =
        app.handle_pane_message(PaneMessage::ToggleWorkspacePane(WorkspacePaneKind::Editor));

    assert!(app.group_for_pane(WorkspacePaneKind::Editor).is_some());
    assert_eq!(app.focused_workspace_pane, Some(WorkspacePaneKind::Editor));
}

#[test]
fn dock_drop_split_respects_inserted_pane_minimum_size() {
    let (mut app, _task) = crate::app::new_with_default_test_state();
    app.window_width = 900.0;
    app.window_height = crate::status_bar::HEIGHT
        + crate::app::transport_bar::HEIGHT
        + crate::app::dock_view::TOOLBAR_HEIGHT
        + crate::app::mixer::MIXER_MIN_HEIGHT * 1.5;
    let target_group = 1;
    let source_group = 2;
    app.dock_groups.clear();
    app.dock_groups.insert(
        target_group,
        DockGroup {
            tabs: vec![WorkspacePaneKind::Score],
            active: WorkspacePaneKind::Score,
        },
    );
    app.dock_groups.insert(
        source_group,
        DockGroup {
            tabs: vec![WorkspacePaneKind::Mixer],
            active: WorkspacePaneKind::Mixer,
        },
    );
    app.next_dock_group_id = 3;
    app.dock_layout = Some(DockNode::Split {
        axis: pane_grid::Axis::Vertical,
        ratio: 0.5,
        first: Box::new(DockNode::Group(target_group)),
        second: Box::new(DockNode::Group(source_group)),
    });
    app.rebuild_workspace_panes();

    app.apply_dock_drop(
        WorkspacePaneKind::Mixer,
        DockDropTarget {
            group_id: target_group,
            region: DockDropRegion::Top,
        },
    );

    let Some(DockNode::Split {
        axis, ratio, first, ..
    }) = app.dock_layout.as_ref()
    else {
        panic!("drop should create a split");
    };
    assert_eq!(*axis, pane_grid::Axis::Horizontal);
    assert!(matches!(first.as_ref(), DockNode::Group(group_id) if *group_id != source_group));
    assert!(
        *ratio > 0.5,
        "mixer split ratio should be expanded above the default half split"
    );
    assert!(
        *ratio * app.workspace_area_size().height >= crate::app::mixer::MIXER_MIN_HEIGHT,
        "mixer split should receive its minimum height"
    );
}
