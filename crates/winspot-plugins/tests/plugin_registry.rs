use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};

use winspot_core::{ActionCapability, SearchResultKind};
use winspot_plugins::{
    PluginManifest, PluginRegistry, PluginValidationError, built_in_plugin_manifests,
};

/// Returns a unique, freshly-created temp directory so tests can run in
/// parallel without colliding on a shared per-process path.
fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "winspot-plugins-{label}-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create plugin dir");
    dir
}

fn manifest(id: &str, name: &str, capabilities: Vec<ActionCapability>) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: name.to_string(),
        capabilities,
        enabled: true,
    }
}

#[test]
fn manifest_directory_loads_enabled_plugins() {
    let root = std::env::temp_dir().join(format!("winspot-plugins-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create plugin dir");
    fs::write(
        root.join("sample.json"),
        r#"{"id":"sample","name":"Sample Plugin","capabilities":["ClipboardWrite"],"enabled":true}"#,
    )
    .expect("write manifest");

    let registry = PluginRegistry::load_dir(&root).expect("load registry");
    let results = registry.internal_results("sample");

    assert_eq!(results[0].title, "Sample Plugin");
    assert_eq!(results[0].kind, SearchResultKind::Plugin);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn manifest_directory_skips_malformed_plugins() {
    let root =
        std::env::temp_dir().join(format!("winspot-plugins-malformed-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create plugin dir");
    fs::write(root.join("broken.json"), "{not json").expect("write bad manifest");
    fs::write(
        root.join("sample.json"),
        r#"{"id":"sample","name":"Sample Plugin","capabilities":[],"enabled":true}"#,
    )
    .expect("write manifest");

    let registry = PluginRegistry::load_dir(&root).expect("load registry");
    let results = registry.internal_results("sample");

    assert_eq!(results[0].title, "Sample Plugin");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn built_in_plugins_declare_capabilities() {
    let manifests = built_in_plugin_manifests();

    assert!(manifests.iter().any(|manifest| {
        manifest.id == "terminal"
            && manifest
                .capabilities
                .contains(&ActionCapability::ProcessExecution)
    }));
}

#[test]
fn validate_accepts_well_formed_manifest() {
    let valid = manifest(
        "my-plugin_2",
        "My Plugin",
        vec![ActionCapability::ClipboardWrite],
    );
    assert_eq!(valid.validate(), Ok(()));
}

#[test]
fn validate_rejects_empty_and_malformed_ids() {
    assert_eq!(
        manifest("", "Name", vec![]).validate(),
        Err(PluginValidationError::EmptyId)
    );
    assert!(matches!(
        manifest("Has Spaces", "Name", vec![]).validate(),
        Err(PluginValidationError::InvalidId { .. })
    ));
    assert!(matches!(
        manifest("UpperCase", "Name", vec![]).validate(),
        Err(PluginValidationError::InvalidId { .. })
    ));
    assert!(matches!(
        manifest("-leading-dash", "Name", vec![]).validate(),
        Err(PluginValidationError::InvalidId { .. })
    ));
}

#[test]
fn validate_rejects_empty_name_and_duplicate_capabilities() {
    assert!(matches!(
        manifest("ok", "   ", vec![]).validate(),
        Err(PluginValidationError::EmptyName { .. })
    ));
    assert!(matches!(
        manifest(
            "ok",
            "Name",
            vec![
                ActionCapability::ClipboardWrite,
                ActionCapability::ClipboardWrite,
            ],
        )
        .validate(),
        Err(PluginValidationError::DuplicateCapability { .. })
    ));
}

#[test]
fn with_built_ins_registers_all_built_in_plugins() {
    let registry = PluginRegistry::with_built_ins();
    let ids: Vec<&str> = registry.manifests().map(|m| m.id.as_str()).collect();

    for expected in ["calculator", "terminal", "clipboard", "unit-conversion"] {
        assert!(ids.contains(&expected), "missing built-in {expected}");
    }
}

#[test]
fn load_dir_into_merges_user_manifests_with_built_ins() {
    let root = unique_temp_dir("merge");
    fs::write(
        root.join("custom.json"),
        r#"{"id":"custom","name":"Custom Plugin","capabilities":[],"enabled":true}"#,
    )
    .expect("write manifest");

    let mut registry = PluginRegistry::with_built_ins();
    registry.load_dir_into(&root).expect("merge user manifests");

    assert_eq!(
        registry.internal_results("Calculator")[0].title,
        "Calculator"
    );
    assert_eq!(
        registry.internal_results("Custom")[0].title,
        "Custom Plugin"
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn load_dir_into_skips_duplicate_and_invalid_manifests() {
    let root = unique_temp_dir("dupe");
    // Reuses a built-in id -> rejected as a duplicate, built-in preserved.
    fs::write(
        root.join("dupe.json"),
        r#"{"id":"calculator","name":"Hijacked","capabilities":[],"enabled":true}"#,
    )
    .expect("write dupe manifest");
    // Invalid id -> rejected by validation.
    fs::write(
        root.join("bad-id.json"),
        r#"{"id":"Bad Id","name":"Bad","capabilities":[],"enabled":true}"#,
    )
    .expect("write invalid manifest");

    let mut registry = PluginRegistry::with_built_ins();
    registry.load_dir_into(&root).expect("merge");

    let calculator = registry.internal_results("Calculator");
    assert_eq!(calculator.len(), 1);
    assert_eq!(calculator[0].title, "Calculator");
    assert!(registry.internal_results("Hijacked").is_empty());
    assert!(registry.internal_results("Bad").is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn disabled_manifests_are_not_returned() {
    let root = unique_temp_dir("disabled");
    fs::write(
        root.join("off.json"),
        r#"{"id":"off","name":"Disabled Plugin","capabilities":[],"enabled":false}"#,
    )
    .expect("write manifest");

    let registry = PluginRegistry::load_dir(&root).expect("load");

    assert!(registry.internal_results("Disabled").is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}
