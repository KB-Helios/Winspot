use std::fs;

use winspot_core::{ActionKind, SearchResultKind};
use winspot_fastflowlm::{
    ContextLimits, FASTFLOWLM_RESULT_ID, FastFlowLmProvider, build_index_context,
    parse_fastflowlm_prompt, should_stop_owned_process, validate_installed_model_from_list_json,
};
use winspot_index::{IndexStore, IndexedItem};
use winspot_search::providers::DynamicSearchProvider;

#[test]
fn parses_only_explicit_ai_prefixes() {
    assert_eq!(
        parse_fastflowlm_prompt("ai summarize the roadmap"),
        Some("summarize the roadmap")
    );
    assert_eq!(
        parse_fastflowlm_prompt(" ask   what changed today "),
        Some("what changed today")
    );
    assert_eq!(parse_fastflowlm_prompt("air quality"), None);
    assert_eq!(parse_fastflowlm_prompt("ai"), None);
    assert_eq!(parse_fastflowlm_prompt("ask"), None);
    assert_eq!(parse_fastflowlm_prompt("notepad"), None);
}

#[test]
fn provider_returns_lightweight_plugin_result_without_executing_flm() {
    let results = FastFlowLmProvider::default().search("ai summarize roadmap");

    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.id, FASTFLOWLM_RESULT_ID);
    assert_eq!(result.title, "summarize roadmap");
    assert_eq!(result.subtitle, "Ask FastFlowLM with launcher context");
    assert_eq!(result.kind, SearchResultKind::Plugin);
    assert_eq!(result.source.as_deref(), Some("fastflowlm"));
    assert_eq!(result.primary_action, ActionKind::PluginCommand);
    assert!(result.actions.iter().any(|action| {
        action.id == "run-plugin"
            && action.label == "Ask"
            && action.kind == ActionKind::PluginCommand
    }));
}

#[test]
fn provider_ignores_queries_without_ai_prefix() {
    assert!(
        FastFlowLmProvider::default()
            .search("summarize roadmap")
            .is_empty()
    );
    assert!(FastFlowLmProvider::default().search("ai").is_empty());
    assert!(FastFlowLmProvider::default().search("ask").is_empty());
}

#[test]
fn validates_installed_model_from_flm_list_json() {
    let json = r#"
        [
          {"name":"llama3.2:1b","installed":true},
          {"model":"gemma4-it:e2b","displayName":"Gemma4-E2B-IT-NPU2","installed":true}
        ]
    "#;

    assert!(validate_installed_model_from_list_json(json, "gemma4-it:e2b").is_ok());

    let error = validate_installed_model_from_list_json(json, "missing:model")
        .expect_err("missing model should fail");
    assert!(error.to_string().contains("missing:model"));
}

#[test]
fn context_uses_index_matches_and_caps_file_content() {
    let root = unique_temp_dir("context");
    let small = root.join("Roadmap.md");
    let oversized = root.join("Roadmap-Long.md");
    let binary = root.join("Roadmap.bin");
    fs::write(&small, "fast context").expect("write small file");
    fs::write(&oversized, "0123456789abcdef").expect("write oversized file");
    fs::write(&binary, b"abc\0def").expect("write binary file");

    let store = IndexStore::open_in_memory().expect("open index");
    store
        .upsert(&indexed_file("file:small", "Roadmap.md", &small))
        .expect("index small");
    store
        .upsert(&indexed_file("file:long", "Roadmap-Long.md", &oversized))
        .expect("index oversized");
    store
        .upsert(&indexed_file("file:bin", "Roadmap.bin", &binary))
        .expect("index binary");

    let context = build_index_context(
        &store,
        "roadmap",
        ContextLimits {
            max_files: 5,
            max_file_bytes: 12,
            max_context_bytes: 24,
        },
    )
    .expect("build context");

    let small_entry = context
        .entries
        .iter()
        .find(|entry| entry.title == "Roadmap.md")
        .expect("small entry");
    assert_eq!(small_entry.content.as_deref(), Some("fast context"));
    assert!(small_entry.metadata_only_reason.is_none());

    let oversized_entry = context
        .entries
        .iter()
        .find(|entry| entry.title == "Roadmap-Long.md")
        .expect("oversized entry");
    assert_eq!(oversized_entry.content, None);
    assert_eq!(
        oversized_entry.metadata_only_reason.as_deref(),
        Some("file exceeds per-file content cap")
    );

    let binary_entry = context
        .entries
        .iter()
        .find(|entry| entry.title == "Roadmap.bin")
        .expect("binary entry");
    assert_eq!(binary_entry.content, None);
    assert_eq!(
        binary_entry.metadata_only_reason.as_deref(),
        Some("binary or non-UTF-8 file")
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn idle_shutdown_applies_only_to_owned_processes() {
    assert!(should_stop_owned_process(true, 1_000, 1_121, 120));
    assert!(!should_stop_owned_process(true, 1_000, 1_119, 120));
    assert!(!should_stop_owned_process(false, 1_000, 1_500, 120));
}

fn indexed_file(id: &str, title: &str, path: &std::path::Path) -> IndexedItem {
    IndexedItem {
        id: id.to_string(),
        title: title.to_string(),
        path: path.display().to_string(),
        kind: SearchResultKind::File,
        modified_unix_seconds: 1,
    }
}

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("winspot-fastflowlm-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp dir");
    root
}
