//! Reusable background plugin scanner.

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::SystemTime,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Maximum number of concurrent validator subprocesses.
pub const PLUGIN_VALIDATOR_CONCURRENCY: usize = 1;
/// Maximum scanner events the app should process in one UI update.
pub const PLUGIN_SCAN_UI_EVENT_BUDGET: usize = 16;

/// Plugin binary format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginFormat {
    /// CLAP plugin.
    Clap,
    /// VST3 plugin.
    Vst3,
}

/// One plugin search root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginSearchPath {
    /// Plugin format to search for.
    pub format: PluginFormat,
    /// Filesystem search root.
    pub path: PathBuf,
    /// Whether this root participates in scans.
    pub enabled: bool,
}

impl Default for PluginSearchPath {
    fn default() -> Self {
        Self {
            format: PluginFormat::Clap,
            path: PathBuf::new(),
            enabled: true,
        }
    }
}

/// Background scanner event.
#[derive(Debug)]
pub enum PluginScanEvent {
    /// Human-readable progress line.
    Log(String),
    /// Validated CLAP plugin metadata.
    ClapPlugins(Vec<lilypalooza_clap::ClapPluginMetadata>),
    /// Validated VST3 plugin metadata.
    Vst3Plugins(Vec<lilypalooza_vst3::Vst3PluginMetadata>),
    /// Scan completion.
    Finished {
        /// Scan summary.
        summary: PluginScanSummary,
        /// Updated cache.
        cache: PluginScanCache,
    },
}

/// Scan counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PluginScanSummary {
    /// Candidate binaries seen.
    pub candidates: usize,
    /// Valid plugin descriptors found.
    pub valid_plugins: usize,
    /// Invalid candidates.
    pub invalid_candidates: usize,
}

/// Background plugin scan state.
#[derive(Debug, Default)]
pub struct PluginScanState {
    receiver: Option<mpsc::Receiver<PluginScanEvent>>,
    active: bool,
}

impl PluginScanState {
    /// Starts a background scan.
    pub fn start(
        &mut self,
        search_paths: Vec<PluginSearchPath>,
        cache: PluginScanCache,
        validator: PathBuf,
    ) {
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.active = true;
        thread::spawn(move || scan_worker(search_paths, cache, validator, sender));
    }

    /// Returns whether a scan is currently running.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Drains pending scan events.
    pub fn drain_events(&mut self) -> Vec<PluginScanEvent> {
        self.drain_events_with_limit(usize::MAX)
    }

    /// Drains up to `limit` pending scan events.
    pub fn drain_events_with_limit(&mut self, limit: usize) -> Vec<PluginScanEvent> {
        let mut events = Vec::new();
        if limit == 0 {
            return events;
        }
        let Some(receiver) = &self.receiver else {
            return events;
        };
        for _ in 0..limit {
            match receiver.try_recv() {
                Ok(event) => {
                    if matches!(event, PluginScanEvent::Finished { .. }) {
                        self.active = false;
                    }
                    events.push(event);
                    if !self.active {
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.active = false;
                    break;
                }
            }
        }
        if !self.active {
            self.receiver = None;
        }
        events
    }
}

/// Persistent scan cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginScanCache {
    entries: HashMap<PathBuf, CachedPluginCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedPluginCandidate {
    fingerprint: PluginCandidateFingerprint,
    #[serde(default)]
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    valid: bool,
    #[serde(default)]
    clap_plugins: Vec<lilypalooza_clap::ClapPluginMetadata>,
    #[serde(default)]
    vst3_plugins: Vec<lilypalooza_vst3::Vst3PluginMetadata>,
}

impl PluginScanCache {
    /// Loads a scan cache from `path`.
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(contents) => ron::from_str(&contents).unwrap_or_default(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves a scan cache to `path`.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let Some(parent) = path.parent() else {
            return Err(format!(
                "Plugin cache path has no parent: {}",
                path.display()
            ));
        };
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create plugin cache directory {}: {error}",
                parent.display()
            )
        })?;
        let contents = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())
            .map_err(|error| format!("Failed to serialize plugin cache: {error}"))?;
        fs::write(path, contents)
            .map_err(|error| format!("Failed to write plugin cache {}: {error}", path.display()))
    }

    /// Returns whether a candidate has changed since the cached scan.
    #[must_use]
    pub fn is_stale(&self, path: &Path, fingerprint: PluginCandidateFingerprint) -> bool {
        self.is_stale_for_validator(path, fingerprint, None)
    }

    /// Returns whether a candidate has changed since the cached scan for this validator.
    #[must_use]
    pub fn is_stale_for_validator(
        &self,
        path: &Path,
        fingerprint: PluginCandidateFingerprint,
        validator_fingerprint: Option<PluginCandidateFingerprint>,
    ) -> bool {
        self.entries.get(path).is_none_or(|entry| {
            entry.fingerprint != fingerprint || entry.validator_fingerprint != validator_fingerprint
        })
    }

    /// Stores a checked candidate.
    pub fn mark_checked(
        &mut self,
        path: PathBuf,
        fingerprint: PluginCandidateFingerprint,
        validator_fingerprint: Option<PluginCandidateFingerprint>,
        valid: bool,
        clap_plugins: Vec<lilypalooza_clap::ClapPluginMetadata>,
        vst3_plugins: Vec<lilypalooza_vst3::Vst3PluginMetadata>,
    ) {
        self.entries.insert(
            path,
            CachedPluginCandidate {
                fingerprint,
                validator_fingerprint,
                valid,
                clap_plugins,
                vst3_plugins,
            },
        );
    }

    fn cached_candidate(
        &self,
        path: &Path,
        fingerprint: PluginCandidateFingerprint,
    ) -> Option<CachedCandidateResult> {
        self.entries.get(path).and_then(|entry| {
            (entry.fingerprint == fingerprint).then(|| {
                if entry.valid && !entry.clap_plugins.is_empty() {
                    CachedCandidateResult::ValidClapPlugins(entry.clap_plugins.clone())
                } else if entry.valid && !entry.vst3_plugins.is_empty() {
                    CachedCandidateResult::ValidVst3Plugins(entry.vst3_plugins.clone())
                } else {
                    CachedCandidateResult::Invalid
                }
            })
        })
    }
}

enum CachedCandidateResult {
    ValidClapPlugins(Vec<lilypalooza_clap::ClapPluginMetadata>),
    ValidVst3Plugins(Vec<lilypalooza_vst3::Vst3PluginMetadata>),
    Invalid,
}

/// Candidate file fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCandidateFingerprint {
    modified_millis: u64,
    len: u64,
}

impl PluginCandidateFingerprint {
    /// Builds a fingerprint from filesystem metadata.
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let modified_millis = modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        Ok(Self {
            modified_millis,
            len: metadata.len(),
        })
    }
}

fn scan_worker(
    search_paths: Vec<PluginSearchPath>,
    mut cache: PluginScanCache,
    validator: PathBuf,
    sender: mpsc::Sender<PluginScanEvent>,
) {
    send_scan_event(
        &sender,
        PluginScanEvent::Log(format!(
            "Scanning plugins with {PLUGIN_VALIDATOR_CONCURRENCY} validator process"
        )),
    );
    let mut summary = PluginScanSummary::default();
    let validator_fingerprint = PluginCandidateFingerprint::from_path(&validator).ok();

    for root in search_paths.into_iter().filter(|path| path.enabled) {
        let Some(candidates) = scan_candidates_for_root(&root, &sender) else {
            continue;
        };
        summary.candidates += candidates.len();

        for candidate in candidates {
            process_scan_candidate(
                root.format,
                candidate,
                &validator,
                validator_fingerprint,
                &mut cache,
                &mut summary,
                &sender,
            );
        }
    }

    send_scan_event(
        &sender,
        PluginScanEvent::Log(format!(
            "Plugin scan finished: {} candidate(s), {} plugin(s), {} invalid",
            summary.candidates, summary.valid_plugins, summary.invalid_candidates
        )),
    );
    send_scan_event(&sender, PluginScanEvent::Finished { summary, cache });
}

fn scan_candidates_for_root(
    root: &PluginSearchPath,
    sender: &mpsc::Sender<PluginScanEvent>,
) -> Option<Vec<PathBuf>> {
    match candidates_for_root(root) {
        Ok(candidates) => Some(candidates),
        Err(error) => {
            send_scan_event(
                sender,
                PluginScanEvent::Log(format!(
                    "Plugin scan skipped {}: {error}",
                    root.path.display()
                )),
            );
            None
        }
    }
}

fn process_scan_candidate(
    format: PluginFormat,
    candidate: PathBuf,
    validator: &Path,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    let Some(fingerprint) = candidate_fingerprint_for_scan(&candidate, summary, sender) else {
        return;
    };
    if !cache.is_stale_for_validator(&candidate, fingerprint, validator_fingerprint) {
        use_cached_scan_candidate(cache, &candidate, fingerprint, summary, sender);
        return;
    }
    let result = validate_candidate(format, &candidate, validator);
    handle_validated_scan_candidate(
        result,
        candidate,
        fingerprint,
        validator_fingerprint,
        cache,
        summary,
        sender,
    );
}

fn candidate_fingerprint_for_scan(
    candidate: &Path,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) -> Option<PluginCandidateFingerprint> {
    match PluginCandidateFingerprint::from_path(candidate) {
        Ok(fingerprint) => Some(fingerprint),
        Err(error) => {
            summary.invalid_candidates += 1;
            send_scan_event(
                sender,
                PluginScanEvent::Log(format!(
                    "Plugin scan skipped {}: {error}",
                    candidate.display()
                )),
            );
            None
        }
    }
}

fn use_cached_scan_candidate(
    cache: &PluginScanCache,
    candidate: &Path,
    fingerprint: PluginCandidateFingerprint,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    match cache.cached_candidate(candidate, fingerprint) {
        Some(CachedCandidateResult::ValidClapPlugins(plugins)) => {
            summary.valid_plugins += plugins.len();
            send_scan_event(sender, PluginScanEvent::ClapPlugins(plugins));
        }
        Some(CachedCandidateResult::ValidVst3Plugins(plugins)) => {
            summary.valid_plugins += plugins.len();
            send_scan_event(sender, PluginScanEvent::Vst3Plugins(plugins));
        }
        Some(CachedCandidateResult::Invalid) | None => {
            summary.invalid_candidates += 1;
        }
    }
}

fn handle_validated_scan_candidate(
    result: Result<ValidatedPlugins, String>,
    candidate: PathBuf,
    fingerprint: PluginCandidateFingerprint,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    match result {
        Ok(plugins) => handle_valid_scan_candidate(
            plugins,
            candidate,
            fingerprint,
            validator_fingerprint,
            cache,
            summary,
            sender,
        ),
        Err(error) => handle_invalid_scan_candidate(
            error,
            candidate,
            fingerprint,
            validator_fingerprint,
            cache,
            summary,
            sender,
        ),
    }
}

fn handle_valid_scan_candidate(
    plugins: ValidatedPlugins,
    candidate: PathBuf,
    fingerprint: PluginCandidateFingerprint,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    if plugins.len() == 0 {
        mark_empty_scan_candidate(
            plugins.format_label(),
            candidate,
            fingerprint,
            validator_fingerprint,
            cache,
            summary,
            sender,
        );
        return;
    }
    summary.valid_plugins += plugins.len();
    plugins.mark_cache_checked(cache, candidate.clone(), fingerprint, validator_fingerprint);
    send_scan_event(
        sender,
        PluginScanEvent::Log(format!(
            "Validated {} {} plugin(s) from {}",
            plugins.len(),
            plugins.format_label(),
            candidate.display()
        )),
    );
    plugins.send_event(sender);
}

fn mark_empty_scan_candidate(
    format_label: &str,
    candidate: PathBuf,
    fingerprint: PluginCandidateFingerprint,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    let reason = format!("validator returned no {format_label} plugins");
    if reuse_cached_valid_candidate(cache, &candidate, fingerprint, summary, sender, &reason) {
        return;
    }
    let message = format!("No {format_label} plugins found in {}", candidate.display());
    mark_invalid_scan_candidate(
        candidate,
        fingerprint,
        validator_fingerprint,
        cache,
        summary,
        sender,
        message,
    );
}

fn handle_invalid_scan_candidate(
    error: String,
    candidate: PathBuf,
    fingerprint: PluginCandidateFingerprint,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
) {
    if reuse_cached_valid_candidate(
        cache,
        &candidate,
        fingerprint,
        summary,
        sender,
        &format!("validation failed: {error}"),
    ) {
        return;
    }
    let message = format!("Invalid plugin {}: {error}", candidate.display());
    mark_invalid_scan_candidate(
        candidate,
        fingerprint,
        validator_fingerprint,
        cache,
        summary,
        sender,
        message,
    );
}

fn mark_invalid_scan_candidate(
    candidate: PathBuf,
    fingerprint: PluginCandidateFingerprint,
    validator_fingerprint: Option<PluginCandidateFingerprint>,
    cache: &mut PluginScanCache,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
    log_message: String,
) {
    summary.invalid_candidates += 1;
    cache.mark_checked(
        candidate.clone(),
        fingerprint,
        validator_fingerprint,
        false,
        Vec::new(),
        Vec::new(),
    );
    send_scan_event(sender, PluginScanEvent::Log(log_message));
}

fn send_scan_event(sender: &mpsc::Sender<PluginScanEvent>, event: PluginScanEvent) {
    match sender.send(event) {
        Ok(()) | Err(_) => {}
    }
}

fn reuse_cached_valid_candidate(
    cache: &PluginScanCache,
    candidate: &Path,
    fingerprint: PluginCandidateFingerprint,
    summary: &mut PluginScanSummary,
    sender: &mpsc::Sender<PluginScanEvent>,
    reason: &str,
) -> bool {
    match cache.cached_candidate(candidate, fingerprint) {
        Some(CachedCandidateResult::ValidClapPlugins(plugins)) => {
            summary.valid_plugins += plugins.len();
            send_scan_event(
                sender,
                PluginScanEvent::Log(format!(
                    "Reusing cached CLAP plugin metadata for {} ({reason})",
                    candidate.display()
                )),
            );
            send_scan_event(sender, PluginScanEvent::ClapPlugins(plugins));
            true
        }
        Some(CachedCandidateResult::ValidVst3Plugins(plugins)) => {
            summary.valid_plugins += plugins.len();
            send_scan_event(
                sender,
                PluginScanEvent::Log(format!(
                    "Reusing cached VST3 plugin metadata for {} ({reason})",
                    candidate.display()
                )),
            );
            send_scan_event(sender, PluginScanEvent::Vst3Plugins(plugins));
            true
        }
        Some(CachedCandidateResult::Invalid) | None => false,
    }
}

/// Returns plugin candidates directly under one search root.
pub fn candidates_for_root(root: &PluginSearchPath) -> Result<Vec<PathBuf>, String> {
    match root.format {
        PluginFormat::Clap => {
            lilypalooza_clap::candidate_paths(&root.path).map_err(|error| error.to_string())
        }
        PluginFormat::Vst3 => {
            lilypalooza_vst3::candidate_paths(&root.path).map_err(|error| error.to_string())
        }
    }
}

enum ValidatedPlugins {
    Clap(Vec<lilypalooza_clap::ClapPluginMetadata>),
    Vst3(Vec<lilypalooza_vst3::Vst3PluginMetadata>),
}

impl ValidatedPlugins {
    fn len(&self) -> usize {
        match self {
            Self::Clap(plugins) => plugins.len(),
            Self::Vst3(plugins) => plugins.len(),
        }
    }

    fn format_label(&self) -> &'static str {
        match self {
            Self::Clap(_) => "CLAP",
            Self::Vst3(_) => "VST3",
        }
    }

    fn mark_cache_checked(
        &self,
        cache: &mut PluginScanCache,
        candidate: PathBuf,
        fingerprint: PluginCandidateFingerprint,
        validator_fingerprint: Option<PluginCandidateFingerprint>,
    ) {
        let (clap, vst3) = match self {
            Self::Clap(plugins) => (plugins.clone(), Vec::new()),
            Self::Vst3(plugins) => (Vec::new(), plugins.clone()),
        };
        cache.mark_checked(
            candidate,
            fingerprint,
            validator_fingerprint,
            true,
            clap,
            vst3,
        );
    }

    fn send_event(self, sender: &mpsc::Sender<PluginScanEvent>) {
        match self {
            Self::Clap(plugins) => send_scan_event(sender, PluginScanEvent::ClapPlugins(plugins)),
            Self::Vst3(plugins) => send_scan_event(sender, PluginScanEvent::Vst3Plugins(plugins)),
        }
    }
}

fn validate_candidate(
    format: PluginFormat,
    path: &Path,
    validator: &Path,
) -> Result<ValidatedPlugins, String> {
    match format {
        PluginFormat::Clap => validate_clap_candidate(path, validator),
        PluginFormat::Vst3 => validate_vst3_candidate(path, validator),
    }
}

fn validate_vst3_candidate(path: &Path, validator: &Path) -> Result<ValidatedPlugins, String> {
    let output = Command::new(validator)
        .arg("--format")
        .arg(lilypalooza_vst3::FORMAT)
        .arg("--path")
        .arg(path)
        .output()
        .map_err(|error| format!("failed to run validator {}: {error}", validator.display()))?;
    parse_vst3_validator_output(output.status.success(), &output.stdout, &output.stderr)
}

fn parse_vst3_validator_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<ValidatedPlugins, String> {
    parse_validator_report::<lilypalooza_vst3::ValidationReport>(
        success,
        stdout,
        stderr,
        vst3_report_plugins,
    )
}

fn vst3_report_plugins(
    report: lilypalooza_vst3::ValidationReport,
) -> Result<ValidatedPlugins, String> {
    report
        .result
        .map(ValidatedPlugins::Vst3)
        .map_err(|error| error.to_string())
}

fn validate_clap_candidate(path: &Path, validator: &Path) -> Result<ValidatedPlugins, String> {
    let output = Command::new(validator)
        .arg("--format")
        .arg(lilypalooza_clap::FORMAT)
        .arg("--path")
        .arg(path)
        .output()
        .map_err(|error| format!("failed to run validator {}: {error}", validator.display()))?;
    parse_clap_validator_output(output.status.success(), &output.stdout, &output.stderr)
}

fn parse_clap_validator_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<ValidatedPlugins, String> {
    parse_validator_report::<lilypalooza_clap::ValidationReport>(
        success,
        stdout,
        stderr,
        clap_report_plugins,
    )
}

fn clap_report_plugins(
    report: lilypalooza_clap::ValidationReport,
) -> Result<ValidatedPlugins, String> {
    report
        .result
        .map(ValidatedPlugins::Clap)
        .map_err(|error| error.to_string())
}

fn parse_validator_report<T>(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
    into_plugins: impl Fn(T) -> Result<ValidatedPlugins, String>,
) -> Result<ValidatedPlugins, String>
where
    T: DeserializeOwned,
{
    let report = parse_validator_stdout::<T>(stdout);
    if !success {
        return match report {
            Ok(report) => into_plugins(report),
            Err(_) => Err(String::from_utf8_lossy(stderr).trim().to_string()),
        };
    }
    let report = report.map_err(|error| error.to_string())?;
    into_plugins(report)
}

fn parse_validator_stdout<T>(stdout: &[u8]) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    if let Ok(report) = serde_json::from_slice(stdout) {
        return Ok(report);
    }

    let mut last_error = None;
    for (index, byte) in stdout.iter().enumerate() {
        if *byte != b'{' {
            continue;
        }
        let Some(json_tail) = stdout.get(index..) else {
            continue;
        };
        let mut deserializer = serde_json::Deserializer::from_slice(json_tail);
        match T::deserialize(&mut deserializer) {
            Ok(report) => return Ok(report),
            Err(error) => last_error = Some(error),
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => serde_json::from_slice(stdout),
    }
}

#[cfg(test)]
mod scan_tests;
