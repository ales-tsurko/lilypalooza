use super::*;

impl LilyView {
    pub(in crate::app) fn handle_pane_message(&mut self, message: PaneMessage) -> Task<Message> {
        match message {
            PaneMessage::WorkspaceResized(event) => {
                let ratio = self.constrained_workspace_split_ratio(event.split, event.ratio);
                self.workspace_panes.resize(event.split, ratio);
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.sync_dock_layout_from_workspace_state();
                self.persist_settings();
            }
            PaneMessage::WorkspaceTabPressed(kind) => {
                self.set_active_workspace_pane(kind);
                self.set_focused_workspace_pane(kind);
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.pressed_workspace_pane = Some(kind);
                self.workspace_drag_origin = None;
                self.dock_drop_target = None;
                self.persist_settings();
                return self.restore_runtime_view_state(kind);
            }
            PaneMessage::FocusWorkspacePane(kind) => {
                self.set_focused_workspace_pane(kind);
            }
            PaneMessage::WorkspaceTabHovered(kind) => {
                self.hovered_workspace_pane = kind;
            }
            PaneMessage::OpenHeaderOverflowMenu(group_id) => {
                if let Some(group) = self.workspace_group(group_id) {
                    self.set_focused_workspace_pane(group.active);
                }
                self.open_project_menu = false;
                self.open_project_recent = false;
                self.open_header_overflow_menu = Some(group_id);
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.hovered_editor_file_menu_section = None;
            }
            PaneMessage::SetEditorHeaderMenuSection(section) => {
                self.open_editor_menu_section = section;
                if section != Some(super::EditorHeaderMenuSection::File) {
                    self.open_editor_file_menu_section = None;
                    self.hovered_editor_file_menu_section = None;
                }
            }
            PaneMessage::HoverEditorFileMenuSection { section, expanded } => {
                self.hovered_editor_file_menu_section = section;
                self.open_editor_file_menu_section = if expanded { section } else { None };
            }
            PaneMessage::CloseHeaderOverflowMenu => {
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.hovered_editor_file_menu_section = None;
            }
            PaneMessage::ToggleProjectMenu => {
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.hovered_editor_file_menu_section = None;
                self.open_project_menu = !self.open_project_menu;
                if !self.open_project_menu {
                    self.open_project_recent = false;
                }
            }
            PaneMessage::CloseProjectMenu => {
                self.open_project_menu = false;
                self.open_project_recent = false;
            }
            PaneMessage::SetProjectRecentOpen(open) => {
                self.open_project_recent = open;
            }
            PaneMessage::ToggleWorkspacePane(pane) => {
                self.open_project_menu = false;
                self.open_project_recent = false;
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.hovered_editor_file_menu_section = None;
                let changed = if self.is_pane_folded(pane) {
                    self.unfold_workspace_pane(pane)
                } else {
                    self.fold_workspace_pane(pane)
                };
                if changed {
                    if self.group_for_pane(pane).is_some() {
                        self.set_focused_workspace_pane(pane);
                    } else {
                        self.normalize_focused_workspace_pane();
                    }
                    self.persist_settings();
                    return self.restore_runtime_view_state(pane);
                }
            }
            PaneMessage::WorkspaceDragMoved(position) => {
                if self.dragged_workspace_pane.is_none()
                    && let Some(pressed_pane) = self.pressed_workspace_pane
                {
                    match self.workspace_drag_origin {
                        Some(origin) if drag_distance(origin, position) >= DRAG_START_THRESHOLD => {
                            self.dragged_workspace_pane = Some(pressed_pane);
                            self.dock_drop_target =
                                self.group_for_pane(pressed_pane)
                                    .map(|group_id| DockDropTarget {
                                        group_id,
                                        region: DockDropRegion::Center,
                                    });
                        }
                        Some(_) => {}
                        None => {
                            self.workspace_drag_origin = Some(position);
                        }
                    }
                }

                if self.dragged_workspace_pane.is_some() {
                    self.dock_drop_target = self.dock_drop_target_for(position);
                }
            }
            PaneMessage::WorkspaceDragReleased => {
                self.pressed_workspace_pane = None;

                if let Some(dragged_pane) = self.dragged_workspace_pane
                    && let Some(target) = self.dock_drop_target
                {
                    self.apply_dock_drop(dragged_pane, target);
                    self.persist_settings();
                    self.clear_workspace_drag_state();
                    self.open_editor_menu_section = None;
                    self.open_editor_file_menu_section = None;
                    self.hovered_editor_file_menu_section = None;
                    return self.restore_runtime_view_state(dragged_pane);
                }

                self.clear_workspace_drag_state();
            }
            PaneMessage::WorkspaceDragExited => {
                if self.dragged_workspace_pane.is_some() {
                    self.dock_drop_target = None;
                }
            }
        }

        Task::none()
    }

    pub(in crate::app) fn rebuild_workspace_panes(&mut self) {
        self.workspace_panes = build_workspace_panes(self.dock_layout.as_ref());
    }

    pub(in crate::app) fn sync_dock_layout_from_workspace_state(&mut self) {
        if self.dock_groups.is_empty() {
            self.dock_layout = None;
        } else if let Some(layout) = dock_node_from_workspace_state(&self.workspace_panes) {
            self.dock_layout = Some(layout);
        }
    }

    pub(in crate::app) fn constrained_workspace_split_ratio(
        &self,
        split: pane_grid::Split,
        ratio: f32,
    ) -> f32 {
        let split_regions =
            self.workspace_panes
                .layout()
                .split_regions(0.0, 0.0, self.workspace_area_size());
        let Some((axis, region, _)) = split_regions.get(&split).copied() else {
            return ratio.clamp(0.05, 0.95);
        };

        if axis != pane_grid::Axis::Vertical {
            return ratio.clamp(0.05, 0.95);
        }

        let Some((first, second)) = split_children(self.workspace_panes.layout(), split) else {
            return ratio.clamp(0.05, 0.95);
        };

        let total_width = region.width.max(1.0);
        let min_first = dock_node_min_width(first, &self.workspace_panes, self).min(total_width);
        let min_second = dock_node_min_width(second, &self.workspace_panes, self).min(total_width);
        let min_ratio = (min_first / total_width).clamp(0.05, 0.95);
        let max_ratio = (1.0 - min_second / total_width).clamp(0.05, 0.95);

        if min_ratio > max_ratio {
            ratio.clamp(0.05, 0.95)
        } else {
            ratio.clamp(min_ratio, max_ratio)
        }
    }

    pub(in crate::app) fn set_active_workspace_pane(&mut self, pane: WorkspacePaneKind) {
        let Some(group_id) = self.group_for_pane(pane) else {
            return;
        };
        let Some(group) = self.dock_groups.get_mut(&group_id) else {
            return;
        };

        if group.tabs.contains(&pane) {
            group.active = pane;
        }
    }

    pub(in crate::app) fn switch_focused_workspace_tab(&mut self, direction: TabDirection) -> bool {
        let Some(focused_pane) = self.focused_workspace_pane() else {
            return false;
        };
        let Some(group_id) = self.group_for_pane(focused_pane) else {
            return false;
        };
        let Some(group) = self.workspace_group(group_id) else {
            return false;
        };

        if group.tabs.len() <= 1 {
            return false;
        }

        let Some(active_index) = group.tabs.iter().position(|pane| *pane == group.active) else {
            return false;
        };

        let next_index = match direction {
            TabDirection::Previous => {
                if active_index == 0 {
                    group.tabs.len() - 1
                } else {
                    active_index - 1
                }
            }
            TabDirection::Next => (active_index + 1) % group.tabs.len(),
        };

        let next_pane = group.tabs[next_index];
        self.set_active_workspace_pane(next_pane);
        self.set_focused_workspace_pane(next_pane);
        true
    }

    pub(in crate::app) fn cycle_workspace_pane_focus(
        &mut self,
        direction: PaneCycleDirection,
    ) -> bool {
        let Some(focused_pane) = self.focused_workspace_pane() else {
            return false;
        };
        let Some(current_group_id) = self.group_for_pane(focused_pane) else {
            return false;
        };

        let ordered_groups = self.visible_workspace_group_order();
        if ordered_groups.len() <= 1 {
            return false;
        }

        let Some(current_index) = ordered_groups
            .iter()
            .position(|group_id| *group_id == current_group_id)
        else {
            return false;
        };

        let next_index = match direction {
            PaneCycleDirection::Previous => {
                if current_index == 0 {
                    ordered_groups.len() - 1
                } else {
                    current_index - 1
                }
            }
            PaneCycleDirection::Next => (current_index + 1) % ordered_groups.len(),
        };

        let Some(target_pane) = self
            .workspace_group(ordered_groups[next_index])
            .map(|group| group.active)
        else {
            return false;
        };

        self.set_focused_workspace_pane(target_pane);
        true
    }

    pub(in crate::app) fn visible_workspace_group_order(&self) -> Vec<DockGroupId> {
        let mut group_ids = Vec::new();

        if let Some(layout) = self.dock_layout.as_ref() {
            collect_visible_group_order(layout, &mut group_ids);
        } else {
            group_ids.extend(self.dock_groups.keys().copied());
            group_ids.sort_unstable();
        }

        group_ids.retain(|group_id| self.dock_groups.contains_key(group_id));
        group_ids.dedup();
        group_ids
    }

    pub(in crate::app) fn dock_drop_target_for(
        &self,
        position: iced::Point,
    ) -> Option<DockDropTarget> {
        let bounds_map = self.workspace_group_bounds();
        let (group_id, bounds) = bounds_map
            .into_iter()
            .find(|(_, bounds)| bounds.contains(position))?;

        Some(DockDropTarget {
            group_id,
            region: dock_drop_region(bounds, position),
        })
    }

    pub(in crate::app) fn workspace_group_bounds(
        &self,
    ) -> std::collections::HashMap<DockGroupId, iced::Rectangle> {
        let mut bounds = std::collections::HashMap::new();
        let root_bounds = self.workspace_bounds();
        collect_workspace_group_bounds(
            &self.workspace_panes,
            self.workspace_panes.layout(),
            root_bounds,
            &mut bounds,
        );
        bounds
    }

    pub(in crate::app) fn fold_workspace_pane(&mut self, pane: WorkspacePaneKind) -> bool {
        if self.is_pane_folded(pane) {
            return false;
        }

        let Some(group_id) = self.group_for_pane(pane) else {
            return false;
        };
        let Some(group) = self.dock_groups.get(&group_id) else {
            return false;
        };

        let restore = if group.tabs.len() > 1 {
            let Some(anchor) = group
                .tabs
                .iter()
                .copied()
                .find(|candidate| *candidate != pane)
            else {
                return false;
            };
            let _ = remove_pane_from_group(&mut self.dock_groups, group_id, pane);
            FoldedPaneRestore::Tab { anchor }
        } else {
            self.dock_groups.remove(&group_id);
            if let Some((axis, ratio, insert_first, anchor, sibling_panes)) =
                self.dock_layout.as_ref().and_then(|layout| {
                    split_restore_target_for_group(layout, group_id, &self.dock_groups)
                })
            {
                let layout = self.dock_layout.take().unwrap_or(DockNode::Group(group_id));
                self.dock_layout = Some(prune_group_from_layout(layout, group_id));
                FoldedPaneRestore::Split {
                    anchor,
                    axis,
                    ratio,
                    insert_first,
                    sibling_panes,
                }
            } else {
                self.dock_layout = None;
                FoldedPaneRestore::Standalone
            }
        };

        self.folded_panes.retain(|folded| folded.pane != pane);
        self.folded_panes.push(FoldedPaneState { pane, restore });
        if pane == WorkspacePaneKind::PianoRoll {
            self.piano_roll.visible = false;
        }

        self.clear_workspace_drag_state();
        self.rebuild_workspace_panes();
        self.normalize_focused_workspace_pane();
        true
    }

    pub(in crate::app) fn unfold_workspace_pane(&mut self, pane: WorkspacePaneKind) -> bool {
        let Some(index) = self
            .folded_panes
            .iter()
            .position(|folded| folded.pane == pane)
        else {
            return false;
        };
        let folded = self.folded_panes.remove(index);

        let restored = match folded.restore {
            FoldedPaneRestore::Tab { anchor } => self.restore_folded_pane_as_tab(pane, anchor),
            FoldedPaneRestore::Standalone => self.restore_folded_pane_as_standalone(pane),
            FoldedPaneRestore::Split {
                anchor,
                axis,
                ratio,
                insert_first,
                sibling_panes,
            } => self.restore_folded_pane_as_split(
                pane,
                anchor,
                axis,
                ratio,
                insert_first,
                &sibling_panes,
            ),
        };

        if !restored {
            if self.dock_groups.is_empty() {
                let _ = self.restore_folded_pane_as_standalone(pane);
            } else if let Some(group_id) =
                self.dock_layout.as_ref().and_then(first_group_id_in_layout)
            {
                if let Some(group) = self.dock_groups.get_mut(&group_id) {
                    group.tabs.push(pane);
                    group.active = pane;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        if pane == WorkspacePaneKind::PianoRoll {
            self.piano_roll.visible = true;
        }

        self.rebuild_workspace_panes();
        self.set_focused_workspace_pane(pane);
        true
    }

    pub(in crate::app) fn restore_folded_pane_as_tab(
        &mut self,
        pane: WorkspacePaneKind,
        anchor: WorkspacePaneKind,
    ) -> bool {
        let Some(group_id) = self.group_for_pane(anchor) else {
            return false;
        };
        let Some(group) = self.dock_groups.get_mut(&group_id) else {
            return false;
        };

        group.tabs.retain(|candidate| *candidate != pane);
        group.tabs.push(pane);
        group.active = pane;
        true
    }

    pub(in crate::app) fn restore_folded_pane_as_standalone(
        &mut self,
        pane: WorkspacePaneKind,
    ) -> bool {
        if let Some(group_id) = self.dock_layout.as_ref().and_then(first_group_id_in_layout)
            && let Some(group) = self.dock_groups.get_mut(&group_id)
        {
            group.tabs.push(pane);
            group.active = pane;
            return true;
        }

        let new_group_id = self.next_dock_group_id;
        self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
        self.dock_groups.insert(
            new_group_id,
            DockGroup {
                tabs: vec![pane],
                active: pane,
            },
        );
        self.dock_layout = Some(DockNode::Group(new_group_id));
        true
    }

    pub(in crate::app) fn restore_folded_pane_as_split(
        &mut self,
        pane: WorkspacePaneKind,
        anchor: WorkspacePaneKind,
        axis: pane_grid::Axis,
        ratio: f32,
        insert_first: bool,
        sibling_panes: &[WorkspacePaneKind],
    ) -> bool {
        let new_group_id = self.next_dock_group_id;
        self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
        self.dock_groups.insert(
            new_group_id,
            DockGroup {
                tabs: vec![pane],
                active: pane,
            },
        );

        if self.dock_layout.as_mut().is_some_and(|layout| {
            replace_subtree_with_split(
                layout,
                axis,
                ratio,
                new_group_id,
                insert_first,
                sibling_panes,
                &self.dock_groups,
            )
        }) {
            return true;
        }

        let Some(group_id) = self.group_for_pane(anchor) else {
            self.dock_groups.remove(&new_group_id);
            return false;
        };
        let Some(layout) = self.dock_layout.as_mut() else {
            self.dock_groups.remove(&new_group_id);
            return false;
        };

        if replace_group_with_split(layout, group_id, axis, ratio, new_group_id, insert_first) {
            true
        } else {
            self.dock_groups.remove(&new_group_id);
            false
        }
    }

    pub(in crate::app) fn apply_dock_drop(
        &mut self,
        dragged: WorkspacePaneKind,
        target: DockDropTarget,
    ) {
        let Some(source_group_id) = self.group_for_pane(dragged) else {
            return;
        };
        if !self.dock_groups.contains_key(&target.group_id) {
            return;
        }

        match target.region {
            DockDropRegion::Center if source_group_id == target.group_id => {
                if let Some(group) = self.dock_groups.get_mut(&source_group_id) {
                    move_tab_to_front(&mut group.tabs, dragged);
                    group.active = dragged;
                }
            }
            DockDropRegion::Center => {
                let source_empty =
                    remove_pane_from_group(&mut self.dock_groups, source_group_id, dragged);

                if source_empty {
                    self.dock_groups.remove(&source_group_id);
                    let layout = self
                        .dock_layout
                        .take()
                        .unwrap_or(DockNode::Group(target.group_id));
                    self.dock_layout = Some(prune_group_from_layout(layout, source_group_id));
                }

                if let Some(target_group) = self.dock_groups.get_mut(&target.group_id) {
                    target_group.tabs.retain(|pane| *pane != dragged);
                    target_group.tabs.push(dragged);
                    target_group.active = dragged;
                }
            }
            region => {
                if source_group_id == target.group_id
                    && self
                        .dock_groups
                        .get(&source_group_id)
                        .is_some_and(|group| group.tabs.len() <= 1)
                {
                    return;
                }

                let source_empty =
                    remove_pane_from_group(&mut self.dock_groups, source_group_id, dragged);

                if source_empty && source_group_id != target.group_id {
                    self.dock_groups.remove(&source_group_id);
                    let layout = self
                        .dock_layout
                        .take()
                        .unwrap_or(DockNode::Group(target.group_id));
                    self.dock_layout = Some(prune_group_from_layout(layout, source_group_id));
                }

                let new_group_id = self.next_dock_group_id;
                self.next_dock_group_id = self.next_dock_group_id.saturating_add(1);
                self.dock_groups.insert(
                    new_group_id,
                    DockGroup {
                        tabs: vec![dragged],
                        active: dragged,
                    },
                );

                let (axis, insert_first) = match region {
                    DockDropRegion::Top => (pane_grid::Axis::Horizontal, true),
                    DockDropRegion::Bottom => (pane_grid::Axis::Horizontal, false),
                    DockDropRegion::Left => (pane_grid::Axis::Vertical, true),
                    DockDropRegion::Right => (pane_grid::Axis::Vertical, false),
                    DockDropRegion::Center => unreachable!(),
                };

                if let Some(layout) = self.dock_layout.as_mut() {
                    replace_group_with_split(
                        layout,
                        target.group_id,
                        axis,
                        0.5,
                        new_group_id,
                        insert_first,
                    );
                } else {
                    self.dock_layout = Some(DockNode::Group(new_group_id));
                }
            }
        }

        self.rebuild_workspace_panes();
        self.set_focused_workspace_pane(dragged);
    }

    pub(in crate::app) fn clear_workspace_drag_state(&mut self) {
        self.hovered_workspace_pane = None;
        self.pressed_workspace_pane = None;
        self.workspace_drag_origin = None;
        self.dragged_workspace_pane = None;
        self.dock_drop_target = None;
    }
}
