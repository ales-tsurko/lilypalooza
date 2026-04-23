use std::collections::HashMap;
use std::ffi::c_void;
use std::num::{NonZeroIsize, NonZeroU32};
use std::ptr::NonNull;

use iced::window::raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, Win32WindowHandle, XcbDisplayHandle,
    XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};
use lilypalooza_audio::{EditorDescriptor, EditorError, EditorParent, EditorSession};

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
    pub(super) _title: String,
    pub(super) _descriptor: EditorDescriptor,
    pub(super) _session: Box<dyn EditorSession>,
}

pub(super) struct PendingEditorWindow {
    pub(super) target: EditorTarget,
    pub(super) _title: String,
    pub(super) _descriptor: EditorDescriptor,
    pub(super) session: Box<dyn EditorSession>,
}

pub(super) enum EditorOpenOutcome {
    Pending(u64),
    Opened,
    Focused,
}

#[derive(Default)]
pub(super) struct EditorWindowManager {
    next_window_token: u64,
    windows: HashMap<EditorTarget, EditorWindow>,
    pending: HashMap<u64, PendingEditorWindow>,
    focused: Option<EditorTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorWindowParent {
    AppKit {
        ns_view: usize,
    },
    Win32 {
        hwnd: isize,
    },
    Xcb {
        window: u32,
    },
    Xlib {
        window: u64,
    },
    Wayland {
        surface: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorDisplayParent {
    AppKit,
    Xcb {
        connection: Option<usize>,
        screen: i32,
    },
    Xlib {
        display: Option<usize>,
        screen: i32,
    },
    Wayland {
        display: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EditorParentSnapshot {
    pub(super) window: EditorWindowParent,
    pub(super) display: Option<EditorDisplayParent>,
}

impl EditorParentSnapshot {
    pub(super) fn capture(
        window: RawWindowHandle,
        display: Option<RawDisplayHandle>,
    ) -> Result<Self, String> {
        let window = match window {
            RawWindowHandle::AppKit(handle) => Self::capture_appkit_window(handle),
            RawWindowHandle::Win32(handle) => Self::capture_win32_window(handle),
            RawWindowHandle::Xcb(handle) => Self::capture_xcb_window(handle),
            RawWindowHandle::Xlib(handle) => Self::capture_xlib_window(handle),
            RawWindowHandle::Wayland(handle) => Self::capture_wayland_window(handle),
            other => return Err(format!("unsupported editor parent handle: {other:?}")),
        }?;
        let display = display
            .map(Self::capture_display)
            .transpose()?;
        Ok(Self { window, display })
    }

    pub(super) fn into_editor_parent(self) -> Result<EditorParent, String> {
        let window = match self.window {
            EditorWindowParent::AppKit { ns_view } => RawWindowHandle::AppKit(
                AppKitWindowHandle::new(non_null_ptr(ns_view, "ns_view")?),
            ),
            EditorWindowParent::Win32 { hwnd } => {
                RawWindowHandle::Win32(Win32WindowHandle::new(non_zero_isize(hwnd, "hwnd")?))
            }
            EditorWindowParent::Xcb { window } => {
                RawWindowHandle::Xcb(XcbWindowHandle::new(non_zero_u32(window, "xcb window")?))
            }
            EditorWindowParent::Xlib { window } => {
                RawWindowHandle::Xlib(XlibWindowHandle::new(window))
            }
            EditorWindowParent::Wayland { surface } => {
                RawWindowHandle::Wayland(WaylandWindowHandle::new(non_null_ptr(
                    surface,
                    "wayland surface",
                )?))
            }
        };
        let display = self
            .display
            .map(|display| match display {
                EditorDisplayParent::AppKit => Ok::<RawDisplayHandle, String>(RawDisplayHandle::AppKit(
                    AppKitDisplayHandle::new(),
                )),
                EditorDisplayParent::Xcb { connection, screen } => Ok::<RawDisplayHandle, String>(RawDisplayHandle::Xcb(
                    XcbDisplayHandle::new(connection.map(non_null_ptr_unchecked), screen),
                )),
                EditorDisplayParent::Xlib { display, screen } => Ok::<RawDisplayHandle, String>(RawDisplayHandle::Xlib(
                    XlibDisplayHandle::new(display.map(non_null_ptr_unchecked), screen),
                )),
                EditorDisplayParent::Wayland { display } => Ok::<RawDisplayHandle, String>(RawDisplayHandle::Wayland(
                    WaylandDisplayHandle::new(non_null_ptr(display, "wayland display")?),
                )),
            })
            .transpose()?;
        Ok(EditorParent { window, display })
    }

    fn capture_appkit_window(handle: AppKitWindowHandle) -> Result<EditorWindowParent, String> {
        Ok(EditorWindowParent::AppKit {
            ns_view: handle.ns_view.as_ptr() as usize,
        })
    }

    fn capture_win32_window(handle: Win32WindowHandle) -> Result<EditorWindowParent, String> {
        Ok(EditorWindowParent::Win32 {
            hwnd: handle.hwnd.get(),
        })
    }

    fn capture_xcb_window(handle: XcbWindowHandle) -> Result<EditorWindowParent, String> {
        Ok(EditorWindowParent::Xcb {
            window: handle.window.get(),
        })
    }

    fn capture_xlib_window(handle: XlibWindowHandle) -> Result<EditorWindowParent, String> {
        Ok(EditorWindowParent::Xlib {
            window: handle.window,
        })
    }

    fn capture_wayland_window(handle: WaylandWindowHandle) -> Result<EditorWindowParent, String> {
        Ok(EditorWindowParent::Wayland {
            surface: handle.surface.as_ptr() as usize,
        })
    }

    fn capture_display(display: RawDisplayHandle) -> Result<EditorDisplayParent, String> {
        match display {
            RawDisplayHandle::AppKit(_) => Ok(EditorDisplayParent::AppKit),
            RawDisplayHandle::Xcb(handle) => Ok(EditorDisplayParent::Xcb {
                connection: handle.connection.map(|connection| connection.as_ptr() as usize),
                screen: handle.screen,
            }),
            RawDisplayHandle::Xlib(handle) => Ok(EditorDisplayParent::Xlib {
                display: handle.display.map(|display| display.as_ptr() as usize),
                screen: handle.screen,
            }),
            RawDisplayHandle::Wayland(handle) => Ok(EditorDisplayParent::Wayland {
                display: handle.display.as_ptr() as usize,
            }),
            other => Err(format!("unsupported editor display handle: {other:?}")),
        }
    }
}

impl EditorWindowManager {
    pub(super) fn begin_open_or_focus(
        &mut self,
        target: EditorTarget,
        title: String,
        descriptor: EditorDescriptor,
        session: Box<dyn EditorSession>,
    ) -> EditorOpenOutcome {
        if self.windows.contains_key(&target) {
            self.focused = Some(target);
            return EditorOpenOutcome::Focused;
        }
        if let Some((token, _)) = self.pending.iter().find(|(_, window)| window.target == target) {
            self.focused = Some(target);
            let _ = token;
            return EditorOpenOutcome::Focused;
        }

        let window_token = self.allocate_window_token();
        self.pending.insert(
            window_token,
            PendingEditorWindow {
                target,
                _title: title,
                _descriptor: descriptor,
                session,
            },
        );
        self.focused = Some(target);
        EditorOpenOutcome::Pending(window_token)
    }

    pub(super) fn attach(
        &mut self,
        window_token: u64,
        parent: EditorParent,
    ) -> Result<EditorOpenOutcome, EditorError> {
        let Some(mut pending) = self.pending.remove(&window_token) else {
            return Err(EditorError::HostUnavailable(format!(
                "pending editor window `{window_token}` is missing"
            )));
        };
        pending.session.attach(parent)?;
        self.focused = Some(pending.target);
        self.windows.insert(
            pending.target,
            EditorWindow {
                _title: pending._title,
                _descriptor: pending._descriptor,
                _session: pending.session,
            },
        );
        Ok(EditorOpenOutcome::Opened)
    }

    fn allocate_window_token(&mut self) -> u64 {
        self.next_window_token = self.next_window_token.saturating_add(1);
        self.next_window_token
    }
}

#[cfg(test)]
impl EditorWindowManager {
    pub(super) fn is_open(&self, target: EditorTarget) -> bool {
        self.windows.contains_key(&target)
    }
}

fn non_null_ptr(value: usize, name: &str) -> Result<NonNull<c_void>, String> {
    NonNull::new(value as *mut c_void).ok_or_else(|| format!("{name} is null"))
}

fn non_null_ptr_unchecked(value: usize) -> NonNull<c_void> {
    NonNull::new(value as *mut c_void).expect("stored non-null pointer")
}

fn non_zero_isize(value: isize, name: &str) -> Result<NonZeroIsize, String> {
    NonZeroIsize::new(value).ok_or_else(|| format!("{name} is zero"))
}

fn non_zero_u32(value: u32, name: &str) -> Result<NonZeroU32, String> {
    NonZeroU32::new(value).ok_or_else(|| format!("{name} is zero"))
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use lilypalooza_audio::{
        EditorDescriptor, EditorError, EditorParent, EditorSession, EditorSize,
    };

    use super::{
        EditorOpenOutcome, EditorParentSnapshot, EditorTarget, EditorWindowManager,
    };

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

        let first = manager.begin_open_or_focus(
            target,
            "Track 4".to_string(),
            descriptor(),
            Box::new(FakeEditorSession),
        );
        let second = manager.begin_open_or_focus(
            target,
            "Track 4".to_string(),
            descriptor(),
            Box::new(FakeEditorSession),
        );

        let first_token = match first {
            EditorOpenOutcome::Pending(token) => token,
            EditorOpenOutcome::Opened | EditorOpenOutcome::Focused => {
                panic!("first open should create pending window")
            }
        };
        let second_token = match second {
            EditorOpenOutcome::Opened | EditorOpenOutcome::Pending(_) => {
                panic!("second open should focus window")
            }
            EditorOpenOutcome::Focused => first_token,
        };

        assert_eq!(first_token, second_token);
        assert_eq!(manager.focused, Some(target));
    }

    #[test]
    fn processor_editor_window_manager_attaches_pending_session_once_parent_arrives() {
        let mut manager = EditorWindowManager::default();
        let target = EditorTarget {
            strip_index: 1,
            slot_index: 0,
        };

        let opened = manager.begin_open_or_focus(
            target,
            "Track 1".to_string(),
            descriptor(),
            Box::new(FakeEditorSession),
        );

        let token = match opened {
            EditorOpenOutcome::Pending(token) => token,
            EditorOpenOutcome::Opened | EditorOpenOutcome::Focused => {
                panic!("first open should become pending until attached")
            }
        };

        let attached = manager
            .attach(
                token,
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

        let attached_token = match attached {
            EditorOpenOutcome::Opened => token,
            EditorOpenOutcome::Pending(_) | EditorOpenOutcome::Focused => {
                panic!("attached window should be opened")
            }
        };

        assert_eq!(attached_token, token);
        assert!(manager.windows.contains_key(&target));
    }

    #[test]
    fn editor_parent_snapshot_roundtrips_appkit_window_handle() {
        let snapshot = EditorParentSnapshot::capture(
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

        let parent = snapshot
            .into_editor_parent()
            .expect("snapshot should restore appkit");

        assert!(matches!(
            parent.window,
            iced::window::raw_window_handle::RawWindowHandle::AppKit(_)
        ));
        assert!(matches!(
            parent.display,
            Some(iced::window::raw_window_handle::RawDisplayHandle::AppKit(_))
        ));
    }
}
