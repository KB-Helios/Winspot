use std::fs;

use winspot_core::SearchResultKind;
use winspot_search::providers::{
    BuiltinCommandProvider, CalculatorProvider, DynamicSearchProvider, FileSystemProvider,
    StartMenuAppProvider,
};

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

#[test]
fn file_system_provider_discovers_files_and_folders_from_configured_roots() {
    let root = std::env::temp_dir().join(format!("winspot-files-test-{}", std::process::id()));
    let projects = root.join("Projects");
    fs::create_dir_all(&projects).expect("create test project directory");
    fs::write(root.join("Roadmap.md"), b"next slice").expect("write test file");

    let provider = FileSystemProvider::new(vec![root.clone()], 2, 32);
    let results = provider.collect_results();

    assert!(
        results
            .iter()
            .any(|result| result.title == "Roadmap.md" && result.kind == SearchResultKind::File)
    );
    assert!(
        results
            .iter()
            .any(|result| result.title == "Projects" && result.kind == SearchResultKind::Folder)
    );

    fs::remove_dir_all(root).expect("cleanup test directory");
}

#[test]
fn file_system_provider_respects_entry_limit() {
    let root =
        std::env::temp_dir().join(format!("winspot-files-limit-test-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create test directory");
    fs::write(root.join("One.txt"), b"one").expect("write first test file");
    fs::write(root.join("Two.txt"), b"two").expect("write second test file");

    let provider = FileSystemProvider::new(vec![root.clone()], 1, 1);
    let results = provider.collect_results();

    assert_eq!(results.len(), 1);

    fs::remove_dir_all(root).expect("cleanup test directory");
}

#[test]
fn calculator_provider_returns_result_for_arithmetic_expression() {
    let results = CalculatorProvider::default().search("2 + 3 * 4");

    assert_eq!(results[0].title, "2 + 3 * 4 = 14");
    assert_eq!(results[0].subtitle, "Calculator result");
    assert_eq!(results[0].kind, SearchResultKind::Command);
}

#[test]
fn calculator_provider_ignores_non_math_queries() {
    let results = CalculatorProvider::default().search("notepad");

    assert!(results.is_empty());
}
