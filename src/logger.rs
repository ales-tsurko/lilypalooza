use iced::widget::{container, scrollable, text_editor};
use iced::{Element, Fill, Font, Shrink};

use crate::ui_style;

const MAX_LOG_LINES: usize = 2_000;

pub(crate) struct Logger {
    lines: Vec<String>,
    text: text_editor::Content,
}

impl Logger {
    pub(crate) fn new() -> Self {
        Self {
            lines: Vec::new(),
            text: text_editor::Content::new(),
        }
    }

    pub(crate) fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());

        if self.lines.len() > MAX_LOG_LINES {
            let overflow = self.lines.len() - MAX_LOG_LINES;
            self.lines.drain(0..overflow);
        }

        self.sync_text();
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.sync_text();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn last_line(&self) -> Option<&str> {
        self.lines.last().map(String::as_str)
    }

    pub(crate) fn handle_editor_action(&mut self, action: text_editor::Action) {
        if action.is_edit() {
            return;
        }

        self.text.perform(action);
    }

    pub(crate) fn view<'a, Message>(
        &'a self,
        enabled: bool,
        on_action: impl Fn(text_editor::Action) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let editor = text_editor(&self.text)
            .font(Font::MONOSPACE)
            .size(ui_style::FONT_SIZE_UI_XS)
            .padding(0)
            .height(Shrink)
            .style(ui_style::logger_text_editor);
        let editor = if enabled {
            editor.on_action(on_action)
        } else {
            editor
        };

        container(
            scrollable(editor)
                .height(Fill)
                .width(Fill)
                .style(ui_style::logger_scrollable),
        )
        .padding([ui_style::PADDING_XS, ui_style::PADDING_STATUS_BAR_H])
        .width(Fill)
        .height(Fill)
        .style(ui_style::pane_logger_surface)
        .into()
    }

    fn sync_text(&mut self) {
        let content = self.lines.join("\n");
        self.text = text_editor::Content::with_text(&content);
    }
}
