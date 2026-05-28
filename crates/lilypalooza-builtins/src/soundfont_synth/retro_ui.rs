use std::sync::Arc;

use lilypalooza_egui_baseview::egui;
use num_traits::ToPrimitive;

use super::{
    PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW,
    ProgramChoice,
    RETRO_DISPLAY_FONT,
    RETRO_UI_FONT,
    positive_rows,
};

pub(super) fn install_retro_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "w95fa".to_string(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/W95FA.otf"
        ))),
    );
    fonts.font_data.insert(
        "cozette".to_string(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/CozetteVector.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name(RETRO_UI_FONT.into()),
        vec!["w95fa".to_string()],
    );
    fonts.families.insert(
        egui::FontFamily::Name(RETRO_DISPLAY_FONT.into()),
        vec!["cozette".to_string()],
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .expect("default proportional font family exists")
        .insert(0, "w95fa".to_string());
    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .expect("default monospace font family exists")
        .insert(0, "cozette".to_string());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.visuals.window_corner_radius = 0.into();
    style.visuals.widgets.noninteractive.corner_radius = 0.into();
    style.visuals.widgets.inactive.corner_radius = 0.into();
    style.visuals.widgets.hovered.corner_radius = 0.into();
    style.visuals.widgets.active.corner_radius = 0.into();
    style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    ctx.set_global_style(style);
}

pub(super) fn draw_window_shell(ui: &mut egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, retro::FACE);
    bevel(painter, rect.shrink(2.0), false);

    let title = egui::Rect::from_min_size(
        rect.min + egui::vec2(8.0, 8.0),
        egui::vec2(rect.width() - 16.0, 34.0),
    );
    painter.rect_filled(title, 0.0, retro::TITLE);
    bevel(painter, title, false);
    painter.text(
        title.left_center() + egui::vec2(12.0, 1.0),
        egui::Align2::LEFT_CENTER,
        "SF-01  SOUNDFONT ROMPLER",
        retro_font(25.0, false),
        retro::TITLE_TEXT,
    );
}

pub(super) fn retro_group<F>(ui: &mut egui::Ui, local: egui::Rect, title: &'static str, content: F)
where
    F: FnOnce(&mut egui::Ui),
{
    let rect = local_rect(ui, local);
    ui.painter().rect_filled(rect, 0.0, retro::FACE);
    bevel(ui.painter(), rect, true);
    let label_pos = rect.min + egui::vec2(18.0, -3.0);
    let label_galley = ui.painter().layout_no_wrap(
        title.to_string(),
        retro_font(20.0, false),
        retro::LABEL_BLUE,
    );
    let label_cover = egui::Rect::from_min_size(
        label_pos + egui::vec2(-6.0, 2.0),
        label_galley.size() + egui::vec2(12.0, 2.0),
    );
    ui.painter().rect_filled(label_cover, 0.0, retro::FACE);
    ui.painter()
        .galley(label_pos, label_galley, retro::LABEL_BLUE);

    let content_rect = rect.shrink2(egui::vec2(20.0, 16.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        content,
    );
}

pub(super) fn retro_select_box(
    ui: &mut egui::Ui,
    local: egui::Rect,
    id: &'static str,
    text: &str,
) -> egui::Response {
    let rect = local_rect(ui, local);
    let response = ui.interact(rect, ui.id().with(id), egui::Sense::click());
    ui.painter().rect_filled(rect, 0.0, retro::FIELD);
    bevel(ui.painter(), rect, true);
    retro_text_aligned(
        ui,
        rect.left_center() + egui::vec2(10.0, -1.0),
        egui::Align2::LEFT_CENTER,
        text,
        18.0,
        retro::TEXT,
        false,
    );
    let button = egui::Rect::from_min_max(
        rect.right_top() - egui::vec2(29.0, 0.0),
        rect.right_bottom(),
    );
    retro_button_frame(
        ui,
        button,
        response.hovered(),
        response.is_pointer_button_down_on(),
    );
    draw_triangle(ui.painter(), button.center(), false, retro::TEXT);
    response
}

pub(super) fn retro_step_button(ui: &mut egui::Ui, local: egui::Rect, up: bool) -> egui::Response {
    let rect = local_rect(ui, local);
    let response = ui.interact(
        rect,
        ui.id()
            .with((rect.min.x.to_bits(), rect.min.y.to_bits(), up)),
        egui::Sense::click(),
    );
    retro_button_frame(
        ui,
        rect,
        response.hovered(),
        response.is_pointer_button_down_on(),
    );
    draw_triangle(ui.painter(), rect.center(), up, retro::TEXT);
    response
}

pub(super) fn retro_choice_list(
    ui: &mut egui::Ui,
    local: egui::Rect,
    choices: &[String],
    selected_index: usize,
    id: &'static str,
) -> Option<usize> {
    let rect = local_rect(ui, local);
    ui.painter().rect_filled(rect, 0.0, retro::FIELD);
    bevel(ui.painter(), rect, true);

    let row_height = 24.0;
    let vertical_padding = 6.0;
    let visible_rows = positive_rows((rect.height() - vertical_padding * 2.0) / row_height);
    let mut selected = None;
    for row in 0..visible_rows {
        let Some(choice) = choices.get(row) else {
            break;
        };
        let row_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(4.0, vertical_padding + row as f32 * row_height),
            egui::vec2(rect.width() - 8.0, row_height - 2.0),
        );
        let response = ui.interact(row_rect, ui.id().with((id, row)), egui::Sense::click());
        if row == selected_index {
            ui.painter().rect_filled(row_rect, 0.0, retro::SELECT);
        } else if response.hovered() {
            ui.painter().rect_filled(row_rect, 0.0, retro::LCD_HOVER);
        }
        retro_text_aligned(
            ui,
            row_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            choice,
            17.0,
            if row == selected_index {
                retro::TITLE_TEXT
            } else {
                retro::TEXT
            },
            false,
        );
        if response.clicked() {
            selected = Some(row);
        }
    }
    selected
}

pub(super) fn retro_checkbox(
    ui: &mut egui::Ui,
    local: egui::Rect,
    checked: bool,
    text: &str,
) -> egui::Response {
    let rect = local_rect(ui, local);
    let response = ui.interact(rect, ui.id().with(text), egui::Sense::click());
    let box_rect =
        egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 2.0), egui::vec2(18.0, 18.0));
    ui.painter().rect_filled(box_rect, 0.0, retro::FIELD);
    bevel(ui.painter(), box_rect, true);
    if checked {
        ui.painter()
            .rect_filled(box_rect.shrink(4.0), 0.0, retro::GREEN);
        ui.painter().rect_stroke(
            box_rect.shrink(4.0),
            0.0,
            egui::Stroke::new(1.0, retro::TEXT),
            egui::StrokeKind::Inside,
        );
    }
    retro_text_abs(
        ui,
        rect.min + egui::vec2(28.0, 1.0),
        text,
        16.0,
        retro::TEXT,
        false,
    );
    response
}

pub(super) fn draw_display_box(ui: &mut egui::Ui, local: egui::Rect, text: &str) {
    let rect = local_rect(ui, local);
    ui.painter().rect_filled(rect, 0.0, retro::LCD);
    bevel(ui.painter(), rect, true);
    retro_text_aligned(
        ui,
        rect.left_center() + egui::vec2(10.0, -1.0),
        egui::Align2::LEFT_CENTER,
        text,
        20.0,
        retro::LCD_TEXT,
        false,
    );
}

pub(super) struct NumberFieldResponse {
    pub(super) value: Option<u16>,
    pub(super) focused: bool,
}

pub(super) fn retro_number_field(
    ui: &mut egui::Ui,
    local: egui::Rect,
    id: &'static str,
    text: &mut String,
    current: u16,
    min: u16,
    max: u16,
) -> NumberFieldResponse {
    let rect = local_rect(ui, local);
    ui.painter().rect_filled(rect, 0.0, retro::LCD);
    bevel(ui.painter(), rect, true);

    let edit_rect = rect.shrink2(egui::vec2(8.0, 3.0));
    let response = ui.put(
        edit_rect,
        egui::TextEdit::singleline(text)
            .id_salt(id)
            .font(retro_font(20.0, true))
            .text_color(retro::LCD_TEXT)
            .desired_width(edit_rect.width())
            .frame(egui::Frame::NONE),
    );

    let commit = response.lost_focus() || ui.input(|input| input.key_pressed(egui::Key::Enter));
    let value = if commit {
        if text.trim().is_empty() {
            None
        } else {
            let parsed = text
                .trim()
                .parse::<u16>()
                .map(|value| value.clamp(min, max))
                .unwrap_or(current);
            *text = parsed.to_string();
            Some(parsed)
        }
    } else {
        None
    };

    NumberFieldResponse {
        value,
        focused: response.has_focus(),
    }
}

pub(super) fn draw_led(ui: &mut egui::Ui, local_pos: egui::Pos2, on: bool, color: egui::Color32) {
    let center = ui.min_rect().min + local_pos.to_vec2();
    let fill = if on { color } else { retro::SHADOW };
    ui.painter().circle_filled(center, 7.0, retro::BLACK);
    ui.painter().circle_filled(center, 6.0, fill);
    ui.painter()
        .circle_stroke(center, 7.0, egui::Stroke::new(1.0, retro::BLACK));
    ui.painter()
        .circle_stroke(center, 5.0, egui::Stroke::new(1.0, retro::SHADOW));
}

pub(super) fn program_list(
    ui: &mut egui::Ui,
    local: egui::Rect,
    programs: &[ProgramChoice],
    selected_index: usize,
    first: &mut usize,
    scroll_remainder: &mut f32,
) -> Option<u8> {
    let mut selected = None;
    let rect = local_rect(ui, local);
    ui.painter().rect_filled(rect, 0.0, retro::FIELD);
    bevel(ui.painter(), rect, true);
    let row_height = 24.0;
    let row_gap = 2.0;
    let vertical_padding = 6.0;
    let visible_rows = positive_rows((rect.height() - vertical_padding * 2.0) / row_height);
    let max_first = programs.len().saturating_sub(visible_rows);
    *first = (*first).min(max_first);
    let list_response = ui.interact(
        rect,
        ui.id().with("program-list-scroll"),
        egui::Sense::hover(),
    );
    if list_response.hovered()
        || ui.input(|input| {
            input
                .pointer
                .latest_pos()
                .is_some_and(|pos| rect.contains(pos))
        })
    {
        let scroll_delta = ui.input(program_list_scroll_delta);
        *scroll_remainder += scroll_delta;
        while *scroll_remainder <= -PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW {
            *first = (*first).saturating_add(1).min(max_first);
            *scroll_remainder += PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW;
        }
        while *scroll_remainder >= PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW {
            *first = (*first).saturating_sub(1);
            *scroll_remainder -= PROGRAM_SCROLL_WHEEL_POINTS_PER_ROW;
        }
    }

    let list_rect = egui::Rect::from_min_max(rect.min, rect.max - egui::vec2(24.0, 0.0));
    for row in 0..visible_rows {
        let index = *first + row;
        let row_rect = egui::Rect::from_min_size(
            list_rect.min + egui::vec2(4.0, vertical_padding + row as f32 * row_height),
            egui::vec2(list_rect.width() - 8.0, row_height - row_gap),
        );
        if let Some(program) = programs.get(index) {
            let response = ui.interact(
                row_rect,
                ui.id().with(("program-row", index)),
                egui::Sense::click(),
            );
            if index == selected_index {
                ui.painter().rect_filled(row_rect, 0.0, retro::SELECT);
            } else if response.hovered() {
                ui.painter().rect_filled(row_rect, 0.0, retro::LCD_HOVER);
            }
            let text_color = if index == selected_index {
                retro::TITLE_TEXT
            } else {
                retro::LCD_TEXT
            };
            retro_text_aligned(
                ui,
                row_rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &program.label,
                17.0,
                text_color,
                false,
            );
            if response.clicked() {
                selected = Some(program.program);
            }
        }
    }

    let scroll_rect = egui::Rect::from_min_max(
        egui::pos2(rect.right() - 24.0, rect.top()),
        rect.right_bottom(),
    );
    let up = egui::Rect::from_min_size(
        scroll_rect.min + egui::vec2(3.0, 3.0),
        egui::vec2(18.0, 16.0),
    );
    let down = egui::Rect::from_min_size(
        egui::pos2(scroll_rect.left() + 3.0, scroll_rect.bottom() - 19.0),
        egui::vec2(18.0, 16.0),
    );
    let up_response = ui.interact(up, ui.id().with("program-scroll-up"), egui::Sense::click());
    let down_response = ui.interact(
        down,
        ui.id().with("program-scroll-down"),
        egui::Sense::click(),
    );
    retro_button_frame(
        ui,
        up,
        up_response.hovered(),
        up_response.is_pointer_button_down_on(),
    );
    retro_button_frame(
        ui,
        down,
        down_response.hovered(),
        down_response.is_pointer_button_down_on(),
    );
    draw_triangle(ui.painter(), up.center(), true, retro::TEXT);
    draw_triangle(ui.painter(), down.center(), false, retro::TEXT);
    if up_response.clicked() {
        *first = (*first).saturating_sub(1);
    }
    if down_response.clicked() {
        *first = (*first).saturating_add(1).min(max_first);
    }
    let track = egui::Rect::from_min_max(
        egui::pos2(scroll_rect.left() + 4.0, up.bottom() + 2.0),
        egui::pos2(scroll_rect.right() - 4.0, down.top() - 2.0),
    );
    ui.painter().rect_filled(track, 0.0, retro::FACE);
    bevel(ui.painter(), track, true);
    let thumb_height = (track.height() * visible_rows as f32
        / programs.len().max(visible_rows) as f32)
        .clamp(18.0, track.height());
    let thumb_top = if max_first == 0 {
        track.top()
    } else {
        track.top() + (track.height() - thumb_height) * (*first as f32 / max_first as f32)
    };
    let thumb = egui::Rect::from_min_size(
        egui::pos2(track.left(), thumb_top),
        egui::vec2(track.width(), thumb_height),
    );
    let thumb_response = ui.interact(
        thumb,
        ui.id().with("program-scroll-thumb"),
        egui::Sense::click_and_drag(),
    );
    let track_response = ui.interact(
        track,
        ui.id().with("program-scroll-track"),
        egui::Sense::click_and_drag(),
    );
    let live_scroll_pointer = ui.input(|input| {
        let pos = input.pointer.latest_pos()?;
        if input.pointer.primary_down() && track.contains(pos) {
            Some(pos)
        } else {
            None
        }
    });
    if max_first > 0
        && (thumb_response.dragged()
            || thumb_response.clicked()
            || track_response.dragged()
            || track_response.clicked()
            || live_scroll_pointer.is_some())
        && let Some(pointer) = live_scroll_pointer
            .or_else(|| thumb_response.interact_pointer_pos())
            .or_else(|| track_response.interact_pointer_pos())
    {
        let travel = (track.height() - thumb_height).max(1.0);
        let ratio = ((pointer.y - track.top() - thumb_height / 2.0) / travel).clamp(0.0, 1.0);
        *first = (ratio * max_first as f32)
            .round()
            .to_usize()
            .unwrap_or(max_first)
            .min(max_first);
    }
    retro_button_frame(
        ui,
        thumb,
        thumb_response.hovered() || thumb_response.dragged(),
        thumb_response.is_pointer_button_down_on(),
    );
    selected
}

pub(super) fn program_list_scroll_delta(input: &egui::InputState) -> f32 {
    let raw_scroll_delta = input
        .raw
        .events
        .iter()
        .filter_map(|event| match event {
            egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                egui::MouseWheelUnit::Point => delta.y,
                egui::MouseWheelUnit::Line => delta.y * 40.0,
                egui::MouseWheelUnit::Page => delta.y * input.viewport_rect().height(),
            }),
            _ => None,
        })
        .sum::<f32>();
    if raw_scroll_delta != 0.0 {
        raw_scroll_delta
    } else {
        input.smooth_scroll_delta.y
    }
}

pub(super) fn retro_slider(
    ui: &mut egui::Ui,
    id: &'static str,
    local: egui::Rect,
    value: f32,
) -> Option<Option<f32>> {
    let rect = local_rect(ui, local);
    let widget_id = ui.id().with(id);
    let response = ui.interact(
        rect.expand2(egui::vec2(8.0, 6.0)),
        widget_id,
        egui::Sense::click_and_drag(),
    );
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.center().y - 2.0),
        egui::pos2(rect.right(), rect.center().y + 2.0),
    );
    ui.painter().rect_filled(track, 0.0, retro::SHADOW);
    bevel(ui.painter(), track, true);
    for tick in 0..=6 {
        let x = rect.left() + rect.width() * tick as f32 / 6.0;
        ui.painter().line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + 6.0)],
            egui::Stroke::new(1.0, retro::DARK_HILITE),
        );
    }

    let normalized = value.clamp(0.0, 1.0);
    let x = rect.left() + rect.width() * normalized;
    let thumb =
        egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(18.0, 26.0));
    retro_button_frame(
        ui,
        thumb,
        response.hovered(),
        response.is_pointer_button_down_on(),
    );
    for offset in [-4.0, 0.0, 4.0] {
        ui.painter().line_segment(
            [
                egui::pos2(thumb.center().x + offset, thumb.top() + 5.0),
                egui::pos2(thumb.center().x + offset, thumb.bottom() - 5.0),
            ],
            egui::Stroke::new(1.0, retro::DARK_HILITE),
        );
    }

    let reset = response.double_clicked() || slider_manual_double_click(ui, widget_id, &response);
    if reset {
        Some(None)
    } else if (response.dragged() || response.clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        Some(Some(
            ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
        ))
    } else {
        None
    }
}

pub(super) fn slider_manual_double_click(
    ui: &mut egui::Ui,
    id: egui::Id,
    response: &egui::Response,
) -> bool {
    if !response.clicked() {
        return false;
    }

    let Some(pointer) = response
        .interact_pointer_pos()
        .or_else(|| ui.input(|input| input.pointer.latest_pos()))
    else {
        return false;
    };
    let now = ui.input(|input| input.time);
    let storage_id = id.with("last-click");
    let previous = ui.data(|data| data.get_temp::<(f64, egui::Pos2)>(storage_id));
    ui.data_mut(|data| data.insert_temp(storage_id, (now, pointer)));

    previous.is_some_and(|(last_time, last_pos)| {
        now - last_time <= 0.35 && last_pos.distance(pointer) <= 8.0
    })
}

pub(super) fn retro_button_frame(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    hovered: bool,
    pressed: bool,
) {
    ui.painter().rect_filled(
        rect,
        0.0,
        if hovered {
            retro::BUTTON_HOVER
        } else {
            retro::FACE
        },
    );
    bevel(ui.painter(), rect, pressed);
}

pub(super) fn bevel(painter: &egui::Painter, rect: egui::Rect, inset: bool) {
    let (top_left, bottom_right) = if inset {
        (retro::SHADOW, retro::HILITE)
    } else {
        (retro::HILITE, retro::SHADOW)
    };
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, top_left),
    );
    painter.line_segment(
        [rect.left_top(), rect.left_bottom()],
        egui::Stroke::new(1.0, top_left),
    );
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(1.0, bottom_right),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        egui::Stroke::new(1.0, bottom_right),
    );
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, retro::BLACK),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn draw_triangle(
    painter: &egui::Painter,
    center: egui::Pos2,
    up: bool,
    color: egui::Color32,
) {
    let points = if up {
        vec![
            center + egui::vec2(-5.0, 3.0),
            center + egui::vec2(5.0, 3.0),
            center + egui::vec2(0.0, -4.0),
        ]
    } else {
        vec![
            center + egui::vec2(-5.0, -3.0),
            center + egui::vec2(5.0, -3.0),
            center + egui::vec2(0.0, 4.0),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

pub(super) fn retro_text(
    ui: &mut egui::Ui,
    local_pos: egui::Pos2,
    text: &str,
    size: f32,
    color: egui::Color32,
    display: bool,
) {
    retro_text_abs(
        ui,
        ui.min_rect().min + local_pos.to_vec2(),
        text,
        size,
        color,
        display,
    );
}

pub(super) fn retro_text_abs(
    ui: &mut egui::Ui,
    pos: egui::Pos2,
    text: &str,
    size: f32,
    color: egui::Color32,
    display: bool,
) {
    retro_text_aligned(ui, pos, egui::Align2::LEFT_TOP, text, size, color, display);
}

pub(super) fn retro_text_aligned(
    ui: &mut egui::Ui,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    size: f32,
    color: egui::Color32,
    display: bool,
) {
    ui.painter()
        .text(pos, align, text, retro_font(size, display), color);
}

pub(super) fn retro_font(size: f32, display: bool) -> egui::FontId {
    let family = if display {
        egui::FontFamily::Name(RETRO_DISPLAY_FONT.into())
    } else {
        egui::FontFamily::Name(RETRO_UI_FONT.into())
    };
    egui::FontId::new(size, family)
}

pub(super) fn local_rect(ui: &egui::Ui, rect: egui::Rect) -> egui::Rect {
    rect.translate(ui.min_rect().min.to_vec2())
}

pub(super) fn rect(x: f32, y: f32, width: f32, height: f32) -> egui::Rect {
    egui::Rect::from_min_size(pos(x, y), egui::vec2(width, height))
}

pub(super) fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

pub(super) mod retro {
    use lilypalooza_egui_baseview::egui::Color32;

    pub const FACE: Color32 = Color32::from_rgb(192, 192, 192);
    pub const FIELD: Color32 = Color32::from_rgb(232, 229, 220);
    pub const LCD: Color32 = Color32::from_rgb(148, 172, 104);
    pub const LCD_HOVER: Color32 = Color32::from_rgb(166, 188, 124);
    pub const LCD_TEXT: Color32 = Color32::from_rgb(16, 24, 12);
    pub const TEXT: Color32 = Color32::from_rgb(16, 16, 16);
    pub const TITLE: Color32 = Color32::from_rgb(0, 0, 128);
    pub const TITLE_TEXT: Color32 = Color32::from_rgb(255, 255, 255);
    pub const LABEL_BLUE: Color32 = Color32::from_rgb(0, 46, 140);
    pub const SELECT: Color32 = Color32::from_rgb(0, 0, 128);
    pub const GREEN: Color32 = Color32::from_rgb(96, 210, 82);
    pub const LED_IDLE: Color32 = Color32::from_rgb(58, 92, 50);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgb(210, 210, 210);
    pub const HILITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const DARK_HILITE: Color32 = Color32::from_rgb(128, 128, 128);
    pub const SHADOW: Color32 = Color32::from_rgb(64, 64, 64);
    pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
    pub const DISPLAY: Color32 = Color32::from_rgb(0, 80, 76);
}
