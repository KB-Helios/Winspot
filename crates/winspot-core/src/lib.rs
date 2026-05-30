pub mod ipc;
pub mod search;

pub use ipc::{
    ActionCompleted, ActionRequested, IpcEnvelope, IpcPayload, PreviewReady, PreviewRequested,
    ResultBatch, SearchStarted,
};
pub use search::{ActionKind, SearchResult, SearchResultKind};
