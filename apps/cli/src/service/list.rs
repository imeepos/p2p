//! p2pctl service list：闭集 10 项只读清单（id/型别/默认/生效值/来源），
//! 离线可跑；requiresRestart = 节点运行中（守护进程在线探测，与 GUI 同语义）。

use clap::Args;
use serde::Serialize;

use p2p_service::{ServiceId, ServiceKind, ServiceRegistry};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store;
use crate::types::GuiConfig;

use super::{effective, load, source_of};

#[derive(Args)]
pub struct ListArgs {
    /// 输出结构化 JSON（services: ServiceView[]，gui-contract §20.3 同形）
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（services.json 与 authz 数据根同根）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

/// §20.3 ServiceView（camelCase 逐字）：requiresRestart = 节点运行中。
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceViewJson {
    pub service_id: String,
    pub kind: &'static str,
    pub enabled: bool,
    pub requires_restart: bool,
}

/// services_list 报告（与 GUI services_list 返回逐字同形，B4 面板对端）。
#[derive(Debug, Serialize)]
pub struct ServiceListReport {
    pub services: Vec<ServiceViewJson>,
}

pub async fn run(args: ListArgs) -> CliResult<()> {
    let registry = load(&args.data_dir)?;
    let cfg = store::load_config(&Paths::new(&args.data_dir));
    let node_running = crate::lifecycle::probe_online(&args.data_dir)
        .await
        .is_some();
    let report = ServiceListReport {
        services: build_views(&registry, &cfg, node_running),
    };
    output::emit(args.json, &report, &render(&registry, &cfg, node_running))
}

/// 闭集全量视图（registry 顺序 = §20.1 表顺序）。
pub(super) fn build_views(
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

fn kind_cn(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::Boolean => "布尔型",
        ServiceKind::Explicit => "显式化型",
        ServiceKind::Adopted => "收编型",
    }
}

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

fn render(registry: &ServiceRegistry, cfg: &GuiConfig, node_running: bool) -> String {
    let mut lines = vec![format!(
        "共 {} 项服务（闭集 v1，只加不删）",
        ServiceId::registry().len()
    )];
    for id in ServiceId::registry() {
        lines.push(format!(
            "{}  {}  默认={}  生效={}  来源={}",
            id.as_str(),
            kind_cn(id.kind()),
            on_off(id.default_enabled()),
            on_off(effective(*id, registry, cfg)),
            source_of(*id, registry),
        ));
    }
    lines.push(
        if node_running {
            "节点运行中：变更需重启节点生效"
        } else {
            "节点未运行：变更于下次启动自然生效"
        }
        .to_owned(),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_views_covers_full_closed_set_with_contract_shape() {
        let registry = ServiceRegistry::default();
        let views = build_views(&registry, &GuiConfig::default(), true);
        assert_eq!(views.len(), 10);
        assert_eq!(views[0].service_id, "serve.llm_share");
        assert_eq!(views[0].kind, "boolean");
        assert!(!views[0].enabled, "布尔型默认 off");
        assert!(views[0].requires_restart);
        let mdns = views.iter().find(|v| v.service_id == "discovery.mdns");
        assert_eq!(mdns.map(|v| v.kind), Some("adopted"));
        assert_eq!(mdns.map(|v| v.enabled), Some(true), "双读回落 enableMdns");
        let json = serde_json::to_value(&views[0]).unwrap_or(serde_json::Value::Null);
        assert!(
            json.get("serviceId").is_some() && json.get("requiresRestart").is_some(),
            "camelCase 逐字（§20.3）: {json}"
        );
    }

    #[test]
    fn entry_overrides_legacy_in_views() {
        let mut registry = ServiceRegistry::default();
        registry.set_enabled(ServiceId::NET_LAN_ONLY, true, 5);
        let views = build_views(&registry, &GuiConfig::default(), false);
        let lan = views.iter().find(|v| v.service_id == "net.lan_only");
        assert_eq!(lan.map(|v| v.enabled), Some(true));
    }

    #[test]
    fn load_rejects_corrupt_file_with_read_error() {
        let dir = std::env::temp_dir().join(format!("p2pctl-svc-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("services.json"), "{ not json").unwrap();
        let err = load(dir.to_str().unwrap_or_default()).unwrap_err();
        assert!(err.to_string().contains("拒绝静默回退空表"), "实际: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_file_is_empty_first_use() {
        let dir = std::env::temp_dir().join(format!("p2pctl-svc-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let registry = load(dir.to_str().unwrap_or_default()).unwrap();
        assert_eq!(registry.entries().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
