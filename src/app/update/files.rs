use super::*;

impl Lilypalooza {
    pub(in crate::app) fn handle_startup_checked(
        &mut self,
        result: Result<lilypond::VersionCheck, String>,
    ) -> Task<Message> {
        match result {
            Ok(version_check) => {
                self.lilypond_status = LilypondStatus::Ready {
                    detected: version_check.detected,
                    min_required: version_check.min_required,
                };
                self.logger.push(format!(
                    "LilyPond ready: installed {}, minimum required {}",
                    version_check.detected, version_check.min_required
                ));
            }
            Err(error) => {
                self.lilypond_status = LilypondStatus::Unavailable;
                self.logger
                    .push(format!("LilyPond startup check failed: {error}"));
                self.show_prompt(
                    ErrorPrompt::new(
                        "LilyPond Startup Error",
                        error,
                        ErrorFatality::Critical,
                        PromptButtons::Ok,
                    ),
                    Some(PromptOkAction::ExitApp),
                );
            }
        }

        Task::none()
    }

    pub(in crate::app) fn handle_file_message(&mut self, message: FileMessage) -> Task<Message> {
        match message {
            FileMessage::RequestOpen => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("LilyPond score", &["ly", "ily"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |picked| Message::File(FileMessage::Picked(picked)),
            ),
            FileMessage::Picked(Some(path)) => {
                let next_project_root = state::find_project_root(&path);
                if next_project_root != self.project_root && self.editor.has_dirty_tabs() {
                    return self.begin_pending_editor_action(
                        self.editor.dirty_tab_ids(),
                        EditorContinuation::OpenScore(path),
                    );
                }

                match selected_score_from_path(path) {
                    Ok(selected_score) => {
                        self.attach_persistence_context_for_score(&selected_score.path);
                        self.activate_score(selected_score)
                    }
                    Err(error) => {
                        self.show_prompt(
                            ErrorPrompt::new(
                                "Open File Error",
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
            FileMessage::Picked(None) => Task::none(),
            FileMessage::RequestCreateProject => {
                self.open_project_menu = false;
                self.open_project_recent = false;
                let suggested_directory = self
                    .project_root
                    .clone()
                    .or_else(|| {
                        self.current_score
                            .as_ref()
                            .and_then(|score| score.path.parent().map(Path::to_path_buf))
                    })
                    .unwrap_or_else(|| PathBuf::from("."));

                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_directory(&suggested_directory)
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_path_buf())
                    },
                    |picked| Message::File(FileMessage::CreateProjectPicked(picked)),
                )
            }
            FileMessage::RequestSaveProject => {
                self.open_project_menu = false;
                self.open_project_recent = false;
                if let Some(project_root) = self.project_root.clone() {
                    self.save_project_to_root(project_root)
                } else {
                    update(self, Message::File(FileMessage::RequestCreateProject))
                }
            }
            FileMessage::RequestLoadProject => {
                self.open_project_menu = false;
                self.open_project_recent = false;
                let suggested_directory = self
                    .project_root
                    .clone()
                    .or_else(|| {
                        self.current_score
                            .as_ref()
                            .and_then(|score| score.path.parent().map(Path::to_path_buf))
                    })
                    .unwrap_or_else(|| PathBuf::from("."));

                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_directory(&suggested_directory)
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_path_buf())
                    },
                    |picked| Message::File(FileMessage::LoadProjectPicked(picked)),
                )
            }
            FileMessage::CreateProjectPicked(Some(project_root)) => {
                self.save_project_to_root(project_root)
            }
            FileMessage::CreateProjectPicked(None) => Task::none(),
            FileMessage::LoadProjectPicked(Some(project_root)) => {
                self.load_project_from_root(project_root)
            }
            FileMessage::LoadProjectPicked(None) => Task::none(),
            FileMessage::OpenRecentProject(project_root) => {
                self.load_project_from_root(project_root)
            }
            FileMessage::RequestSoundfont => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("SoundFont", &["sf2", "sf3"])
                        .pick_file()
                        .await
                        .map(|file| file.path().to_path_buf())
                },
                |picked| Message::File(FileMessage::SoundfontPicked(picked)),
            ),
            FileMessage::SoundfontPicked(Some(path)) => {
                self.logger
                    .push(format!("Selected soundfont {}", path.display()));
                self.soundfont_status = SoundfontStatus::Ready(path.clone());
                self.initialize_playback(path);
                Task::none()
            }
            FileMessage::SoundfontPicked(None) => Task::none(),
        }
    }

    pub(in crate::app) fn handle_tick(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();

        tasks.push(self.tick_editor_tabbar_autoscroll());

        if let Some(tab_id) = self.pending_reveal_editor_tab {
            if self.editor.tab_ids().contains(&tab_id) {
                if self.editor_tab_reveal_target_x(tab_id).is_some() {
                    tasks.push(self.reveal_editor_tab(tab_id));
                } else {
                    self.pending_reveal_editor_tab = None;
                }
            } else {
                self.pending_reveal_editor_tab = None;
            }
        }

        if self.editor_font_metrics_refresh_pending {
            self.editor.refresh_font_metrics();
            self.editor_font_metrics_refresh_pending = false;
            if let Some(tab_id) = self.editor.active_tab_id() {
                tasks.push(
                    self.map_editor_widget_task(tab_id, self.editor.sync_tab_scroll_state(tab_id)),
                );
            }
        }

        for tab_id in self.editor.tab_ids() {
            let task = self.editor.update(tab_id, &iced_code_editor::Message::Tick);
            tasks.push(self.map_editor_widget_task(tab_id, task));
        }

        if self.compile_session.is_some() || self.score_watcher.is_some() {
            self.spinner_step = self.spinner_step.wrapping_add(1);
            self.poll_score_watcher();
            self.poll_compile_logs();
            self.start_compile_if_queued();
            tasks.push(self.apply_initial_piano_roll_center_if_needed());
        }

        if self
            .score_zoom_last_interaction
            .is_some_and(|instant| instant.elapsed() >= SCORE_ZOOM_PREVIEW_SETTLE_DELAY)
        {
            self.score_zoom_last_interaction = None;
        }

        if self.score_zoom_last_interaction.is_none() && self.score_zoom_persist_pending {
            self.score_zoom_persist_pending = false;
            self.persist_settings();
        }

        Task::batch(tasks)
    }

    pub(in crate::app) fn handle_frame(&mut self) -> Task<Message> {
        self.refresh_playback_position();
        Task::none()
    }

    pub(in crate::app) fn activate_score(
        &mut self,
        selected_score: SelectedScore,
    ) -> Task<Message> {
        let watched_path = selected_score.path.clone();
        let editor_task = match self.editor.load_file(&watched_path) {
            Ok((tab_id, task, _)) => {
                self.register_editor_recent_file(&watched_path);
                self.pending_reveal_editor_tab = Some(tab_id);
                let sync_task = self.editor.sync_tab_scroll_state(tab_id);
                self.map_editor_widget_task(tab_id, Task::batch([task, sync_task]))
            }
            Err(error) => {
                self.show_prompt(
                    ErrorPrompt::new(
                        "Editor Load Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                Task::none()
            }
        };
        self.logger.push(format!(
            "Opened score file {}",
            selected_score.path.display()
        ));
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
        self.current_score = Some(selected_score);
        self.persist_settings();
        self.restart_score_watcher(&watched_path);
        self.queue_compile("Score loaded, compiling SVG and MIDI");
        self.start_compile_if_queued();
        editor_task
    }

    pub(in crate::app) fn restart_score_watcher(&mut self, path: &Path) {
        match crate::score_watcher::ScoreWatcher::start(path) {
            Ok(watcher) => {
                self.logger.push(format!("Watching {}", path.display()));
                self.score_watcher = Some(watcher);
            }
            Err(error) => {
                self.score_watcher = None;
                self.show_prompt(
                    ErrorPrompt::new(
                        "File Watcher Error",
                        format!("Failed to watch score file changes: {error}"),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    pub(in crate::app) fn poll_score_watcher(&mut self) {
        let Some(score_watcher) = &self.score_watcher else {
            return;
        };

        let watched_path = score_watcher.watched_path().to_path_buf();
        let mut should_recompile = false;
        let mut disconnected = false;

        loop {
            match score_watcher.try_recv() {
                Ok(Ok(event)) => {
                    if is_relevant_score_change(&event, &watched_path) {
                        should_recompile = true;
                    }
                }
                Ok(Err(error)) => {
                    self.logger.push(format!("[watcher:error] {error}"));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        if disconnected {
            self.score_watcher = None;
            self.show_prompt(
                ErrorPrompt::new(
                    "File Watcher Error",
                    "Score file watcher disconnected",
                    ErrorFatality::Recoverable,
                    PromptButtons::Ok,
                ),
                None,
            );
        }

        if should_recompile {
            self.queue_compile("Score changed, recompiling");
        }
    }

    pub(in crate::app) fn queue_compile(&mut self, message: &str) {
        if !self.compile_requested {
            self.logger.push(message.to_string());
        }
        self.compile_requested = true;
    }

    pub(in crate::app) fn start_compile_if_queued(&mut self) {
        if !self.compile_requested || self.compile_session.is_some() {
            return;
        }

        let Some(selected_score) = self
            .current_score
            .as_ref()
            .map(|score| (score.path.clone(), score.file_name.clone()))
        else {
            self.compile_requested = false;
            return;
        };

        let output_prefix = match self.compile_output_prefix(&selected_score.1) {
            Ok(path) => path,
            Err(error) => {
                self.compile_requested = false;
                self.show_prompt(
                    ErrorPrompt::new(
                        "Build Directory Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
                return;
            }
        };

        let mut request = lilypond::CompileRequest::new(selected_score.0.clone());
        request.args = vec![
            "--svg".to_string(),
            "-dmidi-extension=midi".to_string(),
            "-dinclude-settings=event-listener.ly".to_string(),
            "-dpoint-and-click=note-event".to_string(),
            "-o".to_string(),
            output_prefix.to_string_lossy().to_string(),
        ];
        request.working_dir = selected_score.0.parent().map(std::path::Path::to_path_buf);

        self.logger.push("Starting LilyPond compile".to_string());

        match lilypond::spawn_compile(request) {
            Ok(session) => {
                self.compile_session = Some(session);
                self.compile_requested = false;
            }
            Err(error) => {
                self.compile_requested = false;
                self.show_prompt(
                    ErrorPrompt::new(
                        "LilyPond Compile Error",
                        error.to_string(),
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    pub(in crate::app) fn compile_output_prefix(
        &mut self,
        selected_file_name: &str,
    ) -> Result<std::path::PathBuf, String> {
        if self.build_dir.is_none() {
            let build_dir = tempfile::Builder::new()
                .prefix("lilypalooza-build-")
                .tempdir()
                .map_err(|error| format!("Failed to create temporary build directory: {error}"))?;
            self.logger.push(format!(
                "Using temporary build dir {}",
                build_dir.path().display()
            ));
            self.build_dir = Some(build_dir);
        }

        let file_stem = selected_score_stem(selected_file_name)?;
        let build_dir = self
            .build_dir
            .as_ref()
            .ok_or_else(|| "Temporary build directory is not available".to_string())?;

        Ok(build_dir.path().join(file_stem))
    }

    pub(in crate::app) fn reload_rendered_score(&mut self) {
        let previous_page = self
            .rendered_score
            .as_ref()
            .map(|rendered_score| rendered_score.current_page)
            .unwrap_or(0);

        let Some(selected_score) = &self.current_score else {
            self.rendered_score = None;
            self.score_zoom_preview = None;
            self.score_zoom_preview_pending = None;
            self.score_zoom_last_interaction = None;
            self.score_zoom_persist_pending = false;
            return;
        };

        match self.collect_rendered_pages(&selected_score.file_name) {
            Ok(pages) => {
                if pages.is_empty() {
                    self.rendered_score = None;
                    self.show_prompt(
                        ErrorPrompt::new(
                            "SVG Output Error",
                            "LilyPond finished without SVG output",
                            ErrorFatality::Recoverable,
                            PromptButtons::Ok,
                        ),
                        None,
                    );
                    return;
                }

                let page_count = pages.len();
                let current_page = previous_page.min(page_count.saturating_sub(1));
                self.rendered_score = Some(RenderedScore {
                    pages,
                    current_page,
                });
                self.score_zoom_preview = None;
                self.score_zoom_preview_pending = None;
                self.score_zoom_last_interaction = None;
                self.score_zoom_persist_pending = false;
                self.logger.push(format!("Loaded {page_count} SVG page(s)"));
            }
            Err(error) => {
                self.score_zoom_preview = None;
                self.score_zoom_preview_pending = None;
                self.score_zoom_last_interaction = None;
                self.score_zoom_persist_pending = false;
                self.show_prompt(
                    ErrorPrompt::new(
                        "SVG Output Error",
                        error,
                        ErrorFatality::Recoverable,
                        PromptButtons::Ok,
                    ),
                    None,
                );
            }
        }
    }

    pub(in crate::app) fn collect_rendered_pages(
        &self,
        selected_file_name: &str,
    ) -> Result<Vec<RenderedPage>, String> {
        let build_dir = self
            .build_dir
            .as_ref()
            .ok_or_else(|| "Temporary build directory is not available".to_string())?;
        let score_stem = selected_score_stem(selected_file_name)?;
        let entries = fs::read_dir(build_dir.path()).map_err(|error| {
            format!(
                "Failed to read build directory {}: {error}",
                build_dir.path().display()
            )
        })?;

        let mut pages = Vec::new();

        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read build artifact entry: {error}"))?;
            let path = entry.path();

            if !is_svg_file(&path) {
                continue;
            }

            let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(page_index) = svg_page_index(file_stem, score_stem) else {
                continue;
            };

            let page_size = read_svg_size(&path).unwrap_or(SvgSize {
                width: 1200.0,
                height: 1700.0,
            });

            pages.push((page_index, path, page_size));
        }

        pages.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

        let mut rendered_pages = Vec::with_capacity(pages.len());

        for (index, path, size) in pages {
            let bytes = fs::read(&path)
                .map_err(|error| format!("Failed to read SVG {}: {error}", path.display()))?;
            let source = String::from_utf8_lossy(&bytes);
            let page_index = index.saturating_sub(1) as usize;
            let note_anchors = score_cursor::parse_svg_note_anchors(&source, page_index);
            let system_bands = score_cursor::parse_svg_system_bands(&source);
            let svg_bytes = Bytes::from(bytes.clone());

            rendered_pages.push(RenderedPage {
                handle: svg::Handle::from_memory(bytes),
                svg_bytes,
                size,
                note_anchors,
                system_bands,
            });
        }

        Ok(rendered_pages)
    }

    pub(in crate::app) fn poll_compile_logs(&mut self) {
        let Some(session) = self.compile_session.take() else {
            return;
        };

        let mut keep_session = true;

        loop {
            match session.try_recv() {
                Ok(event) => match event {
                    lilypond::CompileEvent::Log { stream, line } => {
                        let prefix = match stream {
                            lilypond::LogStream::Stdout => "lilypond:stdout",
                            lilypond::LogStream::Stderr => "lilypond:stderr",
                        };
                        self.logger.push(format!("[{prefix}] {line}"));
                    }
                    lilypond::CompileEvent::ProcessError(message) => {
                        self.logger
                            .push(format!("[lilypond:process-error] {message}"));
                    }
                    lilypond::CompileEvent::Finished { success, exit_code } => {
                        if success {
                            self.logger.push(format!(
                                "LilyPond compile finished successfully (exit code {})",
                                exit_code.unwrap_or(0)
                            ));
                            self.reload_rendered_score();
                            self.reload_piano_roll();
                            self.reload_score_cursor_maps();
                            self.refresh_playback_position();
                        } else {
                            self.logger.push(format!(
                                "LilyPond compile failed (exit code {:?})",
                                exit_code
                            ));
                        }
                        keep_session = false;
                        break;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    keep_session = false;
                    break;
                }
            }
        }

        if keep_session {
            self.compile_session = Some(session);
        }
    }

    pub(in crate::app) fn unload_current_score(&mut self) {
        self.current_score = None;
        self.rendered_score = None;
        self.score_cursor_maps = None;
        self.score_cursor_overlay = None;
        self.score_watcher = None;
        self.piano_roll.clear_files();
        self.unload_playback_file();
    }
}
