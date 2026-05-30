use winspot_daemon::server::{PipeConfig, serve_forever};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    serve_forever(PipeConfig::default()).await
}
