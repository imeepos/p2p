//! 身份进程互斥（D6 裁决落地）：一个 data-dir 身份同一时刻只允许一个进程持有 swarm。
//! 多进程同身份会在连接收敛（swarm/dial.rs 恒留小 id 端 Outbound）下产生配对级投递失败，
//! 产品裁决为「不支持一个身份多个程序」——被占即快速失败，可读报错。
//! 跨进程互斥靠 flock（进程死亡内核自动释放）；macOS BSD flock 同进程不互斥，
//! 另以锁文件内 PID 归主比对兜底同进程重复持有。非 unix 平台崩溃残留陈锁时
//! 错误信息含锁路径，删除后可恢复。

use std::path::Path;
use std::time::Duration;

use crate::model::ChatError;
use crate::store_lock::FileLock;

/// 身份锁获取超时：0 = 只试一次，被占立即失败（快速失败语义，不等待）。
const TRY_ONCE: Duration = Duration::ZERO;

/// 尝试获取身份进程锁；被占即返回 IdentityBusy（调用方原样上抛，退出码 1）。
/// 守卫 Drop 自动释放，进程崩溃由内核兜底。
pub fn try_lock_identity(data_dir: &Path) -> Result<IdentityGuard, ChatError> {
    let path = data_dir.join("identity.lock");
    let path_display = path.display().to_string();
    let lock = FileLock::acquire(path.clone(), TRY_ONCE).map_err(|e| {
        ChatError::IdentityBusy(format!(
            "该身份已有进程在运行（同数据目录不支持多程序并行），如需切换请先停止另一进程；锁={path_display}，{e}"
        ))
    })?;
    // BSD flock 同进程不互斥：以锁文件内 PID 归主比对兜底同进程重复持有。
    let pid = std::process::id().to_string();
    let owner = std::fs::read_to_string(&path).unwrap_or_default();
    if owner.trim() == pid {
        return Err(ChatError::IdentityBusy(format!(
            "该身份已被当前进程持有（同数据目录不支持多程序并行）；锁={path_display}"
        )));
    }
    std::fs::write(&path, &pid)
        .map_err(|e| ChatError::IdentityBusy(format!("身份锁写入失败：锁={path_display}，{e}")))?;
    Ok(IdentityGuard { _lock: lock })
}

/// 身份锁守卫：持有 FileLock 期间即持锁，Drop 释放。
#[derive(Debug)]
pub struct IdentityGuard {
    _lock: FileLock,
}
