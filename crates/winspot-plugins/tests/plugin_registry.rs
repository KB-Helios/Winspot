use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};

use winspot_core::{ActionCapability, SearchResultKind};
use winspot_plugins::{
    PluginManifest, PluginRegistry, PluginSource, PluginValidationError, PluginValidationStatus,
    built_in_plugin_manifests,
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
fn load_dir_into_accepts_case_insensitive_json_extensions() {
    let root = unique_temp_dir("case-ext");
    fs::write(
        root.join("custom.JSON"),
        r#"{"id":"custom","name":"Custom Plugin","capabilities":[],"enabled":true}"#,
    )
    .expect("write manifest");

    let registry = PluginRegistry::load_dir(&root).expect("load registry");

    assert_eq!(
        registry.internal_results("Custom")[0].title,
        "Custom Plugin"
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn load_dir_into_with_report_propagates_non_directory_errors() {
    let root = unique_temp_dir("not-dir");
    let not_directory = root.join("plugins.json");
    fs::write(&not_directory, b"not a directory").expect("write file");
    let mut registry = PluginRegistry::default();

    let error = registry
        .load_dir_into_with_report(&not_directory)
        .expect_err("file path should not be treated as empty plugin dir");

    assert!(error.to_string().contains("failed to read directory"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn load_dir_into_with_report_tolerates_missing_directory() {
    let missing =
        std::env::temp_dir().join(format!("winspot-plugins-missing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&missing);
    let mut registry = PluginRegistry::default();

    let report = registry
        .load_dir_into_with_report(&missing)
        .expect("missing directory is allowed for daemon startup");

    assert!(report.entries.is_empty());
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

fn issue_codes(report: &winspot_plugins::PluginValidationReport) -> Vec<&str> {
    report
        .entries
        .iter()
        .flat_map(|entry| entry.issues.iter().map(|issue| issue.code.as_str()))
        .collect()
}

#[test]
fn validation_report_records_unknown_fields_and_user_execution_warnings() {
    let root = unique_temp_dir("warnings");
    fs::write(
        root.join("warn.json"),
        r#"{
            "id":"warn",
            "name":"Warn Plugin",
            "capabilities":["PluginExecution"],
            "description":"future manifest field",
            "enabled":true
        }"#,
    )
    .expect("write warning manifest");

    let (registry, report) = PluginRegistry::load_dir_with_report(&root).expect("load report");
    let entry = report
        .entries
        .iter()
        .find(|entry| entry.id.as_deref() == Some("warn"))
        .expect("warning entry exists");
    let codes = issue_codes(&report);

    assert!(!report.has_errors(), "warnings must not fail validation");
    assert!(report.has_warnings(), "report should expose warning state");
    assert_eq!(entry.source, PluginSource::User);
    assert_eq!(entry.status, PluginValidationStatus::Accepted);
    assert!(!entry.trusted, "user manifests are search-only by default");
    assert!(codes.contains(&"unknown_field"));
    assert!(codes.contains(&"ignored_user_executable_capability"));
    assert!(
        registry
            .enabled_manifests()
            .any(|manifest| manifest.id == "warn"),
        "valid warning-only manifest should still be searchable"
    );
    assert!(
        !registry.plugin_has_executable_action("warn"),
        "user manifest must not gain executable plugin actions in V1"
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn validation_report_records_rejected_manifests() {
    let root = unique_temp_dir("rejected");
    fs::write(root.join("broken.json"), "{not json").expect("write malformed manifest");
    fs::write(
        root.join("bad-id.json"),
        r#"{"id":"Bad Id","name":"Bad","capabilities":[],"enabled":true}"#,
    )
    .expect("write invalid id manifest");
    fs::write(
        root.join("duplicate.json"),
        r#"{"id":"calculator","name":"Hijack","capabilities":[],"enabled":true}"#,
    )
    .expect("write duplicate manifest");

    let (mut registry, mut report) = PluginRegistry::with_built_ins_with_report();
    report.extend(
        registry
            .load_dir_into_with_report(&root)
            .expect("load user report"),
    );
    let codes = issue_codes(&report);

    assert!(report.has_errors());
    assert!(codes.contains(&"manifest_parse_failed"));
    assert!(codes.contains(&"invalid_manifest"));
    assert!(codes.contains(&"duplicate_plugin_id"));
    assert!(
        report
            .entries
            .iter()
            .any(|entry| entry.status == PluginValidationStatus::Rejected)
    );
    assert!(registry.internal_results("Hijack").is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn registry_authorizes_only_trusted_enabled_executable_plugins() {
    let root = unique_temp_dir("authorization");
    fs::write(
        root.join("custom.json"),
        r#"{"id":"custom","name":"Custom Plugin","capabilities":["PluginExecution"],"enabled":true}"#,
    )
    .expect("write custom manifest");
    fs::write(
        root.join("off.json"),
        r#"{"id":"off","name":"Disabled Plugin","capabilities":[],"enabled":false}"#,
    )
    .expect("write disabled manifest");

    let (mut registry, _report) = PluginRegistry::with_built_ins_with_report();
    registry
        .load_dir_into_with_report(&root)
        .expect("load user manifests");

    assert!(
        registry
            .ensure_plugin_command_allowed("plugin:calculator")
            .is_ok(),
        "trusted executable built-in should be authorized"
    );
    assert!(
        registry
            .ensure_plugin_command_allowed("plugin:clipboard")
            .expect_err("search-only built-in should be refused")
            .to_string()
            .contains("no executable action")
    );
    assert!(
        registry
            .ensure_plugin_command_allowed("plugin:custom")
            .expect_err("user plugin should be refused")
            .to_string()
            .contains("not trusted")
    );
    assert!(
        registry
            .ensure_plugin_command_allowed("plugin:off")
            .expect_err("disabled plugin should be refused")
            .to_string()
            .contains("disabled")
    );
    assert!(
        registry
            .ensure_plugin_command_allowed("plugin:")
            .expect_err("malformed id should be refused")
            .to_string()
            .contains("malformed plugin id")
    );

    fs::remove_dir_all(root).expect("cleanup");
}
