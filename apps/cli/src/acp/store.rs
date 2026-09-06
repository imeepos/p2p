//! acp 域存储面：策略表路径派生、文件读写与授时。
//! 存取全部复用 acp-common（serde + tmp/rename 原子写，禁止复制实现）；
//! 语义边界：文件缺失视为空表（首授/首列场景），损坏与版本不符显式报错，
//! 禁止静默回退空表——默认拒绝不等于吞存储故障。

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use acp_common::paths::AcpPaths;
use acp_common::policy::{PolicyStoreError, PolicyTable};
use acp_common::share::{ShareLedger, ShareStoreError};

use crate::error::{CliError, CliResult};

/// 策略表文件：<data-dir>/acp-policy.json（与 acp-agent 同一 AcpPaths 约定）。
pub fn policy_path(data_dir: &str) -> PathBuf {
    AcpPaths::new(data_dir).policy()
}

/// 分享台账文件：<data-dir>/acp-shares.json（acp-share 设计 §3）。
pub fn shares_path(data_dir: &str) -> PathBuf {
    AcpPaths::new(data_dir).shares()
}

/// 读分享台账：缺失视为空账；损坏/版本不符上抛为可读运行失败（退出码 1）。
pub fn load_shares_or_empty(path: &Path) -> CliResult<ShareLedger> {
    match ShareLedger::load(path) {
        Ok(ledger) => Ok(ledger),
        Err(ShareStoreError::Io(e)) if e.kind() == ErrorKind::NotFound => Ok(ShareLedger::new()),
        Err(e) => Err(CliError::Runtime(format!("分享台账读取失败: {e}"))),
    }
}

/// 分享台账原子写回（acp-common tmp+rename）；父目录缺失先建（首创建场景）。
pub fn save_shares(path: &Path, ledger: &ShareLedger) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError::Runtime(format!("分享台账目录创建失败: {e}")))?;
    }
    ledger
        .save(path)
        .map_err(|e| CliError::Runtime(format!("分享台账写入失败: {e}")))
}

/// 读策略表：缺失视为空表；损坏/版本不符上抛为可读运行失败（退出码 1）。
pub fn load_or_empty(path: &Path) -> CliResult<PolicyTable> {
    match PolicyTable::load(path) {
        Ok(table) => Ok(table),
        Err(PolicyStoreError::Io(e)) if e.kind() == ErrorKind::NotFound => Ok(PolicyTable::new()),
        Err(e) => Err(CliError::Runtime(format!("策略表读取失败: {e}"))),
    }
}

/// 原子写回（acp-common tmp+rename）；父目录缺失先建（首授场景目录可能为空）。
pub fn save(path: &Path, table: &PolicyTable) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError::Runtime(format!("策略表目录创建失败: {e}")))?;
    }
    table
        .save(path)
        .map_err(|e| CliError::Runtime(format!("策略表写入失败: {e}")))
}

/// 当前 UTC 时刻的 RFC 3339 秒级字符串（acp-common 统一实现，CLI 侧注入口）。
pub fn rfc3339_now() -> String {
    acp_common::share::rfc3339_from_unix(acp_common::share::unix_now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_string_is_utc_second_precision() {
        let now = rfc3339_now();
        assert!(now.ends_with('Z'), "应为 UTC Z 后缀: {now}");
        assert_eq!(now.len(), 20, "应为秒级 RFC 3339: {now}");
    }

    #[test]
    fn shares_helpers_missing_is_empty_and_roundtrip() {
        let dir = std::env::temp_dir().join(format!("p2pctl-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = shares_path(dir.to_str().expect("utf8"));
        let ledger = load_shares_or_empty(&path).expect("missing = empty");
        assert_eq!(ledger.iter().count(), 0);
        save_shares(&path, &ledger).expect("save empty ledger");
        let reloaded = load_shares_or_empty(&path).expect("reload");
        assert_eq!(reloaded.iter().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
