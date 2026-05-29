use iced::{
    Color, ContentFit, Element, Fill, Length, Point, Rectangle, Size, Theme, alignment, mouse,
    widget::{
        button, canvas, container, mouse_area, responsive, row, scrollable, stack, svg, text,
        tooltip,
    },
};
use iced_core::image;

use super::{
    Lilypalooza, Message, ScoreCursorPlacement, ViewerMessage, dock_view::HeaderControlGroup,
};
use crate::{fonts, icons, ui_style};

const SCROLL_MARKER_THICKNESS: f32 = 3.0;
const SCROLL_MARKER_LENGTH: f32 = 16.0;
const SCROLL_MARKER_EDGE_INSET: f32 = 3.0;
const SCORE_BASE_SCALE: f32 = 1.125;

pub(super) fn score_base_scale() -> f32 {
    SCORE_BASE_SCALE
}

pub(super) fn score_controls<'a>(app: &'a Lilypalooza) -> Vec<HeaderControlGroup<'a>> {
    if app.current_score.is_none() {
        return Vec::new();
    }

    let state = ScoreControlsState::new(app);

    vec![
        HeaderControlGroup {
            min_width: 78.0,
            content: text(state.page_label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(fonts::MONO)
                .into(),
        },
        HeaderControlGroup {
            min_width: 78.0,
            content: row![
                score_icon_button(
                    icons::arrow_left(),
                    state.can_prev_page,
                    ViewerMessage::PrevPage
                ),
                score_icon_button(
                    icons::arrow_right(),
                    state.can_next_page,
                    ViewerMessage::NextPage
                )
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
        HeaderControlGroup {
            min_width: 134.0,
            content: row![
                score_icon_button(
                    icons::zoom_out(),
                    state.can_zoom_out,
                    ViewerMessage::ZoomOut
                ),
                score_reset_value(
                    app,
                    "score-zoom-reset",
                    state.zoom_label,
                    state.can_reset_zoom,
                    ViewerMessage::ResetZoom,
                    "Double-click to reset zoom",
                    tooltip::Position::Bottom,
                ),
                score_icon_button(icons::zoom_in(), state.can_zoom_in, ViewerMessage::ZoomIn)
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
        HeaderControlGroup {
            min_width: 138.0,
            content: row![
                score_icon_button(
                    icons::sun_dim(),
                    state.can_brightness_decrease,
                    ViewerMessage::DecreasePageBrightness,
                ),
                score_reset_value(
                    app,
                    "score-brightness-reset",
                    state.brightness_label,
                    state.can_reset_page_brightness,
                    ViewerMessage::ResetPageBrightness,
                    "Double-click to reset brightness",
                    tooltip::Position::Top,
                ),
                score_icon_button(
                    icons::sun(),
                    state.can_brightness_increase,
                    ViewerMessage::IncreasePageBrightness,
                )
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center)
            .into(),
        },
    ]
}

struct ScoreControlsState {
    page_label: String,
    zoom_label: String,
    brightness_label: String,
    can_prev_page: bool,
    can_next_page: bool,
    can_zoom_in: bool,
    can_zoom_out: bool,
    can_brightness_increase: bool,
    can_brightness_decrease: bool,
    can_reset_zoom: bool,
    can_reset_page_brightness: bool,
}

impl ScoreControlsState {
    fn new(app: &Lilypalooza) -> Self {
        Self {
            page_label: score_page_label(app),
            zoom_label: format!("{:.0}%", app.svg_zoom * 100.0),
            brightness_label: format!("{}%", app.svg_page_brightness),
            can_prev_page: app
                .rendered_score
                .as_ref()
                .is_some_and(|rendered_score| rendered_score.current_page > 0),
            can_next_page: app.rendered_score.as_ref().is_some_and(|rendered_score| {
                rendered_score.current_page.saturating_add(1) < rendered_score.pages.len()
            }),
            can_zoom_in: app.svg_zoom < super::MAX_SVG_ZOOM,
            can_zoom_out: app.svg_zoom > super::MIN_SVG_ZOOM,
            can_brightness_increase: app.svg_page_brightness < super::MAX_SVG_PAGE_BRIGHTNESS,
            can_brightness_decrease: app.svg_page_brightness > super::MIN_SVG_PAGE_BRIGHTNESS,
            can_reset_zoom: (app.svg_zoom - app.default_global_state.score_view.zoom).abs() > 1e-4,
            can_reset_page_brightness: app.svg_page_brightness
                != app.default_global_state.score_view.page_brightness,
        }
    }
}

fn score_page_label(app: &Lilypalooza) -> String {
    app.rendered_score
        .as_ref()
        .map(|rendered_score| {
            format!(
                "Page {}/{}",
                rendered_score.current_page_number(),
                rendered_score.page_count()
            )
        })
        .unwrap_or_else(|| "Page 0/0".to_string())
}

fn score_icon_button(
    icon: svg::Handle,
    enabled: bool,
    message: ViewerMessage,
) -> Element<'static, Message> {
    let button = button(super::dock_view::compact_control_icon(icon))
        .style(ui_style::button_pane_header_control)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    if enabled {
        button.on_press(Message::Viewer(message)).into()
    } else {
        button.into()
    }
}

fn score_reset_value<'a>(
    app: &'a Lilypalooza,
    id: &'static str,
    label: String,
    enabled: bool,
    message: ViewerMessage,
    tooltip_text: &'static str,
    position: tooltip::Position,
) -> Element<'a, Message> {
    let value = text(label)
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(fonts::MONO);
    let value = if enabled {
        mouse_area(value).on_double_click(Message::Viewer(message))
    } else {
        mouse_area(value)
    };
    super::dock_view::delayed_tooltip(
        app,
        id,
        value.into(),
        text(tooltip_text).size(ui_style::FONT_SIZE_UI_XS).into(),
        position,
    )
}

pub(super) fn score_body(app: &Lilypalooza) -> Element<'_, Message> {
    if app.current_score.is_none() {
        return open_score_body();
    }

    rendered_score_body(app).unwrap_or_else(|| score_placeholder_body(app))
}

fn open_score_body() -> Element<'static, Message> {
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

    centered_score_body(open_button.into())
}

fn rendered_score_body(app: &Lilypalooza) -> Option<Element<'static, Message>> {
    let rendered_score = app.rendered_score.as_ref()?;
    let rendered_page = rendered_score.current_page()?;
    let current_page = rendered_score.current_page;
    let cursor_overlay = app
        .score_cursor_overlay
        .filter(|placement| placement.page_index == current_page);
    let preview_handle = app
        .score_zoom_preview
        .as_ref()
        .filter(|preview| app.score_zoom_preview_active() && preview.page_index == current_page)
        .map(|preview| preview.handle.clone());
    let svg_handle = rendered_page.handle.clone();
    let zoom = app.svg_zoom;
    let page_brightness = app.svg_page_brightness;
    let display_size = rendered_page.display_size;
    let coord_size = rendered_page.coord_size;
    let point_and_click_available = app.score_point_and_click_target_at_cursor().is_some();
    let zoom_modifier_active = app.zoom_modifier_active();

    Some(
        responsive(move |size| {
            rendered_score_viewport(
                size,
                RenderedScoreViewport {
                    svg_handle: svg_handle.clone(),
                    preview_handle: preview_handle.clone(),
                    zoom,
                    page_brightness,
                    display_size,
                    coord_size,
                    cursor_overlay,
                    point_and_click_available,
                    zoom_modifier_active,
                },
            )
        })
        .width(Fill)
        .height(Fill)
        .into(),
    )
}

struct RenderedScoreViewport {
    svg_handle: svg::Handle,
    preview_handle: Option<image::Handle>,
    zoom: f32,
    page_brightness: u8,
    display_size: super::SvgSize,
    coord_size: super::SvgSize,
    cursor_overlay: Option<ScoreCursorPlacement>,
    point_and_click_available: bool,
    zoom_modifier_active: bool,
}

fn rendered_score_viewport(
    size: Size,
    viewport: RenderedScoreViewport,
) -> Element<'static, Message> {
    let width = (viewport.display_size.width * SCORE_BASE_SCALE * viewport.zoom).max(1.0);
    let height = (viewport.display_size.height * SCORE_BASE_SCALE * viewport.zoom).max(1.0);
    let page_visual =
        score_page_visual(viewport.svg_handle, viewport.preview_handle, width, height);
    let overlay = score_cursor_overlay(viewport.cursor_overlay, viewport.coord_size, width, height);
    let page_surface = score_page_surface(page_visual, overlay, viewport.page_brightness);
    let score_scroll = score_page_scroll(page_surface);
    let score_scroll_marker =
        score_scroll_position_marker(viewport.cursor_overlay, viewport.coord_size, size);
    let zoom_overlay = score_zoom_overlay(viewport.zoom_modifier_active);

    mouse_area(
        stack([score_scroll, score_scroll_marker, zoom_overlay])
            .width(Fill)
            .height(Fill),
    )
    .interaction(if viewport.point_and_click_available {
        mouse::Interaction::Pointer
    } else {
        mouse::Interaction::default()
    })
    .on_press(Message::Viewer(ViewerMessage::OpenPointAndClick))
    .on_move(|position| Message::Viewer(ViewerMessage::ViewportCursorMoved(position)))
    .on_exit(Message::Viewer(ViewerMessage::ViewportCursorLeft))
    .into()
}

fn score_page_visual(
    svg_handle: svg::Handle,
    preview_handle: Option<image::Handle>,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    match preview_handle {
        Some(handle) => rasterized_score_preview(handle, width, height),
        None => svg(svg_handle)
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .content_fit(ContentFit::Fill)
            .into(),
    }
}

fn score_page_surface(
    page_visual: Element<'static, Message>,
    overlay: Element<'static, Message>,
    page_brightness: u8,
) -> Element<'static, Message> {
    container(stack([page_visual, overlay]))
        .width(Length::Shrink)
        .height(Length::Shrink)
        .padding(ui_style::PADDING_SM)
        .style(move |theme| ui_style::svg_page_surface(theme, page_brightness))
        .into()
}

fn score_page_scroll(page_surface: Element<'static, Message>) -> Element<'static, Message> {
    scrollable(page_surface)
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
        .style(ui_style::workspace_scrollable)
        .into()
}

fn score_zoom_overlay(active: bool) -> Element<'static, Message> {
    if active {
        mouse_area(container(text("")).width(Fill).height(Fill))
            .on_scroll(|delta| Message::Viewer(ViewerMessage::SmoothZoom(delta)))
            .into()
    } else {
        container(text("")).width(Fill).height(Fill).into()
    }
}

fn score_placeholder_body(app: &Lilypalooza) -> Element<'_, Message> {
    if app.compile_requested || app.compile_session.is_some() || app.compile_outputs_loading {
        return centered_score_body(score_compiling_placeholder(app));
    }

    centered_score_body(
        text("No SVG output yet")
            .size(ui_style::FONT_SIZE_BODY_MD)
            .into(),
    )
}

fn score_compiling_placeholder(app: &Lilypalooza) -> Element<'_, Message> {
    row![
        text(app.spinner_frame())
            .size(ui_style::FONT_SIZE_BODY_MD)
            .font(crate::fonts::MONO),
        text("Compiling score to SVG...").size(ui_style::FONT_SIZE_BODY_MD),
    ]
    .spacing(ui_style::SPACE_SM)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn centered_score_body(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .into()
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
