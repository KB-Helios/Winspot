use serde::{Deserialize, Serialize};

fn default_actions() -> Vec<ActionDescriptor> {
    Vec::new()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchResultKind {
    App,
    File,
    Folder,
    Command,
    Setting,
    Plugin,
    Process,
    BrowserHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKind {
    Open,
    Copy,
    RunCommand,
    OpenContainingFolder,
    CopyPath,
    KillProcess,
    PluginCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionCapability {
    ClipboardWrite,
    FilesystemRead,
    FilesystemWrite,
    ProcessExecution,
    ProcessInspection,
    ShellExecution,
    PluginExecution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDescriptor {
    pub id: String,
    pub label: String,
    pub kind: ActionKind,
    #[serde(default)]
    pub capabilities: Vec<ActionCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: SearchResultKind,
    pub score: f32,
    pub primary_action: ActionKind,
    #[serde(default = "default_actions")]
    pub actions: Vec<ActionDescriptor>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub icon_hint: Option<String>,
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            subtitle: String::new(),
            kind: SearchResultKind::Command,
            score: 0.0,
            primary_action: ActionKind::Open,
            actions: Vec::new(),
            source: None,
            icon_hint: None,
        }
    }
}

impl SearchResult {
    pub fn ensure_primary_action_descriptor(&mut self) {
        if self
            .actions
            .iter()
            .any(|action| action.kind == self.primary_action)
        {
            return;
        }

        self.actions.insert(
            0,
            ActionDescriptor {
                id: format!("{:?}", self.primary_action).to_lowercase(),
                label: format!("{:?}", self.primary_action),
                kind: self.primary_action.clone(),
                capabilities: Vec::new(),
            },
        );
    }
}
