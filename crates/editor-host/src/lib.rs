#![allow(missing_docs)]

use std::ffi::c_void;
use std::num::{NonZeroIsize, NonZeroU32};
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub use egui;
use lilypalooza_egui_baseview::{EguiApp, EguiWindowHandle, EguiWindowOptions, open_parented};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, Win32WindowHandle, XcbDisplayHandle,
    XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHandleSnapshot {
    AppKit { ns_view: usize },
    Win32 { hwnd: isize },
    Xcb { window: u32 },
    Xlib { window: u64 },
    Wayland { surface: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayHandleSnapshot {
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
pub struct WindowSnapshot {
    pub window: WindowHandleSnapshot,
    pub display: Option<DisplayHandleSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorHostOptions {
    pub title: String,
    pub resizable: bool,
    pub owner: Option<WindowSnapshot>,
}

impl EditorHostOptions {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            resizable: true,
            owner: None,
        }
    }

    #[must_use]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    #[must_use]
    pub fn with_owner(mut self, owner: WindowSnapshot) -> Self {
        self.owner = Some(owner);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorFrameLayout {
    pub outer_width: f64,
    pub outer_height: f64,
    pub titlebar: Rect,
    pub content: Rect,
}

#[must_use]
pub fn host_layout(
    content_width: f64,
    content_height: f64,
    titlebar_height: f64,
    frame_thickness: f64,
) -> EditorFrameLayout {
    let frame = frame_thickness.max(0.0);
    let titlebar_height = titlebar_height.max(20.0);
    let outer_width = content_width + frame * 2.0;
    let outer_height = content_height + titlebar_height + frame * 2.0;

    EditorFrameLayout {
        outer_width,
        outer_height,
        titlebar: Rect {
            x: frame,
            y: frame + content_height,
            width: content_width,
            height: titlebar_height,
        },
        content: Rect {
            x: frame,
            y: frame,
            width: content_width,
            height: content_height,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHostState {
    pub title: String,
    pub resizable: bool,
    pub close_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorFrameAction {
    None,
    Close,
    DragWindow,
}

pub trait EditorFrame {
    fn layout(&self, content_size: Size) -> EditorFrameLayout;

    fn render(&mut self, ui: &mut egui::Ui, state: &EditorHostState) -> EditorFrameAction;
}

pub struct InstalledHost {
    host: WindowSnapshot,
    content: WindowSnapshot,
    close_requested: Arc<AtomicBool>,
    title: Arc<Mutex<String>>,
    frame_window: Option<EguiWindowHandle>,
}

impl std::fmt::Debug for InstalledHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledHost")
            .field("host", &self.host)
            .field("content", &self.content)
            .field("close_requested", &self.close_requested())
            .finish_non_exhaustive()
    }
}

impl Drop for InstalledHost {
    fn drop(&mut self) {
        if let Some(mut window) = self.frame_window.take() {
            window.close();
        }
    }
}

impl InstalledHost {
    #[must_use]
    pub fn host(&self) -> WindowSnapshot {
        self.host
    }

    #[must_use]
    pub fn content(&self) -> WindowSnapshot {
        self.content
    }

    #[must_use]
    pub fn close_requested(&self) -> bool {
        self.close_requested.load(Ordering::Relaxed)
    }

    pub fn clear_close_requested(&self) {
        self.close_requested.store(false, Ordering::Relaxed);
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), Error> {
        set_host_window_visible(&self.host, visible)
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> Result<(), Error> {
        let title = title.into();
        if let Ok(mut current) = self.title.lock() {
            *current = title.clone();
        }
        set_host_window_title(&self.host, &title)
    }
}

impl WindowSnapshot {
    pub fn capture(
        window: RawWindowHandle,
        display: Option<RawDisplayHandle>,
    ) -> Result<Self, Error> {
        let window = match window {
            RawWindowHandle::AppKit(handle) => WindowHandleSnapshot::AppKit {
                ns_view: handle.ns_view.as_ptr() as usize,
            },
            RawWindowHandle::Win32(handle) => WindowHandleSnapshot::Win32 {
                hwnd: handle.hwnd.get(),
            },
            RawWindowHandle::Xcb(handle) => WindowHandleSnapshot::Xcb {
                window: handle.window.get(),
            },
            RawWindowHandle::Xlib(handle) => WindowHandleSnapshot::Xlib {
                window: handle.window,
            },
            RawWindowHandle::Wayland(handle) => WindowHandleSnapshot::Wayland {
                surface: handle.surface.as_ptr() as usize,
            },
            other => {
                return Err(Error::Message(format!(
                    "unsupported window handle: {other:?}"
                )));
            }
        };
        let display = display
            .map(|display| match display {
                RawDisplayHandle::AppKit(_) => Ok(DisplayHandleSnapshot::AppKit),
                RawDisplayHandle::Xcb(handle) => Ok(DisplayHandleSnapshot::Xcb {
                    connection: handle
                        .connection
                        .map(|connection| connection.as_ptr() as usize),
                    screen: handle.screen,
                }),
                RawDisplayHandle::Xlib(handle) => Ok(DisplayHandleSnapshot::Xlib {
                    display: handle.display.map(|display| display.as_ptr() as usize),
                    screen: handle.screen,
                }),
                RawDisplayHandle::Wayland(handle) => Ok(DisplayHandleSnapshot::Wayland {
                    display: handle.display.as_ptr() as usize,
                }),
                other => Err(Error::Message(format!(
                    "unsupported display handle: {other:?}"
                ))),
            })
            .transpose()?;

        Ok(Self { window, display })
    }

    pub fn raw_window_handle(&self) -> Result<RawWindowHandle, Error> {
        match self.window {
            WindowHandleSnapshot::AppKit { ns_view } => Ok(RawWindowHandle::AppKit(
                AppKitWindowHandle::new(non_null_ptr(ns_view, "ns_view")?),
            )),
            WindowHandleSnapshot::Win32 { hwnd } => Ok(RawWindowHandle::Win32(
                Win32WindowHandle::new(non_zero_isize(hwnd, "hwnd")?),
            )),
            WindowHandleSnapshot::Xcb { window } => Ok(RawWindowHandle::Xcb(XcbWindowHandle::new(
                non_zero_u32(window, "xcb window")?,
            ))),
            WindowHandleSnapshot::Xlib { window } => {
                Ok(RawWindowHandle::Xlib(XlibWindowHandle::new(window)))
            }
            WindowHandleSnapshot::Wayland { surface } => Ok(RawWindowHandle::Wayland(
                WaylandWindowHandle::new(non_null_ptr(surface, "wayland surface")?),
            )),
        }
    }

    pub fn raw_display_handle(&self) -> Result<Option<RawDisplayHandle>, Error> {
        self.display
            .map(|display| match display {
                DisplayHandleSnapshot::AppKit => Ok::<RawDisplayHandle, Error>(
                    RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
                ),
                DisplayHandleSnapshot::Xcb { connection, screen } => {
                    Ok::<RawDisplayHandle, Error>(RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                        connection.map(non_null_ptr_unchecked),
                        screen,
                    )))
                }
                DisplayHandleSnapshot::Xlib { display, screen } => {
                    Ok::<RawDisplayHandle, Error>(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                        display.map(non_null_ptr_unchecked),
                        screen,
                    )))
                }
                DisplayHandleSnapshot::Wayland { display } => {
                    Ok::<RawDisplayHandle, Error>(RawDisplayHandle::Wayland(
                        WaylandDisplayHandle::new(non_null_ptr(display, "wayland display")?),
                    ))
                }
            })
            .transpose()
    }
}

pub fn prepare_process() -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        macos::prepare_process()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub fn route_app_quit_to_window_close(window: &WindowSnapshot) -> Result<(), Error> {
    macos::route_app_quit_to_window_close(window)
}

#[cfg(not(target_os = "macos"))]
pub fn route_app_quit_to_window_close(window: &WindowSnapshot) -> Result<(), Error> {
    window.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn install_editor_host(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
    frame: impl EditorFrame + Send + 'static,
) -> Result<InstalledHost, Error> {
    macos::install_editor_host(host, options, frame)
}

#[cfg(not(target_os = "macos"))]
pub fn install_editor_host(
    host: &WindowSnapshot,
    options: &EditorHostOptions,
    frame: impl EditorFrame + Send + 'static,
) -> Result<InstalledHost, Error> {
    let _ = frame;
    host.raw_window_handle()?;
    if options.title.contains('\0') {
        return Err(Error::Message("host title contains a NUL byte".to_string()));
    }
    Err(Error::Message(
        "editor host framing backend is only implemented on macOS".to_string(),
    ))
}

#[cfg(target_os = "macos")]
pub fn set_host_window_visible(host: &WindowSnapshot, visible: bool) -> Result<(), Error> {
    macos::set_host_window_visible(host, visible)
}

#[cfg(not(target_os = "macos"))]
pub fn set_host_window_visible(host: &WindowSnapshot, visible: bool) -> Result<(), Error> {
    host.raw_window_handle()?;
    if visible {
        return Ok(());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_host_window_title(host: &WindowSnapshot, title: &str) -> Result<(), Error> {
    macos::set_host_window_title(host, title)
}

#[cfg(not(target_os = "macos"))]
fn set_host_window_title(host: &WindowSnapshot, _title: &str) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    macos::begin_host_window_drag(host)
}

#[cfg(not(target_os = "macos"))]
fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

fn non_null_ptr(value: usize, name: &str) -> Result<NonNull<c_void>, Error> {
    NonNull::new(value as *mut c_void).ok_or_else(|| Error::Message(format!("{name} is null")))
}

fn non_null_ptr_unchecked(value: usize) -> NonNull<c_void> {
    NonNull::new(value as *mut c_void).expect("stored non-null pointer")
}

fn non_zero_isize(value: isize, name: &str) -> Result<NonZeroIsize, Error> {
    NonZeroIsize::new(value).ok_or_else(|| Error::Message(format!("{name} is zero")))
}

fn non_zero_u32(value: u32, name: &str) -> Result<NonZeroU32, Error> {
    NonZeroU32::new(value).ok_or_else(|| Error::Message(format!("{name} is zero")))
}

pub(crate) fn open_egui_frame(
    parent: WindowSnapshot,
    options: &EditorHostOptions,
    outer_width: f64,
    outer_height: f64,
    frame: impl EditorFrame + Send + 'static,
) -> Result<EguiFrameHost, Error> {
    let close_requested = Arc::new(AtomicBool::new(false));
    let title = Arc::new(Mutex::new(options.title.clone()));
    let app = FrameApp {
        frame,
        host: parent,
        title: Arc::clone(&title),
        state: EditorHostState {
            title: options.title.clone(),
            resizable: options.resizable,
            close_requested: false,
        },
        close_requested: Arc::clone(&close_requested),
    };
    let window = open_parented(
        parent.raw_window_handle()?,
        EguiWindowOptions {
            title: options.title.clone(),
            width: outer_width,
            height: outer_height,
        },
        move || app,
    )
    .map_err(|error| Error::Message(error.to_string()))?;
    Ok(EguiFrameHost {
        window,
        close_requested,
        title,
    })
}

pub(crate) struct EguiFrameHost {
    pub(crate) window: EguiWindowHandle,
    pub(crate) close_requested: Arc<AtomicBool>,
    pub(crate) title: Arc<Mutex<String>>,
}

struct FrameApp<F> {
    frame: F,
    host: WindowSnapshot,
    title: Arc<Mutex<String>>,
    state: EditorHostState,
    close_requested: Arc<AtomicBool>,
}

impl<F: EditorFrame> EguiApp for FrameApp<F> {
    fn update(&mut self, ctx: &egui::Context) {
        self.state.close_requested = self.close_requested.load(Ordering::Relaxed);
        if let Ok(title) = self.title.lock() {
            self.state.title.clone_from(&title);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| match self.frame.render(ui, &self.state) {
                EditorFrameAction::None => {}
                EditorFrameAction::Close => {
                    self.close_requested.store(true, Ordering::Relaxed);
                }
                EditorFrameAction::DragWindow => {
                    let _ = begin_host_window_drag(&self.host);
                }
            });
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use raw_window_handle::{
        AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    use super::{
        EditorFrame, EditorFrameAction, EditorHostOptions, EditorHostState, Size, WindowSnapshot,
        host_layout,
    };

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

        assert_eq!(layout.content.height, 360.0);
        assert!(layout.titlebar.y >= layout.content.y + layout.content.height);
        assert!(layout.outer_height > 360.0);
    }

    #[test]
    fn host_layout_keeps_content_unclipped_inside_frame() {
        let layout = host_layout(440.0, 360.0, 30.0, 4.0);

        assert_eq!(layout.content.width, 440.0);
        assert_eq!(layout.content.height, 360.0);
        assert_eq!(layout.content.x, 4.0);
        assert_eq!(layout.content.y, 4.0);
    }

    #[test]
    fn host_layout_adds_frame_to_content_slot() {
        let layout = host_layout(820.0, 456.0, 30.0, 4.0);

        assert_eq!(layout.outer_width, 828.0);
        assert_eq!(layout.outer_height, 494.0);
        assert_eq!(layout.content.width, 820.0);
        assert_eq!(layout.content.height, 456.0);
        assert_eq!(layout.titlebar.height, 30.0);
        assert_eq!(layout.titlebar.y, 460.0);
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
                _ui: &mut egui::Ui,
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
        assert_eq!(layout.content.width, 440.0);
        assert_eq!(layout.content.height, 360.0);
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
