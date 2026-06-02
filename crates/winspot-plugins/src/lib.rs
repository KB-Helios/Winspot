use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use winspot_core::{ActionCapability, ActionKind, SearchResult, SearchResultKind};

/// Maximum length for a plugin `id`. Ids double as result-id fragments
/// (`plugin:<id>`) and on-disk identifiers, so we keep them short and bounded.
pub const MAX_PLUGIN_ID_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<ActionCapability>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Reasons a [`PluginManifest`] can be rejected before it is trusted. Keeping
/// these typed lets the registry and tests distinguish failure modes instead of
/// matching on opaque strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginValidationError {
    /// `id` was empty or whitespace-only.
    EmptyId,
    /// `id` exceeded [`MAX_PLUGIN_ID_LENGTH`] or contained characters outside
    /// the allowed slug set (`[a-z0-9]` plus `-` and `_`, starting alphanumeric).
    InvalidId { id: String },
    /// `name` was empty or whitespace-only.
    EmptyName { id: String },
    /// The same capability was declared more than once.
    DuplicateCapability {
        id: String,
        capability: ActionCapability,
    },
}

impl fmt::Display for PluginValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(formatter, "plugin id must not be empty"),
            Self::InvalidId { id } => write!(
                formatter,
                "plugin id {id:?} is invalid: use up to {MAX_PLUGIN_ID_LENGTH} characters of \
                 lowercase letters, digits, '-' or '_', starting with a letter or digit"
            ),
            Self::EmptyName { id } => {
                write!(formatter, "plugin {id:?} must declare a non-empty name")
            }
            Self::DuplicateCapability { id, capability } => write!(
                formatter,
                "plugin {id:?} declares capability {capability:?} more than once"
            ),
        }
    }
}

impl std::error::Error for PluginValidationError {}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub manifest_path: Option<PathBuf>,
    pub loaded_at: SystemTime,
    pub source: PluginSource,
    pub trusted: bool,
    pub normalized_name: String,
    executable_action: bool,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginSource {
    BuiltIn,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginValidationSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginValidationStage {
    Discovery,
    Parse,
    Manifest,
    Policy,
    Registry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginValidationStatus {
    Accepted,
    Disabled,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginValidationIssue {
    pub severity: PluginValidationSeverity,
    pub stage: PluginValidationStage,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginValidationEntry {
    pub id: Option<String>,
    pub name: Option<String>,
    pub manifest_path: Option<String>,
    pub source: PluginSource,
    pub status: PluginValidationStatus,
    pub trusted: bool,
    pub issues: Vec<PluginValidationIssue>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginValidationReport {
    pub entries: Vec<PluginValidationEntry>,
}

impl PluginValidationReport {
    pub fn extend(&mut self, mut other: Self) {
        self.entries.append(&mut other.entries);
    }

    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|entry| {
            entry
                .issues
                .iter()
                .any(|issue| issue.severity == PluginValidationSeverity::Error)
        })
    }

    pub fn has_warnings(&self) -> bool {
        self.entries.iter().any(|entry| {
            entry
                .issues
                .iter()
                .any(|issue| issue.severity == PluginValidationSeverity::Warning)
        })
    }

    pub fn accepted_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == PluginValidationStatus::Accepted)
            .count()
    }

    pub fn rejected_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == PluginValidationStatus::Rejected)
            .count()
    }

    pub fn disabled_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status == PluginValidationStatus::Disabled)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginActionAuthorizationError {
    MalformedId,
    UnknownPlugin { id: String },
    Disabled { id: String },
    Untrusted { id: String },
    NoExecutableAction { id: String },
}

impl fmt::Display for PluginActionAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedId => write!(formatter, "malformed plugin id"),
            Self::UnknownPlugin { id } => write!(formatter, "plugin '{id}' is not registered"),
            Self::Disabled { id } => write!(formatter, "plugin '{id}' is disabled"),
            Self::Untrusted { id } => write!(formatter, "plugin '{id}' is not trusted"),
            Self::NoExecutableAction { id } => {
                write!(formatter, "plugin '{id}' has no executable action")
            }
        }
    }
}

impl std::error::Error for PluginActionAuthorizationError {}

impl PluginManifest {
    /// Validates the manifest's shape. Called before a manifest is registered so
    /// that malformed-but-deserializable manifests are rejected rather than
    /// surfacing broken results to the user.
    pub fn validate(&self) -> Result<(), PluginValidationError> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(PluginValidationError::EmptyId);
        }
        if !is_valid_plugin_id(id) {
            return Err(PluginValidationError::InvalidId { id: id.to_string() });
        }
        if self.name.trim().is_empty() {
            return Err(PluginValidationError::EmptyName { id: id.to_string() });
        }

        let mut seen = Vec::with_capacity(self.capabilities.len());
        for capability in &self.capabilities {
            if seen.contains(capability) {
                return Err(PluginValidationError::DuplicateCapability {
                    id: id.to_string(),
                    capability: capability.clone(),
                });
            }
            seen.push(capability.clone());
        }

        Ok(())
    }
}

impl PluginRegistry {
    /// Builds a registry pre-seeded with the built-in plugin identities. This is
    /// the canonical starting point for the daemon: user manifests are then
    /// merged on top via [`PluginRegistry::load_dir_into`].
    pub fn with_built_ins() -> Self {
        let (registry, report) = Self::with_built_ins_with_report();
        log_report_errors(&report);
        registry
    }

    pub fn with_built_ins_with_report() -> (Self, PluginValidationReport) {
        let mut registry = Self::default();
        let mut report = PluginValidationReport::default();
        for manifest in built_in_plugin_manifests() {
            report.entries.push(registry.register_manifest_for_report(
                manifest,
                None,
                PluginSource::BuiltIn,
                true,
                Vec::new(),
            ));
        }
        (registry, report)
    }

    /// Loads every `*.json` manifest from `path` into a fresh registry. Missing
    /// directories and individual malformed manifests are tolerated (logged, not
    /// fatal) so a bad plugin can never prevent the daemon from starting.
    pub fn load_dir(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let (registry, report) = Self::load_dir_with_report(path)?;
        log_report_errors(&report);
        Ok(registry)
    }

    pub fn load_dir_with_report(
        path: impl AsRef<Path>,
    ) -> anyhow::Result<(Self, PluginValidationReport)> {
        let mut registry = Self::default();
        let report = registry.load_dir_into_with_report(path)?;
        Ok((registry, report))
    }

    /// Merges every `*.json` manifest from `path` into this registry, preserving
    /// already-registered plugins (e.g. built-ins). Invalid or duplicate
    /// manifests are skipped with a diagnostic.
    pub fn load_dir_into(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let report = self.load_dir_into_with_report(path)?;
        log_report_errors(&report);
        Ok(())
    }

    pub fn load_dir_into_with_report(
        &mut self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<PluginValidationReport> {
        let mut report = PluginValidationReport::default();
        let path = path.as_ref();
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(report);
            }
            Err(error) => {
                anyhow::bail!("failed to read directory {}: {error}", path.display());
            }
        };

        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .map(|extension| extension.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
            .collect();
        paths.sort();

        for path in paths {
            report.entries.push(self.load_manifest_for_report(&path));
        }

        Ok(report)
    }

    /// Reads, validates, and registers a single manifest file.
    pub fn load_manifest(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        let manifest: PluginManifest = serde_json::from_str(&content)?;
        self.register(manifest, Some(path.to_path_buf()))?;
        Ok(())
    }

    /// Validates `manifest` and inserts it, rejecting duplicate ids so that a
    /// user plugin can never silently shadow a built-in (or another plugin).
    pub fn register(
        &mut self,
        manifest: PluginManifest,
        manifest_path: Option<PathBuf>,
    ) -> anyhow::Result<()> {
        let entry = self.register_manifest_for_report(
            manifest,
            manifest_path,
            PluginSource::User,
            false,
            Vec::new(),
        );
        match entry.status {
            PluginValidationStatus::Accepted | PluginValidationStatus::Disabled => Ok(()),
            PluginValidationStatus::Rejected => anyhow::bail!(
                "{}",
                entry
                    .issues
                    .iter()
                    .find(|issue| issue.severity == PluginValidationSeverity::Error)
                    .map(|issue| issue.message.as_str())
                    .unwrap_or("plugin manifest rejected")
            ),
        }
    }

    /// All registered manifests (enabled or not), for diagnostics and UIs.
    pub fn manifests(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins.values().map(|plugin| &plugin.manifest)
    }

    pub fn enabled_manifests(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins
            .values()
            .map(|plugin| &plugin.manifest)
            .filter(|manifest| manifest.enabled)
    }

    pub fn plugin_has_executable_action(&self, id: &str) -> bool {
        self.plugins
            .get(id)
            .map(|plugin| plugin.manifest.enabled && plugin.trusted && plugin.executable_action)
            .unwrap_or(false)
    }

    pub fn ensure_plugin_command_allowed(
        &self,
        result_id: &str,
    ) -> Result<&PluginManifest, PluginActionAuthorizationError> {
        let plugin_id = parse_plugin_result_id(result_id)?;
        let plugin = self.plugins.get(plugin_id).ok_or_else(|| {
            PluginActionAuthorizationError::UnknownPlugin {
                id: plugin_id.to_string(),
            }
        })?;

        if !plugin.manifest.enabled {
            return Err(PluginActionAuthorizationError::Disabled {
                id: plugin_id.to_string(),
            });
        }
        if !plugin.trusted {
            return Err(PluginActionAuthorizationError::Untrusted {
                id: plugin_id.to_string(),
            });
        }
        if !plugin.executable_action {
            return Err(PluginActionAuthorizationError::NoExecutableAction {
                id: plugin_id.to_string(),
            });
        }

        Ok(&plugin.manifest)
    }

    pub fn internal_results(&self, query: &str) -> Vec<SearchResult> {
        let normalized = query.trim().to_lowercase();
        self.enabled_manifests()
            .filter(|manifest| manifest.name.to_lowercase().contains(&normalized))
            .map(plugin_search_result)
            .collect()
    }

    fn load_manifest_for_report(&mut self, path: &Path) -> PluginValidationEntry {
        let path_string = Some(path.display().to_string());
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                return rejected_entry(
                    PluginSource::User,
                    path_string,
                    None,
                    None,
                    PluginValidationStage::Discovery,
                    "manifest_read_failed",
                    format!("failed to read plugin manifest: {error}"),
                );
            }
        };

        let value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(error) => {
                return rejected_entry(
                    PluginSource::User,
                    path_string,
                    None,
                    None,
                    PluginValidationStage::Parse,
                    "manifest_parse_failed",
                    format!("failed to parse plugin manifest JSON: {error}"),
                );
            }
        };

        let mut issues = unknown_field_warnings(&value);
        let manifest: PluginManifest = match serde_json::from_value(value) {
            Ok(manifest) => manifest,
            Err(error) => {
                issues.push(error_issue(
                    PluginValidationStage::Parse,
                    "manifest_parse_failed",
                    format!("failed to decode plugin manifest: {error}"),
                ));
                return rejected_entry_from_issues(
                    PluginSource::User,
                    path_string,
                    None,
                    None,
                    issues,
                );
            }
        };

        issues.extend(user_policy_warnings(&manifest));
        self.register_manifest_for_report(
            manifest,
            Some(path.to_path_buf()),
            PluginSource::User,
            false,
            issues,
        )
    }

    fn register_manifest_for_report(
        &mut self,
        manifest: PluginManifest,
        manifest_path: Option<PathBuf>,
        source: PluginSource,
        trusted: bool,
        issues: Vec<PluginValidationIssue>,
    ) -> PluginValidationEntry {
        let id = Some(manifest.id.trim().to_string()).filter(|id| !id.is_empty());
        let name = Some(manifest.name.trim().to_string()).filter(|name| !name.is_empty());
        let manifest_path_string = manifest_path
            .as_ref()
            .map(|path| path.display().to_string());
        let mut entry = PluginValidationEntry {
            id: id.clone(),
            name,
            manifest_path: manifest_path_string,
            source,
            status: PluginValidationStatus::Accepted,
            trusted,
            issues,
        };

        if let Err(error) = manifest.validate() {
            entry.status = PluginValidationStatus::Rejected;
            entry.issues.push(error_issue(
                PluginValidationStage::Manifest,
                "invalid_manifest",
                error.to_string(),
            ));
            return entry;
        }

        let id = id.expect("validated manifest has non-empty id");
        if self.plugins.contains_key(&id) {
            entry.status = PluginValidationStatus::Rejected;
            entry.issues.push(error_issue(
                PluginValidationStage::Registry,
                "duplicate_plugin_id",
                format!("duplicate plugin id {id:?}"),
            ));
            return entry;
        }

        let executable_action =
            trusted && source == PluginSource::BuiltIn && is_builtin_executable(&id);
        entry.status = if manifest.enabled {
            PluginValidationStatus::Accepted
        } else {
            PluginValidationStatus::Disabled
        };
        self.plugins.insert(
            id,
            LoadedPlugin {
                normalized_name: manifest.name.trim().to_lowercase(),
                manifest,
                manifest_path,
                loaded_at: SystemTime::now(),
                source,
                trusted,
                executable_action,
            },
        );
        entry
    }
}

/// Builds the search result surfaced for a matching plugin manifest. Shared so
/// the daemon-side provider and the registry produce identical results.
pub fn plugin_search_result(manifest: &PluginManifest) -> SearchResult {
    SearchResult {
        id: format!("plugin:{}", manifest.id),
        title: manifest.name.clone(),
        subtitle: format!("Internal plugin ({})", manifest.id),
        kind: SearchResultKind::Plugin,
        score: 0.0,
        primary_action: ActionKind::PluginCommand,
        actions: Vec::new(),
        source: Some("plugin".to_string()),
        icon_hint: None,
    }
}

/// Returns `true` if `id` is a valid plugin identifier: 1..=[`MAX_PLUGIN_ID_LENGTH`]
/// characters of lowercase ASCII letters, digits, `-` or `_`, starting with a
/// letter or digit.
pub fn is_valid_plugin_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_PLUGIN_ID_LENGTH {
        return false;
    }

    let mut chars = id.chars();
    let first = chars.next().expect("non-empty id");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    id.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub fn parse_plugin_result_id(result_id: &str) -> Result<&str, PluginActionAuthorizationError> {
    let id = result_id
        .strip_prefix("plugin:")
        .ok_or(PluginActionAuthorizationError::MalformedId)?;

    if is_valid_plugin_id(id) {
        Ok(id)
    } else {
        Err(PluginActionAuthorizationError::MalformedId)
    }
}

pub fn built_in_plugin_manifests() -> Vec<PluginManifest> {
    vec![
        PluginManifest {
            id: "calculator".to_string(),
            name: "Calculator".to_string(),
            capabilities: vec![ActionCapability::ClipboardWrite],
            enabled: true,
        },
        PluginManifest {
            id: "terminal".to_string(),
            name: "Terminal".to_string(),
            capabilities: vec![ActionCapability::ProcessExecution],
            enabled: true,
        },
        PluginManifest {
            id: "clipboard".to_string(),
            name: "Clipboard".to_string(),
            capabilities: vec![ActionCapability::ClipboardWrite],
            enabled: true,
        },
        PluginManifest {
            id: "unit-conversion".to_string(),
            name: "Unit Conversion".to_string(),
            capabilities: vec![ActionCapability::ClipboardWrite],
            enabled: true,
        },
    ]
}

fn default_enabled() -> bool {
    true
}

fn unknown_field_warnings(value: &Value) -> Vec<PluginValidationIssue> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };

    object
        .keys()
        .filter(|key| !matches!(key.as_str(), "id" | "name" | "capabilities" | "enabled"))
        .map(|key| {
            warning_issue(
                PluginValidationStage::Parse,
                "unknown_field",
                format!("unknown plugin manifest field {key:?} is ignored"),
            )
        })
        .collect()
}

fn user_policy_warnings(manifest: &PluginManifest) -> Vec<PluginValidationIssue> {
    manifest
        .capabilities
        .iter()
        .filter(|capability| {
            matches!(
                capability,
                ActionCapability::PluginExecution
                    | ActionCapability::ProcessExecution
                    | ActionCapability::ShellExecution
            )
        })
        .map(|capability| {
            warning_issue(
                PluginValidationStage::Policy,
                "ignored_user_executable_capability",
                format!(
                    "user plugin {:?} declares executable capability {capability:?}, \
                     but user plugins are search-only in V1",
                    manifest.id
                ),
            )
        })
        .collect()
}

fn is_builtin_executable(id: &str) -> bool {
    matches!(id, "calculator" | "terminal")
}

fn warning_issue(
    stage: PluginValidationStage,
    code: impl Into<String>,
    message: impl Into<String>,
) -> PluginValidationIssue {
    PluginValidationIssue {
        severity: PluginValidationSeverity::Warning,
        stage,
        code: code.into(),
        message: message.into(),
    }
}

fn error_issue(
    stage: PluginValidationStage,
    code: impl Into<String>,
    message: impl Into<String>,
) -> PluginValidationIssue {
    PluginValidationIssue {
        severity: PluginValidationSeverity::Error,
        stage,
        code: code.into(),
        message: message.into(),
    }
}

fn rejected_entry(
    source: PluginSource,
    manifest_path: Option<String>,
    id: Option<String>,
    name: Option<String>,
    stage: PluginValidationStage,
    code: impl Into<String>,
    message: impl Into<String>,
) -> PluginValidationEntry {
    rejected_entry_from_issues(
        source,
        manifest_path,
        id,
        name,
        vec![error_issue(stage, code, message)],
    )
}

fn rejected_entry_from_issues(
    source: PluginSource,
    manifest_path: Option<String>,
    id: Option<String>,
    name: Option<String>,
    issues: Vec<PluginValidationIssue>,
) -> PluginValidationEntry {
    PluginValidationEntry {
        id,
        name,
        manifest_path,
        source,
        status: PluginValidationStatus::Rejected,
        trusted: false,
        issues,
    }
}

fn log_report_errors(report: &PluginValidationReport) {
    for entry in &report.entries {
        for issue in &entry.issues {
            if issue.severity == PluginValidationSeverity::Error {
                let location = entry
                    .manifest_path
                    .as_deref()
                    .or(entry.id.as_deref())
                    .unwrap_or("<unknown>");
                eprintln!(
                    "winspot-plugins: failed to load plugin manifest at {location}: {}",
                    issue.message
                );
            }
        }
    }
}
