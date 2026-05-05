//! Minimal egui host on top of parented baseview windows.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub use baseview::Size;
use baseview::gl::GlConfig;
use baseview::{
    Event, EventStatus, MouseButton, MouseEvent, ScrollDelta, Window, WindowHandle, WindowHandler,
    WindowInfo, WindowOpenOptions, WindowScalePolicy,
};
use egui::{Pos2, RawInput, Rect, Vec2};
use egui_glow::Painter;
use glow::HasContext;
use keyboard_types::{Key, KeyState, KeyboardEvent, Modifiers};
use raw_window_handle::{
    AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle, WaylandWindowHandle,
    Win32WindowHandle, XcbWindowHandle, XlibWindowHandle,
};
use raw_window_handle_06 as rwh06;

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use objc2::{class, msg_send};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSViewLayerContentsRedrawPolicy};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// Re-exported egui API for editor implementations.
pub use egui;

/// Editor window open options.
#[derive(Debug, Clone)]
pub struct EguiWindowOptions {
    /// Window title.
    pub title: String,
    /// Logical content width.
    pub width: f64,
    /// Logical content height.
    pub height: f64,
}

/// Live parented egui editor handle.
pub struct EguiWindowHandle {
    window: WindowHandle,
    pending_resize: Arc<AtomicResizeRequest>,
    child_view: usize,
}

/// Cloneable resize endpoint for a live parented egui view.
#[derive(Clone)]
pub struct EguiWindowResizeHandle {
    pending_resize: Arc<AtomicResizeRequest>,
    child_view: usize,
}

impl EguiWindowHandle {
    /// Closes and destroys the parented editor view.
    pub fn close(&mut self) {
        self.window.close();
    }

    /// Resizes the parented editor view.
    pub fn resize(&mut self, width: f64, height: f64) {
        resize_parented_view(
            &self.pending_resize,
            self.child_view,
            Size::new(width, height),
        );
    }

    /// Returns a cloneable resize endpoint for callbacks that cannot borrow this handle.
    #[must_use]
    pub fn resize_handle(&self) -> EguiWindowResizeHandle {
        EguiWindowResizeHandle {
            pending_resize: Arc::clone(&self.pending_resize),
            child_view: self.child_view,
        }
    }
}

impl EguiWindowResizeHandle {
    /// Resizes the parented editor view.
    pub fn resize(&self, width: f64, height: f64) {
        resize_parented_view(
            &self.pending_resize,
            self.child_view,
            Size::new(width, height),
        );
    }
}

fn resize_parented_view(pending_resize: &AtomicResizeRequest, child_view: usize, size: Size) {
    pending_resize.store(size);
    if external_resize_updates_child_view_immediately() {
        resize_child_view_now(child_view, size);
    }
}

#[derive(Debug)]
struct AtomicResizeRequest {
    sequence: AtomicU64,
    consumed_sequence: AtomicU64,
    width: AtomicU64,
    height: AtomicU64,
}

impl AtomicResizeRequest {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            consumed_sequence: AtomicU64::new(0),
            width: AtomicU64::new(0),
            height: AtomicU64::new(0),
        }
    }

    fn store(&self, size: Size) {
        let writing = loop {
            let sequence = self.sequence.load(Ordering::Relaxed);
            if !sequence.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            if self
                .sequence
                .compare_exchange(sequence, sequence + 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break sequence + 1;
            }
            std::hint::spin_loop();
        };
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
        self.sequence.store(writing + 1, Ordering::Relaxed);
    }

    fn take(&self) -> Option<Size> {
        let (sequence, size) = self.load_unconsumed()?;
        self.consumed_sequence.store(sequence, Ordering::Relaxed);
        Some(size)
    }

    fn unconsumed(&self) -> Option<Size> {
        self.load_unconsumed().map(|(_, size)| size)
    }

    fn load_unconsumed(&self) -> Option<(u64, Size)> {
        for _ in 0..3 {
            let sequence = self.sequence.load(Ordering::Relaxed);
            if !sequence.is_multiple_of(2)
                || sequence == self.consumed_sequence.load(Ordering::Relaxed)
            {
                return None;
            }
            let size = Size::new(
                f64::from_bits(self.width.load(Ordering::Relaxed)),
                f64::from_bits(self.height.load(Ordering::Relaxed)),
            );
            if sequence == self.sequence.load(Ordering::Relaxed) {
                return Some((sequence, size));
            }
        }
        None
    }
}

/// Errors returned by the egui/baseview bridge.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Parent handle type is not supported by baseview.
    #[error("unsupported parent window handle: {0}")]
    UnsupportedParent(String),
}

/// Opens a parented egui editor view.
///
/// # Errors
///
/// Returns an error when the parent raw window handle cannot be converted to baseview's raw-window-handle version.
pub fn open_parented<App, Build>(
    parent: rwh06::RawWindowHandle,
    options: EguiWindowOptions,
    build: Build,
) -> Result<EguiWindowHandle, Error>
where
    App: EguiApp + 'static,
    Build: FnOnce() -> App + Send + 'static,
{
    let parent = ParentWindow::from_raw(parent)?;
    let pending_resize = Arc::new(AtomicResizeRequest::new());
    let child_view_ref = Arc::new(AtomicUsize::new(0));
    let open_options = WindowOpenOptions {
        title: options.title,
        size: Size::new(options.width, options.height),
        scale: WindowScalePolicy::SystemScaleFactor,
        gl_config: Some(GlConfig {
            alpha_bits: 8,
            ..GlConfig::default()
        }),
    };
    let window_pending_resize = Arc::clone(&pending_resize);
    let window_child_view_ref = Arc::clone(&child_view_ref);
    let initial_size = Size::new(options.width, options.height);
    let window = Window::open_parented(&parent, open_options, move |window| {
        EguiWindow::new(
            window,
            build(),
            window_pending_resize,
            window_child_view_ref,
            initial_size,
        )
    });
    #[cfg(target_os = "macos")]
    configure_parented_child_view(&window);
    let child_view = child_view_handle(&window);
    child_view_ref.store(child_view, Ordering::Release);
    Ok(EguiWindowHandle {
        window,
        pending_resize,
        child_view,
    })
}

#[cfg(target_os = "macos")]
fn child_view_handle(window: &WindowHandle) -> usize {
    let RawWindowHandle::AppKit(handle) = window.raw_window_handle() else {
        return 0;
    };
    handle.ns_view as usize
}

#[cfg(not(target_os = "macos"))]
fn child_view_handle(_window: &WindowHandle) -> usize {
    0
}

#[cfg(target_os = "macos")]
fn configure_parented_child_view(window: &WindowHandle) {
    let RawWindowHandle::AppKit(handle) = window.raw_window_handle() else {
        return;
    };
    let view = handle.ns_view.cast::<NSView>();
    if view.is_null() {
        return;
    }
    // SAFETY: The raw handle is owned by the live baseview child window.
    let Some(view) = (unsafe { Retained::retain(view) }) else {
        return;
    };
    view.setAutoresizingMask(explicit_resize_autoresizing_mask());
    configure_live_resize_redraw(&view);
    resize_ns_view_and_direct_subviews(&view, view.frame().size);
}

#[cfg(target_os = "macos")]
fn resize_ns_view_and_direct_subviews(view: &NSView, size: NSSize) {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
    view.setFrameSize(size);
    view.setNeedsDisplay(true);
    configure_live_resize_redraw(view);

    let subviews = view.subviews();
    for index in 0..subviews.count() {
        let subview = subviews.objectAtIndex(index);
        subview.setAutoresizingMask(explicit_resize_autoresizing_mask());
        subview.setFrame(frame);
        subview.setNeedsDisplay(true);
        configure_live_resize_redraw(&subview);
    }
}

#[cfg(target_os = "macos")]
fn configure_live_resize_redraw(view: &NSView) {
    view.setLayerContentsRedrawPolicy(NSViewLayerContentsRedrawPolicy::DuringViewResize);
}

#[cfg(target_os = "macos")]
fn explicit_resize_autoresizing_mask() -> NSAutoresizingMaskOptions {
    NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable
}

#[cfg(target_os = "macos")]
fn resize_child_view_now(child_view: usize, size: Size) {
    let view = child_view as *mut NSView;
    if view.is_null() {
        return;
    }
    // SAFETY: `child_view` is captured from the live baseview child window handle.
    let Some(view) = (unsafe { Retained::retain(view) }) else {
        return;
    };
    let size = NSSize::new(size.width.round(), size.height.round());
    resize_ns_view_and_direct_subviews(&view, size);
}

#[cfg(not(target_os = "macos"))]
fn resize_child_view_now(_child_view: usize, _size: Size) {}

/// egui editor application.
pub trait EguiApp {
    /// Draws one egui frame.
    fn update(&mut self, ui: &mut egui::Ui);

    /// Handles a pointer press before it is submitted to egui.
    ///
    /// Return `true` when the press was consumed by native window handling.
    fn mouse_button_pressed(&mut self, _pos: egui::Pos2, _button: egui::PointerButton) -> bool {
        false
    }
}

struct ParentWindow {
    raw: RawWindowHandle,
}

impl ParentWindow {
    fn from_raw(raw: rwh06::RawWindowHandle) -> Result<Self, Error> {
        let raw = match raw {
            rwh06::RawWindowHandle::AppKit(handle) => {
                let mut converted = AppKitWindowHandle::empty();
                converted.ns_view = handle.ns_view.as_ptr();
                RawWindowHandle::AppKit(converted)
            }
            rwh06::RawWindowHandle::Win32(handle) => {
                let mut converted = Win32WindowHandle::empty();
                converted.hwnd = handle.hwnd.get() as *mut _;
                converted.hinstance = handle
                    .hinstance
                    .map_or(std::ptr::null_mut(), |value| value.get() as *mut _);
                RawWindowHandle::Win32(converted)
            }
            rwh06::RawWindowHandle::Xcb(handle) => {
                let mut converted = XcbWindowHandle::empty();
                converted.window = handle.window.get();
                converted.visual_id = handle.visual_id.map_or(0, |id| id.get());
                RawWindowHandle::Xcb(converted)
            }
            rwh06::RawWindowHandle::Xlib(handle) => {
                let mut converted = XlibWindowHandle::empty();
                converted.window = handle.window;
                converted.visual_id = handle.visual_id;
                RawWindowHandle::Xlib(converted)
            }
            rwh06::RawWindowHandle::Wayland(handle) => {
                let mut converted = WaylandWindowHandle::empty();
                converted.surface = handle.surface.as_ptr();
                RawWindowHandle::Wayland(converted)
            }
            other => return Err(Error::UnsupportedParent(format!("{other:?}"))),
        };
        Ok(Self { raw })
    }
}

// SAFETY: `ParentWindow` stores a raw window handle captured from the host while the parent view is
// alive. The bridge only hands it to baseview during immediate parented-window creation.
unsafe impl HasRawWindowHandle for ParentWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.raw
    }
}

struct EguiWindow<App> {
    app: App,
    ctx: egui::Context,
    painter: Painter,
    gl: Arc<glow::Context>,
    input: RawInput,
    window_info: WindowInfo,
    pointer_pos: Option<Pos2>,
    pending_resize: Arc<AtomicResizeRequest>,
    child_view: Arc<AtomicUsize>,
    programmatic_resize_echoes: VecDeque<Size>,
}

impl<App: EguiApp> EguiWindow<App> {
    fn new(
        window: &mut Window<'_>,
        app: App,
        pending_resize: Arc<AtomicResizeRequest>,
        child_view: Arc<AtomicUsize>,
        initial_size: Size,
    ) -> Self {
        let Some(context) = window.gl_context() else {
            panic!("baseview did not create an OpenGL context");
        };
        clear_current_opengl_context();
        // SAFETY: baseview created the OpenGL context for this window and this callback runs on
        // the window thread while the context is valid.
        unsafe {
            context.make_current();
        }
        update_current_opengl_drawable();
        // SAFETY: the loader function is provided by the current baseview OpenGL context and stays
        // valid for the lifetime of that context.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| context.get_proc_address(name) as *const _)
        };
        let gl = Arc::new(gl);
        let painter = match Painter::new(Arc::clone(&gl), "", None, false) {
            Ok(painter) => painter,
            Err(error) => panic!("egui glow painter failed to initialize: {error}"),
        };
        // SAFETY: the current thread owns the baseview OpenGL context for this callback.
        unsafe {
            context.make_not_current();
        }
        clear_current_opengl_context();
        let mut window = Self {
            app,
            ctx: egui::Context::default(),
            painter,
            gl,
            input: RawInput::default(),
            window_info: initial_window_info(initial_size),
            pointer_pos: None,
            pending_resize,
            child_view,
            programmatic_resize_echoes: VecDeque::new(),
        };
        window.set_window_info(initial_window_info(initial_size));
        window
    }

    fn set_window_info(&mut self, window_info: WindowInfo) {
        self.window_info = window_info;
        let logical = window_info.logical_size();
        self.input.screen_rect = Some(Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(logical.width as f32, logical.height as f32),
        ));
        if let Some(viewport) = self.input.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(window_info.scale() as f32);
        }
    }

    #[cfg(target_os = "macos")]
    fn sync_window_info_from_native_view(&mut self, window: &mut Window<'_>) {
        let RawWindowHandle::AppKit(handle) = window.raw_window_handle() else {
            return;
        };
        let view = handle.ns_view.cast::<NSView>();
        if view.is_null() {
            return;
        }
        // SAFETY: The raw handle comes from the live baseview window passed to this frame.
        let Some(view) = (unsafe { Retained::retain(view) }) else {
            return;
        };
        let bounds: NSRect = view.bounds();
        let native_size = Size::new(bounds.size.width, bounds.size.height);
        let current = self.window_info.logical_size();
        if same_size(native_size, current) {
            consume_programmatic_resize_echo(&mut self.programmatic_resize_echoes, native_size);
            return;
        }
        if pending_programmatic_resize(&self.programmatic_resize_echoes, current) {
            return;
        }
        window.resize(native_size);
        self.set_window_info(WindowInfo::from_logical_size(
            native_size,
            self.window_info.scale(),
        ));
    }

    fn push_event(&mut self, event: egui::Event) {
        self.input.events.push(event);
    }

    fn render(&mut self, window: &mut Window<'_>) {
        #[cfg(target_os = "macos")]
        self.sync_window_info_from_native_view(window);
        let Some(context) = window.gl_context() else {
            panic!("baseview lost its OpenGL context");
        };
        clear_current_opengl_context();
        // SAFETY: baseview invokes rendering on the window thread with a live OpenGL context.
        unsafe {
            context.make_current();
        }
        update_current_opengl_drawable();

        let physical = self.window_info.physical_size();
        let Some(screen_size) = renderable_screen_size(physical.width, physical.height) else {
            // SAFETY: the current thread owns the baseview OpenGL context for this callback.
            unsafe {
                context.make_not_current();
            }
            clear_current_opengl_context();
            return;
        };
        let [r, g, b, a] = clear_color();
        if let Err(error) = prepare_default_framebuffer(&self.gl, [r, g, b, a]) {
            update_current_opengl_drawable();
            if let Err(retried_error) = prepare_default_framebuffer(&self.gl, [r, g, b, a]) {
                log::trace!(
                    target: "editor_host",
                    "skipping egui paint before egui run after failed framebuffer clear error=0x{error:x} retried_error=0x{retried_error:x}"
                );
                // SAFETY: the current thread owns the baseview OpenGL context for this callback.
                unsafe {
                    context.make_not_current();
                }
                clear_current_opengl_context();
                return;
            }
        }

        let raw_input = std::mem::take(&mut self.input);
        let output = self.ctx.run_ui(raw_input, |ui| self.app.update(ui));
        self.input = RawInput::default();
        self.set_window_info(self.window_info);

        let clipped_primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        if let Some(error) = first_pending_gl_error(&self.gl) {
            log::trace!(
                target: "editor_host",
                "skipping egui paint after unexpected pre-paint gl error=0x{error:x}"
            );
            // SAFETY: the current thread owns the baseview OpenGL context for this callback.
            unsafe {
                context.make_not_current();
            }
            clear_current_opengl_context();
            return;
        }
        self.painter.paint_and_update_textures(
            screen_size,
            output.pixels_per_point,
            &clipped_primitives,
            &output.textures_delta,
        );
        context.swap_buffers();
        // SAFETY: the current thread owns the baseview OpenGL context for this callback.
        unsafe {
            context.make_not_current();
        }
        clear_current_opengl_context();
    }
}

fn clear_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 0.0]
}

fn renderable_screen_size(width: u32, height: u32) -> Option<[u32; 2]> {
    (width > 0 && height > 0).then_some([width, height])
}

fn prepare_default_framebuffer(gl: &glow::Context, color: [f32; 4]) -> Result<(), u32> {
    // SAFETY: callers only use this while the baseview OpenGL context is current.
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    }
    clear_pending_gl_errors(gl);
    update_current_opengl_drawable();
    clear_default_framebuffer(gl, color)
}

fn clear_default_framebuffer(gl: &glow::Context, [r, g, b, a]: [f32; 4]) -> Result<(), u32> {
    // SAFETY: callers only use this while the baseview OpenGL context is current.
    unsafe {
        gl.clear_color(r, g, b, a);
        gl.clear(glow::COLOR_BUFFER_BIT);
    }
    match first_pending_gl_error(gl) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn clear_pending_gl_errors(gl: &glow::Context) {
    for _ in 0..16 {
        // SAFETY: callers only use this while the baseview OpenGL context is current.
        if unsafe { gl.get_error() } == glow::NO_ERROR {
            break;
        }
    }
}

fn first_pending_gl_error(gl: &glow::Context) -> Option<u32> {
    // SAFETY: callers only use this while the baseview OpenGL context is current.
    let error = unsafe { gl.get_error() };
    (error != glow::NO_ERROR).then_some(error)
}

fn initial_window_info(size: Size) -> WindowInfo {
    WindowInfo::from_logical_size(size, 1.0)
}

#[cfg(target_os = "macos")]
fn update_current_opengl_drawable() {
    // SAFETY: AppKit owns the current OpenGL context; we only ask it to refresh its drawable
    // after making it current for the active baseview render pass.
    let context: *mut AnyObject = unsafe { msg_send![class!(NSOpenGLContext), currentContext] };
    if context.is_null() {
        return;
    }
    // SAFETY: `context` was returned by AppKit's `currentContext` and is valid for this call.
    unsafe {
        let _: () = msg_send![context, update];
    }
}

#[cfg(not(target_os = "macos"))]
fn update_current_opengl_drawable() {}

#[cfg(target_os = "macos")]
fn clear_current_opengl_context() {
    // SAFETY: This clears AppKit's thread-local current context before/after our egui render pass,
    // so a plugin editor cannot leave a different OpenGL context current on the UI thread.
    unsafe {
        let _: () = msg_send![class!(NSOpenGLContext), clearCurrentContext];
    }
}

#[cfg(not(target_os = "macos"))]
fn clear_current_opengl_context() {}

fn same_size(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() < 0.5 && (a.height - b.height).abs() < 0.5
}

fn pending_resize_reapplies_native_window_on_frame() -> bool {
    !cfg!(target_os = "macos")
}

fn external_resize_updates_child_view_immediately() -> bool {
    !cfg!(target_os = "macos")
}

fn record_programmatic_resize_echo(echoes: &mut VecDeque<Size>, size: Size) {
    if echoes
        .back()
        .is_some_and(|previous| same_size(*previous, size))
    {
        return;
    }
    echoes.push_back(size);
    while echoes.len() > 32 {
        echoes.pop_front();
    }
}

fn consume_programmatic_resize_echo(echoes: &mut VecDeque<Size>, size: Size) -> bool {
    let Some(index) = echoes
        .iter()
        .position(|expected| same_size(*expected, size))
    else {
        return false;
    };
    echoes.remove(index);
    true
}

fn pending_programmatic_resize(echoes: &VecDeque<Size>, size: Size) -> bool {
    echoes.iter().any(|expected| same_size(*expected, size))
}

impl<App> Drop for EguiWindow<App> {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}

impl<App: EguiApp> WindowHandler for EguiWindow<App> {
    fn on_frame(&mut self, window: &mut Window<'_>) {
        let requested_size = self.pending_resize.take();
        if let Some(size) = requested_size {
            record_programmatic_resize_echo(&mut self.programmatic_resize_echoes, size);
            if pending_resize_reapplies_native_window_on_frame() {
                window.resize(size);
            }
            resize_child_view_now(self.child_view.load(Ordering::Acquire), size);
            self.set_window_info(WindowInfo::from_logical_size(
                size,
                self.window_info.scale(),
            ));
        }
        self.render(window);
    }

    fn on_event(&mut self, window: &mut Window<'_>, event: Event) -> EventStatus {
        match event {
            Event::Window(baseview::WindowEvent::Resized(info)) => {
                let logical = info.logical_size();
                if let Some(pending) = self.pending_resize.unconsumed()
                    && !same_size(logical, pending)
                {
                    return EventStatus::Captured;
                }
                if consume_programmatic_resize_echo(&mut self.programmatic_resize_echoes, logical) {
                    return EventStatus::Captured;
                }
                self.set_window_info(info);
            }
            Event::Window(baseview::WindowEvent::Focused) => {
                self.input.focused = true;
            }
            Event::Window(baseview::WindowEvent::Unfocused) => {
                self.input.focused = false;
            }
            Event::Window(baseview::WindowEvent::WillClose) => {}
            Event::Mouse(mouse_event) => self.handle_mouse(window, mouse_event),
            Event::Keyboard(key_event) => {
                if is_command_quit(&key_event) {
                    return EventStatus::Ignored;
                }
                self.handle_keyboard(key_event);
            }
        }
        EventStatus::Captured
    }
}

impl<App: EguiApp> EguiWindow<App> {
    fn handle_mouse(&mut self, window: &mut Window<'_>, event: MouseEvent) {
        match event {
            MouseEvent::CursorMoved { position, .. } => {
                let pos = Pos2::new(position.x as f32, position.y as f32);
                self.pointer_pos = Some(pos);
                self.push_event(egui::Event::PointerMoved(pos));
            }
            MouseEvent::ButtonPressed { button, .. } => {
                window.focus();
                self.input.focused = true;
                if let (Some(pos), Some(button)) = (self.pointer_pos, pointer_button(button)) {
                    if self.app.mouse_button_pressed(pos, button) {
                        return;
                    }
                    self.push_event(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: true,
                        modifiers: egui::Modifiers::default(),
                    });
                }
            }
            MouseEvent::ButtonReleased { button, .. } => {
                if let (Some(pos), Some(button)) = (self.pointer_pos, pointer_button(button)) {
                    self.push_event(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed: false,
                        modifiers: egui::Modifiers::default(),
                    });
                }
            }
            MouseEvent::WheelScrolled { delta, .. } => {
                let delta = match delta {
                    ScrollDelta::Lines { x, y } => Vec2::new(x, y) * 24.0,
                    ScrollDelta::Pixels { x, y } => Vec2::new(x, y),
                };
                self.push_event(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::default(),
                });
            }
            MouseEvent::CursorLeft => {
                self.pointer_pos = None;
                self.push_event(egui::Event::PointerGone);
            }
            MouseEvent::CursorEntered
            | MouseEvent::DragEntered { .. }
            | MouseEvent::DragMoved { .. }
            | MouseEvent::DragLeft
            | MouseEvent::DragDropped { .. } => {}
        }
    }

    fn handle_keyboard(&mut self, event: KeyboardEvent) {
        let modifiers = egui_modifiers(event.modifiers);
        self.input.modifiers = modifiers;
        let pressed = event.state == KeyState::Down;

        if pressed
            && !event.is_composing
            && !modifiers.command
            && !modifiers.ctrl
            && let Key::Character(text) = &event.key
            && is_text_input(text)
        {
            self.push_event(egui::Event::Text(text.clone()));
        }

        if let Some(key) = egui_key(&event.key) {
            self.push_event(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: event.repeat,
                modifiers,
            });
        }
    }
}

fn egui_modifiers(modifiers: Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.contains(Modifiers::ALT),
        ctrl: modifiers.contains(Modifiers::CONTROL),
        shift: modifiers.contains(Modifiers::SHIFT),
        mac_cmd: modifiers.contains(Modifiers::META),
        command: if cfg!(target_os = "macos") {
            modifiers.contains(Modifiers::META)
        } else {
            modifiers.contains(Modifiers::CONTROL)
        },
    }
}

fn is_command_quit(event: &KeyboardEvent) -> bool {
    event.state == KeyState::Down
        && event.modifiers.contains(Modifiers::META)
        && matches!(&event.key, Key::Character(value) if value.eq_ignore_ascii_case("q"))
}

fn is_text_input(text: &str) -> bool {
    !text.chars().any(char::is_control)
}

fn egui_key(key: &Key) -> Option<egui::Key> {
    Some(match key {
        Key::ArrowDown => egui::Key::ArrowDown,
        Key::ArrowLeft => egui::Key::ArrowLeft,
        Key::ArrowRight => egui::Key::ArrowRight,
        Key::ArrowUp => egui::Key::ArrowUp,
        Key::Escape => egui::Key::Escape,
        Key::Tab => egui::Key::Tab,
        Key::Backspace => egui::Key::Backspace,
        Key::Enter => egui::Key::Enter,
        Key::Delete => egui::Key::Delete,
        Key::Home => egui::Key::Home,
        Key::End => egui::Key::End,
        Key::PageUp => egui::Key::PageUp,
        Key::PageDown => egui::Key::PageDown,
        Key::Character(value) if value.len() == 1 => match value.as_bytes()[0] {
            b'0' => egui::Key::Num0,
            b'1' => egui::Key::Num1,
            b'2' => egui::Key::Num2,
            b'3' => egui::Key::Num3,
            b'4' => egui::Key::Num4,
            b'5' => egui::Key::Num5,
            b'6' => egui::Key::Num6,
            b'7' => egui::Key::Num7,
            b'8' => egui::Key::Num8,
            b'9' => egui::Key::Num9,
            b'a' | b'A' => egui::Key::A,
            b'c' | b'C' => egui::Key::C,
            b'v' | b'V' => egui::Key::V,
            b'x' | b'X' => egui::Key::X,
            b'z' | b'Z' => egui::Key::Z,
            _ => return None,
        },
        _ => return None,
    })
}

fn pointer_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        MouseButton::Back => Some(egui::PointerButton::Extra1),
        MouseButton::Forward => Some(egui::PointerButton::Extra2),
        MouseButton::Other(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    #[test]
    fn clears_parented_egui_view_to_transparent() {
        assert_eq!(super::clear_color(), [0.0, 0.0, 0.0, 0.0]);
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
                assert_eq!(size.height, size.width + 1.0);
            }
            std::thread::yield_now();
        }

        for writer in writers {
            writer.join().expect("writer should not panic");
        }
        if let Some(size) = request.take() {
            assert_eq!(size.height, size.width + 1.0);
        }
    }
}
