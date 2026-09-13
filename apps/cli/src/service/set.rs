//! p2pctl service enable/disable：闭集内 upsert 落盘 services.json
//!（先重读磁盘合并，tmp+rename 原子写委托 p2p-service；不改运行中节点行为）。

use std::path::Path;

use clap::Args;
use serde::Serialize;

use p2p_service::{save_registry, ServiceId};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{now_unix, parse_closed, store_err};

#[derive(Args)]
pub struct SetArgs {
    /// 服务 id（§20.1 闭集，如 serve.llm_share）
    pub service_id: String,
    /// 输出结构化 JSON（ServiceMutationReport，gui-contract §20.3 同形）
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（services.json 与 authz 数据根同根）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

/// §20.3 ServiceMutationReport（camelCase 逐字）：requiresRestart = 节点运行中。
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceMutationReportJson {
    pub service_id: String,
    pub enabled: bool,
    pub requires_restart: bool,
}

pub async fn run(args: SetArgs, enabled: bool) -> CliResult<()> {
    let id = parse_closed(&args.service_id)?;
    let now = now_unix()?;
    let report = set_enabled(Path::new(&args.data_dir), id, enabled, now)?;
    let node_running = crate::lifecycle::probe_online(&args.data_dir)
        .await
        .is_some();
    let out = ServiceMutationReportJson {
        service_id: report.service_id,
        enabled: report.enabled,
        requires_restart: node_running,
    };
    let verb = if enabled { "已启用" } else { "已停用" };
    output::emit(
        args.json,
        &out,
        &format!(
            "{verb} {}（services.json 已更新，下次节点启动生效）",
            id.as_str()
        ),
    )
}

/// 落盘内核（可测）：先重读磁盘合并再 upsert + 原子写（§4.5 双进程写窗口收敛）。
fn set_enabled(
    data_dir: &Path,
    id: ServiceId,
    enabled: bool,
    now_unix: u64,
) -> CliResult<MutationApplied> {
    let mut registry = super::load(data_dir.to_str().unwrap_or_default())?;
    registry.set_enabled(id, enabled, now_unix);
    save_registry(data_dir, &registry).map_err(store_err)?;
    Ok(MutationApplied {
        service_id: id.as_str().to_owned(),
        enabled,
    })
}

/// 落盘结果（渲染前中间形状，不带 requiresRestart：运行态属命令层观测）。
#[derive(Debug)]
struct MutationApplied {
    service_id: String,
    enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::load;
    use p2p_service::store::services_path;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pctl-svc-set-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn enable_upserts_entry_and_second_toggle_reuses_slot() {
        let dir = scratch("upsert");
        set_enabled(&dir, ServiceId::SERVE_TUNNEL, true, 100).unwrap();
        set_enabled(&dir, ServiceId::SERVE_TUNNEL, false, 200).unwrap();
        let registry = load(dir.to_str().unwrap_or_default()).unwrap();
        let entries = registry.entries();
        assert_eq!(entries.len(), 1, "同 id upsert 不追加条目");
        assert_eq!(entries[0].service_id, ServiceId::SERVE_TUNNEL);
        assert!(!entries[0].enabled);
        assert_eq!(entries[0].updated_at, 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_file_is_version_one_envelope() {
        let dir = scratch("envelope");
        set_enabled(&dir, ServiceId::SERVE_ACP, false, 7).unwrap();
        let raw = std::fs::read_to_string(services_path(&dir)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["services"][0]["service_id"], "serve.acp");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_existing_file_is_explicit_error_not_overwrite() {
        let dir = scratch("corrupt");
        std::fs::write(services_path(&dir), "{ not json").unwrap();
        let err = set_enabled(&dir, ServiceId::SERVE_TUNNEL, true, 1).unwrap_err();
        assert!(err.to_string().contains("拒绝静默回退空表"), "实际: {err}");
        assert!(
            std::fs::read_to_string(services_path(&dir))
                .unwrap()
                .contains("not json"),
            "写失败不覆盖原文件"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mutation_report_json_is_camel_case_contract_shape() {
        let report = ServiceMutationReportJson {
            service_id: "serve.tunnel".to_owned(),
            enabled: true,
            requires_restart: false,
        };
        let json = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        assert_eq!(json["serviceId"], "serve.tunnel");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["requiresRestart"], false);
    }
}
