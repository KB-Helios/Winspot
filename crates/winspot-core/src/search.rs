use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchResultKind {
    App,
    File,
    Folder,
    Command,
    Setting,
    Plugin,
    Process,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionKind {
    Open,
    Copy,
    RunCommand,
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
}
