use std::path::Path;

use plist::{Dictionary, Value};

use super::*;

#[test]
fn au_candidate_detection_is_extension_based() {
    assert!(is_au_candidate(Path::new("Plugin.component")));
    assert!(is_au_candidate(Path::new("Plugin.COMPONENT")));
    assert!(!is_au_candidate(Path::new("Plugin.vst3")));
}

#[test]
fn au_candidate_paths_recurse() {
    let dir = tempfile::tempdir().expect("temp dir");
    let nested = dir.path().join("Vendor").join("Plugin.component");
    std::fs::create_dir_all(&nested).expect("nested component dir");
    std::fs::write(dir.path().join("Other.vst3"), "").expect("other file");

    let candidates = candidate_paths(dir.path()).expect("candidate scan");

    assert_eq!(candidates, vec![nested]);
}

#[test]
fn au_probe_reads_audio_components_from_info_plist() {
    let dir = tempfile::tempdir().expect("temp dir");
    let component = dir.path().join("Test.component");
    let contents = component.join("Contents");
    std::fs::create_dir_all(&contents).expect("component contents");
    let plist = component_plist("aumu", "Tst1", "Acme", "Acme Synth");
    plist
        .to_file_xml(contents.join("Info.plist"))
        .expect("write plist");

    let plugins = probe(&component).expect("probe");

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "Acme Synth");
    assert_eq!(plugins[0].vendor.as_deref(), Some("Acme"));
    assert_eq!(plugins[0].role, registry::Role::Instrument);
    assert_eq!(
        plugins[0].processor_id,
        format!("au:{}#aumu-Tst1-Acme", component.display())
    );
}

#[test]
fn au_probe_uses_name_prefix_as_vendor_when_manufacturer_name_is_missing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let component = dir.path().join("Test.component");
    let contents = component.join("Contents");
    std::fs::create_dir_all(&contents).expect("component contents");
    let mut plist = component_plist("aumu", "Tst1", "Acme", "u-he: Zebralette3");
    let root = plist.as_dictionary_mut().expect("root dictionary");
    let components = root
        .get_mut("AudioComponents")
        .and_then(Value::as_array_mut)
        .expect("components array");
    components[0]
        .as_dictionary_mut()
        .expect("component dictionary")
        .remove("manufacturerName");
    plist
        .to_file_xml(contents.join("Info.plist"))
        .expect("write plist");

    let plugins = probe(&component).expect("probe");

    assert_eq!(plugins[0].vendor.as_deref(), Some("u-he"));
}

#[test]
fn au_probe_ignores_unsupported_component_types() {
    let dir = tempfile::tempdir().expect("temp dir");
    let component = dir.path().join("Test.component");
    let contents = component.join("Contents");
    std::fs::create_dir_all(&contents).expect("component contents");
    component_plist("augn", "Tst1", "Acme", "Generator")
        .to_file_xml(contents.join("Info.plist"))
        .expect("write plist");

    let error = probe(&component).expect_err("unsupported type should not produce plugins");

    assert!(
        error
            .to_string()
            .contains("declares no supported AudioComponents")
    );
}

#[test]
fn stable_processor_id_uses_component_codes() {
    let id = stable_processor_id(
        Path::new("/Plug/Test.component"),
        AuComponentId {
            component_type: probe::test_fourcc("aufx"),
            component_subtype: probe::test_fourcc("Dl01"),
            component_manufacturer: probe::test_fourcc("Acme"),
        },
    );

    assert_eq!(id, "au:/Plug/Test.component#aufx-Dl01-Acme");
}

fn component_plist(component_type: &str, subtype: &str, manufacturer: &str, name: &str) -> Value {
    let mut component = Dictionary::new();
    component.insert(
        "type".to_string(),
        Value::String(component_type.to_string()),
    );
    component.insert("subtype".to_string(), Value::String(subtype.to_string()));
    component.insert(
        "manufacturer".to_string(),
        Value::String(manufacturer.to_string()),
    );
    component.insert("name".to_string(), Value::String(name.to_string()));
    component.insert(
        "manufacturerName".to_string(),
        Value::String("Acme".to_string()),
    );

    let mut root = Dictionary::new();
    root.insert(
        "CFBundleName".to_string(),
        Value::String("Test".to_string()),
    );
    root.insert(
        "AudioComponents".to_string(),
        Value::Array(vec![Value::Dictionary(component)]),
    );
    Value::Dictionary(root)
}
