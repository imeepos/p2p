//! 监督句柄与启动入口（gui-contract.md §15）：产线经 tauri async runtime 启动，
//! 测试经 launch 自选 runtime；句柄供 RunEvent::Exit 幂等收尾。

use std::future::Future;
use std::sync::{Arc, Weak};

use tokio::sync::watch;

use super::spec::{Limits, SpawnSpec};
use super::status::AcpConsoleStatus;
use super::supervisor::{run, Supervisor};

/// 监督句柄：RunEvent::Exit 收尾（幂等）；监督任务结束后退化为 no-op。
#[derive(Clone)]
pub(crate) struct SupervisorHandle {
    sup: Weak<Supervisor>,
}

impl SupervisorHandle {
    /// 置停机位 + 终止在跑子进程 + 唤醒退避等待；stopped 迁移由监督循环完成。
    pub(crate) fn shutdown(&self) {
        if let Some(sup) = self.sup.upgrade() {
            sup.shutdown_now();
        }
    }

    /// 当前子进程 pid（测试观测 Exit 收尾用；退避窗口内无子进程为 None）。
    #[cfg(test)]
    pub(crate) fn current_pid(&self) -> Option<u32> {
        self.sup.upgrade().and_then(|sup| sup.child_pid())
    }
}

/// 产线入口：tauri async runtime 上启动监督任务。
pub(crate) fn start(
    spec: SpawnSpec,
    limits: Limits,
    status_tx: watch::Sender<AcpConsoleStatus>,
) -> SupervisorHandle {
    let (handle, fut) = launch(spec, limits, status_tx);
    tauri::async_runtime::spawn(fut);
    handle
}

/// 测试入口：返回句柄与未启动的监督 future，调用方自选 runtime spawn。
#[doc(hidden)]
pub(crate) fn launch(
    spec: SpawnSpec,
    limits: Limits,
    status_tx: watch::Sender<AcpConsoleStatus>,
) -> (SupervisorHandle, impl Future<Output = ()> + Send) {
    let sup = Arc::new(Supervisor::new(spec, limits, status_tx));
    let handle = SupervisorHandle {
        sup: Arc::downgrade(&sup),
    };
    (handle, run(sup))
}
