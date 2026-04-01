use super::*;

impl LilyView {
    pub(in crate::app) fn handle_editor_message(
        &mut self,
        message: EditorMessage,
    ) -> Task<Message> {
        match message {
            EditorMessage::Widget(message) => {
                if matches!(
                    message,
                    iced_code_editor::Message::CanvasFocusGained
                        | iced_code_editor::Message::MouseClick(_)
                        | iced_code_editor::Message::MouseDrag(_)
                        | iced_code_editor::Message::JumpClick(_)
                ) {
                    self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                }

                self.editor
                    .update(&message)
                    .map(|message| Message::Editor(EditorMessage::Widget(message)))
            }
            EditorMessage::NewRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.editor
                    .new_document()
                    .map(|message| Message::Editor(EditorMessage::Widget(message)))
            }
            EditorMessage::OpenRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    |picked| Message::Editor(EditorMessage::OpenPicked(picked)),
                )
            }
            EditorMessage::OpenPicked(Some(path)) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_editor_file_in_editor(&path)
            }
            EditorMessage::OpenRecent(path) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                self.open_editor_file_in_editor(&path)
            }
            EditorMessage::OpenPicked(None) => Task::none(),
            EditorMessage::SaveRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                if !self.editor.has_document() {
                    return Task::none();
                }

                if !self.editor.has_path() {
                    return update(self, Message::Editor(EditorMessage::SaveAsRequested));
                }

                match self.editor.save_to_disk() {
                    Ok((path, task)) => {
                        self.register_editor_recent_file(&path);
                        self.logger.push(format!("Saved {}", path.display()));
                        if self.editor_targets_main_score() {
                            self.queue_compile("Editor saved, recompiling");
                            self.start_compile_if_queued();
                        }
                        return task.map(|message| Message::Editor(EditorMessage::Widget(message)));
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Editor Save Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                }

                Task::none()
            }
            EditorMessage::SaveAsRequested => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                if !self.editor.has_document() {
                    return Task::none();
                }
                self.open_header_overflow_menu = None;
                self.open_editor_menu_section = None;
                self.open_editor_file_menu_section = None;
                let suggested_name = self.editor.suggested_save_name();
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_file_name(&suggested_name)
                            .save_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    |picked| Message::Editor(EditorMessage::SaveAsPicked(picked)),
                )
            }
            EditorMessage::SaveAsPicked(Some(path)) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                match self.editor.save_to_path(&path) {
                    Ok(task) => {
                        self.register_editor_recent_file(&path);
                        self.logger.push(format!("Saved {}", path.display()));
                        if self.editor_targets_main_score() {
                            self.queue_compile("Editor saved, recompiling");
                            self.start_compile_if_queued();
                        }
                        return task.map(|message| Message::Editor(EditorMessage::Widget(message)));
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Editor Save Error",
                                error,
                                ErrorFatality::Recoverable,
                                PromptButtons::Ok,
                            ),
                            None,
                        );
                    }
                }
                Task::none()
            }
            EditorMessage::SaveAsPicked(None) => Task::none(),
            EditorMessage::ZoomIn => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.zoom_in();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::ZoomOut => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.zoom_out();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::ResetZoom => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.reset_zoom();
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeHueOffsetDegrees(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_hue_offset_degrees(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeSaturation(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_saturation(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeWarmth(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_warmth(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeBrightness(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_brightness(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeTextDim(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_text_dim(value);
                self.persist_settings();
                Task::none()
            }
            EditorMessage::SetThemeCommentDim(value) => {
                self.set_focused_workspace_pane(WorkspacePaneKind::Editor);
                self.editor.set_comment_dim(value);
                self.persist_settings();
                Task::none()
            }
        }
    }

    pub(in crate::app) fn editor_targets_main_score(&self) -> bool {
        self.editor.path()
            == self
                .current_score
                .as_ref()
                .map(|score| score.path.as_path())
    }

    pub(in crate::app) fn open_editor_file_in_editor(&mut self, path: &Path) -> Task<Message> {
        match self.editor.load_file(path) {
            Ok(task) => {
                self.register_editor_recent_file(path);
                self.logger
                    .push(format!("Opened editor file {}", path.display()));
                task.map(|message| Message::Editor(EditorMessage::Widget(message)))
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Open Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        }
    }
}
