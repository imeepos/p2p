//! serve.tunnel 持久化闸（服务注册表 §2 布尔型；gui-contract §20.1 行为变化点：
//! 重启不再回落关闭）。启动恢复读 services.json；start/stop 翻转后 upsert 落盘
//! （§4.5 先重读合并再原子写）。白名单仍为内存会话态（inventory 问题 11 双存储
//! 不在本卡范围）。

use std::path::Path;

use p2p_service::{effective_enabled, load_registry, save_registry, ServiceId};

/// 当前 Unix 秒（写路径 updated_at 注入点；时钟不进存储层）。
pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 启动恢复：services.json 条目（或缺省默认）决定 enabled。读失败 = 显式错误
/// 上浮（§4.2 禁止静默回退空表），调用方按关闭 fail-safe 并留告警日志。
pub(crate) fn restore_enabled(data_dir: &Path) -> Result<bool, String> {
    let registry = load_registry(data_dir)
        .map_err(|e| format!("services.json 不可读，serve.tunnel 按关闭 fail-safe: {e}"))?;
    Ok(effective_enabled(ServiceId::SERVE_TUNNEL, &registry, None))
}

/// 翻转落盘：先重读磁盘合并（缩小双进程写窗口）再 upsert + 原子写。失败显式
/// 上浮，调用方决定开闸方向（开启=拒绝生效，关闭=照关并告警）。
pub(crate) fn persist_enabled(data_dir: &Path, enabled: bool, now_unix: u64) -> Result<(), String> {
    let mut registry = load_registry(data_dir)
        .map_err(|e| format!("services.json 不可读，serve.tunnel 开关未落盘: {e}"))?;
    registry.set_enabled(ServiceId::SERVE_TUNNEL, enabled, now_unix);
    save_registry(data_dir, &registry)
        .map_err(|e| format!("services.json 写入失败，serve.tunnel 开关未落盘: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_service::ServiceRegistry;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gui-tunnel-gate-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn corrupt(dir: &Path) {
        std::fs::write(dir.join("services.json"), "{ not json").unwrap();
    }

    #[test]
    fn missing_file_restores_off() {
        assert_eq!(restore_enabled(&scratch("missing")).unwrap(), false);
    }

    #[test]
    fn persist_restore_roundtrip_both_states() {
        let dir = scratch("roundtrip");
        persist_enabled(&dir, true, 100).unwrap();
        assert_eq!(restore_enabled(&dir).unwrap(), true);
        persist_enabled(&dir, false, 200).unwrap();
        assert_eq!(restore_enabled(&dir).unwrap(), false);
    }

    #[test]
    fn persist_merges_without_dropping_other_services() {
        let dir = scratch("merge");
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::SERVE_LLM_SHARE, true, 1);
        save_registry(&dir, &reg).unwrap();
        persist_enabled(&dir, true, 2).unwrap();
        let loaded = load_registry(&dir).unwrap();
        assert_eq!(loaded.file_value(ServiceId::SERVE_LLM_SHARE), Some(true));
        assert_eq!(loaded.file_value(ServiceId::SERVE_TUNNEL), Some(true));
    }

    #[test]
    fn corrupt_file_is_explicit_error_on_both_paths() {
        let dir = scratch("corrupt");
        corrupt(&dir);
        assert!(restore_enabled(&dir).unwrap_err().contains("fail-safe"));
        assert!(
            persist_enabled(&dir, true, 1)
                .unwrap_err()
                .contains("未落盘"),
            "损坏文件禁止静默覆写（§4.2）"
        );
    }
}
