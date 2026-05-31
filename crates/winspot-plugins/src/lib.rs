use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use winspot_core::{ActionCapability, ActionKind, SearchResult, SearchResultKind};

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

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub manifest_path: PathBuf,
    pub loaded_at: SystemTime,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
}

impl PluginRegistry {
    pub fn load_dir(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut registry = Self::default();
        let Ok(entries) = fs::read_dir(path) else {
            return Ok(registry);
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            registry.load_manifest(path)?;
        }

        Ok(registry)
    }

    pub fn load_manifest(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let manifest: PluginManifest = serde_json::from_str(&fs::read_to_string(path)?)?;
        self.plugins.insert(
            manifest.id.clone(),
            LoadedPlugin {
                manifest,
                manifest_path: path.to_path_buf(),
                loaded_at: SystemTime::now(),
            },
        );
        Ok(())
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
            .map(|manifest| SearchResult {
                id: format!("plugin:{}", manifest.id),
                title: manifest.name.clone(),
                subtitle: format!("Internal plugin ({})", manifest.id),
                kind: SearchResultKind::Plugin,
                score: 0.0,
                primary_action: ActionKind::PluginCommand,
                actions: Vec::new(),
                source: Some("plugin".to_string()),
                icon_hint: None,
            })
            .collect()
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
