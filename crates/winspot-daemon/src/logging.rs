//! Process-wide logging setup for the daemon.
//!
//! The daemon historically wrote diagnostics with `eprintln!`, which vanished
//! whenever it ran detached from a console (the normal case once launched by
//! the app). This module wires up `tracing` so the same diagnostics are emitted
//! as structured events to **both** stderr (useful during development) and a
//! daily-rolled file under the Winspot app-data directory, giving us durable
//! logs to triage production issues.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Initializes the global tracing subscriber.
///
/// The verbosity is controlled by the `WINSPOT_LOG` environment variable (using
/// the standard `tracing` filter syntax, e.g. `winspot_daemon=debug,info`) and
/// defaults to `info`. Logs go to stderr and, when an app-data directory can be
/// resolved, to a daily-rolled file in `…\Winspot\logs`.
///
/// Returns a [`WorkerGuard`] when file logging is active; the caller **must**
/// keep it alive for the lifetime of the process, otherwise the background
/// writer is dropped and buffered file logs are lost. Returns `None` when only
/// stderr logging could be set up.
pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("WINSPOT_LOG")
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = fmt::layer().with_ansi(false).with_writer(std::io::stderr);

    match log_dir().filter(|dir| std::fs::create_dir_all(dir).is_ok()) {
        Some(dir) => {
            let file_appender = tracing_appender::rolling::daily(dir, "winspot-daemon.log");
            let (writer, guard) = tracing_appender::non_blocking(file_appender);
            let file_layer = fmt::layer().with_ansi(false).with_writer(writer);
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        None => {
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .init();
            None
        }
    }
}

/// Resolves the directory where daemon log files are written.
///
/// Mirrors the app-data resolution used for the usage log and plugins
/// directory: a portable install (marked by a `Winspot.portable` file next to
/// the executable) keeps logs in `…\data\logs`, otherwise they live under
/// `%LOCALAPPDATA%\Winspot\logs`.
fn log_dir() -> Option<PathBuf> {
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
        && directory.join("Winspot.portable").exists()
    {
        return Some(directory.join("data").join("logs"));
    }

    std::env::var("LOCALAPPDATA")
        .ok()
        .map(|local_app_data| PathBuf::from(local_app_data).join("Winspot").join("logs"))
}
