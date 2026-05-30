pub use lilypalooza_egui_baseview::egui;
use lilypalooza_egui_baseview::{
    EguiApp, EguiWindowHandle, EguiWindowOptions, EguiWindowResizeHandle, open_parented,
};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, Win32WindowHandle, XcbDisplayHandle,
    XcbWindowHandle, XlibDisplayHandle, XlibWindowHandle,
};

use super::*;

#[cfg(target_os = "macos")]
mod macos;
mod window_ops;
pub use window_ops::*;

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

impl WindowHandleSnapshot {
    fn raw_window_handle(self) -> Result<RawWindowHandle, Error> {
        match self {
            Self::AppKit { ns_view } => appkit_raw_window_handle(ns_view),
            Self::Win32 { hwnd } => win32_raw_window_handle(hwnd),
            Self::Xcb { window } => xcb_raw_window_handle(window),
            Self::Xlib { window } => Ok(RawWindowHandle::Xlib(XlibWindowHandle::new(window))),
            Self::Wayland { surface } => wayland_raw_window_handle(surface),
        }
    }
}

pub(crate) fn appkit_raw_window_handle(ns_view: usize) -> Result<RawWindowHandle, Error> {
    Ok(RawWindowHandle::AppKit(AppKitWindowHandle::new(
        non_null_ptr(ns_view, "ns_view")?,
    )))
}

pub(crate) fn win32_raw_window_handle(hwnd: isize) -> Result<RawWindowHandle, Error> {
    Ok(RawWindowHandle::Win32(Win32WindowHandle::new(
        non_zero_isize(hwnd, "hwnd")?,
    )))
}

pub(crate) fn xcb_raw_window_handle(window: u32) -> Result<RawWindowHandle, Error> {
    Ok(RawWindowHandle::Xcb(XcbWindowHandle::new(non_zero_u32(
        window,
        "xcb window",
    )?)))
}

pub(crate) fn wayland_raw_window_handle(surface: usize) -> Result<RawWindowHandle, Error> {
    Ok(RawWindowHandle::Wayland(WaylandWindowHandle::new(
        non_null_ptr(surface, "wayland surface")?,
    )))
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
        let title = title.into();
        Self {
            title,
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

#[must_use]
pub fn content_size_from_outer_size(
    outer_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
) -> Size {
    let frame = frame_thickness.max(0.0);
    let titlebar_height = titlebar_height.max(20.0);
    Size {
        width: (outer_size.width - frame * 2.0).max(1.0),
        height: (outer_size.height - titlebar_height - frame * 2.0).max(1.0),
    }
}

#[must_use]
pub fn outer_size_from_content_size(
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
) -> Size {
    let layout = host_layout(
        content_size.width,
        content_size.height,
        titlebar_height,
        frame_thickness,
    );
    Size {
        width: layout.outer_width,
        height: layout.outer_height,
    }
}

pub(crate) fn same_size(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() < 0.5 && (a.height - b.height).abs() < 0.5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstalledContentResizeMode {
    NativeContent,
    FrameOnly,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub(crate) struct SharedSize {
    width: AtomicU64,
    height: AtomicU64,
}

impl SharedSize {
    pub(crate) fn new(size: Size) -> Self {
        Self {
            width: AtomicU64::new(size.width.to_bits()),
            height: AtomicU64::new(size.height.to_bits()),
        }
    }

    pub(crate) fn load(&self) -> Size {
        Size {
            width: f64::from_bits(self.width.load(Ordering::Relaxed)),
            height: f64::from_bits(self.height.load(Ordering::Relaxed)),
        }
    }

    pub(crate) fn store(&self, size: Size) {
        self.width.store(size.width.to_bits(), Ordering::Relaxed);
        self.height.store(size.height.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorHostState {
    pub title: String,
    pub resizable: bool,
    pub zoom_percent: u32,
    pub close_requested: bool,
    pub content_size: Size,
    pub preset: Option<EditorPresetState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPresetState {
    pub current_name: String,
    pub selected_id: Option<String>,
    pub expanded: bool,
    pub items: Vec<EditorPresetItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPresetItem {
    pub id: String,
    pub name: String,
    pub origin: EditorPresetOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPresetOrigin {
    User,
    Factory,
    PluginNative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorFrameAction {
    None,
    Close,
    DragWindow,
    Command(EditorFrameCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorFrameCommand {
    PreviousPreset,
    NextPreset,
    SetZoomPercent(u32),
    LoadPreset(String),
    RenamePreset { id: String, name: String },
    DeletePreset(String),
    SavePreset,
    TogglePresetBrowser,
    SetControlsVisible(bool),
}

pub trait EditorFrame {
    fn layout(&self, content_size: Size) -> EditorFrameLayout;

    fn should_begin_window_drag(&self, _pos: egui::Pos2, _state: &EditorHostState) -> bool {
        false
    }

    fn render(&mut self, ui: &mut egui::Ui, state: &EditorHostState) -> EditorFrameAction;
}

pub struct InstalledHost {
    host: WindowSnapshot,
    content: WindowSnapshot,
    close_requested: Arc<AtomicBool>,
    title: Arc<Mutex<String>>,
    preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    frame_commands: Arc<Mutex<Vec<EditorFrameCommand>>>,
    frame_window: Option<EguiWindowHandle>,
    pub(crate) frame_content_size: Arc<SharedSize>,
    frame_resizable: Arc<AtomicBool>,
    pub(crate) frame_zoom_percent: Arc<AtomicU32>,
    content_size: Arc<SharedSize>,
    frame_thickness: f64,
    #[cfg(target_os = "macos")]
    native_content_resize_observer: Option<macos::NativeContentResizeObserver>,
}

#[derive(Clone)]
pub struct InstalledHostResizeHandle {
    host: WindowSnapshot,
    content: WindowSnapshot,
    preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    frame_window: Option<EguiWindowResizeHandle>,
    frame_content_size: Arc<SharedSize>,
    frame_zoom_percent: Arc<AtomicU32>,
    content_size: Arc<SharedSize>,
    frame_thickness: f64,
}

impl std::fmt::Debug for InstalledHostResizeHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InstalledHostResizeHandle")
            .field("host", &self.host)
            .field("content", &self.content)
            .field("content_size", &self.content_size.load())
            .field("frame_thickness", &self.frame_thickness)
            .finish_non_exhaustive()
    }
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
        #[cfg(target_os = "macos")]
        {
            self.native_content_resize_observer.take();
        }
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
    pub fn content_size(&self) -> Size {
        self.content_size.load()
    }

    #[must_use]
    pub fn resize_handle(&self) -> InstalledHostResizeHandle {
        InstalledHostResizeHandle {
            host: self.host,
            content: self.content,
            preset_state: Arc::clone(&self.preset_state),
            frame_window: self
                .frame_window
                .as_ref()
                .map(EguiWindowHandle::resize_handle),
            frame_content_size: Arc::clone(&self.frame_content_size),
            frame_zoom_percent: Arc::clone(&self.frame_zoom_percent),
            content_size: Arc::clone(&self.content_size),
            frame_thickness: self.frame_thickness,
        }
    }

    pub fn enable_native_content_resize_tracking(&self) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        {
            if let Some(observer) = &self.native_content_resize_observer {
                observer.enable(&self.content)
            } else {
                Ok(())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.content.raw_window_handle().map(|_| ())
        }
    }

    pub fn set_native_content_resize_tracking_enabled(&self, enabled: bool) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        {
            if let Some(observer) = &self.native_content_resize_observer {
                observer.set_enabled(enabled, &self.content)
            } else {
                Ok(())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = enabled;
            self.content.raw_window_handle().map(|_| ())
        }
    }

    #[must_use]
    pub fn close_requested(&self) -> bool {
        self.close_requested.load(Ordering::Relaxed)
    }

    pub fn clear_close_requested(&self) {
        self.close_requested.store(false, Ordering::Relaxed);
    }

    pub fn set_preset_state(&mut self, preset: Option<EditorPresetState>) {
        if let Ok(mut current) = self.preset_state.lock() {
            *current = preset;
        }
        if self.frame_window.is_some()
            && let Err(error) = resize_installed_host_layout(
                &self.host,
                Some(&self.content),
                self.content_size(),
                self.titlebar_height(),
                self.frame_thickness,
                self.frame_window.as_mut(),
                HostLayoutOperation::Resize(ResizeAnchor::Bottom),
            )
        {
            trace_editor_host(|| format!("installed-host preset chrome resize failed: {error}"));
        }
    }

    pub fn set_resizable(&mut self, resizable: bool) -> Result<(), Error> {
        self.frame_resizable.store(resizable, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        {
            sync_host_window_resize_policy(&self.host)
        }
        #[cfg(not(target_os = "macos"))]
        {
            sync_host_window_resize_policy(&self.host)
        }
    }

    pub fn set_zoom_percent(&mut self, percent: u32) {
        self.frame_zoom_percent.store(percent, Ordering::Relaxed);
    }

    pub fn resize_content(&mut self, content_size: Size) -> Result<(), Error> {
        self.resize_content_with_anchor(content_size, ResizeAnchor::Bottom)
    }

    pub fn resize_content_from_top(&mut self, content_size: Size) -> Result<(), Error> {
        self.resize_content_with_anchor(content_size, ResizeAnchor::Top)
    }

    pub fn resize_frame_content_from_top(&mut self, content_size: Size) -> Result<(), Error> {
        self.resize_frame_content_with_anchor(content_size, ResizeAnchor::Top)
    }

    /// Accepts a user-driven outer window resize without writing the same size back to the native
    /// window.
    pub fn adopt_content_size_from_outer_resize(
        &mut self,
        content_size: Size,
    ) -> Result<(), Error> {
        let current = self.content_size();
        if same_size(current, content_size) {
            trace_editor_host(|| {
                format!(
                    "installed-host adopt outer resize ignored same size current={:?} \
                     requested={content_size:?}",
                    current
                )
            });
            return Ok(());
        }
        let previous = current;
        self.set_content_size(content_size);
        if self.frame_window.is_none() {
            trace_editor_host(|| {
                format!(
                    "installed-host adopt outer resize stored without frame current={previous:?} \
                     requested={content_size:?}"
                )
            });
            return Ok(());
        }
        trace_editor_host(|| {
            format!(
                "installed-host adopt outer resize applying current={previous:?} \
                 requested={content_size:?}"
            )
        });
        sync_installed_host_layout(
            &self.host,
            &self.content,
            self.content_size(),
            self.titlebar_height(),
            self.frame_thickness,
            self.frame_window.as_mut(),
        )
    }

    pub fn preview_outer_resize(&mut self, outer_size: Size) {
        let content_size = self.content_size_from_outer_size(outer_size);
        self.set_frame_content_size(content_size);
        if let Some(frame_window) = self.frame_window.as_mut() {
            trace_editor_host(|| {
                format!(
                    "egui frame preview outer={}x{} content={content_size:?}",
                    outer_size.width, outer_size.height
                )
            });
            frame_window.resize(outer_size.width, outer_size.height);
        }
    }

    fn resize_content_with_anchor(
        &mut self,
        content_size: Size,
        anchor: ResizeAnchor,
    ) -> Result<(), Error> {
        self.resize_installed_content(
            content_size,
            anchor,
            InstalledContentResizeMode::NativeContent,
        )
    }

    fn resize_frame_content_with_anchor(
        &mut self,
        content_size: Size,
        anchor: ResizeAnchor,
    ) -> Result<(), Error> {
        self.resize_installed_content(content_size, anchor, InstalledContentResizeMode::FrameOnly)
    }

    fn resize_installed_content(
        &mut self,
        content_size: Size,
        anchor: ResizeAnchor,
        mode: InstalledContentResizeMode,
    ) -> Result<(), Error> {
        let current = self.content_size();
        if same_size(current, content_size) {
            trace_editor_host(|| {
                format!(
                    "installed-host resize_content ignored same size mode={mode:?} \
                     current={current:?} requested={content_size:?} anchor={anchor:?}"
                )
            });
            return Ok(());
        }
        let previous = current;
        self.set_content_size(content_size);
        if self.frame_window.is_none() {
            trace_editor_host(|| {
                format!(
                    "installed-host resize_content stored without frame mode={mode:?} \
                     current={previous:?} requested={content_size:?} anchor={anchor:?}"
                )
            });
            return Ok(());
        }
        trace_editor_host(|| {
            format!(
                "installed-host resize_content applying mode={mode:?} current={previous:?} \
                 requested={content_size:?} anchor={anchor:?}"
            )
        });
        match mode {
            InstalledContentResizeMode::NativeContent => resize_installed_host_layout(
                &self.host,
                Some(&self.content),
                self.content_size(),
                self.titlebar_height(),
                self.frame_thickness,
                self.frame_window.as_mut(),
                HostLayoutOperation::Resize(anchor),
            ),
            InstalledContentResizeMode::FrameOnly => resize_installed_host_layout(
                &self.host,
                None,
                self.content_size(),
                self.titlebar_height(),
                self.frame_thickness,
                self.frame_window.as_mut(),
                HostLayoutOperation::ResizeFrame(anchor),
            ),
        }
    }

    fn set_content_size(&mut self, content_size: Size) {
        self.content_size.store(content_size);
        self.set_frame_content_size(content_size);
    }

    pub fn set_frame_content_size(&mut self, content_size: Size) {
        self.frame_content_size.store(content_size);
    }

    pub fn resize_outer(&mut self, outer_size: Size) -> Result<Size, Error> {
        let content_size =
            content_size_from_outer_size(outer_size, self.titlebar_height(), self.frame_thickness);
        self.resize_content(content_size)?;
        Ok(content_size)
    }

    #[must_use]
    pub fn content_size_from_outer_size(&self, outer_size: Size) -> Size {
        content_size_from_outer_size(outer_size, self.titlebar_height(), self.frame_thickness)
    }

    pub fn native_content_size(&self) -> Result<Option<Size>, Error> {
        self.native_content_size_impl()
    }

    pub fn embedded_content_size(&self) -> Result<Option<Size>, Error> {
        self.embedded_content_size_impl()
    }

    pub fn is_live_resizing(&self) -> Result<bool, Error> {
        self.is_live_resizing_impl()
    }

    #[cfg(target_os = "macos")]
    fn native_content_size_impl(&self) -> Result<Option<Size>, Error> {
        native_content_size(&self.host, self.titlebar_height(), self.frame_thickness)
    }

    #[cfg(not(target_os = "macos"))]
    fn native_content_size_impl(&self) -> Result<Option<Size>, Error> {
        native_content_size(&self.host)
    }

    #[cfg(target_os = "macos")]
    fn embedded_content_size_impl(&self) -> Result<Option<Size>, Error> {
        embedded_content_size(&self.content)
    }

    #[cfg(not(target_os = "macos"))]
    fn embedded_content_size_impl(&self) -> Result<Option<Size>, Error> {
        embedded_content_size(&self.content)
    }

    #[cfg(target_os = "macos")]
    fn is_live_resizing_impl(&self) -> Result<bool, Error> {
        is_host_window_live_resizing(&self.host)
    }

    #[cfg(not(target_os = "macos"))]
    fn is_live_resizing_impl(&self) -> Result<bool, Error> {
        is_host_window_live_resizing(&self.host)
    }

    #[must_use]
    pub fn outer_size_from_content_size(&self, content_size: Size) -> Size {
        outer_size_from_content_size(content_size, self.titlebar_height(), self.frame_thickness)
    }

    fn titlebar_height(&self) -> f64 {
        titlebar_height_from_preset_state_value(self.preset_state().as_ref())
    }

    #[must_use]
    pub fn preset_state(&self) -> Option<EditorPresetState> {
        self.preset_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn drain_frame_commands(&mut self) -> Vec<EditorFrameCommand> {
        self.frame_commands
            .lock()
            .map(|mut commands| commands.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        {
            set_host_window_visible(&self.host, visible)
        }
        #[cfg(not(target_os = "macos"))]
        {
            set_host_window_visible(&self.host)
        }
    }

    pub fn set_content_visible(&mut self, visible: bool) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        {
            set_content_view_visible(&self.content, visible)
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.content.raw_window_handle().map(|_| ())
        }
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> Result<(), Error> {
        let title = title.into();
        if let Ok(mut current) = self.title.lock() {
            *current = title.clone();
        }
        set_host_window_title(&self.host, &title)
    }

    pub fn raise(&mut self) -> Result<(), Error> {
        raise_host_window(&self.host)
    }
}

impl InstalledHostResizeHandle {
    pub fn resize_content(&self, content_size: Size) -> Result<(), Error> {
        self.resize_content_with_anchor(content_size, ResizeAnchor::Bottom)
    }

    pub fn resize_content_from_top(&self, content_size: Size) -> Result<(), Error> {
        self.resize_content_with_anchor(content_size, ResizeAnchor::Top)
    }

    fn resize_content_with_anchor(
        &self,
        content_size: Size,
        anchor: ResizeAnchor,
    ) -> Result<(), Error> {
        let current = self.content_size();
        if same_size(current, content_size) {
            trace_editor_host(|| {
                format!(
                    "installed-host resize handle ignored same size current={current:?} \
                     requested={content_size:?} anchor={anchor:?}"
                )
            });
            return Ok(());
        }
        self.content_size.store(content_size);
        self.frame_content_size.store(content_size);
        trace_editor_host(|| {
            format!(
                "installed-host resize handle applying current={current:?} \
                 requested={content_size:?} anchor={anchor:?}"
            )
        });
        resize_installed_host_layout(
            &self.host,
            Some(&self.content),
            content_size,
            self.titlebar_height(),
            self.frame_thickness,
            self.frame_window.as_ref(),
            HostLayoutOperation::Resize(anchor),
        )
    }

    #[must_use]
    pub fn content_size(&self) -> Size {
        self.content_size.load()
    }

    #[must_use]
    pub fn outer_size_from_content_size(&self, content_size: Size) -> Size {
        outer_size_from_content_size(content_size, self.titlebar_height(), self.frame_thickness)
    }

    pub fn set_zoom_percent(&self, percent: u32) {
        self.frame_zoom_percent.store(percent, Ordering::Relaxed);
    }

    fn titlebar_height(&self) -> f64 {
        titlebar_height_from_preset_state(&self.preset_state)
    }
}

pub(crate) fn titlebar_height_from_preset_state(
    preset_state: &Arc<Mutex<Option<EditorPresetState>>>,
) -> f64 {
    preset_state
        .lock()
        .map(|preset| titlebar_height_from_preset_state_value(preset.as_ref()))
        .unwrap_or(34.0)
}

pub(crate) fn titlebar_height_from_preset_state_value(preset: Option<&EditorPresetState>) -> f64 {
    preset.map_or(34.0, |preset| if preset.expanded { 160.0 } else { 34.0 })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeAnchor {
    Top,
    Bottom,
}

pub(crate) fn trace_editor_host(message: impl FnOnce() -> String) {
    log::trace!(
        target: "editor_host",
        "thread={:?} {}",
        std::thread::current().id(),
        message()
    );
}

#[cfg(test)]
impl InstalledHost {
    pub(crate) fn test_with_frame_commands(
        commands: impl IntoIterator<Item = EditorFrameCommand>,
    ) -> (Self, Vec<EditorFrameCommand>) {
        let commands = commands.into_iter().collect::<Vec<_>>();
        let frame_commands = Arc::new(Mutex::new(commands.clone()));
        let content_size = Size {
            width: 440.0,
            height: 360.0,
        };
        let host = WindowSnapshot {
            window: WindowHandleSnapshot::AppKit { ns_view: 1 },
            display: Some(DisplayHandleSnapshot::AppKit),
        };
        (
            Self {
                host,
                content: host,
                close_requested: Arc::new(AtomicBool::new(false)),
                title: Arc::new(Mutex::new("Test".to_string())),
                preset_state: Arc::new(Mutex::new(None)),
                frame_commands,
                frame_window: None,
                frame_content_size: Arc::new(SharedSize::new(content_size)),
                frame_resizable: Arc::new(AtomicBool::new(true)),
                frame_zoom_percent: Arc::new(AtomicU32::new(100)),
                content_size: Arc::new(SharedSize::new(content_size)),
                frame_thickness: 4.0,
                #[cfg(target_os = "macos")]
                native_content_resize_observer: None,
            },
            commands,
        )
    }
}
