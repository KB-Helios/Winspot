use std::{env, path::PathBuf, process::ExitCode};

use winspot_plugins::{PluginValidationReport, PluginValidationSeverity};

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> anyhow::Result<ExitCode> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    match command {
        "validate" => validate(&args[1..]),
        _ => anyhow::bail!(
            "usage: winspot-pluginctl validate --plugins-dir <path> [--format human|json]"
        ),
    }
}

fn validate(args: &[String]) -> anyhow::Result<ExitCode> {
    let mut plugins_dir = None;
    let mut format = OutputFormat::Human;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--plugins-dir" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("--plugins-dir requires a path");
                };
                plugins_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--format" => {
                let Some(value) = args.get(index + 1) else {
                    anyhow::bail!("--format requires human or json");
                };
                format = OutputFormat::parse(value)?;
                index += 2;
            }
            other => anyhow::bail!("unknown argument {other:?}"),
        }
    }

    let plugins_dir = plugins_dir.ok_or_else(|| anyhow::anyhow!("--plugins-dir is required"))?;
    if !plugins_dir.is_dir() {
        anyhow::bail!(
            "--plugins-dir does not exist or is not a directory: {}",
            plugins_dir.display()
        );
    }

    let (mut registry, _builtins_report) =
        winspot_plugins::PluginRegistry::with_built_ins_with_report();
    let report = registry.load_dir_into_with_report(&plugins_dir)?;

    match format {
        OutputFormat::Human => println!("{}", human_report(&report)),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    Ok(if report.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            other => anyhow::bail!("unsupported output format {other:?}"),
        }
    }
}

fn human_report(report: &PluginValidationReport) -> String {
    let (warnings, errors) = issue_counts(report);
    let mut lines = vec![
        "Winspot plugin validation".to_string(),
        format!("accepted: {}", report.accepted_count()),
        format!("disabled: {}", report.disabled_count()),
        format!("rejected: {}", report.rejected_count()),
        format!("warnings: {warnings}"),
        format!("errors: {errors}"),
    ];

    for entry in &report.entries {
        if entry.issues.is_empty() {
            continue;
        }
        let label = entry
            .manifest_path
            .as_deref()
            .or(entry.id.as_deref())
            .unwrap_or("<unknown>");
        lines.push(format!("- {label} [{:?}]", entry.status));
        for issue in &entry.issues {
            lines.push(format!(
                "  - {:?} {:?} {}: {}",
                issue.severity, issue.stage, issue.code, issue.message
            ));
        }
    }

    lines.join("\n")
}

fn issue_counts(report: &PluginValidationReport) -> (usize, usize) {
    let mut warnings = 0;
    let mut errors = 0;
    for issue in report.entries.iter().flat_map(|entry| &entry.issues) {
        match issue.severity {
            PluginValidationSeverity::Warning => warnings += 1,
            PluginValidationSeverity::Error => errors += 1,
        }
    }
    (warnings, errors)
}
