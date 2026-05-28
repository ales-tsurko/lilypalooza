use super::*;

pub(super) struct ProcessorSlotControlArgs<'a> {
    pub(super) target: crate::app::processor_editor_windows::EditorTarget,
    pub(super) role: ProcessorSlotRole,
    pub(super) selected: Option<&'a ProcessorChoice>,
    pub(super) editor_enabled: bool,
    pub(super) bypassed: bool,
    pub(super) hovered_segment: Option<ProcessorSlotSegment>,
    pub(super) controls_enabled: bool,
    pub(super) slot_width: f32,
    pub(super) list_item: bool,
    pub(super) label_override: Option<String>,
}

pub(super) fn processor_slot_controls_sized(
    args: ProcessorSlotControlArgs<'_>,
) -> Element<'static, Message> {
    let ProcessorSlotControlArgs {
        target,
        role,
        selected,
        editor_enabled,
        bypassed,
        hovered_segment,
        controls_enabled,
        slot_width,
        list_item,
        label_override,
    } = args;
    let picker_action =
        controls_enabled.then_some(Message::Mixer(MixerMessage::ToggleProcessorBrowser(target)));
    let editor_action =
        processor_slot_editor_action(target, selected, editor_enabled, controls_enabled);
    let bypass_action =
        (controls_enabled && role == ProcessorSlotRole::Effect && !is_empty_choice(selected))
            .then_some(Message::Mixer(MixerMessage::ToggleSlotBypass(target)));
    let can_drag_effect = controls_enabled
        && role == ProcessorSlotRole::Effect
        && target.slot_index > 0
        && !is_empty_choice(selected);
    let label = if list_item && is_empty_choice(selected) {
        "Add effect".to_string()
    } else if let Some(label) = label_override {
        processor_hover_label(&label, list_item)
    } else {
        processor_hover_label(&processor_trigger_label(selected), list_item)
    };
    let bypass_hovered = hovered_segment == Some(ProcessorSlotSegment::Bypass);
    let editor_hovered = hovered_segment == Some(ProcessorSlotSegment::Editor);
    let picker_hovered = hovered_segment == Some(ProcessorSlotSegment::Picker);
    let label_active = role == ProcessorSlotRole::Instrument && !is_empty_choice(selected);

    let mut split = row![]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center);
    if role == ProcessorSlotRole::Effect {
        split = split.push(processor_slot_segment_with_icon(
            processor_slot_bypass_icon(bypassed),
            ProcessorSlotContent::IconOnly,
            false,
            bypass_hovered,
            bypass_action,
            target,
            ProcessorSlotSegment::Bypass,
        ));
        split = split.push(slot_area_separator());
    }
    split = split.push(processor_slot_segment_with_icon(
        role.slot_icon(),
        ProcessorSlotContent::Label(label),
        label_active,
        editor_hovered,
        editor_action.or_else(|| {
            if is_empty_choice(selected) {
                picker_action.clone()
            } else {
                None
            }
        }),
        target,
        ProcessorSlotSegment::Editor,
    ));
    split = split.push(slot_area_separator());
    split = split.push(processor_slot_segment_with_icon(
        icons::list_tree(),
        ProcessorSlotContent::IconOnly,
        false,
        picker_hovered,
        picker_action.clone(),
        target,
        ProcessorSlotSegment::Picker,
    ));
    let content: Element<'static, Message> = split.into();

    let button_content = container(content)
        .width(Fill)
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .center_y(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT));
    let surface: Element<'static, Message> = button_content
        .style(move |theme| processor_slot_surface(theme, false, list_item))
        .padding([0, ui_style::grid(2)])
        .width(Length::Fixed(slot_width))
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .into();

    let mut hit_area = mouse_area(surface)
        .on_right_press(picker_action.unwrap_or_else(noop_message))
        .interaction(mouse::Interaction::Pointer);
    if can_drag_effect {
        hit_area = hit_area
            .on_press(Message::Mixer(MixerMessage::StartTrackEffectDrag {
                strip_index: target.strip_index,
                effect_index: target.slot_index - 1,
            }))
            .on_release(Message::Mixer(MixerMessage::DropTrackEffect {
                strip_index: target.strip_index,
                effect_index: target.slot_index - 1,
            }));
    }

    container(hit_area)
        .width(Length::Fixed(slot_width))
        .height(Length::Fixed(if list_item {
            PROCESSOR_SLOT_BUTTON_HEIGHT
        } else {
            PROCESSOR_SLOT_HEIGHT
        }))
        .center_x(Length::Fixed(slot_width))
        .center_y(Length::Fixed(if list_item {
            PROCESSOR_SLOT_BUTTON_HEIGHT
        } else {
            PROCESSOR_SLOT_HEIGHT
        }))
        .into()
}

pub(super) fn processor_hover_label(label: &str, _list_item: bool) -> String {
    crate::track_names::ellipsize_middle(label, PROCESSOR_SLOT_LABEL_MAX_LEN)
}

enum ProcessorSlotContent {
    Label(String),
    IconOnly,
}

fn processor_slot_segment_with_icon(
    icon: iced::widget::svg::Handle,
    content: ProcessorSlotContent,
    active: bool,
    slot_hovered: bool,
    action: Option<Message>,
    target: crate::app::processor_editor_windows::EditorTarget,
    segment: ProcessorSlotSegment,
) -> Element<'static, Message> {
    let (content, width, style) = match content {
        ProcessorSlotContent::Label(label) => (
            processor_slot_label_content(icon, label, active, slot_hovered),
            Fill,
            ProcessorSlotButtonStyle::Label,
        ),
        ProcessorSlotContent::IconOnly => (
            processor_slot_icon_container(icon, move |theme, status| {
                processor_slot_active_icon_style(theme, status, active, slot_hovered)
            }),
            Length::Fixed(PROCESSOR_SLOT_SEGMENT_WIDTH),
            ProcessorSlotButtonStyle::Icon,
        ),
    };
    processor_slot_segment(
        content,
        width,
        move |theme, status| style.apply(theme, status, active, slot_hovered),
        action,
        target,
        segment,
    )
}

#[derive(Debug, Clone, Copy)]
enum ProcessorSlotButtonStyle {
    Label,
    Icon,
}

impl ProcessorSlotButtonStyle {
    fn apply(
        self,
        theme: &iced::Theme,
        status: iced::widget::button::Status,
        active: bool,
        slot_hovered: bool,
    ) -> iced::widget::button::Style {
        match self {
            Self::Label => processor_slot_label_button_style(theme, status, active, slot_hovered),
            Self::Icon => processor_slot_icon_button_style(theme, status, slot_hovered),
        }
    }
}

fn processor_slot_label_content(
    icon: iced::widget::svg::Handle,
    label: String,
    active: bool,
    slot_hovered: bool,
) -> Element<'static, Message> {
    let icon = processor_slot_icon_container(icon, move |theme, status| {
        processor_slot_label_icon_style(theme, status, active, slot_hovered)
    });
    row![
        icon,
        container(
            text(label)
                .size(ui_style::FONT_SIZE_UI_XS)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .width(Fill)
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .align_y(alignment::Vertical::Center)
        .clip(true)
    ]
    .spacing(ui_style::SPACE_XS)
    .align_y(alignment::Vertical::Center)
    .into()
}

fn processor_slot_segment(
    content: Element<'static, Message>,
    width: impl Into<Length>,
    style: impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static,
    action: Option<Message>,
    target: crate::app::processor_editor_windows::EditorTarget,
    segment: ProcessorSlotSegment,
) -> Element<'static, Message> {
    let button: Element<'static, Message> = button(content)
        .style(style)
        .padding(0)
        .width(width)
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .on_press_maybe(action)
        .into();
    processor_slot_hover_area(button, target, segment)
}

fn processor_slot_icon_container(
    icon: iced::widget::svg::Handle,
    style: impl Fn(&iced::Theme, iced::widget::svg::Status) -> iced::widget::svg::Style + 'static,
) -> Element<'static, Message> {
    container(ui_style::icon(icon, PROCESSOR_BROWSER_ICON_SIZE, style))
        .width(Length::Fixed(PROCESSOR_SLOT_SEGMENT_WIDTH))
        .height(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .center_x(Length::Fixed(PROCESSOR_SLOT_SEGMENT_WIDTH))
        .center_y(Length::Fixed(PROCESSOR_SLOT_BUTTON_HEIGHT))
        .into()
}

fn processor_slot_hover_area(
    button: Element<'static, Message>,
    target: crate::app::processor_editor_windows::EditorTarget,
    segment: ProcessorSlotSegment,
) -> Element<'static, Message> {
    mouse_area(button)
        .on_enter(Message::Mixer(MixerMessage::SetProcessorSlotHovered(Some(
            (target, segment),
        ))))
        .on_exit(Message::Mixer(MixerMessage::SetProcessorSlotHovered(None)))
        .into()
}

pub(super) fn slot_area_separator() -> Element<'static, Message> {
    container(text(""))
        .style(slot_area_separator_surface)
        .width(Length::Fixed(INSTRUMENT_SLOT_SEPARATOR_WIDTH))
        .height(Length::Fixed(
            INSTRUMENT_SLOT_BUTTON_HEIGHT - ui_style::grid_f32(2),
        ))
        .into()
}

pub(super) fn slot_area_separator_surface(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.strong.color.into()),
        ..container::Style::default()
    }
}

pub(super) fn processor_slot_icon_style(
    theme: &iced::Theme,
    status: iced::widget::svg::Status,
    active: bool,
) -> iced::widget::svg::Style {
    if active {
        let palette = theme.extended_palette();
        return iced::widget::svg::Style {
            color: Some(match status {
                iced::widget::svg::Status::Idle => palette.primary.base.color,
                iced::widget::svg::Status::Hovered => palette.primary.strong.color,
            }),
        };
    }

    ui_style::svg_muted_control(theme, status)
}

pub(super) fn processor_slot_button_style(
    theme: &iced::Theme,
    status: button::Status,
    open: bool,
    list_item: bool,
) -> button::Style {
    let mut style = ui_style::button_selector_field(theme, status, open);
    if list_item {
        let palette = theme.extended_palette();
        style.background = None;
        style.border = border::rounded(0)
            .width(0)
            .color(palette.background.strong.color);
        style.shadow = iced::Shadow::default();
    }
    style
}

pub(super) fn processor_slot_surface(
    theme: &iced::Theme,
    open: bool,
    list_item: bool,
) -> container::Style {
    let button_style = processor_slot_button_style(theme, button::Status::Active, open, list_item);
    container::Style {
        text_color: Some(button_style.text_color),
        background: button_style.background,
        border: button_style.border,
        shadow: button_style.shadow,
        ..container::Style::default()
    }
}

#[cfg(test)]
pub(super) fn instrument_slot_primary_action(
    track_index: usize,
    selected: Option<&InstrumentChoice>,
    editor_enabled: bool,
    controls_enabled: bool,
) -> Option<Message> {
    processor_slot_editor_action(
        crate::app::processor_editor_windows::EditorTarget {
            strip_index: track_index + 1,
            slot_index: 0,
        },
        selected,
        editor_enabled,
        controls_enabled,
    )
    .or_else(|| {
        if controls_enabled && is_empty_choice(selected) {
            Some(Message::Mixer(MixerMessage::ToggleProcessorBrowser(
                crate::app::processor_editor_windows::EditorTarget {
                    strip_index: track_index + 1,
                    slot_index: 0,
                },
            )))
        } else {
            None
        }
    })
}

pub(super) fn processor_slot_editor_action(
    target: crate::app::processor_editor_windows::EditorTarget,
    selected: Option<&ProcessorChoice>,
    editor_enabled: bool,
    controls_enabled: bool,
) -> Option<Message> {
    if !controls_enabled {
        return None;
    }

    match (selected, editor_enabled) {
        (Some(ProcessorChoice::Processor { .. }), true) => {
            Some(Message::Mixer(MixerMessage::OpenEditor(target)))
        }
        _ => None,
    }
}

pub(super) fn is_empty_choice(choice: Option<&ProcessorChoice>) -> bool {
    matches!(choice, None | Some(ProcessorChoice::None))
}

pub(super) fn processor_slot_bypass_icon(bypassed: bool) -> iced::widget::svg::Handle {
    if bypassed {
        icons::power_off()
    } else {
        icons::power()
    }
}
