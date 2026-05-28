use super::*;

pub(super) fn egui_color(color: Color) -> editor_host::egui::Color32 {
    editor_host::egui::Color32::from_rgba_unmultiplied(
        color_channel_u8(color.r),
        color_channel_u8(color.g),
        color_channel_u8(color.b),
        color_channel_u8(color.a),
    )
}

pub(super) fn color_channel_u8(value: f32) -> u8 {
    crate::number::f32_to_u8(value.clamp(0.0, 1.0) * 255.0)
}

pub(super) fn mix_iced_color(a: Color, b: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);

    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub(super) fn render_lucide_icon(
    icon: AppEditorFrameIcon,
) -> Option<editor_host::egui::ColorImage> {
    let svg = std::str::from_utf8(icon.svg_bytes())
        .ok()?
        .replace("currentColor", "white");
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &options).ok()?;
    let size = tree.size().to_int_size().to_size();
    let dimension = 64;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(dimension, dimension)?;
    let transform = resvg::tiny_skia::Transform::from_scale(
        dimension as f32 / size.width(),
        dimension as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(editor_host::egui::ColorImage::from_rgba_unmultiplied(
        [dimension as usize, dimension as usize],
        pixmap.data(),
    ))
}

pub(super) fn ellipsize_for_width(text: &str, width: f32, font_size: f32) -> String {
    let max_chars = crate::number::f32_to_usize((width / (font_size * 0.56)).floor().max(1.0));
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let byte_index = text
        .char_indices()
        .nth(max_chars - 3)
        .map_or(text.len(), |(index, _)| index);
    format!("{}...", text.get(..byte_index).unwrap_or(text))
}

pub(super) fn preset_rename_enter_pressed(ui: &editor_host::egui::Ui) -> bool {
    ui.input(|input| input.key_pressed(editor_host::egui::Key::Enter))
}

pub(super) fn preset_rename_cancelled(
    ui: &editor_host::egui::Ui,
    edit: &editor_host::egui::Response,
    focus_requested: bool,
) -> bool {
    ui.input(|input| input.key_pressed(editor_host::egui::Key::Escape))
        || (!focus_requested && edit.lost_focus())
}

pub(super) fn preset_rename_command(
    item: &editor_host::EditorPresetItem,
    name: String,
) -> Option<editor_host::EditorFrameCommand> {
    (!name.is_empty()).then(|| editor_host::EditorFrameCommand::RenamePreset {
        id: item.id.clone(),
        name,
    })
}
