use super::*;

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
        self.window.raw_window_handle()
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
    _frame: impl EditorFrame + Send + 'static,
) -> Result<InstalledHost, Error> {
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
pub fn set_host_window_visible(host: &WindowSnapshot) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn set_content_view_visible(
    content: &WindowSnapshot,
    visible: bool,
) -> Result<(), Error> {
    macos::set_content_view_visible(content, visible)
}

#[cfg(target_os = "macos")]
pub(crate) fn set_host_window_title(host: &WindowSnapshot, title: &str) -> Result<(), Error> {
    macos::set_host_window_title(host, title)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn set_host_window_title(host: &WindowSnapshot, title: &str) -> Result<(), Error> {
    host.raw_window_handle()?;
    if title.contains('\0') {
        return Err(Error::Message("host title contains a NUL byte".to_string()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn raise_host_window(host: &WindowSnapshot) -> Result<(), Error> {
    macos::raise_host_window(host)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn raise_host_window(host: &WindowSnapshot) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    macos::begin_host_window_drag(host)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn begin_host_window_drag(host: &WindowSnapshot) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn sync_host_window_resize_policy(host: &WindowSnapshot) -> Result<(), Error> {
    macos::sync_host_window_resize_policy(host)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn sync_host_window_resize_policy(host: &WindowSnapshot) -> Result<(), Error> {
    host.raw_window_handle()?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn native_content_size(
    host: &WindowSnapshot,
    titlebar_height: f64,
    frame_thickness: f64,
) -> Result<Option<Size>, Error> {
    macos::native_content_size(host, titlebar_height, frame_thickness).map(Some)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn native_content_size(host: &WindowSnapshot) -> Result<Option<Size>, Error> {
    host.raw_window_handle()?;
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(crate) fn embedded_content_size(content: &WindowSnapshot) -> Result<Option<Size>, Error> {
    macos::embedded_content_size(content)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn embedded_content_size(content: &WindowSnapshot) -> Result<Option<Size>, Error> {
    content.raw_window_handle()?;
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(crate) fn is_host_window_live_resizing(host: &WindowSnapshot) -> Result<bool, Error> {
    macos::is_host_window_live_resizing(host)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn is_host_window_live_resizing(host: &WindowSnapshot) -> Result<bool, Error> {
    host.raw_window_handle()?;
    Ok(false)
}

pub(crate) fn non_null_ptr(value: usize, name: &str) -> Result<NonNull<c_void>, Error> {
    NonNull::new(value as *mut c_void).ok_or_else(|| Error::Message(format!("{name} is null")))
}

pub(crate) fn non_null_ptr_unchecked(value: usize) -> NonNull<c_void> {
    NonNull::new(value as *mut c_void).expect("stored non-null pointer")
}

pub(crate) fn non_zero_isize(value: isize, name: &str) -> Result<NonZeroIsize, Error> {
    NonZeroIsize::new(value).ok_or_else(|| Error::Message(format!("{name} is zero")))
}

pub(crate) fn non_zero_u32(value: u32, name: &str) -> Result<NonZeroU32, Error> {
    NonZeroU32::new(value).ok_or_else(|| Error::Message(format!("{name} is zero")))
}

pub(crate) fn open_egui_frame(
    parent: WindowSnapshot,
    options: &EditorHostOptions,
    content_size: Size,
    outer_width: f64,
    outer_height: f64,
    frame: impl EditorFrame + Send + 'static,
) -> Result<EguiFrameHost, Error> {
    let close_requested = Arc::new(AtomicBool::new(false));
    let title = Arc::new(Mutex::new(options.title.clone()));
    let preset_state = Arc::new(Mutex::new(None));
    let frame_content_size = Arc::new(SharedSize::new(content_size));
    let resizable = Arc::new(AtomicBool::new(options.resizable));
    let zoom_percent = Arc::new(AtomicU32::new(100));
    let frame_commands = Arc::new(Mutex::new(Vec::new()));
    let app = FrameApp {
        frame,
        host: parent,
        title: Arc::clone(&title),
        preset_state: Arc::clone(&preset_state),
        content_size: Arc::clone(&frame_content_size),
        resizable: Arc::clone(&resizable),
        zoom_percent: Arc::clone(&zoom_percent),
        state: EditorHostState {
            title: options.title.clone(),
            resizable: options.resizable,
            zoom_percent: 100,
            close_requested: false,
            content_size,
            preset: None,
        },
        close_requested: Arc::clone(&close_requested),
        frame_commands: Arc::clone(&frame_commands),
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
        preset_state,
        content_size: frame_content_size,
        resizable,
        zoom_percent,
        frame_commands,
    })
}

pub(crate) struct EguiFrameHost {
    pub(crate) window: EguiWindowHandle,
    pub(crate) close_requested: Arc<AtomicBool>,
    pub(crate) title: Arc<Mutex<String>>,
    pub(crate) preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    pub(crate) content_size: Arc<SharedSize>,
    pub(crate) resizable: Arc<AtomicBool>,
    pub(crate) zoom_percent: Arc<AtomicU32>,
    pub(crate) frame_commands: Arc<Mutex<Vec<EditorFrameCommand>>>,
}

struct FrameApp<F> {
    frame: F,
    host: WindowSnapshot,
    title: Arc<Mutex<String>>,
    preset_state: Arc<Mutex<Option<EditorPresetState>>>,
    content_size: Arc<SharedSize>,
    resizable: Arc<AtomicBool>,
    zoom_percent: Arc<AtomicU32>,
    state: EditorHostState,
    close_requested: Arc<AtomicBool>,
    frame_commands: Arc<Mutex<Vec<EditorFrameCommand>>>,
}

impl<F: EditorFrame> EguiApp for FrameApp<F> {
    fn update(&mut self, ui: &mut egui::Ui) {
        self.sync_state();
        let action = self.frame.render(ui, &self.state);
        self.apply_frame_action(action);
        ui.ctx().request_repaint();
    }

    fn mouse_button_pressed(&mut self, pos: egui::Pos2, button: egui::PointerButton) -> bool {
        if button != egui::PointerButton::Primary {
            return false;
        }
        self.sync_state();
        let should_drag = self.frame.should_begin_window_drag(pos, &self.state);
        trace_editor_host(|| {
            format!(
                "frame mouse down pos={pos:?} should_begin_window_drag={should_drag} \
                 content_size={:?}",
                self.state.content_size
            )
        });
        if !should_drag {
            return false;
        }
        trace_editor_host(|| "frame starting native window drag from mouse down".to_string());
        if let Err(error) = begin_host_window_drag(&self.host) {
            trace_editor_host(|| format!("frame native window drag failed: {error}"));
            return false;
        }
        trace_editor_host(|| "frame native window drag returned".to_string());
        true
    }
}

impl<F: EditorFrame> FrameApp<F> {
    fn apply_frame_action(&self, action: EditorFrameAction) {
        match action {
            EditorFrameAction::None => {}
            EditorFrameAction::Close => self.close_requested.store(true, Ordering::Relaxed),
            EditorFrameAction::DragWindow => self.begin_render_loop_window_drag(),
            EditorFrameAction::Command(command) => self.push_frame_command(command),
        }
    }

    fn begin_render_loop_window_drag(&self) {
        trace_editor_host(|| "frame requested render-loop window drag".to_string());
        if let Err(error) = begin_host_window_drag(&self.host) {
            trace_editor_host(|| format!("frame render-loop window drag failed: {error}"));
        }
    }

    fn push_frame_command(&self, command: EditorFrameCommand) {
        if let Ok(mut commands) = self.frame_commands.lock() {
            commands.push(command);
        }
    }

    fn sync_state(&mut self) {
        self.state.close_requested = self.close_requested.load(Ordering::Relaxed);
        if let Ok(title) = self.title.lock() {
            self.state.title.clone_from(&title);
        }
        if let Ok(preset_state) = self.preset_state.lock() {
            self.state.preset.clone_from(&preset_state);
        }
        self.state.content_size = self.content_size.load();
        self.state.resizable = self.resizable.load(Ordering::Relaxed);
        self.state.zoom_percent = self.zoom_percent.load(Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum HostLayoutOperation {
    Resize(ResizeAnchor),
    ResizeFrame(ResizeAnchor),
    Sync,
}

pub(crate) trait FrameWindowResizeTarget {
    fn resize_to_layout(self, layout: crate::EditorFrameLayout);
}

impl FrameWindowResizeTarget for &mut EguiWindowHandle {
    fn resize_to_layout(self, layout: crate::EditorFrameLayout) {
        self.resize(layout.outer_width, layout.outer_height);
    }
}

impl FrameWindowResizeTarget for &EguiWindowResizeHandle {
    fn resize_to_layout(self, layout: crate::EditorFrameLayout) {
        self.resize(layout.outer_width, layout.outer_height);
    }
}

pub(crate) fn resize_installed_host_layout<T: FrameWindowResizeTarget>(
    host: &WindowSnapshot,
    content: Option<&WindowSnapshot>,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
    frame_window: Option<T>,
    operation: HostLayoutOperation,
) -> Result<(), Error> {
    apply_installed_host_layout(
        host,
        content,
        content_size,
        titlebar_height,
        frame_thickness,
        operation,
        frame_window,
    )
}

pub(crate) fn sync_installed_host_layout(
    host: &WindowSnapshot,
    content: &WindowSnapshot,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
    frame_window: Option<&mut EguiWindowHandle>,
) -> Result<(), Error> {
    apply_installed_host_layout(
        host,
        Some(content),
        content_size,
        titlebar_height,
        frame_thickness,
        HostLayoutOperation::Sync,
        frame_window,
    )
}

#[cfg(target_os = "macos")]
fn apply_installed_host_layout<T: FrameWindowResizeTarget>(
    host: &WindowSnapshot,
    content: Option<&WindowSnapshot>,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
    operation: HostLayoutOperation,
    frame_window: Option<T>,
) -> Result<(), Error> {
    let layout = host_layout(
        content_size.width,
        content_size.height,
        titlebar_height,
        frame_thickness,
    );
    let args = InstalledHostLayoutArgs {
        host,
        content,
        content_size,
        titlebar_height,
        frame_thickness,
    };
    apply_macos_installed_host_operation(args, operation)?;
    resize_frame_window_to_layout(frame_window, layout, content_size);
    Ok(())
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct InstalledHostLayoutArgs<'a> {
    host: &'a WindowSnapshot,
    content: Option<&'a WindowSnapshot>,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
}

#[cfg(target_os = "macos")]
fn apply_macos_installed_host_operation(
    args: InstalledHostLayoutArgs<'_>,
    operation: HostLayoutOperation,
) -> Result<(), Error> {
    match operation {
        HostLayoutOperation::Resize(anchor) => apply_macos_content_resize(args, anchor),
        HostLayoutOperation::ResizeFrame(anchor) => apply_macos_frame_resize(args, anchor),
        HostLayoutOperation::Sync => apply_macos_layout_sync(args),
    }
}

#[cfg(target_os = "macos")]
fn required_content(content: Option<&WindowSnapshot>) -> Result<&WindowSnapshot, Error> {
    content.ok_or_else(|| Error::Message("content view is missing".to_string()))
}

#[cfg(target_os = "macos")]
fn apply_macos_content_resize(
    args: InstalledHostLayoutArgs<'_>,
    anchor: ResizeAnchor,
) -> Result<(), Error> {
    macos::resize_installed_host(
        args.host,
        required_content(args.content)?,
        args.content_size,
        args.titlebar_height,
        args.frame_thickness,
        anchor,
    )
}

#[cfg(target_os = "macos")]
fn apply_macos_frame_resize(
    args: InstalledHostLayoutArgs<'_>,
    anchor: ResizeAnchor,
) -> Result<(), Error> {
    macos::resize_installed_frame_host(
        args.host,
        args.content_size,
        args.titlebar_height,
        args.frame_thickness,
        anchor,
    )
}

#[cfg(target_os = "macos")]
fn apply_macos_layout_sync(args: InstalledHostLayoutArgs<'_>) -> Result<(), Error> {
    macos::sync_installed_host_layout(
        args.host,
        required_content(args.content)?,
        args.content_size,
        args.titlebar_height,
        args.frame_thickness,
    )
}

#[cfg(target_os = "macos")]
fn resize_frame_window_to_layout<T: FrameWindowResizeTarget>(
    frame_window: Option<T>,
    layout: EditorFrameLayout,
    content_size: Size,
) {
    let Some(frame_window) = frame_window else {
        return;
    };
    trace_editor_host(|| {
        format!(
            "egui frame layout outer={}x{} content={content_size:?}",
            layout.outer_width, layout.outer_height
        )
    });
    frame_window.resize_to_layout(layout);
}

#[cfg(not(target_os = "macos"))]
fn apply_installed_host_layout<T: FrameWindowResizeTarget>(
    host: &WindowSnapshot,
    content: Option<&WindowSnapshot>,
    content_size: Size,
    titlebar_height: f64,
    frame_thickness: f64,
    operation: HostLayoutOperation,
    frame_window: Option<T>,
) -> Result<(), Error> {
    host.raw_window_handle()?;
    if let Some(content) = content {
        content.raw_window_handle()?;
    }
    if let HostLayoutOperation::Resize(anchor) | HostLayoutOperation::ResizeFrame(anchor) =
        operation
    {
        match anchor {
            ResizeAnchor::Top | ResizeAnchor::Bottom => {}
        }
    }
    let layout = host_layout(
        content_size.width,
        content_size.height,
        titlebar_height,
        frame_thickness,
    );
    if let Some(frame_window) = frame_window {
        frame_window.resize_to_layout(layout);
    }
    Ok(())
}
