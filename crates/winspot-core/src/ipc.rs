use serde::{Deserialize, Serialize};

use crate::search::{ActionKind, SearchResult};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcEnvelope {
    pub protocol_version: u16,
    pub request_id: String,
    pub payload: IpcPayload,
}

impl IpcEnvelope {
    pub fn request(request_id: impl Into<String>, payload: IpcPayload) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IpcPayload {
    SearchStarted(SearchStarted),
    ResultBatch(ResultBatch),
    ActionRequested(ActionRequested),
    ActionCompleted(ActionCompleted),
    PreviewRequested(PreviewRequested),
    PreviewReady(PreviewReady),
    Error(IpcError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchStarted {
    pub query_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultBatch {
    pub query_id: String,
    pub is_final: bool,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequested {
    pub action_id: String,
    pub result_id: String,
    pub title: String,
    pub primary_action: ActionKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCompleted {
    pub action_id: String,
    pub succeeded: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequested {
    pub preview_id: String,
    pub result: SearchResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReady {
    pub preview_id: String,
    pub title: String,
    pub body: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: String,
    pub message: String,
}
