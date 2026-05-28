use super::*;

pub(super) fn request_open_score_dialog() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .add_filter("LilyPond score", &["ly", "ily"])
                .pick_file()
                .await
                .map(|file| file.path().to_path_buf())
        },
        |picked| Message::File(FileMessage::Picked(picked)),
    )
}

pub(super) fn compile_request_for_score(
    score_path: &Path,
    output_prefix: &Path,
) -> lilypond::CompileRequest {
    let mut request = lilypond::CompileRequest::new(score_path.to_path_buf());
    request.args = lilypond_compile_args(output_prefix);
    request.working_dir = score_path.parent().map(Path::to_path_buf);
    request
}

#[derive(Debug)]
pub(super) enum EditorTabDiskState {
    Missing(PathBuf),
    ReadError { path: PathBuf, error: String },
    Unchanged,
    Changed { path: PathBuf, modified: bool },
}

#[derive(Debug, Default)]
pub(super) struct EditorFileWatcherDrain {
    pub(super) affected_tabs: Vec<u64>,
    pub(super) errors: Vec<String>,
    pub(super) disconnected: bool,
}

#[derive(Debug, Default)]
pub(super) struct FileWatcherPoll {
    pub(super) changed: bool,
    pub(super) disconnected: bool,
}

#[derive(Debug, Default)]
pub(super) struct ScoreWatcherPoll {
    pub(super) state: FileWatcherPoll,
    pub(super) errors: Vec<String>,
}

pub(super) fn drain_score_watcher(
    watcher: &crate::score_watcher::ScoreWatcher,
    watched_path: &Path,
) -> ScoreWatcherPoll {
    let mut poll = ScoreWatcherPoll::default();
    loop {
        if !poll_score_watcher_event(watcher, watched_path, &mut poll) {
            break;
        }
    }
    poll
}

pub(super) fn drain_browser_file_watcher(
    watcher: &crate::browser_file_watcher::BrowserFileWatcher,
    watched_root: &Path,
) -> FileWatcherPoll {
    let mut poll = FileWatcherPoll::default();
    loop {
        if !poll_browser_file_watcher_event(watcher, watched_root, &mut poll) {
            break;
        }
    }
    poll
}

pub(super) fn poll_score_watcher_event(
    watcher: &crate::score_watcher::ScoreWatcher,
    watched_path: &Path,
    poll: &mut ScoreWatcherPoll,
) -> bool {
    match watcher.try_recv() {
        Ok(Ok(event)) => {
            poll.state.changed |= is_relevant_score_change(&event, watched_path);
            true
        }
        Ok(Err(error)) => {
            poll.errors.push(error.to_string());
            true
        }
        Err(TryRecvError::Empty) => false,
        Err(TryRecvError::Disconnected) => {
            poll.state.disconnected = true;
            false
        }
    }
}

pub(super) fn poll_browser_file_watcher_event(
    watcher: &crate::browser_file_watcher::BrowserFileWatcher,
    watched_root: &Path,
    poll: &mut FileWatcherPoll,
) -> bool {
    match watcher.try_recv() {
        Ok(Ok(event)) => {
            poll.changed |= is_relevant_browser_file_change(&event, watched_root);
            true
        }
        Ok(Err(_error)) => true,
        Err(TryRecvError::Empty) => false,
        Err(TryRecvError::Disconnected) => {
            poll.disconnected = true;
            false
        }
    }
}

pub(super) fn poll_editor_file_watcher_event(
    watcher: &crate::editor_file_watcher::EditorFileWatcher,
    watched_tabs: &[(u64, PathBuf)],
    drain: &mut EditorFileWatcherDrain,
) -> bool {
    match watcher.try_recv() {
        Ok(Ok(event)) => {
            collect_affected_editor_tabs(&mut drain.affected_tabs, watched_tabs, &event);
            true
        }
        Ok(Err(error)) => {
            drain.errors.push(format!("[editor-watcher:error] {error}"));
            true
        }
        Err(TryRecvError::Empty) => false,
        Err(TryRecvError::Disconnected) => {
            drain.disconnected = true;
            false
        }
    }
}

pub(super) fn editor_tab_disk_content(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| error.to_string())
}

pub(super) fn editor_tab_existing_disk_content(path: &Path) -> Result<String, EditorTabDiskState> {
    if !path.exists() {
        return Err(EditorTabDiskState::Missing(path.to_path_buf()));
    }
    editor_tab_disk_content(path).map_err(|error| EditorTabDiskState::ReadError {
        path: path.to_path_buf(),
        error,
    })
}

pub(super) fn project_request_picker(
    message: &FileMessage,
) -> Option<fn(Option<PathBuf>) -> FileMessage> {
    match message {
        FileMessage::RequestCreateProject => Some(FileMessage::CreateProjectPicked),
        FileMessage::RequestLoadProject => Some(FileMessage::LoadProjectPicked),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub(super) struct CompileLogDrain {
    pub(super) keep_session: bool,
    pub(super) finished_successfully: bool,
    pub(super) lines: Vec<String>,
}
