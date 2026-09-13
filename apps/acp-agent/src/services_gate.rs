//! serve.a2a 服务注册表闸（service-registry-design §2 显式化型，默认 on）。
//! §4.4 同源同根：services.json 与本进程 authz 表同根（= config.data_dir）。
//! 条目 off → 等价 `--a2a-disabled`；与显式 flag 取「或」（保守方向）。
//! 读失败按 §4.2 显式化型 fail-safe=保持既有配置语义，错误显式上浮不静默。

use std::path::Path;

use p2p_service::{effective_enabled, load_registry, ServiceId};

use crate::config::AgentConfig;

/// 消费 services.json 的 serve.a2a 条目并合并进配置：enabled=false 时置位
/// `config.a2a_disabled`（与显式 flag 取「或」）。Ok=条目（或缺省默认）已消费；
/// Err=注册表不可读（fail-safe 保持既有配置语义，错误串供调用方留观测日志）。
pub fn apply_a2a_gate(config: &mut AgentConfig, data_dir: &Path) -> Result<(), String> {
    let registry = load_registry(data_dir)
        .map_err(|e| format!("services.json 不可读，serve.a2a 闸按既有配置处理: {e}"))?;
    let enabled = effective_enabled(ServiceId::SERVE_A2A, &registry, None);
    if !enabled {
        config.a2a_disabled = true;
    }
    tracing::info!(
        enabled,
        "serve.a2a 注册表闸已消费（services.json 条目或缺省默认）"
    );
    Ok(())
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
            "acp-services-gate-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn registry_with(enabled: bool) -> ServiceRegistry {
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::SERVE_A2A, enabled, 1);
        reg
    }

    #[test]
    fn missing_file_keeps_default_on() {
        let mut cfg = AgentConfig::default();
        apply_a2a_gate(&mut cfg, &scratch("missing")).unwrap();
        assert!(!cfg.a2a_disabled);
    }

    #[test]
    fn registry_off_disables_a2a() {
        let dir = scratch("off");
        save_registry(&dir, &registry_with(false)).unwrap();
        let mut cfg = AgentConfig::default();
        apply_a2a_gate(&mut cfg, &dir).unwrap();
        assert!(cfg.a2a_disabled);
    }

    #[test]
    fn registry_on_keeps_flag_semantics() {
        let dir = scratch("on");
        save_registry(&dir, &registry_with(true)).unwrap();
        let mut cfg = AgentConfig::default();
        apply_a2a_gate(&mut cfg, &dir).unwrap();
        assert!(!cfg.a2a_disabled);
    }

    #[test]
    fn flag_and_registry_take_or() {
        let dir = scratch("or");
        save_registry(&dir, &registry_with(true)).unwrap();
        let mut flagged = AgentConfig {
            a2a_disabled: true,
            ..Default::default()
        };
        apply_a2a_gate(&mut flagged, &dir).unwrap();
        assert!(
            flagged.a2a_disabled,
            "显式 flag 不被文件 on 覆盖（保守方向）"
        );
    }

    #[test]
    fn corrupt_registry_failsafe_keeps_config_with_error() {
        let dir = scratch("corrupt");
        std::fs::write(dir.join("services.json"), "{ not json").unwrap();
        let mut cfg = AgentConfig::default();
        let err = apply_a2a_gate(&mut cfg, &dir).unwrap_err();
        assert!(
            err.contains("fail-safe") || err.contains("不可读"),
            "实际: {err}"
        );
        assert!(!cfg.a2a_disabled, "读失败保持既有配置语义（§4.2 显式化型）");
    }
}
