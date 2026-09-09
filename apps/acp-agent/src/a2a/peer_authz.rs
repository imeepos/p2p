//! A2A private task 准入的 peer 级闸（authz-role-design §8 双查分层）：
//! `check(peer, a2a.invoke)` 先拒——绑定缺失/过期/缺权限一律 peer 级拒；
//! agent 级 grants 表保留在 [super::task::TaskService::authorize] 兜对象粒度
//! （「针对哪个 agent」，两表各司其职）。读失败 = 拒（§11 红线 2），warn 日志
//! 加审计 detail 留观测信号，绝不静默放行。公开 agent 卡 discover 路径
//! （stream.rs 相可见性过滤）不经过本闸，维持现状。

use std::path::Path;

use p2p_authz::{Authz, Decision, Permission, SystemClock};

/// peer 级判定器：data_dir 指向节点数据根（authz 表在 authz/ 子目录，§6）。
pub(crate) type PeerInvokeGate = Authz<SystemClock>;

/// 从数据根构造（TaskService::new 装配；check 每次重读磁盘，撤绑立即生效）。
pub(crate) fn invoke_gate(data_root: &Path) -> PeerInvokeGate {
    Authz::new(data_root, SystemClock)
}

/// §8 peer 级闸：Ok = 放行进 agent 级 grants 复查；Err(detail) = 拒，detail 供审计。
pub(crate) fn peer_invoke_check(gate: &PeerInvokeGate, peer: &str) -> Result<(), String> {
    match gate.check(peer, Permission::A2A_INVOKE) {
        Ok(Decision::Allow) => Ok(()),
        Ok(Decision::Deny(reason)) => Err(format!("authz Deny({})", reason.code())),
        Err(err) => {
            tracing::warn!(peer = %peer, error = %err, "authz 读失败，按拒绝处理（红线 2）");
            Err(format!("authz read error: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    //! §8 双查矩阵：peer 级先拒 / agent 级兜底 / owner 与 public 路径不受影响。
    use std::sync::Arc;

    use super::peer_invoke_check;
    use crate::a2a::agents::AgentStore;
    use crate::a2a::grants::GrantStore;
    use crate::a2a::task::{TaskOpError, TaskService};
    use crate::audit::{AuditEvent, CaptureAudit};
    use crate::config::AgentConfig;
    use crate::workspaces::WorkspaceStore;
    use p2p_authz::Authz;

    const AGENT: &str = "agent-1";
    const PEER: &str = "peer-a";

    struct Fixture {
        service: Arc<TaskService>,
        grants: Arc<GrantStore>,
        authz: Authz<p2p_authz::SystemClock>,
        audit: Arc<CaptureAudit>,
        dir: std::path::PathBuf,
    }

    /// 独立临时数据根：agents.json + grants.json + authz/ 表互不串扰。
    fn fixture(visibility: a2a::Visibility) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "a2a-peer-authz-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let cfg = AgentConfig {
            data_dir: dir.to_string_lossy().into_owned(),
            command: vec!["true".into()],
            grace_secs: 1,
            permission_timeout_secs: 1,
            descriptor_disabled: true,
            admin_disabled: true,
            ..AgentConfig::default()
        };
        let agents = AgentStore::open(dir.join("agents.json")).expect("agents");
        agents
            .create(
                Some(AGENT.into()),
                "测试".into(),
                "d".into(),
                vec![],
                visibility,
                1,
            )
            .expect("create agent");
        let grants = Arc::new(GrantStore::open(dir.join("grants.json")).expect("grants"));
        let audit = Arc::new(CaptureAudit::new());
        let ws = Arc::new(WorkspaceStore::open(&[], None, cfg.paths().workspaces()).expect("ws"));
        let service = Arc::new(TaskService::new(
            cfg,
            agents,
            grants.clone(),
            ws,
            audit.clone(),
        ));
        let authz = Authz::new(&dir, p2p_authz::SystemClock);
        Fixture {
            service,
            grants,
            authz,
            audit,
            dir,
        }
    }

    fn denied_detail(fx: &Fixture) -> String {
        fx.audit
            .snapshot()
            .iter()
            .find_map(|e| match e {
                AuditEvent::A2aTaskDenied { detail, .. } => Some(detail.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// 断言 authorize 被 GateDenied 拒（TaskOpError 无 PartialEq，走 matches）。
    fn assert_denied(result: Result<(), TaskOpError>) {
        assert!(
            matches!(result, Err(TaskOpError::GateDenied)),
            "期望 GateDenied，实得 {result:?}"
        );
    }

    #[test]
    fn grant_without_binding_denied_at_peer_level() {
        let fx = fixture(a2a::Visibility::Private);
        fx.grants.grant(AGENT, PEER, 1).expect("grant");
        assert_denied(fx.service.authorize(AGENT, PEER, false));
        assert!(
            denied_detail(&fx).contains("authz Deny(NotBound)"),
            "审计带 reason 码，实得: {}",
            denied_detail(&fx)
        );
    }

    #[test]
    fn grant_with_operator_binding_allowed() {
        let fx = fixture(a2a::Visibility::Private);
        fx.grants.grant(AGENT, PEER, 1).expect("grant");
        fx.authz.bind(PEER, "operator", None, "test").expect("bind");
        assert!(fx.service.authorize(AGENT, PEER, false).is_ok());
        assert!(peer_invoke_check(&fx.authz, PEER).is_ok());
    }

    #[test]
    fn binding_without_grant_denied_at_agent_level() {
        let fx = fixture(a2a::Visibility::Private);
        fx.authz.bind(PEER, "operator", None, "test").expect("bind");
        assert_denied(fx.service.authorize(AGENT, PEER, false));
        let detail = denied_detail(&fx);
        assert!(
            !detail.contains("authz"),
            "peer 级已过，拒绝应归 agent 级，实得: {detail}"
        );
    }

    #[test]
    fn authz_read_failure_denies_with_signal() {
        let fx = fixture(a2a::Visibility::Private);
        fx.grants.grant(AGENT, PEER, 1).expect("grant");
        let authz_dir = fx.dir.join("authz");
        std::fs::create_dir_all(&authz_dir).expect("authz dir");
        std::fs::write(authz_dir.join("bindings.json"), b"{corrupted").expect("corrupt");
        assert_denied(fx.service.authorize(AGENT, PEER, false));
        assert!(denied_detail(&fx).contains("authz read error"));
    }

    #[test]
    fn private_owner_and_public_paths_untouched() {
        let owner = fixture(a2a::Visibility::Private);
        assert!(
            owner.service.authorize(AGENT, PEER, true).is_ok(),
            "owner 不查 authz/grants"
        );
        assert!(owner.grants.list().is_empty());

        let public = fixture(a2a::Visibility::Public);
        assert!(
            public.service.authorize(AGENT, PEER, false).is_ok(),
            "public 全放行，不经 peer 级闸（对照）"
        );
    }

    #[test]
    fn peer_invoke_check_reports_deny_reason_verbatim() {
        let fx = fixture(a2a::Visibility::Private);
        assert_eq!(
            peer_invoke_check(&fx.authz, PEER),
            Err("authz Deny(NotBound)".into())
        );
    }
}
