use iced::widget::{
    Tooltip, button, canvas, container, mouse_area, responsive, row, scrollable, stack, svg, text,
    tooltip,
};
use iced::{
    Color, ContentFit, Element, Fill, Length, Point, Rectangle, Size, Theme, alignment, mouse,
};
use iced_core::image;

use super::{
    LilyView, Message, ScoreCursorPlacement, ViewerMessage, dock_view::HeaderControlGroup,
};
use crate::{icons, ui_style};

const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 16.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
const SCORE_BASE_SCALE: f32 = 5.6;

pub(super) fn score_base_scale() -> f32 {
    SCORE_BASE_SCALE
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

    let prev_button = button(super::dock_view::compact_control_icon(icons::arrow_left()))
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

    let next_button = button(super::dock_view::compact_control_icon(icons::arrow_right()))
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

    let zoom_out_button = button(super::dock_view::compact_control_icon(icons::zoom_out()))
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

    let zoom_in_button = button(super::dock_view::compact_control_icon(icons::zoom_in()))
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

    let brightness_down_button = button(super::dock_view::compact_control_icon(icons::sun_dim()))
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

    let brightness_up_button = button(super::dock_view::compact_control_icon(icons::sun()))
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
            min_width: 134.0,
            content: row![zoom_out_button, zoom_value, zoom_in_button]
                .spacing(ui_style::SPACE_XS)
                .align_y(alignment::Vertical::Center)
                .into(),
        },
        HeaderControlGroup {
            min_width: 138.0,
            content: row![
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

pub(super) fn score_body(app: &LilyView) -> Element<'_, Message> {
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
