//! 登录鉴权与操作授权接缝：服务端可插拔。p2p-authz 权限模型收编属宿主
//! 装配侧工作（经 [Authorizer] 桥接），本 crate 只约定两个判定单点。

use std::collections::HashMap;

use p2p_identity::PeerId;

pub trait Authenticator: Send + Sync {
    /// 返回 true 表示允许该节点以此账号登录。
    fn login(&self, peer: &PeerId, user: &str, pass: &str) -> bool;
}

/// 操作类别（授权判定粒度）：读含浏览/下载，写含一切变更。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FtpOp {
    Read,
    Write,
}

pub trait Authorizer: Send + Sync {
    /// 登录后的逐命令授权；返回 false 服务端回 550。
    fn allow(&self, peer: &PeerId, user: &str, op: FtpOp) -> bool;
}

/// 全放行（默认语义，保持既有行为零变化）。
pub struct AllowAll;

impl Authorizer for AllowAll {
    fn allow(&self, _peer: &PeerId, _user: &str, _op: FtpOp) -> bool {
        true
    }
}

/// 全放行：任意账号密码皆可登录。仅限测试与本机联调，生产禁用。
pub struct OpenAuth;

impl Authenticator for OpenAuth {
    fn login(&self, _peer: &PeerId, _user: &str, _pass: &str) -> bool {
        true
    }
}

/// 静态账号表：精确匹配 user -> password。明文表仅限测试与离线场景。
pub struct StaticAuth {
    users: HashMap<String, String>,
}

impl StaticAuth {
    pub fn new(users: HashMap<String, String>) -> Self {
        Self { users }
    }
}

impl Authenticator for StaticAuth {
    fn login(&self, _peer: &PeerId, user: &str, pass: &str) -> bool {
        self.users.get(user).is_some_and(|want| want == pass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer() -> PeerId {
        PeerId::from_bytes([7u8; 32])
    }

    #[test]
    fn open_auth_accepts_anything() {
        assert!(OpenAuth.login(&peer(), "anyone", "whatever"));
    }

    #[test]
    fn allow_all_authorizes_everything() {
        assert!(AllowAll.allow(&peer(), "u", FtpOp::Read));
        assert!(AllowAll.allow(&peer(), "u", FtpOp::Write));
    }

    #[test]
    fn static_auth_exact_match_only() {
        let mut users = HashMap::new();
        users.insert("alice".to_string(), "s3cret".to_string());
        let auth = StaticAuth::new(users);
        assert!(auth.login(&peer(), "alice", "s3cret"));
        assert!(!auth.login(&peer(), "alice", "wrong"));
        assert!(!auth.login(&peer(), "bob", "s3cret"));
    }
}
