use super::*;

pub(super) fn editor_root_menu_item<'a>(
    label: &'a str,
    active: bool,
    section: EditorHeaderMenuSection,
) -> Element<'a, Message> {
    let button = button(
        container(
            row![
                text(label).size(ui_style::FONT_SIZE_UI_XS),
                container(text("")).width(Fill),
                ui_style::icon(
                    icons::chevron_right(),
                    10.0,
                    move |theme: &Theme, _status| {
                        svg::Style {
                            color: Some(if active {
                                theme.extended_palette().background.weakest.text
                            } else {
                                Color::from_rgb(0.12, 0.12, 0.14)
                            }),
                        }
                    }
                ),
            ]
            .spacing(ui_style::SPACE_XS)
            .width(Fill)
            .align_y(alignment::Vertical::Center),
        )
        .width(Fill)
        .height(Fill)
        .center_y(Fill),
    )
    .width(Fill)
    .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
    .padding([EDITOR_MENU_ITEM_PADDING_V, EDITOR_MENU_ITEM_PADDING_H])
    .style(move |theme: &Theme, status| ui_style::button_menu_item(theme, status, active))
    .on_press(Message::Pane(PaneMessage::SetEditorHeaderMenuSection(
        Some(section),
    )));

    mouse_area(button)
        .interaction(mouse::Interaction::Pointer)
        .on_enter(Message::Pane(PaneMessage::SetEditorHeaderMenuSection(
            Some(section),
        )))
        .into()
}

pub(super) fn editor_file_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    editor_file_actions_column(app)
        .push(editor_recent_files_section(app))
        .into()
}

fn editor_file_actions_column<'a>(app: &'a Lilypalooza) -> Column<'a, Message> {
    let has_document = app.editor.has_document();
    Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(editor_file_menu_item(
            editor_shortcut_label(app, shortcuts::ShortcutAction::NewEditor, "New"),
            true,
            Some(Message::Editor(crate::app::EditorMessage::NewRequested)),
        ))
        .push(editor_file_menu_item(
            editor_shortcut_label(app, shortcuts::ShortcutAction::OpenEditorFile, "Open..."),
            true,
            Some(Message::Editor(crate::app::EditorMessage::OpenRequested)),
        ))
        .push(editor_file_menu_item(
            editor_shortcut_label(app, shortcuts::ShortcutAction::SaveEditor, "Save"),
            has_document,
            Some(Message::Editor(crate::app::EditorMessage::SaveRequested)),
        ))
        .push(editor_file_menu_item(
            "Save As...",
            has_document,
            Some(Message::Editor(crate::app::EditorMessage::SaveAsRequested)),
        ))
        .push(editor_file_menu_item(
            "Rename...",
            has_document,
            Some(Message::Editor(crate::app::EditorMessage::RenameRequested)),
        ))
}

fn editor_shortcut_label(
    app: &Lilypalooza,
    action: shortcuts::ShortcutAction,
    fallback: &str,
) -> String {
    shortcuts::label_for_action(&app.shortcut_settings, action)
        .map(|shortcut| format!("{fallback} ({shortcut})"))
        .unwrap_or_else(|| fallback.to_string())
}

fn editor_recent_files_section<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let has_recent_files = !app.editor_recent_files.is_empty();
    let recent_open = app.open_editor_file_menu_section == Some(EditorFileMenuSection::OpenRecent);
    let recent_hovered =
        app.hovered_editor_file_menu_section == Some(EditorFileMenuSection::OpenRecent);

    let recent_row = if has_recent_files {
        mouse_area(editor_fold_menu_item(
            "Open Recent",
            has_recent_files,
            recent_open,
            recent_hovered,
            Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: Some(EditorFileMenuSection::OpenRecent),
                expanded: !recent_open,
            }),
        ))
        .interaction(mouse::Interaction::Pointer)
        .on_move(|position| {
            Message::Pane(PaneMessage::HoverEditorFileMenuSection {
                section: Some(EditorFileMenuSection::OpenRecent),
                expanded: position.x >= EDITOR_FILE_SUBMENU_WIDTH * 0.5,
            })
        })
        .into()
    } else {
        editor_fold_menu_item(
            "Open Recent",
            false,
            false,
            false,
            Message::Pane(PaneMessage::CloseHeaderOverflowMenu),
        )
    };

    let mut recent_section = Column::new().spacing(ui_style::SPACE_XS).push(recent_row);

    if recent_open {
        recent_section = recent_section.push(container(editor_recent_files_submenu(app)).padding(
            Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: f32::from(ui_style::PADDING_MD),
            },
        ));
    }

    mouse_area(recent_section)
        .interaction(if has_recent_files {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        })
        .on_enter(Message::Pane(PaneMessage::HoverEditorFileMenuSection {
            section: Some(EditorFileMenuSection::OpenRecent),
            expanded: recent_open,
        }))
        .on_exit(Message::Pane(PaneMessage::HoverEditorFileMenuSection {
            section: None,
            expanded: false,
        }))
        .into()
}

pub(super) fn editor_recent_files_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    if app.editor_recent_files.is_empty() {
        return Column::new()
            .spacing(ui_style::SPACE_XS)
            .push(editor_menu_item("No recent files", false, None))
            .into();
    }

    let recent_paths: Vec<_> = app
        .editor_recent_files
        .iter()
        .take(app.editor_recent_files_limit)
        .cloned()
        .collect();
    let labels = recent_file_labels(&recent_paths, EDITOR_RECENT_FILE_LABEL_MAX_CHARS);

    recent_paths
        .into_iter()
        .zip(labels)
        .fold(
            Column::new().spacing(ui_style::SPACE_XS),
            |column, (path, label)| {
                column.push(editor_recent_file_item(
                    app,
                    label,
                    path.clone(),
                    Message::Editor(crate::app::EditorMessage::OpenRecent(path)),
                ))
            },
        )
        .into()
}

pub(super) fn editor_recent_file_item<'a>(
    app: &Lilypalooza,
    label: String,
    full_path: PathBuf,
    on_press: Message,
) -> Element<'a, Message> {
    delayed_tooltip(
        app,
        format!("editor-recent-{}", full_path.display()),
        editor_menu_item(label, true, Some(on_press)),
        text(full_path.display().to_string())
            .size(ui_style::FONT_SIZE_UI_XS)
            .into(),
        tooltip::Position::Right,
    )
}

pub(super) fn recent_file_labels(paths: &[PathBuf], max_chars: usize) -> Vec<String> {
    let components: Vec<Vec<String>> = paths
        .iter()
        .map(|path| path_display_components(path))
        .collect();
    let mut suffix_lengths = vec![1; components.len()];

    loop {
        let collisions = recent_label_collisions(&components, &suffix_lengths);
        if !expand_recent_label_collisions(&components, &mut suffix_lengths, &collisions) {
            break;
        }
    }

    components
        .iter()
        .zip(suffix_lengths)
        .map(|(parts, suffix_len)| {
            truncate_recent_label(&suffix_path(parts, suffix_len), max_chars)
        })
        .collect()
}

pub(super) fn recent_label_collisions(
    components: &[Vec<String>],
    suffix_lengths: &[usize],
) -> HashMap<String, Vec<usize>> {
    let mut collisions: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, parts) in components.iter().enumerate() {
        let suffix_len = suffix_lengths.get(index).copied().unwrap_or(1);
        collisions
            .entry(suffix_path(parts, suffix_len))
            .or_default()
            .push(index);
    }
    collisions
}

pub(super) fn expand_recent_label_collisions(
    components: &[Vec<String>],
    suffix_lengths: &mut [usize],
    collisions: &HashMap<String, Vec<usize>>,
) -> bool {
    let mut changed = false;
    for indices in collisions.values().filter(|indices| indices.len() > 1) {
        changed |= expand_recent_label_collision(components, suffix_lengths, indices);
    }
    changed
}

pub(super) fn expand_recent_label_collision(
    components: &[Vec<String>],
    suffix_lengths: &mut [usize],
    indices: &[usize],
) -> bool {
    let mut changed = false;
    for &index in indices {
        let Some(parts) = components.get(index) else {
            continue;
        };
        let Some(suffix_len) = suffix_lengths.get_mut(index) else {
            continue;
        };
        if *suffix_len < parts.len() {
            *suffix_len += 1;
            changed = true;
        }
    }
    changed
}

pub(super) fn path_display_components(path: &Path) -> Vec<String> {
    let mut parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".to_string()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect();

    if parts.is_empty() {
        parts.push(path.display().to_string());
    }

    parts
}

pub(super) fn suffix_path(parts: &[String], count: usize) -> String {
    let start = parts.len().saturating_sub(count);
    parts.get(start..).unwrap_or(parts).join("/")
}

pub(super) fn truncate_recent_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }

    let parts: Vec<&str> = label.split('/').collect();
    let Some(file_name) = parts.last().copied() else {
        return label.to_string();
    };

    if file_name.chars().count() >= max_chars {
        return truncate_from_left(file_name, max_chars);
    }

    let mut suffix = file_name.to_string();
    let parent_parts = parts.get(..parts.len().saturating_sub(1)).unwrap_or(&[]);
    for parent in parent_parts.iter().rev() {
        let candidate = format!("{parent}/{suffix}");
        let display = format!("…/{candidate}");
        if display.chars().count() <= max_chars {
            suffix = candidate;
        } else {
            break;
        }
    }

    format!("…/{suffix}")
}

pub(super) fn truncate_from_left(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    let keep = max_chars - 1;
    let tail: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

pub(super) fn editor_appearance_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let zoom_out_button = editor_zoom_button(
        icons::zoom_out(),
        app.editor.can_zoom_out(),
        crate::app::EditorMessage::ZoomOut,
    );
    let zoom_in_button = editor_zoom_button(
        icons::zoom_in(),
        app.editor.can_zoom_in(),
        crate::app::EditorMessage::ZoomIn,
    );
    let zoom_value_label = text(format!("{}pt", app.editor.font_size_points()))
        .size(ui_style::FONT_SIZE_UI_XS)
        .font(fonts::MONO);
    let zoom_value = if app.editor.can_reset_zoom() {
        mouse_area(zoom_value_label)
            .on_double_click(Message::Editor(crate::app::EditorMessage::ResetZoom))
    } else {
        mouse_area(zoom_value_label)
    };
    let zoom_value = delayed_tooltip(
        app,
        "editor-font-size-reset",
        zoom_value.into(),
        text("Double-click to reset")
            .size(ui_style::FONT_SIZE_UI_XS)
            .into(),
        tooltip::Position::Top,
    );

    Column::new()
        .spacing(ui_style::SPACE_SM)
        .push(
            row![
                text("Font Size").size(ui_style::FONT_SIZE_UI_XS),
                zoom_out_button,
                zoom_value,
                zoom_in_button
            ]
            .spacing(ui_style::SPACE_XS)
            .align_y(alignment::Vertical::Center),
        )
        .push(editor_menu_separator())
        .push(editor_theme_controls_column(app))
        .into()
}

fn editor_zoom_button<'a>(
    icon: svg::Handle,
    enabled: bool,
    message: crate::app::EditorMessage,
) -> Element<'a, Message> {
    let button = button(compact_control_icon(icon))
        .style(ui_style::button_neutral)
        .padding([
            ui_style::PADDING_BUTTON_COMPACT_V,
            ui_style::PADDING_BUTTON_COMPACT_H,
        ]);
    if enabled {
        button.on_press(Message::Editor(message)).into()
    } else {
        button.into()
    }
}

fn editor_menu_separator<'a>() -> Element<'a, Message> {
    container(
        container(text(""))
            .width(Fill)
            .height(Length::Fixed(1.0))
            .style(ui_style::chrome_separator),
    )
    .padding([ui_style::SPACE_SM as u16, 0])
    .into()
}

pub(super) fn editor_edit_submenu<'a>(app: &'a Lilypalooza) -> Element<'a, Message> {
    let has_document = app.editor.has_document();
    let center_cursor = app.editor.center_cursor();

    Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(editor_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::EditorUndo,
            )
            .map(|shortcut| format!("Undo ({shortcut})"))
            .unwrap_or_else(|| "Undo".to_string()),
            has_document,
            Some(Message::Editor(
                crate::app::EditorMessage::ActiveWidgetMessage(iced_code_editor::Message::Undo),
            )),
        ))
        .push(editor_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::EditorRedo,
            )
            .map(|shortcut| format!("Redo ({shortcut})"))
            .unwrap_or_else(|| "Redo".to_string()),
            has_document,
            Some(Message::Editor(
                crate::app::EditorMessage::ActiveWidgetMessage(iced_code_editor::Message::Redo),
            )),
        ))
        .push(
            container(
                container(text(""))
                    .width(Fill)
                    .height(Length::Fixed(1.0))
                    .style(ui_style::chrome_separator),
            )
            .padding([ui_style::SPACE_XS as u16, 0]),
        )
        .push(editor_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::EditorOpenSearch,
            )
            .map(|shortcut| format!("Find... ({shortcut})"))
            .unwrap_or_else(|| "Find...".to_string()),
            has_document,
            Some(Message::Editor(
                crate::app::EditorMessage::ActiveWidgetMessage(
                    iced_code_editor::Message::OpenSearch,
                ),
            )),
        ))
        .push(editor_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::EditorOpenSearchReplace,
            )
            .map(|shortcut| format!("Find and Replace... ({shortcut})"))
            .unwrap_or_else(|| "Find and Replace...".to_string()),
            has_document,
            Some(Message::Editor(
                crate::app::EditorMessage::ActiveWidgetMessage(
                    iced_code_editor::Message::OpenSearchReplace,
                ),
            )),
        ))
        .push(editor_menu_item(
            shortcuts::label_for_action(
                &app.shortcut_settings,
                shortcuts::ShortcutAction::EditorOpenGotoLine,
            )
            .map(|shortcut| format!("Go to Line... ({shortcut})"))
            .unwrap_or_else(|| "Go to Line...".to_string()),
            has_document,
            Some(Message::Editor(
                crate::app::EditorMessage::ActiveWidgetMessage(
                    iced_code_editor::Message::OpenGotoLine,
                ),
            )),
        ))
        .push(
            container(
                container(text(""))
                    .width(Fill)
                    .height(Length::Fixed(1.0))
                    .style(ui_style::chrome_separator),
            )
            .padding([ui_style::SPACE_XS as u16, 0]),
        )
        .push(editor_menu_item(
            if center_cursor {
                "Centered Cursor: On"
            } else {
                "Centered Cursor: Off"
            },
            true,
            Some(Message::Editor(crate::app::EditorMessage::SetCenterCursor(
                !center_cursor,
            ))),
        ))
        .into()
}

pub(super) fn editor_menu_item<'a>(
    label: impl Into<String>,
    enabled: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let mut item = button(
        container(container(text(label.into()).size(ui_style::FONT_SIZE_UI_XS)).center_y(Fill))
            .width(Fill)
            .height(Fill)
            .align_x(alignment::Horizontal::Left),
    )
    .width(Fill)
    .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
    .padding([EDITOR_MENU_ITEM_PADDING_V, EDITOR_MENU_ITEM_PADDING_H])
    .style(|theme: &Theme, status| ui_style::button_menu_item(theme, status, false));

    if enabled && let Some(message) = on_press {
        item = item.on_press(message);
    }

    item.into()
}

pub(super) fn editor_file_menu_item<'a>(
    label: impl Into<String>,
    enabled: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    editor_menu_item(label, enabled, on_press)
}

pub(super) fn editor_fold_menu_item<'a>(
    label: &'a str,
    enabled: bool,
    active: bool,
    hovered: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let highlighted = active || hovered;
    let content = container(
        row![
            container(text(label).size(ui_style::FONT_SIZE_UI_XS))
                .width(Fill)
                .align_x(alignment::Horizontal::Left),
            ui_style::icon(
                icons::chevron_down(),
                12.0,
                move |theme: &Theme, _status| {
                    svg::Style {
                        color: Some(if highlighted {
                            theme.extended_palette().background.weakest.text
                        } else {
                            Color::from_rgb(0.12, 0.12, 0.14)
                        }),
                    }
                }
            ),
        ]
        .spacing(ui_style::SPACE_XS)
        .width(Fill)
        .align_y(alignment::Vertical::Center),
    )
    .width(Fill)
    .height(Fill)
    .center_y(Fill);

    let button = button(content)
        .width(Fill)
        .height(Length::Fixed(EDITOR_MENU_ITEM_HEIGHT))
        .padding([EDITOR_MENU_ITEM_PADDING_V, EDITOR_MENU_ITEM_PADDING_H])
        .style(move |theme: &Theme, status| {
            ui_style::button_menu_item(theme, status, active || hovered)
        });

    if enabled {
        button.on_press(on_press).into()
    } else {
        button.into()
    }
}

pub(super) fn editor_theme_controls_column<'a>(app: &'a Lilypalooza) -> Column<'a, Message> {
    let settings = app.editor.theme_settings();

    Column::with_children(vec![
        editor_theme_slider(
            "Hue",
            format!("{:+.0}°", settings.hue_offset_degrees),
            -180.0..=180.0,
            settings.hue_offset_degrees,
            1.0,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeHueOffsetDegrees(value)),
        ),
        editor_theme_slider(
            "Saturation",
            format!("{:.2}", settings.saturation),
            0.0..=1.8,
            settings.saturation,
            0.01,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeSaturation(value)),
        ),
        editor_theme_slider(
            "Warmth",
            format!("{:+.2}", settings.warmth),
            -1.0..=1.0,
            settings.warmth,
            0.01,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeWarmth(value)),
        ),
        editor_theme_slider(
            "Brightness",
            format!("{:.2}", settings.brightness),
            0.5..=1.8,
            settings.brightness,
            0.01,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeBrightness(value)),
        ),
        editor_theme_slider(
            "Text Dim",
            format!("{:.2}", settings.text_dim),
            0.5..=3.0,
            settings.text_dim,
            0.01,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeTextDim(value)),
        ),
        editor_theme_slider(
            "Comment Dim",
            format!("{:.2}", settings.comment_dim),
            0.5..=1.8,
            settings.comment_dim,
            0.01,
            |value| Message::Editor(crate::app::EditorMessage::SetThemeCommentDim(value)),
        ),
    ])
    .spacing(ui_style::SPACE_SM)
}

pub(super) fn editor_theme_slider<'a>(
    label: &'a str,
    value: String,
    range: std::ops::RangeInclusive<f32>,
    current: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Column::new()
        .spacing(ui_style::SPACE_XS)
        .push(
            row![
                text(label).size(ui_style::FONT_SIZE_UI_XS),
                container(
                    text(value)
                        .size(ui_style::FONT_SIZE_UI_XS)
                        .font(fonts::MONO)
                )
                .width(Fill)
                .align_x(alignment::Horizontal::Right),
            ]
            .align_y(alignment::Vertical::Center),
        )
        .push(
            slider(range, current, on_change)
                .step(step)
                .shift_step(step * 10.0),
        )
        .into()
}
