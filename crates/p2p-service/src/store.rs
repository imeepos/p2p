//! 存储（service-registry-design §3/§4）：`<data-dir>/services.json`，
//! version=1 信封，tmp+rename 原子写（沿 p2p-authz store 语义）。
//! 缺失文件 = 空表首用态；损坏 / 版本不符 / 条目越闭集（serde 层拒）显式
//! 报错拒读，禁止静默回退空表（§4.2 红线）。data-dir 必须与 authz 数据根
//! 同源同根（§4.4，装配处传同一个 data_dir，禁止各面独立默认值）。

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::registry::{ServiceEntry, ServiceRegistry};

pub const SERVICES_FILE: &str = "services.json";
/// 当前信封版本；读到其他版本 = 显式拒读（升级路径显式化）。
pub const FILE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct ServicesEnvelope {
    version: u32,
    services: Vec<ServiceEntry>,
}

/// 存储错误：不可读 / 损坏（含条目越闭集）/ 版本不符，三类显式区分。
#[derive(Debug, thiserror::Error)]
pub enum ServiceStoreError {
    #[error("services.json 不可读: {0}")]
    Io(#[from] std::io::Error),
    #[error("services.json 损坏或条目越闭集，拒绝静默回退空表: {0}")]
    Corrupted(#[from] serde_json::Error),
    #[error("services.json 版本 {0} 不支持（当前支持 version={FILE_VERSION}）")]
    UnsupportedVersion(u32),
}

pub fn services_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SERVICES_FILE)
}

/// 读注册表：缺失 = 空表首用态；损坏/版本不符/条目越闭集显式报错。
pub fn load_registry(data_dir: &Path) -> Result<ServiceRegistry, ServiceStoreError> {
    let path = services_path(data_dir);
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ServiceRegistry::default()),
        Err(e) => return Err(e.into()),
    };
    let envelope: ServicesEnvelope = serde_json::from_slice(&raw)?;
    check_version(envelope.version)?;
    Ok(ServiceRegistry::new(envelope.services))
}

/// 原子写注册表：同目录临时文件 + sync + rename；失败错误上抛并清理临时文件。
pub fn save_registry(data_dir: &Path, registry: &ServiceRegistry) -> Result<(), ServiceStoreError> {
    let envelope = ServicesEnvelope {
        version: FILE_VERSION,
        services: registry.entries().to_vec(),
    };
    write_atomic(&services_path(data_dir), &envelope)
}

fn check_version(version: u32) -> Result<(), ServiceStoreError> {
    if version == FILE_VERSION {
        Ok(())
    } else {
        Err(ServiceStoreError::UnsupportedVersion(version))
    }
}

/// tmp + sync + rename 原子写：进程崩溃也不会留下半截正式文件；失败清理残留 .tmp。
fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), ServiceStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = write_and_sync(&tmp, json.as_bytes()) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn write_and_sync(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ServiceId;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 隔离临时目录（免 tempfile 依赖：进程内自增计数保证并发测试不互踩）。
    fn scratch_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "p2p-service-{}-{}-{}",
            tag,
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_is_empty_first_use() {
        let dir = scratch_dir("missing");
        let reg = load_registry(&dir).unwrap();
        assert_eq!(reg.entries().len(), 0);
        assert_eq!(reg.file_value(ServiceId::DISCOVERY_MDNS), None);
    }

    #[test]
    fn save_load_roundtrip_preserves_entries() {
        let dir = scratch_dir("roundtrip");
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::DISCOVERY_MDNS, false, 100);
        reg.set_enabled(ServiceId::SERVE_TUNNEL, true, 200);
        save_registry(&dir, &reg).unwrap();
        let loaded = load_registry(&dir).unwrap();
        assert_eq!(loaded, reg);
        assert_eq!(loaded.file_value(ServiceId::DISCOVERY_MDNS), Some(false));
        assert_eq!(loaded.file_value(ServiceId::SERVE_TUNNEL), Some(true));
    }

    #[test]
    fn corrupt_json_is_explicit_error_not_silent_fallback() {
        let dir = scratch_dir("corrupt");
        std::fs::write(services_path(&dir), "{ not json").unwrap();
        let err = load_registry(&dir).unwrap_err();
        assert!(
            matches!(err, ServiceStoreError::Corrupted(_)),
            "实际: {err:?}"
        );
    }

    #[test]
    fn unknown_service_id_entry_is_rejected_on_load() {
        let dir = scratch_dir("unknown-id");
        std::fs::write(
            services_path(&dir),
            r#"{"version":1,"services":[{"service_id":"owner.superuser","enabled":true,"updated_at":1}]}"#,
        )
        .unwrap();
        let err = load_registry(&dir).unwrap_err();
        assert!(
            matches!(err, ServiceStoreError::Corrupted(_)),
            "实际: {err:?}"
        );
    }

    #[test]
    fn unsupported_version_is_explicit_error() {
        let dir = scratch_dir("version");
        std::fs::write(services_path(&dir), r#"{"version":99,"services":[]}"#).unwrap();
        let err = load_registry(&dir).unwrap_err();
        assert!(
            matches!(err, ServiceStoreError::UnsupportedVersion(99)),
            "实际: {err:?}"
        );
    }

    #[test]
    fn envelope_format_is_version_one_with_minimal_fields() {
        let dir = scratch_dir("envelope");
        let mut reg = ServiceRegistry::default();
        reg.set_enabled(ServiceId::NET_LAN_ONLY, true, 7);
        save_registry(&dir, &reg).unwrap();
        let raw = std::fs::read_to_string(services_path(&dir)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["version"], 1);
        let services = value["services"].as_array().unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0]["service_id"], "net.lan_only");
        assert_eq!(services[0]["enabled"], true);
        assert_eq!(services[0]["updated_at"], 7);
        assert!(
            services[0].get("kind").is_none(),
            "描述/型别不进文件（plan §0.2）"
        );
    }

    #[test]
    fn no_tmp_leftbehind_after_successful_save() {
        let dir = scratch_dir("tmp");
        save_registry(&dir, &ServiceRegistry::default()).unwrap();
        assert!(services_path(&dir).exists());
        assert!(
            !dir.join("services.json.tmp").exists(),
            "成功写后不留 .tmp 残留"
        );
    }
}
