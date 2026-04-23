use std::mem;
use std::ptr::NonNull;
use std::sync::Once;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBezelStyle, NSButton, NSButtonType, NSColor,
    NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSTextField, NSView, NSWindow,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowOrderingMode, NSWindowStyleMask,
    NSWindowTabbingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;
use raw_window_handle::RawWindowHandle;

use crate::{Error, HostOptions, InstalledHost, WindowHandleSnapshot, WindowSnapshot, host_layout};

const TITLEBAR_BUTTON_SIZE: f64 = 14.0;
const TITLEBAR_BUTTON_LEFT: f64 = 10.0;
const TITLEBAR_BUTTON_Y_OFFSET: f64 = 1.0;
const TITLEBAR_LABEL_LEFT: f64 = 38.0;
const TITLEBAR_LABEL_RIGHT: f64 = 12.0;
const MACOS_Q_KEY_CODE: u16 = 12;
static INSTALL_COMMAND_MONITOR: Once = Once::new();

pub fn prepare_process() -> Result<(), Error> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(Error::Message(
            "editor host configuration must run on the main thread".to_string(),
        ));
    };
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
    install_command_shortcut_monitor();
    Ok(())
}

pub fn install_editor_host(
    host: &WindowSnapshot,
    owner: Option<&WindowSnapshot>,
    options: &HostOptions,
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

    if let Some(owner) = owner {
        attach_owner_window(owner, &host_window)?;
    }

    let requested_bounds = host_view.bounds();
    let layout = host_layout(
        requested_bounds.size.width,
        requested_bounds.size.height,
        options,
    );

    resize_host_window(&host_window, layout.outer_width, layout.outer_height);
    host_view.setFrameSize(NSSize::new(layout.outer_width, layout.outer_height));
    host_window.setOpaque(false);
    host_window.setBackgroundColor(Some(&NSColor::clearColor()));
    style_host_view(&host_view);

    let host_bounds = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(layout.outer_width, layout.outer_height),
    );
    let chrome = build_chrome_root(&host_bounds, options, mtm);
    let (titlebar_frame, content_frame) = child_frames(&host_bounds, options);
    let titlebar = build_titlebar(&host_window, &options.title, titlebar_frame, mtm);
    let content = build_content_view(content_frame, mtm);

    chrome.addSubview(&content);
    chrome.addSubview(&titlebar);
    host_view.addSubview(&chrome);

    Ok(InstalledHost {
        content: WindowSnapshot::capture(
            RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(
                NonNull::new(Retained::as_ptr(&content) as *mut std::ffi::c_void)
                    .expect("retained appkit content view pointer is non-null"),
            )),
            host.display.map(|_| {
                raw_window_handle::RawDisplayHandle::AppKit(
                    raw_window_handle::AppKitDisplayHandle::new(),
                )
            }),
        )?,
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

fn configure_window(window: &NSWindow, options: &HostOptions, mtm: MainThreadMarker) {
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
    window.setMovableByWindowBackground(true);
    window.setAnimationBehavior(NSWindowAnimationBehavior::UtilityWindow);
    window.setCollectionBehavior(collection_behavior);
    window.setTabbingMode(NSWindowTabbingMode::Disallowed);
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
}

fn resize_host_window(window: &NSWindow, width: f64, height: f64) {
    let mut frame = window.frame();
    let delta_height = height - frame.size.height;
    frame.origin.y -= delta_height;
    frame.size = NSSize::new(width, height);
    window.setFrame_display(frame, false);
}

fn attach_owner_window(owner: &WindowSnapshot, child_window: &NSWindow) -> Result<(), Error> {
    let owner_view = ns_view_from_snapshot(owner)?;
    let owner_window = owner_view
        .window()
        .ok_or_else(|| Error::Message("owner window is missing".to_string()))?;

    // SAFETY: Both windows are live AppKit windows on the main thread. AppKit requires
    // `addChildWindow:ordered:` to run on the main thread and the child is owned by the current
    // process.
    unsafe {
        owner_window.addChildWindow_ordered(child_window, NSWindowOrderingMode::Above);
    }
    Ok(())
}

fn build_titlebar(
    window: &NSWindow,
    title: &str,
    frame: crate::Rect,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let titlebar = NSView::initWithFrame(NSView::alloc(mtm), ns_rect(frame));
    titlebar.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    style_view(
        &titlebar,
        &NSColor::underPageBackgroundColor(),
        None,
        0.0,
        0.0,
    );

    let close_button = build_close_button(window, frame.height, mtm);
    let title_label = build_title_label(title, frame.width, frame.height, mtm);

    titlebar.addSubview(&close_button);
    titlebar.addSubview(&title_label);
    titlebar
}

fn build_close_button(window: &NSWindow, height: f64, mtm: MainThreadMarker) -> Retained<NSButton> {
    let button = NSButton::initWithFrame(NSButton::alloc(mtm), close_button_rect(height));
    button.setTitle(&NSString::from_str("×"));
    button.setButtonType(NSButtonType::MomentaryPushIn);
    button.setBezelStyle(NSBezelStyle::Circular);
    button.setBordered(true);
    button.setShowsBorderOnlyWhileMouseInside(true);
    button.setTransparent(false);
    // SAFETY: The target is the live host window. Borderless auxiliary windows do not reliably
    // route `performClose:` through winit, so the custom button performs the host hide directly.
    unsafe {
        button.setTarget(Some(window));
        button.setAction(Some(sel!(orderOut:)));
    }
    button
}

fn build_title_label(
    title: &str,
    width: f64,
    height: f64,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    let label_height = 18.0;
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setFrame(NSRect::new(
        NSPoint::new(
            TITLEBAR_LABEL_LEFT,
            ((height - label_height) / 2.0).max(0.0),
        ),
        NSSize::new(
            (width - TITLEBAR_LABEL_LEFT - TITLEBAR_LABEL_RIGHT).max(0.0),
            label_height,
        ),
    ));
    label.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    label.setDrawsBackground(false);
    label.setBordered(false);
    label.setBezeled(false);
    label.setEditable(false);
    label.setSelectable(false);
    label.setTextColor(Some(&NSColor::labelColor()));
    label
}

fn build_content_view(frame: crate::Rect, mtm: MainThreadMarker) -> Retained<NSView> {
    let content = NSView::initWithFrame(NSView::alloc(mtm), ns_rect(frame));
    content.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    style_view(&content, &NSColor::controlBackgroundColor(), None, 0.0, 0.0);
    content
}

fn style_host_view(view: &NSView) {
    style_view(view, &NSColor::clearColor(), None, 0.0, 0.0);
}

fn build_chrome_root(
    bounds: &NSRect,
    options: &HostOptions,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let chrome = NSView::initWithFrame(NSView::alloc(mtm), *bounds);
    chrome.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    style_view(
        &chrome,
        &NSColor::underPageBackgroundColor(),
        Some(&NSColor::underPageBackgroundColor()),
        options.decoration.corner_radius,
        options.decoration.frame_thickness,
    );
    chrome
}

fn child_frames(bounds: &NSRect, options: &HostOptions) -> (crate::Rect, crate::Rect) {
    let frame = options.decoration.frame_thickness.max(0.0);
    let titlebar_height = options.decoration.titlebar_height.max(20.0);
    let content_width = (bounds.size.width - frame * 2.0).max(0.0);
    let content_height = (bounds.size.height - frame * 2.0 - titlebar_height).max(0.0);

    (
        crate::Rect {
            x: frame,
            y: frame + content_height,
            width: content_width,
            height: titlebar_height,
        },
        crate::Rect {
            x: frame,
            y: frame,
            width: content_width,
            height: content_height,
        },
    )
}

fn close_button_rect(height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(
            TITLEBAR_BUTTON_LEFT,
            (((height - TITLEBAR_BUTTON_SIZE) / 2.0) + TITLEBAR_BUTTON_Y_OFFSET).max(0.0),
        ),
        NSSize::new(TITLEBAR_BUTTON_SIZE, TITLEBAR_BUTTON_SIZE),
    )
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

fn install_command_shortcut_monitor() {
    INSTALL_COMMAND_MONITOR.call_once(|| {
        let handler = RcBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit passes a non-null NSEvent pointer to local event monitor blocks and
            // the event remains valid for the duration of this callback.
            let event_ref = unsafe { event.as_ref() };
            if is_embedded_quit_shortcut(event_ref) {
                // SAFETY: Local AppKit event monitors run on the main thread.
                let mtm = unsafe { MainThreadMarker::new_unchecked() };
                let app = NSApplication::sharedApplication(mtm);
                request_app_close(&app);
                return std::ptr::null_mut();
            }

            if !routes_command_shortcuts_to_host(event_ref) {
                return event.as_ptr();
            }

            // SAFETY: Local AppKit event monitors run on the main thread.
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            let app = NSApplication::sharedApplication(mtm);
            let handled = app
                .mainMenu()
                .as_deref()
                .is_some_and(|menu| menu.performKeyEquivalent(event_ref));

            if handled {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            }
        });

        // SAFETY: The block is retained by AppKit for the lifetime of the monitor.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &handler)
        };

        mem::forget(handler);
        if let Some(monitor) = monitor {
            mem::forget(monitor);
        }
    });
}

fn routes_command_shortcuts_to_host(event: &NSEvent) -> bool {
    let modifiers = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    modifiers.contains(NSEventModifierFlags::Command)
}

fn is_embedded_quit_shortcut(event: &NSEvent) -> bool {
    if event.r#type() != NSEventType::KeyDown {
        return false;
    }

    let modifiers = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
    if !modifiers.contains(NSEventModifierFlags::Command) {
        return false;
    }

    if !is_command_q_key(event.keyCode(), event.charactersIgnoringModifiers()) {
        return false;
    };

    // SAFETY: This helper is only called from the AppKit local event monitor on the main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    app.keyWindow()
        .as_deref()
        .is_some_and(is_auxiliary_editor_window)
}

fn request_app_close(app: &NSApplication) {
    if let Some(window) = app
        .mainWindow()
        .filter(|window| !is_auxiliary_editor_window(window))
    {
        window.performClose(None);
        return;
    }

    let windows = app.windows();
    for index in 0..windows.count() {
        let window = windows.objectAtIndex(index);
        if !is_auxiliary_editor_window(&window) {
            window.performClose(None);
            return;
        }
    }
}

fn is_command_q_key(key_code: u16, chars: Option<Retained<NSString>>) -> bool {
    key_code == MACOS_Q_KEY_CODE
        || chars.is_some_and(|chars| chars.to_string().eq_ignore_ascii_case("q"))
}

fn is_auxiliary_editor_window(window: &NSWindow) -> bool {
    window
        .collectionBehavior()
        .contains(NSWindowCollectionBehavior::Auxiliary)
}

fn ns_rect(frame: crate::Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(frame.x, frame.y),
        NSSize::new(frame.width, frame.height),
    )
}

#[cfg(test)]
mod tests {
    use super::is_command_q_key;

    #[test]
    fn command_q_key_matches_q_only() {
        assert!(is_command_q_key(12, None));
        assert!(is_command_q_key(
            0,
            Some(objc2_foundation::NSString::from_str("q"))
        ));
        assert!(is_command_q_key(
            0,
            Some(objc2_foundation::NSString::from_str("Q"))
        ));
        assert!(!is_command_q_key(13, None));
        assert!(!is_command_q_key(
            0,
            Some(objc2_foundation::NSString::from_str("w"))
        ));
    }
}
