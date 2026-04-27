use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSColor, NSView, NSWindow, NSWindowAnimationBehavior,
    NSWindowCollectionBehavior, NSWindowOrderingMode, NSWindowStyleMask, NSWindowTabbingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;
use raw_window_handle::RawWindowHandle;

use crate::{
    EditorFrame, EditorHostOptions, Error, InstalledHost, Size, WindowHandleSnapshot,
    WindowSnapshot, open_egui_frame,
};

pub fn prepare_process() -> Result<(), Error> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(Error::Message(
            "editor host configuration must run on the main thread".to_string(),
        ));
    };
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
    Ok(())
}

pub fn install_editor_host(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
    frame: impl EditorFrame + Send + 'static,
) -> Result<InstalledHost, Error> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(Error::Message(
            "editor host configuration must run on the main thread".to_string(),
        ));
    };

    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;

    configure_window(&host_window, options, mtm);
    if let Some(owner) = options.owner {
        attach_child_window_to_owner(&host_window, &owner)?;
    }

    let requested_bounds = host_view.bounds();
    let layout = frame.layout(Size {
        width: requested_bounds.size.width,
        height: requested_bounds.size.height,
    });

    resize_host_window(&host_window, layout.outer_width, layout.outer_height);
    host_view.setFrameSize(NSSize::new(layout.outer_width, layout.outer_height));
    host_window.setOpaque(false);
    host_window.setBackgroundColor(Some(&NSColor::clearColor()));
    style_host_view(&host_view);

    let frame_host = open_egui_frame(
        *host,
        options,
        layout.outer_width,
        layout.outer_height,
        frame,
    )?;

    let content_frame = content_frame_for_host(layout, host_view.isFlipped());
    let content = build_content_view(content_frame, mtm);
    host_view.addSubview(&content);
    let content = WindowSnapshot::capture(
        RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(
            NonNull::new(Retained::as_ptr(&content) as *mut std::ffi::c_void)
                .expect("retained appkit content view pointer is non-null"),
        )),
        host.display.map(|_| {
            raw_window_handle::RawDisplayHandle::AppKit(
                raw_window_handle::AppKitDisplayHandle::new(),
            )
        }),
    )?;

    Ok(InstalledHost {
        host: *host,
        content,
        close_requested: frame_host.close_requested,
        title: frame_host.title,
        frame_window: Some(frame_host.window),
    })
}

pub fn set_host_window_visible(host: &WindowSnapshot, visible: bool) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;

    if visible {
        host_window.makeKeyAndOrderFront(None);
    } else {
        host_window.orderOut(None);
    }

    Ok(())
}

pub fn set_host_window_title(host: &WindowSnapshot, title: &str) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    host_window.setTitle(&NSString::from_str(title));
    Ok(())
}

fn attach_child_window_to_owner(
    child_window: &NSWindow,
    owner: &WindowSnapshot,
) -> Result<(), Error> {
    let owner_view = ns_view_from_snapshot(owner)?;
    let owner_window = owner_view
        .window()
        .ok_or_else(|| Error::Message("owner window is missing".to_string()))?;

    // SAFETY: Both windows are live AppKit windows on the main thread. AppKit uses this
    // relationship to keep auxiliary editor windows above and with the owning app window.
    unsafe {
        owner_window.addChildWindow_ordered(child_window, NSWindowOrderingMode::Above);
    }
    Ok(())
}

pub fn route_app_quit_to_window_close(window: &WindowSnapshot) -> Result<(), Error> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(Error::Message(
            "app quit routing must run on the main thread".to_string(),
        ));
    };
    let view = ns_view_from_snapshot(window)?;
    let window = view
        .window()
        .ok_or_else(|| Error::Message("main window is missing".to_string()))?;
    let app = NSApplication::sharedApplication(mtm);
    let Some(main_menu) = app.mainMenu() else {
        return Err(Error::Message("app menu is missing".to_string()));
    };
    let Some(app_menu_item) = main_menu.itemAtIndex(0) else {
        return Err(Error::Message("app menu item is missing".to_string()));
    };
    let Some(app_menu) = app_menu_item.submenu() else {
        return Err(Error::Message("app submenu is missing".to_string()));
    };
    let item_count = app_menu.numberOfItems();
    if item_count == 0 {
        return Err(Error::Message("app submenu is empty".to_string()));
    }
    let Some(quit_item) = app_menu.itemAtIndex(item_count - 1) else {
        return Err(Error::Message("quit menu item is missing".to_string()));
    };

    // SAFETY: The menu item and main window are live AppKit objects on the main thread.
    unsafe {
        quit_item.setTarget(Some(&window));
        quit_item.setAction(Some(sel!(performClose:)));
    }

    Ok(())
}

fn ns_view_from_snapshot(snapshot: &WindowSnapshot) -> Result<Retained<NSView>, Error> {
    let WindowHandleSnapshot::AppKit { ns_view } = snapshot.window else {
        return Err(Error::Message(
            "auxiliary editor host currently requires an AppKit view".to_string(),
        ));
    };

    // SAFETY: The snapshot stores an AppKit NSView pointer captured from a live window handle on
    // the main thread. We only use it immediately on the main thread to configure that window.
    let view = (ns_view as *mut AnyObject).cast::<NSView>();
    if view.is_null() {
        return Err(Error::Message("host AppKit view is null".to_string()));
    }
    // SAFETY: The pointer came from a live AppKit window handle and is still owned by that window.
    unsafe { Retained::retain(view) }
        .ok_or_else(|| Error::Message("host AppKit view is unavailable".to_string()))
}

fn configure_window(window: &NSWindow, options: &EditorHostOptions, mtm: MainThreadMarker) {
    let mut style_mask = window.styleMask();
    style_mask.insert(NSWindowStyleMask::Closable);
    style_mask.remove(NSWindowStyleMask::Miniaturizable);
    if options.resizable {
        style_mask.insert(NSWindowStyleMask::Resizable);
    } else {
        style_mask.remove(NSWindowStyleMask::Resizable);
    }

    let collection_behavior = window.collectionBehavior()
        | NSWindowCollectionBehavior::Auxiliary
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::FullScreenDisallowsTiling;

    window.setStyleMask(style_mask);
    window.setTitle(&NSString::from_str(&options.title));
    window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
    window.setHasShadow(true);
    window.setMovableByWindowBackground(true);
    window.setAnimationBehavior(NSWindowAnimationBehavior::UtilityWindow);
    window.setCollectionBehavior(collection_behavior);
    window.setTabbingMode(NSWindowTabbingMode::Disallowed);
    window.invalidateShadow();
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
}

pub fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    let event = host_window
        .currentEvent()
        .ok_or_else(|| Error::Message("host window drag event is missing".to_string()))?;

    host_window.performWindowDragWithEvent(&event);
    host_window.invalidateShadow();
    Ok(())
}

fn resize_host_window(window: &NSWindow, width: f64, height: f64) {
    let mut frame = window.frame();
    let delta_height = height - frame.size.height;
    frame.origin.y -= delta_height;
    frame.size = NSSize::new(width, height);
    window.setFrame_display(frame, false);
}

fn build_content_view(frame: crate::Rect, mtm: MainThreadMarker) -> Retained<NSView> {
    let content = NSView::initWithFrame(NSView::alloc(mtm), ns_rect(frame));
    content.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    style_view(&content, &NSColor::clearColor(), None, 0.0, 0.0);
    content
}

fn content_frame_for_host(layout: crate::EditorFrameLayout, host_is_flipped: bool) -> crate::Rect {
    if !host_is_flipped {
        return layout.content;
    }

    crate::Rect {
        y: layout.outer_height - layout.content.y - layout.content.height,
        ..layout.content
    }
}

fn style_host_view(view: &NSView) {
    style_view(view, &NSColor::clearColor(), None, 0.0, 0.0);
}

fn style_view(
    view: &NSView,
    background: &NSColor,
    border: Option<&NSColor>,
    corner_radius: f64,
    border_width: f64,
) {
    view.setWantsLayer(true);
    let layer = CALayer::layer();
    let background = background.CGColor();
    layer.setBackgroundColor(Some(&background));
    if let Some(border) = border {
        let border = border.CGColor();
        layer.setBorderColor(Some(&border));
    }
    layer.setBorderWidth(border_width);
    layer.setCornerRadius(corner_radius);
    layer.setMasksToBounds(corner_radius > 0.0);
    view.setLayer(Some(&layer));
}

fn ns_rect(frame: crate::Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(frame.x, frame.y),
        NSSize::new(frame.width, frame.height),
    )
}

#[cfg(test)]
mod tests {
    use crate::{Rect, host_layout};

    use super::content_frame_for_host;

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
}
