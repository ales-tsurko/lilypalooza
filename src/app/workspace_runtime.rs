use std::collections::HashSet;

use super::*;

#[cfg(test)]
pub(super) fn new_with_default_test_state() -> (Lilypalooza, Task<Message>) {
    new_with_loaded_state(
        None,
        None,
        false,
        settings::AppSettings::default(),
        None,
        GlobalState::default(),
        None,
    )
}

pub(super) fn subscription(app: &Lilypalooza) -> Subscription<Message> {
    let mut subscriptions = base_subscriptions();
    subscriptions.extend(tick_subscription_intervals(app).map(tick_subscription));
    subscriptions.extend(frame_subscription_intervals(app).map(frame_subscription));
    Subscription::batch(subscriptions)
}

pub(super) fn base_subscriptions() -> Vec<Subscription<Message>> {
    vec![
        window::open_events().map(Message::WindowOpened),
        window::close_events().map(Message::WindowClosed),
        window::resize_events().map(|(window_id, size)| Message::WindowResized { window_id, size }),
        window::close_requests().map(Message::WindowCloseRequested),
        event::listen_with(runtime_event_to_message),
    ]
}

pub(super) fn tick_subscription_intervals(app: &Lilypalooza) -> impl Iterator<Item = Duration> {
    [
        app.watch_poll_active().then_some(WATCH_POLL_INTERVAL),
        app.editor_tick_active().then_some(EDITOR_TICK_INTERVAL),
        app.spinner_active().then_some(SPINNER_POLL_INTERVAL),
        app.plugin_scan
            .is_active()
            .then_some(PLUGIN_SCAN_POLL_INTERVAL),
        app.dragged_editor_tab
            .is_some()
            .then_some(EDITOR_TABBAR_AUTOSCROLL_INTERVAL),
        (app.effect_drag_source.is_some() && app.effect_rack_autoscroll_direction != 0)
            .then_some(EDITOR_TABBAR_AUTOSCROLL_INTERVAL),
        (app.score_zoom_preview_active() || app.score_zoom_persist_pending)
            .then_some(SCORE_ZOOM_PREVIEW_INTERVAL),
    ]
    .into_iter()
    .flatten()
}

pub(super) fn frame_subscription_intervals(app: &Lilypalooza) -> impl Iterator<Item = Duration> {
    [
        app.playback_poll_interval(),
        app.processor_editor_windows
            .has_installed_hosts()
            .then_some(EDITOR_HOST_POLL_INTERVAL),
        app.pending_mixer_message_after_editor_detach
            .is_some()
            .then_some(EDITOR_HOST_POLL_INTERVAL),
    ]
    .into_iter()
    .flatten()
}

pub(super) fn tick_subscription(interval: Duration) -> Subscription<Message> {
    iced::time::every(interval).map(|_| Message::Tick)
}

pub(super) fn frame_subscription(interval: Duration) -> Subscription<Message> {
    iced::time::every(interval).map(Message::Frame)
}

pub(super) async fn cleanup_stale_browser_history_dirs(
    current_dir: Option<PathBuf>,
) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let entries = fs::read_dir(&temp_dir)
        .map_err(|error| format!("Failed to read temp dir {}: {error}", temp_dir.display()))?;

    for entry in entries {
        let entry = stale_browser_history_entry(entry, &temp_dir)?;
        remove_stale_browser_history_entry(entry, current_dir.as_ref())?;
    }

    Ok(())
}

pub(super) fn stale_browser_history_entry(
    entry: Result<fs::DirEntry, std::io::Error>,
    temp_dir: &Path,
) -> Result<Option<fs::DirEntry>, String> {
    let entry = entry.map_err(|error| {
        format!(
            "Failed to read temp dir entry in {}: {error}",
            temp_dir.display()
        )
    })?;
    let file_name = entry.file_name();
    let Some(file_name) = file_name.to_str() else {
        return Ok(None);
    };
    Ok(file_name
        .starts_with("lilypalooza-browser-history")
        .then_some(entry))
}

pub(super) fn remove_stale_browser_history_entry(
    entry: Option<fs::DirEntry>,
    current_dir: Option<&PathBuf>,
) -> Result<(), String> {
    let Some(entry) = entry else {
        return Ok(());
    };
    let path = entry.path();
    if current_dir == Some(&path) || !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
        return Ok(());
    }
    fs::remove_dir_all(&path).map_err(|error| {
        format!(
            "Failed to remove stale browser history directory {}: {error}",
            path.display()
        )
    })
}

pub(super) fn runtime_event_to_message(
    event: iced::Event,
    status: event::Status,
    window_id: window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused(window_id)),
        iced::Event::Keyboard(event) => keyboard_runtime_event_to_message(event, status),
        iced::Event::Mouse(event) => mouse_runtime_event_to_message(event),
        _ => None,
    }
}

pub(super) fn keyboard_runtime_event_to_message(
    event: keyboard::Event,
    status: event::Status,
) -> Option<Message> {
    match event {
        keyboard::Event::ModifiersChanged(modifiers) => Some(Message::ModifiersChanged(modifiers)),
        keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        } => Some(Message::KeyPressed(KeyPress {
            status,
            key,
            physical_key,
            modifiers,
        })),
        _ => None,
    }
}

pub(super) fn mouse_runtime_event_to_message(event: mouse::Event) -> Option<Message> {
    match event {
        mouse::Event::ButtonPressed(mouse::Button::Left) => {
            Some(Message::PrimaryMousePressed(true))
        }
        mouse::Event::ButtonReleased(mouse::Button::Left) => {
            Some(Message::PrimaryMousePressed(false))
        }
        _ => None,
    }
}

impl Lilypalooza {
    pub(super) fn watch_poll_active(&self) -> bool {
        self.compile_session.is_some()
            || self.score_watcher.is_some()
            || self.browser_file_watcher.is_some()
            || self.editor_file_watcher.is_some()
    }

    pub(super) fn editor_tick_active(&self) -> bool {
        self.editor.has_document() && self.group_for_pane(WorkspacePaneKind::Editor).is_some()
    }

    pub(super) fn score_pane_visible(&self) -> bool {
        self.group_for_pane(WorkspacePaneKind::Score).is_some()
    }

    pub(super) fn piano_roll_pane_visible(&self) -> bool {
        self.group_for_pane(WorkspacePaneKind::PianoRoll).is_some()
    }

    pub(super) fn mixer_pane_visible(&self) -> bool {
        self.group_for_pane(WorkspacePaneKind::Mixer).is_some()
    }

    pub(super) fn playback_poll_interval(&self) -> Option<Duration> {
        if !(self.playback.is_some() && self.piano_roll.playback_is_playing()) {
            return None;
        }

        if self.score_pane_visible() || self.piano_roll_pane_visible() || self.mixer_pane_visible()
        {
            Some(ACTIVE_PLAYBACK_POLL_INTERVAL)
        } else {
            Some(PASSIVE_PLAYBACK_POLL_INTERVAL)
        }
    }

    pub(super) fn zoom_modifier_active(&self) -> bool {
        self.keyboard_modifiers.command() || self.keyboard_modifiers.control()
    }

    pub(super) fn shortcut_modifier_active(&self) -> bool {
        self.keyboard_modifiers.command() || self.keyboard_modifiers.control()
    }

    pub(super) fn workspace_group(&self, group_id: DockGroupId) -> Option<&DockGroup> {
        self.dock_groups.get(&group_id)
    }

    pub(super) fn group_for_pane(&self, pane: WorkspacePaneKind) -> Option<DockGroupId> {
        self.dock_groups
            .iter()
            .find_map(|(group_id, group)| group.tabs.contains(&pane).then_some(*group_id))
    }

    pub(super) fn focused_workspace_pane(&self) -> Option<WorkspacePaneKind> {
        self.focused_workspace_pane
            .filter(|pane| self.group_for_pane(*pane).is_some())
    }

    pub(super) fn has_saved_project(&self) -> bool {
        self.project_root.is_some()
    }

    pub(super) fn is_tooltip_open(&self, key: &str) -> bool {
        self.open_tooltip_key.as_deref() == Some(key)
    }

    pub(super) fn project_title(&self) -> String {
        self.project_name
            .as_ref()
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| "Unsaved Project".to_string())
    }

    pub(super) fn spinner_active(&self) -> bool {
        self.compile_requested
            || self.compile_outputs_loading
            || self.compile_session.is_some()
            || matches!(self.lilypond_status, LilypondStatus::Checking)
            || self.plugin_scan.is_active()
    }

    pub(super) fn spinner_frame(&self) -> &'static str {
        if self.spinner_active() {
            SPINNER_FRAMES
                .get(self.spinner_step % SPINNER_FRAMES.len())
                .copied()
                .unwrap_or(" ")
        } else {
            " "
        }
    }

    pub(super) fn is_workspace_group_focused(&self, group_id: DockGroupId) -> bool {
        let Some(focused_pane) = self.focused_workspace_pane() else {
            return false;
        };

        self.workspace_group(group_id)
            .is_some_and(|group| group.active == focused_pane)
    }

    pub(super) fn is_pane_folded(&self, pane: WorkspacePaneKind) -> bool {
        self.folded_panes.iter().any(|folded| folded.pane == pane)
    }

    pub(super) fn workspace_visible_pane_count(&self) -> usize {
        self.dock_groups
            .values()
            .map(|group| group.tabs.len())
            .sum()
    }

    pub(super) fn workspace_area_size(&self) -> Size {
        Size::new(self.window_width.max(1.0), self.workspace_height())
    }

    pub(super) fn workspace_bounds(&self) -> Rectangle {
        let size = self.workspace_area_size();
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
        }
    }

    pub(super) fn workspace_height(&self) -> f32 {
        let reserved_height =
            crate::status_bar::HEIGHT + transport_bar::HEIGHT + dock_view::TOOLBAR_HEIGHT;

        (self.window_height - reserved_height).max(1.0)
    }
}

pub(super) fn build_dock_runtime(
    root: Option<&DockNodeSettings>,
) -> (
    Option<DockNode>,
    HashMap<DockGroupId, DockGroup>,
    DockGroupId,
    pane_grid::State<DockGroupId>,
) {
    let mut next_id = 1;
    let mut groups = HashMap::new();
    let layout = root.map(|root| dock_node_from_settings(root, &mut next_id, &mut groups));
    let workspace_panes = build_workspace_panes(layout.as_ref());

    (layout, groups, next_id, workspace_panes)
}

pub(super) fn first_active_workspace_pane(
    node: &DockNode,
    groups: &HashMap<DockGroupId, DockGroup>,
) -> Option<WorkspacePaneKind> {
    match node {
        DockNode::Group(group_id) => groups.get(group_id).map(|group| group.active),
        DockNode::Split { first, second, .. } => first_active_workspace_pane(first, groups)
            .or_else(|| first_active_workspace_pane(second, groups)),
    }
}

pub(super) fn dock_node_from_settings(
    node: &DockNodeSettings,
    next_id: &mut DockGroupId,
    groups: &mut HashMap<DockGroupId, DockGroup>,
) -> DockNode {
    match node {
        DockNodeSettings::Group(group) => {
            let group_id = *next_id;
            *next_id = next_id.saturating_add(1);
            let mut tabs = group.tabs.clone();
            if tabs.is_empty() {
                tabs.push(WorkspacePaneKind::Score);
            }
            let active = if tabs.contains(&group.active) {
                group.active
            } else {
                tabs.first().copied().unwrap_or(WorkspacePaneKind::Score)
            };
            groups.insert(group_id, DockGroup { tabs, active });
            DockNode::Group(group_id)
        }
        DockNodeSettings::Split {
            axis,
            ratio,
            first,
            second,
        } => DockNode::Split {
            axis: pane_grid_axis_from_settings(*axis),
            ratio: ratio.clamp(0.05, 0.95),
            first: Box::new(dock_node_from_settings(first, next_id, groups)),
            second: Box::new(dock_node_from_settings(second, next_id, groups)),
        },
    }
}

pub(super) fn build_workspace_panes(layout: Option<&DockNode>) -> pane_grid::State<DockGroupId> {
    let Some(layout) = layout else {
        return pane_grid::State::new(0).0;
    };
    let configuration = configuration_from_dock_node(layout);

    match configuration {
        pane_grid::Configuration::Pane(group_id) => pane_grid::State::new(group_id).0,
        configuration => pane_grid::State::with_configuration(configuration),
    }
}

pub(super) fn configuration_from_dock_node(
    layout: &DockNode,
) -> pane_grid::Configuration<DockGroupId> {
    match layout {
        DockNode::Group(group_id) => pane_grid::Configuration::Pane(*group_id),
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => pane_grid::Configuration::Split {
            axis: *axis,
            ratio: *ratio,
            a: Box::new(configuration_from_dock_node(first)),
            b: Box::new(configuration_from_dock_node(second)),
        },
    }
}

pub(super) fn dock_node_from_workspace_state(
    state: &pane_grid::State<DockGroupId>,
) -> Option<DockNode> {
    dock_node_from_layout_node(state, state.layout())
}

pub(super) fn dock_node_from_layout_node(
    state: &pane_grid::State<DockGroupId>,
    node: &pane_grid::Node,
) -> Option<DockNode> {
    match node {
        pane_grid::Node::Pane(pane) => dock_node_from_pane(state, *pane),
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => dock_node_from_split(state, *axis, *ratio, a.as_ref(), b.as_ref()),
    }
}

pub(super) fn dock_node_from_pane(
    state: &pane_grid::State<DockGroupId>,
    pane: pane_grid::Pane,
) -> Option<DockNode> {
    Some(DockNode::Group(*state.get(pane)?))
}

pub(super) fn dock_node_from_split(
    state: &pane_grid::State<DockGroupId>,
    axis: pane_grid::Axis,
    ratio: f32,
    first: &pane_grid::Node,
    second: &pane_grid::Node,
) -> Option<DockNode> {
    Some(DockNode::Split {
        axis,
        ratio,
        first: Box::new(dock_node_from_layout_node(state, first)?),
        second: Box::new(dock_node_from_layout_node(state, second)?),
    })
}

pub(super) fn pane_grid_axis_from_settings(axis: DockAxis) -> pane_grid::Axis {
    match axis {
        DockAxis::Horizontal => pane_grid::Axis::Horizontal,
        DockAxis::Vertical => pane_grid::Axis::Vertical,
    }
}

pub(super) fn folded_pane_from_settings(settings: FoldedPaneSettings) -> FoldedPaneState {
    FoldedPaneState {
        pane: settings.pane,
        restore: match settings.restore {
            FoldedPaneRestoreSettings::Tab { anchor } => FoldedPaneRestore::Tab { anchor },
            FoldedPaneRestoreSettings::Standalone => FoldedPaneRestore::Standalone,
            FoldedPaneRestoreSettings::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => FoldedPaneRestore::Split {
                anchor,
                axis: pane_grid_axis_from_settings(axis),
                ratio,
                insert_first,
                sibling_panes,
            },
        },
    }
}

pub(super) fn folded_pane_to_settings(state: FoldedPaneState) -> FoldedPaneSettings {
    FoldedPaneSettings {
        pane: state.pane,
        restore: match state.restore {
            FoldedPaneRestore::Tab { anchor } => FoldedPaneRestoreSettings::Tab { anchor },
            FoldedPaneRestore::Standalone => FoldedPaneRestoreSettings::Standalone,
            FoldedPaneRestore::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => FoldedPaneRestoreSettings::Split {
                anchor,
                axis: dock_axis_to_settings(axis),
                ratio,
                insert_first,
                sibling_panes,
            },
        },
    }
}

pub(super) fn normalize_loaded_folded_panes(
    dock_groups: &HashMap<DockGroupId, DockGroup>,
    folded_panes: &mut Vec<FoldedPaneState>,
) {
    let visible_panes = visible_panes_in_groups(dock_groups);
    let mut seen_folded = HashSet::new();
    folded_panes
        .retain(|folded| !visible_panes.contains(&folded.pane) && seen_folded.insert(folded.pane));

    if !visible_panes.contains(&WorkspacePaneKind::Mixer)
        && !folded_panes
            .iter()
            .any(|folded| folded.pane == WorkspacePaneKind::Mixer)
    {
        folded_panes.push(FoldedPaneState {
            pane: WorkspacePaneKind::Mixer,
            restore: FoldedPaneRestore::Standalone,
        });
    }
}

fn visible_panes_in_groups(
    dock_groups: &HashMap<DockGroupId, DockGroup>,
) -> HashSet<WorkspacePaneKind> {
    dock_groups
        .values()
        .flat_map(|group| group.tabs.iter().copied())
        .collect()
}

pub(super) fn migrate_workspace_layout(
    root: &mut Option<DockNodeSettings>,
    folded_panes: &[FoldedPaneSettings],
) {
    if root
        .as_ref()
        .is_some_and(|root| dock_node_settings_contains_pane(root, WorkspacePaneKind::Logger))
        || folded_panes
            .iter()
            .any(|folded| folded.pane == WorkspacePaneKind::Logger)
    {
        return;
    }

    let previous_root = root.take().unwrap_or_default();
    *root = Some(DockNodeSettings::Split {
        axis: DockAxis::Horizontal,
        ratio: 0.74,
        first: Box::new(previous_root),
        second: Box::new(DockNodeSettings::Group(settings::DockGroupSettings {
            tabs: vec![WorkspacePaneKind::Logger],
            active: WorkspacePaneKind::Logger,
        })),
    });
}

pub(super) fn dock_node_settings_contains_pane(
    node: &DockNodeSettings,
    pane: WorkspacePaneKind,
) -> bool {
    match node {
        DockNodeSettings::Group(group) => group.tabs.contains(&pane),
        DockNodeSettings::Split { first, second, .. } => {
            dock_node_settings_contains_pane(first, pane)
                || dock_node_settings_contains_pane(second, pane)
        }
    }
}

pub(super) fn dock_axis_to_settings(axis: pane_grid::Axis) -> DockAxis {
    match axis {
        pane_grid::Axis::Horizontal => DockAxis::Horizontal,
        pane_grid::Axis::Vertical => DockAxis::Vertical,
    }
}

pub(super) fn contains_group(node: &DockNode, group_id: DockGroupId) -> bool {
    match node {
        DockNode::Group(candidate) => *candidate == group_id,
        DockNode::Split { first, second, .. } => {
            contains_group(first, group_id) || contains_group(second, group_id)
        }
    }
}

pub(super) fn selected_score_from_path(path: PathBuf) -> Result<SelectedScore, String> {
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let file_name = path
        .file_name()
        .ok_or_else(|| "Selected path has no file name".to_string())?
        .to_str()
        .ok_or_else(|| "Selected file name is not valid UTF-8".to_string())?
        .to_string();

    Ok(SelectedScore { path, file_name })
}

pub(super) fn selected_score_stem(selected_file_name: &str) -> Result<&str, String> {
    std::path::Path::new(selected_file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| "Selected score name has no valid stem".to_string())
}

pub(super) fn default_project_name(project_root: &std::path::Path) -> String {
    project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Untitled Project".to_string())
}
