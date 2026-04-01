use iced::widget::{button, container, scrollable, svg, text_editor};
use iced::{Color, Radians, Shadow, Theme, Vector, border, gradient};

pub(crate) const FONT_SIZE_HEADING_LG: u32 = 30;
pub(crate) const FONT_SIZE_BODY_MD: u32 = 15;
pub(crate) const FONT_SIZE_BODY_SM: u32 = 14;
pub(crate) const FONT_SIZE_UI_SM: u32 = 12;
pub(crate) const FONT_SIZE_UI_XS: u32 = 11;

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

pub(crate) fn tooltip_popup(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let background = mix_color(
        Color::from_rgb(0.74, 0.75, 0.78),
        palette.background.weakest.color,
        0.24,
    );
    let border_color = mix_color(background, palette.background.strong.color, 0.18);
    let text_color = Color::from_rgb(0.10, 0.10, 0.12);

    container::Style {
        background: Some(
            Color {
                a: 0.94,
                ..background
            }
            .into(),
        ),
        text_color: Some(text_color),
        border: border::rounded(8).width(1).color(border_color),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

pub(crate) fn pane_main_surface(theme: &Theme) -> container::Style {
    pane_main_surface_focused(theme, false)
}

pub(crate) fn pane_main_surface_focused(theme: &Theme, _focused: bool) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(0).width(0).color(Color::TRANSPARENT),
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

pub(crate) fn pane_title_bar_surface_focused(theme: &Theme, focused: bool) -> container::Style {
    let palette = theme.extended_palette();
    let background = if focused {
        let tinted = mix_color(
            palette.background.weak.color,
            palette.primary.base.color,
            0.18,
        );
        let mid_tint = mix_color(
            palette.background.weak.color,
            palette.primary.base.color,
            0.09,
        );

        gradient::Linear::new(Radians::PI / 2.0)
            .add_stop(0.0, tinted)
            .add_stop(0.42, mid_tint)
            .add_stop(0.78, palette.background.weak.color)
            .add_stop(1.0, palette.background.weak.color)
            .into()
    } else {
        palette.background.weak.color.into()
    };

    container::Style {
        background: Some(background),
        text_color: Some(if focused {
            palette.background.base.text
        } else {
            palette.background.weak.text
        }),
        border: border::rounded(0).width(0).color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

fn mix_color(a: Color, b: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);

    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub(crate) fn workspace_toolbar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(0).width(0).color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn transport_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(0).width(0).color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn chrome_separator(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(
            mix_color(
                palette.background.weak.color,
                palette.background.strong.color,
                0.40,
            )
            .into(),
        ),
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

pub(crate) fn button_compact_active(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.primary.base.color.into()),
        text_color: palette.primary.base.text,
        border: border::rounded(4)
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
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.weak.text,
            border: border::rounded(4)
                .width(1)
                .color(palette.background.base.color),
            ..base
        },
    }
}

pub(crate) fn button_toolbar_chip(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: None,
        text_color: palette.background.base.text,
        border: border::rounded(12).width(0).color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(12)
                .width(1)
                .color(palette.background.strong.color),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
                offset: Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            border: border::rounded(12)
                .width(1)
                .color(palette.primary.strong.color),
            shadow: Shadow::default(),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: palette.background.weak.text,
            shadow: Shadow::default(),
            ..base
        },
    }
}

pub(crate) fn button_menu_item(
    theme: &Theme,
    status: button::Status,
    active: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let foreground = Color::from_rgb(0.12, 0.12, 0.14);
    let foreground_muted = Color::from_rgb(0.30, 0.31, 0.34);
    let foreground_hovered = palette.background.weakest.text;

    let base_background = if active {
        Some(
            mix_color(
                palette.background.strong.color,
                palette.primary.base.color,
                0.08,
            )
            .into(),
        )
    } else {
        None
    };
    let base = button::Style {
        background: base_background,
        text_color: if active {
            foreground_hovered
        } else {
            foreground
        },
        border: border::rounded(6).width(0).color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: foreground_hovered,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(
                mix_color(
                    palette.background.strong.color,
                    palette.primary.base.color,
                    0.14,
                )
                .into(),
            ),
            text_color: foreground_hovered,
            ..base
        },
        button::Status::Disabled => button::Style {
            background: None,
            text_color: foreground_muted,
            ..base
        },
    }
}

pub(crate) fn button_toolbar_toggle_active(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.base.color.into()),
        text_color: palette.background.base.text,
        border: border::rounded(12)
            .width(1)
            .color(palette.background.strong.color),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        },
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(12)
                .width(1)
                .color(palette.background.base.text),
            shadow: base.shadow,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.strong.text,
            border: border::rounded(12)
                .width(1)
                .color(palette.background.base.color),
            shadow: Shadow::default(),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: palette.background.weak.text,
            border: border::rounded(12)
                .width(1)
                .color(palette.background.strong.color),
            shadow: Shadow::default(),
            ..base
        },
    }
}

pub(crate) fn button_window_control(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: None,
        text_color: palette.background.strong.text,
        border: border::rounded(999).width(0).color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(999)
                .width(1)
                .color(palette.background.strong.color),
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 4.0,
            },
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            border: border::rounded(999)
                .width(1)
                .color(palette.primary.strong.color),
            shadow: Shadow::default(),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: palette.background.weak.text,
            shadow: Shadow::default(),
            ..base
        },
    }
}

pub(crate) fn svg_window_control(theme: &Theme, status: svg::Status) -> svg::Style {
    let palette = theme.extended_palette();

    svg::Style {
        color: Some(match status {
            svg::Status::Idle => palette.background.strong.text,
            svg::Status::Hovered => palette.background.base.text,
        }),
    }
}
