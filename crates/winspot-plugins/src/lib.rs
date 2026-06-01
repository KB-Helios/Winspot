use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
}

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
        let mut registry = Self::default();
        for manifest in built_in_plugin_manifests() {
            // Built-in manifests are authored in-tree and always valid; surface a
            // loud message if that ever stops being true rather than silently
            // dropping a built-in.
            if let Err(error) = registry.register(manifest, None) {
                eprintln!("winspot-plugins: built-in manifest rejected: {error}");
            }
        }
        registry
    }

    /// Loads every `*.json` manifest from `path` into a fresh registry. Missing
    /// directories and individual malformed manifests are tolerated (logged, not
    /// fatal) so a bad plugin can never prevent the daemon from starting.
    pub fn load_dir(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut registry = Self::default();
        registry.load_dir_into(path)?;
        Ok(registry)
    }

    /// Merges every `*.json` manifest from `path` into this registry, preserving
    /// already-registered plugins (e.g. built-ins). Invalid or duplicate
    /// manifests are skipped with a diagnostic.
    pub fn load_dir_into(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let Ok(entries) = fs::read_dir(path) else {
            return Ok(());
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Err(err) = self.load_manifest(&path) {
                eprintln!(
                    "winspot-plugins: failed to load plugin manifest at {}: {err:?}",
                    path.display(),
                );
            }
        }

        Ok(())
    }

    /// Reads, validates, and registers a single manifest file.
    pub fn load_manifest(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let manifest: PluginManifest = serde_json::from_str(&fs::read_to_string(path)?)?;
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
        manifest.validate()?;
        let id = manifest.id.trim().to_string();
        if self.plugins.contains_key(&id) {
            anyhow::bail!("duplicate plugin id {id:?}");
        }
        self.plugins.insert(
            id,
            LoadedPlugin {
                manifest,
                manifest_path,
                loaded_at: SystemTime::now(),
            },
        );
        Ok(())
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

    pub fn internal_results(&self, query: &str) -> Vec<SearchResult> {
        let normalized = query.trim().to_lowercase();
        self.enabled_manifests()
            .filter(|manifest| manifest.name.to_lowercase().contains(&normalized))
            .map(plugin_search_result)
            .collect()
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
