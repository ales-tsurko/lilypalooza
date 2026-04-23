use std::collections::HashMap;

use auxiliary_window::WindowSnapshot;
use iced::window;
use lilypalooza_audio::{EditorError, EditorParent, EditorSession};

/// Mixer-strip processor target.
///
/// `strip_index` follows the visible mixer strip order:
/// - `0` is the master strip
/// - `1..=track_count` are instrument tracks
/// - the remaining indices are bus strips
///
/// `slot_index` follows one shared convention on every strip:
/// - `0` is the instrument slot
/// - `1..` are effect slots
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct EditorTarget {
    pub(super) strip_index: usize,
    pub(super) slot_index: usize,
}

pub(super) struct EditorWindow {
    pub(super) title: String,
    pub(super) resizable: bool,
    pub(super) host_window_id: window::Id,
    pub(super) host_snapshot: Option<WindowSnapshot>,
    pub(super) session: Box<dyn EditorSession>,
}

pub(super) struct PendingEditorWindow {
    pub(super) target: EditorTarget,
    pub(super) title: String,
    pub(super) resizable: bool,
    pub(super) host_window_id: window::Id,
    pub(super) session: Box<dyn EditorSession>,
}

#[derive(Default)]
pub(super) struct EditorWindowManager {
    windows: HashMap<EditorTarget, EditorWindow>,
    pending: HashMap<window::Id, PendingEditorWindow>,
    windows_by_id: HashMap<window::Id, EditorTarget>,
    focused: Option<EditorTarget>,
}

pub(super) fn snapshot_into_editor_parent(
    snapshot: WindowSnapshot,
) -> Result<EditorParent, String> {
    let window = snapshot
        .raw_window_handle()
        .map_err(|error| error.to_string())?;
    let display = snapshot
        .raw_display_handle()
        .map_err(|error| error.to_string())?;
    Ok(EditorParent { window, display })
}

impl EditorWindowManager {
    pub(super) fn focus_existing(&mut self, target: EditorTarget) -> Option<window::Id> {
        if let Some(window) = self.windows.get(&target) {
            self.focused = Some(target);
            return Some(window.host_window_id);
        }
        if let Some((window_id, _)) = self
            .pending
            .iter()
            .find(|(_, window)| window.target == target)
        {
            self.focused = Some(target);
            return Some(*window_id);
        }
        None
    }

    pub(super) fn begin_open(
        &mut self,
        target: EditorTarget,
        title: String,
        resizable: bool,
        session: Box<dyn EditorSession>,
        window_id: window::Id,
    ) {
        self.pending.insert(
            window_id,
            PendingEditorWindow {
                target,
                title,
                resizable,
                host_window_id: window_id,
                session,
            },
        );
        self.focused = Some(target);
    }

    pub(super) fn attach(
        &mut self,
        window_id: window::Id,
        host_snapshot: Option<WindowSnapshot>,
        parent: EditorParent,
    ) -> Result<(), EditorError> {
        let Some(mut pending) = self.pending.remove(&window_id) else {
            return Err(EditorError::HostUnavailable(format!(
                "pending editor window `{window_id:?}` is missing"
            )));
        };
        pending.session.attach(parent)?;
        self.focused = Some(pending.target);
        self.windows.insert(
            pending.target,
            EditorWindow {
                title: pending.title,
                resizable: pending.resizable,
                host_window_id: pending.host_window_id,
                host_snapshot,
                session: pending.session,
            },
        );
        self.windows_by_id.insert(window_id, pending.target);
        Ok(())
    }

    pub(super) fn pending_contains(&self, window_id: window::Id) -> bool {
        self.pending.contains_key(&window_id)
    }

    pub(super) fn window_title(&self, window_id: window::Id) -> Option<&str> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.title.as_str()))
            .or_else(|| {
                self.pending
                    .get(&window_id)
                    .map(|window| window.title.as_str())
            })
    }

    pub(super) fn window_resizable(&self, window_id: window::Id) -> Option<bool> {
        self.windows_by_id
            .get(&window_id)
            .and_then(|target| self.windows.get(target).map(|window| window.resizable))
            .or_else(|| self.pending.get(&window_id).map(|window| window.resizable))
    }

    pub(super) fn remove_window(
        &mut self,
        window_id: window::Id,
    ) -> Option<(EditorTarget, Box<dyn EditorSession>)> {
        if let Some(pending) = self.pending.remove(&window_id) {
            if self.focused == Some(pending.target) {
                self.focused = None;
            }
            return Some((pending.target, pending.session));
        }

        let target = self.windows_by_id.remove(&window_id)?;
        let window = self.windows.remove(&target)?;
        if self.focused == Some(target) {
            self.focused = None;
        }
        Some((target, window.session))
    }

    pub(super) fn remove_target(
        &mut self,
        target: EditorTarget,
    ) -> Option<(window::Id, Box<dyn EditorSession>)> {
        let window = self.windows.remove(&target)?;
        self.windows_by_id.remove(&window.host_window_id);
        if self.focused == Some(target) {
            self.focused = None;
        }
        Some((window.host_window_id, window.session))
    }

    pub(super) fn remove_all_windows(&mut self) -> Vec<(window::Id, Box<dyn EditorSession>)> {
        let windows = self
            .windows
            .drain()
            .map(|(target, window)| {
                if self.focused == Some(target) {
                    self.focused = None;
                }
                (window.host_window_id, window.session)
            })
            .collect::<Vec<_>>();
        self.windows_by_id.clear();
        self.pending.clear();
        windows
    }

    pub(super) fn hide_window(
        &mut self,
        window_id: window::Id,
    ) -> Option<(EditorTarget, Option<WindowSnapshot>, &mut dyn EditorSession)> {
        let target = *self.windows_by_id.get(&window_id)?;
        let window = self.windows.get_mut(&target)?;
        let snapshot = window.host_snapshot;
        if self.focused == Some(target) {
            self.focused = None;
        }
        Some((target, snapshot, window.session.as_mut()))
    }

    pub(super) fn hide_all_windows(
        &mut self,
    ) -> Vec<(Option<WindowSnapshot>, Result<(), EditorError>)> {
        self.focused = None;
        self.windows
            .values_mut()
            .map(|window| {
                let visibility = window.session.set_visible(false);
                (window.host_snapshot, visibility)
            })
            .collect()
    }

    pub(super) fn host_snapshot_for(&self, window_id: window::Id) -> Option<WindowSnapshot> {
        let target = *self.windows_by_id.get(&window_id)?;
        self.windows.get(&target)?.host_snapshot
    }

    pub(super) fn session_mut(&mut self, window_id: window::Id) -> Option<&mut dyn EditorSession> {
        let target = *self.windows_by_id.get(&window_id)?;
        Some(self.windows.get_mut(&target)?.session.as_mut())
    }
}

#[cfg(test)]
impl EditorWindowManager {
    pub(super) fn contains_window(&self, target: EditorTarget) -> bool {
        self.windows.contains_key(&target)
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use auxiliary_window::WindowSnapshot;
    use iced::window;
    use lilypalooza_audio::{EditorError, EditorParent, EditorSession, EditorSize};

    use super::{EditorTarget, EditorWindowManager, snapshot_into_editor_parent};

    fn fake_snapshot() -> WindowSnapshot {
        WindowSnapshot::capture(
            iced::window::raw_window_handle::RawWindowHandle::AppKit(
                iced::window::raw_window_handle::AppKitWindowHandle::new(
                    NonNull::<std::ffi::c_void>::dangling(),
                ),
            ),
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(
                iced::window::raw_window_handle::AppKitDisplayHandle::new(),
            )),
        )
        .expect("snapshot should capture appkit")
    }

    struct FakeEditorSession;

    impl EditorSession for FakeEditorSession {
        fn attach(&mut self, _parent: EditorParent) -> Result<(), EditorError> {
            Ok(())
        }

        fn detach(&mut self) -> Result<(), EditorError> {
            Ok(())
        }

        fn set_visible(&mut self, _visible: bool) -> Result<(), EditorError> {
            Ok(())
        }

        fn resize(&mut self, _size: EditorSize) -> Result<(), EditorError> {
            Ok(())
        }
    }

    #[test]
    fn processor_editor_window_manager_reuses_existing_target_window() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 3,
            slot_index: 0,
        };

        let first_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 4".to_string(),
            true,
            Box::new(FakeEditorSession),
            first_id,
        );
        let second_token = manager.focus_existing(target);

        assert_eq!(Some(first_id), second_token);
        assert_eq!(manager.focused, Some(target));
    }

    #[test]
    fn processor_editor_window_manager_attaches_pending_session_once_parent_arrives() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };

        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 1".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );

        manager
            .attach(
                window_id,
                Some(fake_snapshot()),
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");
        assert!(manager.windows.contains_key(&target));
    }

    #[test]
    fn editor_parent_snapshot_roundtrips_appkit_window_handle() {
        let snapshot = WindowSnapshot::capture(
            iced::window::raw_window_handle::RawWindowHandle::AppKit(
                iced::window::raw_window_handle::AppKitWindowHandle::new(
                    NonNull::<std::ffi::c_void>::dangling(),
                ),
            ),
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(
                iced::window::raw_window_handle::AppKitDisplayHandle::new(),
            )),
        )
        .expect("snapshot should capture appkit");

        let parent = snapshot_into_editor_parent(snapshot).expect("snapshot should restore appkit");

        assert!(matches!(
            parent.window,
            iced::window::raw_window_handle::RawWindowHandle::AppKit(_)
        ));
        assert!(matches!(
            parent.display,
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(_))
        ));
    }

    #[test]
    fn processor_editor_window_manager_removes_window_by_host_id() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 2,
            slot_index: 1,
        };
        let window_id = window::Id::unique();
        manager.begin_open(
            target,
            "Track 2".to_string(),
            true,
            Box::new(FakeEditorSession),
            window_id,
        );
        manager
            .attach(
                window_id,
                Some(fake_snapshot()),
                EditorParent {
                    window: iced::window::raw_window_handle::RawWindowHandle::AppKit(
                        iced::window::raw_window_handle::AppKitWindowHandle::new(
                            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
                        ),
                    ),
                    display: None,
                },
            )
            .expect("attach should succeed");

        let removed = manager.remove_window(window_id);

        assert!(removed.is_some());
        assert!(!manager.windows.contains_key(&target));
    }
}
