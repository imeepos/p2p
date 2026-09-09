//! authz 闸（authz-role-design §8 ACP 两行的 PEP）：
//! 准入双查查 acp.session、权限瀑布 Forward 前置查 acp.execute。
//! owner 不进 authz（红线 1：调用方按 scope 分流，本模块不感知 owner）；
//! 判定每次重读两表：CLI/GUI 改绑即时生效（红线 3，无缓存失效问题）；
//! 存储读取失败返回 None，调用方必须按 Deny 处理（红线 2 fail-closed）。
//! trait 抽象供测试替身；磁盘实现委托 p2p-authz，不复制判定语义。

use std::path::{Path, PathBuf};

use p2p_authz::{Authz, Decision, Permission, SystemClock};

/// 判定入口：None = 判定不可得（存储故障，调用方按拒绝处理）。
pub trait AuthzGate: Send + Sync {
    fn check(&self, peer: &str, perm: Permission) -> Option<Decision>;
}

/// 磁盘实现：authz 表在桥数据根的 authz/ 子目录（与 p2pctl authz 同一约定）。
pub struct DiskAuthz {
    data_dir: PathBuf,
}

impl DiskAuthz {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }
}

impl AuthzGate for DiskAuthz {
    fn check(&self, peer: &str, perm: Permission) -> Option<Decision> {
        let authz = Authz::new(&self.data_dir, SystemClock);
        match authz.check(peer, perm) {
            Ok(decision) => {
                tracing::debug!(peer, perm = perm.as_str(), %decision, "authz checked");
                Some(decision)
            }
            Err(err) => {
                tracing::error!(
                    peer,
                    perm = perm.as_str(),
                    error = %err,
                    "authz read failed; denying (fail-closed)"
                );
                None
            }
        }
    }
}

/// 闸口径：恰为 Allow 才放行；None（存储故障）与其余 Deny reason 同拒。
pub fn allows_decision(decision: Option<Decision>) -> bool {
    matches!(decision, Some(Decision::Allow))
}

/// Deny reason 码（审计 detail 用）；None 存储故障记 storage-error。
pub fn deny_detail(decision: Option<Decision>) -> String {
    match decision {
        Some(p2p_authz::Decision::Allow) => "allow".to_owned(),
        Some(p2p_authz::Decision::Deny(reason)) => reason.code().to_owned(),
        None => "storage-error".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_authz::DenyReason;

    #[test]
    fn only_explicit_allow_passes_the_gate() {
        assert!(allows_decision(Some(Decision::Allow)));
        assert!(!allows_decision(Some(Decision::Deny(DenyReason::NotBound))));
        // 存储故障 = 拒（红线 2）
        assert!(!allows_decision(None));
    }

    #[test]
    fn deny_detail_covers_all_outcomes() {
        assert_eq!(deny_detail(Some(Decision::Allow)), "allow");
        assert_eq!(
            deny_detail(Some(Decision::Deny(DenyReason::Expired))),
            "Expired"
        );
        assert_eq!(deny_detail(None), "storage-error");
    }

    struct Fixed(Option<Decision>);

    impl AuthzGate for Fixed {
        fn check(&self, _peer: &str, _perm: Permission) -> Option<Decision> {
            self.0
        }
    }

    #[test]
    fn fixed_gate_returns_its_verdict() {
        assert_eq!(
            Fixed(Some(Decision::Allow)).check("p", Permission::ACP_SESSION),
            Some(Decision::Allow)
        );
        assert_eq!(Fixed(None).check("p", Permission::ACP_SESSION), None);
    }
}
