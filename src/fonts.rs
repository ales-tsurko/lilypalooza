use iced::Font;

pub(crate) const MANROPE_REGULAR_BYTES: &[u8] =
    include_bytes!("../assets/fonts/Manrope-Regular.ttf");
pub(crate) const MANROPE_MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Manrope-Medium.ttf");
pub(crate) const MANROPE_BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/Manrope-Bold.ttf");
pub(crate) const JETBRAINS_MONO_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
pub(crate) const JETBRAINS_MONO_BOLD_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");
pub(crate) const JETBRAINS_MONO_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf");
pub(crate) const JETBRAINS_MONO_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf");
pub(crate) const JETBRAINS_MONO_MEDIUM_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf");
pub(crate) const JETBRAINS_MONO_MEDIUM_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-MediumItalic.ttf");

pub(crate) const UI: Font = Font::with_name("Manrope");
pub(crate) const MONO: Font = Font::with_name("JetBrains Mono");
