use std::{
    env,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
};
use winspot_actions::{ActionExecutor, ActionPolicy};
use winspot_core::{
    BackendError, HelloAccepted, IpcEnvelope, IpcPayload, MAX_JSON_LINE_BYTES,
    MAX_PROTOCOL_VERSION, MIN_PROTOCOL_VERSION, PreviewChunk, PreviewReady, PreviewRequested,
    ResultBatch, SearchCompleted,
};
use winspot_preview::{DefaultPreviewProvider, PreviewProvider};
use winspot_search::{
    engine::SearchEngine,
    providers::{
        BuiltInPluginProvider, BuiltinCommandProvider, CalculatorProvider, FileSystemProvider,
        RefreshableProvider, RunningProcessProvider, StartMenuAppProvider, UnitConversionProvider,
        WindowsSettingsProvider,
    },
    usage::{UsageEvent, UsageSnapshot, UsageStore},
};

/// How long a collected static-candidate snapshot is reused before the engine
/// re-runs its providers, so processes/files/apps stay reasonably fresh without
/// re-collecting on every keystroke.
const CANDIDATE_TTL_SECONDS: u64 = 5;

#[derive(Debug, Clone)]
pub struct PipeConfig {
    pub pipe_name: String,
    pub usage_log_path: Option<PathBuf>,
}

impl Default for PipeConfig {
    fn default() -> Self {
        Self {
            pipe_name: r"\\.\pipe\winspot-dev".to_string(),
            usage_log_path: default_usage_log_path(),
        }
    }
}

pub async fn serve_forever(config: PipeConfig) -> anyhow::Result<()> {
    let engine = build_search_engine(&config).context("build search engine")?;

    // Reserve the first pipe instance as a single-instance guard: if another
    // daemon already owns this pipe name, `first_pipe_instance(true)` fails and
    // we exit quietly instead of leaving a redundant daemon running.
    let first = match ServerOptions::new()
        .first_pipe_instance(true)
        .create(&config.pipe_name)
    {
        Ok(server) => server,
        Err(error) => {
            eprintln!(
                "winspot-daemon: another instance already owns {} ({error}); exiting",
                config.pipe_name
            );
            return Ok(());
        }
    };
    if let Err(error) = serve_connection(first, &engine, &config).await {
        eprintln!("winspot-daemon: connection error: {error:?}");
    }

    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(&config.pipe_name)
            .with_context(|| format!("create named pipe {}", config.pipe_name))?;
        // A single client connection failing (abrupt disconnect, broken pipe,
        // malformed payload) must not take down the daemon: log it and keep
        // accepting subsequent connections.
        if let Err(error) = serve_connection(server, &engine, &config).await {
            eprintln!("winspot-daemon: connection error: {error:?}");
        }
    }
}

pub fn build_search_engine(config: &PipeConfig) -> anyhow::Result<SearchEngine> {
    let usage = match &config.usage_log_path {
        Some(path) => UsageStore::new(path.clone())
            .load_snapshot()
            .with_context(|| format!("load usage log {}", path.display()))?,
        None => UsageSnapshot::default(),
    };

    Ok(SearchEngine::from_refreshable_providers_with_usage(
        default_search_providers(),
        usage,
        CANDIDATE_TTL_SECONDS,
        Arc::new(current_unix_seconds),
    )
    .with_dynamic_provider(Arc::new(CalculatorProvider))
    .with_dynamic_provider(Arc::new(UnitConversionProvider))
    .with_dynamic_provider(Arc::new(BuiltInPluginProvider)))
}

pub async fn serve_pipe_once(config: PipeConfig, engine: &SearchEngine) -> anyhow::Result<()> {
    let server = ServerOptions::new()
        .first_pipe_instance(false)
        .create(&config.pipe_name)
        .with_context(|| format!("create named pipe {}", config.pipe_name))?;
    serve_connection(server, engine, &config).await
}

async fn serve_connection(
    server: NamedPipeServer,
    engine: &SearchEngine,
    config: &PipeConfig,
) -> anyhow::Result<()> {
    server
        .connect()
        .await
        .with_context(|| format!("connect named pipe {}", config.pipe_name))?;

    let mut reader = BufReader::new(server);
    let mut line = String::new();
    while reader.read_line(&mut line).await.context("read IPC line")? > 0 {
        if line.len() > MAX_JSON_LINE_BYTES {
            write_envelope(
                reader.get_mut(),
                &IpcEnvelope::request(
                    "oversized-payload",
                    IpcPayload::Error(BackendError {
                        code: "payload_too_large".to_string(),
                        message: format!("IPC payload exceeds {MAX_JSON_LINE_BYTES} bytes"),
                        retryable: false,
                    }),
                ),
            )
            .await?;
            line.clear();
            break;
        }

        let (responses, close_after) = handle_line(line.trim(), engine, config)?;
        for response in responses {
            write_envelope(reader.get_mut(), &response).await?;
        }
        line.clear();
        if close_after {
            break;
        }
    }

    Ok(())
}

fn handle_line(
    line: &str,
    engine: &SearchEngine,
    config: &PipeConfig,
) -> anyhow::Result<(Vec<IpcEnvelope>, bool)> {
    let envelope: IpcEnvelope = serde_json::from_str(line).context("decode IPC envelope")?;
    let request_id = envelope.request_id.clone();

    match envelope.payload {
        IpcPayload::Hello(hello) => {
            let version = hello
                .max_protocol_version
                .min(MAX_PROTOCOL_VERSION)
                .max(MIN_PROTOCOL_VERSION);
            Ok((
                vec![IpcEnvelope::request(
                    request_id,
                    IpcPayload::HelloAccepted(HelloAccepted {
                        protocol_version: version,
                        max_json_line_bytes: MAX_JSON_LINE_BYTES,
                        server_name: "winspot-daemon".to_string(),
                    }),
                )],
                false,
            ))
        }
        IpcPayload::SearchStarted(search) => {
            let results = engine.search(&search.text, 20);
            let batch = ResultBatch {
                query_id: search.query_id.clone(),
                is_final: true,
                batch_index: 0,
                results,
            };
            Ok((
                vec![
                    IpcEnvelope::request(request_id.clone(), IpcPayload::ResultBatch(batch)),
                    IpcEnvelope::request(
                        request_id,
                        IpcPayload::SearchCompleted(SearchCompleted {
                            query_id: search.query_id,
                            cancelled: false,
                        }),
                    ),
                ],
                true,
            ))
        }
        IpcPayload::ActionRequested(action) => {
            let completed = handle_action(action, engine, config);
            Ok((
                vec![IpcEnvelope::request(
                    request_id,
                    IpcPayload::ActionCompleted(completed),
                )],
                true,
            ))
        }
        IpcPayload::PreviewRequested(preview) => {
            let chunk = build_preview_chunk(&preview);
            let ready = build_preview(preview);
            Ok((
                vec![
                    IpcEnvelope::request(request_id.clone(), IpcPayload::PreviewChunk(chunk)),
                    IpcEnvelope::request(request_id, IpcPayload::PreviewReady(ready)),
                ],
                true,
            ))
        }
        IpcPayload::CancelRequest(cancel) => Ok((
            vec![IpcEnvelope::request(
                request_id,
                IpcPayload::SearchCompleted(SearchCompleted {
                    query_id: cancel.request_to_cancel,
                    cancelled: true,
                }),
            )],
            true,
        )),
        _ => Ok((
            vec![IpcEnvelope::request(
                request_id,
                IpcPayload::Error(BackendError {
                    code: "unsupported_payload".to_string(),
                    message: "Unsupported IPC payload for this daemon.".to_string(),
                    retryable: false,
                }),
            )],
            true,
        )),
    }
}

async fn write_envelope(
    server: &mut NamedPipeServer,
    envelope: &IpcEnvelope,
) -> anyhow::Result<()> {
    let mut response_json = serde_json::to_string(envelope).context("serialize response")?;
    response_json.push('\n');
    server
        .write_all(response_json.as_bytes())
        .await
        .context("write IPC response")?;
    server.flush().await.context("flush IPC response")?;
    Ok(())
}

fn build_preview_chunk(preview: &PreviewRequested) -> PreviewChunk {
    PreviewChunk {
        preview_id: preview.preview_id.clone(),
        title: preview.result.title.clone(),
        body: "Loading preview".to_string(),
        is_final: false,
    }
}

fn build_preview(preview: PreviewRequested) -> PreviewReady {
    let payload = DefaultPreviewProvider.preview(&preview.result);
    PreviewReady {
        preview_id: preview.preview_id,
        title: payload.title().to_string(),
        body: payload.body().to_string(),
        is_final: true,
    }
}

fn handle_action(
    action: winspot_core::ActionRequested,
    engine: &SearchEngine,
    config: &PipeConfig,
) -> winspot_core::ActionCompleted {
    let now = current_unix_seconds();
    let completed = ActionExecutor::new(ActionPolicy::allow_all_local()).execute(action.clone());
    if !completed.succeeded {
        return completed;
    }

    match record_usage(&action, now, config).map(|_| {
        engine.record_usage(&action.result_id, now);
        completed.message.clone()
    }) {
        Ok(message) => winspot_core::ActionCompleted {
            action_id: action.action_id,
            succeeded: true,
            message,
        },
        Err(error) => winspot_core::ActionCompleted {
            action_id: action.action_id,
            succeeded: false,
            message: error.to_string(),
        },
    }
}

fn record_usage(
    action: &winspot_core::ActionRequested,
    now_unix_seconds: u64,
    config: &PipeConfig,
) -> anyhow::Result<()> {
    let Some(path) = &config.usage_log_path else {
        return Ok(());
    };

    UsageStore::new(path.clone())
        .record(UsageEvent {
            result_id: action.result_id.clone(),
            timestamp_unix_seconds: now_unix_seconds,
        })
        .with_context(|| format!("record usage for {}", action.result_id))
}

fn default_search_providers() -> Vec<Arc<dyn RefreshableProvider>> {
    vec![
        Arc::new(BuiltinCommandProvider),
        Arc::new(WindowsSettingsProvider),
        Arc::new(RunningProcessProvider),
        Arc::new(StartMenuAppProvider::default()),
        Arc::new(FileSystemProvider::default()),
    ]
}

fn default_usage_log_path() -> Option<PathBuf> {
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            if directory.join("Winspot.portable").exists() {
                return Some(directory.join("data").join("usage-events.jsonl"));
            }
        }
    }

    env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot\\usage-events.jsonl"))
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
