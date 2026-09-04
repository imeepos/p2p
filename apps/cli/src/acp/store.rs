//! acp 域存储面：策略表路径派生、文件读写与授时。
//! 存取全部复用 acp-common（serde + tmp/rename 原子写，禁止复制实现）；
//! 语义边界：文件缺失视为空表（首授/首列场景），损坏与版本不符显式报错，
//! 禁止静默回退空表——默认拒绝不等于吞存储故障。

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use acp_common::paths::AcpPaths;
use acp_common::policy::{PolicyStoreError, PolicyTable};

use crate::error::{CliError, CliResult};

/// 策略表文件：<data-dir>/acp-policy.json（与 acp-agent 同一 AcpPaths 约定）。
pub fn policy_path(data_dir: &str) -> PathBuf {
    AcpPaths::new(data_dir).policy()
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

/// 当前 UTC 时刻的 RFC 3339 秒级字符串（policy.rs 约定 granted_at 由调用方注入，
/// 本函数是 CLI 侧的注入口；无外部时钟依赖，缺时钟回落 epoch 不 panic）。
pub fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// Unix 秒 → RFC 3339（UTC，秒级，恒 Z 后缀）。
fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

/// Howard Hinnant civil_from_days：Unix 天数 → (年, 月, 日)，纯整数历法换算。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_timestamps() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(rfc3339_from_unix(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn now_string_is_utc_second_precision() {
        let now = rfc3339_now();
        assert!(now.ends_with('Z'), "应为 UTC Z 后缀: {now}");
        assert_eq!(now.len(), 20, "应为秒级 RFC 3339: {now}");
    }
}
