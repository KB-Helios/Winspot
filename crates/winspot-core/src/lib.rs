pub mod ipc;
pub mod search;

pub use ipc::{IpcEnvelope, IpcPayload, ResultBatch, SearchStarted};
pub use search::{ActionKind, SearchResult, SearchResultKind};
