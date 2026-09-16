//! service 命令域（gui-contract §20.4-5 CLI 对等面）：list/enable/disable。
//! 与 GUI services_list/services_set_enabled 同源同语义：同一
//! `<data-dir>/services.json`（与 authz 数据根同根）、同一闭集报错、同一双读。
//! 闭集与存储真值源在 p2p-service，本层只做参数映射、双读回落取值与渲染。

mod list;
mod set;

use std::path::Path;

use clap::Subcommand;

use p2p_service::{ServiceId, ServiceRegistry, ServiceStoreError};

use crate::error::{CliError, CliResult};
use crate::types::GuiConfig;

#[derive(Subcommand)]
pub enum ServiceCommand {
    /// 列出服务闭集 10 项（型别/默认/生效值/来源）
    List(list::ListArgs),
    /// 启用服务（upsert 落盘 services.json，下次节点启动生效）
    Enable(set::SetArgs),
    /// 停用服务（upsert 落盘 services.json，下次节点启动生效）
    Disable(set::SetArgs),
}

pub async fn run(command: ServiceCommand) -> CliResult<()> {
    match command {
        ServiceCommand::List(args) => list::run(args).await,
        ServiceCommand::Enable(args) => set::run(args, true).await,
        ServiceCommand::Disable(args) => set::run(args, false).await,
    }
}

/// 存储错误 → CLI 运行错误（p2p-service 文案已可读中文，原样上浮不静默）。
pub(crate) fn store_err(e: ServiceStoreError) -> CliError {
    CliError::Runtime(e.to_string())
}

/// 读 services.json 注册表：缺失 = 空表首用态；损坏/版本不符/越闭集显式报错。
pub(crate) fn load(data_dir: &str) -> CliResult<ServiceRegistry> {
    p2p_service::load_registry(Path::new(data_dir)).map_err(store_err)
}

/// 闭集精确匹配解析：表外 id → 可读 Err 附闭集清单（§20.4-1 禁静默忽略）。
pub(crate) fn parse_closed(service_id: &str) -> CliResult<ServiceId> {
    ServiceId::parse(service_id).ok_or_else(|| {
        CliError::Runtime(format!(
            "服务 id 不在闭集登记表: {service_id}（可用: {}）",
            closed_set_list()
        ))
    })
}

/// 闭集 id 清单（报错附可用项，§20.1 顺序）。
pub(crate) fn closed_set_list() -> String {
    ServiceId::registry()
        .iter()
        .map(|id| id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 生效值（service-registry-design §2 双读规则）：收编型 = 条目优先 →
/// gui-config legacy 字段 → 默认；其余型 = 条目优先 → 默认（推导委托
/// p2p-service::effective_enabled，legacy 取值用 CLI GuiConfig 契约镜像）。
pub(crate) fn effective(id: ServiceId, registry: &ServiceRegistry, cfg: &GuiConfig) -> bool {
    let legacy = match id {
        ServiceId::DISCOVERY_MDNS => Some(cfg.enable_mdns),
        ServiceId::NET_LAN_ONLY => Some(cfg.lan_only),
        _ => None,
    };
    p2p_service::effective_enabled(id, registry, legacy)
}

/// 来源标注（list 渲染列）：条目 / 双读回落 / 默认。
pub(crate) fn source_of(id: ServiceId, registry: &ServiceRegistry) -> &'static str {
    if registry.file_value(id).is_some() {
        "条目"
    } else if matches!(id, ServiceId::DISCOVERY_MDNS | ServiceId::NET_LAN_ONLY) {
        "双读回落"
    } else {
        "默认"
    }
}

/// Unix 秒（updated_at 注入；时钟异常显式报错，禁静默吞）。
pub(crate) fn now_unix() -> CliResult<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| CliError::Runtime(format!("系统时钟异常，无法生成 updated_at: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_service::ServiceRegistry;

    #[test]
    fn closed_set_error_lists_all_registry_ids() {
        let err = parse_closed("owner.superuser").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("owner.superuser"), "实际: {msg}");
        assert!(msg.contains("serve.llm_share"), "实际: {msg}");
        assert!(msg.contains("net.lan_only"), "实际: {msg}");
        assert_eq!(closed_set_list().split(", ").count(), 11);
    }

    #[test]
    fn effective_adopted_falls_back_to_legacy_then_entry_wins() {
        let id = ServiceId::DISCOVERY_MDNS;
        let empty = ServiceRegistry::default();
        let mut cfg = GuiConfig::default();
        assert!(
            effective(id, &empty, &cfg),
            "无条目回落 enableMdns 默认 true"
        );
        cfg.enable_mdns = false;
        assert!(!effective(id, &empty, &cfg), "无条目回落 legacy 字段");
        let mut with_entry = ServiceRegistry::default();
        with_entry.set_enabled(id, true, 1);
        assert!(effective(id, &with_entry, &cfg), "条目权威压过 legacy");
    }

    #[test]
    fn effective_non_adopted_ignores_legacy_config() {
        let id = ServiceId::SERVE_TUNNEL;
        let registry = ServiceRegistry::default();
        let cfg = GuiConfig::default();
        assert!(!effective(id, &registry, &cfg), "布尔型默认 off");
    }

    #[test]
    fn source_of_marks_entry_fallback_and_default() {
        let id = ServiceId::NET_LAN_ONLY;
        let empty = ServiceRegistry::default();
        assert_eq!(source_of(id, &empty), "双读回落");
        assert_eq!(source_of(ServiceId::SERVE_ACP, &empty), "默认");
        let mut set = ServiceRegistry::default();
        set.set_enabled(ServiceId::SERVE_ACP, false, 1);
        assert_eq!(source_of(ServiceId::SERVE_ACP, &set), "条目");
    }
}
