use iced::Color;
use palette::{FromColor, LinSrgb, OklabHue, Oklch, Srgb};

/// The appearance of a code editor.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Main editor background color
    pub background: Color,
    /// Text content color
    pub text_color: Color,
    /// LilyPond command / escaped identifier color
    pub command_color: Color,
    /// Line numbers gutter background color
    pub gutter_background: Color,
    /// Border color for the gutter
    pub gutter_border: Color,
    /// Color for line numbers text
    pub line_number_color: Color,
    /// Color for the current line number in the gutter
    pub active_line_number_color: Color,
    /// Scrollbar background color
    pub scrollbar_background: Color,
    /// Scrollbar scroller (thumb) color
    pub scroller_color: Color,
    /// Highlight color for the current line where cursor is located
    pub current_line_highlight: Color,
    /// Comment syntax color
    pub comment_color: Color,
    /// String syntax color
    pub string_color: Color,
    /// String delimiter color
    pub string_delimiter_color: Color,
    /// Escape sequence syntax color
    pub escape_color: Color,
    /// Keyword syntax color
    pub keyword_color: Color,
    /// Directive syntax color
    pub directive_color: Color,
    /// Number syntax color
    pub number_color: Color,
    /// Function syntax color
    pub function_color: Color,
    /// Builtin function syntax color
    pub builtin_color: Color,
    /// Variable syntax color
    pub variable_color: Color,
    /// Operator syntax color
    pub operator_color: Color,
    /// Processing/directive syntax color
    pub processing_color: Color,
    /// Type/context syntax color
    pub type_color: Color,
    /// Property/assignment syntax color
    pub property_color: Color,
    /// Parameter/keyword argument syntax color
    pub parameter_color: Color,
    /// Constant/global syntax color
    pub constant_color: Color,
    /// Punctuation syntax color
    pub punctuation_color: Color,
    /// Bracket syntax color
    pub bracket_color: Color,
    /// Invalid/error syntax color
    pub invalid_color: Color,
}

/// The theme catalog of a code editor.
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a code editor.
///
/// This is a shorthand for a function that takes a reference to a
/// [`Theme`](iced::Theme) and returns a [`Style`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(from_iced_theme)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// Creates a theme style automatically from any Iced theme.
///
/// This is the default styling function that adapts to all native Iced themes including:
/// - Basic themes: Light, Dark
/// - Popular themes: Dracula, Nord, Solarized, Gruvbox
/// - Catppuccin variants: Latte, Frappé, Macchiato, Mocha
/// - Tokyo Night variants: Tokyo Night, Storm, Light
/// - Kanagawa variants: Wave, Dragon, Lotus
/// - And more: Moonfly, Nightfly, Oxocarbon, Ferra
///
/// The function automatically detects if the theme is dark or light and adjusts
/// colors accordingly for optimal contrast and readability in code editing.
///
/// # Color Mapping
///
/// - `background`: Uses the theme's base background color
/// - `text_color`: Uses the theme's base text color
/// - `gutter_background`: Slightly darker/lighter than background
/// - `gutter_border`: Border between gutter and editor
/// - `line_number_color`: Dimmed text color for subtle line numbers
/// - `scrollbar_background`: Matches editor background
/// - `scroller_color`: Uses secondary color for visibility
/// - `current_line_highlight`: Subtle highlight using primary color
///
/// # Example
///
/// ```
/// use iced_code_editor::theme;
///
/// let tokyo_night = iced::Theme::TokyoNightStorm;
/// let style = theme::from_iced_theme(&tokyo_night);
///
/// // Or use with any theme variant
/// let dracula = iced::Theme::Dracula;
/// let style = theme::from_iced_theme(&dracula);
/// ```
pub fn from_iced_theme(theme: &iced::Theme) -> Style {
    let palette = theme.extended_palette();
    let background = palette.background.base.color;
    let base_text_color = palette.background.base.text;
    let gutter_background = palette.background.weakest.color;
    let gutter_border = palette.background.strong.color;
    let bg_oklch = to_oklch(background);
    let fg_oklch = to_oklch(base_text_color);
    let primary_oklch = to_oklch(palette.primary.strong.color);
    let secondary_oklch = to_oklch(palette.secondary.strong.color);
    let success_oklch = to_oklch(palette.success.strong.color);
    let warning_oklch = to_oklch(palette.warning.strong.color);
    let danger_oklch = to_oklch(palette.danger.strong.color);
    let is_dark = palette.is_dark;

    let text_color = from_oklch(
        Oklch::new(
            (fg_oklch.l - if is_dark { 0.06 } else { 0.03 }).clamp(0.0, 1.0),
            (fg_oklch.chroma * 0.55).clamp(0.0, 0.03),
            usable_hue(fg_oklch, 260.0),
        ),
        1.0,
    );
    let active_line_number_color = blend_colors(text_color, gutter_background, 0.36);
    let line_number_color = blend_colors(text_color, gutter_background, 0.9);
    let scrollbar_background = palette.background.weak.color;
    let scroller_color = palette.background.strong.color;
    let current_line_highlight = gutter_background;

    let comment_color = derive_neutral(
        bg_oklch,
        fg_oklch,
        if is_dark { 0.42 } else { 0.34 },
        0.18,
        1.0,
    );
    let punctuation_color = derive_neutral(
        bg_oklch,
        fg_oklch,
        if is_dark { 0.60 } else { 0.5 },
        0.12,
        1.0,
    );
    let bracket_color = derive_accent(
        primary_oklch,
        fg_oklch,
        285.0,
        if is_dark { 0.72 } else { 0.46 },
        0.55,
        0.04,
        0.12,
        1.0,
    );
    let variable_color = derive_accent(
        secondary_oklch,
        fg_oklch,
        220.0,
        if is_dark { 0.84 } else { 0.28 },
        0.45,
        0.04,
        0.1,
        1.0,
    );
    let command_color = derive_accent(
        secondary_oklch,
        fg_oklch,
        240.0,
        if is_dark { 0.8 } else { 0.42 },
        1.0,
        0.08,
        0.2,
        1.0,
    );
    let keyword_color = derive_accent(
        primary_oklch,
        fg_oklch,
        285.0,
        if is_dark { 0.74 } else { 0.48 },
        1.05,
        0.1,
        0.22,
        1.0,
    );
    let directive_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(primary_oklch, 285.0),
            usable_hue(danger_oklch, 20.0),
            0.42,
        ),
        if is_dark { 0.8 } else { 0.44 },
        (((primary_oklch.chroma + danger_oklch.chroma) * 0.5) * 1.05).clamp(0.12, 0.24),
        1.0,
    );
    let operator_color = derive_accent(
        primary_oklch,
        fg_oklch,
        285.0,
        if is_dark { 0.76 } else { 0.46 },
        0.75,
        0.08,
        0.18,
        1.0,
    );
    let function_color = derive_accent(
        secondary_oklch,
        fg_oklch,
        235.0,
        if is_dark { 0.78 } else { 0.46 },
        1.0,
        0.1,
        0.22,
        1.0,
    );
    let builtin_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(success_oklch, 145.0),
            usable_hue(primary_oklch, 285.0),
            0.35,
        ),
        if is_dark { 0.8 } else { 0.42 },
        (((success_oklch.chroma + primary_oklch.chroma) * 0.5) * 1.05).clamp(0.1, 0.22),
        1.0,
    );
    let processing_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(primary_oklch, 285.0),
            usable_hue(secondary_oklch, 235.0),
            0.35,
        ),
        if is_dark { 0.82 } else { 0.48 },
        (((primary_oklch.chroma + secondary_oklch.chroma) * 0.5) * 1.1).clamp(0.11, 0.24),
        1.0,
    );
    let type_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(secondary_oklch, 235.0),
            usable_hue(warning_oklch, 85.0),
            0.28,
        ),
        if is_dark { 0.82 } else { 0.44 },
        (((secondary_oklch.chroma + warning_oklch.chroma) * 0.5) * 1.0).clamp(0.1, 0.21),
        1.0,
    );
    let property_color = derive_accent(
        warning_oklch,
        fg_oklch,
        85.0,
        if is_dark { 0.78 } else { 0.42 },
        0.95,
        0.09,
        0.2,
        1.0,
    );
    let parameter_color = derive_accent(
        success_oklch,
        fg_oklch,
        145.0,
        if is_dark { 0.76 } else { 0.4 },
        0.9,
        0.08,
        0.19,
        1.0,
    );
    let string_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(success_oklch, 145.0),
            usable_hue(warning_oklch, 85.0),
            0.3,
        ),
        if is_dark { 0.84 } else { 0.42 },
        (((success_oklch.chroma + warning_oklch.chroma) * 0.5) * 1.05).clamp(0.1, 0.22),
        1.0,
    );
    let string_delimiter_color = derive_accent(
        success_oklch,
        fg_oklch,
        145.0,
        if is_dark { 0.72 } else { 0.36 },
        0.8,
        0.06,
        0.16,
        1.0,
    );
    let escape_color = derive_accent(
        success_oklch,
        fg_oklch,
        145.0,
        if is_dark { 0.9 } else { 0.34 },
        1.1,
        0.1,
        0.22,
        1.0,
    );
    let number_color = derive_accent(
        warning_oklch,
        fg_oklch,
        85.0,
        if is_dark { 0.86 } else { 0.46 },
        1.1,
        0.11,
        0.24,
        1.0,
    );
    let constant_color = derive_accent_from_hue(
        mix_hues(
            usable_hue(warning_oklch, 85.0),
            usable_hue(secondary_oklch, 235.0),
            0.22,
        ),
        if is_dark { 0.8 } else { 0.42 },
        (((warning_oklch.chroma + secondary_oklch.chroma) * 0.5) * 1.0).clamp(0.1, 0.2),
        1.0,
    );
    let invalid_color = derive_accent(
        danger_oklch,
        fg_oklch,
        25.0,
        if is_dark { 0.74 } else { 0.52 },
        1.1,
        0.12,
        0.26,
        1.0,
    );

    Style {
        background,
        text_color,
        command_color,
        gutter_background,
        gutter_border,
        line_number_color,
        active_line_number_color,
        scrollbar_background,
        scroller_color,
        current_line_highlight,
        comment_color,
        string_color,
        string_delimiter_color,
        escape_color,
        keyword_color,
        directive_color,
        number_color,
        function_color,
        builtin_color,
        variable_color,
        operator_color,
        processing_color,
        type_color,
        property_color,
        parameter_color,
        constant_color,
        punctuation_color,
        bracket_color,
        invalid_color,
    }
}

fn to_oklch(color: Color) -> Oklch {
    let srgb = Srgb::new(color.r, color.g, color.b);
    Oklch::from_color(srgb.into_linear())
}

fn from_oklch(color: Oklch, alpha: f32) -> Color {
    let rgb: Srgb<f32> = Srgb::from_linear(LinSrgb::from_color(color));
    let (r, g, b) = rgb.into_components();

    Color {
        r: r.clamp(0.0, 1.0),
        g: g.clamp(0.0, 1.0),
        b: b.clamp(0.0, 1.0),
        a: alpha.clamp(0.0, 1.0),
    }
}

fn derive_neutral(
    background: Oklch,
    foreground: Oklch,
    lightness_mix: f32,
    chroma_scale: f32,
    alpha: f32,
) -> Color {
    let lightness = mix_scalar(background.l, foreground.l, lightness_mix);
    let chroma = (foreground.chroma * chroma_scale).clamp(0.0, 0.035);
    let hue = usable_hue(foreground, 260.0);

    from_oklch(Oklch::new(lightness.clamp(0.0, 1.0), chroma, hue), alpha)
}

#[allow(clippy::too_many_arguments)]
fn derive_accent(
    seed: Oklch,
    fallback: Oklch,
    fallback_hue_degrees: f32,
    lightness: f32,
    chroma_scale: f32,
    chroma_min: f32,
    chroma_max: f32,
    alpha: f32,
) -> Color {
    let hue = if seed.chroma > 0.02 {
        seed.hue
    } else {
        usable_hue(fallback, fallback_hue_degrees)
    };
    let chroma = (seed.chroma * chroma_scale).clamp(chroma_min, chroma_max);

    from_oklch(Oklch::new(lightness.clamp(0.0, 1.0), chroma, hue), alpha)
}

fn derive_accent_from_hue(hue: OklabHue<f32>, lightness: f32, chroma: f32, alpha: f32) -> Color {
    from_oklch(
        Oklch::new(lightness.clamp(0.0, 1.0), chroma.clamp(0.0, 0.22), hue),
        alpha,
    )
}

fn usable_hue(color: Oklch, fallback_hue_degrees: f32) -> OklabHue<f32> {
    if color.chroma > 0.02 {
        color.hue
    } else {
        OklabHue::from_degrees(fallback_hue_degrees)
    }
}

fn mix_hues(start: OklabHue<f32>, end: OklabHue<f32>, factor: f32) -> OklabHue<f32> {
    let start = start.into_degrees();
    let mut delta = end.into_degrees() - start;

    if delta > 180.0 {
        delta -= 360.0;
    } else if delta < -180.0 {
        delta += 360.0;
    }

    OklabHue::from_degrees(start + delta * factor.clamp(0.0, 1.0))
}

fn mix_scalar(start: f32, end: f32, factor: f32) -> f32 {
    start + (end - start) * factor.clamp(0.0, 1.0)
}

/// Blends two colors together by a given factor (0.0 = first color, 1.0 = second color).
fn blend_colors(color1: Color, color2: Color, factor: f32) -> Color {
    Color {
        r: color1.r + (color2.r - color1.r) * factor,
        g: color1.g + (color2.g - color1.g) * factor,
        b: color1.b + (color2.b - color1.b) * factor,
        a: color1.a + (color2.a - color1.a) * factor,
    }
}

#[cfg(test)]
fn darken(color: Color, factor: f32) -> Color {
    Color {
        r: color.r * (1.0 - factor),
        g: color.g * (1.0 - factor),
        b: color.b * (1.0 - factor),
        a: color.a,
    }
}

#[cfg(test)]
fn lighten(color: Color, factor: f32) -> Color {
    Color {
        r: color.r + (1.0 - color.r) * factor,
        g: color.g + (1.0 - color.g) * factor,
        b: color.b + (1.0 - color.b) * factor,
        a: color.a,
    }
}

#[cfg(test)]
fn dim_color(color: Color, factor: f32) -> Color {
    Color {
        r: color.r * factor,
        g: color.g * factor,
        b: color.b * factor,
        a: color.a,
    }
}

#[cfg(test)]
fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_iced_theme_dark() {
        let theme = iced::Theme::Dark;
        let style = from_iced_theme(&theme);

        // Dark theme should have dark background
        let brightness = (style.background.r + style.background.g + style.background.b) / 3.0;
        assert!(brightness < 0.5, "Dark theme should have dark background");

        // Text should be bright for contrast
        let text_brightness = (style.text_color.r + style.text_color.g + style.text_color.b) / 3.0;
        assert!(text_brightness > 0.5, "Dark theme should have bright text");
    }

    #[test]
    fn test_from_iced_theme_light() {
        let theme = iced::Theme::Light;
        let style = from_iced_theme(&theme);

        // Light theme should have bright background
        let brightness = (style.background.r + style.background.g + style.background.b) / 3.0;
        assert!(
            brightness > 0.5,
            "Light theme should have bright background"
        );

        // Text should be dark for contrast
        let text_brightness = (style.text_color.r + style.text_color.g + style.text_color.b) / 3.0;
        assert!(text_brightness < 0.5, "Light theme should have dark text");
    }

    #[test]
    fn test_all_iced_themes_produce_valid_styles() {
        // Test all native Iced themes
        for theme in iced::Theme::ALL {
            let style = from_iced_theme(theme);

            // All color components should be valid (0.0 to 1.0)
            assert!(style.background.r >= 0.0 && style.background.r <= 1.0);
            assert!(style.text_color.r >= 0.0 && style.text_color.r <= 1.0);
            assert!(style.gutter_background.r >= 0.0 && style.gutter_background.r <= 1.0);
            assert!(style.line_number_color.r >= 0.0 && style.line_number_color.r <= 1.0);

            // Current line highlight should have transparency
            assert!(
                style.current_line_highlight.a < 1.0,
                "Current line highlight should be semi-transparent for theme: {:?}",
                theme
            );
        }
    }

    #[test]
    fn test_tokyo_night_themes() {
        // Test Tokyo Night variants specifically
        let tokyo_night = iced::Theme::TokyoNight;
        let style = from_iced_theme(&tokyo_night);
        assert!(style.background.r >= 0.0 && style.background.r <= 1.0);

        let tokyo_storm = iced::Theme::TokyoNightStorm;
        let style = from_iced_theme(&tokyo_storm);
        assert!(style.background.r >= 0.0 && style.background.r <= 1.0);

        let tokyo_light = iced::Theme::TokyoNightLight;
        let style = from_iced_theme(&tokyo_light);
        let brightness = (style.background.r + style.background.g + style.background.b) / 3.0;
        assert!(
            brightness > 0.5,
            "Tokyo Night Light should have bright background"
        );
    }

    #[test]
    fn test_catppuccin_themes() {
        // Test Catppuccin variants
        let themes = [
            iced::Theme::CatppuccinLatte,
            iced::Theme::CatppuccinFrappe,
            iced::Theme::CatppuccinMacchiato,
            iced::Theme::CatppuccinMocha,
        ];

        for theme in themes {
            let style = from_iced_theme(&theme);
            // All should produce valid styles
            assert!(style.background.r >= 0.0 && style.background.r <= 1.0);
            assert!(style.text_color.r >= 0.0 && style.text_color.r <= 1.0);
        }
    }

    #[test]
    fn test_gutter_colors_distinct_from_background() {
        let theme = iced::Theme::Dark;
        let style = from_iced_theme(&theme);

        // Gutter background should be different from editor background
        let gutter_diff = (style.gutter_background.r - style.background.r).abs()
            + (style.gutter_background.g - style.background.g).abs()
            + (style.gutter_background.b - style.background.b).abs();

        assert!(
            gutter_diff > 0.0,
            "Gutter should be visually distinct from background"
        );
    }

    #[test]
    fn test_line_numbers_visible_but_subtle() {
        for theme in [iced::Theme::Dark, iced::Theme::Light] {
            let style = from_iced_theme(&theme);
            let palette = theme.extended_palette();

            // Line numbers should be dimmed compared to text
            let line_num_brightness =
                (style.line_number_color.r + style.line_number_color.g + style.line_number_color.b)
                    / 3.0;

            let text_brightness =
                (style.text_color.r + style.text_color.g + style.text_color.b) / 3.0;

            let bg_brightness =
                (style.background.r + style.background.g + style.background.b) / 3.0;

            // Line numbers should be between text and background (more subtle than text)
            // For dark themes: text is bright, line numbers dimmer, background dark
            // For light themes: text is dark, line numbers lighter (gray), background bright
            if palette.is_dark {
                // Dark theme: line numbers should be less bright than text
                assert!(
                    line_num_brightness < text_brightness,
                    "Dark theme line numbers should be dimmer than text. Line num: {}, Text: {}",
                    line_num_brightness,
                    text_brightness
                );
            } else {
                // Light theme: line numbers should be between text (dark) and background (bright)
                assert!(
                    line_num_brightness > text_brightness && line_num_brightness < bg_brightness,
                    "Light theme line numbers should be between text and background. Text: {}, Line num: {}, Bg: {}",
                    text_brightness,
                    line_num_brightness,
                    bg_brightness
                );
            }
        }
    }

    #[test]
    fn test_color_helper_functions() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);

        // Test darken
        let darker = darken(color, 0.5);
        assert!(darker.r < color.r);
        assert!(darker.g < color.g);
        assert!(darker.b < color.b);

        // Test lighten
        let lighter = lighten(color, 0.5);
        assert!(lighter.r > color.r);
        assert!(lighter.g > color.g);
        assert!(lighter.b > color.b);

        // Test dim_color
        let dimmed = dim_color(color, 0.5);
        assert!(dimmed.r < color.r);

        // Test with_alpha
        let transparent = with_alpha(color, 0.3);
        assert!((transparent.a - 0.3).abs() < f32::EPSILON);
        assert!((transparent.r - color.r).abs() < f32::EPSILON);
    }

    #[test]
    fn test_style_copy() {
        let theme = iced::Theme::Dark;
        let style1 = from_iced_theme(&theme);
        let style2 = style1;

        // Verify colors are approximately equal (using epsilon for float comparison)
        assert!((style1.background.r - style2.background.r).abs() < f32::EPSILON);
        assert!((style1.text_color.r - style2.text_color.r).abs() < f32::EPSILON);
        assert!((style1.gutter_background.r - style2.gutter_background.r).abs() < f32::EPSILON);
    }

    #[test]
    fn test_catalog_default() {
        let theme = iced::Theme::Dark;
        let class = <iced::Theme as Catalog>::default();
        let style = theme.style(&class);

        // Should produce a valid style
        assert!(style.background.r >= 0.0 && style.background.r <= 1.0);
        assert!(style.text_color.r >= 0.0 && style.text_color.r <= 1.0);
    }
}
