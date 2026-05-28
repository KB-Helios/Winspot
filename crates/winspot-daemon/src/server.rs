use anyhow::Context;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::ServerOptions,
};
use winspot_core::{
    ActionKind, IpcEnvelope, IpcPayload, ResultBatch, SearchResult, SearchResultKind,
};

#[derive(Debug, Clone)]
pub struct PipeConfig {
    pub pipe_name: String,
}

impl Default for PipeConfig {
    fn default() -> Self {
        Self {
            pipe_name: r"\\.\pipe\winspot-dev".to_string(),
        }
    }
}

pub async fn serve_forever(config: PipeConfig) -> anyhow::Result<()> {
    loop {
        serve_pipe_once(config.clone()).await?;
    }
}

pub async fn serve_pipe_once(config: PipeConfig) -> anyhow::Result<()> {
    let server = ServerOptions::new()
        .first_pipe_instance(false)
        .create(&config.pipe_name)
        .with_context(|| format!("create named pipe {}", config.pipe_name))?;

    server
        .connect()
        .await
        .with_context(|| format!("connect named pipe {}", config.pipe_name))?;

    let mut reader = BufReader::new(server);
    let mut line = String::new();
    reader.read_line(&mut line).await.context("read IPC line")?;

    let response = handle_line(line.trim())?;
    let mut response_json = serde_json::to_string(&response).context("serialize response")?;
    response_json.push('\n');
    reader
        .get_mut()
        .write_all(response_json.as_bytes())
        .await
        .context("write IPC response")?;
    reader.get_mut().flush().await.context("flush IPC response")?;

    Ok(())
}

fn handle_line(line: &str) -> anyhow::Result<IpcEnvelope> {
    let envelope: IpcEnvelope = serde_json::from_str(line).context("decode IPC envelope")?;
    let request_id = envelope.request_id.clone();

    match envelope.payload {
        IpcPayload::SearchStarted(search) => {
            let batch = ResultBatch {
                query_id: search.query_id,
                is_final: true,
                results: mock_results(&search.text),
            };
            Ok(IpcEnvelope::request(request_id, IpcPayload::ResultBatch(batch)))
        }
        _ => Ok(IpcEnvelope::request(
            request_id,
            IpcPayload::Error(winspot_core::ipc::IpcError {
                code: "unsupported_payload".to_string(),
                message: "Only SearchStarted is supported in the first spine.".to_string(),
            }),
        )),
    }
}

fn mock_results(query: &str) -> Vec<SearchResult> {
    let normalized = query.trim().to_lowercase();
    let mut results = vec![
        SearchResult {
            id: "app:notepad".to_string(),
            title: "Notepad".to_string(),
            subtitle: "App result from Rust daemon".to_string(),
            kind: SearchResultKind::App,
            score: 100.0,
            primary_action: ActionKind::Open,
        },
        SearchResult {
            id: "command:calculator".to_string(),
            title: "Calculator".to_string(),
            subtitle: "Built-in command placeholder".to_string(),
            kind: SearchResultKind::Command,
            score: 80.0,
            primary_action: ActionKind::RunCommand,
        },
    ];

    if !normalized.is_empty() {
        results.retain(|result| result.title.to_lowercase().contains(&normalized));
    }

    if results.is_empty() {
        results.push(SearchResult {
            id: "command:search-web".to_string(),
            title: format!("Search for {query}"),
            subtitle: "Fallback command result".to_string(),
            kind: SearchResultKind::Command,
            score: 10.0,
            primary_action: ActionKind::RunCommand,
        });
    }

    results
}
