use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use lilypalooza_audio::{Controller, ControllerError, EditorSize, ParameterInfo};

pub(in crate::app) type SharedController = Arc<Mutex<Box<dyn Controller>>>;
pub(in crate::app) const GENERIC_CONTROLLER_DEFAULT_SIZE: EditorSize = EditorSize {
    width: 360,
    height: 420,
};

#[derive(Clone)]
pub(in crate::app) struct GenericControllerEditor {
    controller: SharedController,
    parameters: Arc<Mutex<Option<Vec<ParameterInfo>>>>,
    values: Arc<Mutex<HashMap<String, f32>>>,
    last_sent_at: Arc<Mutex<HashMap<String, Instant>>>,
    editing_parameter_id: Arc<Mutex<Option<String>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

const GENERIC_CONTROLLER_ROW_HEIGHT: f32 = 44.0;
const GENERIC_CONTROLLER_ROW_STRIDE: f32 = 48.0;
const GENERIC_CONTROLLER_ROW_INSET_X: f32 = 10.0;
const GENERIC_CONTROLLER_VALUE_WIDTH: f32 = 46.0;
const GENERIC_CONTROLLER_LABEL_VALUE_GAP: f32 = 8.0;
const GENERIC_CONTROLLER_SCROLLBAR_WIDTH: f32 = 6.0;
const GENERIC_CONTROLLER_SCROLLBAR_INNER_MARGIN: f32 = 8.0;
const GENERIC_CONTROLLER_SCROLLBAR_OUTER_MARGIN: f32 = 2.0;
const GENERIC_CONTROLLER_SLIDER_RAIL_HEIGHT: f32 = 7.0;
const GENERIC_CONTROLLER_SLIDER_HANDLE_RADIUS: f32 = 5.5;
const GENERIC_CONTROLLER_DRAG_WRITE_INTERVAL: Duration = Duration::from_millis(33);

impl GenericControllerEditor {
    pub(in crate::app) fn new(controller: SharedController) -> Self {
        Self {
            controller,
            parameters: Arc::new(Mutex::new(None)),
            values: Arc::new(Mutex::new(HashMap::new())),
            last_sent_at: Arc::new(Mutex::new(HashMap::new())),
            editing_parameter_id: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    pub(in crate::app) fn render(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        style: &super::AppEditorFrameStyle,
    ) {
        ui.painter().rect_filled(rect, 0.0, style.frame_color);
        let parameters = self.parameters();
        if parameters.is_empty() {
            ui.painter().text(
                rect.center(),
                editor_host::egui::Align2::CENTER_CENTER,
                self.last_error()
                    .unwrap_or_else(|| "No parameters".to_string()),
                editor_host::egui::FontId::proportional(crate::ui_style::FONT_SIZE_UI_SM as f32),
                style.muted_text,
            );
            return;
        }

        ui.scope_builder(
            editor_host::egui::UiBuilder::new()
                .max_rect(rect.shrink2(editor_host::egui::vec2(14.0, 10.0))),
            |ui| {
                apply_generic_controller_scroll_style(ui, style);
                if let Some(error) = self.last_error() {
                    ui.colored_label(style.muted_text, error);
                    ui.add_space(8.0);
                }
                editor_host::egui::ScrollArea::vertical()
                    .id_salt("generic-controller-parameters")
                    .auto_shrink([false, false])
                    .show_rows(
                        ui,
                        GENERIC_CONTROLLER_ROW_STRIDE,
                        parameters.len(),
                        |ui, rows| {
                            ui.set_width(ui.available_width());
                            if let Some(parameters) = parameters.get(rows) {
                                for parameter in parameters {
                                    self.render_parameter_row(ui, parameter, style);
                                }
                            }
                        },
                    );
            },
        );
    }

    fn render_parameter_row(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        parameter: &ParameterInfo,
        style: &super::AppEditorFrameStyle,
    ) {
        let width = ui.available_width();
        let (rect, _response) = ui.allocate_exact_size(
            editor_host::egui::vec2(width, GENERIC_CONTROLLER_ROW_STRIDE),
            editor_host::egui::Sense::hover(),
        );
        let rect = editor_host::egui::Rect::from_min_max(
            rect.min,
            editor_host::egui::pos2(rect.right(), rect.top() + GENERIC_CONTROLLER_ROW_HEIGHT),
        );
        let layout = generic_parameter_row_layout(rect);
        ui.painter()
            .rect_filled(rect, 5.0, style.control_background);
        ui.painter().text(
            layout.label.left_center(),
            editor_host::egui::Align2::LEFT_CENTER,
            &parameter.name,
            editor_host::egui::FontId::proportional(crate::ui_style::FONT_SIZE_UI_SM as f32),
            style.title_color,
        );

        let value = self.render_parameter_slider(ui, parameter, layout.slider, style);

        ui.painter().text(
            layout.value.right_center(),
            editor_host::egui::Align2::RIGHT_CENTER,
            format!("{:>3}%", (value * 100.0).round() as i32),
            editor_host::egui::FontId::monospace(crate::ui_style::FONT_SIZE_UI_XS as f32),
            style.muted_text,
        );
    }

    fn render_parameter_slider(
        &self,
        ui: &mut editor_host::egui::Ui,
        parameter: &ParameterInfo,
        rect: editor_host::egui::Rect,
        style: &super::AppEditorFrameStyle,
    ) -> f32 {
        let response = generic_slider_response(ui, rect, parameter);
        let value = self.apply_parameter_slider_interaction(ui, parameter, rect, &response);
        let handle_hovered = generic_slider_handle_hovered(ui, rect, value);
        paint_generic_parameter_slider(
            ui,
            rect,
            value,
            parameter.readonly,
            response.hovered(),
            handle_hovered,
            style,
        );
        value
    }

    fn apply_parameter_slider_interaction(
        &self,
        ui: &editor_host::egui::Ui,
        parameter: &ParameterInfo,
        rect: editor_host::egui::Rect,
        response: &editor_host::egui::Response,
    ) -> f32 {
        let mut value = self.parameter_value(parameter).clamp(0.0, 1.0);
        if parameter.readonly {
            return value;
        }
        self.begin_parameter_slider_edit(parameter, response);
        value = self.update_parameter_slider_value(ui, parameter, rect, response, value);
        self.finish_parameter_slider_edit(parameter, response, value);
        value
    }

    fn begin_parameter_slider_edit(
        &self,
        parameter: &ParameterInfo,
        response: &editor_host::egui::Response,
    ) {
        if response.drag_started() || response.clicked() {
            self.begin_edit(&parameter.id);
        }
    }

    fn update_parameter_slider_value(
        &self,
        ui: &editor_host::egui::Ui,
        parameter: &ParameterInfo,
        rect: editor_host::egui::Rect,
        response: &editor_host::egui::Response,
        current: f32,
    ) -> f32 {
        if !(response.dragged() || response.clicked()) {
            return current;
        }
        let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else {
            return current;
        };
        let value = generic_slider_value_for_pointer(rect, pointer.x);
        self.set_parameter_value(&parameter.id, value, !response.dragged());
        value
    }

    fn finish_parameter_slider_edit(
        &self,
        parameter: &ParameterInfo,
        response: &editor_host::egui::Response,
        value: f32,
    ) {
        if response.drag_stopped() {
            self.set_parameter_value(&parameter.id, value, true);
            self.end_edit(&parameter.id);
        } else if response.clicked() && !response.dragged() {
            self.end_edit(&parameter.id);
        }
    }

    fn parameters(&self) -> Vec<ParameterInfo> {
        if let Some(parameters) = self
            .parameters
            .lock()
            .ok()
            .and_then(|parameters| parameters.clone())
        {
            return parameters;
        }
        match self.with_controller(|controller| controller.parameters()) {
            Ok(parameters) => {
                if let Ok(mut cached_parameters) = self.parameters.lock() {
                    *cached_parameters = Some(parameters.clone());
                }
                parameters
            }
            Err(error) => {
                self.set_last_error(Some(error.to_string()));
                Vec::new()
            }
        }
    }

    fn get_parameter_value(&self, id: &str) -> Result<f32, ControllerError> {
        self.with_controller(|controller| controller.get_param(id))?
    }

    fn parameter_value(&self, parameter: &ParameterInfo) -> f32 {
        if let Some(value) = self
            .values
            .lock()
            .ok()
            .and_then(|values| values.get(&parameter.id).copied())
        {
            return value;
        }
        match self.get_parameter_value(&parameter.id) {
            Ok(value) => {
                self.cache_parameter_value(&parameter.id, value);
                value
            }
            Err(error) => {
                self.set_last_error(Some(error.to_string()));
                self.cache_parameter_value(&parameter.id, parameter.default);
                parameter.default
            }
        }
    }

    fn set_parameter_value(&self, id: &str, value: f32, force: bool) {
        self.cache_parameter_value(id, value);
        if !force && !self.should_send_drag_value(id) {
            return;
        }
        let result = self.with_controller(|controller| controller.set_param(id, value));
        self.record_result(result);
    }

    fn cache_parameter_value(&self, id: &str, value: f32) {
        if let Ok(mut values) = self.values.lock() {
            values.insert(id.to_string(), value);
        }
    }

    fn should_send_drag_value(&self, id: &str) -> bool {
        let now = Instant::now();
        let Ok(mut sent_at) = self.last_sent_at.lock() else {
            return true;
        };
        if sent_at
            .get(id)
            .is_some_and(|last| now.duration_since(*last) < GENERIC_CONTROLLER_DRAG_WRITE_INTERVAL)
        {
            return false;
        }
        sent_at.insert(id.to_string(), now);
        true
    }

    fn begin_edit(&self, id: &str) {
        if let Ok(mut editing) = self.editing_parameter_id.lock() {
            *editing = Some(id.to_string());
        }
        self.record_result(self.with_controller(|controller| controller.begin_edit(id)));
    }

    fn end_edit(&self, id: &str) {
        if let Ok(mut editing) = self.editing_parameter_id.lock()
            && editing.as_deref() == Some(id)
        {
            *editing = None;
        }
        self.record_result(self.with_controller(|controller| controller.end_edit(id)));
    }

    fn record_result(&self, result: Result<Result<(), ControllerError>, ControllerError>) {
        match result {
            Ok(Ok(())) => self.set_last_error(None),
            Ok(Err(error)) | Err(error) => self.set_last_error(Some(error.to_string())),
        }
    }

    fn set_last_error(&self, error: Option<String>) {
        if let Ok(mut last_error) = self.last_error.lock() {
            *last_error = error;
        }
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|error| error.clone())
    }

    fn with_controller<T>(
        &self,
        f: impl FnOnce(&dyn Controller) -> T,
    ) -> Result<T, ControllerError> {
        self.controller
            .lock()
            .map(|controller| f(controller.as_ref()))
            .map_err(|error| ControllerError::Backend(error.to_string()))
    }
}

fn apply_generic_controller_scroll_style(
    ui: &mut editor_host::egui::Ui,
    style: &super::AppEditorFrameStyle,
) {
    let ui_style = ui.style_mut();
    ui_style.spacing.scroll = editor_host::egui::style::ScrollStyle {
        bar_width: GENERIC_CONTROLLER_SCROLLBAR_WIDTH,
        bar_inner_margin: GENERIC_CONTROLLER_SCROLLBAR_INNER_MARGIN,
        bar_outer_margin: GENERIC_CONTROLLER_SCROLLBAR_OUTER_MARGIN,
        handle_min_length: 24.0,
        ..editor_host::egui::style::ScrollStyle::solid()
    };
    ui_style.visuals.extreme_bg_color = style.frame_color;
    ui_style.visuals.widgets.inactive.bg_fill = style.border_color.gamma_multiply(0.95);
    ui_style.visuals.widgets.hovered.bg_fill = style.control_background_hovered;
    ui_style.visuals.widgets.active.bg_fill = style.control_background_active;
    ui_style.visuals.widgets.inactive.corner_radius = 3.0.into();
    ui_style.visuals.widgets.hovered.corner_radius = 3.0.into();
    ui_style.visuals.widgets.active.corner_radius = 3.0.into();
}

fn paint_generic_parameter_slider(
    ui: &editor_host::egui::Ui,
    rect: editor_host::egui::Rect,
    value: f32,
    readonly: bool,
    hovered: bool,
    handle_hovered: bool,
    style: &super::AppEditorFrameStyle,
) {
    let track = generic_slider_track_rect(rect);
    let value = value.clamp(0.0, 1.0);
    let handle_center = generic_slider_handle_center(track, value);
    let filled = generic_slider_filled_rect(track, handle_center.x);
    let colors = generic_slider_colors(style, readonly, hovered, handle_hovered);

    ui.painter().rect_filled(
        track,
        GENERIC_CONTROLLER_SLIDER_RAIL_HEIGHT / 2.0,
        colors.rail,
    );
    if filled.width() > 0.5 {
        ui.painter().rect_filled(
            filled,
            GENERIC_CONTROLLER_SLIDER_RAIL_HEIGHT / 2.0,
            colors.fill,
        );
    }
    ui.painter().circle_filled(
        handle_center,
        GENERIC_CONTROLLER_SLIDER_HANDLE_RADIUS,
        colors.handle_fill,
    );
    ui.painter().circle_stroke(
        handle_center,
        GENERIC_CONTROLLER_SLIDER_HANDLE_RADIUS,
        editor_host::egui::Stroke::new(1.5, colors.handle_stroke),
    );
}

fn generic_slider_response(
    ui: &mut editor_host::egui::Ui,
    rect: editor_host::egui::Rect,
    parameter: &ParameterInfo,
) -> editor_host::egui::Response {
    let response = ui.interact(
        rect.expand2(editor_host::egui::vec2(0.0, 5.0)),
        ui.make_persistent_id(("generic-parameter-slider", parameter.id.as_str())),
        generic_slider_sense(parameter.readonly),
    );
    if parameter.readonly {
        response
    } else {
        response.on_hover_and_drag_cursor(editor_host::egui::CursorIcon::PointingHand)
    }
}

fn generic_slider_sense(readonly: bool) -> editor_host::egui::Sense {
    if readonly {
        editor_host::egui::Sense::hover()
    } else {
        editor_host::egui::Sense::click_and_drag()
    }
}

fn generic_slider_handle_hovered(
    ui: &editor_host::egui::Ui,
    rect: editor_host::egui::Rect,
    value: f32,
) -> bool {
    ui.input(|input| {
        input.pointer.latest_pos().is_some_and(|pos| {
            generic_slider_handle_center(generic_slider_track_rect(rect), value).distance(pos)
                <= GENERIC_CONTROLLER_SLIDER_HANDLE_RADIUS + 3.0
        })
    })
}

#[derive(Debug, Clone, Copy)]
struct GenericSliderColors {
    rail: editor_host::egui::Color32,
    fill: editor_host::egui::Color32,
    handle_fill: editor_host::egui::Color32,
    handle_stroke: editor_host::egui::Color32,
}

fn generic_slider_colors(
    style: &super::AppEditorFrameStyle,
    readonly: bool,
    hovered: bool,
    handle_hovered: bool,
) -> GenericSliderColors {
    GenericSliderColors {
        rail: generic_slider_rail_color(style, readonly),
        fill: generic_slider_fill_color(style, readonly, hovered),
        handle_fill: generic_slider_handle_fill(style, readonly, handle_hovered),
        handle_stroke: generic_slider_handle_stroke(style, readonly, handle_hovered),
    }
}

fn generic_slider_rail_color(
    style: &super::AppEditorFrameStyle,
    readonly: bool,
) -> editor_host::egui::Color32 {
    let multiplier = if readonly { 0.75 } else { 0.90 };
    style.frame_color.gamma_multiply(multiplier)
}

fn generic_slider_fill_color(
    style: &super::AppEditorFrameStyle,
    readonly: bool,
    hovered: bool,
) -> editor_host::egui::Color32 {
    if readonly {
        style.border_color
    } else if hovered {
        style.control_background_active
    } else {
        style.control_background_hovered
    }
}

fn generic_slider_handle_fill(
    style: &super::AppEditorFrameStyle,
    readonly: bool,
    handle_hovered: bool,
) -> editor_host::egui::Color32 {
    if readonly {
        style.control_background
    } else if handle_hovered {
        style.control_background_hovered
    } else {
        style.titlebar_color
    }
}

fn generic_slider_handle_stroke(
    style: &super::AppEditorFrameStyle,
    readonly: bool,
    handle_hovered: bool,
) -> editor_host::egui::Color32 {
    if handle_hovered && !readonly {
        style.muted_text.gamma_multiply(0.85)
    } else {
        style.muted_text.gamma_multiply(0.55)
    }
}

fn generic_slider_filled_rect(
    track: editor_host::egui::Rect,
    handle_x: f32,
) -> editor_host::egui::Rect {
    editor_host::egui::Rect::from_min_max(
        track.left_top(),
        editor_host::egui::pos2(handle_x, track.bottom()),
    )
}

fn generic_slider_track_rect(rect: editor_host::egui::Rect) -> editor_host::egui::Rect {
    editor_host::egui::Rect::from_center_size(
        rect.center(),
        editor_host::egui::vec2(
            (rect.width() - GENERIC_CONTROLLER_SLIDER_HANDLE_RADIUS * 2.0).max(1.0),
            GENERIC_CONTROLLER_SLIDER_RAIL_HEIGHT,
        ),
    )
}

fn generic_slider_handle_center(
    track: editor_host::egui::Rect,
    value: f32,
) -> editor_host::egui::Pos2 {
    editor_host::egui::pos2(
        track.left() + track.width() * value.clamp(0.0, 1.0),
        track.center().y,
    )
}

fn generic_slider_value_for_pointer(rect: editor_host::egui::Rect, pointer_x: f32) -> f32 {
    let track = generic_slider_track_rect(rect);
    ((pointer_x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct GenericParameterRowLayout {
    label: editor_host::egui::Rect,
    value: editor_host::egui::Rect,
    slider: editor_host::egui::Rect,
}

fn generic_parameter_row_layout(rect: editor_host::egui::Rect) -> GenericParameterRowLayout {
    let content_left = rect.left() + GENERIC_CONTROLLER_ROW_INSET_X;
    let content_right = (rect.right() - GENERIC_CONTROLLER_ROW_INSET_X).max(content_left);
    let label_value_split =
        (content_right - GENERIC_CONTROLLER_VALUE_WIDTH - GENERIC_CONTROLLER_LABEL_VALUE_GAP)
            .max(content_left);
    let label = editor_host::egui::Rect::from_min_max(
        editor_host::egui::pos2(content_left, rect.top()),
        editor_host::egui::pos2(label_value_split, rect.top() + 20.0),
    );
    let value = editor_host::egui::Rect::from_min_max(
        editor_host::egui::pos2(
            label_value_split + GENERIC_CONTROLLER_LABEL_VALUE_GAP,
            rect.top(),
        ),
        editor_host::egui::pos2(content_right, rect.top() + 20.0),
    );
    let slider = editor_host::egui::Rect::from_min_max(
        editor_host::egui::pos2(content_left, rect.top() + 24.0),
        editor_host::egui::pos2(content_right, rect.bottom() - 5.0),
    );
    GenericParameterRowLayout {
        label,
        value,
        slider,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use lilypalooza_audio::{ParameterDescriptor, ProcessorDescriptor, ProcessorState};

    use super::*;

    static PARAMS: [ParameterDescriptor; 1] = [ParameterDescriptor {
        id: "cutoff",
        name: "Cutoff",
        default: 0.25,
    }];
    static DESCRIPTOR: ProcessorDescriptor = ProcessorDescriptor {
        name: "Test",
        params: &PARAMS,
        editor: None,
    };

    struct CountingController {
        parameter_calls: Arc<AtomicUsize>,
        get_calls: Arc<AtomicUsize>,
        set_calls: Arc<AtomicUsize>,
    }

    impl Controller for CountingController {
        fn descriptor(&self) -> &'static ProcessorDescriptor {
            &DESCRIPTOR
        }

        fn parameters(&self) -> Vec<ParameterInfo> {
            self.parameter_calls.fetch_add(1, Ordering::Relaxed);
            self.descriptor()
                .params
                .iter()
                .map(ParameterInfo::from)
                .collect()
        }

        fn get_param(&self, id: &str) -> Result<f32, ControllerError> {
            if id != "cutoff" {
                return Err(ControllerError::UnknownParameter(id.to_string()));
            }
            self.get_calls.fetch_add(1, Ordering::Relaxed);
            Ok(0.42)
        }

        fn set_param(&self, _id: &str, _normalized: f32) -> Result<(), ControllerError> {
            self.set_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn save_state(&self) -> Result<ProcessorState, ControllerError> {
            Ok(ProcessorState::default())
        }

        fn load_state(&self, _state: &ProcessorState) -> Result<(), ControllerError> {
            Ok(())
        }
    }

    fn counting_editor(
        parameter_calls: Arc<AtomicUsize>,
        get_calls: Arc<AtomicUsize>,
        set_calls: Arc<AtomicUsize>,
    ) -> GenericControllerEditor {
        GenericControllerEditor::new(Arc::new(Mutex::new(Box::new(CountingController {
            parameter_calls,
            get_calls,
            set_calls,
        }))))
    }

    fn assert_f32_near(left: f32, right: f32) {
        assert!(
            (left - right).abs() <= f32::EPSILON,
            "{left} differs from {right}"
        );
    }

    #[test]
    fn generic_controller_caches_parameter_metadata_and_values() {
        let parameter_calls = Arc::new(AtomicUsize::new(0));
        let get_calls = Arc::new(AtomicUsize::new(0));
        let set_calls = Arc::new(AtomicUsize::new(0));
        let editor = counting_editor(
            Arc::clone(&parameter_calls),
            Arc::clone(&get_calls),
            Arc::clone(&set_calls),
        );

        let parameters = editor.parameters();
        let parameters_again = editor.parameters();
        assert_eq!(parameters, parameters_again);
        assert_eq!(parameter_calls.load(Ordering::Relaxed), 1);

        let parameter = parameters.first().expect("test controller has a parameter");
        assert_f32_near(editor.parameter_value(parameter), 0.42);
        assert_f32_near(editor.parameter_value(parameter), 0.42);
        assert_eq!(get_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn generic_controller_throttles_drag_parameter_writes() {
        let set_calls = Arc::new(AtomicUsize::new(0));
        let editor = counting_editor(
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&set_calls),
        );

        editor.set_parameter_value("cutoff", 0.1, false);
        editor.set_parameter_value("cutoff", 0.2, false);
        editor.set_parameter_value("cutoff", 0.3, true);

        assert_eq!(set_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn generic_controller_slider_uses_full_row_width() {
        let rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(0.0, 0.0),
            editor_host::egui::vec2(332.0, GENERIC_CONTROLLER_ROW_HEIGHT),
        );
        let layout = generic_parameter_row_layout(rect);

        assert!(layout.label.right() <= layout.value.left());
        assert_f32_near(layout.slider.left(), 10.0);
        assert_f32_near(layout.slider.right(), 322.0);
        assert!(layout.slider.width() > 300.0);
    }

    #[test]
    fn generic_controller_slider_value_uses_visible_track_bounds() {
        let rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(10.0, 20.0),
            editor_host::egui::vec2(100.0, 18.0),
        );
        let track = generic_slider_track_rect(rect);

        assert_f32_near(generic_slider_value_for_pointer(rect, track.left()), 0.0);
        assert_f32_near(
            generic_slider_value_for_pointer(rect, track.center().x),
            0.5,
        );
        assert_f32_near(generic_slider_value_for_pointer(rect, track.right()), 1.0);
    }
}
