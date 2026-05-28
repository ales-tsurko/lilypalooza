use super::*;

impl Lilypalooza {
    #[cfg(not(test))]
    pub(in crate::app) fn start_plugin_scan(&mut self) {
        self.start_plugin_scan_with_validator(plugin_validator_path());
    }

    pub(in crate::app) fn start_plugin_scan_with_validator(&mut self, validator: PathBuf) {
        if let Err(error) = ensure_plugin_validator_available(&validator) {
            self.logger.push(error);
            return;
        }

        self.logger.push(format!(
            "Scanning plugins from {} path(s)",
            self.plugin_search_paths
                .iter()
                .filter(|path| path.enabled)
                .count()
        ));
        let cache = self.plugin_scan_cache.clone();
        self.plugin_scan
            .start(self.plugin_search_paths.clone(), cache, validator);
    }

    pub(in crate::app) fn poll_plugin_scan(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        for event in self
            .plugin_scan
            .drain_events_with_limit(lilypalooza_plugin_scan::PLUGIN_SCAN_UI_EVENT_BUDGET)
        {
            if let Some(task) = self.apply_plugin_scan_event(event) {
                tasks.push(task);
            }
        }
        Task::batch(tasks)
    }

    pub(super) fn apply_plugin_scan_event(
        &mut self,
        event: lilypalooza_plugin_scan::PluginScanEvent,
    ) -> Option<Task<Message>> {
        match event {
            lilypalooza_plugin_scan::PluginScanEvent::Log(line) => {
                self.logger.push(line);
                None
            }
            lilypalooza_plugin_scan::PluginScanEvent::ClapPlugins(plugins) => {
                lilypalooza_clap::register_plugins(plugins);
                None
            }
            lilypalooza_plugin_scan::PluginScanEvent::Vst3Plugins(plugins) => {
                lilypalooza_vst3::register_plugins(plugins);
                None
            }
            lilypalooza_plugin_scan::PluginScanEvent::Finished { cache, .. } => {
                self.plugin_scan_cache = cache.clone();
                Some(Task::perform(
                    save_plugin_scan_cache(cache, plugin_scan_cache_path()),
                    Message::PluginScanCacheSaved,
                ))
            }
        }
    }

    pub(in crate::app) fn handle_startup_checked(
        &mut self,
        result: Result<lilypond::VersionCheck, String>,
    ) -> Task<Message> {
        match result {
            Ok(version_check) => {
                self.lilypond_status = LilypondStatus::Ready {
                    detected: version_check.detected,
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
            FileMessage::RequestOpen | FileMessage::Picked(_) => {
                self.handle_score_file_message(message)
            }
            FileMessage::RequestCreateProject
            | FileMessage::RequestSaveProject
            | FileMessage::RequestLoadProject => self.handle_project_request_message(message),
            FileMessage::CreateProjectPicked(_)
            | FileMessage::LoadProjectPicked(_)
            | FileMessage::OpenRecentProject(_) => self.handle_project_result_message(message),
        }
    }

    pub(super) fn handle_score_file_message(&mut self, message: FileMessage) -> Task<Message> {
        match message {
            FileMessage::RequestOpen => request_open_score_dialog(),
            FileMessage::Picked(Some(path)) => self.open_picked_score(path),
            FileMessage::Picked(None) => Task::none(),
            _ => Task::none(),
        }
    }

    pub(super) fn handle_project_request_message(&mut self, message: FileMessage) -> Task<Message> {
        if let Some(picked) = project_request_picker(&message) {
            return self.request_project_folder_dialog(picked);
        }
        match message {
            FileMessage::RequestSaveProject => self.request_save_project(),
            _ => Task::none(),
        }
    }

    pub(super) fn request_save_project(&mut self) -> Task<Message> {
        self.close_project_menu();
        if let Some(project_root) = self.project_root.clone() {
            self.save_project_to_root(project_root)
        } else {
            update(self, Message::File(FileMessage::RequestCreateProject))
        }
    }

    pub(super) fn handle_project_result_message(&mut self, message: FileMessage) -> Task<Message> {
        match message {
            FileMessage::CreateProjectPicked(Some(project_root)) => {
                self.save_created_project(project_root)
            }
            FileMessage::LoadProjectPicked(Some(project_root))
            | FileMessage::OpenRecentProject(project_root) => {
                self.load_project_from_root(project_root)
            }
            FileMessage::CreateProjectPicked(None) | FileMessage::LoadProjectPicked(None) => {
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub(super) fn open_picked_score(&mut self, path: PathBuf) -> Task<Message> {
        let next_project_root = state::find_project_root(&path);
        if next_project_root != self.project_root && self.editor.has_dirty_tabs() {
            return self.begin_pending_editor_action(
                self.editor.tabs_requiring_resolution(),
                EditorContinuation::OpenScore(path),
            );
        }
        self.continue_editor_continuation(EditorContinuation::OpenScore(path))
    }

    pub(super) fn request_project_folder_dialog(
        &mut self,
        message: fn(Option<PathBuf>) -> FileMessage,
    ) -> Task<Message> {
        self.close_project_menu();
        let suggested_directory = self.suggested_project_directory();

        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_directory(&suggested_directory)
                    .pick_folder()
                    .await
                    .map(|folder| folder.path().to_path_buf())
            },
            move |picked| Message::File(message(picked)),
        )
    }

    pub(super) fn save_created_project(&mut self, project_root: PathBuf) -> Task<Message> {
        let save_task = self.save_project_to_root(project_root);
        if matches!(
            self.pending_editor_action,
            Some(PendingEditorAction::ResolveDirtyProject { .. })
        ) {
            Task::batch([save_task, self.advance_pending_editor_action()])
        } else {
            save_task
        }
    }

    pub(super) fn close_project_menu(&mut self) {
        self.open_project_menu = false;
        self.open_project_menu_section = None;
        self.open_project_recent = false;
    }

    pub(super) fn suggested_project_directory(&self) -> PathBuf {
        self.project_root
            .clone()
            .or_else(|| {
                self.current_score
                    .as_ref()
                    .and_then(|score| score.path.parent().map(Path::to_path_buf))
            })
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
