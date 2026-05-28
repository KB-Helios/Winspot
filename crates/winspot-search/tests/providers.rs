use std::fs;

use winspot_search::providers::{BuiltinCommandProvider, StartMenuAppProvider};

#[test]
fn builtin_command_provider_exposes_calculator_and_terminal() {
    let results = BuiltinCommandProvider::default().collect_results();

    assert!(results.iter().any(|result| result.title == "Calculator"));
    assert!(results.iter().any(|result| result.title == "Terminal"));
}

#[test]
fn start_menu_provider_discovers_shortcuts_from_configured_roots() {
    let root = std::env::temp_dir().join(format!("winspot-provider-test-{}", std::process::id()));
    let programs = root.join("Programs");
    fs::create_dir_all(&programs).expect("create test programs directory");
    fs::write(programs.join("Sample App.lnk"), b"shortcut").expect("write shortcut");

    let provider = StartMenuAppProvider::new(vec![root.clone()]);
    let results = provider.collect_results();

    assert!(results.iter().any(|result| result.title == "Sample App"));

    fs::remove_dir_all(root).expect("cleanup test directory");
}
