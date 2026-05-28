use super::*;

pub(in crate::app) fn delete_browser_path(path: &std::path::Path) -> Result<(), String> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to delete directory {}: {error}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to delete file {}: {error}", path.display()))
    }
}

pub(super) fn copy_browser_path(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("{} already exists", destination.display()));
    }

    if source.is_dir() {
        copy_browser_directory(source, destination)
    } else {
        copy_browser_file(source, destination)
    }
}

pub(super) fn copy_browser_directory(source: &Path, destination: &Path) -> Result<(), String> {
    create_browser_directory(destination)?;
    for entry in read_browser_directory(source)? {
        copy_browser_directory_entry(source, destination, entry)?;
    }
    Ok(())
}

pub(super) fn create_browser_directory(destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| {
        format!(
            "Failed to create directory {}: {error}",
            destination.display()
        )
    })
}

pub(super) fn read_browser_directory(source: &Path) -> Result<fs::ReadDir, String> {
    fs::read_dir(source)
        .map_err(|error| format!("Failed to read directory {}: {error}", source.display()))
}

pub(super) fn copy_browser_directory_entry(
    source: &Path,
    destination: &Path,
    entry: std::io::Result<fs::DirEntry>,
) -> Result<(), String> {
    let entry = entry.map_err(|error| {
        format!(
            "Failed to read directory entry in {}: {error}",
            source.display()
        )
    })?;
    let child_source = entry.path();
    let child_destination = destination.join(entry.file_name());
    copy_browser_path(&child_source, &child_destination)
}

pub(super) fn copy_browser_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination).map(|_| ()).map_err(|error| {
        format!(
            "Failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

pub(super) fn move_browser_path(source: &Path, destination: &Path) -> Result<(), String> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_browser_path(source, destination)?;
            delete_browser_path(source).map_err(|delete_error| {
                format!(
                    "Failed to move {} to {}: rename failed with {rename_error}; cleanup failed \
                     with {delete_error}",
                    source.display(),
                    destination.display()
                )
            })
        }
    }
}

pub(super) fn ensure_browser_destination_available(destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("{} already exists", destination.display()));
    }
    Ok(())
}

pub(super) fn rename_browser_path(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|error| {
        format!(
            "Failed to rename {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })
}

pub(super) fn create_browser_inline_file(
    destination: PathBuf,
) -> Result<BrowserInlineEditCommit, String> {
    create_browser_inline_path(destination, BrowserInlineKind::File)
}

pub(super) fn create_browser_inline_directory(
    destination: PathBuf,
) -> Result<BrowserInlineEditCommit, String> {
    create_browser_inline_path(destination, BrowserInlineKind::Directory)
}

#[derive(Debug, Clone, Copy)]
enum BrowserInlineKind {
    File,
    Directory,
}

fn create_browser_inline_path(
    destination: PathBuf,
    kind: BrowserInlineKind,
) -> Result<BrowserInlineEditCommit, String> {
    ensure_browser_destination_available(&destination)?;
    match kind {
        BrowserInlineKind::File => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|error| {
                    format!("Failed to create file {}: {error}", destination.display())
                })?;
        }
        BrowserInlineKind::Directory => create_browser_directory(&destination)?,
    }
    Ok(BrowserInlineEditCommit::Applied(
        BrowserHistoryEntry::Create {
            path: destination,
            stash_path: None,
        },
    ))
}

pub(super) fn editor_tab_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::NewRequested
            | EditorMessage::TabPressed(_)
            | EditorMessage::CloseTabRequested(_)
    ) || editor_tab_drag_message(message)
}

pub(super) fn editor_tab_drag_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::TabMoved { .. }
            | EditorMessage::TabGlobalMoved(_)
            | EditorMessage::TabHovered(_)
            | EditorMessage::TabBarScrolled(_)
            | EditorMessage::TabBarEmptyMoved
            | EditorMessage::TabBarMoved(_)
            | EditorMessage::TabDragReleased
            | EditorMessage::TabDragExited
    )
}

pub(super) fn editor_rename_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::StartRename(_)
            | EditorMessage::RenameInputChanged(_)
            | EditorMessage::CommitRename
            | EditorMessage::CancelRename
            | EditorMessage::RenameRequested
            | EditorMessage::RenamePicked(_)
    )
}

pub(super) fn file_browser_command_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::ToggleFileBrowser
            | EditorMessage::FileBrowserFocused
            | EditorMessage::FileBrowserToggleHiddenRequested
            | EditorMessage::FileBrowserCutRequested
            | EditorMessage::FileBrowserCopyRequested
            | EditorMessage::FileBrowserPasteRequested
            | EditorMessage::FileBrowserNewFileRequested
            | EditorMessage::FileBrowserNewDirectoryRequested
            | EditorMessage::FileBrowserRenameRequested
            | EditorMessage::FileBrowserInlineEditChanged(_)
            | EditorMessage::CommitFileBrowserInlineEdit
            | EditorMessage::CancelFileBrowserInlineEdit
            | EditorMessage::FileBrowserTrashRequested
    )
}

pub(super) fn file_browser_scroll_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::FileBrowserScrolled(_) | EditorMessage::FileBrowserColumnScrolled { .. }
    )
}

pub(super) fn file_browser_entry_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::FileBrowserEntryPressed { .. }
            | EditorMessage::FileBrowserEntryHovered { .. }
            | EditorMessage::FileBrowserEntryDragReleased { .. }
            | EditorMessage::FileBrowserEntryDoublePressed { .. }
            | EditorMessage::FileBrowserDragMoved(_)
            | EditorMessage::FileBrowserDragReleased
    )
}

pub(super) fn editor_file_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::OpenRequested
            | EditorMessage::OpenPicked(_)
            | EditorMessage::OpenRecent(_)
            | EditorMessage::SaveRequested
            | EditorMessage::SaveAsRequested
            | EditorMessage::SaveAsPicked(_)
    )
}

pub(super) fn editor_appearance_message(message: &EditorMessage) -> bool {
    matches!(
        message,
        EditorMessage::SetCenterCursor(_)
            | EditorMessage::ZoomIn
            | EditorMessage::ZoomOut
            | EditorMessage::ResetZoom
            | EditorMessage::SetThemeHueOffsetDegrees(_)
            | EditorMessage::SetThemeSaturation(_)
            | EditorMessage::SetThemeWarmth(_)
            | EditorMessage::SetThemeBrightness(_)
            | EditorMessage::SetThemeTextDim(_)
            | EditorMessage::SetThemeCommentDim(_)
    )
}

pub(super) fn editor_open_task(
    app: &mut Lilypalooza,
    message: EditorMessage,
) -> Option<Task<Message>> {
    editor_open_request_task(app, &message).or_else(|| editor_open_picker_task(app, message))
}

pub(super) fn editor_open_request_task(
    app: &mut Lilypalooza,
    message: &EditorMessage,
) -> Option<Task<Message>> {
    match message {
        EditorMessage::OpenRequested => Some(app.request_open_editor_files()),
        EditorMessage::OpenRecent(path) => Some(app.open_recent_editor_file(path.clone())),
        _ => None,
    }
}

pub(super) fn editor_open_picker_task(
    app: &mut Lilypalooza,
    message: EditorMessage,
) -> Option<Task<Message>> {
    match message {
        EditorMessage::OpenPicked(Some(paths)) => Some(app.open_picked_editor_files(paths)),
        EditorMessage::OpenPicked(None) => Some(Task::none()),
        _ => None,
    }
}

pub(super) fn editor_save_task(
    app: &mut Lilypalooza,
    message: EditorMessage,
) -> Option<Task<Message>> {
    editor_save_request_task(app, &message).or_else(|| editor_save_picker_task(app, message))
}

pub(super) fn editor_save_request_task(
    app: &mut Lilypalooza,
    message: &EditorMessage,
) -> Option<Task<Message>> {
    match message {
        EditorMessage::SaveRequested => Some(app.save_requested_editor_file()),
        EditorMessage::SaveAsRequested => Some(app.request_save_active_editor_tab_as()),
        _ => None,
    }
}

pub(super) fn editor_save_picker_task(
    app: &mut Lilypalooza,
    message: EditorMessage,
) -> Option<Task<Message>> {
    match message {
        EditorMessage::SaveAsPicked(Some(path)) => Some(app.finish_save_active_editor_tab_as(path)),
        EditorMessage::SaveAsPicked(None) => Some(app.cancel_save_active_editor_tab_as()),
        _ => None,
    }
}

pub(super) fn browser_move_history_paths<'a>(
    from: &'a Path,
    to: &'a Path,
    redo: bool,
) -> (&'a Path, &'a Path) {
    if redo { (from, to) } else { (to, from) }
}

pub(super) fn browser_delete_history_paths<'a>(
    path: &'a Path,
    stash_path: &'a Path,
    redo: bool,
) -> (&'a Path, &'a Path) {
    if redo {
        (path, stash_path)
    } else {
        (stash_path, path)
    }
}

pub(super) fn normalize_editor_tab_file_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}
