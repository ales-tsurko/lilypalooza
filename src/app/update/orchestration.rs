use super::*;

pub(super) const DRAG_START_THRESHOLD: f32 = 8.0;
pub(super) const SCORE_PREVIEW_FALLBACK_MAX_DIMENSION: f32 = 2200.0;
pub(super) const SCORE_PREVIEW_PRIMARY_MAX_DIMENSION: f32 = 3600.0;
pub(super) const SCORE_PREVIEW_FALLBACK_MIN_ZOOM: f32 = 1.0;
pub(super) const SCORE_PREVIEW_PRIMARY_MIN_ZOOM: f32 = 1.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum TabDirection {
    Previous,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum PaneCycleDirection {
    Previous,
    Next,
}

macro_rules! route_message_groups {
    ($app:expr, $message:expr, $($route:ident),+ $(,)?) => {{
        let mut message = $message;
        $(
            message = match $route($app, message) {
                RoutedMessage::Handled(task) => return task,
                RoutedMessage::Unhandled(message) => message,
            };
        )+
        message
    }};
}

macro_rules! route_message {
    (fn $name:ident($app:ident, $message:ident) {
        $($pattern:pat => $body:expr),+ $(,)?
    }) => {
        fn $name($app: &mut Lilypalooza, $message: Message) -> RoutedMessage {
            match $message {
                $($pattern => RoutedMessage::Handled($body),)+
                other => RoutedMessage::Unhandled(other),
            }
        }
    };
}

pub(in crate::app) fn update(app: &mut Lilypalooza, message: Message) -> Task<Message> {
    if app.renaming_target.is_some() && should_commit_track_rename_before_message(&message) {
        app.apply_pending_track_rename();
    }

    let message = route_message_groups!(
        app,
        message,
        route_startup_message,
        route_background_cleanup_message,
        route_background_compile_message,
        route_workspace_message,
        route_editor_tab_message,
        route_editor_dialog_message,
        route_window_message,
        route_keyboard_input_message,
        route_pointer_input_message,
    );
    handle_frame_message(app, message)
}

enum RoutedMessage {
    Handled(Task<Message>),
    Unhandled(Message),
}

route_message! {
    fn route_startup_message(app, message) {
        Message::Noop => Task::none(),
        Message::StartupChecked(result) => app.handle_startup_checked(result),
    }
}

route_message! {
    fn route_background_cleanup_message(app, message) {
        Message::BrowserHistoryCleanupFinished(result) => {
            log_browser_history_cleanup_result(app, result);
            Task::none()
        },
        Message::PluginScanCacheSaved(result) => {
            log_plugin_scan_cache_save_result(app, result);
            Task::none()
        },
    }
}

route_message! {
    fn route_background_compile_message(app, message) {
        Message::ScorePreviewReady(result) => app.handle_score_preview_ready(result),
        Message::CompileOutputsReady(result) => app.handle_compile_outputs_ready(result),
    }
}

fn log_browser_history_cleanup_result(app: &mut Lilypalooza, result: Result<(), String>) {
    if let Err(error) = result {
        app.logger
            .push(format!("Browser history cleanup failed: {error}"));
    }
}

fn log_plugin_scan_cache_save_result(app: &mut Lilypalooza, result: Result<(), String>) {
    if let Err(error) = result {
        app.logger.push(error);
    }
}

route_message! {
    fn route_workspace_message(app, message) {
        Message::Pane(message) => app.handle_pane_message(message),
        Message::File(message) => app.handle_file_message(message),
        Message::Viewer(message) => app.handle_viewer_message(message),
        Message::PianoRoll(message) => app.handle_piano_roll_message(message),
        Message::Mixer(message) => app.handle_mixer_message(message),
    }
}

route_message! {
    fn route_editor_tab_message(app, message) {
        Message::Editor(message) => app.handle_editor_message(message),
        Message::Logger(message) => app.handle_logger_message(message),
    }
}

route_message! {
    fn route_editor_dialog_message(app, message) {
        Message::Shortcuts(message) => app.handle_shortcuts_message(message),
        Message::Prompt(message) => app.handle_prompt_message(message),
    }
}

route_message! {
    fn route_window_message(app, message) {
        Message::WindowOpened(window_id) => app.handle_window_opened(window_id),
        Message::WindowClosed(window_id) => app.handle_window_closed(window_id),
        Message::WindowFocused(window_id) => app.handle_processor_editor_focused(window_id),
        Message::WindowSnapshotCaptured {
            window_id,
            host,
            parent,
        } => app.handle_processor_editor_attached(window_id, host, parent),
    }
}

route_message! {
    fn route_keyboard_input_message(app, message) {
        Message::KeyPressed(key_press) => app.handle_key_pressed(key_press),
        Message::TrackRenameFocusChanged(focused) => app.handle_track_rename_focus_changed(focused),
    }
}

route_message! {
    fn route_pointer_input_message(app, message) {
        Message::ModifiersChanged(modifiers) => app.handle_modifiers_changed(modifiers),
        Message::PrimaryMousePressed(pressed) => app.handle_primary_mouse_pressed(pressed),
    }
}

pub(super) fn handle_frame_message(app: &mut Lilypalooza, message: Message) -> Task<Message> {
    if let Some(task) = handle_frame_tick_message(app, &message) {
        return task;
    }
    if let Some(task) = handle_frame_window_message(app, &message) {
        return task;
    }
    Task::none()
}

pub(super) fn handle_frame_tick_message(
    app: &mut Lilypalooza,
    message: &Message,
) -> Option<Task<Message>> {
    match message {
        Message::Tick => Some(app.handle_tick()),
        Message::Frame(now) => Some(app.handle_frame(*now)),
        _ => None,
    }
}

pub(super) fn handle_frame_window_message(
    app: &mut Lilypalooza,
    message: &Message,
) -> Option<Task<Message>> {
    match message {
        Message::WindowResized { window_id, size } => {
            Some(app.handle_window_resized(*window_id, *size))
        }
        Message::WindowCloseRequested(window_id) => {
            Some(app.handle_window_close_requested(*window_id))
        }
        _ => None,
    }
}

pub(super) fn should_commit_track_rename_before_message(message: &Message) -> bool {
    !matches!(
        message,
        Message::Noop
            | Message::StartupChecked(_)
            | Message::BrowserHistoryCleanupFinished(_)
            | Message::PluginScanCacheSaved(_)
            | Message::ScorePreviewReady(_)
            | Message::CompileOutputsReady(_)
            | Message::Pane(_)
            | Message::Viewer(
                ViewerMessage::ScrollPositionChanged { .. }
                    | ViewerMessage::ViewportCursorMoved(_)
                    | ViewerMessage::ViewportCursorLeft
            )
            | Message::KeyPressed(_)
            | Message::TrackRenameFocusChanged(_)
            | Message::ModifiersChanged(_)
            | Message::PrimaryMousePressed(_)
            | Message::Tick
            | Message::Frame(_)
            | Message::WindowOpened(_)
            | Message::WindowClosed(_)
            | Message::WindowFocused(_)
            | Message::WindowResized { .. }
            | Message::WindowCloseRequested(_)
            | Message::PianoRoll(
                PianoRollMessage::StartTrackRename(_)
                    | PianoRollMessage::OpenTrackColorPickerForTrack(_)
                    | PianoRollMessage::TrackRenameInputChanged(_)
                    | PianoRollMessage::OpenTrackColorPicker
                    | PianoRollMessage::SubmitTrackColor(_)
                    | PianoRollMessage::PreviewTrackColor(_)
                    | PianoRollMessage::CommitTrackRename
                    | PianoRollMessage::CancelTrackRename
                    | PianoRollMessage::ViewportCursorMoved(_)
                    | PianoRollMessage::ViewportCursorLeft
                    | PianoRollMessage::RollScrolled { .. }
            )
            | Message::Mixer(
                MixerMessage::StartTrackRename(_)
                    | MixerMessage::StartBusRename(_)
                    | MixerMessage::TrackRenameInputChanged(_)
                    | MixerMessage::OpenTrackColorPicker
                    | MixerMessage::SubmitTrackColor(_)
                    | MixerMessage::PreviewTrackColor(_)
                    | MixerMessage::CommitTrackRename
                    | MixerMessage::CancelTrackRename
                    | MixerMessage::InstrumentViewportScrolled(_)
                    | MixerMessage::BusViewportScrolled(_)
            )
    )
}

pub(super) fn dock_node_to_settings(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> DockNodeSettings {
    match node {
        DockNode::Group(group_id) => DockNodeSettings::Group(
            groups
                .get(group_id)
                .map(|group| DockGroupSettings {
                    tabs: group.tabs.clone(),
                    active: group.active,
                })
                .unwrap_or_default(),
        ),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => DockNodeSettings::Split {
            axis: dock_axis_to_settings(*axis),
            ratio: *ratio,
            first: Box::new(dock_node_to_settings(first, groups)),
            second: Box::new(dock_node_to_settings(second, groups)),
        },
    }
}

pub(super) fn collect_workspace_group_bounds(
    state: &pane_grid::State<DockGroupId>,
    node: &pane_grid::Node,
    bounds: iced::Rectangle,
    group_bounds: &mut std::collections::HashMap<DockGroupId, iced::Rectangle>,
) {
    match node {
        pane_grid::Node::Pane(pane) => {
            if let Some(group_id) = state.get(*pane) {
                group_bounds.insert(*group_id, bounds);
            }
        }
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => match axis {
            pane_grid::Axis::Horizontal => {
                let first_height = bounds.height * ratio;
                collect_workspace_group_bounds(
                    state,
                    a,
                    iced::Rectangle {
                        height: first_height,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_workspace_group_bounds(
                    state,
                    b,
                    iced::Rectangle {
                        y: bounds.y + first_height,
                        height: bounds.height - first_height,
                        ..bounds
                    },
                    group_bounds,
                );
            }
            pane_grid::Axis::Vertical => {
                let first_width = bounds.width * ratio;
                collect_workspace_group_bounds(
                    state,
                    a,
                    iced::Rectangle {
                        width: first_width,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_workspace_group_bounds(
                    state,
                    b,
                    iced::Rectangle {
                        x: bounds.x + first_width,
                        width: bounds.width - first_width,
                        ..bounds
                    },
                    group_bounds,
                );
            }
        },
    }
}

pub(super) fn collect_visible_group_order(node: &DockNode, group_ids: &mut Vec<DockGroupId>) {
    match node {
        DockNode::Group(group_id) => group_ids.push(*group_id),
        DockNode::Split { first, second, .. } => {
            collect_visible_group_order(first, group_ids);
            collect_visible_group_order(second, group_ids);
        }
    }
}

pub(super) fn split_children(
    node: &pane_grid::Node,
    split: pane_grid::Split,
) -> Option<(&pane_grid::Node, &pane_grid::Node)> {
    match node {
        pane_grid::Node::Pane(_) => None,
        pane_grid::Node::Split { id, a, b, .. } => {
            if *id == split {
                Some((a.as_ref(), b.as_ref()))
            } else {
                split_children(a, split).or_else(|| split_children(b, split))
            }
        }
    }
}

pub(super) fn dock_node_min_width(
    node: &pane_grid::Node,
    state: &pane_grid::State<DockGroupId>,
    app: &Lilypalooza,
) -> f32 {
    dock_node_min_size(node, state, app, DockMinSize::Width)
}

pub(super) fn dock_node_min_height(
    node: &pane_grid::Node,
    state: &pane_grid::State<DockGroupId>,
    app: &Lilypalooza,
) -> f32 {
    dock_node_min_size(node, state, app, DockMinSize::Height)
}

#[derive(Debug, Clone, Copy)]
enum DockMinSize {
    Width,
    Height,
}

fn dock_node_min_size(
    node: &pane_grid::Node,
    state: &pane_grid::State<DockGroupId>,
    app: &Lilypalooza,
    size: DockMinSize,
) -> f32 {
    match node {
        pane_grid::Node::Pane(pane) => state
            .get(*pane)
            .map(|group_id| match size {
                DockMinSize::Width => super::dock_view::workspace_group_min_width(app, *group_id),
                DockMinSize::Height => super::dock_view::workspace_group_min_height(app, *group_id),
            })
            .unwrap_or(0.0),
        pane_grid::Node::Split { axis, a, b, .. } => {
            let first = dock_node_min_size(a, state, app, size);
            let second = dock_node_min_size(b, state, app, size);

            match (size, axis) {
                (DockMinSize::Width, pane_grid::Axis::Horizontal)
                | (DockMinSize::Height, pane_grid::Axis::Vertical) => first.max(second),
                (DockMinSize::Width, pane_grid::Axis::Vertical)
                | (DockMinSize::Height, pane_grid::Axis::Horizontal) => first + second,
            }
        }
    }
}

pub(super) fn dock_drop_region(bounds: iced::Rectangle, position: iced::Point) -> DockDropRegion {
    let relative_x = ((position.x - bounds.x) / bounds.width.max(1.0)).clamp(0.0, 1.0);
    let relative_y = ((position.y - bounds.y) / bounds.height.max(1.0)).clamp(0.0, 1.0);
    let center_min = 1.0 / 3.0;
    let center_max = 2.0 / 3.0;

    if (center_min..=center_max).contains(&relative_x)
        && (center_min..=center_max).contains(&relative_y)
    {
        return DockDropRegion::Center;
    }

    let top_distance = relative_y;
    let right_distance = 1.0 - relative_x;
    let bottom_distance = 1.0 - relative_y;
    let left_distance = relative_x;
    let mut closest = (DockDropRegion::Top, top_distance);

    for candidate in [
        (DockDropRegion::Right, right_distance),
        (DockDropRegion::Bottom, bottom_distance),
        (DockDropRegion::Left, left_distance),
    ] {
        if candidate.1 < closest.1 {
            closest = candidate;
        }
    }

    closest.0
}

pub(super) fn move_tab_to_front(tabs: &mut Vec<WorkspacePaneKind>, pane: WorkspacePaneKind) {
    if let Some(index) = tabs.iter().position(|candidate| *candidate == pane) {
        let pane = tabs.remove(index);
        tabs.insert(0, pane);
    }
}

pub(super) fn drag_distance(a: iced::Point, b: iced::Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

pub(super) fn remove_pane_from_group(
    groups: &mut std::collections::HashMap<DockGroupId, DockGroup>,
    group_id: DockGroupId,
    pane: WorkspacePaneKind,
) -> bool {
    let Some(group) = groups.get_mut(&group_id) else {
        return false;
    };

    group.tabs.retain(|candidate| *candidate != pane);

    if group.active == pane {
        group.active = group
            .tabs
            .first()
            .copied()
            .unwrap_or(WorkspacePaneKind::Score);
    }

    group.tabs.is_empty()
}

pub(super) fn prune_group_from_layout(layout: DockNode, group_id: DockGroupId) -> DockNode {
    prune_group_from_layout_inner(layout, group_id).unwrap_or(DockNode::Group(group_id))
}

pub(super) fn prune_group_from_layout_inner(
    layout: DockNode,
    group_id: DockGroupId,
) -> Option<DockNode> {
    match layout {
        DockNode::Group(candidate) => (candidate != group_id).then_some(DockNode::Group(candidate)),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let first = prune_group_from_layout_inner(*first, group_id);
            let second = prune_group_from_layout_inner(*second, group_id);

            match (first, second) {
                (Some(first), Some(second)) => Some(DockNode::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            }
        }
    }
}

pub(super) fn replace_group_with_split(
    node: &mut DockNode,
    target_group_id: DockGroupId,
    axis: pane_grid::Axis,
    ratio: f32,
    new_group_id: DockGroupId,
    insert_first: bool,
) -> bool {
    match node {
        DockNode::Group(group_id) if *group_id == target_group_id => {
            let existing_group = DockNode::Group(*group_id);
            let new_group = DockNode::Group(new_group_id);
            *node = DockNode::Split {
                axis,
                ratio,
                first: Box::new(if insert_first {
                    new_group.clone()
                } else {
                    existing_group.clone()
                }),
                second: Box::new(if insert_first {
                    existing_group
                } else {
                    new_group
                }),
            };
            true
        }
        DockNode::Group(_) => false,
        DockNode::Split { first, second, .. } => {
            replace_group_with_split(
                first,
                target_group_id,
                axis,
                ratio,
                new_group_id,
                insert_first,
            ) || replace_group_with_split(
                second,
                target_group_id,
                axis,
                ratio,
                new_group_id,
                insert_first,
            )
        }
    }
}

pub(super) fn split_restore_target_for_group(
    node: &DockNode,
    group_id: DockGroupId,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Option<(
    pane_grid::Axis,
    f32,
    bool,
    WorkspacePaneKind,
    Vec<WorkspacePaneKind>,
)> {
    match node {
        DockNode::Group(_) => None,
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
            ..
        } => {
            if contains_group(first, group_id) {
                if let Some(target) = split_restore_target_for_group(first, group_id, groups) {
                    return Some(target);
                }

                let sibling_panes = panes_in_node(second, groups);
                Some((
                    *axis,
                    *ratio,
                    true,
                    first_pane_in_node(second, groups)?,
                    sibling_panes,
                ))
            } else if contains_group(second, group_id) {
                if let Some(target) = split_restore_target_for_group(second, group_id, groups) {
                    return Some(target);
                }

                let sibling_panes = panes_in_node(first, groups);
                Some((
                    *axis,
                    *ratio,
                    false,
                    first_pane_in_node(first, groups)?,
                    sibling_panes,
                ))
            } else {
                None
            }
        }
    }
}

pub(super) fn first_pane_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Option<WorkspacePaneKind> {
    match node {
        DockNode::Group(group_id) => groups
            .get(group_id)
            .and_then(|group| group.tabs.first().copied()),
        DockNode::Split { first, second, .. } => {
            first_pane_in_node(first, groups).or_else(|| first_pane_in_node(second, groups))
        }
    }
}

pub(super) fn panes_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> Vec<WorkspacePaneKind> {
    let mut panes = Vec::new();
    collect_panes_in_node(node, groups, &mut panes);
    panes.sort_by_key(|pane| pane_sort_key(*pane));
    panes.dedup();
    panes
}

pub(super) fn collect_panes_in_node(
    node: &DockNode,
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
    panes: &mut Vec<WorkspacePaneKind>,
) {
    match node {
        DockNode::Group(group_id) => {
            if let Some(group) = groups.get(group_id) {
                panes.extend(group.tabs.iter().copied());
            }
        }
        DockNode::Split { first, second, .. } => {
            collect_panes_in_node(first, groups, panes);
            collect_panes_in_node(second, groups, panes);
        }
    }
}

pub(super) fn replace_subtree_with_split(
    node: &mut DockNode,
    axis: pane_grid::Axis,
    ratio: f32,
    new_group_id: DockGroupId,
    insert_first: bool,
    target_panes: &[WorkspacePaneKind],
    groups: &std::collections::HashMap<DockGroupId, DockGroup>,
) -> bool {
    if panes_in_node(node, groups) == target_panes {
        let existing = node.clone();
        let new_group = DockNode::Group(new_group_id);
        *node = DockNode::Split {
            axis,
            ratio,
            first: Box::new(if insert_first {
                new_group.clone()
            } else {
                existing.clone()
            }),
            second: Box::new(if insert_first { existing } else { new_group }),
        };
        return true;
    }

    match node {
        DockNode::Group(_) => false,
        DockNode::Split { first, second, .. } => {
            replace_subtree_with_split(
                first,
                axis,
                ratio,
                new_group_id,
                insert_first,
                target_panes,
                groups,
            ) || replace_subtree_with_split(
                second,
                axis,
                ratio,
                new_group_id,
                insert_first,
                target_panes,
                groups,
            )
        }
    }
}

pub(super) fn pane_sort_key(pane: WorkspacePaneKind) -> u8 {
    pane as u8
}

pub(super) fn first_group_id_in_layout(node: &DockNode) -> Option<DockGroupId> {
    match node {
        DockNode::Group(group_id) => Some(*group_id),
        DockNode::Split { first, second, .. } => {
            first_group_id_in_layout(first).or_else(|| first_group_id_in_layout(second))
        }
    }
}

pub(super) fn snap_zoom_to_step(value: f32, step: f32) -> f32 {
    if step <= f32::EPSILON {
        return value;
    }

    (value / step).round() * step
}

pub(super) fn next_zoom_step_up(current: f32, step: f32, max_zoom: f32) -> f32 {
    let snapped = snap_zoom_to_step(current, step);

    if (current - snapped).abs() <= 1e-4 {
        (snapped + step).clamp(MIN_SVG_ZOOM, max_zoom)
    } else if current < snapped {
        snapped.clamp(MIN_SVG_ZOOM, max_zoom)
    } else {
        (snapped + step).clamp(MIN_SVG_ZOOM, max_zoom)
    }
}

pub(super) fn next_zoom_step_down(current: f32, step: f32, min_zoom: f32) -> f32 {
    let snapped = snap_zoom_to_step(current, step);

    if (current - snapped).abs() <= 1e-4 {
        (snapped - step).clamp(min_zoom, MAX_SVG_ZOOM)
    } else if current > snapped {
        snapped.clamp(min_zoom, MAX_SVG_ZOOM)
    } else {
        (snapped - step).clamp(min_zoom, MAX_SVG_ZOOM)
    }
}
