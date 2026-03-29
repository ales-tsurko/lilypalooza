use iced::widget::{button, container, scrollable, text_editor};
use iced::{Color, Shadow, Theme, Vector, border};

pub(crate) const FONT_SIZE_HEADING_LG: u32 = 30;
pub(crate) const FONT_SIZE_BODY_MD: u32 = 16;
pub(crate) const FONT_SIZE_BODY_SM: u32 = 15;
pub(crate) const FONT_SIZE_UI_SM: u32 = 13;
pub(crate) const FONT_SIZE_UI_XS: u32 = 12;

pub(crate) const SIZE_SURFACE_LG: u32 = 620;

pub(crate) const SPACE_SM: u32 = 10;
pub(crate) const SPACE_MD: u32 = 16;
pub(crate) const SPACE_XS: u32 = 4;

pub(crate) const PADDING_XS: u16 = 6;
pub(crate) const PADDING_SM: u16 = 14;
pub(crate) const PADDING_MD: u16 = 24;
pub(crate) const PADDING_BUTTON_V: u16 = 10;
pub(crate) const PADDING_BUTTON_H: u16 = 24;
pub(crate) const PADDING_BUTTON_COMPACT_V: u16 = 2;
pub(crate) const PADDING_BUTTON_COMPACT_H: u16 = 10;
pub(crate) const PADDING_STATUS_BAR_V: u16 = 4;
pub(crate) const PADDING_STATUS_BAR_H: u16 = 8;

pub(crate) fn prompt_message(theme: &Theme, critical: bool) -> container::Style {
    let palette = theme.extended_palette();

    if critical {
        container::Style {
            background: Some(palette.danger.weak.color.into()),
            text_color: Some(palette.danger.weak.text),
            border: border::rounded(10)
                .width(1)
                .color(palette.danger.base.color),
            ..container::Style::default()
        }
    } else {
        container::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: Some(palette.background.weakest.text),
            border: border::rounded(10)
                .width(1)
                .color(palette.background.weak.color),
            ..container::Style::default()
        }
    }
}

pub(crate) fn prompt_dialog(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(14)
            .width(1)
            .color(palette.background.strong.color),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
            offset: Vector::new(0.0, 12.0),
            blur_radius: 28.0,
        },
        ..container::Style::default()
    }
}

pub(crate) fn prompt_backdrop(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.55))
}

pub(crate) fn pane_main_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        ..container::Style::default()
    }
}

pub(crate) fn pane_logger_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(0)
            .width(1)
            .color(palette.background.weak.color),
        ..container::Style::default()
    }
}

pub(crate) fn piano_roll_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(0)
            .width(1)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

pub(crate) fn pane_title_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(0)
            .width(1)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

pub(crate) fn workspace_toolbar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(0)
            .width(1)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

pub(crate) fn workspace_scrollable(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let palette = theme.extended_palette();

    let mut style = scrollable::default(theme, status);
    style.container.background = Some(palette.background.base.color.into());
    style.container.text_color = Some(palette.background.base.text);
    style.vertical_rail.background = Some(palette.background.weak.color.into());
    style.vertical_rail.scroller.background = palette.background.strong.color.into();
    style.horizontal_rail.background = Some(palette.background.weak.color.into());
    style.horizontal_rail.scroller.background = palette.background.strong.color.into();

    style
}

pub(crate) fn svg_page_surface(theme: &Theme, brightness_percent: u8) -> container::Style {
    let palette = theme.extended_palette();
    let alpha = (brightness_percent as f32 / 100.0).clamp(0.0, 1.0);

    container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, alpha).into()),
        text_color: Some(Color::from_rgba(0.08, 0.08, 0.08, alpha.max(0.35))),
        border: border::rounded(0)
            .width(if alpha > 0.0 { 1 } else { 0 })
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

pub(crate) fn logger_scrollable(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let palette = theme.extended_palette();

    let mut style = scrollable::default(theme, status);

    style.container.background = Some(palette.background.weakest.color.into());
    style.container.text_color = Some(palette.background.weakest.text);
    style.vertical_rail.background = Some(palette.background.weak.color.into());
    style.vertical_rail.scroller.background = palette.background.strong.color.into();
    style.horizontal_rail.background = Some(palette.background.weak.color.into());
    style.horizontal_rail.scroller.background = palette.background.strong.color.into();

    style
}

pub(crate) fn logger_text_editor(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let palette = theme.extended_palette();

    let mut style = text_editor::default(theme, status);
    style.background = palette.background.weakest.color.into();
    style.border = border::rounded(0).width(0);
    style.placeholder = palette.background.strong.color;
    style.value = palette.background.weakest.text;
    style.selection = palette.primary.weak.color;

    style
}

pub(crate) fn status_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(0)
            .width(1)
            .color(palette.background.weak.color),
        ..container::Style::default()
    }
}

pub(crate) fn status_block_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(0).width(0),
        ..container::Style::default()
    }
}

pub(crate) fn status_block_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: palette.background.weakest.text,
        border: border::rounded(6)
            .width(1)
            .color(palette.background.strong.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.weak.text,
            border: border::rounded(6)
                .width(1)
                .color(palette.primary.base.color),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            border: border::rounded(6)
                .width(1)
                .color(palette.primary.strong.color),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.strong.text,
            ..base
        },
    }
}

pub(crate) fn button_neutral(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.weak.text,
        border: border::rounded(10)
            .width(1)
            .color(palette.background.strong.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.strong.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.weakest.text,
            ..base
        },
    }
}

pub(crate) fn button_active(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.primary.base.color.into()),
        text_color: palette.primary.base.text,
        border: border::rounded(10)
            .width(1)
            .color(palette.primary.strong.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.primary.strong.color.into()),
            text_color: palette.primary.strong.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.weak.text,
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.weakest.text,
            ..base
        },
    }
}

pub(crate) fn button_danger(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.danger.base.color.into()),
        text_color: palette.danger.base.text,
        border: border::rounded(10)
            .width(1)
            .color(palette.danger.strong.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.danger.strong.color.into()),
            text_color: palette.danger.strong.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.danger.weak.color.into()),
            text_color: palette.danger.weak.text,
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.weakest.text,
            ..base
        },
    }
}

pub(crate) fn button_compact_solid(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.strong.color.into()),
        text_color: palette.background.strong.text,
        border: border::rounded(4)
            .width(1)
            .color(palette.background.base.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.weak.text,
            border: border::rounded(4)
                .width(1)
                .color(palette.primary.base.color),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            border: border::rounded(4)
                .width(1)
                .color(palette.primary.strong.color),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.weak.text,
            ..base
        },
    }
}
