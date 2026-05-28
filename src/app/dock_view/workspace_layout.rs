use super::*;

pub(super) fn workspace_tab_foreground_color(
    is_active: bool,
    is_focused: bool,
    is_hovered: bool,
    is_dragging: bool,
) -> impl Fn(&Theme) -> Color + Copy {
    move |theme: &Theme| {
        let palette = theme.extended_palette();
        let mix = |a: Color, b: Color, amount: f32| Color {
            r: a.r + (b.r - a.r) * amount,
            g: a.g + (b.g - a.g) * amount,
            b: a.b + (b.b - a.b) * amount,
            a: a.a + (b.a - a.a) * amount,
        };
        if is_dragging {
            palette.primary.weak.text
        } else if is_active && is_focused {
            palette.background.base.text
        } else if is_active {
            mix(
                palette.background.base.text,
                palette.background.strong.color,
                0.38,
            )
        } else if is_hovered {
            palette.background.base.text
        } else {
            mix(
                palette.background.base.text,
                palette.background.strong.color,
                0.52,
            )
        }
    }
}

pub(super) fn workspace_pane_title(pane: WorkspacePaneKind) -> &'static str {
    match pane {
        WorkspacePaneKind::Score => "Score",
        WorkspacePaneKind::PianoRoll => "Piano Roll",
        WorkspacePaneKind::Mixer => "Mixer",
        WorkspacePaneKind::Editor => "Editor",
        WorkspacePaneKind::Logger => "Logger",
    }
}

pub(super) fn workspace_pane_icon(pane: WorkspacePaneKind) -> svg::Handle {
    workspace_pane_icon_factory(pane)()
}

pub(super) fn workspace_pane_icon_factory(pane: WorkspacePaneKind) -> fn() -> svg::Handle {
    all_workspace_panes()
        .into_iter()
        .zip(WORKSPACE_PANE_ICONS)
        .find_map(|(candidate, icon)| (candidate == pane).then_some(icon))
        .unwrap_or(icons::music_4)
}

pub(super) fn all_workspace_panes() -> [WorkspacePaneKind; 5] {
    [
        WorkspacePaneKind::Editor,
        WorkspacePaneKind::Score,
        WorkspacePaneKind::PianoRoll,
        WorkspacePaneKind::Mixer,
        WorkspacePaneKind::Logger,
    ]
}

pub(super) const WORKSPACE_PANE_ICONS: [fn() -> svg::Handle; 5] = [
    icons::file_pen,
    icons::music_4,
    icons::piano,
    icons::sliders_vertical,
    icons::scroll_text,
];

pub(super) fn toolbar_pane_toggle(
    app: &Lilypalooza,
    pane: WorkspacePaneKind,
) -> Element<'static, Message> {
    let is_visible = app.group_for_pane(pane).is_some();
    let title = workspace_pane_title(pane);
    let icon = workspace_pane_icon(pane);

    let icon = svg(icon)
        .width(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
        .height(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
        .content_fit(ContentFit::Contain)
        .style(move |theme: &Theme, status| {
            let palette = theme.extended_palette();
            svg::Style {
                color: Some(if is_visible {
                    match status {
                        svg::Status::Idle => palette.background.weakest.text,
                        svg::Status::Hovered => palette.background.base.text,
                    }
                } else {
                    match status {
                        svg::Status::Idle => palette.background.base.text,
                        svg::Status::Hovered => palette.primary.weak.text,
                    }
                }),
            }
        });

    let tooltip_label = shortcuts::label_for_action(
        &app.shortcut_settings,
        shortcuts::ShortcutAction::ToggleWorkspacePane(pane),
    )
    .map(|shortcut| format!("{title} ({shortcut})"))
    .unwrap_or_else(|| title.to_string());

    delayed_tooltip(
        app,
        format!("toolbar-pane-toggle-{pane:?}"),
        button(icon)
            .style(if is_visible {
                ui_style::button_toolbar_toggle_active
            } else {
                ui_style::button_toolbar_chip
            })
            .padding([ui_style::grid(2), ui_style::grid(2)])
            .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
            .on_press(Message::Pane(PaneMessage::ToggleWorkspacePane(pane)))
            .into(),
        text(tooltip_label).size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Bottom,
    )
}

pub(super) fn empty_workspace_placeholder(app: &Lilypalooza) -> Element<'_, Message> {
    let lilypond_label: Element<'_, Message> = match &app.lilypond_status {
        crate::app::LilypondStatus::Checking => row![
            text(app.spinner_frame())
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(fonts::MONO),
            text("LilyPond: checking...").size(ui_style::FONT_SIZE_UI_SM),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into(),
        crate::app::LilypondStatus::Ready { detected, .. } => text(format!("LilyPond: {detected}"))
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO)
            .into(),
        crate::app::LilypondStatus::Unavailable => text("LilyPond: unavailable")
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO)
            .into(),
    };

    container(
        Column::new()
            .push(
                text(format!("Lilypalooza {}", env!("CARGO_PKG_VERSION")))
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(fonts::MONO),
            )
            .push(lilypond_label)
            .spacing(ui_style::SPACE_SM)
            .align_x(alignment::Horizontal::Center),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill)
    .into()
}

pub(super) fn header_overflow_button(
    app: &Lilypalooza,
    group_id: crate::app::DockGroupId,
    is_open: bool,
) -> Element<'static, Message> {
    let on_press = if is_open {
        Message::Pane(PaneMessage::CloseHeaderOverflowMenu)
    } else {
        Message::Pane(PaneMessage::OpenHeaderOverflowMenu(group_id))
    };
    let button = button(header_icon(
        icons::ellipsis_vertical(),
        HEADER_MENU_ICON_SIZE,
    ))
    .style(ui_style::button_pane_header_control)
    .padding([4, 7])
    .width(Length::Fixed(HEADER_MENU_BUTTON_WIDTH))
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
    .on_press(on_press);

    let tooltip = if is_open {
        "Hide pane controls"
    } else {
        "Show pane controls"
    };

    delayed_tooltip(
        app,
        format!("header-overflow-{group_id}"),
        container(button).padding([0, 2]).into(),
        text(tooltip).size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Top,
    )
}

pub(super) fn header_overflow_trigger(
    app: &Lilypalooza,
    group_id: crate::app::DockGroupId,
    is_open: bool,
) -> Element<'static, Message> {
    if app
        .workspace_group(group_id)
        .is_some_and(|group| group.active == WorkspacePaneKind::Editor)
    {
        let browser_open = app.editor.file_browser_expanded();
        row![
            delayed_tooltip(
                app,
                format!("editor-file-browser-toggle-{group_id}"),
                container(
                    button(header_icon(icons::folder_tree(), HEADER_MENU_ICON_SIZE))
                        .style(if browser_open {
                            ui_style::button_pane_header_control_active
                        } else {
                            ui_style::button_pane_header_control
                        })
                        .padding([4, 7])
                        .width(Length::Fixed(HEADER_MENU_BUTTON_WIDTH))
                        .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
                        .on_press(Message::Editor(
                            crate::app::EditorMessage::ToggleFileBrowser
                        )),
                )
                .padding([0, 2])
                .into(),
                text("Toggle file browser")
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .into(),
                tooltip::Position::Top,
            ),
            header_overflow_button(app, group_id, is_open),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .into()
    } else {
        header_overflow_button(app, group_id, is_open)
    }
}

pub(super) fn header_overflow_menu_panel<'a>(
    controls: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        Column::with_children(controls)
            .spacing(ui_style::SPACE_XS)
            .align_x(alignment::Horizontal::Left),
    )
    .padding(ui_style::PADDING_XS)
    .style(ui_style::tooltip_popup)
    .into()
}

pub(in crate::app) fn workspace_group_min_width(
    app: &Lilypalooza,
    group_id: crate::app::DockGroupId,
) -> f32 {
    let Some(group) = app.workspace_group(group_id) else {
        return 0.0;
    };
    let tabs_width = group_tabs_min_width(group);
    let menu_width = if pane_header_has_controls(app, group.active) {
        HEADER_MENU_BUTTON_WIDTH
    } else {
        0.0
    };

    let min_content_width = if group.tabs.contains(&WorkspacePaneKind::Mixer) {
        super::mixer::MIXER_MIN_WIDTH
    } else {
        0.0
    };

    (tabs_width + menu_width + HEADER_WIDTH_SAFETY).max(min_content_width)
}

pub(in crate::app) fn workspace_group_min_height(
    app: &Lilypalooza,
    group_id: crate::app::DockGroupId,
) -> f32 {
    let Some(group) = app.workspace_group(group_id) else {
        return 0.0;
    };

    if group.tabs.contains(&WorkspacePaneKind::Mixer) {
        super::mixer::MIXER_MIN_HEIGHT
    } else {
        0.0
    }
}

pub(super) fn workspace_tab_min_width(pane: WorkspacePaneKind) -> f32 {
    let title_width = match pane {
        WorkspacePaneKind::Score => 36.0,
        WorkspacePaneKind::PianoRoll => 66.0,
        WorkspacePaneKind::Mixer => 34.0,
        WorkspacePaneKind::Editor => 38.0,
        WorkspacePaneKind::Logger => 42.0,
    };

    TOOLBAR_ICON_SIZE
        + TAB_ICON_GAP as f32
        + title_width
        + (ui_style::PADDING_STATUS_BAR_H + 8) as f32 * 2.0
}

pub(super) fn group_tabs_min_width(group: &crate::app::DockGroup) -> f32 {
    group
        .tabs
        .iter()
        .copied()
        .map(workspace_tab_min_width)
        .sum::<f32>()
        + ui_style::SPACE_XS as f32 * group.tabs.len().saturating_sub(1) as f32
}

pub(super) fn split_header_control_groups<'a>(
    groups: Vec<HeaderControlGroup<'a>>,
    available_width: f32,
) -> (Vec<Element<'a, Message>>, Vec<Element<'a, Message>>) {
    let total_width = header_group_widths_total(groups.iter().map(|group| group.min_width));
    if groups.is_empty() || total_width <= available_width {
        return (
            groups.into_iter().map(|group| group.content).collect(),
            Vec::new(),
        );
    }

    split_header_control_groups_with_menu(groups, available_width)
}

pub(super) fn split_header_control_groups_with_menu<'a>(
    groups: Vec<HeaderControlGroup<'a>>,
    available_width: f32,
) -> (Vec<Element<'a, Message>>, Vec<Element<'a, Message>>) {
    let available_inline_width = (available_width - HEADER_MENU_BUTTON_WIDTH).max(0.0);
    let mut used_width = 0.0;
    let mut inline = Vec::new();
    let mut overflow = Vec::new();

    for group in groups {
        let spacing = inline_header_group_spacing(&inline);

        if used_width + spacing + group.min_width <= available_inline_width {
            used_width += spacing + group.min_width;
            inline.push(group.content);
        } else {
            overflow.push(group.content);
        }
    }

    (inline, overflow)
}

pub(super) fn inline_header_group_spacing(inline: &[Element<'_, Message>]) -> f32 {
    if inline.is_empty() {
        0.0
    } else {
        ui_style::SPACE_SM as f32
    }
}

pub(super) fn header_group_widths_total(widths: impl IntoIterator<Item = f32>) -> f32 {
    let mut count = 0usize;
    let total = widths.into_iter().inspect(|_| count += 1).sum::<f32>();

    total + ui_style::SPACE_SM as f32 * count.saturating_sub(1) as f32
}

pub(in crate::app) fn compact_control_icon(icon: svg::Handle) -> Element<'static, Message> {
    container(ui_style::icon(icon, 12.0, ui_style::svg_window_control))
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .center_x(Length::Fixed(12.0))
        .center_y(Length::Fixed(12.0))
        .into()
}

pub(super) fn header_icon(icon: svg::Handle, size: f32) -> Element<'static, Message> {
    container(ui_style::icon(icon, size, ui_style::svg_window_control))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .center_x(Length::Fixed(size))
        .center_y(Length::Fixed(size))
        .into()
}

pub(super) fn pane_header_control_groups<'a>(
    app: &'a Lilypalooza,
    pane: WorkspacePaneKind,
) -> Vec<HeaderControlGroup<'a>> {
    if pane == WorkspacePaneKind::Score {
        return score_view::score_controls(app);
    }
    if pane == WorkspacePaneKind::PianoRoll {
        return piano_roll::controls(app);
    }
    if pane == WorkspacePaneKind::Logger {
        return logger_controls(app);
    }
    Vec::new()
}

pub(super) fn pane_header_has_controls(app: &Lilypalooza, pane: WorkspacePaneKind) -> bool {
    pane_header_always_has_controls(pane)
        || (pane == WorkspacePaneKind::Score && app.current_score.is_some())
}

pub(super) fn pane_header_always_has_controls(pane: WorkspacePaneKind) -> bool {
    matches!(
        pane,
        WorkspacePaneKind::PianoRoll | WorkspacePaneKind::Editor | WorkspacePaneKind::Logger
    )
}

pub(super) fn workspace_drag_overlay(app: &Lilypalooza, size: Size) -> Element<'_, Message> {
    let Some(target) = app.dock_drop_target else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let bounds_map = workspace_group_bounds_map(&app.workspace_panes, size);
    let Some(group_bounds) = bounds_map.get(&target.group_id).copied() else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let target_bounds = preview_bounds_for_region(group_bounds, target.region);

    canvas(DropOverlayCanvas { target_bounds })
        .width(Fill)
        .height(Fill)
        .into()
}

pub(super) fn workspace_drag_capture_layer(app: &Lilypalooza) -> Element<'_, Message> {
    if app.pressed_workspace_pane.is_none() && app.dragged_workspace_pane.is_none() {
        return container(text("")).width(Fill).height(Fill).into();
    }

    mouse_area(container(text("")).width(Fill).height(Fill))
        .on_move(|position| Message::Pane(PaneMessage::WorkspaceDragMoved(position)))
        .on_release(Message::Pane(PaneMessage::WorkspaceDragReleased))
        .on_exit(Message::Pane(PaneMessage::WorkspaceDragExited))
        .into()
}

pub(super) fn workspace_group_bounds_map(
    state: &pane_grid::State<crate::app::DockGroupId>,
    size: Size,
) -> HashMap<crate::app::DockGroupId, Rectangle> {
    let mut bounds = HashMap::new();
    let root_bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: size.width.max(1.0),
        height: size.height.max(1.0),
    };
    collect_group_bounds(state, state.layout(), root_bounds, &mut bounds);

    bounds
}

pub(super) fn collect_group_bounds(
    state: &pane_grid::State<crate::app::DockGroupId>,
    node: &pane_grid::Node,
    bounds: Rectangle,
    group_bounds: &mut HashMap<crate::app::DockGroupId, Rectangle>,
) {
    match node {
        pane_grid::Node::Pane(pane) => {
            if let Some(group_id) = state.get(*pane) {
                group_bounds.insert(*group_id, bounds);
            }
        }
        pane_grid::Node::Split {
            axis, ratio, a, b, ..
        } => collect_split_group_bounds(state, *axis, *ratio, a, b, bounds, group_bounds),
    }
}

pub(super) fn collect_split_group_bounds(
    state: &pane_grid::State<crate::app::DockGroupId>,
    axis: pane_grid::Axis,
    ratio: f32,
    a: &pane_grid::Node,
    b: &pane_grid::Node,
    bounds: Rectangle,
    group_bounds: &mut HashMap<crate::app::DockGroupId, Rectangle>,
) {
    let (first, second) = split_child_bounds(axis, ratio, bounds);
    collect_group_bounds(state, a, first, group_bounds);
    collect_group_bounds(state, b, second, group_bounds);
}

pub(super) fn split_child_bounds(
    axis: pane_grid::Axis,
    ratio: f32,
    bounds: Rectangle,
) -> (Rectangle, Rectangle) {
    split_child_bounds_on_axis(axis, ratio, bounds)
}

fn split_child_bounds_on_axis(
    axis: pane_grid::Axis,
    ratio: f32,
    bounds: Rectangle,
) -> (Rectangle, Rectangle) {
    let first_size = match axis {
        pane_grid::Axis::Horizontal => bounds.height * ratio,
        pane_grid::Axis::Vertical => bounds.width * ratio,
    };
    match axis {
        pane_grid::Axis::Horizontal => (
            Rectangle {
                height: first_size,
                ..bounds
            },
            Rectangle {
                y: bounds.y + first_size,
                height: bounds.height - first_size,
                ..bounds
            },
        ),
        pane_grid::Axis::Vertical => (
            Rectangle {
                width: first_size,
                ..bounds
            },
            Rectangle {
                x: bounds.x + first_size,
                width: bounds.width - first_size,
                ..bounds
            },
        ),
    }
}

pub(super) fn preview_bounds_for_region(bounds: Rectangle, region: DockDropRegion) -> Rectangle {
    match region {
        DockDropRegion::Left => Rectangle {
            width: bounds.width / 2.0,
            ..bounds
        },
        DockDropRegion::Right => Rectangle {
            x: bounds.x + bounds.width / 2.0,
            width: bounds.width / 2.0,
            ..bounds
        },
        DockDropRegion::Top => Rectangle {
            height: bounds.height / 2.0,
            ..bounds
        },
        DockDropRegion::Bottom => Rectangle {
            y: bounds.y + bounds.height / 2.0,
            height: bounds.height / 2.0,
            ..bounds
        },
        DockDropRegion::Center => bounds,
    }
}

pub(super) fn split_rearrange_style(theme: &Theme) -> pane_grid::Style {
    let mut style = pane_grid::default(theme);
    style.hovered_region.background = Color::TRANSPARENT.into();
    style.hovered_region.border = border::rounded(ui_style::RADIUS_NONE)
        .width(0)
        .color(Color::TRANSPARENT);
    style
}

pub(super) struct DropOverlayCanvas {
    target_bounds: Rectangle,
}

impl<Message> canvas::Program<Message> for DropOverlayCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        frame.fill_rectangle(
            Point::new(self.target_bounds.x, self.target_bounds.y),
            Size::new(self.target_bounds.width, self.target_bounds.height),
            Color::from_rgba(
                palette.primary.base.color.r,
                palette.primary.base.color.g,
                palette.primary.base.color.b,
                0.20,
            ),
        );
        frame.stroke_rectangle(
            Point::new(self.target_bounds.x, self.target_bounds.y),
            Size::new(self.target_bounds.width, self.target_bounds.height),
            canvas::Stroke {
                width: 2.0,
                style: canvas::Style::Solid(Color::from_rgba(
                    palette.primary.strong.color.r,
                    palette.primary.strong.color.g,
                    palette.primary.strong.color.b,
                    0.95,
                )),
                ..canvas::Stroke::default()
            },
        );

        vec![frame.into_geometry()]
    }
}

pub(super) fn logger_controls<'a>(app: &'a Lilypalooza) -> Vec<HeaderControlGroup<'a>> {
    let clear_button = button(compact_control_icon(icons::brush_cleaning()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let clear_button = if app.logger.is_empty() {
        clear_button
    } else {
        clear_button.on_press(Message::Logger(crate::app::LoggerMessage::RequestClear))
    };

    vec![HeaderControlGroup {
        min_width: 32.0,
        content: delayed_tooltip(
            app,
            "logger-clear",
            clear_button.into(),
            text("Clear").size(ui_style::FONT_SIZE_UI_XS).into(),
            tooltip::Position::Top,
        ),
    }]
}

pub(super) fn editor_header_menu_panel<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let root_width = EDITOR_MENU_ROOT_WIDTH;
    let root_menu = container(
        Column::new()
            .spacing(ui_style::SPACE_XS)
            .push(editor_root_menu_item(
                "File",
                app.open_editor_menu_section == Some(EditorHeaderMenuSection::File),
                EditorHeaderMenuSection::File,
            ))
            .push(editor_root_menu_item(
                "Edit",
                app.open_editor_menu_section == Some(EditorHeaderMenuSection::Edit),
                EditorHeaderMenuSection::Edit,
            ))
            .push(editor_root_menu_item(
                "Appearance",
                app.open_editor_menu_section == Some(EditorHeaderMenuSection::Appearance),
                EditorHeaderMenuSection::Appearance,
            )),
    )
    .width(Length::Fixed(root_width))
    .padding(ui_style::PADDING_XS)
    .style(ui_style::tooltip_popup);

    match app.open_editor_menu_section {
        Some(EditorHeaderMenuSection::File) => {
            let file_width = EDITOR_FILE_SUBMENU_WIDTH;

            row![
                iced::widget::column![
                    container(text("")).height(Length::Fixed(editor_submenu_offset(
                        EditorHeaderMenuSection::File,
                    ))),
                    container(editor_file_submenu(app))
                        .width(Length::Fixed(file_width))
                        .padding(ui_style::PADDING_SM)
                        .style(ui_style::tooltip_popup),
                ]
                .spacing(0),
                root_menu,
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Top)
            .into()
        }
        Some(EditorHeaderMenuSection::Edit) => {
            let submenu_width = EDITOR_EDIT_SUBMENU_WIDTH;

            row![
                iced::widget::column![
                    container(text("")).height(Length::Fixed(editor_submenu_offset(
                        EditorHeaderMenuSection::Edit,
                    ))),
                    container(editor_edit_submenu(app))
                        .width(Length::Fixed(submenu_width))
                        .padding(ui_style::PADDING_SM)
                        .style(ui_style::tooltip_popup),
                ]
                .spacing(0),
                root_menu,
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Top)
            .into()
        }
        Some(EditorHeaderMenuSection::Appearance) => {
            let submenu_width = EDITOR_APPEARANCE_SUBMENU_WIDTH;
            let submenu: Element<'a, Message> = iced::widget::column![
                container(text("")).height(Length::Fixed(editor_submenu_offset(
                    EditorHeaderMenuSection::Appearance,
                ))),
                container(editor_appearance_submenu(app))
                    .width(Length::Fixed(submenu_width))
                    .padding(Padding {
                        top: f32::from(ui_style::PADDING_MD),
                        right: f32::from(ui_style::PADDING_SM),
                        bottom: f32::from(ui_style::PADDING_MD),
                        left: f32::from(ui_style::PADDING_SM),
                    })
                    .style(ui_style::tooltip_popup),
            ]
            .spacing(0)
            .into();

            row![submenu, root_menu]
                .spacing(ui_style::SPACE_XS)
                .align_y(alignment::Vertical::Top)
                .into()
        }
        None => root_menu.into(),
    }
}

pub(super) fn editor_submenu_offset(section: EditorHeaderMenuSection) -> f32 {
    let item_index = match section {
        EditorHeaderMenuSection::File => 0.0,
        EditorHeaderMenuSection::Edit => 1.0,
        EditorHeaderMenuSection::Appearance => 2.0,
    };

    f32::from(ui_style::PADDING_XS)
        + item_index * (EDITOR_MENU_ITEM_HEIGHT + ui_style::SPACE_XS as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_bounds_for_region_uses_expected_half() {
        let bounds = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };

        assert_eq!(
            preview_bounds_for_region(bounds, DockDropRegion::Left),
            Rectangle {
                width: 50.0,
                ..bounds
            }
        );
        assert_eq!(
            preview_bounds_for_region(bounds, DockDropRegion::Right),
            Rectangle {
                x: 60.0,
                width: 50.0,
                ..bounds
            }
        );
        assert_eq!(
            preview_bounds_for_region(bounds, DockDropRegion::Top),
            Rectangle {
                height: 40.0,
                ..bounds
            }
        );
        assert_eq!(
            preview_bounds_for_region(bounds, DockDropRegion::Bottom),
            Rectangle {
                y: 60.0,
                height: 40.0,
                ..bounds
            }
        );
        assert_eq!(
            preview_bounds_for_region(bounds, DockDropRegion::Center),
            bounds
        );
    }
}
