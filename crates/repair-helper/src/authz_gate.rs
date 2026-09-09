//! mint 前置授权闸（authz-a3-plan §1 S2 P1a）：repair-helper mint-ticket 签发前
//! 校验 bridge_peer 持有 repair.diag（scope=diag）/ repair.fix（scope=fix）。
//! 判定委托 p2p-authz 四步瀑布（§7），读失败=拒（role-design §11 红线 2）；
//! 拒绝落 authz.denied 审计事件（写失败由 audit::record 留日志，不阻塞拒绝）。
//! 票据本体语义（一次性/签名/过期/绑对端）零改动——本闸只做签发前置。

use std::path::Path;

use p2p_authz::audit::AuditEvent;
use p2p_authz::{Authz, Clock, Permission};

/// scope → 权限 key 映射（§4 登记表：repair.diag / repair.fix）。
pub fn scope_permission(scope: &str) -> Option<Permission> {
    match scope {
        "diag" => Permission::parse("repair.diag"),
        "fix" => Permission::parse("repair.fix"),
        _ => None,
    }
}

/// 闸结果：Allowed 放行铸票；Denied 带可读 reason（入 stderr 与审计 note）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    Allowed,
    Denied { reason: String },
}

/// mint 前置判定：scope 映射权限 → p2p-authz check（Clock 注入）。
/// 拒绝（无绑定/过期/权限不含/读失败）即落 authz.denied 审计事件。
pub fn check_bridge<C: Clock>(
    data_dir: &Path,
    bridge_peer: &str,
    scope: &str,
    clock: C,
) -> GateOutcome {
    let deny = |reason: String| {
        record_denial(data_dir, bridge_peer, scope, &reason);
        GateOutcome::Denied { reason }
    };
    let Some(perm) = scope_permission(scope) else {
        return deny(format!("bad scope: {scope}"));
    };
    match Authz::new(data_dir, clock).check(bridge_peer, perm) {
        Ok(p2p_authz::Decision::Allow) => GateOutcome::Allowed,
        Ok(p2p_authz::Decision::Deny(reason)) => deny(reason.code().to_owned()),
        Err(e) => deny(format!("authz read failed: {e}")),
    }
}

/// 拒绝落账：note 携带面（mint-ticket）/对象（peer）/scope/reason 码。
fn record_denial(data_dir: &Path, bridge_peer: &str, scope: &str, reason: &str) {
    let event = AuditEvent::denied(format!(
        "mint-ticket scope={scope} peer={bridge_peer} denied: {reason}"
    ));
    p2p_authz::audit::record(data_dir, &event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_authz::audit::audit_path;
    use p2p_authz::SystemClock;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rh-authz-gate-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn peer(byte: u8) -> String {
        bs58::encode([byte; 32]).into_string()
    }

    /// 造绑定态：bridge_peer → operator（含 repair.diag）/ ally（含 repair.fix）。
    fn bind_peer(data_dir: &Path, bridge_peer: &str, role_id: &str) {
        Authz::new(data_dir, SystemClock)
            .bind(bridge_peer, role_id, None, "")
            .unwrap();
    }

    fn denied_line(data_dir: &Path) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(audit_path(data_dir)).ok()?;
        text.lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|v| v["kind"] == "authz.denied")
    }

    #[test]
    fn bound_bridge_with_scope_permission_passes() {
        let dir = temp_dir("allow");
        let bridge = peer(1);
        bind_peer(&dir, &bridge, "operator");
        assert_eq!(
            check_bridge(&dir, &bridge, "diag", SystemClock),
            GateOutcome::Allowed,
            "operator 持 repair.diag，diag 工单放行"
        );
        bind_peer(&dir, &bridge, "ally");
        assert_eq!(
            check_bridge(&dir, &bridge, "fix", SystemClock),
            GateOutcome::Allowed,
            "ally 持 repair.fix，fix 工单放行"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unbound_bridge_denied_with_audit_event() {
        let dir = temp_dir("deny");
        let bridge = peer(2);
        let outcome = check_bridge(&dir, &bridge, "diag", SystemClock);
        assert_eq!(
            outcome,
            GateOutcome::Denied {
                reason: "NotBound".to_owned()
            }
        );
        let event = denied_line(&dir).expect("拒绝必须落 authz.denied 事件");
        assert_eq!(event["actor"], "owner");
        assert!(
            event["note"]
                .as_str()
                .unwrap()
                .contains(&format!("peer={bridge}")),
            "note 携带对象 peer: {:?}",
            event["note"]
        );
        assert!(event["note"].as_str().unwrap().contains("scope=diag"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_failure_denied_with_audit_event() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(dir.join("authz")).unwrap();
        std::fs::write(dir.join("authz").join("bindings.json"), "{ not json").unwrap();
        let bridge = peer(3);
        let outcome = check_bridge(&dir, &bridge, "fix", SystemClock);
        let GateOutcome::Denied { reason } = outcome else {
            panic!("读失败必须拒绝（红线 2：authz 读失败=拒）");
        };
        assert!(
            reason.contains("authz read failed"),
            "reason 留读失败信号: {reason}"
        );
        assert!(denied_line(&dir).is_some(), "读失败拒绝同样落审计事件");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scope_permission_maps_closed_set() {
        assert_eq!(scope_permission("diag"), Permission::parse("repair.diag"));
        assert_eq!(scope_permission("fix"), Permission::parse("repair.fix"));
        assert_eq!(scope_permission("root"), None, "表外 scope 无权限映射");
    }
}
