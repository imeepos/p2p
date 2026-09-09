//! 闸 1 判定源（authz-role-design §8/§9，A2 切换件）：authz.check(peer, llm.borrow)。
//! 装配 [AuthzChecker::into_gate] 后，[crate::LenderProxy] 闸 1 以绑定表裁决，
//! ProxyConfig.allowlist 转只读归档不再消费（回滚 = 撤下装配点恢复旧判定）。
//! 存储读失败按拒绝处理（§11 红线 2：authz 读失败 = 拒），审计 reason 带明细。

use std::path::Path;
use std::sync::Arc;

use p2p_authz::{Authz, Clock, Decision, Permission};

/// 闸 1 判定闭包：Ok = 准入；Err(审计 reason) = 拒绝。
/// wire 错误码恒 not_allowlisted（协议不 bump），reason 只进 message 与审计日志。
pub type Gate1Fn = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// authz 判定源：peer → check(peer, [Permission::LLM_BORROW])，§7 四步瀑布纯判定。
/// Clock 需 Send + Sync：闸 1 闭包跨请求并发调用（Gate1Fn 的线程安全要求）。
pub struct AuthzChecker<C: Clock + Send + Sync> {
    authz: Authz<C>,
}

impl<C: Clock + Send + Sync + 'static> AuthzChecker<C> {
    pub fn new(data_dir: &Path, clock: C) -> Self {
        Self {
            authz: Authz::new(data_dir, clock),
        }
    }

    /// 判定借方 llm.borrow 准入；authz 存储读失败显式拒绝并携带错误串。
    pub fn check_borrow(&self, peer_id: &str) -> Result<(), String> {
        match self.authz.check(peer_id, Permission::LLM_BORROW) {
            Ok(Decision::Allow) => Ok(()),
            Ok(Decision::Deny(reason)) => Err(reason.code().to_owned()),
            Err(e) => Err(format!("store-error: {e}")),
        }
    }

    /// 类型擦除为闸 1 闭包（LenderProxy::with_gate1_authz 装配用）。
    pub fn into_gate(self) -> Gate1Fn {
        Arc::new(move |peer| self.check_borrow(peer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_authz::SystemClock;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "llm-share-proxy-gate1-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn bind_ally(data_dir: &Path, peer: &str, expires_at: Option<u64>) {
        Authz::new(data_dir, SystemClock)
            .bind(peer, "ally", expires_at, "gate1-test")
            .expect("bind ally");
    }

    #[test]
    fn bound_peer_with_llm_borrow_passes() {
        let dir = temp_dir("allow");
        bind_ally(&dir, "peer-a", None);
        let checker = AuthzChecker::new(&dir, SystemClock);
        assert_eq!(checker.check_borrow("peer-a"), Ok(()));
    }

    #[test]
    fn unbound_peer_denied_with_not_bound() {
        let dir = temp_dir("unbound");
        let checker = AuthzChecker::new(&dir, SystemClock);
        assert_eq!(checker.check_borrow("peer-x"), Err("NotBound".to_owned()));
    }

    #[test]
    fn expired_binding_denied_and_wrong_role_missing_perm() {
        let dir = temp_dir("deny");
        bind_ally(&dir, "peer-old", Some(1_000));
        Authz::new(&dir, SystemClock)
            .bind("peer-friend", "friend", None, "gate1-test")
            .expect("bind friend");
        let checker = AuthzChecker::new(&dir, SystemClock);
        assert_eq!(checker.check_borrow("peer-old"), Err("Expired".to_owned()));
        assert_eq!(
            checker.check_borrow("peer-friend"),
            Err("MissingPerm".to_owned())
        );
    }

    #[test]
    fn corrupted_store_denies_instead_of_failing_open() {
        let dir = temp_dir("corrupt");
        bind_ally(&dir, "peer-a", None);
        let path = dir.join("authz").join("bindings.json");
        fs::write(&path, b"{not json").expect("corrupt store");
        let checker = AuthzChecker::new(&dir, SystemClock);
        let verdict = checker.check_borrow("peer-a");
        assert!(verdict.is_err(), "存储损坏必须拒绝（红线 2）");
        assert!(verdict.unwrap_err().starts_with("store-error:"));
    }

    #[test]
    fn into_gate_erases_type_and_keeps_verdict() {
        let dir = temp_dir("gate");
        bind_ally(&dir, "peer-a", None);
        let gate = AuthzChecker::new(&dir, SystemClock).into_gate();
        assert_eq!(gate("peer-a"), Ok(()));
        assert_eq!(gate("peer-none"), Err("NotBound".to_owned()));
    }
}
