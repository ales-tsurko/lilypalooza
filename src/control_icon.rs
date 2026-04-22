use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::svg as advanced_svg;
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell};
use iced::mouse;
use iced::widget::svg;
use iced::{ContentFit, Element, Event, Length, Point, Rectangle, Size, Theme, Vector};

pub(crate) struct ControlIcon<'a> {
    handle: svg::Handle,
    width: f32,
    height: f32,
    icon_size: f32,
    style: svg::StyleFn<'a, Theme>,
    status: Option<svg::Status>,
}

pub(crate) fn control_icon<'a, Message, Renderer>(
    handle: svg::Handle,
    width: f32,
    height: f32,
    icon_size: f32,
    style: impl Fn(&Theme, svg::Status) -> svg::Style + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Renderer: advanced_svg::Renderer + 'a,
    Message: 'a,
{
    Element::new(ControlIcon {
        handle,
        width,
        height,
        icon_size,
        style: Box::new(style),
        status: None,
    })
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for ControlIcon<'_>
where
    Renderer: advanced_svg::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(
            Length::Fixed(self.width),
            Length::Fixed(self.height),
            Size::new(self.width, self.height),
        );

        layout::Node::new(size)
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let current_status = if cursor.is_over(layout.bounds()) {
            svg::Status::Hovered
        } else {
            svg::Status::Idle
        };

        if let Event::Window(iced::window::Event::RedrawRequested(_)) = event {
            self.status = Some(current_status);
        } else if self.status.is_some_and(|status| status != current_status) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let Size { width, height } = renderer.measure_svg(&self.handle);
        let image_size = Size::new(width as f32, height as f32);
        let fit = ContentFit::Contain.fit(image_size, Size::new(self.icon_size, self.icon_size));
        let scale = Vector::new(fit.width / image_size.width, fit.height / image_size.height);
        let final_size = image_size * scale;
        let position = Point::new(
            bounds.center_x() - final_size.width / 2.0,
            bounds.center_y() - final_size.height / 2.0,
        );
        let drawing_bounds = Rectangle::new(position, final_size);
        let style =
            svg::Catalog::style(theme, &self.style, self.status.unwrap_or(svg::Status::Idle));

        renderer.draw_svg(
            advanced_svg::Svg {
                handle: self.handle.clone(),
                color: style.color,
                rotation: 0.0.into(),
                opacity: 1.0,
            },
            drawing_bounds,
            bounds,
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
