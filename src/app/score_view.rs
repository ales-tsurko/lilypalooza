use iced::widget::{
    Tooltip, button, canvas, container, mouse_area, pane_grid, responsive, row, scrollable, stack,
    svg, text, tooltip,
};
use iced::{ContentFit, Element, Fill, Length, Point, Rectangle, Size, alignment, mouse};

use super::{
    FileMessage, LilyView, Message, PianoRollMessage, ScoreCursorPlacement, ScorePaneKind,
    ViewerMessage,
};
use super::{piano_roll, transport_bar};
use crate::ui_style;

pub(super) fn view(app: &LilyView) -> Element<'_, Message> {
    let panes =
        pane_grid::PaneGrid::new(&app.score_panes, |_pane, kind, _is_maximized| match kind {
            ScorePaneKind::Score => pane_grid::Content::new(score_content(app))
                .title_bar(score_title_bar())
                .style(ui_style::pane_main_surface),
            ScorePaneKind::PianoRoll => pane_grid::Content::new(piano_roll::content(app))
                .title_bar(piano_roll::title_bar(app))
                .style(ui_style::piano_roll_surface),
        })
        .width(Fill)
        .height(Fill)
        .on_drag(|event| Message::Pane(super::PaneMessage::ScoreDragged(event)))
        .on_resize(8, |event| {
            Message::PianoRoll(PianoRollMessage::Resized(event))
        });

    iced::widget::column![panes, transport_bar::view(app)]
        .width(Fill)
        .height(Fill)
        .spacing(0)
        .into()
}

fn score_title_bar<'a>() -> pane_grid::TitleBar<'a, Message> {
    pane_grid::TitleBar::new(text("Score").size(ui_style::FONT_SIZE_UI_SM))
        .padding([
            ui_style::PADDING_STATUS_BAR_V,
            ui_style::PADDING_STATUS_BAR_H,
        ])
        .style(ui_style::pane_title_bar_surface)
}

fn score_content(app: &LilyView) -> Element<'_, Message> {
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
        tooltip::Position::Bottom,
    )
    .gap(8);

    let toolbar = container(
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
        .width(Fill)
        .align_y(alignment::Vertical::Center)
        .spacing(ui_style::SPACE_SM),
    )
    .width(Fill)
    .padding([
        ui_style::PADDING_STATUS_BAR_V,
        ui_style::PADDING_STATUS_BAR_H,
    ])
    .style(ui_style::workspace_toolbar_surface);

    let body: Element<'_, Message> = if let Some(rendered_page) = app
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

            let zoom_overlay: Element<'_, Message> = if app.zoom_modifier_active() {
                mouse_area(container(text("")).width(Fill).height(Fill))
                    .on_scroll(|delta| Message::Viewer(ViewerMessage::SmoothZoom(delta)))
                    .into()
            } else {
                container(text("")).width(Fill).height(Fill).into()
            };

            mouse_area(
                stack([score_scroll.into(), zoom_overlay])
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
    };

    iced::widget::column![toolbar, container(body).width(Fill).height(Fill)]
        .height(Fill)
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
