//! FastFlowLM launcher integration.

use std::{
    collections::HashSet,
    env, fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use winspot_core::{
    ActionCapability, ActionDescriptor, ActionKind, SearchResult, SearchResultKind,
};
use winspot_index::IndexStore;
use winspot_search::providers::DynamicSearchProvider;

pub const FASTFLOWLM_PLUGIN_ID: &str = "fastflowlm";
pub const FASTFLOWLM_RESULT_ID: &str = "plugin:fastflowlm";
pub const DEFAULT_MODEL_TAG: &str = "gemma4-it:e2b";
pub const DEFAULT_MODEL_DISPLAY_NAME: &str = "Gemma4-E2B-IT-NPU2";
pub const DEFAULT_EXECUTABLE_PATH: &str = "flm";
pub const DEFAULT_PORT: u16 = 52625;
pub const DEFAULT_IDLE_TIMEOUT_SECONDS: u64 = 120;
pub const DEFAULT_MAX_CONTEXT_FILES: usize = 5;
pub const DEFAULT_MAX_FILE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_CONTEXT_BYTES: usize = 4 * 1024 * 1024;

const HEALTH_PATH: &str = "/v1/models";
const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_START_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastFlowLmSettings {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_model_tag")]
    pub model_tag: String,
    #[serde(default = "default_executable_path")]
    pub executable_path: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_idle_timeout_seconds")]
    pub idle_timeout_seconds: u64,
    #[serde(default = "default_max_context_files")]
    pub max_context_files: usize,
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: usize,
    #[serde(default = "default_max_context_bytes")]
    pub max_context_bytes: usize,
}

impl Default for FastFlowLmSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            model_tag: DEFAULT_MODEL_TAG.to_string(),
            executable_path: DEFAULT_EXECUTABLE_PATH.to_string(),
            port: DEFAULT_PORT,
            idle_timeout_seconds: DEFAULT_IDLE_TIMEOUT_SECONDS,
            max_context_files: DEFAULT_MAX_CONTEXT_FILES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_context_bytes: DEFAULT_MAX_CONTEXT_BYTES,
        }
    }
}

impl FastFlowLmSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        if self.model_tag.trim().is_empty() {
            self.model_tag = defaults.model_tag;
        } else {
            self.model_tag = self.model_tag.trim().to_string();
        }
        if self.executable_path.trim().is_empty() {
            self.executable_path = defaults.executable_path;
        } else {
            self.executable_path = self.executable_path.trim().to_string();
        }
        if self.port == 0 {
            self.port = defaults.port;
        }
        if self.idle_timeout_seconds == 0 {
            self.idle_timeout_seconds = defaults.idle_timeout_seconds;
        }
        if self.max_context_files == 0 {
            self.max_context_files = defaults.max_context_files;
        }
        if self.max_file_bytes == 0 {
            self.max_file_bytes = defaults.max_file_bytes;
        }
        if self.max_context_bytes == 0 {
            self.max_context_bytes = defaults.max_context_bytes;
        }
        self
    }

    pub fn context_limits(&self) -> ContextLimits {
        ContextLimits {
            max_files: self.max_context_files,
            max_file_bytes: self.max_file_bytes,
            max_context_bytes: self.max_context_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLimits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_context_bytes: usize,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_CONTEXT_FILES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_context_bytes: DEFAULT_MAX_CONTEXT_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBundle {
    pub entries: Vec<ContextEntry>,
    pub total_content_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    pub title: String,
    pub path: String,
    pub kind: SearchResultKind,
    pub source: Option<String>,
    pub content: Option<String>,
    pub metadata_only_reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct FastFlowLmProvider;

impl DynamicSearchProvider for FastFlowLmProvider {
    fn search(&self, query: &str) -> Vec<SearchResult> {
        let Some(prompt) = parse_fastflowlm_prompt(query) else {
            return Vec::new();
        };

        vec![SearchResult {
            id: FASTFLOWLM_RESULT_ID.to_string(),
            title: prompt.to_string(),
            subtitle: "Ask FastFlowLM with launcher context".to_string(),
            kind: SearchResultKind::Plugin,
            score: 1.0,
            primary_action: ActionKind::PluginCommand,
            actions: vec![ActionDescriptor {
                id: "run-plugin".to_string(),
                label: "Ask".to_string(),
                kind: ActionKind::PluginCommand,
                capabilities: vec![ActionCapability::PluginExecution],
            }],
            source: Some(FASTFLOWLM_PLUGIN_ID.to_string()),
            icon_hint: None,
        }]
    }
}

#[derive(Clone)]
pub struct FastFlowLmService {
    settings: FastFlowLmSettings,
    index_store: Arc<Mutex<IndexStore>>,
    manager: FastFlowLmManager,
}

impl FastFlowLmService {
    pub fn new(settings: FastFlowLmSettings, index_store: IndexStore) -> Self {
        let settings = settings.normalized();
        Self {
            manager: FastFlowLmManager::new(settings.clone()),
            settings,
            index_store: Arc::new(Mutex::new(index_store)),
        }
    }

    pub fn ask(&self, question: &str) -> anyhow::Result<String> {
        if !self.settings.enabled {
            anyhow::bail!("FastFlowLM integration is disabled");
        }
        let question = question.trim();
        if question.is_empty() {
            anyhow::bail!("FastFlowLM prompt is empty");
        }

        let context = {
            let store = self
                .index_store
                .lock()
                .map_err(|_| anyhow!("launcher index is unavailable"))?;
            build_index_context(&store, question, self.settings.context_limits())?
        };
        self.manager.ask(question, &context)
    }
}

#[derive(Clone)]
pub struct FastFlowLmManager {
    settings: FastFlowLmSettings,
    state: Arc<ProcessState>,
}

struct ProcessState {
    inner: Mutex<ProcessStateInner>,
    idle_changed: Condvar,
}

#[derive(Default)]
struct ProcessStateInner {
    child: Option<Child>,
    last_used_unix_seconds: u64,
    reaper_running: bool,
}

impl FastFlowLmManager {
    pub fn new(settings: FastFlowLmSettings) -> Self {
        Self {
            settings: settings.normalized(),
            state: Arc::new(ProcessState {
                inner: Mutex::new(ProcessStateInner::default()),
                idle_changed: Condvar::new(),
            }),
        }
    }

    pub fn ask(&self, question: &str, context: &ContextBundle) -> anyhow::Result<String> {
        validate_installed_model(&self.settings)?;
        self.ensure_server()?;
        let messages = build_chat_messages(question, context);
        self.touch_used();
        let answer = request_chat_completion(&self.settings, &messages)?;
        self.touch_used();
        Ok(answer)
    }

    fn ensure_server(&self) -> anyhow::Result<()> {
        if health_check(&self.settings).is_ok() {
            return Ok(());
        }

        self.stop_stale_owned_process();

        let mut command = Command::new(&self.settings.executable_path);
        command.args([
            "serve",
            self.settings.model_tag.as_str(),
            "--host",
            "127.0.0.1",
            "--port",
            &self.settings.port.to_string(),
            "--pmode",
            "balanced",
            "--cors",
            "0",
        ]);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().with_context(|| {
            format!(
                "failed to start FastFlowLM executable '{}'",
                self.settings.executable_path
            )
        })?;

        {
            let mut state = self
                .state
                .inner
                .lock()
                .map_err(|_| anyhow!("FastFlowLM process state is unavailable"))?;
            state.last_used_unix_seconds = current_unix_seconds();
            state.child = Some(child);
        }
        self.start_idle_reaper();

        let started = SystemTime::now();
        loop {
            if health_check(&self.settings).is_ok() {
                return Ok(());
            }
            if started.elapsed().unwrap_or_default() >= SERVER_START_TIMEOUT {
                self.stop_stale_owned_process();
                anyhow::bail!(
                    "FastFlowLM server did not become ready at http://127.0.0.1:{}",
                    self.settings.port
                );
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn stop_stale_owned_process(&self) {
        let Ok(mut state) = self.state.inner.lock() else {
            return;
        };
        if let Some(mut child) = state.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.state.idle_changed.notify_all();
    }

    fn touch_used(&self) {
        if let Ok(mut state) = self.state.inner.lock() {
            state.last_used_unix_seconds = current_unix_seconds();
            self.state.idle_changed.notify_all();
        }
    }

    fn start_idle_reaper(&self) {
        let state = Arc::clone(&self.state);
        let idle_timeout_seconds = self.settings.idle_timeout_seconds;
        {
            let Ok(mut inner) = state.inner.lock() else {
                return;
            };
            if inner.reaper_running {
                return;
            }
            inner.reaper_running = true;
        }

        std::thread::spawn(move || {
            loop {
                let Ok(mut inner) = state.inner.lock() else {
                    return;
                };
                if inner.child.is_none() {
                    inner.reaper_running = false;
                    return;
                }

                let now = current_unix_seconds();
                if should_stop_owned_process(
                    true,
                    inner.last_used_unix_seconds,
                    now,
                    idle_timeout_seconds,
                ) {
                    if let Some(mut child) = inner.child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    inner.reaper_running = false;
                    return;
                }

                let deadline = inner
                    .last_used_unix_seconds
                    .saturating_add(idle_timeout_seconds);
                let wait_seconds = deadline.saturating_sub(now).max(1);
                let Ok((next_inner, _)) = state
                    .idle_changed
                    .wait_timeout(inner, Duration::from_secs(wait_seconds))
                else {
                    return;
                };
                drop(next_inner);
            }
        });
    }
}

pub fn parse_fastflowlm_prompt(query: &str) -> Option<&str> {
    let trimmed = query.trim();
    let lower = trimmed.to_ascii_lowercase();
    let prompt = lower
        .strip_prefix("ai ")
        .and_then(|_| trimmed.get(3..))
        .or_else(|| lower.strip_prefix("ask ").and_then(|_| trimmed.get(4..)))?
        .trim();

    (!prompt.is_empty()).then_some(prompt)
}

pub fn build_index_context(
    store: &IndexStore,
    query: &str,
    limits: ContextLimits,
) -> anyhow::Result<ContextBundle> {
    let limits = ContextLimits {
        max_files: limits.max_files.max(1),
        max_file_bytes: limits.max_file_bytes.max(1),
        max_context_bytes: limits.max_context_bytes.max(1),
    };
    let matches = search_index_for_context(store, query, limits.max_files)?;
    let mut entries = Vec::with_capacity(matches.len());
    let mut total_content_bytes = 0usize;

    for result in matches {
        let mut entry = ContextEntry {
            title: result.title.clone(),
            path: result.subtitle.clone(),
            kind: result.kind.clone(),
            source: result.source.clone(),
            content: None,
            metadata_only_reason: None,
        };

        if result.kind == SearchResultKind::File {
            let path = PathBuf::from(&result.subtitle);
            match read_context_file(&path, limits, total_content_bytes) {
                FileContextRead::Content(content) => {
                    total_content_bytes += content.len();
                    entry.content = Some(content);
                }
                FileContextRead::MetadataOnly(reason) => {
                    entry.metadata_only_reason = Some(reason);
                }
            }
        }

        entries.push(entry);
    }

    Ok(ContextBundle {
        entries,
        total_content_bytes,
    })
}

fn search_index_for_context(
    store: &IndexStore,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<SearchResult>> {
    let mut results = store.search(query, limit)?;
    let mut seen: HashSet<String> = results.iter().map(|result| result.id.clone()).collect();
    if results.len() >= limit {
        return Ok(results);
    }

    for token in query_tokens(query) {
        for result in store.search(token, limit)? {
            if seen.insert(result.id.clone()) {
                results.push(result);
            }
            if results.len() >= limit {
                return Ok(results);
            }
        }
    }

    Ok(results)
}

fn query_tokens(query: &str) -> impl Iterator<Item = &str> {
    query
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .map(str::trim)
        .filter(|token| token.len() >= 3)
}

pub fn validate_installed_model_from_list_json(
    json_output: &str,
    model_tag: &str,
) -> anyhow::Result<()> {
    let value: Value = serde_json::from_str(json_output).context("parse flm list --json output")?;
    if json_contains_installed_model(&value, model_tag) {
        return Ok(());
    }

    anyhow::bail!(
        "FastFlowLM model '{model_tag}' is not installed. Install {DEFAULT_MODEL_DISPLAY_NAME} separately before using Winspot AI."
    );
}

pub fn should_stop_owned_process(
    owns_process: bool,
    last_used_unix_seconds: u64,
    now_unix_seconds: u64,
    idle_timeout_seconds: u64,
) -> bool {
    owns_process && now_unix_seconds.saturating_sub(last_used_unix_seconds) >= idle_timeout_seconds
}

pub fn load_settings_from_path(path: impl AsRef<Path>) -> anyhow::Result<FastFlowLmSettings> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(FastFlowLmSettings::default());
    }

    let json = fs::read_to_string(path)
        .with_context(|| format!("read launcher settings {}", path.display()))?;
    let root: LauncherSettingsRoot =
        serde_json::from_str(&json).with_context(|| format!("parse {}", path.display()))?;
    Ok(root.fast_flow_lm.unwrap_or_default().normalized())
}

pub fn default_settings_path() -> Option<PathBuf> {
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
        && directory.join("Winspot.portable").exists()
    {
        return Some(directory.join("data").join("settings.json"));
    }

    env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot\\settings.json"))
}

pub fn default_index_path() -> Option<PathBuf> {
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
        && directory.join("Winspot.portable").exists()
    {
        return Some(directory.join("data").join("index.sqlite"));
    }

    env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot\\index.sqlite"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LauncherSettingsRoot {
    #[serde(default)]
    fast_flow_lm: Option<FastFlowLmSettings>,
}

enum FileContextRead {
    Content(String),
    MetadataOnly(String),
}

fn read_context_file(
    path: &Path,
    limits: ContextLimits,
    current_total_bytes: usize,
) -> FileContextRead {
    let Ok(metadata) = fs::metadata(path) else {
        return FileContextRead::MetadataOnly("unreadable file".to_string());
    };
    if !metadata.is_file() {
        return FileContextRead::MetadataOnly("not a regular file".to_string());
    }
    if metadata.len() as usize > limits.max_file_bytes {
        return FileContextRead::MetadataOnly("file exceeds per-file content cap".to_string());
    }
    if current_total_bytes.saturating_add(metadata.len() as usize) > limits.max_context_bytes {
        return FileContextRead::MetadataOnly("aggregate context cap reached".to_string());
    }

    let Ok(bytes) = fs::read(path) else {
        return FileContextRead::MetadataOnly("unreadable file".to_string());
    };
    if bytes.contains(&0) {
        return FileContextRead::MetadataOnly("binary or non-UTF-8 file".to_string());
    }
    match String::from_utf8(bytes) {
        Ok(content) => FileContextRead::Content(content),
        Err(_) => FileContextRead::MetadataOnly("binary or non-UTF-8 file".to_string()),
    }
}

fn validate_installed_model(settings: &FastFlowLmSettings) -> anyhow::Result<()> {
    let output = Command::new(&settings.executable_path)
        .args(["list", "--json"])
        .output()
        .with_context(|| {
            format!(
                "failed to run '{} list --json' to validate FastFlowLM model",
                settings.executable_path
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "failed to validate FastFlowLM model '{}': {}",
            settings.model_tag,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8(output.stdout).context("flm list --json returned non-UTF-8")?;
    validate_installed_model_from_list_json(&stdout, &settings.model_tag)
}

fn json_contains_installed_model(value: &Value, model_tag: &str) -> bool {
    match value {
        Value::Array(items) => items
            .iter()
            .any(|item| json_contains_installed_model(item, model_tag)),
        Value::Object(object) => {
            let direct_match = ["name", "model", "tag", "id"]
                .iter()
                .filter_map(|key| object.get(*key).and_then(Value::as_str))
                .any(|candidate| candidate == model_tag);
            let installed = object
                .get("installed")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    object
                        .get("status")
                        .and_then(Value::as_str)
                        .map(|status| status.eq_ignore_ascii_case("installed"))
                        .unwrap_or(true)
                });

            direct_match && installed
                || object
                    .values()
                    .any(|child| json_contains_installed_model(child, model_tag))
        }
        _ => false,
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

fn build_chat_messages(question: &str, context: &ContextBundle) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system",
            content: "You are FastFlowLM running inside Winspot. Answer the user's question using the launcher index context when it is relevant. If the context is insufficient, say so briefly and answer from general knowledge.".to_string(),
        },
        ChatMessage {
            role: "user",
            content: format!(
                "Question:\n{}\n\nLauncher index context:\n{}",
                question.trim(),
                format_context_for_prompt(context)
            ),
        },
    ]
}

fn format_context_for_prompt(context: &ContextBundle) -> String {
    if context.entries.is_empty() {
        return "No index matches.".to_string();
    }

    context
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut text = format!(
                "{}. {}\nPath: {}\nKind: {:?}",
                index + 1,
                entry.title,
                entry.path,
                entry.kind
            );
            if let Some(source) = &entry.source {
                text.push_str(&format!("\nSource: {source}"));
            }
            if let Some(content) = &entry.content {
                text.push_str(&format!("\nContent:\n{content}"));
            } else if let Some(reason) = &entry.metadata_only_reason {
                text.push_str(&format!("\nContent omitted: {reason}"));
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn health_check(settings: &FastFlowLmSettings) -> anyhow::Result<()> {
    http_get(settings.port, HEALTH_PATH).map(|_| ())
}

fn request_chat_completion(
    settings: &FastFlowLmSettings,
    messages: &[ChatMessage],
) -> anyhow::Result<String> {
    let body = json!({
        "model": settings.model_tag,
        "stream": false,
        "messages": messages,
    });
    let response = http_post_json(settings.port, CHAT_COMPLETIONS_PATH, &body)?;
    let parsed: ChatCompletionResponse =
        serde_json::from_str(&response).context("parse FastFlowLM chat response")?;
    parsed
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("FastFlowLM returned an empty response"))
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

fn http_get(port: u16, path: &str) -> anyhow::Result<String> {
    http_request(port, "GET", path, None)
}

fn http_post_json(port: u16, path: &str, body: &Value) -> anyhow::Result<String> {
    http_request(port, "POST", path, Some(body.to_string()))
}

fn http_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<String>,
) -> anyhow::Result<String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, HTTP_TIMEOUT)
        .with_context(|| format!("connect to FastFlowLM at http://127.0.0.1:{port}"))?;
    stream
        .set_read_timeout(Some(HTTP_TIMEOUT))
        .context("set FastFlowLM read timeout")?;
    stream
        .set_write_timeout(Some(HTTP_TIMEOUT))
        .context("set FastFlowLM write timeout")?;

    let body_bytes = body.as_deref().unwrap_or_default().as_bytes();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nAccept: application/json\r\n"
    );
    if body.is_some() {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    }
    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .context("write FastFlowLM HTTP request headers")?;
    if body.is_some() {
        stream
            .write_all(body_bytes)
            .context("write FastFlowLM HTTP request body")?;
    }

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .context("read FastFlowLM HTTP response")?;
    let response = String::from_utf8_lossy(&response);
    let (head, response_body) = response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .ok_or_else(|| anyhow!("FastFlowLM returned a malformed HTTP response"))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| anyhow!("FastFlowLM returned a malformed HTTP status"))?;
    if !(200..300).contains(&status) {
        anyhow::bail!("FastFlowLM HTTP request failed with status {status}: {response_body}");
    }

    Ok(response_body.to_string())
}

fn default_enabled() -> bool {
    true
}

fn default_model_tag() -> String {
    DEFAULT_MODEL_TAG.to_string()
}

fn default_executable_path() -> String {
    DEFAULT_EXECUTABLE_PATH.to_string()
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_idle_timeout_seconds() -> u64 {
    DEFAULT_IDLE_TIMEOUT_SECONDS
}

fn default_max_context_files() -> usize {
    DEFAULT_MAX_CONTEXT_FILES
}

fn default_max_file_bytes() -> usize {
    DEFAULT_MAX_FILE_BYTES
}

fn default_max_context_bytes() -> usize {
    DEFAULT_MAX_CONTEXT_BYTES
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
