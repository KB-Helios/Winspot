use std::fs;
use std::sync::{Arc, RwLock};

use winspot_core::{ActionKind, SearchResultKind};
use winspot_plugins::PluginRegistry;
use winspot_search::providers::{
    BrowserHistoryProvider, BuiltInPluginProvider, BuiltinCommandProvider, CalculatorProvider,
    DynamicSearchProvider, FileSystemProvider, PluginProvider, RunningProcessProvider,
    StartMenuAppProvider, UnitConversionProvider, WINSPOT_SETTINGS_COMMAND_ID,
    WindowsSettingsProvider,
};

#[test]
fn builtin_command_provider_exposes_calculator_and_terminal() {
    let results = BuiltinCommandProvider.collect_results();

    assert!(results.iter().any(|result| result.title == "Calculator"));
    assert!(results.iter().any(|result| result.title == "Terminal"));
}

#[test]
fn builtin_command_provider_attaches_executable_action_descriptors() {
    let results = BuiltinCommandProvider.collect_results();
    let calculator = results
        .iter()
        .find(|result| result.id == "command:calculator")
        .expect("calculator command exists");

    assert!(calculator.actions.iter().any(|action| {
        action.id == "run" && action.label == "Run" && action.kind == ActionKind::RunCommand
    }));
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

    let file = results
        .iter()
        .find(|result| result.title == "Roadmap.md")
        .expect("file result exists");
    assert!(
        file.actions
            .iter()
            .any(|action| { action.id == "open" && action.kind == ActionKind::Open })
    );
    assert!(file.actions.iter().any(|action| {
        action.id == "open-containing-folder" && action.kind == ActionKind::OpenContainingFolder
    }));
    assert!(
        file.actions
            .iter()
            .any(|action| { action.id == "copy-path" && action.kind == ActionKind::CopyPath })
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
fn windows_settings_provider_exposes_common_settings_pages() {
    let results = WindowsSettingsProvider.collect_results();

    assert!(results.iter().any(|result| {
        result.title == "Display settings"
            && result.id == "setting:ms-settings:display"
            && result.kind == SearchResultKind::Setting
            && result.primary_action == ActionKind::Open
    }));
    assert!(
        results
            .iter()
            .any(|result| result.title == "Windows Update")
    );
}

#[test]
fn running_process_provider_parses_tasklist_csv() {
    let output = "\"notepad.exe\",\"1234\",\"Console\",\"1\",\"12,340 K\"\r\n\
        \"Winspot.App.exe\",\"4321\",\"Console\",\"1\",\"98,000 K\"\r\n";

    let results = RunningProcessProvider::collect_from_tasklist_csv(output);

    assert!(results.iter().any(|result| {
        result.title == "notepad.exe"
            && result.id == "process:1234:notepad.exe"
            && result.subtitle == "PID 1234 - 12,340 K"
            && result.kind == SearchResultKind::Process
            && result.primary_action == ActionKind::Copy
            && result
                .actions
                .iter()
                .any(|action| action.id == "kill-process" && action.kind == ActionKind::KillProcess)
    }));
    assert!(
        results
            .iter()
            .any(|result| result.title == "Winspot.App.exe")
    );
}

#[test]
fn calculator_provider_returns_result_for_arithmetic_expression() {
    let results = CalculatorProvider.search("2 + 3 * 4");

    assert_eq!(results[0].title, "2 + 3 * 4 = 14");
    assert_eq!(results[0].subtitle, "Calculator result");
    assert_eq!(results[0].kind, SearchResultKind::Command);
}

#[test]
fn calculator_provider_ignores_non_math_queries() {
    let results = CalculatorProvider.search("notepad");

    assert!(results.is_empty());
}

#[test]
fn unit_conversion_provider_converts_length_units() {
    let results = UnitConversionProvider.search("10 km to mi");

    assert_eq!(results[0].title, "10 km to mi = 6.213712 mi");
    assert_eq!(results[0].subtitle, "Unit conversion");
    assert_eq!(results[0].kind, SearchResultKind::Command);
    assert_eq!(results[0].primary_action, ActionKind::Copy);
}

#[test]
fn unit_conversion_provider_converts_temperature_units() {
    let results = UnitConversionProvider.search("32 f to c");

    assert_eq!(results[0].title, "32 F to C = 0 C");
}

#[test]
fn unit_conversion_provider_ignores_mismatched_dimensions() {
    let results = UnitConversionProvider.search("10 kg to mi");

    assert!(results.is_empty());
}

#[test]
fn browser_history_provider_reads_opt_in_tab_separated_export() {
    let path = std::env::temp_dir().join(format!("winspot-history-{}.tsv", std::process::id()));
    fs::write(&path, "Rust Docs\thttps://doc.rust-lang.org\n").expect("write history file");

    let results = BrowserHistoryProvider::new(vec![path.clone()]).collect_results();

    assert_eq!(results[0].title, "Rust Docs");
    assert_eq!(results[0].kind, SearchResultKind::BrowserHistory);

    fs::remove_file(path).expect("cleanup history");
}

#[test]
fn built_in_plugin_provider_exposes_internal_plugins() {
    let results = BuiltInPluginProvider.search("clipboard");

    assert_eq!(results[0].title, "Clipboard");
    assert_eq!(results[0].kind, SearchResultKind::Plugin);
    assert_eq!(results[0].primary_action, ActionKind::PluginCommand);
}

#[test]
fn builtin_command_provider_exposes_winspot_settings() {
    let results = BuiltinCommandProvider.collect_results();

    let settings = results
        .iter()
        .find(|result| result.id == WINSPOT_SETTINGS_COMMAND_ID)
        .expect("settings command present");
    assert_eq!(settings.title, "Winspot Settings");
}

#[test]
fn plugin_provider_includes_built_ins_and_loaded_manifests() {
    let root = std::env::temp_dir().join(format!("winspot-search-plugins-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create plugin dir");
    fs::write(
        root.join("custom.json"),
        r#"{"id":"custom","name":"Custom Plugin","capabilities":[],"enabled":true}"#,
    )
    .expect("write manifest");

    let mut registry = PluginRegistry::with_built_ins();
    registry.load_dir_into(&root).expect("merge manifests");
    let provider = PluginProvider::new(Arc::new(RwLock::new(registry)));

    let built_in = provider.search("clipboard");
    assert_eq!(built_in[0].title, "Clipboard");
    assert_eq!(built_in[0].primary_action, ActionKind::PluginCommand);

    let custom = provider.search("custom");
    assert_eq!(custom[0].title, "Custom Plugin");
    assert_eq!(custom[0].kind, SearchResultKind::Plugin);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn plugin_provider_does_not_expose_actions_for_untrusted_user_manifest_ids() {
    let root = std::env::temp_dir().join(format!(
        "winspot-search-untrusted-plugins-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create plugin dir");
    fs::write(
        root.join("terminal.json"),
        r#"{"id":"terminal","name":"User Terminal","capabilities":["PluginExecution"],"enabled":true}"#,
    )
    .expect("write manifest");

    let (registry, report) = PluginRegistry::load_dir_with_report(&root).expect("load manifests");
    assert!(report.has_warnings());
    assert!(!registry.plugin_has_executable_action("terminal"));

    let provider = PluginProvider::new(Arc::new(RwLock::new(registry)));
    let results = provider.search("user terminal");

    assert_eq!(results[0].title, "User Terminal");
    assert!(results[0].actions.is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}
