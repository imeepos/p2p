use repair_enforce::{scope::Scope, whitelist::ShellWhitelist};
use repair_helper::{audit::AuditSink, enforce::Enforcement, jail::PathJail, tools, Host};
use std::path::PathBuf;
use tokio::io::BufReader;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let report = p2p_log::init(p2p_log::LogConfig::default());
    if let Some(fallback) = report.fallback {
        tracing::warn!(%fallback, "repair-helper logging fallback");
    }
    let jail = build_jail()?;
    let registry = tools::read_only_registry(jail);
    // P0b 缺省 diag 只读 scope；T26 换 ticket 来源注入 scope。
    let enforcement = Enforcement::new(Scope::Diag, ShellWhitelist::empty());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    Host::guarded(registry, enforcement, AuditSink::default())
        .serve(
            BufReader::new(tokio::io::stdin()),
            tokio::io::stdout(),
            shutdown_rx,
        )
        .await
}

/// 授权根：REPAIR_ROOTS（: 分隔）显式配置，缺省临时演示根。
/// 失败的根配置必须显式报错退出，禁止静默降级。
fn build_jail() -> std::io::Result<PathJail> {
    let jail = match std::env::var("REPAIR_ROOTS") {
        Ok(raw) if !raw.trim().is_empty() => {
            let roots = raw.split(':').map(PathBuf::from).collect();
            PathJail::from_roots(roots)
        }
        _ => PathJail::demo(),
    };
    jail.map_err(|e| {
        tracing::error!(%e, "repair-helper jail init failed");
        std::io::Error::other(e)
    })
}
