//! gui-contract.md §20 服务总控命令面：services_list / services_set_enabled。
//!
//! 语义真值源 = crates/p2p-service（闭集常量表 + services.json 存储）；
//! 数据根 = app 数据目录（§18 authz 同口径，CLI --data-dir 等价物）；
//! requiresRestart = 节点当前运行中（开关一律下次启动生效，不改运行中节点）；
//! 损坏/版本不符/越闭集显式 Err 不静默（§20.4-2 存储纪律）。
//!
//! 收编型双读回落（discovery.mdns←enableMdns、net.lan_only←lanOnly）取自
//! GuiConfig 契约镜像字段（B1 已补 lan_only 并消费，GUI/CLI 同语义）；
//! p2p-service::switches 是装配层消费面（B1），显式报错面归本命令模块。

use std::path::Path;

use p2p_service::{
    effective_enabled, load_registry, save_registry, ServiceId, ServiceRegistry, ServiceStoreError,
};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::types::GuiConfig;

/// §20.3 ServiceView（camelCase 逐字）：kind = boolean|explicit|adopted。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceViewJson {
    pub service_id: String,
    pub kind: &'static str,
    pub enabled: bool,
    pub requires_restart: bool,
}

/// services_list 返回（§20.2）：闭集 10 项全量。
#[derive(Debug, Serialize)]
pub struct ServicesListReport {
    pub services: Vec<ServiceViewJson>,
}

/// §20.3 ServiceMutationReport（camelCase 逐字）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMutationReportJson {
    pub service_id: String,
    pub enabled: bool,
    pub requires_restart: bool,
}

/// services_list（§20.2）：读失败（损坏/版本不符/越闭集）显式 Err 不静默。
#[tauri::command]
pub async fn services_list(state: State<'_, AppState>) -> Result<ServicesListReport, String> {
    let node_running = state.status().await.running;
    let registry = load_registry(data_dir(&state)).map_err(store_err)?;
    let cfg = state.config_get();
    Ok(ServicesListReport {
        services: build_views(&registry, &cfg, node_running),
    })
}

/// services_set_enabled（§20.2）：upsert 条目并原子写盘（先重读磁盘合并）；
/// 表外 serviceId → Err 可读中文（附闭集清单）；写失败 → Err；不改运行中节点，
/// requiresRestart 供面板提示「重启节点后生效」（§20.4-4）。
#[tauri::command]
pub async fn services_set_enabled(
    state: State<'_, AppState>,
    service_id: String,
    enabled: bool,
) -> Result<ServiceMutationReportJson, String> {
    let node_running = state.status().await.running;
    let id = parse_closed(&service_id)?;
    let now = now_unix()?;
    set_enabled(data_dir(&state), id, enabled, now)?;
    Ok(ServiceMutationReportJson {
        service_id: id.as_str().to_owned(),
        enabled,
        requires_restart: node_running,
    })
}

fn data_dir<'a>(state: &'a State<'a, AppState>) -> &'a Path {
    state.data_dir()
}

/// 存储错误上浮（p2p-service 文案已可读中文，原样透出）。
fn store_err(e: ServiceStoreError) -> String {
    e.to_string()
}

/// 闭集精确匹配：表外 id → Err 可读中文附闭集清单（§20.4-1 禁静默忽略）。
fn parse_closed(service_id: &str) -> Result<ServiceId, String> {
    ServiceId::parse(service_id).ok_or_else(|| {
        format!(
            "服务 id 不在闭集登记表: {service_id}（可用: {}）",
            ServiceId::registry()
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// 生效值（service-registry-design §2 双读规则），推导委托 p2p-service；
/// legacy 取值用 GuiConfig 契约镜像（与 apps/cli service 域同形状）。
fn effective(id: ServiceId, registry: &ServiceRegistry, cfg: &GuiConfig) -> bool {
    let legacy = match id {
        ServiceId::DISCOVERY_MDNS => Some(cfg.enable_mdns),
        ServiceId::NET_LAN_ONLY => Some(cfg.lan_only),
        _ => None,
    };
    effective_enabled(id, registry, legacy)
}

/// 闭集全量视图（registry 顺序 = §20.1 表顺序）。
fn build_views(
    registry: &ServiceRegistry,
    cfg: &GuiConfig,
    node_running: bool,
) -> Vec<ServiceViewJson> {
    ServiceId::registry()
        .iter()
        .map(|id| ServiceViewJson {
            service_id: id.as_str().to_owned(),
            kind: id.kind().as_str(),
            enabled: effective(*id, registry, cfg),
            requires_restart: node_running,
        })
        .collect()
}

/// upsert 内核（可测）：先重读磁盘合并再落盘（§4.5 双进程写窗口收敛）。
fn set_enabled(dir: &Path, id: ServiceId, enabled: bool, now_unix: u64) -> Result<(), String> {
    let mut registry = load_registry(dir).map_err(store_err)?;
    registry.set_enabled(id, enabled, now_unix);
    save_registry(dir, &registry).map_err(store_err)
}

/// Unix 秒（updated_at 注入；时钟异常显式报错，禁静默吞）。
fn now_unix() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| format!("系统时钟异常，无法生成 updated_at: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_service::store::services_path;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("p2p-console-svc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 出厂默认配置（serde default 补齐：enableMdns=true、lanOnly=false）。
    fn default_cfg() -> GuiConfig {
        serde_json::from_str("{}").unwrap()
    }

    #[test]
    fn build_views_covers_full_closed_set_contract_shape() {
        let views = build_views(&ServiceRegistry::default(), &default_cfg(), true);
        assert_eq!(views.len(), 10);
        let first = &views[0];
        assert_eq!(first.service_id, "serve.llm_share");
        assert_eq!(first.kind, "boolean");
        assert!(!first.enabled, "布尔型默认 off");
        assert!(first.requires_restart);
        let raw = serde_json::to_value(first).unwrap();
        assert_eq!(raw["serviceId"], "serve.llm_share");
        assert_eq!(raw["requiresRestart"], true);
        let mdns = views
            .iter()
            .find(|v| v.service_id == "discovery.mdns")
            .unwrap();
        assert_eq!(mdns.kind, "adopted");
        assert!(mdns.enabled, "无条目回落 enableMdns 出厂默认 true");
    }

    #[test]
    fn adopted_legacy_fallback_reads_typed_config_fields() {
        let cfg: GuiConfig =
            serde_json::from_str(r#"{"enableMdns":false,"lanOnly":true}"#).unwrap();
        let registry = ServiceRegistry::default();
        assert!(!effective(ServiceId::DISCOVERY_MDNS, &registry, &cfg));
        assert!(effective(ServiceId::NET_LAN_ONLY, &registry, &cfg));
    }

    #[test]
    fn entry_overrides_legacy_fallback() {
        let mut registry = ServiceRegistry::default();
        registry.set_enabled(ServiceId::NET_LAN_ONLY, false, 1);
        let cfg: GuiConfig = serde_json::from_str(r#"{"lanOnly":true}"#).unwrap();
        assert!(
            !effective(ServiceId::NET_LAN_ONLY, &registry, &cfg),
            "条目权威"
        );
    }

    #[test]
    fn parse_closed_error_lists_all_ids() {
        let err = parse_closed("owner.superuser").unwrap_err();
        assert!(err.contains("owner.superuser") && err.contains("serve.llm_share"));
        assert!(err.contains("net.lan_only"), "附闭集清单: {err}");
    }

    #[test]
    fn set_enabled_upserts_atomically_and_survives_reload() {
        let dir = scratch("upsert");
        set_enabled(&dir, ServiceId::SERVE_TUNNEL, true, 100).unwrap();
        set_enabled(&dir, ServiceId::SERVE_TUNNEL, false, 200).unwrap();
        let registry = load_registry(&dir).unwrap();
        let entries = registry.entries();
        assert_eq!(entries.len(), 1, "同 id upsert 不追加条目");
        assert!(!entries[0].enabled);
        assert_eq!(entries[0].updated_at, 200);
        assert!(!dir.join("services.json.tmp").exists(), "成功写无 tmp 残留");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_explicit_error_not_silent_fallback() {
        let dir = scratch("corrupt");
        std::fs::write(services_path(&dir), "{ not json").unwrap();
        assert!(set_enabled(&dir, ServiceId::SERVE_TUNNEL, true, 1).is_err());
        assert!(load_registry(&dir).is_err(), "损坏显式拒读（§20.4-2）");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
