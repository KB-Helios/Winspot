use std::fs;

use winspot_core::{ActionKind, SearchResultKind};
use winspot_fastflowlm::{
    ContextLimits, DEFAULT_MODEL_DISPLAY_NAME, FASTFLOWLM_RESULT_ID, FastFlowLmProvider,
    build_index_context, parse_fastflowlm_prompt, should_stop_owned_process,
    validate_installed_model_from_list_json, validate_models_response_json,
};
use winspot_index::{IndexStore, IndexedItem};
use winspot_search::providers::DynamicSearchProvider;

#[test]
fn parse_ai_query_with_explicit_prefixes_returns_prompt() {
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
fn fastflowlm_provider_with_ai_prefix_returns_lightweight_plugin_result() {
    let results = FastFlowLmProvider.search("ai summarize roadmap");

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

/// Ensures the provider returns no results for queries that do not begin with an explicit AI/ask prefix.
///
/// # Examples
///
/// ```
/// let results = FastFlowLmProvider::default().search("summarize roadmap");
/// assert!(results.is_empty());
/// ```
#[test]
fn fastflowlm_provider_without_ai_prefix_returns_no_results() {
    assert!(FastFlowLmProvider.search("summarize roadmap").is_empty());
    assert!(FastFlowLmProvider.search("ai").is_empty());
    assert!(FastFlowLmProvider.search("ask").is_empty());
}

#[test]
fn validate_installed_model_from_list_json_with_installed_model_succeeds() {
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
    assert!(!error.to_string().contains(DEFAULT_MODEL_DISPLAY_NAME));
}

#[test]
fn validate_models_response_json_with_unrelated_service_rejects_response() {
    let health = r#"{"data":[{"id":"gemma4-it:e2b"}]}"#;
    assert!(validate_models_response_json(health, "gemma4-it:e2b").is_ok());

    let wrong_model = validate_models_response_json(health, "missing:model")
        .expect_err("missing health model should fail");
    assert!(wrong_model.to_string().contains("missing:model"));

    let unrelated = validate_models_response_json(r#"{"ok":true}"#, "gemma4-it:e2b")
        .expect_err("unrelated service response should fail");
    assert!(unrelated.to_string().contains("/v1/models"));
}

#[test]
fn build_context_with_index_matches_caps_file_content() {
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
fn should_stop_owned_process_with_idle_process_applies_ownership_policy() {
    assert!(should_stop_owned_process(true, 1_000, 1_121, 120));
    assert!(!should_stop_owned_process(true, 1_000, 1_119, 120));
    assert!(!should_stop_owned_process(false, 1_000, 1_500, 120));
}

/// Creates an `IndexedItem` representing a file with the given `id`, `title`, and filesystem `path`.
///
/// The returned item has `kind` set to `SearchResultKind::File` and `modified_unix_seconds` set to `1`.
///
/// # Examples
///
/// ```
/// let path = std::path::Path::new("/tmp/Roadmap.md");
/// let item = indexed_file("id-123", "Roadmap", path);
/// assert_eq!(item.id, "id-123");
/// assert_eq!(item.title, "Roadmap");
/// assert_eq!(item.path, path.display().to_string());
/// assert_eq!(item.kind, SearchResultKind::File);
/// assert_eq!(item.modified_unix_seconds, 1);
/// ```
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
