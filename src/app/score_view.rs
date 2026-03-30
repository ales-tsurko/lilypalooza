use std::collections::HashMap;

use iced::widget::{
    Tooltip, button, canvas, container, mouse_area, pane_grid, responsive, row, scrollable, stack,
    svg, text, tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Point, Rectangle, Size, Theme, alignment, border,
    mouse,
};

use super::{
    DockDropRegion, LilyView, Message, PaneMessage, ScoreCursorPlacement, ViewerMessage,
    WorkspacePaneKind, editor, piano_roll, transport_bar,
};
use crate::{icons, ui_style};

const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 16.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
const TOOLBAR_ICON_SIZE: f32 = 14.0;
const FOLD_ICON_SIZE: f32 = 12.0;
const TAB_ICON_GAP: u32 = 6;

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
    let folded = app.folded_panes().iter().copied().fold(
        row![]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        |row, folded| row.push(folded_pane_chip(folded.pane)),
    );

    container(
        row![
            text("Dock")
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(iced::Font::MONOSPACE),
            folded,
            container(text("")).width(Fill),
        ]
        .spacing(ui_style::SPACE_SM)
        .align_y(alignment::Vertical::Center)
        .width(Fill),
    )
    .padding([
        ui_style::PADDING_STATUS_BAR_V,
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .style(ui_style::workspace_toolbar_surface)
    .width(Fill)
    .into()
}

fn workspace_panes(app: &LilyView) -> Element<'_, Message> {
    responsive(move |size| {
        let panes: Element<'_, Message> =
            pane_grid::PaneGrid::new(&app.workspace_panes, |_pane, group_id, _is_maximized| {
                let body = match app
                    .workspace_group(*group_id)
                    .map(|group| group.active)
                    .unwrap_or(WorkspacePaneKind::Score)
                {
                    WorkspacePaneKind::Score => score_body(app),
                    WorkspacePaneKind::PianoRoll => piano_roll::content(app),
                    WorkspacePaneKind::Editor => editor::content(&app.editor, |action| {
                        Message::Editor(super::EditorMessage::Action(action))
                    }),
                };

                pane_grid::Content::new(body)
                    .title_bar(group_title_bar(app, *group_id))
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
) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(group_header(app, group_id))
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .style(ui_style::pane_title_bar_surface)
}

fn group_header<'a>(app: &'a LilyView, group_id: super::DockGroupId) -> Element<'a, Message> {
    let Some(group) = app.workspace_group(group_id) else {
        return container(text("")).width(Fill).into();
    };
    let active_pane = group.active;
    let can_fold = app.can_fold_workspace_pane(active_pane);

    let controls = match active_pane {
        WorkspacePaneKind::Score => score_controls(app),
        WorkspacePaneKind::PianoRoll => piano_roll::controls(app).into(),
        WorkspacePaneKind::Editor => container(text("")).into(),
    };
    let header = row![
        group_tabs(app, group),
        container(text("")).width(Fill),
        controls
    ]
    .align_y(alignment::Vertical::Center)
    .width(Fill);

    let header = if can_fold {
        header
            .push(container(text("")).width(Length::Fixed(ui_style::SPACE_MD as f32)))
            .push(
                container(text(""))
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(20.0))
                    .style(|theme: &Theme| {
                        let palette = theme.extended_palette();
                        container::Style {
                            background: Some(
                                Color::from_rgba(
                                    palette.background.strong.color.r,
                                    palette.background.strong.color.g,
                                    palette.background.strong.color.b,
                                    0.55,
                                )
                                .into(),
                            ),
                            ..container::Style::default()
                        }
                    }),
            )
            .push(fold_button(active_pane))
    } else {
        header
    };

    header.into()
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
    let is_active = app
        .group_for_pane(pane)
        .and_then(|group_id| app.workspace_group(group_id))
        .is_some_and(|group| group.active == pane);
    let is_hovered = app.hovered_workspace_pane == Some(pane);
    let is_dragging = app.dragged_workspace_pane == Some(pane);
    let title = workspace_pane_title(pane);
    let icon = match pane {
        WorkspacePaneKind::Score => icons::music_4(),
        WorkspacePaneKind::PianoRoll => icons::piano(),
        WorkspacePaneKind::Editor => icons::file_pen(),
    };
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
        } else if is_active {
            container::Style {
                background: Some(Color::TRANSPARENT.into()),
                text_color: Some(icon_color(theme)),
                border: border::rounded(10)
                    .width(1)
                    .color(palette.background.strong.color),
                ..container::Style::default()
            }
        } else if is_hovered {
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
    }
}

fn fold_button(pane: WorkspacePaneKind) -> Element<'static, Message> {
    let button = button(
        svg(icons::arrow_up_to_line())
            .width(Length::Fixed(FOLD_ICON_SIZE))
            .height(Length::Fixed(FOLD_ICON_SIZE))
            .content_fit(ContentFit::Contain)
            .style(ui_style::svg_window_control),
    )
    .style(ui_style::button_window_control)
    .padding([4, 7])
    .width(Length::Fixed(26.0))
    .height(Length::Fixed(22.0))
    .on_press(Message::Pane(PaneMessage::FoldWorkspacePane(pane)));

    Tooltip::new(
        container(button).padding([0, 2]),
        text("Fold pane into toolbar").size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
}

fn folded_pane_chip(pane: WorkspacePaneKind) -> Element<'static, Message> {
    let icon = match pane {
        WorkspacePaneKind::Score => icons::music_4(),
        WorkspacePaneKind::PianoRoll => icons::piano(),
        WorkspacePaneKind::Editor => icons::file_pen(),
    };

    Tooltip::new(
        button(
            svg(icon)
                .width(Length::Fixed(TOOLBAR_ICON_SIZE))
                .height(Length::Fixed(TOOLBAR_ICON_SIZE))
                .content_fit(ContentFit::Contain)
                .style(ui_style::svg_toolbar_chip),
        )
        .style(ui_style::button_toolbar_chip)
        .padding([5, 8])
        .on_press(Message::Pane(PaneMessage::UnfoldWorkspacePane(pane))),
        text(workspace_pane_title(pane)).size(ui_style::FONT_SIZE_UI_XS),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(8)
    .style(ui_style::tooltip_popup)
    .into()
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

pub(super) fn score_controls<'a>(app: &'a LilyView) -> Element<'a, Message> {
    if app.current_score.is_none() {
        return container(text("")).into();
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

    let prev_button = button(text("←").size(ui_style::FONT_SIZE_UI_SM))
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

    let next_button = button(text("→").size(ui_style::FONT_SIZE_UI_SM))
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

    row![
        text(page_label)
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(iced::Font::MONOSPACE),
        row![prev_button, next_button]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        row![
            text("⌕")
                .size(ui_style::FONT_SIZE_BODY_SM)
                .font(iced::Font::MONOSPACE),
            zoom_out_button,
            zoom_value,
            zoom_in_button
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
        row![
            text("◐")
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(iced::Font::MONOSPACE),
            brightness_down_button,
            brightness_value,
            brightness_up_button
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    ]
    .spacing(ui_style::SPACE_SM)
    .align_y(alignment::Vertical::Center)
    .into()
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
        let aspect_ratio = (rendered_page.size.height / rendered_page.size.width).clamp(0.25, 8.0);
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
            let page_horizontal_gutter = f32::from(ui_style::PADDING_SM) * 2.0 + 2.0;
            let fit_width = (size.width - page_horizontal_gutter).max(1.0);
            let width = (fit_width * zoom).max(1.0);
            let height = (width * aspect_ratio).max(1.0);

            let page_svg = svg(svg_handle.clone())
                .width(Length::Fixed(width))
                .height(Length::Fixed(height))
                .content_fit(ContentFit::Fill);
            let overlay = score_cursor_overlay(cursor_overlay, page_size, width, height);
            let layered_page = stack([page_svg.into(), overlay]);
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
