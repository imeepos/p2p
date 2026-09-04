//! 出借方 allowlist（G4 准入）：借方 PeerId 白名单 + 可选模型白名单。
//! 文件 <data-dir>/llm-share/allowlist.json；缺失视为空表（首授场景），
//! 损坏显式报错；语义默认拒绝——表无条目即不可用。
//! allow=upsert（granted_at 每次刷新），deny=删条目（不存在明确报错）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::file_path;
use super::read_json_or_none;
use super::validate_peer_id;
use super::write_json_atomic;

pub const FILE_NAME: &str = "allowlist.json";
const FORMAT_VERSION: u8 = 1;

/// 单个借方条目：models 为空 = 不限模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowEntry {
    pub models: Vec<String>,
    #[serde(default)]
    pub note: String,
    pub granted_at: String,
}

/// allowlist 落盘形态：BTreeMap 键序稳定，条目输出可复现。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowlistFile {
    pub v: u8,
    pub entries: BTreeMap<String, AllowEntry>,
}

impl AllowlistFile {
    pub fn new() -> Self {
        Self {
            v: FORMAT_VERSION,
            entries: BTreeMap::new(),
        }
    }

    /// upsert：返回是否为新建条目（false = 已存在，本次为更新）。
    pub fn upsert(
        &mut self,
        peer_id: &str,
        models: Vec<String>,
        note: &str,
        granted_at: &str,
    ) -> bool {
        let created = !self.entries.contains_key(peer_id);
        self.entries.insert(
            peer_id.to_owned(),
            AllowEntry {
                models,
                note: note.to_owned(),
                granted_at: granted_at.to_owned(),
            },
        );
        created
    }

    pub fn remove(&mut self, peer_id: &str) -> bool {
        self.entries.remove(peer_id).is_some()
    }
}

pub fn path(data_dir: &str) -> PathBuf {
    file_path(data_dir, FILE_NAME)
}

/// 读 allowlist：缺失视为空表；损坏/读取失败显式报错。
pub fn load_or_empty(path: &Path) -> Result<AllowlistFile, String> {
    match read_json_or_none(path, "allowlist")? {
        Some(list) => Ok(list),
        None => Ok(AllowlistFile::new()),
    }
}

pub fn save(path: &Path, list: &AllowlistFile) -> Result<(), String> {
    write_json_atomic(path, list, "allowlist")
}

/// --model 白名单规整：trim、去空、去重保序；空结果 = 不限模型。
pub fn normalize_models(raw: &[String]) -> Result<Vec<String>, String> {
    let mut models: Vec<String> = Vec::with_capacity(raw.len());
    for value in raw {
        let model = value.trim();
        if model.is_empty() {
            return Err("--model 模型名不能为空".to_owned());
        }
        if !models.iter().any(|known| known == model) {
            models.push(model.to_owned());
        }
    }
    Ok(models)
}

/// allow 报告：created=false 表示条目已存在（本次为 upsert 更新）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowReport {
    pub created: bool,
    pub peer_id: String,
    pub models: Vec<String>,
    pub note: String,
    pub granted_at: String,
}

/// deny 报告：removed 恒 true（条目不存在时命令已报错退出）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DenyReport {
    pub removed: bool,
    pub peer_id: String,
}

/// allowlist 条目视图。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowlistEntry {
    pub peer_id: String,
    pub models: Vec<String>,
    pub note: String,
    pub granted_at: String,
}

/// allowlist 查询报告。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowlistReport {
    pub peers: Vec<AllowlistEntry>,
}

/// allow 主流程：校验 → 读表 → upsert → 原子写回。
pub fn allow(
    data_dir: &str,
    peer_id: &str,
    models_raw: &[String],
    note: Option<&str>,
    granted_at: &str,
) -> Result<AllowReport, String> {
    validate_peer_id(peer_id)?;
    let models = normalize_models(models_raw)?;
    let note = note.unwrap_or_default();
    let file = path(data_dir);
    let mut list = load_or_empty(&file)?;
    let created = list.upsert(peer_id, models.clone(), note, granted_at);
    save(&file, &list)?;
    Ok(AllowReport {
        created,
        peer_id: peer_id.to_owned(),
        models,
        note: note.to_owned(),
        granted_at: granted_at.to_owned(),
    })
}

/// deny 主流程：校验 → 读表 → 删条目 → 原子写回；不存在明确报错。
pub fn deny(data_dir: &str, peer_id: &str) -> Result<DenyReport, String> {
    validate_peer_id(peer_id)?;
    let file = path(data_dir);
    let mut list = load_or_empty(&file)?;
    if !list.remove(peer_id) {
        return Err(format!(
            "allowlist 无该借方条目：{peer_id}（本就默认拒绝，无需 deny）"
        ));
    }
    save(&file, &list)?;
    Ok(DenyReport {
        removed: true,
        peer_id: peer_id.to_owned(),
    })
}

/// allowlist 查询主流程：缺失视为空表（默认拒绝语义提示）。
pub fn list(data_dir: &str) -> Result<AllowlistReport, String> {
    let list = load_or_empty(&path(data_dir))?;
    Ok(AllowlistReport {
        peers: list
            .entries
            .into_iter()
            .map(|(peer_id, entry)| AllowlistEntry {
                peer_id,
                models: entry.models,
                note: entry.note,
                granted_at: entry.granted_at,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pcli-allow-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn roundtrip_and_upsert_semantics() {
        let dir = temp_dir("roundtrip");
        let file = dir.join(FILE_NAME);
        let peer = bs58::encode([1u8; 32]).into_string();
        let mut list = AllowlistFile::new();
        assert!(list.upsert(&peer, vec!["gpt-4o".into()], "n", "2026-09-04T00:00:00Z"));
        assert!(!list.upsert(&peer, vec![], "", "2026-09-04T01:00:00Z"));
        save(&file, &list).unwrap();
        assert!(!file.with_extension("json.tmp").exists());
        let loaded = load_or_empty(&file).unwrap();
        assert_eq!(loaded.v, FORMAT_VERSION);
        assert_eq!(loaded.entries[&peer].models, Vec::<String>::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_empty_and_corrupt_errors() {
        let dir = temp_dir("missing");
        let file = dir.join(FILE_NAME);
        assert!(load_or_empty(&file).unwrap().entries.is_empty());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, "{ not json").unwrap();
        assert!(load_or_empty(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn deny_missing_entry_is_explicit_error() {
        let dir = temp_dir("deny");
        let peer = bs58::encode([2u8; 32]).into_string();
        assert!(deny(dir.to_str().unwrap(), &peer).is_err());
        allow(
            dir.to_str().unwrap(),
            &peer,
            &[],
            None,
            "2026-09-04T00:00:00Z",
        )
        .unwrap();
        assert!(deny(dir.to_str().unwrap(), &peer).unwrap().removed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn allow_validates_peer_and_models() {
        let dir = temp_dir("validate");
        assert!(allow(dir.to_str().unwrap(), "bad-peer", &[], None, "t").is_err());
        let peer = bs58::encode([3u8; 32]).into_string();
        assert!(allow(dir.to_str().unwrap(), &peer, &["  ".to_owned()], None, "t").is_err());
        let report = allow(
            dir.to_str().unwrap(),
            &peer,
            &[" gpt-4o ".to_owned(), "gpt-4o".to_owned()],
            Some("nb"),
            "t",
        )
        .unwrap();
        assert_eq!(report.models, vec!["gpt-4o".to_owned()]);
    }

    #[test]
    fn list_reports_entries_in_stable_order() {
        let dir = temp_dir("list");
        let a = bs58::encode([4u8; 32]).into_string();
        let b = bs58::encode([5u8; 32]).into_string();
        allow(dir.to_str().unwrap(), &b, &[], None, "t").unwrap();
        allow(dir.to_str().unwrap(), &a, &[], None, "t").unwrap();
        let report = list(dir.to_str().unwrap()).unwrap();
        assert_eq!(report.peers.len(), 2);
        assert_eq!(report.peers[0].peer_id, a, "BTreeMap 序稳定输出");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
