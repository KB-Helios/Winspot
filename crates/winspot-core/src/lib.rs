pub mod ipc;
pub mod search;
pub mod security;

pub use ipc::{
    ActionCompleted, ActionRequested, BackendError, CancelRequest, Hello, HelloAccepted,
    IpcEnvelope, IpcPayload, MAX_JSON_LINE_BYTES, MAX_PROTOCOL_VERSION, MIN_PROTOCOL_VERSION,
    PreviewChunk, PreviewReady, PreviewRequested, ResultBatch, SearchCompleted, SearchStarted,
};
pub use search::{ActionCapability, ActionDescriptor, ActionKind, SearchResult, SearchResultKind};
pub use security::{OpenTarget, OpenTargetError, classify_open_target};
