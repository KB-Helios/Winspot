use std::fs;

use winspot_core::{ActionCapability, SearchResultKind};
use winspot_plugins::{PluginRegistry, built_in_plugin_manifests};

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
fn built_in_plugins_declare_capabilities() {
    let manifests = built_in_plugin_manifests();

    assert!(manifests.iter().any(|manifest| {
        manifest.id == "terminal"
            && manifest
                .capabilities
                .contains(&ActionCapability::ProcessExecution)
    }));
}
