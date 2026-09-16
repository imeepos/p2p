//! daemon 侧 FTP 服务端装配（serve.ftp 开关消费点，service-registry-design §2）。
//!
//! 语义：开关 AND 配置双条件——services.json 里 serve.ftp=on 且
//! <data-root>/ftp.json 配置了 root 才装配 /ftp/ctrl/1 + /ftp/data/1；
//! 两者缺一即不装配（默认全关 = 零行为变化）。ftp.json 缺失/损坏 fail-safe
//! 跳过装配留 stderr 告警（同 NodeServiceSwitches::load 先例，不拒启动）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use p2p::Node;
use p2p_ftp::{serve_with_authz, FtpConfig, LocalFs, OpenAuth, StaticAuth};
use serde::Deserialize;

/// ftp.json：FTP 服务端文件根与账号表。
#[derive(Debug, Deserialize)]
struct FtpDaemonConfig {
    /// 服务端文件根目录（越狱边界，LocalFs canonicalize 固定）
    root: String,
    /// 账号表 user → password；空表 = OpenAuth 全放行（仅限信任网络，
    /// authz=true 时账号表为登录前置，空表即全部拒绝）
    #[serde(default)]
    accounts: HashMap<String, String>,
    /// true = 逐命令授权走 p2p-authz（file.read/file.write 按节点判定）
    #[serde(default)]
    authz: bool,
}

/// p2p-authz 桥（FT6）：登录 = 账号匹配 AND 任一 file.* 权限；
/// 逐命令 = file.read / file.write 按 peer 判定（同源同根 data-dir）。
#[derive(Clone)]
struct AuthzFtpBridge {
    authz: std::sync::Arc<p2p_authz::Authz<p2p_authz::SystemClock>>,
    accounts: HashMap<String, String>,
}

impl AuthzFtpBridge {
    fn new(data_root: &Path, accounts: HashMap<String, String>) -> Self {
        Self {
            authz: std::sync::Arc::new(p2p_authz::Authz::new(data_root, p2p_authz::SystemClock)),
            accounts,
        }
    }

    fn allowed(&self, peer: &p2p::PeerId, op: p2p_ftp::FtpOp) -> bool {
        let perm = match op {
            p2p_ftp::FtpOp::Read => p2p_authz::Permission::FILE_READ,
            p2p_ftp::FtpOp::Write => p2p_authz::Permission::FILE_WRITE,
        };
        matches!(
            self.authz.check(&peer.to_string(), perm),
            Ok(p2p_authz::Decision::Allow),
        )
    }
}

impl p2p_ftp::Authenticator for AuthzFtpBridge {
    fn login(&self, peer: &p2p::PeerId, user: &str, pass: &str) -> bool {
        self.accounts.get(user).is_some_and(|want| want == pass)
            && (self.allowed(peer, p2p_ftp::FtpOp::Read)
                || self.allowed(peer, p2p_ftp::FtpOp::Write))
    }
}

impl p2p_ftp::Authorizer for AuthzFtpBridge {
    fn allow(&self, peer: &p2p::PeerId, _user: &str, op: p2p_ftp::FtpOp) -> bool {
        self.allowed(peer, op)
    }
}

/// 开关 on 才尝试装配；任何配置/根目录问题只告警不拒 daemon 启动。
pub(crate) fn maybe_serve(node: &Node, data_root: &Path, ftp_enabled: bool) {
    if !ftp_enabled {
        return;
    }
    let path = data_root.join("ftp.json");
    let skip = |reason: String| {
        eprintln!("p2pctl-daemon: serve.ftp 不装配：{reason}（path={}）", path.display());
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) => return skip(format!("ftp.json 读取失败: {e}")),
    };
    let cfg: FtpDaemonConfig = match serde_json::from_str(&raw) {
        Ok(cfg) => cfg,
        Err(e) => return skip(format!("ftp.json 解析失败: {e}")),
    };
    if cfg.root.trim().is_empty() {
        return skip("root 为空".into());
    }
    let fs = match LocalFs::open(&cfg.root) {
        Ok(fs) => Arc::new(fs),
        Err(e) => return skip(format!("root 无法 canonicalize（{}）: {e}", cfg.root)),
    };
    if cfg.accounts.is_empty() && !cfg.authz {
        eprintln!("p2pctl-daemon: serve.ftp 账号表为空，按 OpenAuth 全放行（仅限信任网络）");
    }
    let auth: Arc<dyn p2p_ftp::Authenticator>;
    let authz: Arc<dyn p2p_ftp::Authorizer>;
    if cfg.authz {
        if cfg.accounts.is_empty() {
            eprintln!("p2pctl-daemon: serve.ftp authz=true 且账号表为空：所有登录都将被拒绝");
        }
        // authz 同源同根（§4.4）：与 services.json 同一数据根。
        let bridge = AuthzFtpBridge::new(data_root, cfg.accounts.clone());
        auth = Arc::new(bridge.clone());
        authz = Arc::new(bridge);
        eprintln!("p2pctl-daemon: serve.ftp 已按 p2p-authz 收编（file.read/file.write 按节点判定）");
    } else {
        auth = if cfg.accounts.is_empty() {
            Arc::new(OpenAuth)
        } else {
            Arc::new(StaticAuth::new(cfg.accounts.clone()))
        };
        authz = Arc::new(p2p_ftp::AllowAll);
    }
    match serve_with_authz(node, fs, auth, authz, FtpConfig::default()) {
        Ok(_) => eprintln!(
            "p2pctl-daemon: FTP 服务端已装配 root={} accounts={} 协议=/ftp/ctrl/1+/ftp/data/1",
            cfg.root,
            cfg.accounts.len()
        ),
        Err(e) => skip(format!("FTP 服务端装配失败: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftp_json_parses_root_and_accounts() {
        let cfg: FtpDaemonConfig =
            serde_json::from_str(r#"{"root":"/tmp/ftp-root","accounts":{"alice":"pw"}}"#).unwrap();
        assert_eq!(cfg.root, "/tmp/ftp-root");
        assert_eq!(cfg.accounts.len(), 1);
    }

    #[test]
    fn ftp_json_accounts_optional_and_blank_root_detectable() {
        let cfg: FtpDaemonConfig = serde_json::from_str(r#"{"root":"/srv/ftp"}"#).unwrap();
        assert!(cfg.accounts.is_empty(), "accounts 缺省 = 空表（OpenAuth 语义）");
        assert!(!cfg.authz, "authz 缺省关 = 既有语义零变化");
        let bad: FtpDaemonConfig = serde_json::from_str(r#"{"root":"  "}"#).unwrap();
        assert!(bad.root.trim().is_empty(), "空 root 由调用方拦截");
    }

    /// FT6 桥默认拒绝：未绑定 peer（无自定义角色）即使密码正确也拒登录。
    #[test]
    fn authz_bridge_denies_unbound_peer_by_default() {
        use p2p_ftp::{Authorizer as _, Authenticator as _};
        let dir = std::env::temp_dir().join(format!("p2p-ftp-authz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut accounts = HashMap::new();
        accounts.insert("alice".to_string(), "pw".to_string());
        let bridge = AuthzFtpBridge::new(&dir, accounts);
        let peer = p2p::PeerId::from_bytes([9u8; 32]);
        assert!(!bridge.login(&peer, "alice", "pw"), "未绑定 peer 默认拒（NotBound）");
        assert!(!bridge.allowed(&peer, p2p_ftp::FtpOp::Read));
    }
}
