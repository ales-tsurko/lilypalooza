use super::*;

#[derive(Debug, Clone, Copy)]
enum PresetIconButtonKind {
    Segment,
    Framed,
}

#[derive(Debug, Clone, Copy)]
enum PresetTransientMode {
    ConfirmDelete,
    Rename,
}

impl AppEditorFrame {
    pub(super) fn render_frame_close_button(
        &self,
        ui: &mut editor_host::egui::Ui,
        preset_layout: &AppEditorFramePresetLayout,
    ) -> bool {
        let close_rect = preset_layout.close_button;
        let close = ui.allocate_rect(close_rect, editor_host::egui::Sense::click());
        let (close_background, close_icon) = self.close_button_colors(close.hovered());
        ui.painter().rect_filled(close_rect, 4.0, close_background);
        let icon_rect = close_rect.shrink(6.0);
        let stroke = editor_host::egui::Stroke::new(1.5, close_icon);
        ui.painter()
            .line_segment([icon_rect.left_top(), icon_rect.right_bottom()], stroke);
        ui.painter()
            .line_segment([icon_rect.right_top(), icon_rect.left_bottom()], stroke);
        close.clicked()
    }

    pub(super) fn close_button_colors(
        &self,
        hovered: bool,
    ) -> (editor_host::egui::Color32, editor_host::egui::Color32) {
        if hovered {
            (
                self.style.close_background_hovered,
                self.style.close_icon_hovered,
            )
        } else {
            (self.style.close_background, self.style.close_icon)
        }
    }

    pub(super) fn render_frame_title(
        &self,
        ui: &mut editor_host::egui::Ui,
        state: &editor_host::EditorHostState,
        preset_layout: &AppEditorFramePresetLayout,
    ) {
        let title = ellipsize_for_width(
            &state.title,
            preset_layout.title_text.width(),
            ui_style::FONT_SIZE_UI_SM as f32,
        );
        ui.painter().text(
            preset_layout.title_text.right_center(),
            editor_host::egui::Align2::RIGHT_CENTER,
            title,
            editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_SM as f32),
            self.style.title_color,
        );
    }

    pub(super) fn zoom_command(&self, percent: u32) -> editor_host::EditorFrameCommand {
        editor_host::EditorFrameCommand::SetZoomPercent(
            percent.clamp(EDITOR_FRAME_ZOOM_MIN_PERCENT, EDITOR_FRAME_ZOOM_MAX_PERCENT),
        )
    }

    pub(super) fn render_zoom_controls(
        &self,
        ui: &mut editor_host::egui::Ui,
        state: &editor_host::EditorHostState,
        preset_layout: &AppEditorFramePresetLayout,
    ) -> Option<editor_host::EditorFrameCommand> {
        if !state.resizable {
            return None;
        }
        if preset_layout.zoom_row.left() < preset_layout.preset_row.right() + 8.0 {
            return None;
        }

        let mut percent = i32::try_from(state.zoom_percent).unwrap_or(100);
        let response = ui.scope_builder(
            editor_host::egui::UiBuilder::new().max_rect(preset_layout.zoom_row),
            |ui| {
                ui.spacing_mut().item_spacing = editor_host::egui::Vec2::ZERO;
                ui.add_sized(
                    preset_layout.zoom_row.size(),
                    editor_host::egui::DragValue::new(&mut percent)
                        .range(
                            EDITOR_FRAME_ZOOM_MIN_PERCENT as i32
                                ..=EDITOR_FRAME_ZOOM_MAX_PERCENT as i32,
                        )
                        .update_while_editing(EDITOR_FRAME_ZOOM_UPDATE_WHILE_EDITING)
                        .speed(1)
                        .suffix("%"),
                )
            },
        );
        response
            .inner
            .changed()
            .then(|| self.zoom_command(u32::try_from(percent).unwrap_or(100)))
    }

    pub(super) fn frame_rect(
        &self,
        available: editor_host::egui::Rect,
        state: &editor_host::EditorHostState,
    ) -> editor_host::egui::Rect {
        let layout = editor_host::host_layout(
            state.content_size.width,
            state.content_size.height,
            Self::chrome_height(state),
            self.frame_thickness,
        );
        editor_host::egui::Rect::from_min_size(
            available.left_top(),
            editor_host::egui::vec2(layout.outer_width as f32, layout.outer_height as f32),
        )
    }

    pub(super) fn paint_rect(&self, available: editor_host::egui::Rect) -> editor_host::egui::Rect {
        available
    }

    pub(super) fn titlebar_for_state(
        &self,
        state: &editor_host::EditorHostState,
    ) -> editor_host::egui::Rect {
        let rect = self.frame_rect(
            editor_host::egui::Rect::from_min_size(
                editor_host::egui::Pos2::ZERO,
                editor_host::egui::Vec2::ZERO,
            ),
            state,
        );
        editor_host::egui::Rect::from_min_size(
            rect.left_top()
                + editor_host::egui::vec2(self.frame_thickness as f32, self.frame_thickness as f32),
            editor_host::egui::vec2(
                rect.width() - (self.frame_thickness * 2.0) as f32,
                Self::chrome_height(state) as f32,
            ),
        )
    }

    pub(super) fn should_begin_drag_for_state(
        &self,
        pos: editor_host::egui::Pos2,
        state: &editor_host::EditorHostState,
    ) -> bool {
        let titlebar = self.titlebar_for_state(state);
        if !titlebar.contains(pos) {
            return false;
        }
        let preset_layout = self.preset_layout(titlebar);
        !Self::header_interactive_rects(&preset_layout)
            .iter()
            .any(|rect| rect.expand(2.0).contains(pos))
    }

    pub(super) fn header_interactive_rects(
        preset_layout: &AppEditorFramePresetLayout,
    ) -> [editor_host::egui::Rect; 7] {
        [
            preset_layout.close_button,
            preset_layout.zoom_row,
            preset_layout.preset_row,
            preset_layout.previous,
            preset_layout.next,
            preset_layout.name,
            preset_layout.browser,
        ]
    }

    pub(super) fn preset_menu_icon(expanded: bool) -> AppEditorFrameIcon {
        if expanded {
            AppEditorFrameIcon::ChevronDown
        } else {
            AppEditorFrameIcon::ChevronUp
        }
    }

    pub(super) fn begin_preset_rename(&mut self, id: &str, name: &str) {
        self.renaming_preset_id = Some(id.to_string());
        self.renaming_preset_value = name.to_string();
        self.rename_focus_requested = true;
        self.delete_confirmation_preset_id = None;
    }

    pub(super) fn cancel_preset_rename(&mut self) {
        self.renaming_preset_id = None;
        self.renaming_preset_value.clear();
        self.rename_focus_requested = false;
    }

    pub(super) fn request_preset_delete(
        &mut self,
        id: &str,
    ) -> Option<editor_host::EditorFrameCommand> {
        self.delete_confirmation_preset_id = Some(id.to_string());
        self.cancel_preset_rename();
        None
    }

    pub(super) fn cancel_preset_delete(&mut self) {
        self.delete_confirmation_preset_id = None;
    }

    pub(super) fn delete_confirmation_label(name: &str, width: f32) -> String {
        let fixed_width = ui_style::FONT_SIZE_UI_SM as f32 * 10.0 * 0.56;
        let name = ellipsize_for_width(
            name,
            (width - fixed_width).max(ui_style::FONT_SIZE_UI_SM as f32 * 3.0),
            ui_style::FONT_SIZE_UI_SM as f32,
        );
        format!("Remove \"{name}\"?")
    }

    pub(super) fn confirm_preset_delete(
        &mut self,
        id: &str,
    ) -> Option<editor_host::EditorFrameCommand> {
        if self.delete_confirmation_preset_id.as_deref() != Some(id) {
            return None;
        }
        self.delete_confirmation_preset_id = None;
        Some(editor_host::EditorFrameCommand::DeletePreset(
            id.to_string(),
        ))
    }

    pub(super) fn chrome_height(state: &editor_host::EditorHostState) -> f64 {
        state
            .preset
            .as_ref()
            .map_or(EDITOR_FRAME_COMPACT_CHROME_HEIGHT, |preset| {
                if preset.expanded {
                    EDITOR_FRAME_EXPANDED_CHROME_HEIGHT
                } else {
                    EDITOR_FRAME_COMPACT_CHROME_HEIGHT
                }
            })
    }

    pub(super) fn preset_layout(
        &self,
        titlebar: editor_host::egui::Rect,
    ) -> AppEditorFramePresetLayout {
        let left_inset = 8.0;
        let title_gap = 10.0;
        let title_width = 180.0;
        let button_width = 25.0;
        let title_row = editor_host::egui::Rect::from_min_size(
            titlebar.left_top(),
            editor_host::egui::vec2(titlebar.width(), EDITOR_FRAME_TITLE_ROW_HEIGHT),
        );
        let close_button = editor_host::egui::Rect::from_center_size(
            title_row.right_center() - editor_host::egui::vec2(18.0, 0.0),
            editor_host::egui::vec2(20.0, 20.0),
        );
        let zoom_width = EDITOR_FRAME_ZOOM_CONTROL_WIDTH;
        let zoom_right = close_button.left() - 8.0;
        let zoom_left = zoom_right - zoom_width;
        let zoom_row = editor_host::egui::Rect::from_center_size(
            editor_host::egui::pos2((zoom_left + zoom_right) / 2.0, title_row.center().y),
            editor_host::egui::vec2(
                EDITOR_FRAME_ZOOM_CONTROL_WIDTH,
                EDITOR_FRAME_ZOOM_CONTROL_HEIGHT,
            ),
        );
        let preset_max_width = (zoom_row.left() - title_gap - left_inset).max(80.0);
        let preset_width = (titlebar.width() - 176.0)
            .clamp(188.0, 360.0)
            .min(preset_max_width);
        let preset_row = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(titlebar.left() + left_inset, titlebar.top() + 4.0),
            editor_host::egui::vec2(preset_width, EDITOR_FRAME_PRESET_ROW_HEIGHT),
        );
        let title_text = editor_host::egui::Rect::from_min_max(
            editor_host::egui::pos2(
                (zoom_row.left() - 8.0 - title_width).max(preset_row.right() + title_gap),
                title_row.top(),
            ),
            editor_host::egui::pos2(zoom_row.left() - 8.0, title_row.bottom()),
        );
        let previous = editor_host::egui::Rect::from_min_size(
            preset_row.left_top(),
            editor_host::egui::vec2(button_width, preset_row.height()),
        );
        let next = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(preset_row.right() - button_width, preset_row.top()),
            editor_host::egui::vec2(button_width, preset_row.height()),
        );
        let name = editor_host::egui::Rect::from_min_max(
            editor_host::egui::pos2(previous.right(), preset_row.top()),
            editor_host::egui::pos2(next.left(), preset_row.bottom()),
        );
        let browser = editor_host::egui::Rect::from_min_size(
            titlebar.left_top() + editor_host::egui::vec2(8.0, EDITOR_FRAME_TITLE_ROW_HEIGHT + 6.0),
            editor_host::egui::vec2(titlebar.width() - 16.0, EDITOR_FRAME_PRESET_BROWSER_HEIGHT),
        );
        AppEditorFramePresetLayout {
            title_text,
            zoom_row,
            close_button,
            preset_row,
            previous,
            name,
            next,
            browser,
        }
    }

    pub(super) fn render_preset_strip(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        state: &editor_host::EditorHostState,
        layout: &AppEditorFramePresetLayout,
    ) -> Option<editor_host::EditorFrameCommand> {
        let preset = state.preset.as_ref()?;
        self.paint_preset_strip(ui, preset, layout);
        let controls = self.allocate_preset_strip_controls(ui, layout);
        self.paint_preset_name(ui, preset, layout, controls.name.hovered());
        self.preset_strip_command(&controls)
            .or_else(|| self.render_expanded_preset_browser(ui, preset, layout))
    }

    pub(super) fn paint_preset_strip(
        &self,
        ui: &mut editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        layout: &AppEditorFramePresetLayout,
    ) {
        ui.painter().rect_filled(
            layout.preset_row,
            6.0,
            if preset.expanded {
                self.style.control_background_active
            } else {
                self.style.control_background
            },
        );
        ui.painter().rect_stroke(
            layout.preset_row,
            6.0,
            editor_host::egui::Stroke::new(self.border_width, self.style.border_color),
            editor_host::egui::StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [layout.previous.right_top(), layout.previous.right_bottom()],
            editor_host::egui::Stroke::new(self.border_width, self.style.border_color),
        );
        ui.painter().line_segment(
            [layout.next.left_top(), layout.next.left_bottom()],
            editor_host::egui::Stroke::new(self.border_width, self.style.border_color),
        );
    }

    pub(super) fn allocate_preset_strip_controls(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        layout: &AppEditorFramePresetLayout,
    ) -> AppEditorFramePresetControls {
        let previous =
            self.preset_segment_button(ui, layout.previous, AppEditorFrameIcon::ChevronLeft);
        let next = self.preset_segment_button(ui, layout.next, AppEditorFrameIcon::ChevronRight);
        let name = ui.allocate_rect(layout.name, editor_host::egui::Sense::click());
        AppEditorFramePresetControls {
            previous,
            name,
            next,
        }
    }

    pub(super) fn paint_preset_name(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        layout: &AppEditorFramePresetLayout,
        hovered: bool,
    ) {
        if hovered {
            ui.painter().rect_filled(
                layout.name.shrink2(editor_host::egui::vec2(0.0, 1.0)),
                0.0,
                self.style.control_background_hovered,
            );
        }
        ui.painter().text(
            layout.name.left_center() + editor_host::egui::vec2(10.0, 0.0),
            editor_host::egui::Align2::LEFT_CENTER,
            ellipsize_for_width(
                &preset.current_name,
                layout.name.width() - 34.0,
                ui_style::FONT_SIZE_UI_SM as f32,
            ),
            editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_SM as f32),
            self.style.title_color,
        );
        self.paint_icon(
            ui,
            editor_host::egui::Rect::from_center_size(
                layout.name.right_center() - editor_host::egui::vec2(12.0, 0.0),
                editor_host::egui::vec2(EDITOR_FRAME_ICON_SIZE, EDITOR_FRAME_ICON_SIZE),
            ),
            Self::preset_menu_icon(preset.expanded),
            self.style.muted_text,
        );
    }

    pub(super) fn preset_strip_command(
        &self,
        controls: &AppEditorFramePresetControls,
    ) -> Option<editor_host::EditorFrameCommand> {
        if controls.name.clicked() {
            return Some(editor_host::EditorFrameCommand::TogglePresetBrowser);
        }
        if controls.previous.clicked() {
            return Some(editor_host::EditorFrameCommand::PreviousPreset);
        }
        if controls.next.clicked() {
            return Some(editor_host::EditorFrameCommand::NextPreset);
        }
        None
    }

    pub(super) fn render_expanded_preset_browser(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        layout: &AppEditorFramePresetLayout,
    ) -> Option<editor_host::EditorFrameCommand> {
        if preset.expanded {
            return self.render_preset_browser(ui, preset, layout.browser);
        }
        None
    }

    pub(super) fn preset_segment_button(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        icon: AppEditorFrameIcon,
    ) -> editor_host::egui::Response {
        self.preset_icon_response(ui, rect, icon, PresetIconButtonKind::Segment)
    }

    pub(super) fn preset_icon_button(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        icon: AppEditorFrameIcon,
    ) -> editor_host::egui::Response {
        self.preset_icon_response(ui, rect, icon, PresetIconButtonKind::Framed)
    }

    fn preset_icon_response(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        icon: AppEditorFrameIcon,
        kind: PresetIconButtonKind,
    ) -> editor_host::egui::Response {
        let response = match kind {
            PresetIconButtonKind::Segment => {
                let response = ui.allocate_rect(rect, editor_host::egui::Sense::click());
                if response.hovered() {
                    ui.painter().rect_filled(
                        rect.shrink(1.0),
                        5.0,
                        self.style.control_background_hovered,
                    );
                }
                response
            }
            PresetIconButtonKind::Framed => self.preset_button_frame(ui, rect),
        };
        let icon_rect = editor_host::egui::Rect::from_center_size(
            rect.center(),
            editor_host::egui::vec2(EDITOR_FRAME_ICON_SIZE, EDITOR_FRAME_ICON_SIZE),
        );
        self.paint_icon(ui, icon_rect, icon, self.style.close_icon);
        response
    }

    pub(super) fn paint_icon(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        icon: AppEditorFrameIcon,
        tint: editor_host::egui::Color32,
    ) {
        if let Some(texture) = self.icon_texture(ui.ctx(), icon) {
            ui.put(
                rect,
                editor_host::egui::Image::from_texture((texture.id(), rect.size())).tint(tint),
            );
        }
    }

    pub(super) fn icon_texture(
        &mut self,
        ctx: &editor_host::egui::Context,
        icon: AppEditorFrameIcon,
    ) -> Option<&editor_host::egui::TextureHandle> {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.icon_textures.entry(icon) {
            let image = render_lucide_icon(icon)?;
            let texture = ctx.load_texture(
                format!("editor-frame-icon-{icon:?}"),
                image,
                editor_host::egui::TextureOptions::LINEAR,
            );
            entry.insert(texture);
        }
        self.icon_textures.get(&icon)
    }

    pub(super) fn render_preset_browser(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        let row_height = 24.0;
        ui.painter().rect_filled(rect, 7.0, self.style.frame_color);
        ui.painter().rect_stroke(
            rect,
            7.0,
            editor_host::egui::Stroke::new(1.0, self.style.border_color),
            editor_host::egui::StrokeKind::Inside,
        );
        let save_rect = Self::preset_browser_save_rect(rect);
        let save = self.preset_icon_button(ui, save_rect, AppEditorFrameIcon::Save);
        if save.clicked() {
            return Some(editor_host::EditorFrameCommand::SavePreset);
        }
        let list_rect = Self::preset_browser_list_rect(rect);
        if preset.items.is_empty() {
            ui.painter().text(
                list_rect.center(),
                editor_host::egui::Align2::CENTER_CENTER,
                "No user presets",
                editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_XS as f32),
                self.style.close_icon,
            );
            return None;
        }

        let mut command = None;
        ui.scope_builder(
            editor_host::egui::UiBuilder::new()
                .max_rect(list_rect)
                .layout(editor_host::egui::Layout::top_down(
                    editor_host::egui::Align::Min,
                )),
            |ui| {
                editor_host::egui::ScrollArea::vertical()
                    .id_salt("editor-frame-preset-browser")
                    .auto_shrink([false, false])
                    .max_height(list_rect.height())
                    .show(ui, |ui| {
                        ui.set_width(list_rect.width());
                        for item in &preset.items {
                            if command.is_some() {
                                break;
                            }
                            command = self.render_preset_browser_item(
                                ui,
                                preset,
                                item,
                                row_height,
                                list_rect.width(),
                            );
                        }
                        ui.add_space(6.0);
                    });
            },
        );
        command
    }

    pub(super) fn preset_browser_save_rect(
        rect: editor_host::egui::Rect,
    ) -> editor_host::egui::Rect {
        editor_host::egui::Rect::from_min_size(
            rect.left_top() + editor_host::egui::vec2(6.0, 5.0),
            editor_host::egui::vec2(28.0, 22.0),
        )
    }

    pub(super) fn preset_browser_list_rect(
        rect: editor_host::egui::Rect,
    ) -> editor_host::egui::Rect {
        editor_host::egui::Rect::from_min_max(
            rect.left_top() + editor_host::egui::vec2(6.0, 34.0),
            rect.right_bottom() - editor_host::egui::vec2(6.0, 10.0),
        )
    }

    pub(super) fn render_preset_browser_item(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        item: &editor_host::EditorPresetItem,
        row_height: f32,
        row_width: f32,
    ) -> Option<editor_host::EditorFrameCommand> {
        let mode = self.preset_browser_item_mode(item);
        let (item_rect, response) =
            self.allocate_preset_browser_item(ui, mode, row_width, row_height);
        self.paint_preset_browser_item_background(ui, preset, item, mode, item_rect, &response);
        self.render_preset_browser_item_for_mode(ui, item, mode, item_rect, response)
    }

    pub(super) fn allocate_preset_browser_item(
        &self,
        ui: &mut editor_host::egui::Ui,
        mode: PresetBrowserItemMode,
        row_width: f32,
        row_height: f32,
    ) -> (editor_host::egui::Rect, editor_host::egui::Response) {
        ui.allocate_exact_size(
            editor_host::egui::vec2(row_width, row_height),
            preset_browser_item_sense(mode),
        )
    }

    pub(super) fn paint_preset_browser_item_background(
        &self,
        ui: &editor_host::egui::Ui,
        preset: &editor_host::EditorPresetState,
        item: &editor_host::EditorPresetItem,
        mode: PresetBrowserItemMode,
        item_rect: editor_host::egui::Rect,
        response: &editor_host::egui::Response,
    ) {
        if preset_browser_item_highlighted(preset, item, mode, response) {
            ui.painter()
                .rect_filled(item_rect.shrink(2.0), 5.0, self.style.titlebar_color);
        }
    }

    pub(super) fn render_preset_browser_item_for_mode(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        mode: PresetBrowserItemMode,
        item_rect: editor_host::egui::Rect,
        response: editor_host::egui::Response,
    ) -> Option<editor_host::EditorFrameCommand> {
        match mode {
            PresetBrowserItemMode::ConfirmingDelete => {
                self.render_confirming_preset_delete(ui, item, item_rect)
            }
            PresetBrowserItemMode::Renaming => self.render_renaming_preset(ui, item, item_rect),
            PresetBrowserItemMode::Normal => {
                self.render_normal_preset_item(ui, item, item_rect, response)
            }
        }
    }

    pub(super) fn preset_browser_item_mode(
        &self,
        item: &editor_host::EditorPresetItem,
    ) -> PresetBrowserItemMode {
        if self.delete_confirmation_preset_id.as_deref() == Some(item.id.as_str()) {
            PresetBrowserItemMode::ConfirmingDelete
        } else if self.renaming_preset_id.as_deref() == Some(item.id.as_str()) {
            PresetBrowserItemMode::Renaming
        } else {
            PresetBrowserItemMode::Normal
        }
    }

    pub(super) fn render_confirming_preset_delete(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        self.render_preset_transient_item(ui, item, item_rect, PresetTransientMode::ConfirmDelete)
    }

    pub(super) fn render_renaming_preset(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        self.render_preset_transient_item(ui, item, item_rect, PresetTransientMode::Rename)
    }

    fn render_preset_transient_item(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
        mode: PresetTransientMode,
    ) -> Option<editor_host::EditorFrameCommand> {
        match mode {
            PresetTransientMode::ConfirmDelete => {
                self.render_preset_delete_confirmation(ui, item, item_rect)
            }
            PresetTransientMode::Rename => self.render_preset_rename_editor(ui, item, item_rect),
        }
    }

    fn render_preset_delete_confirmation(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        self.draw_preset_delete_confirmation_label(ui, item, item_rect);
        self.handle_preset_delete_confirmation_buttons(ui, item, item_rect)
    }

    fn draw_preset_delete_confirmation_label(
        &self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) {
        ui.painter().text(
            item_rect.left_center() + editor_host::egui::vec2(8.0, 0.0),
            editor_host::egui::Align2::LEFT_CENTER,
            Self::delete_confirmation_label(&item.name, item_rect.width() - 126.0),
            editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_SM as f32),
            self.style.title_color,
        );
    }

    fn handle_preset_delete_confirmation_buttons(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        let remove_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(item_rect.right() - 112.0, item_rect.top() + 2.0),
            editor_host::egui::vec2(62.0, item_rect.height() - 4.0),
        );
        let cancel_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(item_rect.right() - 46.0, item_rect.top() + 2.0),
            editor_host::egui::vec2(42.0, item_rect.height() - 4.0),
        );
        if self.preset_text_button(ui, remove_rect, "Remove").clicked() {
            return self.confirm_preset_delete(&item.id);
        }
        if self.preset_text_button(ui, cancel_rect, "Cancel").clicked() {
            self.cancel_preset_delete();
        }
        None
    }

    fn render_preset_rename_editor(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
    ) -> Option<editor_host::EditorFrameCommand> {
        let rename_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(item_rect.right() - 56.0, item_rect.top() + 2.0),
            editor_host::egui::vec2(24.0, item_rect.height() - 4.0),
        );
        let edit_rect = editor_host::egui::Rect::from_min_max(
            item_rect.left_top() + editor_host::egui::vec2(6.0, 3.0),
            rename_rect.left_bottom() - editor_host::egui::vec2(6.0, 3.0),
        );
        let edit = ui.put(
            edit_rect,
            editor_host::egui::TextEdit::singleline(&mut self.renaming_preset_value)
                .id_salt(("preset-rename", item.id.as_str()))
                .desired_width(edit_rect.width())
                .font(editor_host::egui::FontId::proportional(
                    ui_style::FONT_SIZE_UI_SM as f32,
                )),
        );
        let focus_requested = self.rename_focus_requested;
        if focus_requested {
            edit.request_focus();
            self.rename_focus_requested = false;
        }
        self.finish_renaming_preset(ui, item, edit, focus_requested)
    }

    pub(super) fn finish_renaming_preset(
        &mut self,
        ui: &editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        edit: editor_host::egui::Response,
        focus_requested: bool,
    ) -> Option<editor_host::EditorFrameCommand> {
        if preset_rename_enter_pressed(ui) {
            return self.commit_preset_rename(item);
        }
        if preset_rename_cancelled(ui, &edit, focus_requested) {
            self.cancel_preset_rename();
        }
        None
    }

    pub(super) fn commit_preset_rename(
        &mut self,
        item: &editor_host::EditorPresetItem,
    ) -> Option<editor_host::EditorFrameCommand> {
        let name = self.renaming_preset_value.trim().to_string();
        self.cancel_preset_rename();
        preset_rename_command(item, name)
    }

    pub(super) fn render_normal_preset_item(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        item: &editor_host::EditorPresetItem,
        item_rect: editor_host::egui::Rect,
        response: editor_host::egui::Response,
    ) -> Option<editor_host::EditorFrameCommand> {
        let rename_rect = editor_host::egui::Rect::from_min_size(
            editor_host::egui::pos2(item_rect.right() - 56.0, item_rect.top() + 2.0),
            editor_host::egui::vec2(24.0, item_rect.height() - 4.0),
        );
        let delete_rect = rename_rect.translate(editor_host::egui::vec2(28.0, 0.0));
        let content_rect = editor_host::egui::Rect::from_min_max(
            item_rect.left_top(),
            editor_host::egui::pos2(rename_rect.left() - 4.0, item_rect.bottom()),
        );
        ui.painter().text(
            item_rect.left_center() + editor_host::egui::vec2(8.0, 0.0),
            editor_host::egui::Align2::LEFT_CENTER,
            ellipsize_for_width(
                &item.name,
                content_rect.width() - 12.0,
                ui_style::FONT_SIZE_UI_SM as f32,
            ),
            editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_SM as f32),
            self.style.title_color,
        );
        let rename = self.preset_icon_button(ui, rename_rect, AppEditorFrameIcon::Pencil);
        if rename.clicked() {
            self.begin_preset_rename(&item.id, &item.name);
        }
        let delete = self.preset_icon_button(ui, delete_rect, AppEditorFrameIcon::Trash);
        if delete.clicked() {
            return self.request_preset_delete(&item.id);
        }
        if response.clicked() {
            return Some(editor_host::EditorFrameCommand::LoadPreset(item.id.clone()));
        }
        None
    }

    pub(super) fn preset_text_button(
        &self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
        label: &str,
    ) -> editor_host::egui::Response {
        let response = self.preset_button_frame(ui, rect);
        ui.painter().text(
            rect.center(),
            editor_host::egui::Align2::CENTER_CENTER,
            label,
            editor_host::egui::FontId::proportional(ui_style::FONT_SIZE_UI_XS as f32),
            self.style.title_color,
        );
        response
    }

    fn preset_button_frame(
        &self,
        ui: &mut editor_host::egui::Ui,
        rect: editor_host::egui::Rect,
    ) -> editor_host::egui::Response {
        let response = ui.allocate_rect(rect, editor_host::egui::Sense::click());
        let fill = if response.hovered() {
            self.style.control_background_hovered
        } else {
            self.style.control_background
        };
        ui.painter().rect_filled(rect, 5.0, fill);
        ui.painter().rect_stroke(
            rect,
            5.0,
            editor_host::egui::Stroke::new(self.border_width, self.style.border_color),
            editor_host::egui::StrokeKind::Inside,
        );
        response
    }
}
