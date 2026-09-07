//! 出借方 allowlist（G4 准入）：借方 PeerId 白名单 + 可选模型白名单。
//! 文件 <data-dir>/llm-share/allowlist.json；缺失视为空表（首授场景），
//! 损坏显式报错；语义默认拒绝——表无条目即不可用。
//! allow=upsert（granted_at 每次刷新），deny=删条目（不存在明确报错）。
//! v2（llm-share-link 设计 §4-3）：条目增 source（"share:<shareId>"）与 expires_at，
//! 供分享兑换落条目与 revoke 按 source 级联删除（手工条目不受级联）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::file_path;
use super::read_json_or_none;
use super::validate_peer_id;
use super::write_json_atomic;

pub const FILE_NAME: &str = "allowlist.json";
const FORMAT_VERSION: u8 = 1;

/// 单个借方条目：models 为空 = 不限模型；source/expires_at 为分享兑换注入口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowEntry {
    pub models: Vec<String>,
    #[serde(default)]
    pub note: String,
    /// 条目来源（"share:<shareId>"）；None = 手工条目，不受分享撤销级联。
    #[serde(default)]
    pub source: Option<String>,
    /// 授权到期（Unix 秒）；None = 不过期。
    #[serde(default)]
    pub expires_at: Option<u64>,
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
        source: Option<&str>,
        expires_at: Option<u64>,
        granted_at: &str,
    ) -> bool {
        let created = !self.entries.contains_key(peer_id);
        self.entries.insert(
            peer_id.to_owned(),
            AllowEntry {
                models,
                note: note.to_owned(),
                source: source.map(str::to_owned),
                expires_at,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
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
    source: Option<&str>,
    expires_at: Option<u64>,
    granted_at: &str,
) -> Result<AllowReport, String> {
    validate_peer_id(peer_id)?;
    let models = normalize_models(models_raw)?;
    let note = note.unwrap_or_default();
    let file = path(data_dir);
    let mut list = load_or_empty(&file)?;
    let created = list.upsert(peer_id, models.clone(), note, source, expires_at, granted_at);
    save(&file, &list)?;
    Ok(AllowReport {
        created,
        peer_id: peer_id.to_owned(),
        models,
        note: note.to_owned(),
        source: source.map(str::to_owned),
        expires_at,
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
                source: entry.source,
                expires_at: entry.expires_at,
                granted_at: entry.granted_at,
            })
            .collect(),
    })
}

/// 按来源删除条目（分享撤销级联）：只删 source 匹配项，手工条目不动；
/// 返回移除数，>0 才落盘。
pub fn remove_by_source(data_dir: &str, source: &str) -> Result<usize, String> {
    let file = path(data_dir);
    let mut list = load_or_empty(&file)?;
    let before = list.entries.len();
    list.entries.retain(|_, entry| entry.source.as_deref() != Some(source));
    let removed = before - list.entries.len();
    if removed > 0 {
        save(&file, &list)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests;
