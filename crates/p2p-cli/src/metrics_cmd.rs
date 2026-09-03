//! metrics 子命令（E8-M2）：relay-only 观测节点，stdout 周期输出稳定 key=value。
//!
//! 装配与 bootstrap 相同的 relay 服务（rendezvous 不启用），指标行带 relay_ 前缀
//! 逐行打印可 grep；--duration 0 常驻直到 ctrl-c，>0 到点打末次快照自动退出。

use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{parse_socket_addr, MetricsArgs};
use crate::metrics_log::render_relay_metrics;
use crate::relay_serve::{load_identity, spawn_relay};

/// 常驻运行：启动后先打一次快照，再按周期打印；duration>0 到点自动退出。
pub async fn run(args: MetricsArgs) -> Result<(), String> {
    let quic_addr = parse_socket_addr(&args.listen_quic)?;
    let tcp_addr = parse_socket_addr(&args.listen_tcp)?;
    let keypair =
        load_identity(PathBuf::from(&args.data)).map_err(|e| format!("读取身份失败: {e}"))?;
    let peer_id = keypair.peer_id();
    let relay = spawn_relay(keypair, quic_addr, tcp_addr)
        .await
        .map_err(|e| format!("relay 监听/装配失败: {e}"))?;

    println!("peer_id={peer_id}");
    println!(
        "relay (QUIC {} / TCP {})",
        quic_addr.port(),
        tcp_addr.port()
    );
    print!("{}", render_relay_metrics(&relay.metrics()));

    let mut ticker = tokio::time::interval(Duration::from_secs(args.interval.max(1)));
    ticker.tick().await; // 首个 tick 立即返回，跳过避免与启动快照双打印
    let end = end_at(args.duration);
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                tracing::info!("ctrl-c received, shutting down");
                return Ok(());
            }
            _ = ticker.tick() => print!("{}", render_relay_metrics(&relay.metrics())),
            _ = sleep_until(end) => {
                print!("{}", render_relay_metrics(&relay.metrics()));
                return Ok(());
            }
        }
    }
}

/// duration 0 = 常驻（截止分支永不触发）；>0 = 从 now 起的截止时刻。
fn end_at(duration_secs: u64) -> Option<tokio::time::Instant> {
    if duration_secs == 0 {
        None
    } else {
        Some(tokio::time::Instant::now() + Duration::from_secs(duration_secs))
    }
}

/// Option<Instant> 的 None 分支用永不完成的 future 兜底，统一一个 select 臂。
async fn sleep_until(end: Option<tokio::time::Instant>) {
    match end {
        Some(t) => tokio::time::sleep_until(t).await,
        None => std::future::pending().await,
    }
}
