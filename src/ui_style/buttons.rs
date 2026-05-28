use super::*;

#[derive(Clone, Copy, Default)]
enum ButtonBackground {
    #[default]
    Keep,
    Clear,
    Color(Color),
}

#[derive(Clone, Copy, Default)]
struct ButtonStylePatch {
    background: ButtonBackground,
    text_color: Option<Color>,
    border: Option<Border>,
    shadow: Option<Shadow>,
}

fn apply_button_style_patch(mut style: button::Style, patch: ButtonStylePatch) -> button::Style {
    match patch.background {
        ButtonBackground::Keep => {}
        ButtonBackground::Clear => style.background = None,
        ButtonBackground::Color(color) => style.background = Some(color.into()),
    }
    if let Some(text_color) = patch.text_color {
        style.text_color = text_color;
    }
    if let Some(border) = patch.border {
        style.border = border;
    }
    if let Some(shadow) = patch.shadow {
        style.shadow = shadow;
    }
    style
}

fn button_style_for_status(
    status: button::Status,
    base: button::Style,
    hovered: ButtonStylePatch,
    pressed: ButtonStylePatch,
    disabled: ButtonStylePatch,
) -> button::Style {
    let patch = match status {
        button::Status::Active => return base,
        button::Status::Hovered => hovered,
        button::Status::Pressed => pressed,
        button::Status::Disabled => disabled,
    };
    apply_button_style_patch(base, patch)
}

fn button_shadow(offset_y: f32, blur_radius: f32, alpha: f32) -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, alpha),
        offset: Vector::new(0.0, offset_y),
        blur_radius,
    }
}

#[derive(Clone, Copy)]
enum ButtonStyleKind {
    PaneTab { active: bool },
    CompactSolid,
    CompactActive,
    ToolbarChip,
    EditorTabClose { active: bool },
    ToolbarToggleActive,
    FlatCompactControl,
    PaneHeaderControlActive,
}

fn themed_button_style(
    theme: &Theme,
    status: button::Status,
    kind: ButtonStyleKind,
) -> button::Style {
    let palette = theme.extended_palette();

    match kind {
        ButtonStyleKind::PaneTab { active } => button_style_for_status(
            status,
            button::Style {
                background: None,
                text_color: if active {
                    palette.background.base.text
                } else {
                    palette.background.strong.text
                },
                border: border::rounded(RADIUS_UI)
                    .width(if active { 1 } else { 0 })
                    .color(if active {
                        palette.background.strong.color
                    } else {
                        Color::TRANSPARENT
                    }),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.strong.color),
                text_color: Some(palette.background.base.text),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.base.color),
                text_color: Some(palette.background.base.text),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                text_color: Some(palette.background.weak.text),
                ..ButtonStylePatch::default()
            },
        ),
        ButtonStyleKind::CompactSolid => button_style_for_status(
            status,
            button::Style {
                background: Some(palette.background.strong.color.into()),
                text_color: palette.background.strong.text,
                border: border::rounded(RADIUS_TIGHT)
                    .width(1)
                    .color(palette.background.base.color),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.weak.color),
                text_color: Some(palette.primary.weak.text),
                border: Some(
                    border::rounded(RADIUS_TIGHT)
                        .width(1)
                        .color(palette.primary.base.color),
                ),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.base.color),
                text_color: Some(palette.primary.base.text),
                border: Some(
                    border::rounded(RADIUS_TIGHT)
                        .width(1)
                        .color(palette.primary.strong.color),
                ),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.strong.color),
                text_color: Some(palette.background.weak.text),
                ..ButtonStylePatch::default()
            },
        ),
        ButtonStyleKind::CompactActive => button_style_for_status(
            status,
            button::Style {
                background: Some(palette.primary.base.color.into()),
                text_color: palette.primary.base.text,
                border: border::rounded(RADIUS_TIGHT)
                    .width(1)
                    .color(palette.primary.strong.color),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.strong.color),
                text_color: Some(palette.primary.strong.text),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.weak.color),
                text_color: Some(palette.primary.weak.text),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.strong.color),
                text_color: Some(palette.background.weak.text),
                border: Some(
                    border::rounded(RADIUS_TIGHT)
                        .width(1)
                        .color(palette.background.base.color),
                ),
                ..ButtonStylePatch::default()
            },
        ),
        ButtonStyleKind::ToolbarChip => button_style_for_status(
            status,
            button::Style {
                background: None,
                text_color: palette.background.base.text,
                border: border::rounded(RADIUS_UI)
                    .width(0)
                    .color(Color::TRANSPARENT),
                shadow: Shadow::default(),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.base.color),
                text_color: Some(palette.background.base.text),
                border: Some(
                    border::rounded(RADIUS_UI)
                        .width(1)
                        .color(palette.background.strong.color),
                ),
                shadow: Some(button_shadow(2.0, 6.0, 0.12)),
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.base.color),
                text_color: Some(palette.primary.base.text),
                border: Some(
                    border::rounded(RADIUS_UI)
                        .width(1)
                        .color(palette.primary.strong.color),
                ),
                shadow: Some(Shadow::default()),
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.weak.color),
                text_color: Some(palette.background.weak.text),
                shadow: Some(Shadow::default()),
                ..ButtonStylePatch::default()
            },
        ),
        ButtonStyleKind::EditorTabClose { active } => {
            let base_text = if active {
                palette.background.base.text
            } else {
                palette.background.strong.text
            };

            button::Style {
                background: None,
                text_color: editor_tab_close_text_color(
                    status,
                    base_text,
                    palette.primary.weak.text,
                    palette.primary.base.text,
                    palette.background.weak.text,
                ),
                border: border::rounded(RADIUS_NONE)
                    .width(0)
                    .color(Color::TRANSPARENT),
                shadow: Shadow::default(),
                ..button::Style::default()
            }
        }
        ButtonStyleKind::ToolbarToggleActive => {
            let active_shadow = button_shadow(2.0, 6.0, 0.12);
            button_style_for_status(
                status,
                button::Style {
                    background: Some(palette.background.base.color.into()),
                    text_color: palette.background.base.text,
                    border: border::rounded(RADIUS_UI)
                        .width(1)
                        .color(palette.background.strong.color),
                    shadow: active_shadow,
                    ..button::Style::default()
                },
                ButtonStylePatch {
                    background: ButtonBackground::Color(palette.background.base.color),
                    text_color: Some(palette.background.base.text),
                    border: Some(
                        border::rounded(RADIUS_UI)
                            .width(1)
                            .color(palette.background.base.text),
                    ),
                    shadow: Some(active_shadow),
                },
                ButtonStylePatch {
                    background: ButtonBackground::Color(palette.background.strong.color),
                    text_color: Some(palette.background.strong.text),
                    border: Some(
                        border::rounded(RADIUS_UI)
                            .width(1)
                            .color(palette.background.base.color),
                    ),
                    shadow: Some(Shadow::default()),
                },
                ButtonStylePatch {
                    background: ButtonBackground::Color(palette.background.weak.color),
                    text_color: Some(palette.background.weak.text),
                    border: Some(
                        border::rounded(RADIUS_UI)
                            .width(1)
                            .color(palette.background.strong.color),
                    ),
                    shadow: Some(Shadow::default()),
                },
            )
        }
        ButtonStyleKind::FlatCompactControl => button_style_for_status(
            status,
            button::Style {
                background: None,
                text_color: palette.background.strong.text,
                border: border::rounded(RADIUS_TIGHT)
                    .width(0)
                    .color(Color::TRANSPARENT),
                shadow: Shadow::default(),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.base.color),
                text_color: Some(palette.background.base.text),
                border: Some(
                    border::rounded(RADIUS_TIGHT)
                        .width(1)
                        .color(palette.background.strong.color),
                ),
                shadow: Some(Shadow::default()),
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.strong.color),
                text_color: Some(palette.background.strong.text),
                border: Some(
                    border::rounded(RADIUS_TIGHT)
                        .width(1)
                        .color(palette.background.strong.color),
                ),
                shadow: Some(Shadow::default()),
            },
            ButtonStylePatch {
                background: ButtonBackground::Clear,
                text_color: Some(palette.background.weak.text),
                shadow: Some(Shadow::default()),
                ..ButtonStylePatch::default()
            },
        ),
        ButtonStyleKind::PaneHeaderControlActive => button_style_for_status(
            status,
            button::Style {
                background: Some(palette.background.base.color.into()),
                text_color: palette.background.base.text,
                border: border::rounded(RADIUS_UI)
                    .width(1)
                    .color(palette.background.strong.color),
                shadow: button_shadow(1.0, 3.0, 0.08),
                ..button::Style::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(mix_color(
                    palette.background.base.color,
                    palette.background.strong.color,
                    0.10,
                )),
                ..ButtonStylePatch::default()
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.primary.base.color),
                text_color: Some(palette.primary.base.text),
                border: Some(
                    border::rounded(RADIUS_UI)
                        .width(1)
                        .color(palette.primary.strong.color),
                ),
                shadow: Some(Shadow::default()),
            },
            ButtonStylePatch {
                background: ButtonBackground::Color(palette.background.weak.color),
                text_color: Some(palette.background.weak.text),
                shadow: Some(Shadow::default()),
                ..ButtonStylePatch::default()
            },
        ),
    }
}

pub(crate) fn button_pane_tab(
    theme: &Theme,
    status: button::Status,
    active: bool,
) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::PaneTab { active })
}

pub(crate) fn button_browser_child_entry(
    theme: &Theme,
    status: button::Status,
    selected: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let base_background = palette.background.base.color;
    let selected_background = mix_color(base_background, Color::WHITE, 0.08);
    let base = button::Style {
        background: Some(
            if selected {
                selected_background
            } else {
                base_background
            }
            .into(),
        ),
        text_color: palette.background.base.text,
        border: border::rounded(RADIUS_NONE)
            .width(0)
            .color(Color::TRANSPARENT),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(
                mix_color(base_background, palette.background.strong.color, 0.4).into(),
            ),
            text_color: palette.background.strong.text,
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(mix_color(base_background, palette.background.weak.color, 0.2).into()),
            text_color: palette.background.base.text,
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: palette.background.weak.text,
            ..base
        },
    }
}

pub(crate) fn button_browser_section_header(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    button_browser_child_entry(theme, status, false)
}

pub(crate) fn button_compact_solid(theme: &Theme, status: button::Status) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::CompactSolid)
}

pub(crate) fn button_compact_active(theme: &Theme, status: button::Status) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::CompactActive)
}

pub(crate) fn button_toolbar_chip(theme: &Theme, status: button::Status) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::ToolbarChip)
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
    let hovered_background = shortcut_palette_hover_background(
        selected,
        selected_background,
        palette.background.strong.color,
    );

    button_style_for_status(
        status,
        button::Style {
            background: Some(
                shortcut_palette_base_background(selected, selected_background).into(),
            ),
            text_color,
            border: border::rounded(RADIUS_NONE)
                .width(0)
                .color(Color::TRANSPARENT),
            shadow: Shadow::default(),
            ..button::Style::default()
        },
        ButtonStylePatch {
            background: ButtonBackground::Color(hovered_background),
            ..ButtonStylePatch::default()
        },
        ButtonStylePatch {
            background: ButtonBackground::Color(mix_color(
                hovered_background,
                palette.primary.base.color,
                0.10,
            )),
            ..ButtonStylePatch::default()
        },
        ButtonStylePatch::default(),
    )
}

pub(super) fn shortcut_palette_base_background(
    selected: bool,
    selected_background: Color,
) -> Color {
    if selected {
        selected_background
    } else {
        Color::TRANSPARENT
    }
}

pub(super) fn shortcut_palette_hover_background(
    selected: bool,
    selected_background: Color,
    hovered_background: Color,
) -> Color {
    if selected {
        selected_background
    } else {
        hovered_background
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
    themed_button_style(theme, status, ButtonStyleKind::EditorTabClose { active })
}

pub(super) fn editor_tab_close_text_color(
    status: button::Status,
    base: Color,
    hovered: Color,
    pressed: Color,
    disabled: Color,
) -> Color {
    if status == button::Status::Hovered {
        return hovered;
    }
    if status == button::Status::Pressed {
        return pressed;
    }
    if status == button::Status::Disabled {
        return disabled;
    }
    base
}

pub(crate) fn button_toolbar_toggle_active(theme: &Theme, status: button::Status) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::ToolbarToggleActive)
}

pub(crate) fn button_window_control(theme: &Theme, status: button::Status) -> button::Style {
    inactive_control_button(theme, status, RADIUS_TIGHT)
}

pub(crate) fn button_pane_header_control(theme: &Theme, status: button::Status) -> button::Style {
    inactive_control_button(theme, status, RADIUS_UI)
}

fn inactive_control_button(theme: &Theme, status: button::Status, radius: f32) -> button::Style {
    let palette = theme.extended_palette();

    button_style_for_status(
        status,
        button::Style {
            background: None,
            text_color: palette.background.strong.text,
            border: border::rounded(radius).width(0).color(Color::TRANSPARENT),
            shadow: Shadow::default(),
            ..button::Style::default()
        },
        ButtonStylePatch {
            background: ButtonBackground::Color(palette.background.base.color),
            text_color: Some(palette.background.base.text),
            border: Some(
                border::rounded(radius)
                    .width(1)
                    .color(palette.background.strong.color),
            ),
            shadow: Some(button_shadow(1.0, 4.0, 0.10)),
        },
        ButtonStylePatch {
            background: ButtonBackground::Color(palette.primary.base.color),
            text_color: Some(palette.primary.base.text),
            border: Some(
                border::rounded(radius)
                    .width(1)
                    .color(palette.primary.strong.color),
            ),
            shadow: Some(Shadow::default()),
        },
        ButtonStylePatch {
            background: ButtonBackground::Color(palette.background.weak.color),
            text_color: Some(palette.background.weak.text),
            shadow: Some(Shadow::default()),
            ..ButtonStylePatch::default()
        },
    )
}

pub(crate) fn button_flat_compact_control(theme: &Theme, status: button::Status) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::FlatCompactControl)
}

pub(crate) fn button_pane_header_control_active(
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    themed_button_style(theme, status, ButtonStyleKind::PaneHeaderControlActive)
}
