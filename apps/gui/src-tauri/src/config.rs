//! GuiConfig 持久化（gui-contract.md §1/§3）：app 数据目录 gui-config.json，原子写（tmp+rename）。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tracing::warn;

use crate::types::GuiConfig;

/// 配置文件名（pub(crate) 供 watcher 白名单按 store 层语义归类）。
pub(crate) const FILE_NAME: &str = "gui-config.json";

/// 出厂内置云端 bootstrap（契约 v2：rendezvous，QUIC 语法）。
pub(crate) fn default_bootstrap() -> Vec<String> {
    vec![
        "43.240.223.138/u3400".into(),
        "121.196.193.177/u3400".into(),
    ]
}

/// 出厂内置云端中继（契约 v2：relay，QUIC 语法）。
pub(crate) fn default_relay_addrs() -> Vec<String> {
    vec![
        "43.240.223.138/u3403".into(),
        "121.196.193.177/u3403".into(),
    ]
}

/// 出厂内置观测反射口（socket 语法）。
pub(crate) fn default_observation_addrs() -> Vec<String> {
    vec!["121.196.193.177:3402".into()]
}

/// enableMdns 的字段级默认（serde 字段缺失时生效）。
pub(crate) fn default_true() -> bool {
    true
}

/// dataDir 的字段级默认（无 app 目录上下文时的相对兜底）。
pub(crate) fn default_data_dir() -> String {
    "./p2p-data".into()
}

/// authzDefaultRole 的字段级默认（契约 §18.3：缺省 friend；空串 = 禁用自动绑）。
pub(crate) fn default_authz_default_role() -> String {
    "friend".into()
}

/// rdRequireApproval 的字段级默认（CC2：审批闸缺省开，零行为变化）。
pub(crate) fn default_rd_require_approval() -> bool {
    true
}

/// rdFps 的字段级默认（CC2：初始质量档 15，合法域 1..=60）。
pub(crate) fn default_rd_fps() -> u8 {
    15
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            quic_port: 0,
            tcp_port: 0,
            enable_mdns: true,
            data_dir: default_data_dir(),
            bootstrap: default_bootstrap(),
            relay_addrs: default_relay_addrs(),
            advertised_addrs: Vec::new(),
            observation_port: None,
            observation_addrs: default_observation_addrs(),
            lan_only: false,
            authz_default_role: default_authz_default_role(),
            rd_require_approval: default_rd_require_approval(),
            rd_fps: default_rd_fps(),
            tunnel_serve_allow: Vec::new(),
        }
    }
}

/// 持久化读写句柄：绑定 app 数据目录，串行化写盘。
pub struct ConfigStore {
    app_data_dir: PathBuf,
    io_lock: Mutex<()>,
}

impl ConfigStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            io_lock: Mutex::new(()),
        }
    }

    fn path(&self) -> PathBuf {
        self.app_data_dir.join(FILE_NAME)
    }

    /// 契约 §3 默认值：内置云端端点 + dataDir = app 数据目录下 p2p-data。
    pub fn default_config(&self) -> GuiConfig {
        GuiConfig {
            data_dir: self
                .app_data_dir
                .join("p2p-data")
                .to_string_lossy()
                .into_owned(),
            ..GuiConfig::default()
        }
    }

    /// 读配置：无文件返回默认值；损坏回退默认值并告警（禁止静默）。
    pub fn load(&self) -> GuiConfig {
        let path = self.path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return self.default_config(),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "读取配置失败，回退默认配置");
                return self.default_config();
            }
        };
        match serde_json::from_str(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!(error = %e, path = %path.display(), "配置文件解析失败，回退默认配置");
                self.default_config()
            }
        }
    }

    /// 原子写：先写临时文件再 rename 覆盖，失败清理临时文件。
    pub fn save(&self, cfg: &GuiConfig) -> Result<(), String> {
        let _io = self.io_lock.lock().expect("配置写盘锁中毒");
        let path = self.path();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                warn!(error = %e, path = %parent.display(), "创建配置目录失败");
                return Err(format!("创建配置目录失败: {e}"));
            }
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(cfg).map_err(|e| format!("配置序列化失败: {e}"))?;
        if let Err(e) = fs::write(&tmp, text) {
            let _ = fs::remove_file(&tmp);
            warn!(error = %e, path = %tmp.display(), "写入临时配置失败");
            return Err(format!("写入配置失败: {e}"));
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            warn!(error = %e, path = %path.display(), "替换配置文件失败");
            return Err(format!("保存配置失败: {e}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
