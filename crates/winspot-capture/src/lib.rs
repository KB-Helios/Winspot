//! Windows capture launcher integration.

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use winspot_core::{
    ActionCapability, ActionDescriptor, ActionKind, SearchResult, SearchResultKind,
};
use winspot_search::providers::DynamicSearchProvider;

pub const WINDOWS_CAPTURE_PLUGIN_ID: &str = "windows-capture";
pub const WINDOWS_CAPTURE_RESULT_PREFIX: &str = "plugin:windows-capture:";
pub const DEFAULT_RECORD_SECONDS: u64 = 8;
pub const DEFAULT_MAX_RECORD_SECONDS: u64 = 60;
pub const DEFAULT_PRE_CAPTURE_DELAY_MS: u64 = 250;

const HARD_MAX_RECORD_SECONDS: u64 = 300;
const HARD_MAX_PRE_CAPTURE_DELAY_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsCaptureSettings {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub output_directory: String,
    #[serde(default = "default_record_seconds")]
    pub default_record_seconds: u64,
    #[serde(default = "default_max_record_seconds")]
    pub max_record_seconds: u64,
    #[serde(default = "default_include_cursor")]
    pub include_cursor: bool,
    #[serde(default = "default_pre_capture_delay_ms")]
    pub pre_capture_delay_ms: u64,
}

impl Default for WindowsCaptureSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            output_directory: String::new(),
            default_record_seconds: DEFAULT_RECORD_SECONDS,
            max_record_seconds: DEFAULT_MAX_RECORD_SECONDS,
            include_cursor: true,
            pre_capture_delay_ms: DEFAULT_PRE_CAPTURE_DELAY_MS,
        }
    }
}

impl WindowsCaptureSettings {
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.output_directory = self.output_directory.trim().to_string();
        if self.max_record_seconds == 0 {
            self.max_record_seconds = defaults.max_record_seconds;
        }
        self.max_record_seconds = self.max_record_seconds.clamp(1, HARD_MAX_RECORD_SECONDS);
        if self.default_record_seconds == 0 {
            self.default_record_seconds = defaults.default_record_seconds;
        }
        self.default_record_seconds = self
            .default_record_seconds
            .clamp(1, self.max_record_seconds);
        self.pre_capture_delay_ms = self.pre_capture_delay_ms.min(HARD_MAX_PRE_CAPTURE_DELAY_MS);
        self
    }

    pub fn clamp_record_seconds(&self, seconds: u64) -> u64 {
        seconds.clamp(1, self.max_record_seconds.max(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Screenshot,
    Record { seconds: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSelection {
    Primary,
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    Monitor(MonitorSelection),
    ForegroundWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCommand {
    pub mode: CaptureMode,
    pub target: CaptureTarget,
}

impl CaptureCommand {
    pub const fn screenshot(target: CaptureTarget) -> Self {
        Self {
            mode: CaptureMode::Screenshot,
            target,
        }
    }

    pub const fn record(target: CaptureTarget, seconds: u64) -> Self {
        Self {
            mode: CaptureMode::Record { seconds },
            target,
        }
    }

    pub fn result_id(&self) -> String {
        let mode = match self.mode {
            CaptureMode::Screenshot => "screenshot".to_string(),
            CaptureMode::Record { seconds } => format!("record:{}", seconds),
        };
        let target = match self.target {
            CaptureTarget::Monitor(MonitorSelection::Primary) => "monitor:primary".to_string(),
            CaptureTarget::Monitor(MonitorSelection::Index(index)) => format!("monitor:{index}"),
            CaptureTarget::ForegroundWindow => "window:foreground".to_string(),
        };

        match self.mode {
            CaptureMode::Screenshot => format!("{WINDOWS_CAPTURE_RESULT_PREFIX}{mode}:{target}"),
            CaptureMode::Record { .. } => {
                format!(
                    "{WINDOWS_CAPTURE_RESULT_PREFIX}record:{target}:{}",
                    self.record_seconds()
                )
            }
        }
    }

    pub fn title(&self) -> String {
        match (self.mode, self.target) {
            (CaptureMode::Screenshot, CaptureTarget::Monitor(MonitorSelection::Primary)) => {
                "Screenshot primary monitor".to_string()
            }
            (CaptureMode::Screenshot, CaptureTarget::Monitor(MonitorSelection::Index(index))) => {
                format!("Screenshot monitor {index}")
            }
            (CaptureMode::Screenshot, CaptureTarget::ForegroundWindow) => {
                "Screenshot foreground window".to_string()
            }
            (
                CaptureMode::Record { seconds },
                CaptureTarget::Monitor(MonitorSelection::Primary),
            ) => {
                format!("Record primary monitor for {seconds}s")
            }
            (
                CaptureMode::Record { seconds },
                CaptureTarget::Monitor(MonitorSelection::Index(index)),
            ) => {
                format!("Record monitor {index} for {seconds}s")
            }
            (CaptureMode::Record { seconds }, CaptureTarget::ForegroundWindow) => {
                format!("Record foreground window for {seconds}s")
            }
        }
    }

    pub fn action_label(&self) -> &'static str {
        match self.mode {
            CaptureMode::Screenshot => "Capture",
            CaptureMode::Record { .. } => "Record",
        }
    }

    pub fn record_seconds(&self) -> u64 {
        match self.mode {
            CaptureMode::Screenshot => 0,
            CaptureMode::Record { seconds } => seconds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOutcome {
    pub command: CaptureCommand,
    pub output_path: String,
}

pub trait CaptureBackend: Send + Sync {
    fn capture(
        &self,
        command: &CaptureCommand,
        settings: &WindowsCaptureSettings,
        output_path: &Path,
    ) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct CaptureService {
    settings: WindowsCaptureSettings,
    backend: Arc<dyn CaptureBackend>,
}

impl CaptureService {
    pub fn new(settings: WindowsCaptureSettings) -> Self {
        Self::with_backend(settings, Arc::new(WindowsCaptureBackend))
    }

    pub fn with_backend(
        settings: WindowsCaptureSettings,
        backend: Arc<dyn CaptureBackend>,
    ) -> Self {
        Self {
            settings: settings.normalized(),
            backend,
        }
    }

    pub fn execute_result_id(&self, result_id: &str) -> anyhow::Result<CaptureOutcome> {
        if !self.settings.enabled {
            anyhow::bail!("Windows Capture integration is disabled");
        }

        let command = parse_capture_result_id(result_id, &self.settings)?;
        let output_dir = self.output_directory()?;
        let output_path =
            artifact_path_with_timestamp(&output_dir, command.mode, current_unix_millis());
        self.backend
            .capture(&command, &self.settings, &output_path)
            .with_context(|| format!("capture {}", command.title()))?;

        Ok(CaptureOutcome {
            command,
            output_path: output_path.display().to_string(),
        })
    }

    fn output_directory(&self) -> anyhow::Result<PathBuf> {
        if !self.settings.output_directory.trim().is_empty() {
            return Ok(PathBuf::from(&self.settings.output_directory));
        }
        default_output_directory()
            .ok_or_else(|| anyhow!("Windows Capture output directory could not be resolved"))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsCaptureBackend;

impl CaptureBackend for WindowsCaptureBackend {
    fn capture(
        &self,
        command: &CaptureCommand,
        settings: &WindowsCaptureSettings,
        output_path: &Path,
    ) -> anyhow::Result<()> {
        if settings.pre_capture_delay_ms > 0 {
            thread::sleep(Duration::from_millis(settings.pre_capture_delay_ms));
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create capture output directory {}", parent.display()))?;
        }
        capture_with_windows_capture(command, settings, output_path)
    }
}

#[derive(Debug, Clone)]
pub struct WindowsCaptureProvider {
    settings: WindowsCaptureSettings,
}

impl WindowsCaptureProvider {
    pub fn new(settings: WindowsCaptureSettings) -> Self {
        Self {
            settings: settings.normalized(),
        }
    }
}

impl Default for WindowsCaptureProvider {
    fn default() -> Self {
        Self::new(WindowsCaptureSettings::default())
    }
}

impl DynamicSearchProvider for WindowsCaptureProvider {
    fn search(&self, query: &str) -> Vec<SearchResult> {
        if !self.settings.enabled {
            return Vec::new();
        }
        let Some(command) = parse_capture_query(query, &self.settings) else {
            return Vec::new();
        };

        vec![SearchResult {
            id: command.result_id(),
            title: command.title(),
            subtitle: match command.mode {
                CaptureMode::Screenshot => "Save a screenshot with Windows Capture".to_string(),
                CaptureMode::Record { .. } => {
                    "Save a bounded MP4 recording with Windows Capture".to_string()
                }
            },
            kind: SearchResultKind::Plugin,
            score: 1.0,
            primary_action: ActionKind::PluginCommand,
            actions: vec![ActionDescriptor {
                id: "run-plugin".to_string(),
                label: command.action_label().to_string(),
                kind: ActionKind::PluginCommand,
                capabilities: vec![
                    ActionCapability::PluginExecution,
                    ActionCapability::ScreenCapture,
                    ActionCapability::FilesystemWrite,
                ],
            }],
            source: Some(WINDOWS_CAPTURE_PLUGIN_ID.to_string()),
            icon_hint: None,
        }]
    }
}

pub fn parse_capture_query(
    query: &str,
    settings: &WindowsCaptureSettings,
) -> Option<CaptureCommand> {
    let settings = settings.clone().normalized();
    let normalized = query.trim().to_ascii_lowercase();
    let words: Vec<&str> = normalized.split_whitespace().collect();
    match words.as_slice() {
        ["screenshot"] | ["capture", "screen"] => Some(CaptureCommand::screenshot(
            CaptureTarget::Monitor(MonitorSelection::Primary),
        )),
        ["screenshot", "window"] | ["capture", "window"] => {
            Some(CaptureCommand::screenshot(CaptureTarget::ForegroundWindow))
        }
        ["screenshot", "monitor", index] | ["capture", "monitor", index] => Some(
            CaptureCommand::screenshot(CaptureTarget::Monitor(parse_monitor_index(index)?)),
        ),
        ["record", "screen"] => Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            settings.default_record_seconds,
        )),
        ["record", "screen", seconds] => Some(CaptureCommand::record(
            CaptureTarget::Monitor(MonitorSelection::Primary),
            parse_record_seconds(seconds, &settings)?,
        )),
        ["record", "window"] => Some(CaptureCommand::record(
            CaptureTarget::ForegroundWindow,
            settings.default_record_seconds,
        )),
        ["record", "window", seconds] => Some(CaptureCommand::record(
            CaptureTarget::ForegroundWindow,
            parse_record_seconds(seconds, &settings)?,
        )),
        ["record", "monitor", index] => Some(CaptureCommand::record(
            CaptureTarget::Monitor(parse_monitor_index(index)?),
            settings.default_record_seconds,
        )),
        ["record", "monitor", index, seconds] => Some(CaptureCommand::record(
            CaptureTarget::Monitor(parse_monitor_index(index)?),
            parse_record_seconds(seconds, &settings)?,
        )),
        _ => None,
    }
}

pub fn parse_capture_result_id(
    result_id: &str,
    settings: &WindowsCaptureSettings,
) -> anyhow::Result<CaptureCommand> {
    let settings = settings.clone().normalized();
    let payload = result_id
        .strip_prefix(WINDOWS_CAPTURE_RESULT_PREFIX)
        .ok_or_else(|| anyhow!("malformed Windows Capture result id"))?;
    let parts: Vec<&str> = payload.split(':').collect();
    match parts.as_slice() {
        ["screenshot", "monitor", selector] => Ok(CaptureCommand::screenshot(
            CaptureTarget::Monitor(parse_monitor_selector(selector)?),
        )),
        ["screenshot", "window", "foreground"] => {
            Ok(CaptureCommand::screenshot(CaptureTarget::ForegroundWindow))
        }
        ["record", "monitor", selector, seconds] => Ok(CaptureCommand::record(
            CaptureTarget::Monitor(parse_monitor_selector(selector)?),
            parse_record_seconds_result(seconds, &settings)?,
        )),
        ["record", "window", "foreground", seconds] => Ok(CaptureCommand::record(
            CaptureTarget::ForegroundWindow,
            parse_record_seconds_result(seconds, &settings)?,
        )),
        _ => anyhow::bail!("malformed Windows Capture result id"),
    }
}

pub fn artifact_path_with_timestamp(
    output_directory: &Path,
    mode: CaptureMode,
    unix_millis: u128,
) -> PathBuf {
    let file_name = match mode {
        CaptureMode::Screenshot => format!("winspot-screenshot-{unix_millis}.png"),
        CaptureMode::Record { .. } => format!("winspot-recording-{unix_millis}.mp4"),
    };
    output_directory.join(file_name)
}

pub fn default_output_directory_from(
    executable_directory: &Path,
    portable: bool,
    local_app_data: Option<&str>,
) -> Option<PathBuf> {
    if portable {
        return Some(executable_directory.join("data").join("captures"));
    }

    local_app_data
        .filter(|value| !value.trim().is_empty())
        .map(|value| PathBuf::from(value).join("Winspot").join("captures"))
}

pub fn default_output_directory() -> Option<PathBuf> {
    let executable_directory = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))?;
    let portable = executable_directory.join("Winspot.portable").exists();
    let local_app_data = env::var("LOCALAPPDATA").ok();
    default_output_directory_from(&executable_directory, portable, local_app_data.as_deref())
}

pub fn load_settings_from_path(path: impl AsRef<Path>) -> anyhow::Result<WindowsCaptureSettings> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(WindowsCaptureSettings::default());
    }

    let json = fs::read_to_string(path)
        .with_context(|| format!("read launcher settings {}", path.display()))?;
    let root: LauncherSettingsRoot =
        serde_json::from_str(&json).with_context(|| format!("parse {}", path.display()))?;
    Ok(root.capture.unwrap_or_default().normalized())
}

fn parse_monitor_index(value: &str) -> Option<MonitorSelection> {
    let index = value.parse::<usize>().ok()?;
    (index > 0).then_some(MonitorSelection::Index(index))
}

fn parse_monitor_selector(value: &str) -> anyhow::Result<MonitorSelection> {
    if value == "primary" {
        return Ok(MonitorSelection::Primary);
    }
    let index = value
        .parse::<usize>()
        .map_err(|_| anyhow!("invalid monitor selector"))?;
    if index == 0 {
        anyhow::bail!("invalid monitor selector");
    }
    Ok(MonitorSelection::Index(index))
}

fn parse_record_seconds(value: &str, settings: &WindowsCaptureSettings) -> Option<u64> {
    let seconds_text = value.strip_suffix('s').unwrap_or(value);
    let seconds = seconds_text.parse::<u64>().ok()?;
    Some(settings.clamp_record_seconds(seconds))
}

fn parse_record_seconds_result(
    value: &str,
    settings: &WindowsCaptureSettings,
) -> anyhow::Result<u64> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| anyhow!("invalid recording duration"))?;
    if seconds == 0 || seconds > settings.max_record_seconds.max(1) {
        anyhow::bail!("recording duration is outside configured limits");
    }
    Ok(seconds)
}

fn current_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn default_enabled() -> bool {
    true
}

fn default_record_seconds() -> u64 {
    DEFAULT_RECORD_SECONDS
}

fn default_max_record_seconds() -> u64 {
    DEFAULT_MAX_RECORD_SECONDS
}

fn default_include_cursor() -> bool {
    true
}

fn default_pre_capture_delay_ms() -> u64 {
    DEFAULT_PRE_CAPTURE_DELAY_MS
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LauncherSettingsRoot {
    #[serde(default)]
    capture: Option<WindowsCaptureSettings>,
}

#[cfg(windows)]
fn capture_with_windows_capture(
    command: &CaptureCommand,
    settings: &WindowsCaptureSettings,
    output_path: &Path,
) -> anyhow::Result<()> {
    use std::time::Instant;

    use windows_capture::{
        capture::{Context, GraphicsCaptureApiHandler},
        dxgi_duplication_api::DxgiDuplicationApi,
        encoder::{
            AudioSettingsBuilder, ContainerSettingsBuilder, ImageFormat, VideoEncoder,
            VideoSettingsBuilder,
        },
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        monitor::Monitor,
        settings::{
            ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
            MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
        },
        window::Window,
    };

    type CaptureError = Box<dyn std::error::Error + Send + Sync>;

    fn monitor_from_selection(selection: MonitorSelection) -> anyhow::Result<Monitor> {
        match selection {
            MonitorSelection::Primary => Monitor::primary().map_err(|error| anyhow!("{error}")),
            MonitorSelection::Index(index) => {
                Monitor::from_index(index).map_err(|error| anyhow!("{error}"))
            }
        }
    }

    fn cursor_setting(settings: &WindowsCaptureSettings) -> CursorCaptureSettings {
        if settings.include_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        }
    }

    struct ScreenshotHandler {
        output_path: PathBuf,
    }

    impl GraphicsCaptureApiHandler for ScreenshotHandler {
        type Flags = PathBuf;
        type Error = CaptureError;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self {
                output_path: ctx.flags,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            frame.save_as_image(&self.output_path, ImageFormat::Png)?;
            capture_control.stop();
            Ok(())
        }
    }

    #[derive(Clone)]
    struct RecordingFlags {
        output_path: PathBuf,
        width: u32,
        height: u32,
        seconds: u64,
    }

    struct RecordingHandler {
        encoder: Option<VideoEncoder>,
        start: Instant,
        seconds: u64,
    }

    impl GraphicsCaptureApiHandler for RecordingHandler {
        type Flags = RecordingFlags;
        type Error = CaptureError;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let encoder = VideoEncoder::new(
                VideoSettingsBuilder::new(ctx.flags.width, ctx.flags.height),
                AudioSettingsBuilder::default().disabled(true),
                ContainerSettingsBuilder::default(),
                ctx.flags.output_path.display().to_string(),
            )?;
            Ok(Self {
                encoder: Some(encoder),
                start: Instant::now(),
                seconds: ctx.flags.seconds,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if let Some(encoder) = self.encoder.as_mut() {
                encoder.send_frame(frame)?;
            }
            if self.start.elapsed().as_secs() >= self.seconds {
                if let Some(encoder) = self.encoder.take() {
                    encoder.finish()?;
                }
                capture_control.stop();
            }
            Ok(())
        }
    }

    match (command.mode, command.target) {
        (CaptureMode::Screenshot, CaptureTarget::Monitor(selection)) => {
            let monitor = monitor_from_selection(selection)?;
            let mut duplication =
                DxgiDuplicationApi::new(monitor).map_err(|error| anyhow!("{error}"))?;
            let mut frame = duplication
                .acquire_next_frame(100)
                .map_err(|error| anyhow!("{error}"))?;
            frame
                .save_as_image(output_path, ImageFormat::Png)
                .map_err(|error| anyhow!("{error}"))?;
            Ok(())
        }
        (CaptureMode::Screenshot, CaptureTarget::ForegroundWindow) => {
            let window = Window::foreground().map_err(|error| anyhow!("{error}"))?;
            let settings = Settings::new(
                window,
                cursor_setting(settings),
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Rgba8,
                output_path.to_path_buf(),
            );
            ScreenshotHandler::start(settings).map_err(|error| anyhow!("{error}"))
        }
        (CaptureMode::Record { seconds }, CaptureTarget::Monitor(selection)) => {
            let monitor = monitor_from_selection(selection)?;
            let width = monitor.width().map_err(|error| anyhow!("{error}"))? & !1;
            let height = monitor.height().map_err(|error| anyhow!("{error}"))? & !1;
            let settings = Settings::new(
                monitor,
                cursor_setting(settings),
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Rgba8,
                RecordingFlags {
                    output_path: output_path.to_path_buf(),
                    width,
                    height,
                    seconds,
                },
            );
            RecordingHandler::start(settings).map_err(|error| anyhow!("{error}"))
        }
        (CaptureMode::Record { seconds }, CaptureTarget::ForegroundWindow) => {
            let window = Window::foreground().map_err(|error| anyhow!("{error}"))?;
            let width = window
                .width()
                .map_err(|error| anyhow!("{error}"))?
                .try_into()
                .context("foreground window width is invalid")?;
            let height = window
                .height()
                .map_err(|error| anyhow!("{error}"))?
                .try_into()
                .context("foreground window height is invalid")?;
            let settings = Settings::new(
                window,
                cursor_setting(settings),
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Rgba8,
                RecordingFlags {
                    output_path: output_path.to_path_buf(),
                    width,
                    height,
                    seconds,
                },
            );
            RecordingHandler::start(settings).map_err(|error| anyhow!("{error}"))
        }
    }
}

#[cfg(not(windows))]
fn capture_with_windows_capture(
    _command: &CaptureCommand,
    _settings: &WindowsCaptureSettings,
    _output_path: &Path,
) -> anyhow::Result<()> {
    anyhow::bail!("Windows Capture is only available on Windows")
}
