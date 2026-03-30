use iced::widget::{container, scrollable, text_editor};
use iced::{Element, Fill, Font};

use crate::ui_style;

#[derive(Debug, Default)]
pub(super) struct EditorState {
    content: text_editor::Content,
}

impl EditorState {
    pub(super) fn new() -> Self {
        Self {
            content: text_editor::Content::new(),
        }
    }

    pub(super) fn handle_action(&mut self, action: text_editor::Action) {
        self.content.perform(action);
    }
}

pub(super) fn content<'a, Message>(
    state: &'a EditorState,
    on_action: impl Fn(text_editor::Action) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let editor = text_editor(&state.content)
        .placeholder("Editor")
        .on_action(on_action)
        .font(Font::MONOSPACE)
        .size(ui_style::FONT_SIZE_BODY_SM)
        .padding(ui_style::PADDING_XS)
        .style(ui_style::editor_text_editor);

    container(
        scrollable(editor)
            .height(Fill)
            .width(Fill)
            .style(ui_style::workspace_scrollable),
    )
    .width(Fill)
    .height(Fill)
    .style(ui_style::pane_main_surface)
    .into()
}
