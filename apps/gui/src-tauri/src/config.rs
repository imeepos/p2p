//! GuiConfig 持久化（gui-contract.md §1/§3）：app 数据目录 gui-config.json，原子写（tmp+rename）。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use tracing::warn;

use crate::types::GuiConfig;

/// 配置文件名。
const FILE_NAME: &str = "gui-config.json";

/// 出厂内置云端 bootstrap（契约 v2：rendezvous，QUIC 语法）。
pub(crate) fn default_bootstrap() -> Vec<String> {
    vec!["43.240.223.138/u3400".into(), "121.196.193.177/u3400".into()]
}

/// 出厂内置云端中继（契约 v2：relay，QUIC 语法）。
pub(crate) fn default_relay_addrs() -> Vec<String> {
    vec!["43.240.223.138/u3403".into(), "121.196.193.177/u3403".into()]
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
        let text =
            serde_json::to_string_pretty(cfg).map_err(|e| format!("配置序列化失败: {e}"))?;
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
mod tests {
    use super::*;

    /// 独立临时目录：测试间互不污染，结束清理。
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("p2p-console-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    #[test]
    fn default_config_matches_contract() {
        let dir = temp_root("default");
        let store = ConfigStore::new(dir.join("app"));
        let cfg = store.default_config();
        assert_eq!(cfg.quic_port, 0);
        assert_eq!(cfg.tcp_port, 0);
        assert!(cfg.enable_mdns);
        assert_eq!(cfg.data_dir, dir.join("app").join("p2p-data").to_string_lossy());
        assert_eq!(
            cfg.bootstrap,
            vec!["43.240.223.138/u3400", "121.196.193.177/u3400"]
        );
        assert_eq!(
            cfg.relay_addrs,
            vec!["43.240.223.138/u3403", "121.196.193.177/u3403"]
        );
        assert!(cfg.advertised_addrs.is_empty());
        assert_eq!(cfg.observation_port, None);
        assert_eq!(cfg.observation_addrs, vec!["121.196.193.177:3402"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_default() {
        let dir = temp_root("missing");
        let store = ConfigStore::new(dir.join("app"));
        assert_eq!(store.load(), store.default_config());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_roundtrip_is_atomic_and_stable() {
        let dir = temp_root("roundtrip");
        let store = ConfigStore::new(dir.join("app"));
        let mut cfg = store.default_config();
        cfg.quic_port = 3400;
        cfg.tcp_port = 3401;
        cfg.bootstrap = vec!["1.2.3.4/3400".into(), "5.6.7.8/t3401".into()];
        cfg.relay_addrs = vec!["1.2.3.4/3400".into()];
        cfg.advertised_addrs = vec!["9.9.9.9/4000".into()];
        cfg.observation_port = Some(3402);
        cfg.observation_addrs = vec!["1.2.3.4:3402".into()];
        store.save(&cfg).expect("保存配置");
        // 临时文件已被 rename 消费，不留残骸
        assert!(!dir.join("app").join("gui-config.json.tmp").exists());
        assert_eq!(store.load(), cfg);
        // 二次保存覆盖旧值
        cfg.quic_port = 0;
        store.save(&cfg).expect("再次保存");
        assert_eq!(store.load(), cfg);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_config_fills_missing_fields_with_defaults() {
        let dir = temp_root("partial");
        let app = dir.join("app");
        fs::create_dir_all(&app).expect("创建 app 目录");
        // 旧版配置：只有用户改过的字段，缺端点/端口字段
        fs::write(
            app.join("gui-config.json"),
            serde_json::json!({ "enableMdns": false, "quicPort": 3400 }).to_string(),
        )
        .expect("写入部分配置");
        let store = ConfigStore::new(app.clone());
        let cfg = store.load();
        assert!(!cfg.enable_mdns, "用户已有字段不得被默认覆盖");
        assert_eq!(cfg.quic_port, 3400, "用户已有字段不得被默认覆盖");
        assert_eq!(
            cfg.bootstrap,
            crate::config::default_bootstrap(),
            "缺失字段应补出厂默认端点"
        );
        assert_eq!(cfg.relay_addrs, crate::config::default_relay_addrs());
        assert_eq!(
            cfg.observation_addrs,
            crate::config::default_observation_addrs()
        );
        assert_eq!(cfg.tcp_port, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_explicit_empty_lists_not_overridden_by_defaults() {
        let dir = temp_root("explicit-empty");
        let store = ConfigStore::new(dir.join("app"));
        let mut cfg = store.default_config();
        cfg.bootstrap = Vec::new();
        cfg.observation_addrs = Vec::new();
        store.save(&cfg).expect("保存用户显式空列表");
        let loaded = store.load();
        assert!(loaded.bootstrap.is_empty(), "用户显式空列表不得补默认");
        assert!(loaded.observation_addrs.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_file_falls_back_to_default_with_warning() {
        let dir = temp_root("corrupt");
        let app = dir.join("app");
        fs::create_dir_all(&app).expect("创建 app 目录");
        fs::write(app.join("gui-config.json"), "{ not json").expect("写入坏文件");
        let store = ConfigStore::new(app);
        assert_eq!(store.load(), store.default_config());
        let _ = fs::remove_dir_all(&dir);
    }
}
