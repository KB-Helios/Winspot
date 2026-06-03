//! FastFlowLM launcher integration.

use std::{
    collections::HashSet,
    env, fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
const MODEL_LIST_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// Returns the default FastFlowLmSettings used when no configuration is provided.
    ///
    /// The defaults enable the integration and set the model tag, executable path, port,
    /// idle timeout, and context size limits to sensible module-level constants.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let s = crate::FastFlowLmSettings::default();
    /// assert!(s.enabled);
    /// assert_eq!(s.model_tag, crate::DEFAULT_MODEL_TAG);
    /// ```
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
    /// Normalize settings by trimming string fields and substituting defaults for empty or zero values.
    ///
    /// Trims whitespace from `model_tag` and `executable_path`; if either becomes empty after trimming it is replaced with the corresponding default. For numeric configuration fields (`port`, `idle_timeout_seconds`, `max_context_files`, `max_file_bytes`, `max_context_bytes`), a value of `0` is replaced with the corresponding default.
    ///
    /// # Returns
    ///
    /// A `FastFlowLmSettings` value with normalized string and numeric fields.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let s = FastFlowLmSettings {
    ///     model_tag: "  my-model  ".into(),
    ///     executable_path: "  /usr/bin/flm  ".into(),
    ///     port: 0,
    ///     ..Default::default()
    /// }.normalized();
    ///
    /// assert_eq!(s.model_tag, "my-model");
    /// assert_eq!(s.executable_path, "/usr/bin/flm");
    /// assert_ne!(s.port, 0);
    /// ```
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

    /// Returns the configured context limits for index-derived prompts.
    ///
    /// The resulting `ContextLimits` reflects the service's `max_context_files`,
    /// `max_file_bytes`, and `max_context_bytes` settings.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let settings = FastFlowLmSettings::default();
    /// let limits = settings.context_limits();
    /// assert_eq!(limits.max_files, settings.max_context_files);
    /// assert_eq!(limits.max_file_bytes, settings.max_file_bytes);
    /// assert_eq!(limits.max_context_bytes, settings.max_context_bytes);
    /// ```
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
    /// Create `ContextLimits` populated with the module's default caps.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let limits = ContextLimits::default();
    /// assert_eq!(limits.max_files, DEFAULT_MAX_CONTEXT_FILES);
    /// assert_eq!(limits.max_file_bytes, DEFAULT_MAX_FILE_BYTES);
    /// assert_eq!(limits.max_context_bytes, DEFAULT_MAX_CONTEXT_BYTES);
    /// ```
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
    /// Converts a launcher query into a FastFlowLM plugin search result when the query is an invocation prompt.
    ///
    /// If the query begins with a recognized invocation prefix (for example "ai " or "ask "), the prompt
    /// portion is extracted and returned as a single `SearchResult` that triggers the FastFlowLM plugin.
    /// Otherwise, an empty vector is returned.
    ///
    /// # Returns
    ///
    /// A single-element `Vec<SearchResult>` containing a plugin result with the extracted prompt when the
    /// query targets FastFlowLM, or an empty `Vec` if the query is not a FastFlowLM invocation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Construct the provider (type shown for clarity; actual construction may vary).
    /// let provider = FastFlowLmProvider;
    /// let results = provider.search("ask Tell me a short joke");
    /// assert_eq!(results.len(), 1);
    /// assert_eq!(results[0].title, "Tell me a short joke");
    /// ```
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
    /// Create a new FastFlowLmService with normalized settings and a shared index store.
    ///
    /// The provided `settings` are normalized before use; an internal `FastFlowLmManager` is
    /// initialized from those normalized settings. The `index_store` is wrapped in a
    /// thread-safe `Arc<Mutex<_>>` for shared access by the service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let settings = FastFlowLmSettings::default();
    /// let index_store = IndexStore::default();
    /// let svc = FastFlowLmService::new(settings, index_store);
    /// assert!(svc.settings.model_tag.len() > 0);
    /// ```
    pub fn new(settings: FastFlowLmSettings, index_store: IndexStore) -> Self {
        let settings = settings.normalized();
        Self {
            manager: FastFlowLmManager::new(settings.clone()),
            settings,
            index_store: Arc::new(Mutex::new(index_store)),
        }
    }

    /// Ask the configured FastFlowLM model a question using launcher index context.
    ///
    /// Builds an index-derived context for the trimmed `question` and forwards the request to the FastFlowLM manager, returning the model's answer.
    ///
    /// # Returns
    ///
    /// The model's answer as a `String`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the FastFlowLM integration is disabled in the settings,
    /// - the trimmed `question` is empty,
    /// - the launcher index is unavailable or locked,
    /// - or any error occurs while building context or communicating with the model.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Construct and use a FastFlowLmService (construction omitted).
    /// let svc = /* FastFlowLmService::new(...) */;
    /// let answer = svc.ask("Summarize the repository README.").unwrap();
    /// println!("{}", answer);
    /// ```
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
    startup_lock: Mutex<()>,
}

#[derive(Default)]
struct ProcessStateInner {
    child: Option<Child>,
    last_used_unix_seconds: u64,
    reaper_running: bool,
}

impl FastFlowLmManager {
    /// Creates a new `FastFlowLmManager` with normalized settings and an initialized process state.
    ///
    /// The returned manager holds a cloned, normalized copy of `settings` and a shared `ProcessState`
    /// with default `ProcessStateInner` and a `Condvar` used by the idle reaper.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mgr = FastFlowLmManager::new(FastFlowLmSettings::default());
    /// assert!(mgr.settings.enabled);
    /// ```
    pub fn new(settings: FastFlowLmSettings) -> Self {
        Self {
            settings: settings.normalized(),
            state: Arc::new(ProcessState {
                inner: Mutex::new(ProcessStateInner::default()),
                idle_changed: Condvar::new(),
                startup_lock: Mutex::new(()),
            }),
        }
    }

    /// Ask the configured FastFlowLM model a question, providing index-derived context to inform the response.
    ///
    /// The method ensures the requested model is installed, starts or verifies a local FastFlowLM server if needed, sends the assembled chat request (including the provided context), and updates the manager's idle timestamp.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Assume `manager` is a properly constructed `FastFlowLmManager`
    /// // and `context` is a `ContextBundle` built from the index.
    /// let answer = manager.ask("Summarize the following files", &context).unwrap();
    /// assert!(!answer.trim().is_empty());
    /// ```
    ///
    /// # Returns
    ///
    /// The model's response text on success.
    pub fn ask(&self, question: &str, context: &ContextBundle) -> anyhow::Result<String> {
        validate_installed_model(&self.settings)?;
        self.ensure_server()?;
        let messages = build_chat_messages(question, context);
        self.touch_used();
        let answer = request_chat_completion(&self.settings, &messages)?;
        self.touch_used();
        Ok(answer)
    }

    /// Starts a local FastFlowLM server if one is not already healthy and waits for it to become ready.
    ///
    /// Attempts a HTTP health check first; if the check fails, this will stop any previously
    /// owned process, spawn the configured FastFlowLM executable with serve arguments, record
    /// ownership, start the idle reaper, and poll the server until it responds or the startup
    /// timeout elapses.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the server is already healthy or was started and became ready; `Err` if the
    /// executable could not be spawned or the server did not become ready before the startup timeout.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use winspot_fastflowlm::{FastFlowLmManager, FastFlowLmSettings};
    ///
    /// let settings = FastFlowLmSettings::default();
    /// let manager = FastFlowLmManager::new(settings);
    /// // This may spawn the FastFlowLM process and wait for it to become healthy.
    /// let _ = manager.ensure_server();
    /// ```
    fn ensure_server(&self) -> anyhow::Result<()> {
        let _startup = self
            .state
            .startup_lock
            .lock()
            .map_err(|_| anyhow!("FastFlowLM startup state is unavailable"))?;

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

    /// Stops and clears any child server process owned by this manager.
    ///
    /// If a child process is present, it is sent a kill signal and waited on; the manager's
    /// ownership is then cleared and waiting threads are notified via the condition variable.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Obtain a FastFlowLmManager instance (omitted).
    /// // Calling this will terminate and forget any process the manager started.
    /// manager.stop_stale_owned_process();
    /// ```
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

    /// Updates the manager's last-used timestamp to the current time and notifies the idle reaper.
    ///
    /// This marks the process as recently used so the idle reaper will delay stopping it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Assume `manager` is a `FastFlowLmManager`.
    /// // Calling `touch_used()` records activity and wakes any waiting reaper thread.
    /// manager.touch_used();
    /// ```
    fn touch_used(&self) {
        if let Ok(mut state) = self.state.inner.lock() {
            state.last_used_unix_seconds = current_unix_seconds();
            self.state.idle_changed.notify_all();
        }
    }

    /// Starts a background thread that monitors the owned FastFlowLM process and stops it after the configured idle timeout.
    ///
    /// The reaper will be started only once while no other reaper is running. The thread:
    /// - exits if there is no owned child process,
    /// - kills and waits for the child process when the idle timeout is exceeded,
    /// - otherwise sleeps on a condition variable and wakes when activity is recorded or the timeout elapses.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mgr = FastFlowLmManager::new(FastFlowLmSettings::default());
    /// mgr.start_idle_reaper();
    /// ```
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

/// Extracts a prompt following an `ai` or `ask` prefix.
///
/// The function recognizes the case-insensitive prefixes `"ai "` and `"ask "` at the
/// start of `query`, trims surrounding whitespace, and returns the remainder as a
/// `&str` if it is non-empty. Returns `None` if no recognized prefix is present or
/// the extracted prompt is empty after trimming.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(parse_fastflowlm_prompt("ai Hello, world!"), Some("Hello, world!"));
/// assert_eq!(parse_fastflowlm_prompt("Ask   What is Rust?  "), Some("What is Rust?"));
/// assert_eq!(parse_fastflowlm_prompt("hello there"), None);
/// assert_eq!(parse_fastflowlm_prompt("ai   "), None);
/// ```
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

/// Builds a ContextBundle from index search results for a query, reading file contents where permitted by the provided context limits.
///
/// The function searches the index for up to `limits.max_files` matches for `query`, then for each file result attempts to read its contents subject to `limits.max_file_bytes` and `limits.max_context_bytes`. File reads that cannot be included produce entries with `metadata_only_reason` instead of `content`.
///
/// # Returns
///
/// A `ContextBundle` containing the collected `ContextEntry` items and `total_content_bytes` reflecting the sum of included file contents.
///
/// # Examples
///
/// ```ignore
/// use winspot_fastflowlm::{build_index_context, ContextLimits, IndexStore};
/// // Assume `store` is an existing IndexStore and `query` is the user's query.
/// let store: IndexStore = /* obtain or construct index store */ unimplemented!();
/// let query = "explain async runtime";
/// let limits = ContextLimits { max_files: 5, max_file_bytes: 16_384, max_context_bytes: 65_536 };
/// let bundle = build_index_context(&store, query, limits).expect("build context");
/// assert!(bundle.entries.len() <= 5);
/// ```
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

/// Searches the index for results matching `query`, then expands the results by searching
/// individual query tokens until `limit` unique results are collected.
///
/// The function first performs a search using the full `query`. If the number of results
/// is less than `limit`, it tokenizes `query` and performs additional searches for each
/// token, appending unseen results until the `limit` is reached or no more matches are found.
/// Errors from the underlying index store are propagated.
///
/// # Returns
///
/// A vector of up to `limit` unique `SearchResult` entries matching the query or its tokens.
///
/// # Examples
///
/// ```ignore
/// // Assume `store` is an initialized IndexStore and `search_index_for_context` is in scope.
/// let results = search_index_for_context(&store, "fast model inference", 10).unwrap();
/// assert!(results.len() <= 10);
/// ```
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

/// Produces query tokens by splitting on non-alphanumeric characters (except `-` and `_`).
///
/// The iterator yields trimmed substrings of length at least 3. Splitting treats any character
/// that is not an ASCII alphanumeric, `-`, or `_` as a separator.
///
/// # Examples
///
/// ```ignore
/// let tokens: Vec<&str> = crate::query_tokens("find: fast-flow_lm v1.2 beta").collect();
/// assert_eq!(tokens, vec!["find", "fast-flow_lm", "beta"]);
/// ```
fn query_tokens(query: &str) -> impl Iterator<Item = &str> {
    query
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .map(str::trim)
        .filter(|token| token.len() >= 3)
}

/// Verifies that a FastFlowLM model with the given tag is present in `flm list --json` output.
///
/// Parses the provided JSON string and succeeds if it contains an entry identifying the
/// model tag as installed; returns an error if parsing fails or if no installed model
/// matching `model_tag` is found.
///
/// # Examples
///
/// ```ignore
/// let json = r#"[{ "name": "my-model", "installed": true }]"#;
/// assert!(validate_installed_model_from_list_json(json, "my-model").is_ok());
/// ```
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

pub fn validate_models_response_json(json_output: &str, model_tag: &str) -> anyhow::Result<()> {
    let value: Value =
        serde_json::from_str(json_output).context("parse FastFlowLM /v1/models response")?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("FastFlowLM /v1/models response did not include a data array"))?;

    let contains_model = data.iter().any(|item| {
        ["id", "name", "model", "tag"]
            .iter()
            .filter_map(|key| item.get(*key).and_then(Value::as_str))
            .any(|candidate| candidate == model_tag)
    });
    if contains_model {
        return Ok(());
    }

    anyhow::bail!("FastFlowLM /v1/models response did not include configured model '{model_tag}'");
}

/// Determine whether an owned process has been idle long enough to be stopped.
///
/// `owns_process` indicates whether the caller currently owns the process. `last_used_unix_seconds`
/// and `now_unix_seconds` are Unix timestamps in seconds; `idle_timeout_seconds` is the allowed idle
/// duration in seconds. The function uses saturating subtraction to avoid underflow when computing
/// the elapsed time.
///
/// # Examples
///
/// ```ignore
/// // not owned -> don't stop
/// assert_eq!(should_stop_owned_process(false, 100, 200, 50), false);
/// // owned but not yet timed out -> don't stop
/// assert_eq!(should_stop_owned_process(true, 160, 200, 50), false);
/// // owned and timed out -> stop
/// assert_eq!(should_stop_owned_process(true, 100, 200, 50), true);
/// ```
///
/// # Returns
///
/// `true` if `owns_process` is `true` and `now_unix_seconds - last_used_unix_seconds` is greater than
/// or equal to `idle_timeout_seconds`, `false` otherwise.
pub fn should_stop_owned_process(
    owns_process: bool,
    last_used_unix_seconds: u64,
    now_unix_seconds: u64,
    idle_timeout_seconds: u64,
) -> bool {
    owns_process && now_unix_seconds.saturating_sub(last_used_unix_seconds) >= idle_timeout_seconds
}

/// Load FastFlowLM settings from a JSON settings file, falling back to defaults when the file is missing.
///
/// Attempts to read and parse the file at `path` as JSON containing an optional `fastFlowLm` object.
/// If the file does not exist, returns `FastFlowLmSettings::default()`. The returned settings are
/// normalized (trimmed/zero values replaced by defaults).
///
/// # Parameters
///
/// - `path`: Path to a JSON settings file that may contain a `fastFlowLm` section.
///
/// # Returns
///
/// A `FastFlowLmSettings` instance from the file or the default settings when the file is absent.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// let settings = winspot_fastflowlm::load_settings_from_path(Path::new("nonexistent.json")).unwrap();
/// // defaults are enabled by default
/// assert!(settings.enabled);
/// ```
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

/// Finds the conventional location for Winspot's settings.json on the current machine.
///
/// Checks for a portable installation next to the running executable first:
/// if a sibling file named `Winspot.portable` exists, returns `<exe_parent>/data/settings.json`.
/// Otherwise, falls back to `%LOCALAPPDATA%\Winspot\settings.json` when `LOCALAPPDATA` is set.
///
/// # Returns
///
/// `Some(PathBuf)` with the resolved settings.json path if a portable layout is detected or
/// `LOCALAPPDATA` is present; `None` if neither location can be determined.
///
/// # Examples
///
/// ```ignore
/// if let Some(path) = default_settings_path() {
///     // Use the discovered settings path
///     println!("{}", path.display());
/// }
/// ```
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

/// Determines the default filesystem path for the Winspot index, if one can be inferred.
///
/// Prefer the portable layout next to the running executable: if a sibling file named
/// `Winspot.portable` exists, returns `<executable_parent>/data/index.sqlite`. If no portable
/// layout is detected, returns `%LOCALAPPDATA%\Winspot\index.sqlite` when the `LOCALAPPDATA`
/// environment variable is set.
///
/// # Examples
///
/// ```ignore
/// if let Some(path) = default_index_path() {
///     // Found a candidate index path
///     assert!(path.file_name().and_then(|n| n.to_str()) == Some("index.sqlite"));
/// } else {
///     // No default path could be determined (e.g., LOCALAPPDATA not set)
///     assert!(true);
/// }
/// ```
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

/// Attempts to read a text content preview for a file while enforcing per-file and aggregate context size limits.
///
/// Given a file `path`, returns either `FileContextRead::Content(String)` containing the file's UTF-8 text
/// when the file is readable and within the provided `limits`, or `FileContextRead::MetadataOnly(String)` with
/// a short reason when the file must be omitted from the assembled context.
///
/// Parameters:
/// - `path`: filesystem path to the candidate context file.
/// - `limits`: `ContextLimits` specifying `max_file_bytes` and `max_context_bytes` caps.
/// - `current_total_bytes`: number of bytes already accumulated into the context bundle; used to enforce the aggregate cap.
///
/// Return value:
/// - `FileContextRead::Content(content)` when the file is a regular file, does not exceed per-file or aggregate caps,
///   and its contents are valid UTF-8 (no embedded NUL bytes).
/// - `FileContextRead::MetadataOnly(reason)` when the file is unreadable, not a regular file, exceeds size caps,
///   or appears binary / non-UTF-8; the `reason` is a short machine-oriented string describing why content was omitted.
///
/// # Examples
///
/// ```ignore
/// use std::fs;
/// use std::io::Write;
/// use tempfile::NamedTempFile;
/// // create a small temporary UTF-8 file and read it with generous limits
/// let mut f = NamedTempFile::new().unwrap();
/// writeln!(f, "hello world").unwrap();
/// let path = f.path();
/// let limits = crate::ContextLimits { max_files: 10, max_file_bytes: 1024, max_context_bytes: 4096 };
/// match crate::read_context_file(path, limits, 0) {
///     crate::FileContextRead::Content(s) => assert!(s.contains("hello")),
///     crate::FileContextRead::MetadataOnly(_) => panic!("expected content"),
/// }
/// ```
fn read_context_file(
    path: &Path,
    limits: ContextLimits,
    current_total_bytes: usize,
) -> FileContextRead {
    let Ok(file) = fs::File::open(path) else {
        return FileContextRead::MetadataOnly("unreadable file".to_string());
    };
    let Ok(metadata) = file.metadata() else {
        return FileContextRead::MetadataOnly("unreadable file".to_string());
    };

    if !metadata.is_file() {
        return FileContextRead::MetadataOnly("not a regular file".to_string());
    }
    let file_bytes = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if file_bytes > limits.max_file_bytes {
        return FileContextRead::MetadataOnly("file exceeds per-file content cap".to_string());
    }
    let remaining_context_bytes = limits.max_context_bytes.saturating_sub(current_total_bytes);
    if file_bytes > remaining_context_bytes {
        return FileContextRead::MetadataOnly("aggregate context cap reached".to_string());
    }

    let allowed_bytes = limits.max_file_bytes.min(remaining_context_bytes);
    let read_limit = u64::try_from(allowed_bytes.saturating_add(1)).unwrap_or(u64::MAX);
    let mut bytes = Vec::with_capacity(file_bytes.min(allowed_bytes));
    let mut limited_file = file.take(read_limit);
    if limited_file.read_to_end(&mut bytes).is_err() {
        return FileContextRead::MetadataOnly("unreadable file".to_string());
    }
    if bytes.len() > allowed_bytes {
        let reason = if allowed_bytes == remaining_context_bytes
            && remaining_context_bytes < limits.max_file_bytes
        {
            "aggregate context cap reached"
        } else {
            "file exceeds per-file content cap"
        };
        return FileContextRead::MetadataOnly(reason.to_string());
    }
    if bytes.contains(&0) {
        return FileContextRead::MetadataOnly("binary or non-UTF-8 file".to_string());
    }
    match String::from_utf8(bytes) {
        Ok(content) => FileContextRead::Content(content),
        Err(_) => FileContextRead::MetadataOnly("binary or non-UTF-8 file".to_string()),
    }
}

/// Checks that the configured FastFlowLM model is installed by running
/// the configured executable with `list --json` and inspecting its output.
///
/// On success, returns `Ok(())`. Returns an error if the executable cannot be
/// run, if it exits with a non-zero status, if its stdout is not valid UTF-8,
/// or if the JSON output does not indicate the requested model tag is installed.
///
/// # Examples
///
/// ```ignore
/// let settings = FastFlowLmSettings {
///     model_tag: "my-model:latest".into(),
///     executable_path: "flm".into(),
///     ..Default::default()
/// };
/// validate_installed_model(&settings).unwrap();
/// ```
fn validate_installed_model(settings: &FastFlowLmSettings) -> anyhow::Result<()> {
    let output = run_flm_list_json(settings, MODEL_LIST_TIMEOUT)?;

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

fn run_flm_list_json(settings: &FastFlowLmSettings, timeout: Duration) -> anyhow::Result<Output> {
    let mut child = Command::new(&settings.executable_path)
        .args(["list", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "failed to run '{} list --json' to validate FastFlowLM model",
                settings.executable_path
            )
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture FastFlowLM model-list stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture FastFlowLM model-list stderr"))?;
    let stdout_reader = std::thread::spawn(move || {
        let mut reader = stdout;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut reader = stderr;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map(|_| bytes)
    });

    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .context("wait for FastFlowLM model-list command")?
        {
            return Ok(Output {
                status,
                stdout: join_reader(stdout_reader, "stdout")?,
                stderr: join_reader(stderr_reader, "stderr")?,
            });
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            drop(stdout_reader);
            drop(stderr_reader);
            anyhow::bail!(
                "'{} list --json' timed out after {} ms",
                settings.executable_path,
                timeout.as_millis()
            );
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

fn join_reader(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream_name: &str,
) -> anyhow::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("FastFlowLM model-list {stream_name} reader panicked"))?
        .with_context(|| format!("read FastFlowLM model-list {stream_name}"))
}

/// Checks whether a JSON structure contains an installed model with the given tag.
///
/// This performs a recursive search through arrays and objects. An object is considered a
/// match when any of its string fields `name`, `model`, `tag`, or `id` equals `model_tag`
/// and the object is considered installed. An object is considered installed if it has
/// a boolean `installed` field set to `true`, or a string `status` field equal to
/// `"installed"` (case-insensitive). If neither `installed` nor `status` are present,
/// the object is treated as installed by default.
///
/// # Examples
///
/// ```ignore
/// use serde_json::json;
/// // direct match with installed = true
/// let v = json!({ "name": "foo", "installed": true });
/// assert!(json_contains_installed_model(&v, "foo"));
///
/// // nested match under arrays/objects
/// let v = json!({ "models": [{ "tag": "bar", "status": "installed" }] });
/// assert!(json_contains_installed_model(&v, "bar"));
///
/// // not installed
/// let v = json!({ "id": "baz", "installed": false });
/// assert!(!json_contains_installed_model(&v, "baz"));
/// ```
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

/// Builds the chat messages sent to FastFlowLM for a question and the assembled launcher index context.
///
/// Returns a two-element vector: a system message that instructs the model to prefer launcher index context when relevant, and a user message containing the trimmed question followed by the formatted context.
///
/// # Examples
///
/// ```ignore
/// let ctx = ContextBundle { entries: vec![], total_content_bytes: 0 };
/// let msgs = build_chat_messages("Who wrote the README?", &ctx);
/// assert_eq!(msgs.len(), 2);
/// assert_eq!(msgs[0].role, "system");
/// assert!(msgs[0].content.contains("FastFlowLM"));
/// assert_eq!(msgs[1].role, "user");
/// assert!(msgs[1].content.contains("Question:"));
/// ```
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

/// Format a ContextBundle into a human-readable string suitable for embedding in a chat prompt.
///
/// # Examples
///
/// ```ignore
/// use std::collections::HashMap;
///
/// let entry = crate::ContextEntry {
///     title: "Example".to_string(),
///     path: "/path/to/file".to_string(),
///     kind: crate::ContextEntryKind::File, // adjust variant name to actual enum in this crate
///     source: Some("index".to_string()),
///     content: Some("file contents".to_string()),
///     metadata_only_reason: None,
/// };
/// let bundle = crate::ContextBundle {
///     entries: vec![entry],
///     total_content_bytes: 13,
/// };
/// let formatted = crate::format_context_for_prompt(&bundle);
/// assert!(formatted.contains("1. Example"));
/// assert!(formatted.contains("Path: /path/to/file"));
/// assert!(formatted.contains("Content:\nfile contents"));
///
/// let empty = crate::ContextBundle { entries: vec![], total_content_bytes: 0 };
/// assert_eq!(crate::format_context_for_prompt(&empty), "No index matches.");
/// ```
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

/// Perform a liveness probe against the FastFlowLM HTTP health endpoint.
///
/// Checks the configured port on `settings` by issuing an HTTP GET to the health path
/// and returns `Ok(())` when the service responds with a successful HTTP status; returns
/// an error for connection failures or non-successful HTTP responses.
///
/// # Examples
///
/// ```ignore
/// let settings = FastFlowLmSettings::default();
/// // Succeeds when a FastFlowLM server is reachable on `settings.port`.
/// let _ = health_check(&settings);
/// ```
fn health_check(settings: &FastFlowLmSettings) -> anyhow::Result<()> {
    let response = http_get(settings.port, HEALTH_PATH)?;
    validate_models_response_json(&response, &settings.model_tag)
}

/// Send a chat completion request to the local FastFlowLM server and return the model's reply.
///
/// The function posts the provided messages as a JSON body to the FastFlowLM chat completions endpoint,
/// parses the JSON response, and returns the trimmed content of the first choice's message. It fails
/// if the response is malformed or the first choice's content is empty or missing.
///
/// # Returns
///
/// `String` containing the trimmed content of the first choice's message, or an error if the response
/// cannot be parsed or contains no non-empty content.
///
/// # Examples
///
/// ```ignore
/// use serde_json::json;
///
/// // Construct a minimal settings and messages (values shown for illustration).
/// let settings = FastFlowLmSettings {
///     model_tag: "gpt-like-model".to_string(),
///     ..Default::default()
/// };
/// let messages = vec![ChatMessage { role: "user", content: "Hello, what's up?".to_string() }];
///
/// // Send the request (requires a running FastFlowLM server at settings.port).
/// let reply = request_chat_completion(&settings, &messages).expect("request failed");
/// println!("model reply: {}", reply);
/// ```
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

/// Performs an HTTP GET to the local FastFlowLM HTTP server and returns the response body.
///
/// The function contacts 127.0.0.1 on the given port and requests the provided path. It returns the
/// response body when the server responds with a 2xx status; it returns an error for connection,
/// timeout, or non-successful HTTP responses.
///
/// # Examples
///
/// ```ignore
/// let body = http_get(52625, "/v1/models").unwrap();
/// assert!(body.len() > 0);
/// ```
fn http_get(port: u16, path: &str) -> anyhow::Result<String> {
    http_request(port, "GET", path, None)
}

/// Sends a JSON POST request to the local FastFlowLM HTTP server and returns the response body.
///
/// The JSON `body` is serialized and sent to `127.0.0.1:<port><path>` using a blocking TCP request; the function returns the response body as a string when the server responds with a successful (2xx) status code.
///
/// # Examples
///
/// ```ignore
/// use serde_json::json;
///
/// let body = json!({ "model": "gpt", "messages": [] });
/// let resp = winspot_fastflowlm::http_post_json(52625, "/v1/chat/completions", &body).unwrap();
/// assert!(!resp.is_empty());
/// ```
fn http_post_json(port: u16, path: &str, body: &Value) -> anyhow::Result<String> {
    http_request(port, "POST", path, Some(body.to_string()))
}

/// Send a simple HTTP/1.1 request to the local FastFlowLM server and return its response body.
///
/// This opens a TCP connection to 127.0.0.1:<port>, sends an HTTP request with the given method,
/// path, and optional JSON body, then reads and validates the HTTP response. Errors are returned
/// if the connection fails, timeouts occur, the response is malformed, or the HTTP status is not
/// in the 200..=299 range.
///
/// # Examples
///
/// ```ignore
/// let body = None;
/// let resp = http_request(52625, "GET", "/v1/models", body).expect("request failed");
/// println!("FastFlowLM response body: {}", resp);
/// ```
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

/// Default value for the `enabled` setting.
///
/// # Returns
///
/// `true` — the default enabled state.
///
/// # Examples
///
/// ```ignore
/// assert!(default_enabled());
/// ```
fn default_enabled() -> bool {
    true
}

/// Default model tag used when no model tag is provided in settings.
///
/// # Examples
///
/// ```ignore
/// let tag = default_model_tag();
/// assert!(!tag.is_empty());
/// ```
fn default_model_tag() -> String {
    DEFAULT_MODEL_TAG.to_string()
}

/// Default executable path used to run the FastFlowLM binary.
///
/// # Returns
///
/// A `String` containing the default executable path.
///
/// # Examples
///
/// ```ignore
/// let path = default_executable_path();
/// assert_eq!(path, DEFAULT_EXECUTABLE_PATH.to_string());
/// ```
fn default_executable_path() -> String {
    DEFAULT_EXECUTABLE_PATH.to_string()
}

/// Default TCP port used to contact the FastFlowLM server.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(default_port(), 52625);
/// ```
fn default_port() -> u16 {
    DEFAULT_PORT
}

/// Default idle timeout used by the manager's idle reaper, in seconds.
///
/// # Returns
///
/// The default number of seconds a spawned FastFlowLM process may remain idle before being stopped.
///
/// # Examples
///
/// ```ignore
/// let timeout = default_idle_timeout_seconds();
/// assert!(timeout >= 1);
/// ```
fn default_idle_timeout_seconds() -> u64 {
    DEFAULT_IDLE_TIMEOUT_SECONDS
}

/// Default maximum number of context files allowed.
///
/// # Returns
///
/// The default cap for the number of context files included in a model context bundle.
///
/// # Examples
///
/// ```ignore
/// let cap = default_max_context_files();
/// assert!(cap > 0);
/// ```
fn default_max_context_files() -> usize {
    DEFAULT_MAX_CONTEXT_FILES
}

/// Default per-file context size cap in bytes.
///
/// # Returns
///
/// The default maximum number of bytes allowed for a single context file.
///
/// # Examples
///
/// ```ignore
/// let cap = default_max_file_bytes();
/// assert_eq!(cap, DEFAULT_MAX_FILE_BYTES);
/// ```
fn default_max_file_bytes() -> usize {
    DEFAULT_MAX_FILE_BYTES
}

/// Default maximum allowed aggregate context size in bytes.
///
/// # Examples
///
/// ```ignore
/// let cap = default_max_context_bytes();
/// assert!(cap > 0);
/// ```
fn default_max_context_bytes() -> usize {
    DEFAULT_MAX_CONTEXT_BYTES
}

/// Get the current Unix time as whole seconds since the Unix epoch.
///
/// Returns the number of seconds elapsed since 1970-01-01T00:00:00Z. If the system time is before the Unix epoch or cannot be determined, returns 0.
///
/// # Examples
///
/// ```ignore
/// let secs = current_unix_seconds();
/// // `secs` is the number of seconds since the Unix epoch (or 0 on error)
/// assert!(secs >= 0);
/// ```
fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn run_flm_list_json_when_command_hangs_times_out() {
        let root =
            std::env::temp_dir().join(format!("winspot-fastflowlm-timeout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        let fake_flm = root.join("slow-flm.cmd");
        fs::write(&fake_flm, "@echo off\r\nping -n 6 127.0.0.1 >NUL\r\n").expect("write fake flm");
        let settings = FastFlowLmSettings {
            executable_path: fake_flm.display().to_string(),
            ..FastFlowLmSettings::default()
        };

        let started = std::time::Instant::now();
        let error = run_flm_list_json(&settings, Duration::from_millis(100))
            .expect_err("hung command should time out");

        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
