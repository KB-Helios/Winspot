use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("winspot-plugins")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn pluginctl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_winspot-pluginctl"))
}

#[test]
fn validate_valid_fixture_as_json_succeeds() {
    let output = pluginctl()
        .args([
            "validate",
            "--plugins-dir",
            fixture_dir("valid").to_str().expect("fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run pluginctl");

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is JSON report");
    assert_eq!(report["entries"].as_array().expect("entries").len(), 1);
    assert_eq!(report["entries"][0]["status"], "Accepted");
}

#[test]
fn validate_invalid_fixture_exits_nonzero() {
    let output = pluginctl()
        .args([
            "validate",
            "--plugins-dir",
            fixture_dir("invalid").to_str().expect("fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run pluginctl");

    assert!(!output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is JSON report");
    let codes: Vec<&str> = report["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .flat_map(|entry| entry["issues"].as_array().expect("issues"))
        .map(|issue| issue["code"].as_str().expect("issue code"))
        .collect();

    assert!(codes.contains(&"manifest_parse_failed"));
    assert!(codes.contains(&"invalid_manifest"));
    assert!(codes.contains(&"duplicate_plugin_id"));
}

#[test]
fn validate_missing_plugins_dir_exits_with_usage_error() {
    let missing = fixture_dir("missing");
    let output = pluginctl()
        .args([
            "validate",
            "--plugins-dir",
            missing.to_str().expect("fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run pluginctl");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--plugins-dir does not exist or is not a directory")
    );
}

#[test]
fn validate_human_output_includes_counts_and_issue_codes() {
    let output = pluginctl()
        .args([
            "validate",
            "--plugins-dir",
            fixture_dir("invalid").to_str().expect("fixture path"),
            "--format",
            "human",
        ])
        .output()
        .expect("run pluginctl");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!output.status.success());
    assert!(stdout.contains("Winspot plugin validation"));
    assert!(stdout.contains("rejected:"));
    assert!(stdout.contains("manifest_parse_failed"));
}
