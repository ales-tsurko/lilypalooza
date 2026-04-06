use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::state;

pub(crate) struct EditorFileWatcher {
    watched_files: HashSet<PathBuf>,
    watched_dirs: HashMap<PathBuf, usize>,
    events: Receiver<notify::Result<Event>>,
    watcher: RecommendedWatcher,
}

impl EditorFileWatcher {
    pub(crate) fn start() -> Result<Self, notify::Error> {
        let (event_tx, event_rx) = mpsc::channel();

        let watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })?;

        Ok(Self {
            watched_files: HashSet::new(),
            watched_dirs: HashMap::new(),
            events: event_rx,
            watcher,
        })
    }

    pub(crate) fn sync_paths(&mut self, paths: &[PathBuf]) -> Result<(), notify::Error> {
        let next_files: HashSet<_> = paths.iter().map(|path| state::normalize_path(path)).collect();

        let removed_files: Vec<_> = self
            .watched_files
            .difference(&next_files)
            .cloned()
            .collect();
        let added_files: Vec<_> = next_files
            .difference(&self.watched_files)
            .cloned()
            .collect();

        for path in &removed_files {
            let _ = self.watcher.unwatch(path);
            if let Some(parent) = path.parent() {
                self.unwatch_dir(parent)?;
            }
        }

        for path in &added_files {
            self.watcher.watch(path, RecursiveMode::NonRecursive)?;
            if let Some(parent) = path.parent() {
                self.watch_dir(parent)?;
            }
        }

        self.watched_files = next_files;
        Ok(())
    }

    pub(crate) fn try_recv(&self) -> Result<notify::Result<Event>, TryRecvError> {
        self.events.try_recv()
    }

    fn watch_dir(&mut self, path: &Path) -> Result<(), notify::Error> {
        let path = state::normalize_path(path);
        if let Some(count) = self.watched_dirs.get_mut(&path) {
            *count += 1;
            return Ok(());
        }

        self.watcher.watch(&path, RecursiveMode::NonRecursive)?;
        self.watched_dirs.insert(path, 1);
        Ok(())
    }

    fn unwatch_dir(&mut self, path: &Path) -> Result<(), notify::Error> {
        let path = state::normalize_path(path);
        let Some(count) = self.watched_dirs.get_mut(&path) else {
            return Ok(());
        };

        if *count > 1 {
            *count -= 1;
            return Ok(());
        }

        self.watched_dirs.remove(&path);
        self.watcher.unwatch(&path)
    }
}
