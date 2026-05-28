use winspot_daemon::server::{serve_forever, PipeConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    serve_forever(PipeConfig::default()).await
}
