use std::sync::atomic::{AtomicBool, Ordering};

use objc2_app_kit::{NSAutoresizingMaskOptions, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::{
    content_frame_for_host,
    content_view_autoresizing_mask,
    content_view_masks_to_bounds,
    editor_window_style_mask,
    embedded_subview_extent,
    host_window_frame_resized_from_bottom,
    host_window_frame_resized_from_bottom_clamped,
    host_window_frame_resized_from_top,
    native_content_resize_tracking_enabled,
};
use crate::{Rect, host_layout};

fn assert_f64_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= f64::EPSILON,
        "expected {actual} to equal {expected}"
    );
}

#[test]
fn content_frame_uses_bottom_left_coordinates_for_normal_appkit_views() {
    let layout = host_layout(820.0, 456.0, 30.0, 4.0);

    assert_eq!(
        content_frame_for_host(layout, false),
        Rect {
            x: 4.0,
            y: 4.0,
            width: 820.0,
            height: 456.0,
        }
    );
}

#[test]
fn content_frame_moves_below_titlebar_for_flipped_appkit_views() {
    let layout = host_layout(820.0, 456.0, 30.0, 4.0);

    assert_eq!(
        content_frame_for_host(layout, true),
        Rect {
            x: 4.0,
            y: 34.0,
            width: 820.0,
            height: 456.0,
        }
    );
}

#[test]
fn live_resize_grows_host_upward_to_keep_content_position() {
    let frame = NSRect::new(NSPoint::new(100.0, 200.0), NSSize::new(440.0, 400.0));

    let resized = host_window_frame_resized_from_bottom(frame, 440.0, 500.0);

    assert_f64_close(resized.origin.y, 200.0);
    assert_f64_close(resized.size.height, 500.0);
}

#[test]
fn live_resize_clamps_header_top_to_visible_screen() {
    let frame = NSRect::new(NSPoint::new(100.0, 200.0), NSSize::new(440.0, 400.0));
    let visible = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0));

    let resized = host_window_frame_resized_from_bottom_clamped(frame, 440.0, 1000.0, visible);

    assert_f64_close(resized.origin.y + resized.size.height, 900.0);
}

#[test]
fn content_host_view_autoresizes_with_host_during_native_live_resize() {
    let mask = content_view_autoresizing_mask();

    assert!(mask.contains(NSAutoresizingMaskOptions::ViewWidthSizable));
    assert!(mask.contains(NSAutoresizingMaskOptions::ViewHeightSizable));
}

#[test]
fn content_host_view_clips_embedded_plugins_to_content_area() {
    assert!(content_view_masks_to_bounds());
}

#[test]
fn plugin_owned_resize_size_includes_subview_origin() {
    let frame = NSRect::new(NSPoint::new(24.0, 16.0), NSSize::new(800.0, 600.0));

    assert_eq!(
        embedded_subview_extent(
            frame,
            crate::Size {
                width: 800.0,
                height: 600.0,
            },
            crate::Size {
                width: 0.0,
                height: 0.0,
            }
        ),
        crate::Size {
            width: 824.0,
            height: 616.0,
        }
    );
}

#[test]
fn plugin_owned_resize_size_includes_nested_subview_extent() {
    let frame = NSRect::new(NSPoint::new(10.0, 20.0), NSSize::new(100.0, 100.0));

    assert_eq!(
        embedded_subview_extent(
            frame,
            crate::Size {
                width: 100.0,
                height: 100.0,
            },
            crate::Size {
                width: 180.0,
                height: 140.0,
            }
        ),
        crate::Size {
            width: 190.0,
            height: 160.0,
        }
    );
}

#[test]
fn native_content_resize_tracking_is_opt_in() {
    let enabled = AtomicBool::new(false);

    assert!(!native_content_resize_tracking_enabled(&enabled));

    enabled.store(true, Ordering::Release);

    assert!(native_content_resize_tracking_enabled(&enabled));
}

#[test]
fn plugin_owned_resize_size_uses_bounds_when_frame_is_unchanged() {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0));

    assert_eq!(
        embedded_subview_extent(
            frame,
            crate::Size {
                width: 512.0,
                height: 384.0,
            },
            crate::Size {
                width: 0.0,
                height: 0.0,
            }
        ),
        crate::Size {
            width: 512.0,
            height: 384.0,
        }
    );
}

#[test]
fn resizable_editor_window_uses_app_controlled_resize() {
    let mask = editor_window_style_mask(NSWindowStyleMask::Miniaturizable);

    assert!(mask.contains(NSWindowStyleMask::Closable));
    assert!(!mask.contains(NSWindowStyleMask::Resizable));
    assert!(!mask.contains(NSWindowStyleMask::Miniaturizable));
}

#[test]
fn fixed_editor_window_disables_native_resize() {
    let mask = editor_window_style_mask(NSWindowStyleMask::Resizable);

    assert!(mask.contains(NSWindowStyleMask::Closable));
    assert!(!mask.contains(NSWindowStyleMask::Resizable));
}

#[test]
fn initial_resize_keeps_existing_top_anchor() {
    let frame = NSRect::new(NSPoint::new(100.0, 200.0), NSSize::new(440.0, 400.0));

    let resized = host_window_frame_resized_from_top(frame, 440.0, 500.0);

    assert_f64_close(resized.origin.y, 100.0);
    assert_f64_close(resized.size.height, 500.0);
}
