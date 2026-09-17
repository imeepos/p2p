//! serve.llm_share 服务注册表总闸（service-registry-design §2 布尔型，安全默认
//! 关；§4 读写纪律）。借出 serve 装配前判定：off（含无条目缺省）→ 不注册双
//! handler，留 info 级可观测日志（拒绝路径不静默）；services.json 读失败按
//! §4.2 fail-safe=关，错误显式上告警日志。on → 既有装配流程逐字不变。

use std::path::Path;

use p2p_service::{effective_enabled, load_registry, ServiceId};

/// 装配许可判定：Ok(true)=on 照常装配；Ok(false)=总闸关，不装配；
/// Err=注册表不可读（fail-safe 仍按关处理，调用方必须留告警日志）。
pub(crate) fn assembly_allowed(data_dir: &Path) -> Result<bool, String> {
    let registry = load_registry(data_dir)
        .map_err(|e| format!("services.json 不可读，serve.llm_share 总闸按关闭 fail-safe: {e}"))?;
    Ok(effective_enabled(
        ServiceId::SERVE_LLM_SHARE,
        &registry,
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_service::{save_registry, ServiceRegistry};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gui-llm-gate-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn registry_with(id: ServiceId, enabled: bool) -> ServiceRegistry {
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(id, enabled, 1);
        reg
    }

    #[test]
    fn missing_file_defaults_off() {
        let dir = scratch("missing");
        assert!(!assembly_allowed(&dir).unwrap());
    }

    #[test]
    fn explicit_on_allows_assembly() {
        let dir = scratch("on");
        save_registry(&dir, &registry_with(ServiceId::SERVE_LLM_SHARE, true)).unwrap();
        assert!(assembly_allowed(&dir).unwrap());
    }

    #[test]
    fn explicit_off_blocks_assembly() {
        let dir = scratch("off");
        save_registry(&dir, &registry_with(ServiceId::SERVE_LLM_SHARE, false)).unwrap();
        assert!(!assembly_allowed(&dir).unwrap());
    }

    #[test]
    fn corrupt_registry_is_failsafe_off_with_explicit_error() {
        let dir = scratch("corrupt");
        std::fs::write(dir.join("services.json"), "{ not json").unwrap();
        let err = assembly_allowed(&dir).unwrap_err();
        assert!(err.contains("fail-safe"), "实际: {err}");
    }
}
