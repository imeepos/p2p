//! ACP 桥数据目录约定（沿用 apps/cli/src/paths.rs 风格）：调用方注入 data-dir，
//! 本模块只派生文件路径；策略文件名与 policy 模块共用。

use std::path::PathBuf;

pub const POLICY_FILE: &str = "acp-policy.json";
pub const LOG_DIR: &str = "acp-logs";

/// 一个桥数据目录实例的全部派生路径。
#[derive(Clone, Debug)]
pub struct AcpPaths {
    pub root: PathBuf,
}

impl AcpPaths {
    pub fn new(data_dir: &str) -> Self {
        Self {
            root: PathBuf::from(data_dir),
        }
    }

    pub fn policy(&self) -> PathBuf {
        self.root.join(POLICY_FILE)
    }

    /// 子进程 stderr 滚动日志目录（设计 §4.2-5）。
    pub fn log_dir(&self) -> PathBuf {
        self.root.join(LOG_DIR)
    }

    pub fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_paths_from_root() {
        let p = AcpPaths::new("/tmp/acp-x");
        assert_eq!(p.policy(), PathBuf::from("/tmp/acp-x/acp-policy.json"));
        assert_eq!(p.log_dir(), PathBuf::from("/tmp/acp-x/acp-logs"));
    }

    #[test]
    fn protocol_id_matches_registered_literal() {
        assert_eq!(crate::consts::PROTOCOL_ID, "/dsh-acp/1");
    }
}
