//! serve 闸 1 判定源装配（authz-role-design §8/§9，A3 S1）：authz.check(peer,
//! llm.borrow) 替代 LenderProxy 内建 allowlist 判定（ProxyConfig.allowlist 转
//! 只读归档不再消费，回滚 = revert 装配提交）。数据根与 CLI 共用
//! `<data-dir>/authz/`（store.data_dir() = CLI --data-dir 等价物），Clock 用
//! 系统时钟。每次判定重读绑定表（Authz::check 逐次 load_engine，沿 acp 面
//! DiskAuthz 先例），绑定变更即时生效。读失败 = 拒 + tracing::error（§11 红线 2）。

use std::path::Path;
use std::sync::Arc;

use llm_share_proxy::{AuthzChecker, Gate1Fn};
use p2p_authz::SystemClock;

/// 闸 1 判定闭包：准入 Ok；Err(审计 reason) 拒绝（wire 码恒 not_allowlisted）。
/// store-error 前缀在 LenderProxy 侧不可区分，此处补 error 级日志留可观测信号。
pub(crate) fn gate1(data_dir: &str) -> Gate1Fn {
    let checker = AuthzChecker::new(Path::new(data_dir), SystemClock);
    Arc::new(move |peer| match checker.check_borrow(peer) {
        Ok(()) => Ok(()),
        Err(reason) => {
            if reason.starts_with("store-error:") {
                tracing::error!(
                    peer = %peer,
                    "authz 绑定表读失败，闸 1 拒绝（红线 2，不静默放行）: {reason}"
                );
            }
            Err(reason)
        }
    })
}
