use iced::widget::column;

use super::*;

pub(in crate::app) fn instrument_browser_overlay(app: &Lilypalooza) -> Element<'_, Message> {
    let Some(target) = app.open_processor_browser_target else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let Some(playback) = app.playback.as_ref() else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let mixer = playback.mixer_state();
    let Some(strip) = mixer.strip_by_index(target.strip_index) else {
        return container(text("")).width(Fill).height(Fill).into();
    };
    let role = if target.slot_index == 0 {
        ProcessorSlotRole::Instrument
    } else {
        ProcessorSlotRole::Effect
    };
    let choices = processor_choices(role);
    let selected = selected_processor_choice(strip.slot(target.slot_index), role);

    let header = container(
        row![
            column![
                text(role.title())
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..fonts::UI
                    }),
                text(strip.name.clone())
                    .size(ui_style::FONT_SIZE_UI_XS)
                    .font(fonts::MONO),
            ]
            .spacing(ui_style::SPACE_XS),
            container(text("")).width(Fill),
            ui_style::flat_icon_button(
                icons::x(),
                ui_style::grid_f32(5),
                ui_style::grid_f32(3),
                ui_style::button_pane_header_control,
                ui_style::svg_dimmed_control,
            )
            .width(Length::Fixed(ui_style::grid_f32(5)))
            .height(Length::Fixed(ui_style::grid_f32(5)))
            .on_press(Message::Mixer(MixerMessage::CloseProcessorBrowser)),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
    .style(ui_style::prompt_header);

    let search = text_input(role.search_placeholder(), &app.instrument_browser_search)
        .on_input(|value| Message::Mixer(MixerMessage::ProcessorBrowserSearchChanged(value)))
        .id(app.instrument_browser_search_input_id.clone())
        .style(ui_style::browser_search_input)
        .size(ui_style::FONT_SIZE_UI_SM)
        .padding([ui_style::PADDING_XS, ui_style::PADDING_SM])
        .width(Fill);

    let body = processor_browser_list(ProcessorBrowserListArgs {
        target,
        role,
        choices: &choices,
        selected: selected.as_ref(),
        search: &app.instrument_browser_search,
        expanded_sections: &app.processor_browser_expanded_sections,
        scan_active: app.plugin_scan.is_active(),
    });

    let dialog = container(
        column![
            header,
            container(column![search, body].spacing(ui_style::SPACE_SM))
                .padding(ui_style::PADDING_SM)
        ]
        .spacing(0),
    )
    .width(Length::Fixed(INSTRUMENT_BROWSER_WIDTH))
    .style(ui_style::prompt_dialog);

    let centered_dialog = container(
        mouse_area(opaque(dialog))
            .on_press(Message::Noop)
            .interaction(iced::mouse::Interaction::Pointer),
    )
    .width(Fill)
    .height(Fill)
    .center_x(Fill)
    .center_y(Fill);

    let backdrop = mouse_area(
        container(centered_dialog)
            .width(Fill)
            .height(Fill)
            .style(ui_style::prompt_backdrop),
    )
    .on_press(Message::Mixer(MixerMessage::CloseProcessorBrowser));

    opaque(backdrop)
}

pub(super) struct ProcessorBrowserListArgs<'a> {
    target: crate::app::processor_editor_windows::EditorTarget,
    role: ProcessorSlotRole,
    choices: &'a [ProcessorChoice],
    selected: Option<&'a ProcessorChoice>,
    search: &'a str,
    expanded_sections: &'a [ProcessorBrowserSectionKey],
    scan_active: bool,
}

pub(super) fn processor_browser_list(
    args: ProcessorBrowserListArgs<'_>,
) -> Element<'static, Message> {
    let ProcessorBrowserListArgs {
        target,
        role,
        choices,
        selected,
        search,
        expanded_sections,
        scan_active,
    } = args;
    let browser = processor_browser_entries(choices, role, search);
    let InstrumentBrowserEntries {
        show_none,
        backends,
    } = browser;
    let mut content = column![].spacing(0).width(Fill);
    if show_none {
        content = content.push(instrument_browser_choice_button(
            target,
            role,
            ProcessorChoice::None,
            selected == Some(&InstrumentChoice::None),
            ProcessorBrowserRowDepth::Root,
        ));
    }

    let has_entries = backends.iter().any(|backend| {
        backend
            .sections
            .iter()
            .any(|section| !section.entries.is_empty())
    });
    for backend in backends {
        let backend_expanded =
            processor_browser_section_expanded(&backend.key, expanded_sections, search);
        content = content.push(processor_browser_section_header(
            backend.key,
            backend.title,
            backend_expanded,
            ProcessorBrowserRowDepth::Root,
        ));
        if backend_expanded {
            for section in backend.sections {
                let section_expanded =
                    processor_browser_section_expanded(&section.key, expanded_sections, search);
                content = content.push(processor_browser_section_header(
                    section.key,
                    section.title,
                    section_expanded,
                    ProcessorBrowserRowDepth::Group,
                ));
                if section_expanded {
                    for choice in section.entries {
                        content = content.push(instrument_browser_choice_button(
                            target,
                            role,
                            choice.clone(),
                            selected == Some(&choice),
                            ProcessorBrowserRowDepth::Leaf,
                        ));
                    }
                }
            }
        }
    }

    if !show_none && !has_entries {
        let label = if scan_active {
            "Scanning plugins..."
        } else {
            role.empty_search_label()
        };
        return instrument_browser_empty_state(label);
    }

    scrollable(content)
        .height(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
        .style(ui_style::workspace_scrollable)
        .into()
}

pub(super) fn processor_browser_section_expanded(
    key: &ProcessorBrowserSectionKey,
    expanded_sections: &[ProcessorBrowserSectionKey],
    search: &str,
) -> bool {
    !search.trim().is_empty() || expanded_sections.iter().any(|expanded| expanded == key)
}

pub(super) fn processor_browser_section_header(
    key: ProcessorBrowserSectionKey,
    title: String,
    expanded: bool,
    depth: ProcessorBrowserRowDepth,
) -> Element<'static, Message> {
    button(
        row![
            ui_style::icon(
                processor_browser_section_icon(expanded),
                PROCESSOR_BROWSER_ICON_SIZE,
                ui_style::svg_dimmed_control,
            ),
            text(title)
                .size(ui_style::FONT_SIZE_UI_XS)
                .font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..fonts::UI
                }),
        ]
        .spacing(ui_style::SPACE_XS)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .padding(processor_browser_choice_padding(depth))
    .style(ui_style::button_browser_section_header)
    .on_press(Message::Mixer(MixerMessage::ToggleProcessorBrowserSection(
        key,
    )))
    .into()
}

pub(super) fn instrument_browser_choice_button(
    target: crate::app::processor_editor_windows::EditorTarget,
    role: ProcessorSlotRole,
    choice: ProcessorChoice,
    selected: bool,
    depth: ProcessorBrowserRowDepth,
) -> Element<'static, Message> {
    button(
        container(
            row![
                instrument_browser_choice_icon(role, &choice),
                text(instrument_choice_primary_label(&choice))
                    .size(ui_style::FONT_SIZE_UI_SM)
                    .width(Fill)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .center_y(Fill),
    )
    .style(move |theme, status| ui_style::button_browser_child_entry(theme, status, selected))
    .padding(processor_browser_choice_padding(depth))
    .width(Fill)
    .on_press(Message::Mixer(MixerMessage::SelectProcessor(
        target, choice,
    )))
    .into()
}

pub(super) fn processor_browser_section_icon(expanded: bool) -> iced::widget::svg::Handle {
    if expanded {
        icons::folder_open()
    } else {
        icons::folder()
    }
}

pub(super) fn instrument_browser_choice_icon(
    role: ProcessorSlotRole,
    choice: &ProcessorChoice,
) -> Element<'static, Message> {
    ui_style::icon(
        processor_browser_choice_icon(role, choice),
        PROCESSOR_BROWSER_ICON_SIZE,
        ui_style::svg_dimmed_control,
    )
    .into()
}

pub(super) fn processor_browser_choice_icon(
    role: ProcessorSlotRole,
    choice: &ProcessorChoice,
) -> iced::widget::svg::Handle {
    match choice {
        ProcessorChoice::None => icons::x(),
        ProcessorChoice::Processor { .. } => role.slot_icon(),
    }
}

pub(super) fn processor_browser_choice_padding(depth: ProcessorBrowserRowDepth) -> [u16; 2] {
    [
        ui_style::grid(2),
        ui_style::grid(match depth {
            ProcessorBrowserRowDepth::Root => 3,
            ProcessorBrowserRowDepth::Group => 7,
            ProcessorBrowserRowDepth::Leaf => 11,
        }),
    ]
}

pub(super) fn instrument_browser_empty_state(label: &'static str) -> Element<'static, Message> {
    container(
        text(label)
            .size(ui_style::FONT_SIZE_UI_SM)
            .font(fonts::MONO),
    )
    .width(Fill)
    .height(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
    .center_x(Fill)
    .center_y(Length::Fixed(INSTRUMENT_BROWSER_HEIGHT))
    .into()
}

pub(super) fn instrument_choice_primary_label(choice: &InstrumentChoice) -> String {
    match choice {
        InstrumentChoice::None => "Empty".to_string(),
        InstrumentChoice::Processor { name, .. } => name.clone(),
    }
}

#[cfg(test)]
pub(super) fn instrument_trigger_label(choice: Option<&InstrumentChoice>) -> String {
    processor_trigger_label(choice)
}

pub(super) fn processor_trigger_label(choice: Option<&ProcessorChoice>) -> String {
    crate::track_names::ellipsize_middle(
        &choice
            .map(instrument_choice_primary_label)
            .unwrap_or_else(|| "Empty".to_string()),
        PROCESSOR_SLOT_LABEL_MAX_LEN,
    )
}

pub(super) fn instrument_choice_search_haystack(choice: &InstrumentChoice) -> String {
    match choice {
        InstrumentChoice::None => "empty none no instrument".to_string(),
        InstrumentChoice::Processor { name, backend, .. } => {
            format!(
                "{} {} {}",
                name.to_lowercase(),
                backend.label().to_lowercase(),
                processor_choice_group_label(choice).to_lowercase()
            )
        }
    }
}

#[cfg(test)]
pub(super) fn instrument_browser_entries(
    choices: &[InstrumentChoice],
    search: &str,
) -> InstrumentBrowserEntries {
    processor_browser_entries(choices, ProcessorSlotRole::Instrument, search)
}

pub(super) fn processor_browser_entries(
    choices: &[ProcessorChoice],
    role: ProcessorSlotRole,
    search: &str,
) -> InstrumentBrowserEntries {
    let query = search.trim().to_lowercase();
    let matches = |choice: &ProcessorChoice| {
        query.is_empty() || instrument_choice_search_haystack(choice).contains(&query)
    };

    let mut groups: BTreeMap<ProcessorBrowserBackend, BTreeMap<String, Vec<ProcessorChoice>>> =
        BTreeMap::new();
    let mut show_none = false;
    for choice in choices {
        match choice {
            InstrumentChoice::None => {
                if matches(choice) {
                    show_none = true;
                }
            }
            InstrumentChoice::Processor { backend, .. } if matches(choice) => {
                groups
                    .entry(*backend)
                    .or_default()
                    .entry(processor_choice_group_label(choice))
                    .or_default()
                    .push(choice.clone());
            }
            InstrumentChoice::Processor { .. } => {}
        }
    }

    let backends = groups
        .into_iter()
        .map(|(backend, groups)| ProcessorBrowserBackendSection {
            key: ProcessorBrowserSectionKey::backend(role, backend),
            title: backend.label().to_string(),
            sections: groups
                .into_iter()
                .map(|(title, entries)| ProcessorBrowserSection {
                    key: ProcessorBrowserSectionKey::new(role, backend, title.clone()),
                    title,
                    entries,
                })
                .collect(),
        })
        .collect();

    InstrumentBrowserEntries {
        show_none,
        backends,
    }
}

pub(super) fn processor_choice_group_label(choice: &ProcessorChoice) -> String {
    match choice {
        ProcessorChoice::None => "General".to_string(),
        ProcessorChoice::Processor {
            processor_id,
            backend,
            ..
        } => registry::entry(processor_id)
            .map(|entry| match backend {
                ProcessorBrowserBackend::BuiltIn => entry.category.into_owned(),
                ProcessorBrowserBackend::Clap | ProcessorBrowserBackend::Vst3 => {
                    entry.manufacturer.into_owned()
                }
            })
            .unwrap_or_else(|| match backend {
                ProcessorBrowserBackend::BuiltIn => "Built-in".to_string(),
                ProcessorBrowserBackend::Clap | ProcessorBrowserBackend::Vst3 => {
                    "Unknown Manufacturer".to_string()
                }
            }),
    }
}

pub(super) fn processor_choices(role: ProcessorSlotRole) -> Vec<ProcessorChoice> {
    let mut choices = Vec::new();
    choices.push(ProcessorChoice::None);
    choices.extend(
        registry::all()
            .iter()
            .filter(|entry| entry.role == role.registry_role())
            .filter(|entry| entry.id != BUILTIN_NONE_ID && entry.id != BUILTIN_METRONOME_ID)
            .map(|entry| ProcessorChoice::Processor {
                processor_id: entry.id.to_string(),
                name: entry.name.to_string(),
                backend: match entry.backend {
                    registry::Backend::BuiltIn => ProcessorBrowserBackend::BuiltIn,
                    registry::Backend::Clap => ProcessorBrowserBackend::Clap,
                    registry::Backend::Vst3 => ProcessorBrowserBackend::Vst3,
                },
            }),
    );
    choices
}

pub(super) fn selected_instrument_choice(
    slot: Option<&SlotState>,
    _mixer: &MixerState,
) -> Option<InstrumentChoice> {
    selected_processor_choice(slot, ProcessorSlotRole::Instrument)
}

pub(super) fn selected_processor_choice(
    slot: Option<&SlotState>,
    _role: ProcessorSlotRole,
) -> Option<ProcessorChoice> {
    let slot = slot?;
    if slot.is_empty() {
        return Some(ProcessorChoice::None);
    }
    let entry = registry::resolve(&slot.kind)?;
    Some(ProcessorChoice::Processor {
        processor_id: entry.id.to_string(),
        name: entry.name.to_string(),
        backend: match entry.backend {
            registry::Backend::BuiltIn => ProcessorBrowserBackend::BuiltIn,
            registry::Backend::Clap => ProcessorBrowserBackend::Clap,
            registry::Backend::Vst3 => ProcessorBrowserBackend::Vst3,
        },
    })
}
