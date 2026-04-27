use std::cell::OnceCell;

use iced::widget::{container, scrollable, text_editor};
use iced::{Element, Fill, Shrink};

use crate::{fonts, ui_style};

const MAX_LOG_LINES: usize = 2_000;

pub(crate) struct Logger {
    lines: Vec<String>,
    text: OnceCell<text_editor::Content>,
}

impl Logger {
    pub(crate) fn new() -> Self {
        Self {
            lines: Vec::new(),
            text: OnceCell::new(),
        }
    }

    pub(crate) fn push(&mut self, line: impl Into<String>) {
        self.push_raw(line.into());
        self.sync_text();
    }

    pub(crate) fn extend(&mut self, lines: impl IntoIterator<Item = String>) {
        let mut changed = false;
        for line in lines {
            self.push_raw(line);
            changed = true;
        }

        if changed {
            self.sync_text();
        }
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

        self.ensure_text_mut().perform(action);
    }

    pub(crate) fn view<'a, Message>(
        &'a self,
        enabled: bool,
        on_action: impl Fn(text_editor::Action) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let editor = text_editor(self.text())
            .font(fonts::MONO)
            .size(ui_style::FONT_SIZE_UI_SM)
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
        if let Some(text) = self.text.get_mut() {
            *text = text_editor::Content::with_text(&content);
        }
    }

    fn text(&self) -> &text_editor::Content {
        self.text
            .get_or_init(|| text_editor::Content::with_text(&self.lines.join("\n")))
    }

    fn ensure_text_mut(&mut self) -> &mut text_editor::Content {
        if self.text.get().is_none() {
            let content = text_editor::Content::with_text(&self.lines.join("\n"));
            let _ = self.text.set(content);
        }
        self.text
            .get_mut()
            .expect("logger text should be initialized")
    }

    fn push_raw(&mut self, line: String) {
        self.lines.push(line);

        if self.lines.len() > MAX_LOG_LINES {
            let overflow = self.lines.len() - MAX_LOG_LINES;
            self.lines.drain(0..overflow);
        }
    }
}
