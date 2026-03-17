use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

pub(crate) struct ScoreWatcher {
    watched_path: PathBuf,
    events: Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
}

impl ScoreWatcher {
    pub(crate) fn start(path: &Path) -> Result<Self, notify::Error> {
        let watched_path = path.to_path_buf();
        let (event_tx, event_rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })?;

        watcher.watch(path, RecursiveMode::NonRecursive)?;

        Ok(Self {
            watched_path,
            events: event_rx,
            _watcher: watcher,
        })
    }

    pub(crate) fn watched_path(&self) -> &Path {
        &self.watched_path
    }

    pub(crate) fn try_recv(&self) -> Result<notify::Result<Event>, TryRecvError> {
        self.events.try_recv()
    }
}
