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
use p2p_ftp::{serve_with_config, FtpConfig, LocalFs, OpenAuth, StaticAuth};
use serde::Deserialize;

/// ftp.json：FTP 服务端文件根与账号表。
#[derive(Debug, Deserialize)]
struct FtpDaemonConfig {
    /// 服务端文件根目录（越狱边界，LocalFs canonicalize 固定）
    root: String,
    /// 账号表 user → password；空表 = OpenAuth 全放行（仅限信任网络，
    /// FT6 接 p2p-authz 后按节点+账号权限收敛）
    #[serde(default)]
    accounts: HashMap<String, String>,
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
    if cfg.accounts.is_empty() {
        eprintln!("p2pctl-daemon: serve.ftp 账号表为空，按 OpenAuth 全放行（仅限信任网络）");
    }
    let auth: Arc<dyn p2p_ftp::Authenticator> = if cfg.accounts.is_empty() {
        Arc::new(OpenAuth)
    } else {
        Arc::new(StaticAuth::new(cfg.accounts.clone()))
    };
    match serve_with_config(node, fs, auth, FtpConfig::default()) {
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
        let bad: FtpDaemonConfig = serde_json::from_str(r#"{"root":"  "}"#).unwrap();
        assert!(bad.root.trim().is_empty(), "空 root 由调用方拦截");
    }
}
