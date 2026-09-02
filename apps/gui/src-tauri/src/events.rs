//! 事件转发（gui-contract.md §2）：NodeEvent → NodeEventJson，单通道 node-event。
//!
//! 订阅在 state.start 内建立，本任务由命令层在启动后接管；lagging 时发 node_error 说明。

use p2p::NodeEvent;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::types::NodeEventJson;

/// 前端唯一事件通道名。
pub const NODE_EVENT: &str = "node-event";

/// 启动转发任务；通道关闭（节点停机且发送端清空）后自然退出。
pub fn spawn<R: Runtime>(app: AppHandle<R>, mut rx: broadcast::Receiver<NodeEvent>) {
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => emit(&app, NodeEventJson::from(event)),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(dropped = n, "节点事件通道积压，已丢弃部分事件");
                    emit(&app, lag_event(n));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("节点事件通道关闭，转发任务退出");
                    break;
                }
            }
        }
    });
}

/// 统一出口：盖发射时刻毫秒戳（契约 §2 可选 tsMs）后单通道推送；失败留告警，不中断后续事件。
pub fn emit<R: Runtime>(app: &AppHandle<R>, payload: NodeEventJson) {
    let stamped = payload.stamped(crate::util::now_ms());
    if let Err(e) = app.emit(NODE_EVENT, &stamped) {
        warn!(error = %e, "推送 node-event 失败");
    }
}

/// 桥接层自产的 node_error：事件积压说明（契约 §2 应用级事件）。
fn lag_event(dropped: u64) -> NodeEventJson {
    NodeEventJson::NodeError {
        reason: format!("事件通道积压，已丢弃 {dropped} 条事件"),
        ts_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lag_event_carries_drop_count() {
        let event = lag_event(7);
        match event {
            NodeEventJson::NodeError { reason, .. } => {
                assert!(reason.contains('7'), "原因需带丢弃条数: {reason}");
            }
            other => panic!("期望 node_error，实得 {other:?}"),
        }
    }
}
