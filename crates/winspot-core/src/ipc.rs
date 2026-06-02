use serde::{Deserialize, Serialize};

use crate::search::{ActionKind, SearchResult};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MIN_PROTOCOL_VERSION: u16 = 1;
// The daemon only implements v1 semantics today, so advertise exactly what it
// supports instead of claiming a v2 it never negotiates.
pub const MAX_PROTOCOL_VERSION: u16 = 1;
pub const MAX_JSON_LINE_BYTES: usize = 1024 * 1024;

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
    Hello(Hello),
    HelloAccepted(HelloAccepted),
    SearchStarted(SearchStarted),
    ResultBatch(ResultBatch),
    SearchCompleted(SearchCompleted),
    CancelRequest(CancelRequest),
    ActionRequested(ActionRequested),
    ActionCompleted(ActionCompleted),
    PreviewRequested(PreviewRequested),
    PreviewChunk(PreviewChunk),
    PreviewReady(PreviewReady),
    PluginDiagnosticsRequested(PluginDiagnosticsRequested),
    PluginDiagnosticsReady(PluginDiagnosticsReady),
    Error(BackendError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub min_protocol_version: u16,
    pub max_protocol_version: u16,
    pub client_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAccepted {
    pub protocol_version: u16,
    pub max_json_line_bytes: usize,
    pub server_name: String,
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
    #[serde(default)]
    pub batch_index: u32,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchCompleted {
    pub query_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub request_to_cancel: String,
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
pub struct PreviewChunk {
    pub preview_id: String,
    pub title: String,
    pub body: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDiagnosticsRequested {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDiagnosticsReady {
    pub report: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}
