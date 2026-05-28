use super::*;

pub(super) fn editor_file_browser_toolbar_button<'a>(
    app: &'a Lilypalooza,
    key: impl Into<String>,
    icon: svg::Handle,
    tooltip_label: &'static str,
    on_press: Message,
    active: bool,
) -> Element<'a, Message> {
    delayed_tooltip(
        app,
        key.into(),
        button(
            svg(icon)
                .width(Length::Fixed(ui_style::grid_f32(4)))
                .height(Length::Fixed(ui_style::grid_f32(4)))
                .content_fit(ContentFit::Contain)
                .style(|theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let color = match status {
                        svg::Status::Idle => palette.background.base.text,
                        svg::Status::Hovered => palette.background.base.text,
                    };
                    svg::Style { color: Some(color) }
                }),
        )
        .style(if active {
            ui_style::button_toolbar_toggle_active
        } else {
            ui_style::button_toolbar_chip
        })
        .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
        .padding([ui_style::grid(2), ui_style::grid(2)])
        .on_press(on_press)
        .into(),
        text(tooltip_label).size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Bottom,
    )
}

pub(super) fn editor_file_browser_inline_entry(
    app: &Lilypalooza,
    is_dir: bool,
) -> Element<'_, Message> {
    let icon = if is_dir {
        icons::folder_open()
    } else {
        icons::file()
    };

    container(
        row![
            container(
                svg(icon)
                    .width(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
                    .height(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
                    .content_fit(ContentFit::Contain)
                    .style(|theme: &Theme, _status| {
                        let palette = theme.extended_palette();
                        svg::Style {
                            color: Some(palette.background.base.text),
                        }
                    }),
            )
            .width(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
            .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT))
            .center_y(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT)),
            text_input("", &app.browser_inline_edit_value)
                .id(app.browser_inline_edit_input_id.clone())
                .on_input(|value| {
                    Message::Editor(crate::app::EditorMessage::FileBrowserInlineEditChanged(
                        value,
                    ))
                })
                .on_submit(Message::Editor(
                    crate::app::EditorMessage::CommitFileBrowserInlineEdit,
                ))
                .size(ui_style::FONT_SIZE_UI_SM)
                .padding([ui_style::grid(1), 0])
                .width(Fill),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .width(Fill),
    )
    .width(Fill)
    .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT))
    .padding([0, ui_style::PADDING_XS])
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        let selected_background = Color {
            r: palette.background.strong.color.r
                + (palette.primary.base.color.r - palette.background.strong.color.r) * 0.10,
            g: palette.background.strong.color.g
                + (palette.primary.base.color.g - palette.background.strong.color.g) * 0.10,
            b: palette.background.strong.color.b
                + (palette.primary.base.color.b - palette.background.strong.color.b) * 0.10,
            a: palette.background.strong.color.a
                + (palette.primary.base.color.a - palette.background.strong.color.a) * 0.10,
        };
        container::Style {
            background: Some(selected_background.into()),
            text_color: Some(palette.background.base.text),
            border: border::rounded(ui_style::RADIUS_NONE)
                .width(0)
                .color(Color::TRANSPARENT),
            ..container::Style::default()
        }
    })
    .into()
}

pub(super) fn editor_tab_strip(app: &Lilypalooza) -> Element<'_, Message> {
    let tabs_row = app.editor.tab_summaries().into_iter().fold(
        row![].spacing(0).align_y(alignment::Vertical::Center),
        |tabs, tab| tabs.push(editor_tab(app, tab)),
    );

    let empty_area = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(EDITOR_TAB_HEIGHT)),
    )
    .on_move(|position| Message::Editor(crate::app::EditorMessage::TabBarMoved(position)))
    .on_enter(Message::Editor(crate::app::EditorMessage::TabBarEmptyMoved))
    .on_double_click(Message::Editor(crate::app::EditorMessage::NewRequested));

    let tabs_scroll = mouse_area(
        scrollable(
            container(
                row![tabs_row, empty_area]
                    .spacing(0)
                    .align_y(alignment::Vertical::Center)
                    .width(Fill)
                    .height(Length::Fixed(EDITOR_TAB_HEIGHT)),
            )
            .width(Fill)
            .style(ui_style::workspace_toolbar_surface),
        )
        .id(crate::app::EDITOR_TABBAR_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .on_scroll(|viewport| Message::Editor(crate::app::EditorMessage::TabBarScrolled(viewport)))
        .style(ui_style::editor_tabbar_scrollable)
        .width(Fill)
        .height(Length::Fixed(EDITOR_TAB_HEIGHT)),
    )
    .on_move(|position| Message::Editor(crate::app::EditorMessage::TabBarMoved(position)))
    .on_release(Message::Editor(crate::app::EditorMessage::TabDragReleased))
    .on_exit(Message::Editor(crate::app::EditorMessage::TabDragExited));

    let new_tab = button(ui_style::icon(
        icons::plus(),
        14.0,
        |theme: &Theme, status| {
            let palette = theme.extended_palette();
            svg::Style {
                color: Some(match status {
                    svg::Status::Idle => palette.background.weak.text,
                    svg::Status::Hovered => palette.background.base.text,
                }),
            }
        },
    ))
    .style(ui_style::button_neutral)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .on_press(Message::Editor(crate::app::EditorMessage::NewRequested));

    container(
        iced::widget::column![
            container(
                row![tabs_scroll, new_tab]
                    .spacing(ui_style::SPACE_XS)
                    .align_y(alignment::Vertical::Center)
                    .width(Fill),
            )
            .height(Length::Fixed(EDITOR_TAB_HEIGHT))
            .padding(Padding {
                top: 0.0,
                right: f32::from(ui_style::PADDING_STATUS_BAR_H),
                bottom: 0.0,
                left: 0.0,
            })
            .style(ui_style::workspace_toolbar_surface),
        ]
        .spacing(0),
    )
    .width(Fill)
    .into()
}

pub(super) fn editor_tab(
    app: &Lilypalooza,
    tab: crate::app::editor::EditorTabSummary,
) -> Element<'_, Message> {
    let state = EditorTabViewState::new(app, &tab);
    let title = editor_tab_title(app, &tab, state);
    let dirty_marker = editor_tab_dirty_marker(tab.dirty, tab.active, tab.file_state, state);
    let close_button = editor_tab_close_button(tab.id, tab.active, state);
    let body = editor_tab_body(tab.id, tab.active, title, dirty_marker, close_button, state);

    delayed_tooltip(
        app,
        format!("editor-tab-{}", tab.id),
        body,
        text(editor_tab_tooltip_label(app, &tab))
            .size(ui_style::FONT_SIZE_UI_XS)
            .into(),
        tooltip::Position::Bottom,
    )
}

#[derive(Clone, Copy)]
pub(super) struct EditorTabViewState {
    hovered: bool,
    dragged: bool,
    renaming: bool,
    show_before_drop: bool,
    show_after_drop: bool,
}

impl EditorTabViewState {
    fn new(app: &Lilypalooza, tab: &crate::app::editor::EditorTabSummary) -> Self {
        let hovered = app.hovered_editor_tab == Some(tab.id);
        let dragged = app.dragged_editor_tab == Some(tab.id);
        Self {
            hovered,
            dragged,
            renaming: app.renaming_editor_tab == Some(tab.id),
            show_before_drop: hovered
                && app.dragged_editor_tab.is_some()
                && !app.editor_tab_drop_after,
            show_after_drop: hovered
                && app.dragged_editor_tab.is_some()
                && app.editor_tab_drop_after,
        }
    }
}

pub(super) fn editor_tab_tooltip_label(
    app: &Lilypalooza,
    tab: &crate::app::editor::EditorTabSummary,
) -> String {
    app.editor
        .tab_path(tab.id)
        .map(|path| editor_tab_path_tooltip(path, tab.file_state))
        .unwrap_or_else(|| tab.title.clone())
}

pub(super) fn editor_tab_path_tooltip(
    path: &Path,
    file_state: crate::app::editor::EditorTabFileState,
) -> String {
    let mut label = path.display().to_string();
    match file_state {
        crate::app::editor::EditorTabFileState::Ok => {}
        crate::app::editor::EditorTabFileState::ChangedOnDisk => {
            label.push_str("\nChanged on disk")
        }
        crate::app::editor::EditorTabFileState::MissingOnDisk => {
            label.push_str("\nMissing on disk")
        }
    }
    label
}

pub(super) fn editor_tab_title<'a>(
    app: &'a Lilypalooza,
    tab: &crate::app::editor::EditorTabSummary,
    state: EditorTabViewState,
) -> Element<'a, Message> {
    let active = tab.active;
    if state.renaming {
        text_input("", &app.editor_tab_rename_value)
            .id(app.editor_tab_rename_input_id.clone())
            .on_input(|value| Message::Editor(crate::app::EditorMessage::RenameInputChanged(value)))
            .on_submit(Message::Editor(crate::app::EditorMessage::CommitRename))
            .padding([3, 0])
            .size(ui_style::FONT_SIZE_UI_SM)
            .width(Fill)
            .into()
    } else {
        text(shorten_editor_tab_title(
            &tab.title,
            EDITOR_TAB_TITLE_MAX_CHARS,
        ))
        .size(ui_style::FONT_SIZE_UI_SM)
        .line_height(1.0)
        .style(move |theme: &Theme| iced::widget::text::Style {
            color: Some(editor_tab_text_color(theme, active, state)),
        })
        .width(Fill)
        .into()
    }
}

pub(super) fn editor_tab_text_color(
    theme: &Theme,
    active: bool,
    state: EditorTabViewState,
) -> iced::Color {
    let palette = theme.extended_palette();
    if state.hovered && !state.dragged {
        palette.primary.weak.text
    } else if active {
        palette.background.base.text
    } else {
        palette.background.strong.text
    }
}

pub(super) fn editor_tab_dirty_marker(
    dirty: bool,
    active: bool,
    file_state: crate::app::editor::EditorTabFileState,
    state: EditorTabViewState,
) -> Element<'static, Message> {
    if state.renaming || (!dirty && file_state == crate::app::editor::EditorTabFileState::Ok) {
        return container(text("")).width(Length::Fixed(0.0)).into();
    }

    text(editor_tab_dirty_marker_text(file_state))
        .size(ui_style::FONT_SIZE_UI_SM)
        .line_height(1.0)
        .style(move |theme: &Theme| iced::widget::text::Style {
            color: Some(editor_tab_dirty_marker_color(
                theme, active, file_state, state,
            )),
        })
        .into()
}

pub(super) fn editor_tab_dirty_marker_text(
    file_state: crate::app::editor::EditorTabFileState,
) -> &'static str {
    match file_state {
        crate::app::editor::EditorTabFileState::Ok => "•",
        crate::app::editor::EditorTabFileState::ChangedOnDisk
        | crate::app::editor::EditorTabFileState::MissingOnDisk => "!",
    }
}

pub(super) fn editor_tab_dirty_marker_color(
    theme: &Theme,
    active: bool,
    file_state: crate::app::editor::EditorTabFileState,
    state: EditorTabViewState,
) -> iced::Color {
    let palette = theme.extended_palette();
    match file_state {
        crate::app::editor::EditorTabFileState::MissingOnDisk => palette.danger.base.color,
        crate::app::editor::EditorTabFileState::Ok
        | crate::app::editor::EditorTabFileState::ChangedOnDisk => {
            editor_tab_text_color(theme, active, state)
        }
    }
}

pub(super) fn editor_tab_close_button(
    tab_id: u64,
    active: bool,
    state: EditorTabViewState,
) -> Element<'static, Message> {
    if state.renaming {
        container(text(""))
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(EDITOR_TAB_HEIGHT))
            .center_y(Length::Fixed(EDITOR_TAB_HEIGHT))
            .into()
    } else {
        button(
            container(ui_style::icon(
                icons::x(),
                11.0,
                move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    svg::Style {
                        color: Some(match status {
                            svg::Status::Idle => {
                                if active {
                                    palette.background.base.text
                                } else {
                                    palette.background.strong.text
                                }
                            }
                            svg::Status::Hovered => palette.primary.weak.text,
                        }),
                    }
                },
            ))
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(EDITOR_TAB_HEIGHT))
            .center_y(Length::Fixed(EDITOR_TAB_HEIGHT)),
        )
        .style(move |theme: &Theme, status| {
            ui_style::button_editor_tab_close(theme, status, active)
        })
        .padding([0, 0])
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(EDITOR_TAB_HEIGHT))
        .on_press(Message::Editor(
            crate::app::EditorMessage::CloseTabRequested(tab_id),
        ))
        .into()
    }
}

pub(super) fn editor_tab_body<'a>(
    tab_id: u64,
    active: bool,
    title: Element<'a, Message>,
    dirty_marker: Element<'a, Message>,
    close_button: Element<'a, Message>,
    state: EditorTabViewState,
) -> Element<'a, Message> {
    let body: Element<'_, Message> = row![
        editor_tab_drop_marker(state.show_before_drop),
        container(
            row![
                dirty_marker,
                container(title).width(Fill).height(Fill).center_y(Fill),
                close_button
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .width(Fill)
            .height(Fill),
        )
        .width(Length::Fixed(EDITOR_TAB_WIDTH))
        .height(Length::Fixed(EDITOR_TAB_HEIGHT))
        .padding([0, 8])
        .style(move |theme: &Theme| {
            ui_style::editor_tab_surface(
                theme,
                active,
                state.hovered || state.renaming,
                state.dragged,
            )
        }),
        editor_tab_drop_marker(state.show_after_drop)
    ]
    .spacing(0)
    .height(Length::Fixed(EDITOR_TAB_HEIGHT))
    .align_y(alignment::Vertical::Center)
    .into();

    if state.renaming {
        body
    } else {
        mouse_area(body)
            .on_press(Message::Editor(crate::app::EditorMessage::TabPressed(
                tab_id,
            )))
            .on_double_click(Message::Editor(crate::app::EditorMessage::StartRename(
                tab_id,
            )))
            .on_move(move |position| {
                Message::Editor(crate::app::EditorMessage::TabMoved { tab_id, position })
            })
            .on_exit(Message::Editor(crate::app::EditorMessage::TabHovered(None)))
            .interaction(if state.dragged {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            })
            .into()
    }
}

pub(super) fn editor_tab_drop_marker<'a>(visible: bool) -> Element<'a, Message> {
    let marker = container(text(""))
        .width(Length::Fixed(2.0))
        .height(Length::Fixed(EDITOR_TAB_HEIGHT));
    if visible {
        marker
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().primary.base.color.into()),
                ..container::Style::default()
            })
            .into()
    } else {
        marker.into()
    }
}

pub(super) fn group_title_bar<'a>(
    app: &'a Lilypalooza,
    group_id: crate::app::DockGroupId,
    group_width: f32,
    is_focused: bool,
) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(group_header(app, group_id, group_width))
        .style(move |theme: &Theme| ui_style::pane_title_bar_surface_focused(theme, is_focused))
}

pub(super) fn workspace_pane_focus_body<'a>(
    pane: WorkspacePaneKind,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    mouse_area(body)
        .on_press(Message::Pane(PaneMessage::FocusWorkspacePane(pane)))
        .into()
}

pub(super) fn group_header<'a>(
    app: &'a Lilypalooza,
    group_id: crate::app::DockGroupId,
    group_width: f32,
) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return container(text("")).width(Fill).into();
    };
    let active_pane = group.active;
    let control_groups = pane_header_control_groups(app, active_pane);
    let title_width = group_tabs_min_width(group);
    let available_controls_width = (group_width - title_width).max(0.0);
    let (inline_controls, overflow_controls) = if active_pane == WorkspacePaneKind::Editor {
        (Vec::new(), vec![text("").into()])
    } else {
        split_header_control_groups(control_groups, available_controls_width)
    };
    let header_layout = group_header_layout_from_parts(
        active_pane,
        title_width,
        available_controls_width,
        !overflow_controls.is_empty(),
    );
    let is_menu_open =
        header_layout.shows_menu_button && app.open_header_overflow_menu == Some(group_id);
    let mut header = row![group_tabs(app, group), container(text("")).width(Fill)]
        .align_y(alignment::Vertical::Center)
        .width(Fill);

    if !inline_controls.is_empty() {
        header = header.push(
            row(inline_controls)
                .spacing(ui_style::SPACE_SM)
                .align_y(alignment::Vertical::Center),
        );
    }

    if header_layout.shows_menu_button {
        header = header.push(header_overflow_trigger(app, group_id, is_menu_open));
    }
    header = header.push(header_close_trigger(app, active_pane));
    iced::widget::column![
        mouse_area(
            container(header)
                .padding([PANE_HEADER_VERTICAL_PADDING, ui_style::PADDING_STATUS_BAR_H,]),
        )
        .on_press(Message::Pane(PaneMessage::FocusWorkspacePane(active_pane))),
        container(text(""))
            .height(Length::Fixed(1.0))
            .width(Fill)
            .style(ui_style::chrome_separator),
    ]
    .spacing(0)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GroupHeaderLayout {
    pub(super) active_pane: WorkspacePaneKind,
    pub(super) title_width: f32,
    pub(super) available_controls_width: f32,
    pub(super) shows_menu_button: bool,
    pub(super) shows_close_button: bool,
}

pub(super) fn group_header_layout_from_parts(
    active_pane: WorkspacePaneKind,
    title_width: f32,
    available_controls_width: f32,
    has_overflow_controls: bool,
) -> GroupHeaderLayout {
    let shows_menu_button = active_pane == WorkspacePaneKind::Editor || has_overflow_controls;

    GroupHeaderLayout {
        active_pane,
        title_width,
        available_controls_width,
        shows_menu_button,
        shows_close_button: true,
    }
}

pub(super) fn header_close_trigger(
    app: &Lilypalooza,
    pane: WorkspacePaneKind,
) -> Element<'static, Message> {
    delayed_tooltip(
        app,
        format!("header-close-{pane:?}"),
        container(
            ui_style::flat_icon_button(
                icons::x(),
                HEADER_MENU_BUTTON_WIDTH,
                HEADER_CLOSE_ICON_SIZE,
                ui_style::button_pane_header_control,
                ui_style::svg_dimmed_control,
            )
            .width(Length::Fixed(HEADER_MENU_BUTTON_WIDTH))
            .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
            .on_press(Message::Pane(PaneMessage::ToggleWorkspacePane(pane))),
        )
        .padding([0, 2])
        .into(),
        text("Close pane").size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Top,
    )
}

pub(super) fn header_overflow_menu_overlay<'a>(
    app: &'a Lilypalooza,
    size: Size,
    group_bounds: &HashMap<crate::app::DockGroupId, Rectangle>,
) -> Element<'a, Message> {
    let Some(group_id) = app.open_header_overflow_menu else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let Some(group) = app.workspace_group(group_id) else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    let active_pane = group.active;
    if active_pane != WorkspacePaneKind::Editor && !pane_header_has_controls(app, active_pane) {
        return container(text("")).width(Fill).height(Fill).into();
    }

    let Some(bounds) = group_bounds.get(&group_id).copied() else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    let menu_content = if active_pane == WorkspacePaneKind::Editor {
        editor_header_menu_panel(app)
    } else {
        let control_groups = pane_header_control_groups(app, active_pane);
        let title_width = group_tabs_min_width(group);
        let available_controls_width = (bounds.width - title_width).max(0.0);
        let (_inline_controls, overflow_controls) =
            split_header_control_groups(control_groups, available_controls_width);
        header_overflow_menu_panel(overflow_controls)
    };

    let menu_width = header_overflow_menu_width(app, active_pane);
    let x = header_overflow_menu_x(bounds, size.width, menu_width);
    let y = bounds.y + ui_style::SPACE_XS as f32;

    let backdrop: Element<'a, Message> = mouse_area(container(text("")).width(Fill).height(Fill))
        .on_press(Message::Pane(PaneMessage::CloseHeaderOverflowMenu))
        .into();
    let menu_panel = mouse_area(opaque(menu_content))
        .on_exit(Message::Pane(PaneMessage::CloseHeaderOverflowMenu));
    let positioned = container(menu_panel)
        .padding(Padding {
            top: y,
            right: 0.0,
            bottom: 0.0,
            left: x,
        })
        .width(Fill)
        .height(Fill)
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top);

    stack([backdrop, positioned.into()]).into()
}

pub(super) fn header_overflow_menu_width(app: &Lilypalooza, active_pane: WorkspacePaneKind) -> f32 {
    if active_pane != WorkspacePaneKind::Editor {
        return 320.0;
    }

    let submenu_width = app
        .open_editor_menu_section
        .map_or(0.0, editor_header_submenu_width);

    EDITOR_MENU_ROOT_WIDTH + submenu_width + ui_style::SPACE_XS as f32
}

pub(super) fn editor_header_submenu_width(section: EditorHeaderMenuSection) -> f32 {
    match section {
        EditorHeaderMenuSection::File => EDITOR_FILE_SUBMENU_WIDTH,
        EditorHeaderMenuSection::Edit => EDITOR_EDIT_SUBMENU_WIDTH,
        EditorHeaderMenuSection::Appearance => EDITOR_APPEARANCE_SUBMENU_WIDTH,
    }
}

pub(super) fn header_overflow_menu_x(
    bounds: Rectangle,
    viewport_width: f32,
    menu_width: f32,
) -> f32 {
    let desired = bounds.x + bounds.width - menu_width - ui_style::SPACE_XS as f32;
    desired.clamp(
        ui_style::SPACE_XS as f32,
        (viewport_width - menu_width - ui_style::SPACE_XS as f32).max(ui_style::SPACE_XS as f32),
    )
}

pub(super) fn group_tabs<'a>(
    app: &'a Lilypalooza,
    group: &'a crate::app::DockGroup,
) -> row::Row<'a, Message> {
    group.tabs.iter().copied().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Bottom),
        |tabs, pane| tabs.push(workspace_tab(app, pane)),
    )
}

pub(super) fn workspace_tab(app: &Lilypalooza, pane: WorkspacePaneKind) -> Element<'_, Message> {
    let (is_active, is_stacked, is_focused) = app
        .group_for_pane(pane)
        .and_then(|group_id| app.workspace_group(group_id).map(|group| (group_id, group)))
        .map(|(group_id, group)| {
            (
                group.active == pane,
                group.tabs.len() > 1,
                app.is_workspace_group_focused(group_id),
            )
        })
        .unwrap_or((false, false, false));
    let is_hovered = app.hovered_workspace_pane == Some(pane);
    let is_dragging = app.dragged_workspace_pane == Some(pane);
    let title = workspace_pane_title(pane);
    let icon = workspace_pane_icon(pane);
    let icon_color = workspace_tab_foreground_color(is_active, is_focused, is_hovered, is_dragging);

    let tab_body: Element<'_, Message> = container(
        row![
            container(
                svg(icon)
                    .width(Length::Fixed(TOOLBAR_ICON_SIZE))
                    .height(Length::Fixed(TOOLBAR_ICON_SIZE))
                    .content_fit(ContentFit::Contain)
                    .style(move |theme: &Theme, _status| svg::Style {
                        color: Some(icon_color(theme)),
                    }),
            )
            .width(Length::Fixed(TOOLBAR_ICON_SIZE))
            .height(Length::Fixed(TOOLBAR_ICON_SIZE))
            .center_x(Length::Fixed(TOOLBAR_ICON_SIZE))
            .center_y(Length::Fixed(TOOLBAR_ICON_SIZE)),
            text(title).size(ui_style::FONT_SIZE_UI_SM),
        ]
        .spacing(TAB_ICON_GAP)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Shrink)
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
    .center_y(Length::Fixed(HEADER_CONTROL_HEIGHT))
    .padding([0, ui_style::PADDING_STATUS_BAR_H + 8])
    .style(move |theme: &Theme| {
        let palette = theme.extended_palette();

        if is_dragging {
            container::Style {
                background: Some(palette.primary.weak.color.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(ui_style::RADIUS_UI)
                    .width(1)
                    .color(palette.primary.base.color),
                ..container::Style::default()
            }
        } else if is_stacked && is_active {
            container::Style {
                background: Some(Color::TRANSPARENT.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(ui_style::RADIUS_UI)
                    .width(1)
                    .color(palette.background.strong.color),
                ..container::Style::default()
            }
        } else if is_stacked && is_hovered {
            container::Style {
                background: Some(palette.background.base.color.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(ui_style::RADIUS_UI)
                    .width(0)
                    .color(Color::TRANSPARENT),
                ..container::Style::default()
            }
        } else {
            container::Style {
                background: Some(Color::TRANSPARENT.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(ui_style::RADIUS_UI)
                    .width(0)
                    .color(Color::TRANSPARENT),
                ..container::Style::default()
            }
        }
    })
    .into();

    mouse_area(tab_body)
        .on_press(Message::Pane(PaneMessage::WorkspaceTabPressed(pane)))
        .on_enter(Message::Pane(PaneMessage::WorkspaceTabHovered(Some(pane))))
        .on_exit(Message::Pane(PaneMessage::WorkspaceTabHovered(None)))
        .interaction(if is_dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        })
        .into()
}
