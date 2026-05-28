//! Minimal egui host on top of parented baseview windows.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

mod input;
mod window;

use input::*;
#[cfg(test)]
use window::AtomicResizeRequest;
pub use window::*;
#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use keyboard_types::Key;

    #[test]
    fn egui_key_maps_navigation_keys() {
        assert_eq!(super::egui_key(&Key::ArrowDown), Some(egui::Key::ArrowDown));
        assert_eq!(super::egui_key(&Key::ArrowLeft), Some(egui::Key::ArrowLeft));
        assert_eq!(
            super::egui_key(&Key::ArrowRight),
            Some(egui::Key::ArrowRight)
        );
        assert_eq!(super::egui_key(&Key::ArrowUp), Some(egui::Key::ArrowUp));
        assert_eq!(super::egui_key(&Key::Home), Some(egui::Key::Home));
        assert_eq!(super::egui_key(&Key::End), Some(egui::Key::End));
        assert_eq!(super::egui_key(&Key::PageUp), Some(egui::Key::PageUp));
        assert_eq!(super::egui_key(&Key::PageDown), Some(egui::Key::PageDown));
    }

    #[test]
    fn egui_key_maps_editing_keys() {
        assert_eq!(super::egui_key(&Key::Escape), Some(egui::Key::Escape));
        assert_eq!(super::egui_key(&Key::Tab), Some(egui::Key::Tab));
        assert_eq!(super::egui_key(&Key::Backspace), Some(egui::Key::Backspace));
        assert_eq!(super::egui_key(&Key::Enter), Some(egui::Key::Enter));
        assert_eq!(super::egui_key(&Key::Delete), Some(egui::Key::Delete));
    }

    #[test]
    fn egui_key_maps_shortcut_characters_case_insensitively() {
        assert_eq!(
            super::egui_key(&Key::Character("0".into())),
            Some(egui::Key::Num0)
        );
        assert_eq!(
            super::egui_key(&Key::Character("9".into())),
            Some(egui::Key::Num9)
        );
        assert_eq!(
            super::egui_key(&Key::Character("a".into())),
            Some(egui::Key::A)
        );
        assert_eq!(
            super::egui_key(&Key::Character("C".into())),
            Some(egui::Key::C)
        );
        assert_eq!(
            super::egui_key(&Key::Character("v".into())),
            Some(egui::Key::V)
        );
        assert_eq!(
            super::egui_key(&Key::Character("X".into())),
            Some(egui::Key::X)
        );
        assert_eq!(
            super::egui_key(&Key::Character("z".into())),
            Some(egui::Key::Z)
        );
    }

    #[test]
    fn egui_key_rejects_text_without_egui_key_equivalent() {
        assert_eq!(super::egui_key(&Key::Character("b".into())), None);
        assert_eq!(super::egui_key(&Key::Character("ab".into())), None);
    }

    #[test]
    fn clears_parented_egui_view_to_transparent() {
        assert!(
            super::clear_color()
                .into_iter()
                .all(|value| value.abs() <= f32::EPSILON)
        );
    }

    #[test]
    fn initializes_parented_egui_view_to_requested_size() {
        let info = super::initial_window_info(super::Size::new(640.0, 514.0));

        assert_eq!(info.logical_size(), super::Size::new(640.0, 514.0));
        assert_eq!(info.physical_size().width, 640);
        assert_eq!(info.physical_size().height, 514);
    }

    #[test]
    fn zero_sized_parented_egui_view_is_not_renderable() {
        assert_eq!(super::renderable_screen_size(0, 480), None);
        assert_eq!(super::renderable_screen_size(640, 0), None);
        assert_eq!(super::renderable_screen_size(640, 480), Some([640, 480]));
    }

    #[test]
    fn programmatic_resize_echoes_are_ignored_even_when_out_of_order() {
        let mut echoes = VecDeque::new();
        super::record_programmatic_resize_echo(&mut echoes, super::Size::new(415.0, 449.0));
        super::record_programmatic_resize_echo(&mut echoes, super::Size::new(433.0, 467.0));

        assert!(super::consume_programmatic_resize_echo(
            &mut echoes,
            super::Size::new(433.0, 467.0)
        ));
        assert!(super::consume_programmatic_resize_echo(
            &mut echoes,
            super::Size::new(415.0, 449.0)
        ));
        assert!(echoes.is_empty());
    }

    #[test]
    fn current_programmatic_size_blocks_stale_native_sync() {
        let mut echoes = VecDeque::new();
        let current = super::Size::new(433.0, 467.0);
        super::record_programmatic_resize_echo(&mut echoes, current);

        assert!(super::pending_programmatic_resize(&echoes, current));
        assert!(!super::pending_programmatic_resize(
            &echoes,
            super::Size::new(415.0, 449.0)
        ));
    }

    #[test]
    fn atomic_resize_request_returns_latest_size_once() {
        let request = super::AtomicResizeRequest::new();

        request.store(super::Size::new(400.0, 434.0));
        request.store(super::Size::new(433.0, 467.0));

        assert_eq!(request.unconsumed(), Some(super::Size::new(433.0, 467.0)));
        assert_eq!(request.take(), Some(super::Size::new(433.0, 467.0)));
        assert_eq!(request.unconsumed(), None);
        assert_eq!(request.take(), None);
    }

    #[test]
    fn macos_pending_resize_does_not_reapply_native_window_on_frame() {
        assert_eq!(
            super::pending_resize_reapplies_native_window_on_frame(),
            !cfg!(target_os = "macos")
        );
    }

    #[test]
    fn external_resize_updates_child_view_immediately() {
        assert_eq!(
            super::external_resize_updates_child_view_immediately(),
            !cfg!(target_os = "macos")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn parented_egui_view_autoresizes_with_parent_on_macos() {
        assert_eq!(
            super::explicit_resize_autoresizing_mask(),
            objc2_app_kit::NSAutoresizingMaskOptions::ViewWidthSizable
                | objc2_app_kit::NSAutoresizingMaskOptions::ViewHeightSizable
        );
    }

    #[test]
    fn atomic_resize_request_never_returns_torn_size() {
        let request = Arc::new(super::AtomicResizeRequest::new());
        let mut writers = Vec::new();

        for writer in 0..4 {
            let request = Arc::clone(&request);
            writers.push(std::thread::spawn(move || {
                for index in 0..2_000 {
                    let width = f64::from(writer * 10_000 + index);
                    request.store(super::Size::new(width, width + 1.0));
                }
            }));
        }

        for _ in 0..8_000 {
            if let Some(size) = request.take() {
                assert!((size.height - (size.width + 1.0)).abs() <= f64::EPSILON);
            }
            std::thread::yield_now();
        }

        for writer in writers {
            writer.join().expect("writer should not panic");
        }
        if let Some(size) = request.take() {
            assert!((size.height - (size.width + 1.0)).abs() <= f64::EPSILON);
        }
    }
}
