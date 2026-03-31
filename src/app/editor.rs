use std::fs;
use std::path::{Path, PathBuf};

use iced::widget::{container, text};
use iced::{Element, Fill};
use iced_code_editor::{CodeEditor, Message as EditorWidgetMessage, theme::ThemeTuning};

use crate::settings::EditorThemeSettings;
use crate::ui_style;

const EMPTY_EDITOR_MESSAGE: &str = "Open a LilyPond score to edit its source here.";

pub(super) struct EditorState {
    widget: CodeEditor,
    path: Option<PathBuf>,
    theme_settings: EditorThemeSettings,
}

impl EditorState {
    pub(super) fn new(theme_settings: EditorThemeSettings) -> Self {
        Self {
            widget: build_editor("", "text", theme_settings),
            path: None,
            theme_settings,
        }
    }

    pub(super) fn update(
        &mut self,
        message: &EditorWidgetMessage,
    ) -> iced::Task<EditorWidgetMessage> {
        self.widget.update(message)
    }

    pub(super) fn load_file(&mut self, path: &Path) -> Result<(), String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("Failed to read editor file {}: {error}", path.display()))?;

        self.widget = build_editor(&text, syntax_for_path(path), self.theme_settings);
        self.widget.mark_saved();
        self.path = Some(path.to_path_buf());

        Ok(())
    }

    pub(super) fn reload_from_disk(&mut self) -> Result<(), String> {
        let Some(path) = self.path.clone() else {
            return Err("No editor file is currently loaded".to_string());
        };

        self.load_file(&path)
    }

    pub(super) fn save_to_disk(&mut self) -> Result<PathBuf, String> {
        let Some(path) = self.path.clone() else {
            return Err("No editor file is currently loaded".to_string());
        };

        fs::write(&path, self.widget.content())
            .map_err(|error| format!("Failed to save editor file {}: {error}", path.display()))?;
        self.widget.mark_saved();

        Ok(path)
    }

    pub(super) fn has_document(&self) -> bool {
        self.path.is_some()
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.widget.is_modified()
    }

    pub(super) fn file_name(&self) -> Option<&str> {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|file_name| file_name.to_str())
    }

    pub(super) fn theme_settings(&self) -> EditorThemeSettings {
        self.theme_settings
    }

    pub(super) fn set_hue_offset_degrees(&mut self, value: f32) {
        self.theme_settings.hue_offset_degrees = value;
        self.apply_theme();
    }

    pub(super) fn set_saturation(&mut self, value: f32) {
        self.theme_settings.saturation = value;
        self.apply_theme();
    }

    pub(super) fn set_contrast(&mut self, value: f32) {
        self.theme_settings.contrast = value;
        self.apply_theme();
    }

    pub(super) fn set_text_dim(&mut self, value: f32) {
        self.theme_settings.text_dim = value;
        self.apply_theme();
    }

    pub(super) fn set_comment_dim(&mut self, value: f32) {
        self.theme_settings.comment_dim = value;
        self.apply_theme();
    }

    pub(super) fn view<'a, Message>(
        &'a self,
        map_message: impl Fn(EditorWidgetMessage) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        if !self.has_document() {
            return container(text(EMPTY_EDITOR_MESSAGE).size(ui_style::FONT_SIZE_BODY_MD))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill)
                .into();
        }

        container(self.widget.view().map(map_message))
            .width(Fill)
            .height(Fill)
            .style(ui_style::pane_main_surface)
            .into()
    }

    fn apply_theme(&mut self) {
        self.widget
            .set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
                &iced::Theme::Dark,
                to_editor_theme_tuning(self.theme_settings),
            ));
    }
}

fn build_editor(content: &str, syntax: &str, theme_settings: EditorThemeSettings) -> CodeEditor {
    let mut editor = CodeEditor::new(content, syntax).with_wrap_enabled(false);
    editor.set_font_size(ui_style::FONT_SIZE_BODY_SM.saturating_sub(2) as f32, true);
    editor.set_lsp_enabled(false);
    editor.set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
        &iced::Theme::Dark,
        to_editor_theme_tuning(theme_settings),
    ));
    editor
}

fn to_editor_theme_tuning(settings: EditorThemeSettings) -> ThemeTuning {
    ThemeTuning {
        hue_offset_degrees: settings.hue_offset_degrees,
        saturation: settings.saturation,
        contrast: settings.contrast,
        text_dim: settings.text_dim,
        comment_dim: settings.comment_dim,
    }
}

fn syntax_for_path(path: &Path) -> &str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("ly" | "ily") => "lilypond",
        Some("scm") => "scheme",
        Some(extension) => extension,
        None => "text",
    }
}
