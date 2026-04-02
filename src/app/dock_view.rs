use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use iced::widget::{
    Column, Tooltip, button, canvas, container, mouse_area, opaque, pane_grid, responsive, row,
    scrollable, slider, stack, svg, text, text_input, tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Padding, Point, Rectangle, Size, Theme, alignment,
    border, mouse,
};

use super::{
    DockDropRegion, EditorFileMenuSection, EditorHeaderMenuSection, Lilypalooza, Message,
    PaneMessage, WorkspacePaneKind, piano_roll, score_view, transport_bar,
};
use crate::{fonts, icons, shortcuts, ui_style};

const TOOLBAR_ICON_SIZE: f32 = 14.0;
const HEADER_CONTROL_HEIGHT: f32 = 22.0;
const HEADER_MENU_ICON_SIZE: f32 = 12.0;
const HEADER_MENU_BUTTON_WIDTH: f32 = 26.0;
const EDITOR_MENU_ROOT_WIDTH: f32 = 126.0;
const EDITOR_FILE_SUBMENU_WIDTH: f32 = 320.0;
const EDITOR_APPEARANCE_SUBMENU_WIDTH: f32 = 272.0;
const EDITOR_MENU_ITEM_HEIGHT: f32 = 24.0;
const EDITOR_RECENT_FILE_LABEL_MAX_CHARS: usize = 40;
const TAB_ICON_GAP: u32 = 6;
const HEADER_WIDTH_SAFETY: f32 = 24.0;
pub(super) const TOOLBAR_HEIGHT: f32 = 32.0;
const TOOLBAR_TOGGLE_ICON_SIZE: f32 = 13.0;
const TOOLBAR_BUTTON_HEIGHT: f32 = 25.0;
const TOOLBAR_FILE_NAME_MAX_CHARS: usize = 20;
const TOOLBAR_PROJECT_NAME_MAX_CHARS: usize = 28;
const PROJECT_MENU_WIDTH: f32 = 280.0;
const PROJECT_RECENT_LABEL_MAX_CHARS: usize = 40;
const EDITOR_TAB_WIDTH: f32 = 140.0;
const EDITOR_TAB_HEIGHT: f32 = 32.0;
const EDITOR_TAB_TITLE_MAX_CHARS: usize = 18;

pub(super) struct HeaderControlGroup<'a> {
    pub(super) min_width: f32,
    pub(super) content: Element<'a, Message>,
}

pub(super) fn view(app: &Lilypalooza) -> Element<'_, Message> {
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

fn workspace_toolbar(app: &Lilypalooza) -> Element<'_, Message> {
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
            .padding([
                ui_style::PADDING_STATUS_BAR_V,
                ui_style::PADDING_STATUS_BAR_H,
            ])
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

fn toolbar_separator() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(16.0))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.strong.color.into()),
                ..container::Style::default()
            }
        })
        .into()
}

fn toolbar_project_button(app: &Lilypalooza) -> Element<'_, Message> {
    let project_title =
        truncate_toolbar_file_name(&app.project_title(), TOOLBAR_PROJECT_NAME_MAX_CHARS);
    let main_score_title = truncate_toolbar_file_name(
        app.current_score
            .as_ref()
            .map(|selected_score| selected_score.file_name.as_str())
            .unwrap_or("No main score"),
        TOOLBAR_FILE_NAME_MAX_CHARS,
    );
    let tooltip_text = "Project menu";
    let chevron = svg(icons::chevron_down())
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0))
        .content_fit(ContentFit::Contain)
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            svg::Style {
                color: Some(match status {
                    svg::Status::Idle => palette.background.base.text,
                    svg::Status::Hovered => palette.primary.weak.text,
                }),
            }
        });
    Tooltip::new(
        button(
            row![
                container(chevron)
                    .width(Length::Fixed(12.0))
                    .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
                    .center_x(Length::Fixed(12.0))
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
                    bottom: 2.0,
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
        .padding([0, 8])
        .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
        .on_press(Message::Pane(PaneMessage::ToggleProjectMenu)),
        text(tooltip_text).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn project_menu_overlay(app: &Lilypalooza) -> Element<'_, Message> {
    let backdrop: Element<'_, Message> = mouse_area(container(text("")).width(Fill).height(Fill))
        .on_press(Message::Pane(PaneMessage::CloseProjectMenu))
        .into();
    let panel: Element<'_, Message> = container(
        mouse_area(opaque(project_menu_panel(app)))
            .on_exit(Message::Pane(PaneMessage::CloseProjectMenu)),
    )
    .padding([TOOLBAR_HEIGHT as u16, ui_style::PADDING_STATUS_BAR_H])
    .width(Fill)
    .height(Fill)
    .align_x(alignment::Horizontal::Left)
    .align_y(alignment::Vertical::Top)
    .into();

    stack([backdrop, panel]).into()
}

fn project_menu_panel<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let save_project = editor_menu_item(
        "Save Project",
        true,
        Some(Message::File(if app.has_saved_project() {
            super::FileMessage::RequestSaveProject
        } else {
            super::FileMessage::RequestCreateProject
        })),
    );

    let load_project = editor_menu_item(
        "Load Project...",
        true,
        Some(Message::File(super::FileMessage::RequestLoadProject)),
    );
    let open_main_score = editor_menu_item(
        "Open Main Score...",
        true,
        Some(Message::File(super::FileMessage::RequestOpen)),
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

    container(column)
        .width(Length::Fixed(PROJECT_MENU_WIDTH))
        .padding(ui_style::PADDING_SM)
        .style(ui_style::tooltip_popup)
        .into()
}

fn project_recent_projects_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
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
                column.push(
                    Tooltip::new(
                        editor_menu_item(
                            label,
                            true,
                            Some(Message::File(super::FileMessage::OpenRecentProject(
                                path.clone(),
                            ))),
                        ),
                        text(path.display().to_string()).size(ui_style::FONT_SIZE_UI_XS),
                        tooltip::Position::Right,
                    )
                    .gap(6)
                    .padding(8)
                    .style(ui_style::tooltip_popup),
                )
            },
        )
        .into()
}

fn truncate_toolbar_file_name(file_name: &str, max_chars: usize) -> String {
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

fn shorten_editor_tab_title(title: &str, max_chars: usize) -> String {
    let chars: Vec<_> = title.chars().collect();
    if chars.len() <= max_chars {
        return title.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    if let Some(dot_index) = title.rfind('.') {
        let extension: Vec<_> = title[dot_index..].chars().collect();
        if extension.len() + 2 < max_chars {
            let prefix_len = max_chars.saturating_sub(extension.len() + 1);
            let prefix: String = chars.into_iter().take(prefix_len).collect();
            return format!("{prefix}…{}", &title[dot_index..]);
        }
    }

    let prefix: String = chars.into_iter().take(max_chars - 1).collect();
    format!("{prefix}…")
}

fn workspace_panes(app: &Lilypalooza) -> Element<'_, Message> {
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
                    WorkspacePaneKind::Editor => editor_pane_body(app),
                    WorkspacePaneKind::Logger => app
                        .logger
                        .view(app.is_workspace_group_focused(*group_id), |action| {
                            Message::Logger(super::LoggerMessage::TextAction(action))
                        }),
                };

                let body = workspace_pane_focus_body(active_pane, body);
                let body = pane_body_with_header_menu(app, *group_id, group_width, body);
                let is_focused = app.is_workspace_group_focused(*group_id);

                pane_grid::Content::new(body)
                    .title_bar(group_title_bar(app, *group_id, group_width, is_focused))
                    .style(move |theme: &Theme| {
                        ui_style::pane_main_surface_focused(theme, is_focused)
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

        mouse_area(
            stack([panes, overlay, drag_capture])
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

fn editor_pane_body(app: &Lilypalooza) -> Element<'_, Message> {
    let content: Element<'_, Message> = iced::widget::column![
        editor_tab_strip(app),
        app.editor.view(
            Message::Editor(super::EditorMessage::OpenRequested),
            |tab_id, message| Message::Editor(super::EditorMessage::Widget { tab_id, message })
        ),
    ]
    .spacing(0)
    .width(Fill)
    .height(Fill)
    .into();

    if app.dragged_editor_tab.is_some() {
        let overlay: Element<'_, Message> =
            mouse_area(container(text("")).width(Fill).height(Fill))
                .on_move(|position| Message::Editor(super::EditorMessage::TabGlobalMoved(position)))
                .on_release(Message::Editor(super::EditorMessage::TabDragReleased))
                .into();

        stack([content, overlay]).width(Fill).height(Fill).into()
    } else {
        content
    }
}

fn editor_tab_strip(app: &Lilypalooza) -> Element<'_, Message> {
    let tabs_row = app.editor.tab_summaries().into_iter().fold(
        row![].spacing(0).align_y(alignment::Vertical::Center),
        |tabs, tab| tabs.push(editor_tab(app, tab)),
    );

    let empty_area = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(EDITOR_TAB_HEIGHT)),
    )
    .on_move(|position| Message::Editor(super::EditorMessage::TabBarMoved(position)))
    .on_enter(Message::Editor(super::EditorMessage::TabBarEmptyMoved))
    .on_double_click(Message::Editor(super::EditorMessage::NewRequested));

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
        .id(super::EDITOR_TABBAR_SCROLL_ID)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(4).scroller_width(4),
        ))
        .on_scroll(|viewport| Message::Editor(super::EditorMessage::TabBarScrolled(viewport)))
        .style(ui_style::editor_tabbar_scrollable)
        .width(Fill)
        .height(Length::Fixed(EDITOR_TAB_HEIGHT)),
    )
    .on_move(|position| Message::Editor(super::EditorMessage::TabBarMoved(position)))
    .on_release(Message::Editor(super::EditorMessage::TabDragReleased))
    .on_exit(Message::Editor(super::EditorMessage::TabDragExited));

    let new_tab = button(
        svg(icons::plus())
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .content_fit(ContentFit::Contain)
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                svg::Style {
                    color: Some(match status {
                        svg::Status::Idle => palette.background.weak.text,
                        svg::Status::Hovered => palette.background.base.text,
                    }),
                }
            }),
    )
    .style(ui_style::button_neutral)
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .on_press(Message::Editor(super::EditorMessage::NewRequested));

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

fn editor_tab(app: &Lilypalooza, tab: super::editor::EditorTabSummary) -> Element<'_, Message> {
    let is_hovered = app.hovered_editor_tab == Some(tab.id);
    let is_dragged = app.dragged_editor_tab == Some(tab.id);
    let is_renaming = app.renaming_editor_tab == Some(tab.id);
    let show_before_drop = app.dragged_editor_tab.is_some()
        && app.hovered_editor_tab == Some(tab.id)
        && !app.editor_tab_drop_after;
    let show_after_drop = app.dragged_editor_tab.is_some()
        && app.hovered_editor_tab == Some(tab.id)
        && app.editor_tab_drop_after;
    let tooltip_label = app
        .editor
        .tab_path(tab.id)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| tab.title.clone());

    let title: Element<'_, Message> = if is_renaming {
        text_input("", &app.editor_tab_rename_value)
            .id(app.editor_tab_rename_input_id.clone())
            .on_input(|value| Message::Editor(super::EditorMessage::RenameInputChanged(value)))
            .on_submit(Message::Editor(super::EditorMessage::CommitRename))
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
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            iced::widget::text::Style {
                color: Some(if is_hovered && !is_dragged {
                    palette.primary.weak.text
                } else if tab.active {
                    palette.background.base.text
                } else {
                    palette.background.strong.text
                }),
            }
        })
        .width(Fill)
        .into()
    };

    let dirty_marker: Element<'_, Message> = if tab.dirty && !is_renaming {
        text("•")
            .size(ui_style::FONT_SIZE_UI_SM)
            .line_height(1.0)
            .into()
    } else {
        container(text("")).width(Length::Fixed(0.0)).into()
    };

    let close_button: Element<'_, Message> = if is_renaming {
        container(text(""))
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(EDITOR_TAB_HEIGHT))
            .center_y(Length::Fixed(EDITOR_TAB_HEIGHT))
            .into()
    } else {
        button(
            container(
                svg(icons::x())
                    .width(Length::Fixed(11.0))
                    .height(Length::Fixed(11.0))
                    .content_fit(ContentFit::Contain)
                    .style(move |theme: &Theme, status| {
                        let palette = theme.extended_palette();
                        svg::Style {
                            color: Some(match status {
                                svg::Status::Idle => {
                                    if tab.active {
                                        palette.background.base.text
                                    } else {
                                        palette.background.strong.text
                                    }
                                }
                                svg::Status::Hovered => palette.primary.weak.text,
                            }),
                        }
                    }),
            )
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(EDITOR_TAB_HEIGHT))
            .center_y(Length::Fixed(EDITOR_TAB_HEIGHT)),
        )
        .style(move |theme: &Theme, status| {
            ui_style::button_editor_tab_close(theme, status, tab.active)
        })
        .padding([0, 0])
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(EDITOR_TAB_HEIGHT))
        .on_press(Message::Editor(super::EditorMessage::CloseTabRequested(
            tab.id,
        )))
        .into()
    };

    let drop_marker = |visible: bool| -> Element<'_, Message> {
        if visible {
            container(text(""))
                .width(Length::Fixed(2.0))
                .height(Length::Fixed(EDITOR_TAB_HEIGHT))
                .style(|theme: &Theme| container::Style {
                    background: Some(theme.extended_palette().primary.base.color.into()),
                    ..container::Style::default()
                })
                .into()
        } else {
            container(text(""))
                .width(Length::Fixed(2.0))
                .height(Length::Fixed(EDITOR_TAB_HEIGHT))
                .into()
        }
    };

    let body: Element<'_, Message> = row![
        drop_marker(show_before_drop),
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
            ui_style::editor_tab_surface(theme, tab.active, is_hovered || is_renaming, is_dragged)
        }),
        drop_marker(show_after_drop)
    ]
    .spacing(0)
    .height(Length::Fixed(EDITOR_TAB_HEIGHT))
    .align_y(alignment::Vertical::Center)
    .into();

    let body = if is_renaming {
        body
    } else {
        mouse_area(body)
            .on_press(Message::Editor(super::EditorMessage::TabPressed(tab.id)))
            .on_double_click(Message::Editor(super::EditorMessage::StartRename(tab.id)))
            .on_move(move |position| {
                Message::Editor(super::EditorMessage::TabMoved {
                    tab_id: tab.id,
                    position,
                })
            })
            .on_exit(Message::Editor(super::EditorMessage::TabHovered(None)))
            .interaction(if is_dragged {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            })
            .into()
    };

    Tooltip::new(
        body,
        text(tooltip_label).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn group_title_bar<'a>(
    app: &'a Lilypalooza,
    group_id: super::DockGroupId,
    group_width: f32,
    is_focused: bool,
) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(group_header(app, group_id, group_width))
        .style(move |theme: &Theme| ui_style::pane_title_bar_surface_focused(theme, is_focused))
}

fn workspace_pane_focus_body<'a>(
    pane: WorkspacePaneKind,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    mouse_area(body)
        .on_press(Message::Pane(PaneMessage::FocusWorkspacePane(pane)))
        .into()
}

fn group_header<'a>(
    app: &'a Lilypalooza,
    group_id: super::DockGroupId,
    group_width: f32,
) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return container(text("")).width(Fill).into();
    };
    let active_pane = group.active;
    let control_groups = pane_header_control_groups(app, group_id, active_pane);
    let title_width = group_tabs_min_width(group);
    let available_controls_width = (group_width - title_width).max(0.0);
    let (inline_controls, overflow_controls) = if active_pane == WorkspacePaneKind::Editor {
        (Vec::new(), vec![text("").into()])
    } else {
        split_header_control_groups(control_groups, available_controls_width)
    };
    let shows_menu_button =
        active_pane == WorkspacePaneKind::Editor || !overflow_controls.is_empty();
    let is_menu_open = shows_menu_button && app.open_header_overflow_menu == Some(group_id);
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

    if shows_menu_button {
        header = header.push(header_overflow_trigger(group_id, is_menu_open));
    }
    iced::widget::column![
        mouse_area(container(header).padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ]),)
        .on_press(Message::Pane(PaneMessage::FocusWorkspacePane(active_pane))),
        container(text(""))
            .height(Length::Fixed(1.0))
            .width(Fill)
            .style(ui_style::chrome_separator),
    ]
    .spacing(0)
    .into()
}

fn pane_body_with_header_menu<'a>(
    app: &'a Lilypalooza,
    group_id: super::DockGroupId,
    group_width: f32,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return body;
    };

    let active_pane = group.active;
    let show_menu = app.open_header_overflow_menu == Some(group_id)
        && (active_pane == WorkspacePaneKind::Editor || pane_header_has_controls(app, active_pane));

    let close_backdrop: Element<'a, Message> = if show_menu {
        mouse_area(container(text("")).width(Fill).height(Fill))
            .on_press(Message::Pane(PaneMessage::CloseHeaderOverflowMenu))
            .into()
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };
    let menu: Element<'a, Message> = if show_menu {
        let menu_content = if active_pane == WorkspacePaneKind::Editor {
            editor_header_menu_panel(app)
        } else {
            let control_groups = pane_header_control_groups(app, group_id, active_pane);
            let title_width = group_tabs_min_width(group);
            let available_controls_width = (group_width - title_width).max(0.0);
            let (_inline_controls, overflow_controls) =
                split_header_control_groups(control_groups, available_controls_width);
            header_overflow_menu_panel(overflow_controls)
        };
        let menu_panel = mouse_area(opaque(menu_content))
            .on_exit(Message::Pane(PaneMessage::CloseHeaderOverflowMenu));
        container(menu_panel)
            .width(Fill)
            .height(Fill)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Top)
            .padding([ui_style::SPACE_XS as u16, ui_style::SPACE_XS as u16])
            .into()
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };
    stack([body, close_backdrop, menu]).into()
}

fn group_tabs<'a>(app: &'a Lilypalooza, group: &'a super::DockGroup) -> row::Row<'a, Message> {
    group.tabs.iter().copied().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Bottom),
        |tabs, pane| tabs.push(workspace_tab(app, pane)),
    )
}

fn workspace_tab(app: &Lilypalooza, pane: WorkspacePaneKind) -> Element<'_, Message> {
    let (is_active, is_stacked) = app
        .group_for_pane(pane)
        .and_then(|group_id| app.workspace_group(group_id))
        .map(|group| (group.active == pane, group.tabs.len() > 1))
        .unwrap_or((false, false));
    let is_hovered = app.hovered_workspace_pane == Some(pane);
    let is_dragging = app.dragged_workspace_pane == Some(pane);
    let title = workspace_pane_title(pane);
    let icon = workspace_pane_icon(pane);
    let icon_color = workspace_tab_foreground_color(is_active, is_hovered, is_dragging);

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
    .padding([
        ui_style::PADDING_STATUS_BAR_V + 3,
        ui_style::PADDING_STATUS_BAR_H + 8,
    ])
    .style(move |theme: &Theme| {
        let palette = theme.extended_palette();

        if is_dragging {
            container::Style {
                background: Some(palette.primary.weak.color.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(10)
                    .width(1)
                    .color(palette.primary.base.color),
                ..container::Style::default()
            }
        } else if is_stacked && is_active {
            container::Style {
                background: Some(Color::TRANSPARENT.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(10)
                    .width(1)
                    .color(palette.background.strong.color),
                ..container::Style::default()
            }
        } else if is_stacked && is_hovered {
            container::Style {
                background: Some(palette.background.base.color.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(10).width(0).color(Color::TRANSPARENT),
                ..container::Style::default()
            }
        } else {
            container::Style {
                background: Some(Color::TRANSPARENT.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(10).width(0).color(Color::TRANSPARENT),
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

fn workspace_tab_foreground_color(
    is_active: bool,
    is_hovered: bool,
    is_dragging: bool,
) -> impl Fn(&Theme) -> Color + Copy {
    move |theme: &Theme| {
        let palette = theme.extended_palette();
        if is_dragging {
            palette.primary.weak.text
        } else if is_active {
            palette.background.weakest.text
        } else if is_hovered {
            palette.background.base.text
        } else {
            palette.background.strong.text
        }
    }
}

fn workspace_pane_title(pane: WorkspacePaneKind) -> &'static str {
    match pane {
        WorkspacePaneKind::Score => "Score",
        WorkspacePaneKind::PianoRoll => "Piano Roll",
        WorkspacePaneKind::Editor => "Editor",
        WorkspacePaneKind::Logger => "Logger",
    }
}

fn workspace_pane_icon(pane: WorkspacePaneKind) -> svg::Handle {
    match pane {
        WorkspacePaneKind::Score => icons::music_4(),
        WorkspacePaneKind::PianoRoll => icons::piano(),
        WorkspacePaneKind::Editor => icons::file_pen(),
        WorkspacePaneKind::Logger => icons::scroll_text(),
    }
}

fn all_workspace_panes() -> [WorkspacePaneKind; 4] {
    [
        WorkspacePaneKind::Editor,
        WorkspacePaneKind::Score,
        WorkspacePaneKind::PianoRoll,
        WorkspacePaneKind::Logger,
    ]
}

fn toolbar_pane_toggle(app: &Lilypalooza, pane: WorkspacePaneKind) -> Element<'static, Message> {
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

    Tooltip::new(
        button(icon)
            .style(if is_visible {
                ui_style::button_toolbar_toggle_active
            } else {
                ui_style::button_toolbar_chip
            })
            .padding([6, 7])
            .on_press(Message::Pane(PaneMessage::ToggleWorkspacePane(pane))),
        text(tooltip_label).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn empty_workspace_placeholder(app: &Lilypalooza) -> Element<'_, Message> {
    let lilypond_label = match &app.lilypond_status {
        super::LilypondStatus::Checking => "LilyPond: checking...".to_string(),
        super::LilypondStatus::Ready { detected, .. } => {
            format!("LilyPond: {detected}")
        }
        super::LilypondStatus::Unavailable => "LilyPond: unavailable".to_string(),
    };

    container(
        Column::new()
            .push(
                text(format!("Lilypalooza {}", env!("CARGO_PKG_VERSION")))
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(fonts::MONO),
            )
            .push(
                text(lilypond_label)
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(fonts::MONO),
            )
            .spacing(ui_style::SPACE_SM)
            .align_x(alignment::Horizontal::Center),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill)
    .into()
}

fn header_overflow_button(
    group_id: super::DockGroupId,
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
    .style(ui_style::button_window_control)
    .padding([4, 7])
    .width(Length::Fixed(HEADER_MENU_BUTTON_WIDTH))
    .height(Length::Fixed(HEADER_CONTROL_HEIGHT))
    .on_press(on_press);

    let tooltip = if is_open {
        "Hide pane controls"
    } else {
        "Show pane controls"
    };

    Tooltip::new(
        container(button).padding([0, 2]),
        text(tooltip).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn header_overflow_trigger(
    group_id: super::DockGroupId,
    is_open: bool,
) -> Element<'static, Message> {
    header_overflow_button(group_id, is_open)
}

fn header_overflow_menu_panel<'a>(controls: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    container(
        Column::with_children(controls)
            .spacing(ui_style::SPACE_XS)
            .align_x(alignment::Horizontal::Left),
    )
    .padding(ui_style::PADDING_XS)
    .style(ui_style::tooltip_popup)
    .into()
}

pub(super) fn workspace_group_min_width(app: &Lilypalooza, group_id: super::DockGroupId) -> f32 {
    let Some(group) = app.workspace_group(group_id) else {
        return 0.0;
    };
    let tabs_width = group_tabs_min_width(group);
    let menu_width = if pane_header_has_controls(app, group.active) {
        HEADER_MENU_BUTTON_WIDTH
    } else {
        0.0
    };

    tabs_width + menu_width + HEADER_WIDTH_SAFETY
}

fn workspace_tab_min_width(pane: WorkspacePaneKind) -> f32 {
    let title_width = match pane {
        WorkspacePaneKind::Score => 36.0,
        WorkspacePaneKind::PianoRoll => 66.0,
        WorkspacePaneKind::Editor => 38.0,
        WorkspacePaneKind::Logger => 42.0,
    };

    TOOLBAR_ICON_SIZE
        + TAB_ICON_GAP as f32
        + title_width
        + (ui_style::PADDING_STATUS_BAR_H + 8) as f32 * 2.0
}

fn group_tabs_min_width(group: &super::DockGroup) -> f32 {
    group
        .tabs
        .iter()
        .copied()
        .map(workspace_tab_min_width)
        .sum::<f32>()
        + ui_style::SPACE_XS as f32 * group.tabs.len().saturating_sub(1) as f32
}

fn split_header_control_groups<'a>(
    groups: Vec<HeaderControlGroup<'a>>,
    available_width: f32,
) -> (Vec<Element<'a, Message>>, Vec<Element<'a, Message>>) {
    let total_width = header_groups_total_width(&groups);
    if groups.is_empty() || total_width <= available_width {
        return (
            groups.into_iter().map(|group| group.content).collect(),
            Vec::new(),
        );
    }

    let available_inline_width = (available_width - HEADER_MENU_BUTTON_WIDTH).max(0.0);
    let mut used_width = 0.0;
    let mut inline = Vec::new();
    let mut overflow = Vec::new();

    for group in groups {
        let spacing = if inline.is_empty() {
            0.0
        } else {
            ui_style::SPACE_SM as f32
        };

        if used_width + spacing + group.min_width <= available_inline_width {
            used_width += spacing + group.min_width;
            inline.push(group.content);
        } else {
            overflow.push(group.content);
        }
    }

    (inline, overflow)
}

fn header_groups_total_width(groups: &[HeaderControlGroup<'_>]) -> f32 {
    groups.iter().map(|group| group.min_width).sum::<f32>()
        + ui_style::SPACE_SM as f32 * groups.len().saturating_sub(1) as f32
}

pub(super) fn compact_control_icon(icon: svg::Handle) -> Element<'static, Message> {
    container(
        svg(icon)
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .content_fit(ContentFit::Contain)
            .style(ui_style::svg_window_control),
    )
    .width(Length::Fixed(12.0))
    .height(Length::Fixed(12.0))
    .center_x(Length::Fixed(12.0))
    .center_y(Length::Fixed(12.0))
    .into()
}

fn header_icon(icon: svg::Handle, size: f32) -> Element<'static, Message> {
    container(
        svg(icon)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .content_fit(ContentFit::Contain)
            .style(ui_style::svg_window_control),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fixed(size))
    .center_y(Length::Fixed(size))
    .into()
}

fn pane_header_control_groups<'a>(
    app: &'a Lilypalooza,
    _group_id: super::DockGroupId,
    pane: WorkspacePaneKind,
) -> Vec<HeaderControlGroup<'a>> {
    match pane {
        WorkspacePaneKind::Score => score_view::score_controls(app),
        WorkspacePaneKind::PianoRoll => piano_roll::controls(app),
        WorkspacePaneKind::Editor => Vec::new(),
        WorkspacePaneKind::Logger => logger_controls(app),
    }
}

fn pane_header_has_controls(app: &Lilypalooza, pane: WorkspacePaneKind) -> bool {
    match pane {
        WorkspacePaneKind::Score => app.current_score.is_some(),
        WorkspacePaneKind::PianoRoll => true,
        WorkspacePaneKind::Editor => true,
        WorkspacePaneKind::Logger => true,
    }
}

fn workspace_drag_overlay(app: &Lilypalooza, size: Size) -> Element<'_, Message> {
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

fn workspace_drag_capture_layer(app: &Lilypalooza) -> Element<'_, Message> {
    if app.pressed_workspace_pane.is_none() && app.dragged_workspace_pane.is_none() {
        return container(text("")).width(Fill).height(Fill).into();
    }

    mouse_area(container(text("")).width(Fill).height(Fill))
        .on_move(|position| Message::Pane(PaneMessage::WorkspaceDragMoved(position)))
        .on_release(Message::Pane(PaneMessage::WorkspaceDragReleased))
        .on_exit(Message::Pane(PaneMessage::WorkspaceDragExited))
        .into()
}

fn workspace_group_bounds_map(
    state: &pane_grid::State<super::DockGroupId>,
    size: Size,
) -> HashMap<super::DockGroupId, Rectangle> {
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

fn collect_group_bounds(
    state: &pane_grid::State<super::DockGroupId>,
    node: &pane_grid::Node,
    bounds: Rectangle,
    group_bounds: &mut HashMap<super::DockGroupId, Rectangle>,
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
                collect_group_bounds(
                    state,
                    a,
                    Rectangle {
                        height: first_height,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_group_bounds(
                    state,
                    b,
                    Rectangle {
                        y: bounds.y + first_height,
                        height: bounds.height - first_height,
                        ..bounds
                    },
                    group_bounds,
                );
            }
            pane_grid::Axis::Vertical => {
                let first_width = bounds.width * ratio;
                collect_group_bounds(
                    state,
                    a,
                    Rectangle {
                        width: first_width,
                        ..bounds
                    },
                    group_bounds,
                );
                collect_group_bounds(
                    state,
                    b,
                    Rectangle {
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

fn preview_bounds_for_region(bounds: Rectangle, region: DockDropRegion) -> Rectangle {
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

fn split_rearrange_style(theme: &Theme) -> pane_grid::Style {
    let mut style = pane_grid::default(theme);
    style.hovered_region.background = Color::TRANSPARENT.into();
    style.hovered_region.border = border::rounded(0).width(0).color(Color::TRANSPARENT);
    style
}

struct DropOverlayCanvas {
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

fn logger_controls<'a>(app: &'a Lilypalooza) -> Vec<HeaderControlGroup<'a>> {
    let clear_button = button(compact_control_icon(icons::brush_cleaning()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let clear_button = if app.logger.is_empty() {
        clear_button
    } else {
        clear_button.on_press(Message::Logger(super::LoggerMessage::RequestClear))
    };

    vec![HeaderControlGroup {
        min_width: 32.0,
        content: Tooltip::new(
            clear_button,
            text("Clear").size(ui_style::FONT_SIZE_UI_XS),
            tooltip::Position::Top,
        )
        .gap(6)
        .padding(8)
        .style(ui_style::tooltip_popup)
        .into(),
    }]
}

fn editor_header_menu_panel<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
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

fn editor_submenu_offset(section: EditorHeaderMenuSection) -> f32 {
    let item_index = match section {
        EditorHeaderMenuSection::File => 0.0,
        EditorHeaderMenuSection::Appearance => 1.0,
    };

    f32::from(ui_style::PADDING_XS)
        + item_index * (EDITOR_MENU_ITEM_HEIGHT + ui_style::SPACE_XS as f32)
}

fn editor_root_menu_item<'a>(
    label: &'a str,
    active: bool,
    section: EditorHeaderMenuSection,
) -> Element<'a, Message> {
    let button = button(
        row![
            svg(icons::chevron_left())
                .width(Length::Fixed(10.0))
                .height(Length::Fixed(10.0))
                .content_fit(ContentFit::Contain)
                .style(move |theme: &Theme, _status| svg::Style {
                    color: Some(if active {
                        theme.extended_palette().background.weakest.text
                    } else {
                        Color::from_rgb(0.12, 0.12, 0.14)
                    }),
                }),
            text(label).size(ui_style::FONT_SIZE_UI_XS),
            container(text("")).width(Fill),
        ]
        .spacing(ui_style::SPACE_XS)
        .width(Fill)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V + 2,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .style(move |theme: &Theme, status| ui_style::button_menu_item(theme, status, active))
    .on_press(Message::Pane(PaneMessage::SetEditorHeaderMenuSection(
        Some(section),
    )));

    mouse_area(button)
        .interaction(mouse::Interaction::Pointer)
        .on_enter(Message::Pane(PaneMessage::SetEditorHeaderMenuSection(
            Some(section),
        )))
        .into()
}

fn editor_file_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let has_document = app.editor.has_document();
    let has_recent_files = !app.editor_recent_files.is_empty();
    let recent_open = app.open_editor_file_menu_section == Some(EditorFileMenuSection::OpenRecent);
    let recent_hovered =
        app.hovered_editor_file_menu_section == Some(EditorFileMenuSection::OpenRecent);

    let mut column = Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(editor_file_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::NewEditor,
            )
            .map(|shortcut| format!("New ({shortcut})"))
            .unwrap_or_else(|| "New".to_string()),
            true,
            Some(Message::Editor(super::EditorMessage::NewRequested)),
        ))
        .push(editor_file_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::OpenEditorFile,
            )
            .map(|shortcut| format!("Open... ({shortcut})"))
            .unwrap_or_else(|| "Open...".to_string()),
            true,
            Some(Message::Editor(super::EditorMessage::OpenRequested)),
        ))
        .push(editor_file_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::SaveEditor,
            )
            .map(|shortcut| format!("Save ({shortcut})"))
            .unwrap_or_else(|| "Save".to_string()),
            has_document,
            Some(Message::Editor(super::EditorMessage::SaveRequested)),
        ))
        .push(editor_file_menu_item(
            "Save As...",
            has_document,
            Some(Message::Editor(super::EditorMessage::SaveAsRequested)),
        ))
        .push(editor_file_menu_item(
            "Rename...",
            has_document,
            Some(Message::Editor(super::EditorMessage::RenameRequested)),
        ));

    let recent_row = if has_recent_files {
        mouse_area(editor_fold_menu_item(
            "Open Recent",
            has_recent_files,
            recent_open,
            recent_hovered,
            Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: Some(EditorFileMenuSection::OpenRecent),
                expanded: !recent_open,
            }),
        ))
        .interaction(mouse::Interaction::Pointer)
        .on_move(|position| {
            Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: Some(EditorFileMenuSection::OpenRecent),
                expanded: position.x >= EDITOR_FILE_SUBMENU_WIDTH * 0.5,
            })
        })
        .into()
    } else {
        editor_fold_menu_item(
            "Open Recent",
            false,
            false,
            false,
            Message::Pane(PaneMessage::CloseHeaderOverflowMenu),
        )
    };

    let mut recent_section = Column::new().spacing(ui_style::SPACE_XS).push(recent_row);

    if recent_open {
        recent_section = recent_section.push(container(editor_recent_files_submenu(app)).padding(
            Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: f32::from(ui_style::PADDING_MD),
            },
        ));
    }

    column = column.push(
        mouse_area(recent_section)
            .interaction(if has_recent_files {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            })
            .on_enter(Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: Some(EditorFileMenuSection::OpenRecent),
                expanded: recent_open,
            }))
            .on_exit(Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: None,
                expanded: false,
            })),
    );

    column.into()
}

fn editor_recent_files_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    if app.editor_recent_files.is_empty() {
        return Column::new()
            .spacing(ui_style::SPACE_XS)
            .push(editor_menu_item("No recent files", false, None))
            .into();
    }

    let recent_paths: Vec<_> = app
        .editor_recent_files
        .iter()
        .take(app.editor_recent_files_limit)
        .cloned()
        .collect();
    let labels = recent_file_labels(&recent_paths, EDITOR_RECENT_FILE_LABEL_MAX_CHARS);

    recent_paths
        .into_iter()
        .zip(labels)
        .fold(
            Column::new().spacing(ui_style::SPACE_XS),
            |column, (path, label)| {
                column.push(editor_recent_file_item(
                    label,
                    path.clone(),
                    Message::Editor(super::EditorMessage::OpenRecent(path)),
                ))
            },
        )
        .into()
}

fn editor_recent_file_item<'a>(
    label: String,
    full_path: PathBuf,
    on_press: Message,
) -> Element<'a, Message> {
    Tooltip::new(
        editor_menu_item(label, true, Some(on_press)),
        text(full_path.display().to_string()).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Right,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn recent_file_labels(paths: &[PathBuf], max_chars: usize) -> Vec<String> {
    let components: Vec<Vec<String>> = paths
        .iter()
        .map(|path| path_display_components(path))
        .collect();
    let mut suffix_lengths = vec![1; components.len()];

    loop {
        let mut collisions: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, parts) in components.iter().enumerate() {
            collisions
                .entry(suffix_path(parts, suffix_lengths[index]))
                .or_default()
                .push(index);
        }

        let mut changed = false;
        for indices in collisions.values() {
            if indices.len() < 2 {
                continue;
            }

            for &index in indices {
                if suffix_lengths[index] < components[index].len() {
                    suffix_lengths[index] += 1;
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
    }

    components
        .iter()
        .zip(suffix_lengths)
        .map(|(parts, suffix_len)| {
            truncate_recent_label(&suffix_path(parts, suffix_len), max_chars)
        })
        .collect()
}

fn path_display_components(path: &Path) -> Vec<String> {
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_string()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect();

    if parts.is_empty() {
        parts.push(path.display().to_string());
    }

    parts
}

fn suffix_path(parts: &[String], count: usize) -> String {
    let start = parts.len().saturating_sub(count);
    parts[start..].join("/")
}

fn truncate_recent_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }

    let parts: Vec<&str> = label.split('/').collect();
    let Some(file_name) = parts.last().copied() else {
        return label.to_string();
    };

    if file_name.chars().count() >= max_chars {
        return truncate_from_left(file_name, max_chars);
    }

    let mut suffix = file_name.to_string();
    for parent in parts[..parts.len().saturating_sub(1)].iter().rev() {
        let candidate = format!("{parent}/{suffix}");
        let display = format!("…/{candidate}");
        if display.chars().count() <= max_chars {
            suffix = candidate;
        } else {
            break;
        }
    }

    format!("…/{suffix}")
}

fn truncate_from_left(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    let keep = max_chars - 1;
    let tail: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

fn editor_appearance_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let zoom_out_button = if app.editor.can_zoom_out() {
        button(compact_control_icon(icons::zoom_out()))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
            .on_press(Message::Editor(super::EditorMessage::ZoomOut))
    } else {
        button(compact_control_icon(icons::zoom_out()))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
    };
    let zoom_in_button = if app.editor.can_zoom_in() {
        button(compact_control_icon(icons::zoom_in()))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
            .on_press(Message::Editor(super::EditorMessage::ZoomIn))
    } else {
        button(compact_control_icon(icons::zoom_in()))
            .style(ui_style::button_neutral)
            .padding([
                ui_style::PADDING_BUTTON_COMPACT_V,
                ui_style::PADDING_BUTTON_COMPACT_H,
            ])
    };
    let zoom_value_label = text(format!("{}pt", app.editor.font_size_points()))
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(fonts::MONO);
    let zoom_value = if app.editor.can_reset_zoom() {
        mouse_area(zoom_value_label)
            .on_double_click(Message::Editor(super::EditorMessage::ResetZoom))
    } else {
        mouse_area(zoom_value_label)
    };
    let zoom_value = Tooltip::new(
        zoom_value,
        text("Double-click to reset").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup);

    Column::new()
        .spacing(ui_style::SPACE_SM)
        .push(
            row![
                text("Font Size").size(ui_style::FONT_SIZE_UI_XS),
                zoom_out_button,
                zoom_value,
                zoom_in_button
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .push(
            container(
                container(text(""))
                    .width(Fill)
                    .height(Length::Fixed(1.0))
                    .style(ui_style::chrome_separator),
            )
            .padding([ui_style::SPACE_SM as u16, 0]),
        )
        .push(editor_theme_controls_column(app))
        .into()
}

fn editor_menu_item<'a>(
    label: impl Into<String>,
    enabled: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let mut item = button(
        container(text(label.into()).size(ui_style::FONT_SIZE_UI_XS))
            .width(Fill)
            .align_x(alignment::Horizontal::Left),
    )
    .width(Fill)
    .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
    .padding([
        ui_style::PADDING_BUTTON_COMPACT_V + 2,
        ui_style::PADDING_BUTTON_COMPACT_H,
    ])
    .style(|theme: &Theme, status| ui_style::button_menu_item(theme, status, false));

    if enabled && let Some(message) = on_press {
        item = item.on_press(message);
    }

    item.into()
}

fn editor_file_menu_item<'a>(
    label: impl Into<String>,
    enabled: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    editor_menu_item(label, enabled, on_press)
}

fn editor_fold_menu_item<'a>(
    label: &'a str,
    enabled: bool,
    active: bool,
    hovered: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let highlighted = active || hovered;
    let content = row![
        container(text(label).size(ui_style::FONT_SIZE_UI_XS))
            .width(Fill)
            .align_x(alignment::Horizontal::Left),
        svg(icons::chevron_down())
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(12.0))
            .content_fit(ContentFit::Contain)
            .style(move |theme: &Theme, _status| svg::Style {
                color: Some(if highlighted {
                    theme.extended_palette().background.weakest.text
                } else {
                    Color::from_rgb(0.12, 0.12, 0.14)
                }),
            }),
    ]
    .spacing(ui_style::SPACE_XS)
    .width(Fill)
    .align_y(alignment::Vertical::Center);

    let button = button(content)
        .width(Fill)
        .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V + 2,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ])
        .style(move |theme: &Theme, status| {
            ui_style::button_menu_item(theme, status, active || hovered)
        });

    if enabled {
        button.on_press(on_press).into()
    } else {
        button.into()
    }
}

fn editor_theme_controls_column<'a>(app: &'a Lilypalooza) -> Column<'a, Message> {
    let settings = app.editor.theme_settings();

    Column::with_children(vec![
        editor_theme_slider(
            "Hue",
            format!("{:+.0}°", settings.hue_offset_degrees),
            -180.0..=180.0,
            settings.hue_offset_degrees,
            1.0,
            |value| Message::Editor(super::EditorMessage::SetThemeHueOffsetDegrees(value)),
        ),
        editor_theme_slider(
            "Saturation",
            format!("{:.2}", settings.saturation),
            0.0..=1.8,
            settings.saturation,
            0.01,
            |value| Message::Editor(super::EditorMessage::SetThemeSaturation(value)),
        ),
        editor_theme_slider(
            "Warmth",
            format!("{:+.2}", settings.warmth),
            -1.0..=1.0,
            settings.warmth,
            0.01,
            |value| Message::Editor(super::EditorMessage::SetThemeWarmth(value)),
        ),
        editor_theme_slider(
            "Brightness",
            format!("{:.2}", settings.brightness),
            0.5..=1.8,
            settings.brightness,
            0.01,
            |value| Message::Editor(super::EditorMessage::SetThemeBrightness(value)),
        ),
        editor_theme_slider(
            "Text Dim",
            format!("{:.2}", settings.text_dim),
            0.5..=3.0,
            settings.text_dim,
            0.01,
            |value| Message::Editor(super::EditorMessage::SetThemeTextDim(value)),
        ),
        editor_theme_slider(
            "Comment Dim",
            format!("{:.2}", settings.comment_dim),
            0.5..=1.8,
            settings.comment_dim,
            0.01,
            |value| Message::Editor(super::EditorMessage::SetThemeCommentDim(value)),
        ),
    ])
    .spacing(ui_style::SPACE_SM)
}

fn editor_theme_slider<'a>(
    label: &'a str,
    value: String,
    range: std::ops::RangeInclusive<f32>,
    current: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(
            row![
                text(label).size(ui_style::FONT_SIZE_UI_XS),
                container(
                    text(value)
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .font(fonts::MONO)
                )
                .width(Fill)
                .align_x(alignment::Horizontal::Right),
            ]
            .align_y(alignment::Vertical::Center),
        )
        .push(
            slider(range, current, on_change)
                .step(step)
                .shift_step(step * 10.0),
        )
        .into()
}
