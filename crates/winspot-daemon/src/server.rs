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

use crate::pipe_security::PipeSecurity;
use winspot_core::{
    ActionKind, BackendError, HelloAccepted, IpcEnvelope, IpcPayload, MAX_JSON_LINE_BYTES,
    MAX_PROTOCOL_VERSION, MIN_PROTOCOL_VERSION, PluginDiagnosticsReady, PreviewChunk, PreviewReady,
    PreviewRequested, ResultBatch, SearchCompleted,
};
use winspot_fastflowlm::{
    FASTFLOWLM_RESULT_ID, FastFlowLmProvider, FastFlowLmService, FastFlowLmSettings,
    default_index_path, default_settings_path, load_settings_from_path,
};
use winspot_index::IndexStore;
use winspot_plugins::{PluginRegistry, PluginValidationReport};
use winspot_preview::{DefaultPreviewProvider, PreviewProvider};
use winspot_search::{
    engine::SearchEngine,
    providers::{
        BuiltinCommandProvider, CalculatorProvider, FileSystemProvider, PluginProvider,
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
    /// Directory scanned for user-authored plugin manifests (`*.json`). When
    /// present, valid manifests are merged on top of the built-in plugins.
    pub plugins_dir: Option<PathBuf>,
}

impl Default for PipeConfig {
    fn default() -> Self {
        Self {
            pipe_name: r"\\.\pipe\winspot-dev".to_string(),
            usage_log_path: default_usage_log_path(),
            plugins_dir: default_plugins_dir(),
        }
    }
}

#[derive(Clone)]
pub struct DaemonRuntime {
    pub engine: SearchEngine,
    pub plugin_registry: Arc<PluginRegistry>,
    pub plugin_validation_report: Arc<PluginValidationReport>,
    pub fastflowlm_service: Option<FastFlowLmService>,
}

impl DaemonRuntime {
    pub fn from_engine(engine: SearchEngine) -> Self {
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        Self {
            engine,
            plugin_registry: Arc::new(registry),
            plugin_validation_report: Arc::new(report),
            fastflowlm_service: None,
        }
    }
}

pub async fn serve_forever(config: PipeConfig) -> anyhow::Result<()> {
    let runtime = build_daemon_runtime(&config).context("build daemon runtime")?;

    // Reserve the first pipe instance as a single-instance guard: if another
    // daemon already owns this pipe name, `first_pipe_instance(true)` fails and
    // we exit quietly instead of leaving a redundant daemon running.
    let first = match create_secured_pipe(&config.pipe_name, true) {
        Ok(server) => server,
        Err(error) if is_first_pipe_instance_collision(&error) => {
            eprintln!(
                "winspot-daemon: another instance already owns {} ({error}); exiting",
                config.pipe_name
            );
            return Ok(());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("create first named pipe {}", config.pipe_name));
        }
    };
    if let Err(error) = serve_connection(first, &runtime, &config).await {
        eprintln!("winspot-daemon: connection error: {error:?}");
    }

    loop {
        let server = create_secured_pipe(&config.pipe_name, false)
            .with_context(|| format!("create named pipe {}", config.pipe_name))?;
        // A single client connection failing (abrupt disconnect, broken pipe,
        // malformed payload) must not take down the daemon: log it and keep
        // accepting subsequent connections.
        if let Err(error) = serve_connection(server, &runtime, &config).await {
            eprintln!("winspot-daemon: connection error: {error:?}");
        }
    }
}

/// Creates one named-pipe instance whose DACL is restricted to the current user
/// (and SYSTEM) and that rejects remote clients.
///
/// The security descriptor is built and dropped entirely within this synchronous
/// function: the kernel copies it into the pipe object at creation, so it does
/// not need to outlive the call (and never crosses an `.await`, keeping the
/// async server `Send`).
fn create_secured_pipe(name: &str, first_instance: bool) -> std::io::Result<NamedPipeServer> {
    let security = PipeSecurity::current_user_only()?;
    // SAFETY: `security` owns the SECURITY_ATTRIBUTES (and the descriptor it
    // points at) for the whole call, so the raw pointer stays valid until
    // `create_with_security_attributes_raw` returns.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first_instance)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                security.as_attributes_ptr() as *mut std::ffi::c_void,
            )
    }
}

pub fn build_daemon_runtime(config: &PipeConfig) -> anyhow::Result<DaemonRuntime> {
    let usage = match &config.usage_log_path {
        Some(path) => UsageStore::new(path.clone())
            .load_snapshot()
            .with_context(|| format!("load usage log {}", path.display()))?,
        None => UsageSnapshot::default(),
    };
    let (plugin_registry, plugin_validation_report) =
        build_plugin_registry(config.plugins_dir.as_deref());

    let fastflowlm_settings = load_fastflowlm_settings();

    let mut engine = SearchEngine::from_refreshable_providers_with_usage(
        default_search_providers(),
        usage,
        CANDIDATE_TTL_SECONDS,
        Arc::new(current_unix_seconds),
    )
    .with_dynamic_provider(Arc::new(CalculatorProvider))
    .with_dynamic_provider(Arc::new(UnitConversionProvider))
    .with_dynamic_provider(Arc::new(PluginProvider::new(Arc::clone(&plugin_registry))));
    if fastflowlm_settings.enabled {
        engine = engine.with_dynamic_provider(Arc::new(FastFlowLmProvider));
    }

    Ok(DaemonRuntime {
        engine,
        plugin_registry,
        plugin_validation_report,
        fastflowlm_service: build_fastflowlm_service(fastflowlm_settings),
    })
}

pub fn build_search_engine(config: &PipeConfig) -> anyhow::Result<SearchEngine> {
    build_daemon_runtime(config).map(|runtime| runtime.engine)
}

/// Builds the plugin registry the daemon serves from: the built-in plugin
/// identities plus any valid user manifests found in `plugins_dir`. A missing
/// directory or individual malformed manifest is tolerated so a bad plugin can
/// never stop the daemon from starting.
fn build_plugin_registry(
    plugins_dir: Option<&std::path::Path>,
) -> (Arc<PluginRegistry>, Arc<PluginValidationReport>) {
    let (mut registry, mut report) = PluginRegistry::with_built_ins_with_report();
    if let Some(dir) = plugins_dir
        && let Err(error) = registry
            .load_dir_into_with_report(dir)
            .map(|user_report| report.extend(user_report))
    {
        eprintln!(
            "winspot-daemon: failed to scan plugins directory {}: {error:?}",
            dir.display()
        );
    }
    (Arc::new(registry), Arc::new(report))
}

pub async fn serve_pipe_once(config: PipeConfig, engine: &SearchEngine) -> anyhow::Result<()> {
    let (plugin_registry, plugin_validation_report) =
        build_plugin_registry(config.plugins_dir.as_deref());
    let runtime = DaemonRuntime {
        engine: engine.clone(),
        plugin_registry,
        plugin_validation_report,
        fastflowlm_service: build_fastflowlm_service(load_fastflowlm_settings()),
    };
    serve_runtime_pipe_once(config, &runtime).await
}

pub async fn serve_runtime_pipe_once(
    config: PipeConfig,
    runtime: &DaemonRuntime,
) -> anyhow::Result<()> {
    let server = create_secured_pipe(&config.pipe_name, false)
        .with_context(|| format!("create named pipe {}", config.pipe_name))?;
    serve_connection(server, runtime, &config).await
}

async fn serve_connection(
    server: NamedPipeServer,
    runtime: &DaemonRuntime,
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

        let (responses, close_after) = handle_line(line.trim(), runtime, config)?;
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
    runtime: &DaemonRuntime,
    config: &PipeConfig,
) -> anyhow::Result<(Vec<IpcEnvelope>, bool)> {
    let envelope: IpcEnvelope = serde_json::from_str(line).context("decode IPC envelope")?;
    let request_id = envelope.request_id.clone();

    match envelope.payload {
        IpcPayload::Hello(hello) => {
            // Negotiate the highest version both sides support. If the client's
            // advertised range doesn't overlap ours, refuse instead of silently
            // "accepting" a version the daemon doesn't actually implement.
            if hello.max_protocol_version < MIN_PROTOCOL_VERSION
                || hello.min_protocol_version > MAX_PROTOCOL_VERSION
            {
                return Ok((
                    vec![IpcEnvelope::request(
                        request_id,
                        IpcPayload::Error(BackendError {
                            code: "protocol_unsupported".to_string(),
                            message: format!(
                                "client supports protocol {}-{}, daemon supports {}-{}",
                                hello.min_protocol_version,
                                hello.max_protocol_version,
                                MIN_PROTOCOL_VERSION,
                                MAX_PROTOCOL_VERSION
                            ),
                            retryable: false,
                        }),
                    )],
                    true,
                ));
            }

            let version = hello.max_protocol_version.min(MAX_PROTOCOL_VERSION);
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
            let results = runtime.engine.search(&search.text, 20);
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
            let completed = handle_action(action, runtime, config);
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
        IpcPayload::PluginDiagnosticsRequested(_) => Ok((
            vec![IpcEnvelope::request(
                request_id,
                IpcPayload::PluginDiagnosticsReady(PluginDiagnosticsReady {
                    report: serde_json::to_value(runtime.plugin_validation_report.as_ref())
                        .context("serialize plugin validation report")?,
                }),
            )],
            true,
        )),
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
    if preview.result.id == FASTFLOWLM_RESULT_ID {
        return PreviewReady {
            preview_id: preview.preview_id,
            title: preview.result.title,
            body: "Press Enter to ask FastFlowLM. No model is loaded while previewing.".to_string(),
            is_final: true,
        };
    }

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
    runtime: &DaemonRuntime,
    config: &PipeConfig,
) -> winspot_core::ActionCompleted {
    let now = current_unix_seconds();
    if action.primary_action == ActionKind::PluginCommand
        && let Err(error) = runtime
            .plugin_registry
            .ensure_plugin_command_allowed(&action.result_id)
    {
        return winspot_core::ActionCompleted {
            action_id: action.action_id,
            succeeded: false,
            message: error.to_string(),
        };
    }

    if action.primary_action == ActionKind::PluginCommand
        && action.result_id == FASTFLOWLM_RESULT_ID
    {
        return handle_fastflowlm_action(action, runtime, config, now);
    }

    let completed = ActionExecutor::new(ActionPolicy::allow_all_local()).execute(action.clone());
    if !completed.succeeded {
        return completed;
    }

    match record_usage(&action, now, config).map(|_| {
        runtime.engine.record_usage(&action.result_id, now);
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

fn handle_fastflowlm_action(
    action: winspot_core::ActionRequested,
    runtime: &DaemonRuntime,
    config: &PipeConfig,
    now: u64,
) -> winspot_core::ActionCompleted {
    let Some(service) = &runtime.fastflowlm_service else {
        return winspot_core::ActionCompleted {
            action_id: action.action_id,
            succeeded: false,
            message: "FastFlowLM integration is disabled".to_string(),
        };
    };

    match service.ask(&action.title) {
        Ok(answer) => match record_usage(&action, now, config).map(|_| {
            runtime.engine.record_usage(&action.result_id, now);
            answer
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
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
        && directory.join("Winspot.portable").exists()
    {
        return Some(directory.join("data").join("usage-events.jsonl"));
    }

    env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot\\usage-events.jsonl"))
}

/// Resolves the directory scanned for user plugin manifests, mirroring
/// [`default_usage_log_path`]: a portable install keeps plugins beside the
/// executable, otherwise they live under `%LOCALAPPDATA%\Winspot\plugins`.
fn default_plugins_dir() -> Option<PathBuf> {
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
        && directory.join("Winspot.portable").exists()
    {
        return Some(directory.join("plugins"));
    }

    env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot\\plugins"))
}

fn load_fastflowlm_settings() -> FastFlowLmSettings {
    let Some(path) = default_settings_path() else {
        return FastFlowLmSettings::default();
    };

    match load_settings_from_path(&path) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!(
                "winspot-daemon: failed to read FastFlowLM settings from {}: {error:?}",
                path.display()
            );
            FastFlowLmSettings::default()
        }
    }
}

fn build_fastflowlm_service(settings: FastFlowLmSettings) -> Option<FastFlowLmService> {
    if !settings.enabled {
        return None;
    }

    let store = match default_index_path() {
        Some(path) => IndexStore::open(&path).or_else(|error| {
            eprintln!(
                "winspot-daemon: failed to open launcher index {}: {error:?}; using empty index",
                path.display()
            );
            IndexStore::open_in_memory()
        }),
        None => IndexStore::open_in_memory(),
    };

    match store {
        Ok(store) => Some(FastFlowLmService::new(settings, store)),
        Err(error) => {
            eprintln!("winspot-daemon: failed to initialize FastFlowLM index context: {error:?}");
            None
        }
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn is_first_pipe_instance_collision(error: &std::io::Error) -> bool {
    // CreateNamedPipeW reports ERROR_ACCESS_DENIED when
    // FILE_FLAG_FIRST_PIPE_INSTANCE collides with an already-owned pipe name.
    error.raw_os_error() == Some(5)
}

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{Arc, mpsc},
        thread,
    };

    use super::*;
    use winspot_core::{ActionRequested, SearchResultKind};
    use winspot_fastflowlm::{FastFlowLmService, FastFlowLmSettings};
    use winspot_index::{IndexStore, IndexedItem};

    #[test]
    fn first_pipe_instance_collision_recognizes_windows_access_denied() {
        let error = io::Error::from_raw_os_error(5);

        assert!(is_first_pipe_instance_collision(&error));
    }

    #[test]
    fn first_pipe_instance_collision_rejects_unrelated_errors() {
        let error = io::Error::from_raw_os_error(123);

        assert!(!is_first_pipe_instance_collision(&error));
    }

    #[test]
    fn fastflowlm_action_calls_local_server_and_returns_answer() {
        let root = unique_test_dir("daemon-fastflowlm-action");
        let fake_flm = root.join("flm.cmd");
        fs::write(
            &fake_flm,
            "@echo off\r\nif \"%1\"==\"list\" if \"%2\"==\"--json\" echo [{\"name\":\"gemma4-it:e2b\",\"installed\":true}]\r\n",
        )
        .expect("write fake flm");
        let roadmap = root.join("Roadmap.md");
        fs::write(&roadmap, "Ship the FastFlowLM plugin").expect("write roadmap");

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fake flm server");
        let port = listener.local_addr().expect("fake flm addr").port();
        let (body_tx, body_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept fake flm request");
                let request = read_http_request(&mut stream);
                if request.starts_with("GET /v1/models") {
                    write_json_response(&mut stream, r#"{"data":[{"id":"gemma4-it:e2b"}]}"#);
                } else {
                    body_tx.send(request).expect("send chat request body");
                    write_json_response(
                        &mut stream,
                        r#"{"choices":[{"message":{"content":"The roadmap is ready."}}]}"#,
                    );
                }
            }
        });

        let store = IndexStore::open_in_memory().expect("open index");
        store
            .upsert(&IndexedItem {
                id: format!("file:{}", roadmap.display()),
                title: "Roadmap.md".to_string(),
                path: roadmap.display().to_string(),
                kind: SearchResultKind::File,
                modified_unix_seconds: 1,
            })
            .expect("index roadmap");
        let service = FastFlowLmService::new(
            FastFlowLmSettings {
                executable_path: fake_flm.display().to_string(),
                port,
                ..FastFlowLmSettings::default()
            },
            store,
        );
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        let runtime = DaemonRuntime {
            engine: SearchEngine::from_results(Vec::new()),
            plugin_registry: Arc::new(registry),
            plugin_validation_report: Arc::new(report),
            fastflowlm_service: Some(service),
        };

        let completed = handle_action(
            ActionRequested {
                action_id: "ask-1".to_string(),
                result_id: FASTFLOWLM_RESULT_ID.to_string(),
                title: "summarize roadmap".to_string(),
                primary_action: ActionKind::PluginCommand,
            },
            &runtime,
            &PipeConfig {
                pipe_name: r"\\.\pipe\unused".to_string(),
                usage_log_path: None,
                plugins_dir: None,
            },
        );

        assert!(completed.succeeded, "{}", completed.message);
        assert_eq!(completed.message, "The roadmap is ready.");
        let chat_request = body_rx.recv().expect("chat request captured");
        assert!(chat_request.contains("Roadmap.md"));
        assert!(chat_request.contains("Ship the FastFlowLM plugin"));

        server.join().expect("fake flm server joins");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn fastflowlm_action_reports_missing_executable() {
        let store = IndexStore::open_in_memory().expect("open index");
        let service = FastFlowLmService::new(
            FastFlowLmSettings {
                executable_path: "definitely-not-flm.exe".to_string(),
                ..FastFlowLmSettings::default()
            },
            store,
        );
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        let runtime = DaemonRuntime {
            engine: SearchEngine::from_results(Vec::new()),
            plugin_registry: Arc::new(registry),
            plugin_validation_report: Arc::new(report),
            fastflowlm_service: Some(service),
        };

        let completed = handle_action(
            ActionRequested {
                action_id: "ask-1".to_string(),
                result_id: FASTFLOWLM_RESULT_ID.to_string(),
                title: "test".to_string(),
                primary_action: ActionKind::PluginCommand,
            },
            &runtime,
            &PipeConfig {
                pipe_name: r"\\.\pipe\unused".to_string(),
                usage_log_path: None,
                plugins_dir: None,
            },
        );

        assert!(!completed.succeeded);
        assert!(completed.message.contains("failed to run"));
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create test dir");
        root
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 512];
        let header_end = loop {
            let read = stream.read(&mut buffer).expect("read fake flm request");
            assert!(read > 0, "client closed before headers");
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(position) = find_header_end(&bytes) {
                break position;
            }
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
            })
            .unwrap_or_default();
        let body_start = header_end + 4;
        while bytes.len().saturating_sub(body_start) < content_length {
            let read = stream.read(&mut buffer).expect("read fake flm body");
            assert!(read > 0, "client closed before body");
            bytes.extend_from_slice(&buffer[..read]);
        }

        String::from_utf8_lossy(&bytes).to_string()
    }

    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn write_json_response(stream: &mut TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write fake flm response");
    }
}
