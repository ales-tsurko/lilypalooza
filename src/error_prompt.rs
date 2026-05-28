use iced::{
    Color,
    Element,
    Fill,
    Length,
    Theme,
    alignment,
    widget::{button, column, container, opaque, row, svg, text},
};

use crate::{fonts, icons, ui_style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorFatality {
    Critical,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptTone {
    Error,
    Warning,
    #[expect(dead_code, reason = "Reserved for future informational prompts")]
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptButtons {
    Ok,
    OkCancel,
    SaveDiscardCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptSelectedButton {
    Ok,
    Discard,
    Cancel,
}

#[derive(Debug, Clone)]
pub(crate) struct ErrorPrompt {
    title: String,
    message: String,
    fatality: ErrorFatality,
    tone: PromptTone,
    buttons: PromptButtons,
    ok_label: Option<String>,
    discard_label: Option<String>,
    cancel_label: Option<String>,
}

impl ErrorPrompt {
    pub(crate) fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        fatality: ErrorFatality,
        buttons: PromptButtons,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            fatality,
            tone: match fatality {
                ErrorFatality::Critical => PromptTone::Error,
                ErrorFatality::Recoverable => PromptTone::Warning,
            },
            buttons,
            ok_label: None,
            discard_label: None,
            cancel_label: None,
        }
    }

    pub(crate) fn with_ok_label(mut self, label: impl Into<String>) -> Self {
        self.ok_label = Some(label.into());
        self
    }

    pub(crate) fn with_discard_label(mut self, label: impl Into<String>) -> Self {
        self.discard_label = Some(label.into());
        self
    }

    pub(crate) fn with_cancel_label(mut self, label: impl Into<String>) -> Self {
        self.cancel_label = Some(label.into());
        self
    }

    pub(crate) fn buttons(&self) -> PromptButtons {
        self.buttons
    }

    #[cfg(test)]
    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn overlay_ok<'a, Message>(
        &'a self,
        selected: PromptSelectedButton,
        on_ok: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay(selected, on_ok, None)
    }

    pub(crate) fn overlay_ok_cancel<'a, Message>(
        &'a self,
        selected: PromptSelectedButton,
        on_ok: Message,
        on_cancel: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        self.overlay(selected, on_ok, Some(on_cancel))
    }

    pub(crate) fn overlay_save_discard_cancel<'a, Message>(
        &'a self,
        selected: PromptSelectedButton,
        on_save: Message,
        on_discard: Message,
        on_cancel: Message,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let save_label = self.ok_label.as_deref().unwrap_or("Save");
        let discard_label = self.discard_label.as_deref().unwrap_or("Discard");
        let cancel_label = self.cancel_label.as_deref().unwrap_or("Cancel");
        self.overlay_with_actions(PromptActions {
            selected,
            ok: Some((save_label.to_string(), on_save)),
            discard: Some((discard_label.to_string(), on_discard)),
            cancel: Some((cancel_label.to_string(), on_cancel)),
        })
    }

    fn overlay<'a, Message>(
        &'a self,
        selected: PromptSelectedButton,
        on_ok: Message,
        on_cancel: Option<Message>,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let ok_label = self.ok_label.as_deref().unwrap_or("OK");
        let cancel_label = self.cancel_label.as_deref().unwrap_or("Cancel");
        self.overlay_with_actions(PromptActions {
            selected,
            ok: Some((ok_label.to_string(), on_ok.clone())),
            discard: None,
            cancel: on_cancel.map(|message| (cancel_label.to_string(), message)),
        })
    }

    fn overlay_with_actions<'a, Message>(
        &'a self,
        actions: PromptActions<Message>,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let is_critical = matches!(self.fatality, ErrorFatality::Critical);
        let critical_info = "The app cannot continue and will close after you press OK";

        let error_details = container(
            text(&self.message)
                .size(ui_style::FONT_SIZE_UI_SM)
                .font(fonts::UI),
        )
        .width(Fill)
        .style(move |theme: &Theme| ui_style::prompt_message(theme, is_critical));

        let action_row = self.prompt_action_row(actions, is_critical);

        let actions = container(action_row)
            .width(Fill)
            .align_x(alignment::Horizontal::Right);

        let (title_icon, title_color) = match self.tone {
            PromptTone::Error => (icons::circle_x(), Color::from_rgb(0.84, 0.36, 0.37)),
            PromptTone::Warning => (icons::circle_alert(), Color::from_rgb(0.86, 0.58, 0.23)),
            PromptTone::Info => (icons::info(), Color::from_rgb(0.33, 0.56, 0.86)),
        };

        let title = container(
            row![
                svg(title_icon)
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0))
                    .style(move |_: &Theme, _status| svg::Style {
                        color: Some(title_color)
                    }),
                text(&self.title)
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..fonts::UI
                    })
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
        .style(ui_style::prompt_header);

        let mut content = column![error_details];

        if is_critical {
            content = content.push(
                text(critical_info)
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::UI),
            );
        }

        let body = container(content.push(actions).spacing(ui_style::SPACE_MD))
            .width(Fill)
            .padding(ui_style::PADDING_SM);

        let dialog = container(column![title, body].spacing(0))
            .width(ui_style::SIZE_SURFACE_LG)
            .style(ui_style::prompt_dialog);

        let centered_dialog = container(dialog)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill);

        let backdrop = container(centered_dialog)
            .width(Fill)
            .height(Fill)
            .style(ui_style::prompt_backdrop);

        opaque(backdrop)
    }

    fn prompt_action_row<'a, Message>(
        &self,
        actions: PromptActions<Message>,
        is_critical: bool,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        match self.buttons {
            PromptButtons::Ok => prompt_action_buttons(
                actions
                    .ok
                    .map(|ok| {
                        vec![prompt_action_spec(
                            ok,
                            critical_ok_button_style(is_critical),
                            actions.selected == PromptSelectedButton::Ok,
                        )]
                    })
                    .unwrap_or_default(),
            ),
            PromptButtons::OkCancel => {
                let (Some(cancel), Some(ok)) = (actions.cancel, actions.ok) else {
                    return empty_prompt_action_row();
                };
                prompt_action_buttons(vec![
                    prompt_action_spec(
                        cancel,
                        ui_style::button_neutral,
                        actions.selected == PromptSelectedButton::Cancel,
                    ),
                    prompt_action_spec(
                        ok,
                        critical_ok_button_style(is_critical),
                        actions.selected == PromptSelectedButton::Ok,
                    ),
                ])
            }
            PromptButtons::SaveDiscardCancel => save_discard_cancel_prompt_action_row(actions),
        }
    }
}

#[derive(Clone)]
struct PromptActions<Message> {
    selected: PromptSelectedButton,
    ok: Option<(String, Message)>,
    discard: Option<(String, Message)>,
    cancel: Option<(String, Message)>,
}

fn save_discard_cancel_prompt_action_row<'a, Message>(
    actions: PromptActions<Message>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let (Some(cancel), Some(discard), Some(save)) = (actions.cancel, actions.discard, actions.ok)
    else {
        return empty_prompt_action_row();
    };

    prompt_action_buttons([
        PromptActionButton {
            label: cancel.0,
            message: cancel.1,
            style: ui_style::button_neutral,
            selected: actions.selected == PromptSelectedButton::Cancel,
        },
        PromptActionButton {
            label: discard.0,
            message: discard.1,
            style: ui_style::button_neutral,
            selected: actions.selected == PromptSelectedButton::Discard,
        },
        PromptActionButton {
            label: save.0,
            message: save.1,
            style: ui_style::button_active,
            selected: actions.selected == PromptSelectedButton::Ok,
        },
    ])
}

struct PromptActionButton<Message> {
    label: String,
    message: Message,
    style: fn(&Theme, button::Status) -> button::Style,
    selected: bool,
}

fn prompt_action_spec<Message>(
    action: (String, Message),
    style: fn(&Theme, button::Status) -> button::Style,
    selected: bool,
) -> PromptActionButton<Message> {
    PromptActionButton {
        label: action.0,
        message: action.1,
        style,
        selected,
    }
}

fn prompt_action_buttons<'a, Message>(
    buttons: impl IntoIterator<Item = PromptActionButton<Message>>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    buttons
        .into_iter()
        .fold(row![].spacing(ui_style::SPACE_SM), |row, button| {
            row.push(prompt_action_button(
                button.label,
                button.message,
                button.style,
                button.selected,
            ))
        })
        .into()
}

fn prompt_action_button<'a, Message>(
    label: String,
    message: Message,
    style: fn(&Theme, button::Status) -> button::Style,
    selected: bool,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    button(text(label).size(ui_style::FONT_SIZE_UI_SM))
        .style(prompt_button_style(style, selected))
        .padding([ui_style::PADDING_BUTTON_V, ui_style::PADDING_BUTTON_H])
        .on_press(message)
        .into()
}

fn empty_prompt_action_row<'a, Message>() -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    row![].spacing(ui_style::SPACE_SM).into()
}

fn critical_ok_button_style(is_critical: bool) -> fn(&Theme, button::Status) -> button::Style {
    if is_critical {
        ui_style::button_danger
    } else {
        ui_style::button_active
    }
}

fn prompt_button_style(
    base: fn(&Theme, button::Status) -> button::Style,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let effective_status = if selected && matches!(status, button::Status::Active) {
            button::Status::Pressed
        } else {
            status
        };
        base(theme, effective_status)
    }
}
