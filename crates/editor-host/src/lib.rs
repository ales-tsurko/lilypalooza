#![expect(
    missing_docs,
    reason = "editor-host public API docs need a dedicated documentation pass"
)]

use std::{
    ffi::c_void,
    num::{NonZeroIsize, NonZeroU32},
    ptr::NonNull,
    sync::{
        Arc,
        Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

mod host;

pub use host::*;
pub(crate) use host::{ResizeAnchor, SharedSize};
#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use raw_window_handle::{
        AppKitDisplayHandle,
        AppKitWindowHandle,
        RawDisplayHandle,
        RawWindowHandle,
    };

    use super::{
        EditorFrame,
        EditorFrameAction,
        EditorHostOptions,
        EditorHostState,
        Size,
        WindowSnapshot,
        host_layout,
    };

    fn assert_f64_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= f64::EPSILON,
            "expected {actual} to equal {expected}"
        );
    }

    #[test]
    fn window_snapshot_roundtrips_appkit() {
        let snapshot = WindowSnapshot::capture(
            RawWindowHandle::AppKit(AppKitWindowHandle::new(
                NonNull::<std::ffi::c_void>::dangling(),
            )),
            Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
        )
        .expect("snapshot should capture appkit");

        let window = snapshot
            .raw_window_handle()
            .expect("snapshot should restore appkit window");
        let display = snapshot
            .raw_display_handle()
            .expect("snapshot should restore appkit display");

        assert!(matches!(window, RawWindowHandle::AppKit(_)));
        assert!(matches!(display, Some(RawDisplayHandle::AppKit(_))));
    }

    #[test]
    fn host_layout_puts_titlebar_above_content() {
        let layout = host_layout(440.0, 360.0, 30.0, 4.0);

        assert_f64_close(layout.content.height, 360.0);
        assert!(layout.titlebar.y >= layout.content.y + layout.content.height);
        assert!(layout.outer_height > 360.0);
    }

    #[test]
    fn host_layout_keeps_content_unclipped_inside_frame() {
        let layout = host_layout(440.0, 360.0, 30.0, 4.0);

        assert_f64_close(layout.content.width, 440.0);
        assert_f64_close(layout.content.height, 360.0);
        assert_f64_close(layout.content.x, 4.0);
        assert_f64_close(layout.content.y, 4.0);
    }

    #[test]
    fn host_layout_adds_frame_to_content_slot() {
        let layout = host_layout(820.0, 456.0, 30.0, 4.0);

        assert_f64_close(layout.outer_width, 828.0);
        assert_f64_close(layout.outer_height, 494.0);
        assert_f64_close(layout.content.width, 820.0);
        assert_f64_close(layout.content.height, 456.0);
        assert_f64_close(layout.titlebar.height, 30.0);
        assert_f64_close(layout.titlebar.y, 460.0);
    }

    #[test]
    fn content_size_from_outer_size_removes_frame_and_titlebar() {
        assert_eq!(
            super::content_size_from_outer_size(
                Size {
                    width: 828.0,
                    height: 494.0,
                },
                30.0,
                4.0,
            ),
            Size {
                width: 820.0,
                height: 456.0,
            }
        );
    }

    #[test]
    fn outer_size_from_content_size_adds_frame_and_titlebar() {
        assert_eq!(
            super::outer_size_from_content_size(
                Size {
                    width: 640.0,
                    height: 480.0,
                },
                34.0,
                2.0,
            ),
            Size {
                width: 644.0,
                height: 518.0,
            }
        );
    }

    #[test]
    fn editor_frame_trait_is_the_only_frame_customization_api() {
        struct TestFrame;

        impl EditorFrame for TestFrame {
            fn layout(&self, content_size: Size) -> super::EditorFrameLayout {
                host_layout(content_size.width, content_size.height, 30.0, 4.0)
            }

            fn render(
                &mut self,
                _ui: &mut super::egui::Ui,
                _state: &EditorHostState,
            ) -> EditorFrameAction {
                EditorFrameAction::None
            }
        }

        let frame = TestFrame;
        let layout = frame.layout(Size {
            width: 440.0,
            height: 360.0,
        });
        assert_f64_close(layout.content.width, 440.0);
        assert_f64_close(layout.content.height, 360.0);
    }

    #[test]
    fn installed_host_exposes_frame_commands() {
        let (mut host, commands) = super::InstalledHost::test_with_frame_commands([
            super::EditorFrameCommand::PreviousPreset,
            super::EditorFrameCommand::LoadPreset("preset-1".to_string()),
        ]);

        assert_eq!(host.drain_frame_commands(), commands);
        assert!(host.drain_frame_commands().is_empty());
    }

    #[test]
    fn installed_host_stores_preset_state_for_frame() {
        let (mut host, _) = super::InstalledHost::test_with_frame_commands([]);
        let state = super::EditorPresetState {
            current_name: "Warm Piano".to_string(),
            selected_id: Some("preset-1".to_string()),
            expanded: false,
            items: vec![super::EditorPresetItem {
                id: "preset-1".to_string(),
                name: "Warm Piano".to_string(),
                origin: super::EditorPresetOrigin::User,
            }],
        };

        host.set_preset_state(Some(state.clone()));

        assert_eq!(host.preset_state(), Some(state));
    }

    #[test]
    fn installed_host_derives_content_size_from_current_chrome() {
        let (host, _) = super::InstalledHost::test_with_frame_commands([]);

        assert_eq!(
            host.content_size_from_outer_size(Size {
                width: 648.0,
                height: 522.0,
            }),
            Size {
                width: 640.0,
                height: 480.0,
            }
        );
    }

    #[test]
    fn installed_host_updates_frame_content_size_when_content_resizes() {
        let (mut host, _) = super::InstalledHost::test_with_frame_commands([]);

        host.resize_content(Size {
            width: 512.0,
            height: 384.0,
        })
        .expect("test host without native window should still update state");

        assert_eq!(
            host.frame_content_size.load(),
            Size {
                width: 512.0,
                height: 384.0,
            }
        );
    }

    #[test]
    fn installed_host_resize_handle_updates_zoom_percent() {
        let (host, _) = super::InstalledHost::test_with_frame_commands([]);

        host.resize_handle().set_zoom_percent(125);

        assert_eq!(
            host.frame_zoom_percent
                .load(std::sync::atomic::Ordering::Relaxed),
            125
        );
    }

    #[test]
    fn installed_host_previews_outer_resize_without_accepting_content_size() {
        let (mut host, _) = super::InstalledHost::test_with_frame_commands([]);

        host.preview_outer_resize(Size {
            width: 512.0,
            height: 456.0,
        });

        assert_eq!(
            host.content_size(),
            Size {
                width: 440.0,
                height: 360.0,
            }
        );
        assert_eq!(
            host.frame_content_size.load(),
            Size {
                width: 504.0,
                height: 414.0,
            }
        );
    }

    #[test]
    fn installed_host_adopts_outer_resize_without_native_writeback() {
        let (mut host, _) = super::InstalledHost::test_with_frame_commands([]);

        host.adopt_content_size_from_outer_resize(Size {
            width: 512.0,
            height: 384.0,
        })
        .expect("test host without native window should still update state");

        assert_f64_close(host.content_size().width, 512.0);
        assert_eq!(
            host.frame_content_size.load(),
            Size {
                width: 512.0,
                height: 384.0,
            }
        );
    }

    #[test]
    fn installed_host_ignores_same_content_size_resize() {
        let (mut host, _) = super::InstalledHost::test_with_frame_commands([]);

        host.resize_content(Size {
            width: 440.25,
            height: 359.75,
        })
        .expect("same size resize should be a no-op");

        assert_eq!(
            host.frame_content_size.load(),
            Size {
                width: 440.0,
                height: 360.0,
            }
        );
    }

    #[test]
    fn host_options_default_to_resizable() {
        assert!(EditorHostOptions::new("Editor").resizable);
    }

    #[test]
    fn host_options_default_to_no_owner() {
        assert_eq!(EditorHostOptions::new("Editor").owner, None);
    }

    #[test]
    fn host_options_can_disable_resizing() {
        assert!(
            !EditorHostOptions::new("Editor")
                .with_resizable(false)
                .resizable
        );
    }

    #[test]
    fn host_options_can_store_owner_window() {
        let owner = WindowSnapshot::capture(
            RawWindowHandle::AppKit(AppKitWindowHandle::new(
                NonNull::<std::ffi::c_void>::dangling(),
            )),
            Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
        )
        .expect("snapshot should capture appkit");

        assert_eq!(
            EditorHostOptions::new("Editor").with_owner(owner).owner,
            Some(owner)
        );
    }
}
