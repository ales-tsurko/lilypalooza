use std::{
    ptr::NonNull,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use block2::RcBlock;
use lilypalooza_egui_baseview::EguiWindowResizeHandle;
use objc2::{
    MainThreadMarker, MainThreadOnly,
    rc::Retained,
    runtime::{AnyObject, NSObjectProtocol, ProtocolObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSColor, NSEvent, NSView,
    NSViewBoundsDidChangeNotification, NSViewFrameDidChangeNotification, NSWindow,
    NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowOrderingMode, NSWindowStyleMask,
    NSWindowTabbingMode,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSOperationQueue, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{CALayer, CATransaction};
use raw_window_handle::RawWindowHandle;

use crate::{
    EditorFrame, EditorHostOptions, EditorPresetState, EguiFrameHost, Error, InstalledHost,
    ResizeAnchor, SharedSize, Size, WindowHandleSnapshot, WindowSnapshot, open_egui_frame,
    trace_editor_host,
};

pub fn prepare_process() -> Result<(), Error> {
    let mtm = main_thread_marker_for_host_configuration()?;
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
    Ok(())
}

pub fn install_editor_host(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
    frame: impl EditorFrame + Send + 'static,
) -> Result<InstalledHost, Error> {
    let context = prepare_editor_host_install(host, options)?;
    finish_editor_host_install(host, options, frame, context)
}

struct EditorHostInstallContext {
    mtm: MainThreadMarker,
    host_view: Retained<NSView>,
    host_window: Retained<NSWindow>,
    requested_bounds: NSRect,
}

fn prepare_editor_host_install(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
) -> Result<EditorHostInstallContext, Error> {
    let mtm = main_thread_marker_for_host_configuration()?;
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_window_for_view(&host_view)?;

    configure_window(&host_window, options, mtm);
    attach_host_window_to_owner(&host_window, options)?;

    let requested_bounds = host_view.bounds();
    Ok(EditorHostInstallContext {
        mtm,
        host_view,
        host_window,
        requested_bounds,
    })
}

fn finish_editor_host_install(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
    frame: impl EditorFrame + Send + 'static,
    context: EditorHostInstallContext,
) -> Result<InstalledHost, Error> {
    let layout = frame.layout(Size {
        width: context.requested_bounds.size.width,
        height: context.requested_bounds.size.height,
    });
    trace_editor_host(|| {
        format!(
            "install requested_bounds={} layout_outer={}x{} content={:?}",
            format_rect(context.requested_bounds),
            layout.outer_width,
            layout.outer_height,
            layout.content
        )
    });
    resize_host_window_from_top_clamped_to_screen(
        &context.host_window,
        layout.outer_width,
        layout.outer_height,
    );
    prepare_host_view_for_embedded_frame(&context.host_view, &context.host_window, layout);

    let frame_host = open_egui_frame(
        *host,
        options,
        Size {
            width: context.requested_bounds.size.width,
            height: context.requested_bounds.size.height,
        },
        layout.outer_width,
        layout.outer_height,
        frame,
    )?;

    let content_frame = content_frame_for_host(layout, context.host_view.isFlipped());
    let content_view = build_content_view(content_frame, context.mtm);
    context.host_view.addSubview(&content_view);
    let content = capture_content_view_snapshot(&content_view, host)?;

    let content_size = Size {
        width: context.requested_bounds.size.width,
        height: context.requested_bounds.size.height,
    };
    let content_size = Arc::new(crate::SharedSize::new(content_size));
    build_installed_host(*host, content, frame_host, content_size, layout.content.x)
}

fn main_thread_marker_for_host_configuration() -> Result<MainThreadMarker, Error> {
    MainThreadMarker::new().ok_or_else(|| {
        Error::Message("editor host configuration must run on the main thread".to_string())
    })
}

fn host_window_for_view(host_view: &NSView) -> Result<Retained<NSWindow>, Error> {
    host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))
}

fn attach_host_window_to_owner(
    host_window: &NSWindow,
    options: &EditorHostOptions,
) -> Result<(), Error> {
    let Some(owner) = options.owner else {
        return Ok(());
    };
    attach_child_window_to_owner(host_window, &owner)
}

fn prepare_host_view_for_embedded_frame(
    host_view: &NSView,
    host_window: &NSWindow,
    layout: crate::EditorFrameLayout,
) {
    host_view.setFrameSize(NSSize::new(layout.outer_width, layout.outer_height));
    host_window.setOpaque(false);
    host_window.setBackgroundColor(Some(&NSColor::clearColor()));
    style_view(host_view, &NSColor::clearColor(), None, 0.0, 0.0, false);
}

fn capture_content_view_snapshot(
    content: &Retained<NSView>,
    host: &WindowSnapshot,
) -> Result<WindowSnapshot, Error> {
    let Some(content_ptr) = NonNull::new(Retained::as_ptr(content) as *mut std::ffi::c_void) else {
        return Err(Error::Message(
            "retained AppKit content view pointer is null".to_string(),
        ));
    };
    WindowSnapshot::capture(
        RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(content_ptr)),
        appkit_display_for_host(host),
    )
}

fn appkit_display_for_host(host: &WindowSnapshot) -> Option<raw_window_handle::RawDisplayHandle> {
    host.display.map(|_| {
        raw_window_handle::RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new())
    })
}

fn build_installed_host(
    host: WindowSnapshot,
    content: WindowSnapshot,
    frame_host: EguiFrameHost,
    content_size: Arc<SharedSize>,
    frame_thickness: f64,
) -> Result<InstalledHost, Error> {
    let native_content_resize_observer = Some(install_native_content_resize_observer(
        host,
        content,
        Arc::clone(&frame_host.preset_state),
        Some(frame_host.window.resize_handle()),
        Arc::clone(&frame_host.content_size),
        Arc::clone(&content_size),
        frame_thickness,
    )?);
    Ok(InstalledHost {
        host,
        content,
        close_requested: frame_host.close_requested,
        title: frame_host.title,
        preset_state: frame_host.preset_state,
        frame_content_size: frame_host.content_size,
        frame_resizable: frame_host.resizable,
        frame_zoom_percent: frame_host.zoom_percent,
        frame_commands: frame_host.frame_commands,
        frame_window: Some(frame_host.window),
        content_size,
        frame_thickness,
        native_content_resize_observer,
    })
}

pub fn resize_installed_host(
    host: &WindowSnapshot,
    content: &WindowSnapshot,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
    anchor: ResizeAnchor,
) -> Result<(), Error> {
    let layout = crate::host_layout(
        content_size.width,
        content_size.height,
        titlebar_height,
        frame_thickness,
    );
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    let before_window_frame = host_window.frame();
    let before_host_frame = host_view.frame();
    let before_host_bounds = host_view.bounds();
    let content_view = ns_view_from_snapshot(content)?;
    let content_frame = ns_rect(content_frame_for_host(layout, host_view.isFlipped()));
    without_implicit_layer_animations(|| {
        resize_host_window_clamped_to_screen(
            &host_window,
            layout.outer_width,
            layout.outer_height,
            anchor,
        );
        host_view.setFrameSize(NSSize::new(layout.outer_width, layout.outer_height));
        content_view.setFrame(content_frame);
    });
    trace_editor_host(|| {
        format!(
            "macos resize_installed_host anchor={anchor:?} content={content_size:?} window {} -> \
             {} host_frame {} -> {} host_bounds {} -> {} content_frame={}",
            format_rect(before_window_frame),
            format_rect(host_window.frame()),
            format_rect(before_host_frame),
            format_rect(host_view.frame()),
            format_rect(before_host_bounds),
            format_rect(host_view.bounds()),
            format_rect(content_frame)
        )
    });
    Ok(())
}

pub fn sync_installed_host_layout(
    host: &WindowSnapshot,
    content: &WindowSnapshot,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
) -> Result<(), Error> {
    let layout = crate::host_layout(
        content_size.width,
        content_size.height,
        titlebar_height,
        frame_thickness,
    );
    let host_view = ns_view_from_snapshot(host)?;
    let content_view = ns_view_from_snapshot(content)?;
    let content_frame = ns_rect(content_frame_for_host(layout, host_view.isFlipped()));
    without_implicit_layer_animations(|| {
        content_view.setFrame(content_frame);
    });
    trace_editor_host(|| {
        format!(
            "macos sync_installed_host_layout content={content_size:?} host_bounds={} \
             content_frame={}",
            format_rect(host_view.bounds()),
            format_rect(content_frame)
        )
    });
    Ok(())
}

pub fn native_content_size(
    host: &WindowSnapshot,
    titlebar_height: f64,
    frame_thickness: f64,
) -> Result<Size, Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let bounds = host_view.bounds();
    Ok(crate::content_size_from_outer_size(
        Size {
            width: bounds.size.width,
            height: bounds.size.height,
        },
        titlebar_height,
        frame_thickness,
    ))
}

pub fn embedded_content_size(content: &WindowSnapshot) -> Result<Option<Size>, Error> {
    let content_view = ns_view_from_snapshot(content)?;
    enable_embedded_content_resize_notifications_for_view(&content_view);
    let size = largest_embedded_subview_size(&content_view);
    if size.width <= 0.0 || size.height <= 0.0 {
        return Ok(None);
    }
    Ok(Some(size))
}

pub struct NativeContentResizeObserver {
    frame_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    bounds_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    enabled: Arc<AtomicBool>,
}

impl NativeContentResizeObserver {
    pub fn enable(&self, content: &WindowSnapshot) -> Result<(), Error> {
        self.enabled.store(true, Ordering::Release);
        enable_embedded_content_resize_notifications(content)
    }
}

impl Drop for NativeContentResizeObserver {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        remove_notification_observer(&center, &self.frame_observer);
        remove_notification_observer(&center, &self.bounds_observer);
    }
}

fn remove_notification_observer(
    center: &NSNotificationCenter,
    observer: &Retained<ProtocolObject<dyn NSObjectProtocol>>,
) {
    // SAFETY: The token was returned by NSNotificationCenter and is an Objective-C object accepted
    // by removeObserver:.
    let object = unsafe { &*Retained::as_ptr(observer).cast::<AnyObject>() };
    // SAFETY: The observer token belongs to this notification center.
    unsafe {
        center.removeObserver(object);
    }
}

#[derive(Clone)]
struct NativeContentResizeContext {
    host: WindowSnapshot,
    content: WindowSnapshot,
    preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    frame_window: Option<EguiWindowResizeHandle>,
    frame_content_size: Arc<SharedSize>,
    content_size: Arc<SharedSize>,
    frame_thickness: f64,
    pending_apply: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
}

fn install_native_content_resize_observer(
    host: WindowSnapshot,
    content: WindowSnapshot,
    preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    frame_window: Option<EguiWindowResizeHandle>,
    frame_content_size: Arc<SharedSize>,
    content_size: Arc<SharedSize>,
    frame_thickness: f64,
) -> Result<NativeContentResizeObserver, Error> {
    let center = NSNotificationCenter::defaultCenter();
    let enabled = Arc::new(AtomicBool::new(false));
    let context = NativeContentResizeContext {
        host,
        content,
        preset_state,
        frame_window,
        frame_content_size,
        content_size,
        frame_thickness,
        pending_apply: Arc::new(AtomicBool::new(false)),
        enabled: Arc::clone(&enabled),
    };
    // SAFETY: AppKit exports these notification names as process-wide constant NSStrings.
    let frame_notification = unsafe { NSViewFrameDidChangeNotification };
    // SAFETY: AppKit exports these notification names as process-wide constant NSStrings.
    let bounds_notification = unsafe { NSViewBoundsDidChangeNotification };
    Ok(NativeContentResizeObserver {
        frame_observer: create_view_resize_observer(&center, frame_notification, context.clone()),
        bounds_observer: create_view_resize_observer(&center, bounds_notification, context),
        enabled,
    })
}

fn create_view_resize_observer(
    center: &NSNotificationCenter,
    name: &NSString,
    context: NativeContentResizeContext,
) -> Retained<ProtocolObject<dyn NSObjectProtocol>> {
    let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        // SAFETY: AppKit passes a live notification object to the observer block.
        let notification = unsafe { notification.as_ref() };
        apply_embedded_view_resize_notification(notification, &context);
    });
    // SAFETY: The block is retained by the notification center. `object` and `queue` are nil so
    // AppKit delivers matching view notifications synchronously on the posting thread.
    unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block) }
}

fn apply_embedded_view_resize_notification(
    notification: &NSNotification,
    context: &NativeContentResizeContext,
) {
    if !native_content_resize_tracking_enabled(&context.enabled) {
        return;
    }
    let Some(changed_view) = notification_view(notification) else {
        return;
    };
    let Ok(content_view) = ns_view_from_snapshot(&context.content) else {
        return;
    };
    if !view_is_descendant_of(&changed_view, &content_view) {
        return;
    }
    enable_embedded_content_resize_notifications_for_view(&content_view);
    schedule_embedded_content_resize_apply(context.clone());
}

fn schedule_embedded_content_resize_apply(context: NativeContentResizeContext) {
    if context.pending_apply.swap(true, Ordering::AcqRel) {
        return;
    }

    let block = RcBlock::new(move || {
        apply_embedded_content_resize(&context);
        context.pending_apply.store(false, Ordering::Release);
    });
    let queue = NSOperationQueue::mainQueue();
    // SAFETY: AppKit view notifications are handled on the main thread. The queued block owns a
    // cloned context and only touches AppKit objects after returning to the main queue.
    unsafe {
        queue.addOperationWithBlock(&block);
    }
}

fn apply_embedded_content_resize(context: &NativeContentResizeContext) {
    if !native_content_resize_tracking_enabled(&context.enabled) {
        return;
    }
    let Ok(Some(measured)) = embedded_content_size(&context.content) else {
        return;
    };
    let current = context.content_size.load();
    if crate::same_size(current, measured) {
        return;
    }

    context.content_size.store(measured);
    context.frame_content_size.store(measured);
    let titlebar_height = crate::titlebar_height_from_preset_state(&context.preset_state);
    trace_editor_host(|| {
        format!("macos embedded content resize apply current={current:?} measured={measured:?}")
    });
    if let Err(error) = crate::resize_installed_host(
        &context.host,
        &context.content,
        measured,
        titlebar_height,
        context.frame_thickness,
        context.frame_window.as_ref(),
        ResizeAnchor::Top,
    ) {
        trace_editor_host(|| format!("macos embedded content notification resize failed: {error}"));
    }
}

fn native_content_resize_tracking_enabled(enabled: &AtomicBool) -> bool {
    enabled.load(Ordering::Acquire)
}

fn notification_view(notification: &NSNotification) -> Option<Retained<NSView>> {
    let object = notification.object()?;
    let view = Retained::as_ptr(&object).cast::<NSView>().cast_mut();
    if view.is_null() {
        return None;
    }
    // SAFETY: NSView frame/bounds notifications are posted by NSView objects.
    unsafe { Retained::retain(view) }
}

fn view_is_descendant_of(view: &NSView, ancestor: &NSView) -> bool {
    let ancestor_ptr = ancestor as *const NSView;
    // SAFETY: `view` is a live AppKit view retained for the duration of the walk.
    let mut current = unsafe { Retained::retain(view as *const NSView as *mut NSView) };
    while let Some(view) = current {
        if Retained::as_ptr(&view) == ancestor_ptr {
            return true;
        }
        // SAFETY: Walking AppKit superviews is valid on the main thread for live NSView objects.
        current = unsafe { view.superview() };
    }
    false
}

pub fn enable_embedded_content_resize_notifications(content: &WindowSnapshot) -> Result<(), Error> {
    let content_view = ns_view_from_snapshot(content)?;
    enable_embedded_content_resize_notifications_for_view(&content_view);
    Ok(())
}

fn largest_embedded_subview_size(view: &NSView) -> Size {
    let mut size = Size {
        width: 0.0,
        height: 0.0,
    };
    for subview in view.subviews().to_vec() {
        let frame = subview.frame();
        let bounds = subview.bounds();
        let nested = largest_embedded_subview_size(&subview);
        let extent = embedded_subview_extent(
            frame,
            Size {
                width: bounds.size.width,
                height: bounds.size.height,
            },
            nested,
        );
        size.width = size.width.max(extent.width);
        size.height = size.height.max(extent.height);
    }
    size
}

fn enable_embedded_content_resize_notifications_for_view(view: &NSView) {
    for subview in view.subviews().to_vec() {
        subview.setPostsFrameChangedNotifications(true);
        subview.setPostsBoundsChangedNotifications(true);
        enable_embedded_content_resize_notifications_for_view(&subview);
    }
}

fn embedded_subview_extent(frame: NSRect, bounds: Size, nested: Size) -> Size {
    let origin_x = frame.origin.x.max(0.0);
    let origin_y = frame.origin.y.max(0.0);
    Size {
        width: origin_x + bounds.width.max(nested.width),
        height: origin_y + bounds.height.max(nested.height),
    }
}

pub fn is_host_window_live_resizing(host: &WindowSnapshot) -> Result<bool, Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    Ok(host_window.inLiveResize())
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

pub fn raise_host_window(host: &WindowSnapshot) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;

    if let Some(parent_window) = host_window.parentWindow() {
        parent_window.removeChildWindow(&host_window);
        // SAFETY: Both windows are live AppKit windows on the main thread. Re-adding the editor
        // child above the app window updates sibling editor stacking order.
        unsafe {
            parent_window.addChildWindow_ordered(&host_window, NSWindowOrderingMode::Above);
        }
    }
    host_window.makeKeyAndOrderFront(None);
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
    }
    // SAFETY: The menu item and main window are live AppKit objects on the main thread.
    unsafe {
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

    let view = (ns_view as *mut AnyObject).cast::<NSView>();
    if view.is_null() {
        return Err(Error::Message("host AppKit view is null".to_string()));
    }
    // SAFETY: The pointer came from a live AppKit window handle and is still owned by that window.
    unsafe { Retained::retain(view) }
        .ok_or_else(|| Error::Message("host AppKit view is unavailable".to_string()))
}

fn configure_window(window: &NSWindow, options: &EditorHostOptions, mtm: MainThreadMarker) {
    let style_mask = editor_window_style_mask(window.styleMask());

    let collection_behavior = window.collectionBehavior()
        | NSWindowCollectionBehavior::Auxiliary
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::FullScreenDisallowsTiling;

    window.setStyleMask(style_mask);
    window.setTitle(&NSString::from_str(&options.title));
    window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
    window.setHasShadow(true);
    window.setMovableByWindowBackground(false);
    window.setAnimationBehavior(NSWindowAnimationBehavior::UtilityWindow);
    window.setCollectionBehavior(collection_behavior);
    window.setTabbingMode(NSWindowTabbingMode::Disallowed);
    window.invalidateShadow();
    NSWindow::setAllowsAutomaticWindowTabbing(false, mtm);
}

fn editor_window_style_mask(mut style_mask: NSWindowStyleMask) -> NSWindowStyleMask {
    style_mask.insert(NSWindowStyleMask::Closable);
    style_mask.remove(NSWindowStyleMask::Miniaturizable);
    style_mask.remove(NSWindowStyleMask::Resizable);
    style_mask
}

pub fn sync_host_window_resize_policy(host: &WindowSnapshot) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    host_window.setStyleMask(editor_window_style_mask(host_window.styleMask()));
    trace_editor_host(|| "macos sync resize policy".to_string());
    Ok(())
}

pub fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    let host_view = ns_view_from_snapshot(host)?;
    let host_window = host_view
        .window()
        .ok_or_else(|| Error::Message("host window is missing".to_string()))?;
    let event = host_window
        .currentEvent()
        .ok_or_else(|| Error::Message("host window drag event is missing".to_string()))?;

    trace_editor_host(|| {
        format!(
            "macos performWindowDragWithEvent start event={} window={}",
            format_event(&event),
            format_rect(host_window.frame())
        )
    });
    host_window.performWindowDragWithEvent(&event);
    trace_editor_host(|| {
        format!(
            "macos performWindowDragWithEvent end window={}",
            format_rect(host_window.frame())
        )
    });
    host_window.invalidateShadow();
    Ok(())
}

fn resize_host_window_from_top_clamped_to_screen(window: &NSWindow, width: f64, height: f64) {
    let frame = host_window_frame_resized_from_top(window.frame(), width, height);
    let frame = window
        .screen()
        .map(|screen| host_window_frame_clamped_to_visible_top(frame, screen.visibleFrame()))
        .unwrap_or(frame);
    window.setFrame_display(frame, false);
}

fn resize_host_window_from_bottom_clamped_to_screen(window: &NSWindow, width: f64, height: f64) {
    let frame = host_window_frame_resized_from_bottom(window.frame(), width, height);
    let frame = window
        .screen()
        .map(|screen| host_window_frame_clamped_to_visible_top(frame, screen.visibleFrame()))
        .unwrap_or(frame);
    window.setFrame_display(frame, false);
}

fn resize_host_window_clamped_to_screen(
    window: &NSWindow,
    width: f64,
    height: f64,
    anchor: ResizeAnchor,
) {
    match anchor {
        ResizeAnchor::Top => resize_host_window_from_top_clamped_to_screen(window, width, height),
        ResizeAnchor::Bottom => {
            resize_host_window_from_bottom_clamped_to_screen(window, width, height)
        }
    }
}

fn host_window_frame_resized_from_top(mut frame: NSRect, width: f64, height: f64) -> NSRect {
    let delta_height = height - frame.size.height;
    frame.origin.y -= delta_height;
    frame.size = NSSize::new(width, height);
    frame
}

fn host_window_frame_resized_from_bottom(mut frame: NSRect, width: f64, height: f64) -> NSRect {
    frame.size = NSSize::new(width, height);
    frame
}

#[cfg(test)]
fn host_window_frame_resized_from_bottom_clamped(
    frame: NSRect,
    width: f64,
    height: f64,
    visible: NSRect,
) -> NSRect {
    host_window_frame_clamped_to_visible_top(
        host_window_frame_resized_from_bottom(frame, width, height),
        visible,
    )
}

fn host_window_frame_clamped_to_visible_top(mut frame: NSRect, visible: NSRect) -> NSRect {
    let top = frame.origin.y + frame.size.height;
    let visible_top = visible.origin.y + visible.size.height;
    if top > visible_top {
        frame.origin.y = visible_top - frame.size.height;
    }
    frame
}

fn build_content_view(frame: crate::Rect, mtm: MainThreadMarker) -> Retained<NSView> {
    let content = NSView::initWithFrame(NSView::alloc(mtm), ns_rect(frame));
    content.setAutoresizingMask(content_view_autoresizing_mask());
    style_view(
        &content,
        &NSColor::clearColor(),
        None,
        0.0,
        0.0,
        content_view_masks_to_bounds(),
    );
    content
}

fn content_view_autoresizing_mask() -> NSAutoresizingMaskOptions {
    NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable
}

fn content_view_masks_to_bounds() -> bool {
    true
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

fn style_view(
    view: &NSView,
    background: &NSColor,
    border: Option<&NSColor>,
    corner_radius: f64,
    border_width: f64,
    masks_to_bounds: bool,
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
    layer.setMasksToBounds(masks_to_bounds || corner_radius > 0.0);
    view.setLayer(Some(&layer));
    view.setClipsToBounds(masks_to_bounds);
}

fn without_implicit_layer_animations(operation: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    operation();
    CATransaction::commit();
}

fn ns_rect(frame: crate::Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(frame.x, frame.y),
        NSSize::new(frame.width, frame.height),
    )
}

fn format_rect(rect: NSRect) -> String {
    format!(
        "x={:.1} y={:.1} w={:.1} h={:.1}",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}

fn format_event(event: &NSEvent) -> String {
    format!("type={:?}", event.r#type())
}

#[cfg(test)]
mod window_tests;
