use super::*;

pub(super) const TOOLBAR_ICON_SIZE: f32 = ui_style::grid_f32(3);
pub(in crate::app) const HEADER_CONTROL_HEIGHT: f32 = ui_style::grid_f32(6);
pub(super) const PANE_HEADER_VERTICAL_PADDING: u16 = ui_style::grid(1);
pub(super) const HEADER_MENU_ICON_SIZE: f32 = 12.0;
pub(super) const HEADER_CLOSE_ICON_SIZE: f32 = ui_style::grid_f32(3);
pub(super) const HEADER_MENU_BUTTON_WIDTH: f32 = ui_style::grid_f32(6);
pub(super) const EDITOR_MENU_ROOT_WIDTH: f32 = ui_style::grid_f32(32);
pub(super) const EDITOR_FILE_SUBMENU_WIDTH: f32 = 320.0;
pub(super) const EDITOR_EDIT_SUBMENU_WIDTH: f32 = ui_style::grid_f32(56);
pub(super) const EDITOR_APPEARANCE_SUBMENU_WIDTH: f32 = 272.0;
pub(super) const EDITOR_MENU_ITEM_HEIGHT: f32 = 24.0;
pub(super) const EDITOR_MENU_ITEM_PADDING_V: u16 = 0;
pub(super) const EDITOR_MENU_ITEM_PADDING_H: u16 = ui_style::PADDING_BUTTON_COMPACT_H;
pub(super) const EDITOR_RECENT_FILE_LABEL_MAX_CHARS: usize = 40;
pub(super) const TAB_ICON_GAP: u32 = 6;
pub(super) const HEADER_WIDTH_SAFETY: f32 = 24.0;
pub(super) const TOOLBAR_VERTICAL_PADDING: u16 = ui_style::grid(1);
pub(in crate::app) const TOOLBAR_HEIGHT: f32 = ui_style::grid_f32(10);
pub(super) const TOOLBAR_TOGGLE_ICON_SIZE: f32 = ui_style::grid_f32(4);
pub(super) const TOOLBAR_BUTTON_HEIGHT: f32 = ui_style::grid_f32(8);
pub(super) const TOOLBAR_FILE_NAME_MAX_CHARS: usize = 20;
pub(super) const TOOLBAR_PROJECT_NAME_MAX_CHARS: usize = 28;
pub(super) const PROJECT_MENU_ROOT_WIDTH: f32 = ui_style::grid_f32(32);
pub(super) const PROJECT_MENU_WIDTH: f32 = 280.0;
pub(super) const PROJECT_SETTINGS_SUBMENU_WIDTH: f32 = ui_style::grid_f32(56);
pub(super) const PROJECT_RECENT_LABEL_MAX_CHARS: usize = 40;
pub(super) const EDITOR_FILE_BROWSER_ICON_SIZE: f32 = ui_style::grid_f32(4);
pub(super) const EDITOR_TAB_WIDTH: f32 = 140.0;
pub(super) const EDITOR_TAB_HEIGHT: f32 = 32.0;
pub(super) const EDITOR_TAB_TITLE_MAX_CHARS: usize = 18;

pub(in crate::app) struct HeaderControlGroup<'a> {
    pub(in crate::app) min_width: f32,
    pub(in crate::app) content: Element<'a, Message>,
}

pub(in crate::app) fn view(app: &Lilypalooza) -> Element<'_, Message> {
    let toolbar = workspace_toolbar(app);
    let workspace = workspace_panes(app);
    let content: Element<'_, Message> =
        iced::widget::column![toolbar, workspace, transport_bar::view(app)]
            .width(Fill)
            .height(Fill)
            .spacing(0)
            .into();

    let overlay: Element<'_, Message> = if app.open_project_menu {
        project_menu_overlay(app)
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };

    stack([content, overlay]).into()
}

pub(in crate::app) fn delayed_tooltip<'a>(
    app: &Lilypalooza,
    key: impl Into<String>,
    content: Element<'a, Message>,
    tooltip_content: Element<'a, Message>,
    position: tooltip::Position,
) -> Element<'a, Message> {
    let key = key.into();
    let tracked = mouse_area(content)
        .on_enter(Message::Pane(PaneMessage::TooltipHovered(Some(
            key.clone(),
        ))))
        .on_exit(Message::Pane(PaneMessage::TooltipHovered(None)));

    if app.is_tooltip_open(&key) {
        Tooltip::new(tracked, tooltip_content, position)
            .gap(ui_style::grid_f32(2))
            .padding(8)
            .style(ui_style::tooltip_popup)
            .into()
    } else {
        tracked.into()
    }
}

pub(super) fn workspace_toolbar(app: &Lilypalooza) -> Element<'_, Message> {
    let pane_toggles = all_workspace_panes().into_iter().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        |row, pane| row.push(toolbar_pane_toggle(app, pane)),
    );

    container(
        iced::widget::column![
            container(
                row![
                    toolbar_project_button(app),
                    toolbar_separator(),
                    pane_toggles,
                    container(text("")).width(Fill),
                ]
                .spacing(ui_style::SPACE_SM)
                .align_y(alignment::Vertical::Center)
                .width(Fill),
            )
            .height(Fill)
            .padding([TOOLBAR_VERTICAL_PADDING, ui_style::PADDING_STATUS_BAR_H,])
            .style(ui_style::workspace_toolbar_surface),
            container(text(""))
                .height(Length::Fixed(1.0))
                .width(Fill)
                .style(ui_style::chrome_separator),
        ]
        .spacing(0),
    )
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .width(Fill)
    .into()
}

pub(super) fn toolbar_separator() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.strong.color.into()),
                ..container::Style::default()
            }
        })
        .into()
}

pub(super) fn toolbar_project_button(app: &Lilypalooza) -> Element<'_, Message> {
    let project_title =
        truncate_toolbar_file_name(&app.project_title(), TOOLBAR_PROJECT_NAME_MAX_CHARS);
    let main_score_title = truncate_toolbar_file_name(
        app.current_score
            .as_ref()
            .map(|selected_score| selected_score.file_name.as_str())
            .unwrap_or("No main score"),
        TOOLBAR_FILE_NAME_MAX_CHARS,
    );
    let tooltip_text = "Menu";
    let chevron = ui_style::icon(
        icons::chevron_down(),
        TOOLBAR_TOGGLE_ICON_SIZE,
        |theme: &Theme, status| {
            let palette = theme.extended_palette();
            svg::Style {
                color: Some(match status {
                    svg::Status::Idle => palette.background.base.text,
                    svg::Status::Hovered => palette.primary.weak.text,
                }),
            }
        },
    );
    delayed_tooltip(
        app,
        "toolbar-project-menu",
        button(
            row![
                container(chevron)
                    .width(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
                    .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
                    .center_x(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
                    .center_y(Length::Fixed(TOOLBAR_BUTTON_HEIGHT)),
                container(
                    row![
                        text(project_title)
                            .size(ui_style::FONT_SIZE_UI_SM)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..fonts::UI
                            })
                            .line_height(1.0),
                        text(" | ")
                            .size(ui_style::FONT_SIZE_UI_SM)
                            .font(iced::Font {
                                weight: iced::font::Weight::Normal,
                                ..fonts::UI
                            })
                            .line_height(1.0)
                            .style(|theme: &Theme| {
                                let palette = theme.extended_palette();
                                iced::widget::text::Style {
                                    color: Some(Color {
                                        a: 0.58,
                                        ..palette.background.base.text
                                    }),
                                }
                            }),
                        text(main_score_title)
                            .size(ui_style::FONT_SIZE_UI_SM)
                            .font(iced::Font {
                                weight: iced::font::Weight::Normal,
                                ..fonts::UI
                            })
                            .line_height(1.0)
                            .style(|theme: &Theme| {
                                let palette = theme.extended_palette();
                                iced::widget::text::Style {
                                    color: Some(Color {
                                        a: 0.58,
                                        ..palette.background.base.text
                                    }),
                                }
                            }),
                    ]
                    .spacing(0)
                    .align_y(alignment::Vertical::Center),
                )
                .padding(Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: ui_style::grid_f32(1),
                    left: 0.0,
                })
                .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
                .center_y(Length::Fixed(TOOLBAR_BUTTON_HEIGHT)),
            ]
            .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .style(ui_style::button_toolbar_chip)
        .padding([0, ui_style::grid(3)])
        .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
        .on_press(Message::Pane(PaneMessage::ToggleProjectMenu))
        .into(),
        text(tooltip_text).size(ui_style::FONT_SIZE_UI_XS).into(),
        tooltip::Position::Bottom,
    )
}

pub(super) fn project_menu_overlay(app: &Lilypalooza) -> Element<'_, Message> {
    let backdrop: Element<'_, Message> = mouse_area(container(text("")).width(Fill).height(Fill))
        .on_press(Message::Pane(PaneMessage::CloseProjectMenu))
        .into();
    let panel: Element<'_, Message> = container(
        mouse_area(opaque(project_menu_panel(app)))
            .on_exit(Message::Pane(PaneMessage::CloseProjectMenu)),
    )
    .padding([
        crate::number::f32_to_u16(TOOLBAR_HEIGHT),
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .width(Fill)
    .height(Fill)
    .align_x(alignment::Horizontal::Left)
    .align_y(alignment::Vertical::Top)
    .into();

    stack([backdrop, panel]).into()
}

pub(super) fn project_menu_panel<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let root_menu = container(
        Column::new()
            .spacing(ui_style::SPACE_XS)
            .push(project_root_menu_item(
                "Project",
                app.open_project_menu_section == Some(ProjectMenuSection::Project),
                ProjectMenuSection::Project,
            ))
            .push(project_root_menu_item(
                "View",
                app.open_project_menu_section == Some(ProjectMenuSection::View),
                ProjectMenuSection::View,
            )),
    )
    .width(Length::Fixed(PROJECT_MENU_ROOT_WIDTH))
    .padding(ui_style::PADDING_XS)
    .style(ui_style::tooltip_popup);

    match app
        .open_project_menu_section
        .unwrap_or(ProjectMenuSection::Project)
    {
        ProjectMenuSection::Project => row![
            root_menu,
            iced::widget::column![
                container(text("")).height(Length::Fixed(project_submenu_offset(
                    ProjectMenuSection::Project
                ))),
                container(project_project_submenu(app))
                    .width(Length::Fixed(PROJECT_MENU_WIDTH))
                    .padding(ui_style::PADDING_SM)
                    .style(ui_style::tooltip_popup),
            ]
            .spacing(0),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Top)
        .into(),
        ProjectMenuSection::View => row![
            root_menu,
            iced::widget::column![
                container(text("")).height(Length::Fixed(project_submenu_offset(
                    ProjectMenuSection::View
                ))),
                container(project_view_submenu())
                    .width(Length::Fixed(PROJECT_SETTINGS_SUBMENU_WIDTH))
                    .padding(ui_style::PADDING_SM)
                    .style(ui_style::tooltip_popup),
            ]
            .spacing(0),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Top)
        .into(),
    }
}

pub(super) fn project_project_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let save_project = editor_menu_item(
        "Save Project",
        true,
        Some(Message::File(if app.has_saved_project() {
            crate::app::FileMessage::RequestSaveProject
        } else {
            crate::app::FileMessage::RequestCreateProject
        })),
    );

    let load_project = editor_menu_item(
        "Load Project...",
        true,
        Some(Message::File(crate::app::FileMessage::RequestLoadProject)),
    );
    let open_main_score = editor_menu_item(
        "Open Main Score...",
        true,
        Some(Message::File(crate::app::FileMessage::RequestOpen)),
    );

    let rename_project = editor_menu_item("Rename Project", false, None);

    let recent_open = app.open_project_recent;
    let recent_row = editor_fold_menu_item(
        "Open Recent",
        !app.recent_projects.is_empty(),
        recent_open,
        false,
        Message::Pane(PaneMessage::SetProjectRecentOpen(!recent_open)),
    );

    let mut column = Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(save_project)
        .push(load_project)
        .push(open_main_score)
        .push(rename_project)
        .push(recent_row);

    if recent_open {
        column = column.push(
            container(project_recent_projects_submenu(app)).padding(Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: f32::from(ui_style::PADDING_MD),
            }),
        );
    }

    column.into()
}

pub(super) fn project_view_submenu<'a>() -> Element<'a, Message> {
    Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(editor_menu_item(
            "Actions...",
            true,
            Some(Message::Shortcuts(ShortcutsMessage::OpenDialog)),
        ))
        .push(editor_menu_item(
            "Settings",
            true,
            Some(Message::Shortcuts(ShortcutsMessage::ActivateAction(
                crate::settings::ShortcutActionId::OpenSettingsFile,
            ))),
        ))
        .into()
}

pub(super) fn project_recent_projects_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    if app.recent_projects.is_empty() {
        return Column::new()
            .spacing(ui_style::SPACE_XS)
            .push(editor_menu_item("No recent projects", false, None))
            .into();
    }

    let recent_paths: Vec<_> = app.recent_projects.iter().take(7).cloned().collect();
    let labels = recent_file_labels(&recent_paths, PROJECT_RECENT_LABEL_MAX_CHARS);

    recent_paths
        .into_iter()
        .zip(labels)
        .fold(
            Column::new().spacing(ui_style::SPACE_XS),
            |column, (path, label)| {
                column.push(delayed_tooltip(
                    app,
                    format!("project-recent-{}", path.display()),
                    editor_menu_item(
                        label,
                        true,
                        Some(Message::File(crate::app::FileMessage::OpenRecentProject(
                            path.clone(),
                        ))),
                    ),
                    text(path.display().to_string())
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .into(),
                    tooltip::Position::Right,
                ))
            },
        )
        .into()
}

pub(super) fn project_submenu_offset(section: ProjectMenuSection) -> f32 {
    let item_index = match section {
        ProjectMenuSection::Project => 0.0,
        ProjectMenuSection::View => 1.0,
    };

    f32::from(ui_style::PADDING_XS)
        + item_index * (EDITOR_MENU_ITEM_HEIGHT + ui_style::SPACE_XS as f32)
}

pub(super) fn project_root_menu_item<'a>(
    label: &'a str,
    active: bool,
    section: ProjectMenuSection,
) -> Element<'a, Message> {
    let button = button(
        container(
            row![
                text(label).size(ui_style::FONT_SIZE_UI_XS),
                container(text("")).width(Fill),
                ui_style::icon(
                    icons::chevron_right(),
                    10.0,
                    move |theme: &Theme, _status| {
                        svg::Style {
                            color: Some(if active {
                                theme.extended_palette().background.weakest.text
                            } else {
                                Color::from_rgb(0.12, 0.12, 0.14)
                            }),
                        }
                    }
                ),
            ]
            .spacing(ui_style::SPACE_XS)
            .width(Fill)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Fill)
        .center_y(Fill),
    )
    .width(Fill)
    .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
    .padding([EDITOR_MENU_ITEM_PADDING_V, EDITOR_MENU_ITEM_PADDING_H])
    .style(move |theme: &Theme, status| ui_style::button_menu_item(theme, status, active))
    .on_press(Message::Pane(PaneMessage::SetProjectMenuSection(Some(
        section,
    ))));

    mouse_area(button)
        .interaction(mouse::Interaction::Pointer)
        .on_enter(Message::Pane(PaneMessage::SetProjectMenuSection(Some(
            section,
        ))))
        .into()
}

pub(super) fn truncate_toolbar_file_name(file_name: &str, max_chars: usize) -> String {
    let count = file_name.chars().count();
    if count <= max_chars {
        return file_name.to_string();
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let visible_chars = max_chars - 3;
    let truncated: String = file_name.chars().take(visible_chars).collect();
    format!("{truncated}...")
}

pub(super) fn shorten_editor_tab_title(title: &str, max_chars: usize) -> String {
    let chars: Vec<_> = title.chars().collect();
    if chars.len() <= max_chars {
        return title.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    if let Some(dot_index) = title.rfind('.') {
        let Some(extension_text) = title.get(dot_index..) else {
            return truncate_from_left(title, max_chars);
        };
        let extension: Vec<_> = extension_text.chars().collect();
        if extension.len() + 2 < max_chars {
            let prefix_len = max_chars.saturating_sub(extension.len() + 1);
            let prefix: String = chars.into_iter().take(prefix_len).collect();
            return format!("{prefix}…{extension_text}");
        }
    }

    let prefix: String = chars.into_iter().take(max_chars - 1).collect();
    format!("{prefix}…")
}

pub(super) fn workspace_panes(app: &Lilypalooza) -> Element<'_, Message> {
    if app.workspace_visible_pane_count() == 0 {
        return empty_workspace_placeholder(app);
    }

    responsive(move |size| {
        let group_bounds = workspace_group_bounds_map(&app.workspace_panes, size);
        let panes: Element<'_, Message> =
            pane_grid::PaneGrid::new(&app.workspace_panes, |_pane, group_id, _is_maximized| {
                let group_width = group_bounds
                    .get(group_id)
                    .map(|bounds| bounds.width)
                    .unwrap_or(size.width);
                let active_pane = app
                    .workspace_group(*group_id)
                    .map(|group| group.active)
                    .unwrap_or(WorkspacePaneKind::Score);
                let body = match active_pane {
                    WorkspacePaneKind::Score => score_view::score_body(app),
                    WorkspacePaneKind::PianoRoll => piano_roll::content(app),
                    WorkspacePaneKind::Mixer => mixer::content(app),
                    WorkspacePaneKind::Editor => editor_pane_body(app),
                    WorkspacePaneKind::Logger => app
                        .logger
                        .view(app.is_workspace_group_focused(*group_id), |action| {
                            Message::Logger(crate::app::LoggerMessage::TextAction(action))
                        }),
                };

                let body = workspace_pane_focus_body(active_pane, body);
                let is_focused = app.is_workspace_group_focused(*group_id);

                pane_grid::Content::new(body)
                    .title_bar(group_title_bar(app, *group_id, group_width, is_focused))
                    .style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Style {
                            background: Some(palette.background.base.color.into()),
                            text_color: Some(palette.background.base.text),
                            border: border::rounded(ui_style::RADIUS_NONE)
                                .width(0)
                                .color(Color::TRANSPARENT),
                            ..container::Style::default()
                        }
                    })
            })
            .width(Fill)
            .height(Fill)
            .style(split_rearrange_style)
            .on_resize(8, |event| {
                Message::Pane(PaneMessage::WorkspaceResized(event))
            })
            .into();

        let overlay = workspace_drag_overlay(app, size);
        let drag_capture = workspace_drag_capture_layer(app);
        let header_menu_overlay = header_overflow_menu_overlay(app, size, &group_bounds);

        mouse_area(
            stack([panes, overlay, drag_capture, header_menu_overlay])
                .width(Fill)
                .height(Fill),
        )
        .on_move(|position| Message::Pane(PaneMessage::WorkspaceDragMoved(position)))
        .on_release(Message::Pane(PaneMessage::WorkspaceDragReleased))
        .on_exit(Message::Pane(PaneMessage::WorkspaceDragExited))
        .into()
    })
    .into()
}

pub(super) fn editor_pane_body(app: &Lilypalooza) -> Element<'_, Message> {
    let content: Element<'_, Message> = iced::widget::column![
        editor_file_browser(app),
        editor_tab_strip(app),
        app.editor.view(
            Message::Editor(crate::app::EditorMessage::OpenRequested),
            |tab_id, message| Message::Editor(crate::app::EditorMessage::Widget {
                tab_id,
                message
            })
        ),
    ]
    .spacing(0)
    .width(Fill)
    .height(Fill)
    .into();

    if app.dragged_editor_tab.is_some() {
        let overlay: Element<'_, Message> =
            mouse_area(container(text("")).width(Fill).height(Fill))
                .on_move(|position| {
                    Message::Editor(crate::app::EditorMessage::TabGlobalMoved(position))
                })
                .on_release(Message::Editor(crate::app::EditorMessage::TabDragReleased))
                .into();

        stack([content, overlay]).width(Fill).height(Fill).into()
    } else {
        content
    }
}

pub(super) fn editor_file_browser(app: &Lilypalooza) -> Element<'_, Message> {
    let expanded = app.editor.file_browser_expanded();
    if !expanded {
        return container(text(""))
            .width(Fill)
            .height(Length::Fixed(0.0))
            .into();
    }
    let hidden_toggle = editor_file_browser_toolbar_button(
        app,
        "browser-hidden-toggle",
        icons::hat_glasses(),
        "Hidden",
        Message::Editor(crate::app::EditorMessage::FileBrowserToggleHiddenRequested),
        app.editor.file_browser_show_hidden(),
    );

    let toolbar = container(
        row![
            editor_file_browser_toolbar_button(
                app,
                "browser-new-file",
                icons::file_plus(),
                "New File",
                Message::Editor(crate::app::EditorMessage::FileBrowserNewFileRequested),
                false,
            ),
            editor_file_browser_toolbar_button(
                app,
                "browser-new-folder",
                icons::folder_plus(),
                "New Folder",
                Message::Editor(crate::app::EditorMessage::FileBrowserNewDirectoryRequested),
                false,
            ),
            editor_file_browser_toolbar_button(
                app,
                "browser-rename",
                icons::pencil(),
                "Rename",
                Message::Editor(crate::app::EditorMessage::FileBrowserRenameRequested),
                false,
            ),
            editor_file_browser_toolbar_button(
                app,
                "browser-trash",
                icons::trash_2(),
                "Delete",
                Message::Editor(crate::app::EditorMessage::FileBrowserTrashRequested),
                false,
            ),
            hidden_toggle,
            container(text("")).width(Fill),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center)
        .width(Fill),
    )
    .width(Fill)
    .padding([ui_style::PADDING_XS, ui_style::PADDING_STATUS_BAR_H])
    .style(ui_style::workspace_toolbar_surface);

    let columns = app
        .editor
        .file_browser_columns()
        .into_iter()
        .enumerate()
        .fold(
            row![].spacing(0).align_y(alignment::Vertical::Top),
            |row, (column_index, column)| {
                row.push(editor_file_browser_column(app, column_index, column))
            },
        );

    let browser_body: Element<'_, Message> = scrollable(container(columns).width(Length::Shrink))
        .id(crate::app::EDITOR_FILE_BROWSER_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .on_scroll(|viewport| {
            Message::Editor(crate::app::EditorMessage::FileBrowserScrolled(viewport))
        })
        .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_HEIGHT))
        .style(ui_style::editor_file_browser_scrollable)
        .into();

    let status_line = container(
        row![
            text(app.editor.file_browser_root_label())
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(fonts::MONO)
                .style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    iced::widget::text::Style {
                        color: Some(palette.background.strong.text),
                    }
                }),
            container(text("")).width(Fill),
        ]
        .align_y(alignment::Vertical::Center)
        .spacing(ui_style::SPACE_XS),
    )
    .width(Fill)
    .padding([ui_style::PADDING_XS, ui_style::PADDING_STATUS_BAR_H])
    .style(ui_style::workspace_toolbar_surface);

    let content = container(iced::widget::column![toolbar, browser_body, status_line,].spacing(0))
        .width(Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.base.color.into()),
                text_color: Some(palette.background.base.text),
                border: border::rounded(ui_style::RADIUS_NONE)
                    .width(0)
                    .color(Color::TRANSPARENT),
                ..container::Style::default()
            }
        });

    mouse_area(content)
        .on_press(Message::Editor(
            crate::app::EditorMessage::FileBrowserFocused,
        ))
        .on_move(|position| {
            Message::Editor(crate::app::EditorMessage::FileBrowserDragMoved(position))
        })
        .on_release(Message::Editor(
            crate::app::EditorMessage::FileBrowserDragReleased,
        ))
        .on_exit(Message::Editor(
            crate::app::EditorMessage::FileBrowserDragReleased,
        ))
        .into()
}

pub(super) fn editor_file_browser_column(
    app: &Lilypalooza,
    column_index: usize,
    column: crate::app::editor::EditorBrowserColumnSummary,
) -> Element<'_, Message> {
    match column {
        crate::app::editor::EditorBrowserColumnSummary::Directory { entries } => {
            let inline_create = app
                .browser_inline_edit
                .as_ref()
                .filter(|edit| edit.column_index == column_index && edit.target_path.is_none());
            let entries = entries.into_iter().fold(
                {
                    let mut column = iced::widget::column![].spacing(0).width(Fill);
                    if let Some(edit) = inline_create {
                        column = column.push(editor_file_browser_inline_entry(
                            app,
                            matches!(edit.kind, crate::app::BrowserInlineEditKind::NewDirectory),
                        ));
                    }
                    column
                },
                |column_widget, entry| {
                    let path = entry.path.clone();
                    let editing = app.browser_inline_edit.as_ref().is_some_and(|edit| {
                        edit.column_index == column_index
                            && edit.target_path.as_ref() == Some(&entry.path)
                    });
                    if editing {
                        return column_widget
                            .push(editor_file_browser_inline_entry(app, entry.is_dir));
                    }

                    let name = entry.name;
                    let is_dir = entry.is_dir;
                    let selected = entry.selected;
                    let drop_targeted = is_dir
                        && app
                            .browser_drop_target
                            .as_ref()
                            .is_some_and(|target| target.path == path);
                    let icon = if is_dir {
                        if selected {
                            icons::folder_open()
                        } else {
                            icons::folder()
                        }
                    } else {
                        icons::file()
                    };

                    let icon_color = move |theme: &Theme, _status| {
                        let palette = theme.extended_palette();
                        svg::Style {
                            color: Some(if selected {
                                palette.background.base.text
                            } else {
                                palette.background.strong.text
                            }),
                        }
                    };

                    column_widget.push(
                        mouse_area(
                            container(
                                row![
                                    container(
                                        svg(icon)
                                            .width(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
                                            .height(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
                                            .content_fit(ContentFit::Contain)
                                            .style(icon_color),
                                    )
                                    .width(Length::Fixed(EDITOR_FILE_BROWSER_ICON_SIZE))
                                    .height(Length::Fixed(
                                        crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT
                                    ))
                                    .center_y(Length::Fixed(
                                        crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT
                                    )),
                                    text(name)
                                        .size(ui_style::FONT_SIZE_UI_SM)
                                        .line_height(1.0)
                                        .width(Fill),
                                ]
                                .spacing(ui_style::SPACE_XS)
                                .align_y(alignment::Vertical::Center)
                                .width(Fill),
                            )
                            .width(Fill)
                            .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_ENTRY_HEIGHT))
                            .padding([0, ui_style::PADDING_XS])
                            .style(move |theme| {
                                ui_style::editor_file_browser_entry(
                                    theme,
                                    selected || drop_targeted,
                                )
                            }),
                        )
                        .interaction(mouse::Interaction::Pointer)
                        .on_enter(Message::Editor(
                            crate::app::EditorMessage::FileBrowserEntryHovered {
                                column_index,
                                path: path.clone(),
                                is_dir,
                            },
                        ))
                        .on_press(Message::Editor(
                            crate::app::EditorMessage::FileBrowserEntryPressed {
                                column_index,
                                path: path.clone(),
                                is_dir,
                            },
                        ))
                        .on_release(Message::Editor(
                            crate::app::EditorMessage::FileBrowserEntryDragReleased {
                                path: path.clone(),
                                is_dir,
                            },
                        ))
                        .on_double_click(Message::Editor(
                            crate::app::EditorMessage::FileBrowserEntryDoublePressed {
                                column_index,
                                path,
                                is_dir,
                            },
                        )),
                    )
                },
            );

            row![
                container(
                    scrollable(entries)
                        .id(crate::app::editor_file_browser_column_scroll_id(
                            column_index
                        ))
                        .direction(scrollable::Direction::Vertical(
                            scrollable::Scrollbar::new().width(4).scroller_width(4),
                        ))
                        .on_scroll(move |viewport| {
                            Message::Editor(crate::app::EditorMessage::FileBrowserColumnScrolled {
                                column_index,
                                viewport,
                            })
                        })
                        .style(ui_style::editor_file_browser_scrollable),
                )
                .width(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_COLUMN_WIDTH))
                .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_HEIGHT))
                .style(ui_style::editor_file_browser_column),
                container(text(""))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_HEIGHT))
                    .style(ui_style::chrome_separator),
            ]
            .spacing(0)
            .align_y(alignment::Vertical::Top)
            .into()
        }
        crate::app::editor::EditorBrowserColumnSummary::FilePreview { metadata } => {
            let size_line: Element<'_, Message> = if let Some(size) = metadata.size.as_ref() {
                text(format!("Size: {size}"))
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .into()
            } else {
                container(text("")).into()
            };
            let modified_line: Element<'_, Message> =
                if let Some(modified) = metadata.modified.as_ref() {
                    text(format!("Modified: {modified}"))
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .into()
                } else {
                    container(text("")).into()
                };
            let created_line: Element<'_, Message> =
                if let Some(created) = metadata.created.as_ref() {
                    text(format!("Created: {created}"))
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .into()
                } else {
                    container(text("")).into()
                };

            row![
                container(
                    iced::widget::column![
                        text(metadata.name)
                            .size(ui_style::FONT_SIZE_UI_SM)
                            .font(iced::Font {
                                weight: iced::font::Weight::Bold,
                                ..fonts::UI
                            }),
                        size_line,
                        modified_line,
                        created_line,
                    ]
                    .spacing(ui_style::SPACE_SM)
                    .padding(ui_style::PADDING_SM),
                )
                .width(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_COLUMN_WIDTH))
                .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_HEIGHT))
                .style(ui_style::editor_file_browser_column),
                container(text(""))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(crate::app::EDITOR_FILE_BROWSER_HEIGHT))
                    .style(ui_style::chrome_separator),
            ]
            .spacing(0)
            .align_y(alignment::Vertical::Top)
            .into()
        }
    }
}
