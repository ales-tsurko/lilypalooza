use super::*;

fn test_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("lilypalooza-plugin-scan-")
        .tempdir()
        .expect("temp dir")
}

fn test_path(file: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = test_dir();
    let path = dir.path().join(file);
    (dir, path)
}

fn vst3_report_stdout(path: &Path, role: &str) -> Vec<u8> {
    serde_json::json!({
        "format": "vst3",
        "path": path,
        "result": {
            "Ok": [{
                "processor_id": format!("vst3:{}#00112233445566778899aabbccddeeff", path.display()),
                "class_id": "00112233445566778899aabbccddeeff",
                "name": "Plugin",
                "vendor": "Vendor",
                "version": null,
                "category": null,
                "role": role,
                "path": path,
                "library_path": path,
            }]
        }
    })
    .to_string()
    .into_bytes()
}

#[test]
fn cache_marks_changed_candidate_as_stale() {
    let (_dir, path) = test_path("test.clap");
    let mut cache = PluginScanCache::default();
    let old = PluginCandidateFingerprint {
        modified_millis: 10,
        len: 20,
    };
    let new = PluginCandidateFingerprint {
        modified_millis: 11,
        len: 20,
    };

    assert!(cache.is_stale(&path, old));
    cache.mark_checked(path.clone(), old, None, true, Vec::new(), Vec::new());
    assert!(!cache.is_stale(&path, old));
    assert!(cache.is_stale(&path, new));
}

#[test]
fn cache_marks_changed_validator_as_stale() {
    let (_dir, path) = test_path("test.clap");
    let candidate = PluginCandidateFingerprint {
        modified_millis: 10,
        len: 20,
    };
    let old_validator = Some(PluginCandidateFingerprint {
        modified_millis: 1,
        len: 2,
    });
    let new_validator = Some(PluginCandidateFingerprint {
        modified_millis: 2,
        len: 2,
    });
    let mut cache = PluginScanCache::default();

    cache.mark_checked(
        path.clone(),
        candidate,
        old_validator,
        true,
        Vec::new(),
        Vec::new(),
    );

    assert!(!cache.is_stale_for_validator(&path, candidate, old_validator));
    assert!(cache.is_stale_for_validator(&path, candidate, new_validator));
}

#[test]
fn clap_root_collects_only_clap_candidates() {
    let dir = test_dir();
    std::fs::write(dir.path().join("a.clap"), "").expect("clap file");
    std::fs::write(dir.path().join("b.vst3"), "").expect("vst3 file");
    let root = PluginSearchPath {
        format: PluginFormat::Clap,
        path: dir.path().to_path_buf(),
        enabled: true,
    };

    let candidates = candidates_for_root(&root).expect("scan root");

    assert_eq!(candidates, vec![dir.path().join("a.clap")]);
}

#[test]
fn vst3_root_collects_vst3_candidates_recursively() {
    let dir = test_dir();
    std::fs::write(dir.path().join("a.clap"), "").expect("clap file");
    let nested = dir.path().join("Vendor").join("b.vst3");
    std::fs::create_dir_all(&nested).expect("vst3 bundle");
    let root = PluginSearchPath {
        format: PluginFormat::Vst3,
        path: dir.path().to_path_buf(),
        enabled: true,
    };

    let candidates = candidates_for_root(&root).expect("scan root");

    assert_eq!(candidates, vec![nested]);
}

#[test]
fn cache_roundtrips_from_explicit_path() {
    let (_cache_dir, path) = test_path("plugin-cache.ron");
    let (_candidate_dir, candidate) = test_path("test.clap");
    let fingerprint = PluginCandidateFingerprint {
        modified_millis: 7,
        len: 9,
    };
    let mut cache = PluginScanCache::default();
    cache.mark_checked(
        candidate.clone(),
        fingerprint,
        None,
        true,
        Vec::new(),
        Vec::new(),
    );

    cache.save_to(&path).expect("cache should save");
    let loaded = PluginScanCache::load_from(&path);

    assert!(!loaded.is_stale(&candidate, fingerprint));
}

#[test]
fn unchanged_valid_plugin_is_reused_when_revalidation_fails() {
    let (_dir, path) = test_path("plugin.vst3");
    let fingerprint = PluginCandidateFingerprint {
        modified_millis: 7,
        len: 9,
    };
    let stdout = vst3_report_stdout(&path, "instrument");
    let ValidatedPlugins::Vst3(plugins) =
        parse_vst3_validator_output(true, &stdout, b"").expect("valid plugin metadata")
    else {
        panic!("expected VST3 plugins");
    };
    let mut cache = PluginScanCache::default();
    cache.mark_checked(
        path.clone(),
        fingerprint,
        Some(PluginCandidateFingerprint {
            modified_millis: 1,
            len: 2,
        }),
        true,
        Vec::new(),
        plugins,
    );
    let (sender, receiver) = mpsc::channel();
    let mut summary = PluginScanSummary::default();

    assert!(reuse_cached_valid_candidate(
        &cache,
        &path,
        fingerprint,
        &mut summary,
        &sender,
        "validation failed"
    ));

    assert_eq!(summary.valid_plugins, 1);
    assert!(matches!(
        receiver.try_iter().last(),
        Some(PluginScanEvent::Vst3Plugins(plugins)) if plugins[0].name == "Plugin"
    ));
}

#[test]
fn drain_events_with_limit_keeps_scan_active_when_budget_is_exhausted() {
    let (sender, receiver) = mpsc::channel();
    sender
        .send(PluginScanEvent::Log("one".to_string()))
        .expect("send one");
    sender
        .send(PluginScanEvent::Log("two".to_string()))
        .expect("send two");
    sender
        .send(PluginScanEvent::Log("three".to_string()))
        .expect("send three");
    let mut state = PluginScanState {
        receiver: Some(receiver),
        active: true,
    };

    let events = state.drain_events_with_limit(2);

    assert_eq!(events.len(), 2);
    assert!(state.is_active());
    assert_eq!(state.drain_events().len(), 1);
}

#[test]
fn drain_events_with_limit_zero_does_not_drain() {
    let (sender, receiver) = mpsc::channel();
    sender
        .send(PluginScanEvent::Log("one".to_string()))
        .expect("send one");
    let mut state = PluginScanState {
        receiver: Some(receiver),
        active: true,
    };

    assert!(state.drain_events_with_limit(0).is_empty());
    assert_eq!(state.drain_events().len(), 1);
}

#[test]
fn empty_clap_validation_result_parses_as_empty_plugin_list() {
    let (_dir, path) = test_path("empty.clap");
    let stdout = serde_json::json!({
        "format": "clap",
        "path": path,
        "result": { "Ok": [] },
    })
    .to_string()
    .into_bytes();
    let plugins =
        parse_clap_validator_output(true, &stdout, b"").expect("empty valid report should parse");

    match plugins {
        ValidatedPlugins::Clap(plugins) => assert!(plugins.is_empty()),
        ValidatedPlugins::Vst3(_) => panic!("expected CLAP plugins"),
    }
}

#[test]
fn non_success_validator_with_valid_report_is_accepted() {
    let (_dir, path) = test_path("plugin.vst3");
    let stdout = vst3_report_stdout(&path, "effect");
    let plugins = parse_vst3_validator_output(false, &stdout, b"process exited non-zero")
        .expect("valid stdout should parse");

    match plugins {
        ValidatedPlugins::Vst3(plugins) => assert_eq!(plugins.len(), 1),
        ValidatedPlugins::Clap(_) => panic!("expected VST3 plugins"),
    }
}

#[test]
fn validator_stdout_prefix_noise_is_ignored() {
    let (_dir, path) = test_path("plugin.vst3");
    let mut stdout = b"[info] initializing\n[info] ready\n".to_vec();
    stdout.extend(vst3_report_stdout(&path, "effect"));
    let plugins = parse_vst3_validator_output(true, &stdout, b"")
        .expect("valid report after log lines should parse");

    match plugins {
        ValidatedPlugins::Vst3(plugins) => assert_eq!(plugins[0].name, "Plugin"),
        ValidatedPlugins::Clap(_) => panic!("expected VST3 plugins"),
    }
}
