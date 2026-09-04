//! 数据目录布局：--data-dir 是 CLI 的 app 数据目录等价物（默认 ./p2p-data），
//! 文件名与 GUI 约定一致（gui-config.json / node-profile.json），节点数据目录
//! 默认为其下 p2p-data/（对齐 GUI ConfigStore::default_config 的 dataDir 兜底）。
//!
//! 守护进程可观测信号：daemon.pid（进程号）/ daemon.meta.json（peer 与监听地址）
//! / daemon.log（p2p-log 落盘）/ daemon.sock（控制通道）。

use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "gui-config.json";
pub const PROFILE_FILE: &str = "node-profile.json";
pub const PID_FILE: &str = "daemon.pid";
pub const META_FILE: &str = "daemon.meta.json";
pub const SOCK_FILE: &str = "daemon.sock";
pub const LOG_FILE: &str = "daemon.log";

/// 一个 --data-dir 实例的全部派生路径。
#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(data_dir: &str) -> Self {
        Self {
            root: PathBuf::from(data_dir),
        }
    }

    pub fn config(&self) -> PathBuf {
        self.root.join(CONFIG_FILE)
    }

    pub fn profile(&self) -> PathBuf {
        self.root.join(PROFILE_FILE)
    }

    pub fn pid(&self) -> PathBuf {
        self.root.join(PID_FILE)
    }

    pub fn meta(&self) -> PathBuf {
        self.root.join(META_FILE)
    }

    pub fn sock(&self) -> PathBuf {
        self.root.join(SOCK_FILE)
    }

    pub fn log(&self) -> PathBuf {
        self.root.join(LOG_FILE)
    }

    /// 节点身份数据目录：优先取持久化配置 dataDir，缺省回落 root/p2p-data。
    pub fn node_data_dir(&self, config_data_dir: Option<&str>) -> PathBuf {
        match config_data_dir {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => self.default_node_data_dir(),
        }
    }

    pub fn default_node_data_dir(&self) -> PathBuf {
        self.root.join("p2p-data")
    }

    pub fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }
}

/// 删除文件；NotFound 视为已清理，其余错误上抛（禁止静默吞错）。
pub fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_derive_from_root() {
        let p = Paths::new("/tmp/x");
        assert_eq!(p.config(), PathBuf::from("/tmp/x/gui-config.json"));
        assert_eq!(p.pid(), PathBuf::from("/tmp/x/daemon.pid"));
        assert_eq!(p.default_node_data_dir(), PathBuf::from("/tmp/x/p2p-data"));
    }

    #[test]
    fn node_data_dir_prefers_config_value() {
        let p = Paths::new("/tmp/x");
        assert_eq!(p.node_data_dir(Some("/abs/dir")), PathBuf::from("/abs/dir"));
        assert_eq!(p.node_data_dir(None), PathBuf::from("/tmp/x/p2p-data"));
    }
}
