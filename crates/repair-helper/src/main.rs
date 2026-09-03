use repair_helper::Host;
use tokio::io::BufReader;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let report = p2p_log::init(p2p_log::LogConfig::default());
    if let Some(fallback) = report.fallback {
        tracing::warn!(%fallback, "repair-helper logging fallback");
    }
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    Host::empty()
        .serve(
            BufReader::new(tokio::io::stdin()),
            tokio::io::stdout(),
            shutdown_rx,
        )
        .await
}
