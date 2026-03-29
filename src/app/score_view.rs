use iced::widget::{
    Tooltip, button, canvas, column, container, mouse_area, pane_grid, responsive, row, scrollable,
    stack, svg, text, tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Point, Rectangle, Size, Theme, alignment, border,
    mouse,
};

use super::{
    FileMessage, LilyView, Message, PianoRollMessage, ScoreCursorPlacement, ScorePaneKind,
    StackedDropTarget, ViewerMessage,
};
use super::{piano_roll, transport_bar};
use crate::settings::PaneAxis;
use crate::ui_style;

const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 16.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let workspace = if app.score_layout_axis == PaneAxis::Stacked {
        stacked_panes(app)
    } else {
        split_panes(app)
    };

    iced::widget::column![workspace, transport_bar::view(app)]
        .width(Fill)
        .height(Fill)
        .spacing(0)
        .into()
}

fn split_panes(app: &LilyView) -> Element<'_, Message> {
    responsive(move |size| {
        let panes: Element<'_, Message> =
            pane_grid::PaneGrid::new(&app.score_panes, |_pane, kind, _is_maximized| match kind {
                ScorePaneKind::Score => pane_grid::Content::new(score_body(app))
                    .title_bar(score_title_bar(app))
                    .style(ui_style::pane_main_surface),
                ScorePaneKind::PianoRoll => pane_grid::Content::new(piano_roll::content(app))
                    .title_bar(piano_roll::title_bar(app))
                    .style(ui_style::piano_roll_surface),
            })
            .width(Fill)
            .height(Fill)
            .style(split_rearrange_style)
            .on_drag(|event| Message::Pane(super::PaneMessage::ScoreDragged(event)))
            .on_resize(8, |event| {
                Message::PianoRoll(PianoRollMessage::Resized(event))
            })
            .into();

        let overlay = split_drag_overlay(app, size);

        mouse_area(stack([panes, overlay]).width(Fill).height(Fill))
            .on_move(|position| Message::Pane(super::PaneMessage::SplitDragMoved(position)))
            .on_exit(Message::Pane(super::PaneMessage::SplitDragExited))
            .into()
    })
    .into()
}

fn split_drag_overlay(app: &LilyView, size: Size) -> Element<'_, Message> {
    let Some(_dragging) = app.split_dragging_pane else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let Some(cursor) = app.split_drag_cursor else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    let Some((score_bounds, piano_bounds)) = split_pane_bounds(app, size) else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    let target_bounds = if score_bounds.contains(cursor) {
        split_preview_bounds(score_bounds, cursor)
    } else if piano_bounds.contains(cursor) {
        split_preview_bounds(piano_bounds, cursor)
    } else {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    };

    if target_bounds.width <= 0.0 || target_bounds.height <= 0.0 {
        return container(text("")).width(Fill).height(Fill).into();
    }

    canvas(DropOverlayCanvas { target_bounds })
        .width(Fill)
        .height(Fill)
        .into()
}

fn split_pane_bounds(app: &LilyView, size: Size) -> Option<(Rectangle, Rectangle)> {
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: size.width.max(1.0),
        height: size.height.max(1.0),
    };
    let ratio = app.piano_ratio.clamp(0.05, 0.95);

    match (app.score_split_axis, app.score_pane_order) {
        (pane_grid::Axis::Horizontal, crate::settings::PaneOrder::ScoreFirst) => Some((
            Rectangle {
                height: bounds.height * ratio,
                ..bounds
            },
            Rectangle {
                y: bounds.y + bounds.height * ratio,
                height: bounds.height * (1.0 - ratio),
                ..bounds
            },
        )),
        (pane_grid::Axis::Horizontal, crate::settings::PaneOrder::PianoFirst) => Some((
            Rectangle {
                y: bounds.y + bounds.height * ratio,
                height: bounds.height * (1.0 - ratio),
                ..bounds
            },
            Rectangle {
                height: bounds.height * ratio,
                ..bounds
            },
        )),
        (pane_grid::Axis::Vertical, crate::settings::PaneOrder::ScoreFirst) => Some((
            Rectangle {
                width: bounds.width * ratio,
                ..bounds
            },
            Rectangle {
                x: bounds.x + bounds.width * ratio,
                width: bounds.width * (1.0 - ratio),
                ..bounds
            },
        )),
        (pane_grid::Axis::Vertical, crate::settings::PaneOrder::PianoFirst) => Some((
            Rectangle {
                x: bounds.x + bounds.width * ratio,
                width: bounds.width * (1.0 - ratio),
                ..bounds
            },
            Rectangle {
                width: bounds.width * ratio,
                ..bounds
            },
        )),
    }
}

fn split_preview_bounds(bounds: Rectangle, cursor: Point) -> Rectangle {
    let left = bounds.x + bounds.width / 3.0;
    let right = bounds.x + 2.0 * bounds.width / 3.0;
    let top = bounds.y + bounds.height / 3.0;
    let bottom = bounds.y + 2.0 * bounds.height / 3.0;

    if cursor.x < left {
        Rectangle {
            width: bounds.width / 2.0,
            ..bounds
        }
    } else if cursor.x > right {
        Rectangle {
            x: bounds.x + bounds.width / 2.0,
            width: bounds.width / 2.0,
            ..bounds
        }
    } else if cursor.y < top {
        Rectangle {
            height: bounds.height / 2.0,
            ..bounds
        }
    } else if cursor.y > bottom {
        Rectangle {
            y: bounds.y + bounds.height / 2.0,
            height: bounds.height / 2.0,
            ..bounds
        }
    } else {
        bounds
    }
}

fn split_rearrange_style(theme: &Theme) -> pane_grid::Style {
    let mut style = pane_grid::default(theme);
    style.hovered_region.background = Color::TRANSPARENT.into();
    style.hovered_region.border = border::rounded(0).width(0).color(Color::TRANSPARENT);
    style
}

fn stacked_preview_bounds(size: Size, target: StackedDropTarget) -> Option<Rectangle> {
    let bounds = Rectangle {
        x: 0.0,
        y: 0.0,
        width: size.width.max(1.0),
        height: size.height.max(1.0),
    };

    Some(match target {
        StackedDropTarget::Top => Rectangle {
            height: bounds.height / 2.0,
            ..bounds
        },
        StackedDropTarget::Right => Rectangle {
            x: bounds.x + bounds.width / 2.0,
            width: bounds.width / 2.0,
            ..bounds
        },
        StackedDropTarget::Bottom => Rectangle {
            y: bounds.y + bounds.height / 2.0,
            height: bounds.height / 2.0,
            ..bounds
        },
        StackedDropTarget::Left => Rectangle {
            width: bounds.width / 2.0,
            ..bounds
        },
        StackedDropTarget::Center => bounds,
    })
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

fn stacked_panes(app: &LilyView) -> Element<'_, Message> {
    responsive(move |size| {
        let ordered_panes = match app.score_pane_order {
            crate::settings::PaneOrder::ScoreFirst => {
                [ScorePaneKind::Score, ScorePaneKind::PianoRoll]
            }
            crate::settings::PaneOrder::PianoFirst => {
                [ScorePaneKind::PianoRoll, ScorePaneKind::Score]
            }
        };

        let tab_bar = container(
            row![
                stacked_tab(app, ordered_panes[0]),
                stacked_tab(app, ordered_panes[1]),
            ]
            .spacing(0)
            .padding([0, ui_style::PADDING_STATUS_BAR_H])
            .align_y(alignment::Vertical::Bottom),
        )
        .width(Fill)
        .padding([ui_style::PADDING_STATUS_BAR_V + 2, 0])
        .style(stacked_tab_bar_surface);

        let stacked_view: Element<'_, Message> = match app.stacked_active_pane {
            ScorePaneKind::Score => column![tab_bar, score_header(app), score_body(app)]
                .width(Fill)
                .height(Fill)
                .spacing(0)
                .into(),
            ScorePaneKind::PianoRoll => {
                column![tab_bar, piano_roll::header(app), piano_roll::content(app)]
                    .width(Fill)
                    .height(Fill)
                    .spacing(0)
                    .into()
            }
        };

        let overlay = stacked_drop_overlay(app, size);

        mouse_area(stack([stacked_view, overlay]).width(Fill).height(Fill))
            .on_move(|position| Message::Pane(super::PaneMessage::StackedDragMoved(position)))
            .on_release(Message::Pane(super::PaneMessage::StackedDragReleased))
            .on_exit(Message::Pane(super::PaneMessage::StackedDragExited))
            .into()
    })
    .into()
}

fn stacked_tab(app: &LilyView, kind: ScorePaneKind) -> Element<'_, Message> {
    let is_active = app.stacked_active_pane == kind;
    let is_hovered = app.stacked_hovered_pane == Some(kind);
    let is_dragging = app.stacked_dragging_pane == Some(kind);
    let title = match kind {
        ScorePaneKind::Score => "Score",
        ScorePaneKind::PianoRoll => "Piano Roll",
    };

    let mut label = row![text(title).size(ui_style::FONT_SIZE_UI_SM)]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center);

    if kind == ScorePaneKind::PianoRoll && !app.piano_roll.visible {
        label = label.push(
            text("Folded")
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(iced::Font::MONOSPACE),
        );
    }

    let tab_body = container(label)
        .width(Length::Shrink)
        .padding([
            ui_style::PADDING_STATUS_BAR_V + 3,
            ui_style::PADDING_STATUS_BAR_H + 10,
        ])
        .style(move |theme: &Theme| {
            let palette = theme.extended_palette();
            if is_dragging {
                container::Style {
                    background: Some(palette.primary.weak.color.into()),
                    text_color: Some(palette.primary.weak.text),
                    border: border::rounded(12)
                        .width(1)
                        .color(palette.primary.base.color),
                    ..container::Style::default()
                }
            } else if is_active {
                container::Style {
                    background: Some(palette.background.base.color.into()),
                    text_color: Some(palette.background.base.text),
                    border: border::rounded(9)
                        .width(1)
                        .color(palette.background.strong.color),
                    ..container::Style::default()
                }
            } else if is_hovered {
                container::Style {
                    background: Some(palette.background.weakest.color.into()),
                    text_color: Some(palette.background.weakest.text),
                    border: border::rounded(9)
                        .width(1)
                        .color(palette.background.strong.color),
                    ..container::Style::default()
                }
            } else {
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    text_color: Some(palette.background.weak.text),
                    border: border::rounded(9)
                        .width(1)
                        .color(palette.background.strong.color),
                    ..container::Style::default()
                }
            }
        });

    mouse_area(tab_body)
        .on_press(Message::Pane(super::PaneMessage::StackedTabPressed(kind)))
        .on_enter(Message::Pane(super::PaneMessage::StackedTabHovered(Some(
            kind,
        ))))
        .on_move(move |_| Message::Pane(super::PaneMessage::StackedTabDragStarted(kind)))
        .on_exit(Message::Pane(super::PaneMessage::StackedTabHovered(None)))
        .interaction(if is_dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        })
        .into()
}

fn stacked_tab_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.strong.color.into()),
        text_color: Some(palette.background.strong.text),
        ..container::Style::default()
    }
}

fn stacked_drop_overlay(app: &LilyView, size: Size) -> Element<'_, Message> {
    let Some(target) = app.stacked_drop_target else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let Some(bounds) = stacked_preview_bounds(size, target) else {
        return container(text("")).width(Fill).height(Fill).into();
    };

    canvas(DropOverlayCanvas {
        target_bounds: bounds,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

fn score_title_bar<'a>(app: &'a LilyView) -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(score_header_content(app))
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .style(ui_style::pane_title_bar_surface)
}

fn score_header<'a>(app: &'a LilyView) -> Element<'a, Message> {
    container(score_header_content(app))
        .width(Fill)
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .style(ui_style::pane_title_bar_surface)
        .into()
}

fn score_header_content<'a>(app: &'a LilyView) -> row::Row<'a, Message> {
    let mut header = row![text("Score").size(ui_style::FONT_SIZE_UI_SM)]
        .spacing(ui_style::SPACE_SM)
        .align_y(alignment::Vertical::Center);

    if app.current_score.is_none() {
        return header;
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
    .gap(8);

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
    .gap(6);

    header = header.push(
        text(page_label)
            .size(ui_style::FONT_SIZE_UI_XS)
            .font(iced::Font::MONOSPACE),
    );
    header = header.push(
        row![prev_button, next_button]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
    );
    header = header.push(
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
    );
    header.push(
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
    )
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
        .on_press(Message::File(FileMessage::RequestOpen));

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
