use super::*;

pub(crate) fn svg_window_control(theme: &Theme, status: svg::Status) -> svg::Style {
    let palette = theme.extended_palette();

    svg::Style {
        color: Some(match status {
            svg::Status::Idle => palette.background.strong.text,
            svg::Status::Hovered => palette.background.base.text,
        }),
    }
}

pub(crate) fn svg_muted_control(theme: &Theme, status: svg::Status) -> svg::Style {
    svg_mixed_control(theme, status, 0.38, SvgControlHover::WeakText)
}

pub(crate) fn svg_dimmed_control(theme: &Theme, status: svg::Status) -> svg::Style {
    svg_mixed_control(theme, status, 0.54, SvgControlHover::BaseText)
}

#[derive(Debug, Clone, Copy)]
enum SvgControlHover {
    WeakText,
    BaseText,
}

fn svg_mixed_control(
    theme: &Theme,
    status: svg::Status,
    idle_mix: f32,
    hover: SvgControlHover,
) -> svg::Style {
    let palette = theme.extended_palette();
    let idle = mix_color(
        palette.background.weak.text,
        palette.background.weak.color,
        idle_mix,
    );
    let hovered = match hover {
        SvgControlHover::WeakText => palette.background.weak.text,
        SvgControlHover::BaseText => palette.background.base.text,
    };

    svg::Style {
        color: Some(match status {
            svg::Status::Idle => idle,
            svg::Status::Hovered => hovered,
        }),
    }
}

pub(crate) fn icon<'a, F>(handle: svg::Handle, size: f32, style: F) -> svg::Svg<'a, Theme>
where
    F: Fn(&Theme, svg::Status) -> svg::Style + 'a,
{
    svg(handle)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .content_fit(ContentFit::Contain)
        .style(style)
}

pub(crate) fn flat_icon_button<'a, Message, FB, FI>(
    handle: svg::Handle,
    button_size: f32,
    icon_size: f32,
    button_style: FB,
    icon_style: FI,
) -> button::Button<'a, Message, Theme>
where
    Message: Clone + 'a,
    FB: Fn(&Theme, button::Status) -> button::Style + 'a,
    FI: Fn(&Theme, svg::Status) -> svg::Style + 'a,
{
    button(control_icon::control_icon::<Message, iced::Renderer>(
        handle,
        button_size,
        button_size,
        icon_size,
        icon_style,
    ))
    .style(button_style)
    .padding(0)
}
