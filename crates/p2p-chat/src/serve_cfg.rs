//! serve 端口记忆（F1）：chat serve 首启随机端口绑定成功后落盘 serve.json，
//! 重启沿用上次端口消除端口漂移；--quic-port 显式指定时优先并覆盖记忆值。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store_io::atomic_write;

/// 端口记忆文件名（chat 数据目录下）。
const SERVE_FILE: &str = "serve.json";

#[derive(Serialize, Deserialize)]
struct ServeConfig {
    #[serde(rename = "quicPort")]
    quic_port: u16,
}

fn serve_path(data_dir: &Path) -> PathBuf {
    data_dir.join("chat").join(SERVE_FILE)
}

/// 读记忆端口：文件缺失 → None；损坏留 warn 按 None 处理（不静默吞）。
pub fn load_serve_port(data_dir: &Path) -> Option<u16> {
    let content = match std::fs::read_to_string(serve_path(data_dir)) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(
                path = %serve_path(data_dir).display(),
                error = %e,
                "serve 端口记忆读取失败，按未记忆处理"
            );
            return None;
        }
    };
    match serde_json::from_str::<ServeConfig>(&content) {
        Ok(cfg) => Some(cfg.quic_port),
        Err(e) => {
            tracing::warn!(
                path = %serve_path(data_dir).display(),
                error = %e,
                "serve 端口记忆损坏，按未记忆处理"
            );
            None
        }
    }
}

/// 写记忆端口（serve 绑定成功后调用）：失败经 Err 上抛由调用方留观测。
pub fn save_serve_port(data_dir: &Path, port: u16) -> std::io::Result<()> {
    let cfg = ServeConfig { quic_port: port };
    let bytes = serde_json::to_vec_pretty(&cfg).map_err(std::io::Error::other)?;
    atomic_write(&serve_path(data_dir), &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_roundtrips_port() {
        let dir = std::env::temp_dir().join(format!("pr1-serve-cfg-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("chat")).unwrap();
        assert_eq!(load_serve_port(&dir), None, "未写记忆前为 None");
        save_serve_port(&dir, 45123).unwrap();
        assert_eq!(load_serve_port(&dir), Some(45123));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_memory_file_reads_none() {
        let dir = std::env::temp_dir().join(format!("pr1-serve-bad-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("chat")).unwrap();
        std::fs::write(dir.join("chat").join(SERVE_FILE), "{broken").unwrap();
        assert_eq!(load_serve_port(&dir), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
