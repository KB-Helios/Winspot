use winspot_daemon::server::{PipeConfig, serve_forever};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Keep the file-logging guard alive for the whole process so buffered logs
    // are flushed; dropping it early would silently stop file logging.
    let _log_guard = winspot_daemon::logging::init();
    serve_forever(PipeConfig::default()).await
}
