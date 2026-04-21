use std::collections::HashMap;

use lilypalooza_audio::{EditorDescriptor, EditorSession};

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
    pub(super) window_token: u64,
    pub(super) _title: String,
    pub(super) _descriptor: EditorDescriptor,
    pub(super) _session: Box<dyn EditorSession>,
}

#[allow(dead_code)]
pub(super) enum EditorOpenOutcome {
    Opened(u64),
    Focused(u64),
}

#[derive(Default)]
pub(super) struct EditorWindowManager {
    next_window_token: u64,
    windows: HashMap<EditorTarget, EditorWindow>,
    focused: Option<EditorTarget>,
}

impl EditorWindowManager {
    pub(super) fn open_or_focus(
        &mut self,
        target: EditorTarget,
        title: String,
        descriptor: EditorDescriptor,
        session: Box<dyn EditorSession>,
    ) -> EditorOpenOutcome {
        if let Some(window) = self.windows.get(&target) {
            self.focused = Some(target);
            return EditorOpenOutcome::Focused(window.window_token);
        }

        let window_token = self.allocate_window_token();
        self.windows.insert(
            target,
            EditorWindow {
                window_token,
                _title: title,
                _descriptor: descriptor,
                _session: session,
            },
        );
        self.focused = Some(target);
        EditorOpenOutcome::Opened(window_token)
    }

    #[allow(dead_code)]
    pub(super) fn close(&mut self, target: EditorTarget) -> Option<EditorWindow> {
        let removed = self.windows.remove(&target);
        if self.focused == Some(target) {
            self.focused = None;
        }
        removed
    }

    #[allow(dead_code)]
    pub(super) fn is_open(&self, target: EditorTarget) -> bool {
        self.windows.contains_key(&target)
    }

    #[allow(dead_code)]
    pub(super) fn window_token(&self, target: EditorTarget) -> Option<u64> {
        self.windows.get(&target).map(|window| window.window_token)
    }

    #[allow(dead_code)]
    pub(super) fn focused_target(&self) -> Option<EditorTarget> {
        self.focused
    }

    fn allocate_window_token(&mut self) -> u64 {
        self.next_window_token = self.next_window_token.saturating_add(1);
        self.next_window_token
    }
}

#[cfg(test)]
mod tests {
    use lilypalooza_audio::{
        EditorDescriptor, EditorError, EditorParent, EditorSession, EditorSize,
    };

    use super::{EditorOpenOutcome, EditorTarget, EditorWindowManager};

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

    fn descriptor() -> EditorDescriptor {
        EditorDescriptor {
            default_size: EditorSize {
                width: 640,
                height: 480,
            },
            min_size: None,
            resizable: true,
        }
    }

    #[test]
    fn processor_editor_window_manager_reuses_existing_target_window() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 3,
            slot_index: 0,
        };

        let first = manager.open_or_focus(
            target,
            "Track 4".to_string(),
            descriptor(),
            Box::new(FakeEditorSession),
        );
        let second = manager.open_or_focus(
            target,
            "Track 4".to_string(),
            descriptor(),
            Box::new(FakeEditorSession),
        );

        let first_token = match first {
            EditorOpenOutcome::Opened(token) => token,
            EditorOpenOutcome::Focused(_) => panic!("first open should create window"),
        };
        let second_token = match second {
            EditorOpenOutcome::Opened(_) => panic!("second open should focus window"),
            EditorOpenOutcome::Focused(token) => token,
        };

        assert_eq!(first_token, second_token);
        assert_eq!(manager.window_token(target), Some(first_token));
        assert_eq!(manager.focused_target(), Some(target));
    }
}
