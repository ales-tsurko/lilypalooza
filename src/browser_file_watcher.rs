use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::state;

pub(crate) struct BrowserFileWatcher {
    watched_root: PathBuf,
    events: Receiver<notify::Result<Event>>,
    _watcher: RecommendedWatcher,
}

impl BrowserFileWatcher {
    pub(crate) fn start(path: &Path) -> Result<Self, notify::Error> {
        let watched_root = state::normalize_path(path);
        let (event_tx, event_rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })?;
        watcher.watch(&watched_root, RecursiveMode::Recursive)?;

        Ok(Self {
            watched_root,
            events: event_rx,
            _watcher: watcher,
        })
    }

    pub(crate) fn watched_root(&self) -> &Path {
        &self.watched_root
    }

    pub(crate) fn try_recv(&self) -> Result<notify::Result<Event>, TryRecvError> {
        self.events.try_recv()
    }
}
