use super::{editor::trace_vst3_editor, host_com::*, *};

/// VST3 plugin format id used by the validator.
pub const FORMAT: &str = "vst3";

pub(super) const AUDIO_MODULE_CLASS: &str = "Audio Module Class";
pub(super) const EDITOR_VIEW_NAME: &[u8] = b"editor\0";
pub(super) const DEFAULT_VST3_EDITOR_DESCRIPTOR: lilypalooza_audio::EditorDescriptor =
    lilypalooza_audio::EditorDescriptor {
        default_size: EditorSize {
            width: 640,
            height: 480,
        },
        min_size: None,
        resizable: true,
    };

/// Validated VST3 plugin metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vst3PluginMetadata {
    /// Stable processor id used by Lilypalooza.
    pub processor_id: String,
    /// VST3 processor class id as lowercase hex.
    pub class_id: String,
    /// User-visible plugin name.
    pub name: String,
    /// User-visible vendor, when reported by the plugin.
    pub vendor: Option<String>,
    /// User-visible version, when reported by the plugin.
    pub version: Option<String>,
    /// Raw VST3 subcategory string.
    pub category: Option<String>,
    /// Mixer processor role inferred from VST3 subcategories.
    pub role: registry::Role,
    /// Candidate path, usually the `.vst3` bundle or file.
    pub path: PathBuf,
    /// Native dynamic library path inside the candidate.
    pub library_path: PathBuf,
}

/// Structured validation report emitted by the validator helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Plugin format.
    pub format: String,
    /// Candidate path.
    pub path: PathBuf,
    /// Validation result.
    pub result: Result<Vec<Vst3PluginMetadata>, String>,
}

/// VST3 probing error.
#[derive(thiserror::Error, Debug)]
pub enum Vst3ProbeError {
    /// Candidate is not a VST3 file or bundle.
    #[error("not a VST3 candidate: {0}")]
    NotCandidate(PathBuf),
    /// VST3 binary path could not be resolved.
    #[error("VST3 binary not found for {0}")]
    MissingLibrary(PathBuf),
    /// Dynamic library could not be loaded.
    #[error("failed to load VST3 library {path}: {error}")]
    LoadLibrary {
        /// Library path.
        path: PathBuf,
        /// Loader error.
        error: libloading::Error,
    },
    /// macOS bundle object could not be created.
    #[error("failed to create VST3 bundle for {0}")]
    MissingBundle(PathBuf),
    /// macOS bundle executable could not be loaded.
    #[error("failed to load VST3 bundle executable for {0}")]
    BundleLoadFailed(PathBuf),
    /// Required exported symbol is missing.
    #[error("missing VST3 export `{0}`")]
    MissingExport(&'static str),
    /// Plugin factory export returned null.
    #[error("VST3 factory export returned null")]
    MissingFactory,
    /// Factory exposed no VST3 audio processor classes.
    #[error("no VST3 audio processor classes found")]
    NoPluginClasses,
}

#[derive(thiserror::Error, Debug)]
pub(super) enum Vst3RuntimeError {
    #[error("VST3 plugin `{0}` is not registered")]
    MissingMetadata(String),
    #[error("invalid VST3 class id `{0}`")]
    InvalidClassId(String),
    #[error("VST3 factory failed to create processor")]
    CreateProcessorFailed,
    #[error("VST3 processor does not implement IAudioProcessor")]
    MissingAudioProcessor,
    #[error("VST3 processor initialize failed")]
    InitializeFailed,
    #[error("VST3 processor setup failed")]
    SetupFailed,
    #[error("VST3 processor activation failed")]
    ActivateFailed,
    #[error(transparent)]
    Probe(#[from] Vst3ProbeError),
}

/// Returns whether `path` looks like a VST3 candidate.
#[must_use]
pub fn is_vst3_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vst3"))
}

/// Recursively returns VST3 candidates under `root`.
pub fn candidate_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_candidate_paths(root, &mut out)?;
    out.sort();
    Ok(out)
}

pub(super) fn collect_candidate_paths(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if is_vst3_candidate(path) {
        out.push(path.to_path_buf());
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        collect_candidate_paths(&entry?.path(), out)?;
    }
    Ok(())
}

/// Resolves the dynamic library path for a VST3 candidate.
#[must_use]
pub fn resolve_vst3_library_path(path: &Path) -> PathBuf {
    if path.is_file() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let candidates = platform_vst3_binary_candidates(path, stem);
    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

pub(super) fn platform_vst3_binary_candidates(path: &Path, stem: &str) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![path.join("Contents").join("MacOS").join(stem)]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            path.join("Contents")
                .join("x86_64-win")
                .join(format!("{stem}.vst3")),
            path.join(format!("{stem}.vst3")),
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            path.join("Contents")
                .join("x86_64-linux")
                .join(format!("{stem}.so")),
            path.join(format!("{stem}.so")),
        ]
    }
}

/// Builds a stable Lilypalooza processor id from a VST3 path and class id.
#[must_use]
pub fn stable_processor_id(path: &Path, class_id: &str) -> String {
    format!("vst3:{}#{class_id}", path.display())
}

/// Probes a VST3 candidate in-process.
pub fn probe(path: &Path) -> Result<Vec<Vst3PluginMetadata>, Vst3ProbeError> {
    if !is_vst3_candidate(path) {
        return Err(Vst3ProbeError::NotCandidate(path.to_path_buf()));
    }
    let library_path = resolve_vst3_library_path(path);
    if !library_path.exists() {
        return Err(Vst3ProbeError::MissingLibrary(path.to_path_buf()));
    }
    let module = load_module(path, &library_path)?;
    let plugins = vst3_factory_plugins(&module.factory, path, &library_path);
    if plugins.is_empty() {
        Err(Vst3ProbeError::NoPluginClasses)
    } else {
        Ok(plugins)
    }
}

pub(super) fn vst3_factory_plugins(
    factory: &ComPtr<IPluginFactory>,
    path: &Path,
    library_path: &Path,
) -> Vec<Vst3PluginMetadata> {
    // SAFETY: `factory` is a live COM factory kept alive by `LoadedModule`.
    let count = unsafe { factory.countClasses() }.max(0);
    let factory_vendor = vst3_factory_vendor(factory);
    (0..count)
        .filter_map(|index| {
            vst3_class_metadata(
                factory,
                index,
                path,
                library_path,
                factory_vendor.as_deref(),
            )
        })
        .collect()
}

pub(super) fn vst3_factory_vendor(factory: &ComPtr<IPluginFactory>) -> Option<String> {
    let mut info = zeroed::<PFactoryInfo>();
    // SAFETY: Factory fills the provided `PFactoryInfo`.
    if unsafe { factory.getFactoryInfo(&mut info) } != kResultOk {
        return None;
    }
    non_empty_string(c_char_array_to_string(&info.vendor))
}

pub(super) fn vst3_class_metadata(
    factory: &ComPtr<IPluginFactory>,
    index: i32,
    path: &Path,
    library_path: &Path,
    factory_vendor: Option<&str>,
) -> Option<Vst3PluginMetadata> {
    if let Some(metadata) =
        vst3_class_metadata_from_factory2(factory, index, path, library_path, factory_vendor)
    {
        return Some(metadata);
    }

    let mut info = zeroed::<PClassInfo>();
    // SAFETY: Factory fills the provided `PClassInfo` for a valid class index.
    if unsafe { factory.getClassInfo(index, &mut info) } != kResultOk
        || c_char_array_to_string(&info.category) != AUDIO_MODULE_CLASS
    {
        return None;
    }
    Some(metadata_from_class_info(
        info,
        path,
        library_path,
        factory_vendor,
    ))
}

pub(super) fn vst3_class_metadata_from_factory2(
    factory: &ComPtr<IPluginFactory>,
    index: i32,
    path: &Path,
    library_path: &Path,
    factory_vendor: Option<&str>,
) -> Option<Vst3PluginMetadata> {
    let factory2 = factory.cast::<IPluginFactory2>()?;
    let mut info = zeroed::<PClassInfo2>();
    // SAFETY: Factory fills the provided `PClassInfo2` for a valid class index.
    if unsafe { factory2.getClassInfo2(index, &mut info) } != kResultOk {
        return None;
    }
    (c_char_array_to_string(&info.category) == AUDIO_MODULE_CLASS)
        .then(|| metadata_from_class_info2(info, path, library_path, factory_vendor))
}

pub(super) fn metadata_from_class_info2(
    info: PClassInfo2,
    path: &Path,
    library_path: &Path,
    factory_vendor: Option<&str>,
) -> Vst3PluginMetadata {
    let category = non_empty_string(c_char_array_to_string(&info.subCategories));
    metadata_from_class_parts(ClassMetadataParts::class_info2(
        info,
        path,
        library_path,
        factory_vendor,
        category,
    ))
}

pub(super) fn metadata_from_class_info(
    info: PClassInfo,
    path: &Path,
    library_path: &Path,
    factory_vendor: Option<&str>,
) -> Vst3PluginMetadata {
    metadata_from_class_parts(ClassMetadataParts::class_info(
        info,
        path,
        library_path,
        factory_vendor,
    ))
}

struct ClassMetadataParts<'a> {
    path: &'a Path,
    library_path: &'a Path,
    class_id: String,
    name: String,
    vendor: Option<String>,
    version: Option<String>,
    role: registry::Role,
    category: Option<String>,
}

impl<'a> ClassMetadataParts<'a> {
    fn class_info2(
        info: PClassInfo2,
        path: &'a Path,
        library_path: &'a Path,
        factory_vendor: Option<&str>,
        category: Option<String>,
    ) -> Self {
        Self {
            path,
            library_path,
            class_id: tuid_to_hex(&info.cid),
            name: c_char_array_to_string(&info.name),
            vendor: metadata_vendor(
                non_empty_string(c_char_array_to_string(&info.vendor)),
                factory_vendor,
            ),
            version: non_empty_string(c_char_array_to_string(&info.version)),
            role: role_from_subcategories(category.as_deref()),
            category,
        }
    }

    fn class_info(
        info: PClassInfo,
        path: &'a Path,
        library_path: &'a Path,
        factory_vendor: Option<&str>,
    ) -> Self {
        Self {
            path,
            library_path,
            class_id: tuid_to_hex(&info.cid),
            name: c_char_array_to_string(&info.name),
            vendor: metadata_vendor(None, factory_vendor),
            version: None,
            role: registry::Role::Effect,
            category: None,
        }
    }
}

fn metadata_from_class_parts(parts: ClassMetadataParts<'_>) -> Vst3PluginMetadata {
    Vst3PluginMetadata {
        processor_id: stable_processor_id(parts.path, &parts.class_id),
        class_id: parts.class_id,
        name: parts.name,
        vendor: parts.vendor,
        version: parts.version,
        role: parts.role,
        category: parts.category,
        path: parts.path.to_path_buf(),
        library_path: parts.library_path.to_path_buf(),
    }
}

pub(super) fn metadata_vendor(
    class_vendor: Option<String>,
    factory_vendor: Option<&str>,
) -> Option<String> {
    class_vendor.or_else(|| factory_vendor.map(str::to_owned))
}

pub(super) fn role_from_subcategories(category: Option<&str>) -> registry::Role {
    let category = category.unwrap_or_default().to_ascii_lowercase();
    if category.contains("instrument")
        || category.contains("synth")
        || category.contains("sampler")
        || category.contains("drum")
    {
        registry::Role::Instrument
    } else {
        registry::Role::Effect
    }
}

pub(super) fn non_empty_string(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

pub(super) fn c_char_array_to_string(bytes: &[c_char]) -> String {
    let len = bytes
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(bytes.len());
    let bytes = bytes
        .get(..len)
        .unwrap_or(bytes)
        .iter()
        .map(|value| value.cast_unsigned())
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

pub(super) fn copy_tchar_string(src: &str, dst: &mut [TChar]) {
    let mut len = 0;
    for (src, dst) in src.encode_utf16().zip(dst.iter_mut()) {
        *dst = src as TChar;
        len += 1;
    }
    if len < dst.len() {
        if let Some(dst) = dst.get_mut(len) {
            *dst = 0;
        }
    } else if let Some(last) = dst.last_mut() {
        *last = 0;
    }
}

pub(super) fn tuid_to_hex(tuid: &TUID) -> String {
    tuid.iter()
        .map(|byte| format!("{:02x}", byte.cast_unsigned()))
        .collect()
}

pub(super) fn hex_to_tuid(hex: &str) -> Option<TUID> {
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0 as c_char; 16];
    for (slot, chunk) in out.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let chunk = std::str::from_utf8(chunk).ok()?;
        let byte = u8::from_str_radix(chunk, 16).ok()?;
        *slot = byte.cast_signed();
    }
    Some(out)
}

pub(super) fn zeroed<T>() -> T {
    // SAFETY: VST3 POD structs are C structs where zero-initialization is valid before filling.
    unsafe { std::mem::zeroed() }
}

pub(super) struct LoadedModule {
    pub(super) factory: ComPtr<IPluginFactory>,
    #[cfg(target_os = "macos")]
    pub(super) _bundle: core_foundation::bundle::CFBundle,
    #[cfg(not(target_os = "macos"))]
    pub(super) _library: libloading::Library,
}

// SAFETY: The library and COM factory remain loaded for process lifetime through `LOADED_MODULES`.
unsafe impl Send for LoadedModule {}
// SAFETY: Access to plugin instances created from the module is synchronized by runtime mutexes.
unsafe impl Sync for LoadedModule {}

type GetPluginFactory = unsafe extern "system" fn() -> *mut IPluginFactory;
#[cfg(target_os = "macos")]
type BundleEntry = unsafe extern "system" fn(*mut c_void) -> bool;
#[cfg(target_os = "windows")]
type InitDll = unsafe extern "system" fn() -> bool;
#[cfg(all(unix, not(target_os = "macos")))]
type ModuleEntry = unsafe extern "system" fn(*mut c_void) -> bool;

pub(super) const GET_PLUGIN_FACTORY_SYMBOL: &str = "GetPluginFactory";
#[cfg(target_os = "macos")]
pub(super) const BUNDLE_ENTRY_SYMBOL: &str = "bundleEntry";

pub(super) fn loaded_modules() -> &'static RwLock<HashMap<PathBuf, Arc<LoadedModule>>> {
    static LOADED_MODULES: OnceLock<RwLock<HashMap<PathBuf, Arc<LoadedModule>>>> = OnceLock::new();
    LOADED_MODULES.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn load_module(
    path: &Path,
    library_path: &Path,
) -> Result<Arc<LoadedModule>, Vst3ProbeError> {
    if let Some(module) = loaded_modules()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(library_path)
        .cloned()
    {
        return Ok(module);
    }
    let module = Arc::new(load_module_uncached(path, library_path)?);
    loaded_modules()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(library_path.to_path_buf(), module.clone());
    Ok(module)
}

pub(super) fn load_module_uncached(
    path: &Path,
    _library_path: &Path,
) -> Result<LoadedModule, Vst3ProbeError> {
    #[cfg(target_os = "macos")]
    {
        load_macos_module(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        load_dynamic_module(path, _library_path)
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn load_dynamic_module(
    _path: &Path,
    library_path: &Path,
) -> Result<LoadedModule, Vst3ProbeError> {
    let library = load_vst3_dynamic_library(library_path)?;
    initialize_dynamic_vst3_module(&library)?;
    let factory = dynamic_vst3_factory(&library)?;
    Ok(LoadedModule {
        factory,
        _library: library,
    })
}

#[cfg(not(target_os = "macos"))]
pub(super) fn load_vst3_dynamic_library(
    library_path: &Path,
) -> Result<libloading::Library, Vst3ProbeError> {
    // SAFETY: Loading a plugin library is isolated by the validator subprocess during scanning.
    unsafe { libloading::Library::new(library_path) }.map_err(|error| Vst3ProbeError::LoadLibrary {
        path: library_path.to_path_buf(),
        error,
    })
}

#[cfg(not(target_os = "macos"))]
pub(super) fn initialize_dynamic_vst3_module(
    library: &libloading::Library,
) -> Result<(), Vst3ProbeError> {
    #[cfg(target_os = "windows")]
    call_windows_init(library)?;
    #[cfg(all(unix, not(target_os = "macos")))]
    call_module_entry(library)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn dynamic_vst3_factory(
    library: &libloading::Library,
) -> Result<ComPtr<IPluginFactory>, Vst3ProbeError> {
    // SAFETY: Symbol name is NUL-terminated and expected by VST3.
    let get_factory = unsafe { library.get::<GetPluginFactory>(b"GetPluginFactory\0") }
        .map_err(|_| Vst3ProbeError::MissingExport("GetPluginFactory"))?;
    // SAFETY: Factory export is called after module initialization.
    let factory = unsafe { get_factory() };
    // SAFETY: `GetPluginFactory` returns an owning COM pointer when non-null.
    unsafe { ComPtr::from_raw(factory) }.ok_or(Vst3ProbeError::MissingFactory)
}

#[cfg(target_os = "macos")]
pub(super) fn load_macos_module(path: &Path) -> Result<LoadedModule, Vst3ProbeError> {
    let bundle = macos_vst3_bundle(path)?;
    load_macos_vst3_executable(path, &bundle)?;
    call_macos_bundle_entry(&bundle)?;
    let factory = macos_vst3_factory(&bundle)?;
    Ok(LoadedModule {
        factory,
        _bundle: bundle,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn macos_vst3_bundle(
    path: &Path,
) -> Result<core_foundation::bundle::CFBundle, Vst3ProbeError> {
    use core_foundation::{
        bundle::CFBundle,
        string::CFString,
        url::{CFURL, kCFURLPOSIXPathStyle},
    };

    CFBundle::new(CFURL::from_file_system_path(
        CFString::new(&path.display().to_string()),
        kCFURLPOSIXPathStyle,
        true,
    ))
    .ok_or_else(|| Vst3ProbeError::MissingBundle(path.to_path_buf()))
}

#[cfg(target_os = "macos")]
pub(super) fn load_macos_vst3_executable(
    path: &Path,
    bundle: &core_foundation::bundle::CFBundle,
) -> Result<(), Vst3ProbeError> {
    use core_foundation::{base::TCFType, bundle::CFBundleLoadExecutable};

    // SAFETY: VST3 macOS modules must be loaded via CFBundleLoadExecutable before bundleEntry.
    if unsafe { CFBundleLoadExecutable(bundle.as_concrete_TypeRef()) } == 0 {
        return Err(Vst3ProbeError::BundleLoadFailed(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn call_macos_bundle_entry(
    bundle: &core_foundation::bundle::CFBundle,
) -> Result<(), Vst3ProbeError> {
    use core_foundation::base::TCFType;

    let entry = macos_bundle_symbol::<BundleEntry>(bundle, BUNDLE_ENTRY_SYMBOL)
        .ok_or(Vst3ProbeError::MissingExport(BUNDLE_ENTRY_SYMBOL))?;
    let bundle_ref = bundle.as_concrete_TypeRef() as *mut c_void;
    // SAFETY: bundleEntry is the required VST3 macOS module initializer.
    if !unsafe { entry(bundle_ref) } {
        return Err(Vst3ProbeError::MissingExport(BUNDLE_ENTRY_SYMBOL));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn macos_vst3_factory(
    bundle: &core_foundation::bundle::CFBundle,
) -> Result<ComPtr<IPluginFactory>, Vst3ProbeError> {
    let get_factory = macos_bundle_symbol::<GetPluginFactory>(bundle, GET_PLUGIN_FACTORY_SYMBOL)
        .ok_or(Vst3ProbeError::MissingExport(GET_PLUGIN_FACTORY_SYMBOL))?;
    // SAFETY: Factory export is called after module initialization.
    let factory = unsafe { get_factory() };
    // SAFETY: `GetPluginFactory` returns an owning COM pointer when non-null.
    unsafe { ComPtr::from_raw(factory) }.ok_or(Vst3ProbeError::MissingFactory)
}

#[cfg(target_os = "macos")]
pub(super) fn macos_bundle_symbol<T>(
    bundle: &core_foundation::bundle::CFBundle,
    symbol: &str,
) -> Option<T>
where
    T: Copy,
{
    use core_foundation::string::CFString;

    let pointer = bundle.function_pointer_for_name(CFString::new(symbol));
    if pointer.is_null() {
        return None;
    }
    // SAFETY: The caller chooses `T` to match the named C symbol signature.
    Some(unsafe { std::mem::transmute_copy::<*const c_void, T>(&pointer) })
}

#[cfg(target_os = "windows")]
pub(super) fn call_windows_init(library: &libloading::Library) -> Result<(), Vst3ProbeError> {
    // SAFETY: Symbol lookup uses a static NUL-terminated export name.
    let entry = unsafe { library.get::<InitDll>(b"InitDll\0") };
    if let Ok(entry) = entry {
        // SAFETY: InitDll is the optional VST3 module initializer on Windows.
        if !unsafe { entry() } {
            return Err(Vst3ProbeError::MissingExport("InitDll"));
        }
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn call_module_entry(library: &libloading::Library) -> Result<(), Vst3ProbeError> {
    if let Ok(entry) = unsafe { library.get::<ModuleEntry>(b"ModuleEntry\0") } {
        if !unsafe { entry(std::ptr::null_mut()) } {
            return Err(Vst3ProbeError::MissingExport("ModuleEntry"));
        }
    }
    Ok(())
}

pub(super) fn metadata_store() -> &'static RwLock<HashMap<String, Vst3PluginMetadata>> {
    static PLUGIN_METADATA: OnceLock<RwLock<HashMap<String, Vst3PluginMetadata>>> = OnceLock::new();
    PLUGIN_METADATA.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn plugin_metadata(id: &str) -> Result<Vst3PluginMetadata, Vst3RuntimeError> {
    metadata_store()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(id)
        .cloned()
        .ok_or_else(|| Vst3RuntimeError::MissingMetadata(id.to_string()))
}

pub(super) struct Vst3Host {
    pub(super) requested_size: Mutex<Option<EditorSize>>,
    pub(super) resize_handler: Mutex<Option<Arc<dyn EditorResizeHandler>>>,
}

impl Vst3Host {
    pub(super) fn new() -> Self {
        Self {
            requested_size: Mutex::new(None),
            resize_handler: Mutex::new(None),
        }
    }

    pub(super) fn take_requested_size(&self) -> Option<EditorSize> {
        let requested = self
            .requested_size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        trace_vst3_editor(|| format!("host take_requested_size {requested:?}"));
        requested
    }

    pub(super) fn set_resize_handler(&self, handler: Option<Arc<dyn EditorResizeHandler>>) {
        *self
            .resize_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = handler;
    }

    pub(super) fn resize_handler(&self) -> Option<Arc<dyn EditorResizeHandler>> {
        self.resize_handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(super) fn store_requested_size(&self, size: EditorSize) {
        *self
            .requested_size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(size);
    }
}

impl Class for Vst3Host {
    type Interfaces = (IHostApplication, IPlugFrame, IComponentHandler);
}

impl IHostApplicationTrait for Vst3Host {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        // SAFETY: VST3 provides a writable String128 pointer or null.
        if let Some(name) = unsafe { name.as_mut() } {
            copy_tchar_string("Lilypalooza", name);
            kResultOk
        } else {
            kInvalidArgument
        }
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if obj.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: `obj` is a writable out pointer provided by the plugin.
        unsafe {
            *obj = std::ptr::null_mut();
        }
        // SAFETY: VST3 passes TUID pointers for `cid`/`iid`.
        if unsafe { tuid_ptr_eq(cid, &IMessage_iid) } {
            // SAFETY: `iid` and `obj` are VST3 out-parameters for this factory call.
            return unsafe { create_host_message(iid, obj) };
        }
        // SAFETY: VST3 passes TUID pointers for `cid`/`iid`.
        if unsafe { tuid_ptr_eq(cid, &IAttributeList_iid) } {
            // SAFETY: `iid` and `obj` are VST3 out-parameters for this factory call.
            return unsafe { create_host_attribute_list(iid, obj) };
        }
        kNotImplemented
    }
}

unsafe fn create_host_message(iid: *mut TUID, obj: *mut *mut c_void) -> tresult {
    // SAFETY: `iid` and `obj` are plugin-provided VST3 out-parameters for this factory call.
    unsafe {
        create_host_com_object(
            iid,
            obj,
            &IMessage_iid,
            ComWrapper::new(Vst3Message::new()).to_com_ptr::<IMessage>(),
        )
    }
}

unsafe fn create_host_attribute_list(iid: *mut TUID, obj: *mut *mut c_void) -> tresult {
    // SAFETY: `iid` and `obj` are plugin-provided VST3 out-parameters for this factory call.
    unsafe {
        create_host_com_object(
            iid,
            obj,
            &IAttributeList_iid,
            ComWrapper::new(Vst3AttributeList::default()).to_com_ptr::<IAttributeList>(),
        )
    }
}

unsafe fn create_host_com_object<T: vst3::Interface>(
    iid: *mut TUID,
    obj: *mut *mut c_void,
    interface_iid: &TUID,
    instance: Option<ComPtr<T>>,
) -> tresult {
    // SAFETY: `iid` is supplied by VST3 and points to a TUID when non-null.
    let is_interface = unsafe { tuid_ptr_eq(iid, interface_iid) };
    // SAFETY: `iid` is supplied by VST3 and points to a TUID when non-null.
    let is_unknown = unsafe { tuid_ptr_eq(iid, &FUnknown_iid) };
    if !is_interface && !is_unknown {
        return kNoInterface;
    }
    let Some(instance) = instance else {
        return kNoInterface;
    };
    // SAFETY: `obj` was checked by the caller and receives ownership of the COM pointer.
    unsafe {
        *obj = instance.into_raw().cast();
    }
    kResultOk
}

unsafe fn tuid_ptr_eq(value: *mut TUID, expected: &TUID) -> bool {
    // SAFETY: Caller guarantees `value` is either null or a valid TUID pointer.
    unsafe { value.as_ref().is_some_and(|value| value == expected) }
}

#[derive(Default)]
pub(super) struct Vst3AttributeList {
    pub(super) values: Mutex<HashMap<String, Vst3AttributeValue>>,
}

pub(super) enum Vst3AttributeValue {
    Int(i64),
    Float(f64),
    String(Vec<TChar>),
    Binary(Vec<u8>),
}
