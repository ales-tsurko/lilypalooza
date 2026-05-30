use super::*;

pub(super) const EDITOR_FRAME_THICKNESS: f64 = 2.0;
pub(super) const EDITOR_FRAME_BORDER_WIDTH: f32 = 0.5;
pub(super) const EDITOR_FRAME_COMPACT_CHROME_HEIGHT: f64 = 34.0;
pub(super) const EDITOR_FRAME_EXPANDED_CHROME_HEIGHT: f64 = 160.0;
pub(super) const EDITOR_FRAME_TITLE_ROW_HEIGHT: f32 = 32.0;
pub(super) const EDITOR_FRAME_PRESET_ROW_HEIGHT: f32 = 24.0;
pub(super) const EDITOR_FRAME_PRESET_BROWSER_HEIGHT: f32 = 116.0;
pub(super) const EDITOR_FRAME_ICON_SIZE: f32 = 13.0;
pub(super) const EDITOR_FRAME_ZOOM_CONTROL_WIDTH: f32 = 68.0;
pub(super) const EDITOR_FRAME_ZOOM_CONTROL_HEIGHT: f32 = 22.0;
pub(super) const EDITOR_FRAME_VIEW_TOGGLE_WIDTH: f32 = 88.0;
pub(super) const EDITOR_FRAME_VIEW_TOGGLE_HEIGHT: f32 = 22.0;
pub(in crate::app) const EDITOR_FRAME_ZOOM_MIN_PERCENT: u32 = 50;
pub(in crate::app) const EDITOR_FRAME_ZOOM_MAX_PERCENT: u32 = 200;
pub(super) const EDITOR_FRAME_ZOOM_UPDATE_WHILE_EDITING: bool = false;
pub(super) const EGUI_ICON_CHEVRON_LEFT: &[u8] =
    include_bytes!("../../../assets/icons/chevron-left.svg");
pub(super) const EGUI_ICON_CHEVRON_RIGHT: &[u8] =
    include_bytes!("../../../assets/icons/chevron-right.svg");
pub(super) const EGUI_ICON_CHEVRON_DOWN: &[u8] =
    include_bytes!("../../../assets/icons/chevron-down.svg");
pub(super) const EGUI_ICON_CHEVRON_UP: &[u8] =
    include_bytes!("../../../assets/icons/chevron-up.svg");
pub(super) const EGUI_ICON_PENCIL: &[u8] = include_bytes!("../../../assets/icons/pencil.svg");
pub(super) const EGUI_ICON_SAVE: &[u8] = include_bytes!("../../../assets/icons/save.svg");
pub(super) const EGUI_ICON_SLIDERS: &[u8] =
    include_bytes!("../../../assets/icons/sliders-horizontal.svg");
pub(super) const EGUI_ICON_TRASH: &[u8] = include_bytes!("../../../assets/icons/trash-2.svg");

#[derive(Clone)]
pub(in crate::app) struct AppEditorFrame {
    pub(super) titlebar_height: f64,
    pub(super) frame_thickness: f64,
    pub(super) border_width: f32,
    pub(super) renaming_preset_id: Option<String>,
    pub(super) renaming_preset_value: String,
    pub(super) rename_focus_requested: bool,
    pub(super) delete_confirmation_preset_id: Option<String>,
    pub(super) native_editor_available: bool,
    pub(super) controls_visible: Arc<AtomicBool>,
    pub(super) generic_controls: Option<GenericControllerEditor>,
    pub(super) icon_textures: HashMap<AppEditorFrameIcon, editor_host::egui::TextureHandle>,
    pub(super) style: AppEditorFrameStyle,
}

impl std::fmt::Debug for AppEditorFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppEditorFrame")
            .field("titlebar_height", &self.titlebar_height)
            .field("frame_thickness", &self.frame_thickness)
            .field("border_width", &self.border_width)
            .field("renaming_preset_id", &self.renaming_preset_id)
            .field("renaming_preset_value", &self.renaming_preset_value)
            .field("rename_focus_requested", &self.rename_focus_requested)
            .field(
                "delete_confirmation_preset_id",
                &self.delete_confirmation_preset_id,
            )
            .field("native_editor_available", &self.native_editor_available)
            .field(
                "controls_visible",
                &self.controls_visible.load(Ordering::Relaxed),
            )
            .field("style", &self.style)
            .finish()
    }
}

impl Default for AppEditorFrame {
    fn default() -> Self {
        Self::from_theme(&iced::Theme::Dark)
    }
}

impl AppEditorFrame {
    pub(in crate::app) fn from_theme(theme: &iced::Theme) -> Self {
        Self {
            titlebar_height: EDITOR_FRAME_COMPACT_CHROME_HEIGHT,
            frame_thickness: EDITOR_FRAME_THICKNESS,
            border_width: EDITOR_FRAME_BORDER_WIDTH,
            renaming_preset_id: None,
            renaming_preset_value: String::new(),
            rename_focus_requested: false,
            delete_confirmation_preset_id: None,
            native_editor_available: true,
            controls_visible: Arc::new(AtomicBool::new(false)),
            generic_controls: None,
            icon_textures: HashMap::new(),
            style: AppEditorFrameStyle::from_theme(theme),
        }
    }

    pub(in crate::app) fn with_generic_controls(
        mut self,
        native_editor_available: bool,
        controls_visible: Arc<AtomicBool>,
        generic_controls: GenericControllerEditor,
    ) -> Self {
        self.native_editor_available = native_editor_available;
        self.controls_visible = controls_visible;
        self.generic_controls = Some(generic_controls);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct AppEditorFramePresetLayout {
    pub(super) title_text: editor_host::egui::Rect,
    pub(super) zoom_row: editor_host::egui::Rect,
    pub(super) close_button: editor_host::egui::Rect,
    pub(super) view_toggle: editor_host::egui::Rect,
    pub(super) preset_row: editor_host::egui::Rect,
    pub(super) previous: editor_host::egui::Rect,
    pub(super) name: editor_host::egui::Rect,
    pub(super) next: editor_host::egui::Rect,
    pub(super) browser: editor_host::egui::Rect,
}

pub(in crate::app) struct AppEditorFramePresetControls {
    pub(super) previous: editor_host::egui::Response,
    pub(super) name: editor_host::egui::Response,
    pub(super) next: editor_host::egui::Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct AppEditorFrameStyle {
    pub(in crate::app) frame_color: editor_host::egui::Color32,
    pub(in crate::app) titlebar_color: editor_host::egui::Color32,
    pub(in crate::app) border_color: editor_host::egui::Color32,
    pub(in crate::app) title_color: editor_host::egui::Color32,
    pub(in crate::app) close_background: editor_host::egui::Color32,
    pub(in crate::app) close_background_hovered: editor_host::egui::Color32,
    pub(in crate::app) close_icon: editor_host::egui::Color32,
    pub(in crate::app) close_icon_hovered: editor_host::egui::Color32,
    pub(in crate::app) control_background: editor_host::egui::Color32,
    pub(in crate::app) control_background_hovered: editor_host::egui::Color32,
    pub(in crate::app) control_background_active: editor_host::egui::Color32,
    pub(in crate::app) muted_text: editor_host::egui::Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PresetBrowserItemMode {
    Normal,
    Renaming,
    ConfirmingDelete,
}

pub(super) fn preset_browser_item_sense(mode: PresetBrowserItemMode) -> editor_host::egui::Sense {
    if mode == PresetBrowserItemMode::Normal {
        editor_host::egui::Sense::click()
    } else {
        editor_host::egui::Sense::hover()
    }
}

pub(super) fn preset_browser_item_highlighted(
    preset: &editor_host::EditorPresetState,
    item: &editor_host::EditorPresetItem,
    mode: PresetBrowserItemMode,
    response: &editor_host::egui::Response,
) -> bool {
    response.hovered()
        || preset.selected_id.as_deref() == Some(item.id.as_str())
        || mode != PresetBrowserItemMode::Normal
}

impl AppEditorFrameStyle {
    pub(in crate::app) fn from_theme(theme: &iced::Theme) -> Self {
        let palette = theme.extended_palette();
        let titlebar = mix_iced_color(palette.background.weak.color, Color::WHITE, 0.04);
        let border = mix_iced_color(
            palette.background.weak.color,
            palette.background.strong.color,
            0.40,
        );
        let close_icon = mix_iced_color(
            palette.background.strong.text,
            palette.background.weak.color,
            0.12,
        );
        let control = mix_iced_color(
            palette.background.base.color,
            palette.background.weak.color,
            0.50,
        );
        let control_hovered = mix_iced_color(control, palette.primary.weak.color, 0.20);
        let control_active = mix_iced_color(control, palette.primary.weak.color, 0.32);

        Self {
            frame_color: egui_color(palette.background.base.color),
            titlebar_color: egui_color(titlebar),
            border_color: egui_color(border),
            title_color: egui_color(palette.background.base.text),
            close_background: editor_host::egui::Color32::TRANSPARENT,
            close_background_hovered: egui_color(palette.background.base.color),
            close_icon: egui_color(close_icon),
            close_icon_hovered: egui_color(palette.background.base.text),
            control_background: egui_color(control),
            control_background_hovered: egui_color(control_hovered),
            control_background_active: egui_color(control_active),
            muted_text: egui_color(palette.background.weak.text),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AppEditorFrameIcon {
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Pencil,
    Save,
    Sliders,
    Trash,
}

impl AppEditorFrameIcon {
    pub(super) fn svg_bytes(self) -> &'static [u8] {
        match self {
            Self::ChevronLeft => EGUI_ICON_CHEVRON_LEFT,
            Self::ChevronRight => EGUI_ICON_CHEVRON_RIGHT,
            Self::ChevronDown => EGUI_ICON_CHEVRON_DOWN,
            Self::ChevronUp => EGUI_ICON_CHEVRON_UP,
            Self::Pencil => EGUI_ICON_PENCIL,
            Self::Save => EGUI_ICON_SAVE,
            Self::Sliders => EGUI_ICON_SLIDERS,
            Self::Trash => EGUI_ICON_TRASH,
        }
    }
}

impl editor_host::EditorFrame for AppEditorFrame {
    fn layout(&self, content_size: editor_host::Size) -> editor_host::EditorFrameLayout {
        editor_host::host_layout(
            content_size.width,
            content_size.height,
            self.titlebar_height,
            self.frame_thickness,
        )
    }

    fn should_begin_window_drag(
        &self,
        pos: editor_host::egui::Pos2,
        state: &editor_host::EditorHostState,
    ) -> bool {
        self.should_begin_drag_for_state(pos, state)
    }

    fn render(
        &mut self,
        ui: &mut editor_host::egui::Ui,
        state: &editor_host::EditorHostState,
    ) -> editor_host::EditorFrameAction {
        let live_rect = ui.max_rect();
        let rect = self.paint_rect(live_rect);
        let titlebar = editor_host::egui::Rect::from_min_size(
            rect.left_top()
                + editor_host::egui::vec2(self.frame_thickness as f32, self.frame_thickness as f32),
            editor_host::egui::vec2(
                rect.width() - (self.frame_thickness * 2.0) as f32,
                Self::chrome_height(state) as f32,
            ),
        );
        ui.painter().rect_filled(rect, 0.0, self.style.frame_color);
        ui.painter().rect_stroke(
            rect.shrink(self.border_width / 2.0),
            0.0,
            editor_host::egui::Stroke::new(self.border_width, self.style.border_color),
            editor_host::egui::StrokeKind::Inside,
        );
        ui.painter()
            .rect_filled(titlebar, 0.0, self.style.titlebar_color);

        let preset_layout = self.preset_layout_for_state(titlebar, state);
        let close_clicked = self.render_frame_close_button(ui, &preset_layout);
        self.render_frame_title(ui, state, &preset_layout);
        let zoom_command = self.render_zoom_controls(ui, state, &preset_layout);
        let view_command = self.render_view_toggle(ui, &preset_layout);
        let preset_command = self.render_preset_strip(ui, state, &preset_layout);
        self.render_generic_controls(ui, rect, state);
        editor_frame_action(close_clicked, zoom_command, view_command, preset_command)
    }
}

pub(super) fn editor_frame_action(
    close_clicked: bool,
    zoom_command: Option<editor_host::EditorFrameCommand>,
    view_command: Option<editor_host::EditorFrameCommand>,
    preset_command: Option<editor_host::EditorFrameCommand>,
) -> editor_host::EditorFrameAction {
    if close_clicked {
        return editor_host::EditorFrameAction::Close;
    }
    if let Some(command) = zoom_command {
        return editor_host::EditorFrameAction::Command(command);
    }
    if let Some(command) = view_command {
        return editor_host::EditorFrameAction::Command(command);
    }
    if let Some(command) = preset_command {
        return editor_host::EditorFrameAction::Command(command);
    }
    editor_host::EditorFrameAction::None
}
