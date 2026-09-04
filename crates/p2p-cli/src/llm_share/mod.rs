//! llm-share 管理面逻辑（T21，idle-token-sharing-plan §3.3/§5.1/§5.2/§6 末行）：
//! 出借方 allowlist、能力声明签发/查看、双边流水查询、收据离线验签的纯逻辑层，
//! clap 命令面在 apps/cli/src/llm_share/。数据落盘 <data-dir>/llm-share/ 下
//! allowlist.json / offer.json / ledger.json，写路径一律 tmp+rename 原子落盘，
//! 缺失与损坏路径显式报错禁止静默。出借方签名密钥 = 节点身份种子
//! （p2p-identity::load_seed，0600 标准），本模块不新增任何密钥落盘。

pub mod allowlist;
pub mod ledger;
pub mod offer;
pub mod receipt;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// llm-share 域数据子目录。
pub const DIR_NAME: &str = "llm-share";

pub(crate) fn file_path(data_dir: &str, file_name: &str) -> PathBuf {
    Path::new(data_dir).join(DIR_NAME).join(file_name)
}

/// PeerId 校验：base58 解码后恰 32 字节（对齐 acp/chat 域同一语义）。
pub fn validate_peer_id(peer_id: &str) -> Result<(), String> {
    let decoded = bs58::decode(peer_id)
        .into_vec()
        .map_err(|_| format!("PeerId 非法（不是合法 base58）：{peer_id}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "PeerId 非法（解码后应恰 32 字节，实得 {}）：{peer_id}",
            decoded.len()
        ));
    }
    Ok(())
}

/// 当前 Unix 秒；缺时钟回落 0 不 panic（对齐 acp 域授时口径）。
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// JSON 读：缺失返回 None，读取/解析失败显式报错（禁止静默回退空数据）。
pub(crate) fn read_json_or_none<T: DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<Option<T>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{label}读取失败（{}）: {e}", path.display())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("{label}损坏（{}）: {e}", path.display()))
}

/// JSON 原子写：tmp + rename，失败清理临时文件并返回可读错误。
pub(crate) fn write_json_atomic<T: Serialize>(
    path: &Path,
    value: &T,
    label: &str,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{label}目录创建失败: {e}"))?;
    }
    let text =
        serde_json::to_string_pretty(value).map_err(|e| format!("{label}序列化失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    let written = std::fs::write(&tmp, &text).and_then(|()| std::fs::rename(&tmp, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{label}保存失败（{}）: {e}", path.display()));
    }
    Ok(())
}

/// "model=N" 键值对解析：trim、非空键、u64 值；what 用于错误定位。
pub(crate) fn parse_model_u64(raw: &[String], what: &str) -> Result<Vec<(String, u64)>, String> {
    raw.iter()
        .map(|item| {
            let (model, value) = item
                .rsplit_once('=')
                .ok_or_else(|| format!("{what} 参数格式非法（应为 model=N）：{item}"))?;
            let model = model.trim();
            if model.is_empty() {
                return Err(format!("{what} 模型名不能为空：{item}"));
            }
            let value: u64 = value
                .trim()
                .parse()
                .map_err(|_| format!("{what} 数值非法（应为非负整数）：{item}"))?;
            Ok((model.to_owned(), value))
        })
        .collect()
}

/// 键值对入映射：重复键显式报错（静默覆盖会让限额声明失真）。
pub(crate) fn pairs_to_map(
    pairs: Vec<(String, u64)>,
    what: &str,
) -> Result<BTreeMap<String, u64>, String> {
    let mut map = BTreeMap::new();
    for (key, value) in pairs {
        if map.insert(key.clone(), value).is_some() {
            return Err(format!("{what} 模型 {key} 重复声明"));
        }
    }
    Ok(map)
}

/// YYYY-MM-DD 账期截止日校验（宽松历法：月 1-12、日 1-31）。
pub(crate) fn validate_date_ymd(date: &str) -> Result<(), String> {
    let invalid = || format!("日期格式非法（应为 YYYY-MM-DD）：{date}");
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    let (y, m, d) = (
        parts[0].parse::<i64>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    );
    let (y, m, d) = match (y, m, d) {
        (Ok(y), Ok(m), Ok(d)) => (y, m, d),
        _ => return Err(invalid()),
    };
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(invalid());
    }
    Ok(())
}

/// 当前 UTC 时刻的 RFC 3339 秒级字符串（allowlist granted_at 注入口）。
pub fn rfc3339_now() -> String {
    rfc3339_at(now_secs())
}

/// rfc3339_now 的可测内核（注入秒数）。
fn rfc3339_at(secs: u64) -> String {
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

/// Howard Hinnant civil_from_days：Unix 天数 → (年, 月, 日)。
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
    (if month <= 2 { year + 1 } else { year }, month as u32, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_id_validation_rejects_bad_input() {
        assert!(validate_peer_id("not-base58!").is_err());
        assert!(validate_peer_id(&"1".repeat(43)).is_err());
        let valid = bs58::encode([7u8; 32]).into_string();
        assert!(validate_peer_id(&valid).is_ok());
    }

    #[test]
    fn model_kv_parses_and_rejects_bad_pairs() {
        let raw = vec![" gpt-4o = 150 ".to_owned(), "deepseek-v3=0".to_owned()];
        let pairs = parse_model_u64(&raw, "--spare").unwrap();
        assert_eq!(pairs[0], ("gpt-4o".to_owned(), 150));
        assert_eq!(pairs[1], ("deepseek-v3".to_owned(), 0));
        assert!(parse_model_u64(&["nonsense".to_owned()], "--spare").is_err());
        assert!(parse_model_u64(&["=5".to_owned()], "--spare").is_err());
        assert!(parse_model_u64(&["m=x".to_owned()], "--spare").is_err());
    }

    #[test]
    fn pairs_to_map_rejects_duplicates() {
        let pairs = vec![("m".to_owned(), 1u64), ("m".to_owned(), 2u64)];
        assert!(pairs_to_map(pairs, "--spare").is_err());
    }

    #[test]
    fn date_validation_bounds() {
        assert!(validate_date_ymd("2026-09-30").is_ok());
        assert!(validate_date_ymd("2026-13-01").is_err());
        assert!(validate_date_ymd("2026-09").is_err());
        assert!(validate_date_ymd("bad").is_err());
    }

    #[test]
    fn rfc3339_known_timestamps() {
        assert_eq!(rfc3339_at(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_at(1_709_164_800), "2024-02-29T00:00:00Z");
    }
}
