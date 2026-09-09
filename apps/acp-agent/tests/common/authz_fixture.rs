#![allow(dead_code)]

//! authz 测试夹具（A2-ACP）：与 §9 import 同源的 scope→内建角色映射，模拟
//! 「import → 判定切换」后的稳态；绑定直接落 <data-dir>/authz/（桥 DiskAuthz
//! 同源读取）。绑定缺失即拒的负例用例不写绑定。

use std::path::Path;

use acp_common::Scope;
use p2p_authz::{Authz, SystemClock};

use acp_agent::AgentConfig;
use p2p::PeerId;

/// §9 映射：sandbox→guest / workspace→operator；owner 不进 authz（红线 1）。
pub fn scope_binding_role(scope: Scope) -> Option<&'static str> {
    match scope {
        Scope::Sandbox => Some("guest"),
        Scope::Workspace => Some("operator"),
        Scope::Owner => None,
    }
}

/// 写一条绑定（测试前置件）。
pub fn write_binding(cfg: &AgentConfig, peer: &PeerId, role_id: &str) {
    let authz = Authz::new(Path::new(&cfg.data_dir), SystemClock);
    authz
        .bind(&peer.to_string(), role_id, None, "itest-fixture")
        .expect("fixture binding");
}

/// 按策略条目 scope 写匹配绑定（§9 import 语义；Owner 条目跳过）。
pub fn bind_scope(cfg: &AgentConfig, peer: &PeerId, scope: Scope) {
    if let Some(role) = scope_binding_role(scope) {
        write_binding(cfg, peer, role);
    }
}

/// 清空绑定表（负例构造：策略表有、绑定无 → 准入双查必拒）。
pub fn clear_bindings(cfg: &AgentConfig) {
    let path = Path::new(&cfg.data_dir).join("authz").join("bindings.json");
    let _ = std::fs::remove_file(path);
}

/// 读回绑定表（断言用）。
pub fn bindings(cfg: &AgentConfig) -> Vec<p2p_authz::Binding> {
    p2p_authz::store::load_bindings(Path::new(&cfg.data_dir)).expect("fixture bindings readable")
}
