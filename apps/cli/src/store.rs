//! 持久化读写（形态与 GUI config.rs/profile.rs 一致）：无文件回默认、
//! 损坏回默认并 stderr 告警（禁止静默）、原子写 tmp+rename、失败清理临时文件。

use std::fs;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::CliError;
use crate::paths::Paths;
use crate::types::{GuiConfig, NodeProfile};

fn warn_fallback(path: &Path, stage: &str, e: &dyn std::fmt::Display) {
    eprintln!("p2pctl: {stage}失败（{}）：{e}，回退默认值", path.display());
}

/// 读 JSON 文件：缺失或损坏回退默认值（损坏留告警，可观测）。
fn load_or_default<T: DeserializeOwned + Default>(path: &Path, label: &str) -> T {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return T::default(),
        Err(e) => {
            warn_fallback(path, &format!("读取{label}"), &e);
            return T::default();
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(e) => {
            warn_fallback(path, &format!("解析{label}"), &e);
            T::default()
        }
    }
}

/// 原子写：tmp + rename，失败清理临时文件并返回可读错误。
fn save_atomic<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::Runtime(format!("创建{label}目录失败: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| CliError::Runtime(format!("{label}序列化失败: {e}")))?;
    if let Err(e) = fs::write(&tmp, &text) {
        let _ = fs::remove_file(&tmp);
        return Err(CliError::Runtime(format!("写入{label}失败: {e}")));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(CliError::Runtime(format!("保存{label}失败: {e}")));
    }
    Ok(())
}

/// 读持久化配置；dataDir 缺省补 <data-dir>/p2p-data（GUI default_config 等价）。
pub fn load_config(paths: &Paths) -> GuiConfig {
    let mut cfg: GuiConfig = load_or_default(&paths.config(), "配置");
    if cfg.data_dir.is_empty() {
        cfg.data_dir = paths.default_node_data_dir().to_string_lossy().into_owned();
    }
    cfg
}

/// 读节点资料。
pub fn load_profile(paths: &Paths) -> NodeProfile {
    load_or_default(&paths.profile(), "节点资料")
}

/// 原子保存配置。
pub fn save_config(paths: &Paths, cfg: &GuiConfig) -> Result<(), CliError> {
    save_atomic(&paths.config(), cfg, "配置")
}

/// 原子保存节点资料。
pub fn save_profile(paths: &Paths, profile: &NodeProfile) -> Result<(), CliError> {
    save_atomic(&paths.profile(), profile, "节点资料")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(tag: &str) -> Paths {
        let dir = std::env::temp_dir().join(format!("p2pctl-store-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Paths::new(dir.to_str().unwrap())
    }

    #[test]
    fn missing_config_loads_default_with_data_dir_filled() {
        let paths = temp_paths("missing");
        let cfg = load_config(&paths);
        assert!(cfg.enable_mdns);
        assert_eq!(
            cfg.data_dir,
            paths.default_node_data_dir().to_string_lossy()
        );
        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn config_roundtrip_preserves_user_fields() {
        let paths = temp_paths("roundtrip");
        let mut cfg = load_config(&paths);
        cfg.quic_port = 3400;
        cfg.bootstrap = vec!["1.2.3.4/u3400".into()];
        save_config(&paths, &cfg).unwrap();
        assert!(!paths.config().with_extension("json.tmp").exists());
        let loaded = load_config(&paths);
        assert_eq!(loaded.quic_port, 3400);
        assert_eq!(loaded.bootstrap, vec!["1.2.3.4/u3400".to_string()]);
        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn corrupted_config_falls_back_with_default() {
        let paths = temp_paths("corrupt");
        paths.ensure_dir().unwrap();
        fs::write(paths.config(), "{ not json").unwrap();
        let cfg = load_config(&paths);
        assert_eq!(
            cfg,
            GuiConfig {
                data_dir: cfg.data_dir.clone(),
                ..GuiConfig::default()
            }
        );
        let _ = fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn profile_missing_loads_default_and_roundtrips() {
        let paths = temp_paths("profile");
        assert_eq!(load_profile(&paths), NodeProfile::default());
        let p = NodeProfile {
            name: "家用节点".into(),
            description: "测试".into(),
            avatar: None,
        };
        save_profile(&paths, &p).unwrap();
        assert_eq!(load_profile(&paths), p);
        let _ = fs::remove_dir_all(&paths.root);
    }
}
