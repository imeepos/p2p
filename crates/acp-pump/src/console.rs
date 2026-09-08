//! CLI 装配面：ConsoleConfig → 前台泵。装配实现在 runtime（Pump::start），
//! 这里只补 CLI 前台语义：stdout 就绪行、ctrl_c 收尾、泵自退转显式失败。
//! 二进制（p2pctl acp console 等）只做入参翻译，不复制装配实现。

use serde::Serialize;

use crate::config::ConsoleConfig;
use crate::out;
use crate::runtime::Pump;

/// 就绪行载荷（stdout JSON 行契约：{"kind":"ready","ws":...,"status":...,"token":...,"peer":...}）。
#[derive(Serialize)]
struct ReadyLine {
    ws: String,
    status: String,
    token: String,
    peer: String,
}

/// 起一套前台泵并阻塞到 ctrl_c。装配失败与泵异常退出均转 Err（留因不静默）。
pub async fn run_console(cfg: ConsoleConfig) -> Result<(), String> {
    let mut handle = Pump::start(cfg).await?;
    out::event(
        "ready",
        &ReadyLine {
            ws: handle.ws_addr.to_string(),
            status: handle.status_addr.to_string(),
            token: handle.token.clone(),
            peer: handle.peer.clone(),
        },
    );
    tracing::info!(ws = %handle.ws_addr, status = %handle.status_addr, "acp-pump ready");

    let exit = tokio::select! {
        r = tokio::signal::ctrl_c() => {
            r.map_err(|e| format!("signal: {e}"))?;
            tracing::info!("shutdown by signal");
            handle.stop().await;
            return Ok(());
        }
        exit = handle.wait_exit() => exit,
    };
    Err(exit
        .error
        .unwrap_or_else(|| "pump exited without reason".to_string()))
}
