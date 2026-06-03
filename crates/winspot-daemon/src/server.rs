use std::{
    env,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
#[cfg(windows)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use winspot_actions::{ActionExecutor, ActionPolicy};

#[cfg(windows)]
use crate::pipe_security::PipeSecurity;
use winspot_capture::{
    CaptureService, WINDOWS_CAPTURE_RESULT_PREFIX, WindowsCaptureProvider, WindowsCaptureSettings,
    load_settings_from_path as load_capture_settings_from_path,
};
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
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
    pub plugin_validation_report: Arc<RwLock<PluginValidationReport>>,
    pub fastflowlm_service: Option<FastFlowLmService>,
    pub capture_service: Option<CaptureService>,
}

impl DaemonRuntime {
    /// Creates a `DaemonRuntime` from an existing `SearchEngine`, registering built-in plugins and producing a validation report.
    ///
    /// The returned runtime wraps the provided `engine`, initializes a `PluginRegistry` populated with built-in providers and its `PluginValidationReport`, and leaves `fastflowlm_service` disabled (`None`).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Construct or obtain a SearchEngine instance first (example assumes `Default` is available).
    /// let engine = SearchEngine::default();
    /// let runtime = DaemonRuntime::from_engine(engine);
    /// assert!(runtime.fastflowlm_service.is_none());
    /// ```
    pub fn from_engine(engine: SearchEngine) -> Self {
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        Self {
            engine,
            plugin_registry: Arc::new(RwLock::new(registry)),
            plugin_validation_report: Arc::new(RwLock::new(report)),
            fastflowlm_service: None,
            capture_service: None,
        }
    }
}

#[cfg(windows)]
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
#[cfg(windows)]
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

/// Builds the daemon runtime used by the server.
///
/// The runtime contains a configured `SearchEngine`, a shared plugin registry and its
/// validation report, and an optional `FastFlowLmService` depending on configuration and
/// available settings. Usage history is loaded from `config.usage_log_path` when present;
/// failures to load the usage log are returned as errors.
///
/// # Returns
///
/// `Ok(DaemonRuntime)` with the assembled runtime on success, or an error if usage loading fails.
///
/// # Examples
///
/// ```ignore
/// let cfg = PipeConfig::default();
/// let _runtime = build_daemon_runtime(&cfg).unwrap();
/// ```
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
    let capture_settings = load_capture_settings();

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
    if capture_settings.enabled {
        engine = engine.with_dynamic_provider(Arc::new(WindowsCaptureProvider::new(
            capture_settings.clone(),
        )));
    }

    Ok(DaemonRuntime {
        engine,
        plugin_registry,
        plugin_validation_report,
        fastflowlm_service: build_fastflowlm_service(fastflowlm_settings),
        capture_service: build_capture_service(capture_settings),
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
) -> (
    Arc<RwLock<PluginRegistry>>,
    Arc<RwLock<PluginValidationReport>>,
) {
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
    (
        Arc::new(RwLock::new(registry)),
        Arc::new(RwLock::new(report)),
    )
}

fn current_plugin_validation_report(
    runtime: &DaemonRuntime,
    config: &PipeConfig,
) -> PluginValidationReport {
    let Some(dir) = config.plugins_dir.as_deref() else {
        return runtime
            .plugin_validation_report
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
    };

    let (mut registry, mut report) = PluginRegistry::with_built_ins_with_report();
    match registry.load_dir_into_with_report(dir) {
        Ok(user_report) => {
            report.extend(user_report);
            // Update the live registry and report in the runtime
            *runtime
                .plugin_registry
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = registry;
            *runtime
                .plugin_validation_report
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = report.clone();
            report
        }
        Err(error) => {
            eprintln!(
                "winspot-daemon: failed to rescan plugins directory {}: {error:?}",
                dir.display()
            );
            runtime
                .plugin_validation_report
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }
}

/// Serves a single named-pipe connection using a runtime built from the given engine and config.
///
/// This constructs a `DaemonRuntime` (including a plugin registry, validation report, and an
/// optional FastFlowLM service) from `config` and `engine`, then accepts and handles one client
/// connection on the configured pipe. The function returns when that connection handling completes.
///
/// # Parameters
///
/// - `config`: Pipe configuration used to build plugin registry, usage logging, and pipe options.
/// - `engine`: Search engine instance to use for request handling; it will be cloned into the runtime.
///
/// # Returns
///
/// `Ok(())` if the connection was served successfully, or an error with context if setup or serving fails.
///
/// # Examples
///
/// ```ignore
/// # use winspot_daemon::{serve_pipe_once, PipeConfig};
/// # use winspot_core::SearchEngine;
/// # fn main() {
/// let rt = tokio::runtime::Runtime::new().unwrap();
/// let engine = SearchEngine::default();
/// let config = PipeConfig::default();
/// let res = rt.block_on(async { serve_pipe_once(config, &engine).await });
/// assert!(res.is_ok() || res.is_err()); // illustrate call; real invocation runs the daemon once
/// # }
/// ```
#[cfg(windows)]
pub async fn serve_pipe_once(config: PipeConfig, engine: &SearchEngine) -> anyhow::Result<()> {
    let (plugin_registry, plugin_validation_report) =
        build_plugin_registry(config.plugins_dir.as_deref());
    let runtime = DaemonRuntime {
        engine: engine.clone(),
        plugin_registry,
        plugin_validation_report,
        fastflowlm_service: build_fastflowlm_service(load_fastflowlm_settings()),
        capture_service: build_capture_service(load_capture_settings()),
    };
    serve_runtime_pipe_once(config, &runtime).await
}

#[cfg(windows)]
pub async fn serve_runtime_pipe_once(
    config: PipeConfig,
    runtime: &DaemonRuntime,
) -> anyhow::Result<()> {
    let server = create_secured_pipe(&config.pipe_name, false)
        .with_context(|| format!("create named pipe {}", config.pipe_name))?;
    serve_connection(server, runtime, &config).await
}

#[cfg(windows)]
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
        IpcPayload::PluginDiagnosticsRequested(_) => {
            let report = current_plugin_validation_report(runtime, config);
            Ok((
                vec![IpcEnvelope::request(
                    request_id,
                    IpcPayload::PluginDiagnosticsReady(PluginDiagnosticsReady {
                        report: serde_json::to_value(&report)
                            .context("serialize plugin validation report")?,
                    }),
                )],
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

#[cfg(windows)]
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

/// Builds a non-final preview chunk that indicates the preview is loading.
///
/// The returned `PreviewChunk` uses the incoming preview's `preview_id` and result
/// title, sets the body to `"Loading preview"`, and marks `is_final` as `false`.
///
/// # Examples
///
/// ```ignore
/// let request = PreviewRequested {
///     preview_id: "preview-1".to_string(),
///     result: SearchResult { title: "Example".to_string(), ..Default::default() },
///     ..Default::default()
/// };
/// let chunk = build_preview_chunk(&request);
/// assert_eq!(chunk.preview_id, "preview-1");
/// assert_eq!(chunk.title, "Example");
/// assert_eq!(chunk.body, "Loading preview");
/// assert!(!chunk.is_final);
/// ```
fn build_preview_chunk(preview: &PreviewRequested) -> PreviewChunk {
    PreviewChunk {
        preview_id: preview.preview_id.clone(),
        title: preview.result.title.clone(),
        body: "Loading preview".to_string(),
        is_final: false,
    }
}

/// Builds a final `PreviewReady` for a preview request.
///
/// If the requested result is the FastFlowLM sentinel (`FASTFLOWLM_RESULT_ID`), returns a
/// final preview that instructs the client to press Enter to invoke FastFlowLM. Otherwise,
/// produces a final preview using the default preview provider.
///
/// # Examples
///
/// ```ignore
/// // Construct a minimal PreviewRequested for demonstration; real code will provide
/// // a complete SearchResult value.
/// let preview = PreviewRequested {
///     preview_id: "example".to_string(),
///     result: SearchResult {
///         id: "non-fastflowlm".to_string(),
///         title: "Example".to_string(),
///         ..Default::default()
///     },
/// };
/// let ready = build_preview(preview);
/// assert!(ready.is_final);
/// ```
fn build_preview(preview: PreviewRequested) -> PreviewReady {
    if preview.result.id.starts_with(WINDOWS_CAPTURE_RESULT_PREFIX) {
        return PreviewReady {
            preview_id: preview.preview_id,
            title: preview.result.title,
            body: "Press Enter to save this capture. No capture starts while previewing."
                .to_string(),
            is_final: true,
        };
    }

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

/// Execute an action request: enforce plugin permissions, handle FastFlowLM plugin commands,
/// execute the action locally when allowed, and record usage if configured.
///
/// If the action is a `PluginCommand`, this first ensures the plugin command is allowed by the
/// daemon's plugin registry and returns a failing `ActionCompleted` if not. If the action targets
/// the `FASTFLOWLM_RESULT_ID`, it is routed to the FastFlowLM-specific handler. Otherwise the
/// action is executed locally; when execution succeeds the function attempts to record usage and
/// updates the engine's usage counters. Any failure during execution or usage recording is
/// reflected in the returned `ActionCompleted`.
///
/// # Returns
///
/// `ActionCompleted` describing whether the action succeeded and containing a human-readable
/// message with either the result (on success) or an error description (on failure).
///
/// # Examples
///
/// ```ignore
/// use winspot_daemon::{handle_action, DaemonRuntime, PipeConfig};
/// use winspot_core::ActionRequested;
///
/// // Construct `action`, `runtime`, and `config` appropriate for your application,
/// // then call:
/// // let completed = handle_action(action, &runtime, &config);
/// // assert!(completed.succeeded || !completed.succeeded);
/// ```
fn handle_action(
    action: winspot_core::ActionRequested,
    runtime: &DaemonRuntime,
    config: &PipeConfig,
) -> winspot_core::ActionCompleted {
    let now = current_unix_seconds();
    if action.primary_action == ActionKind::PluginCommand
        && let Err(error) = runtime
            .plugin_registry
            .read()
            .unwrap()
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

    if action.primary_action == ActionKind::PluginCommand
        && action.result_id.starts_with(WINDOWS_CAPTURE_RESULT_PREFIX)
    {
        return handle_capture_action(action, runtime, config, now);
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

fn handle_capture_action(
    action: winspot_core::ActionRequested,
    runtime: &DaemonRuntime,
    config: &PipeConfig,
    now: u64,
) -> winspot_core::ActionCompleted {
    let Some(service) = &runtime.capture_service else {
        return winspot_core::ActionCompleted {
            action_id: action.action_id,
            succeeded: false,
            message: "Windows Capture integration is disabled".to_string(),
        };
    };

    match service.execute_result_id(&action.result_id) {
        Ok(outcome) => match record_usage(&action, now, config).map(|_| {
            runtime.engine.record_usage(&action.result_id, now);
            format!("Saved capture to {}", outcome.output_path)
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

/// Execute a FastFlowLM-backed plugin action by asking the FastFlowLM service for an answer
/// and recording usage if the request succeeds.
///
/// If the daemon's FastFlowLM service is disabled, returns an `ActionCompleted` with
/// `succeeded = false` and an explanatory message. If the service answers successfully,
/// the function attempts to record usage; on success it returns `succeeded = true` with
/// the service's answer as `message`. Any error from the service or from usage recording
/// is returned as `succeeded = false` with the error string as `message`.
///
/// # Examples
///
/// ```ignore
/// // Given valid `action`, `runtime`, `config`, and `now` values:
/// let completed = handle_fastflowlm_action(action, &runtime, &config, now);
/// if completed.succeeded {
///     println!("FastFlowLM answer: {}", completed.message);
/// } else {
///     eprintln!("FastFlowLM action failed: {}", completed.message);
/// }
/// ```
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

/// Records a usage event for the given action to the configured usage log.
///
/// If `config.usage_log_path` is `None`, this function does nothing and returns `Ok(())`.
///
/// # Parameters
///
/// - `action`: the action whose `result_id` will be recorded.
/// - `now_unix_seconds`: timestamp (seconds since UNIX epoch) to attach to the usage event.
/// - `config`: daemon pipe configuration; the `usage_log_path` field controls whether events are persisted.
///
/// # Returns
///
/// `Ok(())` on success, `Err` if writing the usage event fails (the error is annotated with the `result_id`).
///
/// # Examples
///
/// ```ignore
/// let action = winspot_core::ActionRequested {
///     result_id: "example".to_string(),
///     title: "t".to_string(),
///     primary_action: winspot_core::ActionKind::Open,
///     ..Default::default()
/// };
/// let config = PipeConfig { usage_log_path: None, ..Default::default() };
/// // With no usage_log_path this is a no-op and returns Ok.
/// assert!(record_usage(&action, 1_700_000_000, &config).is_ok());
/// ```
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

/// Determines the default directory used to scan for user plugin manifests.
///
/// If the executable's parent directory contains a `Winspot.portable` marker file,
/// returns that parent's `plugins` subdirectory (portable layout). Otherwise,
/// returns `%LOCALAPPDATA%\Winspot\plugins` when `LOCALAPPDATA` is set.
///
/// # Returns
///
/// `Some(PathBuf)` with the resolved plugins directory, or `None` if `LOCALAPPDATA` is not available.
///
/// # Examples
///
/// ```ignore
/// // Use the default plugins directory if available.
/// if let Some(dir) = default_plugins_dir() {
///     let plugin_manifest = dir.join("my_plugin.manifest.json");
///     println!("{}", plugin_manifest.display());
/// }
/// ```
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

/// Load FastFlowLM settings from the default settings path, falling back to defaults on missing path or read errors.
///
/// If `default_settings_path()` returns `None`, this returns `FastFlowLmSettings::default()`. If a path is present but
/// `load_settings_from_path` fails, an error is printed to stderr and `FastFlowLmSettings::default()` is returned.
///
/// # Examples
///
/// ```ignore
/// let _settings = load_fastflowlm_settings();
/// ```
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

fn load_capture_settings() -> WindowsCaptureSettings {
    let Some(path) = default_settings_path() else {
        return WindowsCaptureSettings::default();
    };

    match load_capture_settings_from_path(&path) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!(
                "winspot-daemon: failed to read Windows Capture settings from {}: {error:?}",
                path.display()
            );
            WindowsCaptureSettings::default()
        }
    }
}

fn build_capture_service(settings: WindowsCaptureSettings) -> Option<CaptureService> {
    settings
        .enabled
        .then(|| CaptureService::new(settings.normalized()))
}

/// Initializes a FastFlowLM service if enabled and an index store can be acquired.
///
/// Attempts to open the default index path; on failure it falls back to an in-memory index.
/// If `settings.enabled` is false or the index cannot be initialized, no service is created.
///
/// # Returns
///
/// `Some(FastFlowLmService)` when the service was successfully initialized, `None` otherwise.
///
/// # Examples
///
/// ```ignore
/// let settings = FastFlowLmSettings::default();
/// // Default settings are typically disabled, so this will usually return `None`.
/// let svc = build_fastflowlm_service(settings);
/// assert!(svc.is_none());
/// ```
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

/// Get the current time as seconds since the Unix epoch.
///
/// If the system clock is before the Unix epoch, this returns `0`.
///
/// # Returns
///
/// `u64` seconds elapsed since `UNIX_EPOCH`; `0` if the system time is earlier than the epoch.
///
/// # Examples
///
/// ```ignore
/// let t1 = current_unix_seconds();
/// let t2 = current_unix_seconds();
/// // time should be non-decreasing between two close calls
/// assert!(t2 >= t1);
/// ```
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
        path::Path,
        sync::{Arc, Mutex, mpsc},
        thread,
    };

    use super::*;
    use winspot_capture::{
        CaptureBackend, CaptureCommand, CaptureService, CaptureTarget, WindowsCaptureSettings,
    };
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
            plugin_registry: Arc::new(RwLock::new(registry)),
            plugin_validation_report: Arc::new(RwLock::new(report)),
            fastflowlm_service: Some(service),
            capture_service: None,
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
            plugin_registry: Arc::new(RwLock::new(registry)),
            plugin_validation_report: Arc::new(RwLock::new(report)),
            fastflowlm_service: Some(service),
            capture_service: None,
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

    #[test]
    fn capture_preview_is_instructional_only() {
        let ready = build_preview(PreviewRequested {
            preview_id: "preview-capture".to_string(),
            result: winspot_core::SearchResult {
                id: "plugin:windows-capture:screenshot:monitor:primary".to_string(),
                title: "Screenshot primary monitor".to_string(),
                subtitle: "Save a PNG capture".to_string(),
                kind: SearchResultKind::Plugin,
                score: 1.0,
                primary_action: ActionKind::PluginCommand,
                actions: Vec::new(),
                source: Some("windows-capture".to_string()),
                icon_hint: None,
            },
        });

        assert!(ready.is_final);
        assert_eq!(ready.title, "Screenshot primary monitor");
        assert!(ready.body.contains("Press Enter"));
        assert!(ready.body.contains("No capture starts while previewing"));
    }

    #[test]
    fn capture_action_routes_to_capture_service_and_returns_output_path() {
        let backend = Arc::new(RecordingCaptureBackend::default());
        let service = CaptureService::with_backend(
            WindowsCaptureSettings {
                output_directory: r"C:\Captures".to_string(),
                ..WindowsCaptureSettings::default()
            },
            backend.clone(),
        );
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        let runtime = DaemonRuntime {
            engine: SearchEngine::from_results(Vec::new()),
            plugin_registry: Arc::new(RwLock::new(registry)),
            plugin_validation_report: Arc::new(RwLock::new(report)),
            fastflowlm_service: None,
            capture_service: Some(service),
        };

        let completed = handle_action(
            ActionRequested {
                action_id: "capture-1".to_string(),
                result_id: "plugin:windows-capture:screenshot:window:foreground".to_string(),
                title: "Screenshot foreground window".to_string(),
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
        assert!(completed.message.contains("Saved capture to"));
        assert!(completed.message.contains("winspot-screenshot-"));
        assert_eq!(
            backend.calls.lock().expect("calls").as_slice(),
            &[CaptureCommand::screenshot(CaptureTarget::ForegroundWindow)]
        );
    }

    #[test]
    fn capture_action_reports_disabled_when_service_is_missing() {
        let (registry, report) = PluginRegistry::with_built_ins_with_report();
        let runtime = DaemonRuntime {
            engine: SearchEngine::from_results(Vec::new()),
            plugin_registry: Arc::new(RwLock::new(registry)),
            plugin_validation_report: Arc::new(RwLock::new(report)),
            fastflowlm_service: None,
            capture_service: None,
        };

        let completed = handle_action(
            ActionRequested {
                action_id: "capture-disabled".to_string(),
                result_id: "plugin:windows-capture:record:monitor:primary:8".to_string(),
                title: "Record primary monitor for 8s".to_string(),
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
        assert!(
            completed
                .message
                .contains("Windows Capture integration is disabled")
        );
    }

    #[derive(Default)]
    struct RecordingCaptureBackend {
        calls: Mutex<Vec<CaptureCommand>>,
    }

    impl CaptureBackend for RecordingCaptureBackend {
        fn capture(
            &self,
            command: &CaptureCommand,
            _settings: &WindowsCaptureSettings,
            _output_path: &Path,
        ) -> anyhow::Result<()> {
            self.calls.lock().expect("calls").push(command.clone());
            Ok(())
        }
    }

    /// Create a unique temporary directory for tests.
    ///
    /// The directory name is derived from `label` and the current process id. If a directory already
    /// exists at the computed path it is removed and a fresh directory is created.
    ///
    /// # Parameters
    ///
    /// - `label`: Short label used as part of the directory name.
    ///
    /// # Returns
    ///
    /// A `PathBuf` pointing to the created directory.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let dir = unique_test_dir("my-test");
    /// assert!(dir.exists());
    /// ```
    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create test dir");
        root
    }

    /// Reads a complete HTTP request from the provided `TcpStream` and returns the raw request
    /// (headers and body) as a `String`.
    ///
    /// This function:
    /// - Reads from the stream until it detects the end of the HTTP header section (`\r\n\r\n`).
    /// - Parses the `Content-Length` header (case-insensitive) if present and continues reading
    ///   until that many bytes of body have been received.
    /// - Returns the concatenation of the headers and body. Invalid UTF-8 sequences are replaced
    ///   using `String::from_utf8_lossy`.
    ///
    /// The function will panic if the peer closes the connection before headers or body are fully
    /// received and will propagate I/O errors via `expect` messages used internally.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::net::{TcpListener, TcpStream};
    /// use std::thread;
    ///
    /// // spawn a server that sends a minimal HTTP request to a connecting client
    /// let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    /// let addr = listener.local_addr().unwrap();
    /// thread::spawn(move || {
    ///     let (mut sock, _) = listener.accept().unwrap();
    ///     let req = b"POST / HTTP/1.1\r\nHost: example\r\nContent-Length: 5\r\n\r\nhello";
    ///     sock.write_all(req).unwrap();
    /// });
    ///
    /// let mut stream = TcpStream::connect(addr).unwrap();
    /// let raw = read_http_request(&mut stream);
    /// assert!(raw.contains("Content-Length: 5"));
    /// assert!(raw.ends_with("hello"));
    /// ```
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

    /// Finds the byte index where an HTTP-style header section ends (`"\r\n\r\n"`).
    ///
    /// Scans the provided byte slice for the first occurrence of the four-byte sequence `\r\n\r\n` and returns the index of its first byte if found, or `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(find_header_end(b"foo\r\n\r\nbar"), Some(3));
    /// assert_eq!(find_header_end(b"no delimiter here"), None);
    /// ```
    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes.windows(4).position(|window| window == b"\r\n\r\n")
    }

    /// Writes a simple HTTP/1.1 200 response with JSON body to the given TCP stream and closes the connection.
    ///
    /// # Panics
    ///
    /// Panics if writing to the stream fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::net::{TcpListener, TcpStream};
    /// use std::io::{Read, Write};
    /// use std::thread;
    ///
    /// // Start a listener and accept one connection, then read the response.
    /// let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    /// let addr = listener.local_addr().unwrap();
    ///
    /// let handle = thread::spawn(move || {
    ///     let (mut socket, _) = listener.accept().unwrap();
    ///     let mut buf = String::new();
    ///     socket.read_to_string(&mut buf).unwrap();
    ///     buf
    /// });
    ///
    /// let mut client = TcpStream::connect(addr).unwrap();
    /// // Call the function under test (assumes it's visible in scope)
    /// write_json_response(&mut client, r#"{"ok":true}"#);
    ///
    /// // The server thread will read the raw HTTP response text.
    /// let resp = handle.join().unwrap();
    /// assert!(resp.contains("HTTP/1.1 200 OK"));
    /// assert!(resp.contains(r#"Content-Type: application/json"#));
    /// assert!(resp.contains(r#"{"ok":true}"#));
    /// ```
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
