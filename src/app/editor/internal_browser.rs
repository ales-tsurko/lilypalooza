use super::*;

impl EditorState {
    pub(in crate::app) fn apply_theme(&mut self) {
        let theme = iced_code_editor::theme::from_iced_theme_with_tuning(
            &self.app_theme,
            to_editor_theme_tuning(self.theme_settings),
        );
        for tab in &mut self.tabs {
            tab.widget.set_theme(theme);
        }
    }

    pub(in crate::app) fn set_font_size(&mut self, size: f32) {
        let clamped = size.clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE);
        self.view_settings.font_size = clamped;
        for tab in &mut self.tabs {
            tab.widget.set_font_size(clamped, true);
        }
    }

    pub(in crate::app) fn load_document_into_tab(
        &mut self,
        tab_id: u64,
        content: &str,
        path: Option<PathBuf>,
        modified: bool,
    ) -> Result<iced::Task<EditorWidgetMessage>, String> {
        let syntax = path
            .as_deref()
            .map(syntax_for_path)
            .unwrap_or_else(|| "lilypond".to_string());
        let app_theme = self.app_theme.clone();
        let theme_settings = self.theme_settings;
        let font_size = self.view_settings.font_size;
        let center_cursor = self.view_settings.center_cursor;
        let project_root = self.project_root.clone();

        let task = if let Some(tab) = self.tab_mut(tab_id) {
            let task = tab.widget.reset_document(content, &syntax);
            tab.widget.set_font(fonts::MONO);
            tab.widget.set_document_path(path.clone());
            tab.widget.set_project_root(project_root.clone());
            tab.widget
                .set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
                    &app_theme,
                    to_editor_theme_tuning(theme_settings),
                ));
            tab.widget.set_font_size(font_size, true);
            tab.widget.set_center_cursor(center_cursor);
            if !modified {
                tab.widget.mark_saved();
            }
            tab.path = path;
            tab.saved_content = tab.path.as_ref().map(|_| content.to_string());
            tab.file_state = EditorTabFileState::Ok;
            task
        } else {
            let mut tab = EditorTab {
                id: tab_id,
                widget: build_editor(
                    content,
                    &syntax,
                    path.clone(),
                    self.project_root.clone(),
                    &self.app_theme,
                    self.view_settings,
                    self.theme_settings,
                ),
                path,
                saved_content: None,
                file_state: EditorTabFileState::Ok,
            };
            if !modified {
                tab.widget.mark_saved();
            }
            tab.saved_content = tab.path.as_ref().map(|_| content.to_string());
            self.tabs.push(tab);
            iced::Task::none()
        };

        self.active_tab_id = Some(tab_id);

        Ok(task)
    }

    pub(in crate::app) fn find_reusable_empty_tab(&self) -> Option<u64> {
        self.tabs.iter().find_map(|tab| {
            (tab.path.is_none() && !tab.widget.is_modified() && tab.widget.content().is_empty())
                .then_some(tab.id)
        })
    }

    pub(in crate::app) fn allocate_tab_id(&mut self) -> u64 {
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        tab_id
    }

    pub(in crate::app) fn active_tab(&self) -> Option<&EditorTab> {
        self.active_tab_id.and_then(|tab_id| self.tab(tab_id))
    }

    pub(in crate::app) fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        let tab_id = self.active_tab_id?;
        self.tab_mut(tab_id)
    }

    pub(in crate::app) fn tab(&self, tab_id: u64) -> Option<&EditorTab> {
        self.tabs.iter().find(|tab| tab.id == tab_id)
    }

    pub(in crate::app) fn tab_mut(&mut self, tab_id: u64) -> Option<&mut EditorTab> {
        self.tabs.iter_mut().find(|tab| tab.id == tab_id)
    }

    pub(in crate::app) fn rebuild_file_browser(&mut self) {
        self.file_browser_root = current_editor_browser_root(self.project_root.as_deref());
        self.file_browser_columns = vec![EditorBrowserColumn::Directory {
            path: self.file_browser_root.clone(),
            entries: self
                .read_browser_entries(&self.file_browser_root)
                .unwrap_or_default(),
            selected_path: None,
        }];
        self.file_browser_active_column = 0;
    }

    pub(in crate::app) fn ensure_file_browser_initialized(&mut self) {
        if self.file_browser_columns.is_empty() {
            self.rebuild_file_browser();
        }
    }

    pub(in crate::app) fn move_file_browser_selection(&mut self, delta: i32) -> Result<(), String> {
        if self.file_browser_columns.is_empty() {
            return Ok(());
        }

        let column_index = self
            .file_browser_active_column
            .min(self.file_browser_columns.len().saturating_sub(1));
        let EditorBrowserColumn::Directory {
            path: _,
            entries,
            selected_path,
        } = self
            .file_browser_columns
            .get(column_index)
            .ok_or_else(|| "File browser column is no longer available".to_string())?
        else {
            return Ok(());
        };
        if entries.is_empty() {
            return Ok(());
        }

        let next_index = match selected_path
            .as_ref()
            .and_then(|selected| entries.iter().position(|entry| &entry.path == selected))
        {
            Some(current_index) => offset_index(current_index, delta, entries.len()),
            None if delta >= 0 => 0,
            None => entries.len().saturating_sub(1),
        };
        let Some(entry) = entries.get(next_index) else {
            return Ok(());
        };
        let path = entry.path.clone();
        let is_dir = entry.is_dir;

        self.browse_to_path(column_index, &path, is_dir)
    }

    pub(in crate::app) fn move_file_browser_column(&mut self, right: bool) -> Result<(), String> {
        if self.file_browser_columns.is_empty() {
            return Ok(());
        }

        if !right {
            let active_index = self
                .file_browser_active_column
                .min(self.file_browser_columns.len().saturating_sub(1));
            if let Some(EditorBrowserColumn::Directory { selected_path, .. }) =
                self.file_browser_columns.get_mut(active_index)
            {
                *selected_path = None;
            }
            self.file_browser_active_column = self.file_browser_active_column.saturating_sub(1);
            self.sync_file_browser_preview_from_column(self.file_browser_active_column)?;
            return Ok(());
        }

        let column_index = self
            .file_browser_active_column
            .min(self.file_browser_columns.len().saturating_sub(1));
        let EditorBrowserColumn::Directory {
            path: _,
            entries,
            selected_path,
        } = self
            .file_browser_columns
            .get(column_index)
            .ok_or_else(|| "File browser column is no longer available".to_string())?
        else {
            return Ok(());
        };
        if entries.is_empty() {
            return Ok(());
        }

        let selected_index = selected_path
            .as_ref()
            .and_then(|selected| entries.iter().position(|entry| &entry.path == selected))
            .unwrap_or(0);
        let Some(entry) = entries.get(selected_index) else {
            return Ok(());
        };
        if !entry.is_dir {
            return Ok(());
        }

        let path = entry.path.clone();
        self.browse_to_path(column_index, &path, true)?;
        self.file_browser_active_column =
            (column_index + 1).min(self.file_browser_columns.len().saturating_sub(1));
        if let Some(EditorBrowserColumn::Directory {
            path: _,
            entries,
            selected_path,
        }) = self
            .file_browser_columns
            .get_mut(self.file_browser_active_column)
            && selected_path.is_none()
            && let Some(first_entry) = entries.first()
        {
            *selected_path = Some(first_entry.path.clone());
        }
        self.sync_file_browser_preview_from_column(self.file_browser_active_column)?;
        Ok(())
    }

    pub(in crate::app) fn sync_file_browser_preview_from_column(
        &mut self,
        column_index: usize,
    ) -> Result<(), String> {
        if column_index >= self.file_browser_columns.len() {
            return Ok(());
        }

        let Some(EditorBrowserColumn::Directory {
            path: _,
            entries,
            selected_path,
        }) = self.file_browser_columns.get(column_index)
        else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        let Some(selected_path) = selected_path.clone() else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        let Some((selected_path, selected_is_dir)) = entries
            .iter()
            .find(|entry| entry.path == selected_path)
            .map(|entry| (entry.path.clone(), entry.is_dir))
        else {
            self.file_browser_columns.truncate(column_index + 1);
            return Ok(());
        };

        self.file_browser_columns.truncate(column_index + 1);
        self.file_browser_columns.push(if selected_is_dir {
            EditorBrowserColumn::Directory {
                path: selected_path.clone(),
                entries: self.read_browser_entries(&selected_path)?,
                selected_path: None,
            }
        } else {
            EditorBrowserColumn::FilePreview(build_file_preview(&selected_path)?)
        });
        Ok(())
    }

    pub(in crate::app) fn file_browser_current_directory(&self) -> Option<&Path> {
        self.file_browser_columns
            .iter()
            .take(self.file_browser_active_column.saturating_add(1))
            .rev()
            .find_map(|column| match column {
                EditorBrowserColumn::Directory { path, .. } => Some(path.as_path()),
                EditorBrowserColumn::FilePreview(_) => None,
            })
    }

    pub(in crate::app) fn rebuild_file_browser_preserving_selection(
        &mut self,
    ) -> Result<(), String> {
        let selected_chain: Vec<_> = self
            .file_browser_columns
            .iter()
            .filter_map(|column| match column {
                EditorBrowserColumn::Directory { selected_path, .. } => selected_path.clone(),
                EditorBrowserColumn::FilePreview(_) => None,
            })
            .collect();
        let previous_active = self.file_browser_active_column;
        let mut rebuilt_columns = vec![EditorBrowserColumn::Directory {
            path: self.file_browser_root.clone(),
            entries: self.read_browser_entries(&self.file_browser_root)?,
            selected_path: None,
        }];

        for selected_path in selected_chain {
            let Some((entry_path, entry_is_dir)) = ({
                let Some(EditorBrowserColumn::Directory {
                    entries,
                    selected_path: current_selected,
                    ..
                }) = rebuilt_columns.last_mut()
                else {
                    break;
                };
                let Some(entry) = entries.iter().find(|entry| entry.path == selected_path) else {
                    break;
                };
                *current_selected = Some(entry.path.clone());
                Some((entry.path.clone(), entry.is_dir))
            }) else {
                break;
            };
            if entry_is_dir {
                rebuilt_columns.push(EditorBrowserColumn::Directory {
                    path: entry_path.clone(),
                    entries: self.read_browser_entries(&entry_path)?,
                    selected_path: None,
                });
            } else {
                rebuilt_columns.push(EditorBrowserColumn::FilePreview(build_file_preview(
                    &entry_path,
                )?));
                break;
            }
        }

        self.file_browser_columns = rebuilt_columns;
        self.file_browser_active_column =
            previous_active.min(self.file_browser_columns.len().saturating_sub(1));
        Ok(())
    }

    pub(in crate::app) fn read_browser_entries(
        &self,
        path: &Path,
    ) -> Result<Vec<EditorBrowserEntry>, String> {
        let path = normalize_editor_path(path);
        read_browser_entries(&path, self.file_browser_show_hidden)
    }
}
