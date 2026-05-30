use super::*;

/// Stable adapter backend format.
pub const FORMAT: &str = "au";

const AU_MUSIC_DEVICE: &str = "aumu";
const AU_MUSIC_EFFECT: &str = "aumf";
const AU_EFFECT: &str = "aufx";

/// Stable AUv2 component identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuComponentId {
    /// Audio component type four-character code.
    pub component_type: u32,
    /// Audio component subtype four-character code.
    pub component_subtype: u32,
    /// Audio component manufacturer four-character code.
    pub component_manufacturer: u32,
}

/// One AUv2 plugin discovered in a `.component` bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuPluginMetadata {
    /// Stable host id used by persisted processor slots.
    pub processor_id: String,
    /// Core Audio component id.
    pub component: AuComponentId,
    /// Display name.
    pub name: String,
    /// Optional vendor.
    pub vendor: Option<String>,
    /// Optional version.
    pub version: Option<String>,
    /// Lilypalooza registry role.
    pub role: registry::Role,
    /// Original `.component` candidate path.
    pub path: PathBuf,
}

/// Result returned by the validator process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Validated format.
    pub format: String,
    /// Candidate path.
    pub path: PathBuf,
    /// Probe outcome.
    pub result: Result<Vec<AuPluginMetadata>, String>,
}

/// AUv2 probe errors.
#[derive(Debug, thiserror::Error)]
pub enum AuProbeError {
    /// Candidate path does not look like an AU component.
    #[error("not an AU component candidate: {0}")]
    NotCandidate(String),
    /// Candidate Info.plist is missing.
    #[error("AU component is missing Info.plist: {0}")]
    MissingInfoPlist(String),
    /// Info.plist parsing failed.
    #[error("failed to read AU Info.plist {path}: {error}")]
    InvalidInfoPlist {
        /// Plist path.
        path: PathBuf,
        /// Parser error.
        error: String,
    },
    /// The plist does not declare AUv2 AudioComponents.
    #[error("AU component declares no supported AudioComponents")]
    NoAudioComponents,
    /// One AudioComponents entry is invalid.
    #[error("invalid AU AudioComponents entry {index}: {reason}")]
    InvalidAudioComponent {
        /// Entry index.
        index: usize,
        /// Validation reason.
        reason: String,
    },
}

/// Returns true when a path is an AU component candidate.
#[must_use]
pub fn is_au_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("component"))
}

/// Finds AU component candidates under one root.
pub fn candidate_paths(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut candidates = Vec::new();
    if !root.is_dir() {
        return Ok(candidates);
    }
    collect_candidate_paths(root, &mut candidates)?;
    candidates.sort();
    Ok(candidates)
}

fn collect_candidate_paths(
    path: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if is_au_candidate(&path) {
            candidates.push(path);
        } else if path.is_dir() {
            collect_candidate_paths(&path, candidates)?;
        }
    }
    Ok(())
}

/// Probes one AUv2 `.component` bundle.
pub fn probe(path: &Path) -> Result<Vec<AuPluginMetadata>, AuProbeError> {
    if !is_au_candidate(path) {
        return Err(AuProbeError::NotCandidate(path.display().to_string()));
    }
    let plist_path = path.join("Contents").join("Info.plist");
    if !plist_path.exists() {
        return Err(AuProbeError::MissingInfoPlist(path.display().to_string()));
    }
    let value =
        plist::Value::from_file(&plist_path).map_err(|error| AuProbeError::InvalidInfoPlist {
            path: plist_path,
            error: error.to_string(),
        })?;
    metadata_from_plist(path, &value)
}

fn metadata_from_plist(
    path: &Path,
    value: &plist::Value,
) -> Result<Vec<AuPluginMetadata>, AuProbeError> {
    let Some(root) = value.as_dictionary() else {
        return Err(AuProbeError::NoAudioComponents);
    };
    let bundle_name = plist_string(root, "CFBundleName");
    let bundle_version = plist_string(root, "CFBundleShortVersionString")
        .or_else(|| plist_string(root, "CFBundleVersion"));
    let components = root
        .get("AudioComponents")
        .and_then(plist::Value::as_array)
        .ok_or(AuProbeError::NoAudioComponents)?;
    let mut plugins = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if let Some(metadata) = metadata_from_component(
            path,
            component,
            index,
            bundle_name.as_deref(),
            bundle_version.as_deref(),
        )? {
            plugins.push(metadata);
        }
    }
    if plugins.is_empty() {
        return Err(AuProbeError::NoAudioComponents);
    }
    Ok(plugins)
}

fn metadata_from_component(
    path: &Path,
    value: &plist::Value,
    index: usize,
    bundle_name: Option<&str>,
    bundle_version: Option<&str>,
) -> Result<Option<AuPluginMetadata>, AuProbeError> {
    let Some(component) = value.as_dictionary() else {
        return Err(invalid_component(index, "entry is not a dictionary"));
    };
    let Some(component_type) = code_field(component, "type", index)? else {
        return Ok(None);
    };
    let Some(role) = role_for_component_type(component_type) else {
        return Ok(None);
    };
    let component_subtype = required_code_field(component, "subtype", index)?;
    let component_manufacturer = required_code_field(component, "manufacturer", index)?;
    let component_id = AuComponentId {
        component_type,
        component_subtype,
        component_manufacturer,
    };
    let component_name = plist_string(component, "name");
    let name = component_name
        .clone()
        .or_else(|| plist_string(component, "description"))
        .or_else(|| bundle_name.map(str::to_string))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("AU")
                .to_string()
        });
    Ok(Some(AuPluginMetadata {
        processor_id: stable_processor_id(path, component_id),
        component: component_id,
        vendor: component_vendor(component, component_name.as_deref()),
        version: plist_string(component, "version").or_else(|| bundle_version.map(str::to_string)),
        role,
        name,
        path: path.to_path_buf(),
    }))
}

fn component_vendor(component: &plist::Dictionary, component_name: Option<&str>) -> Option<String> {
    plist_string(component, "manufacturerName").or_else(|| vendor_prefix(component_name))
}

fn vendor_prefix(name: Option<&str>) -> Option<String> {
    name?
        .split_once(':')
        .map(|(vendor, _name)| vendor.trim())
        .filter(|vendor| !vendor.is_empty())
        .map(str::to_string)
}

fn role_for_component_type(component_type: u32) -> Option<registry::Role> {
    match fourcc_to_string(component_type).as_deref() {
        Some(AU_MUSIC_DEVICE) => Some(registry::Role::Instrument),
        Some(AU_MUSIC_EFFECT | AU_EFFECT) => Some(registry::Role::Effect),
        _ => None,
    }
}

fn required_code_field(
    component: &plist::Dictionary,
    key: &str,
    index: usize,
) -> Result<u32, AuProbeError> {
    code_field(component, key, index)?
        .ok_or_else(|| invalid_component(index, format!("missing `{key}`")))
}

fn code_field(
    component: &plist::Dictionary,
    key: &str,
    index: usize,
) -> Result<Option<u32>, AuProbeError> {
    let Some(value) = component.get(key) else {
        return Ok(None);
    };
    match value {
        plist::Value::String(value) => fourcc(value)
            .ok_or_else(|| invalid_component(index, format!("invalid `{key}` fourcc `{value}`")))
            .map(Some),
        plist::Value::Integer(value) => value
            .as_unsigned()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid_component(index, format!("invalid `{key}` integer")))
            .map(Some),
        _ => Err(invalid_component(index, format!("invalid `{key}` value"))),
    }
}

fn plist_string(component: &plist::Dictionary, key: &str) -> Option<String> {
    component
        .get(key)
        .and_then(plist::Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn invalid_component(index: usize, reason: impl Into<String>) -> AuProbeError {
    AuProbeError::InvalidAudioComponent {
        index,
        reason: reason.into(),
    }
}

fn fourcc(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn fourcc_to_string(value: u32) -> Option<String> {
    String::from_utf8(value.to_be_bytes().to_vec()).ok()
}

/// Builds the stable global processor id for one AU component.
#[must_use]
pub fn stable_processor_id(path: &Path, component: AuComponentId) -> String {
    format!(
        "{FORMAT}:{}#{}-{}-{}",
        path.display(),
        fourcc_to_string(component.component_type)
            .unwrap_or_else(|| component.component_type.to_string()),
        fourcc_to_string(component.component_subtype)
            .unwrap_or_else(|| component.component_subtype.to_string()),
        fourcc_to_string(component.component_manufacturer)
            .unwrap_or_else(|| component.component_manufacturer.to_string())
    )
}

#[cfg(test)]
pub(crate) fn test_fourcc(value: &str) -> u32 {
    fourcc(value).expect("test fourcc should be valid")
}
