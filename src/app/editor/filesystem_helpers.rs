use super::*;

pub(in crate::app) fn build_editor(
    content: &str,
    syntax: &str,
    document_path: Option<PathBuf>,
    project_root: Option<PathBuf>,
    app_theme: &iced::Theme,
    view_settings: EditorViewSettings,
    theme_settings: EditorThemeSettings,
) -> CodeEditor {
    let mut editor =
        CodeEditor::new_with_deferred_metrics(content, syntax).with_wrap_enabled(false);
    editor.set_document_path(document_path);
    editor.set_project_root(project_root);
    editor.set_font_deferred(fonts::MONO);
    editor.set_font_size_deferred(view_settings.font_size, true);
    editor.set_center_cursor(view_settings.center_cursor);
    editor.set_lsp_enabled(false);
    editor.set_theme(iced_code_editor::theme::from_iced_theme_with_tuning(
        app_theme,
        to_editor_theme_tuning(theme_settings),
    ));
    editor
}

pub(in crate::app) fn to_editor_theme_tuning(settings: EditorThemeSettings) -> ThemeTuning {
    ThemeTuning {
        hue_offset_degrees: settings.hue_offset_degrees,
        saturation: settings.saturation,
        warmth: settings.warmth,
        contrast: settings.brightness,
        text_dim: settings.text_dim,
        comment_dim: settings.comment_dim,
    }
}

pub(in crate::app) fn normalize_editor_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

pub(in crate::app) fn current_editor_browser_root(project_root: Option<&Path>) -> PathBuf {
    project_root
        .map(normalize_editor_path)
        .or_else(|| {
            env::current_dir()
                .ok()
                .map(|cwd| normalize_editor_path(&cwd))
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(in crate::app) fn read_browser_entries(
    path: &Path,
    show_hidden: bool,
) -> Result<Vec<EditorBrowserEntry>, String> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|error| format!("Failed to read directory {}: {error}", path.display()))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            let entry_path = normalize_editor_path(&entry.path());
            let name = entry.file_name().to_str()?.to_string();
            if !show_hidden && name.starts_with('.') {
                return None;
            }

            Some(EditorBrowserEntry {
                path: entry_path,
                name,
                is_dir: file_type.is_dir(),
            })
        })
        .collect();

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(entries)
}

pub(in crate::app) fn build_file_preview(path: &Path) -> Result<EditorFilePreview, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to read metadata for {}: {error}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_string();
    Ok(EditorFilePreview {
        name,
        size: if metadata.is_file() {
            Some(format_file_size(metadata.len()))
        } else {
            None
        },
        modified: metadata
            .modified()
            .ok()
            .and_then(format_relative_system_time),
        created: metadata
            .created()
            .ok()
            .and_then(format_relative_system_time),
    })
}

pub(in crate::app) fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MB {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

pub(in crate::app) fn offset_index(index: usize, delta: i32, len: usize) -> usize {
    let last_index = len.saturating_sub(1);
    if delta < 0 {
        let offset = usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX);
        index.saturating_sub(offset)
    } else {
        let offset = usize::try_from(delta).unwrap_or(usize::MAX);
        index.saturating_add(offset).min(last_index)
    }
}

pub(in crate::app) fn adjacent_tab_index(
    current_index: usize,
    tab_count: usize,
    next: bool,
) -> usize {
    if next {
        (current_index + 1) % tab_count
    } else {
        current_index.checked_sub(1).unwrap_or(tab_count - 1)
    }
}

pub(in crate::app) fn format_relative_system_time(time: std::time::SystemTime) -> Option<String> {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(time).ok()?;
    let seconds = duration.as_secs();

    Some(if seconds < 60 {
        "just now".to_string()
    } else if seconds < 3_600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} h ago", seconds / 3_600)
    } else {
        format!("{} d ago", seconds / 86_400)
    })
}

pub(in crate::app) fn syntax_for_path(path: &Path) -> String {
    if let Some(syntax) = iced_code_editor::language::syntax_for_path(path) {
        return syntax.to_string();
    }

    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        return extension.to_string();
    }

    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        return file_name.to_string();
    }

    "text".to_string()
}
