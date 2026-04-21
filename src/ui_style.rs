use iced::widget::{button, container, scrollable, svg, text_editor, text_input};
use iced::{Color, Shadow, Theme, Vector, border};
use iced_aw::style::Status as AwStatus;
use iced_aw::style::color_picker::Style as AwColorPickerStyle;

pub(crate) const FONT_SIZE_BODY_MD: u32 = 15;
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

pub(crate) const RADIUS_NONE: f32 = 0.0;
pub(crate) const RADIUS_UI: f32 = 6.0;
pub(crate) const RADIUS_PILL: f32 = 999.0;

fn top_radius(radius: f32) -> border::Radius {
    border::Radius::default()
        .top_left(radius)
        .top_right(radius)
        .bottom_left(RADIUS_NONE)
        .bottom_right(RADIUS_NONE)
}

pub(crate) fn prompt_message(theme: &Theme, critical: bool) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        text_color: Some(if critical {
            palette.danger.base.color
        } else {
            palette.background.weak.text
        }),
        ..container::Style::default()
    }
}

pub(crate) fn prompt_dialog(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(RADIUS_UI)
            .width(1)
            .color(palette.background.strong.color),
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

pub(crate) fn prompt_header(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.strong.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(top_radius(RADIUS_UI))
            .width(0)
            .color(Color::TRANSPARENT),
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
        border: border::rounded(RADIUS_UI).width(1).color(border_color),
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
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn popup_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let border_color = mix_color(
        palette.background.base.color,
        palette.background.strong.color,
        0.38,
    );

    container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_UI).width(1).color(border_color),
        ..container::Style::default()
    }
}

pub(crate) fn transparent_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: None,
        text_color: None,
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn pane_logger_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(RADIUS_NONE)
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
        border: border::rounded(RADIUS_NONE)
            .width(1)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

pub(crate) fn mixer_track_strip_surface(theme: &Theme, accent: Color) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(mix_color(palette.background.base.color, accent, 0.06).into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn piano_roll_track_surface(theme: &Theme, accent: Color) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(mix_color(palette.background.weakest.color, accent, 0.10).into()),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(RADIUS_UI).width(1).color(mix_color(
            palette.background.strong.color,
            accent,
            0.18,
        )),
        ..container::Style::default()
    }
}

pub(crate) fn track_color_swatch_button(
    theme: &Theme,
    status: button::Status,
    color: Color,
) -> button::Style {
    let palette = theme.extended_palette();

    let background = match status {
        button::Status::Pressed => mix_color(color, palette.primary.base.color, 0.18),
        button::Status::Hovered => mix_color(color, palette.background.weakest.color, 0.08),
        button::Status::Active | button::Status::Disabled => color,
    };

    button::Style {
        background: Some(background.into()),
        text_color: palette.background.base.text,
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..button::Style::default()
    }
}

pub(crate) fn track_name_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let palette = theme.extended_palette();
    let background = palette.background.weak.color;
    let strong_border = mix_color(
        palette.background.base.color,
        palette.background.strong.color,
        0.72,
    );
    let border_color = match status {
        text_input::Status::Focused { .. } => palette.primary.base.color,
        text_input::Status::Hovered => mix_color(strong_border, palette.primary.base.color, 0.18),
        text_input::Status::Active | text_input::Status::Disabled => strong_border,
    };

    text_input::Style {
        background: background.into(),
        border: border::rounded(RADIUS_NONE).width(0).color(border_color),
        icon: palette.background.weak.text,
        placeholder: palette.background.strong.text,
        value: palette.background.weak.text,
        selection: palette.primary.weak.color,
    }
}

pub(crate) fn track_name_editor_shell(theme: &Theme, focused: bool) -> container::Style {
    let palette = theme.extended_palette();
    let strong_border = mix_color(
        palette.background.base.color,
        palette.background.strong.color,
        0.72,
    );
    let border_color = if focused {
        palette.primary.base.color
    } else {
        strong_border
    };

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(RADIUS_NONE).width(1).color(border_color),
        ..container::Style::default()
    }
}

pub(crate) fn track_name_editor_divider(theme: &Theme, focused: bool) -> container::Style {
    let palette = theme.extended_palette();
    let strong_border = mix_color(
        palette.background.base.color,
        palette.background.strong.color,
        0.72,
    );
    let divider_color = if focused {
        palette.primary.base.color
    } else {
        strong_border
    };

    container::Style {
        background: Some(divider_color.into()),
        ..container::Style::default()
    }
}

pub(crate) fn color_picker_widget_style(theme: &Theme, status: AwStatus) -> AwColorPickerStyle {
    let palette = theme.extended_palette();
    let popup_border = mix_color(
        palette.background.base.color,
        palette.background.strong.color,
        0.38,
    );
    let border_color = match status {
        AwStatus::Focused => palette.primary.base.color,
        _ => popup_border,
    };

    AwColorPickerStyle {
        background: palette.background.weak.color.into(),
        border_radius: RADIUS_UI,
        border_width: 1.0,
        border_color,
        bar_border_radius: RADIUS_NONE,
        bar_border_width: 1.0,
        bar_border_color: border_color,
    }
}

pub(crate) fn mixer_instrument_group_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(
            mix_color(
                palette.background.base.color,
                palette.background.weak.color,
                0.18,
            )
            .into(),
        ),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn mixer_side_group_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(
            mix_color(
                palette.background.weakest.color,
                palette.background.weak.color,
                0.12,
            )
            .into(),
        ),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn pane_title_bar_surface_focused(theme: &Theme, focused: bool) -> container::Style {
    let palette = theme.extended_palette();
    let background = if focused {
        mix_color(palette.background.weak.color, Color::WHITE, 0.04).into()
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
        border: border::rounded(RADIUS_NONE).width(1).color(mix_color(
            palette.background.weak.color,
            palette.background.strong.color,
            0.40,
        )),
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
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn transport_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
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

pub(crate) fn editor_tabbar_scrollable(
    theme: &Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    let palette = theme.extended_palette();
    let (show_horizontal, track_alpha, thumb_alpha) = match status {
        scrollable::Status::Active {
            is_horizontal_scrollbar_disabled,
            ..
        } => (!is_horizontal_scrollbar_disabled, 0.0, 0.0),
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_disabled,
            ..
        } => (!is_horizontal_scrollbar_disabled, 0.025, 0.08),
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_disabled,
            ..
        } => (!is_horizontal_scrollbar_disabled, 0.04, 0.12),
    };

    let mut style = scrollable::default(theme, status);
    style.container.background = Some(palette.background.weak.color.into());
    style.container.text_color = Some(palette.background.weak.text);
    style.vertical_rail.background = Some(
        Color {
            a: 0.04,
            ..palette.background.base.color
        }
        .into(),
    );
    style.vertical_rail.scroller.background = Color {
        a: 0.14,
        ..palette.background.base.text
    }
    .into();
    style.horizontal_rail.background = Some(
        Color {
            a: if show_horizontal { track_alpha } else { 0.0 },
            ..palette.background.base.color
        }
        .into(),
    );
    style.horizontal_rail.scroller.background = Color {
        a: if show_horizontal { thumb_alpha } else { 0.0 },
        ..palette.background.base.text
    }
    .into();

    style
}

pub(crate) fn svg_page_surface(theme: &Theme, brightness_percent: u8) -> container::Style {
    let palette = theme.extended_palette();
    let alpha = (brightness_percent as f32 / 100.0).clamp(0.0, 1.0);

    container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, alpha).into()),
        text_color: Some(Color::from_rgba(0.08, 0.08, 0.08, alpha.max(0.35))),
        border: border::rounded(RADIUS_NONE)
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
    style.border = border::rounded(RADIUS_NONE).width(0);
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
        border: border::rounded(RADIUS_NONE)
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
        border: border::rounded(RADIUS_NONE).width(0),
        ..container::Style::default()
    }
}

pub(crate) fn button_neutral(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.weak.text,
        border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
            .width(1)
            .color(palette.background.base.color),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.weak.text,
            border: border::rounded(RADIUS_UI)
                .width(1)
                .color(palette.primary.base.color),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.primary.base.color.into()),
            text_color: palette.primary.base.text,
            border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
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
            border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
            .width(0)
            .color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(RADIUS_UI)
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
            border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_UI)
            .width(0)
            .color(Color::TRANSPARENT),
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

pub(crate) fn button_shortcut_palette_item(
    theme: &Theme,
    status: button::Status,
    selected: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let text_color = palette.background.weak.text;
    let selected_background = mix_color(
        palette.background.strong.color,
        palette.primary.base.color,
        0.08,
    );

    let base = button::Style {
        background: Some(
            if selected {
                selected_background
            } else {
                Color::TRANSPARENT
            }
            .into(),
        ),
        text_color,
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(
                if selected {
                    selected_background
                } else {
                    palette.background.strong.color
                }
                .into(),
            ),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(
                mix_color(
                    if selected {
                        selected_background
                    } else {
                        palette.background.strong.color
                    },
                    palette.primary.base.color,
                    0.10,
                )
                .into(),
            ),
            ..base
        },
        button::Status::Disabled => base,
    }
}

pub(crate) fn editor_file_browser_column(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn editor_file_browser_scrollable(
    theme: &Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    let palette = theme.extended_palette();
    let mut style = scrollable::default(theme, status);
    style.container.background = Some(palette.background.base.color.into());
    style.container.text_color = Some(palette.background.base.text);
    style.vertical_rail.background = Some(
        Color {
            a: 0.04,
            ..palette.background.base.color
        }
        .into(),
    );
    style.vertical_rail.scroller.background = Color {
        a: 0.14,
        ..palette.background.base.text
    }
    .into();
    style.horizontal_rail.background = Some(
        Color {
            a: 0.04,
            ..palette.background.base.color
        }
        .into(),
    );
    style.horizontal_rail.scroller.background = Color {
        a: 0.14,
        ..palette.background.base.text
    }
    .into();

    style
}

pub(crate) fn editor_file_browser_entry(theme: &Theme, selected: bool) -> container::Style {
    let palette = theme.extended_palette();
    let selected_background = mix_color(
        palette.background.strong.color,
        palette.primary.base.color,
        0.10,
    );

    container::Style {
        background: Some(
            if selected {
                selected_background
            } else {
                Color::TRANSPARENT
            }
            .into(),
        ),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn shortcut_action_id_label(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.strong.color.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_PILL)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn editor_tab_surface(
    theme: &Theme,
    active: bool,
    hovered: bool,
    dragged: bool,
) -> container::Style {
    let palette = theme.extended_palette();
    let focused_header_background = mix_color(
        palette.background.weak.color,
        palette.primary.base.color,
        0.12,
    );

    let (background, text_color) = if dragged {
        (
            Color {
                a: 0.12,
                ..palette.background.base.color
            },
            Color {
                a: 0.28,
                ..palette.background.base.text
            },
        )
    } else if active {
        (focused_header_background, palette.background.base.text)
    } else if hovered {
        (Color::TRANSPARENT, palette.background.base.text)
    } else {
        (Color::TRANSPARENT, palette.background.strong.text)
    };

    container::Style {
        background: Some(background.into()),
        text_color: Some(text_color),
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub(crate) fn button_editor_tab_close(
    theme: &Theme,
    status: button::Status,
    active: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let base_text = if active {
        palette.background.base.text
    } else {
        palette.background.strong.text
    };

    let base = button::Style {
        background: None,
        text_color: base_text,
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: None,
            text_color: palette.primary.weak.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: None,
            text_color: palette.primary.base.text,
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: palette.background.weak.text,
            ..base
        },
    }
}

pub(crate) fn button_toolbar_toggle_active(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        background: Some(palette.background.base.color.into()),
        text_color: palette.background.base.text,
        border: border::rounded(RADIUS_UI)
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
            border: border::rounded(RADIUS_UI)
                .width(1)
                .color(palette.background.base.text),
            shadow: base.shadow,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.strong.text,
            border: border::rounded(RADIUS_UI)
                .width(1)
                .color(palette.background.base.color),
            shadow: Shadow::default(),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: palette.background.weak.text,
            border: border::rounded(RADIUS_UI)
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
        border: border::rounded(RADIUS_PILL)
            .width(0)
            .color(Color::TRANSPARENT),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(RADIUS_PILL)
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
            border: border::rounded(RADIUS_PILL)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_surface_has_no_background() {
        let style = transparent_surface(&Theme::Dark);
        assert!(style.background.is_none());
    }

    #[test]
    fn mixer_track_strip_surface_differs_from_plain_pane_surface() {
        let plain = pane_main_surface(&Theme::Dark);
        let tinted = mixer_track_strip_surface(&Theme::Dark, Color::from_rgb(0.3, 0.4, 0.5));
        assert_ne!(plain.background, tinted.background);
    }
}
