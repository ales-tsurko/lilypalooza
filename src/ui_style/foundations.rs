use super::*;

pub(crate) const GRID_PX: u16 = 4;

pub(crate) const fn grid(units: u16) -> u16 {
    units * GRID_PX
}

pub(crate) const fn grid_u32(units: u32) -> u32 {
    units * GRID_PX as u32
}

pub(crate) const fn grid_f32(units: u16) -> f32 {
    grid(units) as f32
}

pub(crate) const FONT_SIZE_BODY_MD: u32 = grid_u32(4);
pub(crate) const FONT_SIZE_UI_SM: u32 = grid_u32(3);
pub(crate) const FONT_SIZE_UI_XS: u32 = grid_u32(2) * 3 / 2;

pub(crate) const SIZE_SURFACE_LG: u32 = grid_u32(156);

pub(crate) const SPACE_SM: u32 = grid_u32(3);
pub(crate) const SPACE_MD: u32 = grid_u32(4);
pub(crate) const SPACE_XS: u32 = grid_u32(1);

pub(crate) const PADDING_XS: u16 = grid(2);
pub(crate) const PADDING_SM: u16 = grid(4);
pub(crate) const PADDING_MD: u16 = grid(6);
pub(crate) const PADDING_BUTTON_V: u16 = grid(2);
pub(crate) const PADDING_BUTTON_H: u16 = grid(6);
pub(crate) const PADDING_BUTTON_COMPACT_V: u16 = grid(1);
pub(crate) const PADDING_BUTTON_COMPACT_H: u16 = grid(2);
pub(crate) const PADDING_STATUS_BAR_V: u16 = grid(1);
pub(crate) const PADDING_STATUS_BAR_H: u16 = grid(2);

pub(crate) const RADIUS_NONE: f32 = 0.0;
pub(crate) const RADIUS_TIGHT: f32 = grid_f32(1);
pub(crate) const RADIUS_UI: f32 = grid_f32(2);
pub(crate) const RADIUS_PILL: f32 = 999.0;

pub(super) fn top_radius(radius: f32) -> border::Radius {
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
    prompt_surface(theme, PromptSurface::Dialog)
}

pub(crate) fn prompt_header(theme: &Theme) -> container::Style {
    prompt_surface(theme, PromptSurface::Header)
}

#[derive(Debug, Clone, Copy)]
enum PromptSurface {
    Dialog,
    Header,
}

fn prompt_surface(theme: &Theme, surface: PromptSurface) -> container::Style {
    let palette = theme.extended_palette();

    match surface {
        PromptSurface::Dialog => container::Style {
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
        },
        PromptSurface::Header => container::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: Some(palette.background.weak.text),
            border: border::rounded(top_radius(RADIUS_UI))
                .width(0)
                .color(Color::TRANSPARENT),
            ..container::Style::default()
        },
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

fn flat_surface(
    background: Color,
    text: Color,
    border_width: u32,
    border_color: Color,
) -> container::Style {
    container::Style {
        background: Some(background.into()),
        text_color: Some(text),
        border: border::rounded(RADIUS_NONE)
            .width(border_width)
            .color(border_color),
        ..container::Style::default()
    }
}

pub(crate) fn pane_logger_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    flat_surface(
        palette.background.weakest.color,
        palette.background.weakest.text,
        1,
        palette.background.weak.color,
    )
}

pub(crate) fn piano_roll_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    flat_surface(
        palette.background.weakest.color,
        palette.background.weakest.text,
        1,
        palette.background.strong.color,
    )
}

pub(crate) fn mixer_track_strip_surface(
    theme: &Theme,
    accent: Option<Color>,
    selected: bool,
) -> container::Style {
    let palette = theme.extended_palette();
    let background = accent.map_or(palette.background.base.color, |accent| {
        mix_color(palette.background.base.color, accent, 0.06)
    });
    let border_color = if selected {
        mix_color(
            palette.primary.base.color,
            palette.background.weakest.color,
            0.24,
        )
    } else {
        Color::TRANSPARENT
    };

    container::Style {
        background: Some(background.into()),
        text_color: Some(palette.background.base.text),
        border: border::rounded(RADIUS_NONE)
            .width(if selected { 1 } else { 0 })
            .color(border_color),
        ..container::Style::default()
    }
}

pub(crate) fn piano_roll_track_surface(
    theme: &Theme,
    accent: Color,
    selected: bool,
) -> container::Style {
    let palette = theme.extended_palette();
    let background_mix = if selected { 0.18 } else { 0.10 };
    let border_mix = if selected { 0.34 } else { 0.18 };

    container::Style {
        background: Some(
            mix_color(palette.background.weakest.color, accent, background_mix).into(),
        ),
        text_color: Some(palette.background.weakest.text),
        border: border::rounded(RADIUS_UI).width(1).color(mix_color(
            palette.background.strong.color,
            accent,
            border_mix,
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

pub(crate) fn browser_search_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let palette = theme.extended_palette();
    let border_color = match status {
        text_input::Status::Focused { .. } => palette.primary.base.color,
        text_input::Status::Hovered => mix_color(
            palette.background.strong.color,
            palette.primary.base.color,
            0.18,
        ),
        text_input::Status::Active | text_input::Status::Disabled => {
            palette.background.strong.color
        }
    };

    text_input::Style {
        background: palette.background.weak.color.into(),
        border: border::rounded(RADIUS_UI).width(1).color(border_color),
        icon: palette.background.weak.text,
        placeholder: palette.background.strong.text,
        value: palette.background.weak.text,
        selection: palette.primary.weak.color,
    }
}

pub(crate) fn track_name_editor_shell(theme: &Theme, focused: bool) -> container::Style {
    track_name_editor_part(theme, focused, TrackNameEditorPart::Shell)
}

pub(crate) fn track_name_editor_divider(theme: &Theme, focused: bool) -> container::Style {
    track_name_editor_part(theme, focused, TrackNameEditorPart::Divider)
}

#[derive(Debug, Clone, Copy)]
enum TrackNameEditorPart {
    Shell,
    Divider,
}

fn track_name_editor_part(
    theme: &Theme,
    focused: bool,
    part: TrackNameEditorPart,
) -> container::Style {
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

    match part {
        TrackNameEditorPart::Shell => container::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: Some(palette.background.weak.text),
            border: border::rounded(RADIUS_NONE).width(1).color(divider_color),
            ..container::Style::default()
        },
        TrackNameEditorPart::Divider => container::Style {
            background: Some(divider_color.into()),
            ..container::Style::default()
        },
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
    mixer_group_surface(theme, MixerGroupSurface::Instrument)
}

pub(crate) fn mixer_side_group_surface(theme: &Theme) -> container::Style {
    mixer_group_surface(theme, MixerGroupSurface::Side)
}

#[derive(Debug, Clone, Copy)]
enum MixerGroupSurface {
    Instrument,
    Side,
}

fn mixer_group_surface(theme: &Theme, surface: MixerGroupSurface) -> container::Style {
    let palette = theme.extended_palette();
    let (base, text, amount) = match surface {
        MixerGroupSurface::Instrument => (
            palette.background.base.color,
            palette.background.base.text,
            0.18,
        ),
        MixerGroupSurface::Side => (
            palette.background.weakest.color,
            palette.background.weakest.text,
            0.12,
        ),
    };

    flat_surface(
        mix_color(base, palette.background.weak.color, amount),
        text,
        0,
        Color::TRANSPARENT,
    )
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

pub(super) fn mix_color(a: Color, b: Color, amount: f32) -> Color {
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
    flat_surface(
        palette.background.weak.color,
        palette.background.weak.text,
        0,
        Color::TRANSPARENT,
    )
}

pub(crate) fn transport_bar_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    flat_surface(
        palette.background.weak.color,
        palette.background.weak.text,
        0,
        Color::TRANSPARENT,
    )
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
    chrome_scrollable(
        theme,
        status,
        palette.background.base.color,
        palette.background.base.text,
    )
}

fn chrome_scrollable(
    theme: &Theme,
    status: scrollable::Status,
    background: Color,
    text: Color,
) -> scrollable::Style {
    let palette = theme.extended_palette();
    let mut style = scrollable::default(theme, status);
    style.container.background = Some(background.into());
    style.container.text_color = Some(text);
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
    let metrics = editor_tabbar_scroll_metrics(status);

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
            a: metrics.horizontal_alpha(metrics.track_alpha),
            ..palette.background.base.color
        }
        .into(),
    );
    style.horizontal_rail.scroller.background = Color {
        a: metrics.horizontal_alpha(metrics.thumb_alpha),
        ..palette.background.base.text
    }
    .into();

    style
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScrollbarMetrics {
    show_horizontal: bool,
    track_alpha: f32,
    thumb_alpha: f32,
}

impl ScrollbarMetrics {
    fn horizontal_alpha(self, alpha: f32) -> f32 {
        if self.show_horizontal { alpha } else { 0.0 }
    }
}

fn editor_tabbar_scroll_metrics(status: scrollable::Status) -> ScrollbarMetrics {
    match status {
        scrollable::Status::Active {
            is_horizontal_scrollbar_disabled,
            ..
        } => ScrollbarMetrics {
            show_horizontal: !is_horizontal_scrollbar_disabled,
            track_alpha: 0.0,
            thumb_alpha: 0.0,
        },
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_disabled,
            ..
        } => ScrollbarMetrics {
            show_horizontal: !is_horizontal_scrollbar_disabled,
            track_alpha: 0.025,
            thumb_alpha: 0.08,
        },
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_disabled,
            ..
        } => ScrollbarMetrics {
            show_horizontal: !is_horizontal_scrollbar_disabled,
            track_alpha: 0.04,
            thumb_alpha: 0.12,
        },
    }
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
    chrome_scrollable(
        theme,
        status,
        palette.background.weakest.color,
        palette.background.weakest.text,
    )
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
    flat_surface(
        palette.background.weakest.color,
        palette.background.weakest.text,
        1,
        palette.background.weak.color,
    )
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
    foundation_button(theme, status, FoundationButton::Neutral)
}

pub(crate) fn button_active(theme: &Theme, status: button::Status) -> button::Style {
    foundation_button(theme, status, FoundationButton::Active)
}

pub(crate) fn button_danger(theme: &Theme, status: button::Status) -> button::Style {
    foundation_button(theme, status, FoundationButton::Danger)
}

#[derive(Debug, Clone, Copy)]
enum FoundationButton {
    Neutral,
    Active,
    Danger,
}

fn foundation_button(
    theme: &Theme,
    status: button::Status,
    button_kind: FoundationButton,
) -> button::Style {
    let palette = theme.extended_palette();
    let colors = match button_kind {
        FoundationButton::Neutral => StatusButtonColors {
            base: (palette.background.weak.color, palette.background.weak.text),
            hover: (
                palette.background.strong.color,
                palette.background.strong.text,
            ),
            pressed: (palette.background.base.color, palette.background.base.text),
            border: palette.background.strong.color,
            disabled: (
                palette.background.weakest.color,
                palette.background.weakest.text,
            ),
        },
        FoundationButton::Active => StatusButtonColors {
            base: (palette.primary.base.color, palette.primary.base.text),
            hover: (palette.primary.strong.color, palette.primary.strong.text),
            pressed: (palette.primary.weak.color, palette.primary.weak.text),
            border: palette.primary.strong.color,
            disabled: (
                palette.background.weakest.color,
                palette.background.weakest.text,
            ),
        },
        FoundationButton::Danger => StatusButtonColors {
            base: (palette.danger.base.color, palette.danger.base.text),
            hover: (palette.danger.strong.color, palette.danger.strong.text),
            pressed: (palette.danger.weak.color, palette.danger.weak.text),
            border: palette.danger.strong.color,
            disabled: (
                palette.background.weakest.color,
                palette.background.weakest.text,
            ),
        },
    };
    status_button(status, colors)
}

#[derive(Debug, Clone, Copy)]
struct StatusButtonColors {
    base: (Color, Color),
    hover: (Color, Color),
    pressed: (Color, Color),
    border: Color,
    disabled: (Color, Color),
}

fn status_button(status: button::Status, colors: StatusButtonColors) -> button::Style {
    let base = button::Style {
        background: Some(colors.base.0.into()),
        text_color: colors.base.1,
        border: border::rounded(RADIUS_UI).width(1).color(colors.border),
        ..button::Style::default()
    };

    let pair = match status {
        button::Status::Active => return base,
        button::Status::Hovered => colors.hover,
        button::Status::Pressed => colors.pressed,
        button::Status::Disabled => colors.disabled,
    };
    button::Style {
        background: Some(pair.0.into()),
        text_color: pair.1,
        ..base
    }
}

pub(crate) fn button_selector_field(
    theme: &Theme,
    status: button::Status,
    open: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    let border_color = if open {
        palette.primary.base.color
    } else {
        mix_color(
            palette.background.base.color,
            palette.background.strong.color,
            0.52,
        )
    };
    let base = button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.weak.text,
        border: border::rounded(RADIUS_UI).width(1).color(border_color),
        shadow: Shadow::default(),
        ..button::Style::default()
    };

    match status {
        button::Status::Active => base,
        button::Status::Hovered => button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.strong.text,
            border: border::rounded(RADIUS_UI).width(1).color(if open {
                palette.primary.base.color
            } else {
                palette.background.base.color
            }),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: border::rounded(RADIUS_UI)
                .width(1)
                .color(palette.primary.base.color),
            ..base
        },
        button::Status::Disabled => button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.weakest.text,
            ..base
        },
    }
}
