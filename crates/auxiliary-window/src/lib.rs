#![allow(missing_docs)]

use std::ffi::c_void;
use std::num::{NonZeroIsize, NonZeroU32};
use std::ptr::NonNull;

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
pub struct HostOptions {
    pub title: String,
    pub resizable: bool,
    pub decoration: Decoration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decoration {
    pub titlebar_height: f64,
    pub frame_thickness: f64,
    pub corner_radius: f64,
}

impl HostOptions {
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            resizable: true,
            decoration: Decoration::default(),
        }
    }

    #[must_use]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

impl Default for Decoration {
    fn default() -> Self {
        Self {
            titlebar_height: 30.0,
            frame_thickness: 4.0,
            corner_radius: 8.0,
        }
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
pub struct HostLayout {
    pub outer_width: f64,
    pub outer_height: f64,
    pub titlebar: Rect,
    pub content: Rect,
}

#[must_use]
pub fn host_layout(content_width: f64, content_height: f64, options: &HostOptions) -> HostLayout {
    let frame = options.decoration.frame_thickness.max(0.0);
    let titlebar_height = options.decoration.titlebar_height.max(20.0);

    HostLayout {
        outer_width: content_width + frame * 2.0,
        outer_height: content_height + titlebar_height + frame * 2.0,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledHost {
    pub content: WindowSnapshot,
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

pub fn install_editor_host(
    host: &WindowSnapshot,
    owner: Option<&WindowSnapshot>,
    options: &HostOptions,
) -> Result<InstalledHost, Error> {
    #[cfg(target_os = "macos")]
    {
        macos::install_editor_host(host, owner, options)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = owner;
        let _ = options;
        Ok(InstalledHost { content: *host })
    }
}

pub fn set_host_window_visible(host: &WindowSnapshot, visible: bool) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        macos::set_host_window_visible(host, visible)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (host, visible);
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use raw_window_handle::{
        AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    use super::{Decoration, HostOptions, WindowSnapshot, host_layout};

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
        let layout = host_layout(440.0, 360.0, &HostOptions::new("Editor"));

        assert_eq!(layout.content.height, 360.0);
        assert!(layout.titlebar.y >= layout.content.y + layout.content.height);
        assert!(layout.outer_height > 360.0);
    }

    #[test]
    fn host_layout_keeps_content_unclipped_inside_frame() {
        let layout = host_layout(440.0, 360.0, &HostOptions::new("Editor"));

        assert_eq!(layout.content.width, 440.0);
        assert_eq!(layout.content.height, 360.0);
        assert_eq!(layout.content.x, 4.0);
        assert_eq!(layout.content.y, 4.0);
    }

    #[test]
    fn host_options_default_to_resizable() {
        assert!(HostOptions::new("Editor").resizable);
    }

    #[test]
    fn host_options_can_disable_resizing() {
        assert!(!HostOptions::new("Editor").with_resizable(false).resizable);
    }

    #[test]
    fn host_options_use_default_decoration() {
        assert_eq!(
            HostOptions::new("Editor").decoration,
            Decoration {
                titlebar_height: 30.0,
                frame_thickness: 4.0,
                corner_radius: 8.0,
            }
        );
    }
}
