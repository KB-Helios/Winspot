use anyhow::Context;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::ServerOptions,
};
use winspot_core::{IpcEnvelope, IpcPayload, ResultBatch};
use winspot_search::engine::SearchEngine;

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
    let engine = SearchEngine::default();
    loop {
        serve_pipe_once(config.clone(), &engine).await?;
    }
}

pub async fn serve_pipe_once(config: PipeConfig, engine: &SearchEngine) -> anyhow::Result<()> {
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

    let response = handle_line(line.trim(), engine)?;
    let mut response_json = serde_json::to_string(&response).context("serialize response")?;
    response_json.push('\n');
    reader
        .get_mut()
        .write_all(response_json.as_bytes())
        .await
        .context("write IPC response")?;
    reader
        .get_mut()
        .flush()
        .await
        .context("flush IPC response")?;

    Ok(())
}

fn handle_line(line: &str, engine: &SearchEngine) -> anyhow::Result<IpcEnvelope> {
    let envelope: IpcEnvelope = serde_json::from_str(line).context("decode IPC envelope")?;
    let request_id = envelope.request_id.clone();

    match envelope.payload {
        IpcPayload::SearchStarted(search) => {
            let batch = ResultBatch {
                query_id: search.query_id,
                is_final: true,
                results: engine.search(&search.text, 20),
            };
            Ok(IpcEnvelope::request(
                request_id,
                IpcPayload::ResultBatch(batch),
            ))
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
