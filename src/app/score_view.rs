use std::collections::HashMap;

use iced::widget::{
    Column, Tooltip, button, canvas, container, mouse_area, opaque, pane_grid, responsive, row,
    scrollable, stack, svg, text, tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Point, Rectangle, Size, Theme, alignment, border,
    mouse,
};
use iced_core::image;

use super::{
    DockDropRegion, LilyView, Message, PaneMessage, ScoreCursorPlacement, ViewerMessage,
    WorkspacePaneKind, piano_roll, transport_bar,
};
use crate::{icons, ui_style};

const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 16.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
const TOOLBAR_ICON_SIZE: f32 = 14.0;
const HEADER_CONTROL_HEIGHT: f32 = 22.0;
const HEADER_MENU_ICON_SIZE: f32 = 12.0;
const HEADER_MENU_BUTTON_WIDTH: f32 = 26.0;
const TAB_ICON_GAP: u32 = 6;
const HEADER_WIDTH_SAFETY: f32 = 24.0;
pub(super) const TOOLBAR_HEIGHT: f32 = 32.0;
const TOOLBAR_TOGGLE_ICON_SIZE: f32 = 13.0;
const TOOLBAR_BUTTON_HEIGHT: f32 = 25.0;
const TOOLBAR_FILE_NAME_MAX_CHARS: usize = 24;
const SCORE_BASE_SCALE: f32 = 5.6;

pub(super) struct HeaderControlGroup<'a> {
    pub(super) min_width: f32,
    pub(super) content: Element<'a, Message>,
}

pub(super) fn score_base_scale() -> f32 {
    SCORE_BASE_SCALE
}

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let toolbar = workspace_toolbar(app);
    let workspace = workspace_panes(app);

    iced::widget::column![toolbar, workspace, transport_bar::view(app)]
        .width(Fill)
        .height(Fill)
        .spacing(0)
        .into()
}

fn workspace_toolbar(app: &LilyView) -> Element<'_, Message> {
    let pane_toggles = all_workspace_panes().into_iter().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        |row, pane| row.push(toolbar_pane_toggle(app, pane)),
    );

    container(
        row![
            toolbar_file_button(app),
            toolbar_separator(),
            pane_toggles,
            container(text("")).width(Fill),
        ]
        .spacing(ui_style::SPACE_SM)
        .align_y(alignment::Vertical::Center)
        .width(Fill),
    )
    .height(Length::Fixed(TOOLBAR_HEIGHT))
    .padding([
        ui_style::PADDING_STATUS_BAR_V,
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .style(ui_style::workspace_toolbar_surface)
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

fn toolbar_file_button(app: &LilyView) -> Element<'_, Message> {
    let file_name = app
        .current_score
        .as_ref()
        .map(|selected_score| selected_score.file_name.as_str())
        .unwrap_or("No file");
    let button_label = truncate_toolbar_file_name(file_name, TOOLBAR_FILE_NAME_MAX_CHARS);

    let icon = svg(icons::file_music())
        .width(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
        .height(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
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
                container(icon)
                    .width(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
                    .height(Fill)
                    .center_x(Length::Fixed(TOOLBAR_TOGGLE_ICON_SIZE))
                    .center_y(Fill),
                text(button_label)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(iced::Font::MONOSPACE)
                    .line_height(1.0)
                    .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
                    .align_y(alignment::Vertical::Center),
            ]
            .height(Fill)
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .style(ui_style::button_toolbar_chip)
        .padding([6, 8])
        .height(Length::Fixed(TOOLBAR_BUTTON_HEIGHT))
        .on_press(Message::File(super::FileMessage::RequestOpen)),
        text("Open file").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
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

fn workspace_panes(app: &LilyView) -> Element<'_, Message> {
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
                let body = match app
                    .workspace_group(*group_id)
                    .map(|group| group.active)
                    .unwrap_or(WorkspacePaneKind::Score)
                {
                    WorkspacePaneKind::Score => score_body(app),
                    WorkspacePaneKind::PianoRoll => piano_roll::content(app),
                    WorkspacePaneKind::Editor => app
                        .editor
                        .view(|message| Message::Editor(super::EditorMessage::Widget(message))),
                    WorkspacePaneKind::Logger => app
                        .logger
                        .view(|action| Message::Logger(super::LoggerMessage::TextAction(action))),
                };

                let body = pane_body_with_header_menu(app, *group_id, group_width, body);

                pane_grid::Content::new(body)
                    .title_bar(group_title_bar(app, *group_id, group_width))
                    .style(ui_style::pane_main_surface)
            })
            .width(Fill)
            .height(Fill)
            .style(split_rearrange_style)
            .on_resize(8, |event| {
                Message::Pane(PaneMessage::WorkspaceResized(event))
            })
            .into();

        let overlay = workspace_drag_overlay(app, size);

        mouse_area(stack([panes, overlay]).width(Fill).height(Fill))
            .on_move(|position| Message::Pane(PaneMessage::WorkspaceDragMoved(position)))
            .on_release(Message::Pane(PaneMessage::WorkspaceDragReleased))
            .on_exit(Message::Pane(PaneMessage::WorkspaceDragExited))
            .into()
    })
    .into()
}

fn group_title_bar<'a>(
    app: &'a LilyView,
    group_id: super::DockGroupId,
    group_width: f32,
) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(group_header(app, group_id, group_width))
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .style(ui_style::pane_title_bar_surface)
}

fn group_header<'a>(
    app: &'a LilyView,
    group_id: super::DockGroupId,
    group_width: f32,
) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return container(text("")).width(Fill).into();
    };
    let active_pane = group.active;
    let control_groups = pane_header_control_groups(app, active_pane);
    let title_width = group_tabs_min_width(group);
    let available_controls_width = (group_width - title_width).max(0.0);
    let (inline_controls, overflow_controls) =
        split_header_control_groups(control_groups, available_controls_width);
    let shows_menu_button = !overflow_controls.is_empty();
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
    header.into()
}

fn pane_body_with_header_menu<'a>(
    app: &'a LilyView,
    group_id: super::DockGroupId,
    group_width: f32,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return body;
    };

    let active_pane = group.active;
    let control_groups = pane_header_control_groups(app, active_pane);
    let title_width = group_tabs_min_width(group);
    let available_controls_width = (group_width - title_width).max(0.0);
    let (_inline_controls, overflow_controls) =
        split_header_control_groups(control_groups, available_controls_width);
    let show_menu =
        !overflow_controls.is_empty() && app.open_header_overflow_menu == Some(group_id);

    let close_backdrop: Element<'a, Message> = if show_menu {
        mouse_area(container(text("")).width(Fill).height(Fill))
            .on_press(Message::Pane(PaneMessage::CloseHeaderOverflowMenu))
            .into()
    } else {
        container(text("")).width(Fill).height(Fill).into()
    };
    let menu: Element<'a, Message> = if show_menu {
        let menu_panel = mouse_area(opaque(header_overflow_menu_panel(overflow_controls)))
            .on_press(Message::Pane(PaneMessage::OpenHeaderOverflowMenu(group_id)))
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

fn group_tabs<'a>(app: &'a LilyView, group: &'a super::DockGroup) -> row::Row<'a, Message> {
    group.tabs.iter().copied().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Bottom),
        |tabs, pane| tabs.push(workspace_tab(app, pane)),
    )
}

fn workspace_tab(app: &LilyView, pane: WorkspacePaneKind) -> Element<'_, Message> {
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

fn pane_shortcut_label(pane: WorkspacePaneKind) -> &'static str {
    #[cfg(target_os = "macos")]
    {
        match pane {
            WorkspacePaneKind::Editor => "Cmd+1",
            WorkspacePaneKind::Score => "Cmd+2",
            WorkspacePaneKind::PianoRoll => "Cmd+3",
            WorkspacePaneKind::Logger => "Cmd+4",
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        match pane {
            WorkspacePaneKind::Editor => "Ctrl+1",
            WorkspacePaneKind::Score => "Ctrl+2",
            WorkspacePaneKind::PianoRoll => "Ctrl+3",
            WorkspacePaneKind::Logger => "Ctrl+4",
        }
    }
}

fn toolbar_pane_toggle(app: &LilyView, pane: WorkspacePaneKind) -> Element<'static, Message> {
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

    let tooltip_label = format!("{title} ({})", pane_shortcut_label(pane));

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

fn empty_workspace_placeholder(app: &LilyView) -> Element<'_, Message> {
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
                text(format!("lily-view {}", env!("CARGO_PKG_VERSION")))
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font::MONOSPACE),
            )
            .push(
                text(lilypond_label)
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font::MONOSPACE),
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

pub(super) fn workspace_group_min_width(app: &LilyView, group_id: super::DockGroupId) -> f32 {
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
    app: &'a LilyView,
    pane: WorkspacePaneKind,
) -> Vec<HeaderControlGroup<'a>> {
    match pane {
        WorkspacePaneKind::Score => score_controls(app),
        WorkspacePaneKind::PianoRoll => piano_roll::controls(app),
        WorkspacePaneKind::Editor => editor_controls(app),
        WorkspacePaneKind::Logger => logger_controls(app),
    }
}

fn pane_header_has_controls(app: &LilyView, pane: WorkspacePaneKind) -> bool {
    match pane {
        WorkspacePaneKind::Score => app.current_score.is_some(),
        WorkspacePaneKind::PianoRoll => true,
        WorkspacePaneKind::Editor => app.editor.has_document(),
        WorkspacePaneKind::Logger => true,
    }
}

fn workspace_drag_overlay(app: &LilyView, size: Size) -> Element<'_, Message> {
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

pub(super) fn score_controls<'a>(app: &'a LilyView) -> Vec<HeaderControlGroup<'a>> {
    if app.current_score.is_none() {
        return Vec::new();
    }

    let page_label = app
        .rendered_score
        .as_ref()
        .map(|rendered_score| {
            format!(
                "Page {}/{}",
                rendered_score.current_page_number(),
                rendered_score.page_count()
            )
        })
        .unwrap_or_else(|| "Page 0/0".to_string());
    let zoom_label = format!("{:.0}%", app.svg_zoom * 100.0);
    let brightness_label = format!("{}%", app.svg_page_brightness);
    let can_prev_page = app
        .rendered_score
        .as_ref()
        .is_some_and(|rendered_score| rendered_score.current_page > 0);
    let can_next_page = app.rendered_score.as_ref().is_some_and(|rendered_score| {
        rendered_score.current_page.saturating_add(1) < rendered_score.pages.len()
    });
    let can_zoom_in = app.svg_zoom < super::MAX_SVG_ZOOM;
    let can_zoom_out = app.svg_zoom > super::MIN_SVG_ZOOM;
    let can_brightness_increase = app.svg_page_brightness < super::MAX_SVG_PAGE_BRIGHTNESS;
    let can_brightness_decrease = app.svg_page_brightness > super::MIN_SVG_PAGE_BRIGHTNESS;
    let can_reset_zoom = (app.svg_zoom - app.default_settings.score_view.zoom).abs() > 1e-4;
    let can_reset_page_brightness =
        app.svg_page_brightness != app.default_settings.score_view.page_brightness;

    let prev_button = button(compact_control_icon(icons::arrow_left()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let prev_button = if can_prev_page {
        prev_button.on_press(Message::Viewer(ViewerMessage::PrevPage))
    } else {
        prev_button
    };

    let next_button = button(compact_control_icon(icons::arrow_right()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let next_button = if can_next_page {
        next_button.on_press(Message::Viewer(ViewerMessage::NextPage))
    } else {
        next_button
    };

    let zoom_out_button = button(text("−").size(ui_style::FONT_SIZE_UI_SM))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let zoom_out_button = if can_zoom_out {
        zoom_out_button.on_press(Message::Viewer(ViewerMessage::ZoomOut))
    } else {
        zoom_out_button
    };

    let zoom_in_button = button(text("+").size(ui_style::FONT_SIZE_UI_SM))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let zoom_in_button = if can_zoom_in {
        zoom_in_button.on_press(Message::Viewer(ViewerMessage::ZoomIn))
    } else {
        zoom_in_button
    };

    let brightness_down_button = button(text("−").size(ui_style::FONT_SIZE_UI_SM))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let brightness_down_button = if can_brightness_decrease {
        brightness_down_button.on_press(Message::Viewer(ViewerMessage::DecreasePageBrightness))
    } else {
        brightness_down_button
    };

    let brightness_up_button = button(text("+").size(ui_style::FONT_SIZE_UI_SM))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let brightness_up_button = if can_brightness_increase {
        brightness_up_button.on_press(Message::Viewer(ViewerMessage::IncreasePageBrightness))
    } else {
        brightness_up_button
    };

    let zoom_value = text(zoom_label)
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(iced::Font::MONOSPACE);
    let zoom_value = if can_reset_zoom {
        mouse_area(zoom_value).on_double_click(Message::Viewer(ViewerMessage::ResetZoom))
    } else {
        mouse_area(zoom_value)
    };
    let zoom_value = Tooltip::new(
        zoom_value,
        text("Double-click to reset zoom").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(8)
    .padding(8)
    .style(ui_style::tooltip_popup);

    let brightness_value = text(brightness_label)
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(iced::Font::MONOSPACE);
    let brightness_value = if can_reset_page_brightness {
        mouse_area(brightness_value)
            .on_double_click(Message::Viewer(ViewerMessage::ResetPageBrightness))
    } else {
        mouse_area(brightness_value)
    };
    let brightness_value = Tooltip::new(
        brightness_value,
        text("Double-click to reset brightness").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup);

    vec![
        HeaderControlGroup {
            min_width: 78.0,
            content: text(page_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(iced::Font::MONOSPACE)
                .into(),
        },
        HeaderControlGroup {
            min_width: 78.0,
            content: row![prev_button, next_button]
                .spacing(ui_style::SPACE_XS)
                .align_y(alignment::Vertical::Center)
                .into(),
        },
        HeaderControlGroup {
            min_width: 156.0,
            content: row![
                text("⌕")
                    .size(ui_style::FONT_SIZE_BODY_SM)
                    .font(iced::Font::MONOSPACE),
                zoom_out_button,
                zoom_value,
                zoom_in_button
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
        HeaderControlGroup {
            min_width: 164.0,
            content: row![
                text("◐")
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font::MONOSPACE),
                brightness_down_button,
                brightness_value,
                brightness_up_button
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
    ]
}

fn logger_controls<'a>(app: &'a LilyView) -> Vec<HeaderControlGroup<'a>> {
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

fn editor_controls<'a>(app: &'a LilyView) -> Vec<HeaderControlGroup<'a>> {
    if !app.editor.has_document() {
        return Vec::new();
    }

    let file_name = app.editor.file_name().unwrap_or("Editor");
    let file_label = if app.editor.is_dirty() {
        format!("{file_name} *")
    } else {
        file_name.to_string()
    };

    let reload_button = button(compact_control_icon(icons::refresh_cw()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let reload_button = if app.editor.is_dirty() {
        reload_button
    } else {
        reload_button.on_press(Message::Editor(super::EditorMessage::ReloadRequested))
    };

    let save_button = button(compact_control_icon(icons::save()))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    let save_button = if app.editor.is_dirty() {
        save_button.on_press(Message::Editor(super::EditorMessage::SaveRequested))
    } else {
        save_button
    };

    vec![
        HeaderControlGroup {
            min_width: 120.0,
            content: text(file_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(iced::Font::MONOSPACE)
                .into(),
        },
        HeaderControlGroup {
            min_width: 72.0,
            content: row![
                Tooltip::new(
                    reload_button,
                    text("Reload from disk").size(ui_style::FONT_SIZE_UI_XS),
                    tooltip::Position::Top,
                )
                .gap(6)
                .padding(8)
                .style(ui_style::tooltip_popup),
                Tooltip::new(
                    save_button,
                    text(format!("Save ({})", editor_save_shortcut_label()))
                        .size(ui_style::FONT_SIZE_UI_XS),
                    tooltip::Position::Top,
                )
                .gap(6)
                .padding(8)
                .style(ui_style::tooltip_popup),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
    ]
}

fn compact_control_icon(icon: svg::Handle) -> Element<'static, Message> {
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

fn editor_save_shortcut_label() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Cmd+S"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Ctrl+S"
    }
}

fn score_body(app: &LilyView) -> Element<'_, Message> {
    if app.current_score.is_none() {
        let open_button = button(
            row![
                text("🗎").size(ui_style::FONT_SIZE_UI_SM),
                text("Open file").size(ui_style::FONT_SIZE_BODY_MD),
            ]
            .spacing(ui_style::SPACE_SM)
            .align_y(alignment::Vertical::Center),
        )
        .style(ui_style::button_neutral)
        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
        .on_press(Message::File(super::FileMessage::RequestOpen));

        return container(open_button)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into();
    }

    if let Some(rendered_page) = app
        .rendered_score
        .as_ref()
        .and_then(|rendered_score| rendered_score.current_page())
    {
        let svg_handle = rendered_page.handle.clone();
        let zoom = app.svg_zoom;
        let page_brightness = app.svg_page_brightness;
        let page_size = rendered_page.size;
        let current_page = app
            .rendered_score
            .as_ref()
            .map(|rendered_score| rendered_score.current_page)
            .unwrap_or(0);
        let cursor_overlay = app
            .score_cursor_overlay
            .filter(|placement| placement.page_index == current_page);

        responsive(move |size| {
            let width = (page_size.width * SCORE_BASE_SCALE * zoom).max(1.0);
            let height = (page_size.height * SCORE_BASE_SCALE * zoom).max(1.0);

            let page_visual: Element<'_, Message> = if app.score_zoom_preview_active() {
                if let Some(preview) = app
                    .score_zoom_preview
                    .as_ref()
                    .filter(|preview| preview.page_index == current_page)
                {
                    rasterized_score_preview(preview.handle.clone(), width, height)
                } else {
                    svg(svg_handle.clone())
                        .width(Length::Fixed(width))
                        .height(Length::Fixed(height))
                        .content_fit(ContentFit::Fill)
                        .into()
                }
            } else {
                svg(svg_handle.clone())
                    .width(Length::Fixed(width))
                    .height(Length::Fixed(height))
                    .content_fit(ContentFit::Fill)
                    .into()
            };
            let overlay = score_cursor_overlay(cursor_overlay, page_size, width, height);
            let layered_page = stack([page_visual, overlay]);
            let page_surface = container(layered_page)
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(ui_style::PADDING_SM)
                .style(move |theme| ui_style::svg_page_surface(theme, page_brightness));

            let score_scroll = scrollable(page_surface)
                .direction(scrollable::Direction::Both {
                    vertical: scrollable::Scrollbar::new(),
                    horizontal: scrollable::Scrollbar::new(),
                })
                .on_scroll(|viewport| {
                    let offset = viewport.absolute_offset();
                    Message::Viewer(ViewerMessage::ScrollPositionChanged {
                        x: offset.x,
                        y: offset.y,
                    })
                })
                .id(super::SCORE_SCROLLABLE_ID)
                .width(Fill)
                .height(Fill)
                .style(ui_style::workspace_scrollable);

            let score_scroll_marker = score_scroll_position_marker(cursor_overlay, page_size, size);
            let zoom_overlay: Element<'_, Message> = if app.zoom_modifier_active() {
                mouse_area(container(text("")).width(Fill).height(Fill))
                    .on_scroll(|delta| Message::Viewer(ViewerMessage::SmoothZoom(delta)))
                    .into()
            } else {
                container(text("")).width(Fill).height(Fill).into()
            };

            mouse_area(
                stack([score_scroll.into(), score_scroll_marker, zoom_overlay])
                    .width(Fill)
                    .height(Fill),
            )
            .on_move(|position| Message::Viewer(ViewerMessage::ViewportCursorMoved(position)))
            .on_exit(Message::Viewer(ViewerMessage::ViewportCursorLeft))
            .into()
        })
        .width(Fill)
        .height(Fill)
        .into()
    } else {
        let message = if app.compile_session.is_some() {
            "Compiling score to SVG..."
        } else {
            "No SVG output yet"
        };

        container(text(message).size(ui_style::FONT_SIZE_BODY_MD))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill)
            .into()
    }
}

fn rasterized_score_preview(
    handle: image::Handle,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    canvas(RasterizedScorePreviewCanvas { handle })
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}

fn score_cursor_overlay(
    placement: Option<ScoreCursorPlacement>,
    page_size: super::SvgSize,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    canvas(ScoreCursorCanvas {
        placement,
        page_size,
    })
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .into()
}

fn score_scroll_position_marker(
    placement: Option<ScoreCursorPlacement>,
    page_size: super::SvgSize,
    viewport_size: Size,
) -> Element<'static, Message> {
    let Some(placement) = placement else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    let page_height = page_size.height.max(1.0);
    let normalized = ((placement.min_y + placement.max_y) * 0.5 / page_height).clamp(0.0, 1.0);

    canvas(VerticalScrollMarkerCanvas {
        normalized,
        viewport_size,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

struct VerticalScrollMarkerCanvas {
    normalized: f32,
    viewport_size: Size,
}

struct RasterizedScorePreviewCanvas {
    handle: image::Handle,
}

impl canvas::Program<Message> for RasterizedScorePreviewCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.draw_image(
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: bounds.width,
                height: bounds.height,
            },
            canvas::Image::new(self.handle.clone()),
        );
        vec![frame.into_geometry()]
    }
}

impl canvas::Program<Message> for VerticalScrollMarkerCanvas {
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
        let track_height = self.viewport_size.height.max(1.0);
        let marker_x = (bounds.width - SCROLL_MARKER_THICKNESS - SCROLL_MARKER_EDGE_INSET).max(0.0);
        let marker_center_y = self.normalized * track_height;
        let marker_y = (marker_center_y - SCROLL_MARKER_LENGTH * 0.5)
            .clamp(0.0, (track_height - SCROLL_MARKER_LENGTH).max(0.0));

        frame.fill_rectangle(
            Point::new(marker_x, marker_y),
            Size::new(
                SCROLL_MARKER_THICKNESS,
                SCROLL_MARKER_LENGTH.min(track_height),
            ),
            Color::from_rgba(
                palette.secondary.base.color.r,
                palette.secondary.base.color.g,
                palette.secondary.base.color.b,
                0.72,
            ),
        );

        vec![frame.into_geometry()]
    }
}

#[derive(Clone, Copy)]
struct ScoreCursorCanvas {
    placement: Option<ScoreCursorPlacement>,
    page_size: super::SvgSize,
}

impl canvas::Program<Message> for ScoreCursorCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        if let Some(placement) = self.placement {
            let scale_x = bounds.width / self.page_size.width.max(1.0);
            let scale_y = bounds.height / self.page_size.height.max(1.0);
            let x = placement.x * scale_x;
            let line_padding = 5.0 * scale_y.max(0.25);
            let top = ((placement.min_y * scale_y) - line_padding).max(0.0);
            let bottom = ((placement.max_y * scale_y) + line_padding).min(bounds.height);
            let height = (bottom - top).max(8.0);
            let left = x.clamp(0.0, (bounds.width - 1.0).max(0.0));
            let palette = theme.extended_palette();

            frame.stroke_rectangle(
                Point::new(left, top),
                Size::new(1.0, height),
                canvas::Stroke {
                    width: 1.4,
                    style: canvas::Style::Solid(iced::Color::from_rgba(
                        palette.primary.base.color.r,
                        palette.primary.base.color.g,
                        palette.primary.base.color.b,
                        0.92,
                    )),
                    ..canvas::Stroke::default()
                },
            );
        }

        vec![frame.into_geometry()]
    }
}
