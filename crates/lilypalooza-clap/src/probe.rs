use super::{
    runtime::{
        host_extension_for_id, host_request_callback, host_request_process, host_request_restart,
    },
    *,
};

/// Stable adapter backend format.
pub const FORMAT: &str = "clap";

pub(super) fn trace_clap_editor(message: impl FnOnce() -> String) {
    log::trace!(
        target: "lilypalooza_clap",
        "clap-editor thread={:?} {}",
        std::thread::current().id(),
        message()
    );
}

/// One CLAP plugin discovered inside a CLAP binary or bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClapPluginMetadata {
    /// Stable host id used by persisted processor slots.
    pub processor_id: String,
    /// CLAP plugin id from the descriptor.
    pub clap_id: String,
    /// Display name.
    pub name: String,
    /// Optional vendor.
    pub vendor: Option<String>,
    /// Optional version.
    pub version: Option<String>,
    /// Descriptor feature strings.
    pub features: Vec<String>,
    /// Lilypalooza registry role.
    pub role: registry::Role,
    /// Original candidate path.
    pub path: PathBuf,
    /// Resolved dynamic library path.
    pub library_path: PathBuf,
}

/// Result returned by the validator process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Validated format.
    pub format: String,
    /// Candidate path.
    pub path: PathBuf,
    /// Probe outcome.
    pub result: Result<Vec<ClapPluginMetadata>, String>,
}

/// CLAP probe errors.
#[derive(Debug, thiserror::Error)]
pub enum ClapProbeError {
    /// Candidate path does not look like a CLAP plugin.
    #[error("not a CLAP candidate: {0}")]
    NotCandidate(String),
    /// Path could not be converted for CLAP entry initialization.
    #[error("plugin path contains an interior NUL byte: {0}")]
    InvalidPath(String),
    /// Dynamic library loading failed.
    #[error("failed to load CLAP library {path}: {error}")]
    Load {
        /// Dynamic library path.
        path: PathBuf,
        /// Loader error.
        error: String,
    },
    /// Required CLAP entry symbol is missing.
    #[error("CLAP entry symbol is missing in {0}")]
    MissingEntry(PathBuf),
    /// CLAP version is unsupported.
    #[error("CLAP version is not compatible")]
    IncompatibleVersion,
    /// CLAP entry initialization failed.
    #[error("CLAP entry initialization failed")]
    InitFailed,
    /// Required CLAP function pointer is missing.
    #[error("CLAP entry is missing required function: {0}")]
    MissingFunction(&'static str),
    /// Required CLAP plugin factory is missing.
    #[error("CLAP plugin factory is missing")]
    MissingFactory,
    /// CLAP plugin factory did not expose any plugin descriptors.
    #[error("CLAP plugin factory exposes no plugin descriptors")]
    NoPluginDescriptors,
    /// Plugin descriptor is invalid.
    #[error("CLAP plugin descriptor {index} is invalid")]
    InvalidDescriptor {
        /// Descriptor index reported by the plugin factory.
        index: u32,
    },
}

/// Returns true when a path is a CLAP candidate.
#[must_use]
pub fn is_clap_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("clap"))
}

/// Finds CLAP candidates under one root.
pub fn candidate_paths(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut candidates = Vec::new();
    if !root.is_dir() {
        return Ok(candidates);
    }

    collect_candidate_paths(root, &mut candidates)?;
    candidates.sort();
    Ok(candidates)
}

pub(super) fn collect_candidate_paths(
    root: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if is_clap_candidate(&path) {
            candidates.push(path);
        } else if path.is_dir() {
            collect_candidate_paths(&path, candidates)?;
        }
    }
    Ok(())
}

/// Probes one CLAP candidate in-process. Call this from the validator subprocess, not the app.
pub fn probe(path: &Path) -> Result<Vec<ClapPluginMetadata>, ClapProbeError> {
    validate_clap_probe_candidate(path)?;
    let context = load_clap_probe_context(path)?;
    probe_with_context(path, context)
}

pub(super) fn validate_clap_probe_candidate(path: &Path) -> Result<(), ClapProbeError> {
    is_clap_candidate(path)
        .then_some(())
        .ok_or_else(|| ClapProbeError::NotCandidate(path.display().to_string()))
}

pub(super) fn load_clap_probe_context(path: &Path) -> Result<ClapProbeContext, ClapProbeError> {
    let library_path = resolve_clap_library_path(path);
    let library = load_clap_library(&library_path)?;
    let entry = load_clap_entry(&library, &library_path)?;
    let functions = clap_entry_functions(entry)?;
    Ok(ClapProbeContext {
        library_path,
        _library: library,
        functions,
    })
}

pub(super) fn probe_with_context(
    path: &Path,
    context: ClapProbeContext,
) -> Result<Vec<ClapPluginMetadata>, ClapProbeError> {
    let path_c = clap_probe_path_cstring(path)?;
    let _initialized = InitializedClapEntry::init(&context.functions, &path_c)?;
    probe_initialized_factory(path, &context.library_path, context.functions.get_factory)
}

pub(super) fn clap_probe_path_cstring(path: &Path) -> Result<CString, ClapProbeError> {
    CString::new(path.display().to_string())
        .map_err(|_error| ClapProbeError::InvalidPath(path.display().to_string()))
}

pub(super) struct ClapProbeContext {
    pub(super) library_path: PathBuf,
    pub(super) _library: libloading::Library,
    pub(super) functions: ClapEntryFunctions,
}

pub(super) fn load_clap_library(
    library_path: &Path,
) -> Result<libloading::Library, ClapProbeError> {
    load_library(library_path, |path, error| ClapProbeError::Load {
        path,
        error,
    })
}

pub(super) fn load_clap_entry<'a>(
    library: &'a libloading::Library,
    library_path: &Path,
) -> Result<&'a clap_sys::entry::clap_plugin_entry, ClapProbeError> {
    // SAFETY: The symbol name is the CLAP ABI entry point. The returned pointer is checked before
    // dereference and the library stays loaded for the whole probe.
    let symbol =
        unsafe { library.get::<*const clap_sys::entry::clap_plugin_entry>(b"clap_entry\0") }
            .map_err(|_error| ClapProbeError::MissingEntry(library_path.to_path_buf()))?;
    let entry = *symbol;
    // SAFETY: The returned entry pointer is checked before use and the library stays loaded.
    let entry = unsafe { entry.as_ref() }
        .ok_or_else(|| ClapProbeError::MissingEntry(library_path.to_path_buf()))?;

    if !clap_version_is_compatible(entry.clap_version) {
        return Err(ClapProbeError::IncompatibleVersion);
    }
    Ok(entry)
}

pub(super) struct ClapEntryFunctions {
    pub(super) init: unsafe extern "C" fn(*const c_char) -> bool,
    pub(super) deinit: unsafe extern "C" fn(),
    pub(super) get_factory: unsafe extern "C" fn(*const c_char) -> *const c_void,
}

pub(super) fn clap_entry_functions(
    entry: &clap_sys::entry::clap_plugin_entry,
) -> Result<ClapEntryFunctions, ClapProbeError> {
    let init = entry.init.ok_or(ClapProbeError::MissingFunction("init"))?;
    let deinit = entry
        .deinit
        .ok_or(ClapProbeError::MissingFunction("deinit"))?;
    let get_factory = entry
        .get_factory
        .ok_or(ClapProbeError::MissingFunction("get_factory"))?;
    Ok(ClapEntryFunctions {
        init,
        deinit,
        get_factory,
    })
}

pub(super) struct InitializedClapEntry {
    pub(super) deinit: unsafe extern "C" fn(),
}

impl InitializedClapEntry {
    pub(super) fn init(
        functions: &ClapEntryFunctions,
        path_c: &CString,
    ) -> Result<Self, ClapProbeError> {
        // SAFETY: Function pointer comes from the validated CLAP entry and receives a
        // NUL-terminated path string valid for the duration of the call.
        if unsafe { !((functions.init)(path_c.as_ptr())) } {
            return Err(ClapProbeError::InitFailed);
        }
        Ok(Self {
            deinit: functions.deinit,
        })
    }
}

impl Drop for InitializedClapEntry {
    fn drop(&mut self) {
        // SAFETY: `deinit` is paired with successful `init` for this CLAP entry.
        unsafe { (self.deinit)() };
    }
}

pub(super) fn probe_initialized_factory(
    path: &Path,
    library_path: &Path,
    get_factory: unsafe extern "C" fn(*const std::ffi::c_char) -> *const c_void,
) -> Result<Vec<ClapPluginMetadata>, ClapProbeError> {
    let access = clap_factory_access(get_factory)?;
    collect_clap_factory_metadata(path, library_path, access)
}

pub(super) struct ClapFactoryAccess {
    pub(super) factory: &'static clap_plugin_factory,
    pub(super) count: u32,
    pub(super) descriptor:
        unsafe extern "C" fn(*const clap_plugin_factory, u32) -> *const clap_plugin_descriptor,
}

pub(super) fn clap_factory_access(
    get_factory: unsafe extern "C" fn(*const std::ffi::c_char) -> *const c_void,
) -> Result<ClapFactoryAccess, ClapProbeError> {
    let factory = clap_plugin_factory(get_factory)?;
    let count = clap_plugin_count(factory)?;
    let descriptor = factory
        .get_plugin_descriptor
        .ok_or(ClapProbeError::MissingFunction("get_plugin_descriptor"))?;
    Ok(ClapFactoryAccess {
        factory,
        count,
        descriptor,
    })
}

pub(super) fn clap_plugin_factory(
    get_factory: unsafe extern "C" fn(*const std::ffi::c_char) -> *const c_void,
) -> Result<&'static clap_plugin_factory, ClapProbeError> {
    // SAFETY: Function pointer comes from CLAP entry; factory id is a static C string.
    let factory = unsafe { get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
    // SAFETY: Factory pointer comes from CLAP `get_factory` and is checked for null.
    unsafe { (factory as *const clap_plugin_factory).as_ref() }
        .ok_or(ClapProbeError::MissingFactory)
}

pub(super) fn clap_plugin_count(factory: &clap_plugin_factory) -> Result<u32, ClapProbeError> {
    let count = factory
        .get_plugin_count
        .ok_or(ClapProbeError::MissingFunction("get_plugin_count"))?;
    // SAFETY: CLAP factory function pointer is valid while the CLAP entry is initialized.
    let count = unsafe { count(factory) };
    if count == 0 {
        return Err(ClapProbeError::NoPluginDescriptors);
    }
    Ok(count)
}

pub(super) fn collect_clap_factory_metadata(
    path: &Path,
    library_path: &Path,
    access: ClapFactoryAccess,
) -> Result<Vec<ClapPluginMetadata>, ClapProbeError> {
    let mut plugins = Vec::with_capacity(access.count as usize);
    for index in 0..access.count {
        // SAFETY: Index is below the factory-reported count.
        let desc = unsafe { (access.descriptor)(access.factory, index) };
        let desc = unsafe_descriptor(desc).ok_or(ClapProbeError::InvalidDescriptor { index })?;
        plugins.push(metadata_from_descriptor(path, library_path, desc, index)?);
    }
    Ok(plugins)
}

pub(super) fn unsafe_descriptor(
    descriptor: *const clap_plugin_descriptor,
) -> Option<&'static clap_plugin_descriptor> {
    // SAFETY: Caller passes a descriptor pointer returned by CLAP. Null is handled.
    unsafe { descriptor.as_ref() }
}

pub(super) fn metadata_from_descriptor(
    path: &Path,
    library_path: &Path,
    descriptor: &clap_plugin_descriptor,
    index: u32,
) -> Result<ClapPluginMetadata, ClapProbeError> {
    if !clap_version_is_compatible(descriptor.clap_version) {
        return Err(ClapProbeError::InvalidDescriptor { index });
    }
    let clap_id = cstr_field(descriptor.id).ok_or(ClapProbeError::InvalidDescriptor { index })?;
    let name = cstr_field(descriptor.name).ok_or(ClapProbeError::InvalidDescriptor { index })?;
    let features = features_from_descriptor(descriptor);
    let role = role_from_features(&features);

    Ok(ClapPluginMetadata {
        processor_id: stable_processor_id(path, &clap_id),
        clap_id,
        name,
        vendor: cstr_field(descriptor.vendor),
        version: cstr_field(descriptor.version),
        features,
        role,
        path: path.to_path_buf(),
        library_path: library_path.to_path_buf(),
    })
}

pub(super) fn features_from_descriptor(descriptor: &clap_plugin_descriptor) -> Vec<String> {
    let mut features = Vec::new();
    let mut cursor = descriptor.features;
    if cursor.is_null() {
        return features;
    }

    loop {
        // SAFETY: CLAP feature arrays are null-terminated. We stop at the first null pointer.
        let ptr = unsafe { *cursor };
        if ptr.is_null() {
            break;
        }
        if let Some(feature) = cstr_field(ptr) {
            features.push(feature);
        }
        // SAFETY: Advancing within a CLAP null-terminated feature pointer array.
        cursor = unsafe { cursor.add(1) };
    }
    features
}

pub(super) fn role_from_features(features: &[String]) -> registry::Role {
    let instrument_features = [
        CLAP_PLUGIN_FEATURE_INSTRUMENT,
        CLAP_PLUGIN_FEATURE_SYNTHESIZER,
        CLAP_PLUGIN_FEATURE_SAMPLER,
        CLAP_PLUGIN_FEATURE_DRUM,
        CLAP_PLUGIN_FEATURE_DRUM_MACHINE,
    ];
    if instrument_features
        .iter()
        .any(|feature| has_feature(features, feature))
    {
        registry::Role::Instrument
    } else {
        let _ = CLAP_PLUGIN_FEATURE_AUDIO_EFFECT;
        registry::Role::Effect
    }
}

pub(super) fn has_feature(features: &[String], feature: &CStr) -> bool {
    let feature = feature.to_string_lossy();
    features
        .iter()
        .any(|candidate| candidate == feature.as_ref())
}

pub(super) fn cstr_field(value: *const std::ffi::c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // SAFETY: CLAP descriptor string fields are expected to be valid NUL-terminated strings.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(str::to_string)
}

/// Builds the stable global processor id for one CLAP plugin.
#[must_use]
pub fn stable_processor_id(path: &Path, clap_id: &str) -> String {
    format!("{FORMAT}:{}#{clap_id}", path.display())
}

/// Resolves the actual dynamic library path for a CLAP candidate.
#[must_use]
pub fn resolve_clap_library_path(path: &Path) -> PathBuf {
    let macos_bundle = path.join("Contents").join("MacOS").join(
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default(),
    );
    if macos_bundle.is_file() {
        macos_bundle
    } else {
        path.to_path_buf()
    }
}

/// Parsed components of a stable CLAP processor id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeKey {
    /// Candidate bundle/library path.
    pub path: PathBuf,
    /// CLAP descriptor id.
    pub clap_id: String,
}

impl RuntimeKey {
    /// Parses ids produced by [`stable_processor_id`].
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        let rest = id.strip_prefix("clap:")?;
        let (path, clap_id) = rest.rsplit_once('#')?;
        (!path.is_empty() && !clap_id.is_empty()).then(|| Self {
            path: PathBuf::from(path),
            clap_id: clap_id.to_string(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ClapRuntimeError {
    #[error("unknown CLAP plugin `{0}`")]
    UnknownPlugin(String),
    #[error("CLAP plugin id is invalid: {0}")]
    InvalidPluginId(String),
    #[error("failed to load CLAP library {path}: {error}")]
    Load { path: PathBuf, error: String },
    #[error("CLAP entry symbol is missing in {0}")]
    MissingEntry(PathBuf),
    #[error("CLAP version is not compatible")]
    IncompatibleVersion,
    #[error("CLAP entry initialization failed")]
    InitFailed,
    #[error("CLAP entry is missing required function: {0}")]
    MissingFunction(&'static str),
    #[error("CLAP plugin factory is missing")]
    MissingFactory,
    #[error("CLAP plugin creation failed")]
    CreatePluginFailed,
    #[error("CLAP plugin init failed")]
    PluginInitFailed,
    #[error("CLAP plugin activation failed")]
    ActivateFailed,
    #[error("CLAP plugin processing start failed")]
    StartProcessingFailed,
    #[error("CLAP plugin does not provide a process callback")]
    MissingProcess,
}

pub(super) const DEFAULT_CLAP_EDITOR_DESCRIPTOR: EditorDescriptor = EditorDescriptor {
    default_size: EditorSize {
        width: 720,
        height: 480,
    },
    min_size: Some(EditorSize {
        width: 320,
        height: 220,
    }),
    resizable: false,
};

pub(super) static PLUGIN_METADATA: OnceLock<RwLock<HashMap<String, ClapPluginMetadata>>> =
    OnceLock::new();
pub(super) static LOADED_MODULES: OnceLock<Mutex<HashMap<PathBuf, Arc<LoadedModule>>>> =
    OnceLock::new();

pub(super) fn metadata_store() -> &'static RwLock<HashMap<String, ClapPluginMetadata>> {
    PLUGIN_METADATA.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn loaded_modules() -> &'static Mutex<HashMap<PathBuf, Arc<LoadedModule>>> {
    LOADED_MODULES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) struct LoadedModule {
    pub(super) _library: libloading::Library,
    pub(super) entry: NonNull<clap_sys::entry::clap_plugin_entry>,
}

// SAFETY: The dynamic library is pinned in `LOADED_MODULES`; `entry` points into that live library
// and is only used through CLAP function pointers that are themselves thread-safe by host contract.
unsafe impl Send for LoadedModule {}
// SAFETY: See the `Send` impl; shared access never mutates Rust-owned state inside `LoadedModule`.
unsafe impl Sync for LoadedModule {}

pub(super) fn load_module(
    plugin_path: &Path,
    library_path: &Path,
) -> Result<Arc<LoadedModule>, ClapRuntimeError> {
    let mut modules = loaded_modules()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(module) = modules.get(library_path) {
        return Ok(module.clone());
    }

    let module = Arc::new(load_uncached_module(plugin_path, library_path)?);
    modules.insert(library_path.to_path_buf(), module.clone());
    Ok(module)
}

pub(super) fn load_uncached_module(
    plugin_path: &Path,
    library_path: &Path,
) -> Result<LoadedModule, ClapRuntimeError> {
    let library = load_runtime_library(library_path)?;
    let entry = load_runtime_entry(&library, library_path)?;
    validate_runtime_entry(plugin_path, entry)?;
    Ok(LoadedModule {
        _library: library,
        entry,
    })
}

pub(super) fn load_runtime_library(
    library_path: &Path,
) -> Result<libloading::Library, ClapRuntimeError> {
    load_library(library_path, |path, error| ClapRuntimeError::Load {
        path,
        error,
    })
}

fn load_library<E>(
    library_path: &Path,
    load_error: impl FnOnce(PathBuf, String) -> E,
) -> Result<libloading::Library, E> {
    // SAFETY: Loading third-party plugin code is inherently unsafe. The loaded library is kept
    // alive by the caller for as long as any loaded symbols are used.
    unsafe {
        libloading::Library::new(library_path)
            .map_err(|error| load_error(library_path.to_path_buf(), error.to_string()))
    }
}

pub(super) fn load_runtime_entry(
    library: &libloading::Library,
    library_path: &Path,
) -> Result<NonNull<clap_sys::entry::clap_plugin_entry>, ClapRuntimeError> {
    // SAFETY: The symbol name is the CLAP ABI entry point and the pointer is checked for null.
    unsafe {
        let symbol = library
            .get::<*const clap_sys::entry::clap_plugin_entry>(b"clap_entry\0")
            .map_err(|_error| ClapRuntimeError::MissingEntry(library_path.to_path_buf()))?;
        NonNull::new(*symbol as *mut clap_sys::entry::clap_plugin_entry)
            .ok_or_else(|| ClapRuntimeError::MissingEntry(library_path.to_path_buf()))
    }
}

pub(super) fn validate_runtime_entry(
    plugin_path: &Path,
    entry: NonNull<clap_sys::entry::clap_plugin_entry>,
) -> Result<(), ClapRuntimeError> {
    // SAFETY: `entry` points into the loaded CLAP library and stays valid while `library` lives.
    let entry_ref = unsafe { entry.as_ref() };
    if !clap_version_is_compatible(entry_ref.clap_version) {
        return Err(ClapRuntimeError::IncompatibleVersion);
    }
    let init = entry_ref
        .init
        .ok_or(ClapRuntimeError::MissingFunction("init"))?;
    let path_c = CString::new(plugin_path.display().to_string())
        .map_err(|_error| ClapRuntimeError::InvalidPluginId(plugin_path.display().to_string()))?;
    // SAFETY: CLAP entry function pointer and NUL-terminated path come from validated data.
    if unsafe { !init(path_c.as_ptr()) } {
        return Err(ClapRuntimeError::InitFailed);
    }
    Ok(())
}

pub(super) fn plugin_metadata(plugin_id: &str) -> Result<ClapPluginMetadata, ClapRuntimeError> {
    metadata_store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(plugin_id)
        .cloned()
        .or_else(|| {
            let key = RuntimeKey::parse(plugin_id)?;
            Some(ClapPluginMetadata {
                processor_id: plugin_id.to_string(),
                clap_id: key.clap_id,
                name: plugin_id.to_string(),
                vendor: None,
                version: None,
                features: Vec::new(),
                role: registry::Role::Effect,
                path: key.path.clone(),
                library_path: resolve_clap_library_path(&key.path),
            })
        })
        .ok_or_else(|| ClapRuntimeError::UnknownPlugin(plugin_id.to_string()))
}

pub(super) struct HostContext {
    pub(super) name: CString,
    pub(super) vendor: CString,
    pub(super) url: CString,
    pub(super) version: CString,
    pub(super) host: clap_host,
    pub(super) state: NonNull<HostState>,
}

pub(super) struct HostState {
    pub(super) requested_gui_size: Mutex<Option<EditorSize>>,
    pub(super) resize_handler: Mutex<Option<Arc<dyn EditorResizeHandler>>>,
    pub(super) params_flush_requested: AtomicBool,
}

impl HostContext {
    pub(super) fn new() -> Box<Self> {
        let state = Box::leak(Box::new(HostState {
            requested_gui_size: Mutex::new(None),
            resize_handler: Mutex::new(None),
            params_flush_requested: AtomicBool::new(false),
        }));
        let state = NonNull::from(state);
        let mut context = Box::new(Self {
            name: CString::new("Lilypalooza").expect("static string has no NUL"),
            vendor: CString::new("Lilypalooza").expect("static string has no NUL"),
            url: CString::new("https://github.com/ales-tsurko/lilypalooza")
                .expect("static string has no NUL"),
            version: CString::new(env!("CARGO_PKG_VERSION")).expect("static string has no NUL"),
            host: clap_host {
                clap_version: CLAP_VERSION,
                host_data: state.as_ptr().cast(),
                name: std::ptr::null(),
                vendor: std::ptr::null(),
                url: std::ptr::null(),
                version: std::ptr::null(),
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_request_process),
                request_callback: Some(host_request_callback),
            },
            state,
        });
        context.host.name = context.name.as_ptr();
        context.host.vendor = context.vendor.as_ptr();
        context.host.url = context.url.as_ptr();
        context.host.version = context.version.as_ptr();
        context
    }

    pub(super) fn as_ptr(&self) -> *const clap_host {
        &self.host
    }

    pub(super) fn take_requested_gui_size(&self) -> Option<EditorSize> {
        self.state().take_requested_gui_size()
    }

    pub(super) fn set_resize_handler(&self, handler: Option<Arc<dyn EditorResizeHandler>>) {
        self.state().set_resize_handler(handler);
    }

    pub(super) fn take_params_flush_request(&self) -> bool {
        self.state().take_params_flush_request()
    }

    pub(super) fn state(&self) -> &HostState {
        // SAFETY: `state` comes from `Box::leak` in `new` and is reclaimed only in `Drop`.
        unsafe { self.state.as_ref() }
    }
}

impl Drop for HostContext {
    fn drop(&mut self) {
        // SAFETY: `state` was allocated by `Box::leak` in `new` and is owned by this context.
        unsafe {
            drop(Box::from_raw(self.state.as_ptr()));
        }
    }
}

impl HostState {
    pub(super) fn take_requested_gui_size(&self) -> Option<EditorSize> {
        let requested = self
            .requested_gui_size
            .lock()
            .map(|mut size| size.take())
            .unwrap_or_default();
        trace_clap_editor(|| format!("host take_requested_gui_size {requested:?}"));
        requested
    }

    pub(super) fn set_requested_gui_size(&self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            trace_clap_editor(|| {
                format!("host request_resize ignored invalid width={width} height={height}")
            });
            return false;
        }
        let requested = EditorSize { width, height };
        if let Some(handler) = self.resize_handler() {
            return self.resize_gui_with_handler(handler, requested);
        }
        self.queue_requested_gui_size(requested)
    }

    pub(super) fn resize_handler(&self) -> Option<Arc<dyn EditorResizeHandler>> {
        self.resize_handler
            .lock()
            .ok()
            .and_then(|handler| handler.as_ref().cloned())
    }

    pub(super) fn resize_gui_with_handler(
        &self,
        handler: Arc<dyn EditorResizeHandler>,
        requested: EditorSize,
    ) -> bool {
        match handler.resize_editor(requested) {
            Ok(accepted) => {
                self.store_requested_gui_size(accepted);
                trace_clap_editor(|| {
                    format!(
                        "host request_resize live requested={requested:?} accepted={accepted:?}"
                    )
                });
                true
            }
            Err(error) => {
                trace_clap_editor(|| {
                    format!("host request_resize live failed requested={requested:?} error={error}")
                });
                false
            }
        }
    }

    pub(super) fn store_requested_gui_size(&self, requested: EditorSize) {
        if let Ok(mut size) = self.requested_gui_size.lock() {
            *size = Some(requested);
        }
    }

    pub(super) fn queue_requested_gui_size(&self, requested: EditorSize) -> bool {
        if let Ok(mut size) = self.requested_gui_size.lock() {
            *size = Some(requested);
            trace_clap_editor(|| format!("host request_resize queued requested={requested:?}"));
            true
        } else {
            trace_clap_editor(|| {
                format!("host request_resize failed to lock requested={requested:?}")
            });
            false
        }
    }

    pub(super) fn set_resize_handler(&self, handler: Option<Arc<dyn EditorResizeHandler>>) {
        if let Ok(mut current) = self.resize_handler.lock() {
            *current = handler;
        }
    }

    pub(super) fn request_params_flush(&self) {
        self.params_flush_requested.store(true, Ordering::Release);
    }

    pub(super) fn take_params_flush_request(&self) -> bool {
        self.params_flush_requested.swap(false, Ordering::AcqRel)
    }
}

pub(super) unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    extension_id: *const c_char,
) -> *const c_void {
    if extension_id.is_null() {
        return std::ptr::null();
    }
    // SAFETY: CLAP passes a NUL-terminated extension id pointer.
    let id = unsafe { CStr::from_ptr(extension_id) };
    host_extension_for_id(id).unwrap_or(std::ptr::null())
}
